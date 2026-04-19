//! The `Fuzzer` is the main struct for a fuzz campaign.

#[cfg(feature = "std")]
use alloc::borrow::Cow;
use alloc::{string::ToString, vec::Vec};
use core::{fmt::Debug, time::Duration};
#[cfg(feature = "std")]
use core::{hash::Hash, marker::PhantomData};

#[cfg(feature = "std")]
use fastbloom::BloomFilter;
#[cfg(feature = "std")]
use libafl_bolts::impl_serdeany;
use libafl_bolts::{current_time, tuples::MatchName};
#[cfg(feature = "std")]
use serde::Deserialize;
use serde::{Serialize, de::DeserializeOwned};

#[cfg(feature = "introspection")]
use crate::monitors::stats::PerfFeature;

use crate::{
    Error,
    corpus::{Corpus, Testcase, TestcaseId, schedulers::Scheduler},
    executors::{Executor, ExitKind},
    feedbacks::Feedback,
    inputs::Input,
    observers::ObserversTuple,
    runtimes::RuntimeHandle,
    stages::StagesTuple,
};

pub mod std;
pub use std::*;

/// Holds an feedback
pub trait HasFeedback {
    /// The feedback type
    type Feedback;

    /// The feedback
    fn feedback(&self) -> &Self::Feedback;

    /// The feedback (mutable)
    fn feedback_mut(&mut self) -> &mut Self::Feedback;
}

/// Holds an objective feedback
pub trait HasObjective {
    /// The type of the [`Feedback`] used to find objectives for this fuzzer
    type Objective;

    /// The objective feedback
    fn objective(&self) -> &Self::Objective;

    /// The objective feedback (mutable)
    fn objective_mut(&mut self) -> &mut Self::Objective;
}

/// Evaluates if an input is interesting using the feedback
pub trait ExecutionProcessor<I, OT, S> {
    /// Process `ExecuteInputResult`. Add to corpus, objective or ignore
    fn process_execution(
        &mut self,
        state: &mut S,
        input: &I,
        eval_res: &EvaluationResult,
        observers: &OT,
    ) -> Result<Option<()>, Error>;
}

/// Evaluate an input modifying the state of the fuzzer
pub trait Evaluator<E, I, S> {
    /// Runs the input and triggers observers and feedback
    /// Retusn the "raw" result of the execution.
    /// No post-processing is performed.
    fn execute_input(
        &mut self,
        state: &mut S,
        executor: &mut E,
        input: &I,
    ) -> Result<ExitKind, Error>;

    /// Runs the input and triggers observers and feedback,
    /// returns if is interesting an (option) the index of the new [`Testcase`] in the corpus
    fn evaluate_input(
        &mut self,
        state: &mut S,
        executor: &mut E,
        input: &I,
    ) -> Result<EvaluationResult, Error>;
}

/// The main fuzzer trait.
pub trait Fuzzer<E, I, S, ST> {
    fn init(&mut self, stages: &mut ST, executor: &mut E, state: &mut S) -> Result<(), Error>;

    /// Fuzz for a single iteration.
    ///
    /// (Note: An iteration represents a complete run of every stage.
    /// Therefore, it does not mean that the harness is executed for once,
    /// because each stage could run the harness for multiple times)
    fn fuzz_one(&mut self, stages: &mut ST, executor: &mut E, state: &mut S) -> Result<(), Error>;

    /// Fuzz forever (or until stopped)
    fn fuzz_loop(&mut self, stages: &mut ST, executor: &mut E, state: &mut S) -> Result<(), Error>;

    /// Fuzz for n iterations.
    ///
    /// (Note: An iteration represents a complete run of every stage.
    /// therefore the number n is not always equal to the number of the actual harness executions,
    /// because each stage could run the harness for multiple times)
    fn fuzz_loop_for(
        &mut self,
        stages: &mut ST,
        executor: &mut E,
        state: &mut S,
        iters: u64,
    ) -> Result<(), Error>;
}

#[derive(Debug, PartialEq, Eq)]
pub struct EvaluationResult {
    exit_kind: ExitKind,
    verdict: Verdict,
}

/// The result of harness execution
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// No special input
    Uninteresting,
    /// This input should be stored in the corpus
    Corpus(TestcaseId),
    /// This input leads to an objective
    Objective(TestcaseId),
}

/// A [`NopFuzzer`] that does nothing
#[derive(Debug, Copy, Clone)]
pub struct NopFuzzer;

impl EvaluationResult {
    pub fn new(exit_kind: ExitKind, verdict: Verdict) -> Self {
        Self { exit_kind, verdict }
    }

    pub fn is_corpus_worthy(&self) -> bool {
        match self.verdict {
            Verdict::Uninteresting => false,
            _ => true,
        }
    }

    pub fn vertict(&self) -> &Verdict {
        &self.verdict
    }

    pub fn exit_kind(&self) -> ExitKind {
        self.exit_kind
    }
}

impl NopFuzzer {
    /// Creates a new [`NopFuzzer`]
    #[must_use]
    pub fn new() -> NopFuzzer {
        Self
    }
}

impl Default for NopFuzzer {
    fn default() -> Self {
        Self::new()
    }
}

impl<E, I, S, ST> Fuzzer<E, I, S, ST> for NopFuzzer {
    fn init(&mut self, stages: &mut ST, executor: &mut E, state: &mut S) -> Result<(), Error> {
        unimplemented!("NopFuzzer cannot fuzz");
    }

    fn fuzz_one(
        &mut self,
        _stages: &mut ST,
        _executor: &mut E,
        _state: &mut S,
    ) -> Result<(), Error> {
        unimplemented!("NopFuzzer cannot fuzz");
    }

    fn fuzz_loop(
        &mut self,
        _stages: &mut ST,
        _executor: &mut E,
        _state: &mut S,
    ) -> Result<(), Error> {
        unimplemented!("NopFuzzer cannot fuzz");
    }

    fn fuzz_loop_for(
        &mut self,
        _stages: &mut ST,
        _executor: &mut E,
        _state: &mut S,
        _iters: u64,
    ) -> Result<(), Error> {
        unimplemented!("NopFuzzer cannot fuzz");
    }
}
