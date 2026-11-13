"""Print the layers of a TFLite model (operators, tensors, shapes, options) without TensorFlow,
so a network can be re-implemented elsewhere. Used for YAMNet in docs/adr/core-0019.

usage: python dump_tflite.py MODEL.tflite [--weights OUT.npz]
"""
import sys

import numpy as np
import tflite


def builtin_name(code):
    names = {v: k for k, v in vars(tflite.BuiltinOperator).items() if isinstance(v, int)}
    return names.get(code, str(code))


def main():
    path = sys.argv[1]
    model = tflite.Model.GetRootAsModel(open(path, "rb").read(), 0)
    sg = model.Subgraphs(0)
    codes = [model.OperatorCodes(i) for i in range(model.OperatorCodesLength())]

    def code_of(op):
        c = codes[op.OpcodeIndex()]
        return max(c.BuiltinCode(), c.DeprecatedBuiltinCode())

    def tensor(i):
        t = sg.Tensors(i)
        name = t.Name().decode() if t.Name() else ""
        return name, [t.Shape(j) for j in range(t.ShapeLength())], t.Type()

    print("inputs:", [tensor(sg.Inputs(i)) for i in range(sg.InputsLength())])
    print("outputs:", [tensor(sg.Outputs(i)) for i in range(sg.OutputsLength())])
    print("operators:", sg.OperatorsLength(), "tensors:", sg.TensorsLength())
    weights = {}
    for n in range(sg.OperatorsLength()):
        op = sg.Operators(n)
        name = builtin_name(code_of(op))
        ins = [op.Inputs(i) for i in range(op.InputsLength())]
        outs = [op.Outputs(i) for i in range(op.OutputsLength())]
        extra = ""
        if name in ("CONV_2D", "DEPTHWISE_CONV_2D") and op.BuiltinOptions():
            cls = tflite.Conv2DOptions if name == "CONV_2D" else tflite.DepthwiseConv2DOptions
            o = cls()
            o.Init(op.BuiltinOptions().Bytes, op.BuiltinOptions().Pos)
            extra = f" stride=({o.StrideH()},{o.StrideW()}) padding={'SAME' if o.Padding() == 0 else 'VALID'} act={o.FusedActivationFunction()}"
            if name == "DEPTHWISE_CONV_2D":
                extra += f" multiplier={o.DepthMultiplier()}"
        shapes = [tensor(i)[1] for i in ins if i >= 0]
        print(f"{n:3d} {name:22s} in={shapes} out={[tensor(i)[1] for i in outs]}{extra}")
        for i in ins:
            if i < 0:
                continue
            buf = model.Buffers(sg.Tensors(i).Buffer())
            if buf.DataLength() > 0:
                tname, shape, ttype = tensor(i)
                dtype = {0: np.float32, 1: np.float16, 2: np.int32, 3: np.uint8, 9: np.int8}.get(ttype)
                if dtype is not None:
                    weights[f"{n}:{tname}"] = np.frombuffer(buf.DataAsNumpy().tobytes(), dtype=dtype).reshape(shape)
    if "--weights" in sys.argv:
        out = sys.argv[sys.argv.index("--weights") + 1]
        np.savez(out, **{k.replace("/", "_").replace(":", "__"): v for k, v in weights.items()})
        print("saved", len(weights), "weight tensors,", sum(v.size for v in weights.values()), "values to", out)


if __name__ == "__main__":
    main()
