//! The start of a run, before the first frame: settings, presets, the
//! detected output, the library, the guard and the stage.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::path::Path;
use std::rc::Rc;
use std::time::SystemTime;

use neptron_backend::{AudioSink, Clock, ContextOffer, Display, InputSource};
use neptron_input::PadRoster;
use neptron_input::{Hotkeys, PortStates};
use neptron_library::Catalog;
use neptron_session::{
    Devices as SessionDevices, GameLaunch, InternalScale, Machine, Session, SessionOptions,
    VideoSetup,
};
use neptron_settings::Storage;
use neptron_ui::screens::Screens;
use neptron_ui::snapshot::{DisplayInfo, NoticeKind, ProfileEntry, ProfileId};
use neptron_ui::{SceneCanvas, Snapshot, Theme, UiFrame};
use neptron_video::{
    Content, DisplayConfig, Guard, GuardEvent, GuardRecord, SafeReason, ScanlineSettings,
    VideoEngine, VideoStatusText,
};

use super::{ControlInput, Devices, GameStage, ListStage, Runtime, Shell, Stage};
use crate::config::{Config, GameContext};
use crate::display::{Detected, MonitorError};
use crate::game_files::{self, GameFiles};
use crate::library::{ARCADE_CORE, Library, ScanKind, runs_arcade_boards};
use crate::nav::Navigator;
use crate::network::{NetworkInfo, NetworkWatch};
use crate::notices::ShownNotices;
use crate::outbox::{Outbox, Places};
use crate::pad_maps::PadMapsFile;
use crate::pad_notes::PadNotes;
use crate::paths::Paths;
use crate::presets::Presets;
use crate::record;
use crate::relaunch::{GameRef, NextRun, PadRecord};
use crate::storage::StorageStatus;
use crate::{gl_offer, start_failure};

