//! Nautilus grammar mutator, see <https://github.com/nautilus-fuzz/nautilus>
use alloc::{
    borrow::Cow,
    string::{String, ToString},
};
use core::fmt::Debug;
use std::fs::create_dir_all;

use libafl_bolts::Named;
use serde::{Deserialize, Serialize};

use crate::{
    DependencyResolver, Error,
    common::nautilus::grammartec::{chunkstore::ChunkStore, context::Context},
    corpus::{Corpus, Testcase, TestcaseId, testcase},
    feedbacks::Feedback,
    generators::NautilusContext,
    inputs::NautilusInput,
    states::{FlatState, HasCorpus, named_metadata_mut},
};

/// Metadata for Nautilus grammar mutator chunks
#[derive(Serialize, Deserialize, Default)]
pub struct NautilusChunksMetadata {
    /// the chunk store
    pub cks: ChunkStore,
}

impl Debug for NautilusChunksMetadata {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "NautilusChunksMetadata {{ {} }}",
            serde_json::to_string_pretty(self).unwrap(),
        )
    }
}

libafl_bolts::impl_serdeany!(NautilusChunksMetadata);

impl NautilusChunksMetadata {
    /// Creates a new [`NautilusChunksMetadata`]
    #[must_use]
    pub fn new(work_dir: String) -> Self {
        create_dir_all(format!("{}/outputs/chunks", &work_dir))
            .expect("Could not create folder in workdir");
        Self {
            cks: ChunkStore::new(work_dir),
        }
    }
}

/// A nautilus feedback for grammar fuzzing
#[derive(Debug)]
pub struct NautilusFeedback<'a> {
    ctx: &'a Context,
}

impl<'a> NautilusFeedback<'a> {
    /// Create a new [`NautilusFeedback`]
    #[must_use]
    pub fn new(context: &'a NautilusContext) -> Self {
        Self { ctx: &context.ctx }
    }
}

impl Named for NautilusFeedback<'_> {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("NautilusFeedback");
        &NAME
    }
}

impl DependencyResolver for NautilusFeedback<'_> {
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<(), Error> {
        registrator.register_md_default::<NautilusChunksMetadata>(self.name().to_string());
        Ok(())
    }
}

impl<OT, S> Feedback<NautilusInput, OT, S> for NautilusFeedback<'_>
where
    S: FlatState + HasCorpus<NautilusInput>,
{
    fn append_metadata(
        &mut self,
        state: &mut S,
        _observers: &OT,
        testcase_id: &TestcaseId,
    ) -> Result<(), Error> {
        let input = state.corpus().get(testcase_id)?;
        let meta = named_metadata_mut::<NautilusChunksMetadata>(
            state.named_metadata_map_mut(),
            self.name(),
        )?;
        meta.cks.add_tree(input.input().tree.clone(), self.ctx);
        Ok(())
    }

    #[cfg(feature = "track_hit_feedbacks")]
    fn last_result(&self) -> Result<bool, Error> {
        Ok(false)
    }
}
