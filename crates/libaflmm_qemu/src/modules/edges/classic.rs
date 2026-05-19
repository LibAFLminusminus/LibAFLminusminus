use super::{
    EdgeCoverageVariant,
    helpers::{
        gen_hashed_block_ids, trace_block_transition_hitcount, trace_block_transition_single,
    },
};
use crate::{
    EmulatorModules, Hook,
    modules::{
        AddressFilter, EdgeCoverageModule, EdgeCoverageModuleBuilder, EmulatorModuleTuple,
        PageFilter,
        utils::filters::{StdAddressFilter, StdPageFilter},
    },
};
use libaflmm::states::State;
use std::marker::PhantomData;

#[derive(Debug)]
pub struct EdgeCoverageClassicVariant;

pub type StdEdgeCoverageClassicModule<I, S> =
    EdgeCoverageModule<StdAddressFilter, I, StdPageFilter, S, EdgeCoverageClassicVariant, false, 0>;
pub type StdEdgeCoverageClassicModuleBuilder<I, S> = EdgeCoverageModuleBuilder<
    StdAddressFilter,
    I,
    StdPageFilter,
    S,
    EdgeCoverageClassicVariant,
    false,
    false,
    0,
>;

impl<AF, PF, const IS_CONST_MAP: bool, const MAP_SIZE: usize>
    EdgeCoverageVariant<AF, PF, IS_CONST_MAP, MAP_SIZE> for EdgeCoverageClassicVariant
{
    const DO_SIDE_EFFECTS: bool = false;

    fn jit_hitcount<ET>(&mut self, emulator_modules: &mut EmulatorModules<ET>)
    where
        AF: AddressFilter,
        ET: EmulatorModuleTuple,
        ET::Input: 'static,
        ET::State: State + 'static,
        PF: PageFilter,
    {
        let hook_id = emulator_modules.blocks(
            Hook::Function(gen_hashed_block_ids::<AF, ET, PF, Self, IS_CONST_MAP, MAP_SIZE>),
            Hook::Empty,
            Hook::Empty,
        );

        unsafe {
            libaflmm_qemu_sys::libafl_qemu_block_hook_set_jit(
                hook_id.0,
                Some(libaflmm_qemu_sys::libafl_jit_trace_block_hitcount),
            );
        }
    }

    fn jit_no_hitcount<ET>(&mut self, emulator_modules: &mut EmulatorModules<ET>)
    where
        AF: AddressFilter,
        ET: EmulatorModuleTuple,
        ET::Input: 'static,
        ET::State: State + 'static,
        PF: PageFilter,
    {
        let hook_id = emulator_modules.blocks(
            Hook::Function(gen_hashed_block_ids::<AF, ET, PF, Self, IS_CONST_MAP, MAP_SIZE>),
            Hook::Empty,
            Hook::Empty,
        );

        unsafe {
            libaflmm_qemu_sys::libafl_qemu_block_hook_set_jit(
                hook_id.0,
                Some(libaflmm_qemu_sys::libafl_jit_trace_block_single),
            );
        }
    }

    fn fn_hitcount<ET>(&mut self, emulator_modules: &mut EmulatorModules<ET>)
    where
        AF: AddressFilter,
        ET: EmulatorModuleTuple,
        ET::Input: 'static,
        ET::State: State + 'static,
        PF: PageFilter,
    {
        emulator_modules.blocks(
            Hook::Function(gen_hashed_block_ids::<AF, ET, PF, Self, IS_CONST_MAP, MAP_SIZE>),
            Hook::Empty,
            Hook::Raw(trace_block_transition_hitcount),
        );
    }

    fn fn_no_hitcount<ET>(&mut self, emulator_modules: &mut EmulatorModules<ET>)
    where
        AF: AddressFilter,
        ET: EmulatorModuleTuple,
        ET::Input: 'static,
        ET::State: State + 'static,
        PF: PageFilter,
    {
        emulator_modules.blocks(
            Hook::Function(gen_hashed_block_ids::<AF, ET, PF, Self, IS_CONST_MAP, MAP_SIZE>),
            Hook::Empty,
            Hook::Raw(trace_block_transition_single),
        );
    }
}

impl<I, S> Default for StdEdgeCoverageClassicModuleBuilder<I, S> {
    fn default() -> Self {
        Self {
            variant: EdgeCoverageClassicVariant,
            address_filter: StdAddressFilter::default(),
            page_filter: StdPageFilter::default(),
            use_hitcounts: true,
            use_jit: true,
            phantom: PhantomData,
        }
    }
}

impl<I, S> StdEdgeCoverageClassicModule<I, S> {
    #[must_use]
    pub fn builder() -> StdEdgeCoverageClassicModuleBuilder<I, S> {
        EdgeCoverageModuleBuilder::default()
    }
}
