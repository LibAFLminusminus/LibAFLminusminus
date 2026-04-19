//! Mutations for [`EncodedInput`]s
use alloc::{borrow::Cow, vec::Vec};
use core::{
    cmp::{max, min},
    num::NonZero,
};

use libafl_bolts::{
    non_zero_const,
    rands::Rand,
    tuples::{tuple_list, tuple_list_type},
};

use crate::{
    Error,
    corpus::{Corpus, Scheduler, testcase::TestcaseId},
    fuzzer::EvaluationResult,
    inputs::EncodedInput,
    mutators::{
        MutationResult, Mutator, Named,
        mutations::{ARITH_MAX, buffer_copy, buffer_self_copy},
    },
    state::{FlatState, HasCorpus, HasScheduler},
};

/// Set a code in the input as a random value
#[derive(Debug, Default)]
pub struct EncodedRandMutator;

impl<R: Rand, S> Mutator<EncodedInput, R, S> for EncodedRandMutator {
    fn mutate(
        &mut self,
        input: &mut EncodedInput,
        rand: &mut R,
        state: &S,
    ) -> Result<MutationResult, Error> {
        if input.codes().is_empty() {
            Ok(MutationResult::Skipped)
        } else {
            let val = rand.choose(input.codes_mut()).unwrap();
            *val = rand.next() as u32;
            Ok(MutationResult::Mutated)
        }
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for EncodedRandMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("EncodedRandMutator");
        &NAME
    }
}

