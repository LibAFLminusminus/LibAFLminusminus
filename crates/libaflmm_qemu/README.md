# `LibAFL QEMU`: A Library for Fuzzing-oriented Emulation and Hooking

`libaflmm_qemu` is a fuzzing-oriented emulation library that wraps `QEMU` with a rich API in Rust.

It comes in two variants, `usermode` to fuzz Linux ELFs userspace binaries and `systemmode`, to fuzz arbitrary operating systems with QEMU TCG.

## Usage

To use `libaflmm_qemu` in your project, add it as a dependency in your `Cargo.toml`:

```toml
[dependencies]
# Set this to the latest version
libaflmm_qemu = { version = "0.16.0", features = ["usermode", "x86_64"] }
```

`libaflmm_qemu` offers several feature flags to customize its build for different use cases. These flags are typically enabled in your `Cargo.toml`.

## Modes

* `usermode`: Enables fuzzing of userspace binaries on Linux.
* `systemmode`: Enables fuzzing of arbitrary operating systems with `QEMU` TCG. This is mutually exclusive with `usermode`.

## Unit tests

`libaflmm_qemu` exposes a simple API to create complex unit tests involving some guest code to work on.
Check [the `set_pc` example](./tests/set_pc) to have an idea of how to use it in practice.

## Cite

If you use `LibAFL QEMU` for your academic work, consider citing the following paper:

```bibtex
@InProceedings{libaflqemu:bar24,
  title        = {{LibAFL QEMU: A Library for Fuzzing-oriented Emulation}},
  author       = {Romain Malmain and Andrea Fioraldi and Aurélien Francillon},
  year         = {2024},
  series       = {BAR 24},
  month        = {March},
  booktitle    = {Workshop on Binary Analysis Research (colocated with NDSS Symposium)},
  location     = {San Diego (USA)},
  keywords     = {fuzzing, emulation},
}
```

## The `LibAFLmm` Project

This crate is part of the [LibAFLmm project](https://github.com/LibAFLminusminus/LibAFLminusminus).

The [README](../../README.md) contains the list of maintainers and licensing information.
