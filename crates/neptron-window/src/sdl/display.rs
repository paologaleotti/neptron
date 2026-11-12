use std::time::Instant;

use neptron_backend::{Clock, Display, DisplayError};
use neptron_gfx::{OutputBuffer, ReferenceRenderer};
use neptron_safety::ValidatedMode;
use neptron_types::{FlipEvent, GpuCaps, Modeline, Scene};
use sdl3::VideoSubsystem;
use sdl3::render::ScaleMode;
use sdl3::video::{Window, WindowPos};

use crate::pacing::{refresh_rate_mhz, refreshes_per_flip};
use crate::picture::{CRT_ASPECT, Size, is_whole_scale, picture_bound, picture_size};

use super::gpu::{Gpu, MakeError};
use super::open_window;

/// The widest and tallest mode the window shows, in pixels.
pub const WINDOW_MAX_SIDE_PX: u16 = 4096;

/// What the window can show. A window has no pixel clock, so it takes every
/// clock; the monitor profile and the hard rails still judge the mode. It
/// shows an interlaced mode as one progressive picture and flips once per
/// field.
pub const WINDOW_CAPS: GpuCaps = GpuCaps {
    pclock_min_hz: 0,
    pclock_max_hz: u32::MAX,
    interlace: true,
    flip_per_field: true,
    max_width: WINDOW_MAX_SIDE_PX,
    max_height: WINDOW_MAX_SIDE_PX,
};

/// A [`Display`] in a desktop window. It renders each scene with
/// [`ReferenceRenderer`] and shows the active area of the mode as a 4:3
/// picture.
///
/// # Pacing
///
/// The window presents with the desktop's vsync. [`Display::wait_flip`]
/// shows the last scene for the whole number of desktop refreshes nearest
/// to the mode's field period, and blocks in SDL until the last refresh is
/// queued. The flip rate is the desktop rate divided by that number, not
/// the mode rate: a 59.94 Hz mode flips at 60 Hz on a 60 Hz or a 120 Hz
/// desktop, and a 50 Hz mode also flips at 60 Hz on a 60 Hz desktop.
///
/// A [`FlipEvent`] carries the time at which SDL returned from the last
/// present, on the [`WindowClock`] of this display. Its `sequence` counts
/// flips, one per call, because the window cannot see a missed desktop
/// refresh.
///
/// # Errors
///
/// After a failed `set_mode` at a new active size, and after
/// [`Display::blank`], the old mode is gone, and `present` and `wait_flip`
/// return [`DisplayError::NoMode`] until a mode is set. When SDL cannot make a renderer it closes the window; the
/// next `set_mode` opens a new one.
pub struct WindowDisplay {
    output: Output,
    video: VideoSubsystem,
    title: String,
    renderer: ReferenceRenderer,
    clock: WindowClock,
    sequence: u64,
}

/// The time source of the flip events of a [`WindowDisplay`]: a monotonic
/// clock that starts when the window opens.
#[derive(Debug, Clone, Copy)]
pub struct WindowClock {
    origin: Instant,
}

impl Clock for WindowClock {
    fn now_ns(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }
}

enum Output {
    /// No mode is set.
    Idle(Window),
    /// A mode is set.
    Ready(Screen),
    /// SDL closed the window when it failed to make a renderer.
    Closed,
}

/// The renderer and the buffers of one mode.
struct Screen {
    gpu: Gpu,
    buffer: Vec<u8>,
    active: Size,
    refreshes: u32,
}

impl WindowDisplay {
    pub(crate) fn new(window: Window, video: VideoSubsystem, title: &str) -> Self {
        Self {
            output: Output::Idle(window),
            video,
            title: title.to_owned(),
            renderer: ReferenceRenderer::new(),
            clock: WindowClock {
                origin: Instant::now(),
            },
            sequence: 0,
        }
    }

    /// The clock of the flip events, to use as the frontend clock.
    pub fn clock(&self) -> WindowClock {
        self.clock
    }

    /// Programs `mode` on `output`, and returns the new output state with
    /// the result. A mode with the active size of the current one keeps its
    /// renderer and buffers.
    fn program(&self, output: Output, mode: &Modeline) -> (Output, Result<(), DisplayError>) {
        let active = Size {
            w: u32::from(mode.hactive),
            h: u32::from(mode.vactive),
        };
        let mut window = match output {
            Output::Ready(mut screen) if screen.active == active => {
                let result = fit(screen.gpu.window_mut(), active, mode).map(|(_, refreshes)| {
                    screen.refreshes = refreshes;
                });
                return (Output::Ready(screen), result);
            }
            Output::Ready(screen) => screen.gpu.into_window(),
            Output::Idle(window) => window,
            Output::Closed => match open_window(&self.video, &self.title) {
                Ok(window) => window,
                Err(e) => return (Output::Closed, Err(device("window", e))),
            },
        };
        // The picture comes from the rule, not from the window, because
        // `set_size` is asynchronous on some platforms.
        let (picture, refreshes) = match fit(&mut window, active, mode) {
            Ok(fitted) => fitted,
            Err(e) => return (Output::Idle(window), Err(e)),
        };
        let whole = is_whole_scale(active, picture);
        let scale = if whole {
            ScaleMode::Nearest
        } else {
            ScaleMode::Linear
        };
        let refuse = |reason| DisplayError::ModeRefused {
            mode: *mode,
            reason,
        };
        match Gpu::make(window, active.w, active.h, scale, refuse) {
            Ok(gpu) => Screen::start(gpu, active, refreshes),
            Err(MakeError {
                window: Some(window),
                error,
            }) => (Output::Idle(window), Err(error)),
            Err(MakeError {
                window: None,
                error,
            }) => (Output::Closed, Err(error)),
        }
    }
}

