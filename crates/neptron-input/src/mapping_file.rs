//! The NEPTRON mapping file: gamepad mappings in the `gamecontrollerdb`
//! format of SDL, read once when the gamepad source opens.
//!
//! The file is read and checked here, line by line, and each good line
//! goes to SDL alone. The "add mappings from file" call of SDL returns
//! only a count, so it cannot name a damaged line, and a damaged line
//! must not lose the others.

use std::path::Path;

use crate::notice::InputNotice;

/// The mapping file under the data folder of the image.
pub const MAPPING_FILE: &str = "input/gamecontrollerdb.txt";

/// The community database `SDL_GameControllerDB` in the image. The
/// package `os/package/sdl-gamecontrollerdb` installs it outside the data
/// folder, because the install step of the frontend empties that folder.
pub const COMMUNITY_MAPPING_FILE: &str = "/usr/share/sdl-gamecontrollerdb/gamecontrollerdb.txt";

/// The digits of an SDL GUID in a mapping line.
const GUID_DIGITS: usize = 32;

/// The GUIDs that are not hex digits and that SDL accepts in any case:
/// the fallback mapping, and the mapping of XInput devices.
const SPECIAL_GUIDS: [&str; 2] = [DEFAULT_GUID, "xinput"];

/// The GUID of the fallback mapping of SDL. With one, SDL calls every
/// joystick a gamepad and gives it that mapping when it has no other, so
/// the generic layout and its notices would never apply.
const DEFAULT_GUID: &str = "default";

/// The field of a mapping line that names its platform.
const PLATFORM_FIELD: &str = "platform";

/// The good lines of a mapping file, and a notice for each problem.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mappings {
    /// Each good line for this platform, with its line number from 1.
    pub lines: Vec<(usize, String)>,
    /// A notice for the file, or for each damaged line.
    pub notices: Vec<InputNotice>,
}

/// Why a line of a mapping file is damaged.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MappingLineError {
    /// The first field is not a GUID of 32 hex digits, nor `default` or
    /// `xinput`.
    #[error("{guid:?} is not a GUID of {GUID_DIGITS} hex digits")]
    Guid {
        /// The first field.
        guid: String,
    },
    /// The second field, the name, is empty or missing.
    #[error("the name is missing")]
    NoName,
    /// The line has no `field:value` pair after the name.
    #[error("the line maps nothing")]
    NoMapping,
    /// The line is the fallback mapping of SDL, which NEPTRON refuses.
    #[error(
        "the GUID \"default\" is refused: SDL would give this layout to every joystick with no mapping, and the generic layout would never apply"
    )]
    DefaultGuid,
    /// A pair has no field or no value.
    #[error("{pair:?} is not a field:value pair")]
    Pair {
        /// The pair.
        pair: String,
    },
}

/// Reads the mapping file at `path` and keeps the good lines of
/// `platform` (the platform name of SDL, `Linux` on the image). A missing
/// or unreadable file gives one notice and no line.
pub fn read_mapping_file(path: &Path, platform: &str) -> Mappings {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_mappings(&text, &path.display().to_string(), platform),
        Err(e) => Mappings {
            lines: Vec::new(),
            notices: vec![InputNotice::MappingFile {
                path: path.display().to_string(),
                line: 0,
                reason: e.to_string(),
            }],
        },
    }
}

/// The good lines of the text of a mapping file named `path`, for
/// `platform`. Comments (`#`) and empty lines are skipped, and so is a
/// line of another platform, as SDL does. A damaged line gives a notice
/// and loses only itself.
pub fn parse_mappings(text: &str, path: &str, platform: &str) -> Mappings {
    let mut mappings = Mappings::default();
    for (number, raw) in (1..).zip(text.lines()) {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match check_line(line) {
            Ok(Some(other)) if other != platform => {}
            Ok(_) if is_default(line) => mappings.notices.push(InputNotice::MappingFile {
                path: path.to_owned(),
                line: number,
                reason: MappingLineError::DefaultGuid.to_string(),
            }),
            Ok(_) => mappings.lines.push((number, line.to_owned())),
            Err(e) => mappings.notices.push(InputNotice::MappingFile {
                path: path.to_owned(),
                line: number,
                reason: e.to_string(),
            }),
        }
    }
    mappings
}

