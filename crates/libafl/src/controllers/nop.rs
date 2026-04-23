use crate::{Controller, GlobalController};

pub struct NopMainController;
pub struct NopController;
pub struct NopDescriptor;

impl GlobalController for NopMainController {
    type Controller = NopController;

    fn create_controller(
        &mut self,
        _descriptor: <<Self as GlobalController>::Controller as Controller>::Descriptor,
    ) -> Result<Self::Controller, libafl_core::Error> {
        Ok(NopController)
    }

    fn controllers(&self) -> &[Self::Controller] {
        unimplemented!("nop controller has no clients");
    }
}

impl Controller for NopController {
    type Descriptor = NopDescriptor;

    fn id(&self) -> libafl_core::ClientId {
        unimplemented!("nop controller has no id");
    }

    fn descriptor(&self) -> &Self::Descriptor {
        &NopDescriptor
    }

    fn workdir(&self) -> &std::path::PathBuf {
        unimplemented!("nop controller has no workdir");
    }

    fn reconcile(&self) -> Result<(), libafl_core::Error> {
        Ok(())
    }
}
