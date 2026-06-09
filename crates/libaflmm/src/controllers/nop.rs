//! Nop controller and workers.

use crate::controllers::{Controller, Descriptor, Workdir, Worker};
use libaflmm_bolts::CoreId;
use libaflmm_core::{Result, WorkerId};

/// Nop [`Controller`]
#[derive(Clone, Debug)]
pub struct NopController;

/// Nop [`Worker`]
#[derive(Clone, Debug, Default)]
pub struct NopWorker {
    descriptor: NopDescriptor,
}

/// Nop [`Descriptor`]
#[derive(Clone, Debug)]
pub struct NopDescriptor {
    workdir: Workdir,
}

impl Default for NopDescriptor {
    fn default() -> Self {
        Self {
            workdir: Workdir::new(std::env::temp_dir(), None, None, None)
                .expect("the temporary directory should be a valid workdir root"),
        }
    }
}

impl Descriptor for NopDescriptor {
    fn workdir(&self) -> &Workdir {
        &self.workdir
    }

    fn workdir_mut(&mut self) -> &mut Workdir {
        &mut self.workdir
    }

    fn worker_id(&self) -> WorkerId {
        WorkerId(0)
    }

    fn core_id(&self) -> CoreId {
        CoreId(0)
    }
}

impl Controller for NopController {
    type Worker = NopWorker;
    type Descriptor = NopDescriptor;

    fn create_worker(&mut self, _core_id: CoreId) -> Result<Self::Worker> {
        Ok(NopWorker::default())
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
        &self.descriptor
    }

    fn descriptor_mut(&mut self) -> &mut NopDescriptor {
        &mut self.descriptor
    }

    fn reconcile(&self) -> Result<()> {
        Ok(())
    }
}
