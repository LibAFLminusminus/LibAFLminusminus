use crate::{harness::MAX_INPUT_SIZE, options::FuzzerOptions};
use libaflmm::{
    Result,
    executors::Executor,
    illegal_argument,
    inputs::InputContext,
    key_not_found,
    observers::{CmpLogObserver, ObserversTuple, VarLenMapObserver},
    runtime,
    states::{FlatState, HasContext},
};
use libaflmm_bolts::{CoreId, tuple_list};
use libaflmm_qemu::{
    Emulator, GuestAddr, GuestReg, Qemu, QemuExecutor, Regs,
    elf::EasyElf,
    modules::{
        AsanGuestModule, AsanHostModule, CmpLogModule, DrCovModule, EmulatorModuleTuple,
        InjectionModule, SnapshotModule, StdEdgeCoverageModule, utils::filters::StdAddressFilter,
    },
};
use std::{ops::Range, path::PathBuf};

#[derive(Debug)]
pub struct QemuProfile {
    pub asan_host: bool,
    pub asan_guest: bool,
    pub cmplog: bool,
    pub injection: bool,
    pub drcov: Option<PathBuf>,
    pub replay: Option<PathBuf>,
}

impl QemuProfile {
    // resolve options here, to make sure some rules are enforced.
    pub fn new(opt: &FuzzerOptions, core: CoreId) -> Result<Self> {
        let replay = opt.replay.clone();
        let drcov = opt.drcov.clone();

        let asan_host = opt.is_asan_host_core(core);
        let asan_guest = opt.is_asan_guest_core(core);

        if asan_host && asan_guest {
            return Err(illegal_argument!(
                "A core cannot be both asan-host and asan-guest"
            ));
        }

        let cmplog = opt.is_cmplog_core(core);
        let injection = opt.injections.is_come();

        Ok(Self {
            asan_host,
            asan_guest,
            replay,
            drcov,
            cmplog,
            injection,
        })
    }

    fn build_asan_filter(&self, opt: &FuzzerOptions) -> StdAddressFilter {
        if let Some(include_asan) = opt.include_asan {
            log::info!("ASAN includes: {include_asan:#x?}");
            StdAddressFilter::allow_list(include_asan.to_vec())
        } else if let Some(exclude_asan) = opt.exclude_asan {
            log::info!("ASAN excludes: {exclude_asan:#x?}");
            StdAddressFilter::deny_list(exclude_asan.to_vec())
        } else {
            log::info!("ASAN no additional filter");
            StdAddressFilter::default()
        }
    }

    fn load_injection_module(&self, opt: &FuzzerOptions) -> Result<Option<InjectionModule>> {
        if self.injection {
            opt.injections
                .as_ref()
                .unwrap()
                .and_then(|injections_file| {
                    let extension = injections_file.extension().unwrap().to_str().unwrap();
                    if extension == "yaml" || extension == "yml" {
                        Ok(InjectionModule::from_yaml(injections_file).unwrap())
                    } else if extension == "toml" {
                        Ok(InjectionModule::from_toml(injections_file).unwrap())
                    } else {
                        Err(illegal_argument!(
                            "The injection file must have a yaml,yml or toml extension."
                        ))
                    }
                })
        } else {
            Ok(None)
        }
    }

    fn coverage_filter(&self, qemu: Qemu) -> Result<StdAddressFilter> {
        /* Conversion is required on 32-bit targets, but not on 64-bit ones */
        if let Some(includes) = &self.options.include {
            #[cfg_attr(target_pointer_width = "64", allow(clippy::useless_conversion))]
            let rules = includes
                .iter()
                .map(|x| Range {
                    start: x.start.into(),
                    end: x.end.into(),
                })
                .collect::<Vec<Range<GuestAddr>>>();
            Ok(StdAddressFilter::allow_list(rules))
        } else if let Some(excludes) = &self.options.exclude {
            #[cfg_attr(target_pointer_width = "64", allow(clippy::useless_conversion))]
            let rules = excludes
                .iter()
                .map(|x| Range {
                    start: x.start.into(),
                    end: x.end.into(),
                })
                .collect::<Vec<Range<GuestAddr>>>();
            Ok(StdAddressFilter::deny_list(rules))
        } else {
            let mut elf_buffer = Vec::new();
            let elf = EasyElf::from_file(qemu.binary_path(), &mut elf_buffer)?;
            let range = elf
                .get_section(".text", qemu.load_addr())
                .ok_or_else(|| key_not_found!("Failed to find .text section"))?;
            Ok(StdAddressFilter::allow_list(vec![range]))
        }
    }

