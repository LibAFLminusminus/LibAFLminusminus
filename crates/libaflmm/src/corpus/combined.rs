//! A cached corpus, using a given [`Cache`] policy and two [`Store`]s.

use super::{Corpus, HasScheduler, Testcase, store::Store};
use crate::{
    DependencyResolver,
    corpus::{Cache, Scheduler, TestcaseId, store::StorageResult},
};
use alloc::{rc::Rc, vec::Vec};
use core::{cell::RefCell, marker::PhantomData};
use libaflmm_core::Result;
use serde::{Deserialize, Serialize};

/// A [`CombinedCorpus`] tries first to use the main store according to some policy.
/// If it fails, it falls back to the secondary store.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CombinedCorpus<C, CS, FS, I, SC> {
    /// The cache store
    cache_store: RefCell<CS>,
    /// The fallback store
    fallback_store: FS,
    /// The policy taking decisions
    cache: Rc<RefCell<C>>,
    /// The keys in order (use `Vec::binary_search`)
    keys: Vec<TestcaseId>,
    /// The current ID
    current: Option<TestcaseId>,
    /// The scheduler
    scheduler: SC,
    phantom: PhantomData<I>,
}

impl<C, CS, FS, I, SC> CombinedCorpus<C, CS, FS, I, SC> {
    /// Create a new [`CombinedCorpus`].
    pub fn new(scheduler: SC, cache: C, cache_store: CS, fallback_store: FS) -> Self {
        Self {
            cache: Rc::new(RefCell::new(cache)),
            cache_store: RefCell::new(cache_store),
            fallback_store,
            keys: Vec::new(),
            current: None,
            scheduler,
            phantom: PhantomData,
        }
    }

    /// Get the fallback store reference
    pub fn fallback_store(&self) -> &FS {
        &self.fallback_store
    }
}

impl<C, CS, FS, I, SC> DependencyResolver for CombinedCorpus<C, CS, FS, I, SC> {}

impl<C, CS, FS, I, SC> HasScheduler for CombinedCorpus<C, CS, FS, I, SC>
where
    SC: Scheduler,
{
    type Scheduler = SC;

    fn scheduler(&self) -> &Self::Scheduler {
        &self.scheduler
    }
    fn scheduler_mut(&mut self) -> &mut Self::Scheduler {
        &mut self.scheduler
    }
}

impl<C, CS, FS, I, SC> Corpus for CombinedCorpus<C, CS, FS, I, SC>
where
    C: Cache<CS, FS, I>,
    CS: Store<I>,
    FS: Store<I>,
    I: Clone,
    SC: Scheduler,
{
    type Input = I;

    fn count(&self) -> usize {
        self.fallback_store.count()
    }

    fn count_disabled(&self) -> usize {
        self.fallback_store.count_disabled()
    }

    fn count_all(&self) -> usize {
        self.fallback_store.count_all()
    }

    fn add_shared<const ENABLED: bool>(&mut self, testcase: Testcase<I>) -> Result<TestcaseId> {
        let id = match self.cache.borrow_mut().add_shared::<ENABLED>(
            testcase,
            &mut *self.cache_store.borrow_mut(),
            &mut self.fallback_store,
        )? {
            StorageResult::Stored(id) => {
                self.scheduler.on_add(id)?;
                id
            }
            StorageResult::Duplicate(id) => id,
        };

        Ok(id)
    }

    fn get_from<const ENABLED: bool>(&self, id: &TestcaseId) -> Result<Testcase<I>> {
        let mut cache = self.cache.borrow_mut();
        let cache_store = &mut *self.cache_store.borrow_mut();

        cache.get_from::<ENABLED>(id, cache_store, &self.fallback_store)
    }
}
