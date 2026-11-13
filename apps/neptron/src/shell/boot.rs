//! The start of a run, before the first frame: settings, presets, the
//! detected output, the library, the guard and the stage.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::path::Path;
use std::rc::Rc;
use std::time::SystemTime;

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::{Hotkeys, PortStates};
use neptron_library::Catalog;
use neptron_session::{Devices as SessionDevices, GameLaunch, Session, SessionOptions, VideoSetup};
use neptron_settings::Storage;
use neptron_ui::screens::Screens;
use neptron_ui::snapshot::{DisplayInfo, NoticeKind, ProfileEntry, ProfileId};
use neptron_ui::{CpuCanvas, Snapshot, Theme, UiFrame};
use neptron_video::{
    DisplayConfig, Guard, GuardEvent, GuardRecord, ScanlineSettings, VideoEngine, VideoStatusText,
};

use super::{ControlInput, Devices, GameStage, ListStage, Runtime, Shell, Stage};
use crate::config::{Config, GameContext};
use crate::display::{Detected, MonitorError};
use crate::library::{Library, ScanKind};
use crate::nav::Navigator;
use crate::outbox::{Outbox, Places};
use crate::paths::Paths;
use crate::presets::Presets;
use crate::record;
use crate::relaunch::{GameRef, NextRun};
use crate::storage::StorageStatus;

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
pub enum Booted<D: Display, S: AudioSink, I: InputSource, C: Clock + Clone> {
    /// The run is ready for its first frame.
    Ready(Box<Shell<D, S, I, C>>),
    /// The game did not start, and the display went with it: the run ends
    /// with a relaunch to the game list, which shows why.
    Relaunch(NextRun),
}

/// What the start found, for the notices of the first frame.
struct Found {
    issues: Vec<(NoticeKind, String)>,
}

