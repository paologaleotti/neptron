//! The script format of the test core and its parser.
//!
//! The README of the crate describes the format. The parser checks every
//! value against the limits below, so the core never meets a value it cannot
//! handle.

#![forbid(unsafe_code)]

use std::path::PathBuf;

use crate::options::option_index;

/// Environment variable that names the script file.
pub const SCRIPT_ENV: &str = "NEPTRON_TESTCORE_SCRIPT";
/// The keyword of the first line: `neptron-testcore <version>`.
pub const HEADER_KEYWORD: &str = "neptron-testcore";
/// The newest script version that the parser reads.
pub const SCRIPT_VERSION: u32 = 1;
/// Largest frame width or height, in pixels.
pub const MAX_DIMENSION_PX: u32 = 4096;
/// Smallest frame rate, in millihertz. It bounds the audio of one frame, and
/// so the audio buffer.
pub const MIN_FPS_MHZ: u32 = 1_000;
/// Largest frame rate, in millihertz.
pub const MAX_FPS_MHZ: u32 = 1_000_000;
/// Largest audio sample rate, in hertz.
pub const MAX_SAMPLE_RATE_HZ: u32 = 384_000;
/// Largest aspect ratio, in thousandths.
pub const MAX_ASPECT_MILLI: u32 = 100_000;
/// Largest save RAM, in bytes.
pub const MAX_SAVE_RAM_BYTES: u32 = 16 * 1024 * 1024;

/// A pixel format of the libretro API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// 32 bits per pixel, `0x00RRGGBB`.
    Xrgb8888,
    /// 16 bits per pixel, 5 bits red, 6 bits green, 5 bits blue.
    Rgb565,
    /// 16 bits per pixel, 5 bits per channel, top bit unused. The libretro
    /// default: the core does not ask the frontend for it.
    Rgb1555,
}

impl PixelFormat {
    /// Bytes per pixel.
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Xrgb8888 => 4,
            Self::Rgb565 | Self::Rgb1555 => 2,
        }
    }

    /// The bits that carry colour. The core leaves every other bit zero.
    pub fn colour_mask(self) -> u32 {
        match self {
            Self::Xrgb8888 => 0x00FF_FFFF,
            Self::Rgb565 => 0xFFFF,
            Self::Rgb1555 => 0x7FFF,
        }
    }
}

/// A frame size in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Size {
    /// True when `self` fits inside `max` in both directions.
    pub fn fits(self, max: Size) -> bool {
        self.width <= max.width && self.height <= max.height
    }
}

/// The base geometry that the core reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    /// The base frame size.
    pub size: Size,
    /// Display aspect ratio in thousandths. 0 means width over height.
    pub aspect_milli: u32,
}

/// The timing that the core reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// Frame rate in millihertz.
    pub fps_mhz: u32,
    /// Audio sample rate in hertz.
    pub sample_rate_hz: u32,
}

/// How the core sends its audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioMode {
    /// One `audio_sample_batch` call per frame.
    Batch,
    /// One `audio_sample` call per stereo frame.
    Single,
    /// No audio.
    Off,
}

/// Geometry, maximum size and timing, as `SET_SYSTEM_AV_INFO` sends them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvInfo {
    /// The base geometry.
    pub geometry: Geometry,
    /// The maximum frame size.
    pub max: Size,
    /// The timing.
    pub timing: Timing,
}

/// One thing the core does at a given frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Change the geometry with `SET_GEOMETRY`. Frames take the new size.
    Geometry(Geometry),
    /// Change geometry, maximum size and timing with `SET_SYSTEM_AV_INFO`.
    AvInfo(AvInfo),
    /// Change the size of the frames and tell the frontend nothing.
    Size(Size),
    /// Send no frame data (a duplicate frame) for this frame.
    Dupe,
    /// Write one byte of save RAM.
    SramWrite {
        /// Offset in bytes.
        offset: u32,
        /// The value.
        value: u8,
    },
    /// Ask the frontend to shut down with `RETRO_ENVIRONMENT_SHUTDOWN`.
    Shutdown,
    /// Send an environment command that no frontend knows, with null data.
    UnknownEnv,
    /// Read a core option with `GET_VARIABLE` and write the answer code
    /// (`value_code`) to one byte of save RAM.
    GetVariable {
        /// The index of the option in `CORE_OPTIONS`.
        option: usize,
        /// The save RAM offset in bytes.
        offset: u32,
    },
}

