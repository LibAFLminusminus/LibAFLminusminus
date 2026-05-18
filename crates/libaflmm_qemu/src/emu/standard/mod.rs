use std::{cell::RefCell, collections::HashMap, fmt::Debug, pin::Pin, result};

use libaflmm::{
    Result, executors::ExitKind, inputs::Input, observers::ObserversTuple, states::CoreState,
};
use libaflmm_core::runtime;
use libaflmm_qemu_sys::GuestAddr;

use crate::{
    Emulator, EmulatorDriver, EmulatorDriverError, EmulatorDriverResult, EmulatorExitError,
    EmulatorExitResult, EmulatorHooks, EmulatorModules, NopEmulatorDriver, NopSnapshotManager,
    Qemu, QemuExitError, QemuExitReason, QemuHooks, QemuInitError, QemuParams, StdEmulatorDriver,
    breakpoint::{Breakpoint, BreakpointId},
    command::{CommandManager, NopCommandManager, StdCommandManager},
    config::QemuConfigBuilder,
    modules::EmulatorModuleTuple,
    sync_exit::CustomInsn,
};

pub mod builder;
pub use builder::StdEmulatorBuilder;

#[cfg(feature = "usermode")]
pub(crate) mod usermode;
#[cfg(feature = "usermode")]
pub use usermode::{InputLocation, StdSnapshotManager};

#[cfg(feature = "systemmode")]
pub(crate) mod systemmode;
#[cfg(feature = "systemmode")]
pub use systemmode::*;

/// The high-level interface to [`Qemu`].
///
/// It embeds multiple structures aiming at making QEMU usage easier:
///
/// - An [`IsSnapshotManager`] implementation, implementing the QEMU snapshot method to use.
/// - An [`EmulatorDriver`] implementation, responsible for handling the high-level control flow of QEMU runtime.
/// - A [`CommandManager`] implementation, handling the commands received from the target.
/// - [`EmulatorModules`], containing the [`EmulatorModule`] implementations' state.
///
/// Each of these fields can be set manually to finely tune how QEMU is getting handled.
/// It is highly encouraged to build [`Emulator`] using the associated [`StdEmulatorBuilder`].
/// There are two main functions to access the builder:
///
/// - [`Emulator::builder`] gives access to the standard [`StdEmulatorBuilder`], embedding all the standard components of an [`Emulator`].
/// - [`Emulator::empty`] gives access to an empty [`StdEmulatorBuilder`]. This is mostly useful to create a more custom [`Emulator`].
///
/// Please check the documentation of [`StdEmulatorBuilder`] for more details.
#[derive(Debug)]
pub struct StdEmulator<C, CM, ED, ET, I, S, SM> {
    pub(crate) snapshot_manager: SM,
    pub(crate) modules: Pin<Box<EmulatorModules<ET, I, S>>>,
    pub(crate) command_manager: CM,
    pub(crate) driver: ED,
    breakpoints_by_addr: RefCell<HashMap<GuestAddr, Breakpoint<C>>>, // TODO: change to RC here
    breakpoints_by_id: RefCell<HashMap<BreakpointId, Breakpoint<C>>>,
    pub(crate) qemu: Qemu,
    pub(crate) started: bool,
}

