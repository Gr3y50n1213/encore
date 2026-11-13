"""Compare audio embedding models on the fetched clips: how often a clip's nearest neighbours
share its genre (a stand-in for "sounds like"), how fast each model is, and how big.

usage: python embed_eval.py FMA_DIR RESULTS_JSON openl3|yamnet
Models: YAMNet and VGGish (Apache-2.0, Google) and OpenL3 (CC BY 4.0 weights, MIT code), each
best-effort: one that cannot load is reported and skipped (docs/adr/core-0019).
"""
import json
import subprocess
import sys
import time

import numpy as np
import pandas as pd

RATE = 16_000


def decode(path):
    """The clip as mono 16 kHz float samples, through ffmpeg."""
    out = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", path, "-ac", "1", "-ar", str(RATE), "-f", "f32le", "-"],
        capture_output=True, check=True,
    ).stdout
    return np.frombuffer(out, dtype=np.float32).copy()


def load_models(which):
    models = {}
    if which == "openl3":
        import openl3

        net = openl3.models.load_audio_embedding_model("mel256", "music", 512)
        # The hop between 1 s patches: a longer hop analyses fewer patches, costing less on a phone.
        for hop in (1.0, 2.0, 3.0):
            models[f"openl3-music hop {hop:g} s (CC BY 4.0)"] = lambda w, hop=hop: openl3.get_audio_embedding(
                w, RATE, model=net, hop_size=hop, verbose=False)[0].mean(0)
    elif which == "yamnet":
        import urllib.request

        import tensorflow as tf

        # Google's YAMNet (Apache-2.0) as the small TFLite files MediaPipe publishes: the embedding
        # model first (1024 numbers per 0.975 s), else the classifier (521 scores).
        base = "https://storage.googleapis.com/mediapipe-models/audio_"
        urls = [base + "embedder/yamnet_embedding_metadata/float32/1/yamnet_embedding.tflite",
                base + "classifier/yamnet/float32/1/yamnet.tflite"]
        interp = None
        for url in urls:
            try:
                urllib.request.urlretrieve(url, "yamnet.tflite")
                interp = tf.lite.Interpreter(model_path="yamnet.tflite", experimental_preserve_all_tensors=True)
                interp.allocate_tensors()
                print("yamnet from", url, "input", interp.get_input_details()[0]["shape"],
                      "outputs", [o["shape"].tolist() for o in interp.get_output_details()], flush=True)
                break
            except Exception as e:  # noqa: BLE001
                print("yamnet file failed:", url, repr(e)[:200])
        if interp is None:
            raise RuntimeError("no YAMNet file could be loaded")
        inp = interp.get_input_details()[0]
        frame = int(np.prod(inp["shape"]))

        def embed(w, seconds, pooled):
            """Mean over the 0.975 s patches of `seconds` taken from the middle of the clip: the 521
            class scores, or (pooled) the 1024 numbers before the last layer when the file exposes them."""
            if seconds:
                mid, half = len(w) // 2, int(seconds * RATE / 2)
                w = w[max(0, mid - half):mid + half]
            vecs = []
            for start in range(0, max(1, len(w) - frame + 1), frame // 2):
                chunk = w[start:start + frame]
                if len(chunk) < frame:
                    break
                interp.set_tensor(inp["index"], chunk.reshape(inp["shape"]).astype(np.float32))
                interp.invoke()
                if pooled:
                    cands = [d for d in interp.get_tensor_details() if list(d["shape"]) in ([1, 1024], [1, 1, 1, 1024])]
                    if not cands:
                        raise RuntimeError("no 1024-wide tensor to read")
                    vecs.append(interp.get_tensor(max(cands, key=lambda d: d["index"])["index"]).reshape(-1))
                else:
                    out = [interp.get_tensor(o["index"]) for o in interp.get_output_details()]
                    vecs.append(next(o for o in out if o.shape[-1] in (1024, 521)).reshape(-1))
            return np.mean(vecs, axis=0)

        for seconds in (30, 20, 10, 5):
            models[f"yamnet scores, {seconds} s (Apache-2.0)"] = lambda w, s=seconds: embed(w, s if s < 30 else 0, False)
        for seconds in (30, 10):
            models[f"yamnet 1024-wide, {seconds} s (Apache-2.0)"] = lambda w, s=seconds: embed(w, s if s < 30 else 0, True)    return models

def metrics(vectors, genre):
    v = np.array(vectors, dtype=float)

    def cosine(m):
        n = np.linalg.norm(m, axis=1, keepdims=True)
        n[n == 0] = 1
        u = m / n
        return u @ u.T

    def score(sim, k=5):
        s = sim.copy()
        np.fill_diagonal(s, -9)
        top = np.argsort(-s, axis=1)[:, :k]
        precision = float(np.mean(genre[top] == genre[:, None]))
        hits = 0
        for i in range(len(genre)):
            vals, counts = np.unique(genre[top[i]], return_counts=True)
            hits += vals[np.argmax(counts)] == genre[i]
        return precision, hits / len(genre)

    z = (v - v.mean(0)) / (v.std(0) + 1e-9)
    return {"raw": score(cosine(v)), "standardised": score(cosine(z))}


def main():
    root, out_path, which = sys.argv[1], sys.argv[2], sys.argv[3]
    labels = pd.read_csv(f"{root}/labels.csv", index_col=0)["genre"]
    genre = np.array(labels.tolist())
    waves = [decode(f"{root}/clips/{tid}.mp3") for tid in labels.index]
    print(f"{len(waves)} clips, {len(set(genre))} genres, chance {100 / len(set(genre)):.1f}%", flush=True)
    results = {}
    for name, fn in load_models(which).items():
        vectors, started = [], time.time()
        try:
            for w in waves:
                vectors.append(fn(w))
        except Exception as e:  # noqa: BLE001
            print(name, "failed:", repr(e)[:300])
            continue
        per_clip = (time.time() - started) / len(waves)
        m = metrics(vectors, genre)
        results[name] = {"seconds_per_clip": per_clip, "dims": len(vectors[0]), **{k: {"precision_at_5": v[0], "knn5_genre": v[1]} for k, v in m.items()}}
        print(f"{name:28s} dims {len(vectors[0]):5d}  {per_clip:5.2f} s/clip   "
              f"precision@5 {m['raw'][0] * 100:5.1f}% (standardised {m['standardised'][0] * 100:5.1f}%)   "
              f"5-NN genre {m['raw'][1] * 100:5.1f}% (standardised {m['standardised'][1] * 100:5.1f}%)", flush=True)
    with open(out_path, "w") as f:
        json.dump(results, f, indent=2)


if __name__ == "__main__":
    main()
