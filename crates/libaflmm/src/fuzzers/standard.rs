//! The standard [`Fuzzer`], for everyday use.

use alloc::rc::Rc;
use core::time::Duration;

use libaflmm_bolts::current_time;
use libaflmm_core::Result;
use quanta::{Clock, Instant};
use tuple_list::tuple_list;

use crate::{
    FuzzerHooksTuple, Worker,
    corpus::{Corpus, Scheduler, Testcase},
    dependency::Registrator,
    executors::{Executor, ExitKind},
    feedbacks::Feedback,
    fuzzers::{EvaluationResult, Evaluator, Fuzzer, HasFeedback, HasObjective, Verdict},
    inputs::Input,
    observers::ObserversTuple,
    runtimes::{
        RuntimeHandle,
        inprocess::{CrashStatus, TimeoutStatus},
        utils::{OsTerminationParams, TerminationHandlerData},
    },
    stages::StagesTuple,
    states::State,
};

const STATS_UPDATE_INTERVAL: Duration = Duration::from_secs(4);

/// Note: this code should not allocate at all.
/// Any allocation can result in unexpected locks because of concurrency bug with the standard library.
///
/// In practice, it's very hard to enforce, and most likely some allocations will happen there.
/// If it is ever a real bug, investigate there.
fn handle_objective_in_termination_handler<E, F, H, I, OF, S, W>(
    executor: &mut E,
    state: &mut S,
    input: &I,
    fuzzer: &mut StdFuzzer<F, H, OF>,
    rt_handle: &mut RuntimeHandle<S, W>,
    exit_kind: ExitKind,
) -> Result<()>
where
    E: Executor<I, S>,
    F: Feedback<I, E::Observers, S>,
    H: FuzzerHooksTuple<E, I, S, W>,
    I: Input,
    OF: Feedback<I, E::Observers, S>,
    S: State<Input = I>,
    W: Worker,
{
    executor.observers_mut().post_exec_all(state, &exit_kind)?;

    fuzzer.post_execution(state, rt_handle, executor, input, exit_kind)?;

    Ok(())
}

/// Crash signals will end up there, if it happens during a fuzzing run.
/// Ending up here out of a fuzzing run is an error.
unsafe fn std_on_crash<E, F, H, I, OF, S, W>(
    data: &mut TerminationHandlerData,
    signal_params: &OsTerminationParams,
) -> Result<CrashStatus>
where
    E: Executor<I, S>,
    F: Feedback<I, E::Observers, S>,
    H: FuzzerHooksTuple<E, I, S, W>,
    I: Input,
    OF: Feedback<I, E::Observers, S>,
    S: State<Input = I>,
    W: Worker,
{
    // double check, not mandatory
    assert!(
        data.in_fuzzing(),
        "A crash occured out of the fuzzing loop. This is a fuzzer bug."
    );

    // note: take input to signify we are out of target code
    // it is useful if subsequent code panics / raises another signal.
    let input = unsafe { data.take_input::<I>() };
    let state = unsafe { data.state::<S>() };
    let fuzzer = unsafe { data.fuzzer::<StdFuzzer<F, H, OF>>() };
    let executor = unsafe { data.executor::<E, I, S>() };
    let rt_handle = unsafe { data.rt_handle::<S, W>() };

    let status = unsafe { executor.handle_crash(state, input.as_ref(), signal_params)? };

    if let CrashStatus::TargetCrash = status {
        // if it is a target crash, handle crash termination as target objective.
        handle_objective_in_termination_handler(
            executor,
            state,
            &input.unwrap(), // since it is a target crash, it must be during fuzzing.
            fuzzer,
            rt_handle,
            ExitKind::Crash,
        )?;
    }

    Ok(status)
}

/// Timeout signals will end up there, if it happens during a fuzzing run.
/// Ending up here out of a fuzzing run is an error.
unsafe fn std_on_timeout<E, F, H, I, OF, S, W>(
    data: &mut TerminationHandlerData,
    signal_params: &OsTerminationParams,
) -> Result<TimeoutStatus>
where
    E: Executor<I, S>,
    F: Feedback<I, E::Observers, S>,
    H: FuzzerHooksTuple<E, I, S, W>,
    I: Input,
    OF: Feedback<I, E::Observers, S>,
    S: State<Input = I>,
    W: Worker,
{
    // double check, not mandatory
    assert!(
        data.in_fuzzing(),
        "A timeout occured out of the fuzzing loop. This is a fuzzer bug."
    );

    // note: take input to signify we are out of target code
    // it is useful if subsequent code panics / raises another signal.
    let input = unsafe { data.take_input::<I>() };
    let state = unsafe { data.state::<S>() };
    let fuzzer = unsafe { data.fuzzer::<StdFuzzer<F, H, OF>>() };
    let executor = unsafe { data.executor::<E, I, S>() };
    let rt_handle = unsafe { data.rt_handle::<S, W>() };

    let status = unsafe { executor.handle_timeout(state, input.as_ref(), signal_params)? };

    handle_objective_in_termination_handler(
        executor,
        state,
        &input.unwrap(), // since it is a target crash, it must be during fuzzing.
        fuzzer,
        rt_handle,
        ExitKind::Timeout,
    )?;

    Ok(status)
}

