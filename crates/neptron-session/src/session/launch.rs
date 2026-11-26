//! Launch: open the core, load the content and the save RAM, settle the
//! source, start the audio and the save RAM writer.

use std::path::PathBuf;

use neptron_audio::{AudioPipeline, CoreTiming, FlipRateMeter};
use neptron_backend::{AudioSink, Clock, ContextOffer, Display, InputSource};
use neptron_input::{Hotkeys, PortStates};
use neptron_libretro::{
    Backup, ContentRequest, Host, HostConfig, LOG_TEXT_MAX_BYTES, LoadError, MESSAGE_MAX_BYTES,
    read_save_file, save_file_path, save_folder, write_save_file,
};
use neptron_video::VideoEngine;

use super::hw::ScaleRule;
use super::{INPUT_EVENTS_CAPACITY, Session, State};
use crate::pacing::{FlipClock, Pacing};
use crate::pads::PadMask;
use crate::picture::Picture;
use crate::save::{SAVE_RAM_FLUSH_INTERVAL_NS, SaveRamWriter};
use crate::source::{FrameRules, LaunchSettle, source_geometry};
use crate::speed::Speed;
use crate::start::{START_ERRORS_MAX, StartFacts, collect_errors};
use crate::{Counters, Devices, GameLaunch, LaunchError, SessionOptions, VideoSetup};

/// The save file of a game and what it held at launch.
struct SaveFile {
    path: PathBuf,
    loaded: Vec<u8>,
    ram_bytes: usize,
}

