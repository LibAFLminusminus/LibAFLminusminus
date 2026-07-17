use crate::{
    controllers::{StdController, WorkdirFile},
    sync::StdOrchestrator,
};
use libaflmm_core::Result;
use std::path::PathBuf;

pub struct ControllerBuilder;

/// Builder for the [`SimpleController`].
#[derive(Debug)]
pub struct StdControllerBuilder<O> {
    orchestrator: O,
    root_dir: PathBuf,
    overwrite: bool,
    worker_stdout: Option<WorkdirFile>,
    worker_stderr: Option<WorkdirFile>,
    worker_stats: Option<WorkdirFile>,
}

impl Default for StdControllerBuilder<StdOrchestrator> {
    fn default() -> Self {
        Self {
            orchestrator: StdOrchestrator::default(),
            overwrite: false,
            root_dir: PathBuf::from("./workdir"),
            worker_stdout: Some(WorkdirFile::Path(PathBuf::from("logs.out"))),
            worker_stderr: Some(WorkdirFile::Path(PathBuf::from("logs.err"))),
            worker_stats: Some(WorkdirFile::Path(PathBuf::from("fuzzer_stats"))),
        }
    }
}

impl<O> StdControllerBuilder<O> {
    pub fn orchestrator<O2>(self, orchestrator: O2) -> StdControllerBuilder<O2> {
        Self {
            orchestrator,
            overwrite: self.overwrite,
            root_dir: self.root_dir,
            worker_stats: self.worker_stats,
            worker_stderr: self.worker_stderr,
            worker_stdout: self.worker.stdout,
        }
    }

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
    pub fn build<I>(self) -> Result<StdController<I, O>> {
        StdController::new(
            self.root_dir,
            self.worker_stdout,
            self.worker_stderr,
            self.worker_stats,
            self.overwrite,
        )
    }
}
