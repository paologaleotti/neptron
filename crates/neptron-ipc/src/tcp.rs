use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use crate::transport::{Client, ClientError};

/// The control API on a TCP port. It exists only with the `control`
/// feature, because it opens the device to the network. It reaches the
/// frontend in a QEMU guest through a forwarded port.
#[derive(Debug)]
pub struct TcpControlListener {
    listener: TcpListener,
}

impl TcpControlListener {
    /// Binds the control API to `addr`. Port 0 picks a free port.
    pub fn bind(addr: SocketAddr) -> io::Result<Self> {
        TcpListener::bind(addr).map(|listener| Self { listener })
    }

    /// Waits for the next client.
    pub fn accept(&self) -> io::Result<TcpStream> {
        self.listener.accept().map(|(stream, _)| stream)
    }

    /// The address the listener is bound to.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// The listener, for example to make it non-blocking.
    pub fn listener(&self) -> &TcpListener {
        &self.listener
    }
}

impl Client<TcpStream> {
    /// Connects to the control API on a TCP port.
    pub fn connect_tcp(addr: SocketAddr) -> Result<Self, ClientError> {
        let stream = TcpStream::connect(addr).map_err(|source| ClientError::Connect {
            target: addr.to_string(),
            source,
        })?;
        Ok(Self::from_stream(stream))
    }

    /// Sets the read and the write timeout of the socket. `None` waits
    /// forever.
    pub fn set_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.stream().set_read_timeout(timeout)?;
        self.stream().set_write_timeout(timeout)
    }
}
