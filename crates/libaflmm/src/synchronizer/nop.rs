use crate::{
    Result,
    controllers::Worker,
    inputs::Input,
    synchronizer::{InputRepr, Synchronizer},
};
use std::path::PathBuf;

pub struct NopInputRepr(PathBuf);

pub struct NopSynchronizer;

impl<I> InputRepr<I> for NopInputRepr
where
    I: Input,
{
    fn into_input(self) -> Result<I> {
        I::from_file(self.0)
    }
}

impl<I> Synchronizer<I> for NopSynchronizer
where
    I: Input,
{
    type InputRepr = NopInputRepr;

    fn report_input<W: Worker>(
        &mut self,
        _desc: &mut W::Descriptor,
        _input_repr: Self::InputRepr,
    ) -> Result<()> {
        Ok(())
    }

    fn sync_input<W: Worker>(
        &mut self,
        _desc: &mut W::Descriptor,
    ) -> Result<impl Iterator<Item = Self::InputRepr>> {
        Ok([].into_iter())
    }
}
