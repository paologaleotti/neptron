//! The SDL3 audio sink, in SDL's push model: the frame thread puts samples
//! into an SDL audio stream, and SDL's own device thread plays them. No
//! code of this crate runs on the audio thread.

use neptron_backend::AudioSink;
use sdl3::audio::{AudioDevice, AudioDeviceID, AudioFormat, AudioSpec, AudioStreamOwner};

use crate::alsa::{AlsaText, alsa_outputs, open_name};
use crate::{
    ALSA_DRIVER, AudioOutputs, Monitor, OutputDevice, OutputError, OutputKind, check_device_rate,
};

/// The most frames the sink queues, in milliseconds: twice the control
/// span. SDL's stream has no limit of its own.
pub const SINK_CAPACITY_MS: u32 = 128;

/// The device period SDL is asked for (`SDL_AUDIO_DEVICE_SAMPLE_FRAMES`),
/// in stereo frames: 21.3 ms at 48 kHz, 23.2 ms at 44.1 kHz, and SDL's own
/// default at those rates. SDL's ALSA backend makes a buffer of two such
/// periods. The analog output of the Raspberry Pi 3 (`bcm2835`) runs short
/// of data many times a second with periods of 512 frames, while SDL's
/// stream still holds audio.
pub const DEVICE_PERIOD_FRAMES: u32 = 1024;

/// The earlier name of [`DEVICE_PERIOD_FRAMES`], kept for its callers: the
/// value is one device period, not the whole device buffer.
pub const DEVICE_BUFFER_FRAMES: u32 = DEVICE_PERIOD_FRAMES;

/// The SDL hint that names the ALSA PCM of SDL's default playback device.
/// SDL reads it when the device opens (`ALSA_OpenDevice`).
const ALSA_PLAYBACK_HINT: &str = "SDL_AUDIO_ALSA_DEFAULT_PLAYBACK_DEVICE";
/// The ALSA PCM of the system default device.
const ALSA_DEFAULT_PCM: &str = "default";
/// Where the kernel shows the sound cards.
const PROC_ASOUND: &str = "/proc/asound";

/// The rate used when SDL does not report the device's rate, in hertz.
const FALLBACK_RATE_HZ: u32 = 48_000;
/// Bytes of one stereo 16-bit frame.
const BYTES_PER_FRAME: usize = 4;

/// An error of [`Sdl3Sink::open`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SinkError {
    /// SDL or its audio subsystem did not start.
    #[error("SDL could not start its audio subsystem: {reason}")]
    Init {
        /// SDL's message.
        reason: String,
    },
    /// The default playback device did not open.
    #[error("SDL could not open the default playback device: {reason}")]
    Open {
        /// SDL's message.
        reason: String,
    },
    /// No playback device has this name.
    #[error("SDL lists no playback device named \"{name}\"")]
    NotFound {
        /// The name asked for.
        name: String,
    },
    /// The named playback device did not open.
    #[error("SDL could not open the playback device \"{name}\": {reason}")]
    OpenNamed {
        /// The name of the device.
        name: String,
        /// SDL's message.
        reason: String,
    },
    /// The device reports a rate the pipeline does not take.
    #[error("the playback device runs at {rate_hz} Hz, which the pipeline does not take")]
    Rate {
        /// The device's rate, in hertz.
        rate_hz: u32,
    },
    /// The audio stream could not be made or bound to the device.
    #[error("SDL could not make the audio stream at {rate_hz} Hz: {reason}")]
    Stream {
        /// The stream's rate, in hertz.
        rate_hz: u32,
        /// SDL's message.
        reason: String,
    },
}

/// A playback device, through an SDL3 audio stream of 16-bit
/// stereo at the device's own rate, so that SDL does not resample.
///
/// The queue holds at most [`SINK_CAPACITY_MS`]. An SDL error makes the
/// sink act as full: `write` takes nothing and `queued_frames` reports the
/// capacity.
///
/// Make it on the thread that first started SDL (the `sdl3` crate checks
/// this); it is not `Send`.
pub struct Sdl3Sink {
    // Fields drop in order: the stream unbinds, then the device closes,
    // then the subsystem and SDL stop.
    stream: AudioStreamOwner,
    /// The device of the default path. A named device belongs to its
    /// stream, which closes it.
    _device: Option<AudioDevice>,
    _audio: sdl3::AudioSubsystem,
    _sdl: sdl3::Sdl,
    rate_hz: u32,
    capacity_frames: usize,
    device_buffer_frames: Option<u32>,
}

impl Sdl3Sink {
    /// Starts the SDL audio subsystem only, opens the default playback
    /// device, and binds a stream at the device's rate.
    ///
    /// # Errors
    ///
    /// A [`SinkError`] with SDL's message when a step fails.
    pub fn open() -> Result<Self, SinkError> {
        let init = |error: sdl3::Error| SinkError::Init {
            reason: error.to_string(),
        };
        let sdl = sdl3::init().map_err(init)?;
        let audio = sdl.audio().map_err(init)?;
        Self::open_on(&sdl, &audio, None)
    }

