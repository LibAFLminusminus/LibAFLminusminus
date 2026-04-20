use std::path::PathBuf;

use libafl::{
    corpus::{
        schedulers::{NopScheduler, QueueScheduler},
        Corpus, InMemoryCorpus, OnDiskCorpus, Scheduler,
    },
    executors::StdExecutor,
    feedbacks::{CrashFeedback, MaxMapFeedback},
    fuzzer::{Fuzzer, StdFuzzer},
    generators::RandPrintablesGenerator,
    inputs::{bytes::BytesContext, BytesInput},
    mutators::{havoc_mutations, HavocScheduledMutator},
    non_zero,
    nop::NopController,
    observers::ConstMapObserver,
    runtimes::{direct::DirectRuntime, Runtime, RuntimeHandle},
    stages::StdMutationalStage,
    state::StdState,
    Error,
};
use libafl_bolts::{current_nanos, nonnull_raw_mut, rands::StdRand, tuples::tuple_list};

use crate::target::SIGNALS;

mod target;

fn run_fuzzer<C, OC, SC>(
    rt_handle: &mut RuntimeHandle<'_, NopController, StdState<C, BytesInput, OC, SC>>,
    state: &mut StdState<C, BytesInput, OC, SC>,
) -> Result<(), Error>
where
    C: Corpus<BytesInput>,
    OC: Corpus<BytesInput>,
    SC: Scheduler,
{
    env_logger::init();

    // The source of randomness
    let mut rand = StdRand::with_seed(current_nanos());

    // Create an observation channel using the signals map
    let observer = unsafe { ConstMapObserver::from_mut_ptr("signals", nonnull_raw_mut!(SIGNALS)) };

    // Feedback to rate the interestingness of an input
    let feedback = MaxMapFeedback::new(&observer);

    // A feedback to choose if an input is a solution or not
    let objective_feedback = CrashFeedback::new();

    // Setup a mutational stage with a basic bytes mutator
    let mutator = HavocScheduledMutator::new(havoc_mutations());
    let mut stages = tuple_list!(StdMutationalStage::new(mutator));

    // Create the executor for an in-process function with just one observer
    let mut executor = StdExecutor::new(target::target, tuple_list!(observer), None);

    // A fuzzer with feedbacks and a corpus scheduler
    let mut fuzzer = StdFuzzer::new(feedback, objective_feedback);

    // Initialize the fuzzer
    fuzzer.init(&mut stages, &mut executor, state, rt_handle)?;

    // Generator of printable bytearrays of max size 32
    let mut generator = RandPrintablesGenerator::new(non_zero!(32));

    // Generate 8 initial inputs
    state.generate_initial_inputs(
        &mut fuzzer,
        &mut executor,
        &mut generator,
        &mut rand,
        rt_handle,
        8,
    )?;

    fuzzer.fuzz_loop(&mut stages, &mut executor, &mut rand, state, rt_handle)
}

pub fn main() {
    // A queue policy to get testcasess from the corpus
    let scheduler = QueueScheduler::new();

    // create a State from scratch
    let state = StdState::new(
        // Corpus that will be evolved, we keep it in memory for performance
        InMemoryCorpus::new(BytesContext, scheduler),
        // Corpus in which we store solutions (crashes in this example),
        // on disk so the user can get them after stopping the fuzzer
        OnDiskCorpus::new(PathBuf::from("./crashes"), BytesContext, NopScheduler).unwrap(),
    )
    .unwrap();

    let mut runtime = DirectRuntime::new(state, run_fuzzer);

    runtime.run(&mut NopController).unwrap()
}
