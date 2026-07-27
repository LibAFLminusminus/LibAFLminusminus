use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::fmt::Debug;

// pub mod aflpp;

pub mod exchanges;
pub use exchanges::{
    Exchange, NopCommand, NopExchange, NopNotification, SimpleCommand, SimpleExchange,
    SimpleNotification, StdCommand, StdExchange, StdNotification,
};

pub mod routers;
pub use routers::{NopRouter, Router};

pub mod transports;
pub use transports::{
    ControllerSync, DefaultHandleProviderFactory, HandleProvider, HandleProviderFactory,
    NopControllerSync, NopTransport, NopWorkerSync, SeralizedHandleProviderFactory,
    SerializedHandleProvider, Transport, UnreachableHandlProvider,
    UnreachableHandleProviderFactory, WorkerSync,
};

pub type StdRouter = NopRouter;
pub type StdTransport = NopTransport;
pub type StdHandleProvider = UnreachableHandlProvider;
pub type StdHandleProviderFactory = UnreachableHandleProviderFactory;
pub type StdWorkerSync = NopWorkerSync;
pub type StdOrchestrator =
    GenericOrchestrator<StdExchange, StdHandleProviderFactory, StdRouter, StdTransport>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct GroupId {
    pub(crate) id: u64,
}

/// Shortcut for transportable messages over the wire
/// It is auto-impl for T enforcing these sub traits.
pub trait Transferable: Debug + Serialize + DeserializeOwned {}

pub trait Orchestrator<D, I> {
    type ProviderFactory: HandleProviderFactory<D, I, Provider = Self::Provider>;
    type Exchange: Exchange<D, Self::Handle, Command = Self::Command, Notification = Self::Notification>;
    type Router: Router<Self::Command, D>;
    type Transport: Transport<
            Self::Command,
            D,
            Self::Notification,
            WorkerSync = Self::WorkerSync,
            ControllerSync = Self::ControllerSync,
        >;

    // these types are not owned by orchestrator, but by the types above
    // we explicitly go around the usual rule of the repo here for convenience.
    type Handle: Transferable;
    type Command: Transferable;
    type Notification: Transferable;
    type Provider: HandleProvider<I>;
    type WorkerSync: WorkerSync<Self::Command, Self::Notification>;
    type ControllerSync: ControllerSync<Self::Notification, Self::Command>;
    type GroupConfig;

    fn router(&self) -> &Self::Router;
    fn router_mut(&mut self) -> &mut Self::Router;

    fn transport(&self) -> &Self::Transport;
    fn transport_mut(&mut self) -> &mut Self::Transport;

    fn exchange(&self) -> &Self::Exchange;
    fn exchange_mut(&mut self) -> &mut Self::Exchange;

    fn handle_provider_factory(&self) -> &Self::ProviderFactory;
    fn handle_provider_factory_mut(&mut self) -> &mut Self::ProviderFactory;
}

/// A general orchestrator
///
/// Most complex orchestrators can be derived from this one.
pub struct GenericOrchestrator<E, HPF, R, T> {
    exchange: E,
    router: R,
    transporter: T,
    handle_provider_factory: HPF,
}

impl<T> Transferable for T where T: Debug + Serialize + DeserializeOwned {}

impl<D, E, H, HPF, I, R, T> Orchestrator<D, I> for GenericOrchestrator<E, HPF, R, T>
where
    E: Exchange<D, H>,
    H: Transferable,
    HPF: HandleProviderFactory<D, I>,
    HPF::Provider: HandleProvider<I, Handle = H>,
    R: Router<E::Command, D>,
    T: Transport<E::Command, D, E::Notification>,
{
    type ProviderFactory = HPF;
    type Exchange = E;
    type Router = R;
    type Transport = T;

    type Handle = H;
    type Provider = HPF::Provider;
    type Command = E::Command;
    type Notification = E::Notification;
    type WorkerSync = T::WorkerSync;
    type ControllerSync = T::ControllerSync;
    type GroupConfig = R::GroupConfig;

    fn router(&self) -> &Self::Router {
        &self.router
    }

    fn router_mut(&mut self) -> &mut Self::Router {
        &mut self.router
    }

    fn transport(&self) -> &Self::Transport {
        &self.transporter
    }

    fn transport_mut(&mut self) -> &mut Self::Transport {
        &mut self.transporter
    }

    fn exchange(&self) -> &Self::Exchange {
        &self.exchange
    }

    fn exchange_mut(&mut self) -> &mut Self::Exchange {
        &mut self.exchange
    }

    fn handle_provider_factory(&self) -> &Self::ProviderFactory {
        &self.handle_provider_factory
    }

    fn handle_provider_factory_mut(&mut self) -> &mut Self::ProviderFactory {
        &mut self.handle_provider_factory
    }
}

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

impl GroupId {
    pub fn invalid() -> Self {
        Self { id: u64::MAX }
    }
}
