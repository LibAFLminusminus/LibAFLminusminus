use crate::{Result, launchers::InstanceId};
use alloc::vec::Vec;
use hashbrown::HashMap;
use libafl_bolts::core_affinity::CoreId;
use libafl_core::{Error, WorkerId};
use nix::sys::signal::Signal;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub mod aflpp;
pub mod nop;
pub mod simple;

pub trait Controller {
    type Worker: Worker;
    type Descriptor: Clone;

    fn create_controller(&mut self) -> Result<Self::Worker>;

    fn controllers(&self) -> &[Self::Worker];

    fn on_start(&mut self, descriptor: &Self::Descriptor, id: InstanceId) -> Result<()> {
        Ok(())
    }

    /// Called when a controller exits with some exit code
    fn on_exit(&mut self, descriptor: &Self::Descriptor, exit_code: i32) -> Result<()> {
        Ok(())
    }

    /// Called when a controller exits with a termination (e.g. signal / exception)
    fn on_termination(
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
    fn workdir(&self) -> &PathBuf;

    /// do the work related to reconciling between instances; like sharing corpus.. etc.
    fn reconcile(&self) -> Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StdDescriptor {
    /// path to the workdir of this process
    path: PathBuf,
    /// client id of this process
    id: WorkerId,
}

/// The launcher should create instantiate this alongside binding this instance to a specific core id
impl StdDescriptor {
    /// Default constructor
    pub fn new<P: AsRef<Path>>(path: P, id: WorkerId) -> Result<Self> {
        if !path.as_ref().is_dir() {
            return Err(Error::illegal_argument(
                "The client directory does not exit. This is a fuzzer bug.",
            ));
        }

        Ok(StdDescriptor {
            path: path.as_ref().to_path_buf(),
            id,
        })
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }
}
