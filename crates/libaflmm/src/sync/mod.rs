use core::fmt::Debug;
use libaflmm_bolts::connection::Transferable;
use serde::{Deserialize, Serialize};

pub mod exchanges;
pub use exchanges::{
    Exchange, NopCommand, NopExchange, NopNotification, SimpleCommand, SimpleExchange,
    SimpleNotification, StdCommand, StdExchange, StdNotification,
};

pub mod orchestrators;
pub use orchestrators::{GenericOrchestrator, GraphOrchestrator, Orchestrator, StdOrchestrator};

pub mod routers;
pub use routers::{NopRouter, Router};

pub mod transfers;
pub use transfers::{
    ControllerSync, DefaultInputHandleBackendFactory, DirectTransfer, InputHandleBackend,
    InputHandleBackendFactory, NopControllerSync, NopTransfer, NopWorkerSync,
    SerializedInputHandleBackendFactory, SerializedInputhandleBackend, SocketControllerSync,
    SocketWorkerSync, Transfer, UnreachableInputHandleBackend,
    UnreachableInputHandleBackendFactory, WaitResult, WorkerSync,
};

pub type StdRouter = NopRouter;
pub type StdTransfer = NopTransfer;
pub type StdHandleProvider = UnreachableInputHandleBackend;
pub type StdHandleProviderFactory = UnreachableInputHandleBackendFactory;
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
