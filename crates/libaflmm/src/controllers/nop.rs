//! Nop controller and workers.

use crate::controllers::{Controller, Descriptor, Workdir, Worker};
use libaflmm_bolts::CoreId;
use libaflmm_core::{Result, WorkerId};
use serde::{Deserialize, Serialize};

/// Nop [`Controller`]
#[derive(Clone, Debug)]
pub struct NopController;

/// Nop [`Worker`]
#[derive(Clone, Debug)]
pub struct NopWorker;

/// Nop [`Descriptor`]
#[derive(Clone, Debug)]
pub struct NopDescriptor;

impl Descriptor for NopDescriptor {
    fn workdir(&self) -> &Workdir {
        panic!("No descriptor for NopDescriptor.");
    }

    fn workdir_mut(&mut self) -> &mut Workdir {
        panic!("No descriptor for NopDescriptor.");
    }

    fn worker_id(&self) -> WorkerId {
        panic!("No descriptor for NopDescriptor.");
    }

    fn core_id(&self) -> CoreId {
        panic!("No descriptor for NopDescriptor.");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NopCommand;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NopNotification;

impl Controller for NopController {
    type Worker = NopWorker;
    type Command = NopCommand;

    fn create_worker(&mut self, _core_id: CoreId) -> Result<Self::Worker> {
        Ok(NopWorker)
    }

    #[expect(refining_impl_trait)]
    fn worker_descriptors(&self) -> &[NopDescriptor] {
        unimplemented!("nop controller has no workers");
    }

    #[expect(refining_impl_trait)]
    fn worker_descriptors_mut(&mut self) -> &mut [NopDescriptor] {
        unimplemented!("nop controller has no workers");
    }

    fn send_command(&mut self, _command: Self::Command, _worker_id: WorkerId) -> Result<()> {
        unimplemented!("nop controller has no workers");
    }

    fn send_command_all(&mut self, _command: Self::Command) -> Result<()> {
        unimplemented!("nop controller has no workers");
    }

    fn send_command_all_but(
        &mut self,
        _command: Self::Command,
        _worker_id: WorkerId,
    ) -> Result<()> {
        unimplemented!("nop controller has no workers");
    }

    fn wait_notifications(&mut self, _timeout: Option<std::time::Duration>) -> Result<()> {
        unimplemented!("nop controller has no workers");
    }
}

impl Worker for NopWorker {
    type Controller = NopController;
    type Descriptor = NopDescriptor;
    type Notification = NopNotification;

    fn descriptor(&self) -> &NopDescriptor {
        unimplemented!("nop controller has no descriptor");
    }

    fn descriptor_mut(&mut self) -> &mut NopDescriptor {
        unimplemented!("nop controller has no descriptor");
    }

    fn reconcile(&self) -> Result<()> {
        Ok(())
    }

    fn send_notification(&mut self, _notification: Self::Notification) -> Result<()> {
        unimplemented!("nop controller has no descriptor");
    }

    fn poll_commands(&mut self) -> Result<impl Iterator<Item = NopCommand>> {
        Ok([].into_iter())
    }
}
