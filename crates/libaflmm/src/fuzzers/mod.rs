//! The `Fuzzer` is the main struct for a fuzz campaign.

use crate::{
    Error, corpus::testcase::TestcaseId, executors::ExitKind, generators::Generator, inputs::Input,
    runtimes::RuntimeHandle,
};
use alloc::{collections::VecDeque, string::ToString};
use core::fmt::{self, Debug};
use libaflmm_core::{Result, empty};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

pub mod standard;
pub use standard::{StdFuzzer, StdFuzzerBuilder};

pub mod hooks;
pub use hooks::{CalibrationHook, FuzzerHook, FuzzerHooksTuple};

/// The main fuzzer trait.
pub trait Fuzzer<E, I, R, S, ST, W> {
    /// Fuzz for a single stage iteration.
    ///
    /// Note: An iteration represents a complete run of every stage.
    /// Therefore, it does not mean that the harness is executed for once,
    /// because each stage could run the harness for multiple times
    fn fuzz_one(
        &mut self,
        stages: &mut ST,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<FuzzerOutcome>;

    /// Fuzz forever
    fn fuzz_loop(
        &mut self,
        stages: &mut ST,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        loop {
            if let FuzzerOutcome::Idle = self.fuzz_one(stages, rand, state, rt_handle)? {
                return Err(empty!(
                    "The scheduler is empty, which often indicates the target is incorrectly instrumented. \
                    Set the `allow_empty_scheduler` option to allow this behaviour."
                ));
            }
        }
    }

    /// Fuzz until the fuzzing loop gets idle.
    ///
    /// It typically happens when the scheduler is empty and no testcases remain to synchronize.
    ///
    /// It's more usual to use [`Self::fuzz_loop`] instead.
    /// This variant makes sense if the fuzzer is not supposed to run indefinitely.
    fn fuzz_loop_until_idle(
        &mut self,
        stages: &mut ST,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        loop {
            if let FuzzerOutcome::Idle = self.fuzz_one(stages, rand, state, rt_handle)? {
                return Ok(());
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
    ) -> Result<FuzzerOutcome> {
        if iters == 0 {
            return Err(Error::illegal_argument(
                "Cannot fuzz for 0 iterations!".to_string(),
            ));
        }

        for _ in 0..iters {
            if let FuzzerOutcome::Idle = self.fuzz_one(stages, rand, state, rt_handle)? {
                return Ok(FuzzerOutcome::Idle);
            }
        }

        Ok(FuzzerOutcome::Finished)
    }
}

/// Evaluate an input modifying the state of the fuzzer
pub trait Evaluator<E, I, S, W> {
    /// Runs the input and triggers observers and feedback.
    /// Returns the evaluation outcome: in which corpus is was potentially added and the optionally resulting [`TestcaseId`]
    fn evaluate_input(
        &mut self,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        input: &I,
    ) -> Result<EvaluationResult>;
}

pub trait Loader<I, S, W> {
    /// Load inputs from `inputs_fn`, then return.
    /// Inputs will be evaluated following the usual fuzzing pipeline.
    ///
    /// It must be able to resume if the runtime restarts.
    ///
    /// Returns every loaded input, and their evaluation result.
    fn load(
        &mut self,
        inputs_fn: impl FnOnce(&mut S) -> Result<VecDeque<I>>,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<Vec<LoadResult<I>>>;

    /// Load an input
    fn load_input(
        &mut self,
        input: impl Into<I>,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<Vec<LoadResult<I>>> {
        self.load_inputs([input], state, rt_handle)
    }

    /// Load inputs
    fn load_inputs(
        &mut self,
        inputs: impl IntoIterator<Item = impl Into<I>>,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<Vec<LoadResult<I>>> {
        self.load(
            move |_| Ok(inputs.into_iter().map(Into::into).collect()),
            state,
            rt_handle,
        )
    }

    /// Load an input from a file
    fn load_file(
        &mut self,
        path: impl AsRef<Path>,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<Vec<LoadResult<I>>>
    where
        I: Input,
    {
        self.load_files([path], state, rt_handle)
    }

    /// Load inputs from each file
    fn load_files(
        &mut self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<Vec<LoadResult<I>>>
    where
        I: Input,
    {
        self.load(
            move |_| paths.into_iter().map(I::from_file).collect(),
            state,
            rt_handle,
        )
    }

    /// Load all inputs in a directory (recursively)
    /// It will error out if the directory is empty.
    fn load_dir(
        &mut self,
        dir: impl AsRef<Path>,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<Vec<LoadResult<I>>>
    where
        I: Input,
    {
        self.load(
            move |_| {
                let files = list_files_rec(dir.as_ref())?;

                if files.is_empty() {
                    Err(empty!(
                        "No input loaded from the directory: {}.",
                        dir.as_ref().display()
                    ))
                } else {
                    let inputs: VecDeque<I> =
                        files.into_iter().map(I::from_file).collect::<Result<_>>()?;
                    Ok(inputs)
                }
            },
            state,
            rt_handle,
        )
    }

    /// Load inputs generated by a generator
    fn load_generator<G: Generator<I, R, S>, R>(
        &mut self,
        generator: &mut G,
        rand: &mut R,
        num: usize,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<Vec<LoadResult<I>>> {
        self.load(
            move |state| {
                let mut vec = VecDeque::new();
                for _ in 0..num {
                    let input = generator.generate(rand, state)?;
                    vec.push_back(input);
                }

                Ok(vec)
            },
            state,
            rt_handle,
        )
    }
}

/// The outcome of a fuzzing run
pub enum FuzzerOutcome {
    /// The fuzzer finished a single fuzzing run, by running a scheduled testcase
    Finished,
    /// The fuzzer exited early because the scheduler was empty
    Idle,
}

/// The result of a fuzzer evaluation.
///
/// It tells with which [`ExitKind`] the [`Executor`](crate::executors::Executor) ended (normally, with a timeout, etc...)
/// and what [`Verdict`] the feedback gave.
#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    exit_kind: ExitKind,
    verdict: Verdict,
}

/// The verdict of a fuzzer evaluation
/// It basically tells what was the interest, and in which corpus it was ultimately stored.
#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub enum Verdict {
    /// No special input
    Uninteresting,
    /// This input has been stored in the corpus, with the given [`TestcaseId`]
    Corpus(TestcaseId),
    /// This input has been stored in the objective corpus, with the given [`TestcaseId`]
    Objective(TestcaseId),
}

/// Whether an input is interesting enough to be stored in a corpus.
///
/// It tells what decision the feedback made, and the corpus is not updated at this point.
#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub enum Interest {
    /// No special input
    Uninteresting,
    /// This input should be stored in the corpus
    Corpus,
    /// This input should be stored in the objective corpus
    Objective,
}

/// Result of a load
#[derive(Debug, Clone)]
pub struct LoadResult<I> {
    /// the input that got loaded
    input: I,
    /// the result of its evaluation
    result: EvaluationResult,
}

impl<I> LoadResult<I> {
    /// Create a new [`LoadResult`].
    #[must_use]
    pub fn new(input: I, result: EvaluationResult) -> Self {
        Self { input, result }
    }

    /// The [`TestcaseId`] it got, if it has been stored in a corpus.
    ///
    /// The verdict tells if it has been added to the corpus or the objective corpus.
    #[must_use]
    pub fn testcase_id(&self) -> Option<TestcaseId> {
        self.result.testcase_id()
    }

    /// The ran [`Input`].
    #[must_use]
    pub fn input(&self) -> &I {
        &self.input
    }

    /// The resulting [`EvaluationResult`].
    #[must_use]
    pub fn result(&self) -> &EvaluationResult {
        &self.result
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Verdict::Uninteresting => write!(f, "Uninteresting"),
            Verdict::Corpus(_) => write!(f, "Corpus"),
            Verdict::Objective(_) => write!(f, "Objective"),
        }
    }
}

/// List all files in a directory (recursively).
///
/// It follows symlinks, and it makes sure to only visit each file / dir once.
/// Input paths are returned sorted, to keep it deterministic between fuzzing runs.
fn list_files_rec(dir: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
    let mut dirs_to_visit = vec![fs::canonicalize(dir)?];
    let mut visited_dirs = HashSet::new();
    let mut files = Vec::new();

    while let Some(dir) = dirs_to_visit.pop() {
        if !visited_dirs.insert(dir.clone()) {
            continue;
        }

        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            let md = fs::metadata(&path)?;

            if md.is_dir() {
                dirs_to_visit.push(fs::canonicalize(path)?);
            } else if md.is_file() {
                files.push(fs::canonicalize(path)?);
            }
        }
    }

    files.sort_unstable();
    files.dedup();

    Ok(files)
}

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
        matches!(self.verdict, Verdict::Objective(_))
    }

    /// Is the [`EvaluationResult`] corpus worthy?
    #[must_use]
    pub fn is_corpus_worthy(&self) -> bool {
        matches!(self.verdict, Verdict::Corpus(_))
    }

    /// The [`TestcaseId`] the input got, if it has been stored in a corpus.
    #[must_use]
    pub fn testcase_id(&self) -> Option<TestcaseId> {
        match self.verdict {
            Verdict::Corpus(testcase_id) | Verdict::Objective(testcase_id) => Some(testcase_id),
            Verdict::Uninteresting => None,
        }
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

/// A [`NopFuzzer`] that does nothing
#[derive(Debug, Copy, Clone)]
pub struct NopFuzzer;

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
    fn fuzz_one(
        &mut self,
        _stages: &mut ST,
        _rand: &mut R,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<FuzzerOutcome> {
        unimplemented!("NopFuzzer cannot fuzz");
    }
}
