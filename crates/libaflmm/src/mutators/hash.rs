//! A wrapper around a [`Mutator`] that ensures an input really changed [`MutationResult::Mutated`]
//! by hashing pre- and post-mutation
use alloc::borrow::Cow;
use core::hash::Hash;

use libafl_bolts::{Error, Named, generic_hash_std, rands::Rand};

use super::{MutationResult, Mutator};
use crate::fuzzers::EvaluationResult;

/// A wrapper around a [`Mutator`] that ensures an input really changed [`MutationResult::Mutated`]
/// by hashing pre- and post-mutation and comparing the values
#[derive(Debug)]
pub struct MutationChecker<M> {
    inner: M,
    name: Cow<'static, str>,
}

impl<M> MutationChecker<M>
where
    M: Named,
{
    /// Create a new [`MutationChecker`]
    pub fn new(inner: M) -> Self {
        let name = Cow::Owned(format!("MutationChecker<{}>", inner.name().clone()));
        Self { inner, name }
    }
}

impl<M, I, R: Rand, S> Mutator<I, R, S> for MutationChecker<M>
where
    I: Hash,
    M: Mutator<I, R, S>,
{
    fn mutate(&mut self, input: &mut I, rand: &mut R, state: &S) -> Result<MutationResult, Error> {
        let before = generic_hash_std(input);
        self.inner.mutate(input, rand, state)?;
        if before == generic_hash_std(input) {
            Ok(MutationResult::Skipped)
        } else {
            Ok(MutationResult::Mutated)
        }
    }
    #[inline]
    fn post_exec(&mut self, state: &mut S, eval_res: &EvaluationResult) -> Result<(), Error> {
        self.inner.post_exec(state, eval_res)
    }
}

impl<M> Named for MutationChecker<M> {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use libafl_bolts::rands::StdRand;

    use crate::{
        inputs::BytesInput,
        mutators::{BytesSetMutator, MutationChecker, MutationResult, Mutator},
        states::NopState,
    };

    #[test]
    fn not_mutated() {
        let state: NopState<BytesInput> = NopState::new();
        let mut rand = StdRand::with_seed(1337);
        let mut inner = BytesSetMutator::new();

        let mut input = BytesInput::new(vec![0; 5]);

        // nothing changed, yet `MutationResult::Mutated` was reported
        assert_eq!(
            MutationResult::Mutated,
            inner.mutate(&mut input, &mut rand, &state).unwrap()
        );
        assert_eq!(BytesInput::new(vec![0; 5]), input);

        // now it is correctly reported as `MutationResult::Skipped`
        let mut hash_mutator = MutationChecker::new(inner);
        assert_eq!(
            MutationResult::Skipped,
            hash_mutator.mutate(&mut input, &mut rand, &state).unwrap()
        );
        assert_eq!(BytesInput::new(vec![0; 5]), input);
    }
}
