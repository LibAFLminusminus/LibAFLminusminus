use crate::Result;
use core::{fmt::Debug, marker::PhantomData};
use nix::{
    errno::Errno,
    sys::socket::{
        AddressFamily, MsgFlags, SockFlag, SockType, getsockopt, recv, send, setsockopt,
        socketpair, sockopt,
    },
};
use serde::{Serialize, de::DeserializeOwned};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};

/// shortcut for transportable messages over the wire
/// it is auto-impl for T enforcing these sub traits.
pub trait Transferable: Debug + Serialize + DeserializeOwned {}

#[derive(Debug)]
pub struct Connection<In, Out> {
    fd: OwnedFd,
    in_msgs: Vec<In>,
    buf: Vec<u8>,
    phantom: PhantomData<(In, Out)>,
}

/// Possible outcomes of a send
pub enum SendResult {
    /// Message sent successfully
    Sent,
    /// Pipe is full, try again later
    Full,
    /// Pipe has been closed
    Closed,
}

impl<T> Transferable for T where T: Debug + Serialize + DeserializeOwned {}

impl<In, Out> Connection<In, Out>
where
    In: Transferable,
    Out: Transferable,
{
    /// create connectionA and connectionB, connected together.
    pub fn create(buf_max_bytes: Option<usize>) -> Result<(Self, Connection<Out, In>)> {
        let (in_out, out_in) = socketpair(
            AddressFamily::Unix,
            SockType::SeqPacket,
            None,
            SockFlag::SOCK_NONBLOCK | SockFlag::SOCK_CLOEXEC,
        )?;

        if let Some(max_bytes) = buf_max_bytes {
            setsockopt(&in_out, sockopt::SndBuf, &max_bytes)?;
            setsockopt(&out_in, sockopt::SndBuf, &max_bytes)?;
        }

        Ok((Self::from_fd(in_out), Connection::from_fd(out_in)))
    }

    pub fn send_buf_bytes(&self) -> Result<usize> {
        Ok(getsockopt(&self.as_fd(), sockopt::SndBuf)?)
    }

    fn from_fd(fd: OwnedFd) -> Self {
        Self {
            fd,
            in_msgs: Vec::new(),
            buf: Vec::new(),
            phantom: PhantomData,
        }
    }

    pub fn serialize_msg(msg: &Out) -> Result<Vec<u8>> {
        Ok(postcard::to_allocvec(msg)?)
    }

    /// Send a message that has already been serialized
    ///
    /// # Safety
    ///
    /// The serialized msg must be the result of [`Self::serialize_msg`] for the same connection type.
    /// So, calling [`Self::serialize_msg`] with `Connection<In1, Out1>` and [`Self::send_serialized`] with `Connection<In2, Out2>` is undefined behaviour.
    pub unsafe fn send_serialized(
        &mut self,
        serialized_msg: impl AsRef<[u8]>,
    ) -> Result<SendResult> {
        match send(
            self.fd.as_raw_fd(),
            serialized_msg.as_ref(),
            MsgFlags::MSG_DONTWAIT | MsgFlags::MSG_NOSIGNAL,
        ) {
            Ok(_) => Ok(SendResult::Sent),
            Err(Errno::EPIPE) => Ok(SendResult::Closed),
            Err(Errno::EAGAIN) => Ok(SendResult::Full), // because of EWOULDBLOCK
            Err(e) => Err(e.into()),
        }
    }

    /// Send a message to the other end of the wire
    ///
    /// Non-blocking, returns asap.
    ///
    /// If false is returned, it means the send would be actually blocking otherwise
    /// It usually means the socket is full.
    pub fn send(&mut self, msg: &Out) -> Result<SendResult> {
        let serialized = Self::serialize_msg(msg)?;
        unsafe { self.send_serialized(serialized) }
    }

    /// poll for messages from the other end of the wire.
    /// this is non-blocking.
    pub fn poll(&mut self) -> Result<impl Iterator<Item = In>> {
        assert!(self.in_msgs.is_empty()); // there should not be remaining In messages at this point

        loop {
            // peek size for now. a bit inefficient, but it should be fine.
            let packet_len = match recv(
                self.fd.as_raw_fd(),
                &mut [],
                MsgFlags::MSG_PEEK | MsgFlags::MSG_TRUNC | MsgFlags::MSG_DONTWAIT,
            ) {
                Ok(len) => len,
                Err(Errno::EAGAIN) => break,
                Err(Errno::EINTR) => continue,
                Err(e) => return Err(e.into()),
            };

            if packet_len == 0 {
                // EOF
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

            debug_assert_eq!(real_len, packet_len);

            self.in_msgs
                .push(postcard::from_bytes(&self.buf[..real_len])?);
        }

        Ok(self.in_msgs.drain(..))
    }

    /// Get the borrowed underlying file descriptor.
    #[must_use]
    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}
