//! The [`Testcase`] is a struct embedded in each [`Corpus`].
//! It will contain a respective input, and metadata.

#[cfg(feature = "track_hit_feedbacks")]
use alloc::{borrow::Cow, vec::Vec};
use alloc::{rc::Rc, string::String};
use core::{fmt::Debug, hash::Hasher};

use libafl_bolts::{HasLen, hasher_std};
use serde::{Deserialize, Serialize};

use crate::{Error, corpus::CorpusId, inputs::Input, state::HasCorpus};

/// Shorthand to receive a [`Ref`] or [`RefMut`] to a stored [`Testcase`], by [`CorpusId`].
/// For a normal state, this should return a [`Testcase`] in the corpus, not the objectives.
pub trait HasTestcase<I>: HasCorpus<I> {
    /// Shorthand to receive a [`Ref`] to a stored [`Testcase`], by [`CorpusId`].
    /// For a normal state, this should return a [`Testcase`] in the corpus, not the objectives.
    fn testcase(&self, id: CorpusId) -> Result<Testcase<I>, Error>;
}

/// Indicates how a [`Testcase`] should be named on-disk.
#[derive(Default, Clone, Serialize, Deserialize, Debug)]
pub enum TestcaseFilenameFormat {
    /// Use the unique [`Testcase`] ID as a name.
    #[default]
    Id,
    /// Use a prefix before the id
    Prefix(String),
    /// Use a custom name.
    Custom(String),
}

impl TestcaseFilenameFormat {
    pub fn to_filename(&self, id: &str) -> String {
        match self {
            TestcaseFilenameFormat::Id => id.to_string(),
            TestcaseFilenameFormat::Prefix(prefix) => {
                format!("{}-{}", prefix, id)
            }
            TestcaseFilenameFormat::Custom(custom_name) => custom_name.clone(),
        }
    }
}

/// An entry in the [`Testcase`] Corpus
#[derive(Serialize, Deserialize, Debug)]
pub struct Testcase<I> {
    /// The [`Input`] of this [`Testcase`], or `None`, if it is not currently in memory
    input: Rc<I>,

    /// The unique id for [`Testcase`].
    /// It should uniquely identify the input.
    id: String,
}

impl<I> Clone for Testcase<I> {
    fn clone(&self) -> Self {
        Self {
            input: self.input.clone(),
            id: self.id.clone(),
        }
    }
}

impl<I> Testcase<I> {
    /// Get the input
    #[inline]
    pub fn input(&self) -> Rc<I> {
        self.input.clone()
    }

    /// Get the associated unique ID.
    pub fn id(&self) -> &String {
        &self.id
    }
}

impl<I> Testcase<I>
where
    I: HasLen,
{
    /// Get the input length
    pub fn input_len(&self) -> usize {
        self.input.len()
    }
}

impl<I> Testcase<I>
where
    I: Input,
{
    /// Create a new Testcase instance given an input
    pub fn new(input: Rc<I>) -> Self {
        let id = Self::compute_id(&input);

        Self { input, id }
    }

    /// Get the unique ID associated to an input.
    pub fn compute_id(input: &I) -> String {
        let mut hasher = hasher_std();
        input.hash(&mut hasher);
        let hash = hasher.finish();
        format!("{hash:0>8x}")
    }
}

impl<I> Testcase<I>
where
    I: Clone,
{
    /// Clone the input embedded in the [`Testcase`].
    pub fn cloned_input(&self) -> I {
        self.input.as_ref().clone()
    }
}
