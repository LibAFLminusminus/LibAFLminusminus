//! A collection of various [`Corpus`].

pub mod maps;
pub use maps::StdInMemoryCorpusMap;

pub mod stores;
pub use stores::{StdInMemoryStore, StdOnDiskStore};

pub mod corpus;
pub use corpus::{
    CachedOnDiskConfig, CachedOnDiskCorpus, CachedOnDiskCorpusBuilder, InMemoryCorpus,
    InMemoryCorpusBuilder, InMemoryOnDiskCorpus, InMemoryOnDiskCorpusBuilder,
    ObjectiveCachedOnDiskCorpus, ObjectiveCachedOnDiskCorpusBuilder, ObjectiveInMemoryCorpus,
    ObjectiveInMemoryCorpusBuilder, ObjectiveInMemoryOnDiskCorpus,
    ObjectiveInMemoryOnDiskCorpusBuilder, ObjectiveOnDiskCorpus, ObjectiveOnDiskCorpusBuilder,
    OnDiskCorpus, OnDiskCorpusBuilder,
};
