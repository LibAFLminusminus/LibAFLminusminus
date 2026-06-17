//! A collection of various [`Corpus`].

use crate::{
    common::DependencyResolver,
    controllers::Worker,
    corpus::{
        Corpus, FifoCache, IdentityCache, InMemoryStore, ObjectiveCorpus, OnDiskStore, Scheduler,
        SingleCorpus, Testcase, TestcaseFilenameFormat,
        combined::CombinedCorpus,
        maps::{self, InMemoryCorpusMap},
        schedulers::NopScheduler,
        store::{StorageResult, Store, ondisk::OnDiskStoreBuilder},
        testcase::TestcaseId,
    },
    inputs::Input,
};
use core::marker::PhantomData;
use libaflmm_core::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

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
#[derive(Debug, Clone)]
pub struct OnDiskCorpusBuilder<I, SC> {
    store_builder: OnDiskStoreBuilder,
    scheduler: SC,
    _phantom: PhantomData<I>,
}

/// The standard corpus for storing on disk and in-memory.
#[repr(transparent)]
#[derive(Debug, Serialize, Deserialize)]
pub struct InMemoryOnDiskCorpus<I, SC>(InnerInMemoryOnDiskCorpus<I, SC>);

/// The in-memory + on-disk corpus builder
#[derive(Debug, Clone)]
pub struct InMemoryOnDiskCorpusBuilder<I, SC> {
    store_builder: OnDiskStoreBuilder,
    scheduler: SC,
    _phantom: PhantomData<I>,
}

/// The standard corpus for storing on disk and in-memory with a cache.
/// Useful for very large corpuses.
#[repr(transparent)]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CachedOnDiskCorpus<I, SC>(InnerCachedOnDiskCorpus<I, SC>);

/// The cached on-disk corpus builder
#[derive(Debug, Clone)]
pub struct CachedOnDiskCorpusBuilder<I, SC> {
    store_builder: OnDiskStoreBuilder,
    cache_max_len: usize,
    scheduler: SC,
    _phantom: PhantomData<I>,
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
    /// Create a new [`InMemoryCorpus`] with a given scheduler.
    #[must_use]
    pub fn with_scheduler(scheduler: SC) -> Self {
        InMemoryCorpus(InnerInMemoryCorpus::new(
            InnerStdInMemoryStore::default(),
            scheduler,
        ))
    }
}

impl<I> Default for InMemoryCorpus<I, NopScheduler> {
    fn default() -> Self {
        Self::with_scheduler(NopScheduler)
    }
}

impl<I> InMemoryCorpus<I, NopScheduler> {
    /// Create a new [`InMemoryCorpus`] without a scheduler.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl<I, SC> DependencyResolver for InMemoryCorpus<I, SC> {}

impl<I, SC> Corpus<I, SC> for InMemoryCorpus<I, SC>
where
    I: Input,
    SC: Scheduler,
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

    fn add_shared<const ENABLED: bool>(&mut self, testcase: Testcase<I>) -> Result<TestcaseId> {
        self.0.add_shared::<ENABLED>(testcase)
    }

    fn get_from<const ENABLED: bool>(&self, id: &TestcaseId) -> Result<Testcase<I>> {
        self.0.get_from::<ENABLED>(id)
    }

    fn scheduler(&self) -> &SC {
        self.0.scheduler()
    }

    fn scheduler_mut(&mut self) -> &mut SC {
        self.0.scheduler_mut()
    }
}

impl<I> ObjectiveCorpus<I> for InMemoryCorpus<I, NopScheduler> where I: Input {}

impl<I> Default for OnDiskCorpusBuilder<I, NopScheduler> {
    fn default() -> Self {
        Self {
            store_builder: OnDiskStoreBuilder::default(),
            scheduler: NopScheduler,
            _phantom: PhantomData,
        }
    }
}

impl<I, SC> OnDiskCorpusBuilder<I, SC> {
    /// Set the [`Scheduler`].
    #[must_use]
    pub fn scheduler<SC2>(self, scheduler: SC2) -> OnDiskCorpusBuilder<I, SC2> {
        OnDiskCorpusBuilder {
            scheduler,
            store_builder: self.store_builder,
            _phantom: PhantomData,
        }
    }

