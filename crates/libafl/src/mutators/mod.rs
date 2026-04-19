//! [`Mutator`]`s` mutate input during fuzzing.
//!
//! These can be used standalone or in combination with other mutators to explore the input space more effectively.
//! You can read more about mutators in the [LibAFL book](https://aflplus.plus/libafl-book/core_concepts/mutator.html)
use crate::{Error, corpus::TestcaseId};
use alloc::{borrow::Cow, boxed::Box, vec::Vec};
use core::fmt;
use libafl_bolts::{HasLen, Named, rands::Rand, tuples::IntoVec};
use serde::{Deserialize, Serialize};
use tuple_list::NonEmptyTuple;

pub mod scheduled;
pub use scheduled::*;
pub mod mutations;
pub use mutations::*;
#[cfg(not(feature = "remove_me"))]
pub mod token_mutations;
#[cfg(not(feature = "remove_me"))]
pub use token_mutations::*;
pub mod havoc_mutations;
pub use havoc_mutations::*;
pub mod numeric;
pub use numeric::{int_mutators, mapped_int_mutators};
pub mod encoded_mutations;
pub use encoded_mutations::*;
pub mod mapping;
pub use mapping::*;

#[cfg(feature = "std")]
pub mod hash;
#[cfg(feature = "std")]
pub use hash::*;

#[cfg(feature = "unicode")]
pub mod unicode;
#[cfg(feature = "unicode")]
pub use unicode::*;
#[cfg(feature = "nautilus")]
pub mod nautilus;
#[cfg(feature = "nautilus")]
pub use nautilus::*;

// TODO mutator stats method that produces something that can be sent with the NewTestcase event
// We can use it to report which mutations generated the testcase in the broker logs

/// The index of a mutation in the mutations tuple
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(transparent)]
pub struct MutationId(pub(crate) usize);

impl fmt::Display for MutationId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MutationId({})", self.0)
    }
}

impl From<usize> for MutationId {
    fn from(value: usize) -> Self {
        MutationId(value)
    }
}

impl From<u64> for MutationId {
    fn from(value: u64) -> Self {
        MutationId(value as usize)
    }
}

impl From<i32> for MutationId {
    #[expect(clippy::cast_sign_loss)]
    fn from(value: i32) -> Self {
        debug_assert!(value >= 0);
        MutationId(value as usize)
    }
}

/// Result of the mutation.
///
/// [`MutationResult::Mutated`] does not necessarily mean that the input changed,
/// just that the mutator did something. For slow targets, consider using
/// a fuzzer with a input filter
/// or wrapping your mutator in a [`hash::MutationChecker`].
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MutationResult {
    /// The [`Mutator`] executed on this `Input`. It may not guarantee that the input has actually been changed.
    Mutated,
    /// The [`Mutator`] did not mutate this `Input`. It was `Skipped`.
    Skipped,
}

/// A [`Mutator`] takes an input, and mutates it.
/// Simple as that.
pub trait Mutator<I, R, S>: Named {
    /// Mutate a given input
    fn mutate(&mut self, input: &mut I, rand: &mut R, state: &S) -> Result<MutationResult, Error>;

    /// Post-process given the outcome of the execution
    /// `new_testcase_id` will be `Some` if a new [`crate::corpus::Testcase`] was created this execution.
    fn post_exec(
        &mut self,
        _state: &mut S,
        _new_testcase_id: Option<TestcaseId>,
    ) -> Result<(), Error>;
}

/// A mutator that takes input, and returns a vector of mutated inputs.
/// Simple as that.
pub trait MultiMutator<I, R: Rand, S>: Named {
    /// Mutate a given input up to `max_count` times,
    /// or as many times as appropriate, if no `max_count` is given
    fn multi_mutate(
        &mut self,
        input: &I,
        rand: &mut R,
        state: &mut S,
        max_count: Option<usize>,
    ) -> Result<Vec<I>, Error>;

    /// Post-process given the outcome of the execution
    /// `new_testcase_id` will be `Some` if a new `Testcase` was created this execution.
    #[inline]
    fn multi_post_exec(
        &mut self,
        _state: &mut S,
        _new_testcase_id: Option<TestcaseId>,
    ) -> Result<(), Error> {
        Ok(())
    }
}

/// A `Tuple` of [`Mutator`]`s` that can execute multiple `Mutators` in a row.
pub trait MutatorsTuple<I, R, S>: HasLen {
    /// Runs the [`Mutator::mutate`] function on all [`Mutator`]`s` in this `Tuple`.
    fn mutate_all(&mut self, input: &mut I, rand: &mut R, state: &mut S) -> Result<MutationResult, Error>;

