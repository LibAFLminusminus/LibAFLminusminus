//! Corpuses contain the testcases, either in memory, on disk, or somewhere else.

use alloc::rc::Rc;
use core::{fmt, marker::PhantomData};

use serde::{Deserialize, Serialize};

use crate::Error;

pub mod testcase;
pub use testcase::{Testcase, TestcaseFilenameFormat};

pub mod single;
pub use single::SingleCorpus;

// pub mod dynamic;
// pub use dynamic::DynamicCorpus;

#[cfg(not(feature = "remove_me"))]
pub mod nop;
#[cfg(not(feature = "remove_me"))]
pub use nop::NopCorpus;

pub mod store;
pub use store::{InMemoryStore, OnDiskStore, Store, maps};

pub mod schedulers;
pub use schedulers::Scheduler;

pub mod collection;
pub use collection::{
    InMemoryCorpus, OnDiskCorpus, StdInMemoryCorpusMap, StdInMemoryStore, StdOnDiskStore,
};

/// An abstraction for the index that identify a testcase in the corpus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(transparent)]
pub struct CorpusId(pub usize);

/// A counter for [`Corpus`] implementors.
/// Useful to generate fresh [`CorpusId`]s.
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct CorpusCounter {
    /// A fresh, progressive ID
    /// It stores the next available ID.
    current_id: usize,
}

/// [`Iterator`] over the ids of a [`Corpus`]
#[derive(Debug)]
pub struct CorpusIdIterator<'a, C, I> {
    corpus: &'a C,
    cur: Option<CorpusId>,
    cur_back: Option<CorpusId>,
    phantom: PhantomData<I>,
}

/// Utility macro to call `Corpus::random_id`; fetches only enabled [`Testcase`]`s`
#[macro_export]
macro_rules! random_corpus_id {
    ($corpus:expr, $rand:expr) => {{
        let cnt = $corpus.count();
        #[cfg(debug_assertions)]
        let nth = $rand.below(core::num::NonZero::new(cnt).expect("Corpus may not be empty!"));
        // # Safety
        // This is a hot path. We try to be as fast as possible here.
        // In debug this is checked (see above.)
        // The worst that can happen is a wrong integer to get returned.
        // In this case, the call below will fail.
        #[cfg(not(debug_assertions))]
        let nth = $rand.below(unsafe { core::num::NonZero::new(cnt).unwrap_unchecked() });
        $corpus.nth(nth)
    }};
}

/// Utility macro to call `Corpus::random_id`; fetches both enabled and disabled [`Testcase`]`s`
/// Note: use `Corpus::get_from_all` as disabled entries are inaccessible from `Corpus::get`
#[macro_export]
macro_rules! random_corpus_id_with_disabled {
    ($corpus:expr, $rand:expr) => {{
        let cnt = $corpus.count_all();
        #[cfg(debug_assertions)]
        let nth = $rand.below(core::num::NonZero::new(cnt).expect("Corpus may not be empty!"));
        // # Safety
        // This is a hot path. We try to be as fast as possible here.
        // In debug this is checked (see above.)
        // The worst that can happen is a wrong integer to get returned.
        // In this case, the call below will fail.
        #[cfg(not(debug_assertions))]
        let nth = $rand.below(unsafe { core::num::NonZero::new(cnt).unwrap_unchecked() });
        $corpus.nth_from_all(nth)
    }};
}

/// Corpus with all current [`Testcase`]s, or solutions
pub trait Corpus<I, SC>: Sized {
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
    ///
    /// The default [`TestcaseMetadata`] will be instantiated.
    fn add(&mut self, input: I) -> Result<CorpusId, Error> {
        self.add_shared::<true>(Rc::new(input))
    }

    /// Add a disabled testcase to the corpus and return its index
    ///
    /// The default [`TestcaseMetadata`] will be instantiated.
    fn add_disabled(&mut self, input: I) -> Result<CorpusId, Error> {
        self.add_shared::<false>(Rc::new(input))
    }

    /// Add a testcase to the corpus, and returns its index.
    /// The associated type tells whether the input should be added to the enabled or the disabled corpus.
    ///
    /// The input can be shared through [`Rc`].
    fn add_shared<const ENABLED: bool>(&mut self, input: Rc<I>) -> Result<CorpusId, Error>;

    /// Get testcase by id; considers only enabled testcases
    fn get(&self, id: CorpusId) -> Result<Testcase<I>, Error> {
        Self::get_from::<true>(self, id)
    }

    /// Get testcase by id, looking at the enabled and disabled stores.
    fn get_from_all(&self, id: CorpusId) -> Result<Testcase<I>, Error> {
        Self::get_from::<false>(self, id)
    }

    /// Get testcase by id
    fn get_from<const ENABLED: bool>(&self, id: CorpusId) -> Result<Testcase<I>, Error>;

    /// An iterator over very active corpus id
    #[cfg(not(feature = "remove_me"))]
    fn ids(&self) -> CorpusIdIterator<'_, Self, I> {
        CorpusIdIterator {
            corpus: self,
            cur: self.first(),
            cur_back: self.last(),
            phantom: PhantomData,
        }
    }

    fn scheduler(&self) -> &SC;

    fn scheduler_mut(&mut self) -> &mut SC;
}

/// Marker trait for corpus implementations that actually support enable/disable functionality
pub trait EnableDisableCorpus {
    /// Disables a testcase, moving it to the disabled map
    fn disable(&mut self, id: CorpusId) -> Result<(), Error>;

    /// Enables a testcase, moving it to the enabled map
    fn enable(&mut self, id: CorpusId) -> Result<(), Error>;
}

impl fmt::Display for CorpusId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<usize> for CorpusId {
    fn from(id: usize) -> Self {
        Self(id)
    }
}

impl From<u64> for CorpusId {
    fn from(id: u64) -> Self {
        Self(id as usize)
    }
}

impl From<CorpusId> for usize {
    /// Not that the `CorpusId` is not necessarily stable in the corpus (if we remove [`Testcase`]s, for example).
    fn from(id: CorpusId) -> Self {
        id.0
    }
}

#[cfg(not(feature = "remove_me"))]
impl<C, I> Iterator for CorpusIdIterator<'_, C, I>
where
    C: Corpus<I>,
{
    type Item = CorpusId;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(cur) = self.cur {
            self.cur = self.corpus.next(cur);
            Some(cur)
        } else {
            None
        }
    }
}

#[cfg(not(feature = "remove_me"))]
impl<C, I> DoubleEndedIterator for CorpusIdIterator<'_, C, I>
where
    C: Corpus<I>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(cur_back) = self.cur_back {
            self.cur_back = self.corpus.prev(cur_back);
            Some(cur_back)
        } else {
            None
        }
    }
}

impl CorpusCounter {
    fn new_id(&mut self) -> CorpusId {
        let old = self.current_id;
        self.current_id += 1;
        CorpusId(old)
    }
}
