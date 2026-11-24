use std::cell::{Cell, RefCell};

use neptron_modeline::policy::{ChoicesWork, PictureChoices, PolicyError, PolicySettings};
use neptron_types::{Orientation, SourceGeometry};

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
    /// The orientation whose display sources the work judges.
    orientation: Orientation,
    request: AnswerRequest,
    work: Option<ChoicesWork>,
    done: Option<Result<PictureChoices, PolicyError>>,
    /// The last use, on the clock of the cache.
    used: u64,
}

/// The answers of the picture menus that the engine computed, keyed by the
/// request, by the orientation, and by the generation of the inputs that
/// the request does not carry (the monitor profile, the caps, the veto,
/// the driver refusals and the orientation): a change of any of those is a
/// new generation, and the old answers no longer match. The work judges
/// the display sources of the request's sources in the orientation of
/// the entry. The policy settings and the scanlines are in
/// the request, so a change of them is no new generation. The engine owns
/// it and uses it from the frame thread only, so a `RefCell` and no lock.
#[derive(Debug, Default)]
pub(crate) struct AnswerCache {
    entries: RefCell<Vec<Entry>>,
    /// Counts the calls of [`Self::step`]: an entry used since the last
    /// step is in use.
    clock: Cell<u64>,
    /// Counts the steps that ran work: a measure for the tests that no
    /// work runs while no one waits.
    runs: Cell<u64>,
}

impl AnswerCache {
    /// The answers of `request` at `generation`, or `None` while they are
    /// in work. A request with no entry starts one, made by
    /// [`Self::step`].
    pub(crate) fn get(
        &self,
        generation: u64,
        orientation: Orientation,
        request: &AnswerRequest,
    ) -> Option<Result<PictureChoices, PolicyError>> {
        let now = self.clock.get();
        let mut entries = self.entries.borrow_mut();
        if let Some(entry) = entries
            .iter_mut()
            .find(|e| e.matches(generation, orientation, request))
        {
            entry.used = now;
            return entry.done.clone();
        }
        entries.retain(|e| e.generation == generation);
        evict(&mut entries, now);
        let shown: Vec<SourceGeometry> = request
            .sources
            .iter()
            .map(|s| orientation.display_source(s))
            .collect();
        entries.push(Entry {
            generation,
            orientation,
            request: request.clone(),
            work: Some(ChoicesWork::new(
                &shown,
                request.settings,
                request.scanlines,
            )),
            done: None,
            used: now,
        });
        None
    }

