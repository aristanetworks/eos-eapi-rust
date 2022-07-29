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
    request: UnixStream,
    response: UnixStream,
    _signal: UnixStream,
    _stats: UnixStream,
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

fn make_string_iov<'a>(s: &'a [u8], buf: &'a mut [u8; 4]) -> [std::io::IoSlice<'a>; 2] {
    buf.clone_from(&(s.len() as i32).to_le_bytes());
    [std::io::IoSlice::new(buf), std::io::IoSlice::new(s)]
}

fn send_string<T: AsRef<[u8]>>(socket: RawFd, s: T) -> Result<(), Error> {
    let mut buf = [0; 4];
    let iov = make_string_iov(s.as_ref(), &mut buf);
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
            &sys::socket::UnixAddr::new_abstract(socket_name.as_bytes())?,
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
            .write_vectored(&make_string_iov(request.as_bytes(), &mut buf))?;
        if n < buf.len() + request.len() {
            return Err(Error::IncompleteWrite);
        }

        let mut buf = Vec::new();
        self.response.read_to_end(&mut buf)?;

        Ok(buf)
    }
}
