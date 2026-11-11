//! The game index in memory and its queries. Queries do no I/O and no allocation.

use std::cmp::Ordering;
use std::collections::HashMap;

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
}

/// The games of one system, in sort order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemGames {
    system: String,
    games: Vec<Game>,
}

/// Every game that a scan found, by system.
///
/// Only systems with at least one game are in the index. Systems keep the
/// order in which they were given, and the games of a system are sorted by
/// [`compare_names`], then by path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Index {
    systems: Vec<SystemGames>,
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
    /// Makes a game from its display name and its path relative to `share/roms/`.
    pub fn new(name: String, path: String) -> Self {
        Self { name, path }
    }

    /// Display name: the file name with no extension.
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
    /// Makes the game list of one system. The games are sorted here.
    pub fn new(system: String, mut games: Vec<Game>) -> Self {
        games.sort_by(order);
        Self { system, games }
    }

    /// The system id.
    pub fn system(&self) -> &str {
        &self.system
    }

    /// Every game, in sort order.
    pub fn games(&self) -> &[Game] {
        &self.games
    }

    /// Number of games.
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
        for list in systems.into_iter().filter(|s| !s.is_empty()) {
            match position.get(&list.system) {
                Some(&i) => {
                    if let Some(m) = merged.get_mut(i) {
                        m.games.extend(list.games);
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
                m.games.sort_by(order);
            }
        }
        Self { systems: merged }
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

fn fold_case(s: &str) -> impl Iterator<Item = char> + '_ {
    s.chars().flat_map(char::to_lowercase)
}

#[cfg(test)]
mod tests;
