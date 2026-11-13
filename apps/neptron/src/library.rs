//! The game library of the frontend: the catalog, the index from the cache
//! and the background scan, and the numbers the UI uses for systems and
//! games.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::SystemTime;

use neptron_library::cache::{self, CacheError, CacheLoad};
use neptron_library::{Catalog, Changes, CoreDef, Game, Index, SystemDef, changes, refresh};
use neptron_ui::snapshot::{GameEntry, GameId, GameSource, GameWindow, SystemEntry, SystemId};

use crate::relaunch::GameRef;

/// The catalog, the index and the scan that refreshes it.
///
/// The index is shared with the scan thread, which compares it with its
/// result, and with the writer thread, which writes it to the cache, so
/// neither needs a copy.
#[derive(Debug)]
pub struct Library {
    catalog: Catalog,
    index: Arc<Index>,
    ids: GameIds,
    scan: Option<Running>,
}

/// Why a scan runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanKind {
    /// The game list of a new boot, or of a return when no scan of every
    /// folder reached the cache in this power cycle. Every folder is read,
    /// because the stick may have been changed on another computer, whose
    /// exFAT driver may leave the folder times as they were.
    Boot,
    /// A return to the game list after a game, when a scan of every
    /// folder reached the cache in this power cycle
    /// ([`crate::Paths::library_scanned`]). Only the folders whose time
    /// changed are read.
    Check,
    /// The player asked for it. Every folder is read.
    Rescan,
}

/// What [`Library::start_scan`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanStart {
    /// A scan started.
    Started,
    /// A scan runs already; no second one starts.
    Running,
}

/// A scan that ended and replaced the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scanned {
    /// Why it ran.
    pub kind: ScanKind,
    /// What changed from the index before it.
    pub changes: Changes,
    /// The index before it was empty: no cache, or a cache of another
    /// version.
    pub first: bool,
}

/// A scan on a thread that the library owns. Drop cancels the scan and
/// joins the thread; the scan stops at its next directory entry. The frame
/// thread only drops it at the end of the run.
#[derive(Debug)]
struct Running {
    kind: ScanKind,
    cancel: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<Prepared, String>>>,
}

/// What the scan thread gives back: the new index with its ids and its
/// changes, all made off the frame thread. The comparison and the ids of
/// 10,000 games take several milliseconds, up to tens on a loaded machine.
#[derive(Debug)]
struct Prepared {
    index: Arc<Index>,
    ids: GameIds,
    changes: Changes,
}

/// The game ids of the index. An id comes from the system and the path of
/// its game, so it stays with the game when a scan adds or removes other
/// games, and the game list keeps its focus.
#[derive(Debug, Default)]
struct GameIds {
    /// The ids of each catalog system, in the order of its games.
    by_system: Vec<Vec<GameId>>,
    /// The catalog system and the position of each id.
    places: HashMap<u64, (usize, usize)>,
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
    /// cache, or one of an older format, gives an empty index; a damaged
    /// one gives an empty index and the reason. The scan then fills it.
    /// The cache load reads a file, so it runs at start, before the frame
    /// loop.
    pub fn load(catalog: Catalog, cache_path: &Path) -> (Self, Option<String>) {
        let (index, issue) = match cache::load(cache_path, &catalog) {
            CacheLoad::Loaded(index) => (index, None),
            CacheLoad::Missing => (Index::new(), None),
            // An update writes a newer format; the old file is no fault,
            // and the scan that follows writes the new one.
            CacheLoad::Discarded(CacheError::Version { found, expected }) if found < expected => {
                (Index::new(), None)
            }
            CacheLoad::Discarded(e) => (Index::new(), Some(format!("library cache: {e}"))),
        };
        (Self::with_index(catalog, index), issue)
    }

    fn with_index(catalog: Catalog, index: Index) -> Self {
        let ids = GameIds::of(&catalog, &index);
        Self {
            catalog,
            index: Arc::new(index),
            ids,
            scan: None,
        }
    }

    /// Starts a scan of `roms` on its own thread, unless one runs. `now` is
    /// the wall time, which a [`ScanKind::Check`] compares with the folder
    /// times. The thread also compares the result with the index and makes
    /// the ids of the result.
    ///
    /// # Errors
    ///
    /// The text of the error when the thread cannot start.
    pub fn start_scan(
        &mut self,
        roms: PathBuf,
        kind: ScanKind,
        now: SystemTime,
    ) -> Result<ScanStart, String> {
        if self.scan.is_some() {
            return Ok(ScanStart::Running);
        }
        let catalog = self.catalog.clone();
        let previous = Arc::clone(&self.index);
        let cancel = Arc::new(AtomicBool::new(false));
        let theirs = Arc::clone(&cancel);
        let thread = thread::Builder::new()
            .name("neptron-scan".into())
            .spawn(move || prepare(&roms, &catalog, &previous, kind, now, &theirs))
            .map_err(|e| format!("cannot start the scan thread: {e}"))?;
        self.scan = Some(Running {
            kind,
            cancel,
            thread: Some(thread),
        });
        Ok(ScanStart::Started)
    }

