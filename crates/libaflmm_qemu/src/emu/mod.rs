//! Higher-level abstraction of [`Qemu`]
//!
//! [`Emulator`] is built above [`Qemu`] and provides convenient abstractions.

#[cfg(feature = "systemmode")]
use crate::qemu::DeviceSnapshotFilter;
#[cfg(feature = "usermode")]
use crate::qemu::{GuestMaps, ImageInfo, TargetSignalHandling};
use crate::{
    Result,
    arch::GuestReg,
    breakpoint::{Breakpoint, BreakpointId},
    command::{CommandError, CommandManager},
    modules::{EmulatorModuleTuple, HasStdFiltersTuple},
    qemu::{ArchExtras, CPU, CallingConvention, Qemu, QemuRWError, QemuShutdownCause},
    sync_exit::CustomInsn,
};
use core::fmt::{self, Debug, Display, Formatter};
use delegate::delegate;
use libaflmm::{executors::ExitKind, inputs::Input, observers::ObserversTuple, states::State};
use libaflmm_bolts::os::unix_signals::Signal;
use libaflmm_qemu_sys::{GuestAddr, GuestPhysAddr, GuestVirtAddr};
#[cfg(feature = "usermode")]
use libaflmm_qemu_sys::{MmapPerms, VerifyAccess};
use std::{ops::Add, result};
use thiserror::Error;

pub mod standard;
pub use standard::{StdEmulator, StdEmulatorBuilder};

pub mod hooks;
pub use hooks::{EmulatorHooks, EmulatorModules};

pub mod input_writer;
pub use input_writer::{
    EmulatorDriverResult, InputWriter, LqemuInputWriter, MapKind, NopInputWriter, StdInputWriter,
};

pub mod snapshots;
pub use snapshots::{
    NopSnapshotManager, QemuSnapshotCheckResult, SnapshotId, SnapshotManager,
    SnapshotManagerCheckError, SnapshotManagerError,
};

#[cfg(feature = "systemmode")]
pub use snapshots::{FastSnapshotManager, FastSnapshotPtr, QemuSnapshotManager};

#[cfg(feature = "usermode")]
pub mod usermode;
#[cfg(feature = "usermode")]
pub use usermode::InputLocation;

#[cfg(feature = "systemmode")]
pub mod systemmode;
#[cfg(feature = "systemmode")]
pub use systemmode::InputLocation;

