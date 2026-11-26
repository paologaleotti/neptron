//! The traits that hide display, audio, input and clock behind one
//! interface, so that the frontend runs on KMS, in a window or on test
//! fakes.
//!
//! A backend executes what it gets and decides nothing. The event types the
//! traits pass on, `FlipEvent` and `InputEvent`, are plain data in
//! `neptron-types`.

#![forbid(unsafe_code)]

mod audio;
mod clock;
mod core_render;
mod display;
mod input;

pub use audio::AudioSink;
pub use clock::Clock;
pub use core_render::{
    ContextOffer, ContextRequest, CoreFrameStatus, CoreRender, CoreRenderError, CoreTarget, GlApi,
    GlVersion,
};
pub use display::{Display, DisplayError};
pub use input::InputSource;
