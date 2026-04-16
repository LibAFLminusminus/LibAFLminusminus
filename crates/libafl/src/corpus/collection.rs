//! A collection of various [`Corpus`].

use alloc::rc::Rc;
use std::path::{Path, PathBuf};

use libafl_bolts::Error;
use serde::{Deserialize, Serialize};

use crate::{
    DependencyResolver,
    corpus::{
        Corpus, InMemoryStore, OnDiskStore, SingleCorpus, Testcase,
        TestcaseFilenameFormat,
        maps::{self, InMemoryCorpusMap},
        store::{Store, ondisk::OnDiskStoreBuilder},
        testcase::TestcaseId,
    },
    inputs::Input,
};

const DEFAULT_CACHE_LEN: usize = 32;

#[cfg(not(feature = "corpus_btreemap"))]
type StdInMemoryMap<T> = maps::HashCorpusMap<T>;
#[cfg(feature = "corpus_btreemap")]
type StdInMemoryMap<T> = maps::BtreeCorpusMap<T>;

type InnerStdInMemoryCorpusMap<I> = StdInMemoryMap<Testcase<I>>;
type InnerStdInMemoryStore<I> = InMemoryStore<I, InnerStdInMemoryCorpusMap<I>>;
type InnerInMemoryCorpus<I, SC> = SingleCorpus<I, InnerStdInMemoryStore<I>, SC>;

type InnerStdOnDiskStore<I> = OnDiskStore<I, StdInMemoryMap<TestcaseId>>;
#[cfg(feature = "std")]
type InnerOnDiskCorpus<I, SC> = SingleCorpus<I, InnerStdOnDiskStore<I>, SC>;

/// The standard fully in-memory corpus map.
#[repr(transparent)]
#[derive(Debug, Serialize)]
pub struct StdInMemoryCorpusMap<I>(InnerStdInMemoryCorpusMap<I>);

/// The standard fully in-memory store.
#[repr(transparent)]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StdInMemoryStore<I>(InnerStdInMemoryStore<I>);

/// The standard fully on-disk store.
#[repr(transparent)]
#[derive(Debug, Serialize)]
pub struct StdOnDiskStore<I>(InnerStdOnDiskStore<I>);

/// The standard in-memory corpus.
#[repr(transparent)]
#[derive(Debug, Serialize, Deserialize)]
pub struct InMemoryCorpus<I, SC>(InnerInMemoryCorpus<I, SC>);

/// The standard fully on-disk corpus.
#[cfg(feature = "std")]
#[repr(transparent)]
#[derive(Debug, Serialize, Deserialize)]
pub struct OnDiskCorpus<I, SC>(InnerOnDiskCorpus<I, SC>);

/// The on-disk corpus builder
#[cfg(feature = "std")]
#[derive(Debug, Clone, Default)]
pub struct OnDiskCorpusBuilder(OnDiskStoreBuilder);

impl<I> InMemoryCorpusMap<Testcase<I>> for StdInMemoryCorpusMap<I>
where
    I: Input,
{
    fn count(&self) -> usize {
        self.0.count()
    }

    fn add(&mut self, id: TestcaseId, testcase: Testcase<I>) {
        self.0.add(id, testcase);
    }

    fn get(&self, id: TestcaseId) -> Option<&Testcase<I>> {
        self.0.get(id)
    }

    fn get_mut(&mut self, id: TestcaseId) -> Option<&mut Testcase<I>> {
        self.0.get_mut(id)
    }

    fn remove(&mut self, id: TestcaseId) -> Option<Testcase<I>> {
        self.0.remove(id)
    }

    fn prev(&self, id: TestcaseId) -> Option<TestcaseId> {
        self.0.prev(id)
    }

    fn next(&self, id: TestcaseId) -> Option<TestcaseId> {
        self.0.next(id)
    }

    fn first(&self) -> Option<TestcaseId> {
        self.0.first()
    }

    fn last(&self) -> Option<TestcaseId> {
        self.0.last()
    }

    fn nth(&self, nth: usize) -> TestcaseId {
        self.0.nth(nth)
    }
}

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

    fn add_shared<const ENABLED: bool>(&mut self, input: Rc<I>) -> Result<TestcaseId, Error> {
        self.0.add_shared::<ENABLED>(input)
    }

    fn get_from<const ENABLED: bool>(&self, id: TestcaseId) -> Result<Testcase<I>, Error> {
        self.0.get_from::<ENABLED>(id)
    }

    fn disable(&mut self, id: TestcaseId) -> Result<(), Error> {
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

    fn add_shared<const ENABLED: bool>(&mut self, input: Rc<I>) -> Result<TestcaseId, Error> {
        self.0.add_shared::<ENABLED>(input)
    }

    fn get_from<const ENABLED: bool>(&self, id: TestcaseId) -> Result<Testcase<I>, Error> {
        self.0.get_from::<ENABLED>(id)
    }

    fn disable(&mut self, id: TestcaseId) -> Result<(), Error> {
        self.0.disable(id)
    }
}

