#[cfg(feature = "systemmode")]
use crate::modules::utils::filters::{HasPageFilter, NopPageFilter};
#[cfg(feature = "systemmode")]
use crate::modules::utils::filters::HasPageFilterTuple;
#[cfg(feature = "systemmode")]
use libaflmm_qemu_sys::GuestPhysAddr;
use crate::{
    GuestAddr, Qemu, QemuParams,
    emu::EmulatorModules,
    modules::utils::filters::{
        HasAddressFilter, HasAddressFilterTuple, HasStdFiltersTuple, NopAddressFilter,
    },
};
use core::ops::Range;
use core::fmt::Debug;
use libaflmm::{
    Result, executors::ExitKind, inputs::Input, observers::ObserversTuple, states::State,
};
use libaflmm_bolts::tuples::{MatchFirstType, SplitBorrowExtractFirstType};
use std::marker::PhantomData;

// #[cfg(feature = "usermode")]
// pub mod usermode;
// #[cfg(feature = "usermode")]
// #[cfg_attr(feature = "hexagon", allow(unused_imports))]
// pub use usermode::*;
//
// #[cfg(feature = "systemmode")]
// pub mod systemmode;
// #[cfg(all(feature = "systemmode", feature = "intel_pt"))]
// pub use systemmode::*;

pub mod edges;
pub use edges::{
    EdgeCoverageModule, EdgeCoverageModuleBuilder, StdEdgeCoverageChildModule,
    StdEdgeCoverageChildModuleBuilder, StdEdgeCoverageClassicModule,
    StdEdgeCoverageClassicModuleBuilder, StdEdgeCoverageFullModule,
    StdEdgeCoverageFullModuleBuilder, StdEdgeCoverageModule, StdEdgeCoverageModuleBuilder,
};
//
// #[cfg(not(cpu_target = "hexagon"))]
// pub mod calls;
// #[cfg(not(cpu_target = "hexagon"))]
// pub use calls::CallTracerModule;

#[cfg(not(any(cpu_target = "mips", cpu_target = "hexagon")))]
pub mod cmplog;
#[cfg(not(any(cpu_target = "mips", cpu_target = "hexagon")))]
pub use cmplog::CmpLogModule;

// #[cfg(not(cpu_target = "hexagon"))]
// pub mod drcov;
// #[cfg(not(cpu_target = "hexagon"))]
// pub use drcov::{DrCovMetadata, DrCovModule, DrCovModuleBuilder};

// pub mod logger;
// pub use logger::LoggerModule;

pub mod utils;
pub use utils::filters::{AddressFilter, PageFilter};

