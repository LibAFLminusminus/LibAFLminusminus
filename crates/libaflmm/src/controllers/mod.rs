//! Module defining [`Controller`]s.

use core::time::Duration;
use std::{
    fs::{self, File, OpenOptions},
    io::{stderr, stdout},
    os::fd::{AsRawFd, FromRawFd},
    path::{Path, PathBuf},
};

use libaflmm_core::{Error, WorkerId, internal_bug};
use nix::sys::signal::Signal;
use quanta::{Clock, Instant};

use crate::{
    Result,
    launchers::InstanceId,
    states::{Stats, sync_stats},
};

/// Default wait time between stats updates.
const STATS_UPDATE_INTERVAL: Duration = Duration::from_secs(5);

// pub mod aflpp;
pub mod nop;
pub use nop::{NopController, NopDescriptor, NopWorker};

pub mod simple;
pub use simple::{
    SimpleController, SimpleControllerBuilder, SimpleDescriptor, SimpleWorker, SimpleWorkerRepr,
};

pub type StdController = SimpleController;
pub type StdDescriptor = SimpleDescriptor;
pub type StdWorker = SimpleWorker;

/// A controller is the glue between multiple [`Worker`]s.
///
/// It is reponsible for creating and configurating workers.
/// Note [`Self`] and [`Worker`]s are tightly linked: they are supposed to be defined together and are interdependent.
pub trait Controller {
    /// The associated [`Worker`].
    type Worker: Worker;
    /// The associated [`Descriptor`].
    type Descriptor: Descriptor;
    /// The commands for the [`Worker`]s.
    type Command;

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

    /// Send a command to a given [`Worker`].
    fn send_command(&mut self, _command: Self::Command, _worker_id: WorkerId) -> Result<()>;

    /// Send a command to every [`Worker`].
    fn send_command_all(&mut self, _command: Self::Command) -> Result<()>;

    /// Send a command to every [`Worker`] except the given one.
    fn send_command_all_but(&mut self, _command: Self::Command, _worker_id: WorkerId)
    -> Result<()>;

    /// Wait for events sent by the [`Worker`]s.
    /// The function returns after a notification is received or the given timeout value has elapsed.
    fn wait_notifications(&mut self, _timeout: Option<Duration>) -> Result<()>;
}

/// A worker is a representant of a fuzzing instance.
/// It is linked to a [`Controller`], which holds a reference to all workers.
pub trait Worker {
    /// The associated [`Controller`].
    type Controller: Controller<Worker = Self>;
    /// Notifications for the [`Controller`]
    type Notification;

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

    /// Send a notification to the [`Controller`]
    fn send_notification(&mut self, _notification: Self::Notification) -> Result<()>;

    /// Polls the list of commands received since the last call.
    fn poll_commands(
        &mut self,
    ) -> Result<impl Iterator<Item = <Self::Controller as Controller>::Command>>;
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
    clock: Clock,
    last_stats_sync: Instant,
}

/// A workdir file is an abstract representation of a file owned by a [`Workir`].
/// It enables to get a file as a [`File`] or a [`PathBuf`] transparently.
#[derive(Debug)]
pub enum WorkdirFile {
    /// File described as a [`PathBuf`].
    Path(PathBuf),
    /// File described as a [`File`].
    File(File),
    /// Stdout
    Stdout,
    /// Stderr
    Stderr,
    /// /dev/null
    Null,
}

impl Clone for WorkdirFile {
    fn clone(&self) -> Self {
        match self {
            WorkdirFile::Path(p) => WorkdirFile::Path(p.clone()),
            WorkdirFile::File(f) => WorkdirFile::File(f.try_clone().unwrap()),
            WorkdirFile::Stdout => WorkdirFile::Stdout,
            WorkdirFile::Stderr => WorkdirFile::Stderr,
            WorkdirFile::Null => WorkdirFile::Null,
        }
    }
}

impl WorkdirFile {
    fn setup_fd(&mut self, root_dir: impl AsRef<Path>, is_write: bool) -> Result<()> {
        match self {
            WorkdirFile::File(_) => {}
            WorkdirFile::Path(path) => {
                let full_path = root_dir.as_ref().join(path.as_path());

                let file = if is_write {
                    OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(full_path)?
                } else {
                    if !full_path.exists() {
                        return Ok(());
                    }

                    OpenOptions::new().read(true).open(full_path)?
                };

                *self = WorkdirFile::File(file);
            }
            WorkdirFile::Stdout => {
                *self = WorkdirFile::File(unsafe { File::from_raw_fd(stdout().as_raw_fd()) });
            }
            WorkdirFile::Stderr => {
                *self = WorkdirFile::File(unsafe { File::from_raw_fd(stderr().as_raw_fd()) });
            }
            WorkdirFile::Null => {
                *self =
                    WorkdirFile::File(File::open("/dev/null").expect("Could not open /dev/null"));
            }
        }

        Ok(())
    }

    /// Open a [`File`] in read-only mode from its path.
    ///
    /// Returns [`None`] if the path does not exist.
    pub fn get_file_rd(&mut self, root_dir: impl AsRef<Path>) -> Result<Option<File>> {
        self.setup_fd(root_dir, false)?;

        let file: &mut File = match self {
            WorkdirFile::File(file) => file,
            _ => return Ok(None),
        };

        Ok(Some(file.try_clone().unwrap()))
    }

    /// Open a [`File`] in write-only mode from its path.
    ///
    /// Creates the file if the path does not exist.
    pub fn get_file_wr(&mut self, root_dir: impl AsRef<Path>) -> Result<File> {
        self.setup_fd(root_dir, true)?;

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

        let clock = Clock::new();
        let last_stats_sync = clock.now();

        Ok(Self {
            root_dir: root_dir.as_ref().to_path_buf(),
            stdout,
            stderr,
            stats,
            clock,
            last_stats_sync,
        })
    }

    /// Get the root directory of the [`Workdir`].
    #[must_use]
    pub fn root_dir(&self) -> &Path {
        self.root_dir.as_path()
    }

    /// Get the file associated with stdout for the [`Workdir`].
    pub fn stdout(&mut self) -> Result<Option<File>> {
        if let Some(wd_f) = &mut self.stdout {
            wd_f.get_file_wr(self.root_dir.as_path()).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Get the file associated with stderr for the [`Workdir`].
    pub fn stderr(&mut self) -> Result<Option<File>> {
        if let Some(wd_f) = &mut self.stderr {
            wd_f.get_file_wr(self.root_dir.as_path()).map(Some)
        } else {
            Ok(None)
        }
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

    /// report stats every once in a while.
    #[inline]
    pub fn maybe_report_stats(&mut self, stats: &Stats) -> Result<()> {
        let now = self.clock.now();
        if now.duration_since(self.last_stats_sync) > STATS_UPDATE_INTERVAL {
            self.last_stats_sync = now;
            self.report_stats(stats)
        } else {
            Ok(())
        }
    }

    /// The default objective directory
    pub fn objective_dir(&self) -> Result<PathBuf> {
        self.create_dir("crashes")
    }

    /// The default corpus directory
    pub fn corpus_dir(&self) -> Result<PathBuf> {
        self.create_dir("corpus")
    }
}
