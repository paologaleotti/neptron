//! `cargo xtask arcade-index`: the arcade index of `neptron-library`, made
//! from the ClrMamePro XML DAT of FinalBurn Neo.
//!
//! The DAT parser lives here only, so the image has no XML code. The index
//! is written with the encoder that the frontend reads with.

use std::fs;
use std::path::{Path, PathBuf};

use neptron_library::arcade::{
    self, ArcadeIndex, DiskRecord, IndexError, IndexSource, Orientation, RomRecord, SetRecord,
};

/// The core whose DAT the command reads.
pub const CORE: &str = "fbneo";

/// The licence line of the index. The image keeps the licence texts of each
/// package under `/usr/share/licenses`.
pub const LICENCE: &str =
    "FBNeo licence (non-commercial), see the libretro-fbneo folder of /usr/share/licenses";

const USAGE: &str = "usage: cargo xtask arcade-index --commit <40-hex commit> <dat> <out>";

/// An error of `arcade-index`.
#[derive(Debug, thiserror::Error)]
pub enum ArcadeError {
    /// The arguments do not follow the usage.
    #[error("{0}\n{USAGE}")]
    Usage(String),
    /// A file could not be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The error from the file system.
        source: std::io::Error,
    },
    /// The DAT is not well-formed XML.
    #[error("the DAT is not valid XML: {0}")]
    Xml(#[from] roxmltree::Error),
    /// The DAT does not have the shape of a ClrMamePro DAT.
    #[error("the DAT has no {0}")]
    Shape(&'static str),
    /// A set has a value that the index cannot take.
    #[error("the set `{set}` has a bad `{field}`: `{value}`")]
    Value {
        /// The set.
        set: String,
        /// The attribute or element.
        field: &'static str,
        /// The value found.
        value: String,
    },
    /// The index cannot hold the DAT.
    #[error(transparent)]
    Index(#[from] IndexError),
}

/// Runs `arcade-index` with its arguments.
pub fn run(args: &[&str]) -> Result<(), ArcadeError> {
    let (commit, dat, out) = parse_args(args)?;
    let text = fs::read_to_string(&dat).map_err(|source| ArcadeError::Io {
        path: dat.clone(),
        source,
    })?;
    let dat_name = dat
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_owned();
    let index = index_of_dat(&text, &dat_name, &commit)?;
    let encoded = arcade::encode(&index)?;
    write(&out, &encoded)?;
    let roms: usize = index.sets().iter().map(|s| s.roms.len()).sum();
    eprintln!(
        "xtask: wrote {} ({} sets, {} BIOS, {roms} ROMs, {} bytes)",
        out.display(),
        index.sets().len(),
        index.bios_count(),
        encoded.len()
    );
    Ok(())
}

/// The index of the DAT text. `dat_name` is the file name of the DAT and
/// `commit` the commit of the core that it comes from.
pub fn index_of_dat(text: &str, dat_name: &str, commit: &str) -> Result<ArcadeIndex, ArcadeError> {
    let options = roxmltree::ParsingOptions {
        // The DAT starts with a DOCTYPE that names the Logiqx DTD; the
        // parser refuses any DOCTYPE unless this is set.
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };
    let doc = roxmltree::Document::parse_with_options(text, options)?;
    let root = doc.root_element();
    if !root.has_tag_name("datafile") {
        return Err(ArcadeError::Shape("datafile element"));
    }
    let version = child(root, "header")
        .and_then(|h| child_text(h, "version"))
        .ok_or(ArcadeError::Shape("header version"))?;
    let source = IndexSource {
        core: CORE.to_owned(),
        commit: commit.to_owned(),
        dat_name: dat_name.to_owned(),
        dat_version: version.to_owned(),
        licence: LICENCE.to_owned(),
    };
    let sets = root
        .children()
        .filter(|n| n.has_tag_name("game"))
        .map(set_of_game)
        .collect::<Result<Vec<_>, _>>()?;
    if sets.is_empty() {
        return Err(ArcadeError::Shape("game element"));
    }
    Ok(ArcadeIndex::from_sets(source, sets)?)
}

fn parse_args(args: &[&str]) -> Result<(String, PathBuf, PathBuf), ArcadeError> {
    let mut commit = None;
    let mut paths = Vec::new();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match *arg {
            "--commit" => {
                let value = rest
                    .next()
                    .ok_or_else(|| ArcadeError::Usage("--commit needs a value".to_owned()))?;
                commit = Some(*value);
            }
            other if other.starts_with("--") => {
                return Err(ArcadeError::Usage(format!("unknown option {other}")));
            }
            path => paths.push(PathBuf::from(path)),
        }
    }
    let commit = commit.ok_or_else(|| ArcadeError::Usage("--commit is missing".to_owned()))?;
    if commit.len() != 40 || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ArcadeError::Usage(format!(
            "the commit `{commit}` is not 40 hex digits"
        )));
    }
    let [dat, out]: [PathBuf; 2] = paths
        .try_into()
        .map_err(|_| ArcadeError::Usage("give the DAT and the output path".to_owned()))?;
    Ok((commit.to_ascii_lowercase(), dat, out))
}

