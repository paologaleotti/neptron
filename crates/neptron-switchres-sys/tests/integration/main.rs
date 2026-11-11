//! Integration tests of neptron-switchres-sys: the compiled Switchres gives
//! the modelines of the upstream standalone program, and the call reports
//! each error.
//!
//! The expected modelines come from the upstream standalone program
//! (`switchres <w> <h> <refresh> --calc`), built from the vendored commit.

mod calls;
