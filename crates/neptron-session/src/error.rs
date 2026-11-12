use std::path::PathBuf;

use neptron_audio::AudioError;
use neptron_libretro::{LoadError, OpenError, RunError};
use neptron_video::VideoError;

/// Why a game could not launch.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LaunchError {
    /// The save folder could not be made.
    #[error("cannot make the save folder {path}: {source}")]
    SaveDir {
        /// The folder.
        path: PathBuf,
        /// The error of the file system.
        #[source]
        source: std::io::Error,
    },
    /// The core could not be opened.
    #[error("cannot open the core: {0}")]
    Open(#[from] OpenError),
    /// The core could not load the content.
    #[error("the core cannot load the content: {0}")]
    Load(#[from] LoadError),
    /// A core frame failed while the source settled.
    #[error("the core failed in a frame at launch: {0}")]
    Run(#[source] RunError),
    /// The audio pipeline refuses the core's timing or the device.
    #[error("cannot start the audio: {0}")]
    Audio(#[source] AudioError),
}

/// Why the session cannot go on. The caller ends it with
/// [`Session::quit`](crate::Session::quit).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SessionError {
    /// The session has ended: the core asked to stop, or an earlier error.
    #[error("the session has ended")]
    Ended,
    /// A call into the core failed.
    #[error("the core failed: {0}")]
    Core(#[from] RunError),
    /// No flip completed for this many frames in a row. The engine and its
    /// trace have the details.
    #[error("no flip completed for {frames} frames; the last error: {last}")]
    DisplayStalled {
        /// Frames in a row with no completed flip.
        frames: u32,
        /// The last error of the engine.
        last: VideoError,
    },
}
