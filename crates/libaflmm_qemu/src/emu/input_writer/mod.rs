//! Emulator Drivers, as the name suggests, drive QEMU execution
//! They are used to perform specific actions on the emulator before and / or after QEMU runs.

use crate::{
    Result,
    emu::{EmulatorExitResult, InputLocation},
    qemu::Qemu,
};
use libaflmm::executors::ExitKind;
use std::{fmt::Debug, result};

#[cfg(not(feature = "nyx"))]
pub mod lqemu;
#[cfg(not(feature = "nyx"))]
pub use lqemu::LqemuInputWriter;

#[cfg(feature = "nyx")]
pub mod nyx;
#[cfg(feature = "nyx")]
pub use nyx::StdNyxInputSetter;

#[cfg(not(feature = "nyx"))]
pub type StdInputWriter = LqemuInputWriter;
#[cfg(feature = "nyx")]
pub type StdInputSetter = StdNyxInputSetter;

#[derive(Debug, Clone)]
pub enum EmulatorDriverResult<C> {
    /// Return to the harness immediately. Can happen at any point of the run when the handler is not supposed to handle a request.
    ReturnToClient(EmulatorExitResult<C>),

    /// The run is over and the emulator is ready for the next iteration.
    EndOfRun(ExitKind),

    /// Internal shutdown request
    ShutdownRequest,
}

pub trait InputWriter<I, S> {
    /// Set input in the Emulator.
    fn write_input(&mut self, qemu: Qemu, state: &mut S, input: &I) -> Result<()>;

    /// Set location at which input should be set.
    fn set_input_location(&mut self, location: InputLocation) -> Result<()>;

    /// Get the input location, if it is set.
    fn input_location(&self) -> Option<&InputLocation>;
}

#[derive(Debug, Default)]
pub struct NopInputWriter;

impl<I, S> InputWriter<I, S> for NopInputWriter {
    fn write_input(&mut self, _qemu: Qemu, _state: &mut S, _input: &I) -> Result<()> {
        Ok(())
    }

    fn set_input_location(&mut self, _location: InputLocation) -> Result<()> {
        Ok(())
    }

    fn input_location(&self) -> Option<&InputLocation> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MapKind {
    Cov,
    Cmp,
}

impl<C> TryFrom<EmulatorDriverResult<C>> for ExitKind
where
    C: Debug,
{
    type Error = String;

    fn try_from(value: EmulatorDriverResult<C>) -> result::Result<Self, Self::Error> {
        match value {
            EmulatorDriverResult::ReturnToClient(unhandled_qemu_exit) => {
                Err(format!("Unhandled QEMU exit: {:?}", &unhandled_qemu_exit))
            }
            EmulatorDriverResult::EndOfRun(exit_kind) => Ok(exit_kind),
            EmulatorDriverResult::ShutdownRequest => {
                log::warn!("Shutdown request. Stopping fuzzing...");
                // std::process::exit(CTRL_C_EXIT);
                panic!("Implement proper exit there...")
            }
        }
    }
}
