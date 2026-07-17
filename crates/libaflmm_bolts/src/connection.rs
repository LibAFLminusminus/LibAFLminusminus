use crate::Result;
use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout, poll},
    sys::socket::{AddressFamily, MsgFlags, SockFlag, SockType, recv, send, socketpair},
};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    marker::PhantomData,
    os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd},
};

#[derive(Debug)]
pub struct Connection<In, Out> {
    fd: OwnedFd,
    phantom: PhantomData<(In, Out)>,
    in_msgs: Vec<In>,
    buf: Vec<u8>,
}

impl<In, Out> Connection<In, Out>
where
    In: Serialize + DeserializeOwned,
    Out: Serialize + DeserializeOwned,
{
    /// create connectionA and connectionB, connected together.
    pub fn create() -> Result<(Self, Connection<Out, In>)> {
        let (in_out, out_in) = socketpair(
            AddressFamily::Unix,
            SockType::SeqPacket,
            None,
            SockFlag::SOCK_NONBLOCK | SockFlag::SOCK_CLOEXEC,
        )?;

        Ok((Self::from_fd(in_out), Connection::from_fd(out_in)))
    }

    fn from_fd(fd: OwnedFd) -> Self {
        Self {
            fd,
            in_msgs: Vec::new(),
            buf: Vec::new(),
            phantom: PhantomData,
        }
    }

    /// Send a message to the other end of the wire
    ///
    /// Non-blocking, returns asap.
    ///
    /// If false is returned, it means the send would be actually blocking otherwise
    /// It usually means the socket is full.
    pub fn send(&mut self, msg: &Out) -> Result<bool> {
        let serialized = postcard::to_allocvec(msg)?;
        match send(self.fd.as_raw_fd(), &serialized, MsgFlags::MSG_DONTWAIT) {
            Ok(_) => Ok(true),
            Err(Errno::EAGAIN) => Ok(false), // because of EWOULDBLOCK
            Err(e) => Err(e.into()),
        }
    }

    /// same as `send`, but blocks until the message is actually sent.
    pub fn send_blocking(&mut self, msg: &Out) -> Result<()> {
        let serialized = postcard::to_allocvec(msg)?;
        loop {
            match send(self.fd.as_raw_fd(), &serialized, MsgFlags::empty()) {
                Ok(_) => return Ok(()),
                Err(Errno::EAGAIN) => {
                    let mut fds = [PollFd::new(self.fd.as_fd(), PollFlags::POLLOUT)];
                    poll(&mut fds, PollTimeout::NONE)?;
                }
                Err(Errno::EINTR) => {}
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// poll for messages from the other end of the wire.
    /// this is non-blocking.
    pub fn poll(&mut self) -> Result<impl Iterator<Item = In>> {
        assert!(self.in_msgs.is_empty()); // there should not be remaining In messages at this point

        loop {
            // peek size for now. a bit inefficient, but it should be fine.
            let packet_len = recv(
                self.fd.as_raw_fd(),
                &mut [],
                MsgFlags::MSG_PEEK | MsgFlags::MSG_TRUNC | MsgFlags::MSG_DONTWAIT,
            )?;

            if packet_len == 0 {
                break;
            }

            if self.buf.len() < packet_len {
                self.buf.resize(packet_len, 0);
            }

            let real_len = recv(
                self.fd.as_raw_fd(),
                &mut self.buf[..packet_len],
                MsgFlags::MSG_DONTWAIT,
            )?;

            assert!(real_len == packet_len);

            self.in_msgs
                .push(postcard::from_bytes(&self.buf[..real_len])?);
        }

        Ok(self.in_msgs.drain(..).into_iter())
    }

    /// Get the borrowed underlying file descriptor.
    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}
