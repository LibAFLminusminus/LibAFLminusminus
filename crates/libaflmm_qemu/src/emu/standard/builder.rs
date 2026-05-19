#[cfg(doc)]
use crate::config::QemuConfig;
#[cfg(feature = "systemmode")]
use crate::standard::FastSnapshotManager;
use crate::{
    NopEmulatorDriver, NopSnapshotManager, Qemu, QemuInitError, QemuParams, StdEmulator,
    StdEmulatorDriver,
    command::{NopCommandManager, StdCommandManager},
    config::QemuConfigBuilder,
    modules::{EmulatorModule, EmulatorModuleTuple},
};
use libaflmm::{inputs::Input, states::State};
use libaflmm_bolts::tuples::{Append, Prepend, tuple_list};
use std::marker::PhantomData;

/// An [`Emulator`] Builder.
///
/// It is the most common way to create a new [`Emulator`].
/// In addition to the main components of an [`Emulator`], it expects to receive a way to initialize [`Qemu`].
/// It must be set through [`EmulatorBuilder::qemu_parameters`].
/// At the moment, there are two main ways to initialize QEMU:
/// - with a QEMU-compatible CLI. It will be given to QEMU as-is. The first argument should always be a path to the running binary, as expected by execve.
/// - with an instance of [`QemuConfig`]. It is a more programmatic way to configure [`Qemu`]. It should be built using [`QemuConfigBuilder`].
#[derive(Clone)]
pub struct StdEmulatorBuilder<C, CM, ED, ET, QP, I, S, SM> {
    modules: ET,
    driver: ED,
    snapshot_manager: SM,
    command_manager: CM,
    qemu_parameters: Option<QP>,
    phantom: PhantomData<(C, I, S)>,
}

impl<C, I, S>
    StdEmulatorBuilder<
        C,
        NopCommandManager,
        NopEmulatorDriver,
        (),
        QemuConfigBuilder,
        I,
        S,
        NopSnapshotManager,
    >
{
    #[must_use]
    pub fn empty() -> Self {
        Self {
            modules: tuple_list!(),
            driver: NopEmulatorDriver,
            snapshot_manager: NopSnapshotManager,
            command_manager: NopCommandManager,
            qemu_parameters: None,
            phantom: PhantomData,
        }
    }
}

#[cfg(all(feature = "usermode", not(feature = "systemmode")))]
impl<C, I, S>
    StdEmulatorBuilder<
        C,
        StdCommandManager<S>,
        StdEmulatorDriver,
        (),
        QemuConfigBuilder,
        I,
        S,
        super::StdSnapshotManager,
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
            snapshot_manager: super::StdSnapshotManager::default(),
            driver: StdEmulatorDriver::builder().build(),
            qemu_parameters: None,
            phantom: PhantomData,
        }
    }
}

#[cfg(feature = "systemmode")]
impl<C, I, S>
    StdEmulatorBuilder<
        C,
        StdCommandManager<S>,
        StdEmulatorDriver,
        (),
        QemuConfigBuilder,
        I,
        S,
        super::systemmode::StdSnapshotManager,
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
            snapshot_manager: FastSnapshotManager::default(),
            driver: StdEmulatorDriver::builder().build(),
            qemu_parameters: None,
            phantom: PhantomData,
        }
    }
}

