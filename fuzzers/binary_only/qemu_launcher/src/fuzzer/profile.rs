use crate::options::FuzzOptions;
use libaflmm::Result;
use libaflmm_qemu::prelude::*;
use std::path::PathBuf;

#[derive(Debug)]
pub struct QemuProfile {
    pub asan_host: bool,
    pub asan_guest: bool,
    pub cmplog: bool,
    pub injection: bool,
    pub drcov: Option<PathBuf>,
}

impl QemuProfile {
    // resolve options here, to make sure some rules are enforced.
    pub fn new(options: &FuzzOptions, fuzz: &FuzzOptions, core: CoreId) -> Result<Self> {
        let drcov = options.common.drcov.clone();

        let asan_host = fuzz.is_asan_host_core(core);
        let asan_guest = fuzz.is_asan_guest_core(core);

        if asan_host && asan_guest {
            return Err(illegal_argument!(
                "A core cannot be both asan-host and asan-guest"
            ));
        }

        let cmplog = fuzz.is_cmplog_core(core);
        let injection = options.common.injections.is_some();

        Ok(Self {
            asan_host,
            asan_guest,
            drcov,
            cmplog,
            injection,
        })
    }

    fn build_asan_filter(&self, opt: &FuzzOptions) -> StdAddressFilter {
        if let Some(include_asan) = &opt.include_asan {
            log::info!("ASAN includes: {include_asan:#x?}");
            StdAddressFilter::allow_list(include_asan.to_vec())
        } else if let Some(exclude_asan) = &opt.exclude_asan {
            log::info!("ASAN excludes: {exclude_asan:#x?}");
            StdAddressFilter::deny_list(exclude_asan.to_vec())
        } else {
            log::info!("ASAN no additional filter");
            StdAddressFilter::default()
        }
    }

    #[cfg(feature = "injections")]
    pub fn injection_module(&self, opt: &FuzzOptions) -> Result<Option<InjectionModule>> {
        if !self.injection {
            return Ok(None);
        }

        let injections_file = opt.common.injections.as_ref().unwrap();
        let extension = injections_file.extension().unwrap().to_str().unwrap();

        let module = if extension == "yaml" || extension == "yml" {
            InjectionModule::from_yaml(injections_file).unwrap()
        } else if extension == "toml" {
            InjectionModule::from_toml(injections_file).unwrap()
        } else {
            return Err(illegal_argument!(
                "The injection file must have a yaml,yml or toml extension."
            ));
        };

        Ok(Some(module))
    }

    #[cfg(not(feature = "injections"))]
    pub fn injection_module(&self, _opt: &CommonOptions) -> Result<Option<InjectionModule>> {
        Ok(None)
    }

    pub fn cmplog(&self) -> Option<StdCmpLogObserver> {
        self.cmplog.then(|| CmpLogObserver::new("CmpLog", true))
    }

    pub fn get_modules<I, O, S>(
        &self,
        fuzz: &FuzzOptions,
        env: &[(String, String)],
        edges_observer: &mut O,
        injection_module: Option<InjectionModule>,
    ) -> Result<impl EmulatorModuleTuple<I, S> + HasAddressFilterTuple + use<I, O, S>>
    where
        I: Unpin,
        O: VarLenMapObserver,
        S: State<Input = I> + Unpin,
    {
        let edge_coverage_module = StdEdgeCoverageModule::builder()
            .map_observer(edges_observer.as_mut())
            .jit(false)
            .build()?;

        let mut snapshot_module = SnapshotModule::with_filters(AsanGuestModule::snapshot_filters());

        if !fuzz.snapshots || fuzz.iterations.is_some() {
            snapshot_module.use_manual_reset();
        }

        let drcov_module = self
            .drcov
            .as_ref()
            .map(|p| DrCovModule::builder().path(p).full_trace(true).build());

        let asan_host_module = self.asan_host.then(|| {
            let asan_filter = self.build_asan_filter(fuzz);

            unsafe {
                AsanHostModule::builder()
                    .env(env)
                    .filter(asan_filter)
                    .asan_report()
                    .build()
            }
        });

        let asan_guest_module = self.asan_guest.then(|| {
            let asan_filter = self.build_asan_filter(fuzz);

            AsanGuestModule::new(env, asan_filter)
        });

        let cmplog_module = self.cmplog.then(CmpLogModule::default);

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
