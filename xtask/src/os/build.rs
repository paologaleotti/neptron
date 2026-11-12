//! `cargo xtask os build`: the development container and the Buildroot build.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{Arch, OsError};

/// The repository of the development container images.
pub const CONTAINER_REPO: &str = "neptron-dev";

/// The image name for a `Dockerfile` text: its tag is a hash of the text, so
/// branches with different Dockerfiles never replace each other's image.
pub fn container_image(dockerfile: &str) -> String {
    // FNV-1a, 64 bits: stable across Rust releases, and a tag needs no
    // cryptographic strength.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in dockerfile.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{CONTAINER_REPO}:{hash:016x}")
}

/// The arguments of `os build`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildArgs {
    /// The target to build.
    pub arch: Arch,
    /// Buildroot jobs, and the CPUs of the container.
    pub jobs: u32,
    /// Make targets to run before the full build, such as `linux-reconfigure`.
    pub targets: Vec<String>,
}

impl BuildArgs {
    /// Parses `[--arch A] [--jobs N] [-- targets...]`.
    pub fn parse(args: &[&str], default_arch: Arch) -> Result<Self, OsError> {
        let mut out = Self {
            arch: default_arch,
            jobs: DEFAULT_JOBS,
            targets: Vec::new(),
        };
        let mut it = args.iter();
        while let Some(&arg) = it.next() {
            match arg {
                "--arch" => out.arch = Arch::parse(value(&mut it, "--arch")?)?,
                "--jobs" => out.jobs = positive(value(&mut it, "--jobs")?, "--jobs")?,
                "--" => {
                    out.targets = it.map(|s| (*s).to_owned()).collect();
                    break;
                }
                other => return Err(OsError::Usage(format!("unknown argument {other:?}"))),
            }
        }
        if out.jobs > MAX_JOBS {
            return Err(OsError::TooManyJobs {
                jobs: out.jobs,
                max: MAX_JOBS,
            });
        }
        Ok(out)
    }
}

/// Every Docker command of one `os build`, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildPlan {
    /// The arguments.
    pub args: BuildArgs,
    /// The br2-external tree, `os/` in the checkout.
    pub external: PathBuf,
    /// The host directory that receives the images.
    pub images: PathBuf,
    /// The container image, from [`container_image`].
    pub container: String,
}

impl BuildPlan {
    /// The arguments of `docker build`. The Dockerfile comes on standard
    /// input, so Docker sends no build context: the repository holds `target/`
    /// directories of many gigabytes.
    pub fn container_build(&self) -> Vec<String> {
        container_build(&self.container)
    }

    /// The `docker run` arguments of each step: apply the defconfig, run the
    /// extra targets, build everything, copy the images out.
    pub fn steps(&self) -> Vec<Vec<String>> {
        let jobs = format!("BR2_JLEVEL={}", self.args.jobs);
        let mut steps = vec![self.make(&["BR2_EXTERNAL=/src/os", self.args.arch.defconfig()])];
        for target in &self.args.targets {
            steps.push(self.make(&[&jobs, target]));
        }
        steps.push(self.make(&[&jobs]));
        steps.push(self.run(&["cp", "-rf", "/work/out/images/.", "/work/images/"]));
        steps
    }

    fn make(&self, rest: &[&str]) -> Vec<String> {
        let mut cmd = vec!["make", "-C", "/opt/buildroot", "O=/work/out"];
        cmd.extend_from_slice(rest);
        self.run(&cmd)
    }

    fn run(&self, command: &[&str]) -> Vec<String> {
        let arch = self.args.arch.name();
        let mut args = strings(&["run", "--rm"]);
        args.push("--cpus".to_owned());
        args.push(self.args.jobs.to_string());
        args.push("--memory".to_owned());
        args.push(CONTAINER_MEMORY.to_owned());
        for (source, target) in [
            (DL_VOLUME.to_owned(), "/work/dl"),
            (CCACHE_VOLUME.to_owned(), "/home/br/.buildroot-ccache"),
            (format!("{OUT_VOLUME_PREFIX}{arch}"), "/work/out"),
            (format!("{}", self.external.display()), "/src/os:ro"),
            (format!("{}", self.images.display()), "/work/images"),
        ] {
            args.push("--volume".to_owned());
            args.push(format!("{source}:{target}"));
        }
        args.push(self.container.clone());
        args.extend(command.iter().map(|s| (*s).to_owned()));
        args
    }
}

/// Builds the container, then runs each step of `plan`, under the
/// machine-wide build lock that the caller holds.
pub fn execute(plan: &BuildPlan, dockerfile: &Path) -> Result<(), OsError> {
    build_container(&plan.container_build(), dockerfile)?;
    std::fs::create_dir_all(&plan.images).map_err(|source| OsError::Io {
        path: plan.images.clone(),
        source,
    })?;
    for step in plan.steps() {
        super::run_step(Command::new("docker").args(step))?;
    }
    Ok(())
}

/// The arguments of `docker build` for the image tag `container`, with the
/// Dockerfile on standard input.
pub fn container_build(container: &str) -> Vec<String> {
    strings(&["build", "--tag", container, "-"])
}

/// Runs `docker build` with `args`, the Dockerfile on standard input.
pub fn build_container(args: &[String], dockerfile: &Path) -> Result<(), OsError> {
    let file = std::fs::File::open(dockerfile).map_err(|source| OsError::Io {
        path: dockerfile.to_path_buf(),
        source,
    })?;
    let mut build = Command::new("docker");
    build.args(args).stdin(file);
    super::run_step(&mut build)
}