impl<C, CM, ED, ET, QP, I, S, SM> StdEmulatorBuilder<C, CM, ED, ET, QP, I, S, SM>
where
    I: Unpin,
    S: Unpin,
{
    fn new(
        modules: ET,
        driver: ED,
        command_manager: CM,
        snapshot_manager: SM,
        qemu_parameters: Option<QP>,
    ) -> Self {
        Self {
            modules,
            command_manager,
            driver,
            snapshot_manager,
            qemu_parameters,
            phantom: PhantomData,
        }
    }

    #[allow(clippy::type_complexity)]
    pub fn build<E>(self) -> Result<StdEmulator<C, CM, ED, ET, I, S, SM>, QemuInitError>
    where
        ET: EmulatorModuleTuple<Input = I, State = S>,
        QP: TryInto<QemuParams, Error = E>,
        QemuInitError: From<E>,
    {
        let qemu_params: QemuParams = self
            .qemu_parameters
            .ok_or(QemuInitError::NoParametersProvided)?
            .try_into()?;

        StdEmulator::new(
            qemu_params,
            self.modules,
            self.driver,
            self.snapshot_manager,
            self.command_manager,
        )
    }

    #[allow(clippy::type_complexity)]
    pub fn build_with_qemu(
        self,
        qemu: Qemu,
    ) -> Result<StdEmulator<C, CM, ED, ET, I, S, SM>, QemuInitError>
    where
        ET: EmulatorModuleTuple<Input = I, State = S>,
    {
        // The logic from Emulator::new needs to be duplicated here because of type mismatch on modules
        //  between Emulator::new and Emulator::new_wit_qemu
        let emulator_hooks =
            unsafe { super::EmulatorHooks::new(crate::QemuHooks::get_unchecked()) };
        let emulator_modules = unsafe { super::EmulatorModules::new(emulator_hooks, self.modules) };

        unsafe {
            Ok(StdEmulator::new_with_qemu(
                qemu,
                emulator_modules,
                self.driver,
                self.snapshot_manager,
                self.command_manager,
            ))
        }
    }
}

impl<C, CM, ED, ET, QP, I, S, SM> StdEmulatorBuilder<C, CM, ED, ET, QP, I, S, SM>
where
    I: Unpin,
    S: Unpin,
{
    #[must_use]
    pub fn qemu_parameters<QP2>(
        self,
        qemu_parameters: QP2,
    ) -> StdEmulatorBuilder<C, CM, ED, ET, QP2, I, S, SM>
    where
        QP2: Into<QemuParams>,
    {
        StdEmulatorBuilder::new(
            self.modules,
            self.driver,
            self.command_manager,
            self.snapshot_manager,
            Some(qemu_parameters),
        )
    }

    pub fn prepend_module<EM>(
        self,
        module: EM,
    ) -> StdEmulatorBuilder<C, CM, ED, (EM, ET), QP, I, S, SM>
    where
        EM: EmulatorModule<Input = I, State = S> + Unpin,
        ET: EmulatorModuleTuple<Input = I, State = S>,
    {
        StdEmulatorBuilder::new(
            self.modules.prepend(module),
            self.driver,
            self.command_manager,
            self.snapshot_manager,
            self.qemu_parameters,
        )
    }

    pub fn append_module<EM>(
        self,
        module: EM,
    ) -> StdEmulatorBuilder<C, CM, ED, (ET, EM), QP, I, S, SM>
    where
        EM: EmulatorModule<Input = I, State = S> + Unpin,
        ET: EmulatorModuleTuple<Input = I, State = S>,
    {
        StdEmulatorBuilder::new(
            self.modules.append(module),
            self.driver,
            self.command_manager,
            self.snapshot_manager,
            self.qemu_parameters,
        )
    }

    pub fn driver<ED2>(self, driver: ED2) -> StdEmulatorBuilder<C, CM, ED2, ET, QP, I, S, SM> {
        StdEmulatorBuilder::new(
            self.modules,
            driver,
            self.command_manager,
            self.snapshot_manager,
            self.qemu_parameters,
        )
    }

    pub fn command_manager<CM2>(
        self,
        command_manager: CM2,
    ) -> StdEmulatorBuilder<C, CM2, ED, ET, QP, I, S, SM> {
        StdEmulatorBuilder::new(
            self.modules,
            self.driver,
            command_manager,
            self.snapshot_manager,
            self.qemu_parameters,
        )
    }

    pub fn modules<ET2>(self, modules: ET2) -> StdEmulatorBuilder<C, CM, ED, ET2, QP, I, S, SM> {
        StdEmulatorBuilder::new(
            modules,
            self.driver,
            self.command_manager,
            self.snapshot_manager,
            self.qemu_parameters,
        )
    }

    pub fn snapshot_manager<SM2>(
        self,
        snapshot_manager: SM2,
    ) -> StdEmulatorBuilder<C, CM, ED, ET, QP, I, S, SM2> {
        StdEmulatorBuilder::new(
            self.modules,
            self.driver,
            self.command_manager,
            snapshot_manager,
            self.qemu_parameters,
        )
    }
}
