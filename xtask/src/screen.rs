//! `cargo xtask screen`: starts the screen simulator of `neptron-testkit`
//! (its example `screen`) with the same arguments, under the machine-load
//! rules, then turns the PPM files it wrote under `target/screen/` into
//! PNG files and draws the labels of the contact sheets with the UI font.
//! The simulator itself lives in `neptron_testkit::screen`, so xtask does
//! not depend on the crates of the product.

use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use neptron_ui::font;
use neptron_ui::golden::{GoldenError, Image};
use serde_json::Value;

use crate::load::{LoadError, LoadPolicy, lock_dir};
use crate::repo::{self, Repo, RepoError};
use crate::ui::png;

/// The output directory of the simulator, relative to the repository root.
const OUT_DIR: &str = "target/screen";
/// The suffix of the labels of a contact sheet beside its PPM file.
const LABELS_SUFFIX: &str = ".labels.json";

/// An error of `cargo xtask screen`.
#[derive(Debug, thiserror::Error)]
pub enum ScreenError {
    /// The repository cannot be found, or cargo cannot start.
    #[error(transparent)]
    Repo(#[from] RepoError),
    /// The build slot cannot be taken.
    #[error(transparent)]
    Load(#[from] LoadError),
    /// A file cannot be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The error of the operating system.
        source: io::Error,
    },
    /// A PPM file of the simulator does not decode.
    #[error("{path}: {source}")]
    Ppm {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        source: GoldenError,
    },
    /// A labels file is not the JSON the simulator writes.
    #[error("{path}: not a list of labels")]
    Labels {
        /// The file.
        path: PathBuf,
    },
}

/// Runs `cargo xtask screen ...`: the example `screen` of
/// `neptron-testkit` with `args`, then the PNG files.
///
/// # Errors
///
/// [`ScreenError`] when cargo cannot start or a file of the simulator
/// cannot be converted.
pub fn run(args: &[&str]) -> Result<ExitCode, ScreenError> {
    let repo = Repo::discover()?;
    let policy = LoadPolicy::from_env()?;
    let success = {
        let slot = policy.acquire(&lock_dir(&repo.git_common_dir))?;
        if let Some(slot) = &slot {
            eprintln!("xtask: holding build slot {}", slot.slot);
        }
        let base = [
            "run",
            "--quiet",
            "-p",
            "neptron-testkit",
            "--example",
            "screen",
            "--",
        ];
        let cargo_args: Vec<&str> = base.iter().chain(args).copied().collect();
        repo::status(policy.cargo(&cargo_args).current_dir(&repo.root))?.success()
    };
    let converted = convert_tree(&repo.root.join(OUT_DIR))?;
    if converted > 0 {
        eprintln!("xtask: wrote {converted} PNG file(s) under {OUT_DIR}");
    }
    Ok(if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Turns every PPM file under `dir` into a PNG file and removes the PPM;
/// a PPM with a labels file beside it gets its labels first. Returns the
/// number of files written.
fn convert_tree(dir: &Path) -> Result<usize, ScreenError> {
    let mut ppms = Vec::new();
    collect(dir, &mut ppms)?;
    for ppm in &ppms {
        let image = read_ppm(ppm)?;
        let (width, height) = (image.width(), image.height());
        let mut rgb = image.rgb().to_vec();
        let labels = labels_path(ppm);
        if labels.exists() {
            draw_labels(&labels, width, height, &mut rgb)?;
            remove(&labels)?;
        }
        let png_path = ppm.with_extension("png");
        std::fs::write(&png_path, png::encode(width, height, &rgb)).map_err(|source| {
            ScreenError::Io {
                path: png_path.clone(),
                source,
            }
        })?;
        remove(ppm)?;
    }
    Ok(ppms.len())
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), ScreenError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(ScreenError::Io {
                path: dir.to_path_buf(),
                source,
            });
        }
    };
    for entry in entries {
        let path = entry
            .map_err(|source| ScreenError::Io {
                path: dir.to_path_buf(),
                source,
            })?
            .path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "ppm") {
            out.push(path);
        }
    }
    Ok(())
}

/// `sheet.ppm` has its labels in `sheet.labels.json`.
fn labels_path(ppm: &Path) -> PathBuf {
    let stem = ppm.file_stem().map(|s| s.to_string_lossy().into_owned());
    ppm.with_file_name(format!("{}{LABELS_SUFFIX}", stem.unwrap_or_default()))
}

