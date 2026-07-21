use crate::inputs::Input;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

// pub mod aflpp;

pub mod exchanges;
pub use exchanges::{Exchange, NopCommand, NopExchange, NopNotification};

pub mod routers;
pub use routers::{NopRouter, Router};

pub mod transports;
pub use transports::{
    ControllerSync, IdentityInputRepr, InputRepr, NopControllerSync, NopTransport, NopWorkerSync,
    Transport, WorkerSync,
};

pub type StdOrchestrator = NopOrchestrator;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct GroupId {
    id: u64,
}

pub trait Orchestrator<D, I> {
    type Exchange: Exchange<D>;

    /// The input representation, that can turn an input into its handle and oppositely.
    type InputRepr: InputRepr<I>;

    type Router: Router<<Self::Exchange as Exchange<D>>::Command, D>;
    type Transport: Transport<
            <Self::Exchange as Exchange<D>>::Command,
            D,
            <Self::Exchange as Exchange<D>>::Notification,
        >;

    fn router(&self) -> &Self::Router;
    fn router_mut(&mut self) -> &mut Self::Router;

    fn transport(&self) -> &Self::Transport;
    fn transport_mut(&mut self) -> &mut Self::Transport;
}

/// A nop orchestrator, which does not perform any sharing between a controller and workers.
/// No command / notification will be sent over, and no input will be shared between any
/// worker.
#[derive(Debug, Default)]
pub struct NopOrchestrator {
    router: NopRouter,
    transport: NopTransport,
}

/// A general orchestrator
///
/// Most complex orchestrators can be derived from this one.
pub struct GenericOrchestrator<E, IR, R, T> {
    exchange: E,
    router: R,
    transporter: T,
    input_repr: IR,
}

impl<D, E, I, IR, R, T> Orchestrator<D, I> for GenericOrchestrator<E, IR, R, T>
where
    E: Exchange<D>,
    IR: InputRepr<I>,
    R: Router<E::Command, D>,
    T: Transport<E::Command, D, E::Notification>,
{
    type Exchange = E;
    type Router = R;
    type Transport = T;
    type InputRepr = IR;

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
}

impl<D, I> Orchestrator<D, I> for NopOrchestrator
where
    I: Input,
{
    type InputRepr = IdentityInputRepr;
    type Exchange = NopExchange;
    type Router = NopRouter;
    type Transport = NopTransport;

    fn router(&self) -> &Self::Router {
        &self.router
    }

    fn router_mut(&mut self) -> &mut Self::Router {
        &mut self.router
    }

    fn transport(&self) -> &Self::Transport {
        &self.transport
    }

    fn transport_mut(&mut self) -> &mut Self::Transport {
        &mut self.transport
    }
}

impl GroupId {
    pub fn invalid() -> Self {
        Self { id: u64::MAX }
    }
}
