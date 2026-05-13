//! A [`NopStage`] does nothing

use super::Stage;
use crate::{DependencyResolver, corpus::TestcaseId, runtimes::RuntimeHandle};

/// A [`Stage`] that does nothing
#[derive(Debug, Copy, Clone, Default)]
pub struct NopStage {}

impl NopStage {
    /// Create a [`struct@NopStage`]
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl DependencyResolver for NopStage {}

impl<E, R, S, W, Z> Stage<E, R, S, W, Z> for NopStage {
    fn perform(
        &mut self,
        _fuzzer: &mut Z,
        _executor: &mut E,
        _rand: &mut R,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
        _testcase_id: &TestcaseId,
    ) -> Result<(), libaflmm_bolts::Error> {
        Ok(())
    }
}
