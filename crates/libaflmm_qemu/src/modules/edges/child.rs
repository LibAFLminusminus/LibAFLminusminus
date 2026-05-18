use std::marker::PhantomData;

use libaflmm::states::CoreState;

use super::{
    EdgeCoverageVariant,
    helpers::{gen_hashed_edge_ids, trace_edge_hitcount_ptr, trace_edge_single_ptr},
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
pub struct EdgeCoverageChildVariant;
pub type StdEdgeCoverageChildModule<I, S> =
    EdgeCoverageModule<StdAddressFilter, I, StdPageFilter, S, EdgeCoverageChildVariant, false, 0>;
pub type StdEdgeCoverageChildModuleBuilder<I, S> = EdgeCoverageModuleBuilder<
    StdAddressFilter,
    I,
    StdPageFilter,
    S,
    EdgeCoverageChildVariant,
    false,
    false,
    0,
>;

impl<AF, PF, const IS_CONST_MAP: bool, const MAP_SIZE: usize>
    EdgeCoverageVariant<AF, PF, IS_CONST_MAP, MAP_SIZE> for EdgeCoverageChildVariant
{
    const DO_SIDE_EFFECTS: bool = false;

    fn fn_hitcount<ET>(&mut self, emulator_modules: &mut EmulatorModules<ET>)
    where
        AF: AddressFilter,
        ET: EmulatorModuleTuple,
        ET::Input: 'static,
        ET::State: CoreState + 'static,
        PF: PageFilter,
    {
        emulator_modules.edges(
            Hook::Function(gen_hashed_edge_ids::<AF, ET, PF, Self, IS_CONST_MAP, MAP_SIZE>),
            Hook::Raw(trace_edge_hitcount_ptr),
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
            Hook::Function(gen_hashed_edge_ids::<AF, ET, PF, Self, IS_CONST_MAP, MAP_SIZE>),
            Hook::Raw(trace_edge_single_ptr),
        );
    }
}

impl<I, S> Default for StdEdgeCoverageChildModuleBuilder<I, S> {
    fn default() -> Self {
        Self {
            variant: EdgeCoverageChildVariant,
            address_filter: StdAddressFilter::default(),
            page_filter: StdPageFilter::default(),
            use_hitcounts: true,
            use_jit: true,
            phantom: PhantomData,
        }
    }
}

impl<I, S> StdEdgeCoverageChildModule<I, S> {
    #[must_use]
    pub fn builder() -> StdEdgeCoverageChildModuleBuilder<I, S> {
        EdgeCoverageModuleBuilder::default().jit(false)
    }
}
