use std::string::ToString;

use libafl_bolts::current_time;
use libafl_core::Error;

use crate::{
    Controller,
    corpus::{Corpus, Scheduler, Testcase},
    dependency::Registrator,
    executors::{Executor, ExitKind},
    feedbacks::Feedback,
    fuzzer::{EvaluationResult, Evaluator, Fuzzer, HasFeedback, HasObjective, Verdict},
    observers::ObserversTuple,
    runtimes::{RuntimeHandle, SignalHandlerData},
    stages::StagesTuple,
    state::{FlatState, HasCorpus, HasObjectiveCorpus, HasTestcase, State},
};

unsafe fn std_on_crash<CT, E, F, I, OF, S>(data: &mut SignalHandlerData)
where
    E: Executor<I, S>,
    F: Feedback<I, E::Observers, S>,
    I: Clone,
    OF: Feedback<I, E::Observers, S>,
    S: State<I>,
{
    if !data.in_fuzzing() {
        panic!("A crash occured out of the fuzzing loop. This is a fuzzer bug.");
        return;
    }

    let state = unsafe { data.state::<S>() };
    let fuzzer = unsafe { data.fuzzer::<StdFuzzer<F, OF>>() };
    let input = unsafe { data.input::<I>() };
    let observers = unsafe { data.observers::<E::Observers>() };

    todo!();
}

unsafe fn std_on_timeout<CT, E, F, I, OF, S>(data: &mut SignalHandlerData)
where
    E: Executor<I, S>,
    F: Feedback<I, E::Observers, S>,
    I: Clone,
    OF: Feedback<I, E::Observers, S>,
    S: State<I>,
{
    if !data.in_fuzzing() {
        panic!("A timeout occured out of the fuzzing loop. This is a fuzzer bug.");
        return;
    }

    let state = unsafe { data.state::<S>() };
    let fuzzer = unsafe { data.fuzzer::<StdFuzzer<F, OF>>() };
    let input = unsafe { data.input::<I>() };
    let observers = unsafe { data.observers::<E::Observers>() };

    todo!();
}

/// Your default fuzzer instance, for everyday use.
#[derive(Debug)]
pub struct StdFuzzer<F, OF> {
    /// The [`Feedback`] that will store new testcases on if a run returns `is_interesting`.
    feedback: F,
    /// The [`Feedback`] that will store new testcases as solution (for example, a crash) if a run returns `is_interesting`.
    objective: OF,
    initialized: bool,
}

/// The builder for std fuzzer
#[derive(Debug)]
pub struct StdFuzzerBuilder<F, OF> {
    /// The [`Feedback`] that will store new testcases on if a run returns `is_interesting`.
    feedback: F,
    /// The [`Feedback`] that will store new testcases as solution (for example, a crash) if a run returns `is_interesting`.
    objective_feedback: OF,
}

impl<F, OF> HasFeedback for StdFuzzer<F, OF> {
    type Feedback = F;

    fn feedback(&self) -> &Self::Feedback {
        &self.feedback
    }

    fn feedback_mut(&mut self) -> &mut Self::Feedback {
        &mut self.feedback
    }
}

impl<F, OF> HasObjective for StdFuzzer<F, OF> {
    type Objective = OF;

    fn objective(&self) -> &OF {
        &self.objective
    }

    fn objective_mut(&mut self) -> &mut OF {
        &mut self.objective
    }
}

