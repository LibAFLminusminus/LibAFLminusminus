# libaflmm Crates

This directory contains the various crates that make up the libaflmm ecosystem. Here is a brief overview of each:

## Core Crates

- **[libaflmm](./libaflmm)**: Slot your own fuzzers together and extend their features using Rust. The main crate.
- **[libaflmm_bolts](./libaflmm_bolts)**: Low-level bolts to create fuzzers and so much more.
- **[libaflmm_targets](./libaflmm_targets)**: Common code for target instrumentation that can be used combined with libaflmm.

## Backends & Instrumentation

- **[libaflmm_frida](./libaflmm_frida)**: Frida backend library for libaflmm.
- **[libaflmm_intelpt](./libaflmm_intelpt)**: Intel Processor Trace wrapper for libaflmm.
- **[libaflmm_nyx](./libaflmm_nyx)**: libaflmm using nyx, only avaliable on linux.
- **[libaflmm_qemu](./libaflmm_qemu)**: QEMU user backend library for libaflmm.

## Compatibility & Integration

- **[libaflmm_cc](./libaflmm_cc)**: Commodity library to wrap compilers and link libaflmm.

## Utility & Infrastrucutre

- **[libaflmm_asan](./libaflmm_asan)**: Address sanitizer library for libaflmm.
- **[libaflmm_core](./libaflmm_core)**: Minimal set of core functions shared between almost all `libaflmm` crates.
- **[libaflmm_derive](./libaflmm_derive)**: Derive proc-macro crate for libaflmm.
