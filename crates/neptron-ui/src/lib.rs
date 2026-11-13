//! The sealed UI: snapshot in, scene and intents out.
//!
//! The app gives a [`Snapshot`] and the player's [`UiAction`]s. The UI
//! gives a [`UiFrame`]: a [`UiScene`] of draw commands, a [`UiTree`] for the
//! control API, and [`Intent`]s that the app carries out. A [`Canvas`] draws
//! the scene; [`CpuCanvas`] draws it into a 320x240 XRGB8888 buffer.
//!
//! The crate does no I/O, reads no clock and starts no thread. It depends
//! on no other NEPTRON crate.

#![forbid(unsafe_code)]

pub mod canvas;
pub mod font;
mod frame;
mod geom;
pub mod golden;
mod input;
mod intent;
pub mod registry;
pub mod scene;
pub mod screens;
pub mod skin;
pub mod snapshot;
pub mod text;
pub mod theme;
mod tree;
pub mod widgets;

pub use canvas::{Canvas, CpuCanvas, Sprite, SpriteId, UI_HEIGHT, UI_WIDTH};
pub use frame::UiFrame;
pub use geom::{Color, Rect};
pub use input::UiAction;
pub use intent::{Intent, Value};
pub use scene::{DrawCommand, TextSpan, UiScene};
pub use snapshot::Snapshot;
pub use theme::Theme;
pub use tree::{RowKind, TreeRow, UiTree, WidgetKind};
