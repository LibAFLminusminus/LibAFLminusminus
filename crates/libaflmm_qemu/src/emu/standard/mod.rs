use crate::Result;
use crate::arch::Regs;
use crate::command::Command;
#[cfg(feature = "systemmode")]
use crate::emu::MapKind;
use crate::emu::snapshots::StdSnapshotManager;
use crate::emu::{
    EmulatorError, InputLocation, InputWriter, NopInputWriter, SnapshotId, SnapshotManager,
    StdInputWriter,
};
use crate::modules::HasStdFiltersTuple;
#[cfg(feature = "systemmode")]
use crate::qemu::PhysMemoryChunk;
use crate::{
    breakpoint::{Breakpoint, BreakpointId},
    command::{CommandManager, NopCommandManager, StdCommandManager},
    emu::{
        Emulator, EmulatorExitError, EmulatorExitReason, EmulatorHooks, EmulatorModules,
        EmulatorRunResult, NopSnapshotManager,
    },
    modules::EmulatorModuleTuple,
    qemu::{
        Qemu, QemuExitReason, QemuHooks, QemuParams, QemuShutdownCause, config::QemuConfigBuilder,
    },
    sync_exit::CustomInsn,
};
use libaflmm::{
    executors::ExitKind, inputs::Input, observers::ObserversTuple, runtime, states::State,
};
use libaflmm_qemu_sys::GuestAddr;
use std::cell::OnceCell;
use std::{cell::RefCell, collections::HashMap, fmt::Debug, pin::Pin};

pub mod builder;
pub use builder::StdEmulatorBuilder;

/// The high-level interface to [`Qemu`].
///
/// It embeds multiple structures aiming at making QEMU usage easier:
///
/// - An [`IsSnapshotManager`] implementation, implementing the QEMU snapshot method to use.
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
pub struct StdEmulator<C, CM, ET, I, IS, S, SM> {
    snapshot_manager: SM,
    modules: Pin<Box<EmulatorModules<ET, I, S>>>,
    command_manager: CM,
    input_writer: IS,
    breakpoints_by_addr: RefCell<HashMap<GuestAddr, Breakpoint<C>>>, // TODO: change to RC here
    breakpoints_by_id: RefCell<HashMap<BreakpointId, Breakpoint<C>>>,
    qemu: Qemu,
    started: bool,
    snapshot_id: OnceCell<SnapshotId>,
    // hooks_locked: bool,
    #[cfg(feature = "systemmode")]
    allow_page_on_start: bool,
    #[cfg(feature = "x86_64")]
    #[allow(dead_code)]
    process_only: bool,
    print_commands: bool,
    // maps declared by the VM
    #[cfg(feature = "systemmode")]
    #[allow(dead_code)]
    maps: HashMap<MapKind, PhysMemoryChunk>,
}

