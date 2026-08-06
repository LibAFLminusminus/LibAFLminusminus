use clap::{self, Parser};
use core::time::Duration;
use libaflmm::{prelude::*, Result};
use libaflmm_bolts::{
    core_affinity::Cores,
    rands::StdRand,
    timers::FastTimer,
    tuples::{tuple_list, Merge},
};
use libaflmm_targets::prelude::*;
use mimalloc::MiMalloc;
use std::{env, path::PathBuf};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// The commandline args this fuzzer accepts
#[derive(Debug, Parser)]
#[command(
    name = "sqlite_launcher",
    about = "A fuzzer for ossfuzz with a launcher",
    author = "tokatoka <tokazerkje@outlook.com>, rmalmain <rmalmain@pm.me>"
)]
struct Opt {
    #[arg(
    short,
    long,
    value_parser = Cores::from_cmdline,
    help = "Spawn a client in each of the provided cores. Broker runs in the 0th core. 'all' to select all available cores. 'none' to run a client without binding to any core. eg: '1,2-4,6' selects the cores 1,2,3,4,6.",
    name = "CORES",
    default_value = "1",
    )]
    cores: Cores,

    #[arg(
        short,
        long,
        help = "Set an initial corpus directory",
        name = "INPUT",
        required = true
    )]
    input: Vec<PathBuf>,

    #[arg(
    value_parser = timeout_from_millis_str,
    short,
    long,
    help = "Set the exeucution timeout in milliseconds, default is 10000",
    name = "TIMEOUT",
    default_value = "10000"
    )]
    timeout: Duration,
}

/// Parse a millis string to a [`Duration`]. Used for arg parsing.
fn timeout_from_millis_str(time: &str) -> Result<Duration> {
    Ok(Duration::from_millis(time.parse()?))
}

/// The main fn, `no_mangle` as it is a C symbol
#[no_mangle]
pub extern "C" fn libafl_main() {
    env_logger::init();
    let opt = Opt::parse();

    let controller = StdController::builder()
        .overwrite(true)
        .build()
        .expect("Failed to build the SimpleController");

    // The monitor tracks the fuzzing current status.
    let monitor = WebMonitor::new("sqlite3", &controller);

    // Use the new fast timer
    let fast_timer = FastTimer::new();

    let group = StdGroup::builder(&controller)
        .timer(fast_timer)
        .timeout(Some(opt.timeout))
        .cores(opt.cores)
        .state_builder(|worker| {
            // A queue policy to get testcases from the corpus
            let scheduler = QueueScheduler::new();

            // create a State from scratch
            StdState::new(
                BytesContext::default(),
                // Corpus that will be evolved, we keep it in memory for performance
                InMemoryCorpus::new(scheduler),
                // Corpus in which we store solutions (crashes in this example),
                // on disk so the user can get them after stopping the fuzzer
                ObjectiveOnDiskCorpus::builder(worker)?.build()?,
            )
        })
        .build_inprocess(move |rt_handle, state| {
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

            println!("Worker is ready, let's fuzz :)");
            // Setup a basic mutator with a mutational stage
            let mutator = HavocScheduledMutator::new(havoc_mutations().merge(tokens_mutations()));
            let mut stages = tuple_list!(StdMutationalStage::new(mutator));

            // Create the executor for an in-process function with one observer for edge coverage and one for the execution time
            let executor = StdExecutor::new(
                state,
                |state, input| {
                    let context: &mut BytesContext = state.context_mut();
                    let buf = context.to_bytes(input);
                    unsafe {
                        libfuzzer_test_one_input(&buf);
                    }
                    Ok(ExitKind::Ok)
                },
                tuple_list!(edges_observer, time_observer),
                Some(Duration::new(10, 0)),
            );

            // A fuzzer with feedbacks and a corpus scheduler
            let mut fuzzer =
                StdFuzzer::new(executor, feedback, objective, &mut stages, state, rt_handle)?;

            // The actual target run starts here.
            // Call LLVMFuzzerInitialize() if present.
            let args: Vec<String> = env::args().collect();
            if unsafe { libfuzzer_initialize(&args) } == -1 {
                println!("Warning: LLVMFuzzerInitialize failed with -1");
            }

            // This fuzzer restarts after 1 mio `fuzz_one` executions.
            // Each fuzz_one will internally do many executions of the target.
            // If your target is very unstable, setting a low count here may help.
            // However, you will lose a lot of performance that way.
            let iters = 1_000_000;
            let mut rand = StdRand::new();
            // Load the initial corpus. Already loaded inputs are skipped on restart.
            for input_dir in &opt.input {
                fuzzer
                    .load_dir(input_dir, state, rt_handle)
                    .unwrap_or_else(|e| {
                        panic!("Failed to load initial corpus at {input_dir:?}: {e:?}")
                    });
            }

            fuzzer.fuzz_loop_for(&mut stages, &mut rand, state, rt_handle, iters)?;

            // Restart the runtime after fuzzer have completed all iterations
            unsafe { rt_handle.restart(state) }
        })
        .unwrap();

    // Launch the fuzzer
    StdLauncher::builder()
        .controller(controller)
        .monitor(monitor)
        .add_group(group)
        .build()
        .unwrap()
        .launch()
        .unwrap()
}
