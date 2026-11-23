//! `cargo xtask rust-notices`: the licence notices of the crates that a
//! Rust program of the image contains.
//!
//! The image build runs it in the install step of each Rust package, in
//! the Buildroot build folder, offline. Its input is the crate list of
//! `cargo tree` for the target build of the program, the vendored crates
//! of that build, and the licence files of the Rust toolchain. Its output
//! is one text file per program: each crate with its licence, then every
//! licence, notice and copyright file of those crates and of the Rust
//! standard library. A text that several crates ship is printed once.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// An error of `rust-notices`.
#[derive(Debug, thiserror::Error)]
pub enum NoticesError {
    /// The arguments are wrong.
    #[error("{0}\nusage: {USAGE}")]
    Usage(String),
    /// A file or a folder cannot be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The file or folder.
        path: PathBuf,
        /// The error of the operating system.
        source: std::io::Error,
    },
    /// A line of the crate list is not `name vVERSION ...`.
    #[error("line {line} of the crate list is not a crate: {text:?}")]
    BadLine {
        /// The line number, from 1.
        line: usize,
        /// The line.
        text: String,
    },
    /// A crate of the list is not in the vendor folder.
    #[error("{name} {version} is not in {vendor}")]
    NotVendored {
        /// The crate name.
        name: String,
        /// The crate version.
        version: String,
        /// The vendor folder.
        vendor: PathBuf,
    },
    /// A crate has no licence field and no licence file.
    #[error("{name} {version} in {dir} has no licence field and no licence file")]
    NoLicence {
        /// The crate name.
        name: String,
        /// The crate version.
        version: String,
        /// The crate folder.
        dir: PathBuf,
    },
    /// No folder of the list holds the licence files of the Rust toolchain.
    #[error("no folder holds the MIT and Apache-2.0 texts of the Rust toolchain: {tried:?}")]
    NoStd {
        /// The folders that were tried.
        tried: Vec<PathBuf>,
    },
}

/// The usage of the command.
pub const USAGE: &str = "cargo xtask rust-notices --program NAME --crates FILE \
--vendor DIR --std DIR [--std DIR]... --out FILE";

/// One crate of the program: name and version.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CrateId {
    /// The crate name.
    pub name: String,
    /// The crate version, without the `v` of `cargo tree`.
    pub version: String,
}

/// A licence text that a crate ships: its file name and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LicenceFile {
    /// The path of the file in the crate.
    pub name: String,
    /// The text, with CRLF line ends made LF.
    pub text: String,
}

/// The licence facts of one crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateLicence {
    /// The crate.
    pub id: CrateId,
    /// The `license` field of its manifest, or empty.
    pub expression: String,
    /// The `authors` field of its manifest, joined with commas, or empty.
    pub authors: String,
    /// The licence, notice and copyright files of the crate.
    pub files: Vec<LicenceFile>,
}

/// Runs `rust-notices` with `args`.
pub fn run(args: &[&str]) -> Result<(), NoticesError> {
    let args = Args::parse(args)?;
    let list = read(&args.crates)?;
    let ids = parse_tree(&list)?;
    let mut crates = Vec::with_capacity(ids.len());
    for id in ids {
        let dir = find_crate(&args.vendor, &id)?;
        crates.push(read_crate(&dir, id)?);
    }
    let std = read_std(&args.std)?;
    let text = render(&args.program, &crates, &std);
    if let Some(parent) = args.out.parent() {
        std::fs::create_dir_all(parent).map_err(io(parent))?;
    }
    std::fs::write(&args.out, text).map_err(io(&args.out))?;
    eprintln!(
        "xtask: notices of {} crates of {} in {}",
        crates.len(),
        args.program,
        args.out.display()
    );
    Ok(())
}

