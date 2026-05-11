//! An in-memory store

use super::{InMemoryCorpusMap, RemovableStore, Store};
use crate::{
    corpus::{Testcase, store::StorageResult, testcase::TestcaseId},
    inputs::Input,
};
use core::marker::PhantomData;
use libafl_bolts::Error;
use libafl_core::Result;
use serde::{Deserialize, Serialize};

/// The map type in which testcases are stored (disable the feature `corpus_btreemap` to use a `HashMap` instead of `BTreeMap`)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InMemoryStore<I, M> {
    enabled_map: M,
    disabled_map: M,
    phantom: PhantomData<I>,
}

impl<I, M> Default for InMemoryStore<I, M>
where
    M: Default,
{
    fn default() -> Self {
        Self {
            enabled_map: M::default(),
            disabled_map: M::default(),
            phantom: PhantomData,
        }
    }
}

impl<I, M> Store<I> for InMemoryStore<I, M>
where
    M: InMemoryCorpusMap<Testcase<I>>,
    I: Input,
{
    fn count(&self) -> usize {
        self.enabled_map.count()
    }

    fn count_disabled(&self) -> usize {
        self.disabled_map.count()
    }

    fn is_empty(&self) -> bool {
        self.enabled_map.is_empty()
    }

    fn add_shared<const ENABLED: bool>(&mut self, testcase: Testcase<I>) -> Result<StorageResult> {
        let testcase_id = *testcase.id();

        let already_stored = if ENABLED {
            self.enabled_map.add(testcase_id, testcase)
        } else {
            self.disabled_map.add(testcase_id, testcase)
        };

        let res = if already_stored {
            StorageResult::Duplicate(testcase_id)
        } else {
            StorageResult::Stored(testcase_id)
        };

        Ok(res)
    }

    fn get_from<const ENABLED: bool>(&self, id: &TestcaseId) -> Result<Testcase<I>> {
        if ENABLED {
            self.enabled_map
                .get(id)
                .cloned()
                .ok_or_else(|| Error::key_not_found(format!("Index {id} not found")))
        } else {
            let mut testcase = self.enabled_map.get(id);

            if testcase.is_none() {
                testcase = self.disabled_map.get(id);
            }

            testcase
                .cloned()
                .ok_or_else(|| Error::key_not_found(format!("Index {id} not found")))
        }
    }

    fn disable(&mut self, id: &TestcaseId) -> Result<()> {
        let tc = self
            .enabled_map
            .remove(id)
            .ok_or_else(|| Error::key_not_found(format!("Index {id} not found")))?;
        self.disabled_map.add(*id, tc);
        Ok(())
    }
}

impl<I, M> RemovableStore<I> for InMemoryStore<I, M>
where
    M: InMemoryCorpusMap<Testcase<I>>,
    I: Input,
{
    fn remove(&mut self, id: &TestcaseId) -> Result<Testcase<I>> {
        if let Some(tc) = self.enabled_map.remove(id) {
            Ok(tc)
        } else if let Some(tc) = self.disabled_map.remove(id) {
            Ok(tc)
        } else {
            Err(Error::key_not_found(format!(
                "Index {id} not found for remove"
            )))
        }
    }
}
