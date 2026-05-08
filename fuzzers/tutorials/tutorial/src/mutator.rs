use std::borrow::Cow;

use lain::traits::Mutatable;
use libafl::{
    mutators::{MutationResult, Mutator},
    Error, EvaluationResult,
};
use libafl_bolts::{
    rands::{Rand, StdRand},
    Named,
};

use crate::input::PacketData;

pub struct LainMutator {
    inner: lain::mutator::Mutator<StdRand>,
}

impl<R, S> Mutator<PacketData, R, S> for LainMutator
where
    R: Rand,
{
    fn mutate(
        &mut self,
        input: &mut PacketData,
        rand: &mut R,
        _state: &S,
    ) -> Result<MutationResult, Error> {
        // Lain uses its own instance of StdRand, but we want to keep it in sync with LibAFL's state.
        self.inner.rng_mut().set_seed(rand.next());
        input.mutate(&mut self.inner, None);
        Ok(MutationResult::Mutated)
    }

    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for LainMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("LainMutator");
        &NAME
    }
}

impl LainMutator {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: lain::mutator::Mutator::new(StdRand::with_seed(0)),
        }
    }
}

impl Default for LainMutator {
    fn default() -> Self {
        Self::new()
    }
}
