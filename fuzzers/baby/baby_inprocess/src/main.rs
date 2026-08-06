use crate::target::SIGNALS;
use libaflmm::{Result, prelude::*};
use libaflmm_bolts::{
    FastTimer, current_nanos, nonnull_raw_mut, rands::StdRand, tuples::tuple_list,
};
use std::time::Duration;

mod target;

pub fn main() -> Result<()> {
    env_logger::init();

    // The launcher supervises the fuzzer and communicates with the workers.
    let controller = StdController::builder().overwrite(true).build()?;

    // The monitor tracks the fuzzing current status.
    let monitor = StdMonitor::new();

    let group = StdGroup::builder(&controller)
        .timeout(Some(Duration::from_secs(3)))
        .timer(FastTimer::new())
        .state_builder(|worker| {
            // A scheduler following the queue policy
            let scheduler = QueueScheduler::new();

            // create a State from scratch
            StdState::new(
                BytesContext,
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
            let mut rand = StdRand::with_seed(current_nanos());

            // Create an observation channel using the signals map
            let observer =
                unsafe { ConstMapObserver::from_mut_ptr("signals", nonnull_raw_mut!(SIGNALS)) };

            // Feedback to rate the interestingness of an input
            let feedback = StdFeedback::new(&observer);

            // A feedback to choose if an input is a solution or not
            let objective_feedback =
                feedback_or_fast!(CrashFeedback::new(), TimeoutFeedback::new());

            // Setup a mutational stage with a basic bytes mutator
            let mut stages = tuple_list!(StdStage::default());

            // Create the executor for an in-process function with just one observer
            let executor = StdExecutor::new(state, target::target, tuple_list!(observer), None);

            // Generator of printable bytearrays of max size 32
            let mut generator = RandPrintablesGenerator::new(non_zero!(32));

            let calibration_hk = CalibrationHook::new(&feedback);

            // A fuzzer with feedbacks and a corpus scheduler
            let mut fuzzer = StdFuzzer::with_hooks(
                executor,
                feedback,
                objective_feedback,
                tuple_list!(calibration_hk),
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
    StdLauncher::builder()
        .controller(controller)
        .monitor(monitor)
        .add_group(group)
        .build()?
        .launch()
}
