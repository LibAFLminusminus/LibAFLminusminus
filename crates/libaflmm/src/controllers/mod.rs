//! Module defining [`Controller`]s.

use crate::{
    Result,
    corpus::Testcase,
    launchers::InstanceId,
    states::{Stats, sync_stats},
    sync::GroupId,
};
use core::time::Duration;
use libaflmm_bolts::{CoreId, Cores};
use libaflmm_core::{Error, WorkerId, internal_bug};
use nix::sys::signal::Signal;
use quanta::{Clock, Instant};
use std::{
    fs::{self, File, OpenOptions},
    io::{stderr, stdout},
    os::fd::{AsRawFd, BorrowedFd, FromRawFd},
    path::{Path, PathBuf},
};

/// Default wait time between stats updates.
const STATS_UPDATE_INTERVAL: Duration = Duration::from_secs(5);

pub mod standard;
pub use standard::{StdController, StdControllerBuilder, StdDescriptor, StdWorker, StdWorkerRepr};

pub mod nop;
pub use nop::{NopController, NopDescriptor, NopWorker};

/// A controller is the glue between multiple [`Worker`]s.
///
/// It is reponsible for creating and configurating workers.
/// Note [`Self`] and [`Worker`]s are tightly linked: they are supposed to be defined together and are interdependent.
pub trait Controller {
    /// The associated [`Worker`].
    type Worker: Worker;
    /// Describes how a group should be configured
    type GroupConfig;

    /// Register groups, giving back the descriptors of the workers created from the group.
    fn register_group(&mut self, config: Self::GroupConfig, cores: &Cores) -> Result<GroupId>;

    /// Called after every group have been registered.
    /// It will resolve the final group configuration and create the workers as a result, for each group ID.
    fn finalize_orchestration(&mut self) -> Result<()>;

    /// Take the workers for a given group ID.
    /// This will only work once `finalize_orchestration` has been called.
    fn take_group_workers(&mut self, group: GroupId) -> Result<impl Iterator<Item = Self::Worker>>;

    /// Get an iterator over all [`Self::Worker`] descriptors.
    fn worker_descriptors(&self)
    -> impl IntoIterator<Item = &<Self::Worker as Worker>::Descriptor>;

    /// Get a mutable iterator over all [`Self::Worker`] descriptors.
    fn worker_descriptors_mut(
        &mut self,
    ) -> impl IntoIterator<Item = &mut <Self::Worker as Worker>::Descriptor>;

    /// The root directory of the controller
    fn root_dir(&self) -> &Path;

    /// Hook called when a [`Self::Worker`] actually starts, with its associated [`InstanceId`].
    fn on_worker_start(
        &mut self,
        _descriptor: &<Self::Worker as Worker>::Descriptor,
        _id: InstanceId,
    ) -> Result<()> {
        Ok(())
    }

    /// Hook called when a controller exits with some exit code
    fn on_worker_exit(
        &mut self,
        _descriptor: &<Self::Worker as Worker>::Descriptor,
        _exit_code: i32,
    ) -> Result<()> {
        Ok(())
    }

    /// Hook called when a controller exits with a termination (e.g. signal / exception)
    fn on_worker_termination(
        &mut self,
        _descriptor: &<Self::Worker as Worker>::Descriptor,
        _termination_code: Signal, // TODO: make this os-agnostic
    ) -> Result<()> {
        Ok(())
    }

    // /// Send a command to a given [`Worker`].
    // fn send_command(
    //     &mut self,
    //     command: <Self::Worker as Worker>::Command,
    //     _worker_id: WorkerId,
    // ) -> Result<()>;

