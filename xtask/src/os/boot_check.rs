//! `cargo xtask os boot-check`: the tier T3 gate of the image. It boots the
//! image in QEMU as a PC boots it, waits until the frontend shows the game
//! list, takes the `vkms` writeback picture of the display, and compares it
//! with the golden image, byte for byte. It records the boot time.
//!
//! On x86-64 the firmware (BIOS or UEFI) boots the USB disk image through
//! GRUB. On aarch64 QEMU loads the kernel of the test image directly.
//! In the guest, init starts the launcher as on a PC, and the check script
//! runs beside it (`host-run check`). The guest tool `neptron-capture` waits
//! for the screen on the control socket, then prints the picture.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use neptron_ui::golden::Image;

use super::host::{self, Accel, Cache, MachineLock};
use super::qemu::{self, Boot, Firmware, GuestJob, QemuPlan};
use super::{Arch, OsError, RunScript, transcript};
use crate::load::LoadPolicy;
use crate::repo::Repo;

/// The screen that the check waits for. The image boots with nothing
/// confirmed, so the guard opens the boot prompt over the game list,
/// reverts to the safe default when nobody answers in 10 s, and shows a
/// notice over the home screen of the game list run until the player
/// presses OK. That picture has no timer, so the golden image is the same
/// on every run.
pub const GAME_LIST_SCREEN: &str = "notice";
/// The golden image of the game list, relative to the checkout.
pub const GOLDEN: &str = "os/tests/golden/boot-game-list.ppm";
/// The marker line before the picture.
pub const CAPTURE_BEGIN: &str = "NEPTRON-CAPTURE-BEGIN";
/// The marker line after the picture.
pub const CAPTURE_END: &str = "NEPTRON-CAPTURE-END";
/// The line that `neptron-capture` prints when the screen is on show.
pub const SCREEN_LINE: &str = "screen";
/// The line that `neptron-capture` prints at the first frame.
pub const FIRST_FRAME_LINE: &str = "frame 1";
/// The disk image of the x86-64 target in the images directory.
pub const X86_64_DISK: &str = "neptron-x86_64.img";

/// The arguments of `os boot-check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootCheckArgs {
    /// The target.
    pub arch: Arch,
    /// The firmware of an x86-64 boot.
    pub firmware: FirmwareKind,
    /// The guest deadline.
    pub timeout_secs: u64,
    /// An images directory that replaces the build output.
    pub images: Option<PathBuf>,
    /// Write the picture of this run as the golden image.
    pub bless: bool,
}

/// The firmware that `--firmware` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareKind {
    /// Legacy BIOS.
    Bios,
    /// UEFI.
    Uefi,
}

impl FirmwareKind {
    /// The name on the command line and in the report.
    pub fn name(self) -> &'static str {
        match self {
            Self::Bios => "bios",
            Self::Uefi => "uefi",
        }
    }
}

impl BootCheckArgs {
    /// Parses `[--arch A] [--firmware bios|uefi] [--timeout SECS]
    /// [--images DIR] [--bless]`.
    pub fn parse(args: &[&str], default_arch: Arch) -> Result<Self, OsError> {
        let mut out = Self {
            arch: default_arch,
            firmware: FirmwareKind::Bios,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            images: None,
            bless: false,
        };
        let mut it = args.iter();
        while let Some(&arg) = it.next() {
            if arg == "--bless" {
                out.bless = true;
                continue;
            }
            let value = it
                .next()
                .copied()
                .ok_or_else(|| OsError::Usage(format!("{arg} needs a value")))?;
            match arg {
                "--arch" => out.arch = Arch::parse(value)?,
                "--firmware" => {
                    out.firmware = match value {
                        "bios" => FirmwareKind::Bios,
                        "uefi" => FirmwareKind::Uefi,
                        other => {
                            return Err(OsError::Usage(format!(
                                "--firmware takes bios or uefi, not {other:?}"
                            )));
                        }
                    }
                }
                "--timeout" => {
                    out.timeout_secs = match value.parse::<u64>() {
                        Ok(secs) if secs > 0 => secs,
                        _ => {
                            return Err(OsError::BadNumber {
                                flag: "--timeout",
                                value: value.to_owned(),
                            });
                        }
                    }
                }
                "--images" => out.images = Some(PathBuf::from(value)),
                other => return Err(OsError::Usage(format!("unknown argument {other:?}"))),
            }
        }
        if out.arch == Arch::Aarch64 && out.firmware == FirmwareKind::Uefi {
            return Err(OsError::Usage(
                "the aarch64 test image boots its kernel directly; --firmware is for x86_64"
                    .to_owned(),
            ));
        }
        Ok(out)
    }
}

