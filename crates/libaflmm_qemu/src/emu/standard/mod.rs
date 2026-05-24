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
use crate::Result;
use crate::{
    breakpoint::{Breakpoint, BreakpointId},
    command::{CommandManager, NopCommandManager, StdCommandManager},
    emu::{
        Emulator, EmulatorDriverResult, EmulatorExitError, EmulatorExitResult, EmulatorHooks,
        EmulatorModules, NopSnapshotManager,
    },
    modules::EmulatorModuleTuple,
    qemu::{
        Qemu, QemuExitReason, QemuHooks, QemuParams, QemuShutdownCause, config::QemuConfigBuilder,
    },
    sync_exit::CustomInsn,
};
use libaflmm::{executors::ExitKind, inputs::Input, observers::ObserversTuple, states::State};
use libaflmm_qemu_sys::GuestAddr;
use std::cell::OnceCell;
use std::{cell::RefCell, collections::HashMap, fmt::Debug, marker::PhantomData, pin::Pin};

pub mod builder;
pub use builder::StdEmulatorBuilder;

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
pub struct StdEmulator<C, CM, ET, I, IS, S, SM> {
    snapshot_manager: SM,
    modules: Pin<Box<EmulatorModules<ET, I, S>>>,
    command_manager: CM,
    input_setter: IS,
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
    maps: HashMap<MapKind, PhysMemoryChunk>,
    phantom: PhantomData<(I, S)>,
}

