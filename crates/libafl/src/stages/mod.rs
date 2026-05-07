/*!
A [`Stage`] is a technique used during fuzzing, working on one [`crate::corpus::Corpus`] entry, and potentially altering it or creating new entries.
A well-known [`Stage`], for example, is the mutational stage, running multiple [`crate::mutators::Mutator`]s against a [`crate::corpus::Testcase`], potentially storing new ones, according to [`crate::feedbacks::Feedback`].
Other stages may enrich [`crate::corpus::Testcase`]s with metadata.
*/

use alloc::{
    borrow::{Cow, ToOwned},
    boxed::Box,
    string::ToString,
    vec::Vec,
};
use core::{fmt, marker::PhantomData};

use hashbrown::HashSet;
use libafl_bolts::{
    Named, impl_serdeany,
    tuples::{HasConstLen, IntoVec},
};
use serde::{Deserialize, Serialize};
use tuple_list::NonEmptyTuple;

use crate::{
    DependencyResolver, Error, corpus::TestcaseId, runtimes::RuntimeHandle, states::FlatState,
};

/// Mutational stage is the normal fuzzing stage.
pub mod mutational;
pub use mutational::{MutationalStage, StdMutationalStage};

pub mod single;
pub use single::*;

pub mod logics;
pub use logics::*;

pub mod power;
pub use power::*;

pub mod nop;
pub use nop::NopStage;

pub mod dynamic;
pub use dynamic::DynamicStage;

pub mod generation;
pub use generation::GenStage;

/// The index of a stage
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(transparent)]
pub struct StageId(pub(crate) usize);

impl fmt::Display for StageId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A stage is one step in the fuzzing process.
/// Multiple stages will be scheduled one by one for each input.
pub trait Stage<E, R, S, W, Z>: DependencyResolver {
    /// Run the stage.
    ///
    /// If you want this stage to restart, then
    /// Before a call to perform, [`Restartable::should_restart`] will be (must be!) called.
    /// After returning (so non-target crash or timeout in a restarting case), [`Restartable::clear_progress`] gets called.
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<(), Error>;
}

/// A tuple holding all `Stages` used for fuzzing.
pub trait StagesTuple<E, R, S, W, Z>: DependencyResolver {
    /// Performs all `Stages` in this tuple.
    fn perform_all(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<(), Error>;
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
    ) -> Result<(), Error> {
        Ok(())
    }
}

impl<Head, Tail, E, R, S, W, Z> StagesTuple<E, R, S, W, Z> for (Head, Tail)
where
    Head: Stage<E, R, S, W, Z>,
    Tail: StagesTuple<E, R, S, W, Z> + HasConstLen,
{
    /// Performs all stages in the tuple,
    /// Checks after every stage if state wants to stop
    /// and returns an [`Error::ShuttingDown`] if so
    fn perform_all(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<(), Error> {
        // match state.current_stage_id()? {
        //     Some(idx) if idx < StageId(Self::LEN) => {
        //         // do nothing; we are resuming
        //     }
        //     Some(idx) if idx == StageId(Self::LEN) => {
        //         // perform the stage, but don't set it

        //         let stage = &mut self.0;

        //         match stage.perform_restartable(fuzzer, executor, state, controller) {
        //             Ok(()) => {}
        //             Err(Error::SkipRemainingStages) => {
        //                 state.clear_stage_id()?;
        //                 return Ok(());
        //             }
        //             Err(e) => return Err(e),
        //         }

        //         state.clear_stage_id()?;
        //     }
        //     Some(idx) if idx > StageId(Self::LEN) => {
        //         unreachable!("We should clear the stage index before we get here...");
        //     }
        //     // this is None, but the match can't deduce that
        //     _ => {
        // state.set_current_stage_id(StageId(Self::LEN))?;

        let stage = &mut self.0;

        stage.perform(fuzzer, executor, rand, state, rt_handle, testcase_id)?;

        // state.clear_stage_id()?;
        //     }
        // }

        // Execute the remaining stages
        self.1
            .perform_all(fuzzer, executor, rand, state, rt_handle, testcase_id)
    }
}

impl<Head, Tail, E, R, S, W, Z> IntoVec<Box<dyn Stage<E, R, S, W, Z>>> for (Head, Tail)
where
    Head: Stage<E, R, S, W, Z> + 'static,
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
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<(), Error> {
        for st in self {
            st.register(registrator)?;
        }

        Ok(())
    }

    fn register_with_ty(&mut self, registrator: &mut crate::Registrator) -> Result<(), Error> {
        for st in self {
            st.register_with_ty(registrator)?;
        }

        Ok(())
    }

    fn check(&self, checker: &crate::CompatibilityChecker) -> Result<(), Error> {
        for st in self {
            st.check(checker)?;
        }

        Ok(())
    }
}

