//! `LibAFL` version of the [`Nautilus`](https://github.com/nautilus-fuzz/nautilus) grammar fuzzer
#![doc = include_str!("README.md")]

/// Grammartec module
pub mod grammartec;
pub use grammartec::{
    ChunkStore, ChunkStoreWrapper, Context, GrammarMutator, NTermId, NodeId, PlainRule,
    RecursionInfo, RegExpRule, Rule, RuleChild, RuleId, RuleIdOrCustom, Tree, TreeLike,
    TreeMutation,
};

/// Regex mutator module
pub mod regex_mutator;
pub use regex_mutator::RegexScript;
