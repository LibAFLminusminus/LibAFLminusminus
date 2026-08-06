//! The standard [`Fuzzer`], for everyday use.

use crate::{
    common::{DependencyResolver, Registrator},
    controllers::{SharingWorker, Worker},
    corpus::{ObjectiveCorpus, ScheduledCorpus, Scheduler, Testcase},
    executors::{Executor, ExitKind},
    feedbacks::Feedback,
    fuzzers::{
        EvaluationResult, Evaluator, Fuzzer, FuzzerHooksTuple, FuzzerOutcome, Interest, LoadResult,
        Loader, Verdict,
    },
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
use alloc::{boxed::Box, collections::VecDeque, rc::Rc};
use core::{marker::PhantomPinned, mem, pin::Pin};
use libaflmm_bolts::{current_time, impl_serdeany};
use libaflmm_core::{Result, illegal_state};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tuple_list::tuple_list;

/// Note: this code should not allocate at all.
/// Any allocation can result in unexpected locks because of concurrency bug with the standard library.
///
/// In practice, it's very hard to enforce, and most likely some allocations will happen there.
/// If it is ever a real bug, investigate there.
fn handle_objective_in_termination_handler<E, F, H, I, OF, S, W>(
    state: &mut S,
    input: &I,
    fuzzer: &mut StdFuzzerInner<E, F, H, OF>,
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
    W: Worker + SharingWorker<I>,
{
    fuzzer
        .executor
        .observers_mut()
        .post_exec_all(state, &exit_kind)?;

    let result = fuzzer.post_execution(state, rt_handle, input, exit_kind)?;

    if let Some(loader_md) = state.metadata_map_mut().get_unnamed_mut::<LoaderMetadata>() {
        loader_md.record_result(result)?;
    }

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
    W: Worker + SharingWorker<I>,
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
    let fuzzer = unsafe { data.fuzzer::<StdFuzzerInner<E, F, H, OF>>() };
    let rt_handle = unsafe { data.rt_handle::<S, W>() };

    let status = unsafe {
        fuzzer
            .executor
            .handle_crash(state, input.as_ref(), signal_params)?
    };

    if let CrashStatus::TargetCrash = status {
        // if it is a target crash, handle crash termination as target objective.
        handle_objective_in_termination_handler(
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
    W: Worker + SharingWorker<I>,
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
    let fuzzer = unsafe { data.fuzzer::<StdFuzzerInner<E, F, H, OF>>() };
    let rt_handle = unsafe { data.rt_handle::<S, W>() };

    let status = unsafe {
        fuzzer
            .executor
            .handle_timeout(state, input.as_ref(), signal_params)?
    };

    handle_objective_in_termination_handler(
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
pub struct StdFuzzer<E, F, H, OF> {
    // do not put anything there, move it to StdFuzzerInner.
    // this is necessary to avoid dangling pointers in signal handlers.
    inner: Pin<Box<StdFuzzerInner<E, F, H, OF>>>,
}

/// The pinned state of a [`StdFuzzer`].
#[derive(Debug)]
struct StdFuzzerInner<E, F, H, OF> {
    /// The executor used by the fuzzer to evaluate inputs
    executor: E,
    /// The [`Feedback`] that will store new testcases on if a run returns `is_interesting`.
    feedback: F,
    /// The [`Feedback`] that will store new testcases as solution (for example, a crash) if a run returns `is_interesting`.
    objective: OF,
    fuzzer_hooks: H,
    loading_stage: usize,
    _pinned: PhantomPinned,
}

/// The builder for std fuzzer
#[derive(Debug)]
pub struct StdFuzzerBuilder<E, F, H, OF> {
    /// The [`Executor`] used by the fuzzer to run the target
    executor: E,
    /// The [`Feedback`] that will store new testcases on if a run returns `is_interesting`.
    feedback: F,
    /// The [`Feedback`] that will store new testcases as solution (for example, a crash) if a run returns `is_interesting`.
    objective_feedback: OF,
    /// the hooks to the fuzzer,
    hooks: H,
}

/// A load
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Loading {
    /// The inputs that still have to be run.
    inputs_to_run: VecDeque<Vec<u8>>,
    /// The inputs already run, with the result they got, in load order.
    ran_inputs: Vec<RanInput>,
}

/// The outcome of a loaded input
#[derive(Debug, Serialize, Deserialize)]
struct RanInput {
    input: Vec<u8>,
    result: EvaluationResult,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct LoaderMetadata {
    loads: HashMap<usize, Loading>,
    running_input: Option<(usize, Vec<u8>)>,
}

impl_serdeany!(LoaderMetadata);

impl LoaderMetadata {
    fn load_mut(&mut self, load_id: usize) -> Result<&mut Loading> {
        self.loads
            .get_mut(&load_id)
            .ok_or_else(|| illegal_state!("Unknown load: {load_id}"))
    }

    fn next_input(&mut self, load_id: usize) -> Result<Option<Vec<u8>>> {
        if self.running_input.take().is_some() {
            log::warn!("Found a running input but it has not been recorded yet. Internal bug?");
        }

        let next_input = self.load_mut(load_id)?.inputs_to_run.pop_front();
        self.running_input = next_input.map(|input| (load_id, input));

        Ok(self.running_input.as_ref().map(|(_, input)| input.clone()))
    }

    fn record_result(&mut self, result: EvaluationResult) -> Result<()> {
        let Some((load_id, input)) = self.running_input.take() else {
            return Ok(());
        };

        self.load_mut(load_id)?
            .ran_inputs
            .push(RanInput { input, result });

        Ok(())
    }

    fn take_ran_inputs(&mut self, load_id: usize) -> Result<Vec<RanInput>> {
        Ok(mem::take(&mut self.load_mut(load_id)?.ran_inputs))
    }
}

impl<E, F, H, OF> StdFuzzerInner<E, F, H, OF> {
    fn post_fuzz_one<I, S, W>(
        &mut self,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()>
    where
        H: FuzzerHooksTuple<E, I, S, W>,
        S: State,
        W: Worker,
    {
        self.fuzzer_hooks
            .post_step_all(&mut self.executor, state, rt_handle)?;

        // timer end
        state.perf_stats_mut().iter_end();

        // update stats if it should be updated
        rt_handle
            .worker_mut()
            .workdir_mut()
            .maybe_report_stats(state.stats())
    }

    fn evaluate_execution<I, S>(
        &mut self,
        state: &mut S,
        input: &I,
        exit_kind: ExitKind,
    ) -> Result<Interest>
    where
        E: Executor<I, S>,
        F: Feedback<I, E::Observers, S>,
        I: Input,
        OF: Feedback<I, E::Observers, S>,
        S: State<Input = I>,
    {
        let is_solution =
            self.objective
                .is_interesting(state, input, &*self.executor.observers(), &exit_kind)?;

        let interest = if is_solution {
            Interest::Objective
        } else {
            let corpus_worthy = self.feedback.is_interesting(
                state,
                input,
                &*self.executor.observers(),
                &exit_kind,
            )?;

            if corpus_worthy {
                Interest::Corpus
            } else {
                Interest::Uninteresting
            }
        };

        Ok(interest)
    }

    fn post_execution<I, S, W>(
        &mut self,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
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
        W: Worker + SharingWorker<I>,
    {
        let interest = self.evaluate_execution::<I, S>(state, input, exit_kind)?;

        let verdict = match interest {
            Interest::Objective => {
                // The input is a objective, add it to the respective corpus
                let executions = state.executions();
                let mut testcase = Testcase::new(Rc::new(input.clone()));

                self.fuzzer_hooks.pre_add_all(
                    &mut self.executor,
                    state,
                    rt_handle,
                    &mut testcase,
                    Interest::Objective,
                )?;

                let testcase_id = state.objective_corpus_mut().add_objective(testcase)?;

                let md = state.testcase_md_mut_from_id(&testcase_id);

                md.set_executions(executions);
                md.found_objective();

                // TODO: keep parent id?
                // testcase.set_parent_id_optional(*state.corpus().current());

                self.objective
                    .append_metadata(state, &*self.executor.observers(), &testcase_id)?;

                let stats = state.stats_mut();
                stats.last_found_time = current_time();
                stats.objective += 1;

                let verdict = Verdict::Objective(testcase_id);

                self.fuzzer_hooks
                    .post_add_all(&mut self.executor, state, rt_handle, verdict)?;

                verdict
            }
            Interest::Corpus => {
                // Not an objective
                // Add the input to the main corpus

                let executions = state.executions();
                let mut testcase = Testcase::new(Rc::new(input.clone()));

                self.fuzzer_hooks.pre_add_all(
                    &mut self.executor,
                    state,
                    rt_handle,
                    &mut testcase,
                    Interest::Corpus,
                )?;

                rt_handle.worker_mut().send_testcase(&testcase)?;
                let testcase_id = state.corpus_mut().add(testcase)?;
                state
                    .testcase_md_mut_from_id(&testcase_id)
                    .set_executions(executions);

                self.feedback
                    .append_metadata(state, &*self.executor.observers(), &testcase_id)?;

                let stats = state.stats_mut();
                stats.last_found_time = current_time();
                stats.corpus += 1;

                let verdict = Verdict::Corpus(testcase_id);

                self.fuzzer_hooks
                    .post_add_all(&mut self.executor, state, rt_handle, verdict)?;

                verdict
            }
            Interest::Uninteresting => Verdict::Uninteresting,
        };

        Ok(EvaluationResult::new(exit_kind, verdict))
    }
}

impl<E, F, H, OF> StdFuzzer<E, F, H, OF> {
    /// Mutable ref to the mutable inner struct
    fn inner_mut(&mut self) -> &mut StdFuzzerInner<E, F, H, OF> {
        unsafe { self.inner.as_mut().get_unchecked_mut() }
    }

    fn initialize<I, S, ST, W>(
        &mut self,
        stages: &mut ST,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()>
    where
        E: Executor<I, S>,
        F: Feedback<I, E::Observers, S>,
        H: FuzzerHooksTuple<E, I, S, W>,
        I: Input,
        OF: Feedback<I, E::Observers, S>,
        S: State<Input = I>,
        ST: DependencyResolver,
        W: Worker + SharingWorker<I>,
    {
        if state.should_initialize_metadata() {
            let inner = self.inner_mut();

            // 1 - collect the required mds and involved types
            let mut registrator = Registrator::new(state.metadata_map().clone());

            inner.feedback.register(&mut registrator)?;
            inner.objective.register(&mut registrator)?;
            inner.fuzzer_hooks.register(&mut registrator)?;
            stages.register(&mut registrator)?;
            state.register(&mut registrator)?;
            inner.executor.register(&mut registrator)?;

            // 2 - check that types and mds for each object
            let checker = registrator.finish();
            inner.feedback.check(&checker)?;
            inner.objective.check(&checker)?;
            stages.check(&checker)?;
            state.check(&checker)?;
            inner.executor.check(&checker)?;

            // 3 - now state metadata get replaced by
            *state.metadata_map_mut() = checker.finish();
        }

        let inner = self.inner_mut();

        // 4 - populate signal handler data if the runtime needs it
        rt_handle.init_termination_handlers(
            inner,
            |data, signal_params| unsafe {
                std_on_crash::<E, F, H, I, OF, S, W>(data, signal_params)
            },
            |data, signal_params| unsafe {
                std_on_timeout::<E, F, H, I, OF, S, W>(data, signal_params)
            },
        );

        // 5 - initialize executor
        inner.executor.init(state, rt_handle)?;

        // report empty start during init
        rt_handle
            .worker_mut()
            .workdir_mut()
            .report_stats(state.stats())?;

        Ok(())
    }
}

impl<E, F, H, I, OF, S, W> Evaluator<E, I, S, W> for StdFuzzer<E, F, H, OF>
where
    E: Executor<I, S>,
    F: Feedback<I, E::Observers, S>,
    H: FuzzerHooksTuple<E, I, S, W>,
    OF: Feedback<I, E::Observers, S>,
    I: Input,
    S: State<Input = I>,
    W: Worker + SharingWorker<I>,
{
    /// Process one input, adding to the respective corpora if needed and firing the right events
    #[inline]
    fn evaluate_input(
        &mut self,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        input: &I,
    ) -> Result<EvaluationResult> {
        let inner = self.inner_mut();

        let exit_kind = inner.executor.execute(state, rt_handle, input)?;

        let res = inner.post_execution(state, rt_handle, input, exit_kind)?;

        rt_handle
            .worker_mut()
            .workdir_mut()
            .maybe_report_stats(state.stats())?;

        Ok(res)
    }
}

impl<E, F, H, I, OF, S, W> Loader<I, S, W> for StdFuzzer<E, F, H, OF>
where
    E: Executor<I, S>,
    F: Feedback<I, E::Observers, S>,
    H: FuzzerHooksTuple<E, I, S, W>,
    I: Input + 'static,
    OF: Feedback<I, E::Observers, S>,
    S: State<Input = I>,
    W: Worker + SharingWorker<I>,
{
    fn load(
        &mut self,
        inputs_fn: impl FnOnce(&mut S) -> Result<VecDeque<I>>,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<Vec<LoadResult<I>>> {
        let loading_stage = self.inner.loading_stage;

        if !state
            .get_md_or_insert_with::<LoaderMetadata>(LoaderMetadata::default)
            .loads
            .contains_key(&loading_stage)
        {
            let inputs_to_run: VecDeque<Vec<u8>> = inputs_fn(state)?
                .iter()
                .map(postcard::to_allocvec)
                .collect::<core::result::Result<_, _>>()?;

            state.get_md_mut::<LoaderMetadata>()?.loads.insert(
                loading_stage,
                Loading {
                    ran_inputs: Vec::with_capacity(inputs_to_run.len()),
                    inputs_to_run,
                },
            );
        }

        while let Some(input) = state
            .get_md_mut::<LoaderMetadata>()?
            .next_input(loading_stage)?
        {
            let result = self.evaluate_input(state, rt_handle, &postcard::from_bytes(&input)?)?;

            state
                .get_md_mut::<LoaderMetadata>()?
                .record_result(result)?;
        }

        self.inner_mut().loading_stage += 1;

        state
            .get_md_mut::<LoaderMetadata>()?
            .take_ran_inputs(loading_stage)?
            .into_iter()
            .map(|ran| {
                Ok(LoadResult::new(
                    postcard::from_bytes(&ran.input)?,
                    ran.result,
                ))
            })
            .collect()
    }
}

impl<E, F, H, I, OF, R, S, ST, W> Fuzzer<E, I, R, S, ST, W> for StdFuzzer<E, F, H, OF>
where
    E: Executor<I, S>,
    F: Feedback<I, E::Observers, S>,
    H: FuzzerHooksTuple<E, I, S, W>,
    I: Input + 'static,
    OF: Feedback<I, E::Observers, S>,
    S: State<Input = I>,
    ST: StagesTuple<E, R, S, W, Self>,
    W: Worker + SharingWorker<I>,
{
    fn fuzz_one(
        &mut self,
        stages: &mut ST,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<FuzzerOutcome> {
        if rt_handle.worker_mut().poll()? {
            if rt_handle.worker_mut().should_shutdown() {
                rt_handle.shutdown();
            }

            log::debug!("Adding pending testcases to state");
            state.add_pending_testcases(rt_handle.worker_mut().recv_testcases()?);
        }

        while let Some(tc) = state.next_pending_testcase() {
            log::debug!("Evaluating pending testcase: {:?}", tc.id());
            let res = self.evaluate_input(state, rt_handle, &*tc.input())?;
            log::debug!("Evaluation result: {res:?}");
        }

        let testcase_id = {
            let inner = self.inner_mut();

            // start the timer for this loop
            state.perf_stats_mut().iter_begin();

            inner
                .fuzzer_hooks
                .pre_step_all(&mut inner.executor, state, rt_handle)?;

            // Get the next index from the scheduler
            let Some(testcase_id) = state.scheduler_mut().next()? else {
                inner.post_fuzz_one(state, rt_handle)?;
                return Ok(FuzzerOutcome::Idle);
            };

            inner.fuzzer_hooks.pre_perform_all(
                &mut inner.executor,
                state,
                rt_handle,
                testcase_id,
            )?;

            testcase_id
        };

        // Execute all stages
        stages.perform_all(self, rand, state, rt_handle, &testcase_id)?;

        let inner = self.inner_mut();

        state
            .testcase_md_mut_from_id(&testcase_id)
            .increase_scheduled_count();

        inner.post_fuzz_one(state, rt_handle)?;
        Ok(FuzzerOutcome::Finished)
    }
}

impl<E> StdFuzzerBuilder<E, (), (), ()> {
    /// Creates a new [`StdFuzzerBuilder`] with default (nop) types.
    #[must_use]
    pub fn new(executor: E) -> Self {
        Self {
            executor,
            feedback: (),
            objective_feedback: (),
            hooks: (),
        }
    }
}

impl<E, F, H, OF> StdFuzzerBuilder<E, F, H, OF> {
    /// Sets the feedback that will store new testcases on if a run returns `is_interesting`.
    #[must_use]
    pub fn feedback<F2>(self, feedback: F2) -> StdFuzzerBuilder<E, F2, H, OF> {
        StdFuzzerBuilder {
            executor: self.executor,
            feedback,
            objective_feedback: self.objective_feedback,
            hooks: self.hooks,
        }
    }

    /// Sets the feedback that will store new testcases as solution (for example, a crash) if a run returns `is_interesting`.
    #[must_use]
    pub fn objective_feedback<OF2>(
        self,
        objective_feedback: OF2,
    ) -> StdFuzzerBuilder<E, F, H, OF2> {
        StdFuzzerBuilder {
            executor: self.executor,
            feedback: self.feedback,
            objective_feedback,
            hooks: self.hooks,
        }
    }

    /// Sets the feedback that will store new testcases on if a run returns `is_interesting`.
    #[must_use]
    pub fn executor<E2>(self, executor: E2) -> StdFuzzerBuilder<E2, F, H, OF> {
        StdFuzzerBuilder {
            executor,
            feedback: self.feedback,
            objective_feedback: self.objective_feedback,
            hooks: self.hooks,
        }
    }

    /// Sets the feedback that will store new testcases as solution (for example, a crash) if a run returns `is_interesting`.
    #[must_use]
    pub fn fuzzer_hooks<H2>(self, fuzzer_hooks: H2) -> StdFuzzerBuilder<E, F, H2, OF> {
        StdFuzzerBuilder {
            executor: self.executor,
            feedback: self.feedback,
            objective_feedback: self.objective_feedback,
            hooks: fuzzer_hooks,
        }
    }

    /// Build a [`StdFuzzer`] from this builder.
    pub fn build<I, S, ST, W>(
        self,
        stages: &mut ST,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<StdFuzzer<E, F, H, OF>>
    where
        E: Executor<I, S>,
        F: Feedback<I, E::Observers, S>,
        H: FuzzerHooksTuple<E, I, S, W>,
        I: Input,
        OF: Feedback<I, E::Observers, S>,
        S: State<Input = I>,
        ST: DependencyResolver,
        W: Worker + SharingWorker<I>,
    {
        let mut fuzzer = StdFuzzer {
            inner: Box::pin(StdFuzzerInner {
                executor: self.executor,
                feedback: self.feedback,
                objective: self.objective_feedback,
                fuzzer_hooks: self.hooks,
                loading_stage: 0,
                _pinned: PhantomPinned,
            }),
        };

        fuzzer.initialize(stages, state, rt_handle)?;

        Ok(fuzzer)
    }
}

impl<E, F, OF> StdFuzzer<E, F, (), OF> {
    /// Creates a new [`StdFuzzer`] with standard behavior.
    pub fn new<I, S, ST, W>(
        executor: E,
        feedback: F,
        objective_feedback: OF,
        stages: &mut ST,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<StdFuzzer<E, F, (), OF>>
    where
        E: Executor<I, S>,
        F: Feedback<I, E::Observers, S>,
        I: Input,
        OF: Feedback<I, E::Observers, S>,
        S: State<Input = I>,
        ST: DependencyResolver,
        W: Worker + SharingWorker<I>,
    {
        Self::with_hooks(
            executor,
            feedback,
            objective_feedback,
            tuple_list!(),
            stages,
            state,
            rt_handle,
        )
    }
}

impl<E, F, H, OF> StdFuzzer<E, F, H, OF> {
    /// Creates a new [`StdFuzzer`] with standard behavior.
    pub fn with_hooks<I, S, ST, W>(
        executor: E,
        feedback: F,
        objective_feedback: OF,
        hooks: H,
        stages: &mut ST,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<StdFuzzer<E, F, H, OF>>
    where
        E: Executor<I, S>,
        F: Feedback<I, E::Observers, S>,
        H: FuzzerHooksTuple<E, I, S, W>,
        I: Input,
        OF: Feedback<I, E::Observers, S>,
        S: State<Input = I>,
        ST: DependencyResolver,
        W: Worker + SharingWorker<I>,
    {
        StdFuzzerBuilder::new(executor)
            .feedback(feedback)
            .objective_feedback(objective_feedback)
            .fuzzer_hooks(hooks)
            .build(stages, state, rt_handle)
    }
}

impl<E> StdFuzzer<E, (), (), ()> {
    /// Creates a new [`StdFuzzerBuilder`] with default types.
    #[must_use]
    pub fn builder(executor: E) -> StdFuzzerBuilder<E, (), (), ()> {
        StdFuzzerBuilder::new(executor)
    }
}
