//! Nop controller and workers.

use std::time::Duration;

use crate::controllers::{Controller, Descriptor, Workdir, Worker};
use libaflmm_core::{Result, WorkerId};

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
}

impl Controller for NopController {
    type Worker = NopWorker;
    type Descriptor = NopDescriptor;
    type Command = ();

    fn create_worker(&mut self) -> Result<Self::Worker> {
        Ok(NopWorker)
    }

    #[expect(refining_impl_trait)]
    fn worker_descriptors(&self) -> &[Self::Descriptor] {
        unimplemented!("nop controller has no workers");
    }

    #[expect(refining_impl_trait)]
    fn worker_descriptors_mut(&mut self) -> &mut [Self::Descriptor] {
        unimplemented!("nop controller has no workers");
    }

    fn wait_notifications(&mut self, _timeout: Option<Duration>) -> Result<()> {
        Ok(())
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
}

impl Worker for NopWorker {
    type Controller = NopController;
    type Notification = ();

    fn id(&self) -> WorkerId {
        unimplemented!("nop controller has no id");
    }

    fn descriptor(&self) -> &NopDescriptor {
        &NopDescriptor
    }

    fn workdir(&self) -> &Workdir {
        unimplemented!("nop controller has no workdir");
    }

    fn workdir_mut(&mut self) -> &mut Workdir {
        unimplemented!("nop controller has no workdir");
    }

    fn reconcile(&self) -> Result<()> {
        Ok(())
    }

    fn send_notification(&mut self, _notification: Self::Notification) -> Result<()> {
        todo!()
    }

    fn poll_commands(
        &mut self,
    ) -> Result<impl Iterator<Item = <Self::Controller as Controller>::Command>> {
        Ok([].into_iter())
    }
}