impl<F, OF> StdFuzzer<F, OF> {
    fn evaluate_execution<I, OT, S, SC, Z>(
        &mut self,
        state: &mut S,
        input: &I,
        observers: &OT,
        exit_kind: ExitKind,
    ) -> Result<EvaluationResult, Error>
    where
        F: Feedback<I, OT, S>,
        I: Clone,
        OF: Feedback<I, OT, S>,
        S: State<I>,
    {
        #[cfg(not(feature = "introspection"))]
        let is_solution = self
            .objective
            .is_interesting(state, input, observers, &exit_kind)?;

        #[cfg(feature = "introspection")]
        let is_solution = self
            .objective
            .is_interesting_introspection(state, input, observers, &exit_kind)?;

        let eval_res: EvaluationResult = if is_solution {
            let executions = state.executions();
            let parent_id = state.scheduler().current();

            // The input is a solution, add it to the respective corpus
            let testcase_id = state.objective_corpus_mut().add(input.clone())?;

            let md = state.testcase_md_mut_from_id(&testcase_id);

            md.set_executions(executions);
            md.found_objective();

            // TODO: keep parent id?
            // testcase.set_parent_id_optional(*state.corpus().current());

            #[cfg(feature = "track_hit_feedbacks")]
            self.objective_mut()
                .append_hit_feedbacks(testcase.hit_objectives_mut())?;
            self.objective_mut()
                .append_metadata(state, observers, &testcase_id)?;

            EvaluationResult::new(exit_kind, Verdict::Objective(testcase_id))
        } else {
            #[cfg(not(feature = "introspection"))]
            let corpus_worthy = self
                .feedback
                .is_interesting(state, input, observers, &exit_kind)?;

            #[cfg(feature = "introspection")]
            let corpus_worthy = self
                .feedback
                .is_interesting_introspection(state, input, observers, &exit_kind)?;

            if corpus_worthy {
                // Not a solution
                // Add the input to the main corpus

                let executions = state.executions();
                let parent_id = state.scheduler().current();

                let testcase_id = state.corpus_mut().add(input.clone())?;
                let md = state
                    .testcase_md_mut_from_id(&testcase_id)
                    .set_executions(executions);

                #[cfg(feature = "track_hit_feedbacks")]
                self.feedback_mut()
                    .append_hit_feedbacks(testcase.hit_feedbacks_mut())?;
                self.feedback_mut()
                    .append_metadata(state, observers, &testcase_id)?;

                EvaluationResult::new(exit_kind, Verdict::Corpus(testcase_id))
            } else {
                EvaluationResult::new(exit_kind, Verdict::Uninteresting)
            }
        };

        if eval_res.is_corpus_worthy() {
            *state.last_found_time_mut() = current_time();
        }

        Ok(eval_res)
    }
}

// TODO: do we really need to keep this?
// i don't see when it's really useful
//
// /// Adds an input, even if it's not considered `interesting` by any of the executors
// /// If you are using inprocess executor, be careful.
// /// Your crash-causing testcase will *NOT* be added into the corpus (only to solution)
// fn add_input<E, I, OT, S, Z>(
//     fuzzer: &mut Z,
//     state: &mut S,
//     executor: &mut E,
//     input: I,
// ) -> Result<CorpusId, Error> {
//     *state.last_found_time_mut() = current_time();
//
//     let exit_kind = fuzzer.execute_input(state, executor, &input)?;
//     let observers = executor.observers();
//     // Always consider this to be "interesting"
//     let mut testcase = Testcase::from(input.clone());
//     testcase.set_executions(*state.executions());
//
//     // Maybe a solution
//     #[cfg(not(feature = "introspection"))]
//     let is_solution: bool =
//         fuzzer
//             .objective_mut()
//             .is_interesting(state, &input, &*observers, &exit_kind)?;
//
//     #[cfg(feature = "introspection")]
//     let is_solution = self.objective_mut().is_interesting_introspection(
//         state,
//         &input,
//         &*observers,
//         &exit_kind,
//     )?;
//
//     if is_solution {
//         #[cfg(feature = "track_hit_feedbacks")]
//         self.objective_mut()
//             .append_hit_feedbacks(testcase.hit_objectives_mut())?;
//         fuzzer
//             .objective_mut()
//             .append_metadata(state, &*observers, &mut testcase)?;
//         // we don't care about solution id
//         let id = state.solutions_mut().add(testcase)?;
//
//         return Ok(id);
//     }
//
//     // several is_interesting implementations collect some data about the run, later used in
//     // append_metadata; we *must* invoke is_interesting here to collect it
//     #[cfg(not(feature = "introspection"))]
//     let _corpus_worthy =
//         fuzzer
//             .feedback_mut()
//             .is_interesting(state, &input, &*observers, &exit_kind)?;
//
//     #[cfg(feature = "introspection")]
//     let _corpus_worthy =
//         self.feedback_mut()
//             .is_interesting_introspection(state, &input, &*observers, &exit_kind)?;
//
//     #[cfg(feature = "track_hit_feedbacks")]
//     fuzzer
//         .feedback_mut()
//         .append_hit_feedbacks(testcase.hit_feedbacks_mut())?;
//     // Add the input to the main corpus
//     fuzzer
//         .feedback_mut()
//         .append_metadata(state, &*observers, &mut testcase)?;
//     let id = state.corpus_mut().add(testcase)?;
//     fuzzer.scheduler_mut().on_add(state, id)?;
//
//     Ok(id)
// }