/// An action and the frame it happens at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    /// The frame number. Frame 0 is the first `retro_run` after a load or a
    /// reset.
    pub frame: u64,
    /// What happens.
    pub action: Action,
}

/// A parsed and checked script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    /// The pixel format.
    pub pixel_format: PixelFormat,
    /// Geometry, maximum size and timing at load.
    pub av: AvInfo,
    /// How the core sends audio.
    pub audio: AudioMode,
    /// Size of the save RAM in bytes. 0 means no save RAM.
    pub save_ram_bytes: u32,
    /// The events, ordered by frame.
    pub events: Vec<Event>,
}

/// An error in a script. Line numbers count from 1.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScriptError {
    /// The script is not valid UTF-8.
    #[error("line {line}: the script is not valid UTF-8")]
    NotUtf8 {
        /// The line of the first byte that is not UTF-8.
        line: usize,
    },
    /// The script has no header line.
    #[error("the script is empty; the first line must be `{HEADER_KEYWORD} <version>`")]
    Empty,
    /// The first line is not a header.
    #[error("line {line}: expected `{HEADER_KEYWORD} <version>`, found {found:?}")]
    Header {
        /// The line.
        line: usize,
        /// The text of the line.
        found: String,
    },
    /// The version is newer than this parser.
    #[error("line {line}: script version {version} is not supported (newest: {SCRIPT_VERSION})")]
    Version {
        /// The line.
        line: usize,
        /// The version in the header.
        version: u32,
    },
    /// A line starts with an unknown keyword.
    #[error("line {line}: unknown keyword {keyword:?}")]
    UnknownKeyword {
        /// The line.
        line: usize,
        /// The keyword.
        keyword: String,
    },
    /// A line has the wrong number of arguments.
    #[error("line {line}: `{keyword}` takes {expected} arguments, found {found}")]
    Arguments {
        /// The line.
        line: usize,
        /// The keyword.
        keyword: String,
        /// The number of arguments it takes.
        expected: usize,
        /// The number of arguments on the line.
        found: usize,
    },
    /// An argument is not an unsigned integer, or not a known name.
    #[error("line {line}: {token:?} is not a valid {what}")]
    Value {
        /// The line.
        line: usize,
        /// What the argument is.
        what: &'static str,
        /// The text of the argument.
        token: String,
    },
    /// A value is outside its limits.
    #[error("line {line}: {what} {value} is outside {min}..={max}")]
    OutOfRange {
        /// The line.
        line: usize,
        /// What the value is.
        what: &'static str,
        /// The value.
        value: u64,
        /// The smallest allowed value.
        min: u64,
        /// The largest allowed value.
        max: u64,
    },
    /// A frame size does not fit the maximum size in force at that point.
    #[error("line {line}: size {width}x{height} does not fit the maximum {max_width}x{max_height}")]
    TooLarge {
        /// The line.
        line: usize,
        /// The width.
        width: u32,
        /// The height.
        height: u32,
        /// The maximum width.
        max_width: u32,
        /// The maximum height.
        max_height: u32,
    },
    /// A setup keyword appears twice.
    #[error("line {line}: `{keyword}` appears a second time")]
    Duplicate {
        /// The line.
        line: usize,
        /// The keyword.
        keyword: String,
    },
    /// A setup keyword appears after the first `at` line.
    #[error("line {line}: `{keyword}` must come before the first `at` line")]
    SetupAfterEvent {
        /// The line.
        line: usize,
        /// The keyword.
        keyword: String,
    },
    /// An event has a smaller frame number than the event before it.
    #[error("line {line}: frame {frame} comes after frame {previous}")]
    FrameOrder {
        /// The line.
        line: usize,
        /// The frame of this event.
        frame: u64,
        /// The frame of the event before it.
        previous: u64,
    },
    /// A `sram_write` offset is outside the save RAM.
    #[error("line {line}: offset {offset} is outside the save RAM of {size} bytes")]
    SramOffset {
        /// The line.
        line: usize,
        /// The offset.
        offset: u32,
        /// The size of the save RAM.
        size: u32,
    },
}

