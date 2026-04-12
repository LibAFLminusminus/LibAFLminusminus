//! A simple corpus, with a backing store.
//!
//! A [`SingleCorpus`] owns a single store, in which every testcase is added.

use alloc::{rc::Rc, vec::Vec};
use core::marker::PhantomData;

use libafl_bolts::Error;
use serde::{Deserialize, Serialize};

use crate::corpus::schedulers::RemovableScheduler;

use super::{Corpus, CorpusCounter, CorpusId, Testcase, store::Store};

/// You average corpus.
/// It has one backing store, used to store / retrieve testcases.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SingleCorpus<I, S, SC> {
    /// The backing testcase store
    store: S,
    /// The scheduler
    scheduler: SC,
    /// The corpus ID counter
    counter: CorpusCounter,
    /// The keys in order (use `Vec::binary_search`)
    keys: Vec<CorpusId>,
    /// The current ID
    current: Option<CorpusId>,
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
            counter: CorpusCounter::default(),
            keys: Vec::default(),
            current: None,
            phantom: PhantomData,
        }
    }
}

pub trait DisableEntry {
    /// Disable a corpus entry
    fn disable(&mut self, id: CorpusId) -> Result<(), Error>;
}

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

    fn add_shared<const ENABLED: bool>(&mut self, input: Rc<I>) -> Result<CorpusId, Error> {
        let new_id = self.counter.new_id();
        self.store.add_shared::<ENABLED>(new_id, input)?;
        Ok(new_id)
    }

    /// Get testcase by id
    fn get_from<const ENABLED: bool>(&self, id: CorpusId) -> Result<Testcase<I>, Error> {
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
    fn disable(&mut self, id: CorpusId) -> Result<(), Error> {
        self.store.disable(id)
    }
}
