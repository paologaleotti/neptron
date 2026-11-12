//! `cargo xtask os selftest-check`: the gate of the video self-test. It
//! builds `neptronctl` with the `kms` feature for the guest, runs
//! `neptronctl selftest video` on `vkms` for each profile with a short time
//! per mode, and checks the JSON reports.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use serde_json::Value;

use super::host::{Cache, MachineLock};
use super::qemu::{Job, QemuArgs};
use super::{Arch, OsError, build, cross};
use crate::repo::Repo;

/// The profiles the check runs: a 31 kHz PC monitor and the 15 kHz safe
/// start. On `vkms` the GPU is unknown, so both get the conservative GPU
/// values.
pub const PROFILES: [&str; 2] = ["pc_31_120", "generic_15"];
/// The display time of each mode in the guest, in seconds: enough flips
/// for the period with software GL in the dev build.
pub const SECONDS_PER_MODE_S: u32 = 2;
/// The guest deadline when `--timeout` is not given: the build of the
/// guest program is not part of it.
pub const DEFAULT_TIMEOUT_SECS: u64 = 600;
/// The exit code of a refused self-test run.
pub const REFUSED_EXIT_CODE: i32 = 6;

const PACKAGE: &str = "neptronctl";
const FEATURES: &str = "kms";
const GUEST_DIR: &str = "/mnt/host";
const REPORT_LINE: &str = "selftest-report";
const EXIT_LINE: &str = "selftest-exit";
const REFUSAL_LINE: &str = "selftest-refusal-exit";

/// Runs the check: build, guest run, report checks. The exit code is 0
/// when every report passes.
pub fn run(args: &[&str]) -> Result<ExitCode, OsError> {
    let (arch, timeout) = super::parse_compare_args(
        args,
        Arch::default_for(std::env::consts::ARCH),
        DEFAULT_TIMEOUT_SECS,
    )?;
    let repo = Repo::discover()?;
    let cache = Cache::from_env()?;
    super::host::require_tool(
        "docker",
        "install Docker (macOS: Docker Desktop) to build for the guest",
    )?;
    super::check_docker_daemon()?;
    let work = repo.root.join("target").join("selftest-check");
    let share = work.join("share");
    recreate(&share)?;
    copy_presets(
        &repo.root.join("data").join("monitors"),
        &share.join("monitors"),
    )?;
    build_program(&repo.root, &cache, arch, &share)?;
    let script = work.join("run.sh");
    super::write_file(&script, &guest_script(&PROFILES, SECONDS_PER_MODE_S))?;
    let qemu = QemuArgs {
        arch,
        images: None,
        timeout: Duration::from_secs(timeout),
        share: Some(share),
        job: Job::Script(script),
    };
    let mut console = Vec::new();
    let code = super::boot(&qemu, &mut console)?;
    let text = String::from_utf8_lossy(&console);
    let output = Output::parse(&text);
    for line in &output.other {
        println!("{line}");
    }
    let mut failures = check_output(&output, &PROFILES);
    if code != Some(0) {
        failures.push(format!("the guest script exited with {code:?}"));
    }
    for failure in &failures {
        eprintln!("xtask: selftest-check: FAIL {failure}");
    }
    println!(
        "selftest-check on {}: {} profiles, {} failures",
        arch.name(),
        PROFILES.len(),
        failures.len()
    );
    Ok(super::exit_code(failures.is_empty()))
}

/// The guest script: a refused run (a confirmation that names another
/// profile), then each profile on `vkms`, each followed by its exit code
/// and its report on one line.
pub fn guest_script(profiles: &[&str], seconds: u32) -> String {
    let program = format!("{GUEST_DIR}/{PACKAGE}");
    let common = format!("--monitors {GUEST_DIR}/monitors --device /dev/dri/card0");
    let mut s = String::from("f=0\n");
    s.push_str(&format!(
        "{program} selftest video --profile {p} {common} --report /tmp/refused.json \
         --i-have-checked-the-monitor not_{p}\necho \"{REFUSAL_LINE} $?\"\n",
        p = profiles.first().copied().unwrap_or("pc_31_120"),
    ));
    for p in profiles {
        let report = format!("/tmp/selftest-{p}.json");
        s.push_str(&format!(
            "{program} selftest video --profile {p} {common} --report {report} \
             --seconds-per-mode {seconds} --i-have-checked-the-monitor {p}\n\
             code=$?\necho \"{EXIT_LINE} {p} $code\"\n\
             [ \"$code\" = 0 ] || f=$((f+1))\n\
             printf '{REPORT_LINE} {p} '; tr -d '\\n' < {report}; echo\n"
        ));
    }
    s.push_str("exit $f\n");
    s
}

