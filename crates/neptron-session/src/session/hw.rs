//! A core that draws with GL: its context on the display, the core calls
//! inside it, the picture in its image, the loss of the context and the
//! end of a core that shows no picture (`docs/spec/hw-render.md`).

use neptron_backend::{
    AudioSink, Clock, ContextRequest, CoreRender, CoreRenderError, CoreTarget, Display, InputSource,
};
use neptron_libretro::{Host, RunError};
use neptron_types::{Frame, GamePicture, GpuFrame, ScanlineMode};
use neptron_video::VideoEngine;

use super::{Session, State};
use crate::picture::Picture;
use crate::scale::{InternalScale, Machine, frame_scale, internal_scale, scale_value};
use crate::{Counters, LaunchError, START_WINDOW_NS, SessionError};

/// The times the session makes the context of a GL core again after the
/// GPU lost it. The next loss ends the game.
pub const CONTEXT_RECOVERIES_MAX: u32 = 1;

/// The GL context of the core and what the session knows of it.
#[derive(Debug)]
pub(crate) struct Hw {
    request: ContextRequest,
    target: CoreTarget,
    /// The size of the image: the largest frame of the core.
    storage: (u16, u16),
    /// A larger maximum that the core reported: the image takes it before
    /// the next core call.
    grow_to: Option<(u16, u16)>,
    /// The internal resolution that the core option asks for.
    scale: u8,
    /// The internal resolution of the launch: a core that takes a new
    /// value of the option only at its next launch still draws at it.
    launch_scale: u8,
    /// The lines of the native picture, from the core data.
    native_lines: u16,
    losses: u32,
    /// Core frames with no picture since the launch; `None` after the
    /// first picture.
    dark_frames: Option<u32>,
    /// The core frames of [`START_WINDOW_NS`] at the rate of the core.
    dark_limit: u32,
    /// What went wrong in the last core call, for the frame to act on.
    fault: Option<Fault>,
}

/// A failure of the GL context in a core call.
#[derive(Debug)]
enum Fault {
    Lost,
    Driver(CoreRenderError),
}

/// The rule of the internal resolution for a game, kept for a live change
/// of the scanlines.
#[derive(Debug, Clone)]
pub(crate) struct ScaleRule {
    option: String,
    native_lines: u16,
    values: Vec<(u8, String)>,
    machine: Machine,
}

impl ScaleRule {
    /// The rule of `scale` on `machine`.
    pub(crate) fn new(scale: &InternalScale<'_>, machine: Machine) -> Self {
        Self {
            option: scale.option.to_owned(),
            native_lines: scale.native_lines,
            values: scale.values.to_vec(),
            machine,
        }
    }

    /// The scale for `monitor` with the scanlines `mode`, and the option
    /// pair that sets it.
    pub(crate) fn pick(
        &self,
        monitor: &neptron_types::MonitorKind,
        mode: ScanlineMode,
    ) -> (u8, Option<(String, String)>) {
        let k = internal_scale(monitor, self.machine, mode, self.native_lines, &self.values);
        let scale = InternalScale {
            option: &self.option,
            native_lines: self.native_lines,
            values: &self.values,
        };
        let pair = scale_value(&scale, k).map(|v| (self.option.clone(), v.to_owned()));
        (k, pair)
    }
}

impl Hw {
    fn new(
        request: ContextRequest,
        target: CoreTarget,
        storage: (u16, u16),
        (scale, native_lines): (u8, u16),
        fps_mhz: u32,
    ) -> Self {
        Self {
            request,
            target,
            storage,
            grow_to: None,
            scale,
            launch_scale: scale,
            native_lines,
            losses: 0,
            dark_frames: Some(0),
            dark_limit: dark_limit(fps_mhz),
            fault: None,
        }
    }

    /// The scale of a GL frame of `size` when the core's largest frame is
    /// `max`: the scale that the option asks for, or the one of the launch
    /// (a core can take a new value only at its next launch), when the
    /// frame is that many times a native size; else 1.
    pub(crate) fn frame_scale(&self, size: (u16, u16), max: (u16, u16)) -> u16 {
        [self.scale, self.launch_scale]
            .into_iter()
            .map(|k| frame_scale(size, k, max, self.native_lines))
            .find(|&k| k > 1)
            .unwrap_or(1)
    }

    /// The internal resolution that the core option asks for.
    pub(crate) fn scale(&self) -> u8 {
        self.scale
    }

    /// Takes a larger maximum of the core.
    pub(crate) fn grow(&mut self, max: (u16, u16)) {
        if max.0 > self.storage.0 || max.1 > self.storage.1 {
            self.grow_to = Some((max.0.max(self.storage.0), max.1.max(self.storage.1)));
        }
    }

    /// The core sent a picture.
    pub(crate) fn saw_picture(&mut self) {
        self.dark_frames = None;
    }