/// Buildroot jobs when `--jobs` is not given: the limit for this machine.
pub const DEFAULT_JOBS: u32 = 4;
/// More jobs would take the cores and memory that the owner of the machine needs.
const MAX_JOBS: u32 = 8;
pub const CONTAINER_MEMORY: &str = "6g";
/// Named volumes keep the downloads, the compiler cache and the output on a
/// case-sensitive Linux filesystem, which Buildroot needs and macOS lacks.
const DL_VOLUME: &str = "neptron-br-dl";
const CCACHE_VOLUME: &str = "neptron-br-ccache";
pub const OUT_VOLUME_PREFIX: &str = "neptron-br-out-";

fn value<'a>(it: &mut std::slice::Iter<'_, &'a str>, flag: &str) -> Result<&'a str, OsError> {
    it.next()
        .copied()
        .ok_or_else(|| OsError::Usage(format!("{flag} needs a value")))
}

fn positive(text: &str, flag: &'static str) -> Result<u32, OsError> {
    match text.parse::<u32>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(OsError::BadNumber {
            flag,
            value: text.to_owned(),
        }),
    }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(args: &[&str]) -> BuildPlan {
        BuildPlan {
            args: BuildArgs::parse(args, Arch::Aarch64).unwrap(),
            external: PathBuf::from("/repo/os"),
            images: PathBuf::from("/cache/images/aarch64"),
            container: "neptron-dev:0123456789abcdef".to_owned(),
        }
    }

    #[test]
    fn defaults_are_the_host_arch_and_four_jobs() {
        let a = BuildArgs::parse(&[], Arch::X86_64).unwrap();
        assert_eq!(a.arch, Arch::X86_64);
        assert_eq!(a.jobs, 4);
        assert!(a.targets.is_empty());
    }

    #[test]
    fn flags_and_targets_parse() {
        let a = BuildArgs::parse(
            &["--arch", "x86_64", "--jobs", "2", "--", "linux-reconfigure"],
            Arch::Aarch64,
        )
        .unwrap();
        assert_eq!(a.arch, Arch::X86_64);
        assert_eq!(a.jobs, 2);
        assert_eq!(a.targets, ["linux-reconfigure"]);
    }

    #[test]
    fn bad_arguments_are_rejected() {
        for bad in [
            &["--jobs", "0"][..],
            &["--jobs", "x"],
            &["--jobs", "9"],
            &["--arch"],
            &["--arch", "mips"],
            &["linux"],
        ] {
            assert!(BuildArgs::parse(bad, Arch::Aarch64).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn dockerfile_pins_its_base_and_buildroot() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../Dockerfile");
        let text = std::fs::read_to_string(&path).unwrap();
        let hex64 = |s: &str| s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit());
        let base = text.lines().find_map(|l| l.strip_prefix("FROM ")).unwrap();
        assert!(
            base.split_once("@sha256:").is_some_and(|(_, h)| hex64(h)),
            "{base}"
        );
        let arg = |name: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(&format!("ARG {name}=")))
                .unwrap()
                .to_owned()
        };
        assert!(arg("BUILDROOT_VERSION").starts_with("2026.08"));
        assert!(hex64(&arg("BUILDROOT_SHA256")));
        assert!(text.contains("sha256sum -c"));
    }

    #[test]
    fn container_build_sends_no_context() {
        let args = plan(&[]).container_build();
        assert_eq!(args.join(" "), "build --tag neptron-dev:0123456789abcdef -");
    }

    #[test]
    fn the_container_tag_follows_the_dockerfile_text() {
        let a = container_image("FROM debian\n");
        assert_eq!(a, container_image("FROM debian\n"));
        assert_ne!(a, container_image("FROM debian\nRUN true\n"));
        assert!(a.starts_with("neptron-dev:"), "{a}");
        assert_eq!(a.len(), "neptron-dev:".len() + 16);
        assert_eq!(container_image(""), "neptron-dev:cbf29ce484222325");
    }

    #[test]
    fn steps_apply_the_defconfig_build_and_copy() {
        let steps = plan(&["--", "linux-reconfigure"]).steps();
        assert_eq!(steps.len(), 4);
        let tail = |s: &Vec<String>, n: usize| s[s.len() - n..].join(" ");
        assert_eq!(
            tail(&steps[0], 6),
            "make -C /opt/buildroot O=/work/out BR2_EXTERNAL=/src/os \
             neptron_qemu_aarch64_defconfig"
        );
        assert_eq!(tail(&steps[1], 2), "BR2_JLEVEL=4 linux-reconfigure");
        assert_eq!(
            tail(&steps[2], 4),
            "-C /opt/buildroot O=/work/out BR2_JLEVEL=4"
        );
        assert_eq!(
            tail(&steps[3], 4),
            "cp -rf /work/out/images/. /work/images/"
        );
    }

    #[test]
    fn every_step_is_limited_and_uses_the_shared_volumes() {
        for step in plan(&["--jobs", "3"]).steps() {
            let line = step.join(" ");
            assert!(line.starts_with("run --rm --cpus 3 --memory 6g "), "{line}");
            assert!(line.contains("--volume neptron-br-dl:/work/dl"), "{line}");
            assert!(
                line.contains("--volume neptron-br-ccache:/home/br/.buildroot-ccache"),
                "{line}"
            );
            assert!(
                line.contains("--volume neptron-br-out-aarch64:/work/out"),
                "{line}"
            );
            assert!(line.contains("--volume /repo/os:/src/os:ro"), "{line}");
            assert!(
                line.contains("--volume /cache/images/aarch64:/work/images"),
                "{line}"
            );
            assert!(line.contains(" neptron-dev:0123456789abcdef "), "{line}");
        }
    }
}
