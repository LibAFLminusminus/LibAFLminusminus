use libaflmm_core::WorkerId;
use serde::{Deserialize, Serialize};

use crate::{
    Result,
    corpus::{Testcase, TestcaseId},
    inputs::Input,
    sync::{GroupId, Orchestrator, Router},
};

#[derive(Debug, Default, Serialize, Deserialize, Clone, Copy)]
pub struct NopInputRepr;

#[derive(Debug, Default)]
pub struct NopSynchronizer;

#[derive(Debug, Default)]
pub struct NopTransporter;

#[derive(Debug, Default)]
pub struct NopRouter;

#[derive(Debug, Default)]
pub struct NopOrchestrator {
    router: NopRouter,
    transporter: NopTransporter,
}

impl<I, W> Orchestrator<I, W> for NopOrchestrator {
    type Router = NopRouter;
    type Transporter = NopTransporter;

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

impl<I> Synchronizer<I> for NopSynchronizer
where
    I: Input,
{
    type InputRepr = NopInputRepr;

    fn export(&mut self, testcase: &Testcase<I>) -> Result<Option<Self::InputRepr>> {
        Ok(None)
    }

    fn import(&mut self, source: GroupId, id: TestcaseId, repr: Self::InputRepr) -> Result<()> {
        Ok(())
    }

    fn drain(&mut self) -> Result<impl Iterator<Item = Testcase<I>>> {
        Ok([].into_iter())
    }
}

impl<D> Router<D> for NopRouter {
    type GroupConfig = ();

    fn destinations(&self, worker: WorkerId) -> impl Iterator<Item = WorkerId> {}
}
