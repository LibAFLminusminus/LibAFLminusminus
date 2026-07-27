use crate::sync::transports::WaitResult;
use crate::sync::{Transferable, WorkerSync};
use crate::{Result, sync::ControllerSync};
use core::time::Duration;
use libaflmm_bolts::Connection;
use libaflmm_bolts::connection::SendResult;
use libaflmm_core::{WorkerId, illegal_argument, runtime};
use nix::errno::Errno;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use std::collections::HashMap;

#[derive(Debug)]
pub struct SocketWorkerSync<CMD, NOTIF> {
    conn: Connection<CMD, NOTIF>,
}

#[derive(Debug)]
pub struct SocketControllerSync<CMD, NOTIF> {
    workers: HashMap<WorkerId, Connection<NOTIF, CMD>>,
    pending_notifs: Vec<(NOTIF, WorkerId)>,
}

impl<CMD, NOTIF> WorkerSync<CMD, NOTIF> for SocketWorkerSync<CMD, NOTIF>
where
    CMD: Transferable,
    NOTIF: Transferable,
{
    fn send(&mut self, notif: NOTIF) -> Result<()> {
        match self.conn.send(&notif)? {
            SendResult::Sent => Ok(()),
            SendResult::Full => Err(runtime!("The send socket is full")),
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
    fn send<'a>(
        &mut self,
        workers: impl Iterator<Item = &'a libaflmm_core::WorkerId>,
        cmd: CMD,
    ) -> Result<()> {
        let serialized = Connection::<NOTIF, CMD>::serialize_msg(&cmd)?;

        for worker in workers {
            let res = unsafe {
                self.workers
                    .get_mut(worker)
                    .ok_or(illegal_argument!("unknown worker {worker:?}"))?
                    .send_serialized(&serialized)?
            };

            match res {
                SendResult::Sent => {}

                SendResult::Closed => {
                    log::info!("worker {worker:?} socket closed");
                    self.workers.remove(worker);
                }

                SendResult::Full => {
                    return Err(runtime!("worker {worker:?} socket is full"));
                }
            }
        }

        Ok(())
    }

    fn wait(&mut self, timeout: Duration) -> Result<WaitResult> {
        let mut fds: Vec<PollFd> = self
            .workers
            .values()
            .map(|conn| PollFd::new(conn.as_fd(), PollFlags::POLLIN))
            .collect();

        match poll(&mut fds, PollTimeout::try_from(timeout).unwrap()) {
            Ok(0) | Err(Errno::EINTR) => Ok(WaitResult::Timeout),
            Ok(_) => Ok(WaitResult::NewMsg),
            Err(e) => Err(e.into()),
        }
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
