//! The [`PowerScheduleStage`] is the mutational stage that uses power schedules during fuzzing.
//! For the current input, it will perform a range of random mutations, and then run them in the executor.

use alloc::{
    borrow::{Cow, ToOwned},
    string::ToString,
};
use core::marker::PhantomData;

use libaflmm_bolts::{Named, rands::Rand};

use crate::{
    Result,
    common::{DependencyResolver, PowerScheduleData, Registrator},
    corpus::{Corpus, TestcaseId},
    fuzzers::Evaluator,
    inputs::Input,
    mutators::{MutationResult, Mutator},
    runtimes::RuntimeHandle,
    stages::{AFLPower, MutationalStage, Power, Stage},
    states::State,
};

impl<E, F, I, M, R, S, W, Z> DependencyResolver for PowerScheduleStage<E, F, I, M, R, S, W, Z> {
    fn register(&mut self, registrator: &mut Registrator) -> Result<()> {
        registrator.register_md_default::<PowerScheduleData>("");
        Ok(())
    }
}

/// The [`PowerScheduleStage`]
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

    /// The list of [`Mutator`], added to this stage
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

impl<E, F, I, M, R, S, W, Z> PowerScheduleStage<E, F, I, M, R, S, W, Z>
where
    F: Power<S>,
    R: Rand,
    S: State,
{
    /// Gets the number of iterations calculated through [`Power`]
    fn iterations(state: &mut S, current: TestcaseId) -> Result<usize> {
        F::score(state, current)
    }
}

/// The unique id for mutational stage
static mut POWER_MUTATIONAL_STAGE_ID: usize = 0;
/// The global prefix for [`PowerScheduleStage`]
pub static POWER_MUTATIONAL_STAGE_NAME: &str = "power";

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
    S: State<Input = I>,
    Z: Evaluator<E, I, S, W>,
{
    #[inline]
    fn perform_impl(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<()> {
        let num = Self::iterations(state, *testcase_id)?;

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
    /// Creates a new default [`PowerScheduleStage`]
    #[inline]
    pub fn new(mutator: M) -> Self {
        let stage_id = unsafe {
            let ret = POWER_MUTATIONAL_STAGE_ID;
            POWER_MUTATIONAL_STAGE_ID += 1;
            ret
        };
        let name = Cow::Owned(
            POWER_MUTATIONAL_STAGE_NAME.to_owned() + ":" + stage_id.to_string().as_str(),
        );
        Self {
            name,
            mutator,
            phantom: PhantomData,
        }
    }
}

/// The default [`PowerScheduleStage`] using [`AFLPower`]
pub type AFLPowerScheduleStage<E, I, M, R, S, W, Z> =
    PowerScheduleStage<E, AFLPower, I, M, R, S, W, Z>;