/// An error while the core loads its script.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// The environment variable is not set.
    #[error("{SCRIPT_ENV} is not set; it must name the script file")]
    NotSet,
    /// The file cannot be read.
    #[error("cannot read the script {path}: {source}")]
    Read {
        /// The script file.
        path: PathBuf,
        /// The error of the operating system.
        source: std::io::Error,
    },
    /// The file is not a valid script.
    #[error("script {path}: {source}")]
    Parse {
        /// The script file.
        path: PathBuf,
        /// The error in the script.
        source: ScriptError,
    },
}

impl Script {
    /// Parses and checks a script.
    pub fn parse(text: &str) -> Result<Self, ScriptError> {
        let mut lines = text
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, strip_comment(l)))
            .filter(|(_, l)| !l.is_empty());
        let (line, header) = lines.next().ok_or(ScriptError::Empty)?;
        parse_header(line, header)?;
        let mut builder = Builder::default();
        for (line, text) in lines {
            builder.line(line, text)?;
        }
        builder.finish()
    }

    /// Checks that `bytes` are UTF-8, then parses them as [`Script::parse`]
    /// does.
    pub fn parse_bytes(bytes: &[u8]) -> Result<Self, ScriptError> {
        let text = std::str::from_utf8(bytes).map_err(|e| {
            let valid = bytes.get(..e.valid_up_to()).unwrap_or_default();
            ScriptError::NotUtf8 {
                line: 1 + valid.iter().filter(|&&b| b == b'\n').count(),
            }
        })?;
        Self::parse(text)
    }

    /// Reads and parses the script that `NEPTRON_TESTCORE_SCRIPT` names.
    pub fn from_env() -> Result<Self, LoadError> {
        let path = std::env::var_os(SCRIPT_ENV).ok_or(LoadError::NotSet)?;
        Self::from_file(PathBuf::from(path))
    }

    /// Reads and parses a script file.
    pub fn from_file(path: PathBuf) -> Result<Self, LoadError> {
        let bytes = std::fs::read(&path).map_err(|source| LoadError::Read {
            path: path.clone(),
            source,
        })?;
        Self::parse_bytes(&bytes).map_err(|source| LoadError::Parse { path, source })
    }

    /// The largest maximum size in the script: the size of the frame buffer
    /// that the core allocates at load.
    pub fn buffer_size(&self) -> Size {
        self.events
            .iter()
            .filter_map(|e| match e.action {
                Action::AvInfo(av) => Some(av.max),
                _ => None,
            })
            .fold(self.av.max, |acc, max| Size {
                width: acc.width.max(max.width),
                height: acc.height.max(max.height),
            })
    }
}

/// Parses an unsigned decimal integer: ASCII digits only. `str::parse` also
/// takes a leading `+`, which the format does not allow.
fn parse_decimal(token: &str) -> Option<u64> {
    if token.is_empty() || !token.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    token.parse().ok()
}

fn strip_comment(line: &str) -> &str {
    line.split('#').next().unwrap_or_default().trim()
}

fn parse_header(line: usize, text: &str) -> Result<(), ScriptError> {
    let header_error = || ScriptError::Header {
        line,
        found: text.to_owned(),
    };
    let mut tokens = text.split_whitespace();
    if tokens.next() != Some(HEADER_KEYWORD) {
        return Err(header_error());
    }
    let (Some(token), None) = (tokens.next(), tokens.next()) else {
        return Err(header_error());
    };
    let version = parse_decimal(token)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(header_error)?;
    if version == 0 || version > SCRIPT_VERSION {
        return Err(ScriptError::Version { line, version });
    }
    Ok(())
}

/// The setup values seen so far. `None` means not given.
#[derive(Default)]
struct Builder {
    pixel_format: Option<PixelFormat>,
    geometry: Option<Geometry>,
    max: Option<Size>,
    timing: Option<Timing>,
    audio: Option<AudioMode>,
    save_ram_bytes: Option<u32>,
    max_line: usize,
    events: Vec<(usize, Event)>,
}

const DEFAULT_GEOMETRY: Geometry = Geometry {
    size: Size {
        width: 320,
        height: 240,
    },
    aspect_milli: 0,
};
const DEFAULT_TIMING: Timing = Timing {
    fps_mhz: 60_000,
    sample_rate_hz: 48_000,
};

