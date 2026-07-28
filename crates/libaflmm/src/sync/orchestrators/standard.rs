use crate::sync::{
    DefaultHandleProviderFactory, GenericOrchestrator, StdExchange, StdHandleProviderFactory,
    StdRouter, StdTransfer,
};

pub type StdOrchestrator =
    GenericOrchestrator<StdExchange, StdHandleProviderFactory, StdRouter, StdTransfer>;

impl Default for StdOrchestrator {
    fn default() -> Self {
        Self {
            exchange: StdExchange::default(),
            handle_provider_factory: DefaultHandleProviderFactory::default(),
            router: StdRouter::default(),
            transfer: StdTransfer::default(),
        }
    }
}
