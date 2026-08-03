# Runtime

The `Runtime` trait is one of the new concepts introduced in `LibAFL--` that was not in `LibAFL`.

In `LibAFL`, `Executor` was in fact responsible for two vastly different tasks:
- Feeding an `Input` from the `Fuzzer` to the target: forkserver protocol, harness call, etc...
- Managing the environment of a fuzzing worker: in-process machinery, signal handling, etc...

These two tasks are in fact quite different and should be done in distinct parts of the overall architecture.
In fact, it makes more sense let the environmental side be handled above the fuzzer itself, as it may have to handle state, or even restart the fuzzer.

To solve that, we introduce the `Runtime` trait, which will take care of the second task.
The first task remains a responsibility of `Executor`

## Where do `Runtime`s sit in the big picture

A `Runtime` is basically an abstraction for running a given `task`.
Each runtime will set up the environment differently (depending on its specific features) and ultimately run the task.
A `Runtime` task never returns (except in case of error): it ultimately exits with a successful error code.

## Main `Runtime`s in `LibAFL--`

`LibAFL--` comes with multiple `Runtime`s:
- `SimpleRuntime` runs its task without any side effect
- `RestartingRuntime` runs a task and restarts it if a specific error code is returned.
It also takes care of `State` snapshot.
- `InProcessRuntime` runs a task and exposes custom signal handling for the `Fuzzer`. This is the most complex `Runtime` of `LibAFL--` as it takes care of many OS-specific details.
