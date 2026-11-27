//! The game index in memory and its queries. Queries do no I/O and no allocation.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::disc::{DiscFormat, disc_tag};
use crate::folders::FolderStamp;

/// The longest name or path, in bytes, that the index holds.
///
/// The scan skips a game with a longer path, and the cache rejects a longer
/// string, so every index can go through the cache.
pub const MAX_TEXT_BYTES: usize = 4096;

/// One game: a ROM file under `share/roms/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    name: String,
    path: String,
    size: u64,
    parent: Option<String>,
    details: String,
    marks: Marks,
    /// The discs of a list: the paths that an m3u file names, or the files
    /// of a group of discs.
    discs: Vec<String>,
    /// The entry of a group of disc files, made by [`SystemGames::new`]. It
    /// is not a file of its own, so the cache does not hold it.
    grouped: bool,
}

/// The marks of a game in the list. An arcade game gets them from the
/// arcade index; other games have none.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Marks {
    /// The game has a vertical screen.
    pub vertical: bool,
    /// The core does not run the game well.
    pub not_working: bool,
    /// The zip stem is not a set name of the arcade index.
    pub unknown_set: bool,
}

/// The games of one system, in sort order.
///
/// A clone whose parent set is in the same list is folded: it is not in
/// [`SystemGames::games`] but under its parent, in
/// [`SystemGames::versions`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemGames {
    system: String,
    games: Vec<Game>,
    /// The folded clones, sorted by parent set name (ASCII lower case),
    /// then in game order.
    folded: Vec<Game>,
    /// The disc files that an m3u file names or that a group holds. They
    /// are not in the list.
    hidden: Vec<Game>,
}

/// Every game that a scan found, by system, and the stamps of the folders
/// that the scan read.
///
/// Only systems with at least one game are in the index. Systems keep the
/// order in which they were given, and the games of a system are sorted by
/// [`compare_names`], then by path. The folder stamps are sorted by path.
/// An index with no stamps makes a refresh read every folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Index {
    systems: Vec<SystemGames>,
    folders: Vec<FolderStamp>,
}

/// The sort order of game names.
///
/// Names compare by their Unicode lower-case characters, code point by code
/// point, with no locale rules and no special case for leading articles.
/// Equal names then compare by their bytes, so the order is total and the
/// same on every machine. The comparison allocates nothing.
pub fn compare_names(a: &str, b: &str) -> Ordering {
    fold_case(a).cmp(fold_case(b)).then_with(|| a.cmp(b))
}

impl Game {
    /// Makes a game from its display name and its path relative to
    /// `share/roms/`, with a size of 0 bytes.
    pub fn new(name: String, path: String) -> Self {
        Self {
            name,
            path,
            size: 0,
            parent: None,
            details: String::new(),
            marks: Marks::default(),
            discs: Vec::new(),
            grouped: false,
        }
    }

    /// The same game with the discs of a list: paths below `share/roms/`.
    pub fn with_discs(mut self, discs: Vec<String>) -> Self {
        self.discs = discs;
        self
    }

    /// The discs of a list, in order: the paths below `share/roms/` that an
    /// m3u file names, or the files of a group of discs, the first disc
    /// first. Empty for any other game.
    pub fn discs(&self) -> &[String] {
        &self.discs
    }

    /// True for the entry of a group of disc files: its path is the path of
    /// its first disc, and [`Game::discs`] holds every disc.
    pub fn is_group(&self) -> bool {
        self.grouped
    }

    /// The same game as a clone of the set `parent`.
    pub fn with_parent(mut self, parent: Option<String>) -> Self {
        self.parent = parent;
        self
    }

    /// The same game with a details line, for example the region.
    pub fn with_details(mut self, details: String) -> Self {
        self.details = details;
        self
    }

    /// The same game with marks.
    pub fn with_marks(mut self, marks: Marks) -> Self {
        self.marks = marks;
        self
    }

    /// The parent set of an arcade clone.
    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }

    /// The details line: for an arcade game, the last bracket group of its
    /// description. Empty when there is none.
    pub fn details(&self) -> &str {
        &self.details
    }

    /// The marks in the list.
    pub fn marks(&self) -> Marks {
        self.marks
    }

    /// The file name with no extension and no folder: the set name of an
    /// arcade game.
    pub fn stem(&self) -> &str {
        let file = self.path.rsplit('/').next().unwrap_or(&self.path);
        file.rsplit_once('.').map_or(file, |(stem, _)| stem)
    }

    /// The same game with a file size in bytes.
    pub fn with_size(mut self, size: u64) -> Self {
        self.size = size;
        self
    }

    /// The file size in bytes when the scan read its folder. A file that a
    /// network copy still writes can grow after that.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Display name: the title of an arcade set, or the file name with no
    /// extension.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Path relative to `share/roms/`, with `/` between the parts.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// True when the game is a `.zip` archive, which the core host opens
    /// at launch. The extension decides, with no regard to ASCII case; the
    /// scan never opens the file.
    pub fn is_zip(&self) -> bool {
        self.path
            .rsplit_once('.')
            .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("zip"))
    }
}

