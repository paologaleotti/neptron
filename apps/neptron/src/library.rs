//! The game library of the frontend: the catalog, the index from the cache
//! and the background scan, and the numbers the UI uses for systems and
//! games.

use std::path::{Path, PathBuf};

use neptron_library::cache::{self, CacheLoad};
use neptron_library::{Catalog, CoreDef, Game, Index, ScanWorker, SystemDef};
use neptron_ui::snapshot::{GameEntry, GameId, GameSource, GameWindow, SystemEntry, SystemId};

use crate::relaunch::GameRef;

/// The catalog, the index and the scan that refreshes it.
#[derive(Debug)]
pub struct Library {
    catalog: Catalog,
    index: Index,
    worker: Option<ScanWorker>,
}

/// A game of the index with its system and core.
#[derive(Debug, Clone, Copy)]
pub struct Found<'a> {
    /// The system.
    pub system: &'a SystemDef,
    /// The core of the system.
    pub core: &'a CoreDef,
    /// The game.
    pub game: &'a Game,
}

impl Library {
    /// The library with the index of the cache at `cache_path`. A missing
    /// or damaged cache gives an empty index and the reason; the scan then
    /// fills it. The cache load reads a file, so it runs at start, before
    /// the frame loop.
    pub fn load(catalog: Catalog, cache_path: &Path) -> (Self, Option<String>) {
        let (index, issue) = match cache::load(cache_path, &catalog) {
            CacheLoad::Loaded(index) => (index, None),
            CacheLoad::Missing => (Index::new(), None),
            CacheLoad::Discarded(e) => (Index::new(), Some(format!("library cache: {e}"))),
        };
        (
            Self {
                catalog,
                index,
                worker: None,
            },
            issue,
        )
    }

    /// Starts a scan of `roms` on its own thread.
    ///
    /// # Errors
    ///
    /// The text of the scan error.
    pub fn start_scan(&mut self, roms: PathBuf) -> Result<(), String> {
        let worker = ScanWorker::spawn(roms, self.catalog.clone()).map_err(|e| e.to_string())?;
        self.worker = Some(worker);
        Ok(())
    }

    /// Takes the result of a finished scan: the new index replaces the old
    /// one. `None` while no scan has finished. It never waits: the join
    /// runs only when the thread has ended.
    pub fn poll(&mut self) -> Option<Result<&Index, String>> {
        if !self.worker.as_ref().is_some_and(ScanWorker::is_finished) {
            return None;
        }
        let worker = self.worker.take()?;
        Some(match worker.join() {
            Ok(outcome) => {
                self.index = outcome.index;
                Ok(&self.index)
            }
            Err(e) => Err(e.to_string()),
        })
    }

    /// The catalog.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// The index.
    pub fn index(&self) -> &Index {
        &self.index
    }

    /// The systems that hold games, in catalog order, for the home screen.
    pub fn systems(&self) -> Vec<SystemEntry> {
        self.catalog
            .systems()
            .iter()
            .enumerate()
            .filter_map(|(i, def)| {
                let count = self.index.count(&def.folder);
                (count > 0).then(|| {
                    SystemEntry::new(
                        system_id(i),
                        def.name.clone(),
                        u32::try_from(count).unwrap_or(u32::MAX),
                    )
                })
            })
            .collect()
    }

    /// The rows `first..first + count` of a game list. Favourites and
    /// recent games are not kept in the MVP, so their lists are empty.
    pub fn window(&self, source: GameSource, first: usize, count: usize) -> GameWindow {
        let GameSource::System(id) = source else {
            return GameWindow::new(source, 0, 0, Vec::new());
        };
        let Some(def) = self.catalog.systems().get(id.0 as usize) else {
            return GameWindow::new(source, 0, 0, Vec::new());
        };
        let total = self.index.count(&def.folder);
        let rows = self
            .index
            .page(&def.folder, first, count)
            .iter()
            .enumerate()
            .map(|(i, game)| GameEntry::new(game_id(id, first + i), id, game.name()))
            .collect();
        GameWindow::new(source, first, total, rows)
    }

