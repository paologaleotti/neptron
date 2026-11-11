//! The tier T4 check of real cores: it loads one libretro core through
//! [`Host`], runs one content file for a fixed number of frames with a fixed
//! input script, and prints a trace of what the host saw.
//!
//! The cores are Linux libraries, so this runs in the QEMU guest:
//! `cargo xtask core-trace` builds it, runs one process per case, and
//! compares each trace with `tests/traces`. Every output line starts with
//! `TRACE <case> ` or `FRAME <case> `, so the host can find the lines in a
//! console that the kernel also writes to. The exit code is 0 when the core
//! loaded and ran every frame, and 1 otherwise; the trace is printed in both
//! cases.

#![forbid(unsafe_code)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use neptron_libretro::{
    ContentRequest, Event, Host, HostConfig, LogRecord, Pad, Video, read_save_file, save_file_path,
    write_save_file,
};
use neptron_types::{Control, Frame};

/// The version of the trace text. A change to the lines adds a version.
const TRACE_FORMAT: u32 = 1;
/// Hex digits of a frame dump on one console line. Long lines survive the
/// serial console, but a short line limits the damage of a kernel message
/// in the middle of one.
const DUMP_HEX_PER_LINE: usize = 4096;

fn main() -> ExitCode {
    let args = match Args::parse(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("core_trace: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let out = Out { case: &args.case };
    if run(&args, &out) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

const USAGE: &str = "\
usage: core_trace --case NAME --core FILE --content FILE --ext E[,E...]
                  --frames N --hash-every N --work DIR
                  [--press FRAME:BUTTON[+BUTTON...]:FRAMES]... [--dump FRAME]...";

/// The command line.
#[derive(Debug)]
struct Args {
    case: String,
    core: PathBuf,
    content: PathBuf,
    extensions: Vec<String>,
    frames: u32,
    hash_every: u32,
    work: PathBuf,
    presses: Vec<Press>,
    dumps: Vec<u32>,
}

/// Buttons held from frame `start` for `frames` frames, on port 0.
#[derive(Debug, Clone)]
struct Press {
    start: u32,
    frames: u32,
    names: String,
    pad: Pad,
}

impl Args {
    fn parse(mut it: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut case = None;
        let mut core = None;
        let mut content = None;
        let mut extensions = Vec::new();
        let mut frames = None;
        let mut hash_every = None;
        let mut work = None;
        let mut presses = Vec::new();
        let mut dumps = Vec::new();
        while let Some(flag) = it.next() {
            let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
            match flag.as_str() {
                "--case" => case = Some(value),
                "--core" => core = Some(PathBuf::from(value)),
                "--content" => content = Some(PathBuf::from(value)),
                "--ext" => extensions = value.split(',').map(str::to_owned).collect(),
                "--frames" => frames = Some(number(&flag, &value)?),
                "--hash-every" => hash_every = Some(number(&flag, &value)?),
                "--work" => work = Some(PathBuf::from(value)),
                "--press" => presses.push(Press::parse(&value)?),
                "--dump" => dumps.push(number(&flag, &value)?),
                other => return Err(format!("unknown argument {other:?}")),
            }
        }
        let hash_every = hash_every.ok_or("--hash-every is missing")?;
        if hash_every == 0 {
            return Err("--hash-every must be 1 or more".to_owned());
        }
        Ok(Self {
            case: case.ok_or("--case is missing")?,
            core: core.ok_or("--core is missing")?,
            content: content.ok_or("--content is missing")?,
            extensions,
            frames: frames.ok_or("--frames is missing")?,
            hash_every,
            work: work.ok_or("--work is missing")?,
            presses,
            dumps,
        })
    }

    fn pad_at(&self, frame: u32) -> Pad {
        let mut pad = Pad::default();
        for press in &self.presses {
            if frame >= press.start && frame - press.start < press.frames {
                pad.buttons |= press.pad.buttons;
            }
        }
        pad
    }
}

impl Press {
    fn parse(text: &str) -> Result<Self, String> {
        let mut parts = text.split(':');
        let (Some(start), Some(names), Some(frames), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(format!("--press {text:?} is not FRAME:BUTTONS:FRAMES"));
        };
        let mut pad = Pad::default();
        for name in names.split('+') {
            pad.apply(control(name)?, 1);
        }
        Ok(Self {
            start: number("--press", start)?,
            frames: number("--press", frames)?,
            names: names.to_owned(),
            pad,
        })
    }
}

fn number(flag: &str, value: &str) -> Result<u32, String> {
    value
        .parse()
        .map_err(|_| format!("{flag} needs a number, got {value:?}"))
}

fn control(name: &str) -> Result<Control, String> {
    Ok(match name {
        "a" => Control::A,
        "b" => Control::B,
        "x" => Control::X,
        "y" => Control::Y,
        "l" => Control::L,
        "r" => Control::R,
        "start" => Control::Start,
        "select" => Control::Select,
        "up" => Control::Up,
        "down" => Control::Down,
        "left" => Control::Left,
        "right" => Control::Right,
        other => return Err(format!("unknown button {other:?}")),
    })
}

/// The output of one case.
struct Out<'a> {
    case: &'a str,
}

impl Out<'_> {
    fn line(&self, text: &str) {
        println!("TRACE {} {text}", self.case);
    }

    fn logs(&self, phase: &str, logs: &[LogRecord]) {
        for record in logs {
            self.line(&format!(
                "log {phase} {:?} {:?} {}",
                record.level,
                record.source,
                escape(&record.text)
            ));
        }
    }

    fn dump(&self, frame_no: u32, frame: &Frame<'_>) {
        let mut hex = String::with_capacity(frame.data.len() * 2);
        for row in 0..usize::from(frame.height) {
            let start = row * frame.pitch;
            let bytes = usize::from(frame.width) * frame.format.bytes_per_pixel();
            for byte in frame.data.get(start..start + bytes).unwrap_or(&[]) {
                // Writing to a String cannot fail.
                let _ = write!(hex, "{byte:02x}");
            }
        }
        let chunks: Vec<&str> = hex
            .as_bytes()
            .chunks(DUMP_HEX_PER_LINE)
            .map(|c| std::str::from_utf8(c).unwrap_or(""))
            .collect();
        for (index, chunk) in chunks.iter().enumerate() {
            println!(
                "FRAME {} {frame_no} {} {} {:?} {index}/{} {chunk}",
                self.case,
                frame.width,
                frame.height,
                frame.format,
                chunks.len()
            );
        }
    }
}

/// Runs the case. Returns whether the core loaded and ran every frame.
fn run(args: &Args, out: &Out<'_>) -> bool {
    out.line(&format!("format {TRACE_FORMAT}"));
    out.line(&format!("arch {}", std::env::consts::ARCH));
    out.line(&format!("core {}", file_name(&args.core)));
    out.line(&format!("content {}", file_name(&args.content)));
    out.line(&format!(
        "run frames={} hash_every={}",
        args.frames, args.hash_every
    ));
    for press in &args.presses {
        out.line(&format!(
            "press {} {} frames={}",
            press.start, press.names, press.frames
        ));
    }

    let system_dir = args.work.join("system");
    let save_dir = args.work.join("saves");
    for dir in [&system_dir, &save_dir] {
        if let Err(e) = std::fs::create_dir_all(dir) {
            out.line(&format!("error cannot make {}: {e}", dir.display()));
            return false;
        }
    }
    let config = HostConfig {
        system_dir,
        save_dir: save_dir.clone(),
    };
    let mut host = match Host::open(&args.core, config) {
        Ok(host) => host,
        Err(e) => {
            out.line(&format!("open error {}", escape(&e.to_string())));
            return false;
        }
    };
    let info = host.system_info();
    out.line(&format!(
        "system name={:?} version={:?} need_fullpath={} block_extract={}",
        info.name, info.version, info.need_fullpath, info.block_extract
    ));
    out.line(&format!("extensions {}", info.extensions.join("|")));
    out.logs("open", host.logs());
    for option in host.options() {
        let values: Vec<&str> = option.values().collect();
        out.line(&format!(
            "option {} = {:?} [{}] {}",
            option.key(),
            option.value(),
            values.join("|"),
            escape(option.description())
        ));
    }

    let request = ContentRequest {
        path: &args.content,
        extensions: &args.extensions,
    };
    let loaded = host.load_game(Some(request));
    out.logs("load", host.logs());
    let av = match loaded {
        Ok(av) => av,
        Err(e) => {
            out.line(&format!("load error {}", escape(&e.to_string())));
            return false;
        }
    };
    let g = av.geometry;
    out.line(&format!(
        "av base={}x{} max={}x{} aspect_milli={} fps_mhz={} sample_rate_mhz={}",
        g.base_width,
        g.base_height,
        g.max_width,
        g.max_height,
        g.aspect_milli,
        av.timing.fps_mhz,
        av.timing.sample_rate_mhz
    ));
    out.line(&format!("pixel_format {:?}", host.pixel_format()));
    let options_after_load: Vec<String> = host
        .options()
        .iter()
        .map(|o| format!("{}={}", o.key(), o.value()))
        .collect();
    out.line(&format!(
        "options_after_load count={}",
        options_after_load.len()
    ));

    let ran = run_frames(args, out, &mut host);
    save_ram(out, &mut host, &save_dir, &args.content);
    drop(host);
    out.line("end");
    ran
}

/// What the video of a run of frames was.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Shown {
    Frame { width: u16, height: u16 },
    Duplicate,
    Missing,
    Rejected(String),
}

