use crate::{Controller, Result, StdDescriptor, Worker, launchers::InstanceId};
use libafl_core::{Error, WorkerId, illegal_argument, internal_bug};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, vec::Vec};

#[derive(Debug, Clone)]
pub struct SimpleControllerBuilder {
    root_dir: PathBuf,
    overwrite: bool,
}

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
    /// If overwrite is true, the root_dir will be removed before being created again.
    pub fn new(root_dir: PathBuf, overwrite: bool) -> Result<Self> {
        if root_dir.exists() {
            if overwrite {
                fs::remove_dir_all(root_dir.as_path())?;
            } else {
                return Err(illegal_argument!(
                    "Wordir already exists: {}. Set `overwrite` to `true` if you want to overwrite.",
                    root_dir.display()
                ));
            }
        }

        fs::create_dir(root_dir.as_path())?;

        Ok(Self {
            root_dir,
            workers: Vec::new(),
            id_ctr: 0,
        })
    }

    pub fn builder() -> SimpleControllerBuilder {
        SimpleControllerBuilder::default()
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

    fn on_start(&mut self, descriptor: &Self::Descriptor, id: InstanceId) -> Result<()> {
        log::info!("Started worker {:?}", descriptor.id);
        Ok(())
    }

    fn on_termination(
        &mut self,
        descriptor: &Self::Descriptor,
        termination_code: nix::sys::signal::Signal,
    ) -> Result<()> {
        log::info!("Started worker {:?}", descriptor.id);
        Ok(())
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

impl Default for SimpleControllerBuilder {
    fn default() -> Self {
        Self {
            overwrite: false,
            root_dir: PathBuf::from("./workdir"),
        }
    }
}

impl SimpleControllerBuilder {
    pub fn overwrite(&mut self, overwrite: bool) -> &mut Self {
        self.overwrite = overwrite;
        self
    }

    pub fn root_dir(&mut self, root_dir: impl Into<PathBuf>) -> &mut Self {
        self.root_dir = root_dir.into();
        self
    }

    pub fn build(&mut self) -> Result<SimpleController> {
        SimpleController::new(self.root_dir.clone(), self.overwrite)
    }
}