    /// Opens the default device (`None`) or the playback device with this
    /// name on an SDL context that runs, and binds a stream at the
    /// device's rate.
    fn open_on(
        sdl: &sdl3::Sdl,
        audio: &sdl3::AudioSubsystem,
        name: Option<&str>,
    ) -> Result<Self, SinkError> {
        // SDL reads the hint when a device opens. A false return means that
        // an environment variable already sets it, which is the owner's
        // choice.
        let _hint_set = sdl3::hint::set(
            "SDL_AUDIO_DEVICE_SAMPLE_FRAMES",
            &DEVICE_PERIOD_FRAMES.to_string(),
        );
        match name {
            None => Self::open_default(sdl, audio),
            Some(name) => Self::open_named(sdl, audio, name),
        }
    }

    fn open_default(sdl: &sdl3::Sdl, audio: &sdl3::AudioSubsystem) -> Result<Self, SinkError> {
        let request = AudioSpec::new(None, Some(2), Some(AudioFormat::s16_sys()));
        let device = audio
            .open_playback_device(&request)
            .map_err(|error| SinkError::Open {
                reason: error.to_string(),
            })?;
        let (device_spec, device_buffer_frames) =
            device.format().map_err(|error| SinkError::Open {
                reason: error.to_string(),
            })?;
        let rate_hz = device_rate(&device_spec)?;
        let stream_error = |error: sdl3::Error| SinkError::Stream {
            rate_hz,
            reason: error.to_string(),
        };
        let stream = audio
            .new_playback_stream(&stream_spec(rate_hz), None)
            .map_err(stream_error)?;
        device.bind_stream(&stream).map_err(stream_error)?;
        // A device from SDL_OpenAudioDevice starts playing; resuming it
        // again does nothing.
        let _playing = device.resume();
        Ok(Self::with_parts(
            stream,
            Some(device),
            audio,
            sdl,
            rate_hz,
            device_buffer_frames,
        ))
    }

    fn open_named(
        sdl: &sdl3::Sdl,
        audio: &sdl3::AudioSubsystem,
        name: &str,
    ) -> Result<Self, SinkError> {
        let open_error = |error: sdl3::Error| SinkError::OpenNamed {
            name: name.to_owned(),
            reason: error.to_string(),
        };
        let id = playback_devices(audio)
            .map_err(open_error)?
            .into_iter()
            .find(|(_, n)| n == name)
            .map(|(id, _)| id)
            .ok_or_else(|| SinkError::NotFound {
                name: name.to_owned(),
            })?;
        // A handle on the physical device: its format, then a stream that
        // opens a logical device on it. Dropping a handle of a physical
        // device closes nothing in SDL.
        let physical = AudioDevice::new(id, audio.clone());
        let (device_spec, device_buffer_frames) = physical.format().map_err(open_error)?;
        let rate_hz = device_rate(&device_spec)?;
        let stream = physical
            .open_device_stream(Some(&stream_spec(rate_hz)))
            .map_err(|error| SinkError::Stream {
                rate_hz,
                reason: error.to_string(),
            })?;
        // SDL_OpenAudioDeviceStream leaves the device paused.
        stream.resume().map_err(open_error)?;
        Ok(Self::with_parts(
            stream,
            None,
            audio,
            sdl,
            rate_hz,
            device_buffer_frames,
        ))
    }

    fn with_parts(
        stream: AudioStreamOwner,
        device: Option<AudioDevice>,
        audio: &sdl3::AudioSubsystem,
        sdl: &sdl3::Sdl,
        rate_hz: u32,
        device_buffer_frames: Option<i32>,
    ) -> Self {
        let capacity = u64::from(rate_hz) * u64::from(SINK_CAPACITY_MS) / 1_000;
        Self {
            stream,
            _device: device,
            _audio: audio.clone(),
            _sdl: sdl.clone(),
            rate_hz,
            capacity_frames: usize::try_from(capacity).unwrap_or(usize::MAX),
            device_buffer_frames: device_buffer_frames.and_then(|f| u32::try_from(f).ok()),
        }
    }

    /// The device buffer SDL reports, in frames: the delay after the
    /// stream's queue.
    pub fn device_buffer_frames(&self) -> Option<u32> {
        self.device_buffer_frames
    }

    /// The most frames the queue takes.
    pub fn capacity_frames(&self) -> usize {
        self.capacity_frames
    }

    fn queued(&self) -> Option<usize> {
        let bytes = self.stream.queued_bytes().ok()?;
        usize::try_from(bytes)
            .ok()
            .map(|bytes| bytes / BYTES_PER_FRAME)
    }
}

impl AudioSink for Sdl3Sink {
    fn rate_hz(&self) -> u32 {
        self.rate_hz
    }