impl Display for WindowDisplay {
    fn caps(&self) -> &GpuCaps {
        &WINDOW_CAPS
    }

    fn set_mode(&mut self, mode: &ValidatedMode) -> Result<Modeline, DisplayError> {
        let mode = *mode.mode();
        check_size(&mode)?;
        let output = std::mem::replace(&mut self.output, Output::Closed);
        let (output, result) = self.program(output, &mode);
        self.output = output;
        result.map(|()| mode)
    }

    fn present(&mut self, scene: &Scene) -> Result<(), DisplayError> {
        let Output::Ready(screen) = &mut self.output else {
            return Err(DisplayError::NoMode);
        };
        let pitch = screen.pitch();
        let mut out = OutputBuffer {
            data: &mut screen.buffer,
            width: screen.active.w,
            height: screen.active.h,
            pitch,
        };
        self.renderer
            .render(scene, &mut out)
            .map_err(|e| DisplayError::SceneRefused {
                reason: e.to_string(),
            })?;
        screen.upload()
    }

    fn wait_flip(&mut self) -> Result<FlipEvent, DisplayError> {
        let Output::Ready(screen) = &mut self.output else {
            return Err(DisplayError::NoMode);
        };
        for _ in 0..screen.refreshes {
            screen.gpu.show()?;
        }
        self.sequence += 1;
        Ok(FlipEvent {
            time_ns: self.clock.now_ns(),
            sequence: self.sequence,
        })
    }

    /// Shows black once and drops the renderer, so the window holds no
    /// mode. A window has no sync to turn off.
    fn blank(&mut self) -> Result<(), DisplayError> {
        let output = std::mem::replace(&mut self.output, Output::Closed);
        let Output::Ready(mut screen) = output else {
            self.output = output;
            return Ok(());
        };
        screen.buffer.fill(0);
        if let Err(e) = screen.upload().and_then(|()| screen.gpu.show()) {
            self.output = Output::Ready(screen);
            return Err(e);
        }
        self.output = Output::Idle(screen.gpu.into_window());
        Ok(())
    }
}

impl std::fmt::Debug for WindowDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = match &self.output {
            Output::Idle(_) => "idle",
            Output::Ready(_) => "ready",
            Output::Closed => "closed",
        };
        f.debug_struct("WindowDisplay")
            .field("state", &state)
            .field("sequence", &self.sequence)
            .finish_non_exhaustive()
    }
}

impl Screen {
    /// Makes the buffer for `active` and shows black. On an error it gives
    /// the window back.
    fn start(gpu: Gpu, active: Size, refreshes: u32) -> (Output, Result<(), DisplayError>) {
        let mut screen = Screen {
            gpu,
            buffer: vec![0; active.w as usize * active.h as usize * 4],
            active,
            refreshes,
        };
        match screen.upload() {
            Ok(()) => (Output::Ready(screen), Ok(())),
            Err(e) => (Output::Idle(screen.gpu.into_window()), Err(e)),
        }
    }

    fn pitch(&self) -> usize {
        self.active.w as usize * 4
    }

    fn upload(&mut self) -> Result<(), DisplayError> {
        let pitch = self.pitch();
        self.gpu.upload(&self.buffer, pitch)
    }
}

pub(super) fn device(operation: &'static str, reason: impl std::fmt::Display) -> DisplayError {
    DisplayError::Device {
        operation,
        reason: reason.to_string(),
    }
}

fn check_size(mode: &Modeline) -> Result<(), DisplayError> {
    if mode.hactive > WINDOW_MAX_SIDE_PX || mode.vactive > WINDOW_MAX_SIDE_PX {
        return Err(DisplayError::ModeRefused {
            mode: *mode,
            reason: format!(
                "the window shows at most {WINDOW_MAX_SIDE_PX}x{WINDOW_MAX_SIDE_PX} pixels"
            ),
        });
    }
    Ok(())
}

/// Sizes the window for `active` on its desktop, and returns the picture
/// size and the desktop refreshes per flip for `mode`.
fn fit(window: &mut Window, active: Size, mode: &Modeline) -> Result<(Size, u32), DisplayError> {
    let desktop = window.get_display().map_err(|e| device("display", e))?;
    let usable = desktop
        .get_usable_bounds()
        .map_err(|e| device("usable bounds", e))?;
    let desktop_mode = desktop.get_mode().map_err(|e| device("desktop mode", e))?;
    let bound = picture_bound(Size {
        w: usable.width(),
        h: usable.height(),
    });
    let picture = picture_size(active, bound, CRT_ASPECT);
    window
        .set_size(picture.w, picture.h)
        .map_err(|e| device("window size", e))?;
    // A failed move leaves the window where it is, which is still usable.
    window.set_position(WindowPos::Centered, WindowPos::Centered);
    let desktop_mhz = refresh_rate_mhz(
        desktop_mode.refresh_rate_numerator,
        desktop_mode.refresh_rate_denominator,
    );
    Ok((
        picture,
        refreshes_per_flip(desktop_mhz, mode.field_rate_mhz()),
    ))
}
