use libaflmm::{
    Result, Worker,
    corpus::{
        Corpus, InMemoryCorpus, OnDiskCorpus, Scheduler,
        schedulers::{NopScheduler, QueueScheduler},
    },
    executors::StdExecutor,
    feedback_or_fast,
    feedbacks::{CrashFeedback, MaxMapFeedback, TimeoutFeedback},
    fuzzers::{CalibrationHook, Fuzzer, StdFuzzer},
    generators::RandPrintablesGenerator,
    inputs::{BytesInput, bytes::BytesContext},
    launchers::{DEFAULT_MAX_STATE_SIZE_PER_WORKER, StdLauncher},
    monitors::SimpleMonitor,
    mutators::{HavocScheduledMutator, havoc_mutations},
    non_zero,
    observers::ConstMapObserver,
    runtimes::{RuntimeHandle, StdInProcessRuntime},
    simple::{SimpleController, SimpleWorker},
    stages::StdMutationalStage,
    states::StdState,
};
use libaflmm_bolts::{
    current_nanos, nonnull_raw_mut, rands::StdRand, timers::FastTimer, tuples::tuple_list,
};
use std::time::Duration;

use crate::target::SIGNALS;

mod target;

fn run_fuzzer<C, OC, SC>(
    rt_handle: &mut RuntimeHandle<StdState<C, BytesContext, BytesInput, OC, SC>, SimpleWorker>,
    state: &mut StdState<C, BytesContext, BytesInput, OC, SC>,
) -> Result<()>
where
    C: Corpus<BytesInput>,
    OC: Corpus<BytesInput>,
    SC: Scheduler,
{
    // The source of randomness
    let mut rand = StdRand::with_seed(current_nanos());

    // Create an observation channel using the signals map
    let observer = unsafe { ConstMapObserver::from_mut_ptr("signals", nonnull_raw_mut!(SIGNALS)) };

    // Feedback to rate the interestingness of an input
    let feedback = MaxMapFeedback::new(&observer);

    // A feedback to choose if an input is a solution or not
    // let objective_feedback = CrashFeedback::new();
    let objective_feedback = feedback_or_fast!(CrashFeedback::new(), TimeoutFeedback::new());

    // Setup a mutational stage with a basic bytes mutator
    let mutator = HavocScheduledMutator::new(havoc_mutations());
    let mut stages = tuple_list!(StdMutationalStage::new(mutator));

    // Create the executor for an in-process function with just one observer
    let mut executor = StdExecutor::new(target::target, tuple_list!(observer), None);

    // Generator of printable bytearrays of max size 32
    let mut generator = RandPrintablesGenerator::new(non_zero!(32));

    let calibration_hk = CalibrationHook::new(&feedback);

    // A fuzzer with feedbacks and a corpus scheduler
    let mut fuzzer = StdFuzzer::with_hooks(
        feedback,
        objective_feedback,
        tuple_list!(calibration_hk),
        &mut stages,
        &mut executor,
        state,
        rt_handle,
    )?;

    // Generate 8 initial inputs
    state.generate_initial_inputs(
        &mut fuzzer,
        &mut executor,
        &mut generator,
        &mut rand,
        rt_handle,
        8,
    )?;

    // Start the fuzzer
    fuzzer.fuzz_loop(&mut stages, &mut executor, &mut rand, state, rt_handle)
}

pub fn main() -> Result<()> {
    env_logger::init();

    // The state creation closure.
    let state_builder = |worker: &SimpleWorker| {
        // A queue policy to get testcasess from the corpus
        let scheduler = QueueScheduler::new();
        let crash_dir = worker.workdir().create_dir("crashes")?;

        // create a State from scratch
        StdState::new(
            BytesContext,
            // Corpus that will be evolved, we keep it in memory for performance
            InMemoryCorpus::new(scheduler),
            // Corpus in which we store solutions (crashes in this example),
            // on disk so the user can get them after stopping the fuzzer
            OnDiskCorpus::new(crash_dir, NopScheduler).unwrap(),
        )
    };

    // The launcher supervises the fuzzer and communicates with the workers.
    let controller = SimpleController::builder()
        .worker_stdout(None)
        .worker_stderr(None)
        .overwrite(true)
        .build()?;

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
    StdLauncher::builder()?
        .controller(controller)
        .monitor(monitor)
        .state_builder(state_builder)
        .runtime(runtime)
        // .build_with_task(run_fuzzer)?
        .build()?
        .launch()
}
