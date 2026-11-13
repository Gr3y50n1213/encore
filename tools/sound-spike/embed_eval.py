"""Compare audio embedding models on the fetched clips: how often a clip's nearest neighbours
share its genre (a stand-in for "sounds like"), how fast each model is, and how big.

usage: python embed_eval.py FMA_DIR RESULTS_JSON
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


def load_models():
    models = {}
    try:
        import tensorflow_hub as hub

        yamnet = hub.load("https://tfhub.dev/google/yamnet/1")
        models["yamnet (Apache-2.0)"] = lambda w: yamnet(w)[1].numpy().mean(0)
        vggish = hub.load("https://tfhub.dev/google/vggish/1")
        models["vggish (Apache-2.0)"] = lambda w: vggish(w).numpy().mean(0)
    except Exception as e:  # noqa: BLE001 - report and carry on
        print("tensorflow-hub models unavailable:", repr(e)[:300])
    try:
        import openl3

        net = openl3.models.load_audio_embedding_model("mel256", "music", 512)
        # The hop between 1 s patches: a longer hop analyses fewer patches, costing less on a phone.
        for hop in (1.0, 2.0, 3.0):
            models[f"openl3-music hop {hop:g} s (CC BY 4.0)"] = lambda w, hop=hop: openl3.get_audio_embedding(
                w, RATE, model=net, hop_size=hop, verbose=False)[0].mean(0)
    except Exception as e:  # noqa: BLE001
        print("openl3 unavailable:", repr(e)[:300])
    return models


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
    root, out_path = sys.argv[1], sys.argv[2]
    labels = pd.read_csv(f"{root}/labels.csv", index_col=0)["genre"]
    genre = np.array(labels.tolist())
    waves = [decode(f"{root}/clips/{tid}.mp3") for tid in labels.index]
    print(f"{len(waves)} clips, {len(set(genre))} genres, chance {100 / len(set(genre)):.1f}%", flush=True)
    results = {}
    for name, fn in load_models().items():
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
