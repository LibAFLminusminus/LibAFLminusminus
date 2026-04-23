use alloc::vec::Vec;
use hashbrown::HashMap;
use libafl_bolts::core_affinity::CoreId;
use libafl_core::{ClientId, Error};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub mod aflpp;
pub mod nop;

pub trait GlobalController {
    type Controller: Controller;

    fn create_controller(
        &mut self,
        descriptor: <Self::Controller as Controller>::Descriptor,
    ) -> Result<Self::Controller, Error>;

    fn controllers(&self) -> &[Self::Controller];
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleGlobalController {
    clients: Vec<SimpleController>,
}

impl SimpleGlobalController {
    pub fn new() -> Self {
        Self {
            clients: Vec::new(),
        }
    }
}

impl GlobalController for SimpleGlobalController {
    type Controller = SimpleController;

    fn create_controller(&mut self, descriptor: StdDescriptor) -> Result<SimpleController, Error> {
        let cl = SimpleController::new(descriptor);
        self.clients.push(cl.clone());
        Ok(cl)
    }

    fn controllers(&self) -> &[Self::Controller] {
        &self.clients
    }
}

pub trait Controller {
    type Descriptor;

    /// the client id
    fn id(&self) -> ClientId;

    /// returns the descriptor describing each fuzzer instances
    fn descriptor(&self) -> &Self::Descriptor;

    /// returns the working directory of this instance
    fn workdir(&self) -> &PathBuf;

    /// do the work related to reconciling between instances; like sharing corpus.. etc.
    fn reconcile(&self) -> Result<(), Error>;
}

/// this is just a wrapper around stddescriptor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleController {
    /// the descriptor describing this client
    descriptor: StdDescriptor,
}

impl Controller for SimpleController {
    type Descriptor = StdDescriptor;

    fn id(&self) -> ClientId {
        self.descriptor.client_id
    }

    fn descriptor(&self) -> &Self::Descriptor {
        &self.descriptor
    }

    fn workdir(&self) -> &PathBuf {
        &self.descriptor.path
    }

    fn reconcile(&self) -> Result<(), Error> {
        // do nothing
        Ok(())
    }
}

impl SimpleController {
    pub fn new(descriptor: StdDescriptor) -> Self {
        Self { descriptor }
    }
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
    pub fn new<P: AsRef<Path>>(path: P, client_id: u32) -> Result<Self, Error> {
        if !path.as_ref().is_dir() {
            return Err(Error::illegal_argument("main is not a valid directory"));
        }
        Ok(StdDescriptor {
            path: path.as_ref().to_path_buf(),
            client_id: ClientId(client_id),
        })
    }
}
