//! The [`Testcase`] is a struct embedded in each [`Corpus`].
//! It will contain a respective input, and metadata.

#[cfg(feature = "track_hit_feedbacks")]
use alloc::{borrow::Cow, vec::Vec};
use alloc::{rc::Rc, string::String};
use core::{borrow::Borrow, fmt::Debug, hash::Hasher};
use std::string::ToString;

use libafl_bolts::{HasLen, hasher_std};
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
    /// Use a custom name.
    Custom(String),
}

#[derive(Serialize, Deserialize, Hash, Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct TestcaseId(pub u64);

impl TestcaseId {
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

    filename_fmt: TestcaseFilenameFormat,

    /// The unique id for [`Testcase`].
    /// It should uniquely identify the input.
    id: TestcaseId,
}

impl TestcaseFilenameFormat {
    pub fn to_filename(&self, id: &TestcaseId) -> String {
        match self {
            TestcaseFilenameFormat::Id => id.default_filename(),
            TestcaseFilenameFormat::Prefix(prefix) => {
                format!("{}-{}", prefix, id)
            }
            TestcaseFilenameFormat::Custom(custom_name) => custom_name.clone(),
        }
    }
}

impl<I> Clone for Testcase<I> {
    fn clone(&self) -> Self {
        Self {
            input: self.input.clone(),
            id: self.id.clone(),
            filename_fmt: self.filename_fmt.clone(),
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
    pub fn id(&self) -> &TestcaseId {
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

        Self {
            input,
            id,
            filename_fmt: TestcaseFilenameFormat::default(),
        }
    }

    pub fn with_filename(input: Rc<I>, filename: String) -> Self {
        let mut tc = Self::new(input);

        tc.filename_fmt = TestcaseFilenameFormat::Custom(filename);

        tc
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
    pub fn cloned_input(&self) -> I {
        self.input.as_ref().clone()
    }
}
