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

pub mod filter;
pub use filter::*;

#[cfg(feature = "introspection")]
use crate::monitors::stats::PerfFeature;
#[cfg(feature = "std")]
use crate::monitors::stats::{AggregatorOps, UserStats, UserStatsValue};
use crate::{
    Error, HasMetadata, Resolver,
    corpus::{Corpus, CorpusId, HasCurrentCorpusId, HasTestcase, Testcase},
    events::{
        Event, EventConfig, EventFirer, EventReceiver, EventWithStats, ProgressReporter,
        SendExiting,
    },
    executors::{Executor, ExitKind, HasObservers},
    feedbacks::Feedback,
    inputs::{BytesInputConverter, Input, ToBytesInputConverter, ToTargetBytesConverter},
    mark_feature_time,
    observers::ObserversTuple,
    schedulers::Scheduler,
    stages::StagesTuple,
    start_timer,
    state::{
        HasCorpus, HasCurrentStageId, HasCurrentTestcase, HasExecutions, HasImported,
        HasLastFoundTime, HasLastReportTime, HasSolutions, MaybeHasClientPerfMonitor, Stoppable,
    },
};

/// Send a monitor update all 15 (or more) seconds
pub(crate) const STATS_TIMEOUT_DEFAULT: Duration = Duration::from_secs(15);

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

/// Can convert input to another type
pub trait HasToTargetBytesConverter {
    /// The converter type
    type Converter;

    /// the converter, converting the input to target bytes.
    fn target_bytes_converter(&self) -> &Self::Converter;
    /// the converter, converting the input to target bytes (mut).
    fn target_bytes_converter_mut(&mut self) -> &mut Self::Converter;
}

/// Blanket implementation to shorthand-call [`ToTargetBytesConverter::convert_to_target_bytes`] on the fuzzer directly.
impl<I, S, T> ToTargetBytesConverter<I, S> for T
where
    T: HasToTargetBytesConverter,
    T::Converter: ToTargetBytesConverter<I, S>,
{
    fn convert_to_target_bytes<'a>(
        &mut self,
        state: &mut S,
        input: &'a I,
    ) -> libafl_bolts::ownedref::OwnedSlice<'a, u8> {
        self.target_bytes_converter_mut()
            .convert_to_target_bytes(state, input)
    }
}

/// Evaluates if an input is interesting using the feedback
pub trait ExecutionProcessor<I, OT, S> {
    /// Process `ExecuteInputResult`. Add to corpus, solution or ignore
    fn process_execution(
        &mut self,
        state: &mut S,
        input: &I,
        exec_res: &ExecuteInputResult,
        exit_kind: &ExitKind,
        observers: &OT,
    ) -> Result<Option<CorpusId>, Error>;
}

/// Evaluate an input modifying the state of the fuzzer
pub trait Evaluator<E, I, S> {
    /// Runs the input if it was (likely) not previously run and triggers observers and feedback and adds the input to the previously executed list
    /// if it is interesting, returns an (option) the index of the new [`Testcase`] in the corpus
    fn evaluate_filtered(
        &mut self,
        state: &mut S,
        executor: &mut E,
        input: &I,
    ) -> Result<(ExecuteInputResult, Option<CorpusId>), Error>;

    /// Runs the input and triggers observers and feedback,
    /// returns if is interesting an (option) the index of the new [`Testcase`] in the corpus
    fn evaluate_input(
        &mut self,
        state: &mut S,
        executor: &mut E,
        input: &I,
    ) -> Result<(ExecuteInputResult, Option<CorpusId>), Error>;
}

/// The main fuzzer trait.
pub trait Fuzzer<E, I, S, ST> {
    fn init(&mut self, stages: &mut ST, executor: &mut E, state: &mut S) -> Result<(), Error>;