impl<E, R, S, W, Z> StagesTuple<E, R, S, W, Z> for Vec<Box<dyn Stage<E, R, S, W, Z>>> {
    /// Performs all stages in the `Vec`
    /// Checks after every stage if state wants to stop
    /// and returns an [`Error::ShuttingDown`] if so
    fn perform_all(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        testcase_id: &TestcaseId,
    ) -> Result<(), Error> {
        self.iter_mut().try_for_each(|stage| {
            stage.perform(fuzzer, executor, rand, state, rt_handle, testcase_id)
        })
    }
}

// static mut CLOSURE_STAGE_ID: usize = 0;
// /// The name for closure stage
// pub static CLOSURE_STAGE_NAME: &str = "closure";
//
// /// A [`Stage`] that will call a closure
// #[derive(Debug)]
// pub struct ClosureStage<CB, E, Z> {
//     name: Cow<'static, str>,
//     closure: CB,
//     phantom: PhantomData<(C, E, Z)>,
// }
//
// impl<CB, E, Z> Named for ClosureStage<CB, E, Z> {
//     fn name(&self) -> &Cow<'static, str> {
//         &self.name
//     }
// }
//
// impl<CB, E, S, W, Z> Stage<E, S, W, Z> for ClosureStage<CB, E, Z>
// where
//     CB: FnMut(&mut Z, &mut E, &mut S, &mut C) -> Result<(), Error>,
// {
//     fn perform(
//         &mut self,
//         fuzzer: &mut Z,
//         executor: &mut E,
//         state: &mut S,
//         controller: &mut C,
//     ) -> Result<(), Error> {
//         (self.closure)(fuzzer, executor, state, controller)
//     }
// }
//
// impl<CB, E, EM, S, W, Z> Restartable<S> for ClosureStage<CB, E, EM, Z> {
//     #[inline]
//     fn should_restart(&mut self, state: &mut S) -> Result<bool, Error> {
//         // There's no restart safety in the content of the closure.
//         // don't restart
//         RetryCountRestartHelper::no_retry(state, &self.name)
//     }
//
//     #[inline]
//     fn clear_progress(&mut self, state: &mut S) -> Result<(), Error> {
//         RetryCountRestartHelper::clear_progress(state, &self.name)
//     }
// }
//
// /// A stage that takes a closure
// impl<CB, E, EM, Z> ClosureStage<CB, E, EM, Z> {
//     /// Create a new [`ClosureStage`]
//     #[must_use]
//     pub fn new(closure: CB) -> Self {
//         // unsafe but impossible that you create two threads both instantiating this instance
//         let stage_id = unsafe {
//             let ret = CLOSURE_STAGE_ID;
//             CLOSURE_STAGE_ID += 1;
//             ret
//         };
//         Self {
//             name: Cow::Owned(CLOSURE_STAGE_NAME.to_owned() + ":" + stage_id.to_string().as_ref()),
//             closure,
//             phantom: PhantomData,
//         }
//     }
// }

