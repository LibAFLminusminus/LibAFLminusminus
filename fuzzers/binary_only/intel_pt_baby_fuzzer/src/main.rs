use crate::target::{MAP, MAP_PTR, MAP_SIZE};
use libaflmm::{prelude::*, Result};
use libaflmm_bolts::{
    current_nanos, nonnull_raw_mut, rands::StdRand, tuples::tuple_list, FastTimer,
};
use libaflmm_intelpt::{availability, IntelPT, IntelPTHook, PtImage};
use proc_maps::get_process_maps;
use std::{path::PathBuf, process, ptr::copy_nonoverlapping, time::Duration};

mod target;

fn executable_memory() -> Result<Vec<(Vec<u8>, u64)>> {
    let my_pid = i32::try_from(process::id()).unwrap();
    let process_maps = get_process_maps(my_pid)?;

    Ok(process_maps
        .iter()
        .filter_map(|pm| {
            if pm.is_exec() && pm.filename().is_some() && pm.inode != 0 {
                let mut data = vec![0; pm.size()];
                unsafe {
                    copy_nonoverlapping(pm.start() as *const u8, data.as_mut_ptr(), data.len())
                }
                Some((data, pm.start() as u64))
            } else {
                None
            }
        })
        .collect())
}

pub fn main() -> Result<()> {
    env_logger::init();

    if let Err(reasons) = availability() {
        eprintln!("Intel PT is not available: {reasons}");
        return Ok(());
    }

    // Launch the fuzzer
    StdLauncher::builder()
        .timeout(Some(Duration::from_secs(5)))
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
        .launch_inprocess(|rt_handle, state| {
            // The source of randomness
            let mut rand = StdRand::with_seed(current_nanos());

            // Create an observation channel using the map
            let observer =
                unsafe { ConstMapObserver::from_mut_ptr("signals", nonnull_raw_mut!(MAP)) };

            // Feedback to rate the interestingness of an input
            let feedback = StdFeedback::new(&observer);

            // A feedback to choose if an input is a solution or not.
            let objective_feedback =
                feedback_or_fast!(CrashFeedback::new(), TimeoutFeedback::new());

            // The images the traces get decoded against.
            // They must outlive the tracer, thus the executor.
            let memory = executable_memory()?;
            let images: Vec<PtImage> = memory
                .iter()
                .map(|(data, addr)| PtImage::new(data, *addr))
                .collect();

            let pt = IntelPT::builder().images(&images).build()?;

            // Intel PT hook that will handle the setup of Intel PT for each execution and fill the map
            let pt_hook = unsafe {
                IntelPTHook::builder()
                    .intel_pt(pt)
                    .map_ptr(MAP_PTR)
                    .map_len(MAP_SIZE)
            }
            .build();

            // Setup a mutational stage with a basic bytes mutator
            let mut stages = tuple_list!(StdStage::default());

            // Create the executor for an in-process function with just one observer
            let executor = StdExecutor::with_hooks(
                state,
                tuple_list!(pt_hook),
                target::target,
                tuple_list!(observer),
                None,
            );

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
            fuzzer.load_generator(&mut generator, &mut rand, 8, state, rt_handle)?;

            // Start the fuzzer
            fuzzer.fuzz_loop(&mut stages, &mut rand, state, rt_handle)?;

            Ok(())
        })
}
