use crate::Result;
use libaflmm_core::WorkerId;
use serde::{Serialize, de::DeserializeOwned};
use std::fmt::Debug;

/// A way to get a representation of an input.
/// Think of a file on the filesystem, which can be represented by its [`Path`].
pub trait InputRepr<I> {
    /// An input handle, that represents a given input.
    type InputHandle: Serialize + DeserializeOwned + Clone;

    /// Create a fresh [`Self::InputHandle`] from a given `input`
    fn create_handle(&mut self, input: &I) -> Result<Self::InputHandle>;

    /// Fetch back an input from its [`Self::InputHandle`]
    fn handle_to_input(&mut self, handle: Self::InputHandle) -> Result<I>;
}

/// The worker side of the synchronization mechanism
pub trait WorkerSync<SD, RCV> {
    /// Send a SD value to the [`ControllerSync`]
    fn send(&mut self, val: SD) -> Result<()>;

    /// Poll for RCV values from the [`ControllerSync`]
    fn poll(&mut self) -> Result<impl Iterator<Item = RCV>>;
}

pub trait ControllerSync<SD, RCV> {
    /// Send a SD value to the [`WorkerSync`] with the [`WorkerId`] `worker`.
    fn send(&mut self, worker: WorkerId, val: SD) -> Result<()>;

    /// Poll for RCV values from all the the [`WorkerSync`] attached to [`Self`].
    fn poll(&mut self) -> Result<impl Iterator<Item = (RCV, WorkerId)>>;
}

/// The transfer mechanism for commands and notifications
pub trait Transport<CMD, D, NOTIF> {
    /// Controller side of the sync mechanism
    type ControllerSync: ControllerSync<CMD, NOTIF>;
    /// Worker side of the sync mechanism
    type WorkerSync: WorkerSync<NOTIF, CMD>;

    /// Create a new worker synchronizer for the worker using a given descriptor
    fn create_worker_sync<'a>(
        &mut self,
        descriptor: &'a D,
        sources: impl Iterator<Item = &'a D>,
    ) -> Result<Self::WorkerSync>;

    /// Finalize the transport lifetime with the creation of the controller-side synchronizer
    fn create_controller_sync(self) -> Result<Self::ControllerSync>;
}

#[derive(Debug, Default)]
pub struct NopTransport;

#[derive(Debug, Default)]
pub struct NopControllerSync;

#[derive(Debug, Default)]
pub struct NopWorkerSync;

#[derive(Debug, Default)]
pub struct IdentityInputRepr;

impl<I> InputRepr<I> for IdentityInputRepr {
    type InputHandle = ();

    fn create_handle(&mut self, _input: &I) -> Result<Option<Self::InputHandle>> {
        Ok(None)
    }

    fn handle_to_input(&mut self, _handle: Self::InputHandle) -> Result<Option<I>> {
        Ok(None)
    }
}

impl<CMD, D, NOTIF> Transport<CMD, D, NOTIF> for NopTransport {
    type ControllerSync = NopControllerSync;
    type WorkerSync = NopWorkerSync;

    fn create_worker_sync<'a>(
        &mut self,
        _descriptor: &'a D,
        _sources: impl Iterator<Item = &'a D>,
    ) -> Result<Self::WorkerSync> {
        Ok(NopWorkerSync)
    }

    fn create_controller_sync(self) -> Result<Self::ControllerSync> {
        Ok(NopControllerSync)
    }
}

impl<SD, RCV> ControllerSync<SD, RCV> for NopControllerSync {
    fn send(&mut self, _worker: WorkerId, _val: SD) -> Result<()> {
        Ok(())
    }

    fn poll(&mut self) -> Result<impl Iterator<Item = (RCV, WorkerId)>> {
        Ok([].into_iter())
    }
}

impl<SD, RCV> WorkerSync<SD, RCV> for NopWorkerSync {
    fn send(&mut self, _val: SD) -> Result<()> {
        Ok(())
    }

    fn poll(&mut self) -> Result<impl Iterator<Item = RCV>> {
        Ok([].into_iter())
    }
}
