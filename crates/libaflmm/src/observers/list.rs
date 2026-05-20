//! The observer for observing the value in a [`Vec`]
use alloc::{borrow::Cow, vec::Vec};
use core::fmt::Debug;

use libaflmm_bolts::{Error, Named, ownedref::OwnedMutPtr};
use serde::{Deserialize, Serialize};

use crate::{common::DependencyResolver, observers::Observer};

/// A simple observer with a list of things.
#[derive(Serialize, Deserialize, Debug)]
#[serde(bound = "T: Serialize + for<'a> Deserialize<'a>")]
pub struct ListObserver<T> {
    name: Cow<'static, str>,
    /// The list
    list: OwnedMutPtr<Vec<T>>,
}

impl<T> ListObserver<T> {
    /// Creates a new [`struct@ListObserver`] with the given name.
    ///
    /// # Safety
    /// Will dereference the list.
    /// The list may not move in memory.
    #[must_use]
    pub fn new(name: &'static str, list: OwnedMutPtr<Vec<T>>) -> Self {
        Self {
            name: Cow::from(name),
            list,
        }
    }

    /// Get ref to the inner [`Self::list`]
    #[must_use]
    pub fn list(&self) -> &Vec<T> {
        self.list.as_ref()
    }

    /// Get mut ref to the inner [`Self::list`]
    #[must_use]
    pub fn list_mut(&mut self) -> &mut Vec<T> {
        self.list.as_mut()
    }
}

impl<T> DependencyResolver for ListObserver<T> {}

impl<S, T> Observer<S> for ListObserver<T> {
    fn pre_exec(&mut self, _state: &mut S) -> Result<(), Error> {
        self.list.as_mut().clear();
        Ok(())
    }
}

impl<T> Named for ListObserver<T> {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}
