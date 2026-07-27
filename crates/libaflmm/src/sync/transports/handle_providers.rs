use crate::{Result, inputs::Input, sync::Transferable};
use core::{fmt::Debug, marker::PhantomData};
use libaflmm_core::internal_bug;
use std::path::PathBuf;

pub type SerializedHandleProviderFactory = DefaultHandleProviderFactory<SerializedHandleProvider>;
pub type UnreachableHandleProviderFactory = DefaultHandleProviderFactory<UnreachableHandleProvider>;

/// A way to get a representation of an input.
/// Think of a file on the filesystem, which can be represented by its
/// [`Path`](std::path::Path).
pub trait HandleProvider<I>: Debug {
    /// An input handle, that represents a given input.
    type Handle: Transferable;

    /// Create a fresh [`Self::Handle`] from a given `input`.
    fn create_handle(&mut self, input: &I) -> Result<Self::Handle>;

    /// Fetch back an input from its [`Self::Handle`].
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
pub struct PathHandleProviderFactory;

#[derive(Debug, Default)]
pub struct SerializedHandleProvider;

#[derive(Debug, Default)]
pub struct UnreachableHandleProvider;

#[derive(Debug)]
#[expect(dead_code)]
pub struct PathHandleProvider {
    dir: PathBuf,
}

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

impl<I> HandleProvider<I> for PathHandleProvider {
    type Handle = PathBuf;

    fn create_handle(&mut self, _input: &I) -> Result<Self::Handle> {
        todo!()
    }

    fn resolve_handle(&mut self, _handle: Self::Handle) -> Result<I> {
        todo!()
    }
}

impl<D, I> HandleProviderFactory<D, I> for PathHandleProviderFactory {
    type Provider = PathHandleProvider;

    fn create<'a>(
        &mut self,
        _desc: &'a D,
        _sources: impl Iterator<Item = &'a D>,
    ) -> Result<Self::Provider> {
        todo!()
    }
}
