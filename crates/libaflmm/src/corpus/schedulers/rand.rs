use crate::{
    Result,
    common::DependencyResolver,
    corpus::{Scheduler, TestcaseId},
};
use libaflmm_bolts::Rand;
use libaflmm_core::non_zero;

/// Feed the fuzzer simply with a random testcase on request
#[derive(Debug, Clone)]
pub struct RandScheduler<R> {
    rand: R,
    current: Option<TestcaseId>,
    ids: Vec<TestcaseId>,
}

impl<R> DependencyResolver for RandScheduler<R> {}

impl<R> Scheduler for RandScheduler<R>
where
    R: Rand,
{
    fn on_add(&mut self, id: TestcaseId) -> Result<()> {
        self.ids.push(id);
        Ok(())
    }

    fn current(&self) -> Option<TestcaseId> {
        self.current
    }

    /// Gets the next entry at random
    fn next(&mut self) -> Result<Option<TestcaseId>> {
        if self.ids.is_empty() {
            Ok(None)
        } else {
            let idx = self.rand.below(non_zero!(self.ids.len()));
            let id = self.ids[idx];

            self.current = Some(id);

            log::warn!(
                "There was a call to set_current_scheduled here, what should we do? (cf comments below)"
            );
            Ok(Some(id))
        }
    }

    fn ids(&self) -> &[TestcaseId] {
        &self.ids
    }
}

impl<R> RandScheduler<R> {
    /// Create a new [`RandScheduler`] that just schedules randomly.
    #[must_use]
    pub fn new(rand: R) -> Self {
        Self {
            rand,
            current: None,
            ids: Vec::new(),
        }
    }
}
