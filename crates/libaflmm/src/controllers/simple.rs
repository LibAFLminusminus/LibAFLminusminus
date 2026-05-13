//! Simple controller and worker.

use alloc::vec::Vec;
use std::{
    fs,
    path::{Path, PathBuf},
};

use libaflmm_core::{WorkerId, illegal_argument, internal_bug};
use nix::unistd::{dup2_stderr, dup2_stdout};

use crate::{Controller, Descriptor, Result, Workdir, WorkdirFile, Worker, launchers::InstanceId};

/// Builder for the [`SimpleController`].
#[derive(Debug)]
pub struct SimpleControllerBuilder {
    root_dir: PathBuf,
    overwrite: bool,
    worker_stdout: Option<WorkdirFile>,
    worker_stderr: Option<WorkdirFile>,
    worker_stats: Option<WorkdirFile>,
}

/// A simple [`Controller`].
#[derive(Debug)]
pub struct SimpleController {
    root_dir: PathBuf,
    id_ctr: u32,
    workers: Vec<SimpleWorkerRepr>,
    worker_stdout: Option<WorkdirFile>,
    worker_stderr: Option<WorkdirFile>,
    worker_stats: Option<WorkdirFile>,
}

/// A simple [`Worker`].
#[derive(Debug)]
pub struct SimpleWorker {
    /// the descriptor describing this client
    descriptor: SimpleDescriptor,
}

/// A representation of a [`SimpleWorker`].
#[derive(Debug)]
pub struct SimpleWorkerRepr {
    descriptor: SimpleDescriptor,
}

/// A simple descriptor for a [`SimpleWorker`].
#[derive(Debug, Clone)]
pub struct SimpleDescriptor {
    /// workdir of the worker
    workdir: Workdir,
    /// client id of this process
    worker_id: WorkerId,
}

/// The launcher should create instantiate this alongside binding this instance to a specific core id
impl SimpleDescriptor {
    /// Default constructor
    pub fn new<P: AsRef<Path>>(
        root_dir: P,
        stdout: Option<WorkdirFile>,
        stderr: Option<WorkdirFile>,
        stats: Option<WorkdirFile>,
        id: WorkerId,
    ) -> Result<Self> {
        let workdir = Workdir::new(root_dir, stdout, stderr, stats)?;

        Ok(SimpleDescriptor {
            workdir,
            worker_id: id,
        })
    }
}

impl SimpleController {
    /// Create a new [`SimpleGlobalController`] and will use `root_dir` as the root directory.
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

    /// Get a [`SimpleControllerBuilder`], to build a [`SimpleController`].
    #[must_use]
    pub fn builder() -> SimpleControllerBuilder {
        SimpleControllerBuilder::default()
    }
}

impl Controller for SimpleController {
    type Worker = SimpleWorker;
    type Descriptor = SimpleDescriptor;

    fn create_worker(&mut self) -> Result<SimpleWorker> {
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

        let descriptor = SimpleDescriptor::new(
            worker_dir,
            self.worker_stdout.clone(),
            self.worker_stderr.clone(),
            self.worker_stats.clone(),
            worker_id,
        )?;

        let cl = SimpleWorker::new(descriptor.clone());
        self.workers.push(SimpleWorkerRepr { descriptor });
        Ok(cl)
    }

    fn worker_descriptors(&self) -> impl IntoIterator<Item = &Self::Descriptor> {
        self.workers.iter().map(|repr| &repr.descriptor)
    }

    fn worker_descriptors_mut(&mut self) -> impl IntoIterator<Item = &mut Self::Descriptor> {
        self.workers.iter_mut().map(|repr| &mut repr.descriptor)
    }

    fn on_worker_start(&mut self, descriptor: &Self::Descriptor, _id: InstanceId) -> Result<()> {
        log::info!("Started worker {:?}", descriptor.worker_id);
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
}

impl Worker for SimpleWorker {
    type Controller = SimpleController;

    fn id(&self) -> WorkerId {
        self.descriptor.worker_id
    }

    fn descriptor(&self) -> &SimpleDescriptor {
        &self.descriptor
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
}

impl SimpleWorker {
    /// Get a new [`SimpleWorker`].
    #[must_use]
    pub fn new(descriptor: SimpleDescriptor) -> Self {
        Self { descriptor }
    }
}

impl Default for SimpleControllerBuilder {
    fn default() -> Self {
        Self {
            overwrite: false,
            root_dir: PathBuf::from("./workdir"),
            worker_stdout: Some(WorkdirFile::Path(PathBuf::from("logs.out"))),
            worker_stderr: Some(WorkdirFile::Path(PathBuf::from("logs.err"))),
            worker_stats: Some(WorkdirFile::Path(PathBuf::from("fuzzer_stats"))),
        }
    }
}

impl SimpleControllerBuilder {
    /// Set to `true` if the [`Workdir`] should be overwritten.
    ///
    /// If set to `false` and the [`Workdir`] already exists, it will error out.
    #[must_use]
    pub fn overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }

    /// Set the root directory of the fuzzing session.
    #[must_use]
    pub fn root_dir(mut self, root_dir: impl Into<PathBuf>) -> Self {
        self.root_dir = root_dir.into();
        self
    }

    /// Set [`SimpleWorker`]'s stdout.
    #[must_use]
    pub fn worker_stdout(mut self, file_output: Option<WorkdirFile>) -> Self {
        self.worker_stdout = file_output;
        self
    }

    /// Set [`SimpleWorker`]'s stderr.
    #[must_use]
    pub fn worker_stderr(mut self, file_output: Option<WorkdirFile>) -> Self {
        self.worker_stderr = file_output;
        self
    }

    /// Set [`SimpleWorker`]'s stats file.
    #[must_use]
    pub fn worker_stats(mut self, file_output: WorkdirFile) -> Self {
        self.worker_stats = Some(file_output);
        self
    }

    /// Build a [`SimpleController`].
    pub fn build(self) -> Result<SimpleController> {
        SimpleController::new(
            self.root_dir,
            self.worker_stdout,
            self.worker_stderr,
            self.worker_stats,
            self.overwrite,
        )
    }
}

impl Descriptor for SimpleDescriptor {
    fn workdir(&self) -> &Workdir {
        &self.workdir
    }

    fn workdir_mut(&mut self) -> &mut Workdir {
        &mut self.workdir
    }

    fn worker_id(&self) -> WorkerId {
        self.worker_id
    }
}
