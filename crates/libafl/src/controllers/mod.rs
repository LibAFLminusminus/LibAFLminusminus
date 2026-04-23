use alloc::vec::Vec;
use std::path::{Path, PathBuf};

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
    /// returns the descriptor describing each fuzzer instances
    fn descriptor(&self) -> &Self::Descriptor;

    /// returns the working directory of this instance
    fn workdir(&self) -> &PathBuf;

    /// returns the vec of the secondary instances that this instance manages if any.
    fn child_workdirs(&self) -> Option<&[PathBuf]>;

    /// do the work related to reconciling between instances; like sharing corpus.. etc.
    fn reconcile(&self) -> Result<(), Error>;

    /// tell if this instance is a main one or not
    fn is_main(&self) -> bool;
}

/// this is just a wrapper around stddescriptor
pub struct SimpleController {
    descriptor: StdDescriptor,
}

impl Controller for SimpleController {
    type Descriptor = StdDescriptor;

    fn descriptor(&self) -> &Self::Descriptor {
        &self.descriptor
    }

    fn workdir(&self) -> &PathBuf {
        match &self.descriptor {
            StdDescriptor::Main(main) => &main.this,
            StdDescriptor::Secondary(sec) => &sec.this,
        }
    }

    fn child_workdirs(&self) -> Option<&[PathBuf]> {
        match &self.descriptor {
            StdDescriptor::Main(main) => Some(&main.secondary),
            StdDescriptor::Secondary(sec) => None,
        }
    }

    fn reconcile(&self) -> Result<(), Error> {
        // do nothing
        Ok(())
    }

    fn is_main(&self) -> bool {
        match self.descriptor {
            StdDescriptor::Main(_) => true,
            StdDescriptor::Secondary(_) => false,
        }
    }
}

impl SimpleController {
    pub fn new(descriptor: StdDescriptor) -> Self {
        Self { descriptor }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StdDescriptor {
    Main(MainDescriptor),
    Secondary(SecondaryDescriptor),
}

/// The launcher should create instantiate this alongside binding this instance to a specific core id
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MainDescriptor {
    /// the path to the workdir of this controller
    this: PathBuf,
    /// the vector of the paths to the secondary controllers. Some() if there're multiple instance AND this is held by main controller
    secondary: Vec<PathBuf>,
    /// which core to bind this process to?
    core_id: CoreId,
}

/// The launcher should create instantiate this alongside binding this instance to a specific core id
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SecondaryDescriptor {
    /// the path to the workdir of this controller
    this: PathBuf,
    /// the vector path to the workdir of the main controller.
    main: PathBuf,
    /// which core to bind this process to?
    core_id: CoreId,
}

impl MainDescriptor {
    /// Default constructor
    pub fn main<P: AsRef<Path>>(main: P, core_id: CoreId) -> Result<Self, Error> {
        if !main.as_ref().is_dir() {
            return Err(Error::illegal_argument("main is not a valid directory"));
        }

        Ok(Self {
            this: main.as_ref().to_path_buf(),
            secondary: Vec::new(),
            core_id,
        })
    }

    /// Turn this stddescriptor given from main one to the secondary one.
    /// you use this like
    /// let main = StdDescriptor();
    fn spawn_secondary<P: AsRef<Path>>(
        mut self,
        path: P,
        core_id: CoreId,
    ) -> Result<SecondaryDescriptor, Error> {
        if !path.as_ref().is_dir() {
            return Err(Error::illegal_argument("path is not a valid directory"));
        }

        self.secondary.push(path.as_ref().to_path_buf());

        Ok(SecondaryDescriptor {
            this: path.as_ref().to_path_buf(),
            main: self.this,
            core_id,
        })
    }
}
