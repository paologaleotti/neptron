use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::command::Command;
use crate::error::{ErrorBody, ErrorCode};
use crate::reply::Reply;

/// The protocol version this crate speaks. Every request and every response
/// carries it in the `v` key.
///
/// The protocol is append-only: a new version adds commands, reply fields or
/// error codes, and never renames or removes one. A server therefore serves
/// every request with a version from 1 up to its own.
pub const PROTOCOL_VERSION: u32 = 1;

/// One request: a command with the id that the response echoes.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// A number the client chooses. The response carries the same id.
    pub id: u64,
    /// The command.
    pub command: Command,
}

/// One response: a reply or an error, for the request with `id`.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    /// The id of the request, or `None` when the server could not read one
    /// from the line.
    pub id: Option<u64>,
    /// The reply, or the error.
    pub outcome: Result<Reply, ErrorBody>,
}

/// A request line that the server refuses, with the id when the line has
/// one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The id of the request, if the server could read it.
    pub id: Option<u64>,
    /// Why the line is refused.
    pub error: ErrorBody,
}

/// A failure to encode or decode a message on the client side, or to encode
/// one on the server side.
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    /// A message did not serialise.
    #[error("cannot encode the {what}: {source}")]
    Encode {
        /// The message: "request" or "response".
        what: &'static str,
        /// The serde error.
        source: serde_json::Error,
    },
    /// A response line is not JSON.
    #[error("the response is not JSON: {0}")]
    Syntax(serde_json::Error),
    /// A response line is JSON but not a response object.
    #[error("the response is not a response object: {0}")]
    Shape(String),
}

/// Encodes a request as one line, with the newline.
pub fn encode_request(request: &Request) -> Result<Vec<u8>, CodecError> {
    let wire = RequestWire {
        v: PROTOCOL_VERSION,
        id: request.id,
        command: &request.command,
    };
    to_line(&wire, "request")
}

/// Decodes one request line, without its newline.
///
/// The checks run in a fixed order, so that each fault gets one code: JSON
/// syntax (`malformed`), the envelope keys (`invalid_request`), the version
/// (`unsupported_version`), the command name (`unknown_command`), and last
/// the arguments (`invalid_arguments`). The envelope accepts only the keys
/// `v`, `id`, `cmd` and `args`, and the arguments accept only their own
/// keys: a key the server does not know may change the meaning of the
/// request, so the server refuses it and does not ignore it.
pub fn decode_request(line: &[u8]) -> Result<Request, Refusal> {
    let value: Value = serde_json::from_slice(line).map_err(|e| Refusal {
        id: None,
        error: ErrorBody::new(ErrorCode::Malformed, format!("the line is not JSON: {e}")),
    })?;
    let Value::Object(mut object) = value else {
        return Err(refuse(None, ErrorCode::InvalidRequest, "the line is not a JSON object"));
    };
    let id = take_id(&mut object)?;
    let refuse_here = |code, message: String| refuse(Some(id), code, message);
    let version = take_version(&mut object).map_err(|m| refuse_here(ErrorCode::InvalidRequest, m))?;
    if version == 0 || version > PROTOCOL_VERSION {
        return Err(refuse_here(
            ErrorCode::UnsupportedVersion,
            format!("version {version} is not served; this server speaks 1 to {PROTOCOL_VERSION}"),
        ));
    }
    if let Some(key) = object.keys().find(|k| *k != "cmd" && *k != "args") {
        return Err(refuse_here(
            ErrorCode::InvalidRequest,
            format!("unknown key {key:?} in the request"),
        ));
    }
    let name = match object.get("cmd") {
        Some(Value::String(name)) => name.clone(),
        Some(_) => return Err(refuse_here(ErrorCode::InvalidRequest, "\"cmd\" is not a string".into())),
        None => return Err(refuse_here(ErrorCode::InvalidRequest, "the request has no \"cmd\"".into())),
    };
    if !Command::NAMES.contains(&name.as_str()) {
        return Err(refuse_here(
            ErrorCode::UnknownCommand,
            format!("unknown command {name:?}"),
        ));
    }
    let command = Command::deserialize(Value::Object(object)).map_err(|e| {
        refuse_here(
            ErrorCode::InvalidArguments,
            format!("bad arguments for {name:?}: {e}"),
        )
    })?;
    Ok(Request { id, command })
}