/// Your default fuzzer instance, for everyday use.
#[derive(Debug)]
pub struct StdFuzzer<F, H, OF> {
    /// The [`Feedback`] that will store new testcases on if a run returns `is_interesting`.
    feedback: F,
    /// The [`Feedback`] that will store new testcases as solution (for example, a crash) if a run returns `is_interesting`.
    objective: OF,
    fuzzer_hooks: H,
    initialized: bool,
    clock: Clock,
    last_synced: Instant,
}

/// The builder for std fuzzer
#[derive(Debug)]
pub struct StdFuzzerBuilder<F, H, OF> {
    /// The [`Feedback`] that will store new testcases on if a run returns `is_interesting`.
    feedback: F,
    /// The [`Feedback`] that will store new testcases as solution (for example, a crash) if a run returns `is_interesting`.
    objective_feedback: OF,
    /// the hooks to the fuzzer,
    hooks: H,
}

impl<F, H, OF> HasFeedback for StdFuzzer<F, H, OF> {
    type Feedback = F;

    fn feedback(&self) -> &Self::Feedback {
        &self.feedback
    }

    fn feedback_mut(&mut self) -> &mut Self::Feedback {
        &mut self.feedback
    }
}

impl<F, H, OF> HasObjective for StdFuzzer<F, H, OF> {
    type Objective = OF;

    fn objective(&self) -> &OF {
        &self.objective
    }

    fn objective_feedback_mut(&mut self) -> &mut OF {
        &mut self.objective
    }
}

impl<F, H, OF> StdFuzzer<F, H, OF> {
    fn evaluate_execution<I, OT, S>(
        &mut self,
        state: &mut S,
        input: &I,
        observers: &OT,
        exit_kind: ExitKind,
    ) -> Result<EvaluationResult>
    where
        F: Feedback<I, OT, S>,
        I: Input,
        OF: Feedback<I, OT, S>,
        S: State<Input = I>,
    {
        let is_solution = self
            .objective
            .is_interesting(state, input, observers, &exit_kind)?;

        let eval_res: EvaluationResult = if is_solution {
            EvaluationResult::new(exit_kind, Verdict::Objective)
        } else {
            let corpus_worthy = self
                .feedback
                .is_interesting(state, input, observers, &exit_kind)?;

            if corpus_worthy {
                EvaluationResult::new(exit_kind, Verdict::Corpus)
            } else {
                EvaluationResult::new(exit_kind, Verdict::Uninteresting)
            }
        };

        Ok(eval_res)
    }

    fn post_execution<E, I, S, W>(
        &mut self,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        executor: &mut E,
        input: &I,
        exit_kind: ExitKind,
    ) -> Result<EvaluationResult>
    where
        E: Executor<I, S>,
        F: Feedback<I, E::Observers, S>,
        H: FuzzerHooksTuple<E, I, S, W>,
        I: Input,
        OF: Feedback<I, E::Observers, S>,
        S: State<Input = I>,
        W: Worker,
    {
        let observers = executor.observers();
        let result =
            self.evaluate_execution::<I, E::Observers, S>(state, input, &*observers, exit_kind)?;

        if result.is_objective_worthy() {
            // The input is a objective, add it to the respective corpus
            let executions = state.executions();
            let mut testcase = Testcase::new(Rc::new(input.clone()));

            self.fuzzer_hooks.pre_add_all(
                executor,
                state,
                rt_handle,
                &mut testcase,
                Verdict::Objective,
            )?;

            let testcase_id = state.objective_corpus_mut().add(testcase)?;

            let md = state.testcase_md_mut_from_id(&testcase_id);

            md.set_executions(executions);
            md.found_objective();

            // TODO: keep parent id?
            // testcase.set_parent_id_optional(*state.corpus().current());

            self.objective_feedback_mut().append_metadata(
                state,
                &*executor.observers(),
                &testcase_id,
            )?;

            let stats = state.stats_mut();
            stats.last_found_time = current_time();
            stats.objective += 1;

            self.fuzzer_hooks.post_add_all(
                executor,
                state,
                rt_handle,
                testcase_id,
                Verdict::Objective,
            )?;
        } else if result.is_corpus_worthy() {
            // Not an objective
            // Add the input to the main corpus

            let executions = state.executions();
            let mut testcase = Testcase::new(Rc::new(input.clone()));

            self.fuzzer_hooks.pre_add_all(
                executor,
                state,
                rt_handle,
                &mut testcase,
                Verdict::Corpus,
            )?;

            let testcase_id = state.corpus_mut().add(testcase)?;
            state
                .testcase_md_mut_from_id(&testcase_id)
                .set_executions(executions);

            self.feedback_mut()
                .append_metadata(state, &*executor.observers(), &testcase_id)?;

            let stats = state.stats_mut();
            stats.last_found_time = current_time();
            stats.corpus += 1;

            self.fuzzer_hooks.post_add_all(
                executor,
                state,
                rt_handle,
                testcase_id,
                Verdict::Corpus,
            )?;
        }

        Ok(result)
    }
}

