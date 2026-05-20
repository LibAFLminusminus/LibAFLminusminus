//! Corpuses contain the testcases, either in memory, on disk, or somewhere else.

use crate::DependencyResolver;
use core::fmt;
use libaflmm_core::Result;

pub mod testcase;
pub use testcase::{Testcase, TestcaseFilenameFormat, TestcaseId};

pub mod single;
pub use single::SingleCorpus;

// pub mod dynamic;
// pub use dynamic::DynamicCorpus;

pub mod nop;
pub use nop::NopCorpus;

pub mod store;
pub use store::{InMemoryStore, OnDiskStore, Store, maps};

pub mod schedulers;
pub use schedulers::Scheduler;

pub mod collection;
pub use collection::{
    CachedOnDiskCorpus, CachedOnDiskCorpusBuilder, InMemoryCorpus, InMemoryOnDiskCorpus,
    InMemoryOnDiskCorpusBuilder, OnDiskCorpus, OnDiskCorpusBuilder, StdInMemoryCorpusMap,
    StdInMemoryStore, StdOnDiskStore,
};

pub mod combined;
pub use combined::CombinedCorpus;

pub mod cache;
pub use cache::{Cache, FifoCache, IdentityCache};

pub type StdCorpus<I, SC> = InMemoryCorpus<I, SC>;
pub type StdObjectiveCorpus<I, SC> = OnDiskCorpus<I, SC>;

/// This module has a [`Scheduler`]
pub trait HasScheduler {
    /// [`Scheduler`] type
    type Scheduler: Scheduler;

    /// Ref to the [`Scheduler`]
    fn scheduler(&self) -> &Self::Scheduler;

    /// Mutable ref to the `Scheduler`
    fn scheduler_mut(&mut self) -> &mut Self::Scheduler;
}

/// Corpus with all current [`Testcase`]s, or solutions
pub trait Corpus: HasScheduler + Sized + DependencyResolver {
    type Input;

    /// Returns the number of all enabled entries
    fn count(&self) -> usize;

    /// Returns the number of all disabled entries
    fn count_disabled(&self) -> usize;

    /// Returns the number of elements including disabled entries
    fn count_all(&self) -> usize;

    /// Returns true, if no elements are in this corpus yet
    fn is_empty(&self) -> bool {
        self.count() == 0
    }

    /// Add an enabled testcase to the corpus and return its index
    /// It is allowed to add the same input multiple times.
    /// The corpus is responsible to handle that case without erroring out.
    ///
    /// The default [`TestcaseMetadata`] will be instantiated.
    fn add(&mut self, testcase: Testcase<Self::Input>) -> Result<TestcaseId> {
        self.add_shared::<true>(testcase)
    }

    /// Add a disabled testcase to the corpus and return its index
    /// It is allowed to add the same input multiple times.
    /// The corpus is responsible to handle that case without erroring out.
    ///
    /// The default [`TestcaseMetadata`] will be instantiated.
    fn add_disabled(&mut self, testcase: Testcase<Self::Input>) -> Result<TestcaseId> {
        self.add_shared::<false>(testcase)
    }

    /// Add a testcase to the corpus, and returns its index.
    /// The associated type tells whether the input should be added to the enabled or the disabled corpus.
    /// It is allowed to add the same input multiple times.
    /// The corpus is responsible to handle that case without erroring out.
    ///
    /// The input can be shared through [`Rc`].
    fn add_shared<const ENABLED: bool>(
        &mut self,
        testcase: Testcase<Self::Input>,
    ) -> Result<TestcaseId>;

    /// Get testcase by id; considers only enabled testcases
    fn get(&self, id: &TestcaseId) -> Result<Testcase<Self::Input>> {
        Self::get_from::<true>(self, id)
    }

    /// Get testcase by id, looking at the enabled and disabled stores.
    fn get_from_all(&self, id: &TestcaseId) -> Result<Testcase<Self::Input>> {
        Self::get_from::<false>(self, id)
    }

    /// Get testcase by id
    fn get_from<const ENABLED: bool>(&self, id: &TestcaseId) -> Result<Testcase<Self::Input>>;
}

/// Trait implemented by [`Corpus`]es able to disable an entry.
pub trait DisableEntry {
    /// Disable a corpus entry from its [`TestcaseId`].
    fn disable(&mut self, id: &TestcaseId) -> Result<()>;
}

/// Marker trait for corpus implementations that actually support enable/disable functionality
pub trait EnableDisableCorpus {
    /// Disables a testcase, moving it to the disabled map
    fn disable(&mut self, id: TestcaseId) -> Result<()>;

    /// Enables a testcase, moving it to the enabled map
    fn enable(&mut self, id: TestcaseId) -> Result<()>;
}

impl fmt::Display for TestcaseId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for TestcaseId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl From<TestcaseId> for usize {
    /// Not that the `TestcaseId` is not necessarily stable in the corpus (if we remove [`Testcase`]s, for example).
    fn from(id: TestcaseId) -> Self {
        id.0 as usize
    }
}
