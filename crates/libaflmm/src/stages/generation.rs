//! The [`GenStage`] generates a single input and evaluates it.
//!
//! A [`Stage`] that generates a single input via a
//! [`crate::generators::Generator`] and evaluates it using the fuzzer, possibly
//! adding it to the corpus.

use alloc::borrow::Cow;
use core::marker::PhantomData;

use libaflmm_bolts::Named;

use crate::{
    Result,
    common::DependencyResolver,
    corpus::testcase::TestcaseId,
    fuzzers::Evaluator,
    generators::Generator,
    stages::{RuntimeHandle, Stage},
    states::State,
};

/// A [`Stage`] that generates a single input via a [`Generator`] and evaluates
/// it using the fuzzer, possibly adding it to the corpus.
///
/// This stage can be used to construct black-box (e.g., grammar-based) fuzzers.
#[derive(Debug)]
pub struct GenStage<G, I>(G, PhantomData<I>);

impl<G, I> GenStage<G, I> {
    /// Create a new [`GenStage`].
    pub fn new(g: G) -> Self {
        Self(g, PhantomData)
    }
}

impl<G, I> DependencyResolver for GenStage<G, I> {}

impl<G, I> Named for GenStage<G, I> {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("gen");
        &NAME
    }
}

impl<E, G, I, R, S, W, Z> Stage<E, R, S, W, Z> for GenStage<G, I>
where
    G: Generator<I, R, S>,
    S: State,
    Z: Evaluator<E, I, S, W>,
{
    #[inline]
    fn perform_impl(
        &mut self,
        fuzzer: &mut Z,
        rand: &mut R,
        state: &mut S,
        rt_handle: &mut RuntimeHandle<S, W>,
        _testcase_id: &TestcaseId,
    ) -> Result<()> {
        let input = self.0.generate(rand, state)?;
        fuzzer.evaluate_input(state, rt_handle, &input)?;
        Ok(())
    }
}
