//! A collection of various [`Corpus`].

use alloc::rc::Rc;
use std::path::{Path, PathBuf};

use libafl_bolts::Error;
use serde::{Deserialize, Serialize};

use crate::{
    DependencyResolver,
    corpus::{
        Corpus, InMemoryStore, OnDiskStore, Scheduler, SingleCorpus, Testcase,
        TestcaseFilenameFormat,
        maps::{self, InMemoryCorpusMap},
        store::{Store, ondisk::OnDiskStoreBuilder},
        testcase::TestcaseId,
    },
    inputs::{Input, InputContext},
};

const DEFAULT_CACHE_LEN: usize = 32;

#[cfg(not(feature = "corpus_btreemap"))]
type StdInMemoryMap<T> = maps::HashCorpusMap<T>;
#[cfg(feature = "corpus_btreemap")]
type StdInMemoryMap<T> = maps::BtreeCorpusMap<T>;

type InnerStdInMemoryCorpusMap<I> = StdInMemoryMap<Testcase<I>>;
type InnerStdInMemoryStore<I> = InMemoryStore<I, InnerStdInMemoryCorpusMap<I>>;
type InnerInMemoryCorpus<CT, I, SC> = SingleCorpus<CT, I, InnerStdInMemoryStore<I>, SC>;

type InnerStdOnDiskStore<I> = OnDiskStore<I, StdInMemoryMap<TestcaseId>>;
#[cfg(feature = "std")]
type InnerOnDiskCorpus<CT, I, SC> = SingleCorpus<CT, I, InnerStdOnDiskStore<I>, SC>;

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
pub struct InMemoryCorpus<CT, I, SC>(InnerInMemoryCorpus<CT, I, SC>);

/// The standard fully on-disk corpus.
#[cfg(feature = "std")]
#[repr(transparent)]
#[derive(Debug, Serialize, Deserialize)]
pub struct OnDiskCorpus<CT, I, SC>(InnerOnDiskCorpus<CT, I, SC>);

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

    fn add(&mut self, id: TestcaseId, testcase: Testcase<I>) -> bool {
        self.0.add(id, testcase)
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

impl<CT, I, SC> InMemoryCorpus<CT, I, SC> {
    /// Create a new [`InMemoryCorpus`].
    #[must_use]
    pub fn new(context: CT, scheduler: SC) -> Self {
        InMemoryCorpus(InnerInMemoryCorpus::new(
            context,
            InnerStdInMemoryStore::default(),
            scheduler,
        ))
    }
}

impl<CT, I, SC> DependencyResolver for InMemoryCorpus<CT, I, SC> {}

impl<CT, I, SC> Corpus<I, SC> for InMemoryCorpus<CT, I, SC>
where
    CT: InputContext<I>,
    I: Input,
    SC: Scheduler,
{
    type Context = CT;
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

    fn context(&self) -> &Self::Context {
        self.0.context()
    }

    fn context_mut(&mut self) -> &mut Self::Context {
        self.0.context_mut()
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
    pub fn build<CT, I, SC>(
        &self,
        context: CT,
        scheduler: SC,
    ) -> Result<OnDiskCorpus<CT, I, SC>, Error> {
        Ok(OnDiskCorpus(SingleCorpus::new(
            context,
            self.0.build()?,
            scheduler,
        )))
    }
}

#[cfg(feature = "std")]
impl<CT, I, SC> OnDiskCorpus<CT, I, SC>
where
    I: Input,
{
    /// Create a new [`OnDiskCorpus`]
    pub fn new(root: PathBuf, context: CT, scheduler: SC) -> Result<Self, Error> {
        Self::new_with_format(root, TestcaseFilenameFormat::Id, context, scheduler)
    }

    /// Create a new [`OnDiskCorpus`]
    pub fn new_with_format(
        root: PathBuf,
        filename_format: TestcaseFilenameFormat,
        context: CT,
        scheduler: SC,
    ) -> Result<Self, Error> {
        Ok(OnDiskCorpus(InnerOnDiskCorpus::new(
            context,
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

impl<CT, I, SC> DependencyResolver for OnDiskCorpus<CT, I, SC> {}

#[cfg(feature = "std")]
impl<CT, I, SC> Corpus<I, SC> for OnDiskCorpus<CT, I, SC>
where
    CT: InputContext<I>,
    I: Input,
    SC: Scheduler,
{
    type Context = CT;

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

    fn context(&self) -> &Self::Context {
        self.0.context()
    }

    fn context_mut(&mut self) -> &mut Self::Context {
        self.0.context_mut()
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
