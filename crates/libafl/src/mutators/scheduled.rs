//! The `ScheduledMutator` schedules multiple mutations internally.

use alloc::{borrow::Cow, vec::Vec};
use core::{
    fmt::Debug,
    num::NonZero,
    ops::{Deref, DerefMut},
};

use libafl_bolts::{
    Named,
    rands::Rand,
    tuples::{NamedTuple, tuple_list, tuple_list_type},
};
use serde::{Deserialize, Serialize};

use super::MutationId;
use crate::{
    Error,
    corpus::{Corpus, TestcaseId, testcase},
    fuzzers::EvaluationResult,
    mutators::{MutationResult, Mutator, MutatorsTuple, TokenInsert, TokenReplace},
    states::HasCorpus,
};

/// The metadata placed in a [`crate::corpus::Testcase`] by a [`LoggerScheduledMutator`].
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(miri, expect(clippy::unsafe_derive_deserialize))] // for SerdeAny
pub struct LogMutationMetadata {
    /// A list of logs
    pub list: Vec<Cow<'static, str>>,
}

libafl_bolts::impl_serdeany!(LogMutationMetadata);

impl Deref for LogMutationMetadata {
    type Target = [Cow<'static, str>];
    fn deref(&self) -> &[Cow<'static, str>] {
        &self.list
    }
}
impl DerefMut for LogMutationMetadata {
    fn deref_mut(&mut self) -> &mut [Cow<'static, str>] {
        &mut self.list
    }
}

impl LogMutationMetadata {
    /// Creates new [`struct@LogMutationMetadata`].
    #[must_use]
    pub fn new(list: Vec<Cow<'static, str>>) -> Self {
        Self { list }
    }
}

/// A [`Mutator`] that composes multiple mutations into one.
pub trait ComposedByMutations {
    /// The mutations of this
    type Mutations;
    /// Get the mutations
    fn mutations(&self) -> &Self::Mutations;

    /// Get the mutations (mutable)
    fn mutations_mut(&mut self) -> &mut Self::Mutations;
}

/// A [`Mutator`] scheduling multiple [`Mutator`]s for an input.
pub trait ScheduledMutator<I, R, S>: ComposedByMutations + Mutator<I, R, S>
where
    Self::Mutations: MutatorsTuple<I, R, S>,
{
    /// Compute the number of iterations used to apply stacked mutations
    fn iterations(&self, input: &I, rand: &mut R, state: &S) -> u64;

    /// Get the next mutation to apply
    fn schedule(&self, input: &I, rand: &mut R, state: &S) -> MutationId;

    /// New default implementation for mutate.
    /// Implementations must forward `mutate()` to this method
    fn scheduled_mutate(
        &mut self,
        input: &mut I,
        rand: &mut R,
        state: &S,
    ) -> Result<MutationResult, Error> {
        let mut r = MutationResult::Skipped;
        let num = self.iterations(input, rand, state);
        for _ in 0..num {
            let idx = self.schedule(input, rand, state);
            let outcome = self
                .mutations_mut()
                .get_and_mutate(idx, input, rand, state)?;
            if outcome == MutationResult::Mutated {
                r = MutationResult::Mutated;
            }
        }
        Ok(r)
    }
}

/// A [`Mutator`] that schedules one of the embedded mutations on each call.
#[derive(Debug)]
pub struct SingleChoiceScheduledMutator<MT> {
    name: Cow<'static, str>,
    mutations: MT,
}

impl<MT> Named for SingleChoiceScheduledMutator<MT> {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<I, MT, R, S> Mutator<I, R, S> for SingleChoiceScheduledMutator<MT>
where
    R: Rand,
    MT: MutatorsTuple<I, R, S>,
{
    #[inline]
    fn mutate(&mut self, input: &mut I, rand: &mut R, state: &S) -> Result<MutationResult, Error> {
        self.scheduled_mutate(input, rand, state)
    }
    #[inline]
    fn post_exec(&mut self, state: &mut S, eval_res: &EvaluationResult) -> Result<(), Error> {
        self.mutations.post_exec_all(state, eval_res)
    }
}

impl<MT> ComposedByMutations for SingleChoiceScheduledMutator<MT> {
    type Mutations = MT;
    /// Get the mutations
    #[inline]
    fn mutations(&self) -> &MT {
        &self.mutations
    }

