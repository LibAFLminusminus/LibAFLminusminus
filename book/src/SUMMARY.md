# Summary

[The LibAFL-- Fuzzing Library](./libaflmm.md)

[Introduction](./introduction.md)

- [Getting Started](./getting_started/getting_started.md)
  - [Setup](./getting_started/setup.md)
  - [A first simple fuzzer](./getting_started/build.md)

- [Concepts](./concepts/concepts.md)
  - [Execution modes](./concepts/execution_modes/execution_modes.md)
    - [Forkserver](./concepts/execution_modes/forkserver.md)
    - [In-process](./concepts/execution_modes/in_process.md)
  - [Fuzzer Architecture](./concepts/architecture.md)
  - [Metadata](./concepts/metadata.md)
  - [Tuples](./concepts/tuples.md)
  - [Errors](./concepts/errors.md)
  - [Debugging](./concepts/debugging.md)
  - [Performance](./concepts/performance.md)

- [Components](./components/components.md)
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

- [Targets](./targets/targets.md)

- [Frida](./frida/frida.md)

- [Nyx](./nyx/nyx.md)

- [QEMU](./qemu/qemu.md)
  - [Usermode](./qemu/usermode.md)
  - [Systemmode](./qemu/systemmode.md)

- [Intel PT](./intel_pt/intel_pt.md)

- [Examples](./examples/examples.md)
  - [Forkserver](./examples/forkserver.md)
  - [In-process](./examples/in_process.md)
  - [Frida](./examples/frida.md)
  - [Nyx](./examples/nyx.md)
  - [QEMU](./examples/qemu.md)
    - [Usermode](./examples/qemu_usermode.md)
    - [Systemmode](./examples/qemu_systemmode.md)

[//]: <> (Use cases: binary only, network, etc...)
[//]: <> (Optimal configuration for most common scenarios)
[//]: <> (AFL++ integration)

[Development Tips](./development_tips.md)

[Contributing](./contributing.md)

[Migrating from LibAFL](./libafl_migration.md)
