use crate::{Result, inputs::Input};
use libaflmm_core::WorkerId;
use serde::{Serialize, de::DeserializeOwned};
use std::fmt::Debug;

pub trait InputRepr<I> {
    type InputHandle: Debug + Serialize + DeserializeOwned + Clone;

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
    fn poll(&mut self) -> Result<(RCV, WorkerId)>;
}

/// The transfer mechanism for commands and notifications
pub trait Transport<CMD, D, I, NOTIF> {
    type ControllerSync: ControllerSync<CMD, NOTIF>;
    type WorkerSync: WorkerSync<NOTIF, CMD>;
    type InputRepr: InputRepr<I>;

    /// Create a new worker synchronizer for the worker using a given descriptor
    fn create_worker_sync<'a>(
        &mut self,
        descriptor: &'a D,
        sources: impl Iterator<Item = &'a D>,
    ) -> Result<(Self::InputRepr, Self::WorkerSync)>;

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

impl<I> InputRepr<I> for IdentityInputRepr
where
    I: Input,
{
    type InputHandle = I;

    fn create_handle(&mut self, input: &I) -> Result<Self::InputHandle> {
        Ok(input.clone())
    }

    fn handle_to_input(&mut self, handle: Self::InputHandle) -> Result<I> {
        Ok(handle)
    }
}

impl<CMD, D, I, NOTIF> Transport<CMD, D, I, NOTIF> for NopTransport
where
    I: Input,
{
    type ControllerSync = NopControllerSync;
    type WorkerSync = NopWorkerSync;
    type InputRepr = IdentityInputRepr;

    fn create_worker_sync<'a>(
        &mut self,
        descriptor: &'a D,
        sources: impl Iterator<Item = &'a D>,
    ) -> Result<(Self::InputRepr, Self::WorkerSync)> {
        Ok((IdentityInputRepr, NopWorkerSync))
    }

    fn create_controller_sync(self) -> Result<Self::ControllerSync> {
        Ok(NopControllerSync)
    }
}

impl<SD, RCV> ControllerSync<SD, RCV> for NopControllerSync {
    fn send(&mut self, worker: WorkerId, val: SD) -> Result<()> {
        Ok(())
    }

    fn poll(&mut self) -> Result<(RCV, WorkerId)> {
        Err(unimplemented!("Not implemented"))
    }
}

impl<SD, RCV> WorkerSync<SD, RCV> for NopWorkerSync {
    fn send(&mut self, val: SD) -> Result<()> {
        Ok(())
    }

    fn poll(&mut self) -> Result<impl Iterator<Item = RCV>> {
        Ok([].into_iter())
    }
}
