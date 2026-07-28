use core::fmt::Debug;
use libaflmm_bolts::connection::Transferable;
use serde::{Deserialize, Serialize};

// pub mod aflpp;

pub mod exchanges;
pub use exchanges::{
    Exchange, NopCommand, NopExchange, NopNotification, SimpleCommand, SimpleExchange,
    SimpleNotification, StdCommand, StdExchange, StdNotification,
};

pub mod orchestrators;
pub use orchestrators::{GenericOrchestrator, GraphOrchestrator, Orchestrator, StdOrchestrator};

pub mod routers;
pub use routers::{NopRouter, Router};

pub mod transports;
pub use transports::{
    ControllerSync, DefaultHandleProviderFactory, HandleProvider, HandleProviderFactory,
    NopControllerSync, NopTransport, NopWorkerSync, SerializedHandleProvider,
    SerializedHandleProviderFactory, Transport, UnreachableHandleProvider,
    UnreachableHandleProviderFactory, WorkerSync,
};

pub type StdRouter = NopRouter;
pub type StdTransport = NopTransport;
pub type StdHandleProvider = UnreachableHandleProvider;
pub type StdHandleProviderFactory = UnreachableHandleProviderFactory;
pub type StdWorkerSync = NopWorkerSync;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct GroupId {
    pub(crate) id: u64,
}

impl GroupId {
    #[must_use]
    pub fn invalid() -> Self {
        Self { id: u64::MAX }
    }
}
