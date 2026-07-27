use crate::Result;
use crate::controllers::{Descriptor, Workdir, WorkdirFile};
use crate::sync::GroupId;
use libaflmm_bolts::CoreId;
use libaflmm_core::WorkerId;
use std::path::Path;

pub mod builder;
pub use builder::StdControllerBuilder;

pub mod controller;
pub use controller::StdController;

pub mod worker;
pub use worker::{StdWorker, StdWorkerRepr};

/// A Std descriptor for a [`StdWorker`].
#[derive(Debug, Clone)]
pub struct StdDescriptor {
    /// workdir of the worker
    workdir: Workdir,
    /// client id of this process
    worker_id: WorkerId,
    /// core id of this process
    core_id: Option<CoreId>,
    /// groups ID of this process
    group_id: GroupId,
}

/// The launcher should instantiate this alongside binding this instance to a specific core id
impl StdDescriptor {
    /// Default constructor
    pub fn new(
        root_dir: impl AsRef<Path>,
        stdout: WorkdirFile,
        stderr: WorkdirFile,
        stats: WorkdirFile,
        worker_id: WorkerId,
        core_id: Option<CoreId>,
        group_id: GroupId,
    ) -> Result<Self> {
        let workdir = Workdir::new(root_dir, stdout, stderr, stats)?;

        Ok(StdDescriptor {
            workdir,
            worker_id,
            core_id,
            group_id,
        })
    }
}

impl Descriptor for StdDescriptor {
    fn workdir(&self) -> &Workdir {
        &self.workdir
    }

    fn workdir_mut(&mut self) -> &mut Workdir {
        &mut self.workdir
    }

    fn worker_id(&self) -> WorkerId {
        self.worker_id
    }

    fn core_id(&self) -> Option<CoreId> {
        self.core_id
    }

    fn group_id(&self) -> GroupId {
        self.group_id
    }
}