// impl<CS, F, I, IC, IF, OF, OT, S> ExecutionProcessor<I, OT, S> for StdFuzzer<F, OF> {
//     /// Post process a testcase depending the testcase execution results
//     /// returns corpus id if it put something into corpus (not solution)
//     /// This code will not be reached by inprocess executor if crash happened.
//     fn process_execution(
//         &mut self,
//         state: &mut S,
//         input: &I,
//         eval_res: &EvaluationResult,
//         observers: &OT,
//     ) -> Result<Option<CorpusId>, Error> {
//         match eval_res.verdict() {
//             Verdict::Uninteresting => Ok(None),
//             Verdict::Corpus(testcase_id) => {}
//             ExecuteInputResult::Solution => {}
//         }
//     }
// }

impl<CT, E, F, I, OF, S> Evaluator<CT, E, I, S> for StdFuzzer<F, OF>
where
    CT: Controller,
    E: Executor<I, S>,
    F: Feedback<I, E::Observers, S>,
    OF: Feedback<I, E::Observers, S>,
    I: Clone,
    S: State<I>,
{
    /// Process one input, adding to the respective corpora if needed and firing the right events
    #[inline]
    fn evaluate_input(
        &mut self,
        state: &mut S,
        executor: &mut E,
        rt_handle: &mut RuntimeHandle<CT, S>,
        input: &I,
    ) -> Result<EvaluationResult, Error> {
        let exit_kind = executor.execute(state, rt_handle, input)?;

        let observers = executor.observers();
        self.evaluate_execution::<I, E::Observers, S, S::Scheduler, Self>(
            state,
            &*input,
            &*observers,
            exit_kind,
        )
    }
}

impl<CT, E, F, I, OF, R, S, ST> Fuzzer<CT, E, I, R, S, ST> for StdFuzzer<F, OF>
where
    CT: Controller,
    E: Executor<I, S>,
    F: Feedback<I, E::Observers, S>,
    I: Clone,
    OF: Feedback<I, E::Observers, S>,
    S: State<I>,
    ST: StagesTuple<CT, E, R, S, Self>,
{
    fn init(
        &mut self,
        stages: &mut ST,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
    ) -> Result<(), Error> {
        if self.initialized {
            return Ok(());
        }

        // 1 - collect the required mds and involved types
        let mut registrator = Registrator::new();
        self.feedback.register_with_ty(&mut registrator)?;
        self.objective.register_with_ty(&mut registrator)?;
        stages.register_with_ty(&mut registrator)?;
        state.register_with_ty(&mut registrator)?;
        executor.register_with_ty(&mut registrator)?;

        // 2 - check that types and mds for each object
        let mut checker = registrator.finish();
        self.feedback.check(&mut checker)?;
        self.objective.check(&mut checker)?;
        stages.check(&mut checker)?;
        state.check(&mut checker)?;
        executor.check(&mut checker)?;

        // 3 - add the global metadata to the md map
        state.merge_metadata_map(checker.finish());

        // 4 - initialize executor
        executor.init(rt_handle)?;

        // 5 - populate signal handler data if the runtime supports it
        // maybe do it before executor.init? so that executor can check rt is correctly initialized at runtime
        rt_handle.init_signal_handlers(
            state,
            self,
            &mut *executor.observers_mut(),
            |data| unsafe { std_on_crash::<CT, E, F, I, OF, S>(data) },
            |data| unsafe { std_on_timeout::<CT, E, F, I, OF, S>(data) },
        );

        self.initialized = true;

        Ok(())
    }

    unsafe fn fuzz_one_noinit(
        &mut self,
        stages: &mut ST,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<CT, S>,
    ) -> Result<(), Error> {
        // Init timer for scheduler
        #[cfg(feature = "introspection")]
        state.introspection_stats_mut().start_timer();

        // Get the next index from the scheduler
        let testcase_id = state.scheduler_mut().next()?;

        // Mark the elapsed time for the scheduler
        #[cfg(feature = "introspection")]
        state.introspection_stats_mut().mark_scheduler_time();

        // Mark the elapsed time for the scheduler
        #[cfg(feature = "introspection")]
        state.introspection_stats_mut().reset_stage_index();

        // Execute all stages
        stages.perform_all(self, executor, rand, state, rt_handle, &testcase_id)?;

        state
            .testcase_md_mut_from_id(&testcase_id)
            .increase_scheduled_count();

        if state.stop_requested() {
            state.discard_stop_request();
            return Err(Error::shutting_down());
        }

        Ok(())
    }
}

