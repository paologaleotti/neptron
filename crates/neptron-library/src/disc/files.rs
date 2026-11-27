//! The check of the disc files of a game before launch
//! (`docs/spec/disc-games.md`, section 5). It reads directory entries and
//! the text of cue and m3u files, at most [`LIST_MAX_BYTES`] each, and
//! changes no file.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use super::{DiscFormat, LIST_MAX_BYTES, cue_files, disc_tag, m3u_lines};

/// The discs of a game, as the launch gives them to the core.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscPlan {
    /// The discs after the first, for a group of files: the launch adds them
    /// with the disc control interface, in order. Empty for one disc and
    /// for an m3u file, which the core reads itself.
    pub more: Vec<PathBuf>,
    /// The number that the Disc page shows for each disc, the first disc
    /// first: the tag numbers of a group, else 1, 2, 3...
    pub numbers: Vec<u8>,
}

/// A disc file that is not there: the launch stops with a notice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscProblem {
    /// `file` is missing; `list` is the cue or m3u file that names it, or
    /// the first disc of a group.
    Missing {
        /// The file name of the missing file.
        file: String,
        /// The file name of the file that names it.
        list: String,
    },
    /// The m3u file `list` names no disc, or cannot be read as text.
    NoDisc {
        /// The file name of the m3u file.
        list: String,
    },
}

/// Checks the disc files of the game at `content`, of `format`, and gives
/// the discs for the launch. A `.chd` or `.cue` file with a disc tag takes
/// the other files of its group from its folder, by the rules of the game
/// list. A cue that is larger than [`LIST_MAX_BYTES`] or cannot be read is
/// not checked: the core decides.
pub fn plan_discs(content: &Path, format: DiscFormat) -> Result<DiscPlan, DiscProblem> {
    match format {
        DiscFormat::M3u => plan_list(content),
        DiscFormat::Chd | DiscFormat::Cue => {
            // A file that is not the first of its group is a game of its
            // own in the list (an m3u file names the others, or a launch
            // names that disc): it gets one disc, with its own number, so
            // the Disc page and the core hold the same discs.
            let (group, alone) = match group_of(content) {
                Some(group) if group.first().is_some_and(|(_, p)| p == content) => {
                    (Some(group), None)
                }
                Some(group) => {
                    let number = group.iter().find(|(_, p)| p == content).map(|(n, _)| *n);
                    (None, number)
                }
                None => (None, None),
            };
            let discs: Vec<&Path> = match &group {
                Some(group) => group.iter().map(|(_, p)| p.as_path()).collect(),
                None => vec![content],
            };
            let first = file_name(content);
            for disc in &discs {
                if !disc.exists() {
                    return Err(DiscProblem::Missing {
                        file: file_name(disc),
                        list: first.clone(),
                    });
                }
                if format == DiscFormat::Cue {
                    check_cue(disc)?;
                }
            }
            Ok(match group {
                Some(group) => DiscPlan {
                    more: group.iter().skip(1).map(|(_, p)| p.clone()).collect(),
                    numbers: group.iter().map(|(n, _)| *n).collect(),
                },
                None => DiscPlan {
                    more: Vec::new(),
                    numbers: vec![alone.unwrap_or(1)],
                },
            })
        }
    }
}

fn plan_list(m3u: &Path) -> Result<DiscPlan, DiscProblem> {
    let list = file_name(m3u);
    let no_disc = || DiscProblem::NoDisc { list: list.clone() };
    let text = match read_text(m3u) {
        Ok(Some(text)) => text,
        Ok(None) | Err(_) => return Err(no_disc()),
    };
    let lines = m3u_lines(&text);
    if lines.is_empty() {
        return Err(no_disc());
    }
    if lines.iter().any(|line| names_a_list(line)) {
        return Err(no_disc());
    }
    let folder = m3u.parent().unwrap_or(Path::new(""));
    for line in &lines {
        let wanted = folder.join(line.replace('\\', "/"));
        let Some(disc) = resolve(folder, &line.replace('\\', "/")) else {
            return Err(DiscProblem::Missing {
                file: file_name(&wanted),
                list: list.clone(),
            });
        };
        let is_cue = disc
            .extension()
            .and_then(|e| e.to_str())
            .and_then(DiscFormat::of_extension)
            == Some(DiscFormat::Cue);
        if is_cue {
            check_cue(&disc)?;
        }
    }
    let count = u8::try_from(lines.len()).unwrap_or(u8::MAX);
    Ok(DiscPlan {
        more: Vec::new(),
        numbers: (1..=count).collect(),
    })
}

