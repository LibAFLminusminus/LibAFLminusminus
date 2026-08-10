use core::{fmt::Debug, marker::PhantomData, time::Duration};
use std::os::fd::BorrowedFd;

use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout, poll},
};

use crate::{
    Result,
    controllers::WorkerId,
    sync::{GenericCommand, GenericNotification, Transferable},
};

pub mod handle_backends;
pub use handle_backends::{
    DefaultInputHandleBackendFactory, InputHandleBackend, InputHandleBackendFactory,
    SerializedInputHandleBackend, SerializedInputHandleBackendFactory,
    UnreachableInputHandleBackend, UnreachableInputHandleBackendFactory,
};

pub mod socket;
pub use socket::{DirectTransfer, SocketControllerSync, SocketWorkerSync};

/// The transfer mechanism for commands and notifications
pub trait Transfer<D, H>: Debug {
    /// The custom user type
    type Custom: Transferable;

    /// Controller side of the sync mechanism
    type ControllerSync: ControllerSync<GenericNotification<H, Self::Custom>, GenericCommand<H, Self::Custom>>;
    /// Worker side of the sync mechanism
    type WorkerSync: WorkerSync<GenericCommand<H, Self::Custom>, GenericNotification<H, Self::Custom>>;

    /// Create a new worker synchronizer for the worker using a given descriptor
    fn create_worker_sync<'a>(
        &mut self,
        descriptor: &'a D,
        sources: impl Iterator<Item = &'a D>,
    ) -> Result<Self::WorkerSync>;

    /// Finalize the transfer lifetime with the creation of the controller-side synchronizer
    fn create_controller_sync(&mut self) -> Result<Self::ControllerSync>;
}

/// The worker side of the synchronization mechanism
pub trait WorkerSync<RCV, SD>: Debug {
    /// Send a SD value to the [`ControllerSync`]
    fn send(&mut self, val: SD) -> Result<()>;

    /// Poll for RCV values from the [`ControllerSync`]
    fn poll(&mut self) -> Result<impl Iterator<Item = RCV>>;
}

pub trait ControllerSync<RCV, SD>: Debug {
    /// Send a SD value to the [`WorkerSync`] with the [`WorkerId`]s in `workers`.
    fn send(&mut self, workers: impl Iterator<Item = WorkerId>, value: &SD) -> Result<()>;

    /// Send a SD value to the [`WorkerSync`] with the ID [`WorkerId`].
    fn send_to(&mut self, worker: WorkerId, value: &SD) -> Result<()> {
        self.send([worker].into_iter(), value)
    }

    /// Wait until a message has been received, or `timeout` has been reached.
    /// The return type give the reason why it returned.
    fn wait(&mut self, wake_fds: &[BorrowedFd<'_>], timeout: Duration) -> Result<WaitResult>;

    /// Poll for RCV values from all the the [`WorkerSync`] attached to [`Self`].
    fn poll(&mut self) -> Result<impl Iterator<Item = (RCV, WorkerId)>>;

    /// Signal a worker has exited and should be removed.
    fn remove_worker(&mut self, worker: WorkerId) -> Result<()>;
}

/// Possible results for a wait
pub enum WaitResult {
    /// A new message is ready, poll will surely return at least one output
    Event,
    /// Timeout triggered. poll may or may not return something.
    Timeout,
}

fn wait_readable<'a>(
    fds: impl Iterator<Item = BorrowedFd<'a>>,
    timeout: Duration,
) -> Result<WaitResult> {
    let mut fds: Vec<PollFd> = fds.map(|fd| PollFd::new(fd, PollFlags::POLLIN)).collect();

    match poll(&mut fds, PollTimeout::try_from(timeout).unwrap()) {
        Ok(0) | Err(Errno::EINTR) => Ok(WaitResult::Timeout),
        Ok(_) => Ok(WaitResult::Event),
        Err(e) => Err(e.into()),
    }
}

#[derive(Debug)]
pub struct NopTransfer<U = ()>(PhantomData<U>);

impl<U> Default for NopTransfer<U> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

#[derive(Debug, Default)]
pub struct NopControllerSync;

#[derive(Debug, Default)]
pub struct NopWorkerSync;

impl<D, H, U> Transfer<D, H> for NopTransfer<U>
where
    U: Transferable,
{
    type Custom = U;
    type ControllerSync = NopControllerSync;
    type WorkerSync = NopWorkerSync;

    fn create_worker_sync<'a>(
        &mut self,
        _descriptor: &'a D,
        _sources: impl Iterator<Item = &'a D>,
    ) -> Result<Self::WorkerSync> {
        Ok(NopWorkerSync)
    }

    fn create_controller_sync(&mut self) -> Result<Self::ControllerSync> {
        Ok(NopControllerSync)
    }
}

impl<RCV, SD> ControllerSync<RCV, SD> for NopControllerSync {
    fn send(&mut self, _workers: impl Iterator<Item = WorkerId>, _val: &SD) -> Result<()> {
        Ok(())
    }

    fn remove_worker(&mut self, _worker: WorkerId) -> Result<()> {
        Ok(())
    }

    fn wait(&mut self, wake_fds: &[BorrowedFd<'_>], timeout: Duration) -> Result<WaitResult> {
        wait_readable(wake_fds.iter().copied(), timeout)
    }

    fn poll(&mut self) -> Result<impl Iterator<Item = (RCV, WorkerId)>> {
        Ok([].into_iter())
    }
}

impl<RCV, SD> WorkerSync<RCV, SD> for NopWorkerSync {
    fn send(&mut self, _val: SD) -> Result<()> {
        Ok(())
    }

    fn poll(&mut self) -> Result<impl Iterator<Item = RCV>> {
        Ok([].into_iter())
    }
}
