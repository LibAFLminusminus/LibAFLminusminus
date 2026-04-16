use std::path::PathBuf;

use libafl_bolts::core_affinity::CoreId;
use libafl_core::Error;
use serde::{Deserialize, Serialize};

pub mod aflpp;
pub mod nop;

pub trait MainController {
    type ClientController: Controller;

    fn create_controller(
        &mut self,
        descriptor: <<Self as MainController>::ClientController as Controller>::Descriptor,
    ) -> Result<Self::ClientController, Error>;
}

pub trait Controller {
    type Descriptor;

    fn descriptor(&self) -> &Self::Descriptor;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StdDescriptor {
    root: PathBuf,
    main_controller_root: PathBuf,
    id: usize,
    overcommit_id: usize,
    core_id: CoreId,
}

// pub trait Controller {
//     type GlobalController;
//     type LocalController;
// }
