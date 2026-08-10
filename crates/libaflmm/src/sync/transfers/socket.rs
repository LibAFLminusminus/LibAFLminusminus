use core::time::Duration;
use std::{collections::HashMap, os::fd::BorrowedFd};

use libaflmm_bolts::{Connection, connection::SendResult};
use libaflmm_core::{illegal_argument, illegal_state, runtime};

use crate::{
    Result,
    controllers::{Descriptor, WorkerId},
    sync::{
        ControllerSync, GenericCommand, GenericNotification, Transfer, Transferable, WorkerSync,
        transfers::{WaitResult, wait_readable},
    },
};

/// The connections the controller keeps, one per worker.
type ControllerConns<H, U> =
    HashMap<WorkerId, Connection<GenericNotification<H, U>, GenericCommand<H, U>>>;

/// socket-based transfer between controller and workers.
#[derive(Debug)]
pub struct DirectTransfer<H, U = ()> {
    send_buf_bytes: Option<usize>,
    controller_conns: Option<ControllerConns<H, U>>,
}

#[derive(Debug)]
pub struct SocketWorkerSync<CMD, NOTIF> {
    conn: Connection<CMD, NOTIF>,
}

#[derive(Debug)]
pub struct SocketControllerSync<CMD, NOTIF> {
    workers: HashMap<WorkerId, Connection<NOTIF, CMD>>,
    pending_notifs: Vec<(NOTIF, WorkerId)>,
}

impl<H, U> Default for DirectTransfer<H, U> {
    fn default() -> Self {
        Self {
            controller_conns: Some(HashMap::new()),
            send_buf_bytes: None,
        }
    }
}

impl<CMD, NOTIF> WorkerSync<CMD, NOTIF> for SocketWorkerSync<CMD, NOTIF>
where
    CMD: Transferable,
    NOTIF: Transferable,
{
    fn send(&mut self, notif: NOTIF) -> Result<()> {
        match self.conn.send(&notif)? {
            SendResult::Sent => Ok(()),
            SendResult::Full => {
                log::error!(
                    "The send socket is full. Current socket buffer is {} bytes, either increase it or do not transfer the whole input.",
                    self.conn.send_buf_bytes()?
                );
                Ok(())
            }
            SendResult::Closed => Err(runtime!("The send socket is closed")),
        }
    }

    fn poll(&mut self) -> Result<impl Iterator<Item = CMD>> {
        self.conn.poll()
    }
}

impl<CMD, NOTIF> ControllerSync<NOTIF, CMD> for SocketControllerSync<CMD, NOTIF>
where
    CMD: Transferable,
    NOTIF: Transferable,
{
    fn send(&mut self, workers: impl Iterator<Item = WorkerId>, cmd: &CMD) -> Result<()> {
        let serialized = Connection::<NOTIF, CMD>::serialize_msg(cmd)?;

        for worker in workers {
            let res = unsafe {
                self.workers
                    .get_mut(&worker)
                    .ok_or(illegal_argument!("unknown worker {worker:?}"))?
                    .send_serialized(&serialized)?
            };

            match res {
                SendResult::Sent => {}

                SendResult::Closed => {
                    log::info!("worker {worker:?} socket closed");
                    self.workers.remove(&worker);
                }

                SendResult::Full => {
                    return Err(runtime!("worker {worker:?} socket is full"));
                }
            }
        }

        Ok(())
    }

    fn remove_worker(&mut self, worker: WorkerId) -> Result<()> {
        self.workers.remove(&worker);
        Ok(())
    }

    fn wait(&mut self, wake_fds: &[BorrowedFd<'_>], timeout: Duration) -> Result<WaitResult> {
        wait_readable(
            self.workers
                .values()
                .map(|conn| conn.as_fd())
                .chain(wake_fds.iter().copied()),
            timeout,
        )
    }

    fn poll(&mut self) -> Result<impl Iterator<Item = (NOTIF, WorkerId)>> {
        assert!(self.pending_notifs.is_empty());

        for (&worker, conn) in &mut self.workers {
            self.pending_notifs
                .extend(conn.poll()?.map(|notif| (notif, worker)));
        }

        Ok(self.pending_notifs.drain(..))
    }
}

impl<D, H, U> Transfer<D, H> for DirectTransfer<H, U>
where
    D: Descriptor,
    H: Transferable,
    U: Transferable,
{
    type Custom = U;
    type ControllerSync = SocketControllerSync<GenericCommand<H, U>, GenericNotification<H, U>>;
    type WorkerSync = SocketWorkerSync<GenericCommand<H, U>, GenericNotification<H, U>>;

    fn create_worker_sync<'a>(
        &mut self,
        descriptor: &'a D,
        _sources: impl Iterator<Item = &'a D>,
    ) -> Result<Self::WorkerSync> {
        if let Some(conns) = &mut self.controller_conns {
            let (worker_conn, controller_conn) = Connection::create(self.send_buf_bytes)?;

            conns.insert(descriptor.worker_id(), controller_conn);

            Ok(SocketWorkerSync { conn: worker_conn })
        } else {
            Err(illegal_state!(
                "controller sync has already been created, no worker sync can be created anymore."
            ))
        }
    }

    fn create_controller_sync(&mut self) -> Result<Self::ControllerSync> {
        if let Some(workers) = self.controller_conns.take() {
            Ok(SocketControllerSync {
                workers,
                pending_notifs: Vec::new(),
            })
        } else {
            Err(illegal_state!("controller sync has already been created."))
        }
    }
}