    pub fn get_executor<EMU, I, S>(
        &self,
        emulator: EMU,
        observers: impl ObserversTuple<S>,
    ) -> Result<impl Executor<I, S>>
    where
        EMU: Emulator<I, S>,
        S: HasContext<I>,
    {
        let cmplog_observer = self.cmplog.then(|| CmpLogObserver::new("CmpLog", true));
        QemuExecutor::with_shadow_observers(
            emulator,
            |emu: &mut EMU, state: &mut S, input: &I| {
                let qemu = emu.qemu();

                let target = input.target_bytes();
                let bytes = state.context_mut().to_bytes(input);
                let mut buf = bytes.as_slice();
                let mut len = buf.len();
                if len > MAX_INPUT_SIZE {
                    buf = &buf[0..MAX_INPUT_SIZE];
                    len = MAX_INPUT_SIZE;
                }
                let len = len as GuestReg;

                qemu.write_mem(self.input_addr, buf).map_err(|e| {
                    runtime!("Failed to write to memory@{:#x}: {e:?}", self.input_addr)
                })?;

                qemu.write_reg(Regs::Pc, self.pc)
                    .map_err(|e| runtime!("Failed to write PC: {e:?}"))?;

                qemu.write_reg(Regs::Sp, self.stack_ptr)
                    .map_err(|e| runtime!("Failed to write SP: {e:?}"))?;

                qemu.write_return_address(self.ret_addr)
                    .map_err(|e| runtime!("Failed to write return address: {e:?}"))?;

                qemu.write_function_argument(0, self.input_addr as GuestReg)
                    .map_err(|e| runtime!("Failed to write argument 0: {e:?}"))?;

                qemu.write_function_argument(1, len)
                    .map_err(|e| runtime!("Failed to write argument 1: {e:?}"))?;

                Ok(())
            },
            |_, _| Ok(()), // TODO: take this as arg?
            observers,
            tuple_list!(cmplog_observer),
        )
    }

    pub fn get_modules<I, O, S>(
        &self,
        opt: &FuzzerOptions,
        env: &[(String, String)],
        edges_observer: &mut O,
    ) -> Result<impl EmulatorModuleTuple<I, S>>
    where
        I: Unpin,
        O: VarLenMapObserver,
        S: FlatState + Unpin,
    {
        let edge_coverage_module = StdEdgeCoverageModule::builder()
            .map_observer(edges_observer.as_mut())
            .build()?;

        let mut snapshot_module = SnapshotModule::with_filters(AsanGuestModule::snapshot_filters());

        if !opt.snapshots || opt.iterations.is_some() {
            snapshot_module.use_manual_reset();
        }

        let drcov_module = self
            .drcov
            .as_ref()
            .map(|p| DrCovModule::builder().path(p).full_trace(true).build());

        let asan_host_module = self.asan_host.then(|| {
            let asan_filter = self.build_asan_filter(opt);

            unsafe {
                AsanHostModule::builder()
                    .env(env)
                    .filter(asan_filter)
                    .asan_report()
                    .build()
            }
        });

        let asan_guest_module = self.asan_guest.then(|| {
            let asan_filter = self.build_asan_filter(opt);

            unsafe { AsanGuestModule::new(env, asan_filter) }
        });

        let cmplog_module = self.cmplog.then(CmpLogModule::default);

        let injection_module = self.load_injection_module(opt)?;

        Ok(tuple_list!(
            edge_coverage_module,
            snapshot_module,
            drcov_module,
            asan_host_module,
            asan_guest_module,
            cmplog_module,
            injection_module
        ))
    }
}
