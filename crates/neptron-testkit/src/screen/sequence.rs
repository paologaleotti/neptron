//! A sequence case: mode changes in play, over time. The engine of the app
//! takes one core frame after another, with its debounce and its rule of
//! one modeset in a second, on the fake display; the glass of chosen
//! frames makes a strip of pictures with the frame number where the mode
//! changes.

use neptron_types::{Frame, MonitorKind, Orientation, PixelFormat};
use neptron_video::{Directive, Monitor, PolicySettings, VideoEngine, VideoError, wide_strategy};

use super::case::Source;
use super::glass::Glass;
use super::image::Rgb;
use super::measure::{dropped_lines, milli_text, scan_letter};
use super::run::{RunError, Tables, pattern_of};
use super::sheet::{Cell, card_legend, sheet};
use super::survey::{LABEL_COLOR, LOOK_COLOR, Label, NOTE_COLOR};
use super::wire::WireError;
use crate::{DisplayOptions, FakeDisplay, Render, SimClock};

/// A sequence: a display, its GPU, and the sources of the game in order,
/// each for a number of core frames.
#[derive(Debug, Clone, Copy)]
pub struct SequenceSpec {
    /// The name of the sequence.
    pub name: &'static str,
    /// The display.
    pub display: &'static str,
    /// The GPU entry, `None` for the default of the display.
    pub gpu: Option<&'static str>,
    /// The sources and their frame counts. A name ending in `!` is the
    /// source with no widths: a system that has no wide mode.
    pub steps: &'static [(&'static str, u32)],
}

/// The stored sequences: 240p, 480i, 240p and 480i again on a 15 kHz tube
/// with a GPU that sends interlace, so both holds show; a width change of
/// the SNES, which has a wide mode; a width change of a Mega Drive game
/// with no widths on a PC monitor, where each width has its own mode; and
/// a 480i frame that waits in a 224-line mode.
pub const SEQUENCES: [SequenceSpec; 4] = [
    SequenceSpec {
        name: "playstation_240p_480i",
        display: "generic_15",
        gpu: Some("pi5_rp1_dpi"),
        steps: &[
            ("ps1_320", 30),
            ("ps1_480i", 40),
            ("ps1_320", 40),
            ("ps1_480i", 70),
            ("ps1_320", 10),
        ],
    },
    SequenceSpec {
        name: "snes_width_wide_mode",
        display: "generic_15",
        gpu: None,
        steps: &[("snes_256", 30), ("snes_512", 30), ("snes_256", 30)],
    },
    SequenceSpec {
        name: "megadrive_width_no_wide_mode",
        display: "pc_31_120",
        gpu: None,
        steps: &[("md_320!", 30), ("md_256!", 30), ("md_320!", 60)],
    },
    SequenceSpec {
        name: "snes_224p_then_480i",
        display: "generic_15",
        gpu: Some("pi5_rp1_dpi"),
        steps: &[("snes_256", 30), ("ps1_480i", 10)],
    },
];

/// One frame of a sequence that the strip shows.
#[derive(Debug, Clone)]
pub struct Shot {
    /// The core frame, from 1.
    pub frame: u64,
    /// The source of the frame.
    pub source: String,
    /// What the engine did with the frame.
    pub directive: Directive,
    /// The mode on screen after the frame: width, lines, interlace, field
    /// rate in millihertz.
    pub mode: (u16, u16, bool, u64),
    /// The lines of the frame that the scene shows.
    pub crop_lines: u32,
    /// The lines of the frame that no line of the mode samples, by the tap
    /// rule of the reference renderer.
    pub dropped_lines: u32,
    /// The glass of the frame.
    pub glass: Rgb,
}

/// A sequence that ran.
#[derive(Debug)]
pub struct Sequence {
    /// The spec.
    pub spec: SequenceSpec,
    /// The frames where a modeset came.
    pub modesets: Vec<u64>,
    /// The frames of the strip: the first, the first of each new source,
    /// each modeset, the frame before each modeset, and the last.
    pub shots: Vec<Shot>,
}

