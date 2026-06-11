#[cfg(not(test))]
use crate::asan::errors::AsanErrors;
use crate::helper::{FridaInstrumentationHelper, FridaRuntimeTuple};
#[cfg(windows)]
use crate::windows_hooks::initialize;
use alloc::rc::Rc;
use core::{
    cell::RefCell,
    ffi::c_void,
    fmt::{self, Debug, Formatter},
    marker::PhantomData,
    ptr,
};
use frida_gum::{
    Gum, MemoryRange, NativePointer,
    stalker::{NoneEventSink, Stalker},
};
#[cfg(windows)]
use libafl::executors::{hooks::inprocess::InProcessHooks, inprocess::HasInProcessHooks};
use libaflmm::{
    Result,
    common::{CompatibilityChecker, DependencyResolver, Registrator},
    controllers::Worker,
    executors::{Executor, ExitKind},
    inputs::Input,
    observers::ObserversTuple,
    runtimes::{RuntimeHandle, inprocess::CrashStatus},
    states::State,
};
use libaflmm_bolts::{AsSlice, tuples::RefIndexable};
#[cfg(all(windows, not(test)))]
use std::process::abort;

/// The [`FridaInProcessExecutor`] is an [`Executor`] that executes the target in the same process, usinig [`frida`](https://frida.re/) for binary-only instrumentation.
pub struct FridaExecutor<'a, H, I, OT, RT, S> {
    harness: H,
    observers: OT,
    /// `thread_id` for the Stalker
    thread_id: Option<u32>,
    /// Frida's dynamic rewriting engine
    stalker: Stalker,
    /// User provided callback for instrumentation
    helper: Rc<RefCell<FridaInstrumentationHelper<'a, RT>>>,
    followed: bool,
    phantom: PhantomData<(&'a (), I, S)>,
}

impl<H, I, OT, RT, S> Debug for FridaExecutor<'_, H, I, OT, RT, S>
where
    OT: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("FridaInProcessExecutor")
            .field("helper", &self.helper.borrow_mut())
            .field("followed", &self.followed)
            .finish_non_exhaustive()
    }
}

impl<H, I, OT, RT, S> DependencyResolver for FridaExecutor<'_, H, I, OT, RT, S>
where
    OT: DependencyResolver,
{
    fn register(&mut self, registrator: &mut Registrator) -> Result<()> {
        registrator.register_ty::<Self>();
        self.register_md(registrator)?;

        self.observers.register(registrator)
    }

    fn check(&self, _checker: &CompatibilityChecker) -> Result<()> {
        Ok(())
    }
}

impl<H, I, OT, RT, S> Executor<I, S> for FridaExecutor<'_, H, I, OT, RT, S>
where
    H: FnMut(&mut S, &I) -> Result<ExitKind>,
    I: Input,
    S: State<Input = I>,
    OT: ObserversTuple<S> + DependencyResolver,
    RT: FridaRuntimeTuple,
{
    type Observers = OT;

    fn init<W: Worker>(
        &mut self,
        _state: &mut S,
        _rt_handle: &mut RuntimeHandle<S, W>,
    ) -> Result<()> {
        Ok(())
    }

    /// Instruct the target about the input and run
    #[inline]
    unsafe fn execute_impl(&mut self, state: &mut S, input: &I) -> Result<ExitKind> {
        let target_bytes = state.input_to_bytes(input);
        self.helper.borrow_mut().pre_exec(target_bytes.as_slice())?;
        if self.helper.borrow_mut().stalker_enabled() {
            if !(self.followed) {
                self.followed = true;
                let helper_binding = self.helper.borrow_mut();
                let transformer = helper_binding.transformer();
                if let Some(thread_id) = self.thread_id {
                    self.stalker.follow::<NoneEventSink>(
                        thread_id.try_into().unwrap(),
                        transformer,
                        None,
                    );
                } else {
                    self.stalker.follow_me::<NoneEventSink>(transformer, None);
                    self.stalker.deactivate();
                }
            }
            // We removed the fuzzer from the stalked ranges,
            // but we need to pass the harness entry point
            // so that Stalker knows to pick it despite the module being excluded
            let ptr: *const H = ptr::from_ref::<H>(&self.harness);
            log::info!("Activating Stalker for {ptr:p}");
            self.stalker.activate(NativePointer(ptr as *mut c_void));
        }

        let res = (self.harness)(state, input)?;

        if self.helper.borrow_mut().stalker_enabled() {
            self.stalker.deactivate();
        }

        #[cfg(not(test))]
        unsafe {
            if !AsanErrors::get_mut_blocking().is_empty() {
                log::error!("Crashing target as it had ASan errors");
                libc::raise(libc::SIGABRT);
                #[cfg(windows)]
                abort();
            }
        }

        self.helper
            .borrow_mut()
            .post_exec(target_bytes.as_slice())?;

        Ok(res)
    }

    fn observers(&self) -> RefIndexable<&Self::Observers, Self::Observers> {
        RefIndexable::from(&self.observers)
    }

    fn observers_mut(&mut self) -> RefIndexable<&mut Self::Observers, Self::Observers> {
        RefIndexable::from(&mut self.observers)
    }

    unsafe fn handle_crash(
        &mut self,
        state: &mut S,
        input: Option<&I>,
        _params: &libaflmm::runtimes::OsTerminationParams,
    ) -> Result<CrashStatus> {
        if let Some(input) = input {
            let target_bytes = state.input_to_bytes(input);

            // Add any custom logic specific to FridaInProcessExecutor
            self.helper.borrow_mut().post_exec(target_bytes.as_ref())?;
            Ok(CrashStatus::TargetCrash)
        } else {
            Ok(CrashStatus::FuzzerCrash)
        }
    }
}

