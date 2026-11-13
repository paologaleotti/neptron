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
    /// The public SSH key of the development variant, a file outside the
    /// checkout. `None` builds the release image.
    pub dev_key: Option<PathBuf>,
    /// Leave out the pinned 15 kHz kernel patch set, which the PC image
    /// applies (`--no-kernel-15khz`).
    pub no_kernel_15khz: bool,
    /// A name for the output volume and the images directory, in place of
    /// the variant name, for a build in an empty output (`--output`).
    pub output: Option<String>,
}

impl BuildArgs {
    /// Parses `[--arch A] [--jobs N] [--dev-key FILE] [--no-kernel-15khz]
    /// [-- targets...]`.
    pub fn parse(args: &[&str], default_arch: Arch) -> Result<Self, OsError> {
        let mut out = Self {
            arch: default_arch,
            jobs: DEFAULT_JOBS,
            targets: Vec::new(),
            dev_key: None,
            no_kernel_15khz: false,
            output: None,
        };
        let mut it = args.iter();
        while let Some(&arg) = it.next() {
            match arg {
                "--arch" => out.arch = Arch::parse(value(&mut it, "--arch")?)?,
                "--jobs" => out.jobs = positive(value(&mut it, "--jobs")?, "--jobs")?,
                "--dev-key" => out.dev_key = Some(PathBuf::from(value(&mut it, "--dev-key")?)),
                "--no-kernel-15khz" => out.no_kernel_15khz = true,
                "--output" => {
                    let name = value(&mut it, "--output")?;
                    let plain = !name.is_empty()
                        && name.bytes().all(|b| {
                            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_'
                        });
                    if !plain {
                        return Err(OsError::Usage(format!(
                            "--output takes lowercase letters, digits, - and _, not {name:?}"
                        )));
                    }
                    out.output = Some(name.to_owned());
                }
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
        if (out.dev_key.is_some() || out.no_kernel_15khz) && out.arch != Arch::X86_64 {
            return Err(OsError::Usage(
                "--dev-key and --no-kernel-15khz are variants of the x86_64 PC image".to_owned(),
            ));
        }
        Ok(out)
    }

    /// The configuration fragments of the variant, in `os/configs`, in the
    /// order of the merge.
    pub fn fragments(&self) -> Vec<&'static str> {
        let mut fragments = Vec::new();
        if self.dev_key.is_some() {
            fragments.push(DEV_FRAGMENT);
        }
        if self.no_kernel_15khz {
            fragments.push(NO_15KHZ_FRAGMENT);
        }
        fragments
    }

    /// The name of the images directory: the `--output` name or the
    /// architecture, with `-dev` for the development variant and `-no15khz`
    /// for the kernel without the 15 kHz patch set. Without `--output` it
    /// also names the output volume, so the variants never share an output
    /// and no file of a variant can reach the release image.
    pub fn variant(&self) -> String {
        let mut name = self
            .output
            .clone()
            .unwrap_or_else(|| self.arch.name().to_owned());
        if self.dev_key.is_some() {
            name.push_str(DEV_SUFFIX);
        }
        if self.no_kernel_15khz {
            name.push_str(NO_15KHZ_SUFFIX);
        }
        name
    }

    /// The name of the Buildroot output volume: the `--output` name as it
    /// is, or else the variant name. With `--output` a variant can build in
    /// an output that it does not own; its images still go to the folder of
    /// [`variant`](Self::variant), so they never replace the images of
    /// another variant.
    pub fn volume(&self) -> String {
        self.output.clone().unwrap_or_else(|| self.variant())
    }
}

/// Why `text`, the file at `path`, cannot be the key of the development
/// variant, or `None` when it can: one public SSH key, from a file outside
/// the checkout, so that no key is ever committed.
pub fn dev_key_problem(path: &Path, checkout: &Path, text: &str) -> Option<String> {
    if path.starts_with(checkout) {
        return Some(format!(
            "{} is inside the checkout; keep the key outside the repository",
            path.display()
        ));
    }
    if text.contains("PRIVATE KEY") {
        return Some(format!(
            "{} holds a private key; give the public key (.pub)",
            path.display()
        ));
    }
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let public = |l: &&str| {
        PUBLIC_KEY_TYPES
            .iter()
            .any(|t| l.starts_with(&format!("{t} ")))
    };
    if lines.len() != 1 || !lines.iter().all(public) {
        return Some(format!(
            "{} must hold exactly one public SSH key line",
            path.display()
        ));
    }
    None
}

/// Every Docker command of one `os build`, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildPlan {
    /// The arguments.
    pub args: BuildArgs,
    /// The checkout. Buildroot reads the br2-external tree `os/` in it, and
    /// the packages of the frontend and the launcher build its workspace.
    pub checkout: PathBuf,
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
    /// extra targets, collect the licences, build everything, copy the
    /// images out. The licences come first: the image carries them.
    pub fn steps(&self) -> Vec<Vec<String>> {
        let jobs = format!("BR2_JLEVEL={}", self.args.jobs);
        let external = format!("BR2_EXTERNAL={CHECKOUT_MOUNT}/os");
        let mut steps = vec![self.make(&[&external, self.args.arch.defconfig()])];
        let fragments: Vec<String> = self
            .args
            .fragments()
            .iter()
            .map(|f| format!("{CHECKOUT_MOUNT}/os/configs/{f}"))
            .collect();
        if !fragments.is_empty() {
            let mut merge = vec![
                "/opt/buildroot/support/kconfig/merge_config.sh",
                "-m",
                "-O",
                "/work/out",
                "/work/out/.config",
            ];
            merge.extend(fragments.iter().map(String::as_str));
            steps.push(self.run(&merge));
            steps.push(self.make(&["olddefconfig"]));
        }
        for target in &self.args.targets {
            steps.push(self.make(&[&jobs, target]));
        }
        steps.push(self.make(&[&jobs, "legal-info"]));
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
        let volume = self.args.volume();
        let mut args = strings(&["run", "--rm"]);
        args.push("--cpus".to_owned());
        args.push(self.args.jobs.to_string());
        args.push("--memory".to_owned());
        args.push(CONTAINER_MEMORY.to_owned());
        let checkout = format!("{CHECKOUT_MOUNT}:ro");
        for (source, target) in [
            (DL_VOLUME.to_owned(), "/work/dl"),
            (CCACHE_VOLUME.to_owned(), "/home/br/.buildroot-ccache"),
            (format!("{OUT_VOLUME_PREFIX}{volume}"), "/work/out"),
            (format!("{}", self.checkout.display()), checkout.as_str()),
            (format!("{}", self.images.display()), "/work/images"),
        ] {
            args.push("--volume".to_owned());
            args.push(format!("{source}:{target}"));
        }
        if let Some(key) = &self.args.dev_key {
            args.push("--volume".to_owned());
            args.push(format!("{}:{DEV_KEY_MOUNT}:ro", key.display()));
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
/// Where the checkout is mounted, read-only, in the container.
const CHECKOUT_MOUNT: &str = "/src/repo";
/// The suffix of the output and the images of the development variant.
pub const DEV_SUFFIX: &str = "-dev";
/// The configuration fragment of the development variant, in `os/configs`.
const DEV_FRAGMENT: &str = "neptron_pc_x86_64_dev.fragment";
/// The suffix of the output and the images of the kernel without the
/// 15 kHz patch set.
pub const NO_15KHZ_SUFFIX: &str = "-no15khz";
/// The configuration fragment that leaves out the 15 kHz patch set.
const NO_15KHZ_FRAGMENT: &str = "neptron_pc_x86_64_no15khz.fragment";
/// Where the post-build script of the PC image finds the key.
const DEV_KEY_MOUNT: &str = "/work/dev-key/authorized_keys";
/// The key types that OpenSSH and Dropbear accept.
const PUBLIC_KEY_TYPES: [&str; 6] = [
    "ssh-ed25519",
    "ssh-rsa",
    "ecdsa-sha2-nistp256",
    "ecdsa-sha2-nistp384",
    "ecdsa-sha2-nistp521",
    "sk-ssh-ed25519@openssh.com",
];

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
            checkout: PathBuf::from("/repo"),
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
            &["--dev-key"][..],
            &["--jobs", "0"],
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
    fn the_dev_variant_has_its_own_output_merges_its_fragment_and_mounts_the_key() {
        let mut p = plan(&["--arch", "x86_64", "--dev-key", "/home/u/.ssh/bench.pub"]);
        p.images = PathBuf::from("/cache/images/x86_64-dev");
        assert_eq!(p.args.variant(), "x86_64-dev");
        let steps = p.steps();
        assert_eq!(steps.len(), 6);
        assert!(
            steps[1].join(" ").ends_with(
                "/opt/buildroot/support/kconfig/merge_config.sh -m -O /work/out \
                 /work/out/.config /src/repo/os/configs/neptron_pc_x86_64_dev.fragment"
            ),
            "{:?}",
            steps[1]
        );
        assert!(steps[2].join(" ").ends_with("O=/work/out olddefconfig"));
        for step in &steps {
            let line = step.join(" ");
            assert!(
                line.contains("--volume neptron-br-out-x86_64-dev:/work/out"),
                "{line}"
            );
            assert!(
                line.contains("--volume /home/u/.ssh/bench.pub:/work/dev-key/authorized_keys:ro"),
                "{line}"
            );
        }
        assert!(
            !plan(&["--arch", "x86_64"])
                .steps()
                .iter()
                .any(|s| s.join(" ").contains("dev-key")),
            "the release build mounts no key"
        );
        assert!(BuildArgs::parse(&["--dev-key", "/k.pub"], Arch::Aarch64).is_err());
    }

    #[test]
    fn the_no_15khz_variant_merges_its_fragment_after_the_dev_fragment() {
        let p = plan(&["--arch", "x86_64", "--no-kernel-15khz"]);
        assert_eq!(p.args.variant(), "x86_64-no15khz");
        let merge = p.steps()[1].join(" ");
        assert!(
            merge.ends_with(
                "/work/out/.config /src/repo/os/configs/neptron_pc_x86_64_no15khz.fragment"
            ),
            "{merge}"
        );
        let both = plan(&[
            "--arch",
            "x86_64",
            "--no-kernel-15khz",
            "--dev-key",
            "/k.pub",
        ]);
        assert_eq!(both.args.variant(), "x86_64-dev-no15khz");
        assert!(both.steps()[1].join(" ").ends_with(
            "/src/repo/os/configs/neptron_pc_x86_64_dev.fragment \
                 /src/repo/os/configs/neptron_pc_x86_64_no15khz.fragment"
        ));
        assert!(BuildArgs::parse(&["--no-kernel-15khz"], Arch::Aarch64).is_err());
        assert_eq!(plan(&["--arch", "x86_64"]).steps().len(), 4);
        let os = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../os/configs");
        let fragment = std::fs::read_to_string(os.join(NO_15KHZ_FRAGMENT)).unwrap();
        assert!(fragment.lines().any(|l| l == "BR2_LINUX_KERNEL_PATCH=\"\""));
    }

    #[test]
    fn the_pc_image_pins_every_15khz_patch_by_commit_and_hash() {
        let os = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../os");
        let defconfig =
            std::fs::read_to_string(os.join("configs").join(Arch::X86_64.defconfig())).unwrap();
        let hashes = std::fs::read_to_string(os.join("patches/linux/linux.hash")).unwrap();
        for text in [&defconfig, &hashes] {
            assert!(text.contains("LICENCE QUESTION"), "the reminder is missing");
        }
        let line = defconfig
            .lines()
            .find_map(|l| l.strip_prefix("BR2_LINUX_KERNEL_PATCH=\""))
            .unwrap()
            .trim_end_matches('"');
        let urls: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(urls.len(), 9);
        for url in urls {
            assert!(
                url.starts_with(
                    "https://raw.githubusercontent.com/D0023R/linux_kernel_15khz/\
                     8c96dd9f0b343a6d1ef3c61041783ee106957886/linux-6.18/"
                ),
                "{url}"
            );
            let file = url.rsplit('/').next().unwrap();
            assert!(
                hashes
                    .lines()
                    .any(|l| l.starts_with("sha256  ") && l.ends_with(&format!("  {file}"))),
                "no hash for {file}"
            );
        }
    }

    #[test]
    fn an_output_name_gives_a_volume_and_images_of_their_own() {
        let p = plan(&["--arch", "x86_64", "--output", "x86_64-clean"]);
        assert_eq!(p.args.variant(), "x86_64-clean");
        assert_eq!(p.args.volume(), "x86_64-clean");
        let dev = plan(&[
            "--arch",
            "x86_64",
            "--output",
            "x86_64",
            "--dev-key",
            "/k.pub",
        ]);
        assert_eq!(
            dev.args.volume(),
            "x86_64",
            "the dev build uses the release output"
        );
        assert_eq!(dev.args.variant(), "x86_64-dev", "its images stay apart");
        assert!(
            dev.steps()[0]
                .join(" ")
                .contains("--volume neptron-br-out-x86_64:/work/out"),
            "{:?}",
            dev.steps()[0]
        );
        assert!(
            p.steps()[0]
                .join(" ")
                .contains("--volume neptron-br-out-x86_64-clean:/work/out"),
            "{:?}",
            p.steps()[0]
        );
        for bad in ["", "X86", "a b", "../x", "x/y"] {
            assert!(
                BuildArgs::parse(&["--output", bad], Arch::X86_64).is_err(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn the_dev_key_is_one_public_key_from_outside_the_checkout() {
        let repo = Path::new("/repo");
        let key = Path::new("/home/u/.ssh/bench.pub");
        let ed = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExample bench@lab\n";
        assert_eq!(dev_key_problem(key, repo, ed), None);
        assert!(dev_key_problem(Path::new("/repo/os/key.pub"), repo, ed).is_some());
        assert!(dev_key_problem(key, repo, "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n").is_some());
        assert!(dev_key_problem(key, repo, "").is_some());
        assert!(dev_key_problem(key, repo, "password123\n").is_some());
        assert!(dev_key_problem(key, repo, &format!("{ed}{ed}")).is_some());
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
    fn steps_apply_the_defconfig_collect_the_licences_build_and_copy() {
        let steps = plan(&["--", "linux-reconfigure"]).steps();
        assert_eq!(steps.len(), 5);
        let tail = |s: &Vec<String>, n: usize| s[s.len() - n..].join(" ");
        assert_eq!(
            tail(&steps[0], 6),
            "make -C /opt/buildroot O=/work/out BR2_EXTERNAL=/src/repo/os \
             neptron_qemu_aarch64_defconfig"
        );
        assert_eq!(tail(&steps[1], 2), "BR2_JLEVEL=4 linux-reconfigure");
        assert_eq!(tail(&steps[2], 2), "BR2_JLEVEL=4 legal-info");
        assert_eq!(
            tail(&steps[3], 4),
            "-C /opt/buildroot O=/work/out BR2_JLEVEL=4"
        );
        assert_eq!(
            tail(&steps[4], 4),
            "cp -rf /work/out/images/. /work/images/"
        );
    }

    #[test]
    fn the_x86_64_build_applies_the_pc_defconfig() {
        let mut p = plan(&[]);
        p.args.arch = Arch::X86_64;
        let line = p.steps()[0].join(" ");
        assert!(line.ends_with(" neptron_pc_x86_64_defconfig"), "{line}");
        assert!(
            line.contains("--volume neptron-br-out-x86_64:/work/out"),
            "{line}"
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
            assert!(line.contains("--volume /repo:/src/repo:ro"), "{line}");
            assert!(
                line.contains("--volume /cache/images/aarch64:/work/images"),
                "{line}"
            );
            assert!(line.contains(" neptron-dev:0123456789abcdef "), "{line}");
        }
    }
}
