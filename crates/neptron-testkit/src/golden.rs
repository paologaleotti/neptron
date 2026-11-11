//! Golden images: compare a rendered frame with a binary PPM file, and
//! report the difference. Nothing here writes a golden.

mod diff;
mod ppm;

pub use diff::difference;
pub use ppm::{PpmError, decode_ppm, encode_ppm};

use std::path::{Path, PathBuf};
use std::{fs, io};

use crate::RgbImage;

/// A directory of golden images, and a directory for the actual images of
/// failed checks.
///
/// [`Golden::check`] never writes into the golden directory. To accept a
/// new image, a person (or the reviewer) opens the actual image and copies
/// it over the golden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Golden {
    dir: PathBuf,
    actual_dir: PathBuf,
}

/// Why a golden check failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GoldenError {
    /// The name is not a relative path of `[a-z0-9_-]` segments.
    #[error("the golden name {name:?} must be segments of a-z, 0-9, _ and -, separated by /")]
    BadName {
        /// The name that was given.
        name: String,
    },
    /// The golden file does not exist.
    #[error(
        "the golden {} does not exist. The actual image is in {}. \
         Look at it before you copy it to the golden path",
        golden.display(),
        actual.display()
    )]
    Missing {
        /// The path of the golden.
        golden: PathBuf,
        /// Where the actual image is now.
        actual: PathBuf,
    },
    /// The golden file cannot be read.
    #[error("cannot read the golden {}: {source}", path.display())]
    Unreadable {
        /// The path of the golden.
        path: PathBuf,
        /// The error of the read.
        source: io::Error,
    },
    /// The golden file is not a PPM that [`decode_ppm`] reads.
    #[error("the golden {} is not a valid PPM: {source}", path.display())]
    BadGolden {
        /// The path of the golden.
        path: PathBuf,
        /// What is wrong with the file.
        source: PpmError,
    },
    /// The actual image differs from the golden.
    #[error(
        "the golden {} does not match.\n{report}The actual image is in {}. \
         Look at it before you copy it over the golden",
        golden.display(),
        actual.display()
    )]
    Mismatch {
        /// The path of the golden.
        golden: PathBuf,
        /// Where the actual image is now.
        actual: PathBuf,
        /// The report of [`difference`].
        report: String,
    },
    /// The actual image cannot be written.
    #[error("cannot write the actual image {}: {source}", path.display())]
    WriteActual {
        /// The path of the actual image.
        path: PathBuf,
        /// The error of the write.
        source: io::Error,
    },
}

impl Golden {
    /// The golden images under `tests/golden/` at the workspace root. Failed
    /// checks write their actual image under `actual_dir`, for example
    /// `env!("CARGO_TARGET_TMPDIR")` of an integration test.
    pub fn workspace(actual_dir: impl Into<PathBuf>) -> Self {
        Self::new(workspace_root().join("tests").join("golden"), actual_dir)
    }

    /// The golden images under `dir`, with the actual images of failed
    /// checks under `actual_dir`.
    pub fn new(dir: impl Into<PathBuf>, actual_dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            actual_dir: actual_dir.into(),
        }
    }

    /// The path of the golden `name`: `<dir>/<name>.ppm`.
    ///
    /// # Errors
    ///
    /// [`GoldenError::BadName`] when `name` is not a relative path of
    /// `[a-z0-9_-]` segments separated by `/`.
    pub fn path(&self, name: &str) -> Result<PathBuf, GoldenError> {
        Ok(self.dir.join(file_name(name)?))
    }

    /// Compares `image` with the golden `name`, exactly.
    ///
    /// # Errors
    ///
    /// [`GoldenError::Missing`] or [`GoldenError::Mismatch`] when the golden
    /// does not exist or differs. Both write the actual image to
    /// `<actual_dir>/<name>.ppm` first. The other variants report a bad
    /// name, a golden that cannot be read or decoded, or a failed write.
    pub fn check(&self, name: &str, image: &RgbImage) -> Result<(), GoldenError> {
        let file = file_name(name)?;
        let golden = self.dir.join(&file);
        let actual = self.actual_dir.join(&file);
        let expected = match fs::read(&golden) {
            Ok(bytes) => decode_ppm(&bytes).map_err(|source| GoldenError::BadGolden {
                path: golden.clone(),
                source,
            })?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                write_actual(&actual, image)?;
                return Err(GoldenError::Missing { golden, actual });
            }
            Err(source) => {
                return Err(GoldenError::Unreadable {
                    path: golden,
                    source,
                });
            }
        };
        match difference(&expected, image) {
            None => Ok(()),
            Some(report) => {
                write_actual(&actual, image)?;
                Err(GoldenError::Mismatch {
                    golden,
                    actual,
                    report,
                })
            }
        }
    }
}

/// The root of the workspace, two levels above this crate.
pub(crate) fn workspace_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .map_or_else(|| manifest.join("..").join(".."), Path::to_path_buf)
}

fn file_name(name: &str) -> Result<PathBuf, GoldenError> {
    let valid = name.split('/').all(|segment| {
        !segment.is_empty()
            && segment
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
    });
    if !valid {
        return Err(GoldenError::BadName {
            name: name.to_owned(),
        });
    }
    Ok(PathBuf::from(format!("{name}.ppm")))
}

fn write_actual(path: &Path, image: &RgbImage) -> Result<(), GoldenError> {
    let fail = |source| GoldenError::WriteActual {
        path: path.to_path_buf(),
        source,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(fail)?;
    }
    fs::write(path, encode_ppm(image)).map_err(fail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_relative_paths_of_simple_segments() {
        let golden = Golden::new("/g", "/a");
        assert_eq!(
            golden.path("session/first_frame-2").unwrap(),
            PathBuf::from("/g/session/first_frame-2.ppm")
        );
        for bad in ["", "a//b", "../x", "/abs", "Upper", "a.b", "x/"] {
            assert!(
                matches!(golden.path(bad), Err(GoldenError::BadName { .. })),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn the_workspace_goldens_are_under_tests_golden() {
        let golden = Golden::workspace("/a");
        let path = golden.path("x").unwrap();
        assert!(path.ends_with("tests/golden/x.ppm"));
        assert!(workspace_root().join("Cargo.toml").is_file());
        assert!(workspace_root().join("crates/neptron-testkit").is_dir());
    }
}
