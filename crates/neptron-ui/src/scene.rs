//! The scene: the draw commands of one UI frame. It holds four primitives
//! (rectangle, sprite, text run, clip) and never pixels.

use core::fmt;
use core::ops::Range;

use crate::canvas::{Canvas, SpriteId};
use crate::geom::{Color, Rect};

/// One draw command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawCommand {
    /// Fill a rectangle with one colour.
    Rect {
        /// The rectangle.
        rect: Rect,
        /// The fill colour.
        color: Color,
    },
    /// Draw a sprite with its top-left corner at `x`, `y`.
    Sprite {
        /// The sprite.
        id: SpriteId,
        /// Left edge.
        x: i32,
        /// Top edge.
        y: i32,
    },
    /// Draw a run of text with the built-in font. `x`, `y` is the top-left
    /// corner of the first glyph box.
    Text {
        /// Left edge.
        x: i32,
        /// Top edge of the glyph box.
        y: i32,
        /// The ink colour.
        color: Color,
        /// The text, in the text store of the scene.
        span: TextSpan,
    },
    /// Limit the commands that follow to a rectangle, or remove the limit.
    Clip(Option<Rect>),
}

/// A run of text in the text store of a [`UiScene`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextSpan {
    start: u32,
    end: u32,
}

/// The draw commands of one frame, in drawing order, and the text they use.
///
/// [`UiScene::clear`] keeps the memory, so a scene that is built again every
/// frame stops allocating once it has seen its largest frame.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiScene {
    commands: Vec<DrawCommand>,
    text: String,
}

impl UiScene {
    /// An empty scene.
    pub fn new() -> Self {
        Self::default()
    }

    /// Removes every command and all text, and keeps the memory.
    pub fn clear(&mut self) {
        self.commands.clear();
        self.text.clear();
    }

    /// The commands, in drawing order.
    pub fn commands(&self) -> &[DrawCommand] {
        &self.commands
    }

    /// The text of `span`. A span of a different scene gives an empty or a
    /// wrong string, never a panic.
    pub fn text(&self, span: TextSpan) -> &str {
        self.text.get(span.range()).unwrap_or("")
    }

    /// Adds a filled rectangle.
    pub fn rect(&mut self, rect: Rect, color: Color) {
        if !rect.is_empty() {
            self.commands.push(DrawCommand::Rect { rect, color });
        }
    }

    /// Adds a sprite.
    pub fn sprite(&mut self, id: SpriteId, x: i32, y: i32) {
        self.commands.push(DrawCommand::Sprite { id, x, y });
    }

    /// Adds a run of text.
    pub fn text_run(&mut self, x: i32, y: i32, color: Color, text: &str) {
        if text.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        self.push_text(x, y, color, start);
    }

    /// Adds a run of formatted text, with no temporary string.
    pub fn text_fmt(&mut self, x: i32, y: i32, color: Color, args: fmt::Arguments<'_>) {
        let start = self.text.len();
        // Writing to a `String` cannot fail: its `fmt::Write` impl always
        // returns `Ok`.
        let _ = fmt::Write::write_fmt(&mut self.text, args);
        if self.text.len() > start {
            self.push_text(x, y, color, start);
        }
    }

    /// Sets the clip rectangle for the commands that follow. `None` removes
    /// it.
    pub fn clip(&mut self, clip: Option<Rect>) {
        self.commands.push(DrawCommand::Clip(clip));
    }

    /// Draws every command on `canvas`, in order. The clip starts empty and
    /// is removed again at the end.
    pub fn render<C: Canvas + ?Sized>(&self, canvas: &mut C) {
        canvas.set_clip(None);
        for command in &self.commands {
            match *command {
                DrawCommand::Rect { rect, color } => canvas.fill_rect(rect, color),
                DrawCommand::Sprite { id, x, y } => canvas.draw_sprite(id, x, y),
                DrawCommand::Text { x, y, color, span } => {
                    canvas.draw_text(x, y, color, self.text(span));
                }
                DrawCommand::Clip(clip) => canvas.set_clip(clip),
            }
        }
        canvas.set_clip(None);
    }

    fn push_text(&mut self, x: i32, y: i32, color: Color, start: usize) {
        let span = TextSpan {
            start: offset(start),
            end: offset(self.text.len()),
        };
        self.commands.push(DrawCommand::Text { x, y, color, span });
    }

    #[cfg(test)]
    pub(crate) fn capacity(&self) -> (usize, usize) {
        (self.commands.capacity(), self.text.capacity())
    }
}

impl TextSpan {
    fn range(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }
}

// A scene with more than 4 GiB of text is far outside any UI frame; such
// offsets saturate, and `UiScene::text` then returns an empty string.
fn offset(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder {
        calls: Vec<String>,
    }

    impl Canvas for Recorder {
        fn width(&self) -> u32 {
            320
        }
        fn height(&self) -> u32 {
            240
        }
        fn set_clip(&mut self, clip: Option<Rect>) {
            self.calls.push(format!("clip {clip:?}"));
        }
        fn fill_rect(&mut self, rect: Rect, color: Color) {
            self.calls
                .push(format!("rect {rect:?} {:06x}", color.xrgb()));
        }
        fn draw_sprite(&mut self, id: SpriteId, x: i32, y: i32) {
            self.calls.push(format!("sprite {} {x} {y}", id.0));
        }
        fn draw_text(&mut self, x: i32, y: i32, _color: Color, text: &str) {
            self.calls.push(format!("text {x} {y} {text}"));
        }
    }

    #[test]
    fn render_replays_commands_in_order() {
        let mut scene = UiScene::new();
        scene.rect(Rect::new(0, 0, 4, 4), Color::WHITE);
        scene.clip(Some(Rect::new(1, 1, 2, 2)));
        scene.text_run(3, 4, Color::BLACK, "Hi");
        scene.sprite(SpriteId(7), 5, 6);
        scene.text_fmt(0, 0, Color::BLACK, format_args!("{}/{}", 3, 10));
        let mut canvas = Recorder::default();
        scene.render(&mut canvas);
        assert_eq!(
            canvas.calls,
            [
                "clip None",
                "rect Rect { x: 0, y: 0, w: 4, h: 4 } ffffff",
                "clip Some(Rect { x: 1, y: 1, w: 2, h: 2 })",
                "text 3 4 Hi",
                "sprite 7 5 6",
                "text 0 0 3/10",
                "clip None",
            ]
        );
    }

    #[test]
    fn empty_rects_and_texts_add_no_command() {
        let mut scene = UiScene::new();
        scene.rect(Rect::new(0, 0, 0, 5), Color::WHITE);
        scene.text_run(0, 0, Color::WHITE, "");
        scene.text_fmt(0, 0, Color::WHITE, format_args!(""));
        assert!(scene.commands().is_empty());
    }

    #[test]
    fn clear_keeps_the_memory() {
        let mut scene = UiScene::new();
        scene.text_run(0, 0, Color::WHITE, "some text");
        let capacity = scene.capacity();
        scene.clear();
        assert!(scene.commands().is_empty());
        assert_eq!(scene.capacity(), capacity);
    }

    #[test]
    fn a_span_of_another_scene_gives_no_panic() {
        let mut long = UiScene::new();
        long.text_run(0, 0, Color::WHITE, "a long run of text");
        let Some(DrawCommand::Text { span, .. }) = long.commands().first().copied() else {
            panic!("no text command");
        };
        assert_eq!(UiScene::new().text(span), "");
    }
}
