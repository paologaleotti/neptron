//! `cargo xtask`: build, verification and worktree automation for NEPTRON.
//!
//! Every Cargo build and test that this tool starts follows the machine-load
//! rules in [`load`].

#![forbid(unsafe_code)]

mod bindgen;
mod load;
mod monitors;
mod os;
mod repo;
mod rules;
mod sha256;
mod ui;
mod verify;
mod worktree;

use std::process::ExitCode;

const USAGE: &str = "\
usage: cargo xtask <command> [args]

commands:
  verify --tier <N>    run tier N of the verification ladder and the tiers
                       below it; writes target/verify/report.json
  check [cargo args]   cargo check, under the machine-load rules
  test [nextest args]  cargo nextest run, under the machine-load rules
  clippy [cargo args]  cargo clippy for all targets, tests included, under the
                       machine-load rules
  worktree new <task>  make .worktrees/<task> on branch mvp/<task> from main,
                       with target/ cloned from the main checkout
  worktree cache-check [--from <checkout>]
                       prove that a build in a cloned target/ compiles no
                       dependency again (default source: the main checkout)
  monitors <monitor.cpp> [dir]
                       convert the Switchres preset table into the TOML
                       files of data/monitors (default dir)
  bindgen              generate the libretro bindings of neptron-libretro-sys
                       from its vendored libretro.h (needs the bindgen tool)
  rules                check the dependency rules, the unsafe allow-list
                       and the headless feature set
  window-smoke         build neptron-window with its sdl3 feature, open the
                       development window, show test modes and inject a
                       key (needs a display and SDL3)
  font                 generate the UI glyph table from the vendored BDF font
  shot <scene>         render one UI scene to target/ui/shot, at 1x and 4x
  gallery              render every UI scene onto target/ui/gallery.png
  golden [--bless] [scene...]
                       compare UI scenes with their golden images; --bless
                       writes the golden of each scene that differs
  os build [--arch A] [--jobs N] [--dev-key FILE] [--no-kernel-15khz]
           [--output NAME] [-- make targets...]
                       build the image of target A with Buildroot in the
                       development container: the aarch64 QEMU test image,
                       or the x86_64 PC image (a USB image that also boots
                       in QEMU); --dev-key FILE adds SSH with that public
                       key and the control commands, --no-kernel-15khz
                       leaves out the 15 kHz kernel patches; each variant
                       has its own output
  os app-check [--arch A]
                       check (warnings denied) and build the frontend with
                       kms and the launcher for Linux in the container
  os selftest-check [--arch A] [--timeout SECS]
                       build neptronctl with kms for the guest, run
                       `selftest video` on vkms in QEMU and check the reports
  os boot-check [--arch A] [--firmware bios|uefi] [--timeout SECS]
                [--images DIR] [--bless]
                       boot the image in QEMU to the game list, compare the
                       vkms writeback picture with its golden image and
                       record the boot time (tier T3)
  os push --host H [--user U] [--port P] [--data] [--no-restart]
                       build the frontend, the launcher and neptronctl in the
                       bench profile and put them in the hot folder of a PC
                       that runs the development variant, over SSH
  os logs --host H [--user U] [--port P] [--follow]
                       print the log of the launcher and the frontend, and
                       dmesg, of that PC
  os ssh --host H [--user U] [--port P] -- command...
                       run a command on that PC
  qemu [--arch A] [--images DIR] [--timeout SECS] [--share DIR]
       [--disk FILE] [--ssh-port PORT] [--beside-launcher]
       [--script FILE | -- command...]
                       boot the QEMU image; run the command in the guest
                       and exit with its exit code, or give a console
  gles-compare [--arch A] [--timeout SECS]
                       build the GLES comparison for the guest in the
                       development container and run it in QEMU (tier T3)
  core-trace [--arch A] [--timeout SECS] [--bless] [--dump FRAME[,FRAME...]]
             [--allow-missing-roms]
                       run the test ROMs in the real cores in QEMU and compare
                       the traces with tests/traces (tier T4)
  kms-check [--arch A] [--timeout SECS]
                       build timing check 7 of neptron-kms for the guest and
                       run it on vkms in QEMU (tier T3)

