//! What the control protocol reads from and asks of a run.

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::PadRoster;
use neptron_ipc::Stats;
use neptron_settings::{DISPLAY_ORIENTATION, DISPLAY_OUTPUT, Level};
use neptron_types::{Control, InputEvent};

use super::{Shell, Stage};
use crate::control::value_of_json;
use crate::pages::{self, Gate};
use crate::relaunch::GameRef;

impl<D: Display, S: AudioSink, I: InputSource + PadRoster, C: Clock + Clone> Shell<D, S, I, C> {
    /// The game of the run and its core id.
    pub fn game_reference(&self) -> Option<(&GameRef, &str)> {
        match &self.stage {
            Stage::Game(game) => Some((&game.reference, game.core.as_str())),
            _ => None,
        }
    }

    /// The schema key of the settings row with this label.
    pub fn setting_key_of(&self, label: &str) -> Option<&'static str> {
        self.config
            .schema()
            .entries()
            .iter()
            .find(|e| e.label == label)
            .map(|e| e.key)
    }

    /// The counters of the run for `stats`.
    pub fn stats_now(&self) -> Stats {
        let mut stats = Stats {
            frames_presented: 0,
            frames_dropped: 0,
            frames_repeated: 0,
            audio_underruns: 0,
            audio_queued_frames: 0,
            input_to_frame_ns: 0,
            fields_missed: Some(0),
            late_frames: Some(0),
            late_flips: Some(0),
        };
        if let Stage::Game(game) = &self.stage {
            let counters = game.session.counters();
            stats.frames_presented = counters.flips;
            stats.frames_repeated = counters.repeated_pictures;
            stats.frames_dropped = counters.flipless_frames;
            stats.fields_missed = Some(counters.missed_fields);
            stats.input_to_frame_ns = counters.input_to_flip_ns;
            stats.late_frames = Some(counters.late_frames);
            stats.late_flips = Some(counters.late_flips);
            if let Some(sink) = game.session.audio_sink() {
                stats.audio_queued_frames = sink.queued_frames() as u64;
            }
            if let Some(audio) = game.session.audio_stats() {
                stats.audio_underruns = audio.underruns;
            }
        }
        stats
    }

    /// The last `lines` lines of the log, the oldest first.
    pub fn log_lines(&self, lines: usize) -> Vec<String> {
        let skip = self.log.len().saturating_sub(lines);
        self.log.iter().skip(skip).cloned().collect()
    }

    /// Queues input events of the protocol: each `(frame, port, control,
    /// value)` reaches the input at the next frame plus its frame. Port N
    /// is device N, a direct id. Returns the frame of the first event.
    pub fn queue_input(&mut self, events: &[(u64, u8, Control, i16)]) -> u64 {
        let start = self.frame.get() + 1;
        let mut queue = self.control_queue.borrow_mut();
        let mut sorted: Vec<_> = events.to_vec();
        sorted.sort_by_key(|e| e.0);
        for (frame, port, control, value) in sorted {
            queue.push_back((
                start.saturating_add(frame),
                InputEvent {
                    device: u32::from(port),
                    control,
                    value,
                },
            ));
        }
        start
    }

    /// Launches a game of the ROM folder from the protocol: `rom` is its
    /// path under the ROM folder, and `core` must be the core id of its
    /// system. The run ends with a relaunch into the game, as from the
    /// menu.
    ///
    /// # Errors
    ///
    /// The reason as text.
    pub fn launch_path(&mut self, core: &str, rom: &str) -> Result<(), String> {
        if !matches!(self.stage, Stage::List(_)) {
            return Err("a game runs; quit it first".to_owned());
        }
        let system = rom
            .split('/')
            .next()
            .and_then(|folder| self.library.catalog().system_for_folder(folder))
            .map(|i| self.library.catalog().systems()[i].folder.clone())
            .ok_or_else(|| format!("{rom} is not in the folder of a system"))?;
        let (_, def) = self
            .library
            .resolve(&system)
            .ok_or_else(|| format!("the system {system} has no core"))?;
        if def.id != core {
            return Err(format!("the core of {system} is {}, not {core}", def.id));
        }
        let reference = GameRef {
            system,
            path: rom.to_owned(),
        };
        let id = self
            .library
            .find(&reference)
            .ok_or_else(|| format!("{rom} is not in the game list"))?;
        self.launch(id);
        Ok(())
    }

    /// Sets a setting from the protocol. A display setting goes through the
    /// guard, as a menu choice does, and a picture mode or refresh choice
    /// passes the rule that greys it in the menu of that level.
    ///
    /// # Errors
    ///
    /// The reason as text.
    pub fn set_from_protocol(
        &mut self,
        key: &str,
        level: Level,
        json: &serde_json::Value,
    ) -> Result<(), String> {
        let entry = *self
            .config
            .schema()
            .get(key)
            .ok_or_else(|| format!("no setting {key:?}"))?;
        if entry.key == DISPLAY_OUTPUT {
            return Err(format!(
                "{key} changes at the next start, in the settings file; the guard does not cover it"
            ));
        }
        let value = value_of_json(json).ok_or_else(|| format!("{json} is not a setting value"))?;
        entry.kind.check(&value).map_err(|e| e.to_string())?;
        if !entry.levels.allows(level) {
            return Err(format!("{key} cannot be set at the {} level", level.name()));
        }
        if entry.key == DISPLAY_ORIENTATION && self.in_game() {
            return Err(format!("{key} cannot change during a game"));
        }
        // The rule of the menu of that level: a picture mode or refresh
        // choice that the display does not give is refused. The protocol
        // sets one key: a value that needs the other row at Auto is refused
        // too, and says so.
        let gate = self.gate_now(entry.key, level, &value);
        match &gate {
            Gate::Set { .. } => {}
            Gate::SetWithReset { other, .. } => {
                return Err(format!(
                    "{key} = {value} is only possible with {other} = {}: set it first",
                    pages::other_row(entry.key).map_or("auto", |(_, auto)| auto)
                ));
            }
            Gate::Refused(reason) => {
                return Err(format!(
                    "{key} = {value} is refused: {}",
                    pages::lower_first(reason)
                ));
            }
        }
        // The time of the last frame: every guard call gets a frame time.
        let now_ns = self.now_ns;
        if pages::other_row(entry.key).is_some() {
            self.store_pick(entry.key, level, value, gate, now_ns);
        } else {
            self.change_setting(entry.key, level, Some(value), now_ns);
        }
        Ok(())
    }
}