impl Builder {
    fn line(&mut self, line: usize, text: &str) -> Result<(), ScriptError> {
        let mut tokens = text.split_whitespace();
        // `text` is not empty after trimming, so it has a first token.
        let keyword = tokens.next().unwrap_or_default();
        let args: Vec<&str> = tokens.collect();
        if keyword == "at" {
            return self.event(line, &args);
        }
        if !self.events.is_empty() && SETUP_KEYWORDS.contains(&keyword) {
            return Err(ScriptError::SetupAfterEvent {
                line,
                keyword: keyword.to_owned(),
            });
        }
        let a = Args {
            line,
            keyword,
            args: &args,
        };
        match keyword {
            "pixel_format" => set_once(&mut self.pixel_format, &a, |a| {
                a.count(1)?;
                a.pixel_format(0)
            }),
            "geometry" => set_once(&mut self.geometry, &a, |a| {
                a.count(3)?;
                a.geometry(0)
            }),
            "max_size" => {
                self.max_line = line;
                set_once(&mut self.max, &a, |a| {
                    a.count(2)?;
                    a.size(0)
                })
            }
            "timing" => set_once(&mut self.timing, &a, |a| {
                a.count(2)?;
                a.timing(0)
            }),
            "audio" => set_once(&mut self.audio, &a, |a| {
                a.count(1)?;
                a.audio_mode(0)
            }),
            "save_ram" => set_once(&mut self.save_ram_bytes, &a, |a| {
                a.count(1)?;
                a.number(0, "save RAM size", 0, MAX_SAVE_RAM_BYTES)
            }),
            _ => Err(ScriptError::UnknownKeyword {
                line,
                keyword: keyword.to_owned(),
            }),
        }
    }

    fn event(&mut self, line: usize, args: &[&str]) -> Result<(), ScriptError> {
        let Some((frame_token, rest)) = args.split_first() else {
            return Err(ScriptError::Arguments {
                line,
                keyword: "at".to_owned(),
                expected: 2,
                found: 0,
            });
        };
        let frame = parse_decimal(frame_token).ok_or_else(|| ScriptError::Value {
            line,
            what: "frame number",
            token: (*frame_token).to_owned(),
        })?;
        if let Some((_, previous)) = self.events.last()
            && frame < previous.frame
        {
            return Err(ScriptError::FrameOrder {
                line,
                frame,
                previous: previous.frame,
            });
        }
        let Some((keyword, args)) = rest.split_first() else {
            return Err(ScriptError::Arguments {
                line,
                keyword: "at".to_owned(),
                expected: 2,
                found: 1,
            });
        };
        let a = Args {
            line,
            keyword,
            args,
        };
        let action = match *keyword {
            "geometry" => {
                a.count(3)?;
                Action::Geometry(a.geometry(0)?)
            }
            "av_info" => {
                a.count(7)?;
                Action::AvInfo(AvInfo {
                    geometry: a.geometry(0)?,
                    max: a.size(3)?,
                    timing: a.timing(5)?,
                })
            }
            "size" => {
                a.count(2)?;
                Action::Size(a.size(0)?)
            }
            "dupe" => {
                a.count(0)?;
                Action::Dupe
            }
            "sram_write" => {
                a.count(2)?;
                Action::SramWrite {
                    offset: a.number(0, "save RAM offset", 0, MAX_SAVE_RAM_BYTES)?,
                    value: u8::try_from(a.number(1, "byte value", 0, 255)?)
                        .map_err(|_| a.value_error(1, "byte value"))?,
                }
            }
            "unknown_env" => {
                a.count(0)?;
                Action::UnknownEnv
            }
            "get_variable" => {
                a.count(2)?;
                Action::GetVariable {
                    option: option_index(a.token(0))
                        .ok_or_else(|| a.value_error(0, "core option key"))?,
                    offset: a.number(1, "save RAM offset", 0, MAX_SAVE_RAM_BYTES)?,
                }
            }
            "shutdown" => {
                a.count(0)?;
                Action::Shutdown
            }
            other => {
                return Err(ScriptError::UnknownKeyword {
                    line,
                    keyword: other.to_owned(),
                });
            }
        };
        self.events.push((line, Event { frame, action }));
        Ok(())
    }