    /// Runs the [`Mutator::post_exec`] function on all [`Mutator`]`s` in this `Tuple`.
    /// `new_testcase_id` will be `Some` if a new `Testcase` was created this execution.
    fn post_exec_all(
        &mut self,
        state: &mut S,
        new_testcase_id: Option<TestcaseId>,
    ) -> Result<(), Error>;

    /// Gets the [`Mutator`] at the given index and runs the `mutate` function on it.
    fn get_and_mutate(
        &mut self,
        index: MutationId,
        input: &mut I,
        rand: &mut R,
        state: &S,
    ) -> Result<MutationResult, Error>;

    /// Gets the [`Mutator`] at the given index and runs the `post_exec` function on it.
    /// `new_testcase_id` will be `Some` if a new `Testcase` was created this execution.
    fn get_and_post_exec(
        &mut self,
        index: usize,
        state: &mut S,
        testcase_id: Option<TestcaseId>,
    ) -> Result<(), Error>;
}

impl<I, R: Rand, S> MutatorsTuple<I, R, S> for () {
    #[inline]
    fn mutate_all(&mut self, _input: &mut I, _rand: &mut R, _state: &mut S) -> Result<MutationResult, Error> {
        Ok(MutationResult::Skipped)
    }

    #[inline]
    fn post_exec_all(
        &mut self,
        _state: &mut S,
        _new_testcase_id: Option<TestcaseId>,
    ) -> Result<(), Error> {
        Ok(())
    }

    #[inline]
    fn get_and_mutate(
        &mut self,
        _index: MutationId,
        _input: &mut I,
        _rand: &mut R,
        _state: &S,
    ) -> Result<MutationResult, Error> {
        Ok(MutationResult::Skipped)
    }

    #[inline]
    fn get_and_post_exec(
        &mut self,
        _index: usize,
        _state: &mut S,
        _new_testcase_id: Option<TestcaseId>,
    ) -> Result<(), Error> {
        Ok(())
    }
}

impl<Head, Tail, I, R: Rand, S> MutatorsTuple<I, R, S> for (Head, Tail)
where
    Head: Mutator<I, R, S>,
    Tail: MutatorsTuple<I, R, S>,
{
    fn mutate_all(&mut self, input: &mut I, rand: &mut R, state: &mut S) -> Result<MutationResult, Error> {
        let r = self.0.mutate(input, rand, state)?;
        if self.1.mutate_all(input, rand, state)? == MutationResult::Mutated {
            Ok(MutationResult::Mutated)
        } else {
            Ok(r)
        }
    }

    fn post_exec_all(
        &mut self,
        state: &mut S,
        new_testcase_id: Option<TestcaseId>,
    ) -> Result<(), Error> {
        self.0.post_exec(state, new_testcase_id)?;
        self.1.post_exec_all(state, new_testcase_id)
    }

    fn get_and_mutate(
        &mut self,
        index: MutationId,
        input: &mut I,
        rand: &mut R,
        state: &S,
    ) -> Result<MutationResult, Error> {
        if index.0 == 0 {
            self.0.mutate(input, rand, state)
        } else {
            self.1.get_and_mutate((index.0 - 1).into(), input, rand, state)
        }
    }

    fn get_and_post_exec(
        &mut self,
        index: usize,
        state: &mut S,
        new_testcase_id: Option<TestcaseId>,
    ) -> Result<(), Error> {
        if index == 0 {
            self.0.post_exec(state, new_testcase_id)
        } else {
            self.1.get_and_post_exec(index - 1, state, new_testcase_id)
        }
    }
}

impl<Head, Tail, I, R: Rand, S> IntoVec<Box<dyn Mutator<I, R, S>>> for (Head, Tail)
where
    Head: Mutator<I, R, S> + 'static,
    Tail: IntoVec<Box<dyn Mutator<I, R, S>>>,
{
    fn into_vec_reversed(self) -> Vec<Box<dyn Mutator<I, R, S>>> {
        let (head, tail) = self.uncons();
        let mut ret = tail.into_vec_reversed();
        ret.push(Box::new(head));
        ret
    }

    fn into_vec(self) -> Vec<Box<dyn Mutator<I, R, S>>> {
        let mut ret = self.into_vec_reversed();
        ret.reverse();
        ret
    }
}

