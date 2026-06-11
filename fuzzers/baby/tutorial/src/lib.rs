//! A libfuzzer-like fuzzer with llmp-multithreading support and restarts

#![feature(min_specialization)]
//#![feature(min_const_generics)]

use core::time::Duration;
use std::env;

use libaflmm::{
    controllers::{SimpleController, SimpleWorker, Worker},
    corpus::{schedulers::QueueScheduler, Corpus, InMemoryCorpus, OnDiskCorpus},
    executors::{ExitKind, StdExecutor},
    feedback_or, feedback_or_fast,
    feedbacks::{CrashFeedback, MaxMapFeedback, TimeFeedback, TimeoutFeedback},
    fuzzers::{Fuzzer, StdFuzzer},
    inputs::InputContext,
    launchers::{StdLauncher, DEFAULT_MAX_STATE_SIZE_PER_WORKER},
    monitors::SimpleMonitor,
    observers::{HitcountsMapObserver, StdMapObserver, TimeObserver},
    runtimes::{RuntimeHandle, StdInProcessRuntime},
    stages::StdMutationalStage,
    states::{State, StdState},
    Result,
};
use libaflmm_bolts::{rands::StdRand, timers::FastTimer, tuples::tuple_list};
use libaflmm_targets::{
    coverage::edges_map_mut_slice,
    libfuzzer::{libfuzzer_initialize, libfuzzer_test_one_input},
};

mod input;
use input::PacketData;

mod mutator;
use mutator::LainMutator;

use crate::input::PacketDataContext;

/// The actual fuzzer
fn run_fuzzer<C, OC>(
    rt_handle: &mut RuntimeHandle<StdState<C, PacketDataContext, PacketData, OC>, SimpleWorker>,
    state: &mut StdState<C, PacketDataContext, PacketData, OC>,
) -> Result<()>
where
    C: Corpus<Input = PacketData>,
    OC: Corpus<Input = PacketData>,
{
    // The wrapped harness function, calling out to the LLVM-style harness
    let mut harness = |state: &mut StdState<_, PacketDataContext, PacketData, _>,
                       input: &PacketData| {
        let context: &mut PacketDataContext = state.context_mut();
        let buf = context.to_bytes(input);
        // # Safety
        // We're looking for crashes in there!
        unsafe {
            libfuzzer_test_one_input(&buf);
        }
        Ok(ExitKind::Ok)
    };

    let map = unsafe { StdMapObserver::from_mut_slice("edges", edges_map_mut_slice()) };

    // Create an observation channel using the coverage map
    let edges_observer = HitcountsMapObserver::new(map);

    // Create an observation channel to keep track of the execution time
    let time_observer = TimeObserver::new("time");

    let map_feedback = MaxMapFeedback::new(&edges_observer);

    // Feedback to rate the interestingness of an input
    // This one is composed by two Feedbacks in OR
    let feedback = feedback_or!(
        // New maximization map feedback linked to the edges observer and the feedback state
        map_feedback,
        // Time feedback, this one does not need a feedback state
        TimeFeedback::new(&time_observer),
    );

    // A feedback to choose if an input is a solution or not
    let objective = feedback_or_fast!(CrashFeedback::new(), TimeoutFeedback::new());

    println!("We're a client, let's fuzz :)");

    // Setup a lain mutator with a mutational stage
    let mutator = LainMutator::new();

    let mutational = StdMutationalStage::new(mutator);

    let mut stages = tuple_list!(mutational);

    // Create the executor for an in-process function with one observer for edge coverage and one for the execution time
    let executor = StdExecutor::new(
        &mut harness,
        tuple_list!(edges_observer, time_observer),
        Some(Duration::new(10, 0)),
    );

    // A fuzzer with feedbacks and a corpus scheduler
    let mut fuzzer = StdFuzzer::new(executor, feedback, objective, &mut stages, state, rt_handle)?;

    // The actual target run starts here.
    // Call LLVMFUzzerInitialize() if present.
    let args: Vec<String> = env::args().collect();
    if unsafe { libfuzzer_initialize(&args) } == -1 {
        println!("Warning: LLVMFuzzerInitialize failed with -1");
    }

    // This fuzzer restarts after 1 mio `fuzz_one` executions.
    // Each fuzz_one will internally do many executions of the target.
    // If your target is very instable, setting a low count here may help.
    // However, you will lose a lot of performance that way.
    let mut rand = StdRand::new();
    // Generator of printable bytearrays of max size 32
    // In case the corpus is empty (on first run), reset
    if state.must_load_initial_inputs() {
        let mut in_dirs = env::current_dir()?;
        in_dirs.push("corpus");
        state
            .load_initial_inputs(&mut fuzzer, rt_handle, &[in_dirs.clone()])
            .unwrap_or_else(|_| panic!("Failed to load initial corpus at {:?}", &in_dirs));
        println!("We imported {} inputs from disk.", state.corpus().count());
    }

    fuzzer.fuzz_loop(&mut stages, &mut rand, state, rt_handle)?;

    Ok(())
}

/// The main fn, `no_mangle` as it is a C main
#[no_mangle]
pub extern "C" fn libafl_main() {
    env_logger::init();

    // The state creation closure.
    let state_builder = |worker: &SimpleWorker| {
        // A queue policy to get testcasess from the corpus
        let scheduler = QueueScheduler::new();
        let crash_dir = worker.workdir().objective_dir()?;
        let context = PacketDataContext::default();

        // create a State from scratch
        StdState::new(
            context,
            // Corpus that will be evolved, we keep it in memory for performance
            InMemoryCorpus::with_scheduler(scheduler),
            // Corpus in which we store solutions (crashes in this example),
            // on disk so the user can get them after stopping the fuzzer
            OnDiskCorpus::builder().root_dir(crash_dir).build()?,
        )
    };

    // The launcher supervises the fuzzer and communicates with the workers.
    let controller = SimpleController::builder()
        .overwrite(true)
        .build()
        .expect("Failed to build the SimpleController");

    // The monitor tracks the fuzzing current status.
    let monitor = SimpleMonitor::new();

    let fast_timer = FastTimer::new();
    let runtime = StdInProcessRuntime::new(
        run_fuzzer,
        DEFAULT_MAX_STATE_SIZE_PER_WORKER,
        fast_timer,
        Some(Duration::from_secs(3)),
    );

    // Launch the fuzzer
    let _ = StdLauncher::builder()
        .expect("Failed to instantiate the builder for StdLauncher")
        .controller(controller)
        .monitor(monitor)
        .state_builder(state_builder)
        .runtime(runtime)
        // .build_with_task(run_fuzzer)?
        .build()
        .expect("Failed to build the StdLauncher")
        .launch();
}