/// The guest script: wait for the game list, then print the picture. The
/// capture tool gets the deadline less a margin, so it reports a missing
/// screen itself before the guest deadline kills it. When it fails, the
/// script asks the frontend what it shows, for the console log.
pub fn guest_script(timeout_secs: u64) -> String {
    let wait_ms = timeout_secs.saturating_sub(GUEST_MARGIN_SECS).max(1) * 1000;
    format!(
        "neptron-capture --screen {GAME_LIST_SCREEN} --timeout-ms {wait_ms} \
         --settle-ms {SETTLE_MS} && exit 0\n\
         for command in 'ui tree' 'video state' 'session state' stats 'log tail 40'; do\n\
         \techo \"neptronctl $command:\"; neptronctl $command\n\
         done\n\
         exit 1\n"
    )
}

/// What the guest output of a boot check holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOutput {
    /// Guest uptime when the frontend reported its first frame, in ms.
    pub first_frame_uptime_ms: Option<u64>,
    /// Guest uptime when the frontend reported the game list, in ms.
    pub screen_uptime_ms: Option<u64>,
    /// The picture, a binary PPM file.
    pub picture: Option<Vec<u8>>,
    /// The mode line of the capture tool.
    pub mode: Option<String>,
}

/// An error in the picture that the guest printed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CaptureError {
    /// A begin marker with no end marker.
    #[error("the picture has no end marker")]
    Unterminated,
    /// A character that is not base64.
    #[error("line {line} of the picture is not base64")]
    NotBase64 {
        /// The line number in the picture, from 1.
        line: usize,
    },
}

/// Reads the output lines of the guest script.
pub fn parse_output(lines: &[String]) -> Result<CheckOutput, CaptureError> {
    let mut out = CheckOutput {
        first_frame_uptime_ms: None,
        screen_uptime_ms: None,
        picture: None,
        mode: None,
    };
    let mut picture: Option<Vec<String>> = None;
    for line in lines {
        let line = line.trim_end();
        if let Some(text) = picture.as_mut() {
            if line == CAPTURE_END {
                out.picture = Some(decode_base64(text)?);
                picture = None;
            } else {
                text.push(line.to_owned());
            }
            continue;
        }
        if line.starts_with(CAPTURE_BEGIN) {
            picture = Some(Vec::new());
        } else if let Some(rest) = line.strip_prefix("mode ") {
            out.mode = Some(rest.to_owned());
        } else if let Some(rest) = line.strip_prefix(FIRST_FRAME_LINE)
            && let Some(uptime) = rest.strip_prefix(" uptime ")
        {
            out.first_frame_uptime_ms = transcript::parse_uptime_ms(uptime);
        } else if let Some(rest) = line.strip_prefix(SCREEN_LINE)
            && let Some((_, uptime)) = rest.split_once(" uptime ")
        {
            out.screen_uptime_ms = transcript::parse_uptime_ms(uptime);
        }
    }
    if picture.is_some() {
        return Err(CaptureError::Unterminated);
    }
    Ok(out)
}

