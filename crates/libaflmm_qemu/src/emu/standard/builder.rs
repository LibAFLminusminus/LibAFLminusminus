use crate::Result;
use crate::emu::NopInputWriter;
use crate::emu::StdInputWriter;
use crate::emu::snapshots::StdSnapshotManager;
#[cfg(doc)]
use crate::qemu::config::QemuConfig;
use crate::{
    command::{NopCommandManager, StdCommandManager},
    emu::{NopSnapshotManager, StdEmulator},
    modules::{EmulatorModule, EmulatorModuleTuple},
    qemu::{Qemu, QemuHooks, QemuInitError, QemuParams, config::QemuConfigBuilder},
};
use libaflmm::{inputs::Input, states::State};
use libaflmm_bolts::tuples::{Append, Prepend, tuple_list};
use std::marker::PhantomData;

/// An [`Emulator`](crate::emu::Emulator) Builder.
///
/// It is the most common way to create a new [`Emulator`](crate::emu::Emulator).
/// In addition to the main components of an [`Emulator`](crate::emu::Emulator), it expects to receive a way to initialize [`Qemu`].
/// It must be set through [`StdEmulatorBuilder::qemu_parameters`].
/// At the moment, there are two main ways to initialize QEMU:
/// - with a QEMU-compatible CLI. It will be given to QEMU as-is. The first argument should always be a path to the running binary, as expected by execve.
/// - with an instance of [`QemuConfig`]. It is a more programmatic way to configure [`Qemu`]. It should be built using [`QemuConfigBuilder`].
#[derive(Clone)]
pub struct StdEmulatorBuilder<C, CM, ET, QP, I, IW, S, SM> {
    modules: ET,
    input_writer: IW,
    snapshot_manager: SM,
    command_manager: CM,
    qemu_parameters: Option<QP>,
    phantom: PhantomData<(C, I, S)>,
}

impl<C, I, S>
    StdEmulatorBuilder<
        C,
        NopCommandManager,
        (),
        QemuConfigBuilder,
        I,
        NopInputWriter,
        S,
        NopSnapshotManager,
    >
{
    #[must_use]
    pub fn empty() -> Self {
        Self {
            modules: tuple_list!(),
            snapshot_manager: NopSnapshotManager,
            command_manager: NopCommandManager,
            input_writer: NopInputWriter,
            qemu_parameters: None,
            phantom: PhantomData,
        }
    }
}

#[cfg(all(feature = "usermode", not(feature = "systemmode")))]
impl<C, I, S>
    StdEmulatorBuilder<
        C,
        StdCommandManager,
        (),
        QemuConfigBuilder,
        I,
        StdInputWriter,
        S,
        StdSnapshotManager,
    >
where
    S: State + Unpin,
    I: Input,
{
    #[must_use]
    #[expect(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self {
            modules: tuple_list!(),
            command_manager: StdCommandManager::default(),
            snapshot_manager: StdSnapshotManager::default(),
            input_writer: StdInputWriter::default(),
            qemu_parameters: None,
            phantom: PhantomData,
        }
    }
}

#[cfg(feature = "systemmode")]
impl<C, I, S>
    StdEmulatorBuilder<
        C,
        StdCommandManager,
        (),
        QemuConfigBuilder,
        I,
        StdInputWriter,
        S,
        StdSnapshotManager,
    >
where
    S: State + Unpin,
    I: Input,
{
    #[expect(clippy::should_implement_trait)]
    #[must_use]
    pub fn default() -> Self {
        Self {
            modules: (),
            command_manager: StdCommandManager::default(),
            snapshot_manager: StdSnapshotManager::default(),
            input_writer: StdInputWriter::default(),
            qemu_parameters: None,
            phantom: PhantomData,
        }
    }
}

