//! Golden checks against files in a temporary directory.

use std::fs;
use std::path::PathBuf;

use neptron_testkit::golden::{GoldenError, encode_ppm};
use neptron_testkit::{Golden, RgbImage};

fn dirs(test: &str) -> (PathBuf, PathBuf) {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("testkit-golden")
        .join(test);
    // A run before this one can leave files; each test starts from nothing.
    let _ = fs::remove_dir_all(&root);
    (root.join("golden"), root.join("actual"))
}

fn image(value: u8) -> RgbImage {
    RgbImage::from_rgb(2, 2, vec![value; 12]).unwrap()
}

#[test]
fn an_equal_image_passes_and_writes_nothing() {
    let (golden_dir, actual_dir) = dirs("equal");
    fs::create_dir_all(golden_dir.join("ui")).unwrap();
    fs::write(golden_dir.join("ui/home.ppm"), encode_ppm(&image(7))).unwrap();
    let golden = Golden::new(&golden_dir, &actual_dir);
    golden.check("ui/home", &image(7)).unwrap();
    assert!(!actual_dir.exists());
}

#[test]
fn a_missing_golden_writes_the_actual_image_only() {
    let (golden_dir, actual_dir) = dirs("missing");
    let golden = Golden::new(&golden_dir, &actual_dir);
    let err = golden.check("new", &image(3)).unwrap_err();
    let GoldenError::Missing {
        golden: path,
        actual,
    } = &err
    else {
        panic!("expected Missing, got {err:?}");
    };
    assert_eq!(path, &golden_dir.join("new.ppm"));
    assert!(!path.exists(), "a check never writes a golden");
    assert_eq!(fs::read(actual).unwrap(), encode_ppm(&image(3)));
    assert!(err.to_string().contains("Look at it before you copy it"));
}

#[test]
fn a_different_image_reports_the_pixels_and_keeps_the_golden() {
    let (golden_dir, actual_dir) = dirs("mismatch");
    fs::create_dir_all(&golden_dir).unwrap();
    let path = golden_dir.join("frame.ppm");
    fs::write(&path, encode_ppm(&image(0))).unwrap();
    let golden = Golden::new(&golden_dir, &actual_dir);
    let err = golden.check("frame", &image(5)).unwrap_err();
    let GoldenError::Mismatch { report, actual, .. } = &err else {
        panic!("expected Mismatch, got {err:?}");
    };
    assert!(
        report
            .starts_with("4 pixels differ, in x 0 to 1 and y 0 to 1; largest channel difference 5")
    );
    assert_eq!(fs::read(actual).unwrap(), encode_ppm(&image(5)));
    assert_eq!(fs::read(&path).unwrap(), encode_ppm(&image(0)));
    assert!(err.to_string().contains("4 pixels differ"));
}

#[test]
fn a_broken_golden_is_an_error() {
    let (golden_dir, actual_dir) = dirs("broken");
    fs::create_dir_all(&golden_dir).unwrap();
    fs::write(golden_dir.join("bad.ppm"), b"P3\n1 1\n255\n").unwrap();
    let golden = Golden::new(&golden_dir, &actual_dir);
    assert!(matches!(
        golden.check("bad", &image(0)),
        Err(GoldenError::BadGolden { .. })
    ));
    assert!(matches!(
        golden.check("../bad", &image(0)),
        Err(GoldenError::BadName { .. })
    ));
}
