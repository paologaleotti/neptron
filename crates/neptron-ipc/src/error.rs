use serde::{Deserialize, Serialize};

/// The error a server sends in place of a reply.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, thiserror::Error)]
#[error("{code}: {message}")]
pub struct ErrorBody {
    /// What went wrong, for programs.
    pub code: ErrorCode,
    /// What went wrong, for people, with the value at fault.
    pub message: String,
    /// The oldest protocol version the server speaks. Only an
    /// `unsupported_version` error carries it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_version: Option<u32>,
    /// The newest protocol version the server speaks. Only an
    /// `unsupported_version` error carries it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_version: Option<u32>,
}

/// The kind of an error response. A client acts on the code, not on the
/// message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The line is not JSON: a syntax error, bytes that are not UTF-8, or an
    /// empty line.
    Malformed,
    /// The line is longer than the server reads. The server closes the
    /// connection after this error.
    LineTooLong,
    /// The line is JSON, but not a request object: a key is missing, has the
    /// wrong type, or is not known.
    InvalidRequest,
    /// The request asks for a protocol version that the server does not
    /// speak.
    UnsupportedVersion,
    /// The `cmd` key names no command that the server knows.
    UnknownCommand,
    /// The `args` of a known command are wrong: a missing key, a key that is
    /// not known, or a value of the wrong type or range.
    InvalidArguments,
    /// The server knows the command but cannot serve it in this build or in
    /// this state.
    Unavailable,
    /// A wait passed its timeout.
    Timeout,
    /// The server tried the command and it failed.
    Failed,
    /// A code that a later protocol version added and this client does not
    /// know. A server never sends it; a client reads it so that an error of
    /// a newer server still arrives as an error.
    #[serde(other)]
    Unknown,
}

impl ErrorBody {
    /// An error with a code and a message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            min_version: None,
            max_version: None,
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl ErrorCode {
    /// The wire name of the code.
    pub fn name(self) -> &'static str {
        match self {
            Self::Malformed => "malformed",
            Self::LineTooLong => "line_too_long",
            Self::InvalidRequest => "invalid_request",
            Self::UnsupportedVersion => "unsupported_version",
            Self::UnknownCommand => "unknown_command",
            Self::InvalidArguments => "invalid_arguments",
            Self::Unavailable => "unavailable",
            Self::Timeout => "timeout",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}
