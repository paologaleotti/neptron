//! Disc games: the disc formats, the disc tag of a file name, the region of
//! a name, and the text of m3u and cue files. Everything here is pure; the
//! scan and the launch check read the files.

/// The most bytes of an m3u or cue file that NEPTRON reads.
pub const LIST_MAX_BYTES: usize = 64 * 1024;

/// The most discs of one m3u file. Later lines are ignored.
pub const M3U_MAX_DISCS: usize = 64;

/// The largest number of a disc tag.
pub const DISC_NUMBER_MAX: u8 = 99;

/// A disc format of a system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscFormat {
    /// A compressed disc image. The core reads it.
    Chd,
    /// A cue sheet with its bin files.
    Cue,
    /// A list of discs, one path per line.
    M3u,
}

/// The region of a game or a BIOS file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    /// Japan.
    NtscJ,
    /// North America.
    NtscU,
    /// Europe and the other 50 Hz countries.
    Pal,
}

/// A disc tag of a file name: `(Disc N)` or `(Disc N of M)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscTag {
    /// The number of the disc, 1 to [`DISC_NUMBER_MAX`].
    pub number: u8,
    /// The name with the tag and one space before it removed.
    pub group: String,
}

impl DiscFormat {
    /// The format of a file extension (no dot), ignoring ASCII case.
    pub fn of_extension(extension: &str) -> Option<Self> {
        [("chd", Self::Chd), ("cue", Self::Cue), ("m3u", Self::M3u)]
            .into_iter()
            .find(|(e, _)| e.eq_ignore_ascii_case(extension))
            .map(|(_, f)| f)
    }
}

impl Region {
    /// The region of a system file token: `ntsc-j`, `ntsc-u` or `pal`.
    pub fn of_token(token: &str) -> Option<Self> {
        match token {
            "ntsc-j" => Some(Self::NtscJ),
            "ntsc-u" => Some(Self::NtscU),
            "pal" => Some(Self::Pal),
            _ => None,
        }
    }

    /// The word that the BIOS files page shows.
    pub fn word(self) -> &'static str {
        match self {
            Self::NtscJ => "Japan",
            Self::NtscU => "USA",
            Self::Pal => "Europe",
        }
    }
}

/// The region words of file names, compared with no regard to ASCII case.
const REGION_WORDS: &[(&str, Region)] = &[
    ("japan", Region::NtscJ),
    ("j", Region::NtscJ),
    ("ntsc-j", Region::NtscJ),
    ("usa", Region::NtscU),
    ("us", Region::NtscU),
    ("u", Region::NtscU),
    ("canada", Region::NtscU),
    ("ntsc-u", Region::NtscU),
    ("europe", Region::Pal),
    ("e", Region::Pal),
    ("pal", Region::Pal),
    ("uk", Region::Pal),
    ("germany", Region::Pal),
    ("france", Region::Pal),
    ("italy", Region::Pal),
    ("spain", Region::Pal),
    ("australia", Region::Pal),
    ("netherlands", Region::Pal),
    ("sweden", Region::Pal),
];

/// The region of a game name: the first group in brackets whose first
/// word, before a `,`, is a region word. `None` when no group has one.
pub fn region_of(name: &str) -> Option<Region> {
    groups(name).find_map(|(_, inner)| {
        let first = inner.split(',').next().unwrap_or_default().trim();
        REGION_WORDS
            .iter()
            .find(|(word, _)| word.eq_ignore_ascii_case(first))
            .map(|(_, region)| *region)
    })
}

/// The first disc tag of `stem`, a file name with no extension.
pub fn disc_tag(stem: &str) -> Option<DiscTag> {
    let (start, end, number) = groups(stem).find_map(|(range, inner)| {
        tag_number(inner).map(|number| (range.start, range.end, number))
    })?;
    let before = stem.get(..start).unwrap_or_default();
    let before = before.strip_suffix(' ').unwrap_or(before);
    let after = stem.get(end..).unwrap_or_default();
    Some(DiscTag {
        number,
        group: format!("{before}{after}"),
    })
}

/// The save name of a disc game: its name with no disc tag.
pub fn save_name(stem: &str) -> String {
    disc_tag(stem).map_or_else(|| stem.to_owned(), |tag| tag.group)
}

/// The discs of an m3u text, in order: each line with no spaces at its
/// ends, with the empty lines and the `#` lines left out, at most
/// [`M3U_MAX_DISCS`].
pub fn m3u_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .take(M3U_MAX_DISCS)
        .collect()
}

/// The file names of the `FILE` lines of a cue text, in order. A name is
/// in double quotes, or is the first word after `FILE`.
pub fn cue_files(text: &str) -> Vec<&str> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let rest = line
                .get(..4)
                .filter(|k| k.eq_ignore_ascii_case("FILE"))
                .and_then(|_| line.get(4..))?;
            if !rest.starts_with([' ', '\t']) {
                return None;
            }
            let rest = rest.trim_start();
            let name = match rest.strip_prefix('"') {
                Some(quoted) => quoted.split('"').next()?,
                None => rest.split_whitespace().next()?,
            };
            (!name.is_empty()).then_some(name)
        })
        .collect()
}

/// The path below `share/roms/` of an m3u line, from the path of the m3u
/// file below `share/roms/`. `None` for a line that leaves the folder of
/// the m3u file or has an empty, `.` or `..` part, or an absolute path:
/// such a line hides no game.
pub fn m3u_disc_path(m3u: &str, line: &str) -> Option<String> {
    if line.starts_with(['/', '\\']) || line.get(1..2) == Some(":") {
        return None;
    }
    let folder = m3u.rsplit_once('/').map_or("", |(folder, _)| folder);
    let mut path = folder.to_owned();
    for part in line.split(['/', '\\']) {
        if part.is_empty() || part == "." || part == ".." {
            return None;
        }
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(part);
    }
    Some(path)
}

/// Each group in brackets of `name`: its byte range with the brackets, and
/// its text.
fn groups(name: &str) -> impl Iterator<Item = (std::ops::Range<usize>, &str)> {
    let mut at = 0;
    std::iter::from_fn(move || {
        let open = at + name.get(at..)?.find('(')?;
        let close = open + name.get(open..)?.find(')')?;
        at = close + 1;
        Some((open..close + 1, name.get(open + 1..close)?))
    })
}

/// The number of a tag text: `Disc N` or `Disc N of M`.
fn tag_number(inner: &str) -> Option<u8> {
    let mut words = inner.split(' ');
    if !words.next()?.eq_ignore_ascii_case("disc") {
        return None;
    }
    let number = disc_number(words.next()?)?;
    match (words.next(), words.next(), words.next()) {
        (None, _, _) => Some(number),
        (Some("of"), Some(total), None) => disc_number(total).map(|_| number),
        _ => None,
    }
}

fn disc_number(text: &str) -> Option<u8> {
    if text.is_empty() || text.len() > 2 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse::<u8>()
        .ok()
        .filter(|n| (1..=DISC_NUMBER_MAX).contains(n))
}

#[cfg(test)]
mod tests;