impl<C, CM, ED, ET, I, S, SM> Emulator<I, S> for StdEmulator<C, CM, ED, ET, I, S, SM>
where
    C: Debug + Clone,
    CM: CommandManager<C, ED, ET, I, S, SM, Commands = C>,
    ED: EmulatorDriver<C, CM, ET, I, S, SM>,
    ET: EmulatorModuleTuple<I, S> + Unpin,
    I: Unpin,
    S: Unpin,
{
    fn first_exec(&mut self, state: &mut S) -> Result<()> {
        ED::first_harness_exec(self, state)
    }

    fn pre_exec(&mut self, state: &mut S, input: &I) -> Result<()> {
        ED::pre_harness_exec(self, state, input)
    }

    fn exec_input(&mut self, input: &I) -> Result<ExitKind> {
        match unsafe { self.run(input)? } {
            EmulatorDriverResult::EndOfRun(exit_kind) => Ok(exit_kind),
            EmulatorDriverResult::ReturnToClient(exit_reason) => {
                Err(runtime!("Unexpected return to client: {exit_reason:?}"))
            }
            EmulatorDriverResult::ShutdownRequest => {
                log::warn!(
                    "QEMU received a shutdown request during a fuzzing run. It will be considered as a crash."
                );

                Ok(ExitKind::Crash)
            }
        }
    }

    fn post_exec<OT>(
        &mut self,
        input: &I,
        observers: &mut OT,
        state: &mut S,
        exit_kind: &mut ExitKind,
    ) -> libaflmm::Result<()>
    where
        OT: ObserversTuple<S>,
    {
        ED::post_harness_exec(self, input, observers, state, exit_kind)
    }

    fn on_crash(&mut self) -> Result<()> {
        unsafe { self.modules.modules_mut().on_crash_all() }
    }

    fn on_timeout(&mut self) -> Result<()> {
        unsafe { self.modules.modules_mut().on_timeout_all() }
    }
}

impl<C, I, S> StdEmulator<C, NopCommandManager, NopEmulatorDriver, (), I, S, NopSnapshotManager> {
    #[must_use]
    pub fn empty() -> StdEmulatorBuilder<
        C,
        NopCommandManager,
        NopEmulatorDriver,
        (),
        QemuConfigBuilder,
        I,
        S,
        NopSnapshotManager,
    > {
        StdEmulatorBuilder::empty()
    }
}

impl<C, I, S> StdEmulator<C, StdCommandManager<S>, StdEmulatorDriver, (), I, S, StdSnapshotManager>
where
    S: CoreState + Unpin,
    I: Input,
{
    #[must_use]
    pub fn builder() -> StdEmulatorBuilder<
        C,
        StdCommandManager<S>,
        StdEmulatorDriver,
        (),
        QemuConfigBuilder,
        I,
        S,
        StdSnapshotManager,
    > {
        StdEmulatorBuilder::default()
    }
}

impl<C, CM, ED, ET, I, S, SM> StdEmulator<C, CM, ED, ET, I, S, SM> {
    pub fn modules(&self) -> &EmulatorModules<ET, I, S> {
        &self.modules
    }

    #[must_use]
    pub fn qemu(&self) -> Qemu {
        self.qemu
    }

    #[must_use]
    pub fn driver(&self) -> &ED {
        &self.driver
    }

    #[must_use]
    pub fn driver_mut(&mut self) -> &mut ED {
        &mut self.driver
    }

    #[must_use]
    pub fn snapshot_manager(&self) -> &SM {
        &self.snapshot_manager
    }

    #[must_use]
    pub fn snapshot_manager_mut(&mut self) -> &mut SM {
        &mut self.snapshot_manager
    }

    pub fn command_manager(&self) -> &CM {
        &self.command_manager
    }

    pub fn command_manager_mut(&mut self) -> &mut CM {
        &mut self.command_manager
    }
}

impl<C, CM, ED, ET, I, S, SM> StdEmulator<C, CM, ED, ET, I, S, SM>
where
    ET: Unpin,
    I: Unpin,
    S: Unpin,
{
    pub fn modules_mut(&mut self) -> &mut EmulatorModules<ET, I, S> {
        self.modules.as_mut().get_mut()
    }
}

