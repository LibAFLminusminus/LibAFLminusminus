//! The `CmpObserver` provides access to the logged values of CMP instructions
use alloc::{borrow::Cow, vec::Vec};
use core::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};
use std::string::ToString;

use arbitrary_int::{u1, u4, u5, u6};
use bitbybit::bitfield;
use hashbrown::HashMap;
use libafl_bolts::{AsSlice, HasLen, Named, ownedref::OwnedRefMut};
use serde::{Deserialize, Serialize};

use crate::{
    DependencyResolver, Error,
    executors::ExitKind,
    observers::Observer,
    states::{FlatState, named_metadata_mut},
};

/// A [`CmpObserver`] observes the traced comparisons during the current execution using a [`CmpMap`]
pub trait CmpObserver {
    /// The underlying map
    type Map;
    /// Get the number of usable cmps (all by default)
    fn usable_count(&self) -> usize;

    /// Get the `CmpMap`
    fn cmp_map(&self) -> &Self::Map;

    /// Get the mut `CmpMap`
    fn cmp_map_mut(&mut self) -> &mut Self::Map;
}
