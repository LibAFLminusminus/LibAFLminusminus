use std::fmt::Debug;

use crate::{Result, sync::Transferable};
use serde::{Deserialize, Serialize};

pub mod simple;
pub use simple::{SimpleCommand, SimpleExchange, SimpleNotification};

pub type StdExchange = SimpleExchange;
pub type StdCommand<H> = SimpleCommand<H>;
pub type StdNotification<H> = SimpleNotification<H>;

/// An exchange system between [`Self::Command`] (produced on the controller side)
/// and [`Self::Notification`] (produced on the worker side).
pub trait Exchange<D, H>: Debug {
    type Command: Transferable;
    type Notification: Transferable;

    /// Optionally converts a [`Self::Notification`] into a [`Self::Command`].
    /// Every notification received will go through this function.
    /// If a command is produced, it will be routed through the [`Self::Router`].
    fn notif_to_command(
        &mut self,
        source: &D,
        notif: Self::Notification,
    ) -> Result<Option<Self::Command>>;
}

#[derive(Debug)]
pub struct NopExchange;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NopCommand;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NopNotification;

impl<D, H> Exchange<D, H> for NopExchange {
    type Command = NopCommand;
    type Notification = NopNotification;

    fn notif_to_command(
        &mut self,
        _source: &D,
        _notif: Self::Notification,
    ) -> Result<Option<Self::Command>> {
        Ok(None)
    }
}