impl<D: Display, S: AudioSink, I: InputSource, C: Clock> Session<D, S, I, C> {
    /// Launches a game.
    ///
    /// 1. Makes the save folder, opens the core, gives it the context
    ///    types that the display offers and the core option values to
    ///    hold, with the internal resolution of a GL core (see
    ///    [`internal_scale`](crate::internal_scale)), and loads the
    ///    content. A core that declares its options during the load gets
    ///    the values too.
    /// 2. Reads the save file and loads it into the core's save RAM.
    /// 3. For a core that asked for a GL context: makes the context with an
    ///    image of the core's largest frame, gives the core its framebuffer
    ///    and lookup, and calls `context_reset` inside the context
    ///    (`docs/spec/hw-render.md`, section 3.1).
    /// 4. Runs the core with no display until its source settles (see
    ///    [`LAUNCH_STABLE_FRAMES`](crate::LAUNCH_STABLE_FRAMES)), so the
    ///    first mode is the game's mode. The audio of these frames is
    ///    dropped and the core reads no input.
    /// 5. Starts the audio pipeline at the core's own rate and the save
    ///    RAM writer thread.
    ///
    /// The first [`Session::run_frame`] sets the first mode. Launch is not
    /// part of the frame loop, so its file reads are here.
    ///
    /// # Errors
    ///
    /// [`LaunchError`]. The save file is never written by a failed launch.
    pub fn launch(
        game: &GameLaunch<'_>,
        video: VideoSetup<D>,
        devices: Devices<S, I, C>,
        options: SessionOptions,
    ) -> Result<Self, LaunchError> {
        std::fs::create_dir_all(game.save_dir).map_err(|source| LaunchError::SaveDir {
            path: game.save_dir.to_owned(),
            source,
        })?;
        let config = HostConfig {
            system_dir: game.system_dir.to_owned(),
            save_dir: game.save_dir.to_owned(),
        };
        let mut display = video.display;
        let offer = display
            .core_render()
            .map_or(ContextOffer::NONE, |render| render.offer());
        let mut host = Host::open(game.core, config)?;
        host.set_hw_offer(offer);
        let scale_rule = game
            .internal_scale
            .map(|scale| ScaleRule::new(&scale, options.machine));
        let (scale, scale_pair) = scale_rule.as_ref().map_or((1, None), |rule| {
            rule.pick(&video.monitor.profile().kind, video.scanlines.mode)
        });
        // The value of the rule comes last, so it wins over a value of the
        // core data for the same key.
        let held: Vec<(String, String)> = game
            .core_options
            .iter()
            .cloned()
            .chain(scale_pair)
            .collect();
        host.hold_options(&held);
        let request = game.content.map(|path| ContentRequest {
            path,
            extensions: game.extensions,
        });
        let loaded = host.load_game(request);
        let mut start_errors = Vec::with_capacity(START_ERRORS_MAX);
        collect_errors(&mut start_errors, host.error_logs());
        let start_dropped_logs = host.dropped_logs();
        let av = match loaded {
            Ok(av) => av,
            Err(LoadError::Refused) => {
                return Err(LaunchError::Refused {
                    core_errors: start_errors,
                    dropped_logs: start_dropped_logs,
                    hw_refusal: host.hw_refusal(),
                });
            }
            Err(e) => return Err(LaunchError::Load(e)),
        };
        let refused_options = host.held_option_report().len();
        let save = load_save_file(&mut host, game)?;

        let mut engine = VideoEngine::new(display, video.monitor, video.policy);
        engine.set_scanlines(video.scanlines);
        let mut hotkeys = Hotkeys::new();
        hotkeys.set_start_hold_ns(options.start_hold_ns);
        let geometry = av.geometry;
        let rules = FrameRules {
            rotation: host.rotation(),
            always_progressive: game.always_progressive,
        };
        let mut session = Self {
            host,
            engine,
            audio: None,
            resampling: options.resampling,
            input: devices.input,
            clock: devices.clock,
            events: Vec::with_capacity(INPUT_EVENTS_CAPACITY),
            ports: PortStates::new(),
            hotkeys,
            fired: Vec::with_capacity(INPUT_EVENTS_CAPACITY),
            mask: PadMask::default(),
            picture: Picture::new(geometry.max_width, geometry.max_height),
            hw: None,
            picture_scale: 1,
            scale_rule,
            no_picture: false,
            geometry,
            timing: av.timing,
            always_progressive: game.always_progressive,
            source: source_geometry(
                geometry.base_width,
                geometry.base_height,
                &geometry,
                &av.timing,
                rules,
            ),
            state: State::Running,
            run_rate: options.run_rate,
            meter: FlipRateMeter::new(),
            refused_run_rate: None,
            writer: None,
            next_flush_ns: 0,
            message: String::with_capacity(MESSAGE_MAX_BYTES),
            message_frames: 0,
            start_errors,
            start_dropped_logs,
            start: StartFacts {
                picture: false,
                said_at_launch: false,
                started_ns: 0,
            },
            last_error: String::with_capacity(LOG_TEXT_MAX_BYTES),
            flipless: 0,
            last_flip: None,
            last_flip_session_ns: None,
            latency_from: None,
            flip_clock: FlipClock::default(),
            pacing: Pacing::new(),
            deferred: false,
            queued_ran_core: false,
            queued_paced: false,
            cycle_start_ns: 0,
            cycle_ns: 0,
            speed: Speed::new(),
            run_rate_slewing: false,
            queue_level: 0,
            too_slow_for_sound: false,
            picture_hidden: false,
            counters: Counters {
                refused_options: refused_options as u64,
                ..Counters::default()
            },
        };
        if let Some(request) = session.host.hw_request() {
            let native_lines = game.internal_scale.map_or(0, |s| s.native_lines);
            session.start_hw(request, (scale, native_lines))?;
        }
        session.settle()?;
        session.start.said_at_launch =
            !session.start_errors.is_empty() || !session.message.is_empty();
        session.start.started_ns = session.clock.now_ns();

        let timing = CoreTiming {
            sample_rate_mhz: u64::from(session.timing.sample_rate_mhz),
            frame_rate_mhz: u64::from(session.timing.fps_mhz),
        };
        let pipeline = AudioPipeline::new(
            devices.audio,
            timing,
            timing.frame_rate_mhz,
            session.resampling,
        )
        .map_err(LaunchError::Audio)?
        .with_volume_percent(options.volume_percent);
        session.audio = Some(pipeline);
        if let Some(save) = save {
            let writer = SaveRamWriter::start(save.path, &save.loaded, save.ram_bytes)
                .map_err(LaunchError::Writer)?;
            session.writer = Some(writer);
        }
        session.cycle_start_ns = session.clock.now_ns();
        session.next_flush_ns = session
            .clock
            .now_ns()
            .saturating_add(SAVE_RAM_FLUSH_INTERVAL_NS);
        Ok(session)
    }

