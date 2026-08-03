use crate::sync::{
    GenericOrchestrator, SerializedInputHandleBackendFactory, SimpleExchange, StdCommand,
    StdNotification, routers::graph::GraphRouter, transfers::DirectTransfer,
};

pub type GraphOrchestrator<K = usize> = GenericOrchestrator<
    SimpleExchange,
    SerializedInputHandleBackendFactory,
    GraphRouter<K>,
    DirectTransfer<StdCommand<Vec<u8>>, StdNotification<Vec<u8>>>,
>;

impl<K> GraphOrchestrator<K> {
    #[must_use]
    pub fn new(router: GraphRouter<K>) -> Self {
        Self {
            exchange: SimpleExchange,
            handle_provider_factory: SerializedInputHandleBackendFactory::default(),
            router,
            transfer: DirectTransfer::default(),
        }
    }
}
