//! Rust programs for the guest: a cross build in the development container
//! with the Rust toolchain and the C toolchain of the Buildroot output.

use std::path::{Path, PathBuf};

use super::Arch;
use super::build::{CONTAINER_MEMORY, DEFAULT_JOBS, fnv1a};

/// What to build for the guest: one example of one package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossPlan {
    /// The target.
    pub arch: Arch,
    /// The Buildroot output whose toolchain and sysroot the build uses
    /// (`neptron-br-out-<output>`), which also names the cross volume of
    /// the build (`neptron-cross-<output>`). Two outputs of one
    /// architecture can hold different glibc builds, so each has its own
    /// Cargo target directory.
    pub output: String,
    /// The container image, from [`container_image`](super::build::container_image).
    pub container: String,
    /// The checkout, mounted read-only.
    pub repo: PathBuf,
    /// The host directory that receives the program.
    pub export: PathBuf,
    /// The package.
    pub package: String,
    /// The features to turn on; empty for none.
    pub features: String,
    /// The example to build.
    pub example: String,
    /// Check every target of the package (tests and examples too) before
    /// the build. The host gates never compile code behind a Linux-only
    /// feature. Every guest build denies warnings; the Buildroot Rust
    /// toolchain has no clippy, so these are rustc's lints only.
    pub lint: bool,
    /// Link through `os/package/neptron/link-filter.sh`, as the image
    /// packages do. The `sdl3` crate adds `/usr/local/lib` to the link
    /// search on every Linux host, and the Buildroot compiler wrapper
    /// refuses that path as an error; the filter drops it and links with
    /// the wrapper. Only the builds that link SDL3 set it.
    pub filtered_linker: bool,
}

impl CrossPlan {
    /// The `docker run` arguments of the steps: the lint when asked for,
    /// the build, then the copy of the program to the export directory.
    pub fn steps(&self) -> Vec<Vec<String>> {
        let target = self.arch.rust_target();
        let mut lint = vec!["cargo", "check", "--locked", "--package", &self.package];
        let mut build = vec!["cargo", "build", "--locked", "--package", &self.package];
        if !self.features.is_empty() {
            lint.extend(["--features", &self.features]);
            build.extend(["--features", &self.features]);
        }
        lint.extend(["--all-targets", "--target", target]);
        build.extend(["--example", &self.example, "--target", target]);
        let program = format!(
            "{}/{target}/debug/examples/{}",
            self.target_dir(),
            self.example
        );
        let mut steps = Vec::with_capacity(3);
        if self.lint {
            steps.push(self.run(&lint));
        }
        steps.push(self.run(&build));
        steps.push(self.run(&["cp", &program, "/work/export/"]));
        steps
    }

    /// The `docker run` arguments that check and build the binaries of the
    /// package: the lint when asked for, then `cargo build --bins`. The
    /// example and the export are not used.
    pub fn app_steps(&self) -> Vec<Vec<String>> {
        let target = self.arch.rust_target();
        let mut lint = vec!["cargo", "check", "--locked", "--package", &self.package];
        let mut build = vec!["cargo", "build", "--locked", "--package", &self.package];
        if !self.features.is_empty() {
            lint.extend(["--features", &self.features]);
            build.extend(["--features", &self.features]);
        }
        lint.extend(["--all-targets", "--target", target]);
        build.extend(["--bins", "--target", target]);
        let mut steps = Vec::with_capacity(2);
        if self.lint {
            steps.push(self.run(&lint));
        }
        steps.push(self.run(&build));
        steps
    }

    /// The `docker run` arguments that check the package when asked for,
    /// build its binary `bin`, and copy it to the export directory. The
    /// example is not used.
    pub fn bin_steps(&self, bin: &str) -> Vec<Vec<String>> {
        let target = self.arch.rust_target();
        let mut lint = vec!["cargo", "check", "--locked", "--package", &self.package];
        let mut build = vec!["cargo", "build", "--locked", "--package", &self.package];
        if !self.features.is_empty() {
            lint.extend(["--features", &self.features]);
            build.extend(["--features", &self.features]);
        }
        lint.extend(["--all-targets", "--target", target]);
        build.extend(["--bin", bin, "--target", target]);
        let program = format!("{}/{target}/debug/{bin}", self.target_dir());
        let mut steps = Vec::with_capacity(3);
        if self.lint {
            steps.push(self.run(&lint));
        }
        steps.push(self.run(&build));
        steps.push(self.run(&["cp", &program, "/work/export/"]));
        steps
    }