    /// Wait for events sent by the [`Worker`]s.
    /// The function returns after a notification is received or the given timeout value has elapsed.
    fn wait_notifications(&mut self, wake_fds: &[BorrowedFd<'_>], _timeout: Duration)
    -> Result<()>;

    /// Kindly ask to a worker to shut down.
    /// This is asynchronous, so the worker could still be alive for some time.
    ///
    /// Depending on the orchestrator choice, shutdown may or may not do something.
    fn shutdown(&mut self, worker: WorkerId) -> Result<()>;
}

/// A worker is a representant of a fuzzing instance.
/// It is linked to a [`Controller`], which holds a reference to all workers.
pub trait Worker {
    /// The associated [`Descriptor`].
    type Descriptor: Descriptor;

    /// Returns the reference to the descriptor of the worker.
    fn descriptor(&self) -> &Self::Descriptor;

    /// Returns the mutable reference to the descriptor of the worker.
    fn descriptor_mut(&mut self) -> &mut Self::Descriptor;

    /// Returns the reference of the working directory of the worker.
    fn workdir(&self) -> &Workdir {
        self.descriptor().workdir()
    }

    /// Returns the mutable reference of the working directory of the worker.
    fn workdir_mut(&mut self) -> &mut Workdir {
        self.descriptor_mut().workdir_mut()
    }

    /// Returns the [`WorkerId`] attributed to the worker
    fn id(&self) -> WorkerId {
        self.descriptor().worker_id()
    }

    /// Returns the [`CoreId`] on which the worker is running
    fn core_id(&self) -> Option<CoreId> {
        self.descriptor().core_id()
    }

    /// Hook called before the [`Runtime`](crate::runtimes::Runtime) of the worker gets executed.
    fn pre_runtime_exec(&mut self) -> Result<()> {
        Ok(())
    }

    /// Poll for received commands
    ///
    /// Returns true if something has been received, false otherwise.
    fn poll(&mut self) -> Result<bool>;

    /// Returns true if the worker should shutdown, false otherwise
    ///
    /// It will only take into account requests since the last call to [`Self::poll`].
    /// Any commands received after that would not be considered.
    fn should_shutdown(&mut self) -> bool;
}

/// A [`Worker`] able to share [`Testcase`]s.
pub trait SharingWorker<I>: Worker {
    /// Report a [`Testcase`] that should be shared according to the
    /// [`Router`](crate::sync::Router) policy.
    fn send_testcase(&mut self, testcase: &Testcase<I>) -> Result<()>;

    /// Check for inputs that should be evaluated.
    /// All the pending [`Testcase`]s are returned as an iterator.
    ///
    /// It will only take into account requests since the last call to [`Worker::poll`].
    /// Any commands received after that would not be considered.
    ///
    /// Pending testcases are returned and guaranteed to be removed from the worker buffer.
    fn recv_testcases(&mut self) -> Result<impl Iterator<Item = Testcase<I>>>;
}

/// A descriptor describes a [`Worker`].
pub trait Descriptor: Clone {
    /// Get the reference to the workdir of the [`Worker`].
    fn workdir(&self) -> &Workdir;

    /// Get the mutable reference to the workdir of the [`Worker`].
    fn workdir_mut(&mut self) -> &mut Workdir;

    /// Get the [`WorkerId`] of the [`Worker`].
    fn worker_id(&self) -> WorkerId;

    /// Get the [`CoreId`] of the [`Worker`].
    fn core_id(&self) -> Option<CoreId>;

    /// Get the [`GroupId`] of the [`Worker`].
    fn group_id(&self) -> GroupId;
}

/// A workdir contains information relative to the working environement of a [`Worker`].
#[derive(Debug, Clone)]
pub struct Workdir {
    root_dir: PathBuf,
    stdout: WorkdirFile,
    stderr: WorkdirFile,
    stats: WorkdirFile,
    clock: Clock,
    last_stats_sync: Instant,
}

/// A workdir file is an abstract representation of a file owned by a [`Workdir`].
/// It enables to get a file as a [`File`] or a [`PathBuf`] transparently.
#[derive(Debug)]
pub enum WorkdirFile {
    /// File described as a [`PathBuf`].
    /// If a relative path is used, it's relative to the worker's [`Workdir`].
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
        stdout: WorkdirFile,
        stderr: WorkdirFile,
        stats: WorkdirFile,
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
    pub fn stdout(&mut self) -> Result<File> {
        self.stdout.get_file_wr(self.root_dir.as_path())
    }

    /// Get the file associated with stderr for the [`Workdir`].
    pub fn stderr(&mut self) -> Result<File> {
        self.stderr.get_file_wr(self.root_dir.as_path())
    }

    /// Get the [`Stats`] file of the [`Workdir`].
    pub fn get_stats(&mut self) -> Result<Option<File>> {
        self.stats.get_file_rd(self.root_dir.as_path())
    }

    /// Create a new directory, relative to the [`Workdir`].
    ///
    /// Errors out if the directory already exists.
    pub fn create_dir(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        let full_path = self.root_dir.join(path.as_ref());
        fs::create_dir(&full_path)?;

        Ok(full_path)
    }

    /// Create a new file, relative to the [`Workdir`].
    ///
    /// Errors out if the file already exists.
    pub fn create_file(&self, path: impl AsRef<Path>) -> Result<File> {
        let full_path = self.root_dir.join(path.as_ref());
        Ok(File::create_new(full_path)?)
    }

    /// Open a file in RW mode, relative to the [`Workdir`].
    /// The file cursor is always at the beginning of the file.
    ///
    /// Errors out if the file does not exist.
    pub fn open_file(&self, path: impl AsRef<Path>) -> Result<File> {
        let full_path = self.root_dir.join(path.as_ref());

        Ok(fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(full_path)?)
    }

    pub fn is_file(&self, path: impl AsRef<Path>) -> bool {
        let full_path = self.root_dir.join(path.as_ref());

        full_path.is_file()
    }

    /// Update the [`Stats`] of the [`Workdir`].
    pub fn report_stats(&mut self, stats: &Stats) -> Result<()> {
        let stats_ref = self.stats.get_file_wr(self.root_dir.as_path())?;
        sync_stats(stats_ref, stats)?;

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
