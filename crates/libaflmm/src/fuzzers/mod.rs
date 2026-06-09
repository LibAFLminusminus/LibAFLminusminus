//! The `Fuzzer` is the main struct for a fuzz campaign.

use crate::{Error, executors::ExitKind, runtimes::RuntimeHandle};
use alloc::string::ToString;
use core::fmt::Debug;
use libaflmm_core::Result;

pub mod standard;
pub use standard::{StdFuzzer, StdFuzzerBuilder};

pub mod hooks;
pub use hooks::{CalibrationHook, CustomNameHook, FuzzerHook, FuzzerHooksTuple};

/// Evaluate an input modifying the state of the fuzzer
pub trait Evaluator<E, I, S, W> {
    /// Runs the input and triggers observers and feedback,
    /// returns if is interesting an (option) the index of the new [`Testcase`] in the corpus
    fn evaluate_input(
        &mut self,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        input: &I,
    ) -> Result<EvaluationResult>;
}

/// The main fuzzer trait.
pub trait Fuzzer<E, I, R, S, ST, W> {
    /// Initialize the fuzzer
    ///
    /// It is used to initialize every structure just before fuzzing starts.
    /// It is preferred to not call it manually and let the fuzzer call that directly
    /// through the fuzz_* functions (except of course for `fuzz_one_noinit`).
    ///
    /// It should be possible to call this functions multiple times, and it is of the
    /// fuzzer's responsibility to make sure it can be done without raising errors or
    /// provoking undefined behavior.
    fn init(
        &mut self,
        stages: &mut ST,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()>;

    /// Returns true if the [`Fuzzer`] is initialized and ready to run, false otherwise.
    fn is_initialized(&self) -> bool;

    /// Fuzz for a single iteration.
    ///
    /// (Note: An iteration represents a complete run of every stage.
    /// Therefore, it does not mean that the harness is executed for once,
    /// because each stage could run the harness for multiple times)
    ///
    /// # Safety
    ///
    /// The fuzzer must be initialized with [`Self::init`] before running this function.
    /// It will not be checked for performance reason.
    unsafe fn fuzz_one_initialized(
        &mut self,
        stages: &mut ST,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()>;

    /// Fuzz for a single iteration.
    ///
    /// (Note: An iteration represents a complete run of every stage.
    /// Therefore, it does not mean that the harness is executed for once,
    /// because each stage could run the harness for multiple times)
    fn fuzz_one(
        &mut self,
        stages: &mut ST,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        if !self.is_initialized() {
            return Err(Error::runtime(
                "Fuzzer not initialized. Run Fuzzer::init after creating the fuzzer.",
            ));
        }

        unsafe { self.fuzz_one_initialized(stages, rand, state, rt_handle) }
    }

    /// Fuzz forever (or until stopped)
    fn fuzz_loop(
        &mut self,
        stages: &mut ST,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        if !self.is_initialized() {
            return Err(Error::runtime(
                "Fuzzer not initialized. Run Fuzzer::init after creating the fuzzer.",
            ));
        }

        loop {
            unsafe {
                self.fuzz_one_initialized(stages, rand, state, rt_handle)?;
            }
        }
    }

    /// Fuzz for n iterations.
    ///
    /// (Note: An iteration represents a complete run of every stage.
    /// therefore the number n is not always equal to the number of the actual harness executions,
    /// because each stage could run the harness for multiple times)
    fn fuzz_loop_for(
        &mut self,
        stages: &mut ST,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        iters: u64,
    ) -> Result<()> {
        if !self.is_initialized() {
            return Err(Error::runtime(
                "Fuzzer not initialized. Run Fuzzer::init after creating the fuzzer.",
            ));
        }

        if iters == 0 {
            return Err(Error::illegal_argument(
                "Cannot fuzz for 0 iterations!".to_string(),
            ));
        }

        for _ in 0..iters {
            unsafe {
                self.fuzz_one_initialized(stages, rand, state, rt_handle)?;
            }
        }

        Ok(())
    }
}

/// The result of a fuzzer evaluation.
///
/// It tells with which [`ExitKind`] the [`Executor`] ended (normally, with a timeout, etc...)
/// and what [`Verdict`] the feedback gave.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct EvaluationResult {
    exit_kind: ExitKind,
    verdict: Verdict,
}

/// The result of harness execution
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Verdict {
    /// No special input
    Uninteresting,
    /// This input should be stored in the corpus
    Corpus,
    /// This input leads to an objective
    Objective,
}

/// A [`NopFuzzer`] that does nothing
#[derive(Debug, Copy, Clone)]
pub struct NopFuzzer;

impl EvaluationResult {
    /// Get a new [`EvaluationResult`].
    #[must_use]
    pub fn new(exit_kind: ExitKind, verdict: Verdict) -> Self {
        Self { exit_kind, verdict }
    }

    /// [`EvaluationResult`] when an entry is deemed not interesting.
    #[must_use]
    pub fn not_interesting() -> Self {
        Self {
            exit_kind: ExitKind::Ok,
            verdict: Verdict::Uninteresting,
        }
    }

    /// Is the [`EvaluationResult`] objective worthy?
    #[must_use]
    pub fn is_objective_worthy(&self) -> bool {
        matches!(self.verdict, Verdict::Objective)
    }

    /// Is the [`EvaluationResult`] corpus worthy?
    #[must_use]
    pub fn is_corpus_worthy(&self) -> bool {
        matches!(self.verdict, Verdict::Corpus)
    }

    /// Get the [`EvaluationResult`]'s [`Verdict`].
    #[must_use]
    pub fn vertict(&self) -> &Verdict {
        &self.verdict
    }

    /// Get the [`EvaluationResult`]'s [`ExitKind`].
    #[must_use]
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

impl<E, I, R, S, ST, W> Fuzzer<E, I, R, S, ST, W> for NopFuzzer {
    fn init(
        &mut self,
        _stages: &mut ST,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        unimplemented!("NopFuzzer cannot fuzz");
    }

    fn is_initialized(&self) -> bool {
        false
    }

    unsafe fn fuzz_one_initialized(
        &mut self,
        _stages: &mut ST,
        _rand: &mut R,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        unimplemented!("NopFuzzer cannot fuzz");
    }
}
