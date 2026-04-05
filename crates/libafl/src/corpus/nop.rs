//! The null corpus does not store any [`Testcase`]s.

use alloc::rc::Rc;
use core::marker::PhantomData;

use serde::{Deserialize, Serialize};

use crate::{
    Error,
    corpus::{Corpus, CorpusId, HasScheduler, Testcase, schedulers::NopScheduler},
};

/// A corpus which does not store any [`Testcase`]s.
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct NopCorpus<I, S> {
    empty: Option<CorpusId>,
    phantom: PhantomData<(I, S)>,
}

impl<I, S> HasScheduler<I, S> for NopCorpus<I, S> {
    type Scheduler = NopScheduler;

    fn scheduler(&self) -> &Self::Scheduler {
        &NopScheduler::new()
    }

    fn scheduler_mut(&mut self) -> &mut Self::Scheduler {
        &mut NopScheduler::new()
    }
}

impl<I, S> Corpus<I> for NopCorpus<I, S> {
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
    fn add_shared<const ENABLED: bool>(&mut self, _input: Rc<I>) -> Result<CorpusId, Error> {
        Err(Error::unsupported("Unsupported by NopCorpus"))
    }

    fn get_from<const ENABLED: bool>(&self, _id: CorpusId) -> Result<Testcase<I>, Error> {
        Err(Error::unsupported("Unsupported by NopCorpus"))
    }
}

impl<I, S> NopCorpus<I, S> {
    /// Creates a new [`NopCorpus`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            empty: None,
            phantom: PhantomData {},
        }
    }
}
