use crate::Result;
use libaflmm_core::WorkerId;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::fmt::Debug;

// pub mod aflpp;

pub mod nop;
pub use nop::{NopInputRepr, NopOrchestrator, NopSynchronizer};

pub mod exchangers;

pub type StdOrchestrator = NopOrchestrator;
pub type StdSynchronizer = NopSynchronizer;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct GroupId {
    id: u64,
}

pub trait InputExchanger<I> {
    type InputHandle: Debug + Serialize + DeserializeOwned + Clone;

    /// Create a fresh [`Self::InputHandle`] from a given `input`
    fn create_handle(&mut self, input: &I) -> Result<Self::InputHandle>;

    /// Fetch back an input from its [`Self::InputHandle`]
    fn handle_to_input(&mut self, handle: Self::InputHandle) -> Result<I>;
}

/// The worker side of the synchronization mechanism
pub trait WorkerSync<SD, I, RCV> {
    /// Send a SD value to the [`ControllerSync`]
    fn send(&mut self, val: SD) -> Result<()>;

    /// Poll for RCV values from the [`ControllerSync`]
    fn poll(&mut self) -> Result<impl Iterator<Item = RCV>>;
}

pub trait ControllerSync<SD, RCV> {
    /// Send a SD value to the [`WorkerSync`] with the [`WorkerId`] `worker`.
    fn send(&mut self, worker: WorkerId, val: SD) -> Result<()>;

    /// Poll for RCV values from all the the [`WorkerSync`] attached to [`Self`].
    fn poll(&mut self) -> Result<(RCV, WorkerId)>;
}

/// The sharing policy implementation.
///
/// It will take care of routing of commands.
pub trait Router<CMD, D> {
    type GroupConfig;

    /// Register a group in the router
    fn register_group(&mut self, config: Self::GroupConfig) -> Result<GroupId>;
    /// Register a worker in the router
    fn register_worker(&mut self, desc: &D) -> Result<()>;
    /// Call it when all registrations are done
    /// It can only be called once, and registration becomes illegal after it's called.
    fn finalize(&mut self) -> Result<()>;

    /// Route a command from a source worker
    /// This is where the main routing logic lives
    fn route(&mut self, source: WorkerId, cmd: &CMD) -> Result<impl Iterator<Item = WorkerId>>;
    /// Get all the destination nodes of a [`Worker`]
    fn destinations(&self, worker: WorkerId) -> impl Iterator<Item = WorkerId>;
    /// Get all the source nodes of a [`Worker`]
    fn sources(&self, worker: WorkerId) -> impl Iterator<Item = WorkerId>;

    /// True iff the worker has destination workers
    fn has_destinations(&self, worker: WorkerId) -> bool {
        self.destinations(worker).count() != 0
    }
}

/// The transfer mechanism for commands and notifications
pub trait Transport<CMD, D, I, NOTIF> {
    type ControllerSync: ControllerSync<CMD, NOTIF>;
    type WorkerSync: WorkerSync<NOTIF, I, CMD>;
    type InputExchanger: InputExchanger<I>;

    /// Create a new worker synchronizer for the worker using a given descriptor
    fn create_worker_sync<'a>(
        &mut self,
        descriptor: &'a D,
        sources: impl Iterator<Item = &'a D>,
    ) -> Result<(Self::InputExchanger, Self::WorkerSync)>;

    /// Finalize the transport lifetime with the creation of the controller-side synchronizer
    fn create_controller_sync(self) -> Result<Self::ControllerSync>;
}

// pub trait Synchronizer<I> {
//     /// An input representative.
//     /// It is a way to identify an input without storing it fully.
//     type InputRepr: Clone + Serialize + DeserializeOwned + 'static;
//
//     /// Report an input that has been discovered
//     ///
//     /// It will return an [`Self::InputRepr`] if the input should be reported back using
//     /// the transporter, and None otherwise.
//     fn export(&mut self, testcase: &Testcase<I>) -> Result<Option<Self::InputRepr>>;
//
//     /// Import a [`Testcase`] from its [`Self::InputRepr`].
//     /// As a result, the input will be buffered until [`Self::drain`] is effectively called.
//     fn import(&mut self, source: GroupId, id: TestcaseId, repr: Self::InputRepr) -> Result<()>;
//
//     /// Drain all inputs received since last call.
//     ///
//     /// Once called, all pending inputs will be consumed.
//     fn drain(&mut self) -> Result<impl Iterator<Item = Testcase<I>>>;
// }

pub trait Orchestrator<D, I> {
    type Command;
    type Notification;

    type Router: Router<Self::Command, D>;
    type Transport: Transport<Self::Command, D, I, Self::Notification>;

    fn notif_to_command(
        &mut self,
        source: &D,
        notif: Self::Notification,
    ) -> Result<Option<Self::Command>>;

    fn router(&self) -> &Self::Router;
    fn router_mut(&mut self) -> &mut Self::Router;

    fn transport(&self) -> &Self::Transport;
    fn transport_mut(&mut self) -> &mut Self::Transport;
}

/// A general orchestrator
///
/// Most complex orchestrators can be derived from this one.
pub struct GenericOrchestrator<R, T> {
    router: R,
    transporter: T,
}

impl<D, I, R, T> Orchestrator<D, I> for GenericOrchestrator<R, T>
where
    R: Router<D>,
    T: Transporter<D, I>,
{
    type Router = R;
    type Transporter = T;

    fn router(&self) -> &Self::Router {
        &self.router
    }

    fn router_mut(&mut self) -> &mut Self::Router {
        &mut self.router
    }

    fn transporter(&self) -> &Self::Transporter {
        &self.transporter
    }

    fn transporter_mut(&mut self) -> &mut Self::Transporter {
        &mut self.transporter
    }
}