impl<D: Display, S: AudioSink, I: InputSource, C: Clock + Clone> Shell<D, S, I, C> {
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
        runtime: Runtime,
        next: Option<NextRun>,
    ) -> Result<Booted<D, S, I, C>, BootError> {
        let mut found = Found { issues: Vec::new() };
        found.warn_all(runtime.notices.iter().cloned());
        // The guard boots and its record is written before every step
        // that can fail for a reason other than the display, so a start
        // that fails later still counts as a boot.
        let presets = load_presets(&paths.monitors(), &mut found);
        let game = next.as_ref().and_then(|n| n.game.clone());
        let context = game.as_ref().map(|g| GameContext {
            system: g.system.clone(),
            game: game_name(&g.path),
        });
        let (config, issues) = Config::load(&Storage::new(paths.config()), context);
        found.warn_all(issues);
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
        let (record, record_place) = load_record(
            &paths,
            &detected,
            &presets,
            &here,
            next.is_some(),
            &mut found,
        );
        // The record holds a last known good only for this output, so a
        // panel is the safe default only once the user kept it here.
        let safe = detected.safe(&presets, record.last_known_good.as_ref())?;
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
            found.warn(format!("{e}; the safe picture style applies"));
            safe.settings
        });
        let wanted = DisplayConfig { monitor, settings };
        let scanlines = config.scanlines().unwrap_or_else(|e| {
            found.warn(format!("{e}; no scanlines are drawn"));
            ScanlineSettings::OFF
        });
        let frame = Rc::new(Cell::new(0));
        let queue = Rc::new(RefCell::new(VecDeque::new()));
        let input = ControlInput::new(devices.input, queue.clone(), frame.clone());

        let now_ns = devices.clock.now_ns();
        let (stage, guard, mut step, library) = match &game {
            None => {
                let mut engine =
                    VideoEngine::new(devices.display, wanted.monitor.clone(), wanted.settings);
                engine.set_scanlines(scanlines);
                let (guard, step) = Guard::boot(record, wanted, safe.clone(), now_ns, &mut engine);
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
                let (library, issue) = Library::load(Catalog::builtin()?, &paths.library_cache());
                found.warn_all(issue);
                let launched = launch(
                    &paths,
                    &library,
                    &config,
                    reference,
                    &wanted,
                    &runtime,
                    SessionDevices {
                        audio: devices.audio,
                        input,
                        clock: devices.clock.clone(),
                    },
                    devices.display,
                );
                let mut game = match launched {
                    Ok(game) => game,
                    Err(text) => {
                        return Ok(Booted::Relaunch(NextRun {
                            game: None,
                            notice: Some(text),
                        }));
                    }
                };
                let (guard, step) = Guard::continue_boot(
                    record,
                    wanted,
                    safe.clone(),
                    now_ns,
                    game.session.video_mut(),
                );
                (Stage::Game(Box::new(game)), guard, step, Some(library))
            }
        };
        let boot_panel_prompt = matches!(step.event, Some(GuardEvent::PromptOpened(_)))
            && detected.auto_is_panel()
            && config.monitor() == crate::display::AUTO;
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
        let unrecorded = step.save.take().and_then(|saved| {
            record::save(&paths.guard_record(), &saved, record_place.as_ref())
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
                let (mut library, issue) =
                    Library::load(Catalog::builtin()?, &paths.library_cache());
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
        })
        .map_err(BootError::Writer)?;
        let share_offer = storage.as_ref().and_then(StorageStatus::format_offer);
        let theme = load_theme(&paths.skin(), &mut found);
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
            canvas: CpuCanvas::new(),
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
            panel_windows: u8::from(boot_panel_prompt),
            panel_offered: false,
            picture_noticed: false,
            next_notice: 1,
            frame,
            control_queue: queue,
            log: VecDeque::new(),
            record_place,
            share_offer,
            outcome: None,
        };
        shell.first_snapshot();
        shell.on_guard(step, now_ns);
        if let Some(text) = unrecorded {
            // A boot that cannot count itself could fail again and again
            // and never reach safe mode, so it starts in the safe default.
            shell.safe_for_this_run(text);
        }
        for text in storage.iter().flat_map(StorageStatus::notices) {
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

/// The record for this start, and the place of its last known good, which
/// every later write of the record keeps.
fn load_record(
    paths: &Paths,
    detected: &Detected,
    presets: &Presets,
    here: &record::Place,
    continued: bool,
    found: &mut Found,
) -> (GuardRecord, Option<record::Place>) {
    let stored = record::load(&paths.guard_record(), |r| detected.rebuild(presets, r))
        .unwrap_or_else(|e| {
            found.warn(format!("{e}; the guard starts with no last known good"));
            record::Stored::default()
        });
    let kept = record::confirmed_at(&stored, here).then(|| here.clone());
    let record = record::for_place(stored, here);
    let record = if continued {
        record::for_continued_boot(record)
    } else {
        record
    };
    (record, kept)
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

/// Launches the game of a relaunch file. The text of the error goes into
/// the notice of the game list.
#[allow(clippy::too_many_arguments)]
fn launch<D: Display, S: AudioSink, I: InputSource, C: Clock + Clone>(
    paths: &Paths,
    library: &Library,
    config: &Config,
    reference: &GameRef,
    wanted: &DisplayConfig,
    runtime: &Runtime,
    devices: SessionDevices<S, ControlInput<I>, C>,
    display: D,
) -> Result<GameStage<D, S, I, C>, String> {
    let name = game_name(&reference.path);
    let (system, core) = library.resolve(&reference.system).ok_or_else(|| {
        format!(
            "{name} did not start: the system {} is unknown.",
            reference.system
        )
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
        core_options: &core.options,
    };
    let policy = config.policy().map_err(|e| e.to_string())?;
    let scanlines = config.scanlines().map_err(|e| e.to_string())?;
    let video = VideoSetup {
        display,
        monitor: wanted.monitor.clone(),
        policy,
        scanlines,
    };
    let options = SessionOptions {
        resampling: runtime.resampling,
        run_rate: runtime.run_rate,
        start_hold_ns: config.start_hold_ns(),
        volume_percent: config.volume_percent(),
    };
    let session = Session::launch(&game, video, devices, options)
        .map_err(|e| format!("{name} did not start: {e}."))?;
    Ok(GameStage {
        session,
        reference: reference.clone(),
        core: core.id.clone(),
    })
}