    /// True while a scan runs.
    pub fn scanning(&self) -> bool {
        self.scan.is_some()
    }

    /// Takes the result of a finished scan: the new index replaces the old
    /// one, and the ids follow it. `None` while no scan has finished. It
    /// never waits: the join runs only when the thread has ended. A failed
    /// scan keeps the old index.
    ///
    /// The frame thread only swaps the new index in and drops the old one:
    /// about 0.4 ms for 10,000 games in the dev profile, which the test
    /// `ten_thousand_game_scan_end_fits_the_frame_budget` measures.
    pub fn poll(&mut self) -> Option<Result<Scanned, String>> {
        let finished = self
            .scan
            .as_ref()
            .and_then(|s| s.thread.as_ref())
            .is_some_and(JoinHandle::is_finished);
        if !finished {
            return None;
        }
        let mut running = self.scan.take()?;
        let kind = running.kind;
        let joined = running.thread.take()?.join();
        Some(match joined {
            Ok(Ok(prepared)) => Ok(self.replace(prepared, kind)),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("the scan thread panicked".to_owned()),
        })
    }

    /// Puts a prepared index in place of the index, and says what changed.
    fn replace(&mut self, prepared: Prepared, kind: ScanKind) -> Scanned {
        let scanned = Scanned {
            kind,
            changes: prepared.changes,
            first: self.index.systems().is_empty(),
        };
        self.index = prepared.index;
        self.ids = prepared.ids;
        scanned
    }

    /// The index, shared, for the cache job of the writer thread.
    pub fn shared_index(&self) -> Arc<Index> {
        Arc::clone(&self.index)
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
        let ids = self
            .ids
            .by_system
            .get(id.0 as usize)
            .map_or(&[][..], |ids| ids.get(first..).unwrap_or(&[]));
        let rows = self
            .index
            .page(&def.folder, first, count)
            .iter()
            .zip(ids)
            .map(|(game, game_id)| GameEntry::new(*game_id, id, game.name()))
            .collect();
        GameWindow::new(source, first, total, rows)
    }

    /// The game with this id.
    pub fn game(&self, id: GameId) -> Option<Found<'_>> {
        let &(system_index, game_index) = self.ids.places.get(&id.0)?;
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
        self.ids
            .by_system
            .get(system_index)?
            .get(game_index)
            .copied()
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

impl Drop for Running {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            // The owner gave up the result, so a cancelled or failed scan
            // needs no report here.
            let _ = thread.join();
        }
    }
}

/// The work of the scan thread: the scan, the comparison with `previous`,
/// and the ids of the result. A [`ScanKind::Check`] keeps the unchanged
/// folders of `previous`; the other kinds read every folder.
fn prepare(
    roms: &Path,
    catalog: &Catalog,
    previous: &Index,
    kind: ScanKind,
    now: SystemTime,
    cancel: &AtomicBool,
) -> Result<Prepared, String> {
    let none = Index::new();
    let base = match kind {
        ScanKind::Check => previous,
        ScanKind::Boot | ScanKind::Rescan => &none,
    };
    let outcome =
        refresh(roms, catalog, base, now, cancel, &mut |_| {}).map_err(|e| e.to_string())?;
    Ok(Prepared {
        changes: changes(previous, &outcome.index),
        ids: GameIds::of(catalog, &outcome.index),
        index: Arc::new(outcome.index),
    })
}

impl GameIds {
    /// The ids of every game of `index`. An id is the FNV-1a hash of the
    /// system and the path; an id that another game has already takes the
    /// next free number, so ids never collide, and they depend only on the
    /// index.
    fn of(catalog: &Catalog, index: &Index) -> Self {
        let mut by_system = Vec::with_capacity(catalog.systems().len());
        let mut places = HashMap::with_capacity(index.total());
        for (s, def) in catalog.systems().iter().enumerate() {
            let games = index.system(&def.folder).map_or(&[][..], |g| g.games());
            let mut ids = Vec::with_capacity(games.len());
            for (i, game) in games.iter().enumerate() {
                let id = free_id(&places, stable_id(&def.folder, game.path()));
                places.insert(id, (s, i));
                ids.push(GameId(id));
            }
            by_system.push(ids);
        }
        Self { by_system, places }
    }
}

