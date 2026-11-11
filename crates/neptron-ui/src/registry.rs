//! The scene registry. A scene is a screen or a template with fixture data,
//! under a name such as `widgets/list`. `cargo xtask shot`, `gallery` and
//! `golden` render the scenes, and each scene has a golden image.

mod demos;

use crate::canvas::CpuCanvas;
use crate::frame::UiFrame;
use crate::geom::Color;
use crate::theme::Theme;

/// One registered scene.
#[derive(Clone, Copy, Debug)]
pub struct SceneDef {
    /// The name: lower-case words joined by `-`, in groups joined by `/`.
    /// It is also the path of the golden image.
    pub name: &'static str,
    /// Builds the frame of the scene from its fixture data.
    pub build: fn(&mut UiFrame, &Theme),
}

/// Every registered scene: the template scenes, then the screens.
pub fn scenes() -> impl Iterator<Item = &'static SceneDef> {
    demos::SCENES.iter().chain(crate::screens::SCENES)
}

/// The scene with this name.
pub fn find(name: &str) -> Option<&'static SceneDef> {
    scenes().find(|s| s.name == name)
}

/// True when `name` is a valid scene name.
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

/// Builds `scene` into `frame` and draws it on `canvas`. The canvas is
/// cleared to black first, so the result does not depend on what it held.
pub fn render(scene: &SceneDef, theme: &Theme, frame: &mut UiFrame, canvas: &mut CpuCanvas) {
    frame.clear();
    (scene.build)(frame, theme);
    canvas.clear(Color::BLACK);
    frame.render(canvas);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::SAFE_AREA;

    #[test]
    fn names_are_valid_and_unique() {
        let names: Vec<_> = scenes().map(|s| s.name).collect();
        for (i, name) in names.iter().enumerate() {
            assert!(is_valid_name(name), "{name:?}");
            assert!(
                !names[i + 1..].contains(name),
                "{name:?} is registered twice"
            );
        }
        assert!(find("widgets/list").is_some());
        assert!(find("widgets/none").is_none());
    }

    #[test]
    fn name_rules() {
        assert!(is_valid_name("settings/display"));
        assert!(is_valid_name("widgets/settings-list"));
        for bad in ["", "/a", "a/", "A", "a b", "a/../b", "-a", "a-", "a//b"] {
            assert!(!is_valid_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn every_scene_keeps_its_rows_in_the_safe_area() {
        for scene in scenes() {
            let mut frame = UiFrame::new();
            (scene.build)(&mut frame, &Theme::default());
            for row in frame.tree.rows() {
                assert!(SAFE_AREA.contains(&row.rect), "{}: {row:?}", scene.name);
            }
        }
    }

    #[test]
    fn rendering_is_repeatable() {
        for scene in scenes() {
            let mut frame = UiFrame::new();
            let mut first = CpuCanvas::new();
            render(scene, &Theme::default(), &mut frame, &mut first);
            let mut second = CpuCanvas::new();
            second.clear(Color::WHITE);
            render(scene, &Theme::default(), &mut frame, &mut second);
            assert_eq!(first, second, "{}", scene.name);
        }
    }

    #[test]
    fn a_second_frame_of_a_static_scene_does_not_allocate() {
        for scene in scenes() {
            let mut frame = UiFrame::new();
            let mut canvas = CpuCanvas::new();
            render(scene, &Theme::default(), &mut frame, &mut canvas);
            let before = (
                frame.scene.capacity(),
                frame.tree.capacity(),
                frame.intents.capacity(),
            );
            let pixels = canvas.as_bytes().as_ptr();
            render(scene, &Theme::default(), &mut frame, &mut canvas);
            let after = (
                frame.scene.capacity(),
                frame.tree.capacity(),
                frame.intents.capacity(),
            );
            assert_eq!(before, after, "{}", scene.name);
            assert_eq!(pixels, canvas.as_bytes().as_ptr(), "{}", scene.name);
        }
    }
}