/// [`EmulatorModule`] is a trait designed to define modules that interact with the QEMU emulator
/// during fuzzing. [`EmulatorModule`] provides a set of interfaces (hooks) that can be invoked at various stages
/// of the fuzzer's execution.
///
/// The typical sequence of these hooks execution during a fuzzing session is as follows:
/// ```rust,ignore
/// pre_qemu_init()
/// // Qemu initialization (in the Emulator)
/// post_qemu_init()
/// // Harness initialization
/// first_exec()
///
/// // The following loop is executed for every fuzzing iteration
/// pre_exec()
/// // Harness execution
/// post_exec()
/// ```
///
/// It is important to note that all registered [`EmulatorModule`] instances will have their interfaces (hooks)
/// invoked. The order of invocation depends on the order in which the modules were registered.
///
/// Users typically add hooks, monitoring, or other instrumentation to the **fuzzing target** in [`EmulatorModule`]
/// For example:
/// ```rust,ignore
/// fn post_qemu_init<ET>(&mut self, _qemu: Qemu, _emulator_modules: &mut EmulatorModules<ET>)
/// where
///     ET: EmulatorModuleTuple,
/// {
///     // Add a hook before the execution of a syscall in the fuzzing target
///     _emulator_modules.pre_syscalls(Hook::Function(your_syscall_hooks::<ET>))
///     // ...
/// }
/// ```
/// For more details on adding hooks to the **fuzzing target**, including function signatures,
/// return values, please refer to the [`EmulatorModules`].
// TODO remove 'static when specialization will be stable
pub trait EmulatorModule: 'static + Debug {
    type Input: Input + Unpin;
    type State: State + Unpin;

    const HOOKS_DO_SIDE_EFFECTS: bool = true;

    /// Hook run **before** QEMU is initialized.
    /// This is always run when Emulator gets initialized, in any case.
    /// Install here hooks that should be alive for the whole execution of the VM, even before QEMU gets initialized.
    ///
    /// It is also possible to edit QEMU parameters, just before QEMU gets initialized.
    /// Thus, the module can modify options for QEMU just before it gets initialized.
    fn pre_qemu_init<ET>(
        &mut self,
        _emulator_modules: &mut EmulatorModules<ET>,
        _qemu_params: &mut QemuParams,
    ) where
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>,
    {
    }

    /// Hook run **after** QEMU is initialized.
    /// This is always run when Emulator gets initialized, in any case.
    /// Install here hooks that should be alive for the whole execution of the VM, after QEMU gets initialized.
    fn post_qemu_init<ET>(&mut self, _qemu: Qemu, _emulator_modules: &mut EmulatorModules<ET>)
    where
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>,
    {
    }

    /// Run once just before fuzzing starts.
    /// This call can be delayed to the point at which fuzzing is supposed to start.
    /// It is mostly used to avoid running hooks during VM initialization, either
    /// because it is useless or it would produce wrong results.
    fn first_exec<ET>(
        &mut self,
        _qemu: Qemu,
        _emulator_modules: &mut EmulatorModules<ET>,
        _state: &mut Self::State,
    ) -> Result<()>
    where
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>,
    {
        Ok(())
    }

    /// Run before a new fuzzing run starts.
    /// On the first run, it is executed after [`Self::first_exec`].
    fn pre_exec<ET>(
        &mut self,
        _qemu: Qemu,
        _emulator_modules: &mut EmulatorModules<ET>,
        _state: &mut Self::State,
        _input: &Self::Input,
    ) -> Result<()>
    where
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>,
    {
        Ok(())
    }

    /// Run after a fuzzing run ends.
    fn post_exec<OT, ET>(
        &mut self,
        _qemu: Qemu,
        _emulator_modules: &mut EmulatorModules<ET>,
        _state: &mut Self::State,
        _input: &Self::Input,
        _observers: &mut OT,
        _exit_kind: &mut ExitKind,
    ) -> Result<()>
    where
        OT: ObserversTuple<Self::State>,
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>,
    {
        Ok(())
    }

    /// # Safety
    ///
    /// This is getting executed in a signal handler.
    unsafe fn on_crash(&mut self) -> Result<()> {
        Ok(())
    }

    /// # Safety
    ///
    /// This is getting executed in a signal handler.
    unsafe fn on_timeout(&mut self) -> Result<()> {
        Ok(())
    }
}

pub trait EmulatorModuleTuple:
    MatchFirstType + for<'a> SplitBorrowExtractFirstType<'a> + Unpin
{
    type Input: Input + Unpin;
    type State: State + Debug + Unpin;

    const HOOKS_DO_SIDE_EFFECTS: bool;

    fn pre_qemu_init_all<ET>(
        &mut self,
        emulator_modules: &mut EmulatorModules<ET>,
        qemu_params: &mut QemuParams,
    ) where
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>;

    fn post_qemu_init_all<ET>(&mut self, qemu: Qemu, emulator_modules: &mut EmulatorModules<ET>)
    where
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>;

    fn first_exec_all<ET>(
        &mut self,
        qemu: Qemu,
        emulator_modules: &mut EmulatorModules<ET>,
        state: &mut Self::State,
    ) -> Result<()>
    where
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>;

    fn pre_exec_all<ET>(
        &mut self,
        qemu: Qemu,
        emulator_modules: &mut EmulatorModules<ET>,
        state: &mut Self::State,
        input: &Self::Input,
    ) -> Result<()>
    where
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>;

    fn post_exec_all<OT, ET>(
        &mut self,
        qemu: Qemu,
        emulator_modules: &mut EmulatorModules<ET>,
        state: &mut Self::State,
        input: &Self::Input,
        observers: &mut OT,
        exit_kind: &mut ExitKind,
    ) -> Result<()>
    where
        OT: ObserversTuple<Self::State>,
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>;

    /// # Safety
    ///
    /// This is getting executed in a signal handler.
    unsafe fn on_crash_all(&mut self) -> Result<()>;

    /// # Safety
    ///
    /// This is getting executed in a signal handler.
    unsafe fn on_timeout_all(&mut self) -> Result<()>;
}

#[derive(Debug)]
pub struct NopModule<I, S> {
    phantom: PhantomData<(I, S)>,
}

impl<I, S> Default for NopModule<I, S> {
    fn default() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<'a, I, S> SplitBorrowExtractFirstType<'a> for NopModule<I, S> {
    type SplitBorrowResult = ();
    type SplitBorrowMutResult = ();

    fn borrow(&'a self) -> Self::SplitBorrowResult {}

    fn borrow_mut(&'a mut self) -> Self::SplitBorrowMutResult {}
}

impl<I, S> MatchFirstType for NopModule<I, S> {
    fn match_first_type<T: 'static>(&self) -> Option<&T> {
        None
    }
    fn match_first_type_mut<T: 'static>(&mut self) -> Option<&mut T> {
        None
    }
}

impl<I, S> HasAddressFilter for NopModule<I, S> {
    type AddressFilter = NopAddressFilter;

