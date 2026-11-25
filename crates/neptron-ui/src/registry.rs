//! The scene registry. A scene is a screen or a template with fixture data,
//! under a name such as `widgets/list`. It renders in each [`Form`]; a
//! [`Shot`] is a scene in one form, with a golden image. `cargo xtask shot`,
//! `gallery` and `golden` render the shots.

mod demos;

use crate::canvas::CpuCanvas;
use crate::frame::UiFrame;
use crate::geom::Color;
use crate::metrics::Form;
use crate::theme::Theme;

/// One registered scene.
#[derive(Clone, Copy, Debug)]
pub struct SceneDef {
    /// The name: lower-case words joined by `-`, in groups joined by `/`.
    /// It is also the path of the golden image of the landscape form.
    pub name: &'static str,
    /// Builds the frame of the scene in a form from its fixture data.
    pub build: fn(&mut UiFrame, &Theme, Form),
    /// The forms with a golden image. Every scene renders in every form.
    pub forms: &'static [Form],
}

/// A scene in one form: one picture, with a golden image.
#[derive(Clone, Copy, Debug)]
pub struct Shot {
    /// The scene.
    pub scene: &'static SceneDef,
    /// The form.
    pub form: Form,
}

/// The forms of a scene with a landscape golden only.
pub const LANDSCAPE: &[Form] = &[Form::Landscape];
/// The forms of a scene with a landscape and a portrait golden.
pub const BOTH: &[Form] = &[Form::Landscape, Form::Portrait];
/// The first group of the id of a portrait shot. No scene name starts with
/// it.
pub const PORTRAIT_GROUP: &str = "portrait";

impl Shot {
    /// The id of the shot, which is also the path of its golden image: the
    /// scene name in landscape, `portrait/` and the name in portrait.
    pub fn id(&self) -> String {
        match self.form {
            Form::Landscape => self.scene.name.to_owned(),
            Form::Portrait => format!("{PORTRAIT_GROUP}/{}", self.scene.name),
        }
    }
}

/// Every registered scene: the template scenes, then the screens.
pub fn scenes() -> impl Iterator<Item = &'static SceneDef> {
    demos::SCENES.iter().chain(crate::screens::SCENES)
}

/// Every shot with a golden image: each scene in each of its forms, the
/// landscape shots first.
pub fn shots() -> impl Iterator<Item = Shot> {
    [Form::Landscape, Form::Portrait]
        .into_iter()
        .flat_map(|form| {
            scenes()
                .filter(move |scene| scene.forms.contains(&form))
                .map(move |scene| Shot { scene, form })
        })
}

/// The scene with this name.
pub fn find(name: &str) -> Option<&'static SceneDef> {
    scenes().find(|s| s.name == name)
}

/// The shot with this id, which [`Shot::id`] gives: a scene name, or
/// `portrait/` and a scene name. A portrait shot of a scene with no
/// portrait golden is found too, to look at it.
pub fn find_shot(id: &str) -> Option<Shot> {
    let (name, form) = match id
        .strip_prefix(PORTRAIT_GROUP)
        .and_then(|rest| rest.strip_prefix('/'))
    {
        Some(name) => (name, Form::Portrait),
        None => (id, Form::Landscape),
    };
    find(name).map(|scene| Shot { scene, form })
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

/// Builds `scene` in `form` into `frame` and draws it on `canvas`, which
/// is made again when it has another size. The canvas is cleared to black
/// first, so the result does not depend on what it held.
pub fn render(
    scene: &SceneDef,
    theme: &Theme,
    form: Form,
    frame: &mut UiFrame,
    canvas: &mut CpuCanvas,
) {
    frame.clear();
    (scene.build)(frame, theme, form);
    if canvas.size() != form.size() {
        *canvas = CpuCanvas::with_size(form.size());
    }
    canvas.clear(Color::BLACK);
    frame.render(canvas);
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(
            names
                .iter()
                .all(|n| n.split('/').next() != Some(PORTRAIT_GROUP)),
            "a scene name never starts with the portrait group"
        );
    }

    #[test]
    fn shots_have_ids_that_find_them() {
        let shots: Vec<_> = shots().collect();
        assert_eq!(
            shots.iter().filter(|s| s.form == Form::Landscape).count(),
            scenes().count()
        );
        assert_eq!(
            shots.iter().filter(|s| s.form == Form::Portrait).count(),
            36
        );
        for shot in &shots {
            let found = find_shot(&shot.id()).expect("found by its id");
            assert_eq!((found.scene.name, found.form), (shot.scene.name, shot.form));
        }
        let portrait = find_shot("portrait/screens/home").expect("a portrait shot");
        assert_eq!(portrait.id(), "portrait/screens/home");
        assert_eq!(portrait.form, Form::Portrait);
        assert!(find_shot("portrait/").is_none());
        assert!(find_shot("portrait/widgets/none").is_none());
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
    fn every_scene_keeps_its_rows_in_the_safe_area_in_both_forms() {
        for form in [Form::Landscape, Form::Portrait] {
            let safe = crate::Metrics::of(form, &Theme::default()).safe;
            for scene in scenes() {
                let mut frame = UiFrame::new();
                (scene.build)(&mut frame, &Theme::default(), form);
                for row in frame.tree.rows() {
                    assert!(safe.contains(&row.rect), "{form:?} {}: {row:?}", scene.name);
                }
            }
        }
    }

    #[test]
    fn every_scene_fits_in_both_forms() {
        let mut failures = Vec::new();
        let mut frame = UiFrame::new();
        for form in [Form::Landscape, Form::Portrait] {
            let metrics = crate::Metrics::of(form, &Theme::default());
            for scene in scenes() {
                frame.clear();
                (scene.build)(&mut frame, &Theme::default(), form);
                let report = crate::fit::check(&frame, &metrics);
                for fault in report.faults {
                    failures.push(format!("{form:?} {}: {fault:?}", scene.name));
                }
            }
        }
        assert!(
            failures.is_empty(),
            "{} fault(s):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn rendering_is_repeatable() {
        for scene in scenes() {
            let mut frame = UiFrame::new();
            let mut first = CpuCanvas::new();
            render(
                scene,
                &Theme::default(),
                Form::Landscape,
                &mut frame,
                &mut first,
            );
            let mut second = CpuCanvas::new();
            second.clear(Color::WHITE);
            render(
                scene,
                &Theme::default(),
                Form::Landscape,
                &mut frame,
                &mut second,
            );
            assert_eq!(first, second, "{}", scene.name);
        }
    }

    #[test]
    fn a_second_frame_of_a_static_scene_does_not_allocate() {
        for scene in scenes() {
            let mut frame = UiFrame::new();
            let mut canvas = CpuCanvas::new();
            render(
                scene,
                &Theme::default(),
                Form::Landscape,
                &mut frame,
                &mut canvas,
            );
            let before = (
                frame.scene.capacity(),
                frame.tree.capacity(),
                frame.intents.capacity(),
            );
            let pixels = canvas.as_bytes().as_ptr();
            render(
                scene,
                &Theme::default(),
                Form::Landscape,
                &mut frame,
                &mut canvas,
            );
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
