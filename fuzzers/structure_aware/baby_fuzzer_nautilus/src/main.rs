use crate::target::SIGNALS;
use libaflmm::{Result, prelude::*};
use libaflmm_bolts::{FastTimer, nonnull_raw_mut, rands::StdRand, tuples::tuple_list};
use std::{path::PathBuf, time::Duration};

mod target;

/// The grammar the fuzzer generates its inputs from
const GRAMMAR: &str = "grammar.json";

/// The maximum depth of the generated trees
const TREE_DEPTH: usize = 15;

pub fn main() -> Result<()> {
    env_logger::init();

    // The launcher supervises the fuzzer and communicates with the workers.
    // The target prints every input it gets, keep it out of the terminal.
    let controller = StdController::builder()
        .worker_stdout(WorkdirFile::Path(PathBuf::from("stdout.log")))
        .overwrite(true)
        .build()?;

    // The monitor tracks the fuzzing current status.
    let monitor = StdMonitor::new();

    let group = StdGroup::builder(&controller)
        .timeout(Some(Duration::from_secs(3)))
        .timer(FastTimer::new())
        .state_builder(|worker| {
            // A scheduler following the queue policy
            let scheduler = QueueScheduler::new();

            // create a State from scratch.
            // The context is the grammar, the state uses it to turn an input into the
            // bytes the target consumes.
            StdState::new(
                NautilusContext::from_file(TREE_DEPTH, GRAMMAR)?,
                // Corpus that will be evolved, we keep it in memory for performance
                // It must have a scheduler
                InMemoryCorpus::new(scheduler),
                // Corpus in which we store solutions (crashes in this example),
                // on disk so the user can get them after stopping the fuzzer
                ObjectiveOnDiskCorpus::builder(worker)?.build()?,
            )
        })
        .build_inprocess(|rt_handle, state| {
            // The source of randomness
            let mut rand = StdRand::new();

            // The generator, the feedback and the mutators need the grammar as well.
            // The state owns its own context, this one is theirs.
            let ctx = NautilusContext::from_file(TREE_DEPTH, GRAMMAR)?;

            // Create an observation channel using the signals map
            let observer =
                unsafe { ConstMapObserver::from_mut_ptr("signals", nonnull_raw_mut!(SIGNALS)) };

            // Feedback to rate the interestingness of an input
            // the nautilus feedback stores the chunks it collects in the worker's workdir
            let feedback = feedback_or!(
                StdFeedback::new(&observer),
                NautilusFeedback::new(&ctx, rt_handle.worker())?
            );

            // A feedback to choose if an input is a solution or not
            let objective_feedback =
                feedback_or_fast!(CrashFeedback::new(), TimeoutFeedback::new());

            // Setup a mutational stage with the grammar mutators
            let mutator = HavocScheduledMutator::with_max_stack_pow(
                tuple_list!(
                    NautilusRandomMutator::new(&ctx),
                    NautilusRandomMutator::new(&ctx),
                    NautilusRandomMutator::new(&ctx),
                    NautilusRandomMutator::new(&ctx),
                    NautilusRandomMutator::new(&ctx),
                    NautilusRandomMutator::new(&ctx),
                    NautilusRecursionMutator::new(&ctx),
                    NautilusSpliceMutator::new(&ctx),
                    NautilusSpliceMutator::new(&ctx),
                    NautilusSpliceMutator::new(&ctx),
                ),
                2,
            );
            let mut stages = tuple_list!(StdMutationalStage::new(mutator));

            // Create the executor for an in-process function with just one observer
            let executor = StdExecutor::new(state, target::target, tuple_list!(observer), None);

            // Generator of inputs following the grammar
            let mut generator = NautilusGenerator::new(&ctx);

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
            fuzzer.load_generator(&mut generator, &mut rand, 8, state, rt_handle)?;

            // Start the fuzzer
            fuzzer.fuzz_loop(&mut stages, &mut rand, state, rt_handle)?;

            Ok(())
        })?;

    // Launch the fuzzer
    StdLauncher::empty()
        .controller(controller)
        .monitor(monitor)
        .add_group(group)
        .build()?
        .launch()
}
