# Targets

`libaflmm_targets` is a integral part for `in-process` fuzzing.

This crate provides the static data that must be embedded inside the fuzzed binary.
For example, `libaflmm_targets` defines a zero-initialized public static array where the fuzzed program can write its coverage data back.

Similarly, `libaflmm_targets` overwrites several callbacks inserted by the compiler-time instrumentation.
For example, sancov instrumentation from clang will insert `__sanitizer_cov_trace_pc_guard` into every edges found during compilation.
This is a weakly defined symbol that we can overwrite later.

`libaflmm_targets` defines a strongly defined implementation of these for edge-coverage instrumentation.

# Example
Let's take a example of a typical in-process fuzzing setup.
We first put a visualized representation of the mental model here.

{{#drawio path="assets/targets.drawio" page=0}}

Remember that in `in-process` fuzzing, the fuzzer runtime and the compiled target lives inside the same binary and same process.
`libaflmm_targets` is also compiled together in the same binary along with the others.
It has a data section, usually representing a initialized coverage map.
The instrumented code inside the target will call the strongly defined callbacks living inside `libaflmm_targets`. 
Then, in turn, this callback will write coverage data into the coverage map.
After the execution is finished, the observer from the `libafl--` will observe the changes in this coverage map.