    /// Fuzz for a single iteration.
    /// Returns the index of the last fuzzed corpus item.
    /// (Note: An iteration represents a complete run of every stage.
    /// Therefore, it does not mean that the harness is executed for once,
    /// because each stage could run the harness for multiple times)
    ///
    /// If you use this fn in a restarting scenario to only run for `n` iterations,
    /// before exiting, make sure you call `event_mgr.on_restart(&mut state)?;`.
    /// This way, the state will be available in the next, respawned, iteration.
    fn fuzz_one(
        &mut self,
        stages: &mut ST,
        executor: &mut E,
        state: &mut S,
    ) -> Result<CorpusId, Error>;

    /// Fuzz forever (or until stopped)
    fn fuzz_loop(&mut self, stages: &mut ST, executor: &mut E, state: &mut S) -> Result<(), Error>;

    /// Fuzz for n iterations.
    /// Returns the index of the last fuzzed corpus item.
    /// (Note: An iteration represents a complete run of every stage.
    /// therefore the number n is not always equal to the number of the actual harness executions,
    /// because each stage could run the harness for multiple times)
    ///
    /// If you use this fn in a restarting scenario to only run for `n` iterations,
    /// before exiting, make sure you call `event_mgr.on_restart(&mut state)?;`.
    /// This way, the state will be available in the next, respawned, iteration.
    fn fuzz_loop_for(
        &mut self,
        stages: &mut ST,
        executor: &mut E,
        state: &mut S,
        iters: u64,
    ) -> Result<CorpusId, Error>;
}
/// The result of harness execution
#[derive(Debug, PartialEq, Eq)]
pub enum ExecuteInputResult {
    /// No special input
    None,
    /// This input should be stored in the corpus
    Corpus,
    /// This input leads to a solution
    Solution,
}

/// Your default fuzzer instance, for everyday use.
#[derive(Debug)]
pub struct StdFuzzer<CS, F, IC, IF, OF> {
    /// The scheduler used to schedule new testcases
    scheduler: CS,
    /// The [`Feedback`] that will store new testcases on if a run returns `is_interesting`.
    feedback: F,
    /// The [`Feedback`] that will store new testcases as solution (for example, a crash) if a run returns `is_interesting`.
    objective: OF,
    /// A converter that converts the input to bytes that can be sent to the target (for example, to a [`CommandExecutor`](crate::executors::CommandExecutor).
    target_bytes_converter: IC,
    /// The input filter that will filter out (not execute) certain inputs
    input_filter: IF,
    /// Handles whether to share objective testcases among nodes
    share_objectives: bool,
}

impl<CS, F, I, IC, IF, OF, S> HasScheduler<I, S> for StdFuzzer<CS, F, IC, IF, OF>
where
    CS: Scheduler<I, S>,
{
    type Scheduler = CS;

    fn scheduler(&self) -> &CS {
        &self.scheduler
    }

    fn scheduler_mut(&mut self) -> &mut CS {
        &mut self.scheduler
    }
}

impl<CS, F, IC, IF, OF> HasFeedback for StdFuzzer<CS, F, IC, IF, OF> {
    type Feedback = F;

    fn feedback(&self) -> &Self::Feedback {
        &self.feedback
    }

    fn feedback_mut(&mut self) -> &mut Self::Feedback {
        &mut self.feedback
    }
}

impl<CS, F, IC, IF, OF> HasObjective for StdFuzzer<CS, F, IC, IF, OF> {
    type Objective = OF;

    fn objective(&self) -> &OF {
        &self.objective
    }

    fn objective_mut(&mut self) -> &mut OF {
        &mut self.objective
    }
}

