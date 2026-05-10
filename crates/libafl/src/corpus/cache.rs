//! A collection of cache policy implementations.
//! They are meant to be used by [`crate::corpus::CombinedCorpus`].
//!
//! Caches are acting on two [`Store`]s:
//!     - a **cache store** holding on the testcases with quick access.
//!     - a **backing store** with more expensive access, used when the testcase cannot be found in the cache store.

use alloc::{collections::VecDeque, rc::Rc, vec::Vec};
use core::{cell::RefCell, marker::PhantomData};

use libafl_bolts::Error;
use serde::{Deserialize, Serialize};

use crate::{
    corpus::{
        Testcase, TestcaseId,
        maps::InMemoryCorpusMap,
        store::{RemovableStore, StorageResult, Store},
    },
    inputs::Input,
};

/// A cache, managing a cache store and a fallback store.
pub trait Cache<CS, FS, I> {
    /// Add a testcase to the cache
    fn add_shared<const ENABLED: bool>(
        &mut self,
        testcase: Testcase<I>,
        cache_store: &mut CS,
        fallback_store: &mut FS,
    ) -> Result<StorageResult, Error>;

    /// Get a testcase from the cache
    fn get_from<const ENABLED: bool>(
        &mut self,
        id: &TestcaseId,
        cache_store: &mut CS,
        fallback_store: &FS,
    ) -> Result<Testcase<I>, Error>;

    /// Disable an entry
    fn disable(
        &mut self,
        id: &TestcaseId,
        cache_store: &mut CS,
        fallback_store: &mut FS,
    ) -> Result<(), Error>;
}

/// An identity cache, storing everything both in the cache and the backing store.
#[derive(Debug, Serialize, Deserialize)]
pub struct IdentityCache;

/// A `First In / First Out` cache policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FifoCache<CS, FS, I> {
    cached_ids: VecDeque<TestcaseId>,
    cache_max_len: usize,
    phantom: PhantomData<(I, CS, FS)>,
}

impl<CS, FS, I> FifoCache<CS, FS, I> {
    /// Create a new [`FifoCache`], with at most `cache_max_len` [`Testcase`]s loaded in memory.
    #[must_use]
    pub fn new(cache_max_len: usize) -> Self {
        Self {
            cached_ids: VecDeque::default(),
            cache_max_len,
            phantom: PhantomData,
        }
    }
}

impl<CS, FS, I> Cache<CS, FS, I> for IdentityCache
where
    CS: RemovableStore<I>,
    FS: Store<I>,
    I: Input,
{
    fn add_shared<const ENABLED: bool>(
        &mut self,
        testcase: Testcase<I>,
        cache_store: &mut CS,
        fallback_store: &mut FS,
    ) -> Result<StorageResult, Error> {
        cache_store.add_shared::<ENABLED>(testcase.clone())?;
        fallback_store.add_shared::<ENABLED>(testcase)
    }

    fn get_from<const ENABLED: bool>(
        &mut self,
        id: &TestcaseId,
        cache_store: &mut CS,
        fallback_store: &FS,
    ) -> Result<Testcase<I>, Error> {
        match cache_store.get(id) {
            Ok(tc) => Ok(tc),
            Err(Error::KeyNotFound(_, _)) => {
                let fb_tc = fallback_store.get_from::<ENABLED>(id)?;
                cache_store.add_shared::<ENABLED>(fb_tc.clone())?;
                Ok(fb_tc)
            }
            Err(e) => Err(e),
        }
    }

    fn disable(
        &mut self,
        id: &TestcaseId,
        cache_store: &mut CS,
        fallback_store: &mut FS,
    ) -> Result<(), Error> {
        cache_store.disable(id)?;
        fallback_store.disable(id)
    }
}

impl<CS, FS, I> Cache<CS, FS, I> for FifoCache<CS, FS, I>
where
    CS: RemovableStore<I>,
    FS: Store<I>,
    I: Clone,
{
    fn add_shared<const ENABLED: bool>(
        &mut self,
        testcase: Testcase<I>,
        _cache_store: &mut CS,
        fallback_store: &mut FS,
    ) -> Result<StorageResult, Error> {
        fallback_store.add_shared::<ENABLED>(testcase)
    }

    fn get_from<const ENABLED: bool>(
        &mut self,
        id: &TestcaseId,
        cache_store: &mut CS,
        fallback_store: &FS,
    ) -> Result<Testcase<I>, Error> {
        if self.cached_ids.contains(&id) {
            cache_store.get(id)
        } else {
            if self.cached_ids.len() == self.cache_max_len {
                let to_evict = self.cached_ids.pop_back().unwrap();
                cache_store.remove(&to_evict)?;
            }

            debug_assert!(self.cached_ids.len() < self.cache_max_len);

            // tescase is not cached, fetch it from fallback
            let fb_tc = fallback_store.get_from::<ENABLED>(id)?;
            let fb_tc_id = fb_tc.id().clone();

            cache_store.add_shared::<ENABLED>(fb_tc)?;

            self.cached_ids.push_front(fb_tc_id.clone());

            cache_store.get(&fb_tc_id)
        }
    }

    fn disable(
        &mut self,
        id: &TestcaseId,
        cache_store: &mut CS,
        fallback_store: &mut FS,
    ) -> Result<(), Error> {
        cache_store.disable(id)?;
        fallback_store.disable(id)
    }
}
