//! `cargo xtask core-trace`: the tier T4 check of the real cores.
//!
//! It checks the test ROMs against their checksums, builds the example
//! `core_trace` of `neptron-libretro` for the guest, runs every case in the
//! QEMU guest, and compares each trace with the committed one in
//! `tests/traces`. `--bless` writes the traces of this run instead.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use super::host::{Cache, MachineLock};
use super::qemu::{Job, QemuArgs};
use super::trace::{self, CASES, Case, Output};
use super::{Arch, OsError, build, cross};
use crate::repo::Repo;
use crate::sha256;

/// The arguments of `core-trace`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceArgs {
    /// The target.
    pub arch: Arch,
    /// The guest deadline.
    pub timeout: Duration,
    /// Write the traces of this run to `tests/traces`.
    pub bless: bool,
    /// Frames of each plain case to write as PNG files.
    pub dumps: Vec<u32>,
}

impl TraceArgs {
    /// Parses `[--arch A] [--timeout SECS] [--bless] [--dump FRAME[,FRAME...]]`.
    pub fn parse(args: &[&str], default_arch: Arch) -> Result<Self, OsError> {
        let mut out = Self {
            arch: default_arch,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            bless: false,
            dumps: Vec::new(),
        };
        let mut it = args.iter();
        while let Some(&arg) = it.next() {
            let mut value = || {
                it.next()
                    .copied()
                    .ok_or_else(|| OsError::Usage(format!("{arg} needs a value")))
            };
            match arg {
                "--arch" => out.arch = Arch::parse(value()?)?,
                "--timeout" => {
                    let text = value()?;
                    out.timeout = match text.parse::<u64>() {
                        Ok(secs) if secs > 0 => Duration::from_secs(secs),
                        _ => return Err(bad_number("--timeout", text)),
                    };
                }
                "--bless" => out.bless = true,
                "--dump" => {
                    for frame in value()?.split(',') {
                        match frame.parse::<u32>() {
                            Ok(f) if f < trace::FRAMES => out.dumps.push(f),
                            _ => return Err(bad_number("--dump", frame)),
                        }
                    }
                }
                other => return Err(OsError::Usage(format!("unknown argument {other:?}"))),
            }
        }
        Ok(out)
    }
}

/// Runs a whole case set and every core in about 30 s on an Apple Silicon
/// Mac; the margin covers a slow CI host.
const DEFAULT_TIMEOUT_SECS: u64 = 600;
const EXAMPLE: &str = "core_trace";

fn bad_number(flag: &'static str, value: &str) -> OsError {
    OsError::BadNumber {
        flag,
        value: value.to_owned(),
    }
}

/// `cargo xtask core-trace`.
pub fn run(args: &[&str]) -> Result<ExitCode, OsError> {
    let args = TraceArgs::parse(args, Arch::default_for(std::env::consts::ARCH))?;
    let repo = Repo::discover()?;
    let cache = Cache::from_env()?;
    super::host::require_tool(
        "docker",
        "install Docker (macOS: Docker Desktop) to build for the guest",
    )?;
    super::check_docker_daemon()?;
    let work = repo.root.join("target").join("core-trace");
    let share = work.join("share");
    recreate(&share)?;

    let roms = repo.root.join("tests").join("roms");
    let rom_cache = rom_cache_dir(&cache, std::env::var_os(ROM_CACHE_ENV).as_deref());
    let (cases, skipped) = stage_content(&roms, &rom_cache, &share)?;
    for (case, reason) in &skipped {
        eprintln!("xtask: SKIPPED {case}: {reason}");
    }

    build_program(&repo.root, &cache, args.arch, &share)?;
    let script = work.join("run.sh");
    super::write_file(&script, &trace::guest_script(&cases, &args.dumps))?;
    let timeout = args.timeout;
    let qemu = QemuArgs {
        arch: args.arch,
        images: None,
        timeout,
        share: Some(share.clone()),
        job: Job::Script(script),
    };
    let mut console = Vec::new();
    let code = super::boot(&qemu, &mut console)?;
    let lines: Vec<String> = String::from_utf8_lossy(&console)
        .lines()
        .map(str::to_owned)
        .collect();
    let output = Output::parse(&lines);
    for line in &output.other {
        eprintln!("xtask: guest: {line}");
    }
    write_dumps(&work.join("frames"), &output)?;

    let traces = repo.root.join("tests").join("traces");
    let mut failures = skipped.len();
    if code != Some(0) {
        eprintln!("xtask: the guest script exited with {code:?}");
        failures += 1;
    }
    for case in &cases {
        failures += usize::from(!check_case(case, &output, &work, &traces, args.bless)?);
    }
    eprintln!(
        "xtask: core-trace on {}: {} cases, {} failed or skipped{}",
        args.arch.name(),
        CASES.len(),
        failures,
        if args.bless { " (traces written)" } else { "" }
    );
    Ok(super::exit_code(failures == 0))
}