impl<'a, H, I, OT, RT, S> FridaExecutor<'a, H, I, OT, RT, S>
where
    RT: FridaRuntimeTuple,
{
    /// Creates a new [`FridaInProcessExecutor`].
    pub fn new(
        state: &S,
        harness: H,
        observers: OT,
        gum: &'a Gum,
        helper: Rc<RefCell<FridaInstrumentationHelper<'a, RT>>>,
    ) -> Self
    where
        H: FnMut(&mut S, &I) -> Result<ExitKind>,
    {
        FridaExecutor::with_target_bytes_converter(state, harness, observers, gum, helper, None)
    }

    /// Creates a new [`FridaInProcessExecutor`] tracking the given `thread_id`.
    pub fn on_thread(
        state: &S,
        harness: H,
        observers: OT,
        gum: &'a Gum,
        helper: Rc<RefCell<FridaInstrumentationHelper<'a, RT>>>,
        thread_id: u32,
    ) -> Self
    where
        H: FnMut(&mut S, &I) -> Result<ExitKind>,
    {
        FridaExecutor::with_target_bytes_converter(
            state,
            harness,
            observers,
            gum,
            helper,
            Some(thread_id),
        )
    }
}

impl<'a, H, I, OT, RT, S> FridaExecutor<'a, H, I, OT, RT, S>
where
    RT: FridaRuntimeTuple,
{
    /// Creates a new [`FridaInProcessExecutor`].
    pub fn with_target_bytes_converter(
        _state: &S,
        harness: H,
        observers: OT,
        gum: &'a Gum,
        helper: Rc<RefCell<FridaInstrumentationHelper<'a, RT>>>,
        thread_id: Option<u32>,
    ) -> Self
    where
        H: FnMut(&mut S, &I) -> Result<ExitKind>,
    {
        let mut stalker = Stalker::new(gum);
        let ranges = helper.borrow_mut().ranges().clone();
        for module in frida_gum::Process::obtain(gum).enumerate_modules() {
            let range = module.range();
            if (range.base_address().0 as usize)
                < Self::with_target_bytes_converter as *const () as usize
                && (Self::with_target_bytes_converter as *const () as usize as u64)
                    < range.base_address().0 as u64 + range.size() as u64
            {
                log::info!(
                    "Fuzzer range: {:x}-{:x}",
                    range.base_address().0 as u64,
                    range.base_address().0 as u64 + range.size() as u64
                );
                // Exclude the fuzzer from the stalked ranges, it is really unnecessary and harmfull.
                // Otherwise, Stalker starts messing with our hooks and their callbacks
                // wrecking havoc and causing deadlocks
                stalker.exclude(&MemoryRange::new(
                    NativePointer(range.base_address().0),
                    range.size(),
                ));
                break;
            }
        }

        log::info!(
            "disable_excludes: {:}",
            helper.borrow_mut().disable_excludes
        );
        if !helper.borrow_mut().disable_excludes {
            #[cfg(target_pointer_width = "64")]
            let range_end = u64::MAX;
            #[cfg(target_pointer_width = "32")]
            let range_end = u32::MAX as u64;
            for range in ranges.gaps(&(0..range_end)) {
                log::info!("excluding range: {:x}-{:x}", range.start, range.end);
                stalker.exclude(&MemoryRange::new(
                    NativePointer(range.start as *mut c_void),
                    usize::try_from(range.end - range.start).unwrap_or_else(|err| {
                        panic!("Address out of usize range: {range:?} - {err}")
                    }),
                ));
            }
        }

        #[cfg(windows)]
        initialize(gum);

        Self {
            harness,
            observers,
            thread_id,
            stalker,
            helper,
            followed: false,
            phantom: PhantomData,
        }
    }
}