    fn address_filter(&self) -> &Self::AddressFilter {
        &NopAddressFilter
    }

    fn address_filter_mut(&mut self) -> &mut Self::AddressFilter {
        static mut ADDRESS_FILTER: NopAddressFilter = NopAddressFilter;
        unsafe { &mut *&raw mut ADDRESS_FILTER }
    }
}

#[cfg(feature = "systemmode")]
impl<I, S> HasPageFilter for NopModule<I, S> {
    type PageFilter = NopPageFilter;

    fn page_filter(&self) -> &Self::PageFilter {
        &NopPageFilter
    }

    fn page_filter_mut(&mut self) -> &mut Self::PageFilter {
        static mut PAGE_FILTER: NopPageFilter = NopPageFilter;
        unsafe { &mut *&raw mut PAGE_FILTER }
    }
}

impl<I, S> HasAddressFilterTuple for NopModule<I, S> {
    fn allow_address_range_all(&mut self, _address_range: &Range<GuestAddr>) {}

    fn allowed_address_all(&self, _address: &GuestAddr) -> bool {
        true
    }
}

#[cfg(feature = "systemmode")]
impl<I, S> HasPageFilterTuple for NopModule<I, S> {
    fn allow_page_id_all(&mut self, _page_id: GuestPhysAddr) {}

