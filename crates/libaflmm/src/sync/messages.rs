use crate::{Result, corpus::TestcaseId, sync::GroupId};
use serde::{Deserialize, Serialize};

/// the standard command
pub type StdCommand = GenericCommand<(), ()>;

/// the standard notification
pub type StdNotification = GenericNotification<(), ()>;

/// A command, going from the controller to worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GenericCommand<H, U = ()> {
    Shutdown,
    Import {
        source: GroupId,
        id: TestcaseId,
        handle: H,
    },
    Custom(U),
}

/// A notification, going from the worker to the controller
#[derive(Debug, Serialize, Deserialize)]
pub enum GenericNotification<H, U = ()> {
    NewTestcase { id: TestcaseId, handle: H },
    Custom(U),
}

impl<H, U> GenericNotification<H, U> {
    /// Convert a notification coming from the group `source` into the command to route.
    /// Returns [`None`] if the notification should not be routed at all.
    pub fn into_command(self, source: GroupId) -> Result<Option<GenericCommand<H, U>>> {
        Ok(Some(match self {
            Self::NewTestcase { id, handle } => GenericCommand::Import { source, id, handle },
            Self::Custom(payload) => GenericCommand::Custom(payload),
        }))
    }
}
