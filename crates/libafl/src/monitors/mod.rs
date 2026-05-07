use alloc::{string::String, vec::Vec};
use core::{fmt, time::Duration};
use std::{
    fs,
    path::{Path, PathBuf},
    thread::current,
};

use libafl_bolts::{Error, current_time};
use nix::sys::ptrace::interrupt;
use serde::{Deserialize, Serialize};

use crate::{
    Controller, Worker,
    fuzzers::HasObjective,
    runtimes::RuntimeHandle,
    states::{FlatState, read_stats_json},
};

pub mod simple;
pub use simple::SimpleMonitor;

#[cfg(feature = "web_monitor")]
pub mod web;
#[cfg(feature = "web_monitor")]
pub use web::WebMonitor;

pub trait Monitor {
    /// Display tick.
    ///
    /// This will be called regularly by the launcher to let the Monitor update its state
    /// and display updated information
    fn display<CT: Controller>(&mut self, controller: &mut CT) -> Result<(), Error>;
}