/// The directory under the machine cache that holds the test ROMs that the
/// repository cannot hold. `tests/roms/README.md` says how to fill it.
pub const ROM_CACHE_SUBDIR: &str = "test-roms";
/// Environment variable that names another directory for those ROMs.
pub const ROM_CACHE_ENV: &str = "NEPTRON_TEST_ROMS";

/// The directory of the cached test ROMs: `env` when it is set and not
/// empty, else [`ROM_CACHE_SUBDIR`] in the machine cache.
fn rom_cache_dir(cache: &Cache, env: Option<&std::ffi::OsStr>) -> PathBuf {
    match env {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => cache.root.join(ROM_CACHE_SUBDIR),
    }
}

/// A case that does not run, with the reason.
type Skipped = (&'static str, String);

/// Where the ROM of `case` is: in `roms` at its path when the repository
/// holds it, else in `rom_cache` by its file name.
fn find_rom(case: &Case, roms: &Path, rom_cache: &Path) -> Option<PathBuf> {
    [roms.join(case.rom), rom_cache.join(case.rom_file())]
        .into_iter()
        .find(|p| p.is_file())
}

/// Copies each ROM whose checksum matches into the shared directory, and
/// writes the zip file of each zip case. Returns the cases to run and the
/// skipped ones.
fn stage_content(
    roms: &Path,
    rom_cache: &Path,
    share: &Path,
) -> Result<(Vec<Case>, Vec<Skipped>), OsError> {
    let sums_path = roms.join("SHA256SUMS");
    let sums = std::fs::read_to_string(&sums_path)
        .map_err(|source| OsError::Io {
            path: sums_path.clone(),
            source,
        })
        .and_then(|text| {
            trace::parse_sums(&text)
                .map_err(|e| OsError::Usage(format!("{}: {e}", sums_path.display())))
        })?;
    let mut cases = Vec::new();
    let mut skipped = Vec::new();
    for case in CASES {
        let Some((expected, _)) = sums.iter().find(|(_, p)| p == case.rom) else {
            skipped.push((case.name, format!("{} is not in SHA256SUMS", case.rom)));
            continue;
        };
        let Some(path) = find_rom(&case, roms, rom_cache) else {
            skipped.push((
                case.name,
                format!(
                    "{} is missing; get it as tests/roms/README.md says and put it at {}",
                    case.rom_file(),
                    rom_cache.join(case.rom_file()).display()
                ),
            ));
            continue;
        };
        let data = match std::fs::read(&path) {
            Ok(data) => data,
            Err(e) => {
                skipped.push((case.name, format!("{}: {e}", path.display())));
                continue;
            }
        };
        let found = sha256::hex(&data);
        if &found != expected {
            skipped.push((
                case.name,
                format!("{} has SHA-256 {found}, not {expected}", path.display()),
            ));
            continue;
        }
        let (target, bytes) = if case.zip {
            let zip = trace::stored_zip(case.rom_file(), &data).ok_or_else(|| {
                OsError::Usage(format!("{} is too large for a zip file", path.display()))
            })?;
            (share.join(case.zip_name()), zip)
        } else {
            (share.join("roms").join(case.rom), data)
        };
        write_bytes(&target, &bytes)?;
        cases.push(case);
    }
    Ok((cases, skipped))
}

/// Builds the example for the guest and puts it in the shared directory.
fn build_program(repo: &Path, cache: &Cache, arch: Arch, share: &Path) -> Result<(), OsError> {
    let dockerfile = repo.join("Dockerfile");
    let text = std::fs::read_to_string(&dockerfile).map_err(|source| OsError::Io {
        path: dockerfile.clone(),
        source,
    })?;
    let plan = cross::CrossPlan {
        arch,
        container: build::container_image(&text),
        repo: repo.to_path_buf(),
        export: share.to_path_buf(),
        package: "neptron-libretro".to_owned(),
        features: String::new(),
        example: EXAMPLE.to_owned(),
        lint: false,
    };
    // The build reads the toolchains in the Buildroot output, which an os
    // build changes.
    let _lock = MachineLock::take(&cache.build_lock(), "Buildroot build")?;
    build::build_container(&build::container_build(&plan.container), &dockerfile)?;
    for step in plan.steps() {
        super::run_step(Command::new("docker").args(step))?;
    }
    Ok(())
}

/// Compares or writes the trace of one case. Returns whether it passed.
fn check_case(
    case: &Case,
    output: &Output,
    work: &Path,
    traces: &Path,
    bless: bool,
) -> Result<bool, OsError> {
    let Some(actual) = output.traces.get(case.name) else {
        eprintln!("xtask: FAIL {}: the guest printed no trace", case.name);
        return Ok(false);
    };
    let text = trace::trace_text(actual);
    super::write_file(
        &work.join("actual").join(format!("{}.trace", case.name)),
        &text,
    )?;
    let mut passed = actual.last().is_some_and(|l| l == "exit 0");
    if !passed {
        eprintln!(
            "xtask: FAIL {}: the program did not end with exit 0",
            case.name
        );
    }
    if let Some(plain) = case.plain()
        && let Some(plain_lines) = output.traces.get(plain.name)
        && let Err(e) = trace::zip_matches_plain(&trace::trace_text(plain_lines), &text)
    {
        eprintln!(
            "xtask: FAIL {}: differs from {}: {e}",
            case.name, plain.name
        );
        passed = false;
    }
    let path = traces.join(format!("{}.trace", case.name));
    if bless {
        super::write_file(&path, &text)?;
        eprintln!("xtask: wrote {}", path.display());
        return Ok(passed);
    }
    let expected = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!(
                "xtask: FAIL {}: no committed trace {} ({e}); read the trace of this run \
                 in target/core-trace/actual and record it with --bless",
                case.name,
                path.display()
            );
            return Ok(false);
        }
    };
    let differences = trace::compare(&expected, actual);
    if differences.is_empty() {
        eprintln!("xtask: ok {}", case.name);
        return Ok(passed);
    }
    eprintln!(
        "xtask: FAIL {}: {} lines differ from {}",
        case.name,
        differences.len(),
        path.display()
    );
    for d in differences.iter().take(SHOWN_DIFFERENCES) {
        eprintln!(
            "  line {}: expected {:?}\n  {:width$}  actual   {:?}",
            d.line,
            d.expected.as_deref().unwrap_or("<none>"),
            "",
            d.actual.as_deref().unwrap_or("<none>"),
            width = d.line.to_string().len() + 5
        );
    }
    Ok(false)
}

