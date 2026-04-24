use crate::{Controller, Result, StdDescriptor, Worker};
use libafl_core::{Error, WorkerId, illegal_argument, internal_bug};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, vec::Vec};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleController {
    root_dir: PathBuf,
    id_ctr: u32,
    workers: Vec<SimpleWorker>,
}

/// this is just a wrapper around StdDescriptor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleWorker {
    /// the descriptor describing this client
    descriptor: StdDescriptor,
}

impl SimpleController {
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
            workers: Vec::new(),
            id_ctr: 0,
        })
    }

    /// Create a new [`SimpleGlobalController`] with the default LibAFLmm root directory, "./workdir".
    ///
    /// The directory must not exist before calling this function.
    pub fn new() -> Result<Self> {
        Self::with_workdir(PathBuf::from("./workdir"))
    }
}

impl Controller for SimpleController {
    type Worker = SimpleWorker;
    type Descriptor = StdDescriptor;

    fn create_controller(&mut self) -> Result<SimpleWorker> {
        let client_id = WorkerId(self.id_ctr);
        self.id_ctr += 1;

        let client_dir = self.root_dir.join(format!("client_{}", client_id.0));

        if client_dir.exists() {
            return Err(internal_bug!(
                "The client dir \"{}\" already exists.",
                client_dir.display()
            ));
        }

        fs::create_dir(client_dir.as_path())?;

        let descriptor = StdDescriptor::new(client_dir, client_id)?;

        let cl = SimpleWorker::new(descriptor);
        self.workers.push(cl.clone());
        Ok(cl)
    }

    fn controllers(&self) -> &[Self::Worker] {
        &self.workers
    }
}

impl Worker for SimpleWorker {
    type Controller = SimpleController;

    fn id(&self) -> WorkerId {
        self.descriptor.id
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

impl SimpleWorker {
    pub fn new(descriptor: StdDescriptor) -> Self {
        Self { descriptor }
    }
}