#[derive(Debug, Clone, Error)]
pub enum EmulatorError {
    #[error(transparent)]
    Exit(#[from] EmulatorExitError),
    #[error(transparent)]
    SMError(#[from] SnapshotManagerError),
    #[error(transparent)]
    SMCheckError(#[from] SnapshotManagerCheckError),
    #[error(transparent)]
    CommandError(#[from] CommandError),
    #[error("unhandled signal: {0:?}")]
    UnhandledSignal(Signal),
    #[error("multiple snapshot definitions")]
    MultipleSnapshotDefinition,
    #[error("multiple input location definitions")]
    MultipleInputLocationDefinition,
    #[error("snapshot not found")]
    SnapshotNotFound,
    #[error("emulator not started yet")]
    NotStartedYet,
    #[error("end command received before start")]
    EndBeforeStart,
}

pub trait Emulator {
    type Input: Input + Unpin;
    type State: State + Unpin;

    type CommandManager: CommandManager;
    type InputWriter: InputWriter<Self::Input, Self::State>;
    type Modules: EmulatorModuleTuple<Self::Input, Self::State> + HasStdFiltersTuple + Unpin;
    type SnapshotManager: SnapshotManager;

    fn first_exec(&mut self, state: &mut Self::State) -> Result<()>;

    fn pre_exec(&mut self, state: &mut Self::State, input: &Self::Input) -> Result<()>;

    fn exec_input(&mut self, state: &mut Self::State, input: &Self::Input) -> Result<ExitKind>;

    fn post_exec<OT>(
        &mut self,
        state: &mut Self::State,
        input: &Self::Input,
        observers: &mut OT,
        exit_kind: &mut ExitKind,
    ) -> Result<()>
    where
        OT: ObserversTuple<Self::State>;

    fn on_crash(&mut self) -> Result<()>;

    fn on_timeout(&mut self) -> Result<()>;

    fn qemu(&self) -> Qemu;

    fn add_breakpoint(
        &self,
        bp: Breakpoint<<Self::CommandManager as CommandManager>::Commands>,
        enable: bool,
    ) -> BreakpointId;

    fn remove_breakpoint(&self, bp_id: BreakpointId);

    fn snapshot_id(&self) -> Option<SnapshotId>;

    fn set_snapshot_id(&mut self, snapshot_id: SnapshotId) -> Result<()>;

    fn snapshot_manager_mut(&mut self) -> &mut Self::SnapshotManager;

    fn command_manager_mut(&mut self) -> &mut Self::CommandManager;

    fn modules_mut(&mut self) -> &mut EmulatorModules<Self::Modules, Self::Input, Self::State>;

    fn set_input_location(&mut self, input_location: &InputLocation) -> Result<()>;

    /// Read a value in memory of type T.
    ///
    /// # Safety
    ///
    /// Check [`Qemu::read_mem_val`] for more details.
    unsafe fn read_mem_val<T>(&self, addr: GuestAddr) -> result::Result<T, QemuRWError> {
        unsafe { self.qemu().read_mem_val(addr) }
    }

    /// Write a value in memory of type T.
    ///
    /// # Safety
    ///
    /// Check [`Qemu::write_mem_val`] for more details.
    unsafe fn write_mem_val<T>(&self, addr: GuestAddr, val: &T) -> result::Result<(), QemuRWError> {
        unsafe { self.qemu().write_mem_val(addr, val) }
    }

    fn read_reg(&self, reg: impl Into<i32>) -> result::Result<GuestReg, QemuRWError> {
        self.qemu().read_reg(reg)
    }

    fn write_return_address<T>(&self, val: T) -> result::Result<(), QemuRWError>
    where
        T: Into<GuestAddr>,
    {
        self.qemu().write_return_address(val)
    }

    fn write_function_argument_with_cc<T>(
        &self,
        idx: u8,
        val: T,
        conv: CallingConvention,
    ) -> result::Result<(), QemuRWError>
    where
        T: Into<GuestReg>,
    {
        self.qemu().write_function_argument_with_cc(idx, val, conv)
    }

    delegate! {
        to self.qemu() {
            fn num_cpus(&self) -> usize;

            fn current_cpu(&self) -> Option<CPU>;

            fn cpu_from_index(&self, idx: usize) -> Option<CPU>;

            fn page_from_addr(&self, addr: GuestAddr) -> GuestAddr;

            fn read_mem(&self, addr: GuestAddr, buf: &mut [u8]) -> result::Result<(), QemuRWError>;

            fn read_mem_vec(
                &self,
                addr: GuestAddr,
                len: usize,
            ) -> result::Result<Vec<u8>, QemuRWError>;

            fn write_mem(&self, addr: GuestAddr, buf: &[u8]) -> result::Result<(), QemuRWError>;

            fn num_regs(&self) -> i32;

            fn write_reg(
                &self,
                reg: impl Into<i32>,
                val: impl Into<GuestReg>,
            ) -> result::Result<(), QemuRWError>;

            fn entry_break(&self, addr: GuestAddr) -> libaflmm::Result<()>;

            fn flush_jit(&self);

            fn host_page_size(&self) -> usize;

            fn is_running(&self) -> bool;

            fn write_function_arguments(
                &mut self,
                val: &[impl Into<GuestReg> + Clone],
            ) -> result::Result<(), QemuRWError>;

            fn write_function_arguments_with_cc(
                &mut self,
                val: &[impl Into<GuestReg> + Clone],
                conv: &CallingConvention,
            ) -> result::Result<(), QemuRWError>;

            fn read_function_argument(&self, idx: u8) -> result::Result<GuestReg, QemuRWError>;

            fn write_function_argument(
                &self,
                idx: u8,
                val: impl Into<GuestReg>,
            ) -> result::Result<(), QemuRWError>;

            fn read_return_address(&self) -> result::Result<GuestAddr, QemuRWError>;

            fn read_function_argument_with_cc(
                &self,
                idx: u8,
                conv: CallingConvention,
            ) -> result::Result<GuestReg, QemuRWError>;
        }
    }

    #[cfg(feature = "usermode")]
    fn g2h<T>(&self, addr: GuestAddr) -> *mut T {
        self.qemu().g2h(addr)
    }

    #[cfg(feature = "usermode")]
    fn h2g<T>(&self, addr: *const T) -> GuestAddr {
        self.qemu().h2g(addr)
    }

    #[cfg(feature = "usermode")]
    fn binary_path<'a>(&self) -> &'a str {
        self.qemu().binary_path()
    }

    #[cfg(feature = "usermode")]
    delegate! {
        to self.qemu() {
            fn mappings(&self) -> GuestMaps;

            fn image_info(&self) -> ImageInfo;

            fn access_ok(&self, kind: VerifyAccess, addr: GuestAddr, size: usize) -> Option<bool>;

            fn force_dfl(&self);

            fn load_addr(&self) -> GuestAddr;

            fn get_brk(&self) -> GuestAddr;

            fn get_initial_brk(&self) -> GuestAddr;

            fn set_brk(&self, brk: GuestAddr);

            fn get_mmap_start(&self) -> GuestAddr;

            fn set_mmap_start(&self, start: GuestAddr);

            fn mmap(
                &self,
                addr: GuestAddr,
                size: usize,
                perms: MmapPerms,
                flags: i32,
                fd: i32,
            ) -> Result<GuestAddr>;

            fn map_private(
                &self,
                addr: GuestAddr,
                size: usize,
                perms: MmapPerms,
            ) -> Result<GuestAddr>;

            fn map_fixed(
                &self,
                addr: GuestAddr,
                size: usize,
                perms: MmapPerms,
            ) -> Result<GuestAddr>;

            fn mprotect(&self, addr: GuestAddr, size: usize, perms: MmapPerms) -> Result<()>;

            fn unmap(&self, addr: GuestAddr, size: usize) -> Result<()>;

            /// Set target crash handling.
            ///
            /// # Safety
            ///
            /// Check [`Qemu::set_target_crash_handling`] for more details.
            unsafe fn set_target_crash_handling(&self, handling: &TargetSignalHandling);
        }
    }

    #[cfg(feature = "systemmode")]
    delegate! {
        to self.qemu() {
            unsafe fn write_phys_mem(&self, paddr: GuestPhysAddr, buf: &[u8]);

            unsafe fn read_phys_mem(&self, paddr: GuestPhysAddr, buf: &mut [u8]);

            fn save_snapshot(&self, name: &str, sync: bool);

            fn load_snapshot(&self, name: &str, sync: bool);

            fn create_fast_snapshot(&self, track: bool) -> FastSnapshotPtr;

            fn create_fast_snapshot_filter(
                &self,
                track: bool,
                device_filter: &DeviceSnapshotFilter,
            ) -> FastSnapshotPtr;

            unsafe fn restore_fast_snapshot(&self, snapshot: FastSnapshotPtr);

            fn list_devices(&self) -> Vec<String>;
        }
    }

    #[cfg(feature = "systemmode")]
    fn allow_page_on_start(&self) -> bool;
}

#[derive(Copy, Clone)]
pub enum GuestAddrKind {
    Physical(GuestPhysAddr),
    Virtual(GuestVirtAddr),
}

#[derive(Clone)]
pub enum EmulatorExitResult<C> {
    QemuExit(QemuShutdownCause), // QEMU ended for some reason.
    Breakpoint(Breakpoint<C>),   // Breakpoint triggered. Contains the address of the trigger.
    CustomInsn(CustomInsn<C>), // Synchronous backdoor: The guest triggered a backdoor and should return to LibAFL.
    Crash,                     // Crash
    Timeout,                   // Timeout
    FuzzingStarts,             // The emulator is ready to enter the fuzzing loop.
}

#[derive(Debug, Clone, Error)]
pub enum EmulatorExitError {
    #[error("unknown exit kind")]
    UnknownKind,
    #[error("unexpected exit")]
    UnexpectedExit,
    #[error(transparent)]
    CommandError(#[from] CommandError),
    #[error("breakpoint not found at addr {0:#x}")]
    BreakpointNotFound(GuestAddr),
}

impl From<EmulatorExitError> for crate::Error {
    fn from(error: EmulatorExitError) -> Self {
        EmulatorError::from(error).into()
    }
}

impl<C> Debug for EmulatorExitResult<C>
where
    C: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            EmulatorExitResult::QemuExit(qemu_exit) => {
                write!(f, "{qemu_exit:?}")
            }
            EmulatorExitResult::Breakpoint(bp) => {
                write!(f, "{bp:?}")
            }
            EmulatorExitResult::CustomInsn(sync_exit) => {
                write!(f, "{sync_exit:?}")
            }
            EmulatorExitResult::Crash => {
                write!(f, "Crash")
            }
            EmulatorExitResult::Timeout => {
                write!(f, "Timeout")
            }
            EmulatorExitResult::FuzzingStarts => {
                write!(f, "Fuzzing starts")
            }
        }
    }
}

impl<C> EmulatorDriverResult<C> {
    #[must_use]
    pub fn end_of_run(&self) -> Option<ExitKind> {
        match self {
            EmulatorDriverResult::EndOfRun(exit_kind) => Some(*exit_kind),
            _ => None,
        }
    }
}

impl Debug for GuestAddrKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            GuestAddrKind::Physical(paddr) => write!(f, "paddr {paddr:#x}"),
            GuestAddrKind::Virtual(vaddr) => write!(f, "vaddr {vaddr:#x}"),
        }
    }
}

