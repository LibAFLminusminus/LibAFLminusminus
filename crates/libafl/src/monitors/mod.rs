use crate::{
    Controller, Worker,
    fuzzer::HasObjective,
    runtimes::RuntimeHandle,
    state::{FlatState, read_stats_json},
};
use alloc::{string::String, vec::Vec};
use core::{fmt, time::Duration};
use libafl_bolts::{Error, current_time};
use nix::sys::ptrace::interrupt;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    thread::current,
};

pub mod simple;
pub use simple::SimpleMonitor;

pub trait Monitor {
    /// Display tick.
    ///
    /// This will be called regularly by the launcher to let the Monitor update its state
    /// and display updated information
    fn display<CT: Controller>(&mut self, controller: &mut CT) -> Result<(), Error>;
}
