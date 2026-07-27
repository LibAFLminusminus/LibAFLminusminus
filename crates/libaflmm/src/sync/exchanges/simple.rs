use std::fmt::Debug;

use crate::{
    Result,
    controllers::Descriptor,
    corpus::TestcaseId,
    sync::{GroupId, exchanges::Exchange},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestcaseHandle<H> {
    Corpus(H),
    Objective(H),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SimpleCommand<H> {
    Shutdown,
    Import {
        source: GroupId,
        id: TestcaseId,
        handle: H,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SimpleNotification<H> {
    NewTestcase { id: TestcaseId, handle: H },
}

#[derive(Debug, Default)]
pub struct SimpleExchange;

impl<D, H> Exchange<D, H> for SimpleExchange
where
    D: Descriptor,
    H: Clone + Debug + Serialize + DeserializeOwned,
{
    type Command = SimpleCommand<H>;
    type Notification = SimpleNotification<H>;

    fn notif_to_command(
        &mut self,
        source: &D,
        notif: Self::Notification,
    ) -> Result<Option<Self::Command>> {
        Ok(match notif {
            SimpleNotification::NewTestcase { id, handle } => Some(SimpleCommand::Import {
                source: source.group_id(),
                handle,
                id,
            }),
        })
    }
}