/// Runs `spec` and keeps the glass of its strip at `tile` pixels.
///
/// # Errors
///
/// [`RunError`] for a name that does not parse or an engine that fails.
pub fn run_sequence(
    tables: &Tables,
    spec: &SequenceSpec,
    tile: (u32, u32),
) -> Result<Sequence, RunError> {
    let first = spec.steps.first().map_or("", |s| s.0);
    let case = tables.named_case_on(spec.display, spec.gpu, first.trim_end_matches('!'), &[])?;
    let settings = PolicySettings {
        horizontal: wide_strategy(source_of(tables, first)?.widths),
        ..case.settings.policy(case.source.arcade)?
    };
    let tv_standards = match &case.screen.profile.kind {
        MonitorKind::Tv { standard } => Some(vec![*standard]),
        MonitorKind::Crt { .. } | MonitorKind::FlatPanel { .. } => None,
    };
    let options = DisplayOptions {
        render: Render::Scene,
        tv_standards,
        ..DisplayOptions::default()
    };
    let display = FakeDisplay::with_options(SimClock::new(), case.screen.caps, options);
    let mut engine = VideoEngine::new(
        display,
        Monitor::Profile(case.screen.profile.clone()),
        settings,
    );
    engine.set_scanlines(case.settings.scanlines()?);
    engine.set_orientation(Orientation::NORMAL);
    let total: u32 = spec.steps.iter().map(|s| s.1).sum();
    let mut modesets = Vec::new();
    let mut shots = Vec::new();
    let mut frame_no = 0_u64;
    let mut pending: Option<Shot> = None;
    for (step, &(name, count)) in spec.steps.iter().enumerate() {
        let source = source_of(tables, name)?;
        let picture = pattern_of(&source);
        let data = picture.to_xrgb8888();
        let g = source.geometry;
        for i in 0..count {
            frame_no += 1;
            let game = Frame {
                data: &data,
                width: g.width,
                height: g.height,
                pitch: usize::from(g.width) * 4,
                format: PixelFormat::Xrgb8888,
            };
            let directive = engine.begin_frame(g);
            match engine.present(Some(game), None) {
                Ok(_) => engine
                    .wait_flip()
                    .map_err(|e| WireError::Engine(e.to_string()))?,
                Err(VideoError::NoMode) => continue,
                Err(e) => return Err(WireError::Engine(e.to_string()).into()),
            };
            let modeset = directive == Directive::Modeset;
            if modeset {
                modesets.push(frame_no);
            }
            let keep =
                frame_no == 1 || (i == 0 && step > 0) || modeset || u64::from(total) == frame_no;
            let shot = shot(
                &engine,
                &case.screen.profile,
                name,
                frame_no,
                directive,
                game,
                tile,
            )?;
            if modeset && let Some(before) = pending.take() {
                // The frame before a modeset shows what the hold showed.
                if shots.last().is_none_or(|s: &Shot| s.frame != before.frame) {
                    shots.push(before);
                }
            }
            if keep {
                shots.push(shot);
            } else {
                pending = Some(shot);
            }
        }
    }
    Ok(Sequence {
        spec: *spec,
        modesets,
        shots,
    })
}

/// The source `name`, or with a final `!` the same source with no widths.
fn source_of(tables: &Tables, name: &str) -> Result<Source, RunError> {
    let bare = name.trim_end_matches('!');
    let mut source = Source::named(bare, &tables.boards)?;
    if bare != name {
        source.widths = &[];
        source.name = name.to_owned();
    }
    Ok(source)
}

fn shot(
    engine: &VideoEngine<FakeDisplay>,
    profile: &neptron_types::MonitorProfile,
    source: &str,
    frame: u64,
    directive: Directive,
    game: Frame<'_>,
    tile: (u32, u32),
) -> Result<Shot, RunError> {
    let current = engine
        .current()
        .ok_or_else(|| WireError::Engine("no mode after a present".into()))?;
    let mode = current.read_back;
    let image = engine.display().screen().ok_or(WireError::NoScreen)?;
    let fb = Rgb::from_rgb(image.width(), image.height(), image.bytes().to_vec())
        .ok_or(WireError::NoScreen)?;
    let scene = engine
        .odren()
        .scene(Some(game), None)
        .ok_or(WireError::NoScreen)?;
    let glass = Glass::of(profile, &mode, None);
    Ok(Shot {
        frame,
        source: source.to_owned(),
        directive,
        mode: (
            mode.hactive,
            mode.vactive,
            mode.interlace,
            mode.field_rate_mhz(),
        ),
        crop_lines: scene.crop.h,
        dropped_lines: dropped_lines(scene.crop.h, scene.dest.h, scene.filter),
        glass: glass.render(&mode, &fb, glass.picture_size(tile)),
    })
}

