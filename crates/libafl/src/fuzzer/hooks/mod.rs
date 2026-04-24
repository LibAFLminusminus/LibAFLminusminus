//! This module provides the hooks to insert into fuzzers (most likely StdFuzzer)
//! hook is a specific sub-routine that you can inject into serveral points of the the fuzzing loops

use libafl_bolts::tuples::MatchName;
use libafl_core::Named;

use crate::{Fuzzer, corpus::TestcaseId, runtimes::RuntimeHandle};

pub trait FuzzerHook: Named {
    /// Run before one fuzzing loop starts
    fn pre_step_start<CT, E, S>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
    ) {
    }
    /// Run before fuzzer schedules and fetches a testcase
    fn pre_schedule<CT, E, S>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
    ) {
    }
    /// Run before fuzzer performs all the stages
    fn pre_perform<CT, E, S>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
        testcase: TestcaseId,
    ) {
    }
    /// Run before one fuzzing loop ends
    fn pre_step_end<CT, E, S>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
    ) {
    }
}

pub trait FuzzerHooksTuple: MatchName {
    fn pre_step_start_all<CT, E, S>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
    );
    fn pre_schedule_all<CT, E, S>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
    );
    fn pre_perform_all<CT, E, S>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
        testcase: TestcaseId,
    );
    fn pre_step_end_all<CT, E, S>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
    );
}

impl FuzzerHooksTuple for () {
    fn pre_step_start_all<CT, E, S>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
    ) {
    }
    fn pre_schedule_all<CT, E, S>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
    ) {
    }
    fn pre_perform_all<CT, E, S>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
        testcase: TestcaseId,
    ) {
    }
    fn pre_step_end_all<CT, E, S>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
    ) {
    }
}

impl<Head, Tail> FuzzerHooksTuple for (Head, Tail)
where
    Head: FuzzerHook,
    Tail: FuzzerHooksTuple,
{
    fn pre_step_start_all<CT, E, S>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
    ) {
        self.0.pre_step_start(executor, state, rt_handle);
        self.1.pre_step_start_all(executor, state, rt_handle);
    }

    fn pre_schedule_all<CT, E, S>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
    ) {
        self.0.pre_schedule(executor, state, rt_handle);
        self.1.pre_schedule_all(executor, state, rt_handle);
    }
    fn pre_perform_all<CT, E, S>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
        testcase: TestcaseId,
    ) {
        self.0.pre_perform(executor, state, rt_handle, testcase);
        self.1.pre_perform_all(executor, state, rt_handle, testcase);
    }
    fn pre_step_end_all<CT, E, S>(
        &self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
    ) {
        self.0.pre_step_end(executor, state, rt_handle);
        self.1.pre_step_end_all(executor, state, rt_handle);
    }
}