impl<C, CM, ED, ET, I, S, SM> StdEmulator<C, CM, ED, ET, I, S, SM>
where
    ET: EmulatorModuleTuple<I, S>,
    I: Unpin,
    S: Unpin,
{
    #[allow(clippy::must_use_candidate, clippy::similar_names)]
    pub fn new<T>(
        qemu_params: T,
        modules: ET,
        driver: ED,
        snapshot_manager: SM,
        command_manager: CM,
    ) -> result::Result<Self, QemuInitError>
    where
        T: Into<QemuParams>,
    {
        let mut qemu_params = qemu_params.into();

        // # Safety
        // `QemuHooks` can be used without QEMU being fully initialized, we make sure to only call
        // functions that do not depend on whether QEMU is well-initialized or not.
        let emulator_hooks = unsafe { EmulatorHooks::new(QemuHooks::get_unchecked()) };

        // # Safety
        // This is the only call to `EmulatorModules::new`.
        // Since Emulator can only be created once, we fulfil the conditions to call this function.
        let mut emulator_modules = unsafe { EmulatorModules::new(emulator_hooks, modules) };

        // # Safety
        // This is mostly safe, but can cause issues if module hooks call to emulator_modules.modules_mut().
        // In that case, it would cause the creation of a double mutable reference.
        // We need to refactor Modules to avoid such problem in the future at some point.
        // TODO: fix things there properly. The biggest issue being that it creates 2 mut ref to the module with the callback being called
        unsafe {
            emulator_modules.modules_mut().pre_qemu_init_all(
                EmulatorModules::<ET, I, S>::emulator_modules_mut_unchecked(),
                &mut qemu_params,
            );
        }

        let qemu = Qemu::init(qemu_params)?;

        // # Safety
        // Pre-init hooks have been called above.
        unsafe {
            Ok(Self::new_with_qemu(
                qemu,
                emulator_modules,
                driver,
                snapshot_manager,
                command_manager,
            ))
        }
    }

    /// New emulator with already initialized QEMU.
    /// We suppose modules init hooks have already been run.
    ///
    /// # Safety
    ///
    /// pre-init qemu hooks should be run before calling this.
    unsafe fn new_with_qemu(
        qemu: Qemu,
        emulator_modules: Pin<Box<EmulatorModules<ET, I, S>>>,
        driver: ED,
        snapshot_manager: SM,
        command_manager: CM,
    ) -> Self {
        let mut emulator = StdEmulator {
            modules: emulator_modules,
            command_manager,
            snapshot_manager,
            driver,
            breakpoints_by_addr: RefCell::new(HashMap::new()),
            breakpoints_by_id: RefCell::new(HashMap::new()),
            qemu,
            started: false,
        };

        emulator.modules.post_qemu_init_all(qemu);

        emulator
    }
}

