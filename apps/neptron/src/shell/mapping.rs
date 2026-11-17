//! The guided mapping of a controller in the game list: the capture of
//! the pad, the prompts, the trial of the new map, and the keep or the
//! cancel; and the reset of a map of the player.
//!
//! The mapping runs only in the game list: there the app owns the input
//! source. In a game the session owns it, and the page of a controller is
//! read only.

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::{
    Capture, GUIDED_STEPS, Guided, GuidedMapping, PadKey, PadRoster, PlayerMap, Prompt,
};
use neptron_ui::snapshot::{MappingId, MappingStage, NoticeKind, PadMapping};

use super::{Shell, Stage};
use crate::outbox::Job;
use crate::pad_page::{self, MAPPING_NAME_MAX_CHARS};
use crate::pages::fit_name;

/// How long the review of a new map waits for the answer of the player,
/// in nanoseconds. With no answer the old map comes back.
pub const MAP_REVIEW_TIMEOUT_NS: u64 = 15_000_000_000;

const NANOS_PER_SECOND: u64 = 1_000_000_000;

/// A guided mapping that runs.
#[derive(Debug)]
pub(super) struct MappingRun {
    id: u32,
    key: PadKey,
    name: String,
    stage: RunStage,
    /// What the snapshot shows, so a frame that changes nothing on screen
    /// builds no text.
    shown: Option<Shown>,
}

#[derive(Debug)]
enum RunStage {
    /// The source captures the pad, and the guided mapping reads it.
    Capture(Box<GuidedMapping>),
    /// The new map is on trial, or cannot be kept, until the player
    /// answers or the time runs out.
    Review {
        map: PlayerMap,
        can_keep: bool,
        deadline_ns: u64,
    },
}

/// The part of the snapshot of a mapping that changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shown {
    Prompt(Prompt, u64),
    Review(u64),
}

impl<D: Display, S: AudioSink, I: InputSource + PadRoster, C: Clock + Clone> Shell<D, S, I, C> {
    /// Starts the guided mapping of the pad `key`. It runs only in the
    /// game list, with the pad present, one at a time, and with a file of
    /// layouts that can be written; else it starts nothing and returns
    /// false.
    pub(super) fn start_mapping(&mut self, key: PadKey) -> bool {
        if self.mapping.is_some() || self.pad_maps.locked().is_some() {
            return false;
        }
        let roster = self.roster();
        let Some(entry) = roster
            .ports()
            .iter()
            .flatten()
            .find(|e| e.present && e.pad.key() == key)
        else {
            return false;
        };
        let Stage::List(list) = &mut self.stage else {
            return false;
        };
        list.input.capture(Some(key));
        self.next_mapping = self.next_mapping.wrapping_add(1);
        self.mapping = Some(MappingRun {
            id: self.next_mapping,
            key,
            name: fit_name(&entry.pad.name, MAPPING_NAME_MAX_CHARS),
            stage: RunStage::Capture(Box::default()),
            shown: None,
        });
        self.log(format!("guided mapping of {}", entry.pad.name));
        true
    }

    /// Follows the mapping for one frame, after the poll of the source:
    /// reads the captured pad, moves to the review, and ends the mapping
    /// when its time runs out or its pad goes away.
    pub(super) fn poll_mapping(&mut self, now_ns: u64) {
        let Some(run) = &mut self.mapping else {
            return;
        };
        let Stage::List(list) = &mut self.stage else {
            return;
        };
        let next = match &mut run.stage {
            RunStage::Capture(guided) => match list.input.captured() {
                Capture::Waiting => return,
                Capture::Gone => Next::Gone,
                Capture::Pad(state) => match guided.update(&state, now_ns) {
                    Guided::Prompt(prompt) => {
                        // Whole seconds, so a frame that only counts down
                        // within a second changes nothing on screen.
                        let seconds = prompt.remaining_ns.div_ceil(NANOS_PER_SECOND);
                        Next::Show(Shown::Prompt(
                            Prompt {
                                remaining_ns: seconds.saturating_mul(NANOS_PER_SECOND),
                                holding: false,
                                ..prompt
                            },
                            seconds,
                        ))
                    }
                    Guided::Done(map) => Next::Review(map),
                    Guided::TimedOut => Next::TimedOut,
                },
            },
            RunStage::Review { deadline_ns, .. } => {
                let left = deadline_ns.saturating_sub(now_ns);
                if left == 0 {
                    Next::TimedOut
                } else {
                    Next::Show(Shown::Review(left.div_ceil(NANOS_PER_SECOND)))
                }
            }
        };
        match next {
            Next::Show(shown) => self.show_mapping(shown, now_ns),
            Next::Review(map) => self.review_mapping(map, now_ns),
            Next::Gone => {
                self.end_mapping();
                self.notice(
                    NoticeKind::Info,
                    "The controller went away. Its layout did not change.".to_owned(),
                );
            }
            Next::TimedOut => {
                self.log("the guided mapping timed out; nothing changed".to_owned());
                self.end_mapping();
            }
        }
    }