/// `id`, or the next number after it that `places` does not hold. The
/// index holds far fewer than 2^64 games, so the loop ends.
fn free_id(places: &HashMap<u64, (usize, usize)>, mut id: u64) -> u64 {
    while places.contains_key(&id) {
        id = id.wrapping_add(1);
    }
    id
}

/// The 64-bit FNV-1a hash of `system`, a zero byte and `path`.
fn stable_id(system: &str, path: &str) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    system
        .bytes()
        .chain([0])
        .chain(path.bytes())
        .fold(OFFSET, |h, b| (h ^ u64::from(b)).wrapping_mul(PRIME))
}

/// The notice text of the changes of a scan, for example `3 games added,
/// 1 removed.`, or `None` when nothing changed.
pub fn changes_text(changes: &Changes) -> Option<String> {
    let parts = [
        (changes.added, "added"),
        (changes.removed, "removed"),
        (changes.changed, "changed"),
    ];
    let mut text = String::new();
    for (n, what) in parts.into_iter().filter(|(n, _)| *n > 0) {
        if text.is_empty() {
            let noun = if n == 1 { "game" } else { "games" };
            text = format!("{n} {noun} {what}");
        } else {
            text.push_str(&format!(", {n} {what}"));
        }
    }
    (!text.is_empty()).then(|| text + ".")
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
        Library::with_index(
            Catalog::builtin().expect("catalog"),
            Index::from_systems(vec![SystemGames::new("snes".to_owned(), games)]),
        )
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

    /// A temporary games folder with `files`, removed on drop.
    struct Roms(PathBuf);

    impl Roms {
        fn new(label: &str, files: &[&str]) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "neptron-app-library-{label}-{}",
                std::process::id()
            ));
            // A leftover of an earlier run with the same process id.
            let _ = std::fs::remove_dir_all(&dir);
            for file in files {
                let path = dir.join(file);
                std::fs::create_dir_all(path.parent().expect("a folder")).expect("folder");
                std::fs::write(path, b"rom").expect("file");
            }
            Self(dir)
        }
    }

    impl Drop for Roms {
        fn drop(&mut self) {
            // A folder left in the temporary directory does no harm.
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Polls until the scan ends. The scan thread ends by itself, so the
    /// loop ends; it yields and never sleeps.
    fn wait(lib: &mut Library) -> Result<Scanned, String> {
        loop {
            if let Some(result) = lib.poll() {
                return result;
            }
            std::thread::yield_now();
        }
    }

    #[test]
    fn an_id_stays_with_its_game_when_the_list_changes() {
        let lib = library();
        let alpha = lib.find(&GameRef {
            system: "snes".to_owned(),
            path: "snes/Alpha.sfc".to_owned(),
        });
        let games = vec![
            Game::new("Beta".to_owned(), "snes/Beta.sfc".to_owned()),
            Game::new("Alpha".to_owned(), "snes/Alpha.sfc".to_owned()),
            Game::new("Aardvark".to_owned(), "snes/Aardvark.sfc".to_owned()),
        ];
        let grown = Library::with_index(
            Catalog::builtin().expect("catalog"),
            Index::from_systems(vec![SystemGames::new("snes".to_owned(), games)]),
        );
        let moved = grown.find(&GameRef {
            system: "snes".to_owned(),
            path: "snes/Alpha.sfc".to_owned(),
        });
        assert_eq!(moved, alpha);
        let window = grown.window(GameSource::System(SystemId(1)), 0, 10);
        assert_eq!(window.rows[1].id, alpha.expect("an id"));
        assert_eq!(window.rows[1].title, "Alpha");
    }

    #[test]
    fn a_taken_id_gives_the_next_free_number() {
        let mut places = HashMap::new();
        places.insert(7, (0, 0));
        places.insert(8, (0, 1));
        places.insert(u64::MAX, (0, 2));
        assert_eq!(free_id(&places, 7), 9);
        assert_eq!(free_id(&places, 5), 5);
        assert_eq!(free_id(&places, u64::MAX), 0);
        assert_ne!(stable_id("snes", "a/b"), stable_id("snes/a", "b"));
    }

    #[test]
    fn the_notice_text_names_each_kind_of_change() {
        let c = |added, removed, changed| Changes {
            added,
            removed,
            changed,
        };
        assert_eq!(changes_text(&c(0, 0, 0)), None);
        assert_eq!(
            changes_text(&c(3, 1, 0)).as_deref(),
            Some("3 games added, 1 removed.")
        );
        assert_eq!(
            changes_text(&c(0, 1, 1)).as_deref(),
            Some("1 game removed, 1 changed.")
        );
    }

    #[test]
    fn a_rescan_replaces_the_index_and_a_second_waits() {
        let roms = Roms::new("rescan", &["snes/Alpha.sfc", "nes/New.nes"]);
        let mut lib = library();
        let now = SystemTime::now();
        assert_eq!(
            lib.start_scan(roms.0.clone(), ScanKind::Rescan, now),
            Ok(ScanStart::Started)
        );
        assert!(lib.scanning());
        assert_eq!(
            lib.start_scan(roms.0.clone(), ScanKind::Check, now),
            Ok(ScanStart::Running)
        );
        let scanned = wait(&mut lib).expect("the scan");
        assert_eq!(scanned.kind, ScanKind::Rescan);
        assert!(!scanned.first);
        // Alpha had size 0 and now 3 bytes; Beta is gone; New is new.
        assert_eq!(
            scanned.changes,
            Changes {
                added: 1,
                removed: 1,
                changed: 1
            }
        );
        assert!(!lib.scanning());
        assert_eq!(lib.systems().len(), 2);
        assert!(lib.poll().is_none());
    }

    #[test]
    fn a_failed_scan_keeps_the_index() {
        let roms = Roms::new("missing", &[]);
        let mut lib = library();
        lib.start_scan(roms.0.join("roms"), ScanKind::Rescan, SystemTime::now())
            .expect("the thread starts");
        let error = wait(&mut lib).expect_err("no ROM folder");
        assert!(error.contains("cannot read the ROM folder"), "{error}");
        assert_eq!(lib.index().total(), 2);
        assert!(!lib.scanning());
    }

    #[test]
    fn a_library_dropped_during_a_scan_stops_and_joins_it() {
        let roms = Roms::new("drop", &["nes/A.nes"]);
        let mut lib = library();
        lib.start_scan(roms.0.clone(), ScanKind::Boot, SystemTime::now())
            .expect("the thread starts");
        drop(lib);
    }

    #[test]
    fn a_cache_of_an_older_version_is_missing_with_no_notice() {
        let roms = Roms::new("old-cache", &[]);
        std::fs::create_dir_all(&roms.0).expect("folder");
        let path = roms.0.join("library.cache");
        let mut bytes = cache::encode(&library().index);
        bytes[8..12].copy_from_slice(&(cache::CACHE_VERSION - 1).to_le_bytes());
        std::fs::write(&path, &bytes).expect("old cache");
        let (lib, issue) = Library::load(Catalog::builtin().expect("catalog"), &path);
        assert_eq!(issue, None);
        assert_eq!(lib.index().total(), 0);
        assert!(!path.exists(), "the old file is gone");

        bytes[8..12].copy_from_slice(&(cache::CACHE_VERSION + 1).to_le_bytes());
        std::fs::write(&path, &bytes).expect("newer cache");
        let (_, issue) = Library::load(Catalog::builtin().expect("catalog"), &path);
        assert!(
            issue.is_some_and(|i| i.contains("version")),
            "a newer file warns"
        );
    }

    /// The work of the frame thread at the end of a scan of 10,000 games:
    /// the swap of the index and its ids, the drop of the old ones, and
    /// the shared index for the cache job. The comparison and the ids are
    /// made on the scan thread, outside the measure. The fastest of 5 runs
    /// must take at most `BUDGET`, a quarter of a frame at 60 Hz.
    #[test]
    fn ten_thousand_game_scan_end_fits_the_frame_budget() {
        const GAMES: usize = 10_000;
        const BUDGET: std::time::Duration = std::time::Duration::from_millis(4);
        let index = |extra: usize| {
            let games = (0..GAMES + extra)
                .map(|i| {
                    let name = format!("Game number {i:05} (USA)");
                    Game::new(name.clone(), format!("snes/{name}.sfc")).with_size(1 << 20)
                })
                .collect();
            Index::from_systems(vec![SystemGames::new("snes".to_owned(), games)])
        };
        let catalog = Catalog::builtin().expect("catalog");
        let mut fastest = std::time::Duration::MAX;
        for _ in 0..5 {
            let mut lib = Library::with_index(catalog.clone(), index(0));
            let next = index(1);
            let prepared = Prepared {
                changes: changes(&lib.index, &next),
                ids: GameIds::of(&catalog, &next),
                index: Arc::new(next),
            };
            let t = std::time::Instant::now();
            let scanned = lib.replace(prepared, ScanKind::Rescan);
            let job = lib.shared_index();
            let elapsed = t.elapsed();
            drop(job);
            assert_eq!(scanned.changes.added, 1);
            fastest = fastest.min(elapsed);
        }
        eprintln!("end of a 10,000-game scan on the frame thread: fastest {fastest:?}");
        assert!(fastest <= BUDGET, "{fastest:?}");
    }
}
