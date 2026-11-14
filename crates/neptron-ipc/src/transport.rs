use std::io::{self, BufReader, Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::command::Command;
use crate::envelope::{
    CodecError, MIN_PROTOCOL_VERSION, PROTOCOL_VERSION, Request, decode_response, encode_request,
};
use crate::error::{ErrorBody, ErrorCode};
use crate::line::{LineRead, MAX_RESPONSE_LINE_BYTES, read_line};
use crate::reply::Reply;
use crate::server::{Handler, ServeError, serve_lines};

/// The path of the control socket when nothing else names one.
pub const DEFAULT_SOCKET_PATH: &str = "/run/neptron/control.sock";

/// The environment variable that names the control socket path.
pub const SOCKET_PATH_ENV: &str = "NEPTRON_CONTROL_SOCKET";

/// The listening control socket. Dropping it removes the socket file, if
/// the path still holds the socket that the listener made.
#[derive(Debug)]
pub struct ControlListener {
    listener: UnixListener,
    path: PathBuf,
    /// Device and inode of the socket file, to know it again at drop.
    file_id: (u64, u64),
}

/// A failure to open the control socket.
#[derive(Debug, thiserror::Error)]
pub enum ListenError {
    /// A live server already listens on the path.
    #[error("a server already listens on {path}")]
    InUse {
        /// The socket path.
        path: PathBuf,
    },
    /// The path holds a file that is not a socket. The listener does not
    /// remove it.
    #[error("{path} exists and is not a socket")]
    NotASocket {
        /// The path.
        path: PathBuf,
    },
    /// An I/O call on the path failed.
    #[error("cannot {action} {path}: {source}")]
    Io {
        /// What the listener tried: "inspect", "connect to", "remove" or
        /// "bind".
        action: &'static str,
        /// The path.
        path: PathBuf,
        /// The error.
        source: io::Error,
    },
}

/// A client of the control API over one stream.
///
/// The client sends one request and reads its response before it sends the
/// next. Each request gets the next id, and the client checks that the
/// response carries the same id and the reply of the same command. After a
/// call fails with an error other than [`ClientError::Server`], the stream
/// can hold part of a response: drop the client and connect again.
#[derive(Debug)]
pub struct Client<S> {
    stream: BufReader<S>,
    next_id: u64,
    /// The protocol version of the requests: [`PROTOCOL_VERSION`] until a
    /// server says it speaks only older ones.
    version: u32,
    max_line_bytes: usize,
    line: Vec<u8>,
}

/// A failure of one call of a [`Client`].
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// The client cannot connect to the server.
    #[error("cannot connect to {target}: {source}")]
    Connect {
        /// The socket path or address.
        target: String,
        /// The error.
        source: io::Error,
    },
    /// No response came before the read timeout.
    #[error("no response within the timeout")]
    Timeout,
    /// The server closed the connection before it answered.
    #[error("the server closed the connection before it answered")]
    Closed,
    /// The response line is longer than the client reads.
    #[error("the response has more than {limit} bytes")]
    TooLong {
        /// The limit, in bytes.
        limit: usize,
    },
    /// Sending the request or reading the response failed.
    #[error("connection failed: {0}")]
    Io(io::Error),
    /// A message did not encode or decode.
    #[error(transparent)]
    Codec(CodecError),
    /// The response answers a different request.
    #[error("sent request {sent_id} ({command}), got a response for {got_id:?} ({reply:?})")]
    Mismatch {
        /// The id of the request.
        sent_id: u64,
        /// The command sent.
        command: &'static str,
        /// The id in the response.
        got_id: Option<u64>,
        /// The reply name in the response, if it has a reply.
        reply: Option<&'static str>,
    },
    /// The server answered with an error.
    #[error("the server refused the request: {0}")]
    Server(ErrorBody),
}

