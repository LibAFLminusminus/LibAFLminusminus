//! A simple corpus, with a backing store.
//!
//! A [`SingleCorpus`] owns a single store, in which every testcase is added.

use alloc::{rc::Rc, vec::Vec};
use core::marker::PhantomData;

use libafl_bolts::Error;
use serde::{Deserialize, Serialize};

use super::{Corpus, Testcase, store::Store};
use crate::{
    DependencyResolver,
    corpus::{schedulers::RemovableScheduler, testcase::TestcaseId},
};

/// You average corpus.
/// It has one backing store, used to store / retrieve testcases.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SingleCorpus<I, S, SC> {
    /// The backing testcase store
    store: S,
    /// The scheduler
    scheduler: SC,
    /// The keys in order (use `Vec::binary_search`)
    keys: Vec<TestcaseId>,
    /// The current ID
    current: Option<TestcaseId>,
    phantom: PhantomData<I>,
}

impl<I, S, SC> Default for SingleCorpus<I, S, SC>
where
    S: Default,
    SC: Default,
{
    fn default() -> Self {
        Self::new(S::default(), SC::default())
    }
}

impl<I, S, SC> SingleCorpus<I, S, SC> {
    /// Create a new [`SingleCorpus`]
    pub fn new(store: S, scheduler: SC) -> Self {
        Self {
            store,
            scheduler,
            keys: Vec::default(),
            current: None,
            phantom: PhantomData,
        }
    }
}

pub trait DisableEntry {
    /// Disable a corpus entry
    fn disable(&mut self, id: TestcaseId) -> Result<(), Error>;
}

impl<I, S, SC> DependencyResolver for SingleCorpus<I, S, SC> {}

impl<I, S, SC> Corpus<I, SC> for SingleCorpus<I, S, SC>
where
    S: Store<I>,
{
    fn count(&self) -> usize {
        self.store.count()
    }

    fn count_disabled(&self) -> usize {
        self.store.count_disabled()
    }

    fn count_all(&self) -> usize {
        self.store.count_all()
    }

    fn add_shared<const ENABLED: bool>(&mut self, input: Rc<I>) -> Result<TestcaseId, Error> {
        let new_id = self.store.add_shared::<ENABLED>(input)?;
        Ok(new_id)
    }

    /// Get testcase by id
    fn get_from<const ENABLED: bool>(&self, id: TestcaseId) -> Result<Testcase<I>, Error> {
        self.store.get_from::<ENABLED>(id)
    }

    fn scheduler(&self) -> &SC {
        &self.scheduler
    }

    fn scheduler_mut(&mut self) -> &mut SC {
        &mut self.scheduler
    }
}

impl<I, S, SC> DisableEntry for SingleCorpus<I, S, SC>
where
    S: Store<I>,
    SC: RemovableScheduler<I, S>,
{
    fn disable(&mut self, id: TestcaseId) -> Result<(), Error> {
        self.store.disable(id)
    }
}