    fn finish(self) -> Result<Script, ScriptError> {
        let geometry = self.geometry.unwrap_or(DEFAULT_GEOMETRY);
        let max = self.max.unwrap_or(geometry.size);
        let save_ram_bytes = self.save_ram_bytes.unwrap_or(0);
        // Without a `max_size` line the maximum is the geometry, which fits.
        check_fits(self.max_line, geometry.size, max)?;
        let mut current_max = max;
        for (line, event) in &self.events {
            match event.action {
                Action::Geometry(g) => check_fits(*line, g.size, current_max)?,
                Action::Size(s) => check_fits(*line, s, current_max)?,
                Action::AvInfo(av) => {
                    check_fits(*line, av.geometry.size, av.max)?;
                    current_max = av.max;
                }
                Action::SramWrite { offset, .. } | Action::GetVariable { offset, .. }
                    if offset >= save_ram_bytes =>
                {
                    return Err(ScriptError::SramOffset {
                        line: *line,
                        offset,
                        size: save_ram_bytes,
                    });
                }
                _ => {}
            }
        }
        Ok(Script {
            pixel_format: self.pixel_format.unwrap_or(PixelFormat::Xrgb8888),
            av: AvInfo {
                geometry,
                max,
                timing: self.timing.unwrap_or(DEFAULT_TIMING),
            },
            audio: self.audio.unwrap_or(AudioMode::Batch),
            save_ram_bytes,
            events: self.events.into_iter().map(|(_, e)| e).collect(),
        })
    }
}

const SETUP_KEYWORDS: [&str; 6] = [
    "pixel_format",
    "geometry",
    "max_size",
    "timing",
    "audio",
    "save_ram",
];

fn set_once<T>(
    slot: &mut Option<T>,
    a: &Args<'_>,
    parse: impl FnOnce(&Args<'_>) -> Result<T, ScriptError>,
) -> Result<(), ScriptError> {
    if slot.is_some() {
        return Err(ScriptError::Duplicate {
            line: a.line,
            keyword: a.keyword.to_owned(),
        });
    }
    *slot = Some(parse(a)?);
    Ok(())
}

fn check_fits(line: usize, size: Size, max: Size) -> Result<(), ScriptError> {
    if size.fits(max) {
        Ok(())
    } else {
        Err(ScriptError::TooLarge {
            line,
            width: size.width,
            height: size.height,
            max_width: max.width,
            max_height: max.height,
        })
    }
}

/// The arguments of one line, with the context for its errors.
struct Args<'a> {
    line: usize,
    keyword: &'a str,
    args: &'a [&'a str],
}

