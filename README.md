# `LibAFL--`, the unbloated fuzzer librar

# Why `LibAFL--`?

After a few years maintaining `LibAFL`, we have come to realize maintainance cost was getting higher and higher.
Many features and concepts got integrated into the main repository, representing a massive code base to take care of.
In addition, multiple core components (like `Executor`) have conceptual flaws in `LibAFL`, for which any edition needs to be propagated in most parts of the project.
Finally, we decided to deliberately make important breaking changes, which means `LibAFL` fuzzers cannot be instantly compatible out of the box.

Since the changes we made are still quite early, we are unsure whether the community will prefer this version over the original design of `LibAFL`

For all those reasons, we thought it was easier to fork the project completely to let users choose which design they would prefer to use.
We will keep these changes separated (at least for now), to have a clear comparison point between `LibAFL` and `LibAFL--`, allowing us to easily spot what works and what does not.

### I just want to know what changed compared with `LibAFL`

Everything have been carefully documented in the [`LibAFL--` book](https://libaflminusminus.github.io/LibAFLminusminus/).
We strongly suggest you to take a look there first.
Some concepts are similar, new components have been others, while others have been removed.

There is [a section of the book](https://libaflminusminus.github.io/LibAFLminusminus/libafl_migration.html) dedicated to documenting the main differences with `LibAFL`, which should be helpful if you are already familiar with it.

If you wish to build the book yourself, please check [the book directory](./book).

### `AFL++` dependency

`LibAFL--`, contrarily to `LibAFL`, exposes `AFL++` as a submodule.
We only use it to maintain compabitility and compare with target-side code like `forkserver` or LLVM passes.
Thus, only `libaflmm_targets` depends on `AFL++`, there is no shared code for the fuzzing part.

## LLM Contributions

The [AGENTS.md](./AGENTS.md) file contains our LLM policy.
Please check it carefully if you plan to use LLMs for this repository.
In short, any use of LLMs for generating code is forbidden, the rest is allowed.

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

The original contributors working on `LibAFL` are:
- [Andrea Fioraldi](https://twitter.com/andreafioraldi) <andrea@aflplus.plus>
- [Dominik Maier](https://bsky.app/profile/dmnk.bsky.social) <dominik@aflplus.plus>
- [s1341](https://twitter.com/srubenst1341) <github@shmarya.net>
- [Dongjia Zhang](https://github.com/tokatoka) <toka@aflplus.plus>
- [Addison Crump](https://github.com/addisoncrump) <me@addisoncrump.info>
- [Romain Malmain](https://github.com/rmalmain) <rmalmain@pm.me>

## Contributing

Please check out **[CONTRIBUTING.md](CONTRIBUTING.md)** for the contributing guideline.

## Debugging

Your fuzzer doesn't work as expected? Try reading [DEBUGGING.md](./book/src/debugging.md) to understand how to debug your problems.

## License

`LibAFL--` is licensed under the [Mozilla Public License Version 2.0](LICENSE).

This project is a fork of [LibAFL](https://github.com/AFLplusplus/LibAFL),
which is dual-licensed under [Apache-2.0](LICENSE-APACHE) and [MIT](LICENSE-MIT).
The original LibAFL code remains available under those terms from the upstream
project. The combined work in this repository, including all modifications and
additions, is distributed under MPL-2.0.

[NOTICE.md](NOTICE.md) details the upstream grants, the per-crate licenses and the
carve-outs, notably the example fuzzers in `fuzzers`, licensed as MIT.