/// The guest output, taken apart.
#[derive(Debug, Default, PartialEq)]
pub struct Output {
    /// The report of each profile.
    pub reports: Vec<(String, Value)>,
    /// The exit code of each profile's run.
    pub exits: Vec<(String, i32)>,
    /// The exit code of the refused run.
    pub refusal: Option<i32>,
    /// A report line that is not JSON.
    pub broken: Vec<String>,
    /// Every other line: the tool's own text.
    pub other: Vec<String>,
}

impl Output {
    /// Takes the guest output apart.
    pub fn parse(text: &str) -> Self {
        let mut out = Self::default();
        for line in text.lines() {
            let words: Vec<&str> = line.splitn(3, ' ').collect();
            match words.as_slice() {
                [REPORT_LINE, profile, json] => match serde_json::from_str(json) {
                    Ok(v) => out.reports.push(((*profile).to_owned(), v)),
                    Err(_) => out.broken.push((*profile).to_owned()),
                },
                [EXIT_LINE, profile, code] => {
                    out.exits
                        .push(((*profile).to_owned(), code.trim().parse().unwrap_or(-1)));
                }
                [REFUSAL_LINE, code] => out.refusal = code.trim().parse().ok(),
                _ => out.other.push(line.to_owned()),
            }
        }
        out
    }
}

/// Every failure of the output: a profile with no report or no exit code
/// 0, a broken report line, a refusal that did not refuse, and each
/// failure of a report.
pub fn check_output(output: &Output, profiles: &[&str]) -> Vec<String> {
    let mut failures = Vec::new();
    if output.refusal != Some(REFUSED_EXIT_CODE) {
        failures.push(format!(
            "a confirmation of another profile exited with {:?}, not {REFUSED_EXIT_CODE}",
            output.refusal
        ));
    }
    for profile in &output.broken {
        failures.push(format!("{profile}: the report line is not JSON"));
    }
    for p in profiles {
        match output.exits.iter().find(|(name, _)| name == p) {
            Some((_, 0)) => {}
            Some((_, code)) => failures.push(format!("{p}: neptronctl exited with {code}")),
            None => failures.push(format!("{p}: no exit code")),
        }
        match output.reports.iter().find(|(name, _)| name == p) {
            Some((_, report)) => {
                failures.extend(
                    check_report(report)
                        .into_iter()
                        .map(|f| format!("{p}: {f}")),
                );
            }
            None => failures.push(format!("{p}: no report")),
        }
    }
    failures
}

/// The failures of one report: the run on a display finished and passed,
/// with at least one mode; every mode passed with its read back equal to
/// the request and valid; the output went off at the end.
pub fn check_report(report: &Value) -> Vec<String> {
    let mut failures = Vec::new();
    let mut expect = |ok: bool, what: &str| {
        if !ok {
            failures.push(what.to_owned());
        }
    };
    expect(report["report_version"] == 1, "report_version is not 1");
    expect(report["dry_run"] == false, "the run was a dry run");
    expect(report["state"] == "finished", "the run did not finish");
    expect(report["pass"] == true, "the report does not pass");
    expect(
        report["restore"]["ok"] == true,
        "the restore did not succeed",
    );
    let modes = report["modes"].as_array().map_or(&[][..], Vec::as_slice);
    expect(!modes.is_empty(), "no mode ran");
    for m in modes {
        let family = m["family"].as_str().unwrap_or("?");
        if m["pass"] != true {
            failures.push(format!("{family}: the mode failed: {}", m["failures"]));
        }
        if m["readback_equal"] != true || m["readback"] != m["request"] {
            failures.push(format!("{family}: the read back is not the request"));
        }
        if m["readback_valid"] != true {
            failures.push(format!("{family}: the read back is not valid"));
        }
    }
    failures
}

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
        package: PACKAGE.to_owned(),
        features: FEATURES.to_owned(),
        example: String::new(),
        // No host gate compiles the Linux code of the kms feature.
        lint: true,
        unwrapped_linker: false,
    };
    // The build reads the toolchains in the Buildroot output, which an os
    // build changes.
    let _lock = MachineLock::take(&cache.build_lock(), "Buildroot build")?;
    build::build_container(&build::container_build(&plan.container), &dockerfile)?;
    for step in plan.bin_steps(PACKAGE) {
        super::run_step(Command::new("docker").args(step))?;
    }
    Ok(())
}

/// Copies the preset files, which the image of the guest does not hold.
fn copy_presets(from: &Path, to: &Path) -> Result<(), OsError> {
    let io = |path: &Path| {
        let path = path.to_path_buf();
        move |source| OsError::Io { path, source }
    };
    std::fs::create_dir_all(to).map_err(io(to))?;
    for entry in std::fs::read_dir(from).map_err(io(from))? {
        let path = entry.map_err(io(from))?.path();
        if path.extension().is_some_and(|e| e == "toml")
            && let Some(name) = path.file_name()
        {
            std::fs::copy(&path, to.join(name)).map_err(io(&path))?;
        }
    }
    Ok(())
}

