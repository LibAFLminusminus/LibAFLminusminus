//! The [`MutationalStage`] is the default stage used during fuzzing.
//! For the current input, it will perform a range of random mutations, and then run them in the executor.

use alloc::{
    borrow::{Cow, ToOwned},
    string::ToString,
};
use core::{marker::PhantomData, num::NonZeroUsize};

use libaflmm_bolts::{Named, rands::Rand};
use libaflmm_core::non_zero;

use crate::{
    DependencyResolver, Error,
    corpus::{Corpus, TestcaseId},
    fuzzers::Evaluator,
    inputs::Input,
    mutators::{MutationResult, Mutator},
    runtimes::RuntimeHandle,
    stages::Stage,
    states::State,
};

/// A Mutational stage is the stage in a fuzzing run that mutates inputs.
/// Mutational stages will usually have a range of mutations that are
/// being applied to the input one by one, between executions.
pub trait MutationalStage<R> {
    /// The mutator of this stage
    type Mutator;

    /// The mutator registered for this stage
    fn mutator(&self) -> &Self::Mutator;

    /// The mutator registered for this stage (mutable)
    fn mutator_mut(&mut self) -> &mut Self::Mutator;
}

/// Default value, how many iterations each stage gets, as an upper bound.
/// It may randomly continue earlier.
pub const DEFAULT_MUTATIONAL_MAX_ITERATIONS: usize = 128;

impl<E, I, M, R, S, W, Z> DependencyResolver for StdMutationalStage<E, I, M, R, S, W, Z> {}

/// The default mutational stage
#[derive(Debug, Clone)]
pub struct StdMutationalStage<E, I, M, R, S, W, Z> {
    /// The name
    name: Cow<'static, str>,
    /// The mutator(s) to use
    mutator: M,
    /// The maximum amount of iterations we should do each round
    max_iterations: NonZeroUsize,
    phantom: PhantomData<(E, I, R, S, W, Z)>,
}

impl<E, I, M, R, S, W, Z> MutationalStage<R> for StdMutationalStage<E, I, M, R, S, W, Z>
where
    R: Rand,
{
    type Mutator = M;

    /// The list of [`Mutator`], added to this stage
    #[inline]
    fn mutator(&self) -> &Self::Mutator {
        &self.mutator
    }

    /// The list of [`Mutator`], added to this stage (as mutable ref)
    #[inline]
    fn mutator_mut(&mut self) -> &mut Self::Mutator {
        &mut self.mutator
    }
}

impl<E, I, M, R, S, W, Z> StdMutationalStage<E, I, M, R, S, W, Z>
where
    R: Rand,
    S: State,
{
    /// Gets the number of iterations as a random number
    #[expect(clippy::unnecessary_wraps)]
    fn iterations(&self, rand: &mut R) -> Result<usize, Error> {
        Ok(1 + rand.below(self.max_iterations))
    }
}

/// The unique id for mutational stage
static mut MUTATIONAL_STAGE_ID: usize = 0;
/// The global prefix for mutational stage
pub static MUTATIONAL_STAGE_NAME: &str = "mutational";

impl<E, I, M, R, S, W, Z> Named for StdMutationalStage<E, I, M, R, S, W, Z> {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<E, I, M, R, S, W, Z> Stage<E, R, S, W, Z> for StdMutationalStage<E, I, M, R, S, W, Z>
where
    I: Input,
    M: Mutator<I, R, S>,
    R: Rand,
    S: State<Input = I>,
    Z: Evaluator<E, I, S, W>,
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
        self.perform_mutational(fuzzer, executor, rand, state, rt_handle, *testcase_id)
    }
}

impl<E, I, M, R, S, W, Z> StdMutationalStage<E, I, M, R, S, W, Z> {
    /// Creates a new default [`StdMutationalStage`]
    pub fn new(mutator: M) -> Self {
        Self::with_max_iterations(mutator, non_zero!(DEFAULT_MUTATIONAL_MAX_ITERATIONS))
    }

    /// Creates a new mutational stage with the given max iterations
    #[inline]
    pub fn with_max_iterations(mutator: M, max_iterations: NonZeroUsize) -> Self {
        let stage_id = unsafe {
            let ret = MUTATIONAL_STAGE_ID;
            MUTATIONAL_STAGE_ID += 1;
            ret
        };
        let name =
            Cow::Owned(MUTATIONAL_STAGE_NAME.to_owned() + ":" + stage_id.to_string().as_str());
        Self {
            name,
            mutator,
            max_iterations,
            phantom: PhantomData,
        }
    }
}

impl<E, I, M, R, S, W, Z> StdMutationalStage<E, I, M, R, S, W, Z>
where
    I: Clone,
    M: Mutator<I, R, S>,
    R: Rand,
    S: State<Input = I>,
    Z: Evaluator<E, I, S, W>,
{
    /// Runs this [`StdMutationalStage`] stage for the given testcase
    fn perform_mutational(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: TestcaseId,
    ) -> Result<(), Error> {
        let num = self.iterations(rand)?;

        let tc = state.corpus().get(&testcase_id)?;

        for _ in 0..num {
            let mut input = tc.cloned_input();

            let mutated = self.mutator_mut().mutate(&mut input, rand, state)?;

            if mutated == MutationResult::Skipped {
                continue;
            }

            let eval_res = fuzzer.evaluate_input(state, executor, rt_handle, &input)?;

            self.mutator_mut().post_exec(state, &eval_res)?;
        }

        Ok(())
    }
}
