use crate::{fuzzer::profile::QemuProfile, harness::Harness, options::FuzzOptions};
use libaflmm::Result;
use libaflmm_qemu::prelude::*;

mod profile;

pub struct QemuFuzzer;

impl QemuFuzzer {
    pub fn launch(
        options: FuzzOptions,
        env: Vec<(String, String)>,
        args: Vec<String>,
    ) -> Result<()> {
        let monitor = StdMonitor::new();
        let controller = StdController::builder().overwrite(true).build()?;

        StdLauncher::builder()?
            .cores(options.cores.clone())
            .timeout(Some(options.timeout))
            .monitor(monitor)
            .controller(controller)
            .state_builder(move |worker| {
                let scheduler = QueueScheduler::new();
                let crash_dir = worker.workdir().create_dir("crashes")?;
                let queue_dir = worker.workdir().create_dir("queue")?;

                StdState::new(
                    BytesContext::default(),
                    // Corpus that will be evolved, we keep it in memory for performance
                    InMemoryOnDiskCorpus::builder()
                        .root_dir(queue_dir.as_path())
                        .scheduler(scheduler)
                        .build()?,
                    // Corpus in which we store solutions (crashes in this example),
                    // on disk so the user can get them after stopping the fuzzer
                    OnDiskCorpus::<BytesInput, NopScheduler>::builder()
                        .root_dir(crash_dir.as_path())
                        .build()?,
                )
            })
            .build_inprocess(move |rt_handle, state| {
                let core_id = rt_handle.worker().core_id();
                let profile = QemuProfile::new(&options, &options, core_id)?;

                // Create an observation channel using the coverage map
                let mut edges_observer = unsafe {
                    HitcountsMapObserver::new(VariableMapObserver::from_mut_slice(
                        "edges",
                        OwnedMutSlice::from_raw_parts_mut(
                            edges_map_mut_ptr(),
                            EDGES_MAP_DEFAULT_SIZE,
                        ),
                        &raw mut MAX_EDGES_FOUND,
                    ))
                };

                // Create an observation channel to keep track of the execution time
                let time_observer = TimeObserver::new("time");

                let map_feedback = StdMapFeedback::with_name("map_feedback", &edges_observer);
                let map_objective = StdMapFeedback::with_name("map_objective", &edges_observer);

                // Feedback to rate the interestingness of an input
                // This one is composed by two Feedbacks in OR
                let feedback = feedback_or!(
                    // New maximization map feedback linked to the edges observer and the feedback state
                    map_feedback,
                    // Time feedback, this one does not need a feedback state
                    TimeFeedback::new(&time_observer)
                );

                // A feedback to choose if an input is a solution or not
                let objective = feedback_and_fast!(
                    feedback_or_fast!(CrashFeedback::new(), TimeoutFeedback::new()),
                    map_objective
                );

                let mut tokens = Tokens::new();

                let injection_module = profile.injection_module(&options)?;

                if let Some(inj) = &injection_module {
                    for tok in &inj.tokens {
                        let _ = tokens.add_token(&tok.as_bytes().to_vec());
                    }
                }

                if let Some(tokenfile) = &options.tokens {
                    tokens.add_from_file(tokenfile)?;
                }

                state.metadata_map_mut().insert_unnamed(tokens);

                let modules =
                    profile.get_modules(&options, &env, &mut edges_observer, injection_module)?;

                let cmplog_observer = profile.cmplog();
                let observers = tuple_list!(edges_observer, cmplog_observer, time_observer);

                let mut emulator = StdEmulator::builder()
                    .qemu_parameters(args.clone())
                    .modules(modules)
                    .build()?;

                let harness = Harness::init(&mut emulator, &options.common)?;

                let mut executor = StdQemuExecutor::new(
                    state,
                    emulator,
                    |state, input, emu| harness.run(state, input, emu),
                    |_, _, _, _| Ok(()),
                    observers,
                )?;

                let mut stages = tuple_list!(StdStage::default());
                let mut rand = StdRand::new();

                let mut fuzzer = StdFuzzer::new(
                    feedback,
                    objective,
                    &mut stages,
                    &mut executor,
                    state,
                    rt_handle,
                )?;

                if state.must_load_initial_inputs() {
                    let corpus_dirs = [&options.common.input];

                    state
                        .load_initial_inputs(&mut fuzzer, &mut executor, rt_handle, &corpus_dirs)
                        .expect(&format!("Failed to load initial corpus at {corpus_dirs:?}"));
                    println!("We imported {} inputs from disk.", state.corpus().count());
                }

                fuzzer.fuzz_loop(&mut stages, &mut executor, &mut rand, state, rt_handle)
            })?
            .launch()
    }
}