impl SystemGames {
    /// Makes the game list of one system. The games are sorted and the
    /// clones whose parent is in the list are folded here.
    pub fn new(system: String, games: Vec<Game>) -> Self {
        let (games, hidden) = arrange_discs(games);
        let stems: HashSet<String> = games
            .iter()
            .map(|g| g.stem().to_ascii_lowercase())
            .collect();
        let (mut folded, mut games): (Vec<Game>, Vec<Game>) = games.into_iter().partition(|g| {
            g.parent
                .as_deref()
                .is_some_and(|p| stems.contains(&p.to_ascii_lowercase()))
        });
        games.sort_by(order);
        folded.sort_by(|a, b| fold_key(a).cmp(fold_key(b)).then_with(|| order(a, b)));
        Self {
            system,
            games,
            folded,
            hidden,
        }
    }

    /// The system id.
    pub fn system(&self) -> &str {
        &self.system
    }

    /// Every game of the list, in sort order. Folded clones are not in it.
    pub fn games(&self) -> &[Game] {
        &self.games
    }

    /// Every game file: those of the list, then the folded clones, then
    /// the hidden disc files. The entry of a group of discs is not a file,
    /// so it is not here; its discs are.
    pub fn all(&self) -> impl Iterator<Item = &Game> {
        self.games
            .iter()
            .filter(|g| !g.grouped)
            .chain(&self.folded)
            .chain(&self.hidden)
    }

    /// The disc files that are not in the list: those that an m3u file
    /// names, and those of a group of discs.
    pub fn hidden(&self) -> &[Game] {
        &self.hidden
    }

    /// The folded clones of the set named `stem`, in sort order: its
    /// "other versions". The match ignores ASCII case. It allocates nothing.
    pub fn versions(&self, stem: &str) -> &[Game] {
        self.folded.get(self.versions_range(stem)).unwrap_or(&[])
    }

    /// The positions in [`SystemGames::folded`] of the folded clones of the
    /// set named `stem`. The match ignores ASCII case. It allocates nothing.
    pub fn versions_range(&self, stem: &str) -> std::ops::Range<usize> {
        let wanted = || stem.bytes().map(|b| b.to_ascii_lowercase());
        let start = self.folded.partition_point(|g| fold_key(g).lt(wanted()));
        let end = self.folded.partition_point(|g| fold_key(g).le(wanted()));
        start..end
    }

    /// Every folded clone, sorted by parent set name, then in game order.
    pub fn folded(&self) -> &[Game] {
        &self.folded
    }

    /// Number of games of the list, folded clones not counted.
    pub fn len(&self) -> usize {
        self.games.len()
    }

    /// True when the system has no games.
    pub fn is_empty(&self) -> bool {
        self.games.is_empty()
    }

    /// At most `len` games from position `start`. Positions past the end give fewer games or none.
    pub fn page(&self, start: usize, len: usize) -> &[Game] {
        let start = start.min(self.games.len());
        let end = start.saturating_add(len).min(self.games.len());
        &self.games[start..end]
    }
}

impl Index {
    /// An empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes an index from game lists. Empty lists are dropped. A system
    /// that comes twice gets one list with the games of both, at the
    /// position of its first list.
    ///
    /// The work is linear in the number of lists plus one sort per merged
    /// system, so many lists cannot make it slow.
    pub fn from_systems(systems: Vec<SystemGames>) -> Self {
        let mut position: HashMap<String, usize> = HashMap::new();
        let mut merged: Vec<SystemGames> = Vec::new();
        let mut repeated = Vec::new();
        for list in systems
            .into_iter()
            .filter(|s| !s.games.is_empty() || !s.folded.is_empty() || !s.hidden.is_empty())
        {
            match position.get(&list.system) {
                Some(&i) => {
                    if let Some(m) = merged.get_mut(i) {
                        m.games.extend(list.games);
                        m.games.extend(list.folded);
                        m.games.extend(list.hidden);
                        repeated.push(i);
                    }
                }
                None => {
                    position.insert(list.system.clone(), merged.len());
                    merged.push(list);
                }
            }
        }
        repeated.sort_unstable();
        repeated.dedup();
        for i in repeated {
            if let Some(m) = merged.get_mut(i) {
                let games = std::mem::take(&mut m.games)
                    .into_iter()
                    .chain(std::mem::take(&mut m.folded))
                    .chain(std::mem::take(&mut m.hidden))
                    .collect();
                *m = SystemGames::new(std::mem::take(&mut m.system), games);
            }
        }
        Self {
            systems: merged,
            folders: Vec::new(),
        }
    }

    /// The same index with the stamps of the folders that the scan read.
    /// They are sorted by path here; of two stamps with one path the first
    /// stays.
    pub fn with_folders(mut self, mut folders: Vec<FolderStamp>) -> Self {
        folders.sort_by(|a, b| a.path.cmp(&b.path));
        folders.dedup_by(|b, a| a.path == b.path);
        self.folders = folders;
        self
    }

    /// The stamps of the folders that the scan read, sorted by path.
    pub fn folders(&self) -> &[FolderStamp] {
        &self.folders
    }