fn run_frames(args: &Args, out: &Out<'_>, host: &mut Host) -> bool {
    let mut audio = AudioWindow::default();
    let mut frames = Fnv::default();
    let mut shown: Option<(u32, Shown)> = None;
    for frame_no in 0..args.frames {
        if let Err(e) = host.set_pad(0, args.pad_at(frame_no)) {
            out.line(&format!("error set_pad {e}"));
            return false;
        }
        let output = match host.run_frame() {
            Ok(output) => output,
            Err(e) => {
                out.line(&format!("error run_frame {frame_no} {e}"));
                return false;
            }
        };
        for event in output.events {
            out.line(&format!("event {frame_no} {}", event_text(event)));
        }
        out.logs(&frame_no.to_string(), output.logs);
        let d = output.dropped;
        if d.audio_frames + d.events + d.log_records > 0 {
            out.line(&format!(
                "dropped {frame_no} audio_frames={} events={} log_records={}",
                d.audio_frames, d.events, d.log_records
            ));
        }
        audio.add(output.audio);
        let (now, picture) = match output.video {
            Video::Frame(f) => (
                Shown::Frame {
                    width: f.width,
                    height: f.height,
                },
                Some(f),
            ),
            Video::Duplicate(last) => (Shown::Duplicate, last),
            Video::Missing => (Shown::Missing, None),
            Video::Rejected(fault) => (Shown::Rejected(fault.to_string()), None),
        };
        match &shown {
            Some((_, kind)) if *kind == now => {}
            _ => {
                if let Some((start, kind)) = shown.take() {
                    out.line(&video_text(start, frame_no - 1, &kind));
                }
                shown = Some((frame_no, now));
            }
        }
        // Every frame of the window goes into its hash, so a fault in a
        // frame between two hash lines still changes the trace. A frame
        // with no picture adds zeros.
        frames.bytes(&picture.as_ref().map_or(0, frame_hash).to_le_bytes());
        if args.dumps.contains(&frame_no)
            && let Some(f) = &picture
        {
            out.dump(frame_no, f);
        }
        if (frame_no + 1) % args.hash_every == 0 {
            match &picture {
                Some(f) => out.line(&format!(
                    "hash {frame_no} {}x{} {:?} {:016x}",
                    f.width, f.height, f.format, frames.0
                )),
                None => out.line(&format!("hash {frame_no} none {:016x}", frames.0)),
            }
            frames = Fnv::default();
            out.line(&audio.finish(frame_no));
        }
    }
    if let Some((start, kind)) = shown {
        out.line(&video_text(start, args.frames.saturating_sub(1), &kind));
    }
    true
}

