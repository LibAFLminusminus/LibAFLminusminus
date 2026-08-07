//! Mutators for integer-style inputs

use alloc::borrow::Cow;
use core::marker::PhantomData;

use libaflmm_bolts::{Error, Named, rands::Rand, tuples::Merge};
use tuple_list::{tuple_list, tuple_list_type};

use super::{MutationResult, Mutator};
use crate::{
    corpus::{Corpus, schedulers::Scheduler},
    fuzzers::EvaluationResult,
    inputs::value::Numeric,
    states::State,
};

/// All mutators for integer-like inputs
pub type IntMutatorsType = tuple_list_type!(
    BitFlipMutator,
    NegateMutator,
    IncMutator,
    DecMutator,
    TwosComplementMutator,
    RandMutator,
    CrossoverMutator
);

/// Mutators for integer-like inputs that implement some form of crossover
pub type IntMutatorsCrossoverType = tuple_list_type!(CrossoverMutator);

/// Mapped mutators for integer-like inputs that implement some form of crossover.
pub type MappedIntMutatorsCrossoverType<F, I> = tuple_list_type!(MappedCrossoverMutator<F, I>);

/// Mutators for integer-like inputs without crossover mutations
pub type IntMutatorsNoCrossoverType = tuple_list_type!(
    BitFlipMutator,
    NegateMutator,
    IncMutator,
    DecMutator,
    TwosComplementMutator,
    RandMutator,
);

/// Mutators for integer-like inputs without crossover mutations
#[must_use]
pub fn int_mutators_no_crossover() -> IntMutatorsNoCrossoverType {
    tuple_list!(
        BitFlipMutator,
        NegateMutator,
        IncMutator,
        DecMutator,
        TwosComplementMutator,
        RandMutator,
    )
}

/// Mutators for integer-like inputs that implement some form of crossover
#[must_use]
pub fn int_mutators_crossover() -> IntMutatorsCrossoverType {
    tuple_list!(CrossoverMutator)
}

/// Mutators for integer-like inputs that implement some form of crossover with a mapper to extract the crossed over information.
#[must_use]
pub fn mapped_int_mutators_crossover<F, I>(
    input_mapper: F,
) -> MappedIntMutatorsCrossoverType<F, I> {
    tuple_list!(MappedCrossoverMutator::new(input_mapper))
}

/// Mutators for integer-like inputs
///
/// Modelled after the applicable mutators from [`super::havoc_mutations::havoc_mutations`]
#[must_use]
pub fn int_mutators() -> IntMutatorsType {
    int_mutators_no_crossover().merge(int_mutators_crossover())
}

/// Bitflip mutation for integer-like inputs
#[derive(Debug)]
pub struct BitFlipMutator;

