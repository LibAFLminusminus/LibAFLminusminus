use crate::{Controller, MasterController};

pub struct NopMainController;
pub struct NopController;
pub struct NopDescriptor;

impl MasterController for NopMainController {
    type Client = NopController;

    fn create_controller(
        &mut self,
        _descriptor: <<Self as MasterController>::Client as Controller>::Descriptor,
    ) -> Result<Self::Client, libafl_core::Error> {
        Ok(NopController)
    }

    fn clients(&self) -> &[Self::Client] {
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