/// bunch of handy public functions
fn check_results<I, OT, S, Z>(
    fuzzer: &mut Z,
    state: &mut S,
    input: &I,
    observers: &OT,
    exit_kind: &ExitKind,
) -> Result<ExecuteInputResult, Error> {
    let mut res = ExecuteInputResult::None;

    #[cfg(not(feature = "introspection"))]
    let is_solution = fuzzer
        .objective_mut()
        .is_interesting(state, input, observers, exit_kind)?;

    #[cfg(feature = "introspection")]
    let is_solution = self
        .objective_mut()
        .is_interesting_introspection(state, input, observers, exit_kind)?;

    if is_solution {
        res = ExecuteInputResult::Solution;
    } else {
        #[cfg(not(feature = "introspection"))]
        let corpus_worthy = fuzzer
            .feedback_mut()
            .is_interesting(state, input, observers, exit_kind)?;
        #[cfg(feature = "introspection")]
        let corpus_worthy = self
            .feedback_mut()
            .is_interesting_introspection(state, input, observers, exit_kind)?;

        if corpus_worthy {
            res = ExecuteInputResult::Corpus;
        }
    }
    Ok(res)
}

fn evaluate_execution<I, OT, S, Z>(
    fuzzer: &mut Z,
    state: &mut S,
    input: &I,
    observers: &OT,
    exit_kind: &ExitKind,
    send_events: bool,
) -> Result<(ExecuteInputResult, Option<CorpusId>), Error> {
    let exec_res = fuzzer.check_results(state, input, observers, exit_kind)?;
    let corpus_id = fuzzer.process_execution(state, input, &exec_res, exit_kind, observers)?;

    if exec_res != ExecuteInputResult::None {
        *state.last_found_time_mut() = current_time();
    }
    Ok((exec_res, corpus_id))
}

/// Adds an input, even if it's not considered `interesting` by any of the executors
/// If you are using inprocess executor, be careful.
/// Your crash-causing testcase will *NOT* be added into the corpus (only to solution)
fn add_input<E, I, OT, S, Z>(
    fuzzer: &mut Z,
    state: &mut S,
    executor: &mut E,
    input: I,
) -> Result<CorpusId, Error> {
    *state.last_found_time_mut() = current_time();

    let exit_kind = fuzzer.execute_input(state, executor, &input)?;
    let observers = executor.observers();
    // Always consider this to be "interesting"
    let mut testcase = Testcase::from(input.clone());
    testcase.set_executions(*state.executions());

    // Maybe a solution
    #[cfg(not(feature = "introspection"))]
    let is_solution: bool =
        fuzzer
            .objective_mut()
            .is_interesting(state, &input, &*observers, &exit_kind)?;

    #[cfg(feature = "introspection")]
    let is_solution = self.objective_mut().is_interesting_introspection(
        state,
        &input,
        &*observers,
        &exit_kind,
    )?;

    if is_solution {
        #[cfg(feature = "track_hit_feedbacks")]
        self.objective_mut()
            .append_hit_feedbacks(testcase.hit_objectives_mut())?;
        fuzzer
            .objective_mut()
            .append_metadata(state, &*observers, &mut testcase)?;
        // we don't care about solution id
        let id = state.solutions_mut().add(testcase)?;

        return Ok(id);
    }

    // several is_interesting implementations collect some data about the run, later used in
    // append_metadata; we *must* invoke is_interesting here to collect it
    #[cfg(not(feature = "introspection"))]
    let _corpus_worthy =
        fuzzer
            .feedback_mut()
            .is_interesting(state, &input, &*observers, &exit_kind)?;

    #[cfg(feature = "introspection")]
    let _corpus_worthy =
        self.feedback_mut()
            .is_interesting_introspection(state, &input, &*observers, &exit_kind)?;

    #[cfg(feature = "track_hit_feedbacks")]
    fuzzer
        .feedback_mut()
        .append_hit_feedbacks(testcase.hit_feedbacks_mut())?;
    // Add the input to the main corpus
    fuzzer
        .feedback_mut()
        .append_metadata(state, &*observers, &mut testcase)?;
    let id = state.corpus_mut().add(testcase)?;
    fuzzer.scheduler_mut().on_add(state, id)?;

    Ok(id)
}

