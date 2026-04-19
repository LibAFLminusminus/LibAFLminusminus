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
    corpus::{Scheduler, schedulers::RemovableScheduler, testcase::TestcaseId},
    inputs::InputContext,
};

/// You average corpus.
/// It has one backing store, used to store / retrieve testcases.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SingleCorpus<CT, I, S, SC> {
    context: CT,
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

impl<CT, I, S, SC> SingleCorpus<CT, I, S, SC> {
    /// Create a new [`SingleCorpus`]
    pub fn new(context: CT, store: S, scheduler: SC) -> Self {
        Self {
            context,
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

impl<CT, I, S, SC> DependencyResolver for SingleCorpus<CT, I, S, SC> {}

impl<CT, I, S, SC> Corpus<I, SC> for SingleCorpus<CT, I, S, SC>
where
    CT: InputContext<I>,
    S: Store<I>,
    SC: Scheduler,
{
    type Context = CT;

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
        self.scheduler.on_add(new_id)?;
        Ok(new_id)
    }

    /// Get testcase by id
    fn get_from<const ENABLED: bool>(&self, id: TestcaseId) -> Result<Testcase<I>, Error> {
        self.store.get_from::<ENABLED>(id)
    }

    fn context(&self) -> &CT {
        &self.context
    }

    fn context_mut(&mut self) -> &mut CT {
        &mut self.context
    }

    fn scheduler(&self) -> &SC {
        &self.scheduler
    }

    fn scheduler_mut(&mut self) -> &mut SC {
        &mut self.scheduler
    }
}

impl<CT, I, S, SC> DisableEntry for SingleCorpus<CT, I, S, SC>
where
    S: Store<I>,
    SC: RemovableScheduler<I, S>,
{
    fn disable(&mut self, id: TestcaseId) -> Result<(), Error> {
        self.store.disable(id)
    }
}
