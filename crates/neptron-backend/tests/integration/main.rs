//! Integration tests of neptron-backend: the traits work as trait objects,
//! and the errors carry their context.

#![forbid(unsafe_code)]

use neptron_backend::{AudioSink, Clock, Display, DisplayError, InputSource};
use neptron_safety::ValidatedMode;
use neptron_types::{Control, FlipEvent, GpuCaps, InputEvent, Modeline, Scene};

const CAPS: GpuCaps = GpuCaps {
    pclock_min_hz: 25_000_000,
    pclock_max_hz: 400_000_000,
    interlace: false,
    flip_per_field: false,
    max_width: 3840,
    max_height: 2160,
};

/// A display with no mode, to show that the trait works as `dyn Display`.
struct Unset;

impl Display for Unset {
    fn caps(&self) -> &GpuCaps {
        &CAPS
    }
    fn set_mode(&mut self, mode: &ValidatedMode) -> Result<Modeline, DisplayError> {
        Ok(*mode.mode())
    }
    fn present(&mut self, _scene: &Scene) -> Result<(), DisplayError> {
        Err(DisplayError::NoMode)
    }
    fn wait_flip(&mut self) -> Result<FlipEvent, DisplayError> {
        Err(DisplayError::NoMode)
    }
}

/// A sink that takes at most four frames.
struct Small {
    queued: usize,
}

impl AudioSink for Small {
    fn rate_hz(&self) -> u32 {
        48_000
    }
    fn queued_frames(&self) -> usize {
        self.queued
    }
    fn write(&mut self, stereo: &[i16]) -> usize {
        let taken = (stereo.len() / 2).min(4 - self.queued);
        self.queued += taken;
        taken
    }
}

struct OnePress;

impl InputSource for OnePress {
    fn poll(&mut self, out: &mut Vec<InputEvent>) {
        out.push(InputEvent {
            device: 0,
            control: Control::Start,
            value: 1,
        });
    }
}

struct Fixed(u64);

impl Clock for Fixed {
    fn now_ns(&self) -> u64 {
        self.0
    }
}

#[test]
fn traits_work_as_trait_objects() {
    let mut display: Box<dyn Display> = Box::new(Unset);
    assert_eq!(display.caps(), &CAPS);
    assert_eq!(display.wait_flip(), Err(DisplayError::NoMode));

    let mut audio: Box<dyn AudioSink> = Box::new(Small { queued: 0 });
    assert_eq!(audio.write(&[0; 7]), 3);
    assert_eq!(audio.write(&[0; 8]), 1);
    assert_eq!(audio.queued_frames(), 4);
    assert_eq!(audio.rate_hz(), 48_000);

    let mut input: Box<dyn InputSource> = Box::new(OnePress);
    let mut events = vec![InputEvent {
        device: 1,
        control: Control::A,
        value: 0,
    }];
    input.poll(&mut events);
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].control, Control::Start);

    let clock: Box<dyn Clock> = Box::new(Fixed(16_683_333));
    assert_eq!(clock.now_ns(), 16_683_333);
}

#[test]
fn display_errors_carry_context() {
    let mode = Modeline {
        pclock_hz: 6_700_000,
        hactive: 320,
        hsync_start: 336,
        hsync_end: 368,
        htotal: 426,
        vactive: 240,
        vsync_start: 244,
        vsync_end: 247,
        vtotal: 262,
        interlace: false,
        doublescan: false,
        hsync_positive: false,
        vsync_positive: false,
    };
    let refused = DisplayError::ModeRefused {
        mode,
        reason: "EINVAL".to_owned(),
    };
    let text = refused.to_string();
    assert!(text.starts_with("the driver refused the mode Modeline { pclock_hz: 6700000,"));
    assert!(text.ends_with(": EINVAL"));

    let device = DisplayError::Device {
        operation: "atomic commit",
        reason: "EBUSY".to_owned(),
    };
    assert_eq!(
        device.to_string(),
        "the display device failed in atomic commit: EBUSY"
    );
    assert_eq!(
        DisplayError::SceneRefused {
            reason: "frame has 10 bytes, needs 1280".to_owned()
        }
        .to_string(),
        "the scene cannot be shown: frame has 10 bytes, needs 1280"
    );
}