    /// The game with this number.
    pub fn game(&self, id: GameId) -> Option<Found<'_>> {
        let system_index = usize::try_from(id.0 >> 32).ok()?;
        let game_index = usize::try_from(id.0 & u64::from(u32::MAX)).ok()?;
        let system = self.catalog.systems().get(system_index)?;
        let game = self.index.system(&system.folder)?.games().get(game_index)?;
        let core = self.catalog.core_for(&system.folder)?;
        Some(Found { system, core, game })
    }

    /// The game of a relaunch file.
    pub fn find(&self, game: &GameRef) -> Option<GameId> {
        let system_index = self
            .catalog
            .systems()
            .iter()
            .position(|s| s.folder == game.system)?;
        let games = self.index.system(&game.system)?.games();
        let game_index = games.iter().position(|g| g.path() == game.path)?;
        Some(game_id(system_id(system_index), game_index))
    }

    /// The relaunch reference of a game.
    pub fn reference(&self, id: GameId) -> Option<GameRef> {
        let found = self.game(id)?;
        Some(GameRef {
            system: found.system.folder.clone(),
            path: found.game.path().to_owned(),
        })
    }

    /// The system and core of a game that the index does not list yet: a
    /// relaunch into a game found by a scan that has not reached the cache,
    /// or a launch from the control protocol.
    pub fn resolve(&self, system: &str) -> Option<(&SystemDef, &CoreDef)> {
        let def = self.catalog.system(system)?;
        Some((def, self.catalog.core_for(system)?))
    }
}

fn system_id(index: usize) -> SystemId {
    SystemId(u32::try_from(index).unwrap_or(u32::MAX))
}

fn game_id(system: SystemId, index: usize) -> GameId {
    GameId((u64::from(system.0) << 32) | (index as u64 & u64::from(u32::MAX)))
}

#[cfg(test)]
mod tests {
    use neptron_library::SystemGames;

    use super::*;

    fn library() -> Library {
        let games = vec![
            Game::new("Beta".to_owned(), "snes/Beta.sfc".to_owned()),
            Game::new("Alpha".to_owned(), "snes/Alpha.sfc".to_owned()),
        ];
        Library {
            catalog: Catalog::builtin().expect("catalog"),
            index: Index::from_systems(vec![SystemGames::new("snes".to_owned(), games)]),
            worker: None,
        }
    }

    #[test]
    fn only_systems_with_games_show() {
        let systems = library().systems();
        assert_eq!(systems.len(), 1);
        assert_eq!(systems[0].id, SystemId(1));
        assert_eq!(systems[0].game_count, 2);
    }

    #[test]
    fn a_window_holds_the_rows_and_their_numbers() {
        let lib = library();
        let window = lib.window(GameSource::System(SystemId(1)), 1, 10);
        assert_eq!(window.total, 2);
        assert_eq!(window.first, 1);
        assert_eq!(window.rows.len(), 1);
        let found = lib.game(window.rows[0].id).expect("the game");
        assert_eq!(found.game.name(), "Beta");
        assert_eq!(found.core.id, "snes9x");
        assert!(lib.window(GameSource::Favourites, 0, 10).rows.is_empty());
        assert!(
            lib.window(GameSource::System(SystemId(9)), 0, 10)
                .rows
                .is_empty()
        );
    }

    #[test]
    fn a_game_reference_finds_the_same_game() {
        let lib = library();
        let id = lib.find(&GameRef {
            system: "snes".to_owned(),
            path: "snes/Alpha.sfc".to_owned(),
        });
        let id = id.expect("found");
        assert_eq!(lib.game(id).expect("game").game.name(), "Alpha");
        assert_eq!(lib.reference(id).expect("ref").path, "snes/Alpha.sfc");
        assert!(lib.game(GameId(u64::MAX)).is_none());
    }
}
