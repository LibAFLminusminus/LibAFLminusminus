//! The tracing stage can trace the target and enrich a testcase with metadata, for example for `CmpLog`.

use alloc::{
    borrow::{Cow, ToOwned},
    string::ToString,
};
use core::{fmt::Debug, marker::PhantomData};

use libafl_bolts::Named;

use crate::{
    DependencyResolver, Error, Worker,
    corpus::{Corpus, testcase::TestcaseId},
    executors::Executor,
    inputs::Input,
    observers::ObserversTuple,
    runtimes::RuntimeHandle,
    stages::Stage,
    states::{FlatState, HasCorpus},
};

/// A stage that runs a tracer executor
/// This should *NOT* be used with inprocess executor
#[derive(Debug, Clone)]
pub struct TracerStage<I, TE> {
    name: Cow<'static, str>,
    tracer_executor: TE,
    phantom: PhantomData<I>,
}

impl<I, TE> DependencyResolver for TracerStage<I, TE>
where
    TE: DependencyResolver,
{
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<(), Error> {
        self.tracer_executor.register_with_ty(registrator);

        Ok(())
    }
}

impl<E, I, R, S, TE, W, Z> Stage<E, R, S, W, Z> for TracerStage<I, TE>
where
    TE: Executor<I, S>,
    S: FlatState + HasCorpus<I>,
    I: Input,
    W: Worker,
{
    #[inline]
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<(), Error> {
        let tc = state.corpus().get(testcase_id)?;

        self.tracer_executor.observers_mut().pre_exec_all(state)?;

        let exit_kind = self
            .tracer_executor
            .execute(state, rt_handle, &tc.input())?;

        self.tracer_executor
            .observers_mut()
            .post_exec_all(state, &exit_kind)?;

        Ok(())
    }
}

impl<I, TE> Named for TracerStage<I, TE> {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

/// The counter for giving this stage unique id
static mut TRACING_STAGE_ID: usize = 0;
/// The name for tracing stage
pub static TRACING_STAGE_NAME: &str = "tracing";

impl<I, TE> TracerStage<I, TE> {
    /// Creates a new default stage
    pub fn new(tracer_executor: TE) -> Self {
        // unsafe but impossible that you create two threads both instantiating this instance
        let stage_id = unsafe {
            let ret = TRACING_STAGE_ID;
            TRACING_STAGE_ID += 1;
            ret
        };

        Self {
            name: Cow::Owned(TRACING_STAGE_NAME.to_owned() + ":" + stage_id.to_string().as_ref()),
            tracer_executor,
            phantom: PhantomData,
        }
    }

    /// Gets the underlying tracer executor
    pub fn executor(&self) -> &TE {
        &self.tracer_executor
    }

    /// Gets the underlying tracer executor (mut)
    pub fn executor_mut(&mut self) -> &mut TE {
        &mut self.tracer_executor
    }
}
