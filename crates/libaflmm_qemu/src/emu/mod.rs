//! Higher-level abstraction of [`Qemu`]
//!
//! [`Emulator`] is built above [`Qemu`] and provides convenient abstractions.

#[cfg(feature = "systemmode")]
use crate::qemu::DeviceSnapshotFilter;
#[cfg(feature = "usermode")]
use crate::qemu::{GuestMaps, ImageInfo, TargetSignalHandling};
use crate::{
    arch::GuestReg,
    breakpoint::{Breakpoint, BreakpointId},
    command::{Command, CommandError, CommandManager, IsStdCommandManager},
    modules::{EmulatorModuleTuple, HasAddressFilterTuple},
    qemu::{ArchExtras, CPU, CallingConvention, Qemu, QemuRWError, QemuShutdownCause},
    sync_exit::CustomInsn,
};
use core::fmt::{self, Debug, Display, Formatter};
use libaflmm::{
    Result, executors::ExitKind, inputs::Input, observers::ObserversTuple, states::State,
};
use libaflmm_qemu_sys::{GuestAddr, GuestPhysAddr, GuestVirtAddr};
#[cfg(feature = "usermode")]
use libaflmm_qemu_sys::{MmapPerms, VerifyAccess};
use std::{ops::Add, result};

pub mod standard;
pub use standard::{StdEmulator, StdEmulatorBuilder};

pub mod hooks;
pub use hooks::{EmulatorHooks, EmulatorModules};

pub mod drivers;
pub use drivers::{
    EmulatorDriver, EmulatorDriverError, EmulatorDriverResult, GenericEmulatorDriver, InputSetter,
    LqemuInputSetter, MapKind, NopEmulatorDriver, NopInputSetter, StdEmulatorDriver,
    StdEmulatorDriverBuilder, StdInputSetter,
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

// forward Emulator trait calls to the Qemu function with a similar name.
macro_rules! forward {
    () => {};

    // safe, &self
    (
        $(#[$m:meta])*
        fn $name:ident $(<$($gen:tt)*>)? (&self $(, $arg:ident: $ty:ty)* $(,)?) $(-> $ret:ty)?;
        $($rest:tt)*
    ) => {
        $(#[$m])*
        fn $name $(<$($gen)*>)? (&self $(, $arg: $ty)*) $(-> $ret)? {
            self.qemu().$name($($arg),*)
        }
        forward!($($rest)*);
    };

    // safe, &mut self
    (
        $(#[$m:meta])*
        fn $name:ident $(<$($gen:tt)*>)? (&mut self $(, $arg:ident: $ty:ty)* $(,)?) $(-> $ret:ty)?;
        $($rest:tt)*
    ) => {
        $(#[$m])*
        fn $name $(<$($gen)*>)? (&mut self $(, $arg: $ty)*) $(-> $ret)? {
            self.qemu().$name($($arg),*)
        }
        forward!($($rest)*);
    };

    // unsafe, &self
    (
        $(#[$m:meta])*
        unsafe fn $name:ident $(<$($gen:tt)*>)? (&self $(, $arg:ident: $ty:ty)* $(,)?) $(-> $ret:ty)?;
        $($rest:tt)*
    ) => {
        $(#[$m])*
        unsafe fn $name $(<$($gen)*>)? (&self $(, $arg: $ty)*) $(-> $ret)? {
            unsafe { self.qemu().$name($($arg),*) }
        }
        forward!($($rest)*);
    };

    // unsafe, &mut self
    (
        $(#[$m:meta])*
        unsafe fn $name:ident $(<$($gen:tt)*>)? (&mut self $(, $arg:ident: $ty:ty)* $(,)?) $(-> $ret:ty)?;
        $($rest:tt)*
    ) => {
        $(#[$m])*
        unsafe fn $name $(<$($gen)*>)? (&mut self $(, $arg: $ty)*) $(-> $ret)? {
            unsafe { self.qemu().$name($($arg),*) }
        }
        forward!($($rest)*);
    };
}

pub trait Emulator {
    type Input: Input + Unpin;
    type State: State + Unpin;

    type Command: Command;
    type CommandManager: CommandManager<Commands = Self::Command> + IsStdCommandManager;
    type Driver: EmulatorDriver;
    type Modules: EmulatorModuleTuple<Self::Input, Self::State> + HasAddressFilterTuple + Unpin;
    type SnapshotManager: SnapshotManager;

    fn first_exec(&mut self, state: &mut Self::State) -> Result<()>;

    fn pre_exec(&mut self, state: &mut Self::State, input: &Self::Input) -> Result<()>;

    fn exec_input(&mut self, input: &Self::Input) -> Result<ExitKind>;

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

    fn add_breakpoint(&self, bp: Breakpoint<Self::Command>, enable: bool) -> BreakpointId;

    fn remove_breakpoint(&self, bp_id: BreakpointId);

    fn driver_mut(&mut self) -> &mut Self::Driver;

    fn snapshot_manager_mut(&mut self) -> &mut Self::SnapshotManager;

    fn command_manager_mut(&mut self) -> &mut Self::CommandManager;

    fn modules_mut(&mut self) -> &mut EmulatorModules<Self::Modules, Self::Input, Self::State>;

    fn started(&self) -> bool;

    unsafe fn read_mem_val<T>(&self, addr: GuestAddr) -> result::Result<T, QemuRWError> {
        unsafe { self.qemu().read_mem_val(addr) }
    }

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

    forward! {
        fn num_cpus(&self) -> usize;

        fn current_cpu(&self) -> Option<CPU>;

        fn cpu_from_index(&self, idx: usize) -> Option<CPU>;

        fn page_from_addr(&self, addr: GuestAddr) -> GuestAddr;

        fn read_mem(&self, addr: GuestAddr, buf: &mut [u8]) -> result::Result<(), QemuRWError>;

        fn read_mem_vec(&self, addr: GuestAddr, len: usize) -> result::Result<Vec<u8>, QemuRWError>;

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
    forward! {
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

        fn mprotect(&self, addr: GuestAddr, size: usize, perms: MmapPerms) -> result::Result<(), String>;

        fn unmap(&self, addr: GuestAddr, size: usize) -> result::Result<(), String>;

        unsafe fn set_target_crash_handling(&self, handling: &TargetSignalHandling);
    }

    #[cfg(feature = "systemmode")]
    forward! {
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

#[derive(Debug, Clone)]
pub enum EmulatorExitError {
    UnknownKind,
    UnexpectedExit,
    CommandError(CommandError),
    BreakpointNotFound(GuestAddr),
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

impl From<SnapshotManagerError> for EmulatorDriverError {
    fn from(sm_error: SnapshotManagerError) -> Self {
        EmulatorDriverError::SMError(sm_error)
    }
}

impl From<SnapshotManagerCheckError> for EmulatorDriverError {
    fn from(sm_check_error: SnapshotManagerCheckError) -> Self {
        EmulatorDriverError::SMCheckError(sm_check_error)
    }
}

impl From<EmulatorExitError> for EmulatorDriverError {
    fn from(error: EmulatorExitError) -> Self {
        EmulatorDriverError::QemuExitReasonError(error)
    }
}

impl From<CommandError> for EmulatorDriverError {
    fn from(error: CommandError) -> Self {
        EmulatorDriverError::CommandError(error)
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

impl From<CommandError> for EmulatorExitError {
    fn from(error: CommandError) -> Self {
        EmulatorExitError::CommandError(error)
    }
}