    /// Counts a core frame. True when the core has sent no picture in
    /// [`START_WINDOW_NS`] of core frames.
    fn count_dark(&mut self) -> bool {
        match &mut self.dark_frames {
            Some(frames) => {
                *frames = frames.saturating_add(1);
                *frames >= self.dark_limit
            }
            None => false,
        }
    }

    /// The scene picture of a GL picture of `width` by `height`.
    fn gpu_frame(&self, (width, height): (u16, u16)) -> GpuFrame {
        GpuFrame {
            image: self.target.image,
            width,
            height,
            storage_width: self.storage.0,
            storage_height: self.storage.1,
            origin: self.request.origin,
        }
    }
}

/// The core frames in [`START_WINDOW_NS`] at `fps_mhz`, at least 1.
fn dark_limit(fps_mhz: u32) -> u32 {
    let frames = START_WINDOW_NS.saturating_mul(u64::from(fps_mhz)) / 1_000_000_000_000;
    u32::try_from(frames).unwrap_or(u32::MAX).max(1)
}

/// The game picture of the scene: the frame in memory, or the image of
/// the GL core.
pub(super) fn game_picture<'a>(picture: &'a Picture, hw: Option<&Hw>) -> Option<GamePicture<'a>> {
    match (picture.gpu_size(), hw) {
        (Some(size), Some(hw)) => Some(GamePicture::Gpu(hw.gpu_frame(size))),
        (Some(_), None) => None,
        (None, _) => picture.frame().map(GamePicture::Memory),
    }
}

/// Makes the core context current for a core call, after the image took a
/// larger size that the core asked for. False when the core must not run:
/// the fault is kept for the frame.
fn begin<D: Display>(engine: &mut VideoEngine<D>, hw: &mut Hw) -> bool {
    let Some(render) = engine.core_render() else {
        hw.fault = Some(Fault::Driver(no_display()));
        return false;
    };
    if let Some((width, height)) = hw.grow_to.take() {
        match render.resize(width, height) {
            Ok(()) => hw.storage = (width, height),
            Err(e) => {
                hw.fault = Some(Fault::Driver(e));
                return false;
            }
        }
    }
    match render.begin() {
        Ok(()) => true,
        // The context was made, so a begin that finds none means the GPU
        // lost it: the same loss as one that `end` reports.
        Err(CoreRenderError::NoContext { .. }) => {
            hw.fault = Some(Fault::Lost);
            false
        }
        Err(e) => {
            hw.fault = Some(Fault::Driver(e));
            false
        }
    }
}

/// Ends the core call: counts the GL errors and keeps a loss or a failure
/// for the frame.
pub(super) fn end<D: Display>(engine: &mut VideoEngine<D>, hw: &mut Hw, counters: &mut Counters) {
    let Some(render) = engine.core_render() else {
        hw.fault = Some(Fault::Driver(no_display()));
        return;
    };
    match render.end() {
        Ok(status) => {
            counters.gl_errors = counters
                .gl_errors
                .saturating_add(u64::from(status.gl_errors));
            if status.context_lost {
                hw.fault = Some(Fault::Lost);
            }
        }
        Err(e) => hw.fault = Some(Fault::Driver(e)),
    }
}

fn no_display() -> CoreRenderError {
    CoreRenderError::NoContext {
        operation: "a display with no core context",
    }
}

impl<D: Display, S: AudioSink, I: InputSource, C: Clock> Session<D, S, I, C> {
    /// Makes the context that the core asked for at load, with an image of
    /// the core's largest frame, gives the core its framebuffer and lookup,
    /// and calls `context_reset` inside the context.
    pub(super) fn start_hw(
        &mut self,
        request: ContextRequest,
        scale: (u8, u16),
    ) -> Result<(), LaunchError> {
        let storage = (self.geometry.max_width, self.geometry.max_height);
        let graphics = |error| LaunchError::Graphics { request, error };
        let render = self
            .engine
            .core_render()
            .ok_or_else(|| graphics(CoreRenderError::NotOffered { request }))?;
        let target = render
            .create(&request, storage.0, storage.1)
            .map_err(graphics)?;
        self.host.set_hw_target(target.framebuffer, target.lookup);
        self.hw = Some(Hw::new(
            request,
            target,
            storage,
            scale,
            self.timing.fps_mhz,
        ));
        self.in_core_context(Host::context_reset)
            .map_err(LaunchError::Run)?;
        self.hw_fault().map_err(|e| match e {
            SessionError::Graphics(error) => graphics(error),
            other => graphics(CoreRenderError::Driver {
                operation: "context_reset",
                reason: other.to_string(),
            }),
        })
    }

    /// Runs `call` on the host inside the core context: begin, the call,
    /// end. With no GL core it is the call alone. A failure of the context
    /// is kept for [`Session::hw_fault`]; the call does not run when the
    /// context could not begin.
    pub(super) fn in_core_context(
        &mut self,
        call: impl FnOnce(&mut Host) -> Result<(), RunError>,
    ) -> Result<(), RunError> {
        let Some(hw) = self.hw.as_mut() else {
            return call(&mut self.host);
        };
        if !begin(&mut self.engine, hw) {
            return Ok(());
        }
        let result = call(&mut self.host);
        end(&mut self.engine, hw, &mut self.counters);
        result
    }

