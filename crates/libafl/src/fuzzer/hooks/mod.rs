//! This module provides the hooks to insert into fuzzers (most likely StdFuzzer)
//! hook is a specific sub-routine that you can inject into serveral points of the the fuzzing loops

use libafl_bolts::tuples::MatchName;
use libafl_core::Named;

use crate::{Fuzzer, corpus::TestcaseId, runtimes::RuntimeHandle};

pub trait FuzzerHook: Named {
    /// Run before one fuzzing loop starts
    fn pre_step<E, S, W>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) {
    }
    /// Run before fuzzer schedules and fetches a testcase
    fn pre_schedule<E, S, W>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) {
    }
    /// Run before fuzzer performs all the stages
    fn pre_perform<E, S, W>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase: TestcaseId,
    ) {
    }
    /// Run before one fuzzing loop ends
    fn post_step<E, S, W>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) {
    }
}

pub trait FuzzerHooksTuple: MatchName {
    fn pre_step_all<E, S, W>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    );
    fn pre_schedule_all<E, S, W>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    );
    fn pre_perform_all<E, S, W>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase: TestcaseId,
    );
    fn post_step_all<E, S, W>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    );
}

impl FuzzerHooksTuple for () {
    fn pre_step_all<E, S, W>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) {
    }
    fn pre_schedule_all<E, S, W>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) {
    }
    fn pre_perform_all<E, S, W>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase: TestcaseId,
    ) {
    }
    fn post_step_all<E, S, W>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) {
    }
}

impl<Head, Tail> FuzzerHooksTuple for (Head, Tail)
where
    Head: FuzzerHook,
    Tail: FuzzerHooksTuple,
{
    fn pre_step_all<E, S, W>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) {
        self.0.pre_step(executor, state, rt_handle);
        self.1.pre_step_all(executor, state, rt_handle);
    }

    fn pre_schedule_all<E, S, W>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) {
        self.0.pre_schedule(executor, state, rt_handle);
        self.1.pre_schedule_all(executor, state, rt_handle);
    }
    fn pre_perform_all<E, S, W>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase: TestcaseId,
    ) {
        self.0.pre_perform(executor, state, rt_handle, testcase);
        self.1.pre_perform_all(executor, state, rt_handle, testcase);
    }
    fn post_step_all<E, S, W>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) {
        self.0.post_step(executor, state, rt_handle);
        self.1.post_step_all(executor, state, rt_handle);
    }
}