impl<C, CM, ED, ET, I, S, SM> StdEmulator<C, CM, ED, ET, I, S, SM>
where
    C: Clone,
    CM: CommandManager<C, ED, ET, I, S, SM, Commands = C>,
    ED: EmulatorDriver<C, CM, ET, I, S, SM>,
    ET: EmulatorModuleTuple<I, S>,
    I: Unpin,
    S: Unpin,
{
    /// This function will run the emulator until the exit handler decides to stop the execution for
    /// whatever reason, depending on the choosen handler.
    /// It is a higher-level abstraction of [`Emulator::run`] that will take care of some part of the runtime logic,
    /// returning only when something interesting happen.
    ///
    /// # Safety
    /// Should, in general, be safe to call.
    /// Of course, the emulated target is not contained securely and can corrupt state or interact with the operating system.
    pub unsafe fn run(
        &mut self,
        input: &I,
    ) -> result::Result<EmulatorDriverResult<C>, EmulatorDriverError> {
        if !self.started {
            return Err(EmulatorDriverError::NotStartedYet);
        }

        loop {
            // Insert input if the location is already known
            ED::pre_qemu_exec(self, input);

            // Run QEMU
            log::debug!("Running QEMU...");
            let mut exit_reason = unsafe { self.run_qemu() };
            log::debug!("QEMU stopped.");

            // Handle QEMU exit
            if let Some(exit_handler_result) = ED::post_qemu_exec(self, &mut exit_reason)? {
                return Ok(exit_handler_result);
            }
        }
    }

    /// Start the emulator until a start even occurs
    ///
    /// # Safety
    ///
    /// This will make QEMU start. The calling thread will be running QEMU until an event stops it.
    /// This is (at least) as unsafe as running QEMU.
    pub unsafe fn start(&mut self) -> result::Result<(), EmulatorDriverError> {
        loop {
            let mut exit_result = unsafe { self.run_qemu() };

            // Handle QEMU exit
            if let Some(exit_handler_result) = ED::post_qemu_exec(self, &mut exit_result)? {
                match exit_handler_result {
                    EmulatorDriverResult::ReturnToClient(emulator_exit_result) => {
                        match emulator_exit_result {
                            EmulatorExitResult::QemuExit(qemu_shutdown_cause) => {
                                panic!("QEMU shut down unexpectedly: {qemu_shutdown_cause:?}");
                            }
                            EmulatorExitResult::Breakpoint(_breakpoint) => {}
                            EmulatorExitResult::CustomInsn(_custom_insn) => {}
                            EmulatorExitResult::Crash => {
                                panic!("Unexpected crash")
                            }
                            EmulatorExitResult::Timeout => {
                                panic!("No timeout should happen in start phase")
                            }
                            EmulatorExitResult::FuzzingStarts => {
                                self.started = true;
                                return Ok(());
                            }
                        }
                    }
                    EmulatorDriverResult::ShutdownRequest => {}
                    EmulatorDriverResult::EndOfRun(_exit_kind) => {
                        return Err(EmulatorDriverError::EndBeforeStart);
                    }
                }
            }
        }
    }

    /// This function will run the emulator until the next breakpoint, or until finish.
    ///
    /// # Safety
    ///
    /// Should, in general, be safe to call.
    /// Of course, the emulated target is not contained securely and can corrupt state or interact with the operating system.
    pub unsafe fn run_qemu(&self) -> result::Result<EmulatorExitResult<C>, EmulatorExitError> {
        match unsafe { self.qemu.run() } {
            Ok(qemu_exit_reason) => Ok(match qemu_exit_reason {
                QemuExitReason::End(qemu_shutdown_cause) => {
                    EmulatorExitResult::QemuExit(qemu_shutdown_cause)
                }
                QemuExitReason::Crash => EmulatorExitResult::Crash,
                QemuExitReason::Timeout => EmulatorExitResult::Timeout,
                QemuExitReason::Breakpoint(bp_addr) => {
                    let bp = self
                        .breakpoints_by_addr
                        .borrow()
                        .get(&bp_addr)
                        .ok_or(EmulatorExitError::BreakpointNotFound(bp_addr))?
                        .clone();
                    EmulatorExitResult::Breakpoint(bp.clone())
                }
                QemuExitReason::SyncExit => EmulatorExitResult::CustomInsn(CustomInsn::new(
                    self.command_manager.parse(self.qemu)?,
                )),
            }),
            Err(qemu_exit_reason_error) => Err(match qemu_exit_reason_error {
                QemuExitError::UnexpectedExit => EmulatorExitError::UnexpectedExit,
                QemuExitError::UnknownKind => EmulatorExitError::UnknownKind,
            }),
        }
    }
}

impl<C, CM, ED, ET, I, S, SM> StdEmulator<C, CM, ED, ET, I, S, SM> {
    pub fn add_breakpoint(&self, mut bp: Breakpoint<C>, enable: bool) -> BreakpointId
    where
        C: Clone,
    {
        if enable {
            bp.enable(self.qemu);
        }

        let bp_id = bp.id();
        let bp_addr = bp.addr();

        assert!(
            self.breakpoints_by_addr
                .borrow_mut()
                .insert(bp_addr, bp.clone())
                .is_none(),
            "Adding multiple breakpoints at the same address"
        );

        assert!(
            self.breakpoints_by_id
                .borrow_mut()
                .insert(bp_id, bp)
                .is_none(),
            "Adding the same breakpoint multiple times"
        );

        bp_id
    }

    pub fn remove_breakpoint(&self, bp_id: BreakpointId) {
        let bp_addr = {
            let mut bp_map = self.breakpoints_by_id.borrow_mut();
            let bp = bp_map.get_mut(&bp_id).expect("Did not find the breakpoint");
            bp.disable(self.qemu);
            bp.addr()
        };

        self.breakpoints_by_id
            .borrow_mut()
            .remove(&bp_id)
            .expect("Could not remove bp");
        self.breakpoints_by_addr
            .borrow_mut()
            .remove(&bp_addr)
            .expect("Could not remove bp");
    }
}
