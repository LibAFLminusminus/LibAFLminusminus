//! Schedule the access to the Corpus.

use crate::common::DependencyResolver;
use crate::corpus::{Testcase, testcase::TestcaseId};
use core::fmt::Debug;
use libaflmm_bolts::rands::StdRand;
use libaflmm_core::Result;
use serde::{Deserialize, Serialize};

pub mod queue;
pub use queue::QueueScheduler;

pub mod rand;
pub use rand::RandScheduler;

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
    /// Returns `None` if there is nothing to schedule anymore.
    fn next(&mut self) -> Result<Option<TestcaseId>>;

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

    fn next(&mut self) -> Result<Option<TestcaseId>> {
        Ok(None)
    }

    fn ids(&self) -> &[TestcaseId] {
        &[]
    }
}
