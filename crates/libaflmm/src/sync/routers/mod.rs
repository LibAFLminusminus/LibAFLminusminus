use crate::{Result, sync::GroupId};
use libaflmm_core::WorkerId;
use std::fmt::Debug;

/// The sharing policy implementation.
///
/// It will take care of routing of commands.
pub trait Router<CMD, D>: Debug {
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

#[derive(Debug, Default)]
pub struct NopRouter;

impl<CMD, D> Router<CMD, D> for NopRouter {
    type GroupConfig = ();

    fn destinations(&self, _worker: WorkerId) -> impl Iterator<Item = WorkerId> {
        [].into_iter()
    }

    fn finalize(&mut self) -> Result<()> {
        Ok(())
    }

    fn has_destinations(&self, _worker: WorkerId) -> bool {
        false
    }

    fn register_group(&mut self, _config: Self::GroupConfig) -> Result<GroupId> {
        Ok(GroupId::invalid())
    }

    fn register_worker(&mut self, _desc: &D) -> Result<()> {
        Ok(())
    }

    fn route(&mut self, _source: WorkerId, _cmd: &CMD) -> Result<impl Iterator<Item = WorkerId>> {
        Ok([].into_iter())
    }

    fn sources(&self, _worker: WorkerId) -> impl Iterator<Item = WorkerId> {
        [].into_iter()
    }
}
