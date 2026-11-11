//! The output of one UI frame: scene, widget tree and intents.

use crate::canvas::Canvas;
use crate::intent::Intent;
use crate::scene::UiScene;
use crate::tree::UiTree;

/// Everything one UI frame gives the app.
///
/// Make one at launch and pass it to every frame. [`UiFrame::clear`] keeps
/// the memory, so a frame of a static screen does not allocate once the
/// frame has seen it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiFrame {
    /// The draw commands.
    pub scene: UiScene,
    /// The visible rows, for the control API.
    pub tree: UiTree,
    /// The requests to the app, in order.
    pub intents: Vec<Intent>,
}

impl UiFrame {
    /// An empty frame.
    pub fn new() -> Self {
        Self::default()
    }

    /// Empties the scene, the tree and the intents, and keeps the memory.
    pub fn clear(&mut self) {
        self.scene.clear();
        self.tree.clear();
        self.intents.clear();
    }

    /// Draws the scene on `canvas`.
    pub fn render<C: Canvas + ?Sized>(&self, canvas: &mut C) {
        self.scene.render(canvas);
    }
}
