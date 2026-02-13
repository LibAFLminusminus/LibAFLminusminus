//! A simple corpus, with a backing store.
//!
//! A [`SingleCorpus`] owns a single store, in which every testcase is added.

use alloc::{rc::Rc, vec::Vec};
use core::marker::PhantomData;

use libafl_bolts::Error;
use serde::{Deserialize, Serialize};

use super::{Corpus, CorpusCounter, CorpusId, Testcase, store::Store};

/// You average corpus.
/// It has one backing store, used to store / retrieve testcases.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SingleCorpus<I, S> {
    /// The backing testcase store
    store: S,
    /// The corpus ID counter
    counter: CorpusCounter,
    /// The keys in order (use `Vec::binary_search`)
    keys: Vec<CorpusId>,
    /// The current ID
    current: Option<CorpusId>,
    phantom: PhantomData<I>,
}

impl<I, S> Default for SingleCorpus<I, S>
where
    S: Default,
{
    fn default() -> Self {
        Self::new(S::default())
    }
}

impl<I, S> SingleCorpus<I, S> {
    /// Create a new [`SingleCorpus`]
    pub fn new(store: S) -> Self {
        Self {
            store,
            counter: CorpusCounter::default(),
            keys: Vec::default(),
            current: None,
            phantom: PhantomData,
        }
    }
}

impl<I, S> Corpus<I> for SingleCorpus<I, S>
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

    fn disable(&mut self, id: CorpusId) -> Result<(), Error> {
        self.store.disable(id)
    }
}