/// The crates of `cargo tree --prefix none` output, sorted and without
/// repeats. A package of the workspace (a path in parentheses) is
/// NEPTRON's own code and is left out.
pub fn parse_tree(text: &str) -> Result<Vec<CrateId>, NoticesError> {
    let mut ids = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let bad = || NoticesError::BadLine {
            line: n + 1,
            text: line.to_owned(),
        };
        let mut words = line.split_whitespace();
        let name = words.next().ok_or_else(bad)?;
        let version = words
            .next()
            .and_then(|v| v.strip_prefix('v'))
            .filter(|v| v.starts_with(|c: char| c.is_ascii_digit()))
            .ok_or_else(bad)?;
        if words.any(|w| w.starts_with("(/")) {
            continue;
        }
        ids.push(CrateId {
            name: name.to_owned(),
            version: version.to_owned(),
        });
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

/// The folder of crate `id` in `vendor`: `name-version`, or `name` when its
/// manifest has that version. `cargo vendor` uses the second form for the
/// only version of a crate; a Cargo registry folder uses the first.
pub fn find_crate(vendor: &Path, id: &CrateId) -> Result<PathBuf, NoticesError> {
    for dir in [
        vendor.join(format!("{}-{}", id.name, id.version)),
        vendor.join(&id.name),
    ] {
        if let Ok(manifest) = std::fs::read_to_string(dir.join("Cargo.toml"))
            && package_value(&manifest, "version").as_deref() == Some(&id.version)
        {
            return Ok(dir);
        }
    }
    Err(NoticesError::NotVendored {
        name: id.name.clone(),
        version: id.version.clone(),
        vendor: vendor.to_path_buf(),
    })
}

/// The licence field and the licence files of the crate in `dir`.
pub fn read_crate(dir: &Path, id: CrateId) -> Result<CrateLicence, NoticesError> {
    let manifest = read(&dir.join("Cargo.toml"))?;
    let expression = package_value(&manifest, "license").unwrap_or_default();
    let authors = package_value(&manifest, "authors").unwrap_or_default();
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(io(dir))? {
        let entry = entry.map_err(io(dir))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_licence_file(&name) && entry.path().is_file() {
            names.push(name);
        }
    }
    if let Some(file) = package_value(&manifest, "license-file")
        && !names.contains(&file)
    {
        names.push(file);
    }
    names.sort();
    let mut files = Vec::with_capacity(names.len());
    for name in names {
        let bytes = std::fs::read(dir.join(&name)).map_err(io(&dir.join(&name)))?;
        files.push(LicenceFile {
            text: String::from_utf8_lossy(&bytes).replace("\r\n", "\n"),
            name,
        });
    }
    if expression.is_empty() && files.is_empty() {
        return Err(NoticesError::NoLicence {
            name: id.name,
            version: id.version,
            dir: dir.to_path_buf(),
        });
    }
    Ok(CrateLicence {
        id,
        expression,
        authors,
        files,
    })
}

/// Whether a file at the top of a crate is a licence, notice or copyright
/// file, by its name.
pub fn is_licence_file(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    LICENCE_FILE_PREFIXES.iter().any(|p| upper.starts_with(p))
}

/// The text of the notices of `program`.
pub fn render(program: &str, crates: &[CrateLicence], std: &[LicenceFile]) -> String {
    let mut texts: Vec<(String, Vec<String>)> = Vec::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut add = |owner: String, file: &LicenceFile| -> usize {
        let n = *index.entry(file.text.clone()).or_insert_with(|| {
            texts.push((file.text.clone(), Vec::new()));
            texts.len()
        });
        texts[n - 1].1.push(owner);
        n
    };
    let mut lines = Vec::new();
    for c in crates {
        let refs: Vec<String> = c
            .files
            .iter()
            .map(|f| add(format!("{} {}, {}", c.id.name, c.id.version, f.name), f).to_string())
            .collect();
        let expression = if c.expression.is_empty() {
            "no licence field"
        } else {
            &c.expression
        };
        let refs = if refs.is_empty() && c.authors.is_empty() {
            "the crate ships no licence file".to_owned()
        } else if refs.is_empty() {
            format!(
                "the crate ships no licence file; its authors: {}",
                c.authors
            )
        } else {
            format!("texts {}", refs.join(", "))
        };
        lines.push(format!(
            "- {} {}: {expression} ({refs})",
            c.id.name, c.id.version
        ));
    }
    let std_refs: Vec<String> = std
        .iter()
        .map(|f| add(format!("Rust standard library, {}", f.name), f).to_string())
        .collect();

    let title = format!("Rust crates in {program}");
    let mut out = format!(
        "{title}\n{}\n\n\
         The NEPTRON program `{program}` is written in Rust. Its binary\n\
         contains the Rust standard library and the crates below, from\n\
         crates.io, at the versions of Cargo.lock. Each crate keeps its own\n\
         licence. This file gives the licence of each crate, then every\n\
         licence, notice and copyright file that the crates ship. A text that\n\
         several crates ship is printed once, with the names of those crates.\n\n\
         Crates\n------\n\n",
        "=".repeat(title.len())
    );
    for line in lines {
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str(&format!(
        "\nThe Rust standard library\n-------------------------\n\n\
         The Rust standard library is under the MIT licence or the Apache\n\
         License 2.0, at your option (texts {}).\n\nTexts\n-----\n",
        std_refs.join(", ")
    ));
    for (n, (text, owners)) in texts.iter().enumerate() {
        out.push_str(&format!("\n[{}] {}\n\n", n + 1, owners.join("; ")));
        out.push_str(text.trim_end());
        out.push('\n');
    }
    out
}

/// The file name starts that mark a licence, notice or copyright file.
const LICENCE_FILE_PREFIXES: [&str; 6] = [
    "LICENSE",
    "LICENCE",
    "COPYING",
    "COPYRIGHT",
    "NOTICE",
    "UNLICENSE",
];

/// The layouts of the licence files of a Rust toolchain folder: the files
/// that the folder must have, and the files that it may have. The root of
/// a Rust release archive has the first; `share/doc/rust/licenses` of an
/// installed toolchain of a newer release has the second.
const STD_LAYOUTS: [(&[&str], &[&str]); 2] = [
    (&["LICENSE-MIT", "LICENSE-APACHE"], &["COPYRIGHT"]),
    (&["MIT.txt", "Apache-2.0.txt"], &["LLVM-exception.txt"]),
];

#[derive(Debug)]
struct Args {
    program: String,
    crates: PathBuf,
    vendor: PathBuf,
    std: Vec<PathBuf>,
    out: PathBuf,
}

impl Args {
    fn parse(args: &[&str]) -> Result<Self, NoticesError> {
        let (mut program, mut crates, mut vendor, mut out) = (None, None, None, None);
        let mut std = Vec::new();
        let mut it = args.iter();
        while let Some(&flag) = it.next() {
            let value = it
                .next()
                .copied()
                .ok_or_else(|| NoticesError::Usage(format!("{flag} needs a value")))?;
            match flag {
                "--program" => program = Some(value.to_owned()),
                "--crates" => crates = Some(PathBuf::from(value)),
                "--vendor" => vendor = Some(PathBuf::from(value)),
                "--std" => std.push(PathBuf::from(value)),
                "--out" => out = Some(PathBuf::from(value)),
                other => return Err(NoticesError::Usage(format!("unknown argument {other:?}"))),
            }
        }
        let missing = |flag: &str| NoticesError::Usage(format!("{flag} is missing"));
        if std.is_empty() {
            return Err(missing("--std"));
        }
        Ok(Self {
            program: program.ok_or_else(|| missing("--program"))?,
            crates: crates.ok_or_else(|| missing("--crates"))?,
            vendor: vendor.ok_or_else(|| missing("--vendor"))?,
            std,
            out: out.ok_or_else(|| missing("--out"))?,
        })
    }
}

/// The licence files of the first folder of `dirs` that has every required
/// file of a layout of [`STD_LAYOUTS`]. An empty path, as an unset make
/// variable gives, is skipped.
fn read_std(dirs: &[PathBuf]) -> Result<Vec<LicenceFile>, NoticesError> {
    for dir in dirs.iter().filter(|d| !d.as_os_str().is_empty()) {
        for (required, optional) in STD_LAYOUTS {
            if !required.iter().all(|f| dir.join(f).is_file()) {
                continue;
            }
            let mut files = Vec::new();
            for name in required.iter().chain(optional) {
                let path = dir.join(name);
                if path.is_file() {
                    files.push(LicenceFile {
                        name: (*name).to_owned(),
                        text: read(&path)?.replace("\r\n", "\n"),
                    });
                }
            }
            return Ok(files);
        }
    }
    Err(NoticesError::NoStd {
        tried: dirs.to_vec(),
    })
}

/// The value of `key = "..."` in the `[package]` table of a manifest. For
/// a one-line array of strings, the strings joined with commas.
fn package_value(manifest: &str, key: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some((k, v)) = line.split_once('=')
            && k.trim() == key
        {
            let v = v.trim();
            if let Some(items) = v.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
                let items: Vec<&str> = items
                    .split(',')
                    .map(|i| i.trim().trim_matches('"'))
                    .filter(|i| !i.is_empty())
                    .collect();
                return Some(items.join(", "));
            }
            return v
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .map(str::to_owned);
        }
    }
    None
}

fn read(path: &Path) -> Result<String, NoticesError> {
    std::fs::read_to_string(path).map_err(io(path))
}

fn io(path: &Path) -> impl Fn(std::io::Error) -> NoticesError + '_ {
    move |source| NoticesError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests;
