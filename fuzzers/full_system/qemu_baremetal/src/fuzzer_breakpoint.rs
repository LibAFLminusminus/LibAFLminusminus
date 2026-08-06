//! A fuzzer using qemu in systemmode for binary-only coverage of kernels

use libaflmm::{Result, prelude::*};
use libaflmm_bolts::{
    core_affinity::Cores, ownedref::OwnedMutSlice, rands::StdRand, tuples::tuple_list,
};
use libaflmm_qemu::prelude::*;
use libaflmm_targets::{
    constants::EDGES_MAP_DEFAULT_SIZE,
    coverage::{MAX_EDGES_FOUND, edges_map_mut_ptr},
};
use std::{env, path::PathBuf, time::Duration};

pub static mut MAX_INPUT_SIZE: usize = 50;

pub fn fuzz() -> Result<()> {
    env_logger::init();

    // Hardcoded parameters
    let cores = Cores::from_cmdline("1").unwrap();
    let input_dir = PathBuf::from("./corpus");

    let mut elf_buffer = Vec::new();
    let elf = EasyElf::from_file(
        env::var("KERNEL").expect("KERNEL env not set"),
        &mut elf_buffer,
    )
    .unwrap();

    let input_addr = elf
        .resolve_symbol(
            &env::var("FUZZ_INPUT").unwrap_or_else(|_| "FUZZ_INPUT".to_owned()),
            0,
        )
        .expect("Symbol or env FUZZ_INPUT not found") as GuestPhysAddr;
    println!("FUZZ_INPUT @ {input_addr:#x}");

    let main_addr = elf
        .resolve_symbol("main", 0)
        .expect("Symbol main not found");
    println!("main address = {main_addr:#x}");

    let breakpoint = elf
        .resolve_symbol(
            &env::var("BREAKPOINT").unwrap_or_else(|_| "BREAKPOINT".to_owned()),
            0,
        )
        .expect("Symbol or env BREAKPOINT not found");
    println!("Breakpoint address = {breakpoint:#x}");

    // The monitor
    let monitor = StdMonitor::new();

    // The launcher supervises the fuzzer and communicates with the workers.
    let controller = StdController::builder().overwrite(true).build()?;

    let group = StdGroup::builder(&controller)
        .cores(cores)
        .timeout(Some(Duration::from_secs(5)))
        .state_builder(|worker| {
            let scheduler = QueueScheduler::new();

            StdState::new(
                StdContext::default(),
                // Corpus that will be evolved, we keep it in memory for performance
                InMemoryCorpus::new(scheduler),
                // Corpus in which we store solutions (crashes in this example),
                // on disk so the user can get them after stopping the fuzzer
                ObjectiveOnDiskCorpus::builder(worker)?.build()?,
            )
        })
        .build_inprocess(move |rt_handle, state| {
            // Initialize QEMU
            let args: Vec<String> = env::args().collect();
            let mut rand = StdRand::new();

            // Create an observation channel using the coverage map
            let mut edges_observer = unsafe {
                HitcountsMapObserver::new(SizePtrMapObserver::from_mut_slice(
                    "edges",
                    OwnedMutSlice::from_raw_parts_mut(edges_map_mut_ptr(), EDGES_MAP_DEFAULT_SIZE),
                    &raw mut MAX_EDGES_FOUND,
                ))
            };

            // Choose modules to use
            let modules = tuple_list!(
                StdEdgeCoverageModule::builder()
                    .map_observer(edges_observer.as_mut())
                    .build()?
            );

            let mut emu = StdEmulator::builder()
                .qemu_parameters(args)
                .modules(modules)
                .snapshot_manager(QemuSnapshotManager::default())
                .build()?;

            // Set breakpoints of interest with corresponding commands.
            emu.add_breakpoint(
                Breakpoint::with_command(
                    main_addr,
                    move |qemu| {
                        Ok(StartCommand::new(InputLocation::new(
                            qemu,
                            &QemuMemoryChunk::phys(
                                input_addr,
                                unsafe { MAX_INPUT_SIZE } as GuestReg,
                                qemu.cpu_from_index(0).unwrap(),
                            ),
                            None,
                        ))
                        .into())
                    },
                    true,
                ),
                true,
            );
            emu.add_breakpoint(
                Breakpoint::with_command(
                    breakpoint,
                    |_| Ok(EndCommand::new(Some(ExitKind::Ok)).into()),
                    false,
                ),
                true,
            );

            let devices = emu.list_devices();
            println!("Devices = {:?}", devices);

            emu.start().unwrap();

            // Create an observation channel to keep track of the execution time
            let time_observer = TimeObserver::new("time");

            // Feedback to rate the interestingness of an input
            // This one is composed by two Feedbacks in OR
            let feedback = feedback_or!(
                // New maximization map feedback linked to the edges observer and the feedback state
                MaxMapFeedback::new(&edges_observer),
                // Time feedback, this one does not need a feedback state
                TimeFeedback::new(&time_observer)
            );

            // A feedback to choose if an input is a solution or not
            let objective = feedback_or_fast!(CrashFeedback::new(), TimeoutFeedback::new());

            // Setup an havoc mutator with a mutational stage
            let mut stages = tuple_list!(StdStage::default());

            // Create a QEMU in-process executor
            let mut executor = StdQemuExecutor::new(
                state,
                emu,
                |_, _, _| Ok(()),
                |_, _, _, _| Ok(()),
                tuple_list!(edges_observer, time_observer),
            )
            .expect("Failed to create QemuExecutor");

            // Instead of calling the timeout handler and restart the process, trigger a breakpoint ASAP
            executor.break_on_timeout();

            // A fuzzer with feedbacks and a corpus scheduler
            let mut fuzzer =
                StdFuzzer::new(executor, feedback, objective, &mut stages, state, rt_handle)?;

            fuzzer
                .load_dir(&input_dir, state, rt_handle)
                .unwrap_or_else(|e| {
                    panic!("Failed to load initial corpus in {:?}: {e:?}", &input_dir);
                });

            fuzzer.fuzz_loop(&mut stages, &mut rand, state, rt_handle)?;

            Ok(())
        })?;

    // Build and run a Launcher
    StdLauncher::builder()
        .controller(controller)
        .monitor(monitor)
        .add_group(group)
        .build()?
        .launch()
}