// /// Progress which permits a fixed amount of resumes per round of fuzzing. If this amount is ever
// /// exceeded, the input will no longer be executed by this stage.
// #[derive(Clone, Deserialize, Serialize, Debug)]
// pub struct RetryCountRestartHelper {
//     tries_remaining: Option<usize>,
//     skipped: HashSet<CorpusId>,
// }
//
// impl_serdeany!(RetryCountRestartHelper);
//
// impl RetryCountRestartHelper {
//     /// Don't allow restart
//     pub fn no_retry<S>(state: &mut S, name: &str) -> Result<bool, Error>
//     where
//         S: HasNamedMetadata + HasCurrentCorpusId,
//     {
//         Self::should_restart(state, name, 1)
//     }
//
//     /// Initializes (or counts down in) the progress helper, giving it the amount of max retries
//     ///
//     /// Returns `true` if the stage should run
//     pub fn should_restart<S>(state: &mut S, name: &str, max_retries: usize) -> Result<bool, Error>
//     where
//         S: HasNamedMetadata + HasCurrentCorpusId,
//     {
//         let corpus_id = state.current_corpus_id()?.ok_or_else(|| {
//             Error::illegal_state(
//                 "No current_corpus_id set in State, but called RetryCountRestartHelper::should_skip",
//             )
//         })?;
//
//         let initial_tries_remaining = max_retries + 1;
//         let metadata = state.named_metadata_or_insert_with(name, || Self {
//             tries_remaining: Some(initial_tries_remaining),
//             skipped: HashSet::new(),
//         });
//         let tries_remaining = metadata
//             .tries_remaining
//             .unwrap_or(initial_tries_remaining)
//             .checked_sub(1)
//             .ok_or_else(|| {
//                 Error::illegal_state(
//                     "Attempted further retries after we had already gotten to none remaining.",
//                 )
//             })?;
//
//         metadata.tries_remaining = Some(tries_remaining);
//
//         Ok(if tries_remaining == 0 {
//             metadata.skipped.insert(corpus_id);
//             false
//         } else if metadata.skipped.contains(&corpus_id) {
//             // skip this testcase, we already retried it often enough...
//             false
//         } else {
//             true
//         })
//     }
//
//     /// Clears the progress
//     pub fn clear_progress<S>(state: &mut S, name: &str) -> Result<(), Error>
//     where
//         S: HasNamedMetadata,
//     {
//         state.named_metadata_mut::<Self>(name)?.tries_remaining = None;
//         Ok(())
//     }
// }

// impl_serdeany!(ExecutionCountRestartHelperMetadata);