const SHOWN_DIFFERENCES: usize = 10;

/// Writes each dumped frame as a PNG file.
fn write_dumps(dir: &Path, output: &Output) -> Result<(), OsError> {
    for ((case, frame), dump) in &output.dumps {
        let path = dir.join(format!("{case}-{frame:04}.png"));
        match dump.rgb() {
            Some(rgb) => {
                write_bytes(
                    &path,
                    &crate::ui::png::encode(dump.width, dump.height, &rgb),
                )?;
                eprintln!("xtask: frame {frame} of {case} in {}", path.display());
            }
            None => eprintln!("xtask: frame {frame} of {case} arrived damaged; no image"),
        }
    }
    Ok(())
}

fn recreate(dir: &Path) -> Result<(), OsError> {
    if let Err(source) = std::fs::remove_dir_all(dir)
        && source.kind() != std::io::ErrorKind::NotFound
    {
        return Err(OsError::Io {
            path: dir.to_path_buf(),
            source,
        });
    }
    std::fs::create_dir_all(dir).map_err(|source| OsError::Io {
        path: dir.to_path_buf(),
        source,
    })
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), OsError> {
    let io_err = |source| OsError::Io {
        path: PathBuf::from(path),
        source,
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io_err)?;
    }
    std::fs::write(path, bytes).map_err(io_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_parse() {
        let a = TraceArgs::parse(&[], Arch::Aarch64).unwrap();
        assert_eq!(a.arch, Arch::Aarch64);
        assert_eq!(a.timeout, Duration::from_secs(600));
        assert!(!a.bless);
        assert!(a.dumps.is_empty());
        let a = TraceArgs::parse(
            &[
                "--arch",
                "x86_64",
                "--bless",
                "--dump",
                "59,899",
                "--timeout",
                "90",
            ],
            Arch::Aarch64,
        )
        .unwrap();
        assert_eq!(a.arch, Arch::X86_64);
        assert!(a.bless);
        assert_eq!(a.dumps, [59, 899]);
        assert_eq!(a.timeout, Duration::from_secs(90));
        for bad in [
            &["--dump", "900"][..],
            &["--dump", "x"],
            &["--timeout", "0"],
            &["--arch"],
            &["--images", "x"],
        ] {
            assert!(TraceArgs::parse(bad, Arch::Aarch64).is_err(), "{bad:?}");
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xtask-trace-{name}-{}", std::process::id()));
        recreate(&dir).unwrap();
        dir
    }

    #[test]
    fn roms_come_from_the_repository_or_the_cache_and_bad_ones_are_skipped() {
        let roms = temp_dir("roms");
        let rom_cache = temp_dir("rom-cache");
        let share = temp_dir("share");
        let nes = b"NES\x1a";
        let snes = b"SNES";
        write_bytes(&roms.join("nes/240pee.nes"), nes).unwrap();
        write_bytes(&rom_cache.join("240pSuite.sfc"), snes).unwrap();
        write_bytes(&rom_cache.join("240pSuite-1.32.bin"), b"other").unwrap();
        let sums = format!(
            "{}  nes/240pee.nes\n{}  snes/240pSuite.sfc\n{}  megadrive/240pSuite-1.32.bin\n",
            sha256::hex(nes),
            sha256::hex(snes),
            sha256::hex(b"md")
        );
        write_bytes(&roms.join("SHA256SUMS"), sums.as_bytes()).unwrap();
        let (cases, skipped) = stage_content(&roms, &rom_cache, &share).unwrap();
        let names: Vec<&str> = cases.iter().map(|c| c.name).collect();
        assert_eq!(names, ["fceumm", "fceumm-zip", "snes9x", "snes9x-zip"]);
        let names: Vec<&str> = skipped.iter().map(|(c, _)| *c).collect();
        assert_eq!(names, ["genesis_plus_gx", "genesis_plus_gx-zip"]);
        assert!(skipped[0].1.contains("has SHA-256"), "{}", skipped[0].1);
        assert_eq!(
            std::fs::read(share.join("roms/nes/240pee.nes")).unwrap(),
            nes
        );
        assert_eq!(
            std::fs::read(share.join("roms/snes/240pSuite.sfc")).unwrap(),
            snes
        );
        let zip = std::fs::read(share.join("zips/fceumm-zip.zip")).unwrap();
        assert_eq!(zip, trace::stored_zip("240pee.nes", nes).unwrap());

        std::fs::remove_dir_all(&rom_cache).unwrap();
        let (cases, skipped) = stage_content(&roms, &rom_cache, &share).unwrap();
        assert_eq!(cases.len(), 2);
        assert_eq!(skipped.len(), 4);
        let reason = &skipped[0].1;
        assert!(
            reason.starts_with("240pSuite.sfc is missing")
                && reason.contains(&rom_cache.join("240pSuite.sfc").display().to_string()),
            "{reason}"
        );
        std::fs::remove_dir_all(&roms).unwrap();
        std::fs::remove_dir_all(&share).unwrap();
    }

    #[test]
    fn the_rom_cache_is_in_the_machine_cache_unless_the_variable_names_one() {
        let cache = Cache {
            root: PathBuf::from("/c"),
        };
        assert_eq!(rom_cache_dir(&cache, None), Path::new("/c/test-roms"));
        let empty = std::ffi::OsString::new();
        assert_eq!(
            rom_cache_dir(&cache, Some(&empty)),
            Path::new("/c/test-roms")
        );
        let other = std::ffi::OsString::from("/roms");
        assert_eq!(rom_cache_dir(&cache, Some(&other)), Path::new("/roms"));
    }
}
