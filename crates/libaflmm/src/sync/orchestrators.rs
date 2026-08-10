use crate::sync::{
    DirectTransfer, SerializedInputHandleBackendFactory, StdInputHandleBackendFactory, StdRouter,
    StdTransfer, routers::graph::GraphRouter,
};

pub type StdOrchestrator = Orchestrator<StdInputHandleBackendFactory, StdRouter, StdTransfer>;

pub type GraphOrchestrator<K = usize, U = ()> =
    Orchestrator<SerializedInputHandleBackendFactory, GraphRouter<K>, DirectTransfer<Vec<u8>, U>>;

/// An Orchestrator regroups the sync components and represents the final strategy
#[derive(Debug, Default)]
pub struct Orchestrator<HBF, R, T> {
    /// The input handle backend factory, defining how inputs get shared. It could be nothing, the input itself, its filepath, etc...
    pub handle_backend_factory: HBF,
    /// The router, defining the sharing topology
    pub router: R,
    /// The transfer, defining the transport of commands and notifications
    pub transfer: T,
}

impl<K, U> GraphOrchestrator<K, U> {
    #[must_use]
    pub fn new(router: GraphRouter<K>) -> Self {
        Self {
            handle_backend_factory: SerializedInputHandleBackendFactory::default(),
            router,
            transfer: DirectTransfer::default(),
        }
    }
}
