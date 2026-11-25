//! The fit check: no text is cut, no text overlaps another text, and every
//! text stays in the safe area of the form. It reads the draw commands and
//! the tree of one frame, so it covers every screen and every later text
//! with no help from the widgets.
//!
//! The box of a text run is its position, its width in characters, and one
//! glyph high. A filled rectangle drawn after a text that covers part of it
//! hides that text, as a dialog hides the list under it: a hidden text
//! counts for nothing.

use crate::font;
use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::metrics::Metrics;
use crate::scene::DrawCommand;
use crate::screensaver::SCREENSAVER_SCREEN;
use crate::text;
use crate::tree::WidgetKind;

/// A fault of the layout of one frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    /// A visible text lies outside the area it must stay in: the safe
    /// area, or the canvas on a screen that uses the whole canvas.
    OutsideArea {
        /// The text.
        text: String,
        /// The box of the text.
        rect: Rect,
        /// The area.
        area: Rect,
    },
    /// A visible text lies outside the clip rectangle of its command.
    OutsideClip {
        /// The text.
        text: String,
        /// The box of the text.
        rect: Rect,
        /// The clip rectangle.
        clip: Rect,
    },
    /// Two visible texts cover the same pixels.
    Overlap {
        /// The text drawn first.
        first: String,
        /// The text drawn second.
        second: String,
        /// The pixels that both cover.
        at: Rect,
    },
    /// A row of the tree is cut.
    Clipped {
        /// The label of the row.
        label: String,
    },
}

/// A layout that is allowed but looks bad.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warning {
    /// A dialog message whose last line is one short word.
    ShortLastLine {
        /// The message.
        message: String,
        /// The last line.
        line: String,
    },
}

/// The result of [`check`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// The faults: a frame with one fails the check.
    pub faults: Vec<Fault>,
    /// The warnings: a frame with one passes.
    pub warnings: Vec<Warning>,
}

/// The longest word of the last line of a dialog message that counts as
/// one short word, in characters.
pub const SHORT_WORD_CHARS: usize = 5;

/// The screens that use the whole canvas, not only the safe area.
const WHOLE_CANVAS_SCREENS: [&str; 2] = [SCREENSAVER_SCREEN, "pattern/grid"];

/// Checks the frame `frame`, drawn with `metrics`.
pub fn check(frame: &UiFrame, metrics: &Metrics) -> Report {
    let mut report = Report::default();
    let area = if WHOLE_CANVAS_SCREENS.contains(&frame.tree.screen()) {
        metrics.canvas.rect()
    } else {
        metrics.safe
    };
    let visible = visible_texts(frame);
    for run in &visible {
        if !area.contains(&run.rect) {
            report.faults.push(Fault::OutsideArea {
                text: run.text.to_owned(),
                rect: run.rect,
                area,
            });
        }
        if let Some(clip) = run.clip.filter(|clip| !clip.contains(&run.rect)) {
            report.faults.push(Fault::OutsideClip {
                text: run.text.to_owned(),
                rect: run.rect,
                clip,
            });
        }
    }
    for (i, first) in visible.iter().enumerate() {
        for second in visible.iter().skip(i + 1) {
            if let Some(at) = first.rect.intersect(&second.rect) {
                report.faults.push(Fault::Overlap {
                    first: first.text.to_owned(),
                    second: second.text.to_owned(),
                    at,
                });
            }
        }
    }
    for row in frame.tree.rows() {
        if row.clipped {
            report.faults.push(Fault::Clipped {
                label: row.label.to_owned(),
            });
        }
        let message_w = row.rect.w.saturating_sub(2 * metrics.padding_px);
        if row.widget == WidgetKind::Dialog
            && let Some(line) = short_last_line(row.label, message_w)
        {
            report.warnings.push(Warning::ShortLastLine {
                message: row.label.to_owned(),
                line: line.to_owned(),
            });
        }
    }
    report
}

/// A text run that no later fill covers.
struct Run<'a> {
    text: &'a str,
    rect: Rect,
    clip: Option<Rect>,
}

fn visible_texts(frame: &UiFrame) -> Vec<Run<'_>> {
    let scene = &frame.scene;
    let mut runs: Vec<Run<'_>> = Vec::new();
    let mut clip = None;
    for command in scene.commands() {
        match *command {
            DrawCommand::Text { x, y, span, .. } => {
                let text = scene.text(span);
                let rect = Rect::new(x, y, font::text_width(text), font::GLYPH_HEIGHT);
                runs.push(Run { text, rect, clip });
            }
            DrawCommand::Rect { rect, .. } => {
                let fill = clip.map_or(Some(rect), |c: Rect| c.intersect(&rect));
                if let Some(fill) = fill {
                    runs.retain(|run| run.rect.intersect(&fill).is_none());
                }
            }
            DrawCommand::Clip(rect) => clip = rect,
            DrawCommand::Sprite { .. } => {}
        }
    }
    runs
}

