# Summary

[The LibAFL-- Fuzzing Library](./libaflmm.md)

[Introduction](./introduction.md)

- [Getting Started](./getting_started/index.md)
  - [Setup](./getting_started/setup.md)
  - [A first simple fuzzer](./getting_started/build.md)

- [Concepts](./concepts/index.md)
  - [Operation Modes](./concepts/operation_modes/index.md)
    - [Forkserver](./concepts/operation_modes/forkserver.md)
    - [In-process](./concepts/operation_modes/in_process.md)
  - [Fuzzer Architecture](./concepts/architecture.md)
  - [Metadata](./concepts/metadata.md)
  - [Tuples](./concepts/tuples.md)
  - [Errors](./concepts/errors.md)
  - [Debugging](./concepts/debugging.md)
  - [Performance](./concepts/performance.md)

- [Components](./components/index.md)
  - [State](./components/state.md)
  - [Runtime](./components/runtime.md)
  - [Launcher](./components/launcher.md)
  - [Controller](./components/controller.md)
  - [Fuzzer](./components/fuzzer.md)
  - [Input](./components/input.md)
  - [Executor](./components/executor.md)
  - [Stage](./components/stage.md)
  - [Mutator / Generator](./components/mutator_generator.md)
  - [Observer](./components/observer.md)
  - [Feedback](./components/feedback.md)

- [Targets](./targets/index.md)

- [Frida](./frida/index.md)

- [Nyx](./nyx/index.md)

- [QEMU](./qemu/index.md)
  - [Usermode](./qemu/usermode.md)
  - [Systemmode](./qemu/systemmode.md)

- [Intel PT](./intel_pt/index.md)

- [Examples](./examples/index.md)
  - [Forkserver](./examples/forkserver.md)
  - [In-process](./examples/in_process.md)
  - [Frida](./examples/frida.md)
  - [Nyx](./examples/nyx.md)
  - [QEMU](./examples/qemu/index.md)
    - [Usermode](./examples/qemu/usermode.md)
    - [Systemmode](./examples/qemu/systemmode.md)

[//]: <> (Use cases: binary only, network, etc...)
[//]: <> (Optimal configuration for most common scenarios)
[//]: <> (AFL++ integration)

[Development Tips](./development_tips.md)

[Contributing](./contributing.md)

[Migrating from LibAFL](./libafl_migration.md)
