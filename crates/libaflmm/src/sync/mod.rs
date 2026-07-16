use crate::{
    Result,
    corpus::{Testcase, TestcaseId},
};
use libaflmm_core::WorkerId;
use std::fmt::Debug;

// pub mod aflpp;

pub mod nop;
pub use nop::{NopInputRepr, NopSynchronizer};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct GroupId {
    id: u64,
}

pub trait Orchestrator<D, I> {
    type InputRepr;

    type Synchronizer: Synchronizer<I, InputRepr = Self::InputRepr>;

    type GroupConfig: Debug + 'static;

    fn register_group(&mut self, config: Self::GroupConfig) -> Result<GroupId>;

    fn register_worker(&mut self, desc: &D) -> Result<()>;

    fn finalize(&mut self) -> Result<()>;

    fn create_synchronizer(&mut self, descriptor: &D) -> Result<Self::Synchronizer>;

    fn route(&mut self, source: WorkerId, tc_id: TestcaseId) -> Result<Vec<WorkerId>>;
}

pub trait Synchronizer<I> {
    type InputRepr;

    /// Report an input has been discovered
    fn report_testcase(&mut self, testcase: &Testcase<I>) -> Result<Option<Self::InputRepr>>;

    fn import_input(&mut self, repr: Self::InputRepr) -> Result<()>;

    /// Ask for an input to synchronize
    fn drain_inputs(&mut self) -> Result<impl Iterator<Item = I>>;
}