    /// The stamp of the folder at `path`.
    pub fn folder(&self, path: &str) -> Option<&FolderStamp> {
        self.folders
            .binary_search_by(|f| f.path.as_str().cmp(path))
            .ok()
            .and_then(|i| self.folders.get(i))
    }

    /// Every system with games.
    pub fn systems(&self) -> &[SystemGames] {
        &self.systems
    }

    /// The games of one system.
    pub fn system(&self, id: &str) -> Option<&SystemGames> {
        self.systems.iter().find(|s| s.system == id)
    }

    /// Number of games of one system. An unknown system has none.
    pub fn count(&self, id: &str) -> usize {
        self.system(id).map_or(0, SystemGames::len)
    }

    /// Number of games of every system.
    pub fn total(&self) -> usize {
        self.systems.iter().map(SystemGames::len).sum()
    }

    /// At most `len` games of one system from position `start`.
    pub fn page(&self, id: &str, start: usize, len: usize) -> &[Game] {
        self.system(id).map_or(&[], |s| s.page(start, len))
    }
}

/// Splits the game files of one list into the games of the list and the
/// hidden disc files (`docs/spec/disc-games.md`, sections 3.3 and 3.4): a
/// file that an m3u file names is hidden, and the disc files of one game
/// with a disc tag make one group entry. Group entries in `games` come from
/// an earlier arrangement and are made again from their files.
fn arrange_discs(games: Vec<Game>) -> (Vec<Game>, Vec<Game>) {
    let mut games: Vec<Game> = games.into_iter().filter(|g| !g.grouped).collect();
    if !games
        .iter()
        .any(|g| !g.discs.is_empty() || disc_extension(g))
    {
        return (games, Vec::new());
    }
    // exFAT ignores case, so a line names a file in any case. A line that
    // names an m3u file is an error of its list and hides nothing, so no
    // list can hide another list, or itself.
    let named: HashSet<String> = games
        .iter()
        .flat_map(|g| &g.discs)
        .filter(|d| !names_a_list(d))
        .map(|d| d.to_lowercase())
        .collect();
    let is_named: Vec<bool> = games
        .iter()
        .map(|g| named.contains(&g.path.to_lowercase()))
        .collect();
    let mut groups: HashMap<GroupKey<'_>, Vec<(u8, usize)>> = HashMap::new();
    for (i, game) in games.iter().enumerate() {
        if is_named[i] || !disc_extension(game) {
            continue;
        }
        if let Some(tag) = disc_tag(game.stem()) {
            let folder = game.path.rsplit_once('/').map_or("", |(f, _)| f);
            let extension = extension(game).to_ascii_lowercase();
            groups
                .entry((folder, tag.group, extension))
                .or_default()
                .push((tag.number, i));
        }
    }
    let mut member = vec![false; games.len()];
    let mut entries = Vec::new();
    for ((_, name, _), mut discs) in groups {
        discs.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| games[a.1].path.cmp(&games[b.1].path))
        });
        discs.dedup_by_key(|(number, _)| *number);
        if discs.len() < 2 {
            continue;
        }
        let first = &games[discs[0].1];
        let paths = discs.iter().map(|&(_, i)| games[i].path.clone()).collect();
        entries.push(Game {
            name,
            discs: paths,
            grouped: true,
            ..first.clone()
        });
        for &(_, i) in &discs {
            member[i] = true;
        }
    }
    let mut hidden = Vec::new();
    let mut listed = Vec::with_capacity(games.len());
    for (i, game) in games.drain(..).enumerate() {
        if is_named[i] || member[i] {
            hidden.push(game);
        } else {
            listed.push(game);
        }
    }
    listed.extend(entries);
    hidden.sort_by(|a, b| a.path.cmp(&b.path));
    (listed, hidden)
}

/// True for a disc path of a list that names an m3u file.
fn names_a_list(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| DiscFormat::of_extension(ext) == Some(DiscFormat::M3u))
}

/// The folder, the group name and the lower-case extension of a disc file.
type GroupKey<'a> = (&'a str, String, String);

/// True for a file of a disc format that can carry a disc tag: `.chd` and
/// `.cue`.
fn disc_extension(game: &Game) -> bool {
    matches!(
        DiscFormat::of_extension(extension(game)),
        Some(DiscFormat::Chd | DiscFormat::Cue)
    )
}

fn extension(game: &Game) -> &str {
    let file = game.path.rsplit('/').next().unwrap_or(&game.path);
    file.rsplit_once('.').map_or("", |(_, ext)| ext)
}

fn order(a: &Game, b: &Game) -> Ordering {
    compare_names(&a.name, &b.name).then_with(|| a.path.cmp(&b.path))
}

/// The parent set name of a folded clone, in ASCII lower case. Every
/// folded game has a parent.
fn fold_key(game: &Game) -> impl Iterator<Item = u8> + '_ {
    game.parent
        .as_deref()
        .unwrap_or_default()
        .bytes()
        .map(|b| b.to_ascii_lowercase())
}

fn fold_case(s: &str) -> impl Iterator<Item = char> + '_ {
    s.chars().flat_map(char::to_lowercase)
}

#[cfg(test)]
mod tests;
