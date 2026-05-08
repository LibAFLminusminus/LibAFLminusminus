use std::borrow::Cow;

use libafl::{
    corpus::TestcaseId,
    executors::ExitKind,
    feedbacks::Feedback,
    DependencyResolver, Error,
};
use libafl_bolts::{Named, SerdeAny};
use serde::{Deserialize, Serialize};

use crate::input::PacketData;

#[derive(Debug, SerdeAny, Serialize, Deserialize)]
pub struct PacketLenMetadata {
    pub length: u64,
}

pub struct PacketLenTestcasePenalty {}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct PacketLenFeedback {
    len: u64,
}

impl DependencyResolver for PacketLenFeedback {}

impl<OT, S> Feedback<PacketData, OT, S> for PacketLenFeedback {
    #[inline]
    fn is_interesting(
        &mut self,
        _state: &mut S,
        input: &PacketData,
        _observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool, Error> {
        self.len = input.length;
        Ok(false)
    }

    #[inline]
    fn append_metadata(
        &mut self,
        _state: &mut S,
        _observers: &OT,
        testcase_id: &TestcaseId,
    ) -> Result<(), Error> {
        testcase
            .metadata_map_mut()
            .insert(PacketLenMetadata { length: self.len });
        Ok(())
    }
}

impl Named for PacketLenFeedback {
    #[inline]
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("PacketLenFeedback");
        &NAME
    }
}

impl PacketLenFeedback {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
