use crate::{fuzzer::profile::QemuProfile, harness::Harness, options::ReplayOptions};
use libaflmm::{Result, prelude::*};
use libaflmm_qemu::prelude::*;

pub struct QemuReplay;

impl QemuReplay {
    pub fn launch(
        options: ReplayOptions,
        env: Vec<(String, String)>,
        args: Vec<String>,
    ) -> Result<()> {
        let monitor = StdMonitor::new();
        let controller = StdController::builder().overwrite(true).build()?;

        let group = StdGroup::builder(&controller)
            .timeout(None)
            .state_builder(|worker| {
                let scheduler = QueueScheduler::new();

                StdState::new(
                    BytesContext,
                    InMemoryOnDiskCorpus::builder(worker, scheduler)?.build()?,
                    ObjectiveOnDiskCorpus::builder(worker)?.build()?,
                )
            })
            .build_inprocess(move |rt_handle, state| {
                let profile = QemuProfile::replay(&options.common, &options)?;

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

                let injection_module = profile.injection_module(&options.common)?;
                let modules = profile.get_modules(
                    &options.common,
                    None,
                    &env,
                    &mut edges_observer,
                    injection_module,
                )?;

                let mut emulator = StdEmulator::builder()
                    .qemu_parameters(args.clone())
                    .modules(modules)
                    .build()?;
                let harness = Harness::init(&mut emulator, &options.common)?;

                let executor = StdQemuExecutor::new(
                    state,
                    emulator,
                    |state, input, emu| harness.pre_exec(state, input, emu),
                    |_, _, _, _| Ok(()),
                    tuple_list!(edges_observer),
                )?;

                let mut stages = tuple_list!();
                let mut fuzzer = StdFuzzer::new(
                    executor,
                    ConstFeedback::new(false),
                    CrashFeedback::new(),
                    &mut stages,
                    state,
                    rt_handle,
                )?;

                state.load_initial_inputs_from(&mut fuzzer, rt_handle, &options.common.input)?;
                log::info!("Replay finished");
                Ok(())
            })?;

        StdLauncher::builder()
            .monitor(monitor)
            .controller(controller)
            .add_group(group)
            .build()?
            .launch()
    }
}