    /// The `docker run` arguments that build one binary of the package for
    /// `os push` in the bench profile, then copy it to the export
    /// directory. The bench profile is the release profile with the
    /// changes of [`BENCH_PROFILE`].
    pub fn bench_steps(&self, bin: &str) -> Vec<Vec<String>> {
        let target = self.arch.rust_target();
        let mut build = vec![
            "cargo",
            "build",
            "--locked",
            "--release",
            "--package",
            &self.package,
        ];
        if !self.features.is_empty() {
            build.extend(["--features", &self.features]);
        }
        build.extend(["--bin", bin, "--target", target]);
        let program = format!("{}/{target}/release/{bin}", self.target_dir());
        vec![
            self.run_with(&BENCH_PROFILE, &build),
            self.run(&["cp", &program, "/work/export/"]),
        ]
    }

    /// The Cargo target directory of the checkout in the cross volume:
    /// `/home/br/target/<key>`, with the key of [`worktree_key`].
    pub fn target_dir(&self) -> String {
        format!("{TARGETS}/{}", worktree_key(&self.repo))
    }

    /// The `docker run` arguments that remove each target directory of the
    /// cross volume whose key is not in `live`, and every file of the
    /// shared target directory of older builds. The key of this checkout
    /// always stays, also when `live` does not hold it.
    pub fn prune_step(&self, live: &[String]) -> Vec<String> {
        let own = worktree_key(&self.repo);
        let pattern = format!("{TARGETS}/*");
        let mut find = vec![
            "find",
            HOME,
            "-mindepth",
            "2",
            "-maxdepth",
            "2",
            "-path",
            &pattern,
            "!",
            "-name",
            &own,
        ];
        for key in live.iter().filter(|k| **k != own) {
            find.extend(["!", "-name", key]);
        }
        find.extend(["-exec", "rm", "-rf", "{}", "+"]);
        self.run(&find)
    }

    fn run(&self, command: &[&str]) -> Vec<String> {
        self.run_with(&[], command)
    }

    fn run_with(&self, profile: &[(&str, &str)], command: &[&str]) -> Vec<String> {
        let output = &self.output;
        let host_bin = "/work/out/host/bin";
        let cpus = DEFAULT_JOBS.to_string();
        let mut args = vec!["run".to_owned(), "--rm".to_owned()];
        for pair in [
            ["--cpus", &cpus],
            ["--memory", CONTAINER_MEMORY],
            ["--workdir", "/src/repo"],
        ] {
            args.extend(pair.map(str::to_owned));
        }
        for (source, target) in [
            (
                format!("{}{output}", super::build::OUT_VOLUME_PREFIX),
                "/work/out:ro".to_owned(),
            ),
            (format!("{CROSS_VOLUME_PREFIX}{output}"), HOME.to_owned()),
            (self.repo.display().to_string(), "/src/repo:ro".to_owned()),
            (self.export.display().to_string(), "/work/export".to_owned()),
        ] {
            args.push("--volume".to_owned());
            args.push(format!("{source}:{target}"));
        }
        let linker_var = format!(
            "CARGO_TARGET_{}_LINKER",
            self.arch.rust_target().to_uppercase().replace('-', "_")
        );
        let gcc = format!("{host_bin}/{}-buildroot-linux-gnu-gcc", self.arch.name());
        let linker = if self.filtered_linker {
            LINK_FILTER.to_owned()
        } else {
            gcc.clone()
        };
        // The `cc` crate of build scripts looks for these, and else for a
        // cross compiler name that the container does not have.
        let target_cc = format!("CC_{}", self.arch.rust_target().replace('-', "_"));
        let target_cxx = format!("CXX_{}", self.arch.rust_target().replace('-', "_"));
        let target_ar = format!("AR_{}", self.arch.rust_target().replace('-', "_"));
        let gxx = format!("{host_bin}/{}-buildroot-linux-gnu-g++", self.arch.name());
        let ar = format!("{host_bin}/{}-buildroot-linux-gnu-ar", self.arch.name());
        for (name, value) in [
            // Every guest build denies warnings. One setting for every
            // command keeps the shared cache in the cross volume valid.
            ("RUSTFLAGS".to_owned(), "-D warnings".to_owned()),
            ("PATH".to_owned(), format!("{host_bin}:/usr/bin:/bin")),
            ("CARGO_HOME".to_owned(), format!("{HOME}/.cargo")),
            ("CARGO_TARGET_DIR".to_owned(), self.target_dir()),
            ("CARGO_BUILD_JOBS".to_owned(), cpus.clone()),
            // The pkg-config of the Buildroot output reads the sysroot, so
            // a crate that links a system library (SDL3) gets the guest's
            // paths, not the container's.
            ("PKG_CONFIG".to_owned(), format!("{host_bin}/pkg-config")),
            ("PKG_CONFIG_ALLOW_CROSS".to_owned(), "1".to_owned()),
            (linker_var, linker),
            ("NEPTRON_LINK_CC".to_owned(), gcc.clone()),
            (target_cc, gcc),
            (target_cxx, gxx),
            (target_ar, ar),
        ] {
            args.push("--env".to_owned());
            args.push(format!("{name}={value}"));
        }
        for (name, value) in profile {
            args.push("--env".to_owned());
            args.push(format!("{name}={value}"));
        }
        args.push(self.container.clone());
        args.extend(command.iter().map(|s| (*s).to_owned()));
        args
    }
}