    fn allowed_page_id_all(&self, _page_id: &GuestPhysAddr) -> bool {
        true
    }
}

impl<I, S> HasStdFiltersTuple for NopModule<I, S> {}

impl<I, S> EmulatorModuleTuple for NopModule<I, S>
where
    I: Input + Unpin,
    S: State + Debug + Unpin,
{
    type Input = I;
    type State = S;

    const HOOKS_DO_SIDE_EFFECTS: bool = false;

    fn pre_qemu_init_all<ET>(
        &mut self,
        _emulator_modules: &mut EmulatorModules<ET>,
        _qemu_params: &mut QemuParams,
    ) where
        ET: EmulatorModuleTuple,
    {
    }

    fn post_qemu_init_all<ET>(&mut self, _qemu: Qemu, _emulator_modules: &mut EmulatorModules<ET>)
    where
        ET: EmulatorModuleTuple,
    {
    }

    fn first_exec_all<ET>(
        &mut self,
        _qemu: Qemu,
        _emulator_modules: &mut EmulatorModules<ET>,
        _state: &mut Self::State,
    ) -> Result<()>
    where
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>,
    {
        Ok(())
    }

    fn pre_exec_all<ET>(
        &mut self,
        _qemu: Qemu,
        _emulator_modules: &mut EmulatorModules<ET>,
        _state: &mut Self::State,
        _input: &Self::Input,
    ) -> Result<()>
    where
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>,
    {
        Ok(())
    }

    fn post_exec_all<OT, ET>(
        &mut self,
        _qemu: Qemu,
        _emulator_modules: &mut EmulatorModules<ET>,
        _state: &mut Self::State,
        _input: &Self::Input,
        _observers: &mut OT,
        _exit_kind: &mut ExitKind,
    ) -> Result<()>
    where
        OT: ObserversTuple<Self::State>,
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>,
    {
        Ok(())
    }

    unsafe fn on_crash_all(&mut self) -> Result<()> {
        Ok(())
    }

    unsafe fn on_timeout_all(&mut self) -> Result<()> {
        Ok(())
    }
}

impl<Head, Tail> EmulatorModuleTuple for (Head, Tail)
where
    Head: EmulatorModule + Unpin,
    Head::State: Debug,
    Tail: EmulatorModuleTuple<Input = Head::Input, State = Head::State>,
{
    type Input = Head::Input;
    type State = Head::State;

    const HOOKS_DO_SIDE_EFFECTS: bool = Head::HOOKS_DO_SIDE_EFFECTS || Tail::HOOKS_DO_SIDE_EFFECTS;

    fn pre_qemu_init_all<ET>(
        &mut self,
        emulator_modules: &mut EmulatorModules<ET>,
        qemu_params: &mut QemuParams,
    ) where
        ET: EmulatorModuleTuple<Input = Head::Input, State = Head::State>,
    {
        self.0.pre_qemu_init(emulator_modules, qemu_params);
        self.1.pre_qemu_init_all(emulator_modules, qemu_params);
    }

    fn post_qemu_init_all<ET>(&mut self, qemu: Qemu, emulator_modules: &mut EmulatorModules<ET>)
    where
        ET: EmulatorModuleTuple<Input = Head::Input, State = Head::State>,
    {
        self.0.post_qemu_init(qemu, emulator_modules);
        self.1.post_qemu_init_all(qemu, emulator_modules);
    }

    fn first_exec_all<ET>(
        &mut self,
        qemu: Qemu,
        emulator_modules: &mut EmulatorModules<ET>,
        state: &mut Self::State,
    ) -> Result<()>
    where
        ET: EmulatorModuleTuple<Input = Head::Input, State = Head::State>,
    {
        self.0.first_exec(qemu, emulator_modules, state)?;
        self.1.first_exec_all(qemu, emulator_modules, state)
    }

    fn pre_exec_all<ET>(
        &mut self,
        qemu: Qemu,
        emulator_modules: &mut EmulatorModules<ET>,
        state: &mut Self::State,
        input: &Self::Input,
    ) -> Result<()>
    where
        ET: EmulatorModuleTuple<Input = Head::Input, State = Head::State>,
    {
        self.0.pre_exec(qemu, emulator_modules, state, input)?;
        self.1.pre_exec_all(qemu, emulator_modules, state, input)
    }

    fn post_exec_all<OT, ET>(
        &mut self,
        qemu: Qemu,
        emulator_modules: &mut EmulatorModules<ET>,
        state: &mut Self::State,
        input: &Self::Input,
        observers: &mut OT,
        exit_kind: &mut ExitKind,
    ) -> Result<()>
    where
        OT: ObserversTuple<Head::State>,
        ET: EmulatorModuleTuple<Input = Head::Input, State = Head::State>,
    {
        self.0
            .post_exec(qemu, emulator_modules, state, input, observers, exit_kind)?;
        self.1
            .post_exec_all(qemu, emulator_modules, state, input, observers, exit_kind)
    }

    unsafe fn on_crash_all(&mut self) -> Result<()> {
        unsafe {
            self.0.on_crash()?;
            self.1.on_crash_all()
        }
    }

    unsafe fn on_timeout_all(&mut self) -> Result<()> {
        unsafe {
            self.0.on_timeout()?;
            self.1.on_timeout_all()
        }
    }
}

impl<M> EmulatorModule for Option<M>
where
    M: EmulatorModule,
{
    type Input = M::Input;
    type State = M::State;

    const HOOKS_DO_SIDE_EFFECTS: bool = M::HOOKS_DO_SIDE_EFFECTS;

    fn pre_qemu_init<ET>(
        &mut self,
        emulator_modules: &mut EmulatorModules<ET>,
        qemu_params: &mut QemuParams,
    ) where
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>,
    {
        if let Some(m) = self {
            m.pre_qemu_init(emulator_modules, qemu_params);
        }
    }

    fn post_qemu_init<ET>(&mut self, qemu: Qemu, emulator_modules: &mut EmulatorModules<ET>)
    where
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>,
    {
        if let Some(m) = self {
            m.post_qemu_init(qemu, emulator_modules);
        }
    }

    fn first_exec<ET>(
        &mut self,
        qemu: Qemu,
        emulator_modules: &mut EmulatorModules<ET>,
        state: &mut Self::State,
    ) -> Result<()>
    where
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>,
    {
        if let Some(m) = self {
            m.first_exec(qemu, emulator_modules, state)
        } else {
            Ok(())
        }
    }

    fn pre_exec<ET>(
        &mut self,
        qemu: Qemu,
        emulator_modules: &mut EmulatorModules<ET>,
        state: &mut Self::State,
        input: &Self::Input,
    ) -> Result<()>
    where
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>,
    {
        if let Some(m) = self {
            m.pre_exec(qemu, emulator_modules, state, input)
        } else {
            Ok(())
        }
    }

    fn post_exec<OT, ET>(
        &mut self,
        qemu: Qemu,
        emulator_modules: &mut EmulatorModules<ET>,
        state: &mut Self::State,
        input: &Self::Input,
        observers: &mut OT,
        exit_kind: &mut ExitKind,
    ) -> Result<()>
    where
        OT: ObserversTuple<Self::State>,
        ET: EmulatorModuleTuple<Input = Self::Input, State = Self::State>,
    {
        if let Some(m) = self {
            m.post_exec(qemu, emulator_modules, state, input, observers, exit_kind)
        } else {
            Ok(())
        }
    }

    unsafe fn on_crash(&mut self) -> Result<()> {
        if let Some(m) = self {
            unsafe {
                m.on_crash()?;
            }
        }

        Ok(())
    }

    unsafe fn on_timeout(&mut self) -> Result<()> {
        if let Some(m) = self {
            unsafe {
                m.on_timeout()?;
            }
        }

        Ok(())
    }
}
