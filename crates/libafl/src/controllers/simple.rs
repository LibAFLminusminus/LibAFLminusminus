use crate::{Controller, GlobalController, Result, StdDescriptor};
use libafl_core::{ClientId, Error, illegal_argument, internal_bug};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, vec::Vec};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleGlobalController {
    root_dir: PathBuf,
    client_ctr: u32,
    clients: Vec<SimpleController>,
}

/// this is just a wrapper around StdDescriptor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleController {
    /// the descriptor describing this client
    descriptor: StdDescriptor,
}

impl SimpleGlobalController {
    /// Create a new [`SimpleGlobalController`] and will use `root_dir` as the root directory.
    ///
    /// The directory must not exist before calling this function.
    pub fn with_workdir(root_dir: PathBuf) -> Result<Self> {
        if root_dir.exists() {
            return Err(illegal_argument!(
                "Wordir already exists: {}",
                root_dir.display()
            ));
        }

        fs::create_dir(root_dir.as_path())?;

        Ok(Self {
            root_dir,
            clients: Vec::new(),
            client_ctr: 0,
        })
    }

    /// Create a new [`SimpleGlobalController`] with the default LibAFLmm root directory, "./workdir".
    ///
    /// The directory must not exist before calling this function.
    pub fn new() -> Result<Self> {
        Self::with_workdir(PathBuf::from("./workdir"))
    }
}

impl GlobalController for SimpleGlobalController {
    type Controller = SimpleController;
    type Descriptor = StdDescriptor;

    fn create_controller(&mut self) -> Result<SimpleController> {
        let client_id = ClientId(self.client_ctr);
        self.client_ctr += 1;

        let client_dir = self.root_dir.join(format!("client_{}", client_id.0));

        if client_dir.exists() {
            return Err(internal_bug!(
                "The client dir \"{}\" already exists.",
                client_dir.display()
            ));
        }

        fs::create_dir(client_dir.as_path())?;

        let descriptor = StdDescriptor::new(client_dir, client_id)?;

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

    fn reconcile(&self) -> Result<()> {
        // do nothing
        Ok(())
    }
}

impl SimpleController {
    pub fn new(descriptor: StdDescriptor) -> Self {
        Self { descriptor }
    }
}