/// Decodes standard base64 lines with `=` padding.
pub fn decode_base64(lines: &[String]) -> Result<Vec<u8>, CaptureError> {
    let mut out = Vec::new();
    let mut acc: u32 = 0;
    let mut bits = 0;
    for (index, line) in lines.iter().enumerate() {
        for byte in line.bytes() {
            let value = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' => continue,
                _ => return Err(CaptureError::NotBase64 { line: index + 1 }),
            };
            acc = (acc << 6) | u32::from(value);
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push(((acc >> bits) & 0xff) as u8);
            }
        }
    }
    Ok(out)
}

/// The UEFI flash images next to the QEMU of `qemu_binary`: QEMU installs
/// them in `<prefix>/share/qemu`. Debian and Fedora put them there too.
pub fn uefi_candidates(qemu_binary: &Path) -> Vec<(PathBuf, PathBuf)> {
    let mut dirs = Vec::new();
    if let Some(prefix) = qemu_binary.parent().and_then(Path::parent) {
        dirs.push(prefix.join("share").join("qemu"));
    }
    dirs.push(PathBuf::from("/usr/share/qemu"));
    dirs.push(PathBuf::from("/usr/local/share/qemu"));
    dirs.into_iter()
        .map(|d| (d.join("edk2-x86_64-code.fd"), d.join("edk2-i386-vars.fd")))
        .collect()
}

/// `cargo xtask os boot-check`.
pub fn run(args: &[&str]) -> Result<ExitCode, OsError> {
    let args = BootCheckArgs::parse(args, Arch::default_for(std::env::consts::ARCH))?;
    let repo = Repo::discover()?;
    let cache = Cache::from_env()?;
    let images = match (&args.images, std::env::var_os(host::IMAGES_ENV)) {
        (Some(dir), _) => dir.clone(),
        (None, Some(dir)) if !dir.is_empty() => PathBuf::from(dir),
        _ => cache.images(args.arch),
    };
    let qemu_path = host::require_tool(
        args.arch.qemu_binary(),
        "install QEMU (macOS: brew install qemu; Debian: apt install qemu-system)",
    )?;
    let qemu_path = std::fs::canonicalize(&qemu_path).unwrap_or(qemu_path);
    let out_dir = repo.root.join("target").join("os").join("boot-check");
    let label = match args.arch {
        Arch::X86_64 => format!("{}-{}", args.arch.name(), args.firmware.name()),
        Arch::Aarch64 => args.arch.name().to_owned(),
    };
    let boot = match args.arch {
        Arch::X86_64 => {
            let image = images.join(X86_64_DISK);
            require_file(&image, args.arch)?;
            let firmware = match args.firmware {
                FirmwareKind::Bios => Firmware::Bios,
                FirmwareKind::Uefi => uefi_firmware(&qemu_path, &out_dir)?,
            };
            Boot::UsbDisk { image, firmware }
        }
        Arch::Aarch64 => {
            for file in [args.arch.kernel_image(), qemu::ROOTFS] {
                require_file(&images.join(file), args.arch)?;
            }
            Boot::Kernel
        }
    };
    let accel = Accel::choose(
        args.arch,
        std::env::consts::OS,
        std::env::consts::ARCH,
        host::kvm_available(),
    );
    if accel == Accel::Tcg {
        eprintln!(
            "xtask: no hardware acceleration for an {} guest on this host; QEMU emulates \
             the CPU, and the boot time is an emulated time",
            args.arch.name()
        );
    }

    let _lock = MachineLock::take(&cache.qemu_lock(), "QEMU guest")?;
    let run_dir = repo.root.join("target").join("qemu");
    super::remove_stale_scripts(&run_dir);
    let script = RunScript::write(&run_dir, &guest_script(args.timeout_secs))?;
    let nonce = transcript::new_nonce();
    let plan = QemuPlan {
        arch: args.arch,
        accel,
        images,
        boot,
        job: Some(GuestJob {
            entry: qemu::CHECK_ENTRY,
            script: script.path.clone(),
            nonce: nonce.clone(),
            timeout_secs: args.timeout_secs,
        }),
        share: None,
    };
    let policy = LoadPolicy::from_env().map_err(|e| OsError::Usage(e.to_string()))?;
    let mut cmd = super::under_load_policy(&policy, args.arch.qemu_binary());
    cmd.args(plan.args());
    let stop = super::guest::StopFlag::register()?;
    let log = out_dir.join(format!("{label}-console.log"));
    let deadline = Duration::from_secs(args.timeout_secs) + qemu::HOST_MARGIN;
    let mut output = TimedLines::new();
    let code = super::guest::run(&mut cmd, &plan, &nonce, deadline, &log, &stop, &mut output)?;
    drop(script);

    let lines: Vec<String> = output.lines.iter().map(|(_, l)| l.clone()).collect();
    let host_time = |prefix: &str| {
        output
            .lines
            .iter()
            .find(|(_, l)| l.starts_with(prefix))
            .map(|(at, _)| *at)
    };
    let first_frame_host = host_time(FIRST_FRAME_LINE);
    let screen_host = host_time(SCREEN_LINE);
    for line in lines.iter().filter(|l| !is_picture_line(l)) {
        println!("{line}");
    }
    let parsed = parse_output(&lines).map_err(|e| OsError::Usage(format!("boot-check: {e}")))?;
    let verdict = judge(&repo.root, &out_dir, &label, &parsed, args.bless)?;
    let report = Report {
        label: &label,
        accel,
        first_frame_host,
        screen_host,
        parsed: &parsed,
        verdict: &verdict,
        code,
    };
    let text = report.text();
    super::write_file(&out_dir.join(format!("{label}.txt")), &text)?;
    print!("{text}");
    Ok(super::exit_code(code == 0 && verdict.passed()))
}

