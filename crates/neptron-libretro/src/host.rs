//! The safe host: one core, one game, one thread.

use std::ffi::CString;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use neptron_libretro_sys::{RETRO_API_VERSION, RETRO_DEVICE_JOYPAD};
use neptron_types::PixelFormat;

use crate::content::{ContentRequest, ContentRules, LoadedContent, c_path};
use crate::error::{LoadError, OpenError, RunError};
use crate::ffi::{self, Core, CoreSystemInfo, OpenFailure, Poisoned};
use crate::geometry::AvInfo;
use crate::log::LogRecord;
use crate::options::{CoreOption, OptionError};
use crate::output::FrameOutput;
use crate::pad::{InputError, Pad};
use crate::save::{SaveRamError, SaveRamLoad};
use crate::state::State;

/// Set by the first [`Host::open`] of the process and never cleared.
static PROCESS_USED: AtomicBool = AtomicBool::new(false);

/// The ports that get `RETRO_DEVICE_JOYPAD` after the load, as RetroArch
/// does.
const JOYPAD_PORTS: u32 = 2;

/// The folders that the core asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostConfig {
    /// The BIOS folder, for `GET_SYSTEM_DIRECTORY`.
    pub system_dir: PathBuf,
    /// The save folder of the system (`state/saves/<system>/`), for
    /// `GET_SAVE_DIRECTORY`.
    pub save_dir: PathBuf,
}

/// What `retro_get_system_info` says about the core.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemInfo {
    /// The core name.
    pub name: String,
    /// The core version.
    pub version: String,
    /// The content extensions that the core takes, lower case, without
    /// dots.
    pub extensions: Vec<String>,
    /// The core reads content from its path and gets no data.
    pub need_fullpath: bool,
    /// The core wants archives as they are, not one entry of them.
    pub block_extract: bool,
}

/// A loaded libretro core.
///
/// One process opens one core, once. `Host` is neither `Send` nor `Sync`:
/// the thread that opens it makes every call into the core, as libretro.h
/// asks. Dropping it unloads the game and calls `retro_deinit`; the library
/// stays mapped until the process exits.
pub struct Host {
    core: Core,
    /// `None` only while it is lent to a call into the core, or after a
    /// call lost it.
    state: Option<Box<State>>,
    system_info: SystemInfo,
    initialized: bool,
    game_loaded: bool,
    /// The content of the loaded game. The core can keep pointers into it
    /// until `retro_unload_game`.
    content: Option<LoadedContent>,
    _one_thread: PhantomData<*const ()>,
}

impl Host {
    /// Opens the core at `path`: loads the library, checks the entry points
    /// and the API version, gives the callbacks, calls `retro_init` and
    /// reads the system info.
    pub fn open(path: &Path, config: HostConfig) -> Result<Self, OpenError> {
        if PROCESS_USED.swap(true, Ordering::SeqCst) {
            return Err(OpenError::ProcessAlreadyUsed);
        }
        let system_dir = config_path(&config.system_dir)?;
        let save_dir = config_path(&config.save_dir)?;
        let core = Core::open(path).map_err(|failure| match failure {
            OpenFailure::Library(reason) => OpenError::Library {
                path: path.to_owned(),
                reason,
            },
            OpenFailure::MissingSymbol(name) => OpenError::MissingSymbol {
                path: path.to_owned(),
                name,
            },
        })?;
        Self::start(core, State::new(system_dir, save_dir))
    }

    /// Checks the API version, gives the callbacks, calls `retro_init`,
    /// and reads the system info.
    pub(crate) fn start(core: Core, state: State) -> Result<Self, OpenError> {
        let found = core.api_version();
        if found != RETRO_API_VERSION {
            return Err(OpenError::ApiVersion {
                found,
                expected: RETRO_API_VERSION,
            });
        }
        ffi::install_log_sink();
        let mut host = Self {
            core,
            state: Some(Box::new(state)),
            system_info: SystemInfo::default(),
            initialized: false,
            game_loaded: false,
            content: None,
            _one_thread: PhantomData,
        };
        host.begin_call();
        host.call(|core| {
            core.set_callbacks();
            core.init();
        })
        .map_err(|Poisoned| OpenError::HostPoisoned)?;
        host.initialized = true;
        let raw = host
            .call(Core::system_info)
            .map_err(|Poisoned| OpenError::HostPoisoned)?;
        host.system_info = system_info(raw)?;
        Ok(host)
    }

