//! Schedule the access to the Corpus.

use alloc::borrow::ToOwned;
use libafl_core::non_zero;
use serde::{Deserialize, Serialize};
use std::vec::Vec;

pub mod testcase_score;
pub use testcase_score::{LenTimeMulTestcasePenalty, TestcasePenalty, TestcaseScore};

pub mod queue;
pub use queue::QueueScheduler;

use libafl_bolts::{
    rands::{Rand, StdRand},
    tuples::MatchName,
};

use crate::{
    DependencyResolver, Error,
    corpus::{CorpusId, Testcase},
};

/// The scheduler also implements `on_remove` and `on_replace` if it implements this stage.
pub trait RemovableScheduler<I, S> {
    /// Removed the given entry from the corpus at the given index
    /// When you remove testcases, make sure that that testcase is not currently fuzzed one!
    fn on_remove(
        &mut self,
        _id: CorpusId,
        _testcase: &Option<Testcase<I>>,
    ) -> Result<(), Error> {
        Ok(())
    }

    /// Replaced the given testcase at the given idx
    fn on_replace(
        &mut self,
        _id: CorpusId,
        _prev: &Testcase<I>,
    ) -> Result<(), Error> {
        Ok(())
    }
}

/// The scheduler define how the fuzzer requests a testcase from the corpus.
/// It has hooks to corpus add/replace/remove to allow complex scheduling algorithms to collect data.
pub trait Scheduler: DependencyResolver {
    /// Called when a [`Testcase`] is added to the corpus
    fn on_add(&mut self, _id: CorpusId) -> Result<(), Error>;
    // Add parent_id here if it has no inner

    /// An input has been evaluated
    fn on_evaluation<OT>(
        &mut self,
        _observers: &OT,
    ) -> Result<(), Error>
    where
        OT: MatchName,
    {
        Ok(())
    }

    // Get the current input
    fn current(&self) -> Option<CorpusId>;

    /// Gets the next entry
    fn next(&mut self) -> Result<CorpusId, Error>;
}

/// Feed the fuzzer simply with a random testcase on request
#[derive(Debug, Clone)]
pub struct RandScheduler<R> {
    rand: R,
    current: Option<CorpusId>,
    ids: Vec<CorpusId>,
}

impl<R> DependencyResolver for RandScheduler<R> {}

impl<R> Scheduler for RandScheduler<R>
where
    R: Rand,
{
    fn on_add(&mut self, id: CorpusId) -> Result<(), Error> {
        self.ids.push(id);
        Ok(())
    }

    fn current(&self) -> Option<CorpusId> {
        self.current.clone()
    }

    /// Gets the next entry at random
    fn next(&mut self) -> Result<CorpusId, Error> {
        if self.ids.is_empty() {
            Err(Error::empty(
                "No entries in corpus. This often implies the target is not properly instrumented."
                    .to_owned(),
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

/// A [`StdScheduler`] uses the default scheduler in `LibAFL` to schedule [`Testcase`]s.
///
/// The current `Std` is a [`RandScheduler`], although this may change in the future, if another [`Scheduler`] delivers better results.
pub type StdScheduler = RandScheduler<StdRand>;

#[derive(Debug, Serialize, Deserialize)]
pub struct NopScheduler;

impl DependencyResolver for NopScheduler {}

impl Scheduler for NopScheduler {
    fn on_add(&mut self, _id: CorpusId) -> Result<(), Error> {
        panic!("NopScheduler does not schedule")
    }

    fn current(&self) -> Option<CorpusId> {
        panic!("NopScheduler does not schedule")
    }

    fn next(&mut self) -> Result<CorpusId, Error> {
        panic!("NopScheduler does not schedule")
    }
}