fn video_text(start: u32, end: u32, kind: &Shown) -> String {
    let what = match kind {
        Shown::Frame { width, height } => format!("frame {width}x{height}"),
        Shown::Duplicate => "duplicate".to_owned(),
        Shown::Missing => "missing".to_owned(),
        Shown::Rejected(fault) => format!("rejected {}", escape(fault)),
    };
    format!("video {start}-{end} {what}")
}

fn event_text(event: &Event) -> String {
    match event {
        Event::Geometry(g) => format!(
            "geometry base={}x{} max={}x{} aspect_milli={}",
            g.base_width, g.base_height, g.max_width, g.max_height, g.aspect_milli
        ),
        Event::AvInfo(av) => format!(
            "av_info base={}x{} max={}x{} aspect_milli={} fps_mhz={} sample_rate_mhz={}",
            av.geometry.base_width,
            av.geometry.base_height,
            av.geometry.max_width,
            av.geometry.max_height,
            av.geometry.aspect_milli,
            av.timing.fps_mhz,
            av.timing.sample_rate_mhz
        ),
        Event::Message { text, frames } => format!("message frames={frames} {}", escape(text)),
        other => format!("{other:?}"),
    }
}

/// The audio since the last hash point.
#[derive(Default)]
struct AudioWindow {
    start: u32,
    stereo_frames: usize,
    nonzero_samples: usize,
    hash: Fnv,
}