impl Args<'_> {
    fn count(&self, expected: usize) -> Result<(), ScriptError> {
        if self.args.len() == expected {
            Ok(())
        } else {
            Err(ScriptError::Arguments {
                line: self.line,
                keyword: self.keyword.to_owned(),
                expected,
                found: self.args.len(),
            })
        }
    }

    fn value_error(&self, index: usize, what: &'static str) -> ScriptError {
        ScriptError::Value {
            line: self.line,
            what,
            token: self.args.get(index).copied().unwrap_or_default().to_owned(),
        }
    }

    fn token(&self, index: usize) -> &str {
        // `count` has checked the length before any argument is read.
        self.args.get(index).copied().unwrap_or_default()
    }

    fn number(
        &self,
        index: usize,
        what: &'static str,
        min: u32,
        max: u32,
    ) -> Result<u32, ScriptError> {
        let token = self.token(index);
        let value = parse_decimal(token).ok_or_else(|| self.value_error(index, what))?;
        u32::try_from(value)
            .ok()
            .filter(|v| (min..=max).contains(v))
            .ok_or(ScriptError::OutOfRange {
                line: self.line,
                what,
                value,
                min: u64::from(min),
                max: u64::from(max),
            })
    }

    fn size(&self, index: usize) -> Result<Size, ScriptError> {
        Ok(Size {
            width: self.number(index, "width", 1, MAX_DIMENSION_PX)?,
            height: self.number(index + 1, "height", 1, MAX_DIMENSION_PX)?,
        })
    }

    fn geometry(&self, index: usize) -> Result<Geometry, ScriptError> {
        Ok(Geometry {
            size: self.size(index)?,
            aspect_milli: self.number(index + 2, "aspect ratio", 0, MAX_ASPECT_MILLI)?,
        })
    }

    fn timing(&self, index: usize) -> Result<Timing, ScriptError> {
        Ok(Timing {
            fps_mhz: self.number(index, "frame rate", MIN_FPS_MHZ, MAX_FPS_MHZ)?,
            sample_rate_hz: self.number(index + 1, "sample rate", 1, MAX_SAMPLE_RATE_HZ)?,
        })
    }

    fn pixel_format(&self, index: usize) -> Result<PixelFormat, ScriptError> {
        match self.token(index) {
            "xrgb8888" => Ok(PixelFormat::Xrgb8888),
            "rgb565" => Ok(PixelFormat::Rgb565),
            "0rgb1555" => Ok(PixelFormat::Rgb1555),
            _ => Err(self.value_error(index, "pixel format")),
        }
    }

    fn audio_mode(&self, index: usize) -> Result<AudioMode, ScriptError> {
        match self.token(index) {
            "batch" => Ok(AudioMode::Batch),
            "single" => Ok(AudioMode::Single),
            "off" => Ok(AudioMode::Off),
            _ => Err(self.value_error(index, "audio mode")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(body: &str) -> Result<Script, ScriptError> {
        Script::parse(&format!("neptron-testcore 1\n{body}"))
    }

    #[test]
    fn header_alone_gives_the_defaults() {
        let s = parse("").unwrap();
        assert_eq!(s.pixel_format, PixelFormat::Xrgb8888);
        assert_eq!(s.av.geometry, DEFAULT_GEOMETRY);
        assert_eq!(s.av.max, DEFAULT_GEOMETRY.size);
        assert_eq!(s.av.timing, DEFAULT_TIMING);
        assert_eq!(s.audio, AudioMode::Batch);
        assert_eq!(s.save_ram_bytes, 0);
        assert!(s.events.is_empty());
    }

    #[test]
    fn full_script_parses() {
        let s = parse(
            "# a comment line\n\
             pixel_format rgb565\n\
             geometry 256 224 1333   # trailing comment\n\
             max_size 320 240\n\
             timing 59940 44100\n\
             audio single\n\
             save_ram 8192\n\
             \n\
             at 10 geometry 320 224 1333\n\
             at 10 size 256 224\n\
             at 20 av_info 640 480 1333 640 480 50000 48000\n\
             at 21 dupe\n\
             at 22 sram_write 8191 255\n\
             at 30 shutdown\n",
        )
        .unwrap();
        assert_eq!(s.pixel_format, PixelFormat::Rgb565);
        assert_eq!(
            s.av,
            AvInfo {
                geometry: Geometry {
                    size: Size {
                        width: 256,
                        height: 224
                    },
                    aspect_milli: 1333
                },
                max: Size {
                    width: 320,
                    height: 240
                },
                timing: Timing {
                    fps_mhz: 59_940,
                    sample_rate_hz: 44_100
                },
            }
        );
        assert_eq!(s.audio, AudioMode::Single);
        assert_eq!(s.save_ram_bytes, 8192);
        let actions: Vec<_> = s.events.iter().map(|e| (e.frame, e.action)).collect();
        assert_eq!(actions.len(), 6);
        assert_eq!(actions[3], (21, Action::Dupe), "events keep the file order");
        assert_eq!(
            actions[4],
            (
                22,
                Action::SramWrite {
                    offset: 8191,
                    value: 255
                }
            )
        );
        assert_eq!(actions[5], (30, Action::Shutdown));
        assert_eq!(
            s.buffer_size(),
            Size {
                width: 640,
                height: 480
            }
        );
    }

    #[test]
    fn max_size_defaults_to_the_geometry() {
        let s = parse("geometry 256 224 0\n").unwrap();
        assert_eq!(
            s.av.max,
            Size {
                width: 256,
                height: 224
            }
        );
    }

    #[test]
    fn header_errors() {
        assert_eq!(
            Script::parse("  # only a comment\n\n"),
            Err(ScriptError::Empty)
        );
        assert!(matches!(
            Script::parse("geometry 1 1 0"),
            Err(ScriptError::Header { line: 1, .. })
        ));
        assert!(matches!(
            Script::parse("\nneptron-testcore one"),
            Err(ScriptError::Header { line: 2, .. })
        ));
        assert!(matches!(
            Script::parse("neptron-testcore 1 2"),
            Err(ScriptError::Header { .. })
        ));
        assert_eq!(
            Script::parse("neptron-testcore 2"),
            Err(ScriptError::Version {
                line: 1,
                version: 2
            })
        );
        assert!(matches!(
            Script::parse("neptron-testcore 0"),
            Err(ScriptError::Version { version: 0, .. })
        ));
    }

    #[test]
    fn numbers_are_plain_decimal_digits() {
        assert!(matches!(
            parse("geometry +320 240 0"),
            Err(ScriptError::Value { what: "width", .. })
        ));
        assert!(matches!(
            parse("at +5 dupe"),
            Err(ScriptError::Value {
                what: "frame number",
                ..
            })
        ));
        assert!(matches!(
            Script::parse("neptron-testcore +1"),
            Err(ScriptError::Header { .. })
        ));
        assert!(matches!(
            parse("save_ram 1_000"),
            Err(ScriptError::Value { .. })
        ));
        assert_eq!(parse_decimal("0042"), Some(42));
        assert_eq!(parse_decimal("99999999999999999999"), None);
    }

    #[test]
    fn non_utf8_is_reported_with_its_line() {
        let mut bytes = b"neptron-testcore 1\n# ok\naudio off\n".to_vec();
        assert!(Script::parse_bytes(&bytes).is_ok());
        bytes.extend_from_slice(b"at 1 dupe # \xFF\n");
        assert_eq!(
            Script::parse_bytes(&bytes),
            Err(ScriptError::NotUtf8 { line: 4 })
        );
        assert_eq!(
            Script::parse_bytes(b"\xC3"),
            Err(ScriptError::NotUtf8 { line: 1 })
        );
    }

    #[test]
    fn unknown_keywords_are_errors() {
        assert_eq!(
            parse("rotation 1"),
            Err(ScriptError::UnknownKeyword {
                line: 2,
                keyword: "rotation".to_owned()
            })
        );
        assert_eq!(
            parse("at 1 rotate"),
            Err(ScriptError::UnknownKeyword {
                line: 2,
                keyword: "rotate".to_owned()
            })
        );
    }

    #[test]
    fn argument_counts_are_checked() {
        assert_eq!(
            parse("geometry 320 240"),
            Err(ScriptError::Arguments {
                line: 2,
                keyword: "geometry".to_owned(),
                expected: 3,
                found: 2
            })
        );
        assert!(matches!(
            parse("at 5 dupe now"),
            Err(ScriptError::Arguments {
                expected: 0,
                found: 1,
                ..
            })
        ));
        assert!(matches!(
            parse("at"),
            Err(ScriptError::Arguments { found: 0, .. })
        ));
        assert!(matches!(
            parse("at 5"),
            Err(ScriptError::Arguments { found: 1, .. })
        ));
    }

    #[test]
    fn values_are_checked() {
        assert!(matches!(
            parse("pixel_format rgb888"),
            Err(ScriptError::Value {
                what: "pixel format",
                ..
            })
        ));
        assert!(matches!(
            parse("audio loud"),
            Err(ScriptError::Value {
                what: "audio mode",
                ..
            })
        ));
        assert!(matches!(
            parse("geometry -1 240 0"),
            Err(ScriptError::Value { what: "width", .. })
        ));
        assert!(matches!(
            parse("at x dupe"),
            Err(ScriptError::Value {
                what: "frame number",
                ..
            })
        ));
        assert_eq!(
            parse("geometry 0 240 0"),
            Err(ScriptError::OutOfRange {
                line: 2,
                what: "width",
                value: 0,
                min: 1,
                max: u64::from(MAX_DIMENSION_PX)
            })
        );
        assert!(matches!(
            parse("timing 0 48000"),
            Err(ScriptError::OutOfRange {
                what: "frame rate",
                ..
            })
        ));
        assert!(matches!(
            parse("timing 60000 384001"),
            Err(ScriptError::OutOfRange {
                what: "sample rate",
                ..
            })
        ));
        assert!(matches!(
            parse("save_ram 99999999999"),
            Err(ScriptError::OutOfRange {
                what: "save RAM size",
                ..
            })
        ));
        assert!(matches!(
            parse("save_ram 4\nat 0 sram_write 0 256"),
            Err(ScriptError::OutOfRange {
                what: "byte value",
                ..
            })
        ));
    }

    #[test]
    fn sizes_must_fit_the_maximum_in_force() {
        assert!(matches!(
            parse("geometry 320 240 0\nmax_size 256 240"),
            Err(ScriptError::TooLarge { line: 3, .. })
        ));
        assert!(matches!(
            parse("at 1 geometry 640 240 0"),
            Err(ScriptError::TooLarge {
                line: 2,
                width: 640,
                ..
            })
        ));
        assert!(matches!(
            parse("at 1 size 320 241"),
            Err(ScriptError::TooLarge {
                line: 2,
                height: 241,
                ..
            })
        ));
        assert!(matches!(
            parse("at 1 av_info 640 480 0 320 240 60000 48000"),
            Err(ScriptError::TooLarge { line: 2, .. })
        ));
        let grown = parse("at 1 av_info 320 240 0 640 480 60000 48000\nat 2 geometry 640 480 0");
        assert!(grown.is_ok(), "{grown:?}");
        assert!(matches!(
            parse("at 1 av_info 320 240 0 320 240 60000 48000\nat 2 size 321 240"),
            Err(ScriptError::TooLarge { line: 3, .. })
        ));
    }

    #[test]
    fn setup_lines_appear_once_and_before_events() {
        assert_eq!(
            parse("audio off\naudio batch"),
            Err(ScriptError::Duplicate {
                line: 3,
                keyword: "audio".to_owned()
            })
        );
        assert_eq!(
            parse("at 1 dupe\nsave_ram 4"),
            Err(ScriptError::SetupAfterEvent {
                line: 3,
                keyword: "save_ram".to_owned()
            })
        );
    }

    #[test]
    fn get_variable_names_a_known_option_and_a_save_ram_byte() {
        let s = parse("save_ram 4\nat 2 get_variable neptron_testcore_level 3").unwrap();
        assert_eq!(
            s.events[0].action,
            Action::GetVariable {
                option: 1,
                offset: 3
            }
        );
        assert!(matches!(
            parse("save_ram 4\nat 2 get_variable neptron_testcore_speed 0"),
            Err(ScriptError::Value {
                what: "core option key",
                ..
            })
        ));
        assert!(matches!(
            parse("save_ram 4\nat 2 get_variable neptron_testcore_mode 4"),
            Err(ScriptError::SramOffset { offset: 4, .. })
        ));
        assert!(matches!(
            parse("at 2 get_variable neptron_testcore_mode"),
            Err(ScriptError::Arguments { expected: 2, .. })
        ));
    }

    #[test]
    fn unknown_env_takes_no_arguments() {
        let s = parse("at 7 unknown_env").unwrap();
        assert_eq!(s.events[0].action, Action::UnknownEnv);
        assert!(matches!(
            parse("at 7 unknown_env 3"),
            Err(ScriptError::Arguments { expected: 0, .. })
        ));
    }

    #[test]
    fn frames_must_not_go_back() {
        assert_eq!(
            parse("at 5 dupe\nat 5 dupe\nat 4 dupe"),
            Err(ScriptError::FrameOrder {
                line: 4,
                frame: 4,
                previous: 5
            })
        );
    }

    #[test]
    fn sram_writes_must_hit_the_save_ram() {
        assert_eq!(
            parse("save_ram 4\nat 0 sram_write 4 1"),
            Err(ScriptError::SramOffset {
                line: 3,
                offset: 4,
                size: 4
            })
        );
        assert!(matches!(
            parse("at 0 sram_write 0 1"),
            Err(ScriptError::SramOffset { size: 0, .. })
        ));
    }

    #[test]
    fn from_file_reports_the_path() {
        let missing = PathBuf::from("/nonexistent/neptron-testcore-script");
        match Script::from_file(missing.clone()) {
            Err(LoadError::Read { path, .. }) => assert_eq!(path, missing),
            other => panic!("expected a read error, got {other:?}"),
        }
    }

    #[test]
    fn pixel_format_sizes_and_masks() {
        assert_eq!(PixelFormat::Xrgb8888.bytes_per_pixel(), 4);
        assert_eq!(PixelFormat::Rgb565.bytes_per_pixel(), 2);
        assert_eq!(PixelFormat::Rgb1555.bytes_per_pixel(), 2);
        assert_eq!(PixelFormat::Rgb1555.colour_mask(), 0x7FFF);
    }
}