fn recreate(dir: &Path) -> Result<(), OsError> {
    if let Err(source) = std::fs::remove_dir_all(dir)
        && source.kind() != std::io::ErrorKind::NotFound
    {
        return Err(OsError::Io {
            path: PathBuf::from(dir),
            source,
        });
    }
    std::fs::create_dir_all(dir).map_err(|source| OsError::Io {
        path: PathBuf::from(dir),
        source,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn good_report() -> Value {
        let mode = json!({"pclock_hz": 25_000_000, "hactive": 640});
        json!({
            "report_version": 1,
            "dry_run": false,
            "state": "finished",
            "pass": true,
            "restore": {"method": "blank", "ok": true, "error": null},
            "modes": [{
                "family": "480p",
                "pass": true,
                "failures": [],
                "request": mode,
                "readback": mode,
                "readback_equal": true,
                "readback_valid": true,
            }],
        })
    }

    #[test]
    fn the_script_refuses_once_then_runs_each_profile_with_the_short_time() {
        let s = guest_script(&["pc_31_120", "generic_15"], 2);
        assert!(s.starts_with("f=0\n"));
        assert!(s.contains(
            "--report /tmp/refused.json --i-have-checked-the-monitor not_pc_31_120\n\
             echo \"selftest-refusal-exit $?\""
        ));
        for p in ["pc_31_120", "generic_15"] {
            assert!(
                s.contains(&format!(
                    "/mnt/host/neptronctl selftest video --profile {p} --monitors \
                     /mnt/host/monitors --device /dev/dri/card0 --report \
                     /tmp/selftest-{p}.json --seconds-per-mode 2 \
                     --i-have-checked-the-monitor {p}\n"
                )),
                "{s}"
            );
            assert!(s.contains(&format!("tr -d '\\n' < /tmp/selftest-{p}.json")));
        }
        assert!(s.ends_with("exit $f\n"));
    }

    #[test]
    fn the_output_is_taken_apart() {
        let text = format!(
            "Showing 4 modes\nselftest-refusal-exit 6\nselftest-exit pc_31_120 0\n\
             selftest-report pc_31_120 {}\nselftest-report generic_15 {{broken\n",
            good_report()
        );
        let out = Output::parse(&text);
        assert_eq!(out.refusal, Some(6));
        assert_eq!(out.exits, [("pc_31_120".to_owned(), 0)]);
        assert_eq!(out.reports.len(), 1);
        assert_eq!(out.broken, ["generic_15"]);
        assert_eq!(out.other, ["Showing 4 modes"]);
    }

    #[test]
    fn a_good_output_has_no_failure() {
        let out = Output {
            reports: vec![("a".into(), good_report())],
            exits: vec![("a".into(), 0)],
            refusal: Some(REFUSED_EXIT_CODE),
            ..Output::default()
        };
        assert!(check_output(&out, &["a"]).is_empty());
    }

    #[test]
    fn each_missing_or_failed_part_is_a_failure() {
        let out = Output {
            refusal: Some(0),
            exits: vec![("a".into(), 7)],
            broken: vec!["b".into()],
            ..Output::default()
        };
        let failures = check_output(&out, &["a", "b"]);
        for part in [
            "exited with Some(0)",
            "b: the report line is not JSON",
            "a: neptronctl exited with 7",
            "a: no report",
            "b: no exit code",
        ] {
            assert!(
                failures.iter().any(|f| f.contains(part)),
                "{part}: {failures:?}"
            );
        }
    }

    #[test]
    fn a_report_fails_on_each_broken_field() {
        assert!(check_report(&good_report()).is_empty());
        type Breaks = fn(&mut Value);
        let cases: [(&str, Breaks); 7] = [
            ("dry run", |r| r["dry_run"] = json!(true)),
            ("did not finish", |r| r["state"] = json!("stopped")),
            ("does not pass", |r| r["pass"] = json!(false)),
            ("restore", |r| r["restore"]["ok"] = json!(false)),
            ("no mode", |r| r["modes"] = json!([])),
            ("480p: the mode failed", |r| {
                r["modes"][0]["pass"] = json!(false)
            }),
            ("not the request", |r| {
                r["modes"][0]["readback"]["hactive"] = json!(641)
            }),
        ];
        for (what, breaks) in cases {
            let mut r = good_report();
            breaks(&mut r);
            let failures = check_report(&r);
            assert!(
                failures.iter().any(|f| f.contains(what)),
                "{what}: {failures:?}"
            );
        }
    }

    #[test]
    fn the_presets_are_copied_for_the_guest() {
        let dir = std::env::temp_dir().join(format!("xtask-presets-{}", std::process::id()));
        let from = dir.join("from");
        std::fs::create_dir_all(&from).unwrap();
        std::fs::write(from.join("a.toml"), "id = \"a\"").unwrap();
        std::fs::write(from.join("README.md"), "x").unwrap();
        copy_presets(&from, &dir.join("to")).unwrap();
        let names: Vec<_> = std::fs::read_dir(dir.join("to"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["a.toml"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