impl<E, F, H, I, OF, S, W> Evaluator<E, I, S, W> for StdFuzzer<F, H, OF>
where
    E: Executor<I, S>,
    F: Feedback<I, E::Observers, S>,
    H: FuzzerHooksTuple<E, I, S, W>,
    OF: Feedback<I, E::Observers, S>,
    I: Input,
    S: State<Input = I>,
    W: Worker,
{
    /// Process one input, adding to the respective corpora if needed and firing the right events
    #[inline]
    fn evaluate_input(
        &mut self,
        state: &mut S,
        executor: &mut E,
        rt_handle: &mut RuntimeHandle<S, W>,
        input: &I,
    ) -> Result<EvaluationResult> {
        let exit_kind = executor.execute(state, rt_handle, input)?;

        self.post_execution(state, rt_handle, executor, input, exit_kind)
    }
}

impl<E, F, H, I, OF, R, S, ST, W> Fuzzer<E, I, R, S, ST, W> for StdFuzzer<F, H, OF>
where
    E: Executor<I, S>,
    F: Feedback<I, E::Observers, S>,
    H: FuzzerHooksTuple<E, I, S, W>,
    I: Input,
    OF: Feedback<I, E::Observers, S>,
    S: State<Input = I>,
    ST: StagesTuple<E, R, S, W, Self>,
    W: Worker,
{
    fn init(
        &mut self,
        stages: &mut ST,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        if state.should_initialize_metadata() {
            // 1 - collect the required mds and involved types
            let mut registrator = Registrator::new(state.metadata_map().clone());

            self.feedback.register_with_ty(&mut registrator)?;
            self.objective.register_with_ty(&mut registrator)?;
            self.fuzzer_hooks.register_with_ty(&mut registrator)?;
            stages.register_with_ty(&mut registrator)?;
            state.register_with_ty(&mut registrator)?;
            executor.register_with_ty(&mut registrator)?;

            // 2 - check that types and mds for each object
            let checker = registrator.finish();
            self.feedback.check(&checker)?;
            self.objective.check(&checker)?;
            self.objective.check(&checker)?;
            stages.check(&checker)?;
            state.check(&checker)?;
            executor.check(&checker)?;

            // 3 - now state metadata get replaced by
            *state.metadata_map_mut() = checker.finish();
        }

        // 4 - populate signal handler data if the runtime needs it
        rt_handle.init_termination_handlers::<E, I, R, ST, Self>(
            state,
            self,
            executor,
            |data, signal_params| unsafe {
                std_on_crash::<E, F, H, I, OF, S, W>(data, signal_params)
            },
            |data, signal_params| unsafe {
                std_on_timeout::<E, F, H, I, OF, S, W>(data, signal_params)
            },
        );

        // 5 - initialize executor
        executor.init(state, rt_handle)?;

        // report empty start during init
        rt_handle
            .worker_mut()
            .workdir_mut()
            .report_stats(state.stats())?;

        self.initialized = true;

        Ok(())
    }

    fn is_initialized(&self) -> bool {
        self.initialized
    }

    unsafe fn fuzz_one_initialized(
        &mut self,
        stages: &mut ST,
        executor: &mut E,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        let now = self.clock.now();
        if now - self.last_synced > STATS_UPDATE_INTERVAL {
            rt_handle
                .worker_mut()
                .workdir_mut()
                .report_stats(state.stats())?;
            self.last_synced = now;
        }

        self.fuzzer_hooks.pre_step_all(executor, state, rt_handle)?;

        // Get the next index from the scheduler
        let testcase_id = state.scheduler_mut().next()?;

        self.fuzzer_hooks
            .pre_perform_all(executor, state, rt_handle, testcase_id)?;

        // Execute all stages
        stages.perform_all(self, executor, rand, state, rt_handle, &testcase_id)?;

        state
            .testcase_md_mut_from_id(&testcase_id)
            .increase_scheduled_count();

        self.fuzzer_hooks
            .post_step_all(executor, state, rt_handle)?;

        Ok(())
    }
}