impl<C, CM, ET, I, IW, S, SM> Emulator<I, S> for StdEmulator<C, CM, ET, I, IW, S, SM>
where
    C: Command<I, S>,
    CM: CommandManager<I, S, Commands = C, InputWriter = IW>,
    ET: EmulatorModuleTuple<I, S> + HasStdFiltersTuple + Unpin,
    I: Input + Unpin,
    IW: InputWriter<I, S>,
    S: State + Unpin,
    SM: SnapshotManager,
{
    type CommandManager = CM;
    type Modules = ET;
    type SnapshotManager = SM;

    fn start(&mut self) -> Result<()> {
        match unsafe { self.run_until_outcome()? } {
            EmulatorRunResult::FuzzingStarts => {
                self.started = true;
                Ok(())
            }
            EmulatorRunResult::EndOfRun(_) => Err(EmulatorError::EndBeforeStart.into()),
            EmulatorRunResult::Breakpoint(bp_id) => {
                Err(runtime!("unexpected breakpoint {bp_id:?}").into())
            }
            EmulatorRunResult::ShutdownRequest => {
                log::warn!("QEMU shutdown before fuzzing started.");
                Err(EmulatorError::EndBeforeStart.into())
            }
        }
    }

    fn first_exec(&mut self, state: &mut S) -> Result<()> {
        let qemu = self.qemu();
        self.modules_mut().first_exec_all(qemu, state)
    }

    fn pre_exec(&mut self, state: &mut S, input: &I) -> Result<()> {
        let qemu = self.qemu();
        self.modules_mut().pre_exec_all(qemu, state, input)?;

        Ok(())
    }

    fn exec_input(&mut self, state: &mut S, input: &I) -> Result<ExitKind> {
        match unsafe { self.run(state, input)? } {
            EmulatorRunResult::EndOfRun(exit_kind) => Ok(exit_kind),
            EmulatorRunResult::ShutdownRequest => {
                log::warn!(
                    "QEMU received a shutdown request during a fuzzing run. It will be considered as a crash."
                );

                Ok(ExitKind::Crash)
            }
            EmulatorRunResult::Breakpoint(bp_id) => {
                Err(runtime!("unexpected breakpoint {bp_id:?}").into())
            }
            EmulatorRunResult::FuzzingStarts => {
                Err(runtime!("unexpected fuzzing-start signal during a fuzzing run").into())
            }
        }
    }

    fn post_exec<OT>(
        &mut self,
        state: &mut S,
        input: &I,
        observers: &mut OT,
        exit_kind: &mut ExitKind,
    ) -> Result<()>
    where
        OT: ObserversTuple<S>,
    {
        let qemu = self.qemu();
        self.modules_mut()
            .post_exec_all(qemu, state, input, observers, exit_kind)?;

        match exit_kind {
            ExitKind::Crash => self.on_crash(),
            ExitKind::Timeout => self.on_timeout(),
            _ => Ok(()),
        }
    }

    fn on_crash(&mut self) -> Result<()> {
        unsafe { self.modules.modules_mut().on_crash_all() }
    }

    fn on_timeout(&mut self) -> Result<()> {
        unsafe { self.modules.modules_mut().on_timeout_all() }
    }

    fn qemu(&self) -> Qemu {
        self.qemu
    }

    fn add_breakpoint(&self, mut bp: Breakpoint<C>, enable: bool) -> BreakpointId {
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

    fn remove_breakpoint(&self, bp_id: BreakpointId) {
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

    fn snapshot_manager_mut(&mut self) -> &mut Self::SnapshotManager {
        &mut self.snapshot_manager
    }

    fn command_manager_mut(&mut self) -> &mut Self::CommandManager {
        &mut self.command_manager
    }

    fn modules_mut(&mut self) -> &mut EmulatorModules<ET, I, S> {
        &mut self.modules
    }

    fn snapshot_id(&self) -> Option<SnapshotId> {
        self.snapshot_id.get().copied()
    }

    fn set_snapshot_id(&mut self, snapshot_id: SnapshotId) -> Result<()> {
        self.snapshot_id
            .set(snapshot_id)
            .map_err(|_| EmulatorError::MultipleSnapshotDefinition.into())
    }

    fn set_input_location(&mut self, input_location: &InputLocation) -> Result<()> {
        self.input_writer.set_input_location(input_location.clone())
    }

    fn input_writer_mut(&mut self) -> &mut IW {
        &mut self.input_writer
    }

    #[cfg(feature = "systemmode")]
    fn allow_page_on_start(&self) -> bool {
        self.allow_page_on_start
    }

    fn entry_break(
        &mut self,
        addr: GuestAddr,
        bp_cb: impl FnMut(Qemu) -> Result<C> + 'static,
    ) -> Result<()> {
        self.add_breakpoint(Breakpoint::with_command(addr, bp_cb, true), true);
        self.start()
    }

    fn max_input_size(&self, state: &mut S, input: &I) -> usize {
        self.input_writer.input_size(state, input)
    }
}

impl<C, I, S> StdEmulator<C, NopCommandManager, (), I, NopInputWriter, S, NopSnapshotManager> {
    #[must_use]
    pub fn empty() -> StdEmulatorBuilder<
        C,
        NopCommandManager,
        (),
        QemuConfigBuilder,
        I,
        NopInputWriter,
        S,
        NopSnapshotManager,
    > {
        StdEmulatorBuilder::empty()
    }
}

impl<C, I, S> StdEmulator<C, StdCommandManager, (), I, StdInputWriter, S, StdSnapshotManager>
where
    S: State + Unpin,
    I: Input,
{
    #[must_use]
    pub fn builder() -> StdEmulatorBuilder<
        C,
        StdCommandManager,
        (),
        QemuConfigBuilder,
        I,
        StdInputWriter,
        S,
        StdSnapshotManager,
    > {
        StdEmulatorBuilder::default()
    }
}

impl<C, CM, ET, I, IS, S, SM> StdEmulator<C, CM, ET, I, IS, S, SM> {
    pub fn modules(&self) -> &EmulatorModules<ET, I, S> {
        &self.modules
    }

    #[must_use]
    pub fn qemu(&self) -> Qemu {
        self.qemu
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

impl<C, CM, ET, I, IS, S, SM> StdEmulator<C, CM, ET, I, IS, S, SM>
where
    ET: Unpin,
    I: Unpin,
    S: Unpin,
{
    pub fn modules_mut(&mut self) -> &mut EmulatorModules<ET, I, S> {
        self.modules.as_mut().get_mut()
    }
}

impl<C, CM, ET, I, IW, S, SM> StdEmulator<C, CM, ET, I, IW, S, SM>
where
    ET: EmulatorModuleTuple<I, S>,
    I: Unpin,
    S: Unpin,
{
    #[allow(clippy::must_use_candidate, clippy::similar_names)]
    pub fn new<T>(
        qemu_params: T,
        modules: ET,
        input_writer: IW,
        snapshot_manager: SM,
        command_manager: CM,
    ) -> Result<Self>
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
                input_writer,
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
        input_writer: IW,
        snapshot_manager: SM,
        command_manager: CM,
    ) -> Self {
        let mut emulator = StdEmulator {
            modules: emulator_modules,
            command_manager,
            snapshot_manager,
            input_writer,
            // hooks_locked: true,
            print_commands: false,
            breakpoints_by_addr: RefCell::new(HashMap::new()),
            breakpoints_by_id: RefCell::new(HashMap::new()),
            qemu,
            started: false,
            snapshot_id: OnceCell::new(),
            #[cfg(feature = "systemmode")]
            maps: HashMap::new(),
            #[cfg(feature = "systemmode")]
            allow_page_on_start: false,
            #[cfg(feature = "x86_64")]
            process_only: false,
        };

        emulator.modules.post_qemu_init_all(qemu);

        emulator
    }
}

impl<C, CM, ET, I, IW, S, SM> StdEmulator<C, CM, ET, I, IW, S, SM>
where
    C: Command<I, S>,
    CM: CommandManager<I, S, Commands = C, InputWriter = IW>,
    ET: EmulatorModuleTuple<I, S> + HasStdFiltersTuple + Unpin,
    I: Input + Unpin,
    IW: InputWriter<I, S>,
    S: State + Unpin,
    SM: SnapshotManager,
{
    fn post_qemu_exec(
        &mut self,
        exit_reason: &mut EmulatorExitReason<C>,
    ) -> Result<Option<EmulatorRunResult>> {
        let qemu = self.qemu();

        // If QEMU stopped because of a request, handle it here
        let (command, ret_reg): (C, Option<Regs>) = match exit_reason {
            EmulatorExitReason::QemuExit(shutdown_cause) => match shutdown_cause {
                QemuShutdownCause::HostSignal(signal) => {
                    return Err(EmulatorError::UnhandledSignal(*signal).into());
                }
                QemuShutdownCause::GuestPanic => {
                    return Ok(Some(EmulatorRunResult::EndOfRun(ExitKind::Crash)));
                }
                QemuShutdownCause::GuestShutdown | QemuShutdownCause::HostQmpQuit => {
                    log::warn!("Guest shutdown requested.");
                    panic!("Implement proper exit there...")
                }
                _ => panic!("Unhandled QEMU shutdown cause: {shutdown_cause:?}."),
            },
            EmulatorExitReason::Crash => {
                return Ok(Some(EmulatorRunResult::EndOfRun(ExitKind::Crash)));
            }
            EmulatorExitReason::Timeout => {
                return Ok(Some(EmulatorRunResult::EndOfRun(ExitKind::Timeout)));
            }
            EmulatorExitReason::Breakpoint(bp) => {
                let bp_id = bp.id();
                match bp.trigger(qemu)? {
                    Some(command) => (command, None),
                    None => return Ok(Some(EmulatorRunResult::Breakpoint(bp_id))),
                }
            }
            EmulatorExitReason::CustomInsn(custom_insn) => {
                (custom_insn.command().clone(), Some(custom_insn.ret_reg()))
            }
        };

        // Run the requested command.
        if self.print_commands {
            println!("Received command: {command:?}");
        }
        command.run(self, ret_reg)
    }

    /// Run QEMU, handling stops, until an explicit exit is necessary. Commands that don't end
    /// the run (like snapshot save/load) keep the loop running.
    /// A breakpoint without an explicit handler stops it.
    ///
    /// # Safety
    /// Should, in general, be safe to call.
    /// Of course, the emulated target is not contained securely and can corrupt state or interact with the operating system.
    unsafe fn run_until_outcome(&mut self) -> Result<EmulatorRunResult> {
        loop {
            let mut exit_reason = unsafe { self.run_qemu()? };

            if let Some(outcome) = self.post_qemu_exec(&mut exit_reason)? {
                return Ok(outcome);
            }
        }
    }

    /// Write the input and run a single fuzzing iteration, returning its result.
    ///
    /// # Safety
    /// Should, in general, be safe to call.
    /// Of course, the emulated target is not contained securely and can corrupt state or interact with the operating system.
    pub unsafe fn run(&mut self, state: &mut S, input: &I) -> Result<EmulatorRunResult> {
        if !self.started {
            return Err(EmulatorError::NotStartedYet.into());
        }

        // write the input
        self.input_writer.write_input(self.qemu, state, input)?;

        unsafe { self.run_until_outcome() }
    }

    /// This function will run the emulator until the next breakpoint, or until finish.
    ///
    /// # Safety
    ///
    /// Should, in general, be safe to call.
    /// Of course, the emulated target is not contained securely and can corrupt state or interact with the operating system.
    pub unsafe fn run_qemu(&self) -> Result<EmulatorExitReason<C>> {
        let qemu_exit_reason = unsafe { self.qemu.run()? };

        Ok(match qemu_exit_reason {
            QemuExitReason::End(qemu_shutdown_cause) => {
                EmulatorExitReason::QemuExit(qemu_shutdown_cause)
            }
            QemuExitReason::Crash => EmulatorExitReason::Crash,
            QemuExitReason::Timeout => EmulatorExitReason::Timeout,
            QemuExitReason::Breakpoint(bp_addr) => {
                let bp = self
                    .breakpoints_by_addr
                    .borrow()
                    .get(&bp_addr)
                    .ok_or(EmulatorExitError::BreakpointNotFound(bp_addr))?
                    .clone();
                EmulatorExitReason::Breakpoint(bp.clone())
            }
            QemuExitReason::SyncExit => EmulatorExitReason::CustomInsn(CustomInsn::new(
                self.command_manager.parse(self.qemu)?,
            )),
        })
    }
}
