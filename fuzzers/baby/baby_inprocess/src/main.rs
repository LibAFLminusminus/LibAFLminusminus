use crate::target::SIGNALS;
use libaflmm::prelude::*;
use libaflmm_bolts::{
    current_nanos, nonnull_raw_mut, rands::StdRand, timers::FastTimer, tuples::tuple_list,
};
use std::time::Duration;

mod target;

fn run_fuzzer<C, OC>(
    rt_handle: &mut RuntimeHandle<StdState<C, BytesContext, BytesInput, OC>, SimpleWorker>,
    state: &mut StdState<C, BytesContext, BytesInput, OC>,
) -> Result<()>
where
    C: Corpus<Input = BytesInput>,
    OC: Corpus<Input = BytesInput>,
{
    // The source of randomness
    let mut rand = StdRand::with_seed(current_nanos());

    // Create an observation channel using the signals map
    let observer = unsafe { ConstMapObserver::from_mut_ptr("signals", nonnull_raw_mut!(SIGNALS)) };

    // Feedback to rate the interestingness of an input
    let feedback = StdFeedback::new(&observer);

    // A feedback to choose if an input is a solution or not
    let objective_feedback = feedback_or_fast!(CrashFeedback::new(), TimeoutFeedback::new());

    // Setup a mutational stage with a basic bytes mutator
    let mut stages = tuple_list!(StdStage::default());

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
        // A scheduler following the queue policy
        let scheduler = QueueScheduler::new();
        // The default objective directory
        let crash_dir = worker.workdir().objective_dir()?;

        // create a State from scratch
        StdState::new(
            BytesContext,
            // Corpus that will be evolved, we keep it in memory for performance
            // It must have a scheduler
            InMemoryCorpus::with_scheduler(scheduler),
            // Corpus in which we store solutions (crashes in this example),
            // on disk so the user can get them after stopping the fuzzer
            OnDiskCorpus::builder().root_dir(crash_dir).build()?,
        )
    };

    // The launcher supervises the fuzzer and communicates with the workers.
    let controller = StdController::builder().overwrite(true).build()?;

    // The monitor tracks the fuzzing current status.
    let monitor = SimpleMonitor::new();

    // A fast timer, much faster than classic OS timers.
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
        .build()?
        .launch()
}
