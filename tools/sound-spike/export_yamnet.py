"""Convert MediaPipe's YAMNet TFLite file (Apache-2.0, Google) into the small binary the Rust
analysis crate reads (core/encore-analysis/model/yamnet-v1.bin), and check it.

The TFLite file is fully integer quantised: int8 weights with a scale per output channel, int32
biases with scale (input scale x weight scale). This turns every layer back into real numbers
(weights as int8 times their scale, biases as floats) and keeps the weights as int8 plus one float
scale per output channel, so the file stays about 3.9 MB while the network runs in floats.

usage: python export_yamnet.py yamnet.tflite OUT.bin

File layout, little endian:
  "YAMN", u32 version (1)
  hann window f32[400], mel matrix f32[64 x 257], mel offset f32 (the 0.001 added before the log)
  first convolution (3x3, 1 -> 32 channels, stride 2):   i8[32*9], f32 scale[32], f32 bias[32]
  13 blocks, each a depthwise 3x3 convolution (channels C): i8[C*9], f32 scale[C], f32 bias[C]
                  then a pointwise 1x1 convolution (C -> Cout): i8[Cout*C], f32 scale[Cout], f32 bias[Cout]
  classifier (1024 -> 521): i8[521*1024], f32 scale[521], f32 bias[521]
The strides and channel counts are fixed in the Rust code (docs/adr/core-0019).
"""
import struct
import sys

import numpy as np
import tflite


def main():
    src, out = sys.argv[1], sys.argv[2]
    model = tflite.Model.GetRootAsModel(open(src, "rb").read(), 0)
    sg = model.Subgraphs(0)
    codes = [model.OperatorCodes(i) for i in range(model.OperatorCodesLength())]
    BUILTIN = {v: k for k, v in vars(tflite.BuiltinOperator).items() if isinstance(v, int)}

    def kind(op):
        c = codes[op.OpcodeIndex()]
        return BUILTIN[max(c.BuiltinCode(), c.DeprecatedBuiltinCode())]

    def quant(i):
        q = sg.Tensors(i).Quantization()
        scale = q.ScaleAsNumpy() if q and q.ScaleLength() else np.array([1.0], dtype=np.float32)
        zp = q.ZeroPointAsNumpy() if q and q.ZeroPointLength() else np.array([0], dtype=np.int64)
        return scale.astype(np.float64), zp.astype(np.int64)

    def data(i, dtype):
        t = sg.Tensors(i)
        raw = model.Buffers(t.Buffer()).DataAsNumpy().tobytes()
        return np.frombuffer(raw, dtype=dtype).reshape([t.Shape(j) for j in range(t.ShapeLength())])

    def layer(op, depthwise):
        x, w, b = op.Inputs(0), op.Inputs(1), op.Inputs(2)
        q8 = data(w, np.int8).astype(np.int32)
        w_scale, w_zp = quant(w)
        x_scale, _ = quant(x)
        if depthwise:
            # [1, 3, 3, C] -> [C, 9]; the scale runs along the last axis.
            q8 = np.transpose(q8[0], (2, 0, 1)).reshape(q8.shape[3], 9)
        else:
            q8 = q8.reshape(q8.shape[0], -1)  # [Cout, kh*kw*Cin]
        channels = q8.shape[0]
        w_scale = np.broadcast_to(w_scale, (channels,)).astype(np.float64)
        w_zp = np.broadcast_to(w_zp, (channels,))
        if np.any(w_zp != 0):
            raise SystemExit("weights with a zero point are not handled")
        bias = data(b, np.int32).astype(np.float64) * x_scale[0] * w_scale
        return q8.astype(np.int8), w_scale.astype(np.float32), bias.astype(np.float32)

    ops = [sg.Operators(n) for n in range(sg.OperatorsLength())]
    convs = [(n, op) for n, op in enumerate(ops) if kind(op) in ("CONV_2D", "DEPTHWISE_CONV_2D")]
    assert len(convs) == 1 + 13 * 2 + 0 or len(convs) == 28, len(convs)
    fc = [op for op in ops if kind(op) == "FULLY_CONNECTED"]
    mel_op, class_op = fc[0], fc[-1]

    buf = bytearray(b"YAMN")
    buf += struct.pack("<I", 1)
    window = data(ops[4].Inputs(1), np.float32).astype(np.float32)
    mel = data(mel_op.Inputs(1), np.float32).astype(np.float32)
    offset = data(mel_op.Inputs(2), np.float32).astype(np.float32)
    assert window.shape == (400,) and mel.shape == (64, 257) and np.allclose(offset, offset[0])
    buf += window.tobytes() + mel.tobytes() + struct.pack("<f", float(offset[0]))
    total = 0
    for n, op in convs:
        q, s, b = layer(op, kind(op) == "DEPTHWISE_CONV_2D")
        buf += q.tobytes() + s.tobytes() + b.tobytes()
        total += q.size
    q, s, b = layer(class_op, False)
    assert q.shape == (521, 1024)
    buf += q.tobytes() + s.tobytes() + b.tobytes()
    total += q.size
    open(out, "wb").write(bytes(buf))
    print(f"wrote {out}: {len(buf)} bytes, {total} weights, {len(convs)} convolutions and the classifier")


if __name__ == "__main__":
    main()
