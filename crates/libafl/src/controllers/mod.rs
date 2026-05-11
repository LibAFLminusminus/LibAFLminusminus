//! Module defining [`Controller`]s.

use crate::{
    Result,
    launchers::InstanceId,
    states::{Stats, sync_stats},
};
use libafl_core::{Error, WorkerId, internal_bug};
use nix::sys::signal::Signal;
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

// pub mod aflpp;
pub mod nop;
pub use nop::{NopController, NopDescriptor, NopWorker};

pub mod simple;
pub use simple::{
    SimpleController, SimpleControllerBuilder, SimpleDescriptor, SimpleWorker, SimpleWorkerRepr,
};

/// A controller is the glue between multiple [`Worker`]s.
///
/// It is reponsible for creating and configurating workers.
/// Note [`Self`] and [`Worker`]s are tightly linked: they are supposed to be defined together and are interdependent.
pub trait Controller {
    /// The associated [`Worker`].
    type Worker: Worker;
    /// The associated [`Descriptor`].
    type Descriptor: Descriptor;

    /// Create a new [`Self::Worker`].
    /// The controller must keep track of the worker if necessary.
    fn create_worker(&mut self) -> Result<Self::Worker>;

    /// Get an iterator over all [`Self::Worker`] descriptors.
    fn worker_descriptors(&self) -> impl IntoIterator<Item = &Self::Descriptor>;

    /// Get a mutable iterator over all [`Self::Worker`] descriptors.
    fn worker_descriptors_mut(&mut self) -> impl IntoIterator<Item = &mut Self::Descriptor>;

    /// Hook called when a [`Self::Worker`] actually starts, with its associated [`InstanceId`].
    fn on_worker_start(&mut self, _descriptor: &Self::Descriptor, _id: InstanceId) -> Result<()> {
        Ok(())
    }

    /// Hook called when a controller exits with some exit code
    fn on_worker_exit(&mut self, _descriptor: &Self::Descriptor, _exit_code: i32) -> Result<()> {
        Ok(())
    }

    /// Hook called when a controller exits with a termination (e.g. signal / exception)
    fn on_worker_termination(
        &mut self,
        _descriptor: &Self::Descriptor,
        _termination_code: Signal, // TODO: make this os-agnostic
    ) -> Result<()> {
        Ok(())
    }
}

/// A worker is a representant of a fuzzing instance.
/// It is linked to a [`Controller`], which holds a reference to all workers.
pub trait Worker {
    /// The associated [`Controller`].
    type Controller: Controller<Worker = Self>;

    /// The client id of the worker.
    fn id(&self) -> WorkerId;

    /// Returns the descriptor of the worker.
    fn descriptor(&self) -> &<Self::Controller as Controller>::Descriptor;

    /// Returns the reference of the working directory of the worker.
    fn workdir(&self) -> &Workdir;

    /// Returns the mutable reference of the working directory of the worker.
    fn workdir_mut(&mut self) -> &mut Workdir;

    /// Do the work related to reconciling between instances: sharing corpus, etc.
    fn reconcile(&self) -> Result<()>;

    /// Hook called before the [`Runtime`] of the worker gets executed.
    fn pre_runtime_exec(&mut self) -> Result<()> {
        Ok(())
    }
}

/// A descriptor describes a [`Worker`].
pub trait Descriptor: Clone {
    /// Get the reference to the workdir of the [`Worker`].
    fn workdir(&self) -> &Workdir;

    /// Get the mutable reference to the workdir of the [`Worker`].
    fn workdir_mut(&mut self) -> &mut Workdir;

    /// Get the worker ID of the [`Worker`].
    fn worker_id(&self) -> WorkerId;
}

/// A workdir contains information relative to the working environement of a [`Worker`].
#[derive(Debug, Clone)]
pub struct Workdir {
    root_dir: PathBuf,
    stdout: Option<WorkdirFile>,
    stderr: Option<WorkdirFile>,
    stats: Option<WorkdirFile>,
}

/// A workdir file is an abstract representation of a file owned by a [`Workir`].
/// It enables to get a file as a [`File`] or a [`PathBuf`] transparently.
#[derive(Debug)]
pub enum WorkdirFile {
    /// File described as a [`PathBuf`].
    Path(PathBuf),
    /// File described as a [`File`].
    File(File),
}

impl Clone for WorkdirFile {
    fn clone(&self) -> Self {
        match self {
            WorkdirFile::Path(p) => WorkdirFile::Path(p.clone()),
            WorkdirFile::File(f) => WorkdirFile::File(f.try_clone().unwrap()),
        }
    }
}

