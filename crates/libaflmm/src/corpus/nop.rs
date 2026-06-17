//! The null corpus does not store any [`Testcase`]s.

use core::marker::PhantomData;

use libaflmm_core::Result;
use serde::{Deserialize, Serialize};

use crate::{
    Error,
    common::DependencyResolver,
    corpus::{Corpus, HasScheduler, Testcase, TestcaseId, schedulers::NopScheduler},
    inputs::NopContext,
};

/// A corpus which does not store any [`Testcase`]s.
#[derive(Serialize, Deserialize, Debug)]
pub struct NopCorpus<I, S> {
    scheduler: NopScheduler,
    context: NopContext,
    phantom: PhantomData<(I, S)>,
}

impl<I, S> HasScheduler<NopScheduler> for NopCorpus<I, S> {
    fn scheduler(&self) -> &NopScheduler {
        &self.scheduler
    }

    fn scheduler_mut(&mut self) -> &mut NopScheduler {
        &mut self.scheduler
    }
}

impl<I, S> Corpus<I, NopScheduler> for NopCorpus<I, S> {
    /// Returns the number of all enabled entries
    #[inline]
    fn count(&self) -> usize {
        0
    }

    /// Returns the number of all disabled entries
    fn count_disabled(&self) -> usize {
        0
    }

    /// Returns the number of all entries
    #[inline]
    fn count_all(&self) -> usize {
        0
    }

    /// Add an enabled testcase to the corpus and return its index
    #[inline]
    fn add_shared<const ENABLED: bool>(&mut self, _testcase: Testcase<I>) -> Result<TestcaseId> {
        Err(Error::unsupported("Unsupported by NopCorpus"))
    }

    fn get_from<const ENABLED: bool>(&self, _id: &TestcaseId) -> Result<Testcase<I>> {
        Err(Error::unsupported("Unsupported by NopCorpus"))
    }
}

impl<I, S> DependencyResolver for NopCorpus<I, S> {}

impl<I, S> Default for NopCorpus<I, S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I, S> NopCorpus<I, S> {
    /// Creates a new [`NopCorpus`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            context: NopContext {},
            scheduler: NopScheduler {},
            phantom: PhantomData {},
        }
    }
}
