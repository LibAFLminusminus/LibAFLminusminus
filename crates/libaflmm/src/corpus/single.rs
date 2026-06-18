//! A simple corpus, with a backing store.
//!
//! A [`SingleCorpus`] owns a single store, in which every testcase is added.

use alloc::vec::Vec;
use core::marker::PhantomData;

use libaflmm_core::Result;
use serde::{Deserialize, Serialize};

use super::{Corpus, Testcase, store::Store};
use crate::{
    common::DependencyResolver,
    corpus::{
        DisableEntry, NopScheduler, ObjectiveCorpus, ScheduledCorpus, Scheduler,
        schedulers::RemovableScheduler, store::StorageResult, testcase::TestcaseId,
    },
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

impl<I, S, SC> DependencyResolver for SingleCorpus<I, S, SC> {}

impl<I, S, SC> Corpus<I> for SingleCorpus<I, S, SC>
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

    fn add_inner<const ENABLED: bool>(&mut self, testcase: Testcase<I>) -> Result<TestcaseId> {
        self.store
            .add_shared::<ENABLED>(testcase)
            .map(|res| res.into_testcase_id())
    }

    /// Get testcase by id
    fn get_from<const ENABLED: bool>(&self, id: &TestcaseId) -> Result<Testcase<I>> {
        self.store.get_from::<ENABLED>(id)
    }
}

impl<I, S> ObjectiveCorpus<I> for SingleCorpus<I, S, NopScheduler> where S: Store<I> {}

impl<I, S, SC> ScheduledCorpus<I, SC> for SingleCorpus<I, S, SC>
where
    S: Store<I>,
    SC: Scheduler,
{
    fn add_shared<const ENABLED: bool>(&mut self, testcase: Testcase<I>) -> Result<TestcaseId> {
        let id = match self.store.add_shared::<ENABLED>(testcase)? {
            StorageResult::Stored(id) => {
                self.scheduler.on_add(id)?;
                id
            }
            StorageResult::Duplicate(id) => id,
        };

        Ok(id)
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
    fn disable(&mut self, id: &TestcaseId) -> Result<()> {
        self.store.disable(id)
    }
}
