use crate::sync::{
    DefaultHandleProviderFactory, GenericOrchestrator, StdExchange, StdHandleProviderFactory,
    StdRouter, StdTransport,
};

pub type StdOrchestrator =
    GenericOrchestrator<StdExchange, StdHandleProviderFactory, StdRouter, StdTransport>;

impl Default for StdOrchestrator {
    fn default() -> Self {
        Self {
            exchange: StdExchange::default(),
            handle_provider_factory: DefaultHandleProviderFactory::default(),
            router: StdRouter::default(),
            transporter: StdTransport::default(),
        }
    }
}
