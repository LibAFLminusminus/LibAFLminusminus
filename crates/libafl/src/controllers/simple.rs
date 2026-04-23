use crate::{Controller, GlobalController, Result, StdDescriptor};
use libafl_core::ClientId;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, vec::Vec};

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

    fn create_controller(&mut self) -> Result<SimpleController> {
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
