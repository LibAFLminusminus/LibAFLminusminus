use clap::Parser;
use libaflmm::{Result, prelude::*};
use libaflmm_bolts::{StdTargetArgs, SysVShm, current_nanos, rands::StdRand, tuples::tuple_list};
use libaflmm_core::forkserver::{AFLPP_CMPLOG_MAP, SHM_CMPLOG_ENV_VAR, SHM_ENV_VAR};
use libaflmm_targets::cmps::{AFLppCmplogVals, AFLppLibAFLCmpLogHeader};
use std::{ops::DerefMut, path::PathBuf, time::Duration};

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
        help = "The cmplog instrumented binary we want to fuzz",
        name = "CMPLOG",
        required = true
    )]
    cmplog: String,

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

pub fn main() -> Result<()> {
    env_logger::init();

    // The state creation closure.
    let state_builder = |worker: &SimpleWorker| {
        // A queue policy to get testcasess from the corpus
        let scheduler = QueueScheduler::new();

        // create a State from scratch
        StdState::new(
            BytesContext,
            // Corpus that will be evolved, we keep it in memory for performance
            InMemoryCorpus::new(scheduler),
            // Corpus in which we store solutions (crashes in this example),
            // on disk so the user can get them after stopping the fuzzer
            ObjectiveOnDiskCorpus::builder(worker)?.build()?,
        )
    };

    // The launcher supervises the fuzzer and communicates with the workers.
    let controller = SimpleController::builder().overwrite(true).build()?;

    // The monitor tracks the fuzzing current status.
    let monitor = SimpleMonitor::new();

    let group = StdGroup::builder(&controller)
        .state_builder(state_builder)
        .build_forkserver(|rt_handle, state| {
            const MAP_SIZE: usize = 65536;
            let opt = Opt::parse();
            // The source of randomness
            let mut rand = StdRand::with_seed(current_nanos());
            let mut shmem_buf = SysVShm::new(MAP_SIZE).unwrap();
            let cmplog_buf = SysVShm::new(AFLPP_CMPLOG_MAP).unwrap();

            unsafe {
                shmem_buf.write_to_env(SHM_ENV_VAR).unwrap();
                cmplog_buf.write_to_env(SHM_CMPLOG_ENV_VAR).unwrap()
            }

            // Create an observation channel using the signals map
            let observer = unsafe {
                HitcountsMapObserver::new(StdMapObserver::new("shared_mem", shmem_buf.deref_mut()))
            };

            let cmplog_observer =
                CmpLogObserver::<AFLppLibAFLCmpLogHeader, AFLppCmplogVals>::from_shm(
                    "cmplog_map",
                    cmplog_buf,
                    true,
                )
                .unwrap();
            // Feedback to rate the interestingness of an input
            let feedback = MaxMapFeedback::new(&observer);

            // A feedback to choose if an input is a solution or not
            // let objective_feedback = CrashFeedback::new();
            let objective_feedback =
                feedback_or_fast!(CrashFeedback::new(), TimeoutFeedback::new());

            let args = opt.arguments;
            let mut tokens = Tokens::new();
            // Create the executor for an in-process function with just one observer
            let executor = ForkserverExecutor::builder()
                .program(opt.executable)
                .debug_child(false)
                .autotokens(&mut tokens)
                .args(args.clone())
                .coverage_map_size(MAP_SIZE)
                .try_use_input_shmem()
                .timeout(Duration::from_millis(3000))
                .build(tuple_list!(observer), rt_handle)
                .unwrap();

            let secondary = ForkserverExecutor::builder()
                .program(opt.cmplog)
                .debug_child(true)
                .autotokens(&mut tokens)
                .args(args)
                .coverage_map_size(MAP_SIZE)
                .try_use_input_shmem()
                .timeout(Duration::from_millis(3000))
                .build(tuple_list!(cmplog_observer), rt_handle)
                .unwrap();

            // Setup a mutational stage with a basic bytes mutator
            let tracer = TracerStage::new(secondary);

            let mut stages = tuple_list!(StdStage::default(), tracer);

            // Generator of printable bytearrays of max size 32
            let mut generator = RandPrintablesGenerator::new(non_zero!(32));

            // A fuzzer with feedbacks and a corpus scheduler
            let mut fuzzer = StdFuzzer::new(
                executor,
                feedback,
                objective_feedback,
                &mut stages,
                state,
                rt_handle,
            )?;

            // Generate 8 initial inputs
            state.generate_initial_inputs(&mut fuzzer, &mut generator, &mut rand, rt_handle, 8)?;

            // Start the fuzzer
            fuzzer.fuzz_loop(&mut stages, &mut rand, state, rt_handle)
        })?;

    // Launch the fuzzer
    StdLauncher::builder()
        .controller(controller)
        .monitor(monitor)
        .add_group(group)
        .build()?
        .launch()
}
