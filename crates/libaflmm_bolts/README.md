# `LibAFLmm_bolts`: Handy Libary Collaction

The `libaflmm_bolts` crate is a toolshed combinding a lot of low-level features and crates `LibAFLmm` uses.
It can be a good starting point for low-level projects, even those that are not specifically fuzzers.
Some cross-platform things in bolts include (but are not limited to):
* `SerdeAnyMap`: a map that stores and retrieves elements by type and is serializable and deserializable
* `Shm`: A cross-platform (`Windows`, `Linux`, `Android`, `macOS`) shared memory implementation
* `Core_affinity`: A maintained version of `core_affinity` that can be used to get core information and bind processes to cores
* `Rands`: Fast random number generators for fuzzing (like [RomuRand](https://www.romu-random.org/))
* `MiniBSOD`: get and print information about the current process state including important registers.
* `Tuples`: Haskel-like compile-time tuple lists
* `Os`: OS specific stuff like signal handling, windows exception handling, pipes, and helpers for `fork`

## The `LibAFLmm` Project

This crate is part of the [LibAFLmm project](https://github.com/LibAFLminusminus/LibAFLminusminus).

The [README](../../README.md) contains the list of maintainers and licensing information.