impl AudioWindow {
    fn add(&mut self, samples: &[i16]) {
        self.stereo_frames += samples.len() / 2;
        self.nonzero_samples += samples.iter().filter(|s| **s != 0).count();
        for sample in samples {
            self.hash.bytes(&sample.to_le_bytes());
        }
    }

    /// The count of samples that are not 0 shows silence in the trace,
    /// which the hash alone does not.
    fn finish(&mut self, end: u32) -> String {
        let text = format!(
            "audio {}-{end} stereo_frames={} nonzero_samples={} {:016x}",
            self.start, self.stereo_frames, self.nonzero_samples, self.hash.0
        );
        *self = Self {
            start: end + 1,
            ..Self::default()
        };
        text
    }
}

/// FNV-1a, 64 bits, the frame hash of `neptron-testcore`.
struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Self(0xCBF2_9CE4_8422_2325)
    }
}

impl Fnv {
    fn bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
}

/// The hash of the visible bytes of each row. Pixels are in the byte order
/// of the core, which is little-endian on every NEPTRON target.
fn frame_hash(frame: &Frame<'_>) -> u64 {
    let mut h = Fnv::default();
    let row_bytes = usize::from(frame.width) * frame.format.bytes_per_pixel();
    for row in 0..usize::from(frame.height) {
        let start = row * frame.pitch;
        h.bytes(frame.data.get(start..start + row_bytes).unwrap_or(&[]));
    }
    h.0
}

/// Copies the save RAM out through a file and back in, and loads a pattern
/// to check the way in.
fn save_ram(out: &Out<'_>, host: &mut Host, save_dir: &Path, content: &Path) {
    let mut ram = Vec::new();
    let size = match host.copy_save_ram(&mut ram) {
        Ok(size) => size,
        Err(e) => {
            out.line(&format!("save_ram error copy {e}"));
            return;
        }
    };
    let mut h = Fnv::default();
    h.bytes(&ram);
    out.line(&format!("save_ram size={size} {:016x}", h.0));
    if size == 0 {
        return;
    }
    let Some(path) = save_file_path(save_dir, content) else {
        out.line("save_ram error no save file name");
        return;
    };
    if let Err(e) = write_save_file(&path, &ram) {
        out.line(&format!("save_ram error write {e}"));
        return;
    }
    let read = match read_save_file(&path) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            out.line("save_ram error the written file is missing");
            return;
        }
        Err(e) => {
            out.line(&format!("save_ram error read {e}"));
            return;
        }
    };
    out.line(&format!(
        "save_ram file {} bytes_equal={}",
        file_name(&path),
        read == ram
    ));
    let pattern: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
    match host.load_save_ram(&pattern) {
        Ok(load) => {
            let mut back = Vec::new();
            let equal = host.copy_save_ram(&mut back).is_ok() && back == pattern;
            out.line(&format!(
                "save_ram load loaded={} file_len={} ram_len={} read_back_equal={equal}",
                load.loaded, load.file_len, load.ram_len
            ));
        }
        Err(e) => out.line(&format!("save_ram error load {e}")),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// One line of text: control characters become escapes, so a log record
/// with a newline stays one trace line.
fn escape(text: &str) -> String {
    text.trim_end().escape_debug().to_string()
}
