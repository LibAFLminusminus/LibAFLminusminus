//! Stage wrappers that add logics to stage list

use alloc::borrow::Cow;

use libaflmm_bolts::Named;

use crate::{
    Result,
    common::{DependencyResolver, Registrator},
    corpus::testcase::TestcaseId,
    stages::{RuntimeHandle, Stage, StagesTuple},
    states::State,
};

/// help compiler infer hrtb bounds. useful when you have "closure with signature for any lifetime'1, but it actually implements ... for some specific lifetime '2" error.
pub fn constrain<S, R, W, Z, F>(f: F) -> F
where
    F: FnMut(&mut RuntimeHandle<S, W>, &mut R, &mut S, &mut Z) -> Result<bool>,
{
    f
}

#[derive(Debug)]
/// Perform the stage while the closure evaluates to true
pub struct WhileStage<CB, ST> {
    closure: CB,
    stages: ST,
}

impl<CB, ST> WhileStage<CB, ST> {
    /// Constructor for [`struct@WhileStage`]
    pub fn new(closure: CB, stages: ST) -> Self {
        Self { closure, stages }
    }
}

impl<CB, ST> DependencyResolver for WhileStage<CB, ST>
where
    ST: DependencyResolver,
{
    fn register(&mut self, registrator: &mut Registrator) -> Result<()> {
        registrator.register_ty::<Self>();
        self.register_md(registrator)?;

        self.stages.register(registrator)
    }
}

impl<CB, ST> Named for WhileStage<CB, ST> {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("while");
        &NAME
    }
}

impl<CB, E, R, ST, S, W, Z> Stage<E, R, S, W, Z> for WhileStage<CB, ST>
where
    CB: FnMut(&mut RuntimeHandle<S, W>, &mut R, &mut S, &mut Z) -> Result<bool>,
    S: State,
    ST: StagesTuple<E, R, S, W, Z>,
{
    fn perform_impl(
        &mut self,
        fuzzer: &mut Z,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<()> {
        while (self.closure)(rt_handle, rand, state, fuzzer)? {
            self.stages
                .perform_all(fuzzer, rand, state, rt_handle, testcase_id)?;
        }

        Ok(())
    }

    // don't register into timer; inner stages record themselves.
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<()> {
        self.perform_impl(fuzzer, rand, state, rt_handle, testcase_id)
    }
}

/// A conditionally enabled stage.
/// If the closure returns true, the wrapped stage will be executed, else it will be skipped.
#[derive(Debug)]
pub struct IfStage<CB, ST> {
    closure: CB,
    if_stages: ST,
}

impl<CB, ST> DependencyResolver for IfStage<CB, ST>
where
    ST: DependencyResolver,
{
    fn register(&mut self, registrator: &mut Registrator) -> Result<()> {
        registrator.register_ty::<Self>();
        self.register_md(registrator)?;

        self.if_stages.register(registrator)
    }
}

impl<CB, ST> Named for IfStage<CB, ST> {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("if");
        &NAME
    }
}

impl<CB, ST> IfStage<CB, ST> {
    /// Constructor for [`struct@IfStage`]
    pub fn new(closure: CB, if_stages: ST) -> Self {
        Self { closure, if_stages }
    }
}

impl<CB, E, R, ST, S, W, Z> Stage<E, R, S, W, Z> for IfStage<CB, ST>
where
    CB: FnMut(&mut RuntimeHandle<S, W>, &mut R, &mut S, &mut Z) -> Result<bool>,
    S: State,
    ST: StagesTuple<E, R, S, W, Z>,
{
    fn perform_impl(
        &mut self,
        fuzzer: &mut Z,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<()> {
        if (self.closure)(rt_handle, rand, state, fuzzer)? {
            self.if_stages
                .perform_all(fuzzer, rand, state, rt_handle, testcase_id)?;
        }
        Ok(())
    }

    // don't register into timer; inner stages record themselves.
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<()> {
        self.perform_impl(fuzzer, rand, state, rt_handle, testcase_id)
    }
}

/// Perform [`Self::if_stages`] if the closure evaluates to true, else perfrom [`Self::else_stages`]
#[derive(Debug)]
pub struct IfElseStage<CB, ST1, ST2> {
    closure: CB,
    if_stages: ST1,
    else_stages: ST2,
}

impl<CB, ST1, ST2> DependencyResolver for IfElseStage<CB, ST1, ST2>
where
    ST1: DependencyResolver,
    ST2: DependencyResolver,
{
    fn register(&mut self, registrator: &mut Registrator) -> Result<()> {
        registrator.register_ty::<Self>();
        self.register_md(registrator)?;

        self.if_stages.register(registrator)?;
        self.else_stages.register(registrator)
    }
}

impl<CB, ST1, ST2> Named for IfElseStage<CB, ST1, ST2> {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("ifelse");
        &NAME
    }
}

impl<CB, ST1, ST2> IfElseStage<CB, ST1, ST2> {
    /// Constructor for [`struct@IfElseStage`]
    pub fn new(closure: CB, if_stages: ST1, else_stages: ST2) -> Self {
        Self {
            closure,
            if_stages,
            else_stages,
        }
    }
}

impl<CB, E, R, ST1, ST2, S, W, Z> Stage<E, R, S, W, Z> for IfElseStage<CB, ST1, ST2>
where
    CB: FnMut(&mut RuntimeHandle<S, W>, &mut R, &mut S, &mut Z) -> Result<bool>,
    S: State,
    ST1: StagesTuple<E, R, S, W, Z>,
    ST2: StagesTuple<E, R, S, W, Z>,
{
    fn perform_impl(
        &mut self,
        fuzzer: &mut Z,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<()> {
        if (self.closure)(rt_handle, rand, state, fuzzer)? {
            self.if_stages
                .perform_all(fuzzer, rand, state, rt_handle, testcase_id)?;
        } else {
            self.else_stages
                .perform_all(fuzzer, rand, state, rt_handle, testcase_id)?;
        }
        Ok(())
    }

    // don't register into timer; inner stages record themselves.
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<()> {
        self.perform_impl(fuzzer, rand, state, rt_handle, testcase_id)
    }
}
