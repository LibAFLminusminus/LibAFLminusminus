use crate::{
    controllers::{StdController, StdDescriptor, WorkdirFile, standard::controller::HandleOf},
    sync::{Orchestrator, StdCommand, StdNotification, StdOrchestrator},
};
use core::{fmt::Debug, marker::PhantomData};
use libaflmm_core::Result;
use std::path::PathBuf;

pub struct ControllerBuilder;

/// Builder for the [`StdController`].
#[derive(Debug)]
pub struct StdControllerBuilder<I, O> {
    orchestrator: O,
    root_dir: PathBuf,
    overwrite: bool,
    worker_stdout: WorkdirFile,
    worker_stderr: WorkdirFile,
    worker_stats: WorkdirFile,
    phantom: PhantomData<I>,
}

impl<I> Default for StdControllerBuilder<I, StdOrchestrator> {
    fn default() -> Self {
        Self {
            orchestrator: StdOrchestrator::default(),
            overwrite: true,
            root_dir: PathBuf::from("./workdir"),
            worker_stdout: WorkdirFile::Stdout,
            worker_stderr: WorkdirFile::Stderr,
            worker_stats: WorkdirFile::Path(PathBuf::from("fuzzer_stats")),
            phantom: PhantomData,
        }
    }
}

impl<I, O> StdControllerBuilder<I, O> {
    pub fn orchestrator<O2>(self, orchestrator: O2) -> StdControllerBuilder<I, O2> {
        StdControllerBuilder {
            orchestrator,
            overwrite: self.overwrite,
            root_dir: self.root_dir,
            worker_stats: self.worker_stats,
            worker_stderr: self.worker_stderr,
            worker_stdout: self.worker_stdout,
            phantom: self.phantom,
        }
    }

    /// Set to `true` if the [`Workdir`](crate::controllers::Workdir) should be overwritten.
    ///
    /// If set to `false` and the [`Workdir`](crate::controllers::Workdir) already exists, it will error out.
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

    /// Set [`StdWorker`](crate::controllers::StdWorker)'s stdout.
    #[must_use]
    pub fn worker_stdout(mut self, file_output: WorkdirFile) -> Self {
        self.worker_stdout = file_output;
        self
    }

    /// Set [`StdWorker`](crate::controllers::StdWorker)'s stderr.
    #[must_use]
    pub fn worker_stderr(mut self, file_output: WorkdirFile) -> Self {
        self.worker_stderr = file_output;
        self
    }

    /// Set [`StdWorker`](crate::controllers::StdWorker)'s stats file.
    #[must_use]
    pub fn worker_stats(mut self, file_output: WorkdirFile) -> Self {
        self.worker_stats = file_output;
        self
    }

    /// Build a [`StdController`].
    pub fn build(self) -> Result<StdController<I, O>>
    where
        I: Debug,
        O: Orchestrator<
                StdDescriptor,
                I,
                Command = StdCommand<HandleOf<I, O>>,
                Notification = StdNotification<HandleOf<I, O>>,
            >,
    {
        StdController::new(
            self.orchestrator,
            self.root_dir,
            self.worker_stdout,
            self.worker_stderr,
            self.worker_stats,
            self.overwrite,
        )
    }
}