    // Get the mutations (mutable)
    #[inline]
    fn mutations_mut(&mut self) -> &mut MT {
        &mut self.mutations
    }
}

impl<I, MT, R, S> ScheduledMutator<I, R, S> for SingleChoiceScheduledMutator<MT>
where
    R: Rand,
    MT: MutatorsTuple<I, R, S>,
{
    /// Compute the number of iterations used to apply stacked mutations
    fn iterations(&self, _: &I, rand: &mut R, state: &S) -> u64 {
        1
    }

    /// Get the next mutation to apply
    fn schedule(&self, _: &I, rand: &mut R, state: &S) -> MutationId {
        debug_assert_ne!(self.mutations.len(), 0);
        // # Safety
        // We check for empty mutations

        rand.below(unsafe { NonZero::new_unchecked(self.mutations.len()) })
            .into()
    }
}

impl<MT> SingleChoiceScheduledMutator<MT>
where
    MT: NamedTuple,
{
    /// Create a new [`SingleChoiceScheduledMutator`] instance specifying mutations
    pub fn new(mutations: MT) -> Self {
        SingleChoiceScheduledMutator {
            name: Cow::from(format!(
                "SingleChoiceScheduledMutator[{}]",
                mutations.names().join(", ")
            )),
            mutations,
        }
    }
}

/// A [`Mutator`] that stacks embedded mutations in a havoc manner on each call.
#[derive(Debug)]
pub struct HavocScheduledMutator<MT> {
    name: Cow<'static, str>,
    mutations: MT,
    max_stack_pow: usize,
}

impl<MT> Named for HavocScheduledMutator<MT> {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<I, MT, R, S> Mutator<I, R, S> for HavocScheduledMutator<MT>
where
    R: Rand,
    MT: MutatorsTuple<I, R, S>,
{
    #[inline]
    fn mutate(&mut self, input: &mut I, rand: &mut R, state: &S) -> Result<MutationResult, Error> {
        self.scheduled_mutate(input, rand, state)
    }
    #[inline]
    fn post_exec(&mut self, state: &mut S, eval_res: &EvaluationResult) -> Result<(), Error> {
        self.mutations.post_exec_all(state, eval_res)
    }
}

impl<MT> ComposedByMutations for HavocScheduledMutator<MT> {
    type Mutations = MT;
    /// Get the mutations
    #[inline]
    fn mutations(&self) -> &MT {
        &self.mutations
    }

    // Get the mutations (mutable)
    #[inline]
    fn mutations_mut(&mut self) -> &mut MT {
        &mut self.mutations
    }
}

impl<I, MT, R, S> ScheduledMutator<I, R, S> for HavocScheduledMutator<MT>
where
    R: Rand,
    MT: MutatorsTuple<I, R, S>,
{
    /// Compute the number of iterations used to apply stacked mutations
    fn iterations(&self, _: &I, rand: &mut R, state: &S) -> u64 {
        1 << (1 + rand.below_or_zero(self.max_stack_pow))
    }

    /// Get the next mutation to apply
    fn schedule(&self, _: &I, rand: &mut R, state: &S) -> MutationId {
        debug_assert_ne!(self.mutations.len(), 0);
        // # Safety
        // We check for empty mutations
        rand.below(unsafe { NonZero::new_unchecked(self.mutations.len()) })
            .into()
    }
}

impl<MT> HavocScheduledMutator<MT>
where
    MT: NamedTuple,
{
    /// Create a new [`HavocScheduledMutator`] instance specifying mutations
    pub fn new(mutations: MT) -> Self {
        HavocScheduledMutator {
            name: Cow::from(format!(
                "HavocScheduledMutator[{}]",
                mutations.names().join(", ")
            )),
            mutations,
            max_stack_pow: 7,
        }
    }

    /// Create a new [`HavocScheduledMutator`] instance specifying mutations and the maximun number of iterations
    #[inline]
    pub fn with_max_stack_pow(mutations: MT, max_stack_pow: usize) -> Self {
        Self {
            name: Cow::from(format!(
                "HavocScheduledMutator[{}]",
                mutations.names().join(", ")
            )),
            mutations,
            max_stack_pow,
        }
    }
}

/// Get the mutations that uses the Tokens metadata
#[must_use]
pub fn tokens_mutations() -> tuple_list_type!(TokenInsert, TokenReplace) {
    tuple_list!(TokenInsert::new(), TokenReplace::new())
}

#[cfg(test)]
mod tests {
    use libafl_bolts::rands::{StdRand, XkcdRand};