impl StdFuzzerBuilder<(), (), ()> {
    /// Creates a new [`StdFuzzerBuilder`] with default (nop) types.
    #[must_use]
    pub fn new() -> Self {
        Self {
            feedback: (),
            objective_feedback: (),
            hooks: (),
        }
    }
}

impl Default for StdFuzzerBuilder<(), (), ()> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F, H, OF> StdFuzzerBuilder<F, H, OF> {
    /// Sets the feedback that will store new testcases on if a run returns `is_interesting`.
    #[must_use]
    pub fn feedback<F2>(self, feedback: F2) -> StdFuzzerBuilder<F2, H, OF> {
        StdFuzzerBuilder {
            feedback,
            objective_feedback: self.objective_feedback,
            hooks: self.hooks,
        }
    }
}

impl<F, H, OF> StdFuzzerBuilder<F, H, OF> {
    /// Sets the feedback that will store new testcases as solution (for example, a crash) if a run returns `is_interesting`.
    #[must_use]
    pub fn objective_feedback<OF2>(self, objective_feedback: OF2) -> StdFuzzerBuilder<F, H, OF2> {
        StdFuzzerBuilder {
            feedback: self.feedback,
            objective_feedback,
            hooks: self.hooks,
        }
    }
}

impl<F, H, OF> StdFuzzerBuilder<F, H, OF> {
    /// Sets the feedback that will store new testcases as solution (for example, a crash) if a run returns `is_interesting`.
    #[must_use]
    pub fn fuzzer_hooks<H2>(self, fuzzer_hooks: H2) -> StdFuzzerBuilder<F, H2, OF> {
        StdFuzzerBuilder {
            feedback: self.feedback,
            objective_feedback: self.objective_feedback,
            hooks: fuzzer_hooks,
        }
    }
}

impl<F, H, OF> StdFuzzerBuilder<F, H, OF> {
    /// Build a [`StdFuzzer`] from this builder.
    pub fn build(self) -> StdFuzzer<F, H, OF> {
        let clock = Clock::new();
        let now = clock.now();

        StdFuzzer {
            feedback: self.feedback,
            objective: self.objective_feedback,
            fuzzer_hooks: self.hooks,
            initialized: false,
            clock,
            last_synced: now,
        }
    }
}

impl<F, OF> StdFuzzer<F, (), OF> {
    /// Creates a new [`StdFuzzer`] with standard behavior.
    pub fn new<E, I, R, S, ST, W>(
        feedback: F,
        objective_feedback: OF,
        stages: &mut ST,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<StdFuzzer<F, (), OF>>
    where
        E: Executor<I, S>,
        F: Feedback<I, E::Observers, S>,
        I: Input,
        OF: Feedback<I, E::Observers, S>,
        S: State<Input = I>,
        ST: StagesTuple<E, R, S, W, Self>,
        W: Worker,
    {
        Self::with_hooks(
            feedback,
            objective_feedback,
            tuple_list!(),
            stages,
            executor,
            state,
            rt_handle,
        )
    }
}

impl<F, H, OF> StdFuzzer<F, H, OF> {
    /// Creates a new [`StdFuzzer`] with standard behavior.
    pub fn with_hooks<E, I, R, S, ST, W>(
        feedback: F,
        objective_feedback: OF,
        hooks: H,
        stages: &mut ST,
        executor: &mut E,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<StdFuzzer<F, H, OF>>
    where
        E: Executor<I, S>,
        F: Feedback<I, E::Observers, S>,
        H: FuzzerHooksTuple<E, I, S, W>,
        I: Input,
        OF: Feedback<I, E::Observers, S>,
        S: State<Input = I>,
        ST: StagesTuple<E, R, S, W, Self>,
        W: Worker,
    {
        let mut fuzzer = StdFuzzerBuilder::new()
            .feedback(feedback)
            .objective_feedback(objective_feedback)
            .fuzzer_hooks(hooks)
            .build();

        fuzzer.init(stages, executor, state, rt_handle)?;

        Ok(fuzzer)
    }
}

impl StdFuzzer<(), (), ()> {
    /// Creates a new [`StdFuzzerBuilder`] with default types.
    #[must_use]
    pub fn builder() -> StdFuzzerBuilder<(), (), ()> {
        StdFuzzerBuilder::new()
    }
}
