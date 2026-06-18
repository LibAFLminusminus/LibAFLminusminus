use crate::{
    common::DependencyResolver,
    controllers::Worker,
    corpus::{
        Corpus, FifoCache, IdentityCache, ObjectiveCorpus, ScheduledCorpus, Scheduler,
        SingleCorpus, Testcase, TestcaseFilenameFormat,
        collection::stores::{InnerStdInMemoryStore, InnerStdOnDiskStore},
        combined::CombinedCorpus,
        schedulers::NopScheduler,
        store::{inmemory::InMemoryStoreBuilder, ondisk::OnDiskStoreBuilder},
        testcase::TestcaseId,
    },
    inputs::{Input, NopInput},
};
use core::marker::PhantomData;
use libaflmm_core::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

const DEFAULT_CACHE_LEN: usize = 32;

macro_rules! define_corpus {
    (@roles
        $(#[$smeta:meta])* $sched:ident,
        $(#[$ometa:meta])* $obj:ident,
        $inner:ident
    ) => {
        $(#[$smeta])*
        #[repr(transparent)]
        #[derive(Debug, Serialize, Deserialize)]
        pub struct $sched<I, SC>($inner<I, SC>);

        impl<I, SC> DependencyResolver for $sched<I, SC> {}

        impl<I, SC> Corpus<I> for $sched<I, SC> where I: Input {
            fn count(&self) -> usize { self.0.count() }
            fn count_disabled(&self) -> usize { self.0.count_disabled() }
            fn count_all(&self) -> usize { self.0.count_all() }
            fn add_inner<const ENABLED: bool>(&mut self, tc: Testcase<I>) -> Result<TestcaseId> {
                self.0.add_inner::<ENABLED>(tc)
            }
            fn get_from<const ENABLED: bool>(&self, id: &TestcaseId) -> Result<Testcase<I>> {
                self.0.get_from::<ENABLED>(id)
            }
        }

        impl<I, SC> ScheduledCorpus<I, SC> for $sched<I, SC> where I: Input, SC: Scheduler {
            fn add_shared<const ENABLED: bool>(&mut self, tc: Testcase<I>) -> Result<TestcaseId> {
                self.0.add_shared::<ENABLED>(tc)
            }
            fn scheduler(&self) -> &SC { self.0.scheduler() }
            fn scheduler_mut(&mut self) -> &mut SC { self.0.scheduler_mut() }
        }

        $(#[$ometa])*
        #[repr(transparent)]
        #[derive(Debug, Serialize, Deserialize)]
        pub struct $obj<I>($inner<I, NopScheduler>);

        impl<I> DependencyResolver for $obj<I> {}

        impl<I> Corpus<I> for $obj<I> where I: Input {
            fn count(&self) -> usize { self.0.count() }
            fn count_disabled(&self) -> usize { self.0.count_disabled() }
            fn count_all(&self) -> usize { self.0.count_all() }
            fn add_inner<const ENABLED: bool>(&mut self, tc: Testcase<I>) -> Result<TestcaseId> {
                self.0.add_inner::<ENABLED>(tc)
            }
            fn get_from<const ENABLED: bool>(&self, id: &TestcaseId) -> Result<Testcase<I>> {
                self.0.get_from::<ENABLED>(id)
            }
        }

        impl<I> ObjectiveCorpus<I> for $obj<I> where I: Input {
            fn add_objective(&mut self, tc: Testcase<I>) -> Result<TestcaseId> {
                self.0.add_objective(tc)
            }
        }
    };

    (
        inner = $inner:ident,
        $(#[$smeta:meta])* scheduled = $sched:ident,
        $(#[$ometa:meta])* objective = $obj:ident,
        $(#[$sbmeta:meta])* scheduled_builder = $sbuilder:ident,
        $(#[$obmeta:meta])* objective_builder = $obuilder:ident,
        config = $cfg:ty,
        $(workdir { scheduled = $sdir:ident, objective = $odir:ident },)?
        build  = $build:expr $(,)?
    ) => {
        define_corpus!(@roles $(#[$smeta])* $sched, $(#[$ometa])* $obj, $inner);

        $(#[$sbmeta])*
        #[derive(Debug, Clone)]
        pub struct $sbuilder<I, SC> {
            config: $cfg,
            scheduler: SC,
            _phantom: PhantomData<I>,
        }

        impl<I, SC> $sbuilder<I, SC> {
            /// Build the scheduled corpus.
            pub fn build(self) -> Result<$sched<I, SC>> {
                let build: fn($cfg, SC) -> Result<$inner<I, SC>> = $build;
                Ok($sched(build(self.config, self.scheduler)?))
            }
        }

        impl<I, SC> $sched<I, SC>
        where
            $cfg: Default,
        {
            /// Get a builder for this corpus using the given [`Scheduler`].
            #[must_use]
            pub fn builder(scheduler: SC) -> $sbuilder<I, SC> {
                $sbuilder { config: <$cfg>::default(), scheduler, _phantom: PhantomData }
            }
        }

        $(#[$obmeta])*
        #[derive(Debug, Clone)]
        pub struct $obuilder<I> {
            config: $cfg,
            _phantom: PhantomData<I>,
        }

        impl<I> Default for $obuilder<I>
        where
            $cfg: Default,
        {
            fn default() -> Self {
                Self { config: <$cfg>::default(), _phantom: PhantomData }
            }
        }

        impl<I> $obuilder<I> {
            /// Build the objective corpus.
            pub fn build(self) -> Result<$obj<I>> {
                let build: fn($cfg, NopScheduler) -> Result<$inner<I, NopScheduler>> = $build;
                Ok($obj(build(self.config, NopScheduler)?))
            }
        }

        impl<I> $obj<I> where $cfg: Default {
            /// Get a builder for this objective corpus.
            #[must_use]
            pub fn builder() -> $obuilder<I> { $obuilder::default() }
        }

        $(
            impl<I, SC> $sbuilder<I, SC> {
                /// Root this corpus at the worker's directory.
                pub fn from_worker<W: Worker>(self, worker: &W) -> Result<Self> {
                    Ok(self.root_dir(worker.workdir().$sdir()?))
                }
            }

            impl<I> $obuilder<I> {
                /// Root this objective corpus at the worker's directory.
                pub fn from_worker<W: Worker>(self, worker: &W) -> Result<Self> {
                    Ok(self.root_dir(worker.workdir().$odir()?))
                }
            }
        )?
    };
}

/// Config for the [`CachedOnDiskCorpusBuilder`], bundling the on-disk store config
/// with the cache length.
#[derive(Debug, Clone)]
pub struct CachedOnDiskConfig {
    store_builder: OnDiskStoreBuilder,
    cache_max_len: usize,
}

type InnerInMemoryCorpus<I, SC> = SingleCorpus<I, InnerStdInMemoryStore<I>, SC>;

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

define_corpus! {
    inner = InnerInMemoryCorpus,
    scheduled = InMemoryCorpus,
    objective = ObjectiveInMemoryCorpus,
    scheduled_builder = InMemoryCorpusBuilder,
    objective_builder = ObjectiveInMemoryCorpusBuilder,
    config = InMemoryStoreBuilder,
    build = |cfg, scheduler| -> Result<_> {
        Ok(SingleCorpus::new(cfg.build()?, scheduler))
    },
}

define_corpus! {
    inner = InnerOnDiskCorpus,
    scheduled = OnDiskCorpus,
    objective = ObjectiveOnDiskCorpus,
    scheduled_builder = OnDiskCorpusBuilder,
    objective_builder = ObjectiveOnDiskCorpusBuilder,
    config = OnDiskStoreBuilder,
    workdir { scheduled = corpus_dir, objective = objective_dir },
    build = |cfg, scheduler| -> Result<_> {
        Ok(SingleCorpus::new(cfg.build()?, scheduler))
    },
}

define_corpus! {
    inner = InnerInMemoryOnDiskCorpus,
    scheduled = InMemoryOnDiskCorpus,
    objective = ObjectiveInMemoryOnDiskCorpus,
    scheduled_builder = InMemoryOnDiskCorpusBuilder,
    objective_builder = ObjectiveInMemoryOnDiskCorpusBuilder,
    config = OnDiskStoreBuilder,
    workdir { scheduled = corpus_dir, objective = objective_dir },
    build = |cfg, scheduler| -> Result<_> {
        Ok(CombinedCorpus::new(
            scheduler, IdentityCache,
            InnerStdInMemoryStore::default(), cfg.build()?,
        ))
    },
}

define_corpus! {
    inner = InnerCachedOnDiskCorpus,
    scheduled = CachedOnDiskCorpus,
    objective = ObjectiveCachedOnDiskCorpus,
    scheduled_builder = CachedOnDiskCorpusBuilder,
    objective_builder = ObjectiveCachedOnDiskCorpusBuilder,
    config = CachedOnDiskConfig,
    workdir { scheduled = corpus_dir, objective = objective_dir },
    build = |cfg, scheduler| -> Result<_> {
        Ok(CombinedCorpus::new(
            scheduler, FifoCache::new(cfg.cache_max_len),
            InnerStdInMemoryStore::default(), cfg.store_builder.build()?,
        ))
    },
}

impl<I, SC> OnDiskCorpusBuilder<I, SC> {
    /// Set the root directory, where the testcases will be stored.
    #[must_use]
    pub fn root_dir(mut self, root_dir: impl AsRef<Path>) -> Self {
        self.config.root_dir(root_dir);
        self
    }

    /// Set the on-disk filename format
    #[must_use]
    pub fn filename_format(mut self, filename_format: TestcaseFilenameFormat) -> Self {
        self.config.filename_format(filename_format);
        self
    }
}

impl<I, SC> InMemoryOnDiskCorpusBuilder<I, SC> {
    /// Set the root directory, where the testcases will be stored.
    #[must_use]
    pub fn root_dir(mut self, root_dir: impl AsRef<Path>) -> Self {
        self.config.root_dir(root_dir);
        self
    }

    /// Set the on-disk filename format
    #[must_use]
    pub fn filename_format(mut self, filename_format: TestcaseFilenameFormat) -> Self {
        self.config.filename_format(filename_format);
        self
    }
}

impl<I, SC> CachedOnDiskCorpusBuilder<I, SC> {
    /// Set the cache max length.
    #[must_use]
    pub fn cache_max_len(mut self, cache_max_len: usize) -> Self {
        self.config.cache_max_len = cache_max_len;
        self
    }

    /// Set the root directory, where the testcases will be stored.
    #[must_use]
    pub fn root_dir(mut self, root: impl AsRef<Path>) -> Self {
        self.config.store_builder.root_dir(root);
        self
    }

    /// Set the on-disk filename format
    #[must_use]
    pub fn filename_format(mut self, filename_format: TestcaseFilenameFormat) -> Self {
        self.config.store_builder.filename_format(filename_format);
        self
    }
}

impl<I> ObjectiveOnDiskCorpusBuilder<I> {
    /// Set the root directory, where the testcases will be stored.
    #[must_use]
    pub fn root_dir(mut self, root_dir: impl AsRef<Path>) -> Self {
        self.config.root_dir(root_dir);
        self
    }

    /// Set the on-disk filename format
    #[must_use]
    pub fn filename_format(mut self, filename_format: TestcaseFilenameFormat) -> Self {
        self.config.filename_format(filename_format);
        self
    }
}

impl<I> ObjectiveInMemoryOnDiskCorpusBuilder<I> {
    /// Set the root directory, where the testcases will be stored.
    #[must_use]
    pub fn root_dir(mut self, root_dir: impl AsRef<Path>) -> Self {
        self.config.root_dir(root_dir);
        self
    }

    /// Set the on-disk filename format
    #[must_use]
    pub fn filename_format(mut self, filename_format: TestcaseFilenameFormat) -> Self {
        self.config.filename_format(filename_format);
        self
    }
}

impl<I> ObjectiveCachedOnDiskCorpusBuilder<I> {
    /// Set the root directory, where the testcases will be stored.
    #[must_use]
    pub fn root_dir(mut self, root: impl AsRef<Path>) -> Self {
        self.config.store_builder.root_dir(root);
        self
    }

    /// Set the on-disk filename format
    #[must_use]
    pub fn filename_format(mut self, filename_format: TestcaseFilenameFormat) -> Self {
        self.config.store_builder.filename_format(filename_format);
        self
    }

    /// Set the cache max length.
    #[must_use]
    pub fn cache_max_len(mut self, cache_max_len: usize) -> Self {
        self.config.cache_max_len = cache_max_len;
        self
    }
}

impl<I, SC> CachedOnDiskCorpus<I, SC> {
    /// Get the fallback store
    pub fn fallback_store(&self) -> &InnerStdOnDiskStore<I> {
        self.0.fallback_store()
    }
}

impl Default for CachedOnDiskConfig {
    fn default() -> Self {
        Self {
            store_builder: OnDiskStoreBuilder::default(),
            cache_max_len: DEFAULT_CACHE_LEN,
        }
    }
}

impl<I, SC> InMemoryCorpus<I, SC> {
    /// Create a new [`InMemoryCorpus`] with the given [`Scheduler`].
    #[must_use]
    pub fn new(scheduler: SC) -> Self {
        InMemoryCorpus(InnerInMemoryCorpus::new(
            InnerStdInMemoryStore::default(),
            scheduler,
        ))
    }
}

impl InMemoryCorpus<NopInput, NopScheduler> {
    /// Create a new [`InMemoryCorpus`] with the given [`Scheduler`].
    #[must_use]
    pub fn nop() -> Self {
        InMemoryCorpus(InnerInMemoryCorpus::new(
            InnerStdInMemoryStore::default(),
            NopScheduler,
        ))
    }
}

impl<I> ObjectiveInMemoryCorpus<I> {
    /// Create a new [`ObjectiveInMemoryCorpus`].
    #[must_use]
    pub fn new() -> Self {
        ObjectiveInMemoryCorpus(InnerInMemoryCorpus::new(
            InnerStdInMemoryStore::default(),
            NopScheduler,
        ))
    }
}

impl<I> Default for ObjectiveInMemoryCorpus<I> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I, SC> OnDiskCorpus<I, SC> {
    /// Create a new [`OnDiskCorpus`].
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

impl<I> ObjectiveOnDiskCorpus<I> {
    /// Create a new [`ObjectiveOnDiskCorpus`].
    pub fn new(root: impl AsRef<Path>, filename_format: TestcaseFilenameFormat) -> Result<Self> {
        Ok(ObjectiveOnDiskCorpus(InnerOnDiskCorpus::new(
            InnerStdOnDiskStore::new(root, filename_format)?,
            NopScheduler,
        )))
    }
}
