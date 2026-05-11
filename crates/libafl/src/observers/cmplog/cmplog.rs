//! `CmpLog` logs and reports back values touched during fuzzing.
//! The values will then be used in subsequent mutations.

use core::fmt::Debug;
use libafl_bolts::{Named, ownedref::OwnedMutPtr};
use libafl_targets::{
    CMPLOG_KIND_INS, CMPLOG_KIND_RTN, CMPLOG_MAP_H, CMPLOG_MAP_RTN_H, CMPLOG_MAP_W, CMPLOG_RTN_LEN,
    CmpLogHeader, CmpLogMap, CmpLogVals, LibAFLCmpLogHeader, LibAFLCmpLogInstruction,
    LibAFLCmpLogRoutine, LibAFLCmpLogVals, Operand, Routine, StdCmpLogMap,
    cmps::libafl_cmplog_map_ptr, exports::CMPLOG_ENABLED,
};

use alloc::{borrow::Cow, string::ToString, vec::Vec};
use serde::{Deserialize, Serialize};

use core::ops::{Deref, DerefMut};

use libafl_core::{AsSlice, HasLen};

use crate::{
    DependencyResolver, Error,
    executors::ExitKind,
    observers::{CmpObserver, CmpValues, CmplogBytes, Observer},
    states::{FlatState, named_metadata, named_metadata_mut, named_metadata_or_insert_with},
};
/// A [`CmpObserver`] observer for `CmpLog`
#[derive(Debug)]
pub struct CmpLogObserver<H, V> {
    map: OwnedMutPtr<CmpLogMap<H, V>>,
    size: Option<OwnedMutPtr<usize>>,
    add_meta: bool,
    name: Cow<'static, str>,
}

impl<H, V> CmpObserver for CmpLogObserver<H, V>
where
    H: CmpLogHeader,
{
    type Map = CmpLogMap<H, V>;
    /// Get the number of usable cmps (all by default)
    fn usable_count(&self) -> usize {
        match &self.size {
            None => self.map.as_ref().len(),
            Some(o) => *o.as_ref(),
        }
    }

    fn cmp_map(&self) -> &Self::Map {
        self.map.as_ref()
    }

    fn cmp_map_mut(&mut self) -> &mut Self::Map {
        self.map.as_mut()
    }
}

impl<H, V> DependencyResolver for CmpLogObserver<H, V> {
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<(), Error> {
        registrator.register_md_default::<CmpLogMetadata>(self.name().to_string());
        Ok(())
    }
}

impl<H, S, V> Observer<S> for CmpLogObserver<H, V>
where
    H: CmpLogHeader,
    V: CmpLogVals,
    S: FlatState,
{
    fn pre_exec(&mut self, _state: &mut S) -> Result<(), Error> {
        self.map.as_mut().reset()?;
        Ok(())
    }

    fn post_exec(&mut self, state: &mut S, _exit_kind: &ExitKind) -> Result<(), Error> {
        if self.add_meta {
            let meta =
                named_metadata_mut::<CmpLogMetadata>(state.named_metadata_map_mut(), self.name())?;

            let usable_count = self.usable_count();

            meta.add_from(usable_count, self.cmp_map_mut());
        }

        Ok(())
    }
}

