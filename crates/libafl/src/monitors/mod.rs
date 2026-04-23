use alloc::{vec::Vec, string::String};
use core::{time::Duration, fmt};
use std::{
    fs,
    path::{Path, PathBuf},
    thread::current,
};

use libafl_bolts::{Error, current_time};
use nix::sys::ptrace::interrupt;
use serde::{Deserialize, Serialize};

use crate::{Controller, fuzzer::HasObjective, runtimes::RuntimeHandle, state::FlatState};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Stats {
    /// How many times the executor ran the harness/target
    pub(crate) executions: u64,
    /// At what time the fuzzing started
    pub(crate) start_time: Duration,
    /// number of corpus
    pub(crate) corpus: usize,
    /// number of objective
    pub(crate) objective: usize,
    /// last time smth was found
    pub(crate) last_found_time: Duration,
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] execs: {} ({}/s) | corpus: {} | objectives: {}",
            format_duration(self.start_time),
            self.executions,
            self.execs_per_sec(),
            self.corpus,
            self.objective,
        )
    }
}

/// put duration in a human readable format
fn format_duration(d: Duration) -> String {
    let total = d.as_secs();
    let days = total / 86_400;
    let hours = (total % 86_400) / 3_600;
    let mins = (total % 3_600) / 60;
    let secs = total % 60;

    if days > 0 {
        format!("{days}d{hours:02}:{mins:02}:{secs:02}")
    } else {
        format!("{hours:02}:{mins:02}:{secs:02}")
    }
}

impl Stats {
    fn update_corpus(&mut self, corpus: usize) {
        self.corpus = corpus;
    }

    fn update_objective(&mut self, objective: usize) {
        self.objective = objective;
    }

    fn execs_per_sec(&self) -> u64 {
        let secs = self.start_time.as_secs();
        if secs == 0 { 0 } else { self.executions / secs }
    }

    /// write to json stat file
    fn write_json<P: AsRef<Path>>(&self, path: &P) -> Result<(), Error> {
        let file = fs::File::create(path)?;
        serde_json::to_writer_pretty(file, self)
            .map_err(|_| Error::runtime("Failed to dump the stats to a file"));
        Ok(())
    }
    /// read from json stat file
    fn read_json<P: AsRef<Path>>(&self, path: &P) -> Result<Self, Error> {
        let file = fs::File::open(path)?;
        serde_json::from_reader(file)
            .map_err(|_| Error::runtime("Failed to read the stats from a file"))
    }
}

pub trait Monitor<S> {
    /// sync the stats data to the file system
    fn sync(&self, state: &S) -> Result<(), Error>;

    /// display (or not because you only want to display into the terminal if you are a main instance)
    fn display(&self, state: &S) -> Result<(), Error>;
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

    fn display(&self, state: &S) -> Result<(), Error>  {
        match &self.role {
            MonitorRole::Secondary => {
                Ok(())
            },
            MonitorRole::Main { read_paths, display } => {
                for path in read_paths {
                    let stat = state.stats().read_json(&path)?;
                    println!("{:?}", stat);
                }
                Ok(())
            }
        }
    }
}
