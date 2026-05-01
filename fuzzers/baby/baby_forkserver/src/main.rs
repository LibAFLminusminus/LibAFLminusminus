use std::{ops::DerefMut, path::PathBuf, time::Duration};

use clap::Parser;
use libafl::{
    Result, Worker,
    corpus::{
        Corpus, InMemoryCorpus, OnDiskCorpus, Scheduler,
        schedulers::{NopScheduler, QueueScheduler},
    },
    executors::{ForkserverExecutor, StdChildArgs},
    feedback_or_fast,
    feedbacks::{CrashFeedback, MaxMapFeedback, TimeoutFeedback},
    fuzzers::{Fuzzer, StdFuzzer},
    generators::RandPrintablesGenerator,
    inputs::{BytesInput, bytes::BytesContext},
    launchers::{DEFAULT_MAX_STATE_SIZE_PER_CLIENT, StdLauncher},
    monitors::SimpleMonitor,
    mutators::{HavocScheduledMutator, Tokens, havoc_mutations},
    non_zero,
    observers::{HitcountsMapObserver, StdMapObserver},
    runtimes::{RuntimeHandle, simple::SimpleRuntime},
    simple::{SimpleController, SimpleWorker},
    stages::StdMutationalStage,
    states::StdState,
};
use libafl_bolts::{
    StdTargetArgs, SysVShm, current_nanos, rands::StdRand, timers::FastTimer, tuples::tuple_list,
};

/// The commandline args this fuzzer accepts
#[derive(Debug, Parser)]
#[command(
    name = "forkserver_simple",
    about = "This is a simple example fuzzer to fuzz a executable instrumented by afl-cc.",
    author = "tokatoka <tokazerkje@outlook.com>"
)]
struct Opt {
    #[arg(
        help = "The instrumented binary we want to fuzz",
        name = "EXEC",
        required = true
    )]
    executable: String,

    #[arg(
        help = "The directory to read initial inputs from ('seeds')",
        name = "INPUT_DIR",
        required = true
    )]
    in_dir: PathBuf,

    #[arg(
        help = "Timeout for each individual execution, in milliseconds",
        short = 't',
        long = "timeout",
        default_value = "1200"
    )]
    timeout: u64,

    #[arg(
        help = "If not set, the child's stdout and stderror will be redirected to /dev/null",
        short = 'd',
        long = "debug-child",
        default_value = "false"
    )]
    debug_child: bool,

    #[arg(
        help = "Arguments passed to the target",
        name = "arguments",
        num_args(1..),
        allow_hyphen_values = true,
    )]
    arguments: Vec<String>,
}

fn run_fuzzer<C, OC, SC>(
    rt_handle: &mut RuntimeHandle<StdState<C, BytesInput, OC, SC>, SimpleWorker>,
    state: &mut StdState<C, BytesInput, OC, SC>,
) -> Result<()>
where
    C: Corpus<BytesInput>,
    OC: Corpus<BytesInput>,
    SC: Scheduler,
{
    const MAP_SIZE: usize = 65536;
    let opt = Opt::parse();
    // The source of randomness
    let mut rand = StdRand::with_seed(current_nanos());
    let mut shmem_buf = SysVShm::new(MAP_SIZE).unwrap();

    unsafe {
        shmem_buf.write_to_env("__AFL_SHM_ID").unwrap();
    }

    // Create an observation channel using the signals map
    let observer = unsafe {
        HitcountsMapObserver::new(StdMapObserver::new("shared_mem", shmem_buf.deref_mut()))
    };
    // Feedback to rate the interestingness of an input
    let feedback = MaxMapFeedback::new(&observer);

    // A feedback to choose if an input is a solution or not
    // let objective_feedback = CrashFeedback::new();
    let objective_feedback = feedback_or_fast!(CrashFeedback::new(), TimeoutFeedback::new());

    // Setup a mutational stage with a basic bytes mutator
    let mutator = HavocScheduledMutator::new(havoc_mutations());
    let mut stages = tuple_list!(StdMutationalStage::new(mutator));

    let args = opt.arguments;
    let mut tokens = Tokens::new();
    // Create the executor for an in-process function with just one observer
    let mut executor = ForkserverExecutor::builder()
        .program("./program")
        .debug_child(false)
        .autotokens(&mut tokens)
        .parse_afl_cmdline(args)
        .coverage_map_size(MAP_SIZE)
        .try_use_input_shmem(true)
        .timeout(Duration::from_millis(3000))
        .build(tuple_list!(observer))
        .unwrap();
    // Generator of printable bytearrays of max size 32
    let mut generator = RandPrintablesGenerator::new(non_zero!(32));

    // A fuzzer with feedbacks and a corpus scheduler
    let mut fuzzer = StdFuzzer::new(
        feedback,
        objective_feedback,
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
            // Corpus that will be evolved, we keep it in memory for performance
            InMemoryCorpus::new(BytesContext, scheduler),
            // Corpus in which we store solutions (crashes in this example),
            // on disk so the user can get them after stopping the fuzzer
            OnDiskCorpus::new(crash_dir, BytesContext, NopScheduler).unwrap(),
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
    let runtime = SimpleRuntime::new(run_fuzzer);

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