    /// Makes the core context current for the core frame. False when the
    /// core must not run.
    pub(super) fn begin_core_frame(&mut self) -> bool {
        match self.hw.as_mut() {
            Some(hw) => begin(&mut self.engine, hw),
            None => true,
        }
    }

    /// Counts a core frame of a GL core with no picture, and ends the game
    /// once [`START_WINDOW_NS`] of core frames passed with none.
    pub(super) fn count_dark_frame(&mut self) {
        if self.hw.as_mut().is_some_and(Hw::count_dark) {
            self.no_picture = true;
            self.state = State::Ended;
        }
    }

    /// Acts on a fault of the last core call: makes the context again
    /// after the first loss, and ends the game at a later loss or at a
    /// failure of the driver.
    ///
    /// # Errors
    ///
    /// [`SessionError::ContextLost`] and [`SessionError::Graphics`].
    pub(super) fn hw_fault(&mut self) -> Result<(), SessionError> {
        let Some(hw) = self.hw.as_mut() else {
            return Ok(());
        };
        match hw.fault.take() {
            None => Ok(()),
            Some(Fault::Driver(e)) => Err(SessionError::Graphics(e)),
            Some(Fault::Lost) => {
                hw.losses = hw.losses.saturating_add(1);
                self.counters.context_losses = self.counters.context_losses.saturating_add(1);
                if hw.losses > CONTEXT_RECOVERIES_MAX {
                    return Err(SessionError::ContextLost { losses: hw.losses });
                }
                self.recover()
            }
        }
    }

    /// Makes the context of the core again after a loss and calls
    /// `context_reset` in it, with no `context_destroy`: the old context
    /// and every object of the core are gone.
    fn recover(&mut self) -> Result<(), SessionError> {
        let Some(hw) = self.hw.as_mut() else {
            return Ok(());
        };
        let render = self
            .engine
            .core_render()
            .ok_or_else(|| SessionError::Graphics(no_display()))?;
        let target = render
            .create(&hw.request, hw.storage.0, hw.storage.1)
            .map_err(SessionError::Graphics)?;
        hw.target = target;
        hw.grow_to = None;
        self.host.set_hw_target(target.framebuffer, target.lookup);
        self.in_core_context(Host::context_reset)?;
        match self.hw.as_mut().and_then(|hw| hw.fault.take()) {
            None => Ok(()),
            Some(Fault::Driver(e)) => Err(SessionError::Graphics(e)),
            Some(Fault::Lost) => Err(SessionError::ContextLost {
                losses: CONTEXT_RECOVERIES_MAX.saturating_add(1),
            }),
        }
    }

    /// The last game frame, as the session presents it, for a screenshot or
    /// the control API. The picture of a GL core is read back from its
    /// image, which stalls the GPU: call it on a request, never each frame.
    /// `None` before the first picture, or when the read back fails.
    pub fn read_picture(&mut self) -> Option<Frame<'_>> {
        let Some((width, height)) = self.picture.gpu_size() else {
            return self.picture.frame();
        };
        let origin = self.hw.as_ref()?.request.origin;
        let render: &mut dyn CoreRender = self.engine.core_render()?;
        let out = self.picture.read_back_buffer()?;
        render.read_back(out, width, height, origin).ok()?;
        self.picture.read_back_frame()
    }

    /// The context of a GL core goes at the end of the game:
    /// `context_destroy` runs with the context current, and the context
    /// stays current for the unload and the deinit of the host, whose drop
    /// comes next. The display destroys the context when it drops.
    pub(super) fn end_hw(&mut self) {
        if self.hw.take().is_none() {
            return;
        }
        let Some(render) = self.engine.core_render() else {
            return;
        };
        if render.begin().is_ok() {
            // The game ends; a core that fails here still unloads next.
            let _ = self.host.context_destroy();
        }
    }

    /// Sets the scanline settings, and the internal resolution of a GL
    /// core that they decide: scanlines on a flat panel take 1x. The core
    /// takes the new option value when it reads its options.
    pub fn set_scanlines(&mut self, scanlines: neptron_video::ScanlineSettings) {
        self.engine.set_scanlines(scanlines);
        let (Some(rule), Some(hw)) = (self.scale_rule.as_ref(), self.hw.as_mut()) else {
            return;
        };
        let monitor = &self.engine.odren().monitor().profile().kind;
        let (k, pair) = rule.pick(monitor, scanlines.mode);
        if k == hw.scale {
            return;
        }
        if let Some((key, value)) = pair
            && self.host.set_option(&key, &value).is_ok()
        {
            hw.scale = k;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dark_window_is_ten_seconds_of_core_frames() {
        assert_eq!(dark_limit(60_000), 600);
        assert_eq!(dark_limit(59_940), 599);
        assert_eq!(dark_limit(50_000), 500);
        assert_eq!(dark_limit(0), 1);
    }
}