    /// Runs the work of one request with `run`, which makes at most its
    /// budget of decisions and returns whether the work is done: the first
    /// request of `waited` that is in work, in the order of `waited` (the
    /// keep prompt while it shows, else the page on show). A request that
    /// no one waits for gets no work. The requests of `waited` count as in
    /// use. Returns whether a request got its answers in this call.
    pub(crate) fn step<'r>(
        &self,
        generation: u64,
        orientation: Orientation,
        waited: impl IntoIterator<Item = &'r AnswerRequest> + Clone,
        run: impl FnOnce(&mut ChoicesWork) -> bool,
    ) -> bool {
        let now = self.clock.get().wrapping_add(1);
        self.clock.set(now);
        let mut entries = self.entries.borrow_mut();
        for entry in entries.iter_mut() {
            if waited
                .clone()
                .into_iter()
                .any(|w| entry.matches(generation, orientation, w))
            {
                entry.used = now;
            }
        }
        let first = waited.into_iter().find_map(|w| {
            entries
                .iter()
                .position(|e| e.matches(generation, orientation, w) && e.work.is_some())
        });
        let Some(index) = first else {
            return false;
        };
        let Some(entry) = entries.get_mut(index) else {
            return false;
        };
        let Some(work) = entry.work.as_mut() else {
            return false;
        };
        self.runs.set(self.runs.get().wrapping_add(1));
        if !run(work) {
            return false;
        }
        entry.done = entry.work.take().map(ChoicesWork::finish);
        true
    }

    /// The steps that ran work so far.
    pub(crate) fn runs(&self) -> u64 {
        self.runs.get()
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

impl Entry {
    fn matches(&self, generation: u64, orientation: Orientation, request: &AnswerRequest) -> bool {
        self.generation == generation && self.orientation == orientation && &self.request == request
    }
}

/// Makes room for one more entry when the cache holds [`ANSWERS_KEPT`]: it
/// drops the entry that was used least recently, done or in work, but never
/// one used since the last step (`now`): the page on show and the keep
/// prompt keep theirs. With no such entry the cache grows: those in use are
/// few.
fn evict(entries: &mut Vec<Entry>, now: u64) {
    if entries.len() < ANSWERS_KEPT {
        return;
    }
    let oldest = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.used != now)
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

    const NORMAL: Orientation = Orientation::NORMAL;

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
    fn only_a_waited_request_gets_work() {
        let cache = AnswerCache::default();
        let (a, b) = (request(224), request(240));
        assert!(cache.get(0, NORMAL, &a).is_none());
        assert!(cache.get(0, NORMAL, &b).is_none());
        assert!(!cache.step(0, NORMAL, [], done), "no one waits: no work");
        assert_eq!(cache.runs(), 0);
        assert!(cache.step(0, NORMAL, [&b, &a], done));
        assert!(
            cache.get(0, NORMAL, &b).is_some(),
            "the first waited request first"
        );
        assert!(cache.get(0, NORMAL, &a).is_none());
        assert!(
            !cache.step(0, NORMAL, [&b], done),
            "a is left: no one waits for it"
        );
        assert!(cache.working());
        assert!(cache.step(0, NORMAL, [&a], done));
        assert!(!cache.working());
        assert_eq!(cache.runs(), 2);
    }

    #[test]
    fn an_entry_in_use_is_never_dropped_and_an_unwaited_one_can_go() {
        let cache = AnswerCache::default();
        let page = request(100);
        assert!(cache.get(0, NORMAL, &page).is_none());
        assert!(cache.step(0, NORMAL, [&page], done));
        // More requests than the cache keeps, all in work and waited by no
        // one: the oldest go first, the page in use stays.
        let others: Vec<AnswerRequest> = (0..8).map(|i| request(200 + i)).collect();
        for other in &others {
            // One frame each: the page is still on show.
            assert!(!cache.step(0, NORMAL, [&page], done));
            assert!(cache.get(0, NORMAL, other).is_none());
        }
        assert_eq!(cache.len(), ANSWERS_KEPT);
        assert!(
            cache.get(0, NORMAL, &page).is_some(),
            "the page in use stays"
        );
        assert!(
            cache.get(0, NORMAL, &others[0]).is_none(),
            "the oldest went"
        );
        // Requests in use are kept also when the cache is full of them: it
        // grows by the new one.
        let held: Vec<AnswerRequest> = (0..ANSWERS_KEPT as u16).map(|i| request(400 + i)).collect();
        let fresh = AnswerCache::default();
        for r in &held {
            assert!(fresh.get(0, NORMAL, r).is_none());
        }
        assert!(
            !fresh.step(0, NORMAL, held.iter(), |_| false),
            "the work goes on"
        );
        assert!(fresh.get(0, NORMAL, &request(500)).is_none());
        assert_eq!(fresh.len(), ANSWERS_KEPT + 1);
    }

    #[test]
    fn a_new_generation_drops_the_old_answers() {
        let cache = AnswerCache::default();
        let a = request(224);
        assert!(cache.get(0, NORMAL, &a).is_none());
        assert!(cache.step(0, NORMAL, [&a], done));
        assert!(cache.get(0, NORMAL, &a).is_some());
        assert!(
            cache.get(1, NORMAL, &a).is_none(),
            "a new generation is new work"
        );
        assert_eq!(cache.len(), 1);
        let settings = PolicySettings {
            style: PictureStyle::X2,
            refresh: RefreshPolicy::Force50,
            ..PolicySettings::default()
        };
        let b = AnswerRequest { settings, ..a };
        assert!(
            cache.get(1, NORMAL, &b).is_none(),
            "other settings are another request"
        );
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn another_orientation_is_another_entry() {
        let cache = AnswerCache::default();
        let a = request(224);
        let turned = Orientation {
            screen: Rotation::R90,
            game_override: None,
        };
        assert!(cache.get(0, NORMAL, &a).is_none());
        assert!(cache.step(0, NORMAL, [&a], done));
        assert!(cache.get(0, NORMAL, &a).is_some());
        assert!(
            cache.get(0, turned, &a).is_none(),
            "the answers of another orientation are new work"
        );
        assert!(
            !cache.step(0, NORMAL, [&a], done),
            "the turned work is not waited"
        );
        assert!(cache.step(0, turned, [&a], done));
        assert!(cache.get(0, turned, &a).is_some());
    }
}
