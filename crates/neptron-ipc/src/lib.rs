//! The control and test protocol: JSON lines on a Unix socket for screenshot, UI tree, input and clock. A TCP listener exists only with the `control` feature.

#![forbid(unsafe_code)]
