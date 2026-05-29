import ctypes
import platform

import pylibaflmm.sugar as sugar

print("Starting to fuzz from python!")
fuzzer = sugar.InProcessBytesCoverageSugar(
    input_dirs=["./in"], output_dir="out", broker_port=1337, cores=[0, 1]
)
fuzzer.run(lambda b: print("foo"))
