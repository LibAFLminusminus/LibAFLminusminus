//! Nop controller and workers.

use crate::{
    controllers::{Controller, Descriptor, SyncWorker, Workdir, WorkdirFile, Worker},
    corpus::Testcase,
    sync::GroupId,
};
use alloc::sync::Arc;
use core::time::Duration;
use libaflmm_bolts::CoreId;
use libaflmm_core::{Result, WorkerId};
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
    fn group_id(&self) -> GroupId {
        GroupId { id: 0 }
    }

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

impl Controller for NopController {
    type Worker = NopWorker;
    type GroupConfig = ();

    fn root_dir(&self) -> &std::path::Path {
        unimplemented!("nop controller has no root directory");
    }

    fn register_group(
        &mut self,
        _config: Self::GroupConfig,
        _cores: &libaflmm_bolts::prelude::Cores,
    ) -> Result<GroupId> {
        unimplemented!("nop controller cannot register groups");
    }

    fn finalize_orchestration(&mut self) -> Result<()> {
        unimplemented!("nop controller cannot finalize orchestration");
    }

    fn take_group_workers(
        &mut self,
        _group: GroupId,
    ) -> Result<impl Iterator<Item = Self::Worker>> {
        Ok([].into_iter())
    }

    #[expect(refining_impl_trait)]
    fn worker_descriptors(&self) -> &[NopDescriptor] {
        unimplemented!("nop controller has no workers");
    }

    #[expect(refining_impl_trait)]
    fn worker_descriptors_mut(&mut self) -> &mut [NopDescriptor] {
        unimplemented!("nop controller has no workers");
    }

    fn wait_notifications(&mut self, _timeout: Option<Duration>) -> Result<()> {
        unimplemented!("nop controller has no workers");
    }

    fn shutdown(&mut self, _worker: WorkerId) -> Result<()> {
        Ok(())
    }

    // fn send_command(
    //     &mut self,
    //     command: <Self::Worker as Worker>::Command,
    //     _worker_id: WorkerId,
    // ) -> Result<()> {
    //     unimplemented!("nop controller cannot send commands");
    // }
}

impl Worker for NopWorker {
    type Descriptor = NopDescriptor;

    fn descriptor(&self) -> &NopDescriptor {
        &self.descriptor
    }

    fn descriptor_mut(&mut self) -> &mut NopDescriptor {
        &mut self.descriptor
    }

    fn poll_shutdown(&mut self) -> Result<bool> {
        Ok(false)
    }

    // fn send_notification(&mut self, _notification: Self::Notification) -> Result<()> {
    //     unimplemented!("nop controller has no descriptor");
    // }

    // fn poll_commands_filtered(
    //     &mut self,
    //     filter: impl FnMut(&Self::Command) -> bool,
    // ) -> Result<impl Iterator<Item = Self::Command>> {
    //     Ok([].into_iter())
    // }
}

impl<I> SyncWorker<I> for NopWorker {
    fn send_testcase(&mut self, _testcase: &Testcase<I>) -> Result<()> {
        Ok(())
    }

    fn recv_testcases(&mut self) -> Result<impl Iterator<Item = Testcase<I>>> {
        Ok([].into_iter())
    }
}
