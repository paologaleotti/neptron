use neptron_types::SourceGeometry;

use crate::decision::Decision;

/// How many sources keep their decision. A core that moves between a few
/// sizes (Mega Drive 256 and 320, SNES 256 and 512) finds each in the cache
/// after its first frame.
pub const CACHE_ENTRIES: usize = 8;

/// The decisions of the last [`CACHE_ENTRIES`] sources. The monitor, the
/// settings and the caps are the same for every entry: the engine clears
/// the cache when one of them changes.
pub(crate) struct Cache {
    entries: Vec<(SourceGeometry, Decision)>,
    /// The entry the next new source replaces once the cache is full.
    next: usize,
}

impl Cache {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::with_capacity(CACHE_ENTRIES),
            next: 0,
        }
    }

    /// The index of the decision for `source`; `make` runs only when the
    /// cache does not hold it.
    pub(crate) fn find_or_insert(
        &mut self,
        source: SourceGeometry,
        make: impl FnOnce() -> Decision,
    ) -> usize {
        if let Some(i) = self.entries.iter().position(|(s, _)| *s == source) {
            return i;
        }
        let entry = (source, make());
        if self.entries.len() < CACHE_ENTRIES {
            self.entries.push(entry);
            self.entries.len() - 1
        } else {
            let i = self.next;
            self.entries[i] = entry;
            self.next = (i + 1) % CACHE_ENTRIES;
            i
        }
    }

    /// The decision at `index`, from [`Cache::find_or_insert`].
    pub(crate) fn get(&self, index: usize) -> Option<&Decision> {
        self.entries.get(index).map(|(_, d)| d)
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.next = 0;
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use neptron_types::Rotation;

    use super::*;
    use crate::Unshowable;

    fn source(width: u16) -> SourceGeometry {
        SourceGeometry {
            width,
            height: 224,
            refresh_mhz: 60_000,
            aspect_milli: 1_333,
            interlaced: false,
            rotation: Rotation::R0,
        }
    }

    fn decision() -> Decision {
        Decision {
            plan: Err(Unshowable::NoWinner),
            winner: None,
            refused: Vec::new(),
        }
    }

    #[test]
    fn a_cached_source_is_not_decided_again() {
        let mut cache = Cache::new();
        let first = cache.find_or_insert(source(256), decision);
        let again = cache.find_or_insert(source(256), || unreachable!("cached"));
        assert_eq!(first, again);
        assert!(cache.get(first).is_some());
    }

    #[test]
    fn a_full_cache_replaces_the_oldest_entry() {
        let mut cache = Cache::new();
        for w in 0..CACHE_ENTRIES as u16 {
            cache.find_or_insert(source(w + 1), decision);
        }
        assert_eq!(cache.len(), CACHE_ENTRIES);
        let replaced = cache.find_or_insert(source(1_000), decision);
        assert_eq!(replaced, 0);
        assert_eq!(cache.len(), CACHE_ENTRIES);
        let mut made = false;
        cache.find_or_insert(source(1), || {
            made = true;
            decision()
        });
        assert!(made, "the oldest source left the cache");
    }

    #[test]
    fn clear_empties_the_cache() {
        let mut cache = Cache::new();
        cache.find_or_insert(source(256), decision);
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.get(0).is_none());
    }
}
