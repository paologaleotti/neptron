//! Stage A, the wire: the engine of the app decides the mode and the scene
//! for the source on the display, and the reference renderer draws the
//! frame buffer of the mode, as a fake display of `neptron-testkit` does
//! in the tests of the engine.

use crate::{DisplayOptions, FakeDisplay, Render, SimClock};
use neptron_types::{
    Filter, Frame, Modeline, MonitorKind, Orientation, PixelFormat, Rect, Rotation, Scanlines,
    SourceGeometry,
};
use neptron_video::{
    Monitor, Notice, Offer, PolicySettings, Proposal, Rung, ScanlineSettings, VideoEngine,
    VideoError,
};

use super::case::Screen;
use super::image::Rgb;

/// What the engine shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// The mode the display read back.
    pub mode: Modeline,
    /// The winner's rung.
    pub rung: Rung,
    /// The winner: its mode, its placement and its timing.
    pub proposal: Proposal,
    /// The notice of the winner.
    pub notice: Option<Notice>,
    /// The scene of the frame: the part of the frame shown, where, turned
    /// and filtered how, with which scanlines.
    pub scene: SceneFacts,
    /// The frame buffer of the mode, as the reference renderer draws it.
    pub frame_buffer: Rgb,
}

/// The values of the scene that the measures read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneFacts {
    /// The part of the game frame shown, in frame pixels.
    pub crop: Rect,
    /// Where the turned picture goes, in mode pixels.
    pub dest: Rect,
    /// The turn of the picture.
    pub rotation: Rotation,
    /// The filter.
    pub filter: Filter,
    /// The scanlines the renderer draws.
    pub scanlines: Scanlines,
}

/// The result of stage A.
#[derive(Debug, Clone, PartialEq)]
pub struct Wire {
    /// What shows, or `None` when the engine has no mode.
    pub shown: Option<Shown>,
    /// The answer of the menus for these settings: offered, or declined
    /// with its reason.
    pub offer: Result<Offer, String>,
    /// Why nothing shows, when nothing does.
    pub no_picture: Option<String>,
}

/// Why stage A failed: an error of the engine or the renderer that a
/// correct case never gives.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WireError {
    /// The frame does not have the size of the source.
    #[error("the frame is {frame_w}x{frame_h}, the source {source_w}x{source_h}")]
    FrameSize {
        /// Frame width.
        frame_w: u32,
        /// Frame height.
        frame_h: u32,
        /// Source width.
        source_w: u16,
        /// Source height.
        source_h: u16,
    },
    /// The engine refused the frame after it set a mode.
    #[error("the engine refused the frame: {0}")]
    Engine(String),
    /// The fake display kept no picture.
    #[error("the display shows no frame after the flip")]
    NoScreen,
}

/// Runs the engine of the app for one core frame of `source` with the
/// picture `frame` on `screen` at `orientation`, and returns the frame
/// buffer of the mode and what the engine decided. `source` is the source
/// as the core sends it: the engine plans with the display source of the
/// orientation, as in the app.
///
/// # Errors
///
/// [`WireError`] for a frame of another size than the source, or a
/// present that fails after the engine set a mode.
pub fn run(
    screen: &Screen,
    source: &SourceGeometry,
    settings: PolicySettings,
    scanlines: ScanlineSettings,
    orientation: Orientation,
    frame: &Rgb,
) -> Result<Wire, WireError> {
    if frame.width != u32::from(source.width) || frame.height != u32::from(source.height) {
        return Err(WireError::FrameSize {
            frame_w: frame.width,
            frame_h: frame.height,
            source_w: source.width,
            source_h: source.height,
        });
    }
    let tv_standards = match &screen.profile.kind {
        MonitorKind::Tv { standard } => Some(vec![*standard]),
        MonitorKind::Crt { .. } | MonitorKind::FlatPanel { .. } => None,
    };
    let options = DisplayOptions {
        render: Render::Scene,
        tv_standards,
        ..DisplayOptions::default()
    };
    let display = FakeDisplay::with_options(SimClock::new(), screen.caps, options);
    let monitor = Monitor::Profile(screen.profile.clone());
    let mut engine = VideoEngine::new(display, monitor, settings);
    engine.set_scanlines(scanlines);
    engine.set_orientation(orientation);
    let offer = engine.offer(source).map_err(|e| e.to_string());
    let data = frame.to_xrgb8888();
    let game = Frame {
        data: &data,
        width: source.width,
        height: source.height,
        pitch: usize::from(source.width) * 4,
        format: PixelFormat::Xrgb8888,
    };
    engine.begin_frame(*source);
    let current = match engine.present(Some(game), None) {
        Ok(_) => {
            engine
                .wait_flip()
                .map_err(|e| WireError::Engine(e.to_string()))?;
            engine.current().cloned()
        }
        Err(VideoError::NoMode) => None,
        Err(e) => return Err(WireError::Engine(e.to_string())),
    };
    let Some(current) = current else {
        let reason = no_picture_reason(&offer);
        return Ok(Wire {
            shown: None,
            offer,
            no_picture: Some(reason),
        });
    };
    let screen_image = engine.display().screen().ok_or(WireError::NoScreen)?;
    let scene = engine
        .odren()
        .scene(Some(game), None)
        .ok_or_else(|| WireError::Engine("no scene after the modeset".into()))?;
    let frame_buffer = Rgb::from_rgb(
        screen_image.width(),
        screen_image.height(),
        screen_image.bytes().to_vec(),
    )
    .ok_or(WireError::NoScreen)?;
    Ok(Wire {
        shown: Some(Shown {
            mode: current.read_back,
            rung: current.chosen.rung,
            proposal: current.chosen.proposal,
            notice: current.chosen.notice,
            scene: SceneFacts {
                crop: scene.crop,
                dest: scene.dest,
                rotation: scene.rotation,
                filter: scene.filter,
                scanlines: scene.scanlines,
            },
            frame_buffer,
        }),
        offer,
        no_picture: None,
    })
}

fn no_picture_reason(offer: &Result<Offer, String>) -> String {
    match offer {
        Ok(Ok(_)) => "the menus offer it, but the engine set no mode".to_owned(),
        Ok(Err(declined)) => format!("declined: {:?}", declined.reason),
        Err(error) => error.clone(),
    }
}
