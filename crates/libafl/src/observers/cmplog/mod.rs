pub mod cmplog;
pub use cmplog::*;

use crate::observers::Observer;
use alloc::vec::Vec;
use libafl_core::{AsSlice, HasLen};
use libafl_targets::{CMPLOG_KIND_INS, CMPLOG_KIND_RTN, CMPLOG_RTN_LEN, CmpLogMap};
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};

/// A bytes string for cmplog with up to 32 elements.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct CmplogBytes {
    buf: [u8; 32],
    len: u8,
}

impl CmplogBytes {
    /// Creates a new [`CmplogBytes`] object from the provided buf and length.
    /// Lengths above 32 are illegal but will be ignored.
    #[must_use]
    pub fn from_buf_and_len(buf: [u8; 32], len: u8) -> Self {
        debug_assert!(len <= 32, "Len too big: {len}, max: 32");
        CmplogBytes { buf, len }
    }
}

impl<'a> AsSlice<'a> for CmplogBytes {
    type Entry = u8;

    type SliceRef = &'a [u8];

    fn as_slice(&'a self) -> Self::SliceRef {
        &self.buf[0..(self.len as usize)]
    }
}

impl HasLen for CmplogBytes {
    fn len(&self) -> usize {
        self.len as usize
    }
}

/// Compare values collected during a run
#[derive(Eq, PartialEq, Debug, Serialize, Deserialize, Clone)]
pub enum CmpValues {
    /// (side 1 of comparison, side 2 of comparison, side 1 value is const)
    U8((u8, u8, bool)),
    /// (side 1 of comparison, side 2 of comparison, side 1 value is const)
    U16((u16, u16, bool)),
    /// (side 1 of comparison, side 2 of comparison, side 1 value is const)
    U32((u32, u32, bool)),
    /// (side 1 of comparison, side 2 of comparison, side 1 value is const)
    U64((u64, u64, bool)),
    /// Two vecs of u8 values/byte
    Bytes((CmplogBytes, CmplogBytes)),
}

impl CmpValues {
    /// Returns if the values are numericals
    #[must_use]
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            CmpValues::U8(_) | CmpValues::U16(_) | CmpValues::U32(_) | CmpValues::U64(_)
        )
    }

    /// Converts the value to a u64 tuple
    #[must_use]
    pub fn to_u64_tuple(&self) -> Option<(u64, u64, bool)> {
        match self {
            CmpValues::U8(t) => Some((u64::from(t.0), u64::from(t.1), t.2)),
            CmpValues::U16(t) => Some((u64::from(t.0), u64::from(t.1), t.2)),
            CmpValues::U32(t) => Some((u64::from(t.0), u64::from(t.1), t.2)),
            CmpValues::U64(t) => Some(*t),
            CmpValues::Bytes(_) => None,
        }
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

pub fn parse_cmplog_map(map: &mut CmpLogMap, idx: usize, execution: usize) -> Option<CmpValues> {
    if map.headers[idx].kind == CMPLOG_KIND_INS {
        let shape = map.headers[idx].shape;
        unsafe {
            match shape {
                0 => Some(CmpValues::U8((
                    map.vals.operands[idx][execution].0 as u8,
                    map.vals.operands[idx][execution].1 as u8,
                    map.vals.operands[idx][execution].2 == 1,
                ))),
                1 => Some(CmpValues::U16((
                    map.vals.operands[idx][execution].0 as u16,
                    map.vals.operands[idx][execution].1 as u16,
                    map.vals.operands[idx][execution].2 == 1,
                ))),
                3 => Some(CmpValues::U32((
                    map.vals.operands[idx][execution].0 as u32,
                    map.vals.operands[idx][execution].1 as u32,
                    map.vals.operands[idx][execution].2 == 1,
                ))),
                7 => Some(CmpValues::U64((
                    map.vals.operands[idx][execution].0,
                    map.vals.operands[idx][execution].1,
                    map.vals.operands[idx][execution].2 == 1,
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
                    map.vals.routines[idx][execution].0,
                    CMPLOG_RTN_LEN as u8,
                ),
                CmplogBytes::from_buf_and_len(
                    map.vals.routines[idx][execution].1,
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
    pub fn add_from(&mut self, usable_count: usize, cmp_map: &mut CmpLogMap) {
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
