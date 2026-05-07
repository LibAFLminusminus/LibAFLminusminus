//! This module provides the hooks to insert into fuzzers (most likely StdFuzzer)
//! hook is a specific sub-routine that you can inject into serveral points of the the fuzzing loops

use libafl_bolts::tuples::MatchName;
use libafl_core::Named;

use crate::{
    Fuzzer, Result, corpus::TestcaseId, corpus::testcase::Testcase, runtimes::RuntimeHandle,
};

pub mod custom_name;
pub use custom_name::*;

pub trait FuzzerHook<E, I, S, W> {
    /// Run before one fuzzing loop starts
    fn pre_step(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        Ok(())
    }
    /// Run before fuzzer schedules and fetches a testcase
    fn pre_schedule(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        Ok(())
    }

    fn pre_add(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase: &mut Testcase<I>,
    ) -> Result<()> {
        Ok(())
    }

    /// Run before fuzzer performs all the stages
    fn pre_perform(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase: TestcaseId,
    ) -> Result<()> {
        Ok(())
    }
    /// Run before one fuzzing loop ends
    fn post_step(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        Ok(())
    }
}

pub trait FuzzerHooksTuple<E, I, S, W> {
    fn pre_step_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()>;
    fn pre_schedule_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()>;

    fn pre_add_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase: &mut Testcase<I>,
    ) -> Result<()>;
    fn pre_perform_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase: TestcaseId,
    ) -> Result<()>;
    fn post_step_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()>;
}

impl<E, I, S, W> FuzzerHooksTuple<E, I, S, W> for () {
    fn pre_step_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        Ok(())
    }
    fn pre_schedule_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        Ok(())
    }
    fn pre_add_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase: &mut Testcase<I>,
    ) -> Result<()> {
        Ok(())
    }
    fn pre_perform_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase: TestcaseId,
    ) -> Result<()> {
        Ok(())
    }
    fn post_step_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        Ok(())
    }
}

impl<Head, Tail, E, I, S, W> FuzzerHooksTuple<E, I, S, W> for (Head, Tail)
where
    Head: FuzzerHook<E, I, S, W>,
    Tail: FuzzerHooksTuple<E, I, S, W>,
{
    fn pre_step_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        self.0.pre_step(executor, state, rt_handle)?;
        self.1.pre_step_all(executor, state, rt_handle)
    }

    fn pre_schedule_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        self.0.pre_schedule(executor, state, rt_handle)?;
        self.1.pre_schedule_all(executor, state, rt_handle)
    }
    fn pre_add_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase: &mut Testcase<I>,
    ) -> Result<()> {
        self.0.pre_add(executor, state, rt_handle, testcase)?;
        self.1.pre_add_all(executor, state, rt_handle, testcase)
    }
    fn pre_perform_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase: TestcaseId,
    ) -> Result<()> {
        self.0.pre_perform(executor, state, rt_handle, testcase)?;
        self.1.pre_perform_all(executor, state, rt_handle, testcase)
    }
    fn post_step_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        self.0.post_step(executor, state, rt_handle)?;
        self.1.post_step_all(executor, state, rt_handle)
    }
}