environment:
  NEPTRON_BUILD_SLOTS  build slots (default on macOS: 2, elsewhere: no lock)
  NEPTRON_BUILD_JOBS   CARGO_BUILD_JOBS per build (default on macOS: 5)
  NEPTRON_BINDGEN      the bindgen executable (default: bindgen on the PATH)
  NEPTRON_CACHE_DIR    machine cache for the OS images (default ~/.cache/neptron)
  NEPTRON_OS_IMAGES    images directory that qemu boots in place of the build
";

/// An error of any xtask command.
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("unknown command {0:?}\n{USAGE}")]
    UnknownCommand(String),
    #[error(transparent)]
    Bindgen(#[from] bindgen::BindgenError),
    #[error(transparent)]
    Monitors(#[from] monitors::MonitorsError),
    #[error(transparent)]
    Os(#[from] os::OsError),
    #[error(transparent)]
    Repo(#[from] repo::RepoError),
    #[error(transparent)]
    Load(#[from] load::LoadError),
    #[error(transparent)]
    Rules(#[from] rules::RulesError),
    #[error(transparent)]
    Ui(#[from] ui::UiError),
    #[error(transparent)]
    Verify(#[from] verify::VerifyError),
    #[error(transparent)]
    Worktree(#[from] worktree::WorktreeError),
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, Error> {
    let Some((cmd, rest)) = args.split_first() else {
        eprint!("{USAGE}");
        return Ok(ExitCode::FAILURE);
    };
    let rest: Vec<&str> = rest.iter().map(String::as_str).collect();
    match cmd.as_str() {
        "verify" => Ok(verify::run(&rest)?),
        "worktree" => Ok(worktree::run(&rest)?),
        "monitors" => {
            monitors::run(&rest)?;
            Ok(ExitCode::SUCCESS)
        }
        "bindgen" => {
            bindgen::run(&rest)?;
            Ok(ExitCode::SUCCESS)
        }
        "font" => Ok(ui::font(&rest)?),
        "shot" => Ok(ui::shot(&rest)?),
        "gallery" => Ok(ui::gallery(&rest)?),
        "golden" => Ok(ui::golden(&rest)?),
        "os" => Ok(os::run_os(&rest)?),
        "qemu" => Ok(os::run_qemu(&rest)?),
        "gles-compare" => Ok(os::run_guest_check(&os::GLES_COMPARE, &rest)?),
        "kms-check" => Ok(os::run_guest_check(&os::KMS_CHECK, &rest)?),
        "core-trace" => Ok(os::run_core_trace(&rest)?),
        "check" => passthrough(&["check"], &rest),
        "test" => passthrough(&["nextest", "run"], &rest),
        "clippy" => passthrough(&["clippy", "--all-targets"], &rest),
        "window-smoke" => passthrough(
            &[
                "run",
                "-p",
                "neptron-window",
                "--features",
                "sdl3",
                "--example",
                "smoke",
            ],
            &rest,
        ),
        "rules" => {
            let repo = repo::Repo::discover()?;
            let policy = load::LoadPolicy::from_env()?;
            Ok(exit_code(rules::run(&repo, &policy)? == 0))
        }
        "help" | "-h" | "--help" => {
            print!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        other => Err(Error::UnknownCommand(other.to_owned())),
    }
}

fn passthrough(base: &[&str], rest: &[&str]) -> Result<ExitCode, Error> {
    let repo = repo::Repo::discover()?;
    let policy = load::LoadPolicy::from_env()?;
    let slot = policy.acquire(&load::lock_dir(&repo.git_common_dir))?;
    if let Some(slot) = &slot {
        eprintln!("xtask: holding build slot {}", slot.slot);
    }
    let args: Vec<&str> = base.iter().chain(rest).copied().collect();
    let status = repo::status(policy.cargo(&args).current_dir(&repo.root))?;
    Ok(exit_code(status.success()))
}

fn exit_code(success: bool) -> ExitCode {
    if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
