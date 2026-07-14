use libaflmm::prelude::*;

use crate::options::{CommonOptions, ReplayOptions};

pub struct QemuReplay;

impl QemuReplay {
    pub fn launch(
        options: &CommonOptions,
        replay_options: &ReplayOptions,
        env: &Vec<(String, String)>,
        args: &Vec<String>,
    ) -> Result<()> {
        StdLauncher::builder()?
            .timeout(None) // no timeout during replay.
            .state_builder(|worker| {
                let crash_dir = worker.workdir().create_dir("crashes")?;

                StdState::new(
                    BytesContext::default(),
                    // Corpus that will be evolved, we keep it in memory for performance
                    InMemoryCorpus::builder()
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
            .build_inprocess(|rt_handle, state| {
                let core_id = rt_handle.worker().core_id();
                let profile = QemuProfile::new(cli, core_id)?;

                // Create an observation channel using the coverage map
                let mut edges_observer = unsafe {
                    HitcountsMapObserver::new(SizePtrMapObserver::from_mut_slice(
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
                let mut feedback = feedback_or!(
                    // New maximization map feedback linked to the edges observer and the feedback state
                    map_feedback,
                    // Time feedback, this one does not need a feedback state
                    TimeFeedback::new(&time_observer)
                );

                // A feedback to choose if an input is a solution or not
                let mut objective = feedback_and_fast!(
                    feedback_or_fast!(CrashFeedback::new(), TimeoutFeedback::new()),
                    map_objective
                );

                let observers = tuple_list!(edges_observer, time_observer);

                let mut tokens = Tokens::new();

                for token in extra_tokens {
                    let bytes = token.as_bytes().to_vec();
                    let _ = tokens.add_token(&bytes);
                }

                if let Some(tokenfile) = &cli.tokens {
                    tokens.add_from_file(tokenfile)?;
                }

                let executor = profile.get_executor(
                    state, observers, input_addr, pc, stack_ptr, ret_addr, input_addr,
                )?;

                Ok(())
            })?
            .launch()
    }
}
