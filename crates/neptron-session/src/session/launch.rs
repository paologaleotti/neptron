//! Launch: open the core, load the content and the save RAM, settle the
//! source, start the audio and the save RAM writer.

use std::path::PathBuf;

use neptron_audio::{AudioPipeline, CoreTiming, FlipRateMeter};
use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::{Hotkeys, PortStates};
use neptron_libretro::{
    ContentRequest, Host, HostConfig, MESSAGE_MAX_BYTES, read_save_file, save_file_path,
};
use neptron_video::VideoEngine;

use super::{INPUT_EVENTS_CAPACITY, Session, State};
use crate::pads::PadMask;
use crate::picture::Picture;
use crate::save::{SAVE_RAM_FLUSH_INTERVAL_NS, SaveRamWriter};
use crate::source::{LaunchSettle, source_geometry};
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
    /// 1. Makes the save folder, opens the core, sets the core options and
    ///    loads the content.
    /// 2. Reads the save file and loads it into the core's save RAM.
    /// 3. Runs the core with no display until its source settles (see
    ///    [`LAUNCH_STABLE_FRAMES`](crate::LAUNCH_STABLE_FRAMES)), so the
    ///    first mode is the game's mode. The audio of these frames is
    ///    dropped and the core reads no input.
    /// 4. Starts the audio pipeline at the core's own rate and the save
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
        let mut host = Host::open(game.core, config)?;
        let refused_options = game
            .core_options
            .iter()
            .filter(|(key, value)| host.set_option(key, value).is_err())
            .count();
        let request = game.content.map(|path| ContentRequest {
            path,
            extensions: game.extensions,
        });
        let av = host.load_game(request)?;
        let save = load_save_file(&mut host, game)?;

        let mut engine = VideoEngine::new(video.display, video.monitor, video.policy);
        engine.set_scanlines(video.scanlines);
        let mut hotkeys = Hotkeys::new();
        hotkeys.set_start_hold_ns(options.start_hold_ns);
        let geometry = av.geometry;
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
            geometry,
            timing: av.timing,
            source: source_geometry(
                geometry.base_width,
                geometry.base_height,
                &geometry,
                &av.timing,
            ),
            state: State::Running,
            run_rate: options.run_rate,
            meter: FlipRateMeter::new(),
            refused_run_rate: None,
            writer: None,
            next_flush_ns: 0,
            message: String::with_capacity(MESSAGE_MAX_BYTES),
            message_frames: 0,
            flipless: 0,
            counters: Counters {
                refused_options: refused_options as u64,
                ..Counters::default()
            },
        };
        session.settle()?;

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
        session.next_flush_ns = session
            .clock
            .now_ns()
            .saturating_add(SAVE_RAM_FLUSH_INTERVAL_NS);
        Ok(session)
    }

    /// Runs core frames until the source settles or the core stops.
    fn settle(&mut self) -> Result<(), LaunchError> {
        let mut settle = LaunchSettle::default();
        loop {
            self.run_core(false).map_err(LaunchError::Run)?;
            self.counters.settle_frames += 1;
            if self.state == State::Ended || settle.push(self.source) {
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
    let path = game
        .content
        .and_then(|content| save_file_path(game.save_dir, content));
    let (Some(path), true) = (path, ram_bytes > 0) else {
        return Ok(None);
    };
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