/// The outcome of the picture comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The guest printed no picture.
    NoPicture,
    /// The picture is not a valid PPM file.
    BadPicture(String),
    /// No golden image exists yet.
    NoGolden,
    /// The picture equals the golden image.
    Same {
        /// The size of the picture.
        width: u32,
        /// The size of the picture.
        height: u32,
    },
    /// The picture differs, with the summary of the difference.
    Differs(String),
    /// `--bless` wrote the picture as the golden image.
    Blessed,
}

impl Verdict {
    /// Whether the gate passes.
    pub fn passed(&self) -> bool {
        matches!(self, Self::Same { .. })
    }

    fn summary(&self) -> String {
        match self {
            Self::NoPicture => "no picture".to_owned(),
            Self::BadPicture(e) => format!("the picture is not a PPM file: {e}"),
            Self::NoGolden => {
                format!("no golden image at {GOLDEN}; look at the picture, then run with --bless")
            }
            Self::Same { width, height } => {
                format!("the writeback picture ({width}x{height}) equals the golden image")
            }
            Self::Differs(diff) => {
                format!("the writeback picture differs from the golden image: {diff}")
            }
            Self::Blessed => format!(
                "the picture is now the golden image {GOLDEN}; read it before you commit it"
            ),
        }
    }
}

fn judge(
    root: &Path,
    out_dir: &Path,
    label: &str,
    parsed: &CheckOutput,
    bless: bool,
) -> Result<Verdict, OsError> {
    let Some(bytes) = &parsed.picture else {
        return Ok(Verdict::NoPicture);
    };
    let actual = match Image::from_ppm(bytes) {
        Ok(image) => image,
        Err(e) => return Ok(Verdict::BadPicture(e.to_string())),
    };
    let ppm = out_dir.join(format!("{label}.ppm"));
    write_bytes(&ppm, bytes)?;
    let png = crate::ui::png::encode(actual.width(), actual.height(), actual.rgb());
    write_bytes(&out_dir.join(format!("{label}.png")), &png)?;
    eprintln!("xtask: the picture is in {}", ppm.display());
    let golden = root.join(GOLDEN);
    if bless {
        write_bytes(&golden, &actual.to_ppm())?;
        return Ok(Verdict::Blessed);
    }
    let expected = match std::fs::read(&golden) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Verdict::NoGolden),
        Err(source) => {
            return Err(OsError::Io {
                path: golden,
                source,
            });
        }
    };
    let expected = Image::from_ppm(&expected)
        .map_err(|e| OsError::Usage(format!("{}: {e}", golden.display())))?;
    Ok(match expected.diff(&actual) {
        None => Verdict::Same {
            width: actual.width(),
            height: actual.height(),
        },
        Some(diff) => {
            let base = out_dir.join(label);
            crate::ui::write_report_images(&base, &expected, &actual)
                .map_err(|e| OsError::Usage(e.to_string()))?;
            Verdict::Differs(diff.to_string())
        }
    })
}

