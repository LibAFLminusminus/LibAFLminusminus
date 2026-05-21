//! This modules are for all observers that observe the cmplog values

use libaflmm_core::{AsSlice, HasLen};
use serde::{Deserialize, Serialize};

pub mod observer;
pub use observer::{CmpLogObserver, StdCmpLogObserver};

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
