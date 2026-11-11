//! A 1,000-frame session on the fakes gives the same output twice.
//!
//! The session drives the fakes only through the backend traits, as a
//! frontend does: a mode switch half way, a game frame that changes every
//! frame and with the input, audio every frame, seeded jitter and late
//! flips, and scripted faults.

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_testkit::preset::{CAPS_LOW_CLOCK, MODE_240P, MODE_288P, validated_mode};
use neptron_testkit::{
    AudioWrite, Delivered, DisplayOptions, FakeAudio, FakeDisplay, FlipFault, FrameRecord, Jitter,
    LateFlips, ModeRecord, Render, RgbImage, ScriptedInput, SimClock, Underrun, frame_hash,
};
use neptron_types::{Control, Filter, Frame, InputEvent, PixelFormat, Rect, Scene};

const WIDTH: u16 = 320;
const HEIGHT: u16 = 240;
const PITCH: usize = WIDTH as usize * 2;
/// Simulated work per frame, in nanoseconds.
const WORK_NS: u64 = 3_000_000;
/// Stereo frames of audio per video frame.
const AUDIO_FRAMES: usize = 800;
/// Side of the block that moves one pixel per frame.
const BLOCK: usize = 16;

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    modes: Vec<ModeRecord>,
    frames: Vec<FrameRecord>,
    screen: Option<RgbImage>,
    captured: Option<RgbImage>,
    writes: Vec<AudioWrite>,
    underruns: Vec<Underrun>,
    sample_hash: u64,
    delivered: Vec<Delivered>,
    end_ns: u64,
}

/// The game: an RGB565 frame with a fixed gradient, the frame number and
/// the buttons held in the first pixels, and a block that moves one pixel
/// per frame. Each frame differs from every other.
struct Game {
    pixels: Vec<u8>,
}

impl Game {
    fn new() -> Self {
        let mut pixels = vec![0_u8; PITCH * usize::from(HEIGHT)];
        for (i, px) in pixels.chunks_exact_mut(2).enumerate() {
            let (x, y) = (i % usize::from(WIDTH), i / usize::from(WIDTH));
            let v = ((x & 0x1F) << 11 | (y & 0x3F) << 5 | ((x ^ y) & 0x1F)) as u16;
            px.copy_from_slice(&v.to_ne_bytes());
        }
        Self { pixels }
    }

    fn draw(&mut self, n: u64, held: u16) {
        self.pixels[..8].copy_from_slice(&n.to_ne_bytes());
        self.pixels[8..10].copy_from_slice(&held.to_ne_bytes());
        let width = usize::from(WIDTH) - BLOCK;
        let at = |k: u64| (k as usize % width, 8 + (k as usize / width) % 200);
        for (k, value) in [(n.wrapping_sub(1), 0x0000_u16), (n, 0xFFFF)] {
            let (x, y) = at(k);
            for row in y..y + BLOCK {
                let start = row * PITCH + x * 2;
                for px in self.pixels[start..start + BLOCK * 2].chunks_exact_mut(2) {
                    px.copy_from_slice(&value.to_ne_bytes());
                }
            }
        }
    }
}

