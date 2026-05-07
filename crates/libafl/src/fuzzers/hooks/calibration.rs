use crate::{DependencyResolver, FuzzerHook};

pub struct CalibrationHook {}

impl DependencyResolver for CalibrationHook {}

impl<E, I, S, W> FuzzerHook<E, I, S, W> for CalibrationHook {
    fn post_add(
        &mut self,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut crate::runtimes::RuntimeHandle<S, W>,
        testcase_id: crate::corpus::TestcaseId,
    ) -> libafl_core::Result<()> {
        Ok(())
    }
}
