use crate::{
    Result,
    controllers::Worker,
    inputs::Input,
    sync::{Orchestrator, Synchronizer},
};
use std::path::PathBuf;

pub struct NopInputRepr(PathBuf);

pub struct NopSynchronizer;

pub struct StdOrchestrator;

impl<I> InputRepr<I> for NopInputRepr
where
    I: Input,
{
    fn load_input(&self) -> Result<I> {
        I::from_file(&self.0)
    }
}

impl<I, W> Orchestrator<I, W> for StdOrchestrator
where
    I: Input,
    W: Worker,
{
    type Synchronizer = NopSynchronizer;
}

impl<I, W> Synchronizer<I, W> for NopSynchronizer
where
    I: Input,
    W: Worker,
{
    type InputRepr = NopInputRepr;

    fn report_input(
        &mut self,
        _desc: &mut W::Descriptor,
        _input_repr: Self::InputRepr,
    ) -> Result<()> {
        Ok(())
    }

    fn sync_input(
        &mut self,
        _desc: &mut W::Descriptor,
    ) -> Result<impl Iterator<Item = Self::InputRepr>> {
        Ok([].into_iter())
    }

    fn on_create(&mut self) -> Result<()> {
        Ok(())
    }

    fn on_new_worker(&mut self, _desc: &<W as Worker>::Descriptor) -> Result<()> {
        Ok(())
    }
}
