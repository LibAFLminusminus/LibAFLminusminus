//! `CmpLog` logs and reports back values touched during fuzzing.
//! The values will then be used in subsequent mutations.

use alloc::{borrow::Cow, string::ToString};
use core::fmt::Debug;

use crate::{
    DependencyResolver, Error,
    executors::ExitKind,
    observers::{
        CmpMap, CmpObserver, CmplogBytes, Observer,
        cmp::{CmpValues, CmpValuesMetadata},
    },
    states::{FlatState, named_metadata, named_metadata_mut, named_metadata_or_insert_with},
};
use libafl_bolts::{Named, ownedref::OwnedMutPtr};

use libafl_targets::cmps::libafl_cmplog_map_ptr;
use libafl_targets::{
    CMPLOG_KIND_INS, CMPLOG_KIND_RTN, CMPLOG_MAP_H, CMPLOG_MAP_RTN_H, CMPLOG_MAP_W, CMPLOG_RTN_LEN,
    CmpLogHeader, CmpLogMap, exports::CMPLOG_ENABLED,
};
/// A [`CmpObserver`] observer for `CmpLog`
#[derive(Debug)]
pub struct CmpLogObserver {
    map: OwnedMutPtr<CmpLogMap>,
    size: Option<OwnedMutPtr<usize>>,
    add_meta: bool,
    name: Cow<'static, str>,
}

// Is the only difference here between this and StdCmpObserver that CMPLOG_ENABLED = 1??
impl CmpObserver for CmpLogObserver {
    type Map = CmpLogMap;
    /// Get the number of usable cmps (all by default)
    fn usable_count(&self) -> usize {
        match &self.size {
            None => self.map.as_ref().len(),
            Some(o) => *o.as_ref(),
        }
    }

    fn cmp_map(&self) -> &CmpLogMap {
        self.map.as_ref()
    }

    fn cmp_map_mut(&mut self) -> &mut CmpLogMap {
        self.map.as_mut()
    }
}

impl DependencyResolver for CmpLogObserver {
    fn register(&mut self, registrator: &mut crate::Registrator) -> Result<(), Error> {
        registrator.register_md_default::<CmpValuesMetadata>(self.name().to_string());
        Ok(())
    }
}

impl<S> Observer<S> for CmpLogObserver
where
    S: FlatState,
{
    fn pre_exec(&mut self, _state: &mut S) -> Result<(), Error> {
        self.map.as_mut().reset()?;
        Ok(())
    }

    fn post_exec(&mut self, state: &mut S, _exit_kind: &ExitKind) -> Result<(), Error> {
        if self.add_meta {
            let meta = named_metadata_mut::<CmpValuesMetadata>(
                state.named_metadata_map_mut(),
                self.name(),
            )?;

            let usable_count = self.usable_count();

            meta.add_from(usable_count, self.cmp_map_mut());
        }

        Ok(())
    }
}

impl Named for CmpLogObserver {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl CmpLogObserver {
    /// Creates a new [`CmpLogObserver`] with the given map and name.
    ///
    /// # Safety
    /// Will keep a ptr to the map. The map may not move in memory!
    #[must_use]
    pub unsafe fn with_map_ptr(name: &'static str, map: *mut CmpLogMap, add_meta: bool) -> Self {
        Self {
            name: Cow::from(name),
            size: None,
            add_meta,
            map: OwnedMutPtr::Ptr(map),
        }
    }

    /// Creates a new [`CmpLogObserver`] with the given name from the default cmplog map
    #[must_use]
    pub fn new(name: &'static str, add_meta: bool) -> Self {
        unsafe { Self::with_map_ptr(name, libafl_cmplog_map_ptr, add_meta) }
    }

    // TODO with_size
}

impl CmpMap for CmpLogMap {
    fn len(&self) -> usize {
        CMPLOG_MAP_W
    }

    fn executions_for(&self, idx: usize) -> usize {
        self.headers[idx].hits as usize
    }

    fn usable_executions_for(&self, idx: usize) -> usize {
        if self.headers[idx].kind == CMPLOG_KIND_INS {
            if self.executions_for(idx) < CMPLOG_MAP_H {
                self.executions_for(idx)
            } else {
                CMPLOG_MAP_H
            }
        } else if self.executions_for(idx) < CMPLOG_MAP_RTN_H {
            self.executions_for(idx)
        } else {
            CMPLOG_MAP_RTN_H
        }
    }

    fn values_of(&self, idx: usize, execution: usize) -> Option<CmpValues> {
        if self.headers[idx].kind == CMPLOG_KIND_INS {
            let shape = self.headers[idx].shape;
            unsafe {
                match shape {
                    0 => Some(CmpValues::U8((
                        self.vals.operands[idx][execution].0 as u8,
                        self.vals.operands[idx][execution].1 as u8,
                        self.vals.operands[idx][execution].2 == 1,
                    ))),
                    1 => Some(CmpValues::U16((
                        self.vals.operands[idx][execution].0 as u16,
                        self.vals.operands[idx][execution].1 as u16,
                        self.vals.operands[idx][execution].2 == 1,
                    ))),
                    3 => Some(CmpValues::U32((
                        self.vals.operands[idx][execution].0 as u32,
                        self.vals.operands[idx][execution].1 as u32,
                        self.vals.operands[idx][execution].2 == 1,
                    ))),
                    7 => Some(CmpValues::U64((
                        self.vals.operands[idx][execution].0,
                        self.vals.operands[idx][execution].1,
                        self.vals.operands[idx][execution].2 == 1,
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
                        self.vals.routines[idx][execution].0,
                        CMPLOG_RTN_LEN as u8,
                    ),
                    CmplogBytes::from_buf_and_len(
                        self.vals.routines[idx][execution].1,
                        CMPLOG_RTN_LEN as u8,
                    ),
                )))
            }
        }
    }

    fn reset(&mut self) -> Result<(), Error> {
        // For performance, we reset just the headers
        self.headers.fill(CmpLogHeader {
            hits: 0,
            shape: 0,
            kind: 0,
        });

        Ok(())
    }
}
