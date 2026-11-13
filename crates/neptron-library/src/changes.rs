//! What changed between two indexes, for the notice after a scan.

use std::collections::HashMap;

use crate::index::{Game, Index};

/// The games that a scan added, removed, renamed or changed, compared with
/// the index before it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Changes {
    /// Paths that are new, less the renames.
    pub added: u32,
    /// Paths that are gone, less the renames.
    pub removed: u32,
    /// A gone path and a new path of one system with the same size, not 0.
    pub renamed: u32,
    /// Paths in both indexes with another size.
    pub changed: u32,
}

/// The changes from `old` to `new`, system by system.
///
/// A game is known by its path. A path in both indexes with another size is
/// changed. A gone path and a new path of one system whose sizes are equal
/// and not 0 count as one rename, the first pairs first. An empty file
/// tells nothing, so it never pairs. The work is linear in the number of
/// games.
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
    let mut added_sizes = Vec::new();
    for game in after {
        match gone.remove(game.path()) {
            Some(size) if size != game.size() => bump(&mut out.changed),
            Some(_) => {}
            None => added_sizes.push(game.size()),
        }
    }
    let mut gone_sizes: HashMap<u64, u32> = HashMap::new();
    let mut removed = 0u32;
    for size in gone.into_values() {
        bump(&mut removed);
        if size > 0 {
            bump(gone_sizes.entry(size).or_default());
        }
    }
    let mut added = 0u32;
    for size in added_sizes {
        match gone_sizes.get_mut(&size).filter(|n| **n > 0) {
            Some(n) => {
                *n -= 1;
                removed = removed.saturating_sub(1);
                bump(&mut out.renamed);
            }
            None => bump(&mut added),
        }
    }
    out.added = out.added.saturating_add(added);
    out.removed = out.removed.saturating_add(removed);
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
                renamed: 0,
                changed: 0
            }
        );
        let back = changes(&new, &old);
        assert_eq!((back.added, back.removed), (1, 3));
    }

    #[test]
    fn a_gone_and_a_new_path_of_one_size_are_a_rename() {
        let old = index(&[("nes", &[("nes/a.nes", 10), ("nes/b.nes", 20)])]);
        let new = index(&[("nes", &[("nes/sub/a2.nes", 10), ("nes/b.nes", 20)])]);
        assert_eq!(
            changes(&old, &new),
            Changes {
                renamed: 1,
                ..Changes::default()
            }
        );
    }

    #[test]
    fn a_rename_needs_one_system_and_a_size() {
        let old = index(&[("nes", &[("nes/a.nes", 10), ("nes/z.nes", 0)])]);
        let new = index(&[
            ("snes", &[("snes/a.sfc", 10)]),
            ("nes", &[("nes/y.nes", 0)]),
        ]);
        assert_eq!(
            changes(&old, &new),
            Changes {
                added: 2,
                removed: 2,
                ..Changes::default()
            }
        );
    }

    #[test]
    fn each_gone_path_pairs_once() {
        let old = index(&[("nes", &[("nes/a.nes", 10)])]);
        let new = index(&[("nes", &[("nes/b.nes", 10), ("nes/c.nes", 10)])]);
        assert_eq!(
            changes(&old, &new),
            Changes {
                added: 1,
                renamed: 1,
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
