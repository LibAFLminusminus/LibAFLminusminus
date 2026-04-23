use crate::Result;
use alloc::vec::Vec;
use hashbrown::HashMap;
use libafl_bolts::core_affinity::CoreId;
use libafl_core::{ClientId, Error};
use nix::sys::signal::Signal;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub mod aflpp;
pub mod nop;
pub mod simple;

pub trait GlobalController {
    type Controller: Controller;
    type Descriptor: Clone;

    fn create_controller(&mut self) -> Result<Self::Controller>;

    fn controllers(&self) -> &[Self::Controller];

    fn on_start(&mut self, descriptor: &Self::Descriptor) -> Result<()> {
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

pub trait Controller {
    type GlobalController: GlobalController<Controller = Self>;

    /// the client id
    fn id(&self) -> ClientId;

    /// returns the descriptor describing each fuzzer instances
    fn descriptor(&self) -> &<Self::GlobalController as GlobalController>::Descriptor;

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
    client_id: ClientId,
}

/// The launcher should create instantiate this alongside binding this instance to a specific core id
impl StdDescriptor {
    /// Default constructor
    pub fn new<P: AsRef<Path>>(path: P, client_id: ClientId) -> Result<Self> {
        if !path.as_ref().is_dir() {
            return Err(Error::illegal_argument(
                "The client directory does not exit. This is a fuzzer bug.",
            ));
        }

        Ok(StdDescriptor {
            path: path.as_ref().to_path_buf(),
            client_id,
        })
    }
}
