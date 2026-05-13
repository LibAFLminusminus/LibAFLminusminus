# `libaflmm_targets`: Runtime Components for Instrumented Fuzzing Targets

The `libaflmm_targets` crate provides the essential runtime components that are injected into a target program during its compilation for fuzzing with `LibAFLmm`.
This crate contains the code that runs inside the fuzzed program, communicating with the `LibAFL` fuzzer to provide coverage information, comparison data, and other feedback.

## Features

`libaflmm_targets` is highly modular and uses a feature-based system to allow users to select the specific instrumentation and runtime components needed for their fuzzing campaign. This ensures that only the necessary code is included in the target, minimizing overhead.

A non-exhaustive list of features includes:

- **Coverage Tracking**: Different strategies for tracking code coverage are available:
  - `sancov_pcguard`: Implements coverage tracking using `__sanitizer_cov_pc_guard`, which can be used for edge coverage (`sancov_pcguard_edges`) or hit count tracking (`sancov_pcguard_hitcounts`).
  - `sancov_ngram`: Supports N-gram coverage to track sequences of executed basic blocks.
  - `sancov_ctx`: Provides context-sensitive coverage.
  - ...
- **Comparison and Value Profiling**:
  - `sancov_cmplog`: Instruments compare instructions and `memcmp`/`strcmp` calls to log interesting values, which can be used by the `CmpLog` feedback mechanism in `LibAFL` and solve comparisons during fuzzing.
  - `sancov_value_profile`: Gathers information about values observed at compare sites.
- **[`LibFuzzer`](https://llvm.org/docs/LibFuzzer.html) Compatibility Layer**:
  - `libfuzzer`: A set of features to provide compatibility with the `LibFuzzer` fuzzing engine, allowing `LibAFL` to be used for `LibFuzzer` harnesses.
- **Forkserver**:
  - `forkserver`: Includes the client-side implementation of a forkserver, which can significantly speed up the fuzzing of programs that have a slow initialization phase.
- **Dynamic Analysis**:
  - `drcov`: Support for `drcov` output format (`DynamoRIO` code coverage) for coverage visualizations.
- ...

## The `LibAFLmm` Project

This crate is part of the [LibAFLmm project](https://github.com/LibAFLminusminus/LibAFLminusminus).

The [README](../../README.md) contains the list of maintainers and licensing information.