    /// Keeps the map of the review: the file gets it, and the source keeps
    /// it for every pad of the model.
    pub(super) fn keep_mapping(&mut self) {
        let Some(run) = &self.mapping else {
            return;
        };
        let RunStage::Review {
            map,
            can_keep: true,
            ..
        } = run.stage
        else {
            self.end_mapping();
            return;
        };
        let (identity, name) = (run.key.identity, run.name.clone());
        match self.pad_maps.store(identity, &name, &map) {
            Ok(text) => {
                self.outbox.send(Job::PadMaps { text });
                self.log(format!("the layout of {name} is kept"));
            }
            Err(e) => self.notice(NoticeKind::Warning, format!("The layout is not kept: {e}.")),
        }
        self.end_mapping();
    }

    /// Ends the mapping with no change: the capture ends, and the maps of
    /// the file come back.
    pub(super) fn end_mapping(&mut self) {
        let Some(run) = self.mapping.take() else {
            return;
        };
        self.snap.pad_mapping = None;
        if let Stage::List(list) = &mut self.stage {
            match run.stage {
                RunStage::Capture(_) => list.input.capture(None),
                RunStage::Review { .. } => list.input.set_player_maps(self.pad_maps.maps()),
            }
        }
        self.reload_page();
    }

    /// Removes the map of the player of the pad on `port`, in the game
    /// list.
    pub(super) fn reset_layout(&mut self, port: u8) {
        let roster = self.roster();
        let Some(entry) = roster.get(port).filter(|e| e.present) else {
            return;
        };
        let Stage::List(list) = &mut self.stage else {
            return;
        };
        match self.pad_maps.remove(entry.pad.identity) {
            Ok(text) => {
                list.input.set_player_maps(self.pad_maps.maps());
                self.outbox.send(Job::PadMaps { text });
                self.log(format!("the layout of {} is reset", entry.pad.name));
            }
            Err(e) => self.notice(
                NoticeKind::Warning,
                format!("The layout is not reset: {e}."),
            ),
        }
        self.reload_page();
    }

    /// Ends the capture and puts `map` on trial: the source reads the pad
    /// with it, so the player tests it on the review. A map that cannot
    /// drive the menus is not given to the pad.
    fn review_mapping(&mut self, map: PlayerMap, now_ns: u64) {
        let (message, can_keep) = pad_page::review_text(&map);
        let Some(run) = &mut self.mapping else {
            return;
        };
        let Stage::List(list) = &mut self.stage else {
            return;
        };
        list.input.capture(None);
        if can_keep {
            let mut trial = self.pad_maps.maps().clone();
            trial.insert(run.key.identity, map);
            list.input.set_player_maps(&trial);
        }
        run.stage = RunStage::Review {
            map,
            can_keep,
            deadline_ns: now_ns.saturating_add(MAP_REVIEW_TIMEOUT_NS),
        };
        let seconds = MAP_REVIEW_TIMEOUT_NS / NANOS_PER_SECOND;
        run.shown = Some(Shown::Review(seconds));
        self.snap.pad_mapping = Some(PadMapping::new(
            MappingId(run.id),
            run.name.clone(),
            MappingStage::Review { message, can_keep },
            MAP_REVIEW_TIMEOUT_NS,
        ));
    }

    /// Puts the view of the mapping in the snapshot when it changed.
    fn show_mapping(&mut self, shown: Shown, now_ns: u64) {
        let Some(run) = &mut self.mapping else {
            return;
        };
        if run.shown == Some(shown) && self.snap.pad_mapping.is_some() {
            return;
        }
        run.shown = Some(shown);
        match (shown, &mut self.snap.pad_mapping) {
            (Shown::Review(_), Some(mapping)) => {
                if let RunStage::Review { deadline_ns, .. } = run.stage {
                    mapping.remaining_ns = deadline_ns.saturating_sub(now_ns);
                }
            }
            (Shown::Prompt(prompt, _), _) => {
                let steps = u32::try_from(GUIDED_STEPS.len()).unwrap_or(u32::MAX);
                let step = u32::try_from(prompt.step + 1).unwrap_or(u32::MAX);
                let note = pad_page::prompt_note(&prompt);
                self.snap.pad_mapping = Some(PadMapping::new(
                    MappingId(run.id),
                    run.name.clone(),
                    MappingStage::Prompt {
                        prompt: pad_page::prompt_text(prompt.control).to_owned(),
                        step,
                        steps,
                        note,
                    },
                    prompt.remaining_ns,
                ));
            }
            (Shown::Review(_), None) => {}
        }
    }
}

/// What a frame of the mapping does.
enum Next {
    Show(Shown),
    Review(PlayerMap),
    Gone,
    TimedOut,
}
