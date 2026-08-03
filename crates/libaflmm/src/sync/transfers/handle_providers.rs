use crate::{Result, inputs::Input, sync::Transferable};
use core::{fmt::Debug, marker::PhantomData};
use libaflmm_core::internal_bug;
use std::path::PathBuf;

/// A way to get a representation of an input.
/// Think of a file on the filesystem, which can be represented by its
/// [`Path`](std::path::Path).
pub trait InputHandleBackend<I>: Debug {
    /// An input handle, that represents a given input.
    type Handle: Transferable;

    /// Create a fresh [`Self::Handle`] from a given `input`.
    fn create_input_handle(&mut self, input: &I) -> Result<Self::Handle>;

    /// Fetch back an input from its [`Self::Handle`].
    fn resolve_input_handle(&mut self, handle: Self::Handle) -> Result<I>;
}

pub trait InputHandleBackendFactory<D, I>: Debug {
    type Backend: InputHandleBackend<I>;

    fn create<'a>(
        &mut self,
        desc: &'a D,
        sources: impl Iterator<Item = &'a D>,
    ) -> Result<Self::Backend>;

    /// Called once every "create" have been called
    /// It's useful for some input reprs, like SHM.
    fn finalize(&mut self) -> Result<()> {
        Ok(())
    }
}

pub type SerializedInputHandleBackendFactory =
    DefaultInputHandleBackendFactory<SerializedInputhandleBackend>;
pub type UnreachableInputHandleBackendFactory =
    DefaultInputHandleBackendFactory<UnreachableInputHandleBackend>;

/// Creates a [`Default`] [`HandleProvider`].
#[derive(Debug, Default)]
pub struct DefaultInputHandleBackendFactory<HP>(PhantomData<HP>);

#[derive(Debug, Default)]
pub struct PathInputHandleBackendFactory;

#[derive(Debug, Default)]
pub struct SerializedInputhandleBackend;

#[derive(Debug, Default)]
pub struct UnreachableInputHandleBackend;

#[derive(Debug)]
#[expect(dead_code)]
pub struct PathInputHandleBackend {
    dir: PathBuf,
}

impl<D, HP, I> InputHandleBackendFactory<D, I> for DefaultInputHandleBackendFactory<HP>
where
    HP: InputHandleBackend<I> + Default,
{
    type Backend = HP;

    fn create<'a>(
        &mut self,
        _desc: &'a D,
        _sources: impl Iterator<Item = &'a D>,
    ) -> Result<Self::Backend> {
        Ok(HP::default())
    }
}

impl<I> InputHandleBackend<I> for SerializedInputhandleBackend
where
    I: Input,
{
    type Handle = Vec<u8>;

    fn create_input_handle(&mut self, input: &I) -> Result<Self::Handle> {
        Ok(postcard::to_allocvec(input)?)
    }

    fn resolve_input_handle(&mut self, handle: Self::Handle) -> Result<I> {
        Ok(postcard::from_bytes(&handle)?)
    }
}

impl<I> InputHandleBackend<I> for UnreachableInputHandleBackend {
    type Handle = ();

    fn create_input_handle(&mut self, _input: &I) -> Result<Self::Handle> {
        Err(internal_bug!(
            "The orchestrator is not supposed to share any testcase, this is an internal bug."
        ))
    }

    fn resolve_input_handle(&mut self, _handle: Self::Handle) -> Result<I> {
        Err(internal_bug!(
            "The orchestrator is not supposed to share any testcase, this is an internal bug."
        ))
    }
}

impl<I> InputHandleBackend<I> for PathInputHandleBackend {
    type Handle = PathBuf;

    fn create_input_handle(&mut self, _input: &I) -> Result<Self::Handle> {
        todo!()
    }

    fn resolve_input_handle(&mut self, _handle: Self::Handle) -> Result<I> {
        todo!()
    }
}

impl<D, I> InputHandleBackendFactory<D, I> for PathInputHandleBackendFactory {
    type Backend = PathInputHandleBackend;

    fn create<'a>(
        &mut self,
        _desc: &'a D,
        _sources: impl Iterator<Item = &'a D>,
    ) -> Result<Self::Backend> {
        todo!()
    }
}
