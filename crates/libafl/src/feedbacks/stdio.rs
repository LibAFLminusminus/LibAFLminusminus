//! Feedback and metatadata for stderr and stdout.

use alloc::{borrow::Cow, string::String};

use hashbrown::HashMap;
use libafl_bolts::{
    Named, impl_serdeany,
    tuples::{Handle, Handled, MatchName, MatchNameRef},
};
use serde::{Deserialize, Serialize};

use crate::{
    Error,
    common::DependencyResolver,
    corpus::{Testcase, TestcaseId},
    feedbacks::Feedback,
    observers::{StdErrObserver, StdOutObserver},
    states::{FlatState, HasTestcase},
};

/// Metadata for [`StdOutToMetadataFeedback`].
#[derive(Debug, Serialize, Deserialize)]
pub struct StdOutMetadata {
    stdout: HashMap<TestcaseId, String>,
}

/// Nop feedback that annotates stdout in the new testcase. The testcase
/// is never interesting (use with an OR).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StdOutToMetadataFeedback {
    o_ref: Handle<StdOutObserver>,
}

/// Metadata for [`StdErrToMetadataFeedback`].
#[derive(Debug, Serialize, Deserialize)]
pub struct StdErrMetadata {
    stderr: HashMap<TestcaseId, String>,
}

/// Nop feedback that annotates stderr in the new testcase. The testcase
/// is never interesting (use with an OR).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StdErrToMetadataFeedback {
    o_ref: Handle<StdErrObserver>,
}

impl_serdeany!(StdOutMetadata);

impl DependencyResolver for StdOutToMetadataFeedback {}

impl<I, OT, S> Feedback<I, OT, S> for StdOutToMetadataFeedback
where
    OT: MatchName,
    S: HasTestcase<I> + FlatState,
{
    /// Append to the testcase the generated metadata in case of a new corpus item.
    #[inline]
    fn append_metadata(
        &mut self,
        state: &mut S,
        observers: &OT,
        testcase_id: &TestcaseId,
    ) -> Result<(), Error> {
        let observer = observers
            .get(&self.o_ref)
            .ok_or_else(|| Error::illegal_state("StdOutObserver is missing"))?;
        let buffer = observer
            .output
            .as_ref()
            .ok_or_else(|| Error::illegal_state("StdOutObserver has no stdout"))?;
        let stdout = String::from_utf8_lossy(buffer).into_owned();
        state
            .named_metadata_map_mut()
            .get_mut::<StdOutMetadata>(&self.name())
            .unwrap()
            .stdout
            .insert(*testcase_id, stdout);
        Ok(())
    }
}

impl Named for StdOutToMetadataFeedback {
    #[inline]
    fn name(&self) -> &Cow<'static, str> {
        self.o_ref.name()
    }
}

impl StdOutToMetadataFeedback {
    /// Creates a new [`StdOutToMetadataFeedback`].
    #[must_use]
    pub fn new(observer: &StdOutObserver) -> Self {
        Self {
            o_ref: observer.handle(),
        }
    }
}

impl_serdeany!(StdErrMetadata);

impl DependencyResolver for StdErrToMetadataFeedback {}

impl<I, OT, S> Feedback<I, OT, S> for StdErrToMetadataFeedback
where
    OT: MatchName,
    S: HasTestcase<I> + FlatState,
{
    /// Append to the testcase the generated metadata in case of a new corpus item.
    #[inline]
    fn append_metadata(
        &mut self,
        state: &mut S,
        observers: &OT,
        testcase_id: &TestcaseId,
    ) -> Result<(), Error> {
        let observer = observers
            .get(&self.o_ref)
            .ok_or_else(|| Error::illegal_state("StdErrObserver is missing"))?;
        let buffer = observer
            .output
            .as_ref()
            .ok_or_else(|| Error::illegal_state("StdErrObserver has no stderr"))?;
        let stderr = String::from_utf8_lossy(buffer).into_owned();
        state
            .named_metadata_map_mut()
            .get_mut::<StdErrMetadata>(&self.name())
            .unwrap()
            .stderr
            .insert(*testcase_id, stderr);

        Ok(())
    }
}

impl Named for StdErrToMetadataFeedback {
    #[inline]
    fn name(&self) -> &Cow<'static, str> {
        self.o_ref.name()
    }
}

impl StdErrToMetadataFeedback {
    /// Creates a new [`StdErrToMetadataFeedback`].
    #[must_use]
    pub fn new(observer: &StdErrObserver) -> Self {
        Self {
            o_ref: observer.handle(),
        }
    }
}
