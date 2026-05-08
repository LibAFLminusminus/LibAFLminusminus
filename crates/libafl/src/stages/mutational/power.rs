//| The [`PowerScheduleStage`] is the default stage used during fuzzing.
//! For the current input, it will perform a range of random mutations, and then run them in the executor.

use alloc::{
    borrow::{Cow, ToOwned},
    string::ToString,
};
use core::{marker::PhantomData, num::NonZeroUsize};

use libafl_bolts::{Named, rands::Rand};
use libafl_core::non_zero;

use crate::{
    DependencyResolver, Error, PowerScheduleData, Result,
    corpus::{Corpus, Testcase, TestcaseId},
    fuzzers::Evaluator,
    inputs::Input,
    mutators::{MutationResult, Mutator},
    runtimes::RuntimeHandle,
    stages::{AFLPower, MutationalStage, Power, Stage},
    states::{HasCorpus, HasScheduler, State},
};

/// Default value, how many iterations each stage gets, as an upper bound.
/// It may randomly continue earlier.
pub const DEFAULT_MUTATIONAL_MAX_ITERATIONS: usize = 128;

impl<E, F, I, M, R, S, W, Z> DependencyResolver for PowerScheduleStage<E, F, I, M, R, S, W, Z> {
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<()> {
        registrator.register_md_default::<PowerScheduleData>("".to_string());
        Ok(())
    }
}

/// The default mutational stage
#[derive(Debug, Clone)]
pub struct PowerScheduleStage<E, F, I, M, R, S, W, Z> {
    /// The name
    name: Cow<'static, str>,
    /// The mutator(s) to use
    mutator: M,
    phantom: PhantomData<(E, F, I, R, S, W, Z)>,
}

impl<E, F, I, M, R, S, W, Z> MutationalStage<R> for PowerScheduleStage<E, F, I, M, R, S, W, Z>
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
}

impl<E, F, I, M, R, S, SC, W, Z> PowerScheduleStage<E, F, I, M, R, S, W, Z>
where
    F: Power<S>,
    R: Rand,
    S: HasScheduler<Scheduler = SC>,
{
    /// Gets the number of iterations as a random number
    fn iterations(&self, state: &mut S, current: TestcaseId) -> Result<usize> {
        F::score(state, current)
    }
}

/// The unique id for mutational stage
static mut MUTATIONAL_STAGE_ID: usize = 0;
/// The name for mutational stage
pub static MUTATIONAL_STAGE_NAME: &str = "mutational";

impl<E, F, I, M, R, S, W, Z> Named for PowerScheduleStage<E, F, I, M, R, S, W, Z> {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<E, F, I, M, R, S, W, Z> Stage<E, R, S, W, Z> for PowerScheduleStage<E, F, I, M, R, S, W, Z>
where
    F: Power<S>,
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
    ) -> Result<()> {
        let num = self.iterations(state, *testcase_id)?;

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

impl<E, F, I, M, R, S, W, Z> PowerScheduleStage<E, F, I, M, R, S, W, Z> {
    /// Creates a new default mutational stage
    #[inline]
    pub fn new(mutator: M) -> Self {
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
            phantom: PhantomData,
        }
    }
}

pub type AFLPowerScheduleStage<E, I, M, R, S, W, Z> =
    PowerScheduleStage<E, AFLPower, I, M, R, S, W, Z>;
