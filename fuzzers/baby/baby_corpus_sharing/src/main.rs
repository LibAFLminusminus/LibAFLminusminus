use crate::target::SIGNALS;
use libaflmm::{
    Result,
    launchers::groups::WorkerLayout,
    prelude::*,
    sync::{GraphOrchestrator, routers::graph::GraphRouter},
};
use libaflmm_bolts::{Cores, current_nanos, nonnull_raw_mut, rands::StdRand, tuples::tuple_list};
use std::{thread::sleep, time::Duration};

mod target;

/// Our groups identifier
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Groups {
    /// fuzzer group, which runs the usual fuzzer
    Fuzzer,
    /// receiver group, which sleeps and only evaluates the incoming inputs
    Receiver,
}

pub fn main() -> Result<()> {
    env_logger::init();

    // The router describes the topology of the groups
    // Here, we describe a unidirectional route from the fuzzer group to the receiver group
    let router = GraphRouter::builder()
        .route(Groups::Fuzzer, Groups::Receiver)?
        .build();

    // The orchestrator contains all the gory details of the sync mechanism
    // Which set of commands / notifications to use, the way inputs get exchanged, etc...
    //
    // We attach the newly built router to the orchestrator.
    let orchestrator = GraphOrchestrator::new(router);

    // The monitor tracks the fuzzing current status.
    let monitor = StdMonitor::new();

    // Build the controller, which will use the orchestrator to deploy the new topology.
    let controller = StdController::builder()
        .orchestrator(orchestrator)
        .overwrite(true)
        .build()?;

    // The common blueprint for the 2 new groups
    // We will use the same timeout for both.
    let group_builder = StdGroup::builder(&controller).timeout(Some(Duration::from_secs(3)));

    // The fuzzing group itself, with the usual fuzzing build
    let fuzzing_group = group_builder
        .clone()
        .cores(Cores::one())
        .state_builder(|worker| {
            StdState::new(
                BytesContext,
                // Corpus that will be evolved, we keep it in memory for performance
                // It must have a scheduler
                InMemoryCorpus::new(QueueScheduler::new()),
                // Corpus in which we store solutions (crashes in this example),
                // on disk so the user can get them after stopping the fuzzer
                ObjectiveOnDiskCorpus::builder(worker)?.build()?,
            )
        })
        .build_inprocess(move |rt_handle, state| {
            // The source of randomness
            let mut rand = StdRand::with_seed(current_nanos());

            // Create an observation channel using the signals map
            let observer =
                unsafe { ConstMapObserver::from_mut_ptr("signals", nonnull_raw_mut!(SIGNALS)) };

            // Feedback to rate the interestingness of an input
            let feedback = StdFeedback::new(&observer);

            // A feedback to choose if an input is a solution or not
            let objective_feedback =
                feedback_or_fast!(CrashFeedback::new(), TimeoutFeedback::new());

            // Setup a mutational stage with a basic bytes mutator
            let mut stages = tuple_list!(StdStage::default());

            // Create the executor for an in-process function with just one observer
            let executor = StdExecutor::new(state, target::target, tuple_list!(observer), None);

            // Generator of printable bytearrays of max size 32
            let mut generator = RandPrintablesGenerator::new(non_zero!(32));

            let calibration_hk = CalibrationHook::new(&feedback);

            // A fuzzer with feedbacks and a corpus scheduler
            let mut fuzzer = StdFuzzer::with_hooks(
                executor,
                feedback,
                objective_feedback,
                tuple_list!(calibration_hk),
                &mut stages,
                state,
                rt_handle,
            )?;

            // Generate 8 initial inputs
            fuzzer.load_generator(&mut generator, &mut rand, 8, state, rt_handle)?;

            fuzzer.fuzz_loop(&mut stages, &mut rand, state, rt_handle)?;

            Ok(())
        })?;

    // The receiving group, which does nothing and synchronizes
    // incoming inputs every 100ms
    //
    // Note we do not pin this group, as it will be mostly inactive.
    let receiving_group = group_builder
        .worker_layout_fn(|_gid, _wid| WorkerLayout::flat("worker_receiving"))
        .cores(Cores::unpinned(1))
        .state_builder(|worker| {
            StdState::new(
                BytesContext,
                OnDiskCorpus::builder(worker, QueueScheduler::new())?.build()?,
                ObjectiveOnDiskCorpus::builder(worker)?.build()?,
            )
        })
        .build_inprocess(move |rt_handle, state| {
            let mut rand = StdRand::with_seed(current_nanos());

            let observer =
                unsafe { ConstMapObserver::from_mut_ptr("signals", nonnull_raw_mut!(SIGNALS)) };

            let feedback = StdFeedback::new(&observer);

            let objective_feedback =
                feedback_or_fast!(CrashFeedback::new(), TimeoutFeedback::new());

            let mut stages = tuple_list!();

            let executor = StdExecutor::new(state, target::target, tuple_list!(observer), None);

            // fuzzers can also be built with the builder
            let mut fuzzer = StdFuzzer::builder(executor)
                .feedback(feedback)
                .objective_feedback(objective_feedback)
                .build(&mut stages, state, rt_handle)?;

            loop {
                // fuzz until the fuzzer has nothing more to do
                fuzzer.fuzz_loop_until_idle(&mut stages, &mut rand, state, rt_handle)?;
                // sleep for a while, to limit CPU cycles
                sleep(Duration::from_millis(100));
            }
        })?;

    // Final launcher setup.
    // This is where we bind our group identifier to the actual groups that will be launched.
    StdLauncher::builder()
        .controller(controller)
        .monitor(monitor)
        .add_group_with(fuzzing_group, Groups::Fuzzer)
        .add_group_with(receiving_group, Groups::Receiver)
        .build()?
        .launch()
}
