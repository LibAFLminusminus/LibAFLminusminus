//! A fuzzer using qemu in systemmode for binary-only coverage of kernels
use core::time::Duration;
use libaflmm::{
    corpus::{
        schedulers::{NopScheduler, QueueScheduler},
        Corpus, InMemoryCorpus, OnDiskCorpus,
    },
    executors::ExitKind,
    feedback_or, feedback_or_fast,
    feedbacks::{CrashFeedback, MaxMapFeedback, TimeFeedback, TimeoutFeedback},
    inputs::{BytesContext, InputContext, StdContext},
    launchers::StdLauncher,
    monitors::StdMonitor,
    mutators::{havoc_mutations::havoc_mutations, scheduled::HavocScheduledMutator},
    observers::{HitcountsMapObserver, TimeObserver, VariableMapObserver},
    stages::StdMutationalStage,
    states::{State, StdState},
    Fuzzer, Result, StdFuzzer, Worker,
};
use libaflmm_bolts::{
    core_affinity::Cores, ownedref::OwnedMutSlice, rands::StdRand, tuples::tuple_list, AsSlice,
};
use libaflmm_qemu::{
    config::{self, QemuConfig},
    elf::EasyElf,
    executor::QemuExecutor,
    modules::{edges::StdEdgeCoverageModuleBuilder, NopModule},
    GuestAddr, GuestPhysAddr, QemuExitReason, QemuRWError, Regs, StdEmulator,
};
use libaflmm_targets::{edges_map_mut_ptr, EDGES_MAP_DEFAULT_SIZE, MAX_EDGES_FOUND};
use std::{env, path::PathBuf, process, result};

pub static mut MAX_INPUT_SIZE: usize = 50;

