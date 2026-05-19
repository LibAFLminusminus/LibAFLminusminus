/*!
A [`Stage`] is a module used during fuzzing, working on one [`crate::corpus::Corpus`] entry, and potentially altering it or creating new entries.
A well-known [`Stage`], for example, is the mutational stage, running multiple [`crate::mutators::Mutator`]s against a [`crate::corpus::Testcase`], potentially storing new ones, according to [`crate::feedbacks::Feedback`].
Other stages may enrich [`crate::corpus::Testcase`]s with metadata.
*/

use crate::{
    DependencyResolver, Result, corpus::TestcaseId, mutators::StdMutator, runtimes::RuntimeHandle,
    states::CoreState,
};
use alloc::{boxed::Box, vec::Vec};
use libaflmm_bolts::tuples::{HasConstLen, IntoVec};
use tuple_list::NonEmptyTuple;

use libaflmm_bolts::{Named, current_time};

pub mod tracer;
pub use tracer::*;

pub mod single;
pub use single::*;

pub mod logics;
pub use logics::*;

pub mod mutational;
pub use mutational::*;

pub mod nop;
pub use nop::NopStage;

pub mod dynamic;
pub use dynamic::DynamicStage;

pub mod generation;
pub use generation::GenStage;

pub type StdStage<E, I, R, S, W, Z> = StdMutationalStage<E, I, StdMutator, R, S, W, Z>;

/// A stage is one step in the fuzzing loop.
/// Multiple stages will be scheduled one by one for each input.
pub trait Stage<E, R, S, W, Z>: DependencyResolver + Named
where
    S: CoreState,
{
    /// The actual stage body. Implementors put their work here.
    fn perform_impl(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<()>;

    /// Run the stage. Called from the fuzzer loop. The wrapper makes it
    /// mandatory to record per-stage time, keyed by [`Named::name`], into
    /// `state.perf_stats_mut()`. Meta-stages should override to skip the
    /// recording so their inner stages each get their own bucket.
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<()> {
        let start = current_time();
        let res = self.perform_impl(fuzzer, executor, rand, state, rt_handle, testcase_id);
        let elapsed = current_time().saturating_sub(start);
        let name = self.name().clone();
        state.perf_stats_mut().record_stage(name, elapsed);
        res
    }
}

/// A tuple holding all [`Stages`] used for fuzzing.
pub trait StagesTuple<E, R, S, W, Z>: DependencyResolver {
    /// Performs all [`Stages`] in this tuple.
    fn perform_all(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<()>;
}

impl<E, R, S, W, Z> StagesTuple<E, R, S, W, Z> for () {
    fn perform_all(
        &mut self,
        _fuzzer: &mut Z,
        _executor: &mut E,
        _rand: &mut R,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
        _testcase_id: &TestcaseId,
    ) -> Result<()> {
        Ok(())
    }
}

impl<Head, Tail, E, R, S, W, Z> StagesTuple<E, R, S, W, Z> for (Head, Tail)
where
    Head: Stage<E, R, S, W, Z>,
    S: CoreState,
    Tail: StagesTuple<E, R, S, W, Z> + HasConstLen,
{
    /// Performs all [`Stages`] in the tuple,
    fn perform_all(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<()> {
        let stage = &mut self.0;

        stage.perform(fuzzer, executor, rand, state, rt_handle, testcase_id)?;

        self.1
            .perform_all(fuzzer, executor, rand, state, rt_handle, testcase_id)
    }
}

impl<Head, Tail, E, R, S, W, Z> IntoVec<Box<dyn Stage<E, R, S, W, Z>>> for (Head, Tail)
where
    Head: Stage<E, R, S, W, Z> + 'static,
    S: CoreState,
    Tail: StagesTuple<E, R, S, W, Z> + HasConstLen + IntoVec<Box<dyn Stage<E, R, S, W, Z>>>,
{
    fn into_vec_reversed(self) -> Vec<Box<dyn Stage<E, R, S, W, Z>>> {
        let (head, tail) = self.uncons();
        let mut ret = tail.0.into_vec_reversed();
        ret.push(Box::new(head));
        ret
    }

    fn into_vec(self) -> Vec<Box<dyn Stage<E, R, S, W, Z>>> {
        let mut ret = self.into_vec_reversed();
        ret.reverse();
        ret
    }
}

impl<Tail, E, R, S, W, Z> IntoVec<Box<dyn Stage<E, R, S, W, Z>>> for (Tail,)
where
    Tail: IntoVec<Box<dyn Stage<E, R, S, W, Z>>>,
{
    fn into_vec(self) -> Vec<Box<dyn Stage<E, R, S, W, Z>>> {
        self.0.into_vec()
    }
}

impl<E, R, S, W, Z> IntoVec<Box<dyn Stage<E, R, S, W, Z>>> for Vec<Box<dyn Stage<E, R, S, W, Z>>> {
    fn into_vec(self) -> Vec<Box<dyn Stage<E, R, S, W, Z>>> {
        self
    }
}

impl<E, R, S, W, Z> DependencyResolver for Vec<Box<dyn Stage<E, R, S, W, Z>>> {
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<()> {
        for st in self {
            st.register(registrator)?;
        }

        Ok(())
    }

    fn register_with_ty(&mut self, registrator: &mut crate::Registrator) -> Result<()> {
        for st in self {
            st.register_with_ty(registrator)?;
        }

        Ok(())
    }

    fn check(&self, checker: &crate::CompatibilityChecker) -> Result<()> {
        for st in self {
            st.check(checker)?;
        }

        Ok(())
    }
}

impl<E, R, S, W, Z> StagesTuple<E, R, S, W, Z> for Vec<Box<dyn Stage<E, R, S, W, Z>>>
where
    S: CoreState,
{
    /// Performs all stages in the `Vec`
    fn perform_all(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<()> {
        self.iter_mut().try_for_each(|stage| {
            stage.perform(fuzzer, executor, rand, state, rt_handle, testcase_id)
        })
    }
}
