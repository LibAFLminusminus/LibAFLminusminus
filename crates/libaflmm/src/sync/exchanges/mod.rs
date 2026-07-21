use std::marker::PhantomData;

use crate::Result;
use serde::{Deserialize, Serialize};

pub mod simple;
pub use simple::{SimpleCommand, SimpleExchange, SimpleNotification};

pub type StdExchange = SimpleExchange;
pub type StdCommand<IH> = SimpleCommand<IH>;
pub type StdNotification<IH> = SimpleNotification<IH>;

/// An exchange system between [`Self::Command`] (produced on the controller side)
/// and [`Self::Notification`] (produced on the worker side).
pub trait Exchange<D, IH> {
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
pub struct NopCommand<IH>(PhantomData<IH>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NopNotification<IH>(PhantomData<IH>);

impl<D, IH> Exchange<D, IH> for NopExchange {
    type Command = NopCommand<IH>;
    type Notification = NopNotification<IH>;

    fn notif_to_command(
        &mut self,
        _source: &D,
        _notif: &Self::Notification,
    ) -> Result<Option<Self::Command>> {
        Ok(None)
    }
}