/// The last line of `message` wrapped at `width_px`, when the message has
/// more than one line and that line is one word of [`SHORT_WORD_CHARS`] or
/// fewer.
fn short_last_line(message: &str, width_px: u32) -> Option<&str> {
    let lines = text::wrap(message, width_px);
    let (count, last) = lines.fold((0, ""), |(n, _), line| (n + 1, line));
    let word = last.trim();
    (count > 1
        && !word.is_empty()
        && !word.contains(' ')
        && word.chars().count() <= SHORT_WORD_CHARS)
        .then_some(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Color;
    use crate::metrics::Form;
    use crate::theme::Theme;
    use crate::tree::TreeRow;

    fn landscape() -> Metrics {
        Metrics::of(Form::Landscape, &Theme::default())
    }

    fn row(frame: &mut UiFrame, label: &str, clipped: bool) {
        frame.tree.push(TreeRow {
            widget: WidgetKind::Label,
            kind: None,
            label,
            value: None,
            source: None,
            rect: Rect::new(20, 20, 60, 12),
            focused: false,
            clipped,
        });
    }

    #[test]
    fn a_text_across_the_edge_of_the_safe_area_is_a_fault() {
        let m = landscape();
        let mut frame = UiFrame::new();
        frame.scene.text_run(m.safe.x - 1, 40, Color::WHITE, "Edge");
        let report = check(&frame, &m);
        assert!(
            matches!(report.faults.as_slice(), [Fault::OutsideArea { text, .. }] if text == "Edge"),
            "{report:?}"
        );
        let mut inside = UiFrame::new();
        inside.scene.text_run(m.safe.x, 40, Color::WHITE, "Edge");
        assert_eq!(check(&inside, &m), Report::default());
    }

    #[test]
    fn two_texts_that_overlap_by_one_pixel_are_a_fault() {
        let mut frame = UiFrame::new();
        frame.scene.text_run(40, 40, Color::WHITE, "abc");
        frame.scene.text_run(57, 51, Color::WHITE, "def");
        let report = check(&frame, &landscape());
        assert_eq!(
            report.faults,
            [Fault::Overlap {
                first: "abc".into(),
                second: "def".into(),
                at: Rect::new(57, 51, 1, 1)
            }]
        );
        let mut apart = UiFrame::new();
        apart.scene.text_run(40, 40, Color::WHITE, "abc");
        apart.scene.text_run(58, 40, Color::WHITE, "def");
        apart.scene.text_run(40, 52, Color::WHITE, "ghi");
        assert!(check(&apart, &landscape()).faults.is_empty());
    }

    #[test]
    fn a_text_under_a_dialog_is_no_fault() {
        let mut frame = UiFrame::new();
        frame
            .scene
            .text_run(40, 40, Color::WHITE, "Under the dialog");
        frame.scene.rect(Rect::new(30, 30, 200, 60), Color::BLACK);
        frame.scene.text_run(40, 40, Color::WHITE, "Dialog text");
        assert_eq!(check(&frame, &landscape()), Report::default());
    }

    #[test]
    fn a_text_half_under_a_dialog_rim_is_no_fault() {
        let mut frame = UiFrame::new();
        frame.scene.text_run(40, 40, Color::WHITE, "Half hidden");
        frame.scene.rect(Rect::new(70, 30, 1, 30), Color::BLACK);
        frame.scene.text_run(40, 40, Color::WHITE, "On top");
        assert_eq!(check(&frame, &landscape()), Report::default());
    }

    #[test]
    fn a_text_outside_its_clip_is_a_fault() {
        let mut frame = UiFrame::new();
        frame.scene.clip(Some(Rect::new(40, 40, 30, 12)));
        frame.scene.text_run(40, 40, Color::WHITE, "Too wide");
        frame.scene.clip(None);
        let report = check(&frame, &landscape());
        assert!(
            matches!(report.faults.as_slice(), [Fault::OutsideClip { .. }]),
            "{report:?}"
        );
    }

    #[test]
    fn a_cut_text_is_a_fault_and_a_shortened_one_is_not() {
        let rect = Rect::new(20, 20, 60, 12);
        let mut cut = UiFrame::new();
        let clipped = text::draw_in(
            &mut cut.scene,
            rect,
            text::Align::Left,
            Color::WHITE,
            "A very long game title",
        );
        row(&mut cut, "A very long game title", clipped);
        assert_eq!(
            check(&cut, &landscape()).faults,
            [Fault::Clipped {
                label: "A very long game title".into()
            }]
        );
        let mut short = UiFrame::new();
        text::draw_short(
            &mut short.scene,
            rect,
            text::Align::Left,
            Color::WHITE,
            "A very long game title",
        );
        row(&mut short, "A very long game title", false);
        assert_eq!(check(&short, &landscape()), Report::default());
    }

    #[test]
    fn a_message_that_ends_in_one_short_word_is_a_warning() {
        assert_eq!(
            short_last_line("The old picture comes back to us ok.", 204),
            Some("ok.")
        );
        assert_eq!(
            short_last_line("The old picture comes back in 10 s.", 204),
            None,
            "a unit stays with its number"
        );
        assert_eq!(
            short_last_line("The old picture returns in 10 s.", 204),
            None
        );
        assert_eq!(short_last_line("One line", 204), None);
        assert_eq!(short_last_line("one two three four", 50), Some("four"));
        assert_eq!(short_last_line("one two three fourteen", 50), None);
    }

    #[test]
    fn the_screensaver_may_use_the_whole_canvas() {
        let m = landscape();
        let mut frame = UiFrame::new();
        frame.tree.set_screen(SCREENSAVER_SCREEN);
        frame.scene.text_run(0, 0, Color::WHITE, "NEPTRON");
        assert!(check(&frame, &m).faults.is_empty());
    }
}