    /// Runs core frames until the source settles. A core that asks to
    /// stop in these frames, or during the load, stops the launch.
    fn settle(&mut self) -> Result<(), LaunchError> {
        let mut settle = LaunchSettle::default();
        loop {
            self.run_core(false).map_err(LaunchError::Run)?;
            self.hw_fault().map_err(LaunchError::Session)?;
            self.counters.settle_frames += 1;
            collect_errors(&mut self.start_errors, self.host.error_logs());
            self.start_dropped_logs += self.host.dropped_logs();
            if self.state == State::Ended {
                return Err(LaunchError::CoreStopped {
                    core_errors: std::mem::take(&mut self.start_errors),
                    dropped_logs: self.start_dropped_logs,
                });
            }
            if settle.push(self.source) {
                return Ok(());
            }
        }
    }
}

/// Reads the save file into the core's save RAM. `None` when the core has
/// no save RAM or the content has no file name.
fn load_save_file(host: &mut Host, game: &GameLaunch<'_>) -> Result<Option<SaveFile>, LaunchError> {
    let mut scratch = Vec::new();
    let ram_bytes = host
        .copy_save_ram(&mut scratch)
        .map_err(LaunchError::SaveRam)?;
    let (Some(content), true) = (game.content, ram_bytes > 0) else {
        return Ok(None);
    };
    let folders: Vec<&str> = game.save_folders.iter().map(String::as_str).collect();
    let folder = save_folder(game.save_dir, &folders).map_err(LaunchError::SaveFolder)?;
    let Some(path) = save_file_path(&folder, content) else {
        return Ok(None);
    };
    if game.adopt_flat_save && !folders.is_empty() {
        adopt_flat_save(game.save_dir, content, &path)?;
    }
    let loaded = read_save_file(&path)
        .map_err(LaunchError::ReadSave)?
        .unwrap_or_default();
    if !loaded.is_empty() {
        host.load_save_ram(&loaded).map_err(LaunchError::SaveRam)?;
    }
    Ok(Some(SaveFile {
        path,
        loaded,
        ram_bytes,
    }))
}

/// Copies the flat save `<save_dir>/<stem>.srm` to `path`, the game's own
/// save file, when `path` has no file yet and the flat save exists, once:
/// the empty file `<path>.adopted` records it. The
/// flat save stays as it is. The copy is a full write (temp file, fsync,
/// rename), so a crash leaves no half copy, and it makes no backup: the
/// first write of the session does.
fn adopt_flat_save(
    save_dir: &std::path::Path,
    content: &std::path::Path,
    path: &std::path::Path,
) -> Result<(), LaunchError> {
    let mut record = path.as_os_str().to_owned();
    record.push(ADOPTED_SUFFIX);
    let record = std::path::PathBuf::from(record);
    if std::fs::symlink_metadata(&record).is_ok() {
        return Ok(());
    }
    let own = read_save_file(path).map_err(LaunchError::ReadSave)?;
    let flat = match (own, save_file_path(save_dir, content)) {
        (None, Some(flat)) => read_save_file(&flat).map_err(LaunchError::ReadSave)?,
        _ => None,
    };
    if let Some(bytes) = flat {
        write_save_file(path, &bytes, Backup::Keep).map_err(LaunchError::AdoptSave)?;
    }
    // The record settles the question once, also when nothing was copied:
    // a player who removes the save later to start again gets no copy.
    write_save_file(&record, &[], Backup::Keep).map_err(LaunchError::AdoptSave)
}

/// The suffix of the empty file beside a save that records that its game
/// took, or did not need, the flat save once.
const ADOPTED_SUFFIX: &str = ".adopted";
