//! The game index in memory and its queries. Queries do no I/O and no allocation.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

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
        }
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

    /// Every game: those of the list, then the folded clones.
    pub fn all(&self) -> impl Iterator<Item = &Game> {
        self.games.iter().chain(&self.folded)
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
            .filter(|s| !s.games.is_empty() || !s.folded.is_empty())
        {
            match position.get(&list.system) {
                Some(&i) => {
                    if let Some(m) = merged.get_mut(i) {
                        m.games.extend(list.games);
                        m.games.extend(list.folded);
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
