use crate::{
    controllers::{
        Controller, Descriptor, StdDescriptor, StdWorker, StdWorkerRepr, WorkdirFile,
        standard::builder::StdControllerBuilder,
    },
    launchers::InstanceId,
};
use libaflmm_bolts::CoreId;
use libaflmm_core::{Result, WorkerId, illegal_argument, internal_bug};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

// C: Command
// D: Descriptor
/// The standard controller.
#[derive(Debug)]
pub struct StdController {
    root_dir: PathBuf,
    id_ctr: u32,
    workers: Vec<StdWorkerRepr>,
    worker_stdout: Option<WorkdirFile>,
    worker_stderr: Option<WorkdirFile>,
    worker_stats: Option<WorkdirFile>,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum StdCommand {
    Shutdown,
    NewInput(PathBuf),
}

impl Controller for StdController {
    type Worker = StdWorker;
    type Descriptor = StdDescriptor;
    type Command = StdCommand;

    fn create_worker(&mut self, core_id: CoreId) -> Result<StdWorker> {
        let worker_id = WorkerId(self.id_ctr);
        self.id_ctr += 1;

        let worker_dir = self.root_dir.join(format!("worker_{}", worker_id.0));

        if worker_dir.exists() {
            return Err(internal_bug!(
                "The worker dir \"{}\" already exists.",
                worker_dir.display()
            ));
        }

        fs::create_dir(worker_dir.as_path())?;

        let descriptor = StdDescriptor::new(
            worker_dir,
            self.worker_stdout.clone(),
            self.worker_stderr.clone(),
            self.worker_stats.clone(),
            worker_id,
            core_id,
        )?;

        let cl = StdWorker::new(descriptor.clone());
        self.workers.push(StdWorkerRepr::new(descriptor));
        Ok(cl)
    }

    fn worker_descriptors(&self) -> impl IntoIterator<Item = &Self::Descriptor> {
        self.workers.iter().map(|repr| repr.descriptor())
    }

    fn worker_descriptors_mut(&mut self) -> impl IntoIterator<Item = &mut Self::Descriptor> {
        self.workers.iter_mut().map(|repr| repr.descriptor_mut())
    }

    fn on_worker_start(&mut self, descriptor: &Self::Descriptor, _id: InstanceId) -> Result<()> {
        log::info!("Started worker {:?}", descriptor.worker_id());
        Ok(())
    }

    fn on_worker_termination(
        &mut self,
        descriptor: &Self::Descriptor,
        _termination_code: nix::sys::signal::Signal,
    ) -> Result<()> {
        log::info!("Started worker {:?}", descriptor.worker_id);
        Ok(())
    }

    fn send_command(&mut self, _command: Self::Command, _worker_id: WorkerId) -> Result<()> {
        todo!()
    }

    fn send_command_all(&mut self, _command: Self::Command) -> Result<()> {
        todo!()
    }

    fn send_command_all_but(
        &mut self,
        _command: Self::Command,
        _worker_id: WorkerId,
    ) -> Result<()> {
        todo!()
    }

    fn wait_notifications(&mut self, _timeout: Option<std::time::Duration>) -> Result<()> {
        todo!()
    }
}

impl StdController {
    /// Create a new [`StdGlobalController`] and will use `root_dir` as the root directory.
    /// If overwrite is true, the `root_dir` will be removed before being created again.
    pub fn new(
        root_dir: PathBuf,
        worker_stdout: Option<WorkdirFile>,
        worker_stderr: Option<WorkdirFile>,
        worker_stats: Option<WorkdirFile>,
        overwrite: bool,
    ) -> Result<Self> {
        if root_dir.exists() {
            if overwrite {
                fs::remove_dir_all(root_dir.as_path())?;
            } else {
                return Err(illegal_argument!(
                    "Wordir already exists: {}. Set `overwrite` to `true` if you want to overwrite.",
                    root_dir.display()
                ));
            }
        }

        fs::create_dir(root_dir.as_path())?;

        Ok(Self {
            root_dir,
            worker_stdout,
            worker_stderr,
            worker_stats,
            workers: Vec::new(),
            id_ctr: 0,
        })
    }

    /// Get a [`StdControllerBuilder`], to build a [`StdController`].
    #[must_use]
    pub fn builder() -> StdControllerBuilder {
        StdControllerBuilder::default()
    }
}
