use crate::{
    Result,
    launchers::InstanceId,
    state::{Stats, sync_stats},
};
use alloc::vec::Vec;
use hashbrown::HashMap;
use libafl_bolts::core_affinity::CoreId;
use libafl_core::{Error, WorkerId, internal_bug};
use nix::sys::signal::Signal;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

pub mod aflpp;
pub mod nop;
pub mod simple;

pub trait Controller {
    type Worker: Worker;
    type Descriptor: Clone;

    fn create_worker(&mut self) -> Result<Self::Worker>;

    fn workers(&self) -> impl IntoIterator<Item = &Self::Descriptor>;

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
    pub fn get_file<P: AsRef<Path>>(&mut self, root_dir: P) -> Result<&mut File> {
        let path: Option<PathBuf> = if let WorkdirFile::Path(p) = self {
            Some(p.clone())
        } else {
            None
        };

        if let Some(p) = path {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
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

        Ok(file)
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

    pub fn stdout(&mut self) -> Result<Option<&mut File>> {
        if let Some(wd_f) = &mut self.stdout {
            wd_f.get_file(self.root_dir.as_path())
                .map(|file| Some(file))
        } else {
            Ok(None)
        }
    }

    pub fn stderr(&mut self) -> Result<Option<&mut File>> {
        if let Some(wd_f) = &mut self.stderr {
            wd_f.get_file(self.root_dir.as_path())
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
            let stats_ref = stats_f.get_file(self.root_dir.as_path())?;
            sync_stats(stats_ref, stats)?;
        }

        Ok(())
    }
}
