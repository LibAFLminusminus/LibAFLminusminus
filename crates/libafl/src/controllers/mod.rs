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
    type Descriptor;

    fn create_controller(&mut self) -> Result<Self::Controller, Error>;

    fn controllers(&self) -> &[Self::Controller];
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
    fn reconcile(&self) -> Result<(), Error>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleGlobalController {
    root_dir: PathBuf,
    client_ctr: u32,
    clients: Vec<SimpleController>,
}

/// this is just a wrapper around stddescriptor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleController {
    /// the descriptor describing this client
    descriptor: StdDescriptor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StdDescriptor {
    /// path to the workdir of this process
    path: PathBuf,
    /// client id of this process
    client_id: ClientId,
}

impl SimpleGlobalController {
    pub fn new(root_dir: PathBuf) -> Self {
        Self {
            root_dir,
            clients: Vec::new(),
            client_ctr: 0,
        }
    }
}

impl GlobalController for SimpleGlobalController {
    type Controller = SimpleController;
    type Descriptor = StdDescriptor;

    fn create_controller(&mut self) -> Result<SimpleController, Error> {
        let client_id = ClientId(self.client_ctr);
        self.client_ctr += 1;

        let descriptor = StdDescriptor::new(
            self.root_dir.join(format!("client_{}", client_id.0)),
            client_id,
        )?;

        let cl = SimpleController::new(descriptor);
        self.clients.push(cl.clone());
        Ok(cl)
    }

    fn controllers(&self) -> &[Self::Controller] {
        &self.clients
    }
}

impl Controller for SimpleController {
    type GlobalController = SimpleGlobalController;

    fn id(&self) -> ClientId {
        self.descriptor.client_id
    }

    fn descriptor(&self) -> &StdDescriptor {
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

/// The launcher should create instantiate this alongside binding this instance to a specific core id
impl StdDescriptor {
    /// Default constructor
    pub fn new<P: AsRef<Path>>(path: P, client_id: ClientId) -> Result<Self, Error> {
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