impl<CS, F, I, IC, IF, OF, OT, S> ExecutionProcessor<I, OT, S> for StdFuzzer<CS, F, IC, IF, OF>
where
    CS: Scheduler<I, S>,
    F: Feedback<EM, I, OT, S>,
    I: Input,
    OF: Feedback<EM, I, OT, S>,
    OT: ObserversTuple<I, S> + Serialize,
    S: HasCorpus<I>
        + MaybeHasClientPerfMonitor
        + HasExecutions
        + HasCurrentTestcase<I>
        + HasSolutions<I>
        + HasLastFoundTime
        + HasExecutions,
{
    /// Post process a testcase depending the testcase execution results
    /// returns corpus id if it put something into corpus (not solution)
    /// This code will not be reached by inprocess executor if crash happened.
    fn process_execution(
        &mut self,
        state: &mut S,
        input: &I,
        exec_res: &ExecuteInputResult,
        _exit_kind: &ExitKind,
        observers: &OT,
    ) -> Result<Option<CorpusId>, Error> {
        match exec_res {
            ExecuteInputResult::None => Ok(None),
            ExecuteInputResult::Corpus => {
                // Not a solution
                // Add the input to the main corpus
                let mut testcase = Testcase::from(input.clone());
                #[cfg(feature = "track_hit_feedbacks")]
                self.feedback_mut()
                    .append_hit_feedbacks(testcase.hit_feedbacks_mut())?;
                self.feedback_mut()
                    .append_metadata(state, observers, &mut testcase)?;
                let id = state.corpus_mut().add(testcase)?;
                self.scheduler_mut().on_add(state, id)?;

                Ok(Some(id))
            }
            ExecuteInputResult::Solution => {
                // The input is a solution, add it to the respective corpus
                let mut testcase = Testcase::from(input.clone());
                testcase.set_parent_id_optional(*state.corpus().current());
                if let Ok(mut tc) = state.current_testcase_mut() {
                    tc.found_objective();
                }
                #[cfg(feature = "track_hit_feedbacks")]
                self.objective_mut()
                    .append_hit_feedbacks(testcase.hit_objectives_mut())?;
                self.objective_mut()
                    .append_metadata(stateger, observers, &mut testcase)?;
                state.solutions_mut().add(testcase)?;

                Ok(None)
            }
        }
    }
}

impl<CS, E, F, I, IC, IF, OF, S> Evaluator<E, I, S> for StdFuzzer<CS, F, IC, IF, OF>
where
    CS: Scheduler<I, S>,
    E: HasObservers + Executor<I, S, Self>,
    E::Observers: MatchName + ObserversTuple<I, S> + Serialize,
    F: Feedback<I, E::Observers, S>,
    OF: Feedback<I, E::Observers, S>,
    S: HasCorpus<I>
        + HasSolutions<I>
        + MaybeHasClientPerfMonitor
        + HasCurrentTestcase<I>
        + HasLastFoundTime
        + HasExecutions,
    I: Input,
    IF: InputFilter<I, S>,
{
    fn evaluate_filtered(
        &mut self,
        state: &mut S,
        executor: &mut E,
        input: &I,
    ) -> Result<(ExecuteInputResult, Option<CorpusId>), Error> {
        if self.input_filter.should_execute(input, state)? {
            self.evaluate_input(state, executor, input)
        } else {
            Ok((ExecuteInputResult::None, None))
        }
    }

    /// Process one input, adding to the respective corpora if needed and firing the right events
    #[inline]
    fn evaluate_input(
        &mut self,
        state: &mut S,
        executor: &mut E,
        input: &I,
    ) -> Result<(ExecuteInputResult, Option<CorpusId>), Error> {
        self.evaluate_input_with_observers(state, executor, input, true)
    }
}

