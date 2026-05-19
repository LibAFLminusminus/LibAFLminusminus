//! A collection of various [`Corpus`].

use std::path::Path;

use libaflmm_core::Result;
use serde::{Deserialize, Serialize};

use crate::{
    DependencyResolver,
    corpus::{
        Corpus, FifoCache, HasScheduler, IdentityCache, InMemoryStore, OnDiskStore, Scheduler,
        SingleCorpus, Testcase, TestcaseFilenameFormat,
        combined::CombinedCorpus,
        maps::{self, InMemoryCorpusMap},
        store::{StorageResult, Store, ondisk::OnDiskStoreBuilder},
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
type InnerOnDiskCorpus<I, SC> = SingleCorpus<I, InnerStdOnDiskStore<I>, SC>;

type InnerInMemoryOnDiskCorpus<I, SC> =
    CombinedCorpus<IdentityCache, InnerStdInMemoryStore<I>, InnerStdOnDiskStore<I>, I, SC>;

type InnerCachedOnDiskCorpus<I, SC> = CombinedCorpus<
    FifoCache<InnerStdInMemoryStore<I>, InnerStdOnDiskStore<I>, I>,
    InnerStdInMemoryStore<I>,
    InnerStdOnDiskStore<I>,
    I,
    SC,
>;

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
#[repr(transparent)]
#[derive(Debug, Serialize, Deserialize)]
pub struct OnDiskCorpus<I, SC>(InnerOnDiskCorpus<I, SC>);

/// The on-disk corpus builder
#[derive(Debug, Clone, Default)]
pub struct OnDiskCorpusBuilder(OnDiskStoreBuilder);

/// The standard corpus for storing on disk and in-memory.
#[repr(transparent)]
#[derive(Debug, Serialize)]
pub struct InMemoryOnDiskCorpus<I, SC>(InnerInMemoryOnDiskCorpus<I, SC>);

/// The standard corpus for storing on disk and in-memory with a cache.
/// Useful for very large corpuses.
#[repr(transparent)]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CachedOnDiskCorpus<I, SC>(InnerCachedOnDiskCorpus<I, SC>);

/// The cached on-disk corpus builder
#[derive(Debug, Clone)]
pub struct CachedOnDiskCorpusBuilder<SC> {
    store_builder: OnDiskStoreBuilder,
    cache_max_len: usize,
    scheduler: Option<SC>,
}

impl<I> InMemoryCorpusMap<Testcase<I>> for StdInMemoryCorpusMap<I>
where
    I: Input,
{
    fn count(&self) -> usize {
        self.0.count()
    }

    fn add(&mut self, id: TestcaseId, testcase: Testcase<I>) -> bool {
        self.0.add(id, testcase)
    }

    fn get(&self, id: &TestcaseId) -> Option<&Testcase<I>> {
        self.0.get(id)
    }

    fn get_mut(&mut self, id: &TestcaseId) -> Option<&mut Testcase<I>> {
        self.0.get_mut(id)
    }

    fn remove(&mut self, id: &TestcaseId) -> Option<Testcase<I>> {
        self.0.remove(id)
    }

    fn prev(&self, id: &TestcaseId) -> Option<TestcaseId> {
        self.0.prev(id)
    }

    fn next(&self, id: &TestcaseId) -> Option<TestcaseId> {
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

impl<I, SC> HasScheduler for InMemoryCorpus<I, SC>
where
    SC: Scheduler,
{
    type Scheduler = SC;

    fn scheduler(&self) -> &Self::Scheduler {
        self.0.scheduler()
    }

    fn scheduler_mut(&mut self) -> &mut Self::Scheduler {
        self.0.scheduler_mut()
    }
}

impl<I, SC> Corpus for InMemoryCorpus<I, SC>
where
    I: Input,
    SC: Scheduler,
{
    type Input = I;

    fn count(&self) -> usize {
        self.0.count()
    }

    fn count_disabled(&self) -> usize {
        self.0.count_disabled()
    }

    fn count_all(&self) -> usize {
        self.0.count_all()
    }

    fn add_shared<const ENABLED: bool>(&mut self, testcase: Testcase<I>) -> Result<TestcaseId> {
        self.0.add_shared::<ENABLED>(testcase)
    }

    fn get_from<const ENABLED: bool>(&self, id: &TestcaseId) -> Result<Testcase<I>> {
        self.0.get_from::<ENABLED>(id)
    }
}

// impl<I, SC> DisableEntry for InMemoryCorpus<I, SC>
// where
//     SC: RemovableScheduler<I, S>,
// {
//     fn disable(&mut self, id: TestcaseId) -> Result<()> {
//         self.0.disable(id)
//     }
// }

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
    pub fn build<I, SC>(&self, scheduler: SC) -> Result<OnDiskCorpus<I, SC>> {
        Ok(OnDiskCorpus(SingleCorpus::new(self.0.build()?, scheduler)))
    }
}

impl<I, SC> OnDiskCorpus<I, SC>
where
    I: Input,
{
    /// Create a new [`OnDiskCorpus`]
    pub fn new<P: AsRef<Path>>(root: P, scheduler: SC) -> Result<Self> {
        Self::new_with_format(root, TestcaseFilenameFormat::Id, scheduler)
    }

    /// Create a new [`OnDiskCorpus`]
    pub fn new_with_format<P: AsRef<Path>>(
        root: P,
        filename_format: TestcaseFilenameFormat,
        scheduler: SC,
    ) -> Result<Self> {
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

impl<I, SC> HasScheduler for OnDiskCorpus<I, SC>
where
    SC: Scheduler,
{
    type Scheduler = SC;

    fn scheduler(&self) -> &Self::Scheduler {
        self.0.scheduler()
    }

    fn scheduler_mut(&mut self) -> &mut Self::Scheduler {
        self.0.scheduler_mut()
    }
}

impl<I, SC> Corpus for OnDiskCorpus<I, SC>
where
    I: Input,
    SC: Scheduler,
{
    type Input = I;

    fn count(&self) -> usize {
        self.0.count()
    }

    fn count_disabled(&self) -> usize {
        self.0.count_disabled()
    }

    fn count_all(&self) -> usize {
        self.0.count_all()
    }

    fn add_shared<const ENABLED: bool>(&mut self, testcase: Testcase<I>) -> Result<TestcaseId> {
        self.0.add_shared::<ENABLED>(testcase)
    }

    fn get_from<const ENABLED: bool>(&self, id: &TestcaseId) -> Result<Testcase<I>> {
        self.0.get_from::<ENABLED>(id)
    }
}

// impl<I, SC> DisableEntry for OnDiskCorpus<I, SC> {
//     fn disable(&mut self, id: TestcaseId) -> Result<()> {
//         self.0.disable(id)
//     }
// }

impl<I, SC> HasScheduler for InMemoryOnDiskCorpus<I, SC>
where
    SC: Scheduler,
{
    type Scheduler = SC;

    fn scheduler(&self) -> &Self::Scheduler {
        self.0.scheduler()
    }

    fn scheduler_mut(&mut self) -> &mut Self::Scheduler {
        self.0.scheduler_mut()
    }
}

impl<I, SC> DependencyResolver for InMemoryOnDiskCorpus<I, SC> {}

impl<I, SC> Corpus for InMemoryOnDiskCorpus<I, SC>
where
    I: Input,
    SC: Scheduler,
{
    type Input = I;

    fn count(&self) -> usize {
        self.0.count()
    }

    fn count_disabled(&self) -> usize {
        self.0.count_disabled()
    }

    fn count_all(&self) -> usize {
        self.0.count_all()
    }

    fn add_shared<const ENABLED: bool>(&mut self, testcase: Testcase<I>) -> Result<TestcaseId> {
        self.0.add_shared::<ENABLED>(testcase)
    }

    fn get_from<const ENABLED: bool>(&self, id: &TestcaseId) -> Result<Testcase<I>> {
        self.0.get_from::<ENABLED>(id)
    }
}

impl<I, SC> HasScheduler for CachedOnDiskCorpus<I, SC>
where
    SC: Scheduler,
{
    type Scheduler = SC;

    fn scheduler(&self) -> &Self::Scheduler {
        self.0.scheduler()
    }

    fn scheduler_mut(&mut self) -> &mut Self::Scheduler {
        self.0.scheduler_mut()
    }
}

impl<I, SC> DependencyResolver for CachedOnDiskCorpus<I, SC> {}

impl<I, SC> Corpus for CachedOnDiskCorpus<I, SC>
where
    I: Input,
    SC: Scheduler,
{
    type Input = I;

    fn count(&self) -> usize {
        self.0.count()
    }

    fn count_disabled(&self) -> usize {
        self.0.count_disabled()
    }

    fn count_all(&self) -> usize {
        self.0.count_all()
    }

    fn add_shared<const ENABLED: bool>(&mut self, testcase: Testcase<I>) -> Result<TestcaseId> {
        self.0.add_shared::<ENABLED>(testcase)
    }

    fn get_from<const ENABLED: bool>(&self, id: &TestcaseId) -> Result<Testcase<I>> {
        self.0.get_from::<ENABLED>(id)
    }
}

impl<I, SC> CachedOnDiskCorpus<I, SC> {
    /// Get a [`CachedOnDiskCorpus`] builder.
    #[must_use]
    pub fn builder() -> CachedOnDiskCorpusBuilder<SC> {
        CachedOnDiskCorpusBuilder::new()
    }

    /// Get the fallback store
    pub fn fallback_store(&self) -> &InnerStdOnDiskStore<I> {
        self.0.fallback_store()
    }
}

impl<SC> Default for CachedOnDiskCorpusBuilder<SC> {
    fn default() -> Self {
        Self {
            store_builder: OnDiskStoreBuilder::new(),
            cache_max_len: DEFAULT_CACHE_LEN,
            scheduler: None,
        }
    }
}

impl<SC> CachedOnDiskCorpusBuilder<SC> {
    /// Create a new [`CachedOnDiskCorpusBuilder`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the [`Scheduler`].
    #[must_use]
    pub fn scheduler(mut self, scheduler: SC) -> Self {
        self.scheduler = Some(scheduler);
        self
    }

    /// Set the cache max length.
    #[must_use]
    pub fn cache_max_len(mut self, cache_max_len: usize) -> Self {
        self.cache_max_len = cache_max_len;
        self
    }

    /// Set the root directory, where the testcases will be stored.
    #[must_use]
    pub fn root_dir(mut self, root: &Path) -> Self {
        self.store_builder.root_dir(root);
        self
    }

    /// Set the on-disk filename format
    #[must_use]
    pub fn filename_format(mut self, filename_format: TestcaseFilenameFormat) -> Self {
        self.store_builder.filename_format(filename_format);
        self
    }

    /// Build an [`OnDiskStore`].
    /// The root directory must be set.
    pub fn build<I: Input>(self) -> Result<CachedOnDiskCorpus<I, SC>> {
        Ok(CachedOnDiskCorpus(CombinedCorpus::new(
            self.scheduler.unwrap(),
            FifoCache::new(self.cache_max_len),
            InnerStdInMemoryStore::default(),
            self.store_builder.build()?,
        )))
    }
}
