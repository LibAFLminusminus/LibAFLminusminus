//! Higher-level abstraction of [`Qemu`]
//!
//! [`Emulator`] is built above [`Qemu`] and provides convenient abstractions.

use core::fmt::{self, Debug, Display, Formatter};
use std::ops::Add;

use libaflmm::{
    Result, executors::ExitKind, inputs::Input, observers::ObserversTuple, states::State,
};
use libaflmm_qemu_sys::{GuestAddr, GuestPhysAddr, GuestVirtAddr};

use crate::{
    QemuShutdownCause, breakpoint::Breakpoint, command::CommandError, sync_exit::CustomInsn,
};

pub mod standard;
pub use standard::{InputLocation, StdEmulator, StdEmulatorBuilder, StdSnapshotManager};

pub mod hooks;
pub use hooks::*;

pub mod drivers;
pub use drivers::*;

pub mod snapshot;
pub use snapshot::*;

pub trait Emulator {
    type Input: Input;
    type State: State;

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
