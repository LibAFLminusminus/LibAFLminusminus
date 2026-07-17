use crate::{
    Result,
    corpus::{Testcase, TestcaseId},
};
use libaflmm_core::WorkerId;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::fmt::Debug;

// pub mod aflpp;

pub mod nop;
pub use nop::{NopInputRepr, NopOrchestrator, NopSynchronizer};

pub type StdOrchestrator = NopOrchestrator;
pub type StdSynchronizer = NopSynchronizer;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct GroupId {
    id: u64,
}

/// The sharing policy implementation.
pub trait Router<D> {
    type GroupConfig;

    /// Register a group in the router
    fn register_group(&mut self, config: Self::GroupConfig) -> Result<GroupId>;
    /// Register a worker in the router
    fn register_worker(&mut self, desc: &D) -> Result<()>;

    /// Call it when all registrations are done
    fn finalize(&mut self) -> Result<()>;

    /// Get all the destination nodes of a [`Worker`]
    fn destinations(&self, worker: WorkerId) -> impl Iterator<Item = WorkerId>;

    /// Get all the source nodes of a [`Worker`]
    fn sources(&self, worker: WorkerId) -> impl Iterator<Item = WorkerId>;

    /// Route a testcase from a source worker
    /// This is where the main routing logic lives
    fn route(
        &mut self,
        source: WorkerId,
        tc_id: TestcaseId,
    ) -> Result<impl Iterator<Item = WorkerId>>;

    /// True iff the worker has destination workers
    fn has_destinations(&self, worker: WorkerId) -> bool {
        self.destinations(worker).count() != 0
    }
}

/// The transfer mechanism for inputs
pub trait Transporter<D, I> {
    /// The worker-side synchronization mechanism
    type Synchronizer: Synchronizer<I> + Debug;

    /// Create a new synchronizer for the worker using a given descriptor
    fn create_synchronizer(
        &mut self,
        descriptor: &D,
        sources: impl for<'a> Iterator<Item = &'a D>,
    ) -> Result<Self::Synchronizer>;
}

pub trait Synchronizer<I> {
    /// An input representative.
    /// It is a way to identify an input without storing it fully.
    type InputRepr: Clone + Serialize + DeserializeOwned + 'static;

    /// Report an input that has been discovered
    ///
    /// It will return an [`Self::InputRepr`] if the input should be reported back using
    /// the transporter, and None otherwise.
    fn export(&mut self, testcase: &Testcase<I>) -> Result<Option<Self::InputRepr>>;

    /// Import a [`Testcase`] from its [`Self::InputRepr`].
    /// As a result, the input will be buffered until [`Self::drain`] is effectively called.
    fn import(&mut self, source: GroupId, id: TestcaseId, repr: Self::InputRepr) -> Result<()>;

    /// Drain all inputs received since last call.
    ///
    /// Once called, all pending inputs will be consumed.
    fn drain(&mut self) -> Result<impl Iterator<Item = Testcase<I>>>;
}

pub trait Orchestrator<D, I> {
    type Router: Router<D>;
    type Transporter: Transporter<D, I>;

    fn router(&self) -> &Self::Router;
    fn router_mut(&mut self) -> &mut Self::Router;

    fn transporter(&self) -> &Self::Transporter;
    fn transporter_mut(&mut self) -> &mut Self::Transporter;
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
