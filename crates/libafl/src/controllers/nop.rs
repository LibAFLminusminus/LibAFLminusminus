use crate::{Controller, Descriptor, Workdir, Worker};

pub struct NopController;
pub struct NopWorker;

#[derive(Clone)]
pub struct NopDescriptor;

impl Descriptor for NopDescriptor {
    fn workdir(&self) -> &Workdir {
        panic!("No descriptor for NopDescriptor.");
    }

    fn workdir_mut(&mut self) -> &mut Workdir {
        panic!("No descriptor for NopDescriptor.");
    }
}

impl Controller for NopController {
    type Worker = NopWorker;
    type Descriptor = NopDescriptor;

    fn create_worker(&mut self) -> Result<Self::Worker, libafl_core::Error> {
        Ok(NopWorker)
    }

    fn worker_descriptors(&self) -> &[Self::Descriptor] {
        unimplemented!("nop controller has no workers");
    }

    fn worker_descriptors_mut(&mut self) -> &mut [Self::Descriptor] {
        unimplemented!("nop controller has no workers");
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

    fn workdir(&self) -> &Workdir {
        unimplemented!("nop controller has no workdir");
    }

    fn workdir_mut(&mut self) -> &mut Workdir {
        unimplemented!("nop controller has no workdir");
    }

    fn reconcile(&self) -> Result<(), libafl_core::Error> {
        Ok(())
    }
}