impl<CS, E, F, I, IC, IF, OF, S, ST> Fuzzer<E, I, S, ST> for StdFuzzer<CS, F, IC, IF, OF>
where
    CS: Scheduler<I, S>,
    E: HasObservers + Executor<I, S, Self>,
    E::Observers: DeserializeOwned + Serialize + ObserversTuple<I, S>,
    I: Input,
    F: Feedback<I, E::Observers, S>,
    OF: Feedback<I, E::Observers, S>,
    S: HasExecutions
        + HasMetadata
        + HasCorpus<I>
        + HasSolutions<I>
        + HasLastReportTime
        + HasLastFoundTime
        + HasImported
        + HasTestcase<I>
        + HasCurrentCorpusId
        + HasCurrentStageId
        + Stoppable
        + MaybeHasClientPerfMonitor,
    ST: StagesTuple<E, S, Self>,
{
    fn init(&mut self, stages: &mut ST, executor: &mut E, state: &mut S) -> Result<(), Error> {
        let mut resolver = Resolver::new();

        self.feedback.resolve(&mut resolver)?;
        self.objective.resolve(&mut resolver)?;
        stages.resolve(&mut resolver)?;
        state.resolve(&mut resolver)?;
        executor.resolve(&mut resolver)?;

        state.register_metadata(resolver)
    }

    fn fuzz_one(
        &mut self,
        stages: &mut ST,
        executor: &mut E,
        state: &mut S,
    ) -> Result<CorpusId, Error> {
        // Init timer for scheduler
        #[cfg(feature = "introspection")]
        state.introspection_stats_mut().start_timer();

        // Get the next index from the scheduler
        let id = if let Some(id) = state.current_corpus_id()? {
            id // we are resuming
        } else {
            let id = self.scheduler.next(state)?;
            state.set_corpus_id(id)?; // set up for resume
            id
        };

        // Mark the elapsed time for the scheduler
        #[cfg(feature = "introspection")]
        state.introspection_stats_mut().mark_scheduler_time();

        // Mark the elapsed time for the scheduler
        #[cfg(feature = "introspection")]
        state.introspection_stats_mut().reset_stage_index();

        // Execute all stages
        stages.perform_all(self, executor, state)?;

        self.process_events(state, executor)?;

        {
            if let Ok(mut testcase) = state.testcase_mut(id) {
                let scheduled_count = testcase.scheduled_count();
                // increase scheduled count, this was fuzz_level in afl
                testcase.set_scheduled_count(scheduled_count + 1);
            }
        }

        state.clear_corpus_id()?;

        if state.stop_requested() {
            state.discard_stop_request();
            return Err(Error::shutting_down());
        }

        Ok(id)
    }

    fn fuzz_loop(&mut self, stages: &mut ST, executor: &mut E, state: &mut S) -> Result<(), Error> {
        let monitor_timeout = STATS_TIMEOUT_DEFAULT;
        loop {
            self.fuzz_one(stages, executor, state)?;
        }
    }

    fn fuzz_loop_for(
        &mut self,
        stages: &mut ST,
        executor: &mut E,
        state: &mut S,
        iters: u64,
    ) -> Result<CorpusId, Error> {
        if iters == 0 {
            return Err(Error::illegal_argument(
                "Cannot fuzz for 0 iterations!".to_string(),
            ));
        }

        let mut ret = None;
        let monitor_timeout = STATS_TIMEOUT_DEFAULT;

        for _ in 0..iters {
            ret = Some(self.fuzz_one(stages, executor, state)?);
        }

        // If we assumed the fuzzer loop will always exit after this, we could do this here:
        // But as the state may grow to a few megabytes,
        // for now we won't, and the user has to do it (unless we find a way to do this on `Drop`).

        Ok(ret.unwrap())
    }
}

/// The builder for std fuzzer
#[derive(Debug)]
pub struct StdFuzzerBuilder<CS, F, IC, IF, OF> {
    /// The scheduler used to schedule new testcases
    scheduler: CS,
    /// The [`Feedback`] that will store new testcases on if a run returns `is_interesting`.
    feedback: F,
    /// The [`Feedback`] that will store new testcases as solution (for example, a crash) if a run returns `is_interesting`.
    objective: OF,
    /// A converter that converts the input to bytes that can be sent to the target (for example, to a [`CommandExecutor`](crate::executors::CommandExecutor).
    target_bytes_converter: IC,
    /// The input filter that will filter out (not execute) certain inputs
    input_filter: IF,
    /// Handles whether to share objective testcases among nodes
    share_objectives: bool,
}