impl StdFuzzerBuilder<(), ()> {
    /// Creates a new [`StdFuzzerBuilder`] with default (nop) types.
    #[must_use]
    pub fn new() -> Self {
        Self {
            feedback: (),
            objective_feedback: (),
        }
    }
}

impl Default for StdFuzzerBuilder<(), ()> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F, OF> StdFuzzerBuilder<F, OF> {
    /// Sets the converter to target bytes.
    /// The converter converts the input to bytes that can be sent to the target (for example, to a [`CommandExecutor`](crate::executors::CommandExecutor).
    #[must_use]
    pub fn target_bytes_converter<I, IC2>(
        self,
        target_bytes_converter: IC2,
    ) -> StdFuzzerBuilder<F, OF> {
        StdFuzzerBuilder {
            feedback: self.feedback,
            objective_feedback: self.objective_feedback,
        }
    }
}

impl<F, OF> StdFuzzerBuilder<F, OF> {
    /// Sets the feedback that will store new testcases on if a run returns `is_interesting`.
    #[must_use]
    pub fn feedback<F2>(self, feedback: F2) -> StdFuzzerBuilder<F2, OF> {
        StdFuzzerBuilder {
            feedback,
            objective_feedback: self.objective_feedback,
        }
    }
}

impl<F, OF> StdFuzzerBuilder<F, OF> {
    /// Sets the feedback that will store new testcases as solution (for example, a crash) if a run returns `is_interesting`.
    #[must_use]
    pub fn objective_feedback<OF2>(self, objective_feedback: OF2) -> StdFuzzerBuilder<F, OF2> {
        StdFuzzerBuilder {
            feedback: self.feedback,
            objective_feedback,
        }
    }
}

impl<F, OF> StdFuzzerBuilder<F, OF> {
    /// Build a [`StdFuzzer`] from this builder.
    pub fn build(self) -> StdFuzzer<F, OF> {
        StdFuzzer {
            feedback: self.feedback,
            objective: self.objective_feedback,
            initialized: false,
        }
    }
}

impl<F, OF> StdFuzzer<F, OF> {
    /// Creates a new [`StdFuzzer`] with standard behavior.
    pub fn new(feedback: F, objective_feedback: OF) -> StdFuzzer<F, OF> {
        StdFuzzerBuilder::new()
            .feedback(feedback)
            .objective_feedback(objective_feedback)
            .build()
    }
}

impl StdFuzzer<(), ()> {
    /// Creates a new [`StdFuzzerBuilder`] with default types.
    #[must_use]
    pub fn builder() -> StdFuzzerBuilder<(), ()> {
        StdFuzzerBuilder::new()
    }
}