/// The strip of `seq`: its shots in order, each with the frame number, the
/// source, the mode and what the engine did.
pub fn strip(seq: &Sequence, tile: (u32, u32)) -> (Rgb, Vec<Label>) {
    let cells: Vec<Cell<'_>> = seq
        .shots
        .iter()
        .map(|s| {
            let (what, color) = match s.directive {
                Directive::Modeset => ("MODESET".to_owned(), LOOK_COLOR),
                Directive::Waiting { stable_frames } => {
                    (format!("held, stable {stable_frames}"), NOTE_COLOR)
                }
                other => (format!("{other:?}").to_lowercase(), LABEL_COLOR),
            };
            Cell {
                picture: Some(&s.glass),
                lines: vec![
                    (format!("frame {} {}", s.frame, s.source), LABEL_COLOR),
                    (mode_text(s.mode), LABEL_COLOR),
                    (format!("{what}; {} lines shown", s.crop_lines), color),
                    (
                        format!("{} lines dropped", s.dropped_lines),
                        if s.dropped_lines > 0 {
                            LOOK_COLOR
                        } else {
                            LABEL_COLOR
                        },
                    ),
                ],
                mark: (s.directive == Directive::Modeset).then_some(LOOK_COLOR),
            }
        })
        .collect();
    let mut legend = card_legend();
    legend.push(
        "The cards of a sequence are drawn for the aspect of the core (ref: core 4:3); a sequence shows the mode changes and has no shape verdict."
            .to_owned(),
    );
    sheet(&cells, 5, tile, 4, &legend)
}

fn mode_text((w, h, interlace, rate): (u16, u16, bool, u64)) -> String {
    let rate = milli_text(rate);
    format!(
        "{w}x{h}{} {}Hz",
        scan_letter(interlace, false),
        &rate[..rate.len() - 1]
    )
}

/// The sequence as text: the modeset frames, then one line per shot.
pub fn sequence_text(seq: &Sequence) -> String {
    let mut out = format!(
        "{} on {}: modesets at frames {:?}\n",
        seq.spec.name, seq.spec.display, seq.modesets
    );
    for s in &seq.shots {
        out.push_str(&format!(
            "  frame {:>3} {:<10} {:<22} {:?}, {} lines shown, {} dropped\n",
            s.frame,
            s.source,
            mode_text(s.mode),
            s.directive,
            s.crop_lines,
            s.dropped_lines
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golden::workspace_root;

    /// The modesets of each stored sequence come where the debounce and
    /// the hold put them, and a 480i frame held in the 240p mode shows all
    /// its lines.
    #[test]
    fn the_sequences_change_mode_where_the_engine_rules_say() {
        let tables = Tables::load(&workspace_root()).unwrap();
        let runs: Vec<Sequence> = SEQUENCES
            .iter()
            .map(|s| run_sequence(&tables, s, (32, 24)).unwrap())
            .collect();
        let modesets: Vec<&[u64]> = runs.iter().map(|r| r.modesets.as_slice()).collect();
        assert_eq!(
            modesets,
            [&[1, 33, 93, 153][..], &[1], &[1, 33, 93], &[1, 33]]
        );
        let held = runs[0].shots.iter().find(|s| s.frame == 111).unwrap();
        assert!(matches!(held.directive, Directive::Waiting { .. }));
        assert_eq!((held.mode.1, held.mode.2), (240, false));
        assert_eq!(held.crop_lines, 480, "every line of the 480i frame");
        let before = runs[0].shots.iter().find(|s| s.frame == 152).unwrap();
        assert_eq!(before.crop_lines, 480);
        assert!(
            runs[1]
                .shots
                .iter()
                .all(|s| s.mode == runs[1].shots[0].mode)
        );
    }
}
