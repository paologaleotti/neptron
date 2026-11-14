//! A canvas that draws a scene again only when the scene changed.

use crate::canvas::CpuCanvas;
use crate::scene::UiScene;

/// A [`CpuCanvas`] and the scene it shows.
///
/// A UI on screen builds the same scene frame after frame while nothing
/// moves. [`SceneCanvas::paint`] draws only a scene that differs from the
/// one on the canvas, so a still UI costs one comparison of the draw
/// commands per frame, and a renderer that keeps the canvas between frames
/// sees the same pixels.
#[derive(Clone, Debug, Default)]
pub struct SceneCanvas {
    canvas: CpuCanvas,
    shown: UiScene,
    painted: bool,
    paints: u64,
}

impl SceneCanvas {
    /// A black canvas that shows no scene yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Draws `scene` when it differs from the scene on the canvas, or when
    /// the canvas shows none yet. Returns whether it drew. The copy of the
    /// scene keeps its memory, so a steady UI does not allocate.
    pub fn paint(&mut self, scene: &UiScene) -> bool {
        if self.painted && self.shown == *scene {
            return false;
        }
        scene.render(&mut self.canvas);
        self.shown.copy_from(scene);
        self.painted = true;
        self.paints = self.paints.saturating_add(1);
        true
    }

    /// How many scenes [`SceneCanvas::paint`] has drawn.
    pub fn paints(&self) -> u64 {
        self.paints
    }

    /// The pixels of the last scene drawn.
    pub fn canvas(&self) -> &CpuCanvas {
        &self.canvas
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{UI_HEIGHT, UI_WIDTH};
    use crate::geom::{Color, Rect};

    /// A scene that covers the whole canvas, as every screen does.
    fn scene(color: Color) -> UiScene {
        let mut scene = UiScene::new();
        scene.rect(Rect::new(0, 0, UI_WIDTH, UI_HEIGHT), Color::BLACK);
        scene.rect(Rect::new(0, 0, 10, 10), color);
        scene.text_run(20, 20, Color::WHITE, "Keep");
        scene
    }

    fn painted(scene: &UiScene) -> CpuCanvas {
        let mut canvas = CpuCanvas::new();
        scene.render(&mut canvas);
        canvas
    }

    #[test]
    fn the_first_scene_is_drawn_even_when_it_is_empty() {
        let mut canvas = SceneCanvas::new();
        assert!(canvas.paint(&UiScene::new()));
        assert_eq!(*canvas.canvas(), CpuCanvas::new());
    }

    #[test]
    fn an_unchanged_scene_is_not_drawn_again() {
        let mut canvas = SceneCanvas::new();
        let red = scene(Color::rgb(0xFF, 0, 0));
        assert!(canvas.paint(&red));
        assert!(!canvas.paint(&red.clone()));
        assert_eq!(*canvas.canvas(), painted(&red));
        assert_eq!(canvas.paints(), 1);
    }

    #[test]
    fn a_changed_scene_is_drawn_and_then_kept() {
        let mut canvas = SceneCanvas::new();
        let red = scene(Color::rgb(0xFF, 0, 0));
        let blue = scene(Color::rgb(0, 0, 0xFF));
        assert!(canvas.paint(&red));
        assert!(canvas.paint(&blue));
        assert_eq!(*canvas.canvas(), painted(&blue));
        assert!(!canvas.paint(&blue));
        assert!(canvas.paint(&red));
        assert_eq!(*canvas.canvas(), painted(&red));
    }

    #[test]
    fn a_change_of_text_alone_is_drawn() {
        let mut canvas = SceneCanvas::new();
        let mut nine = scene(Color::WHITE);
        nine.text_run(0, 0, Color::WHITE, "9 s");
        let mut eight = scene(Color::WHITE);
        eight.text_run(0, 0, Color::WHITE, "8 s");
        assert!(canvas.paint(&nine));
        assert!(canvas.paint(&eight));
        assert_eq!(*canvas.canvas(), painted(&eight));
    }

    #[test]
    fn the_copy_of_the_scene_keeps_its_memory() {
        let mut copy = UiScene::new();
        let big = scene(Color::WHITE);
        copy.copy_from(&big);
        let capacity = copy.capacity();
        copy.copy_from(&UiScene::new());
        copy.copy_from(&big);
        assert_eq!(copy, big);
        assert_eq!(copy.capacity(), capacity);
    }
}
