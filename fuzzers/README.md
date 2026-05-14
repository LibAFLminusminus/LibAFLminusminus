# LibAFL Fuzzers

## Example fuzzers

You can find a large amount of example fuzzers built on top of LibAFL.
They are sorted by focus:

- [`baby`](./baby/): Minimal fuzzers and fuzzers demonstrating specific features that don't fit anywhere else.
- [`binary_only`](./binary_only/): Fuzzers for binary-only targets.
- [`full_system`](./full_system/): Fuzzers for full-system targets (kernels, firmwares, etc...).
- [`inprocess`](./inprocess/): Common In-process fuzzers. Most of the time, this is what you want.
- [`structure_aware`](./structure_aware/): Grammar fuzzers, fuzzers for certain languages, fuzzers with custom inputs, and more.

(Some fuzzers may fit into multiple categories, in which case we sort them as it makes sense, for example `structure_aware > full_system > binary_only > the rest`)

## Fully-feature Fuzzers

Some rather complete fuzzers worth looking at are:

- [`LibAFL-QEMU-Launcher`](./binary_only/qemu_launcher/): A full-featured QEMU-mode fuzzer that runs on multiple cores

They may not be the best starting point for your own custom fuzzer, but they might be easy enough to just use.

## Paper Artifacts

Multiple papers based on LibAFL have been published and include artifacts.
Here is a list of LibAFL artifacts:

- Fuzzbench implementation: https://github.com/AFLplusplus/libafl_fuzzbench
- LibAFL QEMU experiments: https://github.com/AFLplusplus/libafl_qemu_artifacts
