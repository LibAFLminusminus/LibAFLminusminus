# Summary

[The LibAFL-- Fuzzing Library](./libaflmm.md)

[Introduction](./introduction.md)

[Getting Started](./getting_started/getting_started.md)
  - [Setup](./getting_started/setup.md)
  - [A first simple fuzzer](./getting_started/build.md)

[Concepts](./concepts/concepts.md)
  - [Execution modes](./concepts/execution_modes/execution_modes.md)
    - [Forkserver](./concepts/execution_modes/forkserver.md)
    - [In-process](./concepts/execution_modes/in_process.md)
  - [Fuzzer Archictecture](./concepts/architecture.md)
  - [Metadata](./concepts/metadata.md)
  - [Tuples](./concepts/tuples.md)
  - [Errors](./concepts/errors.md)
  - [Debugging](./concepts/debugging.md)
  - [Performance](./concepts/performance.md)

[Components](./core_concepts/core_components.md)
  - [State](./core_concepts/state.md)
  - [Runtime](./core_concepts/runtime.md)
  - [Launcher](./core_concepts/launcher.md)
  - [Controller](./core_concepts/controller.md)
  - [Fuzzer](./core_concepts/fuzzer.md)
  - [Input](./core_concepts/input.md)
  - [Executor](./core_concepts/executor.md)
  - [Stage](./core_concepts/stage.md)
  - [Mutator / Generator](./core_concepts/mutator_generator.md)
  - [Observer](./core_concepts/observer.md)
  - [Feedback](./core_concepts/feedback.md)

[Targets](./targets/targets.md)

[Frida](./frida/frida.md)

[Nyx](./nyx/nyx.md)

[QEMU](./qemu/qemu.md)
  - [Usermode](./qemu/usermode.md)
  - [Systemmode](./qemu/systemmode.md)

- [Intel PT](./intel_pt/intel_pt.md)

- [Examples](./examples/examples.md)
  - [Forkserver](./examples/forkserver.md)
  - [In-process](./examples/in_process.md)
  - [Frida](./advanced_features/frida.md)
  - [Nyx](./advanced_features/nyx.md)
  - [QEMU](./advanced_features/qemu.md)
    - [Usermode](./advanced_features/qemu.md)
    - [Systemmode](./advanced_features/qemu.md)

[//]: <> (Use cases: binary only, network, etc...)
[//]: <> (Optimal configuration for most common scenarios)
[//]: <> (AFL++ integration)

[Contributing](./contributing.md)

[Migrating from LibAFL](./libafl_migration.md)
