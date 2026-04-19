//! A nop stage does nothing

use super::Stage;
use crate::{DependencyResolver, runtimes::RuntimeHandle};

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

impl<CT, E, S, Z> Stage<CT, E, S, Z> for NopStage {
    fn perform(
        &mut self,
        _fuzzer: &mut Z,
        _executor: &mut E,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<CT, S>,
    ) -> Result<(), libafl_bolts::Error> {
        Ok(())
    }
}