impl ControlListener {
    /// Binds the control socket at `path`.
    ///
    /// A socket file that refuses a connection is a leftover of a crash, and
    /// the listener replaces it. A live server, a file that is not a socket,
    /// or any other connect error (for example no permission on the socket
    /// of another user) makes the call fail, and the file stays.
    pub fn bind(path: &Path) -> Result<Self, ListenError> {
        let io_error = |action, source| ListenError::Io {
            action,
            path: path.to_owned(),
            source,
        };
        match std::fs::symlink_metadata(path) {
            Ok(meta) if !meta.file_type().is_socket() => {
                return Err(ListenError::NotASocket {
                    path: path.to_owned(),
                });
            }
            Ok(_) => match UnixStream::connect(path) {
                Ok(_) => {
                    return Err(ListenError::InUse {
                        path: path.to_owned(),
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
                    // The path can change between the two calls; unlink it
                    // only while it is still a socket.
                    let meta =
                        std::fs::symlink_metadata(path).map_err(|e| io_error("inspect", e))?;
                    if !meta.file_type().is_socket() {
                        return Err(ListenError::NotASocket {
                            path: path.to_owned(),
                        });
                    }
                    std::fs::remove_file(path).map_err(|e| io_error("remove", e))?;
                }
                Err(e) => return Err(io_error("connect to", e)),
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_error("inspect", e)),
        }
        let listener = UnixListener::bind(path).map_err(|e| io_error("bind", e))?;
        let meta = std::fs::symlink_metadata(path).map_err(|e| io_error("inspect", e))?;
        Ok(Self {
            listener,
            path: path.to_owned(),
            file_id: (meta.dev(), meta.ino()),
        })
    }

    /// Waits for the next client.
    pub fn accept(&self) -> io::Result<UnixStream> {
        self.listener.accept().map(|(stream, _)| stream)
    }

    /// The listener, for example to make it non-blocking.
    pub fn listener(&self) -> &UnixListener {
        &self.listener
    }

    /// The socket path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ControlListener {
    fn drop(&mut self) {
        let ours = std::fs::symlink_metadata(&self.path).is_ok_and(|meta| {
            meta.file_type().is_socket() && (meta.dev(), meta.ino()) == self.file_id
        });
        if ours {
            // A drop has no caller to report a failure to, and the next bind
            // replaces a leftover socket file.
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Serves one connection on `stream` until the peer closes it. See
/// [`serve_lines`].
pub fn serve_stream<S, H>(stream: &S, handler: &mut H) -> Result<(), ServeError>
where
    for<'a> &'a S: Read + Write,
    H: Handler + ?Sized,
{
    let mut reader = BufReader::new(stream);
    let mut writer = stream;
    serve_lines(&mut reader, &mut writer, handler)
}

impl Client<UnixStream> {
    /// Connects to the control socket at `path`.
    pub fn connect(path: &Path) -> Result<Self, ClientError> {
        let stream = UnixStream::connect(path).map_err(|source| ClientError::Connect {
            target: path.display().to_string(),
            source,
        })?;
        Ok(Self::from_stream(stream))
    }

    /// Sets the read and the write timeout of the socket. `None` waits
    /// forever.
    pub fn set_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.stream.get_ref().set_read_timeout(timeout)?;
        self.stream.get_ref().set_write_timeout(timeout)
    }
}

impl<S: Read + Write> Client<S> {
    /// A client over a connected stream.
    pub fn from_stream(stream: S) -> Self {
        Self {
            stream: BufReader::new(stream),
            next_id: 1,
            version: PROTOCOL_VERSION,
            max_line_bytes: MAX_RESPONSE_LINE_BYTES,
            line: Vec::new(),
        }
    }

    /// Sets the longest response line the client reads, in bytes. The
    /// default is [`MAX_RESPONSE_LINE_BYTES`].
    pub fn set_max_line_bytes(&mut self, bytes: usize) {
        self.max_line_bytes = bytes;
    }

    /// The stream.
    pub fn stream(&self) -> &S {
        self.stream.get_ref()
    }

    /// The protocol version the client sends: [`PROTOCOL_VERSION`], or the
    /// newest version of an older server after its `unsupported_version`.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Sends one command and waits for its reply. A server that does not
    /// speak the client's version answers `unsupported_version` with the
    /// range it speaks; the client then sends the command again once, at
    /// the newest version of that range that it speaks too, and keeps that
    /// version for the next calls, as the version discovery of the README
    /// says.
    pub fn call(&mut self, command: Command) -> Result<Reply, ClientError> {
        match self.call_at(self.version, command.clone()) {
            Err(ClientError::Server(error)) => match fallback(self.version, &error) {
                Some(version) => {
                    self.version = version;
                    self.call_at(version, command)
                }
                None => Err(ClientError::Server(error)),
            },
            outcome => outcome,
        }
    }

    fn call_at(&mut self, version: u32, command: Command) -> Result<Reply, ClientError> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let name = command.name();
        let request = encode_request(&Request {
            version,
            id,
            command,
        })
        .map_err(ClientError::Codec)?;
        let writer = self.stream.get_mut();
        writer.write_all(&request).map_err(map_io)?;
        writer.flush().map_err(map_io)?;
        match read_line(&mut self.stream, self.max_line_bytes, &mut self.line).map_err(map_io)? {
            LineRead::Line => {}
            LineRead::End => return Err(ClientError::Closed),
            LineRead::TooLong => {
                return Err(ClientError::TooLong {
                    limit: self.max_line_bytes,
                });
            }
        }
        let response = decode_response(&self.line).map_err(ClientError::Codec)?;
        match response.outcome {
            Err(error) if response.id.is_none() || response.id == Some(id) => {
                Err(ClientError::Server(error))
            }
            Ok(reply) if response.id == Some(id) && reply.name() == name => Ok(reply),
            outcome => Err(ClientError::Mismatch {
                sent_id: id,
                command: name,
                got_id: response.id,
                reply: outcome.ok().map(|r| r.name()),
            }),
        }
    }
}

/// The version to send again after `error` from a request of `version`: the
/// newest version the server speaks, when it is older than `version` and
/// not older than [`MIN_PROTOCOL_VERSION`].
fn fallback(version: u32, error: &ErrorBody) -> Option<u32> {
    if error.code != ErrorCode::UnsupportedVersion {
        return None;
    }
    let newest = error.max_version?;
    (MIN_PROTOCOL_VERSION..version)
        .contains(&newest)
        .then_some(newest)
}

fn map_io(error: io::Error) -> ClientError {
    match error.kind() {
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => ClientError::Timeout,
        _ => ClientError::Io(error),
    }
}