    /// What the core says about itself.
    pub fn system_info(&self) -> &SystemInfo {
        &self.system_info
    }

    /// The options that the core declared, with their current values.
    pub fn options(&self) -> &[CoreOption] {
        self.state().map_or(&[], State::options)
    }

    /// Sets the value of a declared option. The core sees the change at its
    /// next `GET_VARIABLE_UPDATE`.
    pub fn set_option(&mut self, key: &str, value: &str) -> Result<(), OptionError> {
        match self.state.as_deref_mut() {
            Some(state) => state.set_option(key, value),
            None => Err(OptionError::UnknownKey {
                key: key.to_owned(),
            }),
        }
    }

    /// Loads content, or no content when the core supports that. Returns
    /// the geometry and timing of the game.
    pub fn load_game(&mut self, request: Option<ContentRequest<'_>>) -> Result<AvInfo, LoadError> {
        if self.game_loaded {
            return Err(LoadError::GameAlreadyLoaded);
        }
        let content = match request {
            Some(request) => Some(self.prepare(&request)?),
            None if self.state().is_some_and(State::supports_no_game) => None,
            None => return Err(LoadError::NoContent),
        };
        self.begin_call();
        let info_ext = content
            .as_ref()
            .map_or(std::ptr::null(), LoadedContent::info_ext);
        self.content = content;
        if let Some(state) = self.state.as_deref_mut() {
            state.set_game_info_ext(info_ext);
        }
        let core = &self.core;
        let content = self.content.as_ref();
        let loaded =
            ffi::lend(&mut self.state, || core.load_game(content)).map_err(poisoned_load)?;
        if !loaded {
            self.drop_content();
            return Err(LoadError::Refused);
        }
        self.game_loaded = true;
        let av = self.call(Core::system_av_info).map_err(poisoned_load)?;
        let av = match av.convert() {
            Ok(av) => av,
            Err(e) => {
                self.unload();
                return Err(LoadError::InvalidAvInfo(e));
            }
        };
        let buffers = match self.state.as_deref_mut() {
            Some(state) => state.set_loaded_av(av),
            None => return Err(LoadError::HostPoisoned),
        };
        if let Err(bytes) = buffers {
            self.unload();
            return Err(LoadError::OutOfMemory { bytes });
        }
        self.call(|core| {
            for port in 0..JOYPAD_PORTS {
                core.set_controller_port_device(port, RETRO_DEVICE_JOYPAD);
            }
        })
        .map_err(poisoned_load)?;
        Ok(av)
    }

    /// The geometry and timing in force, after `load_game`.
    pub fn av_info(&self) -> Option<AvInfo> {
        self.state().and_then(State::av)
    }

    /// The pixel format in force.
    pub fn pixel_format(&self) -> PixelFormat {
        self.state()
            .map_or(PixelFormat::Xrgb1555, State::pixel_format)
    }

    /// Sets what the core reads from `port` in the next frame.
    pub fn set_pad(&mut self, port: usize, pad: Pad) -> Result<(), InputError> {
        let set = self
            .state
            .as_deref_mut()
            .is_some_and(|state| state.set_pad(port, pad));
        if set {
            Ok(())
        } else {
            Err(InputError::NoSuchPort { port })
        }
    }

