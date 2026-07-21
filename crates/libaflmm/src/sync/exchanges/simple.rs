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
pub enum SimpleCommand<IH> {
    Shutdown,
    Import {
        source: GroupId,
        id: TestcaseId,
        handle: IH,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SimpleNotification<IH> {
    NewTestcase { id: TestcaseId, handle: IH },
}

pub struct SimpleExchange<IH> {
    phantom: PhantomData<IH>,
}

impl<D, IH> Exchange<D> for SimpleExchange<IH>
where
    D: Descriptor,
    IH: Clone,
{
    type Command = SimpleCommand<IH>;
    type Notification = SimpleNotification<IH>;

    fn notif_to_command(
        &mut self,
        source: &D,
        notif: &Self::Notification,
    ) -> Result<Option<Self::Command>> {
        Ok(match notif {
            SimpleNotification::NewTestcase { id, handle } => Some(SimpleCommand::Import {
                source: source.group_id(),
                handle: handle.clone(),
                id: *id,
            }),
        })
    }
}