    /// Set the root directory, where the testcases will be stored.
    #[must_use]
    pub fn root_dir(mut self, root_dir: impl AsRef<Path>) -> Self {
        self.store_builder.root_dir(root_dir);
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
    pub fn build(self) -> Result<OnDiskCorpus<I, SC>> {
        Ok(OnDiskCorpus(SingleCorpus::new(
            self.store_builder.build()?,
            self.scheduler,
        )))
    }
}

impl<I, SC> OnDiskCorpus<I, SC> {
    /// Create a new [`OnDiskCorpus`]
    pub fn new(
        root: impl AsRef<Path>,
        filename_format: TestcaseFilenameFormat,
        scheduler: SC,
    ) -> Result<Self> {
        Ok(OnDiskCorpus(InnerOnDiskCorpus::new(
            InnerStdOnDiskStore::new(root, filename_format)?,
            scheduler,
        )))
    }
}

impl<I> OnDiskCorpus<I, NopScheduler> {
    /// Get a [`OnDiskCorpus`] builder for a corpus.
    #[must_use]
    pub fn corpus_builder<W: Worker>(worker: &W) -> Result<OnDiskCorpusBuilder<I, NopScheduler>> {
        Ok(OnDiskCorpusBuilder::default().root_dir(worker.workdir().corpus_dir()?))
    }

    /// Get a [`OnDiskCorpus`] builder for an objective corpus.
    #[must_use]
    pub fn objective_builder<W: Worker>(
        worker: &W,
    ) -> Result<OnDiskCorpusBuilder<I, NopScheduler>> {
        Ok(OnDiskCorpusBuilder::default().root_dir(worker.workdir().objective_dir()?))
    }
}

impl<I, SC> DependencyResolver for OnDiskCorpus<I, SC> {}

impl<I, SC> Corpus<I, SC> for OnDiskCorpus<I, SC>
where
    I: Input,
    SC: Scheduler,
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

    fn add_shared<const ENABLED: bool>(&mut self, testcase: Testcase<I>) -> Result<TestcaseId> {
        self.0.add_shared::<ENABLED>(testcase)
    }

    fn get_from<const ENABLED: bool>(&self, id: &TestcaseId) -> Result<Testcase<I>> {
        self.0.get_from::<ENABLED>(id)
    }

    fn scheduler(&self) -> &SC {
        self.0.scheduler()
    }

    fn scheduler_mut(&mut self) -> &mut SC {
        self.0.scheduler_mut()
    }
}

impl<I> ObjectiveCorpus<I> for OnDiskCorpus<I, NopScheduler> where I: Input {}

impl<I, SC> DependencyResolver for InMemoryOnDiskCorpus<I, SC> {}

impl<I, SC> Corpus<I, SC> for InMemoryOnDiskCorpus<I, SC>
where
    I: Input,
    SC: Scheduler,
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

    fn add_shared<const ENABLED: bool>(&mut self, testcase: Testcase<I>) -> Result<TestcaseId> {
        self.0.add_shared::<ENABLED>(testcase)
    }

    fn get_from<const ENABLED: bool>(&self, id: &TestcaseId) -> Result<Testcase<I>> {
        self.0.get_from::<ENABLED>(id)
    }

    fn scheduler(&self) -> &SC {
        self.0.scheduler()
    }

    fn scheduler_mut(&mut self) -> &mut SC {
        self.0.scheduler_mut()
    }
}

impl<I> ObjectiveCorpus<I> for InMemoryOnDiskCorpus<I, NopScheduler> where I: Input {}

impl<I> InMemoryOnDiskCorpus<I, NopScheduler> {
    /// Get a [`InMemoryOnDiskCorpus`] builder.
    #[must_use]
    pub fn builder() -> InMemoryOnDiskCorpusBuilder<I, NopScheduler> {
        InMemoryOnDiskCorpusBuilder::default()
    }
}

