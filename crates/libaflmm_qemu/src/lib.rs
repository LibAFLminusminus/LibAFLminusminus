//! Welcome to `LibAFL` QEMU
//!
//! __Warning__: The documentation is built by default for `x86_64` in `usermode`. To access the documentation of other architectures or `systemmode`, the documentation must be rebuilt with the right features.
/*! */
#![doc = include_str!("../README.md")]
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]
// libafl_qemu only supports Linux currently
#![cfg(target_os = "linux")]
// This lint triggers too often on the current GuestAddr type when emulating 64-bit targets because
// u64::from(GuestAddr) is a no-op, but the .into() call is needed when GuestAddr is u32.
#![cfg_attr(
    any(cpu_target = "x86_64", cpu_target = "aarch64"),
    allow(clippy::useless_conversion)
)]
// libafl_qemu_sys export types with empty struct markers (e.g. struct {} start_init_save).
// This causes bindgen to generate empty Rust struct that are generally not FFI-safe due to C++ having empty structs with size 1
// As the QEMU codebase is C, it is FFI-safe and we just ignore the warning
#![allow(improper_ctypes)]
// you don't build this without std
#![allow(clippy::std_instead_of_core)]
// same
#![allow(clippy::std_instead_of_alloc)]

#[cfg(feature = "python")]
use pyo3::prelude::*;
use std::env;
#[cfg(feature = "python")]
use strum::IntoEnumIterator;

pub mod arch;
pub mod breakpoint;
pub mod command;
pub mod elf;
pub mod emu;
pub mod executors;
pub mod modules;
pub mod qemu;
pub mod sync_exit;
pub use libaflmm_qemu_sys as sys;

#[cfg(feature = "usermode")]
pub use libaflmm_qemu_sys::GuestAbiUlong;
#[cfg(feature = "systemmode")]
pub use libaflmm_qemu_sys::{CPUArchState, GuestPhysAddr, GuestVirtAddr};
pub use libaflmm_qemu_sys::{GuestAddr, GuestUlong, GuestUsize, MmapPerms};

pub mod prelude {
    #[cfg(feature = "usermode")]
    pub use crate::GuestAbiUlong;

    #[cfg(feature = "systemmode")]
    pub use crate::{CPUArchState, GuestPhysAddr, GuestVirtAddr};

    pub use crate::{GuestAddr, GuestUlong, GuestUsize, MmapPerms};

    #[cfg(feature = "usermode")]
    pub use crate::arch::syscalls;
    pub use crate::arch::{GuestReg, Regs, capstone, get_exit_arch_regs};

    pub use crate::command::{
        CommandError, CommandManager, IsCommand, IsStdCommandManager, NativeCommandParser,
        NopCommand, NopCommandManager, StdCommandManager,
    };

    #[cfg(feature = "nyx")]
    pub use crate::command::NyxCommandManager;
    #[cfg(not(feature = "nyx"))]
    pub use crate::command::{
        AddressAllowCommand, EndCommand, LoadCommand, LqemuCommandManager, LqprintfCommand,
        SaveCommand, StartCommand, TestCommand, VersionCommand,
    };

    #[cfg(all(feature = "systemmode", not(feature = "nyx")))]
    pub use crate::command::SetMapCommand;

    pub use crate::emu::{
        Emulator, EmulatorDriver, EmulatorDriverError, EmulatorDriverResult, EmulatorExitError,
        EmulatorExitResult, EmulatorHooks, EmulatorModules, GenericEmulatorDriver, GuestAddrKind,
        InputLocation, InputSetter, IsSnapshotManager, MapKind, NopEmulatorDriver, NopInputSetter,
        NopSnapshotManager, QemuSnapshotCheckResult, SnapshotId, SnapshotManagerCheckError,
        SnapshotManagerError, StdEmulator, StdEmulatorBuilder, StdEmulatorDriver,
        StdEmulatorDriverBuilder, StdInputSetter,
    };

    #[cfg(feature = "systemmode")]
    pub use crate::emu::{FastSnapshotManager, FastSnapshotPtr, QemuSnapshotManager};

    pub use crate::executors::{SimpleQemuExecutor, StdQemuExecutor};

    pub use crate::modules::{
        CallTracerModule, CmpLogModule, DrCovModule, DrCovModuleBuilder, EdgeCoverageModule,
        EmulatorModule, EmulatorModuleTuple, LoggerModule, StdEdgeCoverageChildModule,
        StdEdgeCoverageClassicModule, StdEdgeCoverageFullModule, StdEdgeCoverageModule,
    };

    #[cfg(feature = "injections")]
    pub use crate::modules::InjectionModule;
    #[cfg(feature = "usermode")]
    pub use crate::modules::{
        AsanGuestModule, AsanHostModule, RedirectStdinModule, RedirectStdoutModule, SnapshotModule,
    };

    pub use crate::qemu::{
        ArchExtras, CPU, CallingConvention, MemAccessInfo, Qemu, QemuConfig, QemuError,
        QemuExitError, QemuExitReason, QemuHooks, QemuInitError, QemuMemoryChunk, QemuParams,
        QemuRWError, QemuRWErrorCause, QemuRWErrorKind, QemuShutdownCause, config,
    };

    #[cfg(feature = "systemmode")]
    pub use crate::qemu::{
        DeviceSnapshotFilter, HostMemoryChunk, HostMemoryIter, HostMemorySegments, PhysMemoryChunk,
        PhysMemoryIter,
    };

    pub use crate::breakpoint::{Breakpoint, BreakpointId};

    pub use crate::elf::EasyElf;

    pub use crate::sync_exit::{CustomInsn, ExitArgs};
}

#[must_use]
pub fn filter_qemu_args() -> Vec<String> {
    let mut args = vec![env::args().next().unwrap()];
    let mut args_iter = env::args();

    while let Some(arg) = args_iter.next() {
        if arg.starts_with("--libafl") {
            args.push(arg);
            args.push(args_iter.next().unwrap());
        } else if arg.starts_with("-libafl") {
            args.push("-".to_owned() + &arg);
            args.push(args_iter.next().unwrap());
        }
    }
    args
}

#[cfg(feature = "python")]
#[pymodule]
#[pyo3(name = "libafl_qemu")]
pub fn python_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    use pyo3::types::PyString;

    let regsm = PyModule::new(m.py(), "regs")?;
    for r in arch::Regs::iter() {
        let v: i32 = r.into();
        regsm.add(PyString::new(m.py(), &format!("{r:?}")), v)?;
    }
    m.add_submodule(&regsm)?;

    let mmapm = PyModule::new(m.py(), "mmap")?;
    for r in MmapPerms::iter() {
        let v: i32 = r.into();
        mmapm.add(PyString::new(m.py(), &format!("{r:?}")), v)?;
    }
    m.add_submodule(&mmapm)?;

    #[cfg(feature = "usermode")]
    m.add_class::<sys::MapInfo>()?;

    #[cfg(feature = "usermode")]
    m.add_class::<qemu::GuestMaps>()?;

    m.add_class::<qemu::pybind::SyscallHookResult>()?;
    m.add_class::<qemu::pybind::Qemu>()?;

    Ok(())
}