fn run(render: Render, frames: u64) -> Outcome {
    let clock = SimClock::starting_at(1_000_000);
    let options = DisplayOptions {
        render,
        jitter: Jitter::Seeded {
            seed: 0x5EED,
            max_ns: 300_000,
        },
        late_flips: Some(LateFlips {
            seed: 0x1A7E,
            per_million: 10_000,
            max_vblanks: 2,
        }),
        modeset_ns: 40_000_000,
        ..DisplayOptions::default()
    };
    let lost = frames * 7 / 10;
    let probe = FakeDisplay::with_options(clock.clone(), CAPS_LOW_CLOCK, options);
    probe.fault_flip(frames * 4 / 10, FlipFault::Late { vblanks: 2 });
    probe.fault_flip(lost, FlipFault::Lost);
    probe.capture(frames - 1);
    let audio_probe = FakeAudio::new(clock.clone(), 48_000, 4_096);
    let input_probe = ScriptedInput::new(clock.clone(), Vec::new());
    for i in 0..20 {
        let control = [Control::A, Control::B, Control::Up, Control::Start][i % 4];
        let time_ns = 1_000_000 + i as u64 * frames * 700_000;
        input_probe.tap(time_ns, 90_000_000, 0, control);
    }

    let mut display: Box<dyn Display> = Box::new(probe.clone());
    let mut audio: Box<dyn AudioSink> = Box::new(audio_probe.clone());
    let mut input: Box<dyn InputSource> = Box::new(input_probe.clone());

    let mode_240p = validated_mode(&MODE_240P, "generic_15", &CAPS_LOW_CLOCK).unwrap();
    let mode_288p = validated_mode(&MODE_288P, "generic_15", &CAPS_LOW_CLOCK).unwrap();
    display.set_mode(&mode_240p).unwrap();

    let mut game = Game::new();
    let mut events: Vec<InputEvent> = Vec::new();
    let mut samples = vec![0_i16; AUDIO_FRAMES * 2];
    let mut held: u16 = 0;
    for n in 0..frames {
        let lines: i32 = if n < frames / 2 { 240 } else { 288 };
        if n == frames / 2 {
            display.set_mode(&mode_288p).unwrap();
        }
        events.clear();
        input.poll(&mut events);
        for e in &events {
            let bit = 1 << (e.control as u16);
            held = if e.value != 0 {
                held | bit
            } else {
                held & !bit
            };
        }
        game.draw(n, held);
        let scene = Scene {
            game: Some(Frame {
                data: &game.pixels,
                width: WIDTH,
                height: HEIGHT,
                pitch: PITCH,
                format: PixelFormat::Rgb565,
            }),
            crop: Rect {
                x: 0,
                y: 0,
                w: u32::from(WIDTH),
                h: u32::from(HEIGHT),
            },
            dest: Rect {
                x: 0,
                y: (lines - i32::from(HEIGHT)) / 2,
                w: u32::from(WIDTH),
                h: u32::from(HEIGHT),
            },
            filter: Filter::Nearest,
            ui: None,
            ui_dest: Rect {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
            },
        };
        display.present(&scene).unwrap();
        let flipped = display.wait_flip();
        assert_eq!(flipped.is_err(), n == lost, "frame {n}: {flipped:?}");
        clock.advance(WORK_NS);
        for (i, s) in samples.iter_mut().enumerate() {
            *s = ((n as i64 * 31 + i as i64 * 7) % 4_001 - 2_000) as i16;
        }
        audio.write(&samples);
    }

    Outcome {
        modes: probe.mode_calls(),
        frames: probe.frames(),
        screen: probe.screen(),
        captured: probe.captured(frames - 1),
        writes: audio_probe.writes(),
        underruns: audio_probe.underruns(),
        sample_hash: audio_probe.sample_hash(),
        delivered: input_probe.delivered(),
        end_ns: clock.now_ns(),
    }
}

/// Checks that the run did what the script says, so that two equal runs
/// are not two equal empty runs.
fn check_outcome(outcome: &Outcome, frames: u64) {
    let count = usize::try_from(frames).unwrap();
    let hashes: Vec<u64> = outcome.frames.iter().filter_map(|f| f.hash).collect();
    assert_eq!(hashes.len(), count);
    let mut distinct = hashes.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), count, "every frame differs");

    assert_eq!(outcome.modes.len(), 2);
    let flipped = outcome.frames.iter().filter(|f| f.flip.is_some()).count();
    assert_eq!(flipped, count - 1);
    assert!(outcome.frames[count * 7 / 10].flip_error.is_some());
    assert_eq!(outcome.frames[count * 4 / 10].late_by(), Some(2));
    assert_eq!(outcome.delivered.len(), 40);
    assert_eq!(outcome.writes.len(), count);
    let screen = outcome.screen.as_ref().unwrap();
    assert_eq!((screen.width(), screen.height()), (320, 288));
    assert_eq!(outcome.captured.as_ref(), Some(screen));
}

fn digest(outcome: &Outcome) -> u64 {
    let hashes: Vec<u8> = outcome
        .frames
        .iter()
        .filter_map(|f| f.hash)
        .flat_map(u64::to_le_bytes)
        .collect();
    frame_hash(&hashes)
}

#[test]
fn a_thousand_frame_session_gives_the_same_output_twice() {
    let first = run(Render::Scene, 1_000);
    let second = run(Render::Scene, 1_000);
    check_outcome(&first, 1_000);
    assert!(first == second, "the two runs differ");
    let late = first
        .frames
        .iter()
        .filter(|f| f.late_by() > Some(0))
        .count();
    assert!(late > 2, "{late} late flips");
    eprintln!(
        "1000 frames: scene hash digest {:016x}, sample hash {:016x}, end {} ns",
        digest(&first),
        first.sample_hash,
        first.end_ns
    );
}

#[test]
fn a_rendered_session_gives_the_same_pixels_twice() {
    let first = run(Render::Pixels, 100);
    let second = run(Render::Pixels, 100);
    check_outcome(&first, 100);
    assert!(first == second, "the two runs differ");
    eprintln!("100 frames: pixel hash digest {:016x}", digest(&first));
}
