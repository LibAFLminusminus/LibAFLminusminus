use libaflmm::{Result, prelude::*};
use libaflmm_bolts::{non_zero, rands::StdRand, tuples::tuple_list};
use libaflmm_nyx::{executor::NyxExecutor, helper::NyxHelper, settings::NyxSettings};

pub fn main() -> Result<()> {
    env_logger::init();

    // The launcher supervises the fuzzer and communicates with the workers.
    let controller = SimpleController::builder().overwrite(true).build()?;

    // The monitor tracks the fuzzing current status.
    let monitor = SimpleMonitor::new();

    // Launch the fuzzer
    StdLauncher::builder()?
        .controller(controller)
        .monitor(monitor)
        .state_builder(|worker| {
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
        })
        .build_forkserver(|rt_handle, state| {
            // nyx stuff
            let settings = NyxSettings::builder().cpu_id(0).parent_cpu_id(None).build();
            let helper = NyxHelper::new("/tmp/nyx_libxml2/", settings).unwrap();
            let observer = unsafe {
                StdMapObserver::from_mut_ptr("trace", helper.bitmap_buffer, helper.bitmap_size)
            };

            let mut rand = StdRand::new();

            // libafl stuff
            let feedback = MaxMapFeedback::new(&observer);
            let objective = CrashFeedback::new();

            // switch monitor if you want
            // let monitor = SimpleMonitor::new(|x|-> () {println!("{}",x)});
            let executor = NyxExecutor::builder().build(helper, tuple_list!(observer));

            let mut stages = tuple_list!(StdStage::default());
            let mut fuzzer =
                StdFuzzer::new(executor, feedback, objective, &mut stages, state, rt_handle)?;
            // Generator of printable bytearrays of max size 32
            let mut generator = RandPrintablesGenerator::new(non_zero!(32));
            state.generate_initial_inputs(&mut fuzzer, &mut generator, &mut rand, rt_handle, 8)?;
            // start fuzz
            fuzzer.fuzz_loop(&mut stages, &mut rand, state, rt_handle)
        })?
        .launch()
}
