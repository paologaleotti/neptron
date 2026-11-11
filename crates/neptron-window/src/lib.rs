//! Desktop window backend for development. It shows the reference renderer's output in an SDL3 window and maps the keyboard to a gamepad. Tests never use it.
//!
//! All SDL3 code sits behind the `sdl3` feature, which is off by default.
//! Without it the crate has no public items, and the headless build
//! compiles and links no SDL3 code.

#![forbid(unsafe_code)]

// Only the SDL3 backend calls the rules below. They are plain functions so
// that the tests check them without the feature and without a window.
#[cfg_attr(not(feature = "sdl3"), allow(dead_code))]
mod pacing;
#[cfg_attr(not(feature = "sdl3"), allow(dead_code))]
mod picture;
