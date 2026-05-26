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

use crate::{
    command::CommandError,
    emu::EmulatorError,
    qemu::{QemuError, QemuExitError, QemuInitError, QemuRWError},
};
use libaflmm::runtime;
use libaflmm_core::{ErrorBacktrace, display_error_backtrace};
#[cfg(feature = "python")]
use pyo3::prelude::*;
use std::{env, error, fmt, io, result};
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

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    // [`libaflmm::Error`] already carries its own backtrace, so we don't capture another one here.
    Libaflmm {
        source: libaflmm::Error,
    },
    Emulator {
        source: EmulatorError,
        backtrace: ErrorBacktrace,
    },
    Qemu {
        source: QemuError,
        backtrace: ErrorBacktrace,
    },
    Command {
        source: CommandError,
        backtrace: ErrorBacktrace,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // The inner error already appends its own backtrace on `Display`.
            Error::Libaflmm { source } => write!(f, "{source}"),
            Error::Emulator { source, backtrace } => {
                write!(f, "{source}")?;
                display_error_backtrace(f, backtrace)
            }
            Error::Qemu { source, backtrace } => {
                write!(f, "{source}")?;
                display_error_backtrace(f, backtrace)
            }
            Error::Command { source, backtrace } => {
                write!(f, "{source}")?;
                display_error_backtrace(f, backtrace)
            }
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::Libaflmm { source } => Some(source),
            Error::Emulator { source, .. } => Some(source),
            Error::Qemu { source, .. } => Some(source),
            Error::Command { source, .. } => Some(source),
        }
    }
}

impl From<libaflmm::Error> for Error {
    fn from(source: libaflmm::Error) -> Self {
        Error::Libaflmm { source }
    }
}

impl From<EmulatorError> for Error {
    fn from(source: EmulatorError) -> Self {
        Error::Emulator {
            source,
            backtrace: ErrorBacktrace::capture(),
        }
    }
}

impl From<QemuError> for Error {
    fn from(source: QemuError) -> Self {
        Error::Qemu {
            source,
            backtrace: ErrorBacktrace::capture(),
        }
    }
}

impl From<CommandError> for Error {
    fn from(source: CommandError) -> Self {
        Error::Command {
            source,
            backtrace: ErrorBacktrace::capture(),
        }
    }
}

impl From<QemuInitError> for Error {
    fn from(error: QemuInitError) -> Self {
        QemuError::from(error).into()
    }
}

impl From<QemuExitError> for Error {
    fn from(error: QemuExitError) -> Self {
        QemuError::from(error).into()
    }
}

impl From<QemuRWError> for Error {
    fn from(error: QemuRWError) -> Self {
        QemuError::from(error).into()
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        libaflmm::Error::from(error).into()
    }
}

impl From<Error> for libaflmm::Error {
    fn from(error: Error) -> Self {
        match error {
            Error::Libaflmm { source, .. } => source,
            ref e => runtime!("LibAFLmm QEMU error: {e}"),
        }
    }
}

pub mod prelude {
    pub use libaflmm::prelude::*;
    pub use libaflmm_bolts::prelude::*;
    pub use libaflmm_targets::prelude::*;

    #[cfg(feature = "usermode")]
    pub use crate::GuestAbiUlong;

    #[cfg(feature = "systemmode")]
    pub use crate::{CPUArchState, GuestPhysAddr, GuestVirtAddr};

    pub use crate::{GuestAddr, GuestUlong, GuestUsize, MmapPerms};

    #[cfg(feature = "usermode")]
    pub use crate::arch::syscalls;
    pub use crate::arch::{GuestReg, Regs, capstone, get_exit_arch_regs};

    pub use crate::command::{
        Command, CommandError, CommandManager, NativeCommandParser, NopCommand, NopCommandManager,
        StdCommandManager, StdCommands,
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
        Emulator, EmulatorExitError, EmulatorExitReason, EmulatorHooks, EmulatorModules,
        EmulatorRunResult, GuestAddrKind, InputLocation, InputWriter, MapKind, NopInputWriter,
        NopSnapshotManager, QemuSnapshotCheckResult, SnapshotId, SnapshotManager,
        SnapshotManagerCheckError, SnapshotManagerError, StdEmulator, StdEmulatorBuilder,
        StdInputWriter,
    };

    #[cfg(feature = "systemmode")]
    pub use crate::emu::{FastSnapshotManager, FastSnapshotPtr, QemuSnapshotManager};

    pub use crate::executors::{SimpleQemuExecutor, StdQemuExecutor};

    pub use crate::modules::{
        AddressFilter, AddressFilterVec, CallTracerModule, CmpLogModule, DrCovModule,
        DrCovModuleBuilder, EdgeCoverageModule, EmulatorModule, EmulatorModuleTuple, FilterList,
        HasAddressFilter, HasAddressFilterTuple, HasPageFilter, HasStdFilters, HasStdFiltersTuple,
        LoggerModule, NopAddressFilter, NopPageFilter, PageFilter, PageFilterVec, StdAddressFilter,
        StdEdgeCoverageChildModule, StdEdgeCoverageClassicModule, StdEdgeCoverageFullModule,
        StdEdgeCoverageModule, StdPageFilter,
    };

    #[cfg(all(
        feature = "usermode",
        feature = "asan_guest",
        not(cpu_target = "hexagon")
    ))]
    pub use crate::modules::AsanGuestModule;
    #[cfg(all(
        feature = "usermode",
        feature = "asan_host",
        not(cpu_target = "hexagon")
    ))]
    pub use crate::modules::AsanHostModule;
    #[cfg(feature = "injections")]
    pub use crate::modules::InjectionModule;
    #[cfg(feature = "usermode")]
    pub use crate::modules::{RedirectStdinModule, RedirectStdoutModule, SnapshotModule};

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