/// Every `FILE` of the cue exists, relative to the folder of the cue.
fn check_cue(cue: &Path) -> Result<(), DiscProblem> {
    let Ok(Some(text)) = read_text(cue) else {
        return Ok(());
    };
    let folder = cue.parent().unwrap_or(Path::new(""));
    match cue_files(&text)
        .into_iter()
        .map(|name| name.replace('\\', "/"))
        .find(|name| resolve(folder, name).is_none())
        .map(|name| folder.join(name))
    {
        Some(missing) => Err(DiscProblem::Missing {
            file: file_name(&missing),
            list: file_name(cue),
        }),
        None => Ok(()),
    }
}

/// The files of the group of `content` in its folder, by number, when it
/// has a disc tag and its group has two numbers or more. A number that
/// two files carry goes to the first path in byte order.
fn group_of(content: &Path) -> Option<Vec<(u8, PathBuf)>> {
    let name = content.file_name()?.to_str()?;
    let (stem, extension) = name.rsplit_once('.')?;
    let tag = disc_tag(stem)?;
    let folder = content.parent()?;
    let mut discs: Vec<(u8, PathBuf)> = fs::read_dir(folder)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|entry| {
            let other = entry.file_name().into_string().ok()?;
            if other.starts_with('.') {
                return None;
            }
            let (other_stem, other_ext) = other.rsplit_once('.')?;
            let other_tag = disc_tag(other_stem)?;
            (other_ext.eq_ignore_ascii_case(extension) && other_tag.group == tag.group)
                .then(|| (other_tag.number, entry.path()))
        })
        .collect();
    discs.sort_by(|a, b| {
        a.0.cmp(&b.0).then_with(|| {
            a.1.as_os_str()
                .as_encoded_bytes()
                .cmp(b.1.as_os_str().as_encoded_bytes())
        })
    });
    discs.dedup_by_key(|(number, _)| *number);
    (discs.len() >= 2 && discs.iter().any(|(_, p)| p == content)).then_some(discs)
}

/// The path of `relative` below `folder` as the share finds it: exFAT
/// ignores case, so each part matches a name of its folder with no regard
/// to case. An exact name comes first; of two names that differ only in
/// case (possible on ext4, not on exFAT), the first in byte order. `None`
/// when a part is missing.
fn resolve(folder: &Path, relative: &str) -> Option<PathBuf> {
    let mut path = folder.to_path_buf();
    for part in relative.split('/').filter(|p| !p.is_empty()) {
        if part == "." || part == ".." {
            path = path.join(part);
            continue;
        }
        // The names come from the folder, so a file system that ignores
        // case (exFAT, APFS) gives the same answer as one that does not.
        let wanted = part.to_lowercase();
        let mut names: Vec<String> = fs::read_dir(&path)
            .ok()?
            .filter_map(Result::ok)
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.to_lowercase() == wanted)
            .collect();
        names.sort_by(|a, b| {
            (a != part)
                .cmp(&(b != part))
                .then_with(|| a.as_bytes().cmp(b.as_bytes()))
        });
        path = path.join(names.first()?);
    }
    path.exists().then_some(path)
}

/// True for a list line that names an m3u file: an error of its list.
fn names_a_list(line: &str) -> bool {
    line.rsplit_once('.')
        .is_some_and(|(_, ext)| DiscFormat::of_extension(ext) == Some(DiscFormat::M3u))
}

/// The text of a list file. `None` when it is larger than
/// [`LIST_MAX_BYTES`] or not UTF-8.
fn read_text(path: &Path) -> io::Result<Option<String>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(LIST_MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > LIST_MAX_BYTES {
        return Ok(None);
    }
    Ok(String::from_utf8(bytes).ok())
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests;
