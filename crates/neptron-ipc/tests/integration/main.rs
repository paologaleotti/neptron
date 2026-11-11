//! Integration tests of neptron-ipc: the shapes on the wire, the codec, the
//! server loop and the socket transport.

#![forbid(unsafe_code)]

mod codec;
mod fixture;
mod messages;
mod server;
