use neptron_safety::ValidatedMode;
use neptron_types::{FlipEvent, GpuCaps, Modeline, Scene};

/// A display output: KMS, a desktop window or a test fake.
///
/// The only way to change the mode is [`Display::set_mode`], and it takes a
/// [`ValidatedMode`] only. [`Display::blank`] turns the output off and sets
/// no mode.
pub trait Display {
    /// What the output can drive.
    fn caps(&self) -> &GpuCaps;

    /// Programs `mode` and returns the mode the driver reports back, which
    /// can differ from `mode`, for example in the pixel clock. After a
    /// [`Display::blank`] it is always a real modeset, also for the mode
    /// that was set before the blank.
    ///
    /// # Errors
    ///
    /// [`DisplayError::ModeRefused`] when the driver refuses the mode,
    /// [`DisplayError::Device`] while a flip is queued (busy) or when the
    /// device fails. On an error the mode that was active stays.
    fn set_mode(&mut self, mode: &ValidatedMode) -> Result<Modeline, DisplayError>;

    /// Queues one flip that shows `scene`, and returns without waiting for
    /// it.
    ///
    /// # Errors
    ///
    /// [`DisplayError::NoMode`] before the first `set_mode` and after a
    /// `blank`, [`DisplayError::SceneRefused`] when the scene does not fit the mode
    /// or its frames, [`DisplayError::Device`] while a flip is queued
    /// (busy) or when the device fails.
    fn present(&mut self, scene: &Scene) -> Result<(), DisplayError>;

    /// Blocks until the next flip completes, but no longer than a limit of
    /// the backend, and returns when the flip took effect.
    ///
    /// When the limit passes first, it returns [`DisplayError::Device`]
    /// and the flip stays queued: it can still complete, a later
    /// `wait_flip` returns its event, and until then `present`,
    /// `set_mode` and `blank` return [`DisplayError::Device`] as busy. Any other error
    /// of the wait also keeps the flip queued. A backend never drops a
    /// queued flip.
    ///
    /// # Errors
    ///
    /// [`DisplayError::NoMode`] before the first `set_mode` and after a
    /// `blank`, [`DisplayError::Device`] with no flip queued, when the
    /// limit passes, or when the device fails.
    fn wait_flip(&mut self) -> Result<FlipEvent, DisplayError>;

    /// Turns the output off: no sync and no picture, until the next
    /// [`Display::set_mode`]. It is the one call that changes the display
    /// without a [`ValidatedMode`], and it carries no mode.
    ///
    /// While the output is off, `present` and `wait_flip` return
    /// [`DisplayError::NoMode`], as before the first `set_mode`, and no
    /// mode is read back. The next `set_mode` is a real modeset with its
    /// read back. A blank of an output that is off, or that never had a
    /// mode, turns it off again and succeeds.
    ///
    /// With a flip queued, `blank` follows the busy rule of `set_mode`: it
    /// returns [`DisplayError::Device`] and changes nothing. The caller
    /// waits for the flip first.
    ///
    /// # Errors
    ///
    /// [`DisplayError::Device`] while a flip is queued, or when the device
    /// fails. On an error the output keeps its state.
    fn blank(&mut self) -> Result<(), DisplayError>;
}

/// An error of a [`Display`] backend.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DisplayError {
    /// The driver refused a mode that passed validation.
    #[error("the driver refused the mode {mode:?}: {reason}")]
    ModeRefused {
        /// The mode that was refused.
        mode: Modeline,
        /// What the driver reported.
        reason: String,
    },
    /// The scene cannot be shown, for example because a frame holds fewer
    /// bytes than its size and pitch need.
    #[error("the scene cannot be shown: {reason}")]
    SceneRefused {
        /// What is wrong with the scene.
        reason: String,
    },
    /// No mode is set yet.
    #[error("no mode is set")]
    NoMode,
    /// The device failed.
    #[error("the display device failed in {operation}: {reason}")]
    Device {
        /// The operation that failed, for example `atomic commit`.
        operation: &'static str,
        /// What the device or the driver reported.
        reason: String,
    },
}