/// The release profile of the workspace with these changes is the bench
/// profile of `os push`: no LTO and parallel code generation, so a warm
/// build of the frontend takes about a minute, and line tables and no strip,
/// so a backtrace names files and lines. `panic = "abort"` stays: the panic
/// hook prints the backtrace before the abort, and a panic must not unwind
/// into a core.
pub const BENCH_PROFILE: [(&str, &str); 5] = [
    ("CARGO_PROFILE_RELEASE_LTO", "off"),
    ("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "16"),
    ("CARGO_PROFILE_RELEASE_INCREMENTAL", "true"),
    ("CARGO_PROFILE_RELEASE_DEBUG", "line-tables-only"),
    ("CARGO_PROFILE_RELEASE_STRIP", "none"),
];

/// The name of the Cargo target directory of the checkout at `repo` in the
/// cross volume: `wt-`, the last part of the path, and a hash of the whole
/// path. Cargo decides with file times whether an output is fresh, and it
/// names the outputs of a workspace crate by its path relative to the
/// workspace, so two checkouts that share a target directory take each
/// other's outputs as fresh. One target directory per checkout makes the
/// times of one checkout the only ones that count. The dependencies build
/// once per checkout: Cargo has no safe way to share them.
pub fn worktree_key(repo: &Path) -> String {
    let name: String = repo
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(KEY_NAME_CHARS)
        .collect();
    format!("{KEY_PREFIX}{name}-{:016x}", fnv1a(&repo.to_string_lossy()))
}

/// The checkout paths in the output of `git worktree list --porcelain`.
pub fn worktree_paths(porcelain: &str) -> Vec<PathBuf> {
    porcelain
        .lines()
        .filter_map(|l| l.strip_prefix("worktree "))
        .map(PathBuf::from)
        .collect()
}

/// The home of the build user in the container. The cross volume is
/// mounted here: Docker fills a new volume from this directory of the
/// image, so the build user owns it. It holds the Cargo registry and the
/// target directory.
const HOME: &str = "/home/br";
const CROSS_VOLUME_PREFIX: &str = "neptron-cross-";
/// The parent of the target directories of the checkouts. Older builds
/// used it as one shared target directory; the prune removes those files.
const TARGETS: &str = "/home/br/target";
const KEY_PREFIX: &str = "wt-";
const KEY_NAME_CHARS: usize = 24;
/// The linker filter of the image packages, in the checkout mount.
const LINK_FILTER: &str = "/src/repo/os/package/neptron/link-filter.sh";

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> CrossPlan {
        CrossPlan {
            arch: Arch::Aarch64,
            output: "aarch64".to_owned(),
            container: "neptron-dev:0123456789abcdef".to_owned(),
            repo: PathBuf::from("/repo"),
            export: PathBuf::from("/repo/target/gles-compare"),
            package: "neptron-gfx".to_owned(),
            features: "gles".to_owned(),
            example: "gles_compare".to_owned(),
            lint: false,
            filtered_linker: false,
        }
    }

    #[test]
    fn the_lint_checks_every_target_first() {
        let steps = CrossPlan {
            lint: true,
            ..plan()
        }
        .steps();
        assert_eq!(steps.len(), 3);
        let lint = steps[0].join(" ");
        assert!(
            lint.ends_with(
                "cargo check --locked --package neptron-gfx --features gles --all-targets \
                 --target aarch64-unknown-linux-gnu"
            ),
            "{lint}"
        );
        assert!(steps[1].join(" ").contains("cargo build"));
    }

    #[test]
    fn every_guest_build_denies_warnings() {
        let lint = CrossPlan {
            lint: true,
            ..plan()
        };
        let no_features = CrossPlan {
            features: String::new(),
            ..plan()
        };
        for p in [plan(), lint, no_features] {
            for step in p.steps() {
                assert!(
                    step.iter().any(|a| a == "RUSTFLAGS=-D warnings"),
                    "{step:?}"
                );
            }
        }
    }

    #[test]
    fn the_build_uses_the_buildroot_toolchains() {
        let steps = plan().steps();
        assert_eq!(steps.len(), 2);
        let build = steps[0].join(" ");
        assert!(
            build.ends_with(
                "neptron-dev:0123456789abcdef cargo build --locked --package neptron-gfx \
                 --features gles --example gles_compare --target aarch64-unknown-linux-gnu"
            ),
            "{build}"
        );
        assert!(
            build.contains("--env PATH=/work/out/host/bin:/usr/bin:/bin"),
            "{build}"
        );
        assert!(
            build.contains(
                "--env CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=\
                 /work/out/host/bin/aarch64-buildroot-linux-gnu-gcc"
            ),
            "{build}"
        );
        assert!(
            build.contains(
                "--env CC_aarch64_unknown_linux_gnu=\
                 /work/out/host/bin/aarch64-buildroot-linux-gnu-gcc"
            ),
            "{build}"
        );
        assert!(
            build.contains(
                "--env CXX_aarch64_unknown_linux_gnu=\
                 /work/out/host/bin/aarch64-buildroot-linux-gnu-g++"
            ),
            "{build}"
        );
    }

    #[test]
    fn a_build_without_features_names_none() {
        let mut p = plan();
        p.features = String::new();
        let build = p.steps()[0].join(" ");
        assert!(
            build.ends_with(
                "cargo build --locked --package neptron-gfx --example gles_compare \
                 --target aarch64-unknown-linux-gnu"
            ),
            "{build}"
        );
    }

    #[test]
    fn the_app_steps_check_then_build_the_binaries() {
        let steps = CrossPlan {
            package: "neptron".to_owned(),
            features: "kms".to_owned(),
            lint: true,
            filtered_linker: true,
            ..plan()
        }
        .app_steps();
        assert_eq!(steps.len(), 2);
        assert!(
            steps[0].join(" ").ends_with(
                "cargo check --locked --package neptron --features kms --all-targets \
                 --target aarch64-unknown-linux-gnu"
            ),
            "{:?}",
            steps[0]
        );
        assert!(
            steps[1].join(" ").ends_with(
                "cargo build --locked --package neptron --features kms --bins \
                 --target aarch64-unknown-linux-gnu"
            ),
            "{:?}",
            steps[1]
        );
        assert!(
            steps
                .iter()
                .all(|s| s.iter().any(|a| a == "RUSTFLAGS=-D warnings"))
        );
        assert!(
            steps.iter().all(|s| s
                .iter()
                .any(|a| a == "PKG_CONFIG=/work/out/host/bin/pkg-config")),
            "a linked system library comes from the sysroot"
        );
        let build = steps[1].join(" ");
        assert!(
            build.contains(
                "--env CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=\
                 /src/repo/os/package/neptron/link-filter.sh"
            ),
            "{build}"
        );
        assert!(
            build.contains(
                "--env NEPTRON_LINK_CC=/work/out/host/bin/aarch64-buildroot-linux-gnu-gcc"
            ),
            "{build}"
        );
        assert!(!build.contains("br_real"), "the wrapper checks every link");
        assert!(
            !plan().steps()[0].join(" ").contains("link-filter"),
            "the other guest builds link with the wrapper itself"
        );
    }

    #[test]
    fn the_link_filter_drops_only_usr_local_lib() {
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../os/package/neptron/link-filter.sh");
        let out = std::process::Command::new("sh")
            .arg(&script)
            .args([
                "-L",
                "/usr/local/lib",
                "-L/usr/local/lib",
                "-L",
                "/sysroot/a b",
                "-L/usr/local/lib64",
                "-o",
                "neptron",
                "-L",
            ])
            .env("NEPTRON_LINK_CC", "printf <%s>")
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        assert_eq!(
            String::from_utf8(out.stdout).unwrap(),
            "<-L></sysroot/a b><-L/usr/local/lib64><-o><neptron><-L>"
        );
    }

    #[test]
    fn the_bin_steps_check_build_and_copy_one_binary() {
        let steps = CrossPlan {
            package: "neptronctl".to_owned(),
            features: "kms".to_owned(),
            lint: true,
            ..plan()
        }
        .bin_steps("neptronctl");
        assert_eq!(steps.len(), 3);
        assert!(
            steps[0].join(" ").ends_with(
                "cargo check --locked --package neptronctl --features kms --all-targets \
                 --target aarch64-unknown-linux-gnu"
            ),
            "{:?}",
            steps[0]
        );
        assert!(
            steps[1].join(" ").ends_with(
                "cargo build --locked --package neptronctl --features kms --bin neptronctl \
                 --target aarch64-unknown-linux-gnu"
            ),
            "{:?}",
            steps[1]
        );
        assert!(
            steps[2].join(" ").ends_with(&format!(
                "cp {}/aarch64-unknown-linux-gnu/debug/neptronctl /work/export/",
                plan().target_dir()
            )),
            "{:?}",
            steps[2]
        );
        assert!(
            steps
                .iter()
                .all(|s| s.iter().any(|a| a == "RUSTFLAGS=-D warnings"))
        );
    }

    #[test]
    fn the_bench_build_is_the_release_profile_with_line_tables_and_no_lto() {
        let p = CrossPlan {
            arch: Arch::X86_64,
            output: "x86_64".to_owned(),
            package: "neptron".to_owned(),
            features: "kms,control".to_owned(),
            filtered_linker: true,
            ..plan()
        };
        let steps = p.bench_steps("neptron");
        assert_eq!(steps.len(), 2);
        let build = steps[0].join(" ");
        assert!(
            build.ends_with(
                "cargo build --locked --release --package neptron --features kms,control \
                 --bin neptron --target x86_64-unknown-linux-gnu"
            ),
            "{build}"
        );
        for (name, value) in BENCH_PROFILE {
            assert!(build.contains(&format!("--env {name}={value}")), "{build}");
        }
        assert!(build.contains("RUSTFLAGS=-D warnings"), "{build}");
        assert!(
            !build.contains("PANIC"),
            "the bench profile keeps panic = abort"
        );
        assert!(
            steps[1].join(" ").ends_with(&format!(
                "cp {}/x86_64-unknown-linux-gnu/release/neptron /work/export/",
                p.target_dir()
            )),
            "{:?}",
            steps[1]
        );
        assert!(
            !plan().steps()[0].join(" ").contains("CARGO_PROFILE"),
            "the other guest builds keep their profile"
        );
    }

    #[test]
    fn a_pi_build_uses_the_pi_output_and_a_cross_volume_of_its_own() {
        let p = CrossPlan {
            output: "rpi-arm64-dev".to_owned(),
            package: "neptron".to_owned(),
            features: "kms,control".to_owned(),
            filtered_linker: true,
            ..plan()
        };
        for step in p.bench_steps("neptron") {
            let line = step.join(" ");
            assert!(
                line.contains("--volume neptron-br-out-rpi-arm64-dev:/work/out:ro"),
                "{line}"
            );
            assert!(
                line.contains("--volume neptron-cross-rpi-arm64-dev:/home/br"),
                "{line}"
            );
            assert!(!line.contains("neptron-br-out-aarch64"), "{line}");
        }
        let build = p.bench_steps("neptron")[0].join(" ");
        assert!(
            build.contains("NEPTRON_LINK_CC=/work/out/host/bin/aarch64-buildroot-linux-gnu-gcc"),
            "{build}"
        );
        assert!(
            build.ends_with("--target aarch64-unknown-linux-gnu"),
            "{build}"
        );
    }

    #[test]
    fn the_copy_takes_the_example_program() {
        let copy = plan().steps()[1].join(" ");
        assert!(
            copy.ends_with(&format!(
                "cp {}/aarch64-unknown-linux-gnu/debug/examples/gles_compare /work/export/",
                plan().target_dir()
            )),
            "{copy}"
        );
    }

    #[test]
    fn every_step_is_limited_and_mounts_the_source_read_only() {
        for step in plan().steps() {
            let line = step.join(" ");
            assert!(line.starts_with("run --rm --cpus 4 --memory 6g "), "{line}");
            assert!(
                line.contains("--volume neptron-br-out-aarch64:/work/out:ro"),
                "{line}"
            );
            assert!(
                line.contains("--volume neptron-cross-aarch64:/home/br"),
                "{line}"
            );
            assert!(line.contains("--volume /repo:/src/repo:ro"), "{line}");
            assert!(
                line.contains("--volume /repo/target/gles-compare:/work/export"),
                "{line}"
            );
        }
    }

    #[test]
    fn two_worktrees_build_in_two_target_directories() {
        let main = CrossPlan {
            repo: PathBuf::from("/Users/u/neptron"),
            ..plan()
        };
        let task = CrossPlan {
            repo: PathBuf::from("/Users/u/neptron/.worktrees/M11"),
            ..plan()
        };
        let same_name = CrossPlan {
            repo: PathBuf::from("/Users/v/neptron"),
            ..plan()
        };
        let dirs = [main.target_dir(), task.target_dir(), same_name.target_dir()];
        assert_ne!(dirs[0], dirs[1]);
        assert_ne!(
            dirs[0], dirs[2],
            "the hash tells apart two checkouts of one name"
        );
        assert_ne!(dirs[1], dirs[2]);
        for (p, dir) in [&main, &task].into_iter().zip(&dirs) {
            for step in p
                .steps()
                .iter()
                .chain(&p.app_steps())
                .chain(&p.bench_steps("x"))
            {
                let line = step.join(" ");
                assert!(
                    line.contains(&format!("--env CARGO_TARGET_DIR={dir} ")),
                    "{line}"
                );
            }
        }
        assert!(
            dirs[1].starts_with("/home/br/target/wt-m11-"),
            "{}",
            dirs[1]
        );
        assert_eq!(dirs[1].len(), "/home/br/target/wt-m11-".len() + 16);
    }

    #[test]
    fn one_worktree_keeps_its_target_directory() {
        let a = worktree_key(Path::new("/Users/u/neptron/.worktrees/M11"));
        assert_eq!(
            a,
            worktree_key(Path::new("/Users/u/neptron/.worktrees/M11"))
        );
        assert!(a.starts_with("wt-m11-"), "{a}");
        let odd = worktree_key(Path::new("/x/A b.C/My Task!"));
        assert!(odd.starts_with("wt-my_task_-"), "{odd}");
        assert!(
            odd.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_'),
            "the key is a plain file name: {odd}"
        );
        let long = worktree_key(Path::new("/x/a-very-long-name-of-a-task-worktree-folder"));
        assert_eq!(long.len(), "wt-".len() + 24 + 1 + 16, "{long}");
    }

    #[test]
    fn the_worktree_list_gives_every_checkout_path() {
        let porcelain = "worktree /Users/u/neptron\nHEAD 0123\nbranch refs/heads/main\n\n\
                         worktree /Users/u/neptron/.worktrees/M11\nHEAD 4567\n\
                         branch refs/heads/mvp/M11\n\n\
                         worktree /Users/u/gone\nHEAD 89ab\ndetached\nprunable gitdir file points to non-existent location\n";
        assert_eq!(
            worktree_paths(porcelain),
            [
                PathBuf::from("/Users/u/neptron"),
                PathBuf::from("/Users/u/neptron/.worktrees/M11"),
                PathBuf::from("/Users/u/gone"),
            ]
        );
        assert!(worktree_paths("").is_empty());
    }

    #[test]
    fn the_prune_keeps_the_live_keys_and_its_own() {
        let p = plan();
        let own = worktree_key(Path::new("/repo"));
        let live = vec![
            worktree_key(Path::new("/Users/u/neptron")),
            worktree_key(Path::new("/Users/u/neptron/.worktrees/M11")),
        ];
        let step = p.prune_step(&live);
        let line = step.join(" ");
        assert!(
            line.contains(
                "neptron-dev:0123456789abcdef find /home/br -mindepth 2 -maxdepth 2 \
                           -path /home/br/target/*"
            ),
            "{line}"
        );
        assert!(line.ends_with(" -exec rm -rf {} +"), "{line}");
        assert!(
            line.contains("--volume neptron-cross-aarch64:/home/br"),
            "{line}"
        );
        for key in live.iter().chain([&own]) {
            assert!(line.contains(&format!("! -name {key} ")), "{key}: {line}");
        }
        let with_own = p.prune_step(std::slice::from_ref(&own)).join(" ");
        assert_eq!(
            with_own.matches(&format!("-name {own}")).count(),
            1,
            "{with_own}"
        );
        let alone = p.prune_step(&[]).join(" ");
        assert!(alone.contains(&format!("! -name {own} -exec")), "{alone}");
    }
}