    /// Runs one frame of the core and returns what it produced. The output
    /// borrows the host until the next call.
    pub fn run_frame(&mut self) -> Result<FrameOutput<'_>, RunError> {
        if !self.game_loaded {
            return Err(RunError::NoGame);
        }
        let state = self.state.as_deref_mut().ok_or(RunError::HostPoisoned)?;
        state.begin_call();
        state.begin_run();
        self.call(Core::run)
            .map_err(|Poisoned| RunError::HostPoisoned)?;
        let state = self.state.as_deref_mut().ok_or(RunError::HostPoisoned)?;
        state.end_run();
        Ok(state.output())
    }

    /// Calls `retro_reset`.
    pub fn reset(&mut self) -> Result<(), RunError> {
        if !self.game_loaded {
            return Err(RunError::NoGame);
        }
        self.begin_call();
        self.call(Core::reset)
            .map_err(|Poisoned| RunError::HostPoisoned)
    }

    /// Copies the save RAM of the core into `out` and returns its length.
    /// 0 means the core has no save RAM. The session writes `out` with
    /// [`write_save_file`](crate::write_save_file) off the frame thread.
    pub fn copy_save_ram(&mut self, out: &mut Vec<u8>) -> Result<usize, SaveRamError> {
        if !self.game_loaded {
            return Err(SaveRamError::NoGame);
        }
        self.call(|core| core.read_save_ram(out))
            .map_err(|Poisoned| SaveRamError::HostPoisoned)?
    }

    /// Copies `bytes` (a save file) into the save RAM of the core, after
    /// `load_game`. A length that differs from the core's loads the smaller
    /// of the two.
    pub fn load_save_ram(&mut self, bytes: &[u8]) -> Result<SaveRamLoad, SaveRamError> {
        if !self.game_loaded {
            return Err(SaveRamError::NoGame);
        }
        self.call(|core| core.write_save_ram(bytes))
            .map_err(|Poisoned| SaveRamError::HostPoisoned)?
    }

    /// The log records of the last call into the core.
    pub fn logs(&self) -> &[LogRecord] {
        self.state().map_or(&[], State::logs)
    }

    fn prepare(&self, request: &ContentRequest<'_>) -> Result<LoadedContent, LoadError> {
        let rules = ContentRules {
            need_fullpath: self.system_info.need_fullpath,
            block_extract: self.system_info.block_extract,
            overrides: self.state().map_or(&[], State::content_overrides),
        };
        LoadedContent::prepare(request, &rules)
    }

    fn unload(&mut self) {
        if self.game_loaded {
            self.call_for_teardown(Core::unload_game);
            self.game_loaded = false;
        }
        self.drop_content();
    }

    /// Clears the state's pointer to the content before the content goes.
    fn drop_content(&mut self) {
        if let Some(state) = self.state.as_deref_mut() {
            state.set_game_info_ext(std::ptr::null());
        }
        self.content = None;
    }

    /// Starts a public call that is not a frame.
    fn begin_call(&mut self) {
        if let Some(state) = self.state.as_deref_mut() {
            state.begin_call();
            state.begin_outside_run();
        }
    }

    fn state(&self) -> Option<&State> {
        self.state.as_deref()
    }

    /// Calls into the core for teardown, also when the state is lost: then
    /// the call runs with no state lent, and every callback returns its
    /// default. The core must never keep a game loaded after its content is
    /// freed.
    fn call_for_teardown(&mut self, f: impl Fn(&Core)) {
        let core = &self.core;
        let mut ran = false;
        // A state lost after the call is reported by `ran`; nothing else can
        // use the error here.
        let _ = ffi::lend(&mut self.state, || {
            ran = true;
            f(core);
        });
        if !ran {
            f(core);
        }
    }

    /// Drops the state, as a call that loses it does. For tests only.
    #[cfg(test)]
    pub(crate) fn lose_state(&mut self) {
        self.state = None;
    }

    /// Calls into the core with the state lent to the callbacks.
    fn call<R>(&mut self, f: impl FnOnce(&Core) -> R) -> Result<R, Poisoned> {
        let core = &self.core;
        ffi::lend(&mut self.state, || f(core))
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.unload();
        if self.initialized {
            self.call_for_teardown(Core::deinit);
        }
    }
}

fn poisoned_load(_: Poisoned) -> LoadError {
    LoadError::HostPoisoned
}

fn config_path(path: &Path) -> Result<CString, OpenError> {
    c_path(path).map_err(|_| OpenError::InvalidPath {
        path: path.to_owned(),
    })
}

fn system_info(raw: CoreSystemInfo) -> Result<SystemInfo, OpenError> {
    let text = |bytes: Option<Vec<u8>>, field| {
        bytes
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .ok_or(OpenError::InvalidSystemInfo { field })
    };
    let extensions = text(raw.extensions, "valid_extensions")?;
    Ok(SystemInfo {
        name: text(raw.name, "library_name")?,
        version: text(raw.version, "library_version")?,
        extensions: extensions
            .split('|')
            .filter(|e| !e.is_empty())
            .map(str::to_ascii_lowercase)
            .collect(),
        need_fullpath: raw.need_fullpath,
        block_extract: raw.block_extract,
    })
}
