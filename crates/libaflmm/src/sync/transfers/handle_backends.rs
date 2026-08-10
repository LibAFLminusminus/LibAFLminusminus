use core::{fmt::Debug, marker::PhantomData};

use libaflmm_core::internal_bug;

use crate::{Result, inputs::Input, sync::Transferable};

/// A way to get a representation of an input.
/// Think of a file on the filesystem, which can be represented by its
/// [`Path`](std::path::Path).
pub trait InputHandleBackend<I>: Debug {
    /// An input handle, that represents a given input.
    type Handle: Transferable;

    /// Create a [`Self::Handle`] from a given `input`.
    fn create_handle(&mut self, input: &I) -> Result<Self::Handle>;

    /// Get back an input from its [`Self::Handle`].
    fn resolve_handle(&mut self, handle: Self::Handle) -> Result<I>;
}

pub trait InputHandleBackendFactory<D, I>: Debug {
    type Backend: InputHandleBackend<I>;

    fn create<'a>(
        &mut self,
        desc: &'a D,
        sources: impl Iterator<Item = &'a D>,
    ) -> Result<Self::Backend>;

    /// Called once every "create" have been called
    /// It's useful for some input handle backends, like SHM.
    fn finalize(&mut self) -> Result<()> {
        Ok(())
    }
}

pub type SerializedInputHandleBackendFactory =
    DefaultInputHandleBackendFactory<SerializedInputHandleBackend>;
pub type UnreachableInputHandleBackendFactory =
    DefaultInputHandleBackendFactory<UnreachableInputHandleBackend>;

/// Creates a [`Default`] [`InputHandleBackend`].
#[derive(Debug, Default)]
pub struct DefaultInputHandleBackendFactory<HB>(PhantomData<HB>);

#[derive(Debug, Default)]
pub struct SerializedInputHandleBackend;

#[derive(Debug, Default)]
pub struct UnreachableInputHandleBackend;

impl<D, HB, I> InputHandleBackendFactory<D, I> for DefaultInputHandleBackendFactory<HB>
where
    HB: InputHandleBackend<I> + Default,
{
    type Backend = HB;

    fn create<'a>(
        &mut self,
        _desc: &'a D,
        _sources: impl Iterator<Item = &'a D>,
    ) -> Result<Self::Backend> {
        Ok(HB::default())
    }
}

impl<I> InputHandleBackend<I> for SerializedInputHandleBackend
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

impl<I> InputHandleBackend<I> for UnreachableInputHandleBackend {
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
