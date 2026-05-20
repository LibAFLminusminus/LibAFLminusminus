//! Schedule the access to the Corpus.

use crate::common::DependencyResolver;
use crate::corpus::{Testcase, testcase::TestcaseId};
use alloc::vec::Vec;
use core::fmt::Debug;
use libaflmm_bolts::rands::{Rand, StdRand};
use libaflmm_core::{Result, empty, non_zero};
use serde::{Deserialize, Serialize};

pub mod queue;
pub use queue::QueueScheduler;

/// A [`StdScheduler`] uses the default scheduler in `LibAFL` to schedule [`Testcase`]s.
///
/// The current `Std` is a [`RandScheduler`], although this may change in the future, if another [`Scheduler`] delivers better results.
pub type StdScheduler = RandScheduler<StdRand>;

/// The scheduler define how the fuzzer requests a testcase from the corpus.
/// It has hooks to corpus add/replace/remove to allow complex scheduling algorithms to collect data.
pub trait Scheduler: DependencyResolver {
    /// Called when a [`Testcase`] is added to the corpus
    /// You need to keep a vector of all the ids in your scheduler when you add a testcase to the corpus and schedulers
    fn on_add(&mut self, _id: TestcaseId) -> Result<()>;

    /// Get the [`TestcaseId`] of the input currently scheduled.
    fn current(&self) -> Option<TestcaseId>;

    /// Gets the [`TestcaseId`] of the next input to schedule.
    fn next(&mut self) -> Result<TestcaseId>;

    /// Returns all [`TestcaseId`]s tracked by this scheduler.
    fn ids(&self) -> &[TestcaseId];
}

/// The scheduler also implements `on_remove` and `on_replace` if it implements this stage.
pub trait RemovableScheduler<I, S> {
    /// Removed the given entry from the corpus at the given index
    /// When you remove testcases, make sure that that testcase is not currently fuzzed one!
    fn on_remove(&mut self, _id: TestcaseId, _testcase: &Option<Testcase<I>>) -> Result<()> {
        Ok(())
    }

    /// Replaced the given testcase at the given idx
    fn on_replace(&mut self, _id: TestcaseId, _prev: &Testcase<I>) -> Result<()> {
        Ok(())
    }
}

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
    fn next(&mut self) -> Result<TestcaseId> {
        if self.ids.is_empty() {
            Err(empty!(
                "No entries in corpus. This often implies the target is not properly instrumented."
            ))
        } else {
            let idx = self.rand.below(non_zero!(self.ids.len()));
            let id = self.ids[idx];

            self.current = Some(id);

            log::warn!(
                "There was a call to set_current_scheduled here, what should we do? (cf comments below)"
            );
            Ok(id)
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

/// A nop [`Scheduler`], which does not schedule anything.
#[derive(Debug, Serialize, Deserialize)]
pub struct NopScheduler;

impl DependencyResolver for NopScheduler {}

impl Scheduler for NopScheduler {
    fn on_add(&mut self, _id: TestcaseId) -> Result<()> {
        Ok(())
    }

    fn current(&self) -> Option<TestcaseId> {
        None
    }

    fn next(&mut self) -> Result<TestcaseId> {
        panic!("NopScheduler does not schedule")
    }

    fn ids(&self) -> &[TestcaseId] {
        &[]
    }
}
