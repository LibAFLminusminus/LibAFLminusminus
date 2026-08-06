//! A libfuzzer-like fuzzer with llmp-multithreading support and restarts
//! The example harness is built for libpng.
use std::{cell::RefCell, rc::Rc};

use core::time::Duration;
use frida_gum::Gum;
use libaflmm::{
    controllers::{SimpleWorker, StdController, Worker},
    corpus::{schedulers::QueueScheduler, Corpus, InMemoryCorpus, OnDiskCorpus},
    executors::ExitKind,
    feedback_or, feedback_or_fast,
    feedbacks::{CrashFeedback, MaxMapFeedback, TimeFeedback, TimeoutFeedback},
    fuzzers::{Fuzzer, StdFuzzer},
    inputs::{BytesContext, BytesInput, InputContext},
    launchers::{StdLauncher, DEFAULT_MAX_STATE_SIZE_PER_WORKER},
    monitors::WebMonitor,
    mutators::{
        havoc_mutations::havoc_mutations,
        scheduled::{tokens_mutations, HavocScheduledMutator},
        token_mutations::I2SRandReplace,
    },
    observers::{CmpLogObserver, HitcountsMapObserver, StdMapObserver, TimeObserver},
    runtimes::{RuntimeHandle, StdInProcessRuntime},
    stages::{
        cmplog_post_hook, cmplog_pre_hook, constrain, IfElseStage, SingleRunStage,
        StdMutationalStage,
    },
    states::{State, StdState},
};
use libaflmm_bolts::{
    rands::StdRand,
    tuples::{tuple_list, Merge},
    FastTimer, Result,
};
use libaflmm_frida::{
    asan::{
        asan_rt::AsanRuntime,
        errors::{AsanErrorsFeedback, AsanErrorsObserver},
    },
    cmplog_rt::CmpLogRuntime,
    coverage_rt::{CoverageRuntime, MAP_SIZE},
    executor::FridaExecutor,
    helper::{FridaInstrumentationHelper, IfElseRuntime},
    options::parse_args,
};
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// The main fn, usually parsing parameters, and starting the fuzzer
pub fn main() -> Result<()> {
    env_logger::init();
    color_backtrace::install();

    log::info!("Frida fuzzer starting up.");

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
    let controller = StdController::builder()
        .worker_stdout(None)
        .silence_stderr()
        .overwrite(true)
        .build()?;

    // The monitor tracks the fuzzing current status.
    let monitor = WebMonitor::new("frida_libpng", &controller);

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
        .launch()?;

    Ok(())
}

/// The actual fuzzer
fn run_fuzzer<C, OC>(
    rt_handle: &mut RuntimeHandle<StdState<C, BytesContext, BytesInput, OC>, SimpleWorker>,
    state: &mut StdState<C, BytesContext, BytesInput, OC>,
) -> Result<()>
where
    C: Corpus<Input = BytesInput>,
    OC: Corpus<Input = BytesInput>,
{
    let options = parse_args();

    // 'While the stats are state, they are usually used in the broker - which is likely never restarted
    let is_asan = {
        let core_id = rt_handle.worker().core_id();
        options.asan && options.asan_cores.contains(core_id)
    };
    let is_cmplog = {
        let core_id = rt_handle.worker().core_id();
        options.cmplog && options.cmplog_cores.contains(core_id)
    };

    // The restarting state will spawn the same process again as child, then restarted it each time it crashes.

    // println!("{:?}", mgr.mgr_id());

    let lib = unsafe { libloading::Library::new(options.clone().harness.unwrap()).unwrap() };
    let target_func: libloading::Symbol<unsafe extern "C" fn(data: *const u8, size: usize) -> i32> =
        unsafe { lib.get(options.harness_function.as_bytes()).unwrap() };

    let frida_harness = |state: &mut StdState<_, BytesContext, BytesInput, _>,
                         input: &BytesInput| {
        let context: &mut BytesContext = state.context_mut();
        let buf = context.to_bytes(input);
        unsafe { (target_func)(buf.as_ptr(), buf.len()) };
        Ok(ExitKind::Ok)
    };

    let gum = Gum::obtain();

    let coverage = CoverageRuntime::new();
    let asan = AsanRuntime::new(&options);
    let cmplog = CmpLogRuntime::new();

    let frida_helper = Rc::new(RefCell::new(FridaInstrumentationHelper::new(
        &gum,
        &options,
        tuple_list!(
            IfElseRuntime::new(move || Ok(is_asan), tuple_list!(asan), tuple_list!()),
            IfElseRuntime::new(move || Ok(is_cmplog), tuple_list!(cmplog), tuple_list!()),
            coverage
        ),
    )));

    // Create an observation channel using the coverage map
    let edges_observer = HitcountsMapObserver::new(unsafe {
        StdMapObserver::from_mut_ptr(
            "edges",
            frida_helper.borrow_mut().map_mut_ptr().unwrap(),
            MAP_SIZE,
        )
    });

    // Create an observation channel to keep track of the execution time
    let time_observer = TimeObserver::new("time");
    let asan_observer = AsanErrorsObserver::from_static_asan_errors();
    // Create an observation channel using cmplog map
    let cmplog_observer = CmpLogObserver::new("cmplog", true);

    // Feedback to rate the interestingness of an input
    // This one is composed by two Feedbacks in OR
    let feedback = feedback_or!(
        // New maximization map feedback linked to the edges observer and the feedback state
        MaxMapFeedback::new(&edges_observer),
        // Time feedback, this one does not need a feedback state
        TimeFeedback::new(&time_observer)
    );

    // Feedbacks to recognize an input as solution
    let objective = feedback_or_fast!(
        CrashFeedback::new(),
        AsanErrorsFeedback::new(&asan_observer),
        TimeoutFeedback::new(),
    );

    println!("We're a client, let's fuzz :)");

    // Setup a basic mutator with a mutational stage
    let mutator = HavocScheduledMutator::new(havoc_mutations().merge(tokens_mutations()));

    // A minimization+queue policy to get testcasess from the corpus
    // A fuzzer with feedbacks and a corpus scheduler

    let observers = tuple_list!(
        edges_observer,
        time_observer,
        cmplog_observer,
        asan_observer
    );

    // Create the executor
    let executor = FridaExecutor::new(state, frida_harness, observers, &gum, frida_helper);

    // Setup a randomic Input2State stage
    let i2s = StdMutationalStage::new(HavocScheduledMutator::new(tuple_list!(
        I2SRandReplace::new()
    )));

    let tracing = SingleRunStage::new(cmplog_pre_hook, cmplog_post_hook);

    let mut stages = tuple_list!(
        IfElseStage::new(
            constrain(move |_, _, _, _| Ok(is_cmplog)),
            tuple_list!(tracing, i2s),
            tuple_list!()
        ),
        StdMutationalStage::new(mutator)
    );
    let mut fuzzer = StdFuzzer::new(executor, feedback, objective, &mut stages, state, rt_handle)?;

    let mut rand = StdRand::new();

    // load initial corpus
    for input_dir in &options.input {
        fuzzer.load_dir(input_dir, state, rt_handle)?;
    }

    // fuzz
    fuzzer.fuzz_loop(&mut stages, &mut rand, state, rt_handle)?;

    Ok(())
}