fn set_of_game(game: roxmltree::Node<'_, '_>) -> Result<SetRecord, ArcadeError> {
    let name = game
        .attribute("name")
        .ok_or(ArcadeError::Shape("name of a game"))?;
    let bad = |field: &'static str, value: &str| ArcadeError::Value {
        set: name.to_owned(),
        field,
        value: value.to_owned(),
    };
    let description = child_text(game, "description").unwrap_or_default();
    let (title, details) = arcade::split_description(description);
    let mut set = SetRecord::new(name.to_owned(), title.to_owned());
    set.details = details.to_owned();
    set.parent = game.attribute("cloneof").map(str::to_owned);
    set.romof = game.attribute("romof").map(str::to_owned);
    set.flags.bios = match game.attribute("isbios") {
        None | Some("no") => false,
        Some("yes") => true,
        Some(other) => return Err(bad("isbios", other)),
    };
    set.year = child_text(game, "year").unwrap_or_default().to_owned();
    set.maker = child_text(game, "manufacturer")
        .unwrap_or_default()
        .to_owned();
    set.board = game.attribute("sourcefile").unwrap_or_default().to_owned();
    if let Some(video) = child(game, "video") {
        set.orientation = match video.attribute("orientation") {
            Some("horizontal") => Orientation::Horizontal,
            Some("vertical") => Orientation::Vertical,
            other => return Err(bad("video orientation", other.unwrap_or_default())),
        };
        set.flags.vertical = set.orientation == Orientation::Vertical;
        set.width = number(video.attribute("width"))
            .ok_or_else(|| bad("video width", video.attribute("width").unwrap_or_default()))?;
        set.height = number(video.attribute("height")).ok_or_else(|| {
            bad(
                "video height",
                video.attribute("height").unwrap_or_default(),
            )
        })?;
    }
    if let Some(driver) = child(game, "driver") {
        match driver.attribute("status") {
            Some("good") => {}
            // FinalBurn Neo writes `preliminary` for every driver without
            // its working flag (src/burner/dat.cpp), so the status also
            // means "not working".
            Some("preliminary") => {
                set.flags.preliminary = true;
                set.flags.not_working = true;
            }
            other => return Err(bad("driver status", other.unwrap_or_default())),
        }
    }
    set.roms = game
        .children()
        .filter(|n| n.has_tag_name("rom"))
        .map(|rom| rom_of(name, rom))
        .collect::<Result<_, _>>()?;
    set.disks = game
        .children()
        .filter(|n| n.has_tag_name("disk"))
        .map(|disk| disk_of(name, disk))
        .collect::<Result<_, _>>()?;
    Ok(set)
}

/// A `disk` element: a hard disk image of the set. The SHA-1 is kept in
/// lower case, as the index holds it.
fn disk_of(set: &str, disk: roxmltree::Node<'_, '_>) -> Result<DiskRecord, ArcadeError> {
    let bad = |field: &'static str, value: Option<&str>| ArcadeError::Value {
        set: set.to_owned(),
        field,
        value: value.unwrap_or_default().to_owned(),
    };
    let name = disk
        .attribute("name")
        .ok_or_else(|| bad("disk name", None))?;
    let sha1 = match disk.attribute("sha1") {
        None => None,
        Some(hex) if hex.len() == 40 && hex.bytes().all(|b| b.is_ascii_hexdigit()) => {
            Some(hex.to_ascii_lowercase())
        }
        other => return Err(bad("disk sha1", other)),
    };
    Ok(DiskRecord {
        name: name.to_owned(),
        sha1,
    })
}

fn rom_of(set: &str, rom: roxmltree::Node<'_, '_>) -> Result<RomRecord, ArcadeError> {
    let bad = |field: &'static str, value: Option<&str>| ArcadeError::Value {
        set: set.to_owned(),
        field,
        value: value.unwrap_or_default().to_owned(),
    };
    let name = rom.attribute("name").ok_or_else(|| bad("rom name", None))?;
    let size_text = rom.attribute("size");
    let size = size_text
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or_else(|| bad("rom size", size_text))?;
    let crc_text = rom.attribute("crc");
    let crc = match (rom.attribute("status"), crc_text) {
        (Some("nodump"), None) => None,
        (None, Some(hex)) if hex.len() == 8 => {
            Some(u32::from_str_radix(hex, 16).map_err(|_| bad("rom crc", crc_text))?)
        }
        (None, _) => return Err(bad("rom crc", crc_text)),
        (status, _) => return Err(bad("rom status", status)),
    };
    Ok(RomRecord {
        name: name.to_owned(),
        size,
        crc,
        merge: rom.attribute("merge").map(str::to_owned),
    })
}

fn child<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    tag: &str,
) -> Option<roxmltree::Node<'a, 'input>> {
    node.children().find(|n| n.has_tag_name(tag))
}

fn child_text<'a>(node: roxmltree::Node<'a, '_>, tag: &str) -> Option<&'a str> {
    child(node, tag).map(|n| n.text().unwrap_or_default())
}

fn number(text: Option<&str>) -> Option<u32> {
    text?.parse().ok()
}

/// Writes the index next to its place and renames it, so a stopped run
/// leaves no half file at `out`.
fn write(out: &Path, text: &str) -> Result<(), ArcadeError> {
    let io = |path: &Path| {
        let path = path.to_path_buf();
        move |source| ArcadeError::Io { path, source }
    };
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir).map_err(io(dir))?;
    }
    let mut tmp = out.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, text).map_err(io(&tmp))?;
    fs::rename(&tmp, out).map_err(io(out))
}

#[cfg(test)]
mod tests;
