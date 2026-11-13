//! What changed between two indexes, for the notice after a scan.

use std::collections::HashMap;

use crate::index::{Game, Index};

/// The games that a scan added, removed or changed, compared with the
/// index before it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Changes {
    /// Paths that are new.
    pub added: u32,
    /// Paths that are gone.
    pub removed: u32,
    /// Paths in both indexes with another size.
    pub changed: u32,
}

/// The changes from `old` to `new`, system by system.
///
/// A game is known by its path. A path in both indexes with another size is
/// changed. A renamed file counts as one removed and one added: many ROMs
/// share a power-of-two size, so equal sizes do not tell a rename from two
/// different games. The work is linear in the number of games.
pub fn changes(old: &Index, new: &Index) -> Changes {
    let mut out = Changes::default();
    let mut ids: Vec<&str> = old
        .systems()
        .iter()
        .chain(new.systems())
        .map(|s| s.system())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        let before = old.system(id).map_or(&[][..], |s| s.games());
        let after = new.system(id).map_or(&[][..], |s| s.games());
        system_changes(before, after, &mut out);
    }
    out
}

impl Changes {
    /// True when nothing changed.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

fn system_changes(before: &[Game], after: &[Game], out: &mut Changes) {
    let mut gone: HashMap<&str, u64> = before.iter().map(|g| (g.path(), g.size())).collect();
    for game in after {
        match gone.remove(game.path()) {
            Some(size) if size != game.size() => bump(&mut out.changed),
            Some(_) => {}
            None => bump(&mut out.added),
        }
    }
    for _ in gone {
        bump(&mut out.removed);
    }
}

fn bump(n: &mut u32) {
    *n = n.saturating_add(1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::SystemGames;

    fn index(systems: &[(&str, &[(&str, u64)])]) -> Index {
        Index::from_systems(
            systems
                .iter()
                .map(|(id, games)| {
                    let games = games
                        .iter()
                        .map(|(path, size)| {
                            let name = path.rsplit('/').next().unwrap_or(path).to_owned();
                            Game::new(name, (*path).to_owned()).with_size(*size)
                        })
                        .collect();
                    SystemGames::new((*id).to_owned(), games)
                })
                .collect(),
        )
    }

    #[test]
    fn equal_indexes_have_no_changes() {
        let a = index(&[("nes", &[("nes/a.nes", 10), ("nes/b.nes", 20)])]);
        assert!(changes(&a, &a.clone()).is_empty());
        assert!(changes(&Index::new(), &Index::new()).is_empty());
    }

    #[test]
    fn added_and_removed_games_are_counted_per_path() {
        let old = index(&[("nes", &[("nes/a.nes", 10), ("nes/b.nes", 20)])]);
        let new = index(&[
            (
                "nes",
                &[("nes/a.nes", 10), ("nes/c.nes", 30), ("nes/d.nes", 40)],
            ),
            ("snes", &[("snes/e.sfc", 50)]),
        ]);
        let c = changes(&old, &new);
        assert_eq!(
            c,
            Changes {
                added: 3,
                removed: 1,
                changed: 0
            }
        );
        let back = changes(&new, &old);
        assert_eq!((back.added, back.removed), (1, 3));
    }

    #[test]
    fn a_gone_and_a_new_path_of_one_size_are_a_remove_and_an_add() {
        // Mario out, Zelda in: two games of 1 MiB, not a rename.
        let old = index(&[("snes", &[("snes/Mario.sfc", 1 << 20)])]);
        let new = index(&[("snes", &[("snes/Zelda.sfc", 1 << 20)])]);
        assert_eq!(
            changes(&old, &new),
            Changes {
                added: 1,
                removed: 1,
                ..Changes::default()
            }
        );
    }

    #[test]
    fn a_path_with_another_size_is_changed() {
        let old = index(&[("nes", &[("nes/a.nes", 10), ("nes/b.nes", 20)])]);
        let new = index(&[("nes", &[("nes/a.nes", 11), ("nes/b.nes", 20)])]);
        assert_eq!(
            changes(&old, &new),
            Changes {
                changed: 1,
                ..Changes::default()
            }
        );
    }
}
