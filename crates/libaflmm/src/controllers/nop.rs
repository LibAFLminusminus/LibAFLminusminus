//! Nop controller and workers.

use crate::controllers::{Controller, Descriptor, Workdir, Worker};
use libaflmm_bolts::CoreId;
use libaflmm_core::{Result, WorkerId};

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

    fn core_id(&self) -> CoreId {
        panic!("No descriptor for NopDescriptor.");
    }
}

impl Controller for NopController {
    type Worker = NopWorker;
    type Descriptor = NopDescriptor;

    fn create_worker(&mut self, _core_id: CoreId) -> Result<Self::Worker> {
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

    fn descriptor(&self) -> &NopDescriptor {
        unimplemented!("nop controller has no descriptor");
    }

    fn descriptor_mut(&mut self) -> &mut NopDescriptor {
        unimplemented!("nop controller has no descriptor");
    }

    fn reconcile(&self) -> Result<()> {
        Ok(())
    }
}
