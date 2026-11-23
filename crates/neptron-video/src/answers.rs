use std::cell::{Cell, RefCell};

use neptron_modeline::policy::{ChoicesWork, PictureChoices, PolicyError, PolicySettings};
use neptron_types::SourceGeometry;

use crate::ScanlineSettings;

/// The answers that a page asks for: the picture answers of `sources` (one
/// source gives its own answers; several give one answer for every game)
/// with `settings` and `scanlines`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerRequest {
    /// The sources judged.
    pub sources: Vec<SourceGeometry>,
    /// The policy settings of the level judged.
    pub settings: PolicySettings,
    /// The scanline settings of the level judged.
    pub scanlines: ScanlineSettings,
}

/// The requests whose answers the engine keeps when none of them is in use:
/// Settings > Display, the quick menu, and the keep prompt with its two
/// levels. A request in work or in use is never dropped, so the cache can
/// hold more while they last.
const ANSWERS_KEPT: usize = 6;

/// One request: its key, its work while it runs, then its answers.
#[derive(Debug)]
struct Entry {
    generation: u64,
    request: AnswerRequest,
    work: Option<ChoicesWork>,
    done: Option<Result<PictureChoices, PolicyError>>,
    /// The last use, on the clock of the cache.
    used: u64,
}

/// The answers of the picture menus that the engine computed, keyed by the
/// request and by the generation of the inputs that the request does not
/// carry (the monitor profile, the caps, the veto and the driver
/// refusals): a change of any of those is a new generation, and the old
/// answers no longer match. The policy settings and the scanlines are in
/// the request, so a change of them is no new generation. The engine owns
/// it and uses it from the frame thread only, so a `RefCell` and no lock.
#[derive(Debug, Default)]
pub(crate) struct AnswerCache {
    entries: RefCell<Vec<Entry>>,
    /// Counts the calls of [`Self::step`]: an entry used since the last
    /// step is in use.
    clock: Cell<u64>,
}

impl AnswerCache {
    /// The answers of `request` at `generation`, or `None` while they are
    /// in work. A request with no entry starts one, made by
    /// [`Self::step`].
    pub(crate) fn get(
        &self,
        generation: u64,
        request: &AnswerRequest,
    ) -> Option<Result<PictureChoices, PolicyError>> {
        let now = self.clock.get();
        let mut entries = self.entries.borrow_mut();
        if let Some(entry) = entries
            .iter_mut()
            .find(|e| e.generation == generation && &e.request == request)
        {
            entry.used = now;
            return entry.done.clone();
        }
        entries.retain(|e| e.generation == generation);
        evict(&mut entries, now);
        entries.push(Entry {
            generation,
            request: request.clone(),
            work: Some(ChoicesWork::new(
                &request.sources,
                request.settings,
                request.scanlines,
            )),
            done: None,
            used: now,
        });
        None
    }

    /// Runs the work of one request with `run`, which makes at most its
    /// budget of decisions and returns whether the work is done. The first
    /// request of `waited` that is in work goes first, in the order of
    /// `waited` (the page on show, then the keep prompt); else the oldest
    /// request in work. The requests of `waited` count as in use. Returns
    /// whether a request got its answers in this call.
    pub(crate) fn step<'r>(
        &self,
        generation: u64,
        waited: impl IntoIterator<Item = &'r AnswerRequest> + Clone,
        run: impl FnOnce(&mut ChoicesWork) -> bool,
    ) -> bool {
        let now = self.clock.get().wrapping_add(1);
        self.clock.set(now);
        let mut entries = self.entries.borrow_mut();
        for entry in entries.iter_mut() {
            if entry.generation == generation
                && waited.clone().into_iter().any(|w| *w == entry.request)
            {
                entry.used = now;
            }
        }
        let first = waited.into_iter().find_map(|w| {
            entries
                .iter()
                .position(|e| e.generation == generation && e.request == *w && e.work.is_some())
        });
        let Some(index) = first.or_else(|| {
            entries
                .iter()
                .position(|e| e.generation == generation && e.work.is_some())
        }) else {
            return false;
        };
        let Some(entry) = entries.get_mut(index) else {
            return false;
        };
        let Some(work) = entry.work.as_mut() else {
            return false;
        };
        if !run(work) {
            return false;
        }
        entry.done = entry.work.take().map(ChoicesWork::finish);
        true
    }

    /// Whether a request is in work.
    pub(crate) fn working(&self) -> bool {
        self.entries.borrow().iter().any(|e| e.work.is_some())
    }

    /// The number of requests the cache holds.
    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.borrow().len()
    }
}

