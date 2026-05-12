//! Mutators for the `Nautilus` grammmar fuzzer
//! See <https://www.ndss-symposium.org/ndss-paper/nautilus-fishing-for-deep-bugs-with-grammars/>
use alloc::borrow::Cow;
use core::fmt::Debug;

use libafl_bolts::{
    Named,
    rands::{Rand, RomuDuoJrRand},
};

use crate::{
    Error,
    common::nautilus::grammartec::{
        context::Context,
        mutator::Mutator as BackingMutator,
        tree::{Tree, TreeMutation},
    },
    feedbacks::NautilusChunksMetadata,
    fuzzers::EvaluationResult,
    generators::nautilus::NautilusContext,
    inputs::nautilus::NautilusInput,
    mutators::{MutationResult, Mutator},
    states::{FlatState, named_metadata},
};

/// The randomic mutator for `Nautilus` grammar.
pub struct NautilusRandomMutator<'a> {
    ctx: &'a Context,
    mutator: BackingMutator,
}

impl Debug for NautilusRandomMutator<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "NautilusRandomMutator {{}}")
    }
}

impl<R: Rand, S> Mutator<NautilusInput, R, S> for NautilusRandomMutator<'_> {
    fn mutate(
        &mut self,
        input: &mut NautilusInput,
        rand: &mut R,
        _state: &S,
    ) -> Result<MutationResult, Error> {
        // TODO get rid of tmp
        let mut tmp = vec![];
        self.mutator
            .mut_random::<_, _>(
                rand,
                &input.tree,
                self.ctx,
                &mut |t: &TreeMutation, _ctx: &Context| {
                    tmp.extend_from_slice(t.prefix);
                    tmp.extend_from_slice(t.repl);
                    tmp.extend_from_slice(t.postfix);
                    Ok(())
                },
            )
            .unwrap();
        if tmp.is_empty() {
            Ok(MutationResult::Skipped)
        } else {
            input.tree = Tree::from_rule_vec(tmp, self.ctx);
            Ok(MutationResult::Mutated)
        }
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for NautilusRandomMutator<'_> {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("NautilusRandomMutator");
        &NAME
    }
}

impl<'a> NautilusRandomMutator<'a> {
    /// Creates a new [`NautilusRandomMutator`].
    #[must_use]
    pub fn new(context: &'a NautilusContext) -> Self {
        let mutator = BackingMutator::new(&context.ctx);
        Self {
            ctx: &context.ctx,
            mutator,
        }
    }
}

/// The `Nautilus` recursion mutator
// TODO calculate reucursions only for new items in corpus
pub struct NautilusRecursionMutator<'a> {
    ctx: &'a Context,
    mutator: BackingMutator,
}

impl Debug for NautilusRecursionMutator<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "NautilusRecursionMutator {{}}")
    }
}

impl<R: Rand, S> Mutator<NautilusInput, R, S> for NautilusRecursionMutator<'_> {
    fn mutate(
        &mut self,
        input: &mut NautilusInput,
        rand: &mut R,
        _state: &S,
    ) -> Result<MutationResult, Error> {
        // TODO don't calc recursions here
        if let Some(ref mut recursions) = input.tree.calc_recursions(self.ctx) {
            // TODO get rid of tmp
            let mut tmp = vec![];
            self.mutator
                .mut_random_recursion::<_, _>(
                    rand,
                    &input.tree,
                    recursions,
                    self.ctx,
                    &mut |t: &TreeMutation, _ctx: &Context| {
                        tmp.extend_from_slice(t.prefix);
                        tmp.extend_from_slice(t.repl);
                        tmp.extend_from_slice(t.postfix);
                        Ok(())
                    },
                )
                .unwrap();
            if !tmp.is_empty() {
                input.tree = Tree::from_rule_vec(tmp, self.ctx);
                return Ok(MutationResult::Mutated);
            }
        }
        Ok(MutationResult::Skipped)
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for NautilusRecursionMutator<'_> {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("NautilusRecursionMutator");
        &NAME
    }
}

impl<'a> NautilusRecursionMutator<'a> {
    /// Creates a new [`NautilusRecursionMutator`].
    #[must_use]
    pub fn new(context: &'a NautilusContext) -> Self {
        let mutator = BackingMutator::new(&context.ctx);
        Self {
            ctx: &context.ctx,
            mutator,
        }
    }
}

/// The splicing mutator for `Nautilus` that can splice inputs together
pub struct NautilusSpliceMutator<'a> {
    ctx: &'a Context,
    mutator: BackingMutator,
}

impl Debug for NautilusSpliceMutator<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "NautilusSpliceMutator {{}}")
    }
}

impl<R: Rand, S> Mutator<NautilusInput, R, S> for NautilusSpliceMutator<'_>
where
    S: FlatState,
{
    fn mutate(
        &mut self,
        input: &mut NautilusInput,
        rand: &mut R,
        state: &S,
    ) -> Result<MutationResult, Error> {
        // TODO get rid of tmp
        let mut tmp = vec![];
        // Create a fast temp mutator to get around borrowing..
        let mut rand_cpy = { RomuDuoJrRand::with_seed(rand.next()) };
        let meta = named_metadata::<NautilusChunksMetadata>(state.metadata_map(), self.name())?;
        self.mutator
            .mut_splice::<_, _>(
                &mut rand_cpy,
                &input.tree,
                self.ctx,
                &meta.cks,
                &mut |t: &TreeMutation, _ctx: &Context| {
                    tmp.extend_from_slice(t.prefix);
                    tmp.extend_from_slice(t.repl);
                    tmp.extend_from_slice(t.postfix);
                    Ok(())
                },
            )
            .unwrap();
        if tmp.is_empty() {
            Ok(MutationResult::Skipped)
        } else {
            input.tree = Tree::from_rule_vec(tmp, self.ctx);
            Ok(MutationResult::Mutated)
        }
    }
    #[inline]
    fn post_exec(&mut self, _state: &mut S, _eval_res: &EvaluationResult) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for NautilusSpliceMutator<'_> {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("NautilusSpliceMutator");
        &NAME
    }
}

impl<'a> NautilusSpliceMutator<'a> {
    /// Creates a new [`NautilusSpliceMutator`].
    #[must_use]
    pub fn new(context: &'a NautilusContext) -> Self {
        let mutator = BackingMutator::new(&context.ctx);
        Self {
            ctx: &context.ctx,
            mutator,
        }
    }
}