// /// `SerdeAny` metadata used to keep track of executions since start for a given stage.
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct ExecutionCountRestartHelperMetadata {
//     /// How many executions we had when we started this stage initially (this round)
//     started_at_execs: u64,
// }
//
// /// A tool shed of functions to be used for stages that try to run for `n` iterations.
// ///
// /// # Note
// /// This helper assumes resumable mutational stages are not nested.
// /// If you want to nest them, you will have to switch all uses of `metadata` in this helper to `named_metadata` instead.
// #[derive(Debug, Default, Clone)]
// pub struct ExecutionCountRestartHelper {
//     /// At what exec count this Stage was started (cache)
//     /// Only used as cache for the value stored in [`MutationalStageMetadata`].
//     started_at_execs: Option<u64>,
// }
//
// impl ExecutionCountRestartHelper {
//     /// Create a new [`ExecutionCountRestartHelperMetadata`]
//     #[must_use]
//     pub fn new() -> Self {
//         Self {
//             started_at_execs: None,
//         }
//     }
//
//     /// The execs done since start of this [`Stage`]/helper
//     pub fn execs_since_progress_start<S>(&mut self, state: &mut S, name: &str) -> Result<u64, Error>
//     where
//         S: HasNamedMetadata + HasExecutions,
//     {
//         let started_at_execs = if let Some(started_at_execs) = self.started_at_execs {
//             started_at_execs
//         } else {
//             state
//                 .named_metadata::<ExecutionCountRestartHelperMetadata>(name)
//                 .map(|x| {
//                     self.started_at_execs = Some(x.started_at_execs);
//                     x.started_at_execs
//                 })
//                 .map_err(|err| {
//                     Error::illegal_state(format!(
//                         "The ExecutionCountRestartHelperMetadata should have been set at this point - {err}"
//                     ))
//                 })?
//         };
//         Ok(state.executions() - started_at_execs)
//     }
//
//     /// Initialize progress for the stage this wrapper wraps.
//     pub fn should_restart<S>(&mut self, state: &mut S, name: &str) -> Result<bool, Error>
//     where
//         S: HasNamedMetadata + HasExecutions,
//     {
//         let executions = *state.executions();
//         let metadata =
//             state.named_metadata_or_insert_with(name, || ExecutionCountRestartHelperMetadata {
//                 started_at_execs: executions,
//             });
//         self.started_at_execs = Some(metadata.started_at_execs);
//         Ok(true)
//     }
//
//     /// Clear progress for the stage this wrapper wraps.
//     pub fn clear_progress<S>(&mut self, state: &mut S, name: &str) -> Result<(), Error>
//     where
//         S: HasNamedMetadata,
//     {
//         self.started_at_execs = None;
//         let _metadata = state.remove_named_metadata::<ExecutionCountRestartHelperMetadata>(name);
//         debug_assert!(
//             _metadata.is_some(),
//             "Called clear_progress, but should_restart was not called before (or did mutational stages get nested?)"
//         );
//         Ok(())
//     }
// }
//
// #[cfg(test)]
// mod test {
//     use alloc::borrow::Cow;
//
//     use libafl_bolts::{Error, Named};
//
//     use crate::{
//         corpus::{Corpus, Testcase},
//         inputs::NopInput,
//         stages::RetryCountRestartHelper,
//         states::{HasCorpus, StdState},
//     };
//
//     /// Test to test retries in stages
//     #[test]
//     fn test_tries_progress() -> Result<(), Error> {
//         struct StageWithOneTry;
//
//         impl Named for StageWithOneTry {
//             fn name(&self) -> &Cow<'static, str> {
//                 static NAME: Cow<'static, str> = Cow::Borrowed("TestStage");
//                 &NAME
//             }
//         }
//
//         // # Safety
//         // No concurrency per testcase
//         #[cfg(any(not(feature = "serdeany_autoreg"), miri))]
//         unsafe {
//             RetryCountRestartHelper::register();
//         }
//
//         let mut state = StdState::nop()?;
//         let stage = StageWithOneTry;
//
//         let corpus_id = state.corpus_mut().add(Testcase::new(NopInput {}))?;
//
//         state.set_corpus_id(corpus_id)?;
//
//         for _ in 0..10 {
//             // used normally, no retries means we never skip
//             assert!(RetryCountRestartHelper::should_restart(
//                 &mut state,
//                 stage.name(),
//                 1
//             )?);
//             RetryCountRestartHelper::clear_progress(&mut state, stage.name())?;
//         }
//
//         for _ in 0..10 {
//             // used normally, only one retry means we never skip
//             assert!(RetryCountRestartHelper::should_restart(
//                 &mut state,
//                 stage.name(),
//                 2
//             )?);
//             assert!(RetryCountRestartHelper::should_restart(
//                 &mut state,
//                 stage.name(),
//                 2
//             )?);
//             RetryCountRestartHelper::clear_progress(&mut state, stage.name())?;
//         }
//
//         assert!(RetryCountRestartHelper::should_restart(
//             &mut state,
//             stage.name(),
//             2
//         )?);
//         // task failed, let's resume
//         // we still have one more try!
//         assert!(RetryCountRestartHelper::should_restart(
//             &mut state,
//             stage.name(),
//             2
//         )?);
//
//         // task failed, let's resume
//         // out of retries, so now we skip
//         assert!(!RetryCountRestartHelper::should_restart(
//             &mut state,
//             stage.name(),
//             2
//         )?);
//         RetryCountRestartHelper::clear_progress(&mut state, stage.name())?;
//
//         // we previously exhausted this testcase's retries, so we skip
//         assert!(!RetryCountRestartHelper::should_restart(
//             &mut state,
//             stage.name(),
//             2
//         )?);
//         RetryCountRestartHelper::clear_progress(&mut state, stage.name())?;
//
//         Ok(())
//     }
// }
