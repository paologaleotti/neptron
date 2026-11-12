//! The UI commands of xtask. `cargo xtask font` generates the glyph table
//! of `neptron-ui` from the vendored BDF font. `shot`, `gallery` and
//! `golden` render the scenes of the `neptron-ui` registry to look at them
//! and to compare them with their golden images.

mod font;
mod image;
pub(crate) mod png;

use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use neptron_ui::golden::{GOLDEN_DIR, GOLDEN_EXTENSION, GoldenError, Image};
use neptron_ui::registry::{self, SceneDef};
use neptron_ui::{CpuCanvas, Theme, UiFrame};

use crate::repo::Repo;

/// Directory of the UI crate, relative to the repository root.
const UI_CRATE: &str = "crates/neptron-ui";
/// Output directory, relative to the repository root.
const OUT_DIR: &str = "target/ui";
/// Enlargement of the second shot.
const SHOT_SCALE: u32 = 4;

/// An error of a UI command.
#[derive(Debug, thiserror::Error)]
pub enum UiError {
    /// The arguments are wrong.
    #[error("usage: {0}")]
    Usage(&'static str),
    /// No scene has this name.
    #[error("no scene {name:?}; the scenes are: {known}")]
    UnknownScene {
        /// The name given.
        name: String,
        /// Every registered name.
        known: String,
    },
    /// A file cannot be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The error of the operating system.
        source: io::Error,
    },
    /// A golden file is not a valid image.
    #[error("{path}: {source}")]
    Golden {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        source: GoldenError,
    },
    /// The font file cannot be converted.
    #[error("{path}: {source}")]
    Font {
        /// The BDF file.
        path: PathBuf,
        /// What is wrong.
        source: font::BdfError,
    },
    /// The repository cannot be found.
    #[error(transparent)]
    Repo(#[from] crate::repo::RepoError),
}

/// Runs `cargo xtask font`: writes the glyph table of `neptron-ui` from
/// the vendored BDF file.
pub fn font(args: &[&str]) -> Result<ExitCode, UiError> {
    if !args.is_empty() {
        return Err(UiError::Usage("cargo xtask font"));
    }
    let root = Repo::discover()?.root;
    let table = font::generate_from(&root)?;
    let path = root.join(font::OUT);
    write(&path, table.as_bytes())?;
    println!("{} (from {})", path.display(), font::BDF);
    Ok(ExitCode::SUCCESS)
}

/// Runs `cargo xtask shot <scene>`: one PNG at 1x and one at 4x.
pub fn shot(args: &[&str]) -> Result<ExitCode, UiError> {
    let [name] = args else {
        return Err(UiError::Usage("cargo xtask shot <scene>"));
    };
    let scene = find(name)?;
    let root = Repo::discover()?.root;
    let image = render(scene);
    let base = root.join(OUT_DIR).join("shot").join(scene.name);
    let plain = base.with_extension("png");
    let large = with_suffix(&base, &format!("@{SHOT_SCALE}x.png"));
    write(
        &plain,
        &png::encode(image.width(), image.height(), image.rgb()),
    )?;
    let scaled = image::scale(&image, SHOT_SCALE);
    write(
        &large,
        &png::encode(scaled.width, scaled.height, &scaled.rgb),
    )?;
    println!("{}\n{}", plain.display(), large.display());
    Ok(ExitCode::SUCCESS)
}

/// Runs `cargo xtask gallery`: every scene on one contact sheet.
pub fn gallery(args: &[&str]) -> Result<ExitCode, UiError> {
    if !args.is_empty() {
        return Err(UiError::Usage("cargo xtask gallery"));
    }
    let root = Repo::discover()?.root;
    let tiles: Vec<(&str, Image)> = registry::scenes().map(|s| (s.name, render(s))).collect();
    let sheet = image::contact_sheet(&tiles);
    let path = root.join(OUT_DIR).join("gallery.png");
    write(&path, &png::encode(sheet.width, sheet.height, &sheet.rgb))?;
    println!("{} ({} scenes)", path.display(), tiles.len());
    Ok(ExitCode::SUCCESS)
}

/// Runs `cargo xtask golden [--bless] [scene...]`. Without `--bless` it
/// compares each scene with its golden image, byte for byte, and fails on
/// any difference. With `--bless` it writes the golden image of each scene
/// that differs or has none.
pub fn golden(args: &[&str]) -> Result<ExitCode, UiError> {
    let bless = args.contains(&"--bless");
    let names: Vec<&str> = args.iter().copied().filter(|a| *a != "--bless").collect();
    if names.iter().any(|n| n.starts_with('-')) {
        return Err(UiError::Usage("cargo xtask golden [--bless] [scene...]"));
    }
    let scenes: Vec<&SceneDef> = if names.is_empty() {
        registry::scenes().collect()
    } else {
        names.iter().map(|n| find(n)).collect::<Result<_, _>>()?
    };
    let root = Repo::discover()?.root;
    let golden_dir = root.join(UI_CRATE).join(GOLDEN_DIR);
    let mut failures = 0;
    for scene in &scenes {
        let path = golden_path(&golden_dir, scene.name);
        let actual = render(scene);
        let verdict = compare(&path, &actual)?;
        match (&verdict, bless) {
            (Verdict::Same, _) => println!("same     {}", scene.name),
            (_, true) => {
                write(&path, &actual.to_ppm())?;
                println!("blessed  {} ({})", scene.name, verdict.summary());
            }
            (_, false) => {
                failures += 1;
                println!("FAILED   {}: {}", scene.name, verdict.summary());
                if let Verdict::Differs(expected, _) = &verdict {
                    let dir = root.join(OUT_DIR).join("golden");
                    report_images(&dir, scene.name, expected, &actual)?;
                }
            }
        }
    }
    if names.is_empty() {
        for stale in stale_goldens(&golden_dir)? {
            failures += 1;
            println!(
                "STALE    {}: no scene has this golden; delete it by hand",
                stale.display()
            );
        }
    }
    if failures > 0 {
        println!(
            "{failures} golden check(s) failed. Look at target/ui/golden, then bless with --bless only if the change is intended."
        );
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

enum Verdict {
    Same,
    Missing,
    Differs(Image, neptron_ui::golden::Diff),
}

impl Verdict {
    fn summary(&self) -> String {
        match self {
            Self::Same => "same".into(),
            Self::Missing => "no golden image yet".into(),
            Self::Differs(_, diff) => diff.to_string(),
        }
    }
}

fn compare(path: &Path, actual: &Image) -> Result<Verdict, UiError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Verdict::Missing),
        Err(source) => {
            return Err(UiError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let expected = Image::from_ppm(&bytes).map_err(|source| UiError::Golden {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(match expected.diff(actual) {
        None => Verdict::Same,
        Some(diff) => Verdict::Differs(expected, diff),
    })
}

fn report_images(dir: &Path, name: &str, expected: &Image, actual: &Image) -> Result<(), UiError> {
    write_report_images(&dir.join(name), expected, actual)
}

/// Writes `<base>.expected.png`, `<base>.actual.png` and `<base>.diff.png`
/// for a picture that differs from its golden image, and prints their
/// paths. `os boot-check` uses it too.
pub(crate) fn write_report_images(
    base: &Path,
    expected: &Image,
    actual: &Image,
) -> Result<(), UiError> {
    let mut written = Vec::new();
    for (suffix, img) in [
        (".expected.png", image::from_golden(expected)),
        (".actual.png", image::from_golden(actual)),
        (".diff.png", image::diff(expected, actual)),
    ] {
        let path = with_suffix(base, suffix);
        write(&path, &png::encode(img.width, img.height, &img.rgb))?;
        written.push(path);
    }
    for path in written {
        println!("         {}", path.display());
    }
    Ok(())
}

fn stale_goldens(dir: &Path) -> Result<Vec<PathBuf>, UiError> {
    let mut found = Vec::new();
    collect_goldens(dir, &mut found)?;
    let known: Vec<PathBuf> = registry::scenes()
        .map(|s| golden_path(dir, s.name))
        .collect();
    found.retain(|p| !known.contains(p));
    found.sort();
    Ok(found)
}

fn collect_goldens(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), UiError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(UiError::Io {
                path: dir.to_path_buf(),
                source,
            });
        }
    };
    for entry in entries {
        let path = entry
            .map_err(|source| UiError::Io {
                path: dir.to_path_buf(),
                source,
            })?
            .path();
        if path.is_dir() {
            collect_goldens(&path, out)?;
        } else if path.extension().is_some_and(|e| e == GOLDEN_EXTENSION) {
            out.push(path);
        }
    }
    Ok(())
}

fn find(name: &str) -> Result<&'static SceneDef, UiError> {
    registry::find(name).ok_or_else(|| UiError::UnknownScene {
        name: name.to_owned(),
        known: registry::scenes()
            .map(|s| s.name)
            .collect::<Vec<_>>()
            .join(", "),
    })
}

fn render(scene: &SceneDef) -> Image {
    let mut frame = UiFrame::new();
    let mut canvas = CpuCanvas::new();
    registry::render(scene, &Theme::default(), &mut frame, &mut canvas);
    Image::from_canvas(&canvas)
}

fn golden_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(name).with_extension(GOLDEN_EXTENSION)
}

