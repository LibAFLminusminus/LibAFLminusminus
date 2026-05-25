//! Input writers: set the fuzz input into the emulator before each run.

use crate::{Result, emu::InputLocation, qemu::Qemu};

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

pub trait InputWriter<I, S> {
    /// Set input in the Emulator.
    fn write_input(&mut self, qemu: Qemu, state: &mut S, input: &I) -> Result<()>;

    /// The effective input size
    fn input_size(&self, state: &mut S, input: &I) -> usize;

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

    fn input_size(&self, _state: &mut S, _input: &I) -> usize {
        0
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
