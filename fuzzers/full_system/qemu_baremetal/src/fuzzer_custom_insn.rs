//! A fuzzer using qemu in systemmode for binary-only coverage of kernels

use libaflmm::{
    corpus::{
        schedulers::{NopScheduler, QueueScheduler},
        Corpus, InMemoryCorpus, OnDiskCorpus,
    },
    feedback_or, feedback_or_fast,
    feedbacks::{CrashFeedback, MaxMapFeedback, TimeFeedback, TimeoutFeedback},
    inputs::StdContext,
    launchers::StdLauncher,
    monitors::StdMonitor,
    mutators::{havoc_mutations::havoc_mutations, scheduled::HavocScheduledMutator},
    observers::{HitcountsMapObserver, TimeObserver, VariableMapObserver},
    stages::StdMutationalStage,
    states::{State, StdState},
    Fuzzer, Result, StdController, StdFuzzer, Worker,
};
use libaflmm_bolts::{
    core_affinity::Cores, ownedref::OwnedMutSlice, rands::StdRand, tuples::tuple_list,
};
use libaflmm_qemu::{
    modules::StdEdgeCoverageModule, standard::QemuSnapshotManager, StdEmulator, StdQemuExecutor,
};
use libaflmm_targets::{edges_map_mut_ptr, EDGES_MAP_DEFAULT_SIZE, MAX_EDGES_FOUND};
use std::{env, path::PathBuf, time::Duration};

pub fn fuzz() -> Result<()> {
    env_logger::init();

    // Hardcoded parameters
    let cores = Cores::from_cmdline("1").unwrap();
    let input_dir = PathBuf::from("./corpus");

    // The monitor
    let monitor = StdMonitor::new();

    // The launcher supervises the fuzzer and communicates with the workers.
    let controller = StdController::builder().overwrite(true).build()?;

    // Build and run a Launcher
    StdLauncher::builder()?
        .controller(controller)
        .timeout(Some(Duration::from_secs(5)))
        .state_builder(|worker| {
            let objective_dir = worker.workdir().create_dir("./crashes")?;
            let scheduler = QueueScheduler::new();

            StdState::new(
                StdContext::default(),
                // Corpus that will be evolved, we keep it in memory for performance
                InMemoryCorpus::new(scheduler),
                // Corpus in which we store solutions (crashes in this example),
                // on disk so the user can get them after stopping the fuzzer
                OnDiskCorpus::new(objective_dir, NopScheduler)?,
            )
        })
        .monitor(monitor)
        .cores(cores)
        .build_inprocess(move |rt_handle, state| {
            // Initialize QEMU
            let args: Vec<String> = env::args().collect();
            let mut rand = StdRand::new();

            // Create an observation channel using the coverage map
            let mut edges_observer = unsafe {
                HitcountsMapObserver::new(VariableMapObserver::from_mut_slice(
                    "edges",
                    OwnedMutSlice::from_raw_parts_mut(edges_map_mut_ptr(), EDGES_MAP_DEFAULT_SIZE),
                    &raw mut MAX_EDGES_FOUND,
                ))
            };

            // Choose modules to use
            let modules = tuple_list!(StdEdgeCoverageModule::builder()
                .map_observer(edges_observer.as_mut())
                .build()?);

            let mut emu = StdEmulator::builder()
                .qemu_parameters(args)
                .modules(modules)
                .snapshot_manager(QemuSnapshotManager::default())
                .build()?;

            let devices = emu.list_devices();
            println!("Devices = {:?}", devices);

            unsafe {
                emu.start().unwrap();
            }

            // Create an observation channel to keep track of the execution time
            let time_observer = TimeObserver::new("time");

            // Feedback to rate the interestingness of an input
            // This one is composed by two Feedbacks in OR
            let feedback = feedback_or!(
                // New maximization map feedback linked to the edges observer and the feedback state
                MaxMapFeedback::new(&edges_observer),
                // Time feedback, this one does not need a feedback state
                TimeFeedback::new(&time_observer)
            );

            // A feedback to choose if an input is a solution or not
            let objective = feedback_or_fast!(CrashFeedback::new(), TimeoutFeedback::new());

            // Setup an havoc mutator with a mutational stage
            let mutator = HavocScheduledMutator::new(havoc_mutations());
            let mut stages = tuple_list!(StdMutationalStage::new(mutator));

            // Create a QEMU in-process executor
            let mut executor = StdQemuExecutor::new(
                state,
                emu,
                |_, _, _| Ok(()),
                |_, _, _, _| Ok(()),
                tuple_list!(edges_observer, time_observer),
            )
            .expect("Failed to create QemuExecutor");

            // Instead of calling the timeout handler and restart the process, trigger a breakpoint ASAP
            executor.break_on_timeout();

            // A fuzzer with feedbacks and a corpus scheduler
            let mut fuzzer = StdFuzzer::new(
                feedback,
                objective,
                &mut stages,
                &mut executor,
                state,
                rt_handle,
            )?;

            if state.must_load_initial_inputs() {
                state
                    .load_initial_inputs(
                        &mut fuzzer,
                        &mut executor,
                        rt_handle,
                        &[input_dir.clone()],
                    )
                    .unwrap_or_else(|e| {
                        panic!("Failed to load initial corpus in {:?}: {e:?}", &input_dir);
                    });
                println!("We imported {} inputs from disk.", state.corpus().count());
            }

            fuzzer.fuzz_loop(&mut stages, &mut executor, &mut rand, state, rt_handle)
        })?
        .launch()
}