impl<I> Default for InMemoryOnDiskCorpusBuilder<I, NopScheduler> {
    fn default() -> Self {
        Self {
            store_builder: OnDiskStoreBuilder::default(),
            scheduler: NopScheduler,
            _phantom: PhantomData,
        }
    }
}

impl<I, SC> InMemoryOnDiskCorpusBuilder<I, SC> {
    /// Set the [`Scheduler`].
    #[must_use]
    pub fn scheduler<SC2>(self, scheduler: SC2) -> InMemoryOnDiskCorpusBuilder<I, SC2> {
        InMemoryOnDiskCorpusBuilder {
            scheduler,
            store_builder: self.store_builder,
            _phantom: PhantomData,
        }
    }

    /// Set the root directory, where the testcases will be stored.
    #[must_use]
    pub fn root_dir(mut self, root_dir: impl AsRef<Path>) -> Self {
        self.store_builder.root_dir(root_dir);
        self
    }

    /// Set the on-disk filename format
    #[must_use]
    pub fn filename_format(mut self, filename_format: TestcaseFilenameFormat) -> Self {
        self.store_builder.filename_format(filename_format);
        self
    }

    /// Build an [`InMemoryOnDiskCorpus`].
    /// The root directory must be set.
    pub fn build(self) -> Result<InMemoryOnDiskCorpus<I, SC>> {
        Ok(InMemoryOnDiskCorpus(CombinedCorpus::new(
            self.scheduler,
            IdentityCache,
            InnerStdInMemoryStore::default(),
            self.store_builder.build()?,
        )))
    }
}

impl<I, SC> DependencyResolver for CachedOnDiskCorpus<I, SC> {}

impl<I, SC> Corpus<I, SC> for CachedOnDiskCorpus<I, SC>
where
    I: Input,
    SC: Scheduler,
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

    fn add_shared<const ENABLED: bool>(&mut self, testcase: Testcase<I>) -> Result<TestcaseId> {
        self.0.add_shared::<ENABLED>(testcase)
    }

    fn get_from<const ENABLED: bool>(&self, id: &TestcaseId) -> Result<Testcase<I>> {
        self.0.get_from::<ENABLED>(id)
    }

    fn scheduler(&self) -> &SC {
        self.0.scheduler()
    }

    fn scheduler_mut(&mut self) -> &mut SC {
        self.0.scheduler_mut()
    }
}

impl<I> ObjectiveCorpus<I> for CachedOnDiskCorpus<I, NopScheduler> where I: Input {}

impl<I, SC> CachedOnDiskCorpus<I, SC> {
    /// Get the fallback store
    pub fn fallback_store(&self) -> &InnerStdOnDiskStore<I> {
        self.0.fallback_store()
    }
}

impl<I> CachedOnDiskCorpus<I, NopScheduler> {
    /// Get a [`CachedOnDiskCorpus`] builder.
    #[must_use]
    pub fn builder() -> CachedOnDiskCorpusBuilder<I, NopScheduler> {
        CachedOnDiskCorpusBuilder::default()
    }
}

impl<I> Default for CachedOnDiskCorpusBuilder<I, NopScheduler> {
    fn default() -> Self {
        Self {
            store_builder: OnDiskStoreBuilder::new(),
            cache_max_len: DEFAULT_CACHE_LEN,
            scheduler: NopScheduler,
            _phantom: PhantomData,
        }
    }
}

impl<I, SC> CachedOnDiskCorpusBuilder<I, SC> {
    /// Set the [`Scheduler`].
    #[must_use]
    pub fn scheduler(mut self, scheduler: SC) -> Self {
        self.scheduler = scheduler;
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
    pub fn build(self) -> Result<CachedOnDiskCorpus<I, SC>> {
        Ok(CachedOnDiskCorpus(CombinedCorpus::new(
            self.scheduler,
            FifoCache::new(self.cache_max_len),
            InnerStdInMemoryStore::default(),
            self.store_builder.build()?,
        )))
    }
}
