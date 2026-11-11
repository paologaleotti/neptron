//! The UI commands of xtask. `cargo xtask font` generates the glyph table
//! of `neptron-ui` from the vendored BDF font.

mod font;

use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::repo::Repo;

/// An error of a UI command.
#[derive(Debug, thiserror::Error)]
pub enum UiError {
    /// The arguments are wrong.
    #[error("usage: {0}")]
    Usage(&'static str),
    /// A file cannot be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The error of the operating system.
        source: io::Error,
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
    fn font_takes_no_arguments() {
        assert!(matches!(font(&["x"]), Err(UiError::Usage(_))));
    }
}
