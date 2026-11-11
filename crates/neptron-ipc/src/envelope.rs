use std::collections::BTreeSet;

use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::command::Command;
use crate::error::{ErrorBody, ErrorCode};
use crate::reply::Reply;

/// The newest protocol version this crate speaks. Every request and every
/// response carries a version in the `v` key.
///
/// The protocol is append-only: a new version adds commands, reply fields,
/// enum values or error codes, and never renames or removes one. A server
/// therefore serves every request with a version from
/// [`MIN_PROTOCOL_VERSION`] up to its own. A response echoes the version of
/// its request, and holds only names that exist in that version, so that an
/// older client can read every response it gets.
pub const PROTOCOL_VERSION: u32 = 1;

/// The oldest protocol version this crate serves.
pub const MIN_PROTOCOL_VERSION: u32 = 1;

/// One request: a command with the id that the response echoes.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// The protocol version of the request. A client sets
    /// [`PROTOCOL_VERSION`].
    pub version: u32,
    /// A number the client chooses. The response carries the same id.
    pub id: u64,
    /// The command.
    pub command: Command,
}

/// One response: a reply or an error, for the request with `id`.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    /// The protocol version of the response: the version of the request, or
    /// [`PROTOCOL_VERSION`] when the server could not read a version it
    /// speaks.
    pub version: u32,
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
    /// The version of the error response: the version of the request once
    /// the server has checked it, else [`PROTOCOL_VERSION`].
    pub version: u32,
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
        v: request.version,
        id: request.id,
        command: &request.command,
    };
    to_line(&wire, "request")
}

/// Decodes one request line, without its newline.
///
/// The checks run in a fixed order, so that each fault gets one code: JSON
/// syntax (`malformed`), a key twice in one object (`invalid_request`), the
/// envelope keys (`invalid_request`), the version
/// (`unsupported_version`), the command name (`unknown_command`), and last
/// the arguments (`invalid_arguments`). The envelope accepts only the keys
/// `v`, `id`, `cmd` and `args`, and the arguments accept only their own
/// keys: a key the server does not know may change the meaning of the
/// request, so the server refuses it and does not ignore it.
///
/// An `unsupported_version` error carries the range of versions the server
/// speaks in `min_version` and `max_version`, so that a client of any version
/// can find a version to use.
pub fn decode_request(line: &[u8]) -> Result<Request, Refusal> {
    let value: Value = serde_json::from_slice(line).map_err(|e| {
        refuse(
            None,
            ErrorCode::Malformed,
            format!("the line is not JSON: {e}"),
        )
    })?;
    // The JSON value keeps only the last of two equal keys, so a second
    // pass over the text finds them.
    serde_json::from_slice::<NoDuplicateKeys>(line)
        .map_err(|e| refuse(None, ErrorCode::InvalidRequest, e.to_string()))?;
    let Value::Object(mut object) = value else {
        return Err(refuse(
            None,
            ErrorCode::InvalidRequest,
            "the line is not a JSON object",
        ));
    };
    let id = take_id(&mut object)?;
    let version =
        take_version(&mut object).map_err(|m| refuse(Some(id), ErrorCode::InvalidRequest, m))?;
    if !(MIN_PROTOCOL_VERSION..=PROTOCOL_VERSION).contains(&version) {
        let mut refusal = refuse(
            Some(id),
            ErrorCode::UnsupportedVersion,
            format!(
                "version {version} is not served; this server speaks {MIN_PROTOCOL_VERSION} to {PROTOCOL_VERSION}"
            ),
        );
        refusal.error.min_version = Some(MIN_PROTOCOL_VERSION);
        refusal.error.max_version = Some(PROTOCOL_VERSION);
        return Err(refusal);
    }
    let refuse_here = |code, message: String| Refusal {
        version,
        ..refuse(Some(id), code, message)
    };
    if let Some(key) = object.keys().find(|k| *k != "cmd" && *k != "args") {
        return Err(refuse_here(
            ErrorCode::InvalidRequest,
            format!("unknown key {key:?} in the request"),
        ));
    }
    let name = match object.get("cmd") {
        Some(Value::String(name)) => name.clone(),
        Some(_) => {
            return Err(refuse_here(
                ErrorCode::InvalidRequest,
                "\"cmd\" is not a string".into(),
            ));
        }
        None => {
            return Err(refuse_here(
                ErrorCode::InvalidRequest,
                "the request has no \"cmd\"".into(),
            ));
        }
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
    Ok(Request {
        version,
        id,
        command,
    })
}

/// Encodes a response as one line, with the newline.
pub fn encode_response(response: &Response) -> Result<Vec<u8>, CodecError> {
    match &response.outcome {
        Ok(reply) => to_line(
            &ReplyWire {
                v: response.version,
                id: response.id,
                reply,
            },
            "response",
        ),
        Err(error) => to_line(
            &ErrorWire {
                v: response.version,
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
/// know, because a later server version may add them. An error code it does
/// not know reads as [`ErrorCode::Unknown`]. A reply enum value it does not
/// know is a codec error; a server that follows the version rule of
/// [`PROTOCOL_VERSION`] never sends one.
pub fn decode_response(line: &[u8]) -> Result<Response, CodecError> {
    let value: Value = serde_json::from_slice(line).map_err(CodecError::Syntax)?;
    let Value::Object(mut object) = value else {
        return Err(CodecError::Shape("the line is not a JSON object".into()));
    };
    let version = object
        .get("v")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| CodecError::Shape("the response has no \"v\" from 0 to 2^32-1".into()))?;
    let id = match object.remove("id") {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            v.as_u64()
                .ok_or_else(|| CodecError::Shape(format!("\"id\" {v} is not a u64")))?,
        ),
    };
    let outcome = if let Some(error) = object.remove("error") {
        Err(ErrorBody::deserialize(error)
            .map_err(|e| CodecError::Shape(format!("bad error: {e}")))?)
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
    Ok(Response {
        version,
        id,
        outcome,
    })
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
    let mut line =
        serde_json::to_vec(message).map_err(|source| CodecError::Encode { what, source })?;
    line.push(b'\n');
    Ok(line)
}

fn refuse(id: Option<u64>, code: ErrorCode, message: impl Into<String>) -> Refusal {
    Refusal {
        version: PROTOCOL_VERSION,
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
        None => Err(refuse(
            None,
            ErrorCode::InvalidRequest,
            "the request has no \"id\"",
        )),
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

/// Reads any JSON text and fails on the first object that has a key twice.
struct NoDuplicateKeys;

impl<'de> Deserialize<'de> for NoDuplicateKeys {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(NoDuplicateKeys)
    }
}

impl<'de> Visitor<'de> for NoDuplicateKeys {
    type Value = NoDuplicateKeys;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Self, E> {
        Ok(self)
    }

    fn visit_i64<E: de::Error>(self, _: i64) -> Result<Self, E> {
        Ok(self)
    }

    fn visit_u64<E: de::Error>(self, _: u64) -> Result<Self, E> {
        Ok(self)
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Self, E> {
        Ok(self)
    }

    fn visit_str<E: de::Error>(self, _: &str) -> Result<Self, E> {
        Ok(self)
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self, E> {
        Ok(self)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self, A::Error> {
        while seq.next_element::<NoDuplicateKeys>()?.is_some() {}
        Ok(self)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self, A::Error> {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            map.next_value::<NoDuplicateKeys>()?;
            if !keys.insert(key.clone()) {
                return Err(de::Error::custom(format!("the key {key:?} appears twice")));
            }
        }
        Ok(self)
    }
}
