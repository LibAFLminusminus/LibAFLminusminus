//! The [`TracerStage`] can trace the target with an alternate [`Executor`] and enrich a testcase with metadata, for example for `CmpLog`.

use alloc::{
    borrow::{Cow, ToOwned},
    string::ToString,
};
use core::{fmt::Debug, marker::PhantomData};

use libaflmm_bolts::Named;

use crate::{
    DependencyResolver, Error, Worker,
    corpus::{Corpus, testcase::TestcaseId},
    executors::Executor,
    inputs::Input,
    observers::ObserversTuple,
    runtimes::RuntimeHandle,
    stages::Stage,
    states::State,
};

/// A stage that runs a tracer executor
/// This should *NOT* be used with inprocess executor because usually you should never have more than one inprocess executors inside one process.
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
        self.tracer_executor.register_with_ty(registrator)?;

        Ok(())
    }
}

impl<E, I, R, S, TE, W, Z> Stage<E, R, S, W, Z> for TracerStage<I, TE>
where
    TE: Executor<I, S>,
    S: State<Input = I>,
    I: Input,
    W: Worker,
{
    #[inline]
    fn perform_impl(
        &mut self,
        _fuzzer: &mut Z,
        _executor: &mut E,
        _rand: &mut R,
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
static mut TRACER_STAGE_ID: usize = 0;
/// The name prefix for tracing stage
pub static TRACER_STAGE_NAME: &str = "tracing";

impl<I, TE> TracerStage<I, TE> {
    /// Creates a new [`struct@TracerStage`] from `tracer_executor`
    pub fn new(tracer_executor: TE) -> Self {
        // unsafe but impossible that you create two threads both instantiating this instance
        let stage_id = unsafe {
            let ret = TRACER_STAGE_ID;
            TRACER_STAGE_ID += 1;
            ret
        };

        Self {
            name: Cow::Owned(TRACER_STAGE_NAME.to_owned() + ":" + stage_id.to_string().as_ref()),
            tracer_executor,
            phantom: PhantomData,
        }
    }

    /// Gets the underlying [`Self::tracer_executor`]
    pub fn executor(&self) -> &TE {
        &self.tracer_executor
    }

    /// Gets mutable reference to the underlying [`Self::tracer_executor`]
    pub fn executor_mut(&mut self) -> &mut TE {
        &mut self.tracer_executor
    }
}
