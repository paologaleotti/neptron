//! Rust programs for the guest: a cross build in the development container
//! with the Rust toolchain and the C toolchain of the Buildroot output.

use std::path::PathBuf;

use super::Arch;
use super::build::{CONTAINER_MEMORY, DEFAULT_JOBS};

/// What to build for the guest: one example of one package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossPlan {
    /// The target.
    pub arch: Arch,
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
    /// Link with the compiler behind the Buildroot wrapper. The `sdl3`
    /// crate adds `/usr/local/lib` to the link search on every Linux host,
    /// and the wrapper refuses that path as an error. The compiler itself
    /// links against the sysroot and only warns. Only the builds that link
    /// SDL3 set it; the compiler of the C code stays the wrapper.
    pub unwrapped_linker: bool,
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
        let program = format!("{HOME}/target/{target}/debug/examples/{}", self.example);
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
        let program = format!("{HOME}/target/{target}/debug/{bin}");
        let mut steps = Vec::with_capacity(3);
        if self.lint {
            steps.push(self.run(&lint));
        }
        steps.push(self.run(&build));
        steps.push(self.run(&["cp", &program, "/work/export/"]));
        steps
    }

    fn run(&self, command: &[&str]) -> Vec<String> {
        let arch = self.arch.name();
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
                format!("{}{arch}", super::build::OUT_VOLUME_PREFIX),
                "/work/out:ro".to_owned(),
            ),
            (format!("{CROSS_VOLUME_PREFIX}{arch}"), HOME.to_owned()),
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
        let linker = if self.unwrapped_linker {
            format!("{gcc}.br_real")
        } else {
            gcc.clone()
        };
        // The `cc` crate of build scripts looks for these, and else for a
        // cross compiler name that the container does not have.
        let target_cc = format!("CC_{}", self.arch.rust_target().replace('-', "_"));
        let target_ar = format!("AR_{}", self.arch.rust_target().replace('-', "_"));
        let ar = format!("{host_bin}/{}-buildroot-linux-gnu-ar", self.arch.name());
        for (name, value) in [
            // Every guest build denies warnings. One setting for every
            // command keeps the shared cache in the cross volume valid.
            ("RUSTFLAGS".to_owned(), "-D warnings".to_owned()),
            ("PATH".to_owned(), format!("{host_bin}:/usr/bin:/bin")),
            ("CARGO_HOME".to_owned(), format!("{HOME}/.cargo")),
            ("CARGO_TARGET_DIR".to_owned(), format!("{HOME}/target")),
            ("CARGO_BUILD_JOBS".to_owned(), cpus.clone()),
            // The pkg-config of the Buildroot output reads the sysroot, so
            // a crate that links a system library (SDL3) gets the guest's
            // paths, not the container's.
            ("PKG_CONFIG".to_owned(), format!("{host_bin}/pkg-config")),
            ("PKG_CONFIG_ALLOW_CROSS".to_owned(), "1".to_owned()),
            (linker_var, linker),
            (target_cc, gcc),
            (target_ar, ar),
        ] {
            args.push("--env".to_owned());
            args.push(format!("{name}={value}"));
        }
        args.push(self.container.clone());
        args.extend(command.iter().map(|s| (*s).to_owned()));
        args
    }
}

/// The home of the build user in the container. The cross volume is
/// mounted here: Docker fills a new volume from this directory of the
/// image, so the build user owns it. It holds the Cargo registry and the
/// target directory.
const HOME: &str = "/home/br";
const CROSS_VOLUME_PREFIX: &str = "neptron-cross-";

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> CrossPlan {
        CrossPlan {
            arch: Arch::Aarch64,
            container: "neptron-dev:0123456789abcdef".to_owned(),
            repo: PathBuf::from("/repo"),
            export: PathBuf::from("/repo/target/gles-compare"),
            package: "neptron-gfx".to_owned(),
            features: "gles".to_owned(),
            example: "gles_compare".to_owned(),
            lint: false,
            unwrapped_linker: false,
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
            unwrapped_linker: true,
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
        assert!(
            steps[1]
                .join(" ")
                .contains("LINKER=/work/out/host/bin/aarch64-buildroot-linux-gnu-gcc.br_real"),
            "{:?}",
            steps[1]
        );
        assert!(
            !plan().steps()[0].join(" ").contains("br_real"),
            "the other guest builds keep the wrapper"
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
            steps[2].join(" ").ends_with(
                "cp /home/br/target/aarch64-unknown-linux-gnu/debug/neptronctl /work/export/"
            ),
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
    fn the_copy_takes_the_example_program() {
        let copy = plan().steps()[1].join(" ");
        assert!(
            copy.ends_with(
                "cp /home/br/target/aarch64-unknown-linux-gnu/debug/examples/gles_compare \
                 /work/export/"
            ),
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
}
