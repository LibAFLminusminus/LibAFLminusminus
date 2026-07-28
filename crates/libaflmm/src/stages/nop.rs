//! A [`NopStage`] does nothing
//! It can optionally sleep before continuing.

use alloc::borrow::Cow;
use core::time::Duration;
use libaflmm_bolts::Named;
use std::thread;

use super::Stage;
use crate::{
    Result, common::DependencyResolver, corpus::TestcaseId, runtimes::RuntimeHandle, states::State,
};

/// A [`Stage`] that does nothing
#[derive(Debug, Copy, Clone, Default)]
pub struct NopStage {
    sleep: Option<Duration>,
}

impl NopStage {
    pub fn with_sleep(sleep: Duration) -> Self {
        Self { sleep: Some(sleep) }
    }

    pub fn nop() -> Self {
        Self { sleep: None }
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
    S: State,
{
    fn perform_impl(
        &mut self,
        _fuzzer: &mut Z,
        _rand: &mut R,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
        _testcase_id: &TestcaseId,
    ) -> Result<()> {
        if let Some(sleep) = self.sleep {
            thread::sleep(sleep);
        }

        Ok(())
    }
}