impl StdFuzzerBuilder<(), (), BytesInputConverter, NopInputFilter, ()> {
    /// Creates a new [`StdFuzzerBuilder`] with default (nop) types.
    #[must_use]
    pub fn new() -> Self {
        Self {
            target_bytes_converter: BytesInputConverter::new(),
            input_filter: NopInputFilter,
            scheduler: (),
            feedback: (),
            objective: (),
            share_objectives: false,
        }
    }
}

impl Default for StdFuzzerBuilder<(), (), BytesInputConverter, NopInputFilter, ()> {
    fn default() -> Self {
        Self::new()
    }
}

impl<CS, F, IC, IF, OF> StdFuzzerBuilder<CS, F, IC, IF, OF> {
    /// Sets the converter to target bytes.
    /// The converter converts the input to bytes that can be sent to the target (for example, to a [`CommandExecutor`](crate::executors::CommandExecutor).
    #[must_use]
    pub fn target_bytes_converter<I, IC2>(
        self,
        target_bytes_converter: IC2,
    ) -> StdFuzzerBuilder<CS, F, ToBytesInputConverter<I, IC2>, IF, OF> {
        StdFuzzerBuilder {
            target_bytes_converter: ToBytesInputConverter::new(target_bytes_converter),
            input_filter: self.input_filter,
            scheduler: self.scheduler,
            feedback: self.feedback,
            objective: self.objective,
            share_objectives: self.share_objectives,
        }
    }
}

impl<CS, F, IC, IF, OF> StdFuzzerBuilder<CS, F, IC, IF, OF> {
    /// Set the input filter.
    /// The input filter will filter out (i.e., not execute) certain inputs.
    #[must_use]
    pub fn input_filter<IF2>(self, input_filter: IF2) -> StdFuzzerBuilder<CS, F, IC, IF2, OF> {
        StdFuzzerBuilder {
            target_bytes_converter: self.target_bytes_converter,
            input_filter,
            scheduler: self.scheduler,
            feedback: self.feedback,
            objective: self.objective,
            share_objectives: self.share_objectives,
        }
    }
}

impl<CS, F, IC, IF, OF> StdFuzzerBuilder<CS, F, IC, IF, OF> {
    /// Sets the scheduler used to schedule new testcases
    #[must_use]
    pub fn scheduler<CS2>(self, scheduler: CS2) -> StdFuzzerBuilder<CS2, F, IC, IF, OF> {
        StdFuzzerBuilder {
            target_bytes_converter: self.target_bytes_converter,
            input_filter: self.input_filter,
            scheduler,
            feedback: self.feedback,
            objective: self.objective,
            share_objectives: self.share_objectives,
        }
    }
}

impl<CS, F, IC, IF, OF> StdFuzzerBuilder<CS, F, IC, IF, OF> {
    /// Sets the feedback that will store new testcases on if a run returns `is_interesting`.
    #[must_use]
    pub fn feedback<F2>(self, feedback: F2) -> StdFuzzerBuilder<CS, F2, IC, IF, OF> {
        StdFuzzerBuilder {
            target_bytes_converter: self.target_bytes_converter,
            input_filter: self.input_filter,
            scheduler: self.scheduler,
            feedback,
            objective: self.objective,
            share_objectives: self.share_objectives,
        }
    }
}

impl<CS, F, IC, IF, OF> StdFuzzerBuilder<CS, F, IC, IF, OF> {
    /// Sets the feedback that will store new testcases as solution (for example, a crash) if a run returns `is_interesting`.
    #[must_use]
    pub fn objective<OF2>(self, objective: OF2) -> StdFuzzerBuilder<CS, F, IC, IF, OF2> {
        StdFuzzerBuilder {
            target_bytes_converter: self.target_bytes_converter,
            input_filter: self.input_filter,
            scheduler: self.scheduler,
            feedback: self.feedback,
            objective,
            share_objectives: self.share_objectives,
        }
    }
}

