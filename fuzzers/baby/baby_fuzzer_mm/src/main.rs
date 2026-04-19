use std::path::PathBuf;

use libafl::{
    Error,
    corpus::{
        InMemoryCorpus, OnDiskCorpus,
        schedulers::{NopScheduler, QueueScheduler},
    },
    executors::StdExecutor,
    feedbacks::{CrashFeedback, MaxMapFeedback},
    fuzzer::StdFuzzer,
    generators::RandPrintablesGenerator,
    inputs::NopContext,
    non_zero,
    nop::NopController,
    observers::ConstMapObserver,
    runtimes::Runtime,
    runtimes::{RuntimeHandle, direct::DirectRuntime},
    state::StdState,
};
use libafl_bolts::{current_nanos, nonnull_raw_mut, rands::StdRand, tuples::tuple_list};

use crate::target::SIGNALS;

mod target;

fn run_fuzzer<CT, S>(
    driver: &mut RuntimeHandle<CT, S>,
    state: &mut S,
    controller: &mut CT,
) -> Result<(), Error> {
    env_logger::init();

    // Create an observation channel using the signals map
    let observer = unsafe { ConstMapObserver::from_mut_ptr("signals", nonnull_raw_mut!(SIGNALS)) };

    // Feedback to rate the interestingness of an input
    let mut feedback = MaxMapFeedback::new(&observer);

    // A feedback to choose if an input is a solution or not
    let mut objective_feedback = CrashFeedback::new();

    // A fuzzer with feedbacks and a corpus scheduler
    let mut fuzzer = StdFuzzer::new(feedback, objective_feedback);

    // Create the executor for an in-process function with just one observer
    let mut executor = StdExecutor::new(target::target, tuple_list!(observer), None);

    // Generator of printable bytearrays of max size 32
    let mut generator = RandPrintablesGenerator::new(non_zero!(32));

    // Generate 8 initial inputs
    state.generate_initial_inputs(&mut fuzzer, &mut executor, &mut generator, 8)?;

    // Setup a mutational stage with a basic bytes mutator
    let mutator = HavocScheduledMutator::new(havoc_mutations());
    let mut stages = tuple_list!(StdMutationalStage::new(mutator));

    fuzzer.fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
}

pub fn main() {
    // A queue policy to get testcasess from the corpus
    let scheduler = QueueScheduler::new();

    // create a State from scratch
    let state = StdState::new(
        // RNG
        StdRand::with_seed(current_nanos()),
        // Corpus that will be evolved, we keep it in memory for performance
        InMemoryCorpus::new(NopContext, scheduler),
        // Corpus in which we store solutions (crashes in this example),
        // on disk so the user can get them after stopping the fuzzer
        OnDiskCorpus::new(PathBuf::from("./crashes"), NopContext, NopScheduler).unwrap(),
    )
    .unwrap();

    let mut runner = DirectRuntime::new(state, run_fuzzer);

    runner.run(&mut NopController).unwrap()
}
