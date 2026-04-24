//| The [`MutationalStage`] is the default stage used during fuzzing.
//! For the current input, it will perform a range of random mutations, and then run them in the executor.

use alloc::{
    borrow::{Cow, ToOwned},
    string::ToString,
};
use core::{marker::PhantomData, num::NonZeroUsize};

use libafl_bolts::{Named, rands::Rand};
use libafl_core::non_zero;

use crate::{
    DependencyResolver, Error,
    corpus::{Corpus, Testcase, TestcaseId},
    fuzzer::Evaluator,
    inputs::Input,
    mutators::{MultiMutator, MutationResult, Mutator},
    runtimes::RuntimeHandle,
    stages::Stage,
    state::{HasCorpus, State},
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

    /// Gets the number of iterations this mutator should run for.
    fn iterations(&self, rand: &mut R) -> Result<usize, Error>;
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

    /// The mutator, added to this stage
    #[inline]
    fn mutator(&self) -> &Self::Mutator {
        &self.mutator
    }

    /// The list of mutators, added to this stage (as mutable ref)
    #[inline]
    fn mutator_mut(&mut self) -> &mut Self::Mutator {
        &mut self.mutator
    }

    /// Gets the number of iterations as a random number
    fn iterations(&self, rand: &mut R) -> Result<usize, Error> {
        Ok(1 + rand.below(self.max_iterations))
    }
}

/// The unique id for mutational stage
static mut MUTATIONAL_STAGE_ID: usize = 0;
/// The name for mutational stage
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
    S: State<I>,
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
        self.perform_mutational(fuzzer, executor, rand, state, rt_handle, testcase_id)
    }
}

impl<E, I, M, R, S, W, Z> StdMutationalStage<E, I, M, R, S, W, Z> {
    /// Creates a new default mutational stage
    pub fn new(mutator: M) -> Self {
        // Safe to unwrap: DEFAULT_MUTATIONAL_MAX_ITERATIONS is never 0.
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
    S: State<I>,
    Z: Evaluator<E, I, S, W>,
{
    /// Runs this (mutational) stage for the given testcase
    fn perform_mutational(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<(), Error> {
        // Here saturating_sub is needed as self.iterations() might be actually smaller than the previous value before reset.
        /*
        let num = self
            .iterations(state)?
            .saturating_sub(self.execs_since_progress_start(state)?);
        */
        let num = self.iterations(rand)?;

        let tc = state.corpus().get(testcase_id)?;

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
/// A mutational stage that operates on multiple inputs, as returned by [`MultiMutator::multi_mutate`].
#[derive(Debug, Clone)]
pub struct MultiMutationalStage<E, I, M, R, S, W, Z> {
    name: Cow<'static, str>,
    mutator: M,
    phantom: PhantomData<(E, I, R, S, W, Z)>,
}

/// The unique id for multi mutational stage
static mut MULTI_MUTATIONAL_STAGE_ID: usize = 0;
/// The name for multi mutational stage
pub static MULTI_MUTATIONAL_STAGE_NAME: &str = "multimutational";

impl<E, I, M, R, S, W, Z> Named for MultiMutationalStage<E, I, M, R, S, W, Z> {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<E, I, M, R, S, W, Z> DependencyResolver for MultiMutationalStage<E, I, M, R, S, W, Z> {}

impl<E, I, M, R, S, W, Z> Stage<E, R, S, W, Z> for MultiMutationalStage<E, I, M, R, S, W, Z>
where
    I: Clone,
    M: MultiMutator<I, R, S>,
    S: State<I>,
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
        let tc = state.corpus().get(testcase_id)?;

        let generated = self.mutator.multi_mutate(&*tc.input(), rand, state, None)?;
        for new_input in generated {
            let eval_res = fuzzer.evaluate_input(state, executor, rt_handle, &new_input)?;
            self.mutator.multi_post_exec(state, &eval_res)?;
        }

        Ok(())
    }
}

impl<E, I, R, M, S, W, Z> MultiMutationalStage<E, I, M, R, S, W, Z> {
    /// Creates a new [`MultiMutationalStage`]
    pub fn new(mutator: M) -> Self {
        Self::transforming(mutator)
    }
}

impl<E, I, R, M, S, W, Z> MultiMutationalStage<E, I, M, R, S, W, Z> {
    /// Creates a new transforming mutational stage
    pub fn transforming(mutator: M) -> Self {
        // unsafe but impossible that you create two threads both instantiating this instance
        let stage_id = unsafe {
            let ret = MULTI_MUTATIONAL_STAGE_ID;
            MULTI_MUTATIONAL_STAGE_ID += 1;
            ret
        };
        Self {
            name: Cow::Owned(
                MULTI_MUTATIONAL_STAGE_NAME.to_owned() + ":" + stage_id.to_string().as_str(),
            ),
            mutator,
            phantom: PhantomData,
        }
    }
}
