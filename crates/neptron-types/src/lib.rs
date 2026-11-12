//! Shared plain data: modeline, CRT range, monitor profile, GPU capabilities,
//! output kind, source geometry, frame, scene and backend events. Plain data with serde,
//! no I/O.
//!
//! Every contract value is an integer: frequencies in millihertz (or hertz
//! where the name says `_hz`), times in nanoseconds.

#![forbid(unsafe_code)]

mod event;
mod gpu;
mod modeline;
mod monitor;
mod output;
mod scanline;
mod scene;
mod source;

pub use event::{Control, FlipEvent, InputEvent};
pub use gpu::GpuCaps;
pub use modeline::Modeline;
pub use monitor::{CrtRange, MonitorKind, MonitorProfile};
pub use output::OutputKind;
pub use scanline::{
    CRT_15KHZ_CLASS_MAX_LINE_RATE_MHZ, ScanlineKind, ScanlineMode, Scanlines, ScreenClass,
    scanlines_for,
};
pub use scene::{Filter, Frame, PixelFormat, Rect, Scene, ui_dest_fill, ui_dest_uniform};
pub use source::{Rotation, SourceGeometry};