struct Report<'a> {
    label: &'a str,
    accel: Accel,
    first_frame_host: Option<Duration>,
    screen_host: Option<Duration>,
    parsed: &'a CheckOutput,
    verdict: &'a Verdict,
    code: i32,
}

impl Report<'_> {
    fn text(&self) -> String {
        let ms = |d: Option<u64>| d.map_or_else(|| "unknown".to_owned(), format_secs);
        let host =
            |d: Option<Duration>| ms(d.map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)));
        let mut text = String::new();
        text.push_str(&format!(
            "boot-check {}: QEMU {} (emulated time, not a hardware time)\n",
            self.label,
            self.accel.name()
        ));
        text.push_str(&format!(
            "  first frame of the frontend: {} after QEMU start, guest uptime {}\n",
            host(self.first_frame_host),
            ms(self.parsed.first_frame_uptime_ms)
        ));
        text.push_str(&format!(
            "  game list on screen: {} after QEMU start, guest uptime {}\n",
            host(self.screen_host),
            ms(self.parsed.screen_uptime_ms)
        ));
        if let Some(mode) = &self.parsed.mode {
            text.push_str(&format!("  mode: {mode}\n"));
        }
        text.push_str(&format!("  picture: {}\n", self.verdict.summary()));
        text.push_str(&format!("  guest script exit code: {}\n", self.code));
        let result = if self.code == 0 && self.verdict.passed() {
            "PASS"
        } else {
            "FAIL"
        };
        text.push_str(&format!("boot-check {}: {result}\n", self.label));
        text
    }
}

fn format_secs(ms: u64) -> String {
    format!("{}.{:03} s", ms / 1000, ms % 1000)
}

fn is_picture_line(line: &str) -> bool {
    !line.is_empty()
        && !line.contains(' ')
        && line
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
}

fn require_file(path: &Path, arch: Arch) -> Result<(), OsError> {
    if path.is_file() {
        Ok(())
    } else {
        Err(OsError::NoImage {
            missing: path.to_path_buf(),
            arch: arch.name(),
            kernel: arch.kernel_image(),
        })
    }
}

fn uefi_firmware(qemu_binary: &Path, out_dir: &Path) -> Result<Firmware, OsError> {
    let candidates = uefi_candidates(qemu_binary);
    let Some((code, vars)) = candidates
        .iter()
        .find(|(code, vars)| code.is_file() && vars.is_file())
    else {
        return Err(OsError::ToolMissing {
            tool: "edk2-x86_64-code.fd".to_owned(),
            hint: "install the UEFI firmware of QEMU (it comes with QEMU on macOS; Debian: apt install qemu-efi or ovmf)",
        });
    };
    // QEMU opens the variable store for writing, also under -snapshot; a
    // copy keeps the installed file unchanged.
    let copy = out_dir.join("uefi-vars.fd");
    std::fs::create_dir_all(out_dir).map_err(|source| OsError::Io {
        path: out_dir.to_path_buf(),
        source,
    })?;
    std::fs::copy(vars, &copy).map_err(|source| OsError::Io {
        path: copy.clone(),
        source,
    })?;
    Ok(Firmware::Uefi {
        code: code.clone(),
        vars: copy,
    })
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), OsError> {
    let io_err = |source| OsError::Io {
        path: path.to_path_buf(),
        source,
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io_err)?;
    }
    std::fs::write(path, bytes).map_err(io_err)
}

