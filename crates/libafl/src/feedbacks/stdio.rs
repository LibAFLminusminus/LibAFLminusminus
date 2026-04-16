//! Feedback and metatadata for stderr and stdout.

use alloc::{borrow::Cow, string::String};

use libafl_bolts::{
    Named, impl_serdeany,
    tuples::{Handle, Handled, MatchName, MatchNameRef},
};
use serde::{Deserialize, Serialize};

use crate::{
    DependencyResolver, Error, corpus::Testcase, feedbacks::Feedback, observers::{StdErrObserver, StdOutObserver}, state::{HasTestcase, add_named_metadata}
};

/// Metadata for [`StdOutToMetadataFeedback`].
#[derive(Debug, Serialize, Deserialize)]
pub struct StdOutMetadata {
    stdout: String,
}

impl_serdeany!(StdOutMetadata);

/// Nop feedback that annotates stdout in the new testcase. The testcase
/// is never interesting (use with an OR).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StdOutToMetadataFeedback {
    o_ref: Handle<StdOutObserver>,
}

impl DependencyResolver for StdOutToMetadataFeedback {}

impl<I, OT, S> Feedback<I, OT, S> for StdOutToMetadataFeedback
where
    OT: MatchName,
    S: HasTestcase<I>,
{
    #[cfg(feature = "track_hit_feedbacks")]
    fn last_result(&self) -> Result<bool, Error> {
        Ok(false)
    }

    /// Append to the testcase the generated metadata in case of a new corpus item.
    #[inline]
    fn append_metadata(
        &mut self,
        state: &mut S,
        observers: &OT,
        testcase: &mut Testcase<I>,
    ) -> Result<(), Error> {
        let observer = observers
            .get(&self.o_ref)
            .ok_or_else(|| Error::illegal_state("StdOutObserver is missing"))?;
        let buffer = observer
            .output
            .as_ref()
            .ok_or_else(|| Error::illegal_state("StdOutObserver has no stdout"))?;
        let stdout = String::from_utf8_lossy(buffer).into_owned();
        add_named_metadata(
            state.testcase_md_mut(testcase).named_metadata_map_mut(),
            self.name(),
            StdOutMetadata { stdout },
        );
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

/// Metadata for [`StdErrToMetadataFeedback`].
#[derive(Debug, Serialize, Deserialize)]
pub struct StdErrMetadata {
    stderr: String,
}

impl_serdeany!(StdErrMetadata);

/// Nop feedback that annotates stderr in the new testcase. The testcase
/// is never interesting (use with an OR).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StdErrToMetadataFeedback {
    o_ref: Handle<StdErrObserver>,
}

impl DependencyResolver for StdErrToMetadataFeedback {}

impl<I, OT, S> Feedback<I, OT, S> for StdErrToMetadataFeedback
where
    OT: MatchName,
    S: HasTestcase<I>
{
    #[cfg(feature = "track_hit_feedbacks")]
    fn last_result(&self) -> Result<bool, Error> {
        Ok(false)
    }

    /// Append to the testcase the generated metadata in case of a new corpus item.
    #[inline]
    fn append_metadata(
        &mut self,
        state: &mut S,
        observers: &OT,
        testcase: &mut Testcase<I>,
    ) -> Result<(), Error> {
        let observer = observers
            .get(&self.o_ref)
            .ok_or_else(|| Error::illegal_state("StdErrObserver is missing"))?;
        let buffer = observer
            .output
            .as_ref()
            .ok_or_else(|| Error::illegal_state("StdErrObserver has no stderr"))?;
        let stderr = String::from_utf8_lossy(buffer).into_owned();
        add_named_metadata(
            state.testcase_md_mut(testcase).named_metadata_map_mut(),
            self.name(),
            StdErrMetadata { stderr },
        );

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
