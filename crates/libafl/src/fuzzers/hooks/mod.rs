//! This module provides the hooks to insert into fuzzers (most likely [`StdFuzzer`](crate::StdFuzzer))
//! hook is a specific sub-routine that you can inject into serveral points of the the fuzzing loops

use crate::{
    DependencyResolver, Result, Verdict,
    corpus::{TestcaseId, testcase::Testcase},
    runtimes::RuntimeHandle,
};

pub mod custom_name;
pub use custom_name::CustomNameHook;

pub mod calibration;
pub use calibration::CalibrationHook;

/// A fuzzer hook is used to insert custom callbacks at various stages of the fuzzer execution.
pub trait FuzzerHook<E, I, S, W>: DependencyResolver {
    /// Run before one fuzzing loop starts
    fn pre_step(
        &mut self,
        _executor: &mut E,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        Ok(())
    }

    /// Run before the fuzzer adds an input
    fn pre_add(
        &mut self,
        _executor: &mut E,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
        _testcase: &mut Testcase<I>,
        _verdict: Verdict,
    ) -> Result<()> {
        Ok(())
    }

    /// Run after the fuzzer adds an input
    fn post_add(
        &mut self,
        _executor: &mut E,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
        _testcase_id: TestcaseId,
        _verdict: Verdict,
    ) -> Result<()> {
        Ok(())
    }

    /// Run before fuzzer performs all the stages
    fn pre_perform(
        &self,
        _executor: &mut E,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
        _testcase: TestcaseId,
    ) -> Result<()> {
        Ok(())
    }

    /// Run before one fuzzing loop ends
    fn post_step(
        &self,
        _executor: &mut E,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        Ok(())
    }
}

/// A tuple of [`FuzzerHook`]s.
pub trait FuzzerHooksTuple<E, I, S, W>: DependencyResolver {
    /// run all [`FuzzerHook::pre_add`]
    fn pre_add_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase: &mut Testcase<I>,
        verdict: Verdict,
    ) -> Result<()>;

    /// run all [`FuzzerHook::post_add`]
    fn post_add_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: TestcaseId,
        verdict: Verdict,
    ) -> Result<()>;

    /// run all [`FuzzerHook::pre_perform`]
    fn pre_perform_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase: TestcaseId,
    ) -> Result<()>;

    /// run all [`FuzzerHook::pre_step`]
    fn pre_step_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()>;

    /// run all [`FuzzerHook::post_step`]
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
        _executor: &mut E,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        Ok(())
    }

    fn pre_add_all(
        &mut self,
        _executor: &mut E,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
        _testcase: &mut Testcase<I>,
        _verdict: Verdict,
    ) -> Result<()> {
        Ok(())
    }
    fn post_add_all(
        &mut self,
        _executor: &mut E,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
        _testcase_id: TestcaseId,
        _verdict: Verdict,
    ) -> Result<()> {
        Ok(())
    }

    fn pre_perform_all(
        &mut self,
        _executor: &mut E,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
        _testcase: TestcaseId,
    ) -> Result<()> {
        Ok(())
    }
    fn post_step_all(
        &mut self,
        _executor: &mut E,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
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

    fn pre_add_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase: &mut Testcase<I>,
        verdict: Verdict,
    ) -> Result<()> {
        self.0
            .pre_add(executor, state, rt_handle, testcase, verdict)?;
        self.1
            .pre_add_all(executor, state, rt_handle, testcase, verdict)
    }

    fn post_add_all(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: TestcaseId,
        verdict: Verdict,
    ) -> Result<()> {
        self.0
            .post_add(executor, state, rt_handle, testcase_id, verdict)?;
        self.1
            .post_add_all(executor, state, rt_handle, testcase_id, verdict)
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
