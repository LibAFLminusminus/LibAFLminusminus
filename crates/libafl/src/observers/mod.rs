//! Observers give insights about runs of a target, such as coverage, timing, stack depth, and more.
use alloc::borrow::Cow;

pub mod cmp;
pub use cmp::*;

#[cfg(feature = "std")]
pub mod stdio;
#[cfg(feature = "std")]
pub use stdio::{StdErrObserver, StdOutObserver};

#[cfg(feature = "regex")]
pub mod stacktrace;
#[cfg(feature = "regex")]
pub use stacktrace::*;

pub mod map;
pub use map::*;

pub mod value;

/// List observer
pub mod list;
use core::{fmt::Debug, time::Duration};
#[cfg(feature = "std")]
use std::time::Instant;

#[cfg(not(feature = "std"))]
use libafl_bolts::current_time;
use libafl_bolts::{Named, tuples::MatchName};
pub use list::*;
use serde::{Deserialize, Serialize};
pub use value::*;

use crate::{Error, executors::ExitKind};

/// Observers observe different information about the target.
/// They can then be used by various sorts of feedback.
pub trait Observer<S>: Named {
    /// Called right before execution starts.
    #[inline]
    fn pre_exec(&mut self, _state: &mut S) -> Result<(), Error> {
        Ok(())
    }

    /// Called right after execution finishes.
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _exit_kind: &ExitKind) -> Result<(), Error> {
        Ok(())
    }
}

/// A haskell-style tuple of observers
pub trait ObserversTuple<S>: MatchName {
    /// This is called right before the next execution.
    fn pre_exec_all(&mut self, state: &mut S) -> Result<(), Error>;

    /// This is called right after the last execution
    fn post_exec_all(&mut self, state: &mut S, exit_kind: &ExitKind) -> Result<(), Error>;
}

impl<S> ObserversTuple<S> for () {
    fn pre_exec_all(&mut self, _state: &mut S) -> Result<(), Error> {
        Ok(())
    }

    fn post_exec_all(&mut self, _state: &mut S, _exit_kind: &ExitKind) -> Result<(), Error> {
        Ok(())
    }
}

impl<Head, Tail, S> ObserversTuple<S> for (Head, Tail)
where
    Head: Observer<S>,
    Tail: ObserversTuple<S>,
{
    fn pre_exec_all(&mut self, state: &mut S) -> Result<(), Error> {
        self.0.pre_exec(state)?;
        self.1.pre_exec_all(state)
    }

    fn post_exec_all(&mut self, state: &mut S, exit_kind: &ExitKind) -> Result<(), Error> {
        self.0.post_exec(state, exit_kind)?;
        self.1.post_exec_all(state, exit_kind)
    }
}

/// A trait for [`Observer`]`s` with a hash field
pub trait ObserverWithHashField {
    /// get the value of the hash field
    fn hash(&self) -> Option<u64>;
}

/// A simple observer, just overlooking the runtime of the target.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TimeObserver {
    name: Cow<'static, str>,

    #[cfg(feature = "std")]
    #[serde(with = "instant_serializer")]
    start_time: Instant,

    #[cfg(not(feature = "std"))]
    start_time: Duration,

    last_runtime: Option<Duration>,
}

#[cfg(feature = "std")]
mod instant_serializer {
    use core::time::Duration;
    use std::time::Instant;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(instant: &Instant, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = instant.elapsed();
        duration.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Instant, D::Error>
    where
        D: Deserializer<'de>,
    {
        let duration = Duration::deserialize(deserializer)?;
        let now = Instant::now();
        let instant = now.checked_sub(duration).unwrap_or(now);
        Ok(instant)
    }
}

impl TimeObserver {
    /// Creates a new [`TimeObserver`] with the given name.
    #[must_use]
    pub fn new<S>(name: S) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        Self {
            name: name.into(),

            #[cfg(feature = "std")]
            start_time: Instant::now(),

            #[cfg(not(feature = "std"))]
            start_time: Duration::from_secs(0),

            last_runtime: None,
        }
    }

    /// Gets the runtime for the last execution of this target.
    #[must_use]
    pub fn last_runtime(&self) -> &Option<Duration> {
        &self.last_runtime
    }
}

impl<S> Observer<S> for TimeObserver {
    #[cfg(feature = "std")]
    fn pre_exec(&mut self, _state: &mut S) -> Result<(), Error> {
        self.last_runtime = None;
        self.start_time = Instant::now();
        Ok(())
    }

    #[cfg(not(feature = "std"))]
    fn pre_exec(&mut self, _state: &mut S) -> Result<(), Error> {
        self.last_runtime = None;
        self.start_time = current_time();
        Ok(())
    }

    #[cfg(feature = "std")]
    fn post_exec(&mut self, _state: &mut S, _exit_kind: &ExitKind) -> Result<(), Error> {
        self.last_runtime = Some(self.start_time.elapsed());
        Ok(())
    }

    #[cfg(not(feature = "std"))]
    fn post_exec(&mut self, _state: &mut S, _exit_kind: &ExitKind) -> Result<(), Error> {
        self.last_runtime = Some(current_time().saturating_sub(self.start_time));
        Ok(())
    }
}

impl Named for TimeObserver {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

#[cfg(feature = "std")]
#[cfg(test)]
#[cfg(not(feature = "remove_me"))]
mod tests {

    use libafl_bolts::{
        Named,
        ownedref::OwnedMutSlice,
        tuples::{tuple_list, tuple_list_type},
    };

    use crate::observers::{StdMapObserver, TimeObserver};

    static mut MAP: [u32; 4] = [0; 4];

    #[test]
    fn test_observer_serde() {
        let map_ptr = &raw const MAP;
        let obv = tuple_list!(TimeObserver::new("time"), unsafe {
            let len = (*map_ptr).len();
            StdMapObserver::from_ownedref(
                "map",
                OwnedMutSlice::from_raw_parts_mut(&raw mut MAP as *mut u32, len),
            )
        });
        let vec = postcard::to_allocvec(&obv).unwrap();
        log::info!("{vec:?}");
        let obv2: tuple_list_type!(TimeObserver, StdMapObserver<u32, false>) =
            postcard::from_bytes(&vec).unwrap();
        assert_eq!(obv.0.name(), obv2.0.name());
    }
}
