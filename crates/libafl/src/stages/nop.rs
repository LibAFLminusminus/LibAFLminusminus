//! A nop stage does nothing

use crate::DependencyResolver;

use super::Stage;

/// A stage that does nothing
#[derive(Debug, Copy, Clone, Default)]
pub struct NopStage {}

impl NopStage {
    /// Create a [`NopStage`]
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl DependencyResolver for NopStage {}

impl<C, E, S, Z> Stage<C, E, S, Z> for NopStage {
    fn perform(
        &mut self,
        _fuzzer: &mut Z,
        _executor: &mut E,
        _state: &mut S,
        _controller: &mut C,
    ) -> Result<(), libafl_bolts::Error> {
        Ok(())
    }
}
