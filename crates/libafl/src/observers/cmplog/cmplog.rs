//! `CmpLog` logs and reports back values touched during fuzzing.
//! The values will then be used in subsequent mutations.

use alloc::{borrow::Cow, string::ToString};
use core::fmt::Debug;

use crate::{
    DependencyResolver, Error,
    executors::ExitKind,
    observers::{CmpLogMetadata, CmpObserver, Observer},
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
        registrator.register_md_default::<CmpLogMetadata>(self.name().to_string());
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
            let meta =
                named_metadata_mut::<CmpLogMetadata>(state.named_metadata_map_mut(), self.name())?;

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