/// The mapping files in the order of the calls to SDL. `files` is in
/// priority order, and for one GUID SDL keeps the line of the later call,
/// so the file that must win goes last.
#[cfg_attr(
    not(feature = "sdl3"),
    allow(dead_code, reason = "used by the SDL3 source")
)]
pub(crate) fn sdl_add_order<'a>(files: &'a [&'a Path]) -> impl Iterator<Item = &'a Path> {
    files.iter().rev().copied()
}

/// Whether `line` is the fallback mapping of SDL.
fn is_default(line: &str) -> bool {
    line.split(',')
        .next()
        .is_some_and(|guid| guid.trim().eq_ignore_ascii_case(DEFAULT_GUID))
}

/// Checks one mapping line, and gives its platform when it names one.
fn check_line(line: &str) -> Result<Option<&str>, MappingLineError> {
    let mut fields = line.split(',');
    let guid = fields.next().unwrap_or_default().trim();
    let hex = guid.len() == GUID_DIGITS && guid.bytes().all(|b| b.is_ascii_hexdigit());
    if !hex && !SPECIAL_GUIDS.iter().any(|s| s.eq_ignore_ascii_case(guid)) {
        return Err(MappingLineError::Guid {
            guid: guid.to_owned(),
        });
    }
    if fields.next().is_none_or(|name| name.trim().is_empty()) {
        return Err(MappingLineError::NoName);
    }
    let mut platform = None;
    let mut pairs = 0;
    // SDL lines end with a comma, which gives one empty field.
    for pair in fields.map(str::trim).filter(|p| !p.is_empty()) {
        let Some((field, value)) = pair.split_once(':') else {
            return Err(MappingLineError::Pair {
                pair: pair.to_owned(),
            });
        };
        if field.is_empty() || value.is_empty() {
            return Err(MappingLineError::Pair {
                pair: pair.to_owned(),
            });
        }
        if field == PLATFORM_FIELD {
            platform = Some(value);
        } else {
            pairs += 1;
        }
    }
    if pairs == 0 {
        return Err(MappingLineError::NoMapping);
    }
    Ok(platform)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str =
        "03000000aaaa0000bbbb000000000000,Test Pad,a:b1,b:b2,leftx:a0,platform:Linux,";
    const OTHER: &str = "03000000cccc0000dddd000000000000,Mac Pad,a:b0,platform:Mac OS X,";
    const ANY: &str = "03000000eeee0000ffff000000000000,Any Pad,a:b0,dpup:h0.1";

    fn line_notice(line: usize, reason: &str) -> InputNotice {
        InputNotice::MappingFile {
            path: "map.txt".to_owned(),
            line,
            reason: reason.to_owned(),
        }
    }

    #[test]
    fn a_valid_file_gives_its_lines_of_this_platform() {
        let text = format!("# NEPTRON gamepad mappings\n\n{GOOD}\n{OTHER}\n  {ANY}  \n");
        let mappings = parse_mappings(&text, "map.txt", "Linux");
        assert_eq!(mappings.lines, [(3, GOOD.to_owned()), (5, ANY.to_owned())]);
        assert_eq!(mappings.notices, []);
        let mac = parse_mappings(&text, "map.txt", "Mac OS X");
        assert_eq!(mac.lines.len(), 2);
        assert_eq!(mac.lines[0].1, OTHER);
    }

    #[test]
    fn a_damaged_line_loses_only_itself() {
        let text = format!(
            "{GOOD}\nnot a guid,Pad,a:b0\n03000000aaaa0000bbbb00000000000g,Pad,a:b0\n\
             03000000aaaa0000bbbb000000000000,,a:b0\n03000000aaaa0000bbbb000000000000\n\
             03000000aaaa0000bbbb000000000000,Pad,platform:Linux,\n\
             03000000aaaa0000bbbb000000000000,Pad,a\n03000000aaaa0000bbbb000000000000,Pad,a:\n{ANY}\n"
        );
        let mappings = parse_mappings(&text, "map.txt", "Linux");
        assert_eq!(mappings.lines, [(1, GOOD.to_owned()), (9, ANY.to_owned())]);
        assert_eq!(
            mappings.notices,
            [
                line_notice(2, "\"not a guid\" is not a GUID of 32 hex digits"),
                line_notice(
                    3,
                    "\"03000000aaaa0000bbbb00000000000g\" is not a GUID of 32 hex digits"
                ),
                line_notice(4, "the name is missing"),
                line_notice(5, "the name is missing"),
                line_notice(6, "the line maps nothing"),
                line_notice(7, "\"a\" is not a field:value pair"),
                line_notice(8, "\"a:\" is not a field:value pair"),
            ]
        );
    }

    #[test]
    fn the_xinput_guid_is_good_and_the_default_guid_is_refused() {
        let text = "xinput,XInput Controller,a:b0,platform:Linux,\n\
                    XInput,XInput Controller,a:b0,platform:Linux,\n\
                    Default,Any Pad,a:b0,platform:Linux,\n\
                    default,Any Pad,a:b0,platform:Windows,\n\
                    xinputs,Pad,a:b0,platform:Linux,\n";
        let mappings = parse_mappings(text, "map.txt", "Linux");
        let numbers: Vec<usize> = mappings.lines.iter().map(|(n, _)| *n).collect();
        assert_eq!(numbers, [1, 2]);
        assert_eq!(
            mappings.notices,
            [
                line_notice(
                    3,
                    "the GUID \"default\" is refused: SDL would give this layout to every \
                     joystick with no mapping, and the generic layout would never apply"
                ),
                line_notice(5, "\"xinputs\" is not a GUID of 32 hex digits"),
            ]
        );
    }

    #[test]
    fn a_file_of_the_community_database_gives_its_linux_lines_only() {
        // The shape of SDL_GameControllerDB: a header, a comment for each
        // platform, every line with its platform. The GUIDs are invented.
        let text = "# Game Controller DB for SDL in 2.0.16 format\n\
                    # Source: https://github.com/mdqinc/SDL_GameControllerDB\n\
                    \n\
                    # Windows\n\
                    03000000111100002222000000000000,Pad W,a:b0,b:b1,platform:Windows,\n\
                    xinput,XInput Controller,a:b0,b:b1,platform:Windows,\n\
                    \n\
                    # Mac OS X\n\
                    03000000111100002222000000010000,Pad M,a:b1,b:b2,platform:Mac OS X,\n\
                    \n\
                    # Linux\n\
                    03000000111100002222000011010000,Pad L,a:b1,b:b2,x:b0,platform:Linux,\n\
                    05000000333300004444000000010000,Pad L2,a:b0,dpup:h0.1,platform:Linux,\n\
                    xinput,XInput Controller,a:b0,b:b1,platform:Linux,\n\
                    \n\
                    # Android\n\
                    38653964633230666463343334313533,Pad A,a:b0,platform:Android,\n\
                    \n\
                    # iOS\n\
                    05000000ac0500000100000000006d01,Pad I,a:b0,platform:iOS,\n";
        let mappings = parse_mappings(text, COMMUNITY_MAPPING_FILE, "Linux");
        assert_eq!(mappings.notices, []);
        let numbers: Vec<usize> = mappings.lines.iter().map(|(n, _)| *n).collect();
        assert_eq!(numbers, [12, 13, 14]);
        assert!(
            mappings
                .lines
                .iter()
                .all(|(_, l)| l.ends_with("platform:Linux,"))
        );
    }

    #[test]
    fn the_file_that_must_win_goes_to_sdl_last() {
        let neptron = Path::new("neptron.txt");
        let community = Path::new(COMMUNITY_MAPPING_FILE);
        let files = [neptron, community];
        let order: Vec<&Path> = sdl_add_order(&files).collect();
        assert_eq!(order, [community, neptron]);
    }

    #[test]
    fn a_missing_file_gives_one_notice_and_no_line() {
        let path = std::env::temp_dir().join(format!(
            "neptron-mappings-missing-{}.txt",
            std::process::id()
        ));
        let mappings = read_mapping_file(&path, "Linux");
        assert_eq!(mappings.lines, []);
        assert!(matches!(
            mappings.notices.as_slice(),
            [InputNotice::MappingFile { line: 0, .. }]
        ));
    }

    #[test]
    fn a_file_on_disk_is_read_line_by_line() {
        let path =
            std::env::temp_dir().join(format!("neptron-mappings-read-{}.txt", std::process::id()));
        std::fs::write(&path, format!("{GOOD}\nbroken\n{ANY}\n")).unwrap();
        let mappings = read_mapping_file(&path, "Linux");
        std::fs::remove_file(&path).unwrap();
        assert_eq!(mappings.lines.len(), 2);
        assert_eq!(mappings.notices.len(), 1);
    }

    #[test]
    fn the_shipped_file_is_valid() {
        let text = include_str!("../../../data/input/gamecontrollerdb.txt");
        let mappings = parse_mappings(text, MAPPING_FILE, "Linux");
        assert_eq!(mappings.notices, []);
    }
}
