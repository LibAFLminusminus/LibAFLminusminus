use crate::{Controller, GlobalController};

pub struct NopGlobalController;
pub struct NopController;

#[derive(Clone)]
pub struct NopDescriptor;

impl GlobalController for NopGlobalController {
    type Controller = NopController;
    type Descriptor = NopDescriptor;

    fn create_controller(&mut self) -> Result<Self::Controller, libafl_core::Error> {
        Ok(NopController)
    }

    fn controllers(&self) -> &[Self::Controller] {
        unimplemented!("nop controller has no clients");
    }
}

impl Controller for NopController {
    type GlobalController = NopGlobalController;

    fn id(&self) -> libafl_core::ClientId {
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
