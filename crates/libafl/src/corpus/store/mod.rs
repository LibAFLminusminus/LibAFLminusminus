//! Stores are collections managing testcases

use alloc::rc::Rc;

use libafl_bolts::Error;

use super::{testcase::TestcaseId, Testcase};

pub mod maps;
pub use maps::{BtreeCorpusMap, HashCorpusMap, InMemoryCorpusMap};

pub mod inmemory;
pub use inmemory::InMemoryStore;

pub mod ondisk;
pub use ondisk::{DiskMgr, OnDiskMetadataFormat, OnDiskStore};

/// A store is responsible for storing and retrieving [`Testcase`]s, ordered by add time.
pub trait Store<I> {
    /// Returns the number of all enabled entries
    fn count(&self) -> usize;

    /// Returns the number of all disabled entries
    fn count_disabled(&self) -> usize;

    /// Returns the number of elements including disabled entries
    fn count_all(&self) -> usize {
        self.count().saturating_add(self.count_disabled())
    }

    /// Returns true, if no elements are in this corpus yet
    fn is_empty(&self) -> bool {
        self.count() == 0
    }

    /// Store the testcase associated to `corpus_id` to the set.
    fn add_shared<const ENABLED: bool>(&mut self, input: Rc<I>) -> Result<TestcaseId, Error>;

    /// Get testcase by id; considers only enabled testcases
    fn get(&self, id: TestcaseId) -> Result<Testcase<I>, Error> {
        Self::get_from::<true>(self, id)
    }

    /// Get testcase by id; considers both enabled and disabled testcases
    fn get_from_all(&self, id: TestcaseId) -> Result<Testcase<I>, Error> {
        Self::get_from::<false>(self, id)
    }

    /// Get testcase by id
    fn get_from<const ENABLED: bool>(&self, id: TestcaseId) -> Result<Testcase<I>, Error>;

    /// Disable a testcase by id
    fn disable(&mut self, id: TestcaseId) -> Result<(), Error>;
}

/// A Store with removable entries
pub trait RemovableStore<I>: Store<I> {
    /// Removes an entry from the corpus, returning it; considers both enabled and disabled testcases
    fn remove(&mut self, id: TestcaseId) -> Result<Testcase<I>, Error>;
}