impl EncodedRandMutator {
    /// Creates a new [`EncodedRandMutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Increment a random code in the input
#[derive(Debug, Default)]
pub struct EncodedIncMutator;

impl<R: Rand, S> Mutator<EncodedInput, R, S> for EncodedIncMutator {
    fn mutate(
        &mut self,
        input: &mut EncodedInput,
        rand: &mut R,
        state: &S,
    ) -> Result<MutationResult, Error> {
        if input.codes().is_empty() {
            Ok(MutationResult::Skipped)
        } else {
            let val = rand.choose(input.codes_mut()).unwrap();
            *val = val.wrapping_add(1);
            Ok(MutationResult::Mutated)
        }
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for EncodedIncMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("EncodedIncMutator");
        &NAME
    }
}

impl EncodedIncMutator {
    /// Creates a new [`EncodedIncMutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Decrement a random code in the input
#[derive(Debug, Default)]
pub struct EncodedDecMutator;

impl<R: Rand, S> Mutator<EncodedInput, R, S> for EncodedDecMutator {
    fn mutate(
        &mut self,
        input: &mut EncodedInput,
        rand: &mut R,
        state: &S,
    ) -> Result<MutationResult, Error> {
        if input.codes().is_empty() {
            Ok(MutationResult::Skipped)
        } else {
            let val = rand.choose(input.codes_mut()).unwrap();
            *val = val.wrapping_sub(1);
            Ok(MutationResult::Mutated)
        }
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for EncodedDecMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("EncodedDecMutator");
        &NAME
    }
}

impl EncodedDecMutator {
    /// Creates a new [`EncodedDecMutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Adds or subtracts a random value up to `ARITH_MAX` to a random place in the codes [`Vec`].
#[derive(Debug, Default)]
pub struct EncodedAddMutator;

impl<R: Rand, S> Mutator<EncodedInput, R, S> for EncodedAddMutator {
    fn mutate(
        &mut self,
        input: &mut EncodedInput,
        rand: &mut R,
        state: &S,
    ) -> Result<MutationResult, Error> {
        if input.codes().is_empty() {
            Ok(MutationResult::Skipped)
        } else {
            let val = rand.choose(input.codes_mut()).unwrap();
            let num = 1 + rand.below(non_zero_const!(ARITH_MAX)) as u32;
            *val = match rand.below(non_zero_const!(2)) {
                0 => val.wrapping_add(num),
                _ => val.wrapping_sub(num),
            };
            Ok(MutationResult::Mutated)
        }
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for EncodedAddMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("EncodedAddMutator");
        &NAME
    }
}

impl EncodedAddMutator {
    /// Creates a new [`EncodedAddMutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Codes delete mutation for encoded inputs
#[derive(Debug, Default)]
pub struct EncodedDeleteMutator;

impl<R: Rand, S> Mutator<EncodedInput, R, S> for EncodedDeleteMutator {
    fn mutate(
        &mut self,
        input: &mut EncodedInput,
        rand: &mut R,
        state: &S,
    ) -> Result<MutationResult, Error> {
        let size = input.codes().len();
        if size <= 2 {
            return Ok(MutationResult::Skipped);
        }
        // # Safety
        // The size is larger than 1 here (checked just above)
        let off = rand.below(unsafe { NonZero::new_unchecked(size) });
        // # Safety
        // The size of the offset is below size, the value is never 0.
        let len = rand.below(unsafe { NonZero::new_unchecked(size - off) });
        input.codes_mut().drain(off..off + len);

        Ok(MutationResult::Mutated)
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for EncodedDeleteMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("EncodedDeleteMutator");
        &NAME
    }
}

impl EncodedDeleteMutator {
    /// Creates a new [`EncodedDeleteMutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Insert mutation for encoded inputs
#[derive(Debug, Default)]
pub struct EncodedInsertCopyMutator {
    tmp_buf: Vec<u32>,
}

impl<R: Rand, S> Mutator<EncodedInput, R, S> for EncodedInsertCopyMutator
where
    S: FlatState,
{
    fn mutate(
        &mut self,
        input: &mut EncodedInput,
        rand: &mut R,
        state: &S,
    ) -> Result<MutationResult, Error> {
        let max_size = state.max_size();
        let size = input.codes().len();
        let Some(nz) = NonZero::new(size) else {
            return Ok(MutationResult::Skipped);
        };

        // # Safety
        // The input.codes() len should never be close to an usize, so adding 1 will always result in a non-zero value.
        // Worst case, we will get a wrong int value as return, not too bad.
        let off = rand.below(unsafe { NonZero::new_unchecked(size + 1) });
        let mut len = 1 + rand.below(nz);

        if size + len > max_size {
            if max_size > size {
                len = max_size - size;
            } else {
                return Ok(MutationResult::Skipped);
            }
        }

        let from = if let Some(bound) = NonZero::new(size - len) {
            rand.below(bound)
        } else {
            0
        };

        input.codes_mut().resize(size + len, 0);
        self.tmp_buf.resize(len, 0);
        unsafe {
            buffer_copy(&mut self.tmp_buf, input.codes(), from, 0, len);

            buffer_self_copy(input.codes_mut(), off, off + len, size - off);
            buffer_copy(input.codes_mut(), &self.tmp_buf, 0, off, len);
        };

        Ok(MutationResult::Mutated)
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for EncodedInsertCopyMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("EncodedInsertCopyMutator");
        &NAME
    }
}

impl EncodedInsertCopyMutator {
    /// Creates a new [`EncodedInsertCopyMutator`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Codes copy mutation for encoded inputs
#[derive(Debug, Default)]
pub struct EncodedCopyMutator;

impl<R: Rand, S> Mutator<EncodedInput, R, S> for EncodedCopyMutator {
    fn mutate(
        &mut self,
        input: &mut EncodedInput,
        rand: &mut R,
        state: &S,
    ) -> Result<MutationResult, Error> {
        let size = input.codes().len();
        if size <= 1 {
            return Ok(MutationResult::Skipped);
        }

        // # Safety
        // it's larger than 1
        let from = rand.below(unsafe { NonZero::new_unchecked(size) });
        let to = rand.below(unsafe { NonZero::new_unchecked(size) });
        // # Safety
        // Both from and to are smaller than size, so size minus any of these can never be 0.
        let len = 1 + rand.below(unsafe { NonZero::new_unchecked(size - max(from, to)) });

        unsafe {
            buffer_self_copy(input.codes_mut(), from, to, len);
        }

        Ok(MutationResult::Mutated)
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for EncodedCopyMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("EncodedCopyMutator");
        &NAME
    }
}

impl EncodedCopyMutator {
    /// Creates a new [`EncodedCopyMutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Crossover insert mutation for encoded inputs
#[derive(Debug, Default)]
pub struct EncodedCrossoverInsertMutator;

impl<R: Rand, S> Mutator<EncodedInput, R, S> for EncodedCrossoverInsertMutator
where
    S: HasCorpus<EncodedInput> + HasScheduler + FlatState,
{
    fn mutate(
        &mut self,
        input: &mut EncodedInput,
        rand: &mut R,
        state: &S,
    ) -> Result<MutationResult, Error> {
        let size = input.codes().len();
        let ids = state.scheduler().ids();

        let Some(id) = rand.choose(ids.into_iter()).copied() else {
            return Ok(MutationResult::Skipped);
        };
        // We don't want to use the testcase we're already using for splicing
        if let Some(cur) = state.scheduler().current()
            && id == cur
        {
            return Ok(MutationResult::Skipped);
        }

        let Some(nz) = NonZero::new(size) else {
            return Ok(MutationResult::Skipped);
        };

        let other_size = {
            // new scope to make the borrow checker happy
            let mut other_testcase = state.corpus().get_from_all(&id)?;
            other_testcase.input_len()
        };

        if other_size < 2 {
            return Ok(MutationResult::Skipped);
        }

        // # Safety
        // it's larger than 1
        let max_size = state.max_size();
        let from = rand.below(unsafe { NonZero::new_unchecked(other_size) });
        let to = rand.below(nz);
        // # Safety
        // from is smaller than other_size, other_size is larger than 2, so the subtraction is larger than 0.
        let mut len = 1 + rand.below(unsafe { NonZero::new_unchecked(other_size - from) });

        if size + len > max_size {
            if max_size > size {
                len = max_size - size;
            } else {
                return Ok(MutationResult::Skipped);
            }
        }

        let other = state.corpus().get_from_all(&id)?;

        input.codes_mut().resize(size + len, 0);
        unsafe {
            buffer_self_copy(input.codes_mut(), to, to + len, size - to);
            buffer_copy(input.codes_mut(), other.input().codes(), from, to, len);
        }

        Ok(MutationResult::Mutated)
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for EncodedCrossoverInsertMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("EncodedCrossoverInsertMutator");
        &NAME
    }
}

impl EncodedCrossoverInsertMutator {
    /// Creates a new [`EncodedCrossoverInsertMutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Crossover replace mutation for encoded inputs
#[derive(Debug, Default)]
pub struct EncodedCrossoverReplaceMutator;

impl<R: Rand, S> Mutator<EncodedInput, R, S> for EncodedCrossoverReplaceMutator
where
    S: HasCorpus<EncodedInput> + HasScheduler,
{
    fn mutate(
        &mut self,
        input: &mut EncodedInput,
        rand: &mut R,
        state: &S,
    ) -> Result<MutationResult, Error> {
        let size = input.codes().len();

        let ids = state.scheduler().ids();

        let Some(id) = rand.choose(ids.into_iter()).copied() else {
            return Ok(MutationResult::Skipped);
        };
        // We don't want to use the testcase we're already using for splicing
        if let Some(cur) = state.scheduler().current()
            && id == cur
        {
            return Ok(MutationResult::Skipped);
        }

        let other_size = {
            // new scope to make the borrow checker happy
            let mut other_testcase = state.corpus().get_from_all(&id)?;
            other_testcase.input_len()
        };

        if other_size < 2 {
            return Ok(MutationResult::Skipped);
        }
        // # Safety
        // other_size >= 2
        let from = rand.below(unsafe { NonZero::new_unchecked(other_size) });

        // # Safety
        // size > 0, other_size > from,
        let len = rand.below(unsafe { NonZero::new_unchecked(min(other_size - from, size)) });

        // # Safety
        // size is non-zero, len is below min(size, ...), so the subtraction will always be positive.
        let to = rand.below(unsafe { NonZero::new_unchecked(size - len) });

        let other = state.corpus().get_from_all(&id)?;

        unsafe {
            buffer_copy(input.codes_mut(), other.input().codes(), from, to, len);
        }

        Ok(MutationResult::Mutated)
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for EncodedCrossoverReplaceMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("EncodedCrossoverReplaceMutator");
        &NAME
    }
}

impl EncodedCrossoverReplaceMutator {
    /// Creates a new [`EncodedCrossoverReplaceMutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Get the mutations that compose the encoded mutator
#[must_use]
pub fn encoded_mutations() -> tuple_list_type!(
    EncodedRandMutator,
    EncodedIncMutator,
    EncodedDecMutator,
    EncodedAddMutator,
    EncodedDeleteMutator,
    EncodedInsertCopyMutator,
    EncodedCopyMutator,
    EncodedCrossoverInsertMutator,
    EncodedCrossoverReplaceMutator,
) {
    tuple_list!(
        EncodedRandMutator::new(),
        EncodedIncMutator::new(),
        EncodedDecMutator::new(),
        EncodedAddMutator::new(),
        EncodedDeleteMutator::new(),
        EncodedInsertCopyMutator::new(),
        EncodedCopyMutator::new(),
        EncodedCrossoverInsertMutator::new(),
        EncodedCrossoverReplaceMutator::new(),
    )
}
