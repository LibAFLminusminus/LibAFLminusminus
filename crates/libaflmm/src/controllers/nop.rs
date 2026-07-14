//! Nop controller and workers.

use crate::controllers::{Controller, Descriptor, Workdir, WorkdirFile, Worker};
use alloc::sync::Arc;
use libaflmm_bolts::CoreId;
use libaflmm_core::{Result, WorkerId};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

/// Nop [`Controller`]
#[derive(Clone, Debug)]
pub struct NopController;

/// Nop [`Worker`]
#[derive(Clone, Debug, Default)]
pub struct NopWorker {
    descriptor: NopDescriptor,
}

/// Nop [`Descriptor`]
#[derive(Clone, Debug)]
pub struct NopDescriptor {
    workdir: Workdir,
    _tmp_dir: Arc<TempDir>,
}

impl Default for NopDescriptor {
    fn default() -> Self {
        let tmp_dir = TempDir::new().expect("failed to create the nop worker temporary directory");
        let workdir = Workdir::new(
            tmp_dir.path(),
            WorkdirFile::Null,
            WorkdirFile::Null,
            WorkdirFile::Null,
        )
        .expect("the temporary directory should be a valid workdir root");

        Self {
            workdir,
            _tmp_dir: Arc::new(tmp_dir),
        }
    }
}

impl Descriptor for NopDescriptor {
    fn workdir(&self) -> &Workdir {
        &self.workdir
    }

    fn workdir_mut(&mut self) -> &mut Workdir {
        &mut self.workdir
    }

    fn worker_id(&self) -> WorkerId {
        WorkerId(0)
    }

    fn core_id(&self) -> Option<CoreId> {
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NopCommand;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NopNotification;

impl Controller for NopController {
    type Worker = NopWorker;
    type Command = NopCommand;

    fn root_dir(&self) -> &std::path::Path {
        unimplemented!("nop controller has no root directory");
    }

    fn create_worker(&mut self, _core_id: Option<CoreId>) -> Result<Self::Worker> {
        Ok(NopWorker::default())
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
        &self.descriptor
    }

    fn descriptor_mut(&mut self) -> &mut NopDescriptor {
        &mut self.descriptor
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