/// A run that cannot start. The binary reports it and exits; the launcher
/// starts it again after its backoff.
#[derive(Debug, thiserror::Error)]
pub enum BootError {
    /// The built-in system and core definitions do not load.
    #[error("the system and core definitions do not load: {0}")]
    Catalog(#[from] neptron_library::CatalogError),
    /// No safe monitor profile exists.
    #[error(transparent)]
    Monitor(#[from] MonitorError),
    /// The settings and the policy disagree.
    #[error("the settings do not map to the policy: {0}")]
    Settings(#[from] neptron_video::SettingsError),
    /// The writer thread cannot start.
    #[error("cannot start the writer thread: {0}")]
    Writer(#[source] std::io::Error),
}

/// What [`Shell::boot`] gives.
pub enum Booted<D: Display, S: AudioSink, I: InputSource + PadRoster, C: Clock + Clone> {
    /// The run is ready for its first frame.
    Ready(Box<Shell<D, S, I, C>>),
    /// The game did not start, and the display went with it: the run ends
    /// with a relaunch to the game list, which shows why.
    Relaunch {
        /// The relaunch to the game list, with the notice.
        next: NextRun,
        /// The lines for the log: the notice, and the error lines of the
        /// core.
        log: Vec<String>,
    },
}

/// What the start found, for the notices of the first frame.
struct Found {
    issues: Vec<(NoticeKind, String)>,
}

impl<D: Display, S: AudioSink, I: InputSource + PadRoster, C: Clock + Clone> Shell<D, S, I, C> {
    /// Starts a run: the game list when `next` names no game, else that
    /// game. `next` is the relaunch file of the run before; `None` is a new
    /// boot. The guard record is written before the first modeset.
    ///
    /// # Errors
    ///
    /// [`BootError`].
    pub fn boot(
        paths: Paths,
        devices: Devices<D, S, I, C>,
        mut runtime: Runtime,
        next: Option<NextRun>,
    ) -> Result<Booted<D, S, I, C>, BootError> {
        let mut found = Found { issues: Vec::new() };
        let imported = std::mem::take(&mut runtime.boot_file);
        found.warn_all(runtime.notices.iter().cloned());
        // The guard boots and its record is written before every step
        // that can fail for a reason other than the display, so a start
        // that fails later still counts as a boot.
        let presets = load_presets(&paths.monitors(), &mut found);
        let catalog = Catalog::builtin()?;
        let game = next.as_ref().and_then(|n| n.game.clone());
        let storage = Storage::new(paths.config());
        let mut game_log = Vec::new();
        let files = game.as_ref().map(|g| {
            let mut issues = Vec::new();
            let files = game_files::find(&paths.roms(), &catalog, g, &mut issues);
            found.warn_all(issues);
            game_log.extend(files.long_folder.clone());
            if files.adopt_flat {
                adopt_flat_settings(&storage, &g.system, &files, &mut found);
            }
            files
        });
        let context = game.as_ref().zip(files.as_ref()).map(|(g, f)| GameContext {
            system: g.system.clone(),
            game: f.key(),
            arcade: runs_arcade_boards(&catalog, &g.system),
        });
        let (config, issues) = Config::load(&storage, context);
        found.warn_all(issues);
        let (shown_notices, issue) = ShownNotices::load(&paths.notices());
        found.warn_all(issue);
        let (pad_notes, pad_notes_issue) = PadNotes::load(&paths.pad_notes());
        let (pad_maps, pad_maps_issues) = PadMapsFile::load(&paths.pad_maps());
        if let Some(reason) = pad_maps.locked() {
            found.warn(format!(
                "{reason}. The controller layouts are not used, and a new one is not kept."
            ));
        }
        if let Some(reason) = pad_maps.newer() {
            found.warn(format!(
                "{reason}. Its controller layouts are not used. A new layout starts a new file and keeps the old one as pads.toml.bak."
            ));
        }
        match pad_maps.unusable() {
            0 => {}
            1 => found.warn(
                "A controller layout has no direction, A or B. Its controller uses its default layout."
                    .to_owned(),
            ),
            n => found.warn(format!(
                "{n} controller layouts have no direction, A or B. Their controllers use their default layout."
            )),
        }
        if let Some(notice) = next.as_ref().and_then(|n| n.notice.clone()) {
            found.issues.push((NoticeKind::Error, notice));
        }
        // The output the backend drives, so the EDID and the class are
        // those of the display on the wire.
        let mut detected = Detected::read(
            &paths.sysfs,
            &runtime.output,
            runtime.subconnector.as_deref(),
        )
        .with_tv(runtime.tv_standards.clone());
        detected.fit_to(devices.display.caps());
        found.warn_all(detected.notices.iter().cloned());
        let here = record::Place {
            output: runtime.output.clone(),
            monitor: detected.identity(),
        };
        let (record, places) = load_record(&paths, &detected, &presets, &here, &mut found);
        let setting = config.monitor();
        let safe = detected.safe(&presets, &setting)?;
        let trusted = detected.trusts(&setting);
        // A relaunch is a run of the same boot: the first run said it.
        if next.is_none() && detected.is_tv() && config.monitor() != crate::display::AUTO {
            found.warn(format!(
                "display.monitor {:?} has no effect on a TV output: display.tv sets its standard.",
                config.monitor()
            ));
        }
        let monitor = match detected.wanted(&config.monitor(), &config.tv(), &presets) {
            Ok(monitor) => monitor,
            Err(e) => {
                found.warn(e.to_string());
                safe.monitor.clone()
            }
        };
        let settings = config.global_policy().unwrap_or_else(|e| {
            found.warn(format!("{e}; the safe picture mode applies"));
            safe.settings
        });
        let wanted = DisplayConfig { monitor, settings };
        let scanlines = config.scanlines().unwrap_or_else(|e| {
            found.warn(format!("{e}; no scanlines are drawn"));
            ScanlineSettings::OFF
        });
        let frame = Rc::new(Cell::new(0));
        let queue = Rc::new(RefCell::new(VecDeque::new()));
        let mut source = devices.input;
        // Before the first poll, so every pad opens with its map.
        source.set_player_maps(pad_maps.maps());
        let input = ControlInput::new(source, queue.clone(), frame.clone());

        let now_ns = devices.clock.now_ns();
        let (stage, guard, mut step, library) = match &game {
            None => {
                let mut engine =
                    VideoEngine::new(devices.display, wanted.monitor.clone(), wanted.settings);
                engine.set_scanlines(scanlines);
                // Before the guard sets the first mode: the first picture
                // of the boot is already turned.
                engine.set_content(Content::Menu);
                engine.set_orientation(config.orientation());
                // A return from a game is a run of the same boot: the first
                // run had the boot prompt, so this one never prompts.
                let start = if next.is_some() {
                    Guard::continue_boot
                } else {
                    Guard::boot
                };
                let (guard, step) =
                    start(record, wanted, safe.clone(), trusted, now_ns, &mut engine);
                let mut hotkeys = Hotkeys::new();
                hotkeys.set_start_hold_ns(config.start_hold_ns());
                let list = ListStage {
                    engine,
                    input,
                    audio: devices.audio,
                    ports: PortStates::new(),
                    hotkeys,
                    fired: Vec::new(),
                };
                (Stage::List(Box::new(list)), guard, step, None)
            }
            // A game run follows a relaunch file, so the run before it
            // has counted this boot already, and it had the boot prompt:
            // the game run starts on what was kept, with no prompt.
            Some(reference) => {
                // Made above for every game.
                let files = files.clone().unwrap_or_else(|| GameFiles::of(reference));
                let (library, issue) = Library::load(
                    catalog.clone(),
                    &paths.library_cache(),
                    paths.arcade_index(ARCADE_CORE),
                );
                found.warn_all(issue);
                // Taken before the launch takes the input: a launch that
                // fails gives the order of the players back to the list.
                let pads = PadRecord::list(&input.roster());
                let mut display = devices.display;
                let offer = gl_offer::kept_offer(&paths.gl_offer(), &mut display, &mut |w| {
                    found.warn(w);
                });
                let launched = launch(
                    &paths,
                    &library,
                    &config,
                    reference,
                    &files,
                    &wanted,
                    &runtime,
                    SessionDevices {
                        audio: devices.audio,
                        input,
                        clock: devices.clock.clone(),
                    },
                    (display, offer),
                );
                let mut game = match launched {
                    Ok(game) => {
                        found.warn_all(start_failure::fallback_line(
                            game.session.host().hw_refusal(),
                        ));
                        runtime.pulse.enter_game();
                        game
                    }
                    Err((text, mut log)) => {
                        log.insert(0, text.clone());
                        return Ok(Booted::Relaunch {
                            next: NextRun {
                                pads,
                                game: None,
                                notice: Some(text),
                            },
                            log,
                        });
                    }
                };
                let (guard, step) = Guard::continue_boot(
                    record,
                    wanted,
                    safe.clone(),
                    trusted,
                    now_ns,
                    game.session.video_mut(),
                );
                (Stage::Game(Box::new(game)), guard, step, Some(library))
            }
        };
        let storage = match StorageStatus::read(&paths.run) {
            Ok(status) => status,
            Err(e) => {
                found.warn(e.to_string());
                None
            }
        };
        // A record in a RAM folder still counts the failed boots of this
        // power cycle, so safe mode comes after crashes as on a partition,
        // and a new power cycle has no last known good: its boot prompt
        // reverts to the safe default. Only a write that fails is a boot
        // that counts nothing.
        let mut places = places;
        let unrecorded = step.save.take().and_then(|saved| {
            places = record::merge(&places, &here, saved.last_known_good.as_ref());
            record::save(&paths.guard_record(), &saved, &places)
                .err()
                .map(|e| {
                    format!(
                        "Safe mode: the guard record cannot be written ({e}). This start uses the safe picture."
                    )
                })
        });
        let library = match library {
            Some(library) => library,
            None => {
                let (mut library, issue) = Library::load(
                    catalog,
                    &paths.library_cache(),
                    paths.arcade_index(ARCADE_CORE),
                );
                found.warn_all(issue);
                // A relaunch file and the marker of a full scan mean that
                // the cache comes from this power cycle: every change since
                // came through this device. A cache of an earlier power
                // cycle may miss a change made on another computer, whose
                // exFAT driver may leave the folder times.
                let kind = if next.is_some() && paths.library_scanned().exists() {
                    ScanKind::Check
                } else {
                    ScanKind::Boot
                };
                if let Err(e) = library.start_scan(paths.roms(), kind, SystemTime::now()) {
                    found.warn(format!("the game scan did not start: {e}"));
                }
                library
            }
        };
        let outbox = Outbox::start(Places {
            config: paths.config(),
            record: paths.guard_record(),
            cache: paths.library_cache(),
            scanned: paths.library_scanned(),
            share_request: paths.share_format_request(),
            notices: paths.notices(),
            pad_notes: paths.pad_notes(),
            pad_maps: paths.pad_maps(),
        })
        .map_err(BootError::Writer)?;
        let share_offer = storage.as_ref().and_then(StorageStatus::format_offer);
        // The first read starts now, so the Network panel has its rows when
        // the player opens it.
        let (network, network_info) =
            match NetworkWatch::start(paths.run.clone(), paths.state.clone()) {
                Ok(watch) => {
                    watch.request();
                    (Some(watch), None)
                }
                Err(e) => (
                    None,
                    Some(NetworkInfo::Unknown {
                        reason: format!("the reader thread did not start: {e}"),
                    }),
                ),
            };
        let theme = load_theme(&paths.skin(), &mut found);
        let screensaver =
            super::screensaver::Screensaver::new(now_ns, config.screensaver_timeout_ns());
        let mut shell = Shell {
            paths,
            config,
            presets,
            detected,
            safe,
            library,
            guard,
            stage,
            clock: devices.clock,
            runtime,
            screens: Screens::new(theme),
            ui: UiFrame::new(),
            canvas: SceneCanvas::new(),
            spare_canvas: None,
            snap: Snapshot::default(),
            nav: Navigator::new(),
            actions: Vec::with_capacity(16),
            events: Vec::with_capacity(256),
            outbox,
            pending: Vec::new(),
            status_text: VideoStatusText::default(),
            shown_mode: None,
            shown_safe: false,
            now_ns,
            menu_hotkey: false,
            picture_notice: super::PictureNotice::Launch,
            speed_notices: crate::speed_notice::SpeedNotices::new(),
            keep_batch: super::pair::KeepBatch::default(),
            page_ask: None,
            keep_asks: Vec::new(),
            last_answers: None,
            shown_notices,
            pad_notes,
            pad_maps,
            mapping: None,
            next_mapping: 0,
            input_notices: Vec::with_capacity(INPUT_NOTICES_CAPACITY),
            next_notice: 1,
            unchecked_launch: None,
            frame,
            control_queue: queue,
            log: VecDeque::new(),
            here,
            places,
            share_offer,
            format_request: super::FormatRequest::None,
            network,
            network_info,
            network_next_ns: 0,
            network_shown: false,
            network_requests: 0,
            network_builds: 0,
            pad_changes: 0,
            screensaver,
            outcome: None,
        };
        shell.first_snapshot();
        shell.sync_form();
        for line in imported.log {
            shell.log(line);
        }
        for line in game_log {
            shell.log(line);
        }
        // A relaunch is a run of the same boot: its first run showed the
        // problems already.
        if next.is_none() {
            found.warn_all(imported.problems);
        } else {
            for problem in imported.problems {
                shell.log(problem);
            }
        }
        if let Some(issue) = pad_notes_issue {
            shell.log(format!("input: {issue}"));
        }
        for issue in pad_maps_issues {
            shell.log(format!("input: {issue}"));
        }
        if let Some(NetworkInfo::Unknown { reason }) = &shell.network_info {
            // The panel shows `Not known` for this run; the run goes on.
            let line = format!("network: {reason}");
            shell.log(line);
        }
        if let Some(GuardEvent::SafeMode(reason)) = step.event.take() {
            shell.start_in_safe_mode(reason);
        }
        shell.on_guard(step, now_ns);
        if let Some(text) = unrecorded {
            // A boot that cannot count itself could fail again and again
            // and never reach safe mode, so it starts in the safe default.
            shell.safe_for_this_run(text);
        }
        let power = shell.runtime.power;
        for text in storage.iter().flat_map(|s| s.notices(power)) {
            shell.log(format!("storage: {text}"));
            let kind = if text == crate::storage::FORMATTED_NOTICE {
                NoticeKind::Info
            } else {
                NoticeKind::Warning
            };
            found.issues.push((kind, text));
        }
        if shell.in_game() {
            let settings = shell.config.policy()?;
            if let Some(step) =
                shell.with_engine(|guard, engine| guard.follow_settings(settings, engine))
            {
                shell.on_guard(step, now_ns);
            }
        }
        for (kind, text) in found.issues {
            shell.notice(kind, text);
        }
        Ok(Booted::Ready(Box::new(shell)))
    }

    /// The notice of a start in safe mode, once per power cycle: every
    /// relaunch and every restart of the launcher starts in it again, and
    /// the notice would stand in the way each time. It plays no tone; the
    /// tone confirms a hold of the blind recovery.
    fn start_in_safe_mode(&mut self, reason: SafeReason) {
        self.log(format!("guard: start in safe mode ({reason:?})"));
        let marker = self.paths.safe_notice_shown();
        if marker.exists() {
            return;
        }
        // With no marker the next start of this power cycle shows the
        // notice again, which only repeats it.
        let _ =
            std::fs::create_dir_all(&self.paths.run).and_then(|()| std::fs::write(&marker, b""));
        self.notice(
            NoticeKind::Warning,
            super::guard_events::safe_text(reason).to_owned(),
        );
    }

    fn first_snapshot(&mut self) {
        self.snap.systems = self.library.systems();
        self.snap.profiles = self
            .presets
            .profiles()
            .iter()
            .enumerate()
            .map(|(i, p)| ProfileEntry::new(profile_id(i), p.id.clone()))
            .collect();
        self.snap.display = Some(DisplayInfo::new(self.runtime.output.clone(), String::new()));
        self.refresh_profile();
    }
}

impl Found {
    fn warn(&mut self, text: String) {
        self.issues.push((NoticeKind::Warning, text));
    }

    fn warn_all(&mut self, texts: impl IntoIterator<Item = String>) {
        for text in texts {
            self.warn(text);
        }
    }
}

/// The input notices one frame can take with no allocation: the queue of
/// the gamepad source and of the keyboard source, each with its count of
/// dropped notices.
const INPUT_NOTICES_CAPACITY: usize = 2 * (neptron_input::MAX_NOTICES + 1);

/// The profile number of the preset at `index`.
pub(super) fn profile_id(index: usize) -> ProfileId {
    ProfileId(u32::try_from(index).unwrap_or(u32::MAX))
}

/// The game name of a path under the ROM folder: the file name without its
/// extension, as the library and the settings files name a game.
pub(super) fn game_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_owned()
}

/// Copies the flat settings file of a game in a folder to its own file,
/// when its own file is missing (see [`game_files`]). A failed copy is a
/// warning: the game then runs on the settings of its system.
fn adopt_flat_settings(storage: &Storage, system: &str, files: &GameFiles, found: &mut Found) {
    if let Err(e) = storage.copy_game_file(system, &files.stem, &files.key()) {
        found.warn(format!(
            "the earlier settings of {} are not copied to its folder: {e}",
            files.stem
        ));
    }
}

fn load_presets(dir: &Path, found: &mut Found) -> Presets {
    match Presets::load(dir) {
        Ok((presets, issues)) => {
            found.warn_all(issues.iter().map(ToString::to_string));
            presets
        }
        Err(e) => {
            found.warn(e.to_string());
            Presets::default()
        }
    }
}

/// The record for this start at `here`, and the last known good of every
/// place, which every later write of the record keeps.
fn load_record(
    paths: &Paths,
    detected: &Detected,
    presets: &Presets,
    here: &record::Place,
    found: &mut Found,
) -> (GuardRecord, Vec<record::PlaceEntry>) {
    let stored = record::load(&paths.guard_record()).unwrap_or_else(|e| {
        found.warn(format!("{e}; the guard starts with no last known good"));
        record::Stored::default()
    });
    let record = record::for_place(&stored, here, |r| detected.rebuild(presets, r));
    (record, stored.places)
}

fn load_theme(path: &Path, found: &mut Found) -> Theme {
    let parsed = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read the skin {}: {e}", path.display()))
        .and_then(|text| {
            neptron_ui::skin::parse(&text).map_err(|e| format!("the skin {}: {e}", path.display()))
        });
    parsed.unwrap_or_else(|e| {
        found.warn(e);
        Theme::default()
    })
}

/// The machine for the internal resolution of a GL core. The ARM images
/// are the Raspberry Pi images, whose GPU cost of a larger picture is not
/// measured yet.
const MACHINE: Machine = if cfg!(any(target_arch = "aarch64", target_arch = "arm")) {
    Machine::RaspberryPi
} else {
    Machine::Pc
};

/// A launch that failed: the notice of the game list, and the log lines.
type Failure = (String, Vec<String>);

/// Launches the game of a relaunch file. The error is the notice of the
/// game list and the log lines of the failure.
#[allow(clippy::too_many_arguments)]
fn launch<D: Display, S: AudioSink, I: InputSource + PadRoster, C: Clock + Clone>(
    paths: &Paths,
    library: &Library,
    config: &Config,
    reference: &GameRef,
    files: &GameFiles,
    wanted: &DisplayConfig,
    runtime: &Runtime,
    devices: SessionDevices<S, ControlInput<I>, C>,
    (display, core_offer): (D, ContextOffer),
) -> Result<GameStage<D, S, I, C>, Failure> {
    let name = game_name(&reference.path);
    let (system, core) = library.resolve(&reference.system).ok_or_else(|| {
        let text = format!(
            "{name} did not start: the system {} is unknown.",
            reference.system
        );
        (text, Vec::new())
    })?;
    let core_path = paths.core(&core.library);
    let content = paths.game(&reference.path);
    let system_dir = paths.bios();
    let save_dir = paths.saves(&system.folder);
    let game = GameLaunch {
        core: &core_path,
        content: Some(&content),
        extensions: &system.extensions,
        system_dir: &system_dir,
        save_dir: &save_dir,
        save_folders: &files.folders,
        adopt_flat_save: files.adopt_flat,
        core_options: &core.options,
        always_progressive: system.always_progressive,
        internal_scale: core.internal_resolution.as_ref().map(|r| InternalScale {
            option: &r.option,
            native_lines: r.native_lines,
            values: &r.values,
        }),
        more_discs: &[],
        save_name: None,
        core_saves_in_game_folder: false,
    };
    let policy = config.policy().map_err(|e| (e.to_string(), Vec::new()))?;
    let scanlines = config
        .scanlines()
        .map_err(|e| (e.to_string(), Vec::new()))?;
    let video = VideoSetup {
        display,
        monitor: wanted.monitor.clone(),
        policy,
        scanlines,
        core_offer: Some(core_offer),
    };
    let options = SessionOptions {
        resampling: runtime.resampling,
        run_rate: runtime.run_rate,
        start_hold_ns: config.start_hold_ns(),
        volume_percent: config.volume_percent(),
        machine: MACHINE,
    };
    let mut session = Session::launch(&game, video, devices, options).map_err(|e| {
        (
            start_failure::launch_notice(&name, &e),
            start_failure::launch_log(&e),
        )
    })?;
    // The launch sets no mode; the first `run_frame` does, with this
    // orientation.
    session.video_mut().set_orientation(config.orientation());
    Ok(GameStage {
        session,
        reference: reference.clone(),
        core: core.id.clone(),
    })
}