    use crate::{
        corpus::{Corpus, InMemoryCorpus, Testcase, schedulers::QueueScheduler},
        feedbacks::ConstFeedback,
        inputs::{BytesInput, HasMutatorBytes, bytes::BytesContext},
        mutators::{
            Mutator,
            havoc_mutations::havoc_mutations,
            mutations::SpliceMutator,
            scheduled::{HavocScheduledMutator, SingleChoiceScheduledMutator},
        },
        states::StdState,
    };

    use alloc::rc::Rc;

    #[test]
    fn test_mut_scheduled() {
        let mut rand = XkcdRand::with_seed(0);
        let mut corpus = InMemoryCorpus::new(QueueScheduler::new());
        let id1 = corpus
            .add(Testcase::new(Rc::new(BytesInput::new(
                vec![b'a', b'b', b'c'].into(),
            ))))
            .unwrap();
        let id2 = corpus
            .add(Testcase::new(Rc::new(BytesInput::new(
                vec![b'd', b'e', b'f'].into(),
            ))))
            .unwrap();

        let mut input = corpus.get(&id1).unwrap().cloned_input();

        let mut state = StdState::new(
            BytesContext::default(),
            corpus,
            InMemoryCorpus::new(QueueScheduler::new()),
        )
        .unwrap();

        let mut splice = SpliceMutator::new();
        splice.mutate(&mut input, &mut rand, &state).unwrap();

        log::trace!("{:?}", input.mutator_bytes());

        // The pre-seeded rand should have spliced at position 2.
        assert_eq!(input.mutator_bytes(), b"abf");
    }

    #[test]
    fn test_havoc() {
        let mut rand = StdRand::with_seed(0x1337);
        let mut corpus = InMemoryCorpus::new(QueueScheduler::new());
        let id1 = corpus
            .add(Testcase::new(Rc::new(BytesInput::new(
                b"abc".to_vec().into(),
            ))))
            .unwrap();
        let id2 = corpus
            .add(Testcase::new(Rc::new(BytesInput::new(
                b"def".to_vec().into(),
            ))))
            .unwrap();

        let mut input = corpus.get(&id1).unwrap().cloned_input();
        let input_prior = input.clone();

        let mut state = StdState::new(
            BytesContext::default(),
            corpus,
            InMemoryCorpus::new(QueueScheduler::new()),
        )
        .unwrap();

        let mut havoc = HavocScheduledMutator::new(havoc_mutations());

        assert_eq!(input, input_prior);

        let mut equal_in_a_row = 0;

        for _ in 0..42 {
            havoc.mutate(&mut input, &mut rand, &state).unwrap();

            // Make sure we actually mutate something, at least sometimes
            equal_in_a_row = if input == input_prior {
                equal_in_a_row + 1
            } else {
                0
            };
            assert_ne!(equal_in_a_row, 5);
        }
    }

    #[test]
    fn test_single_choice() {
        let mut rand = StdRand::with_seed(0x1337);
        let mut corpus = InMemoryCorpus::new(QueueScheduler::new());
        let id1 = corpus
            .add(Testcase::new(Rc::new(BytesInput::new(
                b"abc".to_vec().into(),
            ))))
            .unwrap();
        let id2 = corpus
            .add(Testcase::new(Rc::new(BytesInput::new(
                b"def".to_vec().into(),
            ))))
            .unwrap();

        let mut input = corpus.get(&id1).unwrap().cloned_input();
        let input_prior = input.clone();

        let mut state = StdState::new(
            BytesContext::default(),
            corpus,
            InMemoryCorpus::new(QueueScheduler::new()),
        )
        .unwrap();

        let mut mutator = SingleChoiceScheduledMutator::new(havoc_mutations());

        assert_eq!(input, input_prior);

        let mut equal_in_a_row = 0;

        for _ in 0..100 {
            mutator.mutate(&mut input, &mut rand, &state).unwrap();

            // Make sure we actually mutate something, at least sometimes
            equal_in_a_row = if input == input_prior {
                equal_in_a_row + 1
            } else {
                0
            };
            assert_ne!(equal_in_a_row, 20);
        }
    }
}