impl<C, CM, ET, QP, I, IW, S, SM> StdEmulatorBuilder<C, CM, ET, QP, I, IW, S, SM>
where
    I: Unpin,
    S: Unpin,
{
    fn new(
        modules: ET,
        input_writer: IW,
        command_manager: CM,
        snapshot_manager: SM,
        qemu_parameters: Option<QP>,
    ) -> Self {
        Self {
            modules,
            input_writer,
            command_manager,
            snapshot_manager,
            qemu_parameters,
            phantom: PhantomData,
        }
    }

    #[allow(clippy::type_complexity)]
    pub fn build<E>(self) -> Result<StdEmulator<C, CM, ET, I, IW, S, SM>>
    where
        ET: EmulatorModuleTuple<I, S>,
        QP: TryInto<QemuParams, Error = E>,
        QemuInitError: From<E>,
    {
        let qemu_params: QemuParams = self
            .qemu_parameters
            .ok_or(QemuInitError::NoParametersProvided)?
            .try_into()
            .map_err(QemuInitError::from)?;

        StdEmulator::new(
            qemu_params,
            self.modules,
            self.input_writer,
            self.snapshot_manager,
            self.command_manager,
        )
    }

    #[allow(clippy::type_complexity)]
    pub fn build_with_qemu(self, qemu: Qemu) -> Result<StdEmulator<C, CM, ET, I, IW, S, SM>>
    where
        ET: EmulatorModuleTuple<I, S>,
    {
        // The logic from Emulator::new needs to be duplicated here because of type mismatch on modules
        //  between Emulator::new and Emulator::new_wit_qemu
        let emulator_hooks = unsafe { super::EmulatorHooks::new(QemuHooks::get_unchecked()) };
        let emulator_modules = unsafe { super::EmulatorModules::new(emulator_hooks, self.modules) };

        unsafe {
            Ok(StdEmulator::new_with_qemu(
                qemu,
                emulator_modules,
                self.input_writer,
                self.snapshot_manager,
                self.command_manager,
            ))
        }
    }
}

impl<C, CM, ET, QP, I, IW, S, SM> StdEmulatorBuilder<C, CM, ET, QP, I, IW, S, SM>
where
    I: Unpin,
    S: Unpin,
{
    #[must_use]
    pub fn qemu_parameters<QP2>(
        self,
        qemu_parameters: QP2,
    ) -> StdEmulatorBuilder<C, CM, ET, QP2, I, IW, S, SM>
    where
        QP2: Into<QemuParams>,
    {
        StdEmulatorBuilder::new(
            self.modules,
            self.input_writer,
            self.command_manager,
            self.snapshot_manager,
            Some(qemu_parameters),
        )
    }

    pub fn prepend_module<EM>(
        self,
        module: EM,
    ) -> StdEmulatorBuilder<C, CM, (EM, ET), QP, I, IW, S, SM>
    where
        EM: EmulatorModule<I, S> + Unpin,
        ET: EmulatorModuleTuple<I, S>,
    {
        StdEmulatorBuilder::new(
            self.modules.prepend(module),
            self.input_writer,
            self.command_manager,
            self.snapshot_manager,
            self.qemu_parameters,
        )
    }

    pub fn append_module<EM>(
        self,
        module: EM,
    ) -> StdEmulatorBuilder<C, CM, (ET, EM), QP, I, IW, S, SM>
    where
        EM: EmulatorModule<I, S> + Unpin,
        ET: EmulatorModuleTuple<I, S>,
    {
        StdEmulatorBuilder::new(
            self.modules.append(module),
            self.input_writer,
            self.command_manager,
            self.snapshot_manager,
            self.qemu_parameters,
        )
    }

    pub fn input_writer<IW2>(
        self,
        input_writer: IW2,
    ) -> StdEmulatorBuilder<C, CM, ET, QP, I, IW2, S, SM> {
        StdEmulatorBuilder::new(
            self.modules,
            input_writer,
            self.command_manager,
            self.snapshot_manager,
            self.qemu_parameters,
        )
    }

    pub fn command_manager<CM2>(
        self,
        command_manager: CM2,
    ) -> StdEmulatorBuilder<C, CM2, ET, QP, I, IW, S, SM> {
        StdEmulatorBuilder::new(
            self.modules,
            self.input_writer,
            command_manager,
            self.snapshot_manager,
            self.qemu_parameters,
        )
    }

    pub fn modules<ET2>(self, modules: ET2) -> StdEmulatorBuilder<C, CM, ET2, QP, I, IW, S, SM> {
        StdEmulatorBuilder::new(
            modules,
            self.input_writer,
            self.command_manager,
            self.snapshot_manager,
            self.qemu_parameters,
        )
    }

    pub fn snapshot_manager<SM2>(
        self,
        snapshot_manager: SM2,
    ) -> StdEmulatorBuilder<C, CM, ET, QP, I, IW, S, SM2> {
        StdEmulatorBuilder::new(
            self.modules,
            self.input_writer,
            self.command_manager,
            snapshot_manager,
            self.qemu_parameters,
        )
    }
}
