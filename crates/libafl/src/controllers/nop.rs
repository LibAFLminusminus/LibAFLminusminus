use crate::{Controller, Worker};

pub struct NopController;
pub struct NopWorker;

#[derive(Clone)]
pub struct NopDescriptor;

impl Controller for NopController {
    type Worker = NopWorker;
    type Descriptor = NopDescriptor;

    fn create_controller(&mut self) -> Result<Self::Worker, libafl_core::Error> {
        Ok(NopWorker)
    }

    fn controllers(&self) -> &[Self::Worker] {
        unimplemented!("nop controller has no clients");
    }
}

impl Worker for NopWorker {
    type Controller = NopController;

    fn id(&self) -> libafl_core::WorkerId {
        unimplemented!("nop controller has no id");
    }

    fn descriptor(&self) -> &NopDescriptor {
        &NopDescriptor
    }

    fn workdir(&self) -> &std::path::PathBuf {
        unimplemented!("nop controller has no workdir");
    }

    fn reconcile(&self) -> Result<(), libafl_core::Error> {
        Ok(())
    }
}
