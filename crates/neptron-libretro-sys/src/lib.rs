//! Raw FFI bindings to `libretro.h`. `cargo xtask bindgen` generates them from
//! `vendor/libretro.h`, and the generated code is committed.
//!
//! The crate holds every `retro_*` type and `RETRO_*` constant of the header,
//! and a function pointer type for each core entry point. It has no
//! declarations of the entry points: a frontend looks them up in a loaded
//! library, and a core defines them.
//!
//! The generated code checks the size, the alignment and the field offsets
//! of every struct at compile time, against the layout that clang computes for
//! an LP64 target. The crate therefore builds only for a 64-bit target with
//! the LP64 data model.

// The names follow the header, and the header is the documentation.
#[allow(
    non_camel_case_types,
    non_upper_case_globals,
    non_snake_case,
    missing_docs,
    clippy::all
)]
mod bindings;
#[cfg(test)]
#[allow(
    non_camel_case_types,
    non_upper_case_globals,
    non_snake_case,
    missing_docs,
    dead_code,
    clippy::all
)]
mod core_api_decls;

mod core_api;

pub use bindings::*;
pub use core_api::*;