impl<CS, F, IC, IF, OF> StdFuzzerBuilder<CS, F, IC, IF, OF> {
    /// Sets whether to share objective testcases among nodes
    #[must_use]
    pub fn share_objectives(self, share_objectives: bool) -> StdFuzzerBuilder<CS, F, IC, IF, OF> {
        StdFuzzerBuilder {
            target_bytes_converter: self.target_bytes_converter,
            input_filter: self.input_filter,
            scheduler: self.scheduler,
            feedback: self.feedback,
            objective: self.objective,
            share_objectives,
        }
    }
}

impl<CS, F, IC, IF, OF> StdFuzzerBuilder<CS, F, IC, IF, OF> {
    /// Build a [`StdFuzzer`] from this builder.
    pub fn build(self) -> StdFuzzer<CS, F, IC, IF, OF> {
        StdFuzzer {
            target_bytes_converter: self.target_bytes_converter,
            input_filter: self.input_filter,
            scheduler: self.scheduler,
            feedback: self.feedback,
            objective: self.objective,
            share_objectives: self.share_objectives,
        }
    }
}

impl<CS, F, IC, IF, OF> HasToTargetBytesConverter for StdFuzzer<CS, F, IC, IF, OF> {
    type Converter = IC;

    fn target_bytes_converter(&self) -> &Self::Converter {
        &self.target_bytes_converter
    }

    fn target_bytes_converter_mut(&mut self) -> &mut Self::Converter {
        &mut self.target_bytes_converter
    }
}

impl<CS, F, OF> StdFuzzer<CS, F, BytesInputConverter, NopInputFilter, OF> {
    /// Creates a new [`StdFuzzer`] with standard behavior.
    pub fn new(
        scheduler: CS,
        feedback: F,
        objective: OF,
    ) -> StdFuzzer<CS, F, BytesInputConverter, NopInputFilter, OF> {
        StdFuzzerBuilder::new()
            .scheduler(scheduler)
            .feedback(feedback)
            .objective(objective)
            .build()
    }
}

impl StdFuzzer<(), (), BytesInputConverter, NopInputFilter, ()> {
    /// Creates a new [`StdFuzzerBuilder`] with default types.
    #[must_use]
    pub fn builder() -> StdFuzzerBuilder<(), (), BytesInputConverter, NopInputFilter, ()> {
        StdFuzzerBuilder::new()
    }
}

/// Structs with this trait will execute an input
pub trait ExecutesInput<E, I, S> {
    /// Runs the input and triggers observers and feedback
    fn execute_input(
        &mut self,
        state: &mut S,
        executor: &mut E,
        input: &I,
    ) -> Result<ExitKind, Error>;
}

impl<CS, E, F, I, IC, IF, OF, S> ExecutesInput<E, I, S> for StdFuzzer<CS, F, IC, IF, OF>
where
    CS: Scheduler<I, S>,
    E: Executor<EM, I, S, Self> + HasObservers,
    E::Observers: ObserversTuple<I, S>,
    S: HasExecutions + HasCorpus<I> + MaybeHasClientPerfMonitor,
{
    /// Runs the input and triggers observers and feedback
    fn execute_input(
        &mut self,
        state: &mut S,
        executor: &mut E,
        input: &I,
    ) -> Result<ExitKind, Error> {
        start_timer!(state);
        executor.observers_mut().pre_exec_all(state, input)?;
        mark_feature_time!(state, PerfFeature::PreExecObservers);

        start_timer!(state);
        let exit_kind = executor.run_target(self, state, event_mgr, input)?;
        mark_feature_time!(state, PerfFeature::TargetExecution);

        start_timer!(state);
        executor
            .observers_mut()
            .post_exec_all(state, input, &exit_kind)?;
        mark_feature_time!(state, PerfFeature::PostExecObservers);

        Ok(exit_kind)
    }
}

