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
}

impl CrossPlan {
    /// The `docker run` arguments of the two steps: the build, then the copy
    /// of the program to the export directory.
    pub fn steps(&self) -> Vec<Vec<String>> {
        let target = self.arch.rust_target();
        let mut build = vec!["cargo", "build", "--locked", "--package", &self.package];
        if !self.features.is_empty() {
            build.extend(["--features", &self.features]);
        }
        build.extend(["--example", &self.example, "--target", target]);
        let program = format!("{HOME}/target/{target}/debug/examples/{}", self.example);
        vec![
            self.run(&build),
            self.run(&["cp", &program, "/work/export/"]),
        ]
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
        // The `cc` crate of build scripts looks for these, and else for a
        // cross compiler name that the container does not have.
        let target_cc = format!("CC_{}", self.arch.rust_target().replace('-', "_"));
        let target_ar = format!("AR_{}", self.arch.rust_target().replace('-', "_"));
        let ar = format!("{host_bin}/{}-buildroot-linux-gnu-ar", self.arch.name());
        for (name, value) in [
            ("PATH".to_owned(), format!("{host_bin}:/usr/bin:/bin")),
            ("CARGO_HOME".to_owned(), format!("{HOME}/.cargo")),
            ("CARGO_TARGET_DIR".to_owned(), format!("{HOME}/target")),
            ("CARGO_BUILD_JOBS".to_owned(), cpus.clone()),
            (linker_var, gcc.clone()),
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
