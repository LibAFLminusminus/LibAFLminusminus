//! Module defining [`Controller`]s.

use crate::{
    Result,
    corpus::Testcase,
    launchers::{InstanceId, groups::Group},
    states::{Stats, read_stats_json, stats_to_json},
    sync::GroupId,
};
use alloc::sync::Arc;
use core::time::Duration;
use libaflmm_bolts::CoreId;
use libaflmm_core::Error;
use nix::sys::signal::Signal;
use quanta::{Clock, Instant};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Write, stderr, stdout},
    os::fd::{AsFd, BorrowedFd},
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
    fn register_group<G>(&mut self, config: Self::GroupConfig, group: &mut G) -> Result<GroupId>
    where
        G: Group<Self::Worker>;

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
    /// Name of the Worker.
    fn name(&self) -> impl AsRef<str>;

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

/// The worker ID for various use cases across `LibAFL`
#[repr(transparent)]
#[derive(
    Debug, Default, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct WorkerId(pub u32);

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
#[derive(Debug, Clone)]
pub enum WorkdirFile {
    /// File described as a [`PathBuf`].
    /// If a relative path is used, it's relative to the worker's [`Workdir`].
    Path(PathBuf),
    /// File described as an already opened [`File`].
    File(Arc<File>),
    /// Stdout
    Stdout,
    /// Stderr
    Stderr,
    /// /dev/null
    Null,
}

impl WorkerId {
    #[must_use]
    pub fn id(&self) -> u32 {
        self.0
    }
}

impl WorkdirFile {
    /// Open a truncated [`File`] in write-only mode
    ///
    /// Creates the file if the path does not exist.
    pub fn open_write(&self, root_dir: impl AsRef<Path>) -> Result<File> {
        Ok(match self {
            WorkdirFile::Path(path) => OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(root_dir.as_ref().join(path))?,
            WorkdirFile::File(file) => file.try_clone()?,
            WorkdirFile::Stdout => File::from(stdout().as_fd().try_clone_to_owned()?),
            WorkdirFile::Stderr => File::from(stderr().as_fd().try_clone_to_owned()?),
            WorkdirFile::Null => OpenOptions::new().write(true).open("/dev/null")?,
        })
    }

    /// Open a [`File`] in read-only mode.
    ///
    /// Returns [`None`] if there is nothing to read from.
    pub fn open_read(&self, root_dir: impl AsRef<Path>) -> Result<Option<File>> {
        Ok(match self {
            WorkdirFile::Path(path) => {
                let full_path = root_dir.as_ref().join(path);
                if !full_path.exists() {
                    return Ok(None);
                }

                Some(OpenOptions::new().read(true).open(full_path)?)
            }
            WorkdirFile::File(file) => Some(file.try_clone()?),
            WorkdirFile::Stdout | WorkdirFile::Stderr | WorkdirFile::Null => None,
        })
    }

    /// Replace the whole content of the file.
    ///
    /// If a path is used, the replacement is atomic.
    /// Otherise, the file is simply truncated.
    pub fn replace(&self, root_dir: impl AsRef<Path>, buf: &[u8]) -> Result<()> {
        if let WorkdirFile::Path(path) = self {
            let full_path = root_dir.as_ref().join(path);
            let tmp_path = full_path.with_extension("tmp");

            let mut tmp = File::create(&tmp_path)?;
            tmp.write_all(buf)?;
            tmp.sync_all()?;
            fs::rename(tmp_path, full_path)?;

            Ok(())
        } else {
            self.open_write(root_dir)?.write_all(buf)?;

            Ok(())
        }
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
    pub fn stdout(&self) -> Result<File> {
        self.stdout.open_write(self.root_dir.as_path())
    }

    /// Get the file associated with stderr for the [`Workdir`].
    pub fn stderr(&self) -> Result<File> {
        self.stderr.open_write(self.root_dir.as_path())
    }

    /// Read the [`Stats`] of the [`Workdir`].
    ///
    /// Returns `None` if no stats have been reported yet.
    pub fn read_stats(&self) -> Result<Option<Stats>> {
        self.stats
            .open_read(self.root_dir.as_path())?
            .map(read_stats_json)
            .transpose()
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

    /// Update the [`Stats`] of the [`Workdir`] atomically.
    pub fn report_stats(&self, stats: &Stats) -> Result<()> {
        self.stats
            .replace(self.root_dir.as_path(), &stats_to_json(stats)?)
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