/// A [`NopFuzzer`] that does nothing
#[derive(Debug, Copy, Clone)]
pub struct NopFuzzer<IC = BytesInputConverter> {
    input_converter: IC,
}

impl NopFuzzer<BytesInputConverter> {
    /// Creates a new [`NopFuzzer`]
    #[must_use]
    pub fn new() -> NopFuzzer<BytesInputConverter> {
        Self {
            input_converter: BytesInputConverter::new(),
        }
    }
}

impl<IC> NopFuzzer<IC> {
    /// Creates a new [`NopFuzzer`] with the given input converter
    #[must_use]
    pub fn new_with_converter(input_converter: IC) -> Self {
        Self { input_converter }
    }
}

impl Default for NopFuzzer<BytesInputConverter> {
    fn default() -> Self {
        Self::new()
    }
}

impl<IC> HasToTargetBytesConverter for NopFuzzer<IC> {
    type Converter = IC;
    fn target_bytes_converter(&self) -> &Self::Converter {
        &self.input_converter
    }

    fn target_bytes_converter_mut(&mut self) -> &mut Self::Converter {
        &mut self.input_converter
    }
}

impl<E, I, IC, S, ST> Fuzzer<E, I, S, ST> for NopFuzzer<IC>
where
    ST: StagesTuple<E, EM, S, Self>,
    S: HasMetadata + HasExecutions + HasLastReportTime + HasCurrentStageId,
{
    fn fuzz_one(
        &mut self,
        _stages: &mut ST,
        _executor: &mut E,
        _state: &mut S,
    ) -> Result<CorpusId, Error> {
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
    ) -> Result<CorpusId, Error> {
        unimplemented!("NopFuzzer cannot fuzz");
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use core::cell::RefCell;

    use libafl_bolts::rands::StdRand;
    use serial_test::serial;

    use crate::{
        StdFuzzer,
        corpus::InMemoryCorpus,
        executors::{ExitKind, InProcessExecutor},
        fuzzer::{BloomInputFilter, Evaluator},
        inputs::BytesInput,
        schedulers::StdScheduler,
        state::StdState,
    };

    #[test]
    #[serial]
    fn filtered_execution() {
        let execution_count = RefCell::new(0);
        let scheduler = StdScheduler::new();
        let bloom_filter = BloomInputFilter::default();
        let mut fuzzer = StdFuzzer::builder()
            .input_filter(bloom_filter)
            .scheduler(scheduler)
            .feedback(())
            .objective(())
            .build();
        let mut state = StdState::new(
            StdRand::new(),
            InMemoryCorpus::new(),
            InMemoryCorpus::new(),
            &mut (),
            &mut (),
        )
        .unwrap();
        let mut harness = |_input: &BytesInput| {
            *execution_count.borrow_mut() += 1;
            ExitKind::Ok
        };
        let mut executor =
            InProcessExecutor::new(&mut harness, (), &mut fuzzer, &mut state).unwrap();
        let input = BytesInput::new(vec![1, 2, 3]);
        assert!(
            fuzzer
                .evaluate_input(&mut state, &mut executor, &input)
                .is_ok()
        );
        assert_eq!(1, *execution_count.borrow()); // evaluate_input does not add it to the filter

        assert!(
            fuzzer
                .evaluate_filtered(&mut state, &mut executor, &input)
                .is_ok()
        );
        assert_eq!(2, *execution_count.borrow()); // at to the filter

        assert!(
            fuzzer
                .evaluate_filtered(&mut state, &mut executor, &input)
                .is_ok()
        );
        assert_eq!(2, *execution_count.borrow()); // the harness is not called

        assert!(
            fuzzer
                .evaluate_input(&mut state, &mut executor, &input)
                .is_ok()
        );
        assert_eq!(3, *execution_count.borrow()); // evaluate_input ignores filters
    }
}
