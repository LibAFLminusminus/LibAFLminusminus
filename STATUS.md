# LibAFL to LibAFL\-- plans

# Feature Consensus

## definitions

- keep: obvious.
- rewrite: base can be kept the same, but major parts must be changed or removed.
- full rewrite: everything must be removed and rewritten. but the module in itself makes sense architecturally.
- remove: obvious.
- externalize: can be kept, but should be moved out of libafl. no one in libafl would maintain it.
- merge: move from separate crate to 

## table

| module                                              | decision1 | decision2 | note |
|-----------------------------------------------------|-----------|-----------|------------------------------------------------------------------------------------|
| build_id2                                           | keep      | keep      ||
| core_affinity2                                      | keep      | keep      ||
| exceptional                                         | keep      | merge     | i don't know if it needs to be split tbh. it can be in bolts.|
| fast_rands                                          | keep      | keep      |                                                                                    |
| libafl/common                                       | keep      | keep      |                                                                                    |
| corpus (partially done)                                             | rewrite   | rewrite   | need InMemoryOnDisk at least to keep in memory the generated seeds on disk if needed |
| events  (gone)                                            | remove    | full rewrite ||
| executors (done)                                          | rewrite   | rewrite ||
| feedbacks                                           | keep      | keep      ||
| fuzzer (done)                                          | rewrite   | rewrite   ||
| generator                                           | keep      | keep      ||
| inputs                                              | keep      | keep      ||
| monitors  (partially done)                                          | rewrite   | rewrite   | can be improved, very simple for now. |
| mutators  (done)                                          | rewrite   | rewrite   ||
| observers                                           | keep      | keep      ||
| schedulers  (done)                                        | rewrite   | rewrite   ||
| stages  (done)                                            | rewrite   | rewrite   ||
| state   (partially done)                                            | rewrite      | rewrite      | more stats? |
| libafl_asan                                         | ?         | keep      ||
| libafl_bolts                                        | keep      | keep      ||
| libafl_cc                                           | keep      | keep      ||
| libafl_concolic                                     | remove    | externalize (?) ||
| libafl_derive                                       | keep      | keep      ||
| libafl_frida                                        | ?         | keep      ||
| libafl_intelpt                                      | keep      | keep      ||
| libafl_libfuzzer/libafl_libfuzzer_runtime           | externalize    | externalize ||
| libafl_nyx                                          | keep      | keep      ||
| libafl_sugar                                        | remove    | remove    ||
| libafl_targets (P1)                                 | rewrite   | rewrite   ||
| libafl_tinyinst                                    | externalize     | exernalize    ||
| libafl_unicorn                                      | externalize    | externalize ||
| ll_mp                                               | remove    | remove  ||
| minibsod                                            | keep      | rewrite   | not signal-safe. also printing can be improved. |
| no_std_time                                         | remove          | remove  ||
| nonzero_macros                                      | keep      | keep      ||
| utils/build_and_test_fuzzers                        |           | keep      ||
| utils/cfg_builder                                   |           | keep      ||
| utils/ci_runner                                     |           | keep      ||
| utils/ci_splitter                                   |           | keep      ||
| utils/deexit                                        | remove    | ?         ||
| utils/drcov_utils                                   |           | keep      ||
| utils/find_llvm_config                              |           | keep      ||
| utils/gdb_qemu                                      |           | ?         | never used it|
| utils/gramatron                                     | remove    | remove | |
| utils/libafl_benches                                |           | keep      ||
| utils/libafl_jumper                                 |           | remove?   | useless?|
| utils/libafl_repo_tools                             |           | keep      ||
| utils/multi_machine_generator                       |           | ?         | remove if corpus sharing doesnt work |
| utils/noaslr                                        |           | keep?     ||
| others                                              | keep      | keep      ||


## Work queue

- [ x ] make libafl build again with remove_me feature
- [ x ] metadata upd
- [ x ] fix scheduler
- [ x ] workdir impl
- [ x ] monidor impl
- [ x ] laucher impl
- [ ] libafl target
- [ ] libafl intelpt
- [ ] libafl qemu
- [ ] write nyx fuzzer
- [ ] check if libafl-fuzz is doable
- [ ] every fuzzer MUST have proper documentation

# Structural problems

## Not everything can be seperated into modules
In some cases, modules need to communicate with each other. Currently we use a global-data for handling it (aka metadata). I think this is a good solution but relying on metadata is very hacky.
We should accept the need for global variables but design how to handle it in a better way

## There should be only one way to do a thing.
Each module should have clear definition on what it should do.
We should have a design that, when you want to implement a feature, you should be able to clearly tell where you're supposed to implement this stuff.

## State is not clearly defined
It's unclear what is getting saved and restored on crash.
Why should fuzzer contain feedback but not corpus for example?

# What holds what

1. State:
    - rand
    - stats
    - corpus
    - solutions
    - metadata
    - current corpus id
    - scheduler <--- moved from fuzzer
    - workdir: PathBuf

2. Fuzzer:
    - feedback <- decision maker, no state out of metadata
    - objective feedback

3. Executor:
    - ObserverHook
    - observer <- keep "logs"

Observer HitCount -> find a way to fit in the state cleanly.

# Runner and executor

Executor is now split into 2 parts:
- Runner: the environment-dependent task runner. it handles signals, timeouts, etc... it does NOT depend on fuzzer objects
- Executor: what the fuzzer runs. it takes care of observer resets, timeout configuration in the runner, exec count, etc...

executor and runner share a "handle", which enables the executor to configure the runner generically (independently of the underlying runner).

# State vs Input

for now, state and input are separated.
we can put input directly in state to avoid having to pass both around.
state holds input (i.e. current testcase).

# Formatting nickpick

## Structure modules properly

We should enforce a common ordering for mudules

```rust
<imports>

<module declaration>

<trait def>

<struct / enum def>

<struct / enum impl>
```

In particular:
- do NOT mix `use` imports with `pub use` and `mod` statements
- stop putting definitions of traits at the end of the file, it's unreadable
- keep module declarations consistent:
```rust
pub mod mymod;
pub use mymod::{MyStruct, MyEnum};
```
it should only be formatted like that. no `pub use` 100 lines below the module declaration.

-> added a simple linter check, experimental