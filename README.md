# `LibAFL--`, the unbloated fuzzer library

Temporary README.

Add temp stuff to `IDEAS.md`, this is useless atm.


# Why `LibAFL--`?

### Core concepts

### `AFL++` dependency

`LibAFL--`, contrarily to `LibAFL`, exposes `AFL++` as a submodule.
We only use it to maintain compabitility and compare with target-side code like `forkserver` or LLVM passes.
Thus, only `libafl_targets` depends on `AFL++`, there is no shared code for the fuzzing part.

## Building and installing

#### Install the Dependencies

- **The Rust development language**
  - We highly recommend *not* to use e.g. your Linux distribution package as this is likely outdated. So rather install Rust directly, instructions can be found [here](https://www.rust-lang.org/tools/install).
  - The minimum supported Rust version is defined. You can always check the currently required version in LibAFL’s [Cargo.toml](https://github.com/AFLplusplus/LibAFL/blob/main/crates/libafl/Cargo.toml):

    If your installed Rust version is older than the one listed in Cargo.toml, update to the latest stable toolchain:

    ```bash
    rustup update stable
    ```
- **LLVM tools**
  - The LLVM tools (including clang, clang++) are needed (newer than LLVM 15.0.0 up to LLVM 18.1.3) If you are using Debian/Ubuntu, again, we highly recommmend that you install the package from [here](https://apt.llvm.org/)
  - We use [just](https://github.com/casey/just) to build the fuzzers in `fuzzers/` directory. You can find instructions to install it in your environment [in the Just Programmer's Manual](https://just.systems/man/en/packages.html).

#### Clone the `LibAFL` repository with

```sh
git clone https://github.com/AFLplusplus/LibAFL
```

#### Build the library using

```sh
cargo build --release
```

#### Build the API documentation with

```sh
cargo doc
```

#### Browse the `LibAFL` book (WIP!) with (requires [mdbook](https://rust-lang.github.io/mdBook/index.html))

```sh
cd docs && mdbook serve
```

## Getting started

We collect all example fuzzers in [`./fuzzers`](./fuzzers/).
Be sure to read their documentation (and source), this is *the natural way to get started!*

```sh
just run
```

You can run each example fuzzer with this following command, as long as the fuzzer directory has a `Justfile` file.

## Contributors

`LibAFL--` is forked from [LibAFL](https://github.com/AFLplusplus/LibAFL).
It is written and maintained by 

- [Romain Malmain](https://github.com/rmalmain) <rmalmain@pm.me>
- [Dongjia Zhang](https://github.com/tokatoka) <tokazerkje@outlook.jp>

## Contributing

Please check out **[CONTRIBUTING.md](CONTRIBUTING.md)** for the contributing guideline.

## Debugging

Your fuzzer doesn't work as expected? Try reading [DEBUGGING.md](./docs/src/DEBUGGING.md) to understand how to debug your problems.

## License

`LibAFL--` is licensed under the [GNU Affero General Public License v3.0](LICENSE).

This project is a fork of [LibAFL](https://github.com/AFLplusplus/LibAFL),
which is dual-licensed under [Apache-2.0](LICENSE-APACHE) and [MIT](LICENSE-MIT).
The original LibAFL code remains available under those terms from the upstream
project. The combined work in this repository, including all modifications and
additions, is distributed under AGPL-3.0.
