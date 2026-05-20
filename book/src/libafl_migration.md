# Migrating from `LibAFL`

This section servers as a quick migration guide from `LibAFL` for experienced users.
The main common parts and differences are briefly introduced here, helping users to port their existing `LibAFL` fuzzers as painlessly as possible.
Thus, this part is NOT intended for beginners.

Migrating from `LibAFL` to `LibAFL--` should be mostly straightforward, at least if you do not rely on some specific parts of the library.
Most of the core API is largely similar, while only some parts truly differ from `LibAFL`
As a result, most users should be able to reuse most of their code, except for `Manager`, `Event` and `Executor` related things.
For `Executor`s, the port should be moderately difficult, and often results in dropping most of the code, which has been generically moved to `Runtime` (especially for the inprocess / forkserver parts).

## What did actually change

### `llmp` dependency removal

The biggest change of `LibAFL--` with respect to `LibAFL` is the removal of `llmp` from the core.
Fuzzers no longer need to rely on shared memory and `llmp` by default, which significantly simplifies the overall design.
Our experiments showed that embedding `llmp` at the core did not offer measurable performance or coverage gains in practice, while adding substantial code complexity.
Where users still want `llmp`, it can be opted into without being a mandatory building block.

### Introduction of `Runtime`

The other important difference with `LibAFL` is the split of `Executor` into `Executor` and `Runtime`.
`Executor` used to carry two different roles: setup the environment in which the fuzzer would run (in-process, forkserver, etc...), and feed the input to the target.
It resulted in large, tightly coupled types like `GenericInProcessExecutor` that were hard to extend without touching unrelated paths.

### `Workdir`

Another significant change is the introduction of `Workdir`.
In `LibAFL`, the workdir layout was unspecified: directories could be created in arbitrary locations relative to where the fuzzer was launched, and the library did not prescribe a standard structure.
As a result, users either shared directories across clients or had a to implement pre-client separation themselves.
Creating a proper working directory structure is tedious, and required to re-implement the same basic blocs for each fuzzer.
`LibAFL--` provides `Workdir`, a structure for managing on-disk working directories.
It provides a convenient interface to create directories in the right place, without having to remember where the client root directory is.
By default, `LibAFL--` is organized as follows:
```
workdir/
  worker_0/
    crashes/ <-- the default objective directory
    corpus/ <-- the default evolving corpus directory
    logs.out <-- `stdout` logs
    logs.err <-- `stderr` logs
    fuzzer_stats <-- stats of the fuzzer, updated every few seconds
  worker_1/
    ...
  worker_2/
    ...
  ...
```
Simple, yet effective.
Note how `LibAFL--` does not share the same directories for `crashes` and `corpus` anymore.
Crashes and corpus entries are stored in separate directories, avoiding the concurrency issues that came with sharing them.

### Summary

Here is a more exhaustive list of what changed between `LibAFL` and `LibAFL--`:
- `controllers`: new.
Does the synchronization between `workers` (previously `clients`) and the `controller`.
Used for stat and corpus sharing, `workdir`, etc.
- `events`: fully removed.
- `executors`: split into `runtimes` (notably taking care of in-process and forkserver) and `executors` (run the target with a given `Input`).
- `launchers`: kind of new.
It used to be an optional part of `LibAFL`, resulting in important structural differences between fuzzers.
To unify fuzzers design, we made it a core part of the library, with its own `StdLauncher`.
Although it shares the same name and role as in `LibAFL`, the implementation has been completely rewritten.

## What does not really change

Here is a non-exhaustive list of the concepts that are similar, and will only require to make some trivial fixes:
- `Feedbacks`
- `Generators`
- `Inputs`
- `Mutators`
- `Observers`
- `Stages`

The commonest issues that could occur with these are:
- **Some implementations have been removed**: we deleted some implementations that could be occasionally useful (like the unicode mutators), but were largely unused.
The reason they are removed is mostly to keep the core library with the most important parts, while moving away the least used objects.

- **Generics have been turned into associated types for some traits**: a lot of traits were commonly using generics to carry around traits like `Input` and `State`, even though they were always determined for a given `Self`.
We moved away from this when it made sense, and let the object carry the generics.
That way, traits are much easier to use around, and helps the compiler for type inference.

## What should I do in practice to port my old fuzzer?

This part will provide some concrete code snippets demonstrating how `LibAFL` fuzzers can be ported to `LibAFL--` with minimal effort.
We will use some parts of `baby_inprocess` to do so.
Please refer to `fuzzers/baby/baby_inprocess` for the full working example.

Let `fn target(input: &I) -> Result<ExitKind>` be the signature of the target function in the rest of the section.

### State Builder Closure

A state builder closure must be provided to the `StdLauncher`.
It is given the worker for which it will be build, and must return a state that will be provided to the worker during execution.

Typically, a simple state builder can be created as follows:
```rust
let state_builder = |worker: &SimpleWorker| {
    // A scheduler following the queue policy
    let scheduler = QueueScheduler::new();
    // The default objective directory
    let crash_dir = worker.workdir().objective_dir()?;

    // create a State from scratch
    StdState::new(
        BytesContext,
        // Corpus that will be evolved, we keep it in memory for performance
        // It must have a scheduler
        InMemoryCorpus::with_scheduler(scheduler),
        // Corpus in which we store solutions (crashes in this example),
        // on disk so the user can get them after stopping the fuzzer
        OnDiskCorpus::builder().root_dir(crash_dir).build()?,
    )
};
```

This closure is called by the `StdLauncher` to create the state when starting a new `Runtime`.
Then, the state ownership is given to the `Runtime` through a call to `Runtime::run`.
That way, it is easy to create a state per-worker.

## Creating a `Controller`

The controller will dictate how the fuzzing campaign will be orchestrated, and how workers should be configured.
Through its creation, it is possible to set the root working directory, where `stdout` / `stderr` should be redirected, etc...
Its default builder already sets reasonable default values, leading to the same structure as presented in the `Workdir` section.

```rust
// The launcher supervises the fuzzer and communicates with the workers.
let controller = StdController::builder().overwrite(true).build()?;
```

## Configuring the `StdLauncher`

The `StdLauncher` needs to be built was at least 4 main elements:
- The state builder
- The controller
- The monitor (same idea as in `LibAFL`)
- The runtime or the task

Here is a typical `StdLauncher` run:
```rust
StdLauncher::builder()?
    .controller(controller)
    .monitor(monitor)
    .state_builder(state_builder)
    .build_inprocess(task)      // Use the default in-process style runtime
    // .build_forkserver(task)  // Use the default forkserver style runtime
    // .runtime(rt).build()     // Use a manually-created runtime
    .launch()
```

For completeness, we commented out the other possible ways to build the launcher, to adapt to your use case:
- `build_inprocess(task)` will fire the task wrapped into a default `StdInProcessRuntime` and build the `StdLauncher`.
- `build_forkserver(task)` will similarly spawn the task with a default `StdForkserverRuntime` and build the `StdLauncher`.
- `runtime(rt).build()` will finally set the runtime you configured beforehand (which will most likely embed the task) and build the `StdLauncher.

In other words, `build_*` are convenient short hands for `runtime(*).build()`.

### Running the fuzzer

Now that the launcher is properly configured, we can configure the fuzzer as usual.
The most notable difference is that state is now provided as an argument of the task, instead of being created in the task.
This is necessary because of the separation between `Runtime` and `Executor`.
The rest of the setup should be familiar if you already wrote `LibAFL` fuzzers.
Please refer to `fuzzers/baby/baby_inprocess` to check the full minimal code.
It slightly differs from the code showed here, but ultimately fills the same mission.