impl<H, V> Named for CmpLogObserver<H, V> {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<H, V> CmpLogObserver<H, V> {
    /// Creates a new [`CmpLogObserver`] with the given map and name.
    ///
    /// # Safety
    /// Will keep a ptr to the map. The map may not move in memory!
    #[must_use]
    pub unsafe fn with_map_ptr(
        name: &'static str,
        map: *mut CmpLogMap<H, V>,
        add_meta: bool,
    ) -> Self {
        Self {
            name: Cow::from(name),
            size: None,
            add_meta,
            map: OwnedMutPtr::Ptr(map),
        }
    }
}

impl CmpLogObserver<LibAFLCmpLogHeader, LibAFLCmpLogVals> {
    /// Creates a new [`CmpLogObserver`] with the given name from the default cmplog map
    #[must_use]
    pub fn new(name: &'static str, add_meta: bool) -> Self {
        unsafe { Self::with_map_ptr(name, libafl_cmplog_map_ptr, add_meta) }
    }
}

/// A state metadata holding a list of values logged from comparisons
#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(miri, expect(clippy::unsafe_derive_deserialize))] // for SerdeAny
pub struct CmpLogMetadata {
    /// A `list` of values.
    #[serde(skip)]
    pub list: Vec<CmpValues>,
}

libafl_bolts::impl_serdeany!(CmpLogMetadata);

impl Deref for CmpLogMetadata {
    type Target = [CmpValues];
    fn deref(&self) -> &[CmpValues] {
        &self.list
    }
}

impl DerefMut for CmpLogMetadata {
    fn deref_mut(&mut self) -> &mut [CmpValues] {
        &mut self.list
    }
}

pub fn parse_cmplog_map<H, V>(
    map: &mut CmpLogMap<H, V>,
    idx: usize,
    execution: usize,
) -> Option<CmpValues>
where
    H: CmpLogHeader,
    V: CmpLogVals,
{
    if map.headers[idx].kind() == CMPLOG_KIND_INS {
        let shape = map.headers[idx].shape();
        unsafe {
            match shape {
                0 => Some(CmpValues::U8((
                    map.vals.operands()[idx][execution].v0() as u8,
                    map.vals.operands()[idx][execution].v1() as u8,
                    map.vals.operands()[idx][execution].aux() == 1,
                ))),
                1 => Some(CmpValues::U16((
                    map.vals.operands()[idx][execution].v0() as u16,
                    map.vals.operands()[idx][execution].v1() as u16,
                    map.vals.operands()[idx][execution].aux() == 1,
                ))),
                3 => Some(CmpValues::U32((
                    map.vals.operands()[idx][execution].v0() as u32,
                    map.vals.operands()[idx][execution].v1() as u32,
                    map.vals.operands()[idx][execution].aux() == 1,
                ))),
                7 => Some(CmpValues::U64((
                    map.vals.operands()[idx][execution].v0(),
                    map.vals.operands()[idx][execution].v1(),
                    map.vals.operands()[idx][execution].aux() == 1,
                ))),
                // TODO handle 128 bits & 256 bits & 512 bits cmps
                15 | 31 | 63 => None,
                _ => panic!("Invalid CmpLog shape {shape}"),
            }
        }
    } else {
        unsafe {
            Some(CmpValues::Bytes((
                CmplogBytes::from_buf_and_len(
                    *map.vals.routines()[idx][execution].v0(),
                    CMPLOG_RTN_LEN as u8,
                ),
                CmplogBytes::from_buf_and_len(
                    *map.vals.routines()[idx][execution].v1(),
                    CMPLOG_RTN_LEN as u8,
                ),
            )))
        }
    }
}

impl CmpLogMetadata {
    /// Creates a new [`struct@CmpLogMetadata`]
    #[must_use]
    pub fn new() -> Self {
        Self { list: vec![] }
    }

    /// Add comparisons to a metadata from a `CmpObserver`. `cmp_map` is mutable in case
    /// it is needed for a custom map, but this is not utilized for `CmpObserver` or
    /// `AflppCmpLogObserver`.
    pub fn add_from<H, V>(&mut self, usable_count: usize, cmp_map: &mut CmpLogMap<H, V>)
    where
        H: CmpLogHeader,
        V: CmpLogVals,
    {
        self.list.clear();
        let count = usable_count;
        for i in 0..count {
            let execs = cmp_map.usable_executions_for(i);
            if execs > 0 {
                // Recongize loops and discard if needed
                if execs > 4 {
                    let mut increasing_v0 = 0;
                    let mut increasing_v1 = 0;
                    let mut decreasing_v0 = 0;
                    let mut decreasing_v1 = 0;

                    let mut last: Option<CmpValues> = None;
                    for j in 0..execs {
                        if let Some(val) = parse_cmplog_map(cmp_map, i, j) {
                            if let Some(l) = last.and_then(|x| x.to_u64_tuple())
                                && let Some(v) = val.to_u64_tuple()
                            {
                                if l.0.wrapping_add(1) == v.0 {
                                    increasing_v0 += 1;
                                }
                                if l.1.wrapping_add(1) == v.1 {
                                    increasing_v1 += 1;
                                }
                                if l.0.wrapping_sub(1) == v.0 {
                                    decreasing_v0 += 1;
                                }
                                if l.1.wrapping_sub(1) == v.1 {
                                    decreasing_v1 += 1;
                                }
                            }
                            last = Some(val);
                        }
                    }
                    // We check for execs-2 because the logged execs may wrap and have something like
                    // 8 9 10 3 4 5 6 7
                    if increasing_v0 >= execs - 2
                        || increasing_v1 >= execs - 2
                        || decreasing_v0 >= execs - 2
                        || decreasing_v1 >= execs - 2
                    {
                        continue;
                    }
                }
                for j in 0..execs {
                    if let Some(val) = parse_cmplog_map(cmp_map, i, j) {
                        self.list.push(val);
                    }
                }
            }
        }
    }
}
