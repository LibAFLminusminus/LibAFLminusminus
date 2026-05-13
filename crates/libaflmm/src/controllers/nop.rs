//! Nop controller and workers.

use libafl_core::{Result, WorkerId};

use crate::{Controller, Descriptor, Workdir, Worker};

/// Nop [`Controller`]
#[derive(Clone, Debug)]
pub struct NopController;

/// Nop [`Worker`]
#[derive(Clone, Debug)]
pub struct NopWorker;

/// Nop [`Descriptor`]
#[derive(Clone, Debug)]
pub struct NopDescriptor;

impl Descriptor for NopDescriptor {
    fn workdir(&self) -> &Workdir {
        panic!("No descriptor for NopDescriptor.");
    }

    fn workdir_mut(&mut self) -> &mut Workdir {
        panic!("No descriptor for NopDescriptor.");
    }

    fn worker_id(&self) -> WorkerId {
        panic!("No descriptor for NopDescriptor.");
    }
}

impl Controller for NopController {
    type Worker = NopWorker;
    type Descriptor = NopDescriptor;

    fn create_worker(&mut self) -> Result<Self::Worker> {
        Ok(NopWorker)
    }

    #[expect(refining_impl_trait)]
    fn worker_descriptors(&self) -> &[Self::Descriptor] {
        unimplemented!("nop controller has no workers");
    }

    #[expect(refining_impl_trait)]
    fn worker_descriptors_mut(&mut self) -> &mut [Self::Descriptor] {
        unimplemented!("nop controller has no workers");
    }
}

impl Worker for NopWorker {
    type Controller = NopController;

    fn id(&self) -> WorkerId {
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

    fn reconcile(&self) -> Result<()> {
        Ok(())
    }
}
