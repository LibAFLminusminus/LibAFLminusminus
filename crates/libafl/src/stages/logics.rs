//! Stage wrappers that add logics to stage list

use crate::{
    DependencyResolver, Error,
    corpus::testcase::TestcaseId,
    stages::{RuntimeHandle, Stage, StagesTuple},
};

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
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<(), Error> {
        self.stages.register(registrator)
    }
}

impl<CB, E, R, ST, S, W, Z> Stage<E, R, S, W, Z> for WhileStage<CB, ST>
where
    CB: FnMut(&mut RuntimeHandle<S, W>, &mut E, &mut R, &mut S, &mut Z) -> Result<bool, Error>,
    ST: StagesTuple<E, R, S, W, Z>,
{
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<(), Error> {
        while (self.closure)(rt_handle, executor, rand, state, fuzzer)? {
            self.stages
                .perform_all(fuzzer, executor, rand, state, rt_handle, testcase_id)?;
        }

        Ok(())
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
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<(), Error> {
        self.if_stages.register(registrator)
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
    CB: FnMut(&mut RuntimeHandle<S, W>, &mut E, &mut R, &mut S, &mut Z) -> Result<bool, Error>,
    ST: StagesTuple<E, R, S, W, Z>,
{
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<(), Error> {
        if (self.closure)(rt_handle, executor, rand, state, fuzzer)? {
            self.if_stages
                .perform_all(fuzzer, executor, rand, state, rt_handle, testcase_id)?;
        }
        Ok(())
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
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<(), Error> {
        self.if_stages.register(registrator)?;
        self.else_stages.register(registrator)
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
    CB: FnMut(&mut RuntimeHandle<S, W>, &mut E, &mut R, &mut S, &mut Z) -> Result<bool, Error>,
    ST1: StagesTuple<E, R, S, W, Z>,
    ST2: StagesTuple<E, R, S, W, Z>,
{
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<(), Error> {
        if (self.closure)(rt_handle, executor, rand, state, fuzzer)? {
            self.if_stages
                .perform_all(fuzzer, executor, rand, state, rt_handle, testcase_id)?;
        } else {
            self.else_stages
                .perform_all(fuzzer, executor, rand, state, rt_handle, testcase_id)?;
        }
        Ok(())
    }
}