    fn queued_frames(&self) -> usize {
        self.queued().unwrap_or(self.capacity_frames)
    }

    fn write(&mut self, stereo: &[i16]) -> usize {
        let Some(queued) = self.queued() else {
            return 0;
        };
        let taken = (stereo.len() / 2).min(self.capacity_frames.saturating_sub(queued));
        if taken == 0 {
            return 0;
        }
        match self.stream.put_data_i16(&stereo[..taken * 2]) {
            Ok(()) => taken,
            Err(_) => 0,
        }
    }
}

/// The playback devices of SDL's audio backend, and the open of one of
/// them. Make it on the thread that first started SDL; it is not `Send`.
///
/// With the ALSA driver the list is NEPTRON's own, read from
/// `/proc/asound`, because SDL's ALSA list opens the card's default PCM for
/// each of its entries. A device opens as SDL's default device with the
/// hint that names its `plughw:` PCM. With another driver the list and the
/// open are SDL's.
pub struct Sdl3Outputs {
    audio: sdl3::AudioSubsystem,
    sdl: sdl3::Sdl,
}

impl Sdl3Outputs {
    /// Starts SDL's audio subsystem only. SDL reads the hint only when a
    /// device opens, so `open` sets it then.
    ///
    /// # Errors
    ///
    /// [`SinkError::Init`] with SDL's message.
    pub fn start() -> Result<Self, SinkError> {
        let init = |error: sdl3::Error| SinkError::Init {
            reason: error.to_string(),
        };
        let sdl = sdl3::init().map_err(init)?;
        let audio = sdl.audio().map_err(init)?;
        Ok(Self { audio, sdl })
    }

    fn is_alsa(&self) -> bool {
        self.audio.current_audio_driver() == ALSA_DRIVER
    }
}

impl AudioOutputs for Sdl3Outputs {
    type Sink = Sdl3Sink;

    fn driver(&self) -> &str {
        self.audio.current_audio_driver()
    }

    fn devices(&mut self) -> Result<Vec<OutputDevice>, OutputError> {
        if self.is_alsa() {
            return Ok(alsa_outputs(&AlsaText::read(std::path::Path::new(
                PROC_ASOUND,
            ))));
        }
        let devices = playback_devices(&self.audio).map_err(|error| OutputError::List {
            reason: error.to_string(),
        })?;
        Ok(devices
            .into_iter()
            .map(|(_, name)| OutputDevice {
                label: name.clone(),
                name,
                detail: String::new(),
                monitor: Monitor::NotDigital,
                kind: OutputKind::Other,
            })
            .collect())
    }

    fn open(&mut self, device: Option<&str>) -> Result<Sdl3Sink, OutputError> {
        let alsa = self.is_alsa();
        let result = if alsa {
            set_alsa_pcm(&device.map_or_else(|| ALSA_DEFAULT_PCM.to_owned(), open_name));
            Sdl3Sink::open_on(&self.sdl, &self.audio, None)
        } else {
            Sdl3Sink::open_on(&self.sdl, &self.audio, device)
        };
        result.map_err(|error| match error {
            SinkError::NotFound { name } => OutputError::NotFound { name },
            other => OutputError::Open {
                device: device.unwrap_or(ALSA_DEFAULT_PCM).to_owned(),
                reason: other.to_string(),
            },
        })
    }
}

/// Names the ALSA PCM that SDL's default playback device opens.
fn set_alsa_pcm(pcm: &str) {
    // A false return means that the environment variable of the same name
    // is set: SDL gives it priority over a hint set in code, so a person on
    // the bench can force a PCM.
    let _hint_set = sdl3::hint::set(ALSA_PLAYBACK_HINT, pcm);
}

/// The playback devices with their names. A device whose name SDL cannot
/// give is left out.
fn playback_devices(
    audio: &sdl3::AudioSubsystem,
) -> Result<Vec<(AudioDeviceID, String)>, sdl3::Error> {
    Ok(audio
        .audio_playback_device_ids()?
        .into_iter()
        .filter_map(|id| id.name().ok().map(|name| (id, name)))
        .collect())
}

/// The rate of a device's format, checked against the pipeline's limits.
fn device_rate(spec: &AudioSpec) -> Result<u32, SinkError> {
    let rate_hz = spec
        .freq
        .and_then(|freq| u32::try_from(freq).ok())
        .unwrap_or(FALLBACK_RATE_HZ);
    check_device_rate(rate_hz).map_err(|_| SinkError::Rate { rate_hz })?;
    Ok(rate_hz)
}

/// The format the frame thread writes: 16-bit stereo at `rate_hz`.
fn stream_spec(rate_hz: u32) -> AudioSpec {
    // `rate_hz` passed the device rate check, so it fits an i32.
    let freq = i32::try_from(rate_hz).unwrap_or(i32::MAX);
    AudioSpec::new(Some(freq), Some(2), Some(AudioFormat::s16_sys()))
}
