use std::marker::PhantomData;

use libaflmm_core::{Result, WorkerId};
use nix::unistd::{dup2_stderr, dup2_stdout};
use serde::{Deserialize, Serialize};

use crate::{
    controllers::{Controller, StdDescriptor, Workdir, Worker, standard::StdController},
    sync::Synchronizer,
};

/// A simple [`Worker`].
#[derive(Debug)]
pub struct StdWorker<I, SY> {
    /// the descriptor describing this client
    descriptor: StdDescriptor,
    sync: SY,
    phantom: PhantomData<I>,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum StdNotification {
    Ping,
}

/// A representation of a [`StdWorker`].
#[derive(Debug)]
pub struct StdWorkerRepr<IR> {
    descriptor: StdDescriptor,
    phantom: PhantomData<IR>,
}

impl<IR> StdWorkerRepr<IR> {
    pub fn new(descriptor: StdDescriptor) -> Self {
        Self { descriptor }
    }

    pub fn descriptor(&self) -> &StdDescriptor {
        &self.descriptor
    }

    pub fn descriptor_mut(&mut self) -> &mut StdDescriptor {
        &mut self.descriptor
    }
}

impl<I, SY> Worker for StdWorker<I, SY>
where
    SY: Synchronizer<I> + Default,
{
    type Controller = StdController<I, SY>;
    type Descriptor = StdDescriptor;
    type Notification = StdNotification;

    fn id(&self) -> WorkerId {
        self.descriptor.worker_id
    }

    fn descriptor(&self) -> &StdDescriptor {
        &self.descriptor
    }

    fn descriptor_mut(&mut self) -> &mut StdDescriptor {
        &mut self.descriptor
    }

    fn workdir(&self) -> &Workdir {
        &self.descriptor.workdir
    }

    fn workdir_mut(&mut self) -> &mut Workdir {
        &mut self.descriptor.workdir
    }

    fn reconcile(&self) -> Result<()> {
        // do nothing
        Ok(())
    }

    fn pre_runtime_exec(&mut self) -> Result<()> {
        if let Some(f) = self.descriptor.workdir.stdout()? {
            dup2_stdout(f)?;
        }

        if let Some(f) = self.descriptor.workdir.stderr()? {
            dup2_stderr(f)?;
        }

        Ok(())
    }

    fn poll_commands<'a>(
        &'a mut self,
    ) -> Result<impl Iterator<Item = <Self::Controller as Controller>::Command>> {
        Ok([].into_iter())
    }

    fn send_notification(&mut self, _notification: Self::Notification) -> Result<()> {
        todo!()
    }
}

impl<I, SY> StdWorker<I, SY>
where
    SY: Default,
{
    /// Get a new [`StdWorker`].
    #[must_use]
    pub fn new(descriptor: StdDescriptor) -> Self {
        Self {
            descriptor,
            sync: SY::default(),
            phantom: PhantomData,
        }
    }
}
