use std::marker::PhantomData;

use libaflmm::states::CoreState;

use super::{
    EdgeCoverageVariant,
    helpers::{gen_unique_edge_ids, trace_edge_hitcount, trace_edge_single},
};
use crate::{
    EmulatorModules, Hook,
    modules::{
        AddressFilter, EdgeCoverageModule, EdgeCoverageModuleBuilder, EmulatorModuleTuple,
        PageFilter,
        utils::filters::{StdAddressFilter, StdPageFilter},
    },
};

#[derive(Debug)]
pub struct EdgeCoverageFullVariant;

pub type StdEdgeCoverageFullModule<I, S> =
    EdgeCoverageModule<StdAddressFilter, I, StdPageFilter, S, EdgeCoverageFullVariant, false, 0>;
pub type StdEdgeCoverageFullModuleBuilder<I, S> = EdgeCoverageModuleBuilder<
    StdAddressFilter,
    I,
    StdPageFilter,
    S,
    EdgeCoverageFullVariant,
    false,
    false,
    0,
>;

impl<AF, PF, const IS_CONST_MAP: bool, const MAP_SIZE: usize>
    EdgeCoverageVariant<AF, PF, IS_CONST_MAP, MAP_SIZE> for EdgeCoverageFullVariant
{
    fn jit_hitcount<ET>(&mut self, emulator_modules: &mut EmulatorModules<ET>)
    where
        AF: AddressFilter,
        ET: EmulatorModuleTuple,
        ET::Input: 'static,
        ET::State: CoreState + 'static,
        PF: PageFilter,
    {
        let hook_id = emulator_modules.edges(
            Hook::Function(gen_unique_edge_ids::<AF, ET, PF, Self, IS_CONST_MAP, MAP_SIZE>),
            Hook::Empty,
        );
        unsafe {
            libaflmm_qemu_sys::libafl_qemu_edge_hook_set_jit(
                hook_id.0,
                Some(libaflmm_qemu_sys::libafl_jit_trace_edge_hitcount),
            );
        }
    }

    fn jit_no_hitcount<ET>(&mut self, emulator_modules: &mut EmulatorModules<ET>)
    where
        AF: AddressFilter,
        ET: EmulatorModuleTuple,
        ET::Input: 'static,
        ET::State: CoreState + 'static,
        PF: PageFilter,
    {
        let hook_id = emulator_modules.edges(
            Hook::Function(gen_unique_edge_ids::<AF, ET, PF, Self, IS_CONST_MAP, MAP_SIZE>),
            Hook::Empty,
        );
        unsafe {
            libaflmm_qemu_sys::libafl_qemu_edge_hook_set_jit(
                hook_id.0,
                Some(libaflmm_qemu_sys::libafl_jit_trace_edge_single),
            );
        }
    }

    fn fn_hitcount<ET>(&mut self, emulator_modules: &mut EmulatorModules<ET>)
    where
        AF: AddressFilter,
        ET: EmulatorModuleTuple,
        ET::Input: 'static,
        ET::State: CoreState + 'static,
        PF: PageFilter,
    {
        emulator_modules.edges(
            Hook::Function(gen_unique_edge_ids::<AF, ET, PF, Self, IS_CONST_MAP, MAP_SIZE>),
            Hook::Raw(trace_edge_hitcount),
        );
    }

    fn fn_no_hitcount<ET>(&mut self, emulator_modules: &mut EmulatorModules<ET>)
    where
        AF: AddressFilter,
        ET: EmulatorModuleTuple,
        ET::Input: 'static,
        ET::State: CoreState + 'static,
        PF: PageFilter,
    {
        emulator_modules.edges(
            Hook::Function(gen_unique_edge_ids::<AF, ET, PF, Self, IS_CONST_MAP, MAP_SIZE>),
            Hook::Raw(trace_edge_single),
        );
    }
}

impl<I, S> Default for StdEdgeCoverageFullModuleBuilder<I, S> {
    fn default() -> Self {
        Self {
            variant: EdgeCoverageFullVariant,
            address_filter: StdAddressFilter::default(),
            page_filter: StdPageFilter::default(),
            use_hitcounts: true,
            use_jit: true,
            phantom: PhantomData,
        }
    }
}

impl<I, S> StdEdgeCoverageFullModule<I, S> {
    #[must_use]
    pub fn builder() -> StdEdgeCoverageFullModuleBuilder<I, S> {
        EdgeCoverageModuleBuilder::default()
    }
}
