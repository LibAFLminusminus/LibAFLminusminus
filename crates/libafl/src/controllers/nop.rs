use crate::{Controller, MainController};

pub struct NopMainController;
pub struct NopController;
pub struct NopDescriptor;

impl MainController for NopMainController {
    type ClientController = NopController;

    fn create_controller(
        &mut self,
        _descriptor: <<Self as MainController>::ClientController as Controller>::Descriptor,
    ) -> Result<Self::ClientController, libafl_core::Error> {
        Ok(NopController)
    }
}

impl Controller for NopController {
    type Descriptor = NopDescriptor;

    fn descriptor(&self) -> &Self::Descriptor {
        &NopDescriptor
    }

    fn is_main(&self) -> bool {
        true
    }

    fn workdir(&self) -> &std::path::PathBuf {
        unimplemented!("nop controller has no workdir");
    }

    fn child_workdirs(&self) -> Option<&[std::path::PathBuf]> {
        unimplemented!("nop controller has no child workdir");
    }

    fn reconcile(&self) -> Result<(), libafl_core::Error> {
        Ok(())
    }
}
