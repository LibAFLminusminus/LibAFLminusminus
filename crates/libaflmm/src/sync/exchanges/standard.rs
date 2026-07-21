use crate::{
    Result,
    controllers::Descriptor,
    corpus::TestcaseId,
    sync::{GroupId, exchanges::Exchange},
};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestcaseHandle<IH> {
    Corpus(IH),
    Objective(IH),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StdCommand<IH> {
    Shutdown,
    Import {
        source: GroupId,
        id: TestcaseId,
        handle: IH,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StdNotification<IH> {
    NewTestcase { id: TestcaseId, handle: IH },
}

pub struct StdExchange<IH> {
    phantom: PhantomData<IH>,
}

impl<D, IH> Exchange<D> for StdExchange<IH>
where
    D: Descriptor,
{
    type Command = StdCommand<IH>;
    type Notification = StdNotification<IH>;

    fn notif_to_command(
        &mut self,
        source: &D,
        notif: &Self::Notification,
    ) -> Result<Option<Self::Command>> {
        Ok(match notif {
            StdNotification::NewTestcase { id, handle } => Some(StdCommand::Import {
                source: source.group_idf(),
                handle,
                id,
            }),
            _ => None,
        })
    }
}