fn with_suffix(base: &Path, suffix: &str) -> PathBuf {
    let mut s = base.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), UiError> {
    let io = |source| UiError::Io {
        path: path.to_path_buf(),
        source,
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io)?;
    }
    std::fs::write(path, bytes).map_err(io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_paths_follow_the_scene_name() {
        let dir = Path::new("/g");
        assert_eq!(
            golden_path(dir, "widgets/settings-list"),
            Path::new("/g/widgets/settings-list.ppm")
        );
        assert_eq!(
            with_suffix(Path::new("/o/widgets/list"), "@4x.png"),
            Path::new("/o/widgets/list@4x.png")
        );
    }

    #[test]
    fn unknown_scenes_list_the_known_ones() {
        let err = find("widgets/nothing").expect_err("no such scene");
        let text = err.to_string();
        assert!(
            text.contains("widgets/nothing") && text.contains("widgets/list"),
            "{text}"
        );
    }

    #[test]
    fn a_missing_golden_is_reported_not_an_error() {
        let actual = render(find("widgets/list").expect("registered"));
        let verdict = compare(Path::new("/nonexistent/golden.ppm"), &actual).expect("no io error");
        assert!(matches!(verdict, Verdict::Missing));
    }

    #[test]
    fn bad_arguments_are_rejected() {
        assert!(matches!(font(&["x"]), Err(UiError::Usage(_))));
        assert!(matches!(shot(&[]), Err(UiError::Usage(_))));
        assert!(matches!(gallery(&["x"]), Err(UiError::Usage(_))));
        assert!(matches!(golden(&["--force"]), Err(UiError::Usage(_))));
    }
}