/// The output lines of the guest script, each with the time since this
/// value was made.
struct TimedLines {
    start: Instant,
    partial: Vec<u8>,
    lines: Vec<(Duration, String)>,
}

impl TimedLines {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            partial: Vec::new(),
            lines: Vec::new(),
        }
    }
}

impl Write for TimedLines {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        for &byte in buf {
            if byte == b'\n' {
                let line = String::from_utf8_lossy(&self.partial).into_owned();
                self.lines.push((self.start.elapsed(), line));
                self.partial.clear();
            } else {
                self.partial.push(byte);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Five minutes of emulated x86-64 boot under TCG, with room to spare.
const DEFAULT_TIMEOUT_SECS: u64 = 900;
/// The capture tool reports a missing screen this long before the guest
/// deadline.
const GUEST_MARGIN_SECS: u64 = 30;
/// The library scan and the first frames settle before the capture.
const SETTLE_MS: u64 = 3000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_bios_and_a_long_deadline() {
        let a = BootCheckArgs::parse(&[], Arch::X86_64).unwrap();
        assert_eq!(a.arch, Arch::X86_64);
        assert_eq!(a.firmware, FirmwareKind::Bios);
        assert_eq!(a.timeout_secs, 900);
        assert!(!a.bless);
        assert_eq!(a.images, None);
    }

    #[test]
    fn flags_parse() {
        let a = BootCheckArgs::parse(
            &[
                "--arch",
                "x86_64",
                "--firmware",
                "uefi",
                "--timeout",
                "60",
                "--images",
                "/i",
                "--bless",
            ],
            Arch::Aarch64,
        )
        .unwrap();
        assert_eq!(a.firmware, FirmwareKind::Uefi);
        assert_eq!(a.timeout_secs, 60);
        assert_eq!(a.images, Some(PathBuf::from("/i")));
        assert!(a.bless);
    }

    #[test]
    fn bad_arguments_are_rejected() {
        for bad in [
            &["--firmware", "coreboot"][..],
            &["--timeout", "0"],
            &["--timeout"],
            &["--bogus", "x"],
            &["--arch", "aarch64", "--firmware", "uefi"],
        ] {
            assert!(BootCheckArgs::parse(bad, Arch::X86_64).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_guest_script_waits_for_the_game_list_inside_the_deadline() {
        let script = guest_script(900);
        assert!(
            script.starts_with(
                "neptron-capture --screen notice --timeout-ms 870000 --settle-ms 3000 && exit 0\n"
            ),
            "{script}"
        );
        assert!(script.contains("neptronctl $command"), "{script}");
        assert!(script.ends_with("exit 1\n"), "{script}");
        assert!(guest_script(10).contains("--timeout-ms 1000 "));
    }

    #[test]
    fn base64_decodes_with_and_without_padding() {
        let lines = |s: &[&str]| s.iter().map(|l| (*l).to_owned()).collect::<Vec<_>>();
        assert_eq!(decode_base64(&lines(&["TWFu"])).unwrap(), b"Man");
        assert_eq!(decode_base64(&lines(&["TWE="])).unwrap(), b"Ma");
        assert_eq!(decode_base64(&lines(&["TQ=="])).unwrap(), b"M");
        assert_eq!(
            decode_base64(&lines(&["UDYKMSAx", "CjI1NQr/AAA="])).unwrap(),
            b"P6\n1 1\n255\n\xff\0\0"
        );
        assert_eq!(
            decode_base64(&lines(&["TWFu", "T!=="])),
            Err(CaptureError::NotBase64 { line: 2 })
        );
    }

    fn strings(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|l| (*l).to_owned()).collect()
    }

    #[test]
    fn the_output_gives_the_uptime_the_mode_and_the_picture() {
        let out = parse_output(&strings(&[
            "neptron: some log line",
            "frame 1 uptime 9.87",
            "screen home uptime 12.34",
            "mode 320x240 clock 6700 kHz",
            "NEPTRON-CAPTURE-BEGIN 1 1",
            "UDYKMSAx",
            "CjI1NQr/AAA=",
            "NEPTRON-CAPTURE-END",
        ]))
        .unwrap();
        assert_eq!(out.first_frame_uptime_ms, Some(9_870));
        assert_eq!(out.screen_uptime_ms, Some(12_340));
        assert_eq!(out.mode.as_deref(), Some("320x240 clock 6700 kHz"));
        let picture = out.picture.unwrap();
        let image = Image::from_ppm(&picture).unwrap();
        assert_eq!((image.width(), image.height()), (1, 1));
    }

    #[test]
    fn a_picture_with_no_end_is_an_error_and_no_picture_is_none() {
        assert_eq!(
            parse_output(&strings(&["NEPTRON-CAPTURE-BEGIN 1 1", "UDYK"])),
            Err(CaptureError::Unterminated)
        );
        let out = parse_output(&strings(&["FAIL: no writeback connector"])).unwrap();
        assert_eq!(out.picture, None);
        assert_eq!(out.screen_uptime_ms, None);
    }

    #[test]
    fn picture_lines_are_left_out_of_the_printed_output() {
        assert!(is_picture_line("UDYKMSAx"));
        assert!(!is_picture_line("screen home uptime 1.00"));
        assert!(!is_picture_line(""));
    }

    #[test]
    fn uefi_firmware_is_looked_for_next_to_qemu_first() {
        let c = uefi_candidates(Path::new(
            "/opt/homebrew/Cellar/qemu/11/bin/qemu-system-x86_64",
        ));
        assert_eq!(
            c[0],
            (
                PathBuf::from("/opt/homebrew/Cellar/qemu/11/share/qemu/edk2-x86_64-code.fd"),
                PathBuf::from("/opt/homebrew/Cellar/qemu/11/share/qemu/edk2-i386-vars.fd")
            )
        );
        assert_eq!(c[1].0, PathBuf::from("/usr/share/qemu/edk2-x86_64-code.fd"));
    }

    #[test]
    fn the_verdict_passes_only_on_an_equal_picture() {
        assert!(
            Verdict::Same {
                width: 1,
                height: 1
            }
            .passed()
        );
        for v in [
            Verdict::NoPicture,
            Verdict::NoGolden,
            Verdict::Blessed,
            Verdict::Differs("1 pixel".into()),
            Verdict::BadPicture("x".into()),
        ] {
            assert!(!v.passed(), "{v:?}");
        }
    }

    #[test]
    fn the_report_says_the_time_is_emulated() {
        let parsed = CheckOutput {
            first_frame_uptime_ms: Some(4_000),
            screen_uptime_ms: Some(5_250),
            picture: None,
            mode: Some("320x240".into()),
        };
        let verdict = Verdict::Same {
            width: 320,
            height: 240,
        };
        let text = Report {
            label: "x86_64-bios",
            accel: Accel::Tcg,
            first_frame_host: Some(Duration::from_millis(59_000)),
            screen_host: Some(Duration::from_millis(61_005)),
            parsed: &parsed,
            verdict: &verdict,
            code: 0,
        }
        .text();
        assert!(
            text.contains("emulated time, not a hardware time"),
            "{text}"
        );
        assert!(
            text.contains("61.005 s after QEMU start, guest uptime 5.250 s"),
            "{text}"
        );
        assert!(text.ends_with("boot-check x86_64-bios: PASS\n"), "{text}");
    }

    #[test]
    fn timed_lines_split_writes_at_newlines() {
        let mut t = TimedLines::new();
        write!(t, "a").unwrap();
        writeln!(t, "b").unwrap();
        t.write_all(b"c\nd\n").unwrap();
        let lines: Vec<&str> = t.lines.iter().map(|(_, l)| l.as_str()).collect();
        assert_eq!(lines, ["ab", "c", "d"]);
    }
}
