use std::cell::RefCell;

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

/// The most requests whose answers the engine keeps: Settings > Display,
/// the quick menu, and a keep at another level.
const ANSWERS_KEPT: usize = 4;

/// One request: its key, its work while it runs, then its answers.
#[derive(Debug)]
struct Entry {
    generation: u64,
    request: AnswerRequest,
    work: Option<ChoicesWork>,
    done: Option<Result<PictureChoices, PolicyError>>,
}

/// The answers of the picture menus that the engine computed, keyed by the
/// request and by the generation of the engine's inputs (the monitor
/// profile, the caps, the veto and the driver refusals): a change of any
/// of those is a new generation, and the old answers no longer match. The
/// engine owns it and uses it from the frame thread only, so a `RefCell`
/// and no lock.
#[derive(Debug, Default)]
pub(crate) struct AnswerCache {
    entries: RefCell<Vec<Entry>>,
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
        let mut entries = self.entries.borrow_mut();
        if let Some(entry) = entries
            .iter()
            .find(|e| e.generation == generation && &e.request == request)
        {
            return entry.done.clone();
        }
        entries.retain(|e| e.generation == generation);
        if entries.len() == ANSWERS_KEPT {
            entries.remove(0);
        }
        entries.push(Entry {
            generation,
            request: request.clone(),
            work: Some(ChoicesWork::new(
                &request.sources,
                request.settings,
                request.scanlines,
            )),
            done: None,
        });
        None
    }

    /// Runs the work of the oldest request in work with `run`, which makes
    /// at most its budget of decisions and returns whether the work is
    /// done. Returns whether a request got its answers in this call.
    pub(crate) fn step(&self, run: impl FnOnce(&mut ChoicesWork) -> bool) -> bool {
        let mut entries = self.entries.borrow_mut();
        let Some(entry) = entries.iter_mut().find(|e| e.work.is_some()) else {
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
}