impl<I, R, S> Mutator<I, R, S> for BitFlipMutator
where
    R: Rand,
    I: Numeric,
{
    fn mutate(&mut self, input: &mut I, rand: &mut R, _state: &S) -> Result<MutationResult, Error> {
        let offset = rand.choose(0..input.bits()).unwrap();
        input.flip_bit_at(offset);
        Ok(MutationResult::Mutated)
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for BitFlipMutator {
    fn name(&self) -> &Cow<'static, str> {
        &Cow::Borrowed("BitFlipMutator")
    }
}

/// Negate mutation for integer-like inputs, i.e. flip all bits
#[derive(Debug)]
pub struct NegateMutator;

impl<I, R, S> Mutator<I, R, S> for NegateMutator
where
    I: Numeric,
{
    fn mutate(
        &mut self,
        input: &mut I,
        _rand: &mut R,
        _state: &S,
    ) -> Result<MutationResult, Error> {
        input.flip_all_bits();
        Ok(MutationResult::Mutated)
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for NegateMutator {
    fn name(&self) -> &Cow<'static, str> {
        &Cow::Borrowed("ByteFlipMutator")
    }
}

/// Increment mutation for integer-like inputs. Wraps on overflows.
#[derive(Debug)]
pub struct IncMutator;

impl<I, R, S> Mutator<I, R, S> for IncMutator
where
    I: Numeric,
{
    fn mutate(
        &mut self,
        input: &mut I,
        _rand: &mut R,
        _state: &S,
    ) -> Result<MutationResult, Error> {
        input.wrapping_inc();
        Ok(MutationResult::Mutated)
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for IncMutator {
    fn name(&self) -> &Cow<'static, str> {
        &Cow::Borrowed("IncMutator")
    }
}

/// Decrement mutation for integer-like inputs. Wraps on underflow.
#[derive(Debug)]
pub struct DecMutator;

impl<I, R, S> Mutator<I, R, S> for DecMutator
where
    I: Numeric,
{
    fn mutate(
        &mut self,
        input: &mut I,
        _rand: &mut R,
        _state: &S,
    ) -> Result<MutationResult, Error> {
        input.wrapping_dec();
        Ok(MutationResult::Mutated)
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for DecMutator {
    fn name(&self) -> &Cow<'static, str> {
        &Cow::Borrowed("DecMutator")
    }
}

/// Two's complement mutation for integer-like inputs
#[derive(Debug)]
pub struct TwosComplementMutator;

impl<I, R, S> Mutator<I, R, S> for TwosComplementMutator
where
    I: Numeric,
{
    fn mutate(
        &mut self,
        input: &mut I,
        _rand: &mut R,
        _state: &S,
    ) -> Result<MutationResult, Error> {
        input.twos_complement();
        Ok(MutationResult::Mutated)
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for TwosComplementMutator {
    fn name(&self) -> &Cow<'static, str> {
        &Cow::Borrowed("NegMutator")
    }
}

/// Randomize mutation for integer-like inputs
#[derive(Debug)]
pub struct RandMutator;

impl<I, R, S> Mutator<I, R, S> for RandMutator
where
    I: Numeric,
    R: Rand,
{
    fn mutate(&mut self, input: &mut I, rand: &mut R, _state: &S) -> Result<MutationResult, Error> {
        // set to random data byte-wise since the RNGs don't work for all numeric types
        input.randomize(rand);
        Ok(MutationResult::Mutated)
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for RandMutator {
    fn name(&self) -> &Cow<'static, str> {
        &Cow::Borrowed("RandMutator")
    }
}

/// Crossover mutation for integer-like inputs
#[derive(Debug)]
pub struct CrossoverMutator;

impl<I, R, S> Mutator<I, R, S> for CrossoverMutator
where
    R: Rand,
    S: State<Input = I>,
    I: Copy,
{
    fn mutate(&mut self, input: &mut I, rand: &mut R, state: &S) -> Result<MutationResult, Error> {
        let ids = state.scheduler().ids();
        let Some(id) = rand.choose(ids) else {
            return Ok(MutationResult::Skipped);
        };

        if state.scheduler().current().is_some_and(|cur| cur == *id) {
            return Ok(MutationResult::Skipped);
        }

        let other_testcase = state.corpus().get_from_all(id)?;
        *input = *other_testcase.input();
        Ok(MutationResult::Mutated)
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for CrossoverMutator {
    fn name(&self) -> &Cow<'static, str> {
        &Cow::Borrowed("CrossoverMutator")
    }
}
/// Crossover mutation for integer-like inputs with custom state extraction function
#[derive(Debug)]
pub struct MappedCrossoverMutator<F, I> {
    input_mapper: F,
    phantom: PhantomData<I>,
}

impl<F, I> MappedCrossoverMutator<F, I> {
    /// Create a new [`MappedCrossoverMutator`]
    pub fn new(input_mapper: F) -> Self {
        Self {
            input_mapper,
            phantom: PhantomData,
        }
    }
}

impl<I, O, R, S, F> Mutator<O, R, S> for MappedCrossoverMutator<F, I>
where
    R: Rand,
    S: State<Input = I>,
    for<'b> F: Fn(&'b I) -> &'b O,
    O: Clone,
{
    fn mutate(&mut self, input: &mut O, rand: &mut R, state: &S) -> Result<MutationResult, Error> {
        let ids = state.scheduler().ids();
        let Some(id) = rand.choose(ids) else {
            return Ok(MutationResult::Skipped);
        };

        if state.scheduler().current().is_some_and(|cur| cur == *id) {
            return Ok(MutationResult::Skipped);
        }

        let other_testcase = state.corpus().get_from_all(id)?;
        let other_input = other_testcase.input();
        let mapped_input = (self.input_mapper)(&other_input).clone();
        *input = mapped_input;
        Ok(MutationResult::Mutated)
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl<F, I> Named for MappedCrossoverMutator<F, I> {
    fn name(&self) -> &Cow<'static, str> {
        &Cow::Borrowed("MappedCrossoverMutator")
    }
}

#[cfg(test)]
mod tests {

    use alloc::rc::Rc;

    use libaflmm_bolts::{
        StdRand,
        rands::{Rand, XkcdRand},
        tuples::IntoVec as _,
    };
    use serde::{Deserialize, Serialize};

    use super::{Numeric, int_mutators};
    use crate::{
        corpus::{
            InMemoryCorpus, ObjectiveInMemoryCorpus, ScheduledCorpus, Testcase,
            schedulers::QueueScheduler,
        },
        inputs::value::{I16Input, PrimitiveContext, U32Input},
        mutators::{MutationResult, Mutator, numeric::BitFlipMutator},
        states::StdState,
    };

    #[test]
    fn test_numeric_bits() {
        assert_eq!(0_u8.bits(), 8);
        assert_eq!(0_u16.bits(), 16);
        assert_eq!(0_u32.bits(), 32);
        assert_eq!(0_u64.bits(), 64);
        assert_eq!(0_u128.bits(), 128);
        assert_eq!(0_i8.bits(), 8);
        assert_eq!(0_i16.bits(), 16);
        assert_eq!(0_i32.bits(), 32);
        assert_eq!(0_i64.bits(), 64);
        assert_eq!(0_i128.bits(), 128);
    }

    #[test]
    fn test_bit_flip_mutator_upper_bits() {
        let state = StdState::nop();

        let mut rand = StdRand::with_seed(0);
        let mut mutator = BitFlipMutator;
        let mut flipped_above_byte = false;

        for _ in 0..100 {
            let mut input = U32Input::new(0_u32);
            mutator.mutate(&mut input, &mut rand, &state).unwrap();
            if !input.into_inner().trailing_zeros().is_multiple_of(8) {
                flipped_above_byte = true;
                break;
            }
        }

        assert!(flipped_above_byte, "BitFlipMutator byte alignment error");
    }
    #[test]
    fn randomized() {
        const RAND_NUM: u64 = 0xAAAAAAAAAAAAAAAA; // 0b10101010..
        #[derive(Serialize, Deserialize, Debug)]
        struct FixedRand;
        impl Rand for FixedRand {
            fn set_seed(&mut self, _seed: u64) {}
            fn next(&mut self) -> u64 {
                RAND_NUM
            }
        }

        let rand = &mut FixedRand;

        let mut i = 0_u8;
        Numeric::randomize(&mut i, rand);
        assert_eq!(0xAA, i);

        let mut i = 0_u128;
        Numeric::randomize(&mut i, rand);
        assert_eq!(((u128::from(RAND_NUM) << 64) | u128::from(RAND_NUM)), i);

        let mut i = 0_i16;
        Numeric::randomize(&mut i, rand);
        assert_eq!(-0b101010101010110, i); // two's complement
    }

    #[test]
    fn all_mutate_owned() {
        let mut corpus = InMemoryCorpus::new(QueueScheduler::new());
        corpus
            .add(Testcase::new(Rc::new(I16Input::new(42_i16))))
            .unwrap();
        let primitive_context: PrimitiveContext<i16> = PrimitiveContext::default();
        let state =
            StdState::new(primitive_context, corpus, ObjectiveInMemoryCorpus::new()).unwrap();
        let mut rand = XkcdRand::new();

        let mutators = int_mutators().into_vec();

        for mut m in mutators {
            let mut input: I16Input = 1_i16.into();
            assert_eq!(
                MutationResult::Mutated,
                m.mutate(&mut input, &mut rand, &state).unwrap(),
                "Errored with {}",
                m.name()
            );
            assert_ne!(1, input.into_inner(), "Errored with {}", m.name());
        }
    }
}
