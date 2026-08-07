//! Nautilus grammar mutator, see <https://github.com/nautilus-fuzz/nautilus>

use crate::{
    common::{
        DependencyResolver, Registrator,
        nautilus::grammartec::{chunkstore::ChunkStore, context::Context},
    },
    controllers::Worker,
    corpus::{Corpus, TestcaseId},
    feedbacks::Feedback,
    generators::NautilusContext,
    inputs::NautilusInput,
    states::State,
};
use alloc::{borrow::Cow, string::String};
use core::fmt::Debug;
use libaflmm_bolts::{Named, anymap::named_metadata_mut};
use libaflmm_core::Result;
use serde::{Deserialize, Serialize};
use std::{fs::create_dir_all, path::PathBuf};

pub static NAUTILUS_CHUNKS_METADATA_NAME: &str = "NautilusChunksMetadata";

/// Metadata for Nautilus grammar mutator chunks
#[derive(Serialize, Deserialize)]
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

libaflmm_bolts::impl_serdeany!(NautilusChunksMetadata);

impl NautilusChunksMetadata {
    /// Creates a new [`NautilusChunksMetadata`]
    #[must_use]
    pub fn new(work_dir: String) -> Self {
        create_dir_all(format!("{work_dir}/outputs/chunks"))
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
    chunks_dir: PathBuf,
}

impl<'a> NautilusFeedback<'a> {
    /// Create a new [`NautilusFeedback`]
    pub fn new<W: Worker>(context: &'a NautilusContext, worker: &W) -> Result<Self> {
        Ok(Self {
            ctx: &context.ctx,
            chunks_dir: worker.workdir().create_dir("nautilus_chunks")?,
        })
    }
}

impl Named for NautilusFeedback<'_> {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("NautilusFeedback");
        &NAME
    }
}

impl DependencyResolver for NautilusFeedback<'_> {
    fn register_md(&mut self, registrator: &mut Registrator) -> Result<()> {
        registrator.register_md(
            NAUTILUS_CHUNKS_METADATA_NAME,
            NautilusChunksMetadata::new(self.chunks_dir.to_string_lossy().into_owned()),
        );

        Ok(())
    }
}

impl<OT, S> Feedback<NautilusInput, OT, S> for NautilusFeedback<'_>
where
    S: State<Input = NautilusInput>,
{
    fn append_metadata(
        &mut self,
        state: &mut S,
        _observers: &OT,
        testcase_id: &TestcaseId,
    ) -> Result<()> {
        let input = state.corpus().get(testcase_id)?;
        let meta = named_metadata_mut::<NautilusChunksMetadata>(
            state.metadata_map_mut(),
            NAUTILUS_CHUNKS_METADATA_NAME,
        )?;
        meta.cks.add_tree(input.input().tree.clone(), self.ctx);
        Ok(())
    }
}
