use crate::{
    Controller, GlobalController,
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
    /// display (or not because you only want to display into the terminal if you are a main instance)
    fn display<GCT: GlobalController>(&mut self, global_controller: &mut GCT) -> Result<(), Error>;
}