fn read_ppm(path: &Path) -> Result<Image, ScreenError> {
    let bytes = std::fs::read(path).map_err(|source| ScreenError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Image::from_ppm(&bytes).map_err(|source| ScreenError::Ppm {
        path: path.to_path_buf(),
        source,
    })
}

fn remove(path: &Path) -> Result<(), ScreenError> {
    std::fs::remove_file(path).map_err(|source| ScreenError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Draws the labels of `path` (a JSON list of `{x, y, text, color}`) into
/// the picture `rgb` of `width` by `height` pixels.
fn draw_labels(path: &Path, width: u32, height: u32, rgb: &mut [u8]) -> Result<(), ScreenError> {
    let bad = || ScreenError::Labels {
        path: path.to_path_buf(),
    };
    let text = std::fs::read_to_string(path).map_err(|source| ScreenError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let value: Value = serde_json::from_str(&text).map_err(|_| bad())?;
    for label in value.as_array().ok_or_else(bad)? {
        let num = |key: &str| label.get(key).and_then(Value::as_u64).ok_or_else(bad);
        let (x, y) = (num("x")?, num("y")?);
        let words = label.get("text").and_then(Value::as_str).ok_or_else(bad)?;
        let color: Vec<u8> = label
            .get("color")
            .and_then(Value::as_array)
            .ok_or_else(bad)?
            .iter()
            .filter_map(|c| c.as_u64().and_then(|c| u8::try_from(c).ok()))
            .collect();
        let color: [u8; 3] = color.try_into().map_err(|_| bad())?;
        let (x, y) = (
            u32::try_from(x).map_err(|_| bad())?,
            u32::try_from(y).map_err(|_| bad())?,
        );
        draw_text(rgb, (width, height), (x, y), words, color);
    }
    Ok(())
}

/// Writes `text` with the UI font, its top-left corner at `at`.
fn draw_text(rgb: &mut [u8], size: (u32, u32), at: (u32, u32), text: &str, color: [u8; 3]) {
    let (width, height) = size;
    for (i, c) in text.chars().enumerate() {
        let glyph = font::glyph(c);
        let gx = at.0 + i as u32 * font::ADVANCE;
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..font::GLYPH_WIDTH {
                let (x, y) = (gx + col, at.1 + row as u32);
                if bits & (0x80 >> col) != 0 && x < width && y < height {
                    let p = (y as usize * width as usize + x as usize) * 3;
                    rgb[p..p + 3].copy_from_slice(&color);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sheet_and_its_labels_become_one_png() {
        let dir = std::env::temp_dir().join(format!("xtask-screen-{}", std::process::id()));
        let sub = dir.join("survey");
        std::fs::create_dir_all(&sub).unwrap();
        let mut ppm = b"P6\n20 14\n255\n".to_vec();
        ppm.extend(std::iter::repeat_n(0_u8, 20 * 14 * 3));
        std::fs::write(sub.join("ntsc.ppm"), &ppm).unwrap();
        std::fs::write(
            sub.join("ntsc.labels.json"),
            r#"[{"x":0,"y":0,"text":"A","color":[255,0,0]}]"#,
        )
        .unwrap();
        std::fs::write(dir.join("wire.ppm"), &ppm).unwrap();
        assert_eq!(convert_tree(&dir).unwrap(), 2);
        assert!(sub.join("ntsc.png").is_file());
        assert!(!sub.join("ntsc.ppm").exists());
        assert!(!sub.join("ntsc.labels.json").exists());
        assert!(dir.join("wire.png").is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn labels_are_drawn_in_their_colour_and_bad_labels_are_an_error() {
        let mut rgb = vec![0_u8; 12 * 12 * 3];
        draw_text(&mut rgb, (12, 12), (0, 0), "A", [9, 8, 7]);
        assert!(rgb.chunks(3).any(|c| c == [9, 8, 7]));
        let dir = std::env::temp_dir().join(format!("xtask-labels-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("x.labels.json");
        std::fs::write(&path, r#"[{"x":0}]"#).unwrap();
        assert!(matches!(
            draw_labels(&path, 12, 12, &mut rgb),
            Err(ScreenError::Labels { .. })
        ));
        assert_eq!(
            labels_path(Path::new("/a/ntsc.ppm")),
            Path::new("/a/ntsc.labels.json")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
