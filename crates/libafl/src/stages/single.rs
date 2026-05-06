//! The tracing stage can trace the target and enrich a testcase with metadata, for example for `CmpLog`.

use alloc::{
    borrow::{Cow, ToOwned},
    string::ToString,
};
use core::{fmt::Debug, marker::PhantomData};

use libafl_bolts::Named;

use crate::{
    DependencyResolver, Error, Evaluator,
    corpus::{Corpus, testcase::TestcaseId},
    executors::Executor,
    inputs::Input,
    observers::ObserversTuple,
    stages::{RuntimeHandle, Stage},
    states::HasCorpus,
};

/// A stage that runs a tracer executor
/// This should *NOT* be used with inprocess executor
#[derive(Debug, Clone)]
pub struct SingleRunStage<I, Pre, Post> {
    name: Cow<'static, str>,
    pre: Pre,
    post: Post,
    phantom: PhantomData<I>,
}

impl<I, Pre, Post> DependencyResolver for SingleRunStage<I, Pre, Post> {}

impl<E, I, Pre, Post, R, S, W, Z> Stage<E, R, S, W, Z> for SingleRunStage<I, Pre, Post>
where
    S: HasCorpus<I>,
    Z: Evaluator<E, I, S, W>,
    Pre: FnMut(&mut RuntimeHandle<S, W>, &mut E, &mut R, &mut S, &mut Z) -> Result<(), Error>,
    Post: FnMut(&mut RuntimeHandle<S, W>, &mut E, &mut R, &mut S, &mut Z) -> Result<(), Error>,
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
        (self.pre)(rt_handle, executor, rand, state, fuzzer)?;

        let input = state.corpus().get(testcase_id)?.input();
        fuzzer.evaluate_input(state, executor, rt_handle, &input)?;

        (self.post)(rt_handle, executor, rand, state, fuzzer)?;

        Ok(())
    }
}

impl<I, Pre, Post> Named for SingleRunStage<I, Pre, Post> {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

/// The counter for giving this stage unique id
static mut SINGLE_RUN_STAGE_ID: usize = 0;
/// The name for tracing stage
pub static SINGLE_RUN_STAGE_NAME: &str = "single";

/// short type for the hook type
pub type RunHookFn<E, R, S, W, Z> =
    fn(&mut RuntimeHandle<S, W>, &mut E, &mut R, &mut S, &mut Z) -> Result<(), Error>;

fn noop_hook<E, R, S, W, Z>(
    _: &mut RuntimeHandle<S, W>,
    _: &mut E,
    _: &mut R,
    _: &mut S,
    _: &mut Z,
) -> Result<(), Error> {
    Ok(())
}

impl<I, E, R, S, W, Z> Default
    for SingleRunStage<I, RunHookFn<E, R, S, W, Z>, RunHookFn<E, R, S, W, Z>>
{
    fn default() -> Self {
        Self::new(noop_hook, noop_hook)
    }
}

/// hook for cmplog where you toggles CMPLOG_ENABLED for enabling it
pub fn cmplog_pre_hook<E, R, S, W, Z>(
    _: &mut RuntimeHandle<S, W>,
    _: &mut E,
    _: &mut R,
    _: &mut S,
    _: &mut Z,
) -> Result<(), Error> {
    unsafe {
        libafl_targets::CMPLOG_ENABLED = 1;
    }
    Ok(())
}

/// hook for cmplog where you toggles CMPLOG_ENABLED for disabling it
pub fn cmplog_post_hook<E, R, S, W, Z>(
    _: &mut RuntimeHandle<S, W>,
    _: &mut E,
    _: &mut R,
    _: &mut S,
    _: &mut Z,
) -> Result<(), Error> {
    unsafe {
        libafl_targets::CMPLOG_ENABLED = 0;
    }
    Ok(())
}

impl<I, E, R, S, W, Z> SingleRunStage<I, RunHookFn<E, R, S, W, Z>, RunHookFn<E, R, S, W, Z>> {
    pub fn cmplog() -> Self {
        Self::new(cmplog_pre_hook, cmplog_post_hook)
    }
}

impl<I, Pre, Post> SingleRunStage<I, Pre, Post> {
    /// constructor for this single run stage. use default() instead if you have absolutely nothing to hook
    pub fn new(pre: Pre, post: Post) -> Self {
        // unsafe but impossible that you create two threads both instantiating this instance
        let stage_id = unsafe {
            let ret = SINGLE_RUN_STAGE_ID;
            SINGLE_RUN_STAGE_ID += 1;
            ret
        };

        Self {
            name: Cow::Owned(
                SINGLE_RUN_STAGE_NAME.to_owned() + ":" + stage_id.to_string().as_ref(),
            ),
            pre,
            post,
            phantom: PhantomData,
        }
    }
}