impl<I, SC> InMemoryCorpus<I, SC> {
    /// Create a new [`InMemoryCorpus`].
    #[must_use]
    pub fn new(scheduler: SC) -> Self {
        InMemoryCorpus(InnerInMemoryCorpus::new(
            InnerStdInMemoryStore::default(),
            scheduler,
        ))
    }
}

impl<I, SC> DependencyResolver for InMemoryCorpus<I, SC> {}

impl<I, SC> Corpus<I, SC> for InMemoryCorpus<I, SC>
where
    I: Input,
{
    fn count(&self) -> usize {
        self.0.count()
    }

    fn count_disabled(&self) -> usize {
        self.0.count_disabled()
    }

    fn count_all(&self) -> usize {
        self.0.count_all()
    }

    fn add_shared<const ENABLED: bool>(&mut self, input: Rc<I>) -> Result<TestcaseId, Error> {
        self.0.add_shared::<ENABLED>(input)
    }

    fn get_from<const ENABLED: bool>(&self, id: TestcaseId) -> Result<Testcase<I>, Error> {
        self.0.get_from::<ENABLED>(id)
    }

    fn scheduler(&self) -> &SC {
        self.0.scheduler()
    }

    fn scheduler_mut(&mut self) -> &mut SC {
        self.0.scheduler_mut()
    }
}

// impl<I, SC> DisableEntry for InMemoryCorpus<I, SC>
// where
//     SC: RemovableScheduler<I, S>,
// {
//     fn disable(&mut self, id: TestcaseId) -> Result<(), Error> {
//         self.0.disable(id)
//     }
// }

#[cfg(feature = "std")]
impl OnDiskCorpusBuilder {
    /// Create a new [`OnDiskCorpusBuilder`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the root directory, where the testcases will be stored.
    pub fn root_dir(&mut self, root: &Path) -> &mut Self {
        self.0.root_dir(root);
        self
    }

    /// Set the on-disk filename format
    pub fn filename_format(&mut self, filename_format: TestcaseFilenameFormat) -> &mut Self {
        self.0.filename_format(filename_format);
        self
    }

    /// Build an [`OnDiskStore`].
    /// The root directory must be set.
    pub fn build<I, SC>(&self, scheduler: SC) -> Result<OnDiskCorpus<I, SC>, Error> {
        Ok(OnDiskCorpus(SingleCorpus::new(self.0.build()?, scheduler)))
    }
}

#[cfg(feature = "std")]
impl<I, SC> OnDiskCorpus<I, SC>
where
    I: Input,
{
    /// Create a new [`OnDiskCorpus`]
    pub fn new(root: PathBuf, scheduler: SC) -> Result<Self, Error> {
        Self::new_with_format(root, TestcaseFilenameFormat::Id, scheduler)
    }

    /// Create a new [`OnDiskCorpus`]
    pub fn new_with_format(
        root: PathBuf,
        filename_format: TestcaseFilenameFormat,
        scheduler: SC,
    ) -> Result<Self, Error> {
        Ok(OnDiskCorpus(InnerOnDiskCorpus::new(
            InnerStdOnDiskStore::new(root, filename_format)?,
            scheduler,
        )))
    }

    /// Get a [`OnDiskCorpus`] builder.
    #[must_use]
    pub fn builder() -> OnDiskCorpusBuilder {
        OnDiskCorpusBuilder::default()
    }
}

impl<I, SC> DependencyResolver for OnDiskCorpus<I, SC> {}

#[cfg(feature = "std")]
impl<I, SC> Corpus<I, SC> for OnDiskCorpus<I, SC>
where
    I: Input,
{
    fn count(&self) -> usize {
        self.0.count()
    }

    fn count_disabled(&self) -> usize {
        self.0.count_disabled()
    }

    fn count_all(&self) -> usize {
        self.0.count_all()
    }

    fn add_shared<const ENABLED: bool>(&mut self, input: Rc<I>) -> Result<TestcaseId, Error> {
        self.0.add_shared::<ENABLED>(input)
    }

    fn get_from<const ENABLED: bool>(&self, id: TestcaseId) -> Result<Testcase<I>, Error> {
        self.0.get_from::<ENABLED>(id)
    }

    fn scheduler(&self) -> &SC {
        self.0.scheduler()
    }

    fn scheduler_mut(&mut self) -> &mut SC {
        self.0.scheduler_mut()
    }
}

// impl<I, SC> DisableEntry for OnDiskCorpus<I, SC> {
//     fn disable(&mut self, id: TestcaseId) -> Result<(), Error> {
//         self.0.disable(id)
//     }
// }