/// Encodes a response as one line, with the newline.
pub fn encode_response(response: &Response) -> Result<Vec<u8>, CodecError> {
    match &response.outcome {
        Ok(reply) => to_line(
            &ReplyWire {
                v: PROTOCOL_VERSION,
                id: response.id,
                reply,
            },
            "response",
        ),
        Err(error) => to_line(
            &ErrorWire {
                v: PROTOCOL_VERSION,
                id: response.id,
                error,
            },
            "response",
        ),
    }
}

/// Decodes one response line, without its newline.
///
/// The client accepts a response of any version and ignores keys it does not
/// know, because a later server version may add them.
pub fn decode_response(line: &[u8]) -> Result<Response, CodecError> {
    let value: Value = serde_json::from_slice(line).map_err(CodecError::Syntax)?;
    let Value::Object(mut object) = value else {
        return Err(CodecError::Shape("the line is not a JSON object".into()));
    };
    if !matches!(object.get("v"), Some(Value::Number(_))) {
        return Err(CodecError::Shape("the response has no numeric \"v\"".into()));
    }
    let id = match object.remove("id") {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            v.as_u64()
                .ok_or_else(|| CodecError::Shape(format!("\"id\" {v} is not a u64")))?,
        ),
    };
    let outcome = if let Some(error) = object.remove("error") {
        Err(ErrorBody::deserialize(error).map_err(|e| CodecError::Shape(format!("bad error: {e}")))?)
    } else {
        let mut reply = Map::new();
        for key in ["reply", "data"] {
            if let Some(v) = object.remove(key) {
                reply.insert(key.to_owned(), v);
            }
        }
        Ok(Reply::deserialize(Value::Object(reply))
            .map_err(|e| CodecError::Shape(format!("bad reply: {e}")))?)
    };
    Ok(Response { id, outcome })
}

#[derive(Serialize)]
struct RequestWire<'a> {
    v: u32,
    id: u64,
    #[serde(flatten)]
    command: &'a Command,
}

#[derive(Serialize)]
struct ReplyWire<'a> {
    v: u32,
    id: Option<u64>,
    #[serde(flatten)]
    reply: &'a Reply,
}

#[derive(Serialize)]
struct ErrorWire<'a> {
    v: u32,
    id: Option<u64>,
    error: &'a ErrorBody,
}

fn to_line<T: Serialize>(message: &T, what: &'static str) -> Result<Vec<u8>, CodecError> {
    let mut line = serde_json::to_vec(message).map_err(|source| CodecError::Encode { what, source })?;
    line.push(b'\n');
    Ok(line)
}

fn refuse(id: Option<u64>, code: ErrorCode, message: impl Into<String>) -> Refusal {
    Refusal {
        id,
        error: ErrorBody::new(code, message),
    }
}

fn take_id(object: &mut Map<String, Value>) -> Result<u64, Refusal> {
    match object.remove("id") {
        Some(v) => v.as_u64().ok_or_else(|| {
            refuse(
                None,
                ErrorCode::InvalidRequest,
                format!("\"id\" {v} is not an integer from 0 to 2^64-1"),
            )
        }),
        None => Err(refuse(None, ErrorCode::InvalidRequest, "the request has no \"id\"")),
    }
}

fn take_version(object: &mut Map<String, Value>) -> Result<u32, String> {
    match object.remove("v") {
        Some(v) => v
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| format!("\"v\" {v} is not an integer from 0 to 2^32-1")),
        None => Err("the request has no \"v\"".into()),
    }
}
