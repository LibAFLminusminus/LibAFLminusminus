//! A stage implementation that can have dynamic stage runtime

use alloc::borrow::Cow;

use libaflmm_bolts::Named;

use super::Stage;
use crate::{
    DependencyResolver, Result, corpus::testcase::TestcaseId, stages::RuntimeHandle,
    states::CoreState,
};

/// A dynamic stage implementation. This explicity uses enum so that rustc can better
/// reason about the bounds.
#[derive(Debug)]
pub enum DynamicStage<T1, T2> {
    /// One stage
    Stage1(T1),
    /// The alernative stage
    Stage2(T2),
}

impl<T1, T2> Named for DynamicStage<T1, T2> {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("dynamic");
        &NAME
    }
}

impl<T1, T2> DependencyResolver for DynamicStage<T1, T2>
where
    T1: DependencyResolver,
    T2: DependencyResolver,
{
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<()> {
        match self {
            Self::Stage1(st1) => st1.register(registrator),
            Self::Stage2(st2) => st2.register(registrator),
        }
    }
}

impl<E, R, S, T1, T2, W, Z> Stage<E, R, S, W, Z> for DynamicStage<T1, T2>
where
    S: CoreState,
    T1: Stage<E, R, S, W, Z>,
    T2: Stage<E, R, S, W, Z>,
{
    fn perform_impl(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<()> {
        match self {
            Self::Stage1(st1) => st1.perform(fuzzer, executor, rand, state, rt_handle, testcase_id),
            Self::Stage2(st2) => st2.perform(fuzzer, executor, rand, state, rt_handle, testcase_id),
        }
    }

    // don't register into timer; inner stages record themselves.
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<()> {
        self.perform_impl(fuzzer, executor, rand, state, rt_handle, testcase_id)
    }
}
