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

pub mod aflpp;
pub mod nop;
pub mod simple;

/// A controller is the glue between multiple [`Worker`]s.
///
/// It is reponsible for creating and configurating workers.
/// Note [`Self`] and [`Worker`]s are tightly linked: they are supposed to be defined together and are interdependent.
pub trait Controller {
    /// The associated [`Worker`].
    type Worker: Worker;
    /// The associated [`Descriptor`].
    type Descriptor: Descriptor;

    fn create_worker(&mut self) -> Result<Self::Worker>;

    fn worker_descriptors(&self) -> impl IntoIterator<Item = &Self::Descriptor>;

    fn worker_descriptors_mut(&mut self) -> impl IntoIterator<Item = &mut Self::Descriptor>;

    fn on_worker_start(&mut self, descriptor: &Self::Descriptor, id: InstanceId) -> Result<()> {
        Ok(())
    }

    /// Called when a controller exits with some exit code
    fn on_worker_exit(&mut self, descriptor: &Self::Descriptor, exit_code: i32) -> Result<()> {
        Ok(())
    }

    /// Called when a controller exits with a termination (e.g. signal / exception)
    fn on_worker_termination(
        &mut self,
        descriptor: &Self::Descriptor,
        termination_code: Signal, // TODO: make this os-agnostic
    ) -> Result<()> {
        Ok(())
    }
}

pub trait Worker {
    type Controller: Controller<Worker = Self>;

    /// the client id
    fn id(&self) -> WorkerId;

    /// returns the descriptor describing each fuzzer instances
    fn descriptor(&self) -> &<Self::Controller as Controller>::Descriptor;

    /// returns the working directory of this instance
    fn workdir(&self) -> &Workdir;

    /// returns the working directory of this instance
    fn workdir_mut(&mut self) -> &mut Workdir;

    /// do the work related to reconciling between instances; like sharing corpus.. etc.
    fn reconcile(&self) -> Result<()>;

    fn pre_runtime_exec(&mut self) -> Result<()> {
        Ok(())
    }
}

pub trait Descriptor: Clone {
    fn workdir(&self) -> &Workdir;
    fn workdir_mut(&mut self) -> &mut Workdir;
    fn worker_id(&self) -> WorkerId;
}

#[derive(Debug, Clone)]
pub struct Workdir {
    root_dir: PathBuf,
    stdout: Option<WorkdirFile>,
    stderr: Option<WorkdirFile>,
    stats: Option<WorkdirFile>,
}

#[derive(Debug)]
pub enum WorkdirFile {
    Path(PathBuf),
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
    pub fn get_file_rd<P: AsRef<Path>>(&mut self, root_dir: P) -> Result<Option<File>> {
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

    pub fn get_file_wr<P: AsRef<Path>>(&mut self, root_dir: P) -> Result<File> {
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
    pub fn new<P: AsRef<Path>>(
        root_dir: P,
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

    pub fn root_dir(&self) -> &Path {
        self.root_dir.as_path()
    }

    pub fn stdout(&mut self) -> Result<Option<File>> {
        if let Some(wd_f) = &mut self.stdout {
            wd_f.get_file_wr(self.root_dir.as_path())
                .map(|file| Some(file))
        } else {
            Ok(None)
        }
    }

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

    pub fn get_stats(&mut self) -> Result<Option<File>> {
        if let Some(stats_f) = &mut self.stats {
            Ok(stats_f.get_file_rd(self.root_dir.as_path())?)
        } else {
            Ok(None)
        }
    }

    /// create a new directory, relative to the workdir.
    ///
    /// Errors out if the directory already exists.
    pub fn create_dir<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
        let full_path = self.root_dir.join(path.as_ref());
        fs::create_dir(full_path.as_path())?;

        Ok(full_path)
    }

    pub fn report_stats(&mut self, stats: &Stats) -> Result<()> {
        if let Some(stats_f) = &mut self.stats {
            let stats_ref = stats_f.get_file_wr(self.root_dir.as_path())?;
            sync_stats(stats_ref, stats)?;
        }

        Ok(())
    }
}