impl Add<GuestAddr> for GuestAddrKind {
    type Output = Self;

    fn add(self, rhs: GuestAddr) -> Self::Output {
        match self {
            GuestAddrKind::Physical(paddr) => {
                GuestAddrKind::Physical(paddr + GuestPhysAddr::try_from(rhs).unwrap())
            }
            GuestAddrKind::Virtual(vaddr) => GuestAddrKind::Virtual(vaddr + rhs as GuestVirtAddr),
        }
    }
}

impl Display for GuestAddrKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            GuestAddrKind::Physical(phys_addr) => write!(f, "hwaddr 0x{phys_addr:x}"),
            GuestAddrKind::Virtual(virt_addr) => write!(f, "vaddr 0x{virt_addr:x}"),
        }
    }
}

impl<C> Display for EmulatorExitResult<C>
where
    C: Debug,
{
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            EmulatorExitResult::QemuExit(shutdown_cause) => write!(f, "End: {shutdown_cause:?}"),
            EmulatorExitResult::Breakpoint(bp) => write!(f, "{bp}"),
            EmulatorExitResult::CustomInsn(sync_exit) => {
                write!(f, "Sync exit: {sync_exit:?}")
            }
            EmulatorExitResult::Crash => {
                write!(f, "Crash")
            }
            EmulatorExitResult::Timeout => {
                write!(f, "Timeout")
            }
            EmulatorExitResult::FuzzingStarts => {
                write!(f, "Fuzzing starts")
            }
        }
    }
}
