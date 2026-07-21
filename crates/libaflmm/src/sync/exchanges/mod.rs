use crate::Result;
use serde::{Deserialize, Serialize};

pub mod standard;

/// An exchange system between [`Self::Command`] (produced on the controller side)
/// and [`Self::Notification`] (produced on the worker side).
pub trait Exchange<D> {
    type Command;
    type Notification;

    /// Optionally converts a [`Self::Notification`] into a [`Self::Command`].
    /// Every notification received will go through this function.
    /// If a command is produced, it will be routed through the [`Self::Router`].
    fn notif_to_command(
        &mut self,
        source: &D,
        notif: &Self::Notification,
    ) -> Result<Option<Self::Command>>;
}

pub struct NopExchange;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NopCommand;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NopNotification;

impl<D> Exchange<D> for NopExchange {
    type Command = NopCommand;
    type Notification = NopNotification;

    fn notif_to_command(
        &mut self,
        source: &D,
        notif: &Self::Notification,
    ) -> Result<Option<Self::Command>> {
        Ok(None)
    }
}
