//! The [`Testcase`] is a struct embedded in each [`Corpus`](crate::corpus::Corpus).
//! It will contain a respective input, and metadata.

use alloc::{rc::Rc, string::String};
use core::{borrow::Borrow, fmt::Debug, hash::Hasher};

use libaflmm_bolts::{HasLen, hasher_std};
use serde::{Deserialize, Serialize};

use crate::inputs::Input;

/// Indicates how a [`Testcase`] should be named on-disk.
#[derive(Default, Clone, Serialize, Deserialize, Debug)]
pub enum TestcaseFilenameFormat {
    /// Use the unique [`Testcase`] ID as a name.
    #[default]
    Id,
    /// Use a prefix before the id
    Prefix(String),
}

/// A [`Testcase`] identifier.
///
/// It falls back to a 64-bits integer, so its use is very lightweight.
/// Prefer using this over storing a whole [`Testcase`].
#[derive(Serialize, Deserialize, Hash, Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct TestcaseId(pub u64);

impl TestcaseId {
    /// Default file name for a [`Testcase`].
    #[must_use]
    pub fn default_filename(&self) -> String {
        format!("{:016x}", self.0)
    }
}

impl<I> Borrow<TestcaseId> for Testcase<I> {
    fn borrow(&self) -> &TestcaseId {
        &self.id
    }
}

/// An entry in the [`Testcase`] Corpus
#[derive(Serialize, Deserialize, Debug)]
pub struct Testcase<I> {
    /// The [`Input`] of this [`Testcase`], or `None`, if it is not currently in memory
    input: Rc<I>,
    /// The unique id for [`Testcase`].
    /// It should uniquely identify the input.
    id: TestcaseId,
}

impl TestcaseFilenameFormat {
    /// Get the actual file name as a [`String`].
    #[must_use]
    pub fn to_filename(&self, id: &TestcaseId) -> String {
        match self {
            TestcaseFilenameFormat::Id => id.default_filename(),
            TestcaseFilenameFormat::Prefix(prefix) => {
                format!("{prefix}-{id}")
            }
        }
    }
}

impl<I> Clone for Testcase<I> {
    fn clone(&self) -> Self {
        Self {
            input: self.input.clone(),
            id: self.id,
        }
    }
}

impl<I> Testcase<I> {
    /// Get the input
    #[inline]
    #[must_use]
    pub fn input(&self) -> Rc<I> {
        self.input.clone()
    }

    /// Get the associated unique ID.
    #[must_use]
    pub fn id(&self) -> &TestcaseId {
        &self.id
    }
}

impl<I> Testcase<I>
where
    I: HasLen,
{
    /// Get the input length
    #[must_use]
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
    pub fn compute_id(input: &I) -> TestcaseId {
        let mut hasher = hasher_std();
        input.hash(&mut hasher);
        let hash = hasher.finish();
        TestcaseId(hash)
    }
}

impl<I> Testcase<I>
where
    I: Clone,
{
    /// Clone the input embedded in the [`Testcase`].
    #[must_use]
    pub fn cloned_input(&self) -> I {
        self.input.as_ref().clone()
    }
}
