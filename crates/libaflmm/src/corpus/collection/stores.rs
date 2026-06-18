use crate::{
    Result,
    corpus::{
        InMemoryStore, OnDiskStore, Store, Testcase, TestcaseId,
        collection::maps::{InnerStdInMemoryCorpusMap, StdInMemoryMap},
        store::StorageResult,
    },
    inputs::Input,
};
use serde::{Deserialize, Serialize};

pub(crate) type InnerStdInMemoryStore<I> = InMemoryStore<I, InnerStdInMemoryCorpusMap<I>>;
pub(crate) type InnerStdOnDiskStore<I> = OnDiskStore<I, StdInMemoryMap<TestcaseId>>;

/// The standard fully in-memory store.
#[repr(transparent)]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StdInMemoryStore<I>(InnerStdInMemoryStore<I>);

/// The standard fully on-disk store.
#[repr(transparent)]
#[derive(Debug, Serialize)]
pub struct StdOnDiskStore<I>(InnerStdOnDiskStore<I>);

impl<I> Store<I> for StdInMemoryStore<I>
where
    I: Input,
{
    fn count(&self) -> usize {
        self.0.count()
    }

    fn count_disabled(&self) -> usize {
        self.0.count_disabled()
    }

    fn add_shared<const ENABLED: bool>(&mut self, testcase: Testcase<I>) -> Result<StorageResult> {
        self.0.add_shared::<ENABLED>(testcase)
    }

    fn get_from<const ENABLED: bool>(&self, id: &TestcaseId) -> Result<Testcase<I>> {
        self.0.get_from::<ENABLED>(id)
    }

    fn disable(&mut self, id: &TestcaseId) -> Result<()> {
        self.0.disable(id)
    }
}

impl<I> Store<I> for StdOnDiskStore<I>
where
    I: Input,
{
    fn count(&self) -> usize {
        self.0.count()
    }

    fn count_disabled(&self) -> usize {
        self.0.count_disabled()
    }

    fn add_shared<const ENABLED: bool>(&mut self, testcase: Testcase<I>) -> Result<StorageResult> {
        self.0.add_shared::<ENABLED>(testcase)
    }

    fn get_from<const ENABLED: bool>(&self, id: &TestcaseId) -> Result<Testcase<I>> {
        self.0.get_from::<ENABLED>(id)
    }

    fn disable(&mut self, id: &TestcaseId) -> Result<()> {
        self.0.disable(id)
    }
}