/// Makes room for one more entry when the cache holds [`ANSWERS_KEPT`]: it
/// drops the done entry that was used least recently, but never one in work
/// and never one used since the last step (`now`). With no such entry the
/// cache grows: the entries in work end, and those in use are few.
fn evict(entries: &mut Vec<Entry>, now: u64) {
    if entries.len() < ANSWERS_KEPT {
        return;
    }
    let oldest = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.work.is_none() && e.used != now)
        .min_by_key(|(_, e)| e.used)
        .map(|(i, _)| i);
    if let Some(i) = oldest {
        entries.remove(i);
    }
}

#[cfg(test)]
mod tests {
    use neptron_modeline::policy::{PictureStyle, RefreshPolicy};
    use neptron_types::Rotation;

    use super::*;

    fn request(height: u16) -> AnswerRequest {
        AnswerRequest {
            sources: vec![SourceGeometry {
                width: 256,
                height,
                refresh_mhz: 60_000,
                aspect_milli: 1_333,
                interlaced: false,
                rotation: Rotation::R0,
            }],
            settings: PolicySettings::default(),
            scanlines: ScanlineSettings::OFF,
        }
    }

    /// A work that ends at its first step.
    fn done(_: &mut ChoicesWork) -> bool {
        true
    }

    #[test]
    fn the_waited_request_goes_first_and_the_others_after() {
        let cache = AnswerCache::default();
        let (a, b) = (request(224), request(240));
        assert!(cache.get(0, &a).is_none());
        assert!(cache.get(0, &b).is_none());
        assert!(cache.step(0, [&b], done));
        assert!(cache.get(0, &b).is_some(), "the waited request first");
        assert!(cache.get(0, &a).is_none());
        assert!(cache.step(0, [], done));
        assert!(cache.get(0, &a).is_some(), "then the oldest in work");
        assert!(!cache.working());
        assert!(!cache.step(0, [], done));
    }

    #[test]
    fn no_entry_in_work_or_in_use_is_dropped() {
        let cache = AnswerCache::default();
        let page = request(100);
        assert!(cache.get(0, &page).is_none());
        assert!(cache.step(0, [&page], done));
        // More requests than the cache keeps, all in work.
        let others: Vec<AnswerRequest> = (0..8).map(|i| request(200 + i)).collect();
        for other in &others {
            assert!(cache.get(0, other).is_none());
        }
        assert_eq!(cache.len(), 9, "the entries in work stay, and the page");
        assert!(cache.get(0, &page).is_some(), "the page in use stays");
        // Once done and no longer used, the oldest go first.
        while cache.step(0, [&page], done) {}
        assert!(cache.get(0, &request(300)).is_none());
        assert_eq!(cache.len(), 9, "one old entry made room");
        assert!(cache.get(0, &page).is_some(), "the page stays");
        assert!(cache.get(0, &others[0]).is_none(), "the oldest went");
    }

    #[test]
    fn a_new_generation_drops_the_old_answers() {
        let cache = AnswerCache::default();
        let a = request(224);
        assert!(cache.get(0, &a).is_none());
        assert!(cache.step(0, [&a], done));
        assert!(cache.get(0, &a).is_some());
        assert!(cache.get(1, &a).is_none(), "a new generation is new work");
        assert_eq!(cache.len(), 1);
        let settings = PolicySettings {
            style: PictureStyle::X2,
            refresh: RefreshPolicy::Force50,
            ..PolicySettings::default()
        };
        let b = AnswerRequest { settings, ..a };
        assert!(
            cache.get(1, &b).is_none(),
            "other settings are another request"
        );
        assert_eq!(cache.len(), 2);
    }
}
