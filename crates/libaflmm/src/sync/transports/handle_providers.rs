use crate::{Result, inputs::Input, sync::Transferable};
use libaflmm_core::internal_bug;
use std::{fmt::Debug, marker::PhantomData};

pub type SerializedHandleProviderFactory = DefaultHandleProviderFactory<SerializedHandleProvider>;
pub type UnreachableHandleProviderFactory = DefaultHandleProviderFactory<UnreachableHandleProvider>;

/// A way to get a representation of an input.
/// Think of a file on the filesystem, which can be represented by its [`Path`].
pub trait HandleProvider<I>: Debug {
    /// An input handle, that represents a given input.
    type Handle: Transferable;

    /// Create a fresh [`Self::InputHandle`] from a given `input`
    fn create_handle(&mut self, input: &I) -> Result<Self::Handle>;

    /// Fetch back an input from its [`Self::InputHandle`]
    fn resolve_handle(&mut self, handle: Self::Handle) -> Result<I>;
}

pub trait HandleProviderFactory<D, I>: Debug {
    type Provider: HandleProvider<I>;

    fn create<'a>(
        &mut self,
        desc: &'a D,
        sources: impl Iterator<Item = &'a D>,
    ) -> Result<Self::Provider>;

    /// Called once every "create" have been called
    /// It's useful for some input reprs, like SHM.
    fn finalize(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Creates a [`Default`] [`HandleProvider`].
#[derive(Debug, Default)]
pub struct DefaultHandleProviderFactory<HP>(PhantomData<HP>);

#[derive(Debug, Default)]
pub struct SerializedHandleProvider;

#[derive(Debug, Default)]
pub struct UnreachableHandleProvider;

impl<D, HP, I> HandleProviderFactory<D, I> for DefaultHandleProviderFactory<HP>
where
    HP: HandleProvider<I> + Default,
{
    type Provider = HP;

    fn create<'a>(
        &mut self,
        _desc: &'a D,
        _sources: impl Iterator<Item = &'a D>,
    ) -> Result<Self::Provider> {
        Ok(HP::default())
    }
}

impl<I> HandleProvider<I> for SerializedHandleProvider
where
    I: Input,
{
    type Handle = Vec<u8>;

    fn create_handle(&mut self, input: &I) -> Result<Self::Handle> {
        Ok(postcard::to_allocvec(input)?)
    }

    fn resolve_handle(&mut self, handle: Self::Handle) -> Result<I> {
        Ok(postcard::from_bytes(&handle)?)
    }
}

impl<I> HandleProvider<I> for UnreachableHandleProvider {
    type Handle = ();

    fn create_handle(&mut self, _input: &I) -> Result<Self::Handle> {
        Err(internal_bug!(
            "The orchestrator is not supposed to share any testcase, this is an internal bug."
        ))
    }

    fn resolve_handle(&mut self, _handle: Self::Handle) -> Result<I> {
        Err(internal_bug!(
            "The orchestrator is not supposed to share any testcase, this is an internal bug."
        ))
    }
}
