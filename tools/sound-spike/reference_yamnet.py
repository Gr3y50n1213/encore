"""Reference outputs of MediaPipe's YAMNet TFLite file on a synthetic waveform, for the Rust test
that checks core/encore-analysis/src/yamnet.rs against the original network
(core/encore-analysis/model/yamnet-reference.json).

The waveform is made by a formula the Rust test repeats, so no audio is stored:
  t = i / 16000
  x = 0.25 sin(2 pi 220 t) + 0.15 sin(2 pi 1318.5 t) + 0.10 sin(2 pi (300 t + 100 t^2)) + 0.05 noise
with noise from the 64-bit linear congruential generator in `wave` below.

usage: python reference_yamnet.py yamnet.tflite OUT.json
"""
import json
import sys

import numpy as np
import tensorflow as tf

RATE = 16_000
PATCH = 15_600
STARTS = (0, 15_600)


def wave(n):
    t = np.arange(n) / RATE
    x = (0.25 * np.sin(2 * np.pi * 220 * t) + 0.15 * np.sin(2 * np.pi * 1318.5 * t)
         + 0.10 * np.sin(2 * np.pi * (300 * t + 100 * t * t)))
    state = 12345
    noise = np.empty(n)
    for i in range(n):
        state = (state * 6364136223846793005 + 1442695040888963407) & 0xFFFFFFFFFFFFFFFF
        noise[i] = (state >> 40) / (1 << 24) * 2 - 1
    return (x + 0.05 * noise).astype(np.float32)


def main():
    model, out = sys.argv[1], sys.argv[2]
    interp = tf.lite.Interpreter(model_path=model, experimental_preserve_all_tensors=True)
    interp.allocate_tensors()
    inp = interp.get_input_details()[0]
    w = wave(STARTS[-1] + PATCH)
    patches = []
    for start in STARTS:
        interp.set_tensor(inp["index"], w[start:start + PATCH].reshape(inp["shape"]))
        interp.invoke()
        scores = interp.get_tensor(interp.get_output_details()[0]["index"]).reshape(-1)
        cands = [d for d in interp.get_tensor_details() if list(d["shape"]) in ([1, 1024], [1, 1, 1, 1024])]
        pooled = interp.get_tensor(max(cands, key=lambda d: d["index"])["index"]).reshape(-1)
        patches.append({"start": start, "pooled": [round(float(v), 4) for v in pooled],
                        "scores": [round(float(v), 4) for v in scores]})
    json.dump({"rate": RATE, "patch": PATCH, "patches": patches}, open(out, "w"))
    print("wrote", out, [len(p["pooled"]) for p in patches], [len(p["scores"]) for p in patches])


if __name__ == "__main__":
    main()