impl<C, CM, ET, I, IW, S, SM> Emulator for StdEmulator<C, CM, ET, I, IW, S, SM>
where
    C: Command,
    CM: CommandManager<Commands = C>,
    ET: EmulatorModuleTuple<I, S> + HasStdFiltersTuple + Unpin,
    I: Input + Unpin,
    IW: InputWriter<I, S>,
    S: State + Unpin,
    SM: SnapshotManager,
{
    type Input = I;
    type State = S;

    type Command = C;
    type CommandManager = CM;
    type InputWriter = IW;
    type Modules = ET;
    type SnapshotManager = SM;

    fn first_exec(&mut self, state: &mut Self::State) -> Result<()> {
        let qemu = self.qemu();
        self.modules_mut().first_exec_all(qemu, state)
    }

    fn pre_exec(&mut self, state: &mut Self::State, input: &Self::Input) -> Result<()> {
        let qemu = self.qemu();
        self.modules_mut().pre_exec_all(qemu, state, input)?;

        Ok(())
    }

    fn exec_input(&mut self, state: &mut Self::State, input: &Self::Input) -> Result<ExitKind> {
        match unsafe { self.run(state, input)? } {
            EmulatorDriverResult::EndOfRun(exit_kind) => Ok(exit_kind),
            EmulatorDriverResult::ReturnToClient(EmulatorExitResult::QemuExit(qemu_exit)) => {
                match qemu_exit {
                    QemuShutdownCause::GuestPanic
                    | QemuShutdownCause::GuestReset
                    | QemuShutdownCause::GuestShutdown => Ok(ExitKind::Crash),
                    e => panic!("Bug in LibAFL QEMU fuzzer: {e:?}"),
                }
            }
            EmulatorDriverResult::ShutdownRequest => {
                log::warn!(
                    "QEMU received a shutdown request during a fuzzing run. It will be considered as a crash."
                );

                Ok(ExitKind::Crash)
            }
            EmulatorDriverResult::ReturnToClient(exit_reason) => {
                panic!("Unexpected return to client: {exit_reason:?}")
            }
        }
    }

    fn post_exec<OT>(
        &mut self,
        state: &mut Self::State,
        input: &Self::Input,
        observers: &mut OT,
        exit_kind: &mut ExitKind,
    ) -> Result<()>
    where
        OT: ObserversTuple<Self::State>,
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
        self.snapshot_id.get().map(|sid| *sid)
    }

    fn set_snapshot_id(&mut self, snapshot_id: SnapshotId) -> Result<()> {
        self.snapshot_id
            .set(snapshot_id)
            .map_err(|_| EmulatorError::MultipleSnapshotDefinition.into())
    }

    fn set_input_location(&mut self, input_location: &InputLocation) -> Result<()> {
        self.input_setter.set_input_location(input_location.clone())
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
            input_setter: input_writer,
            // hooks_locked: true,
            print_commands: true,
            breakpoints_by_addr: RefCell::new(HashMap::new()),
            breakpoints_by_id: RefCell::new(HashMap::new()),
            qemu,
            started: false,
            snapshot_id: OnceCell::new(),
            phantom: PhantomData,
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
    C: Command,
    CM: CommandManager<Commands = C>,
    ET: EmulatorModuleTuple<I, S> + HasStdFiltersTuple + Unpin,
    I: Input + Unpin,
    IW: InputWriter<I, S>,
    S: State + Unpin,
    SM: SnapshotManager,
{
    fn post_qemu_exec(
        &mut self,
        exit_reason: &mut EmulatorExitResult<C>,
    ) -> Result<Option<EmulatorDriverResult<C>>> {
        let qemu = self.qemu();

        // If QEMU stopped because of a request, handle it here
        let (command, ret_reg): (Option<C>, Option<Regs>) = match exit_reason {
            EmulatorExitResult::QemuExit(shutdown_cause) => match shutdown_cause {
                QemuShutdownCause::HostSignal(signal) => {
                    return Err(EmulatorError::UnhandledSignal(*signal).into());
                }
                QemuShutdownCause::GuestPanic => {
                    return Ok(Some(EmulatorDriverResult::EndOfRun(ExitKind::Crash)));
                }
                QemuShutdownCause::GuestShutdown | QemuShutdownCause::HostQmpQuit => {
                    log::warn!("Guest shutdown. Stopping fuzzing...");
                    // std::process::exit(CTRL_C_EXIT);
                    panic!("Implement proper exit there...")
                }
                _ => panic!("Unhandled QEMU shutdown cause: {shutdown_cause:?}."),
            },
            EmulatorExitResult::Crash => {
                return Ok(Some(EmulatorDriverResult::EndOfRun(ExitKind::Crash)));
            }
            EmulatorExitResult::Timeout => {
                return Ok(Some(EmulatorDriverResult::EndOfRun(ExitKind::Timeout)));
            }
            EmulatorExitResult::FuzzingStarts => {
                return Ok(Some(EmulatorDriverResult::ReturnToClient(
                    EmulatorExitResult::FuzzingStarts,
                )));
            }
            EmulatorExitResult::Breakpoint(bp) => (bp.trigger(qemu), None),
            EmulatorExitResult::CustomInsn(custom_insn) => {
                let command = custom_insn.command().clone();
                (Some(command), Some(custom_insn.ret_reg()))
            }
        };

        // If QEMU requested to handle a command, run it here.
        if let Some(cmd) = command {
            if self.print_commands {
                println!("Received command: {cmd:?}");
            }
            cmd.run(self, ret_reg)
        } else {
            Ok(Some(EmulatorDriverResult::ReturnToClient(
                exit_reason.clone(),
            )))
        }
    }

    /// This function will run the emulator until the exit handler decides to stop the execution for
    /// whatever reason, depending on the choosen handler.
    /// It is a higher-level abstraction of [`Emulator::run`] that will take care of some part of the runtime logic,
    /// returning only when something interesting happen.
    ///
    /// # Safety
    /// Should, in general, be safe to call.
    /// Of course, the emulated target is not contained securely and can corrupt state or interact with the operating system.
    pub unsafe fn run(&mut self, state: &mut S, input: &I) -> Result<EmulatorDriverResult<C>> {
        if !self.started {
            return Err(EmulatorError::NotStartedYet.into());
        }

        // write the input
        self.input_setter
            .write_input(self.qemu, state, input)
            .unwrap();

        loop {
            // Run QEMU
            log::debug!("Running QEMU...");
            let mut exit_reason = unsafe { self.run_qemu()? };
            log::debug!("QEMU stopped.");

            // Handle QEMU exit
            if let Some(exit_handler_result) = self.post_qemu_exec(&mut exit_reason)? {
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
    pub unsafe fn start(&mut self) -> Result<()> {
        loop {
            let mut exit_result = unsafe { self.run_qemu()? };

            // Handle QEMU exit
            if let Some(exit_handler_result) = self.post_qemu_exec(&mut exit_result)? {
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
                        return Err(EmulatorError::EndBeforeStart.into());
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
    pub unsafe fn run_qemu(&self) -> Result<EmulatorExitResult<C>> {
        let qemu_exit_reason = unsafe { self.qemu.run()? };

        Ok(match qemu_exit_reason {
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
        })
    }
}