impl<Tail, I, R: Rand, S> MutatorsTuple<I, R, S> for (Tail,)
where
    Tail: MutatorsTuple<I, R, S>,
{
    fn mutate_all(&mut self, input: &mut I, rand: &mut R, state: &mut S) -> Result<MutationResult, Error> {
        self.0.mutate_all(input, rand, state)
    }

    fn post_exec_all(
        &mut self,
        state: &mut S,
        new_testcase_id: Option<TestcaseId>,
    ) -> Result<(), Error> {
        self.0.post_exec_all(state, new_testcase_id)
    }

    fn get_and_mutate(
        &mut self,
        index: MutationId,
        input: &mut I,
        rand: &mut R,
        state: &S,
    ) -> Result<MutationResult, Error> {
        self.0.get_and_mutate(index, input, rand, state)
    }

    fn get_and_post_exec(
        &mut self,
        index: usize,
        state: &mut S,
        new_testcase_id: Option<TestcaseId>,
    ) -> Result<(), Error> {
        self.0.get_and_post_exec(index, state, new_testcase_id)
    }
}

impl<Tail, I, R: Rand, S> IntoVec<Box<dyn Mutator<I, R, S>>> for (Tail,)
where
    Tail: IntoVec<Box<dyn Mutator<I, R, S>>>,
{
    fn into_vec(self) -> Vec<Box<dyn Mutator<I, R, S>>> {
        self.0.into_vec()
    }
}

impl<I, R: Rand, S> MutatorsTuple<I, R, S> for Vec<Box<dyn Mutator<I, R, S>>> {
    fn mutate_all(&mut self, input: &mut I, rand: &mut R, state: &mut S) -> Result<MutationResult, Error> {
        self.iter_mut()
            .try_fold(MutationResult::Skipped, |ret, mutator| {
                if mutator.mutate(input, rand, state)? == MutationResult::Mutated {
                    Ok(MutationResult::Mutated)
                } else {
                    Ok(ret)
                }
            })
    }

    fn post_exec_all(
        &mut self,
        state: &mut S,
        new_testcase_id: Option<TestcaseId>,
    ) -> Result<(), Error> {
        for mutator in self.iter_mut() {
            mutator.post_exec(state, new_testcase_id)?;
        }
        Ok(())
    }

    fn get_and_mutate(
        &mut self,
        index: MutationId,
        input: &mut I,
        rand: &mut R,
        state: &S,
    ) -> Result<MutationResult, Error> {
        let mutator = self
            .get_mut(index.0)
            .ok_or_else(|| Error::key_not_found(format!("Mutator with id {index:?} not found.")))?;
        mutator.mutate(input, rand, state)
    }

    fn get_and_post_exec(
        &mut self,
        index: usize,
        state: &mut S,
        new_testcase_id: Option<TestcaseId>,
    ) -> Result<(), Error> {
        let mutator = self
            .get_mut(index)
            .ok_or_else(|| Error::key_not_found(format!("Mutator with id {index:?} not found.")))?;
        mutator.post_exec(state, new_testcase_id)
    }
}

impl<I, R: Rand, S> IntoVec<Box<dyn Mutator<I, R, S>>> for Vec<Box<dyn Mutator<I, R, S>>> {
    fn into_vec(self) -> Vec<Box<dyn Mutator<I, R, S>>> {
        self
    }
}

/// [`Mutator`] that does nothing, used for testing.
///
/// Example:
///
/// ```rust,ignore
/// let mut stages = tuple_list!(StdMutationalStage::new(NopMutator(MutationResult::Mutated)));
/// ```
#[derive(Debug, Copy, Clone)]
pub struct NopMutator {
    result: MutationResult,
}

impl NopMutator {
    /// The passed argument is returned every time the mutator is called.
    #[must_use]
    pub fn new(result: MutationResult) -> Self {
        Self { result }
    }
}

impl<I, R: Rand, S> Mutator<I, R, S> for NopMutator {
    fn mutate(&mut self, _input: &mut I, _rand: &mut R, state: &S) -> Result<MutationResult, Error> {
        Ok(self.result)
    }
    #[inline]
    fn post_exec(
        &mut self,
        _state: &mut S,
        _new_testcase_id: Option<TestcaseId>,
    ) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for NopMutator {
    fn name(&self) -> &Cow<'static, str> {
        &Cow::Borrowed("NopMutator")
    }
}

/// [`Mutator`] that inverts a boolean value.
///
/// Mostly useful in combination with [`mapping::MappingMutator`]s to mutate parts of a complex input.
#[derive(Debug)]
pub struct BoolInvertMutator;

impl<R: Rand, S> Mutator<bool, R, S> for BoolInvertMutator {
    fn mutate(&mut self, input: &mut bool, _rand: &mut R, state: &S) -> Result<MutationResult, Error> {
        *input = !*input;
        Ok(MutationResult::Mutated)
    }
    #[inline]
    fn post_exec(
        &mut self,
        _state: &mut S,
        _new_testcase_id: Option<TestcaseId>,
    ) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for BoolInvertMutator {
    fn name(&self) -> &Cow<'static, str> {
        &Cow::Borrowed("BoolInvertMutator")
    }
}
