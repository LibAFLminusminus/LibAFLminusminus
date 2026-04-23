use alloc::vec::Vec;
use core::time::Duration;
use std::{
    fs,
    path::{Path, PathBuf},
    thread::current,
};

use libafl_bolts::{Error, current_time};
use nix::sys::ptrace::interrupt;
use serde::{Deserialize, Serialize};

use crate::{Controller, runtimes::RuntimeHandle, state::FlatState};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Stats {
    /// How many times the executor ran the harness/target
    pub(crate) executions: u64,
    /// At what time the fuzzing started
    pub(crate) start_time: Duration,
    /// the number of new paths that imported from other fuzzers
    pub(crate) imported: usize,
    /// The last time we reported progress (if available/used).
    /// This information is used by fuzzer `maybe_report_progress`.
    pub(crate) last_report_time: Option<Duration>,
    /// The last time something was added to the corpus
    pub(crate) last_found_time: Duration,
}

impl Stats {
    /// write to json stat file
    fn write_json<P: AsRef<Path>>(&self, path: &P) -> Result<(), Error> {
        let file = fs::File::create(path)?;
        serde_json::to_writer_pretty(file, self)
            .map_err(|_| Error::runtime("Failed to dump the stats to a file"));
        Ok(())
    }
    /// read from json stat file
    fn read_json<P: AsRef<Path>>(path: &P) -> Result<Self, Error> {
        let file = fs::File::open(path)?;
        serde_json::from_reader(file)
            .map_err(|_| Error::runtime("Failed to read the stats from a file"))
    }
}

pub trait Monitor<S> {
    /// sync the stats data to the file system
    fn sync(&self, state: &S) -> Result<(), Error>;

    /// display (or not because you only want to display into the terminal if you are a main instance)
    fn display(&self);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleMonitor {
    /// the path that this instance writes its stats to
    write_path: PathBuf,
    /// the last time monitor was updated,
    last_update: Duration,
    /// the intervals to update monitor
    intervals: Duration,
    /// the role of this monitor
    role: MonitorRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MonitorRole {
    Main {
        read_paths: Vec<PathBuf>,
        display: bool,
    },
    Secondary,
}

impl SimpleMonitor {
    fn main<CT: Controller>(controller: CT, intervals: Duration, display: bool) -> Self {
        let write_path = controller.workdir().clone().join("fuzzer_stats");
        let read_paths = controller.child_workdirs().unwrap_or_default().to_vec();
        Self {
            write_path,
            last_update: current_time(),
            intervals,
            role: MonitorRole::Main {
                read_paths,
                display,
            },
        }
    }

    fn secondary<CT: Controller>(controller: CT, intervals: Duration) -> Self {
        let write_path = controller.workdir().clone().join("fuzzer_stats");
        Self {
            write_path,
            last_update: current_time(),
            intervals,
            role: MonitorRole::Secondary,
        }
    }
}

impl<S> Monitor<S> for SimpleMonitor
where
    S: FlatState,
{
    fn sync(&self, state: &S) -> Result<(), Error> {
        if current_time() - self.last_update < self.intervals {
            return Ok(());
        }
        state.stats().write_json(&self.write_path);
        Ok(())
    }

    fn display(&self) {
        match &self.role {
            MonitorRole::Secondary => (),
            MonitorRole::Main { read_paths, display } => {
                // gather all the stats. then show it.
            }
        }

        println!("LOL!");
    }
}