impl WorkdirFile {
    /// Open a [`File`] in read-only mode from its path.
    ///
    /// Returns [`None`] if the path does not exist.
    pub fn get_file_rd(&mut self, root_dir: impl AsRef<Path>) -> Result<Option<File>> {
        let path: Option<PathBuf> = if let WorkdirFile::Path(p) = self {
            Some(p.clone())
        } else {
            None
        };

        if let Some(p) = path {
            let full_path = root_dir.as_ref().join(p.as_path());

            if !full_path.exists() {
                return Ok(None);
            }

            let file = OpenOptions::new().read(true).open(full_path)?;

            *self = WorkdirFile::File(file);
        }

        let file: &mut File = match self {
            WorkdirFile::File(file) => file,
            _ => {
                return Err(internal_bug!(
                    "The workdir file should be a file at this point"
                ));
            }
        };

        Ok(Some(file.try_clone().unwrap()))
    }

    /// Open a [`File`] in write-only mode from its path.
    ///
    /// Creates the file if the path does not exist.
    pub fn get_file_wr(&mut self, root_dir: impl AsRef<Path>) -> Result<File> {
        let path: Option<PathBuf> = if let WorkdirFile::Path(p) = self {
            Some(p.clone())
        } else {
            None
        };

        if let Some(p) = path {
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .open(root_dir.as_ref().join(p.as_path()))?;

            *self = WorkdirFile::File(file);
        }

        let file: &mut File = match self {
            WorkdirFile::File(file) => file,
            _ => {
                return Err(internal_bug!(
                    "The workdir file should be a file at this point"
                ));
            }
        };

        Ok(file.try_clone().unwrap())
    }
}

impl Workdir {
    /// Create a new [`Workdir`].
    pub fn new(
        root_dir: impl AsRef<Path>,
        stdout: Option<WorkdirFile>,
        stderr: Option<WorkdirFile>,
        stats: Option<WorkdirFile>,
    ) -> Result<Self> {
        if !root_dir.as_ref().is_dir() {
            return Err(Error::illegal_argument(
                "The client directory does not exit. This is a fuzzer bug.",
            ));
        }

        Ok(Self {
            root_dir: root_dir.as_ref().to_path_buf(),
            stdout,
            stderr,
            stats,
        })
    }

    /// Get the root directory of the [`Workdir`].
    pub fn root_dir(&self) -> &Path {
        self.root_dir.as_path()
    }

    /// Get the file associated with stdout for the [`Workdir`].
    pub fn stdout(&mut self) -> Result<Option<File>> {
        if let Some(wd_f) = &mut self.stdout {
            wd_f.get_file_wr(self.root_dir.as_path())
                .map(|file| Some(file))
        } else {
            Ok(None)
        }
    }

    /// Get the file associated with stderr for the [`Workdir`].
    pub fn stderr(&mut self) -> Result<Option<File>> {
        if let Some(wd_f) = &mut self.stderr {
            wd_f.get_file_wr(self.root_dir.as_path())
                .map(|file| Some(file))
        } else {
            Ok(None)
        }
    }

    /// Create a new file, relative to the workdir.
    ///
    /// If the file exists, it is opened without being truncated.
    /// Cursor will be at the end of the file.
    ///
    /// Files are always opened in read / write.
    pub fn create_file<P: AsRef<Path>>(&self, path: P) -> Result<File> {
        let full_path = self.root_dir.join(path.as_ref());

        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(full_path.as_path())?;

        Ok(file)
    }

    /// Get the [`Stats`] file of the [`Workdir`].
    pub fn get_stats(&mut self) -> Result<Option<File>> {
        if let Some(stats_f) = &mut self.stats {
            Ok(stats_f.get_file_rd(self.root_dir.as_path())?)
        } else {
            Ok(None)
        }
    }

    /// Create a new directory, relative to the [`Workdir`].
    ///
    /// Errors out if the directory already exists.
    pub fn create_dir<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
        let full_path = self.root_dir.join(path.as_ref());
        fs::create_dir(full_path.as_path())?;

        Ok(full_path)
    }

    /// Update the [`Stats`] of the [`Workdir`].
    pub fn report_stats(&mut self, stats: &Stats) -> Result<()> {
        if let Some(stats_f) = &mut self.stats {
            let stats_ref = stats_f.get_file_wr(self.root_dir.as_path())?;
            sync_stats(stats_ref, stats)?;
        }

        Ok(())
    }
}
