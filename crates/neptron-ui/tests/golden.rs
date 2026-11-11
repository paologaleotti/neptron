//! Every registered scene matches its golden image byte for byte. The
//! pixel path uses integer maths only, so every host gives the same bytes.
//! `cargo xtask golden` shows the difference and blesses a new golden.

#![forbid(unsafe_code)]

use std::path::PathBuf;

use neptron_ui::golden::{GOLDEN_DIR, GOLDEN_EXTENSION, Image};
use neptron_ui::registry;
use neptron_ui::{CpuCanvas, Theme, UiFrame};

#[test]
fn every_scene_matches_its_golden() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(GOLDEN_DIR);
    let mut failures = Vec::new();
    let mut frame = UiFrame::new();
    let mut canvas = CpuCanvas::new();
    for scene in registry::scenes() {
        registry::render(scene, &Theme::default(), &mut frame, &mut canvas);
        let actual = Image::from_canvas(&canvas);
        let path = dir.join(scene.name).with_extension(GOLDEN_EXTENSION);
        let verdict = match std::fs::read(&path) {
            Err(e) => format!("cannot read {}: {e}", path.display()),
            Ok(bytes) => match Image::from_ppm(&bytes) {
                Err(e) => format!("{}: {e}", path.display()),
                Ok(expected) => match expected.diff(&actual) {
                    None => continue,
                    Some(diff) => diff.to_string(),
                },
            },
        };
        failures.push(format!("{}: {verdict}", scene.name));
    }
    assert!(
        failures.is_empty(),
        "{} scene(s) do not match their golden; run `cargo xtask golden` for diff images:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
