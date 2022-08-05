use crate::protocol;
use nix::sys;
use std::io::{Read, Write};
use std::os::unix::{io::RawFd, net::UnixStream, prelude::AsRawFd};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("i/o error: {0}")]
    Nix(#[from] nix::errno::Errno),
    #[error("i/o error: {0}")]
    IO(#[from] std::io::Error),
    #[error("incomplete write")]
    IncompleteWrite,
}

pub struct Client {
    pub request: UnixStream,
    pub response: UnixStream,
    pub _signal: UnixStream,
    pub _stats: UnixStream,
}

fn send_fd<T: AsRawFd>(socket: RawFd, fd: T) -> Result<(), Error> {
    let fd = [fd.as_raw_fd()];
    let cmsg = sys::socket::ControlMessage::ScmRights(&fd);
    let iov = std::io::IoSlice::new(&[0]);

    sys::socket::sendmsg::<()>(
        socket,
        &[iov],
        &[cmsg],
        sys::socket::MsgFlags::empty(),
        None,
    )?;

    Ok(())
}

fn send_string<T: AsRef<[u8]>>(socket: RawFd, s: T) -> Result<(), Error> {
    let mut buf = [0; 4];
    let iov = protocol::make_string(s.as_ref(), &mut buf);
    sys::socket::sendmsg::<()>(socket, &iov, &[], sys::socket::MsgFlags::empty(), None)?;

    Ok(())
}

impl Client {
    pub fn connect<T: AsRef<str>>(sysname: T) -> Result<Self, Error> {
        let socket_name = protocol::make_socket_name(&sysname);
        let args = protocol::make_args(&sysname);
        let env = protocol::make_env()?;

        let socket = sys::socket::socket(
            sys::socket::AddressFamily::Unix,
            sys::socket::SockType::Stream,
            sys::socket::SockFlag::empty(),
            None,
        )?;

        sys::socket::connect(
            socket,
            #[cfg(target_os = "linux")]
            &sys::socket::UnixAddr::new_abstract(socket_name.as_bytes())?,
            #[cfg(any(test, not(target_os = "linux")))]
            &sys::socket::UnixAddr::new(socket_name.as_bytes())?,
        )?;

        let signal = UnixStream::pair()?;
        let request = UnixStream::pair()?;
        let response = UnixStream::pair()?;
        let stats = UnixStream::pair()?;

        let (signal, to_send) = signal;
        send_fd(socket, to_send)?;

        send_string(socket, args)?;
        send_string(socket, env)?;

        // Send uid = 0
        send_string(socket, "0")?;
        // Send gid = 0
        send_string(socket, "0")?;
        // Send terminal name
        send_string(socket, "")?;

        // Set stateless mode
        sys::socket::send(socket, &[b'c'], sys::socket::MsgFlags::empty())?;

        let (response, to_send) = response;
        send_fd(socket, to_send)?;

        let (request, to_send) = request;
        send_fd(socket, to_send)?;

        let (stats, to_send) = stats;
        send_fd(socket, to_send)?;

        Ok(Self {
            _signal: signal,
            request,
            response,
            _stats: stats,
        })
    }

    pub fn do_request<T: AsRef<str>>(&mut self, request: T) -> Result<Vec<u8>, Error> {
        let request = request.as_ref();
        let mut buf = [0; 4];
        let n = self
            .request
            .write_vectored(&protocol::make_string(request.as_bytes(), &mut buf))?;
        if n < buf.len() + request.len() {
            return Err(Error::IncompleteWrite);
        }

        let mut buf = Vec::new();
        self.response.read_to_end(&mut buf)?;

        Ok(buf)
    }
}
