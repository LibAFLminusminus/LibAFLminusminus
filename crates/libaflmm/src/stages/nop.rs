//! A [`NopStage`] does nothing

use alloc::borrow::Cow;

use libaflmm_bolts::Named;

use super::Stage;
use crate::{
    DependencyResolver, Result, corpus::TestcaseId, runtimes::RuntimeHandle, states::CoreState,
};

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

impl Named for NopStage {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("nop");
        &NAME
    }
}

impl<E, R, S, W, Z> Stage<E, R, S, W, Z> for NopStage
where
    S: CoreState,
{
    fn perform_impl(
        &mut self,
        _fuzzer: &mut Z,
        _executor: &mut E,
        _rand: &mut R,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
        _testcase_id: &TestcaseId,
    ) -> Result<()> {
        Ok(())
    }
}
