use crate::{
    corpus::{
        Testcase, TestcaseId,
        maps::{self, InMemoryCorpusMap},
    },
    inputs::Input,
};
use serde::Serialize;

#[cfg(not(feature = "corpus_btreemap"))]
pub type StdInMemoryMap<T> = maps::HashCorpusMap<T>;
#[cfg(feature = "corpus_btreemap")]
pub type StdInMemoryMap<T> = maps::BtreeCorpusMap<T>;

pub(crate) type InnerStdInMemoryCorpusMap<I> = StdInMemoryMap<Testcase<I>>;

/// The standard fully in-memory corpus map.
#[repr(transparent)]
#[derive(Debug, Serialize)]
pub struct StdInMemoryCorpusMap<I>(InnerStdInMemoryCorpusMap<I>);

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