pub fn fuzz() -> Result<()> {
    env_logger::init();

    if let Ok(s) = env::var("FUZZ_SIZE") {
        str::parse::<usize>(&s).expect("FUZZ_SIZE was not a number");
    };

    // Hardcoded parameters
    let timeout = Duration::from_secs(3);
    let broker_port = 1338;
    let cores = Cores::from_cmdline("1").unwrap();
    let input_dir = PathBuf::from("./corpus");

    let mut elf_buffer = Vec::new();
    let elf = EasyElf::from_file(
        env::var("KERNEL").expect("KERNEL env not set"),
        &mut elf_buffer,
    )
    .unwrap();

    let input_addr = GuestPhysAddr::try_from(
        elf.resolve_symbol(
            &env::var("FUZZ_INPUT").unwrap_or_else(|_| "FUZZ_INPUT".to_owned()),
            0,
        )
        .expect("Symbol or env FUZZ_INPUT not found"),
    )
    .unwrap();
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

    let mut worker = |state: _, rt_handle: _| {
        let target_dir = env::var("TARGET_DIR").expect("TARGET_DIR env not set");
        let rand = StdRand::new();

        // Create an observation channel using the coverage map
        let mut edges_observer = unsafe {
            HitcountsMapObserver::new(VariableMapObserver::from_mut_slice(
                "edges",
                OwnedMutSlice::from_raw_parts_mut(edges_map_mut_ptr(), EDGES_MAP_DEFAULT_SIZE),
                &raw mut MAX_EDGES_FOUND,
            ))
        };

        // Create QEMU configuration
        let qemu_config = QemuConfig::builder()
            .machine("mps2-an385")
            .monitor(config::Monitor::Null)
            .kernel(format!("{target_dir}/example.elf"))
            .serial(config::Serial::Null)
            .no_graphic(true)
            .snapshot(true)
            .drives([config::Drive::builder()
                .interface(config::DriveInterface::None)
                .format(config::DiskImageFileFormat::Qcow2)
                .file(format!("{target_dir}/dummy.qcow2"))
                .build()])
            .start_cpu(false)
            .build();

        let emulator_modules = (
            StdEdgeCoverageModuleBuilder::default()
                .map_observer(edges_observer.as_mut())
                .build()?,
            NopModule::default(),
        );

        let emulator = StdEmulator::empty()
            .qemu_parameters(qemu_config)
            .modules(emulator_modules)
            .build()?;

        let qemu = emulator.qemu();

        qemu.set_breakpoint(main_addr);

        unsafe {
            match qemu.run() {
                Ok(QemuExitReason::Breakpoint(_)) => {}
                _ => panic!("Unexpected QEMU exit."),
            }
        }

        qemu.remove_breakpoint(main_addr);

        qemu.set_breakpoint(breakpoint); // BREAKPOINT

        let devices = qemu.list_devices();
        println!("Devices = {devices:?}");

        let snap = qemu.create_fast_snapshot(true);

        // Create an observation channel to keep track of the execution time
        let time_observer = TimeObserver::new("time");

        // Feedback to rate the interestingness of an input
        // This one is composed by two Feedbacks in OR
        let mut feedback = feedback_or!(
            // New maximization map feedback linked to the edges observer and the feedback state
            MaxMapFeedback::new(&edges_observer),
            // Time feedback, this one does not need a feedback state
            TimeFeedback::new(&time_observer)
        );

        // A feedback to choose if an input is a solution or not
        let mut objective = feedback_or_fast!(CrashFeedback::new(), TimeoutFeedback::new());

        let mut pre_exec = |state: &mut StdState<_, BytesContext, _>, input, emulator| {
            let target = state.context_mut().to_bytes(input);
            let mut buf = target.as_slice();
            let len = buf.len();
            unsafe {
                if len > MAX_INPUT_SIZE {
                    buf = &buf[0..MAX_INPUT_SIZE];
                    // len = MAX_INPUT_SIZE;
                }

                qemu.write_phys_mem(input_addr, buf);
            }

            Ok(())
        };

        let mut post_exec =
            |state: &mut StdState<_, BytesContext, _>, input, emu, exit_kind: &mut _| {
                // If the execution stops at any point other than the designated breakpoint (e.g. a breakpoint on a panic method) we consider it a crash
                let mut pcs = (0..qemu.num_cpus())
                    .map(|i| qemu.cpu_from_index(i).unwrap())
                    .map(|cpu| -> result::Result<GuestAddr, QemuRWError> {
                        cpu.read_reg(Regs::Pc).map(|res| res as GuestAddr)
                    });

                let ret = match pcs
                    .find(|pc| (breakpoint..breakpoint + 5).contains(pc.as_ref().unwrap_or(&0)))
                {
                    Some(_) => {
                        *exit_kind = ExitKind::Ok;
                    }
                    None => {
                        *exit_kind = ExitKind::Crash;
                    }
                };

                // OPTION 1: restore only the CPU state (registers et. al)
                // for (i, s) in saved_cpu_states.iter().enumerate() {
                //     emu.cpu_from_index(i).restore_state(s);
                // }

                // OPTION 2: restore a slow vanilla QEMU snapshot
                // emu.load_snapshot("start", true);

                // OPTION 3: restore a fast devices+mem snapshot
                unsafe {
                    qemu.restore_fast_snapshot(snap);
                }

                Ok(())
            };

        // Create a QEMU in-process executor
        let mut executor = QemuExecutor::new(
            emulator,
            &mut pre_exec,
            &mut post_exec,
            tuple_list!(edges_observer, time_observer),
        )
        .expect("Failed to create QemuExecutor");

        // Instead of calling the timeout handler and restart the process, trigger a breakpoint ASAP
        executor.break_on_timeout();

        // Setup an havoc mutator with a mutational stage
        let mutator = HavocScheduledMutator::new(havoc_mutations());
        let mut stages = tuple_list!(StdMutationalStage::new(mutator));

        // A fuzzer with feedbacks and a corpus scheduler
        let mut fuzzer = StdFuzzer::new(
            feedback,
            objective,
            &mut stages,
            &mut executor,
            state,
            rt_handle,
        )?;

        if state.must_load_initial_inputs() {
            state
                .load_initial_inputs(&mut fuzzer, &mut executor, rt_handle, &[input_dir])
                .unwrap_or_else(|_| {
                    println!("Failed to load initial corpus at {:?}", &input_dir);
                    process::exit(0);
                });
            println!("We imported {} inputs from disk.", state.corpus().count());
        }

        fuzzer
            .fuzz_loop(&mut stages, &mut executor, &mut rand, state, rt_handle)
            .unwrap();

        Ok(())
    };

    // If not restarting, create a State from scratch
    let state_builder = |worker| {
        let objective_dir = worker.workdir().root().join("./crashes");
        let scheduler = QueueScheduler::new();

        StdState::new(
            StdContext::default(),
            // Corpus that will be evolved, we keep it in memory for performance
            InMemoryCorpus::new(scheduler),
            // Corpus in which we store solutions (crashes in this example),
            // on disk so the user can get them after stopping the fuzzer
            OnDiskCorpus::new(objective_dir, NopScheduler)?,
        )
    };

    // The monitor
    let monitor = StdMonitor::new();

    // Build and run a Launcher
    StdLauncher::builder()?
        .state_builder(state_builder)
        .monitor(monitor)
        .cores(cores)
        .build_inprocess(&mut worker)
        .launch()
}
