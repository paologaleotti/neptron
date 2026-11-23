//! `cargo xtask os build`: the development container and the Buildroot build.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::source::SourceInfo;
use super::{Arch, Board, OsError};

/// The repository of the development container images.
pub const CONTAINER_REPO: &str = "neptron-dev";

/// The image name for a `Dockerfile` text: its tag is a hash of the text, so
/// branches with different Dockerfiles never replace each other's image.
pub fn container_image(dockerfile: &str) -> String {
    format!("{CONTAINER_REPO}:{:016x}", fnv1a(dockerfile))
}

/// FNV-1a, 64 bits, of `text`: stable across Rust releases, for names that
/// need no cryptographic strength.
pub fn fnv1a(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The arguments of `os build`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildArgs {
    /// The image to build.
    pub board: Board,
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
    /// [--output NAME] [-- targets...]`. `--arch` takes a QEMU target or
    /// `rpi-arm64`; with none, the default architecture names the image.
    pub fn parse(args: &[&str], default_arch: Arch) -> Result<Self, OsError> {
        let mut out = Self {
            board: Board::of_arch(default_arch),
            jobs: DEFAULT_JOBS,
            targets: Vec::new(),
            dev_key: None,
            no_kernel_15khz: false,
            output: None,
        };
        let mut it = args.iter();
        while let Some(&arg) = it.next() {
            match arg {
                "--arch" => out.board = Board::parse(value(&mut it, "--arch")?)?,
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
        if out.dev_key.is_some() && out.board.dev_fragment().is_none() {
            return Err(OsError::Usage(format!(
                "--dev-key: the {} image has no development variant",
                out.board.name()
            )));
        }
        if out.no_kernel_15khz && out.board != Board::Pc {
            return Err(OsError::Usage(
                "--no-kernel-15khz is a variant of the x86_64 PC image".to_owned(),
            ));
        }
        Ok(out)
    }

    /// The configuration fragments of the variant, in `os/configs`, in the
    /// order of the merge.
    pub fn fragments(&self) -> Vec<&'static str> {
        let mut fragments = Vec::new();
        if self.dev_key.is_some()
            && let Some(fragment) = self.board.dev_fragment()
        {
            fragments.push(fragment);
        }
        if self.no_kernel_15khz {
            fragments.push(NO_15KHZ_FRAGMENT);
        }
        fragments
    }

    /// The name of the images directory: the `--output` name or the
    /// image name, with `-dev` for the development variant and `-no15khz`
    /// for the kernel without the 15 kHz patch set. Without `--output` it
    /// also names the output volume, so the variants never share an output
    /// and no file of a variant can reach the release image.
    pub fn variant(&self) -> String {
        let mut name = self
            .output
            .clone()
            .unwrap_or_else(|| self.board.name().to_owned());
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

/// The packages of `os/package` that build from this repository, in build
/// order, with the configuration symbol that turns each one on.
pub const LOCAL_PACKAGES: [(&str, &str); 5] = [
    ("neptron", "BR2_PACKAGE_NEPTRON"),
    ("neptron-launch", "BR2_PACKAGE_NEPTRON_LAUNCH"),
    ("neptronctl", "BR2_PACKAGE_NEPTRONCTL"),
    ("neptron-capture", "BR2_PACKAGE_NEPTRON_CAPTURE"),
    ("neptron-vkms-probe", "BR2_PACKAGE_NEPTRON_VKMS_PROBE"),
];

/// The make targets that build each package of [`LOCAL_PACKAGES`] that
/// `configs` turn on again from the current source: `<package>-reconfigure`.
/// `configs` are the texts of the defconfig and of the fragments, in the
/// order of the merge; a later line wins.
///
/// Buildroot does not see a change to the source of a local package. A
/// `-rebuild` or a `-reconfigure` copies the source again (with the hooks
/// of `os/package/neptron/local-source.mk`, every file of it).
/// `-reconfigure`, not `-rebuild`: the configure step runs `cargo vendor`,
/// and a `Cargo.lock` with a new crate fails the offline build with the
/// old vendor folder. A package that is off gets no target, because its
/// target would build it and put it in the image.
pub fn rebuild_targets(configs: &[&str]) -> Vec<String> {
    LOCAL_PACKAGES
        .iter()
        .filter(|(_, symbol)| {
            let mut on = false;
            for line in configs.iter().flat_map(|c| c.lines()).map(str::trim) {
                if line == format!("{symbol}=y") {
                    on = true;
                } else if line.starts_with(&format!("{symbol}="))
                    || line == format!("# {symbol} is not set")
                {
                    on = false;
                }
            }
            on
        })
        .map(|(package, _)| format!("{package}-reconfigure"))
        .collect()
}

/// Every Docker command of one `os build`, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildPlan {
    /// The arguments.
    pub args: BuildArgs,
    /// The targets that build the local packages again, from
    /// [`rebuild_targets`]. They run before the targets of the user.
    pub rebuild: Vec<String>,
    /// The checkout. Buildroot reads the br2-external tree `os/` in it, and
    /// the packages of the frontend and the launcher build its workspace.
    pub checkout: PathBuf,
    /// The host directory that receives the images.
    pub images: PathBuf,
    /// The container image, from [`container_image`].
    pub container: String,
    /// The commit, commit date and release of the checkout, which the
    /// image names (`board/common/install-licences.sh`).
    pub source: SourceInfo,
}

impl BuildPlan {
    /// The arguments of `docker build`. The Dockerfile comes on standard
    /// input, so Docker sends no build context: the repository holds `target/`
    /// directories of many gigabytes.
    pub fn container_build(&self) -> Vec<String> {
        container_build(&self.container)
    }

    /// The `docker run` arguments of each step: apply the defconfig, build
    /// the local packages again, run the extra targets, collect the
    /// licences, build everything, copy the images out. The licences come
    /// first: the image carries them. A target of the user that is also a
    /// rebuild target runs once, in the rebuild.
    pub fn steps(&self) -> Vec<Vec<String>> {
        let jobs = format!("BR2_JLEVEL={}", self.args.jobs);
        let external = format!("BR2_EXTERNAL={CHECKOUT_MOUNT}/os");
        let mut steps = vec![self.make(&[&external, self.args.board.defconfig()])];
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
        let user = self
            .args
            .targets
            .iter()
            .filter(|t| !self.rebuild.contains(t));
        for target in self.rebuild.iter().chain(user) {
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
        args.extend(self.source.env_args());
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
            rebuild: Vec::new(),
            checkout: PathBuf::from("/repo"),
            images: PathBuf::from("/cache/images/aarch64"),
            container: "neptron-dev:0123456789abcdef".to_owned(),
            source: SourceInfo::from_git(
                Some("0123456789abcdef0123456789abcdef01234567"),
                Some(""),
                Some("1795437296"),
                None,
            ),
        }
    }

    #[test]
    fn defaults_are_the_host_arch_and_four_jobs() {
        let a = BuildArgs::parse(&[], Arch::X86_64).unwrap();
        assert_eq!(a.board, Board::Pc);
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
        assert_eq!(a.board, Board::Pc);
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
    fn the_pi_image_and_its_dev_variant_build_in_outputs_of_their_own() {
        let release = plan(&["--arch", "rpi-arm64"]);
        assert_eq!(release.args.board, Board::RpiArm64);
        assert_eq!(release.args.variant(), "rpi-arm64");
        let steps = release.steps();
        assert_eq!(steps.len(), 4, "no fragment to merge");
        assert!(
            steps[0]
                .join(" ")
                .ends_with("BR2_EXTERNAL=/src/repo/os neptron_rpi_arm64_defconfig"),
            "{:?}",
            steps[0]
        );
        for step in &steps {
            let line = step.join(" ");
            assert!(
                line.contains("--volume neptron-br-out-rpi-arm64:/work/out"),
                "{line}"
            );
            assert!(!line.contains("dev-key"), "{line}");
        }
        let dev = plan(&["--arch", "rpi-arm64", "--dev-key", "/home/u/.ssh/bench.pub"]);
        assert_eq!(dev.args.variant(), "rpi-arm64-dev");
        let steps = dev.steps();
        assert_eq!(steps.len(), 6);
        assert!(
            steps[1]
                .join(" ")
                .ends_with("/work/out/.config /src/repo/os/configs/neptron_rpi_arm64_dev.fragment"),
            "{:?}",
            steps[1]
        );
        for step in &steps {
            let line = step.join(" ");
            assert!(
                line.contains("--volume neptron-br-out-rpi-arm64-dev:/work/out"),
                "{line}"
            );
            assert!(
                line.contains("--volume /home/u/.ssh/bench.pub:/work/dev-key/authorized_keys:ro"),
                "{line}"
            );
        }
        assert!(
            BuildArgs::parse(&["--arch", "rpi-arm64", "--no-kernel-15khz"], Arch::X86_64).is_err(),
            "the Pi kernel has no 15 kHz patch set to leave out"
        );
        let x86 = BuildArgs::parse(&["--arch", "x86_64"], Arch::Aarch64).unwrap();
        assert_eq!(
            (x86.board, x86.variant()),
            (Board::Pc, "x86_64".to_owned()),
            "the PC image keeps its output"
        );
        let qemu = BuildArgs::parse(&[], Arch::Aarch64).unwrap();
        assert_eq!(
            (qemu.board, qemu.variant()),
            (Board::QemuAarch64, "aarch64".to_owned()),
            "aarch64 stays the QEMU test image"
        );
    }

    #[test]
    fn the_pc_image_pins_every_15khz_patch_by_commit_and_hash() {
        let os = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../os");
        let defconfig =
            std::fs::read_to_string(os.join("configs").join(Arch::X86_64.defconfig())).unwrap();
        let hashes = std::fs::read_to_string(os.join("patches/linux/linux.hash")).unwrap();
        for text in [&defconfig, &hashes] {
            assert!(
                text.contains("LICENCE: GPL-2.0-only") && text.contains("ADR 0018"),
                "the licence of the pinned set is not recorded"
            );
            assert!(!text.contains("8c96dd9f"), "the GPL-3.0 commit is pinned");
        }
        let notes = std::fs::read_to_string(os.join("board/pc/LICENCES.txt")).unwrap();
        for fact in [
            "github.com/D0023R/linux_kernel_15khz",
            "Commit: d85a343379f7f54155f69ef56dc544b666635894.",
            "Licence: GNU General Public License 2.0 only",
            "sources/linux-6.18.63",
        ] {
            assert!(notes.contains(fact), "LICENCES.txt misses {fact:?}");
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
                     d85a343379f7f54155f69ef56dc544b666635894/linux-6.18/"
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

    /// `LICENCES.txt` goes into every PC variant, so it must be true for an
    /// image with the 15 kHz set and for one built without it, and it must
    /// carry the written offer of GPL-2.0 section 3(b).
    #[test]
    fn the_pc_licence_notes_hold_for_both_kernel_variants_and_offer_the_source() {
        let os = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../os");
        let notes = std::fs::read_to_string(os.join("board/pc/LICENCES.txt")).unwrap();
        let flat = notes.split_whitespace().collect::<Vec<_>>().join(" ");
        for fact in [
            "The PC image is built in two variants, and this file goes into both",
            "The image with the 15 kHz patch set (the default build) applies the nine patches",
            "--no-kernel-15khz` applies no kernel patch",
            "(none in the image without the set)",
            "For at least three years from the last date on which the NEPTRON project gives out this image",
            "gives any third party, on request, a complete machine-readable copy",
            "a medium customarily used for software interchange; the usual case is a download",
            "the GNU General Public License for a GPL package, and the GNU Lesser General Public License for an LGPL package",
            "Name the image file and its SHA-256, and the NEPTRON commit at the top of this file.",
            "https://github.com/paologaleotti/neptron/releases/tag/RELEASE, where RELEASE is the release named at the top of this file.",
            "Next to each image file, `NAME.img.xz`, is its source, `NAME-sources.tar`.",
            "the Buildroot release that built the image, the NEPTRON source at the commit named at the top of this file",
            "An issue needs a GitHub account; the project has no other contact today.",
            "also for an image that reached you without its source",
            "The file `grub.cfg` on the boot partition",
            "GNU General Public License or the GNU Lesser General Public License",
            "the Linux kernel with the patches that this image applies",
            "no more than the cost of the physical act of the source distribution",
            "https://github.com/paologaleotti/neptron/issues",
        ] {
            assert!(flat.contains(fact), "LICENCES.txt misses {fact:?}");
        }
        for gone in [
            "ask the person who gave you the image",
            "from the date of this image",
            "when possible",
        ] {
            assert!(!flat.contains(gone), "LICENCES.txt still says {gone:?}");
        }
        // The PARTUUIDs that tell the variants apart are those post-image.sh
        // writes for each variant.
        let post_image = std::fs::read_to_string(os.join("board/pc/post-image.sh")).unwrap();
        let signature = |case: &str| {
            post_image
                .lines()
                .find_map(|l| l.trim().strip_prefix(&format!("{case}) signature=")))
                .unwrap()
                .trim_end_matches(" ;;")
                .to_owned()
        };
        let with_set = format!(
            "PARTUUID {}-02 or {}-02 in an image with the set",
            signature("no-yes"),
            signature("yes-yes")
        );
        let without = format!(
            "{}-02 or {}-02 in an image without it",
            signature("no-no"),
            signature("yes-no")
        );
        assert!(flat.contains(&with_set), "{with_set}");
        assert!(flat.contains(&without), "{without}");
    }

    /// With `BR2_KERNEL_HEADERS_AS_KERNEL`, the `linux-headers` package of
    /// Buildroot downloads the remote kernel patches too, and checks them
    /// against its own hash file only. With forced hash checks, a patch
    /// that is missing there stops a build from an empty output at
    /// "No hash found" (OP-111).
    #[test]
    fn the_kernel_headers_check_every_kernel_patch_against_a_hash() {
        let os = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../os");
        let defconfig =
            std::fs::read_to_string(os.join("configs").join(Arch::X86_64.defconfig())).unwrap();
        assert!(defconfig.contains("BR2_KERNEL_HEADERS_AS_KERNEL=y"));
        assert!(defconfig.contains("BR2_DOWNLOAD_FORCE_CHECK_HASHES=y"));
        let kernel = std::fs::read_to_string(os.join("patches/linux/linux.hash")).unwrap();
        let headers =
            std::fs::read_to_string(os.join("patches/linux-headers/linux-headers.hash")).unwrap();
        assert_eq!(headers, kernel, "the two packages check the same files");
        let line = defconfig
            .lines()
            .find_map(|l| l.strip_prefix("BR2_LINUX_KERNEL_PATCH=\""))
            .unwrap()
            .trim_end_matches('"');
        for url in line.split_whitespace() {
            let file = url.rsplit('/').next().unwrap();
            assert!(
                headers
                    .lines()
                    .any(|l| l.starts_with("sha256  ") && l.ends_with(&format!("  {file}"))),
                "linux-headers.hash has no hash for {file}"
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
        p.args.board = Board::Pc;
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
            assert!(
                line.contains(
                    " --env NEPTRON_SOURCE_COMMIT=0123456789abcdef0123456789abcdef01234567 \
                     --env NEPTRON_SOURCE_DATE=2026-11-23T12:34:56Z \
                     --env NEPTRON_SOURCE_RELEASE=none neptron-dev:"
                ),
                "{line}"
            );
        }
    }

    fn repo_file(path: &str) -> String {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(path))
            .unwrap()
    }

    #[test]
    fn the_rebuild_targets_run_once_and_before_the_targets_of_the_user() {
        let mut p = plan(&[
            "--",
            "source",
            "neptron-reconfigure",
            "linux-reconfigure",
            "neptronctl-reconfigure",
        ]);
        p.rebuild = rebuild_targets(&[&repo_file("os/configs/neptron_qemu_aarch64_defconfig")]);
        let tail = |s: &Vec<String>| s[s.len() - 2..].join(" ");
        let made: Vec<String> = p.steps()[1..9].iter().map(tail).collect();
        assert_eq!(
            made,
            [
                "BR2_JLEVEL=4 neptron-reconfigure",
                "BR2_JLEVEL=4 neptron-launch-reconfigure",
                "BR2_JLEVEL=4 neptronctl-reconfigure",
                "BR2_JLEVEL=4 neptron-capture-reconfigure",
                "BR2_JLEVEL=4 neptron-vkms-probe-reconfigure",
                "BR2_JLEVEL=4 source",
                "BR2_JLEVEL=4 linux-reconfigure",
                "BR2_JLEVEL=4 legal-info",
            ]
        );
        assert_eq!(p.steps().len(), 11);
    }

    #[test]
    fn a_build_with_no_targets_still_builds_the_local_packages_again() {
        let mut p = plan(&["--arch", "rpi-arm64"]);
        p.rebuild = rebuild_targets(&[&repo_file("os/configs/neptron_rpi_arm64_defconfig")]);
        let lines: Vec<String> = p.steps().iter().map(|s| s.join(" ")).collect();
        let at = |t: &str| {
            lines
                .iter()
                .position(|l| l.ends_with(&format!("BR2_JLEVEL=4 {t}")))
        };
        let rebuild = [
            "neptron-reconfigure",
            "neptron-launch-reconfigure",
            "neptronctl-reconfigure",
        ];
        for (n, target) in rebuild.iter().enumerate() {
            assert_eq!(at(target), Some(n + 1), "{target}: {lines:#?}");
        }
        assert!(
            at("neptron-capture-reconfigure").is_none(),
            "off in the Pi image"
        );
        assert!(at("legal-info").unwrap() > 3);
    }

    #[test]
    fn the_rebuild_targets_follow_the_configuration() {
        for (defconfig, count) in [
            ("neptron_qemu_aarch64_defconfig", 5),
            ("neptron_pc_x86_64_defconfig", 5),
            ("neptron_rpi_arm64_defconfig", 3),
        ] {
            let text = repo_file(&format!("os/configs/{defconfig}"));
            assert_eq!(rebuild_targets(&[&text]).len(), count, "{defconfig}");
        }
        let base = "BR2_PACKAGE_NEPTRON=y\nBR2_PACKAGE_NEPTRON_CAPTURE=y\n";
        assert_eq!(
            rebuild_targets(&[base]),
            ["neptron-reconfigure", "neptron-capture-reconfigure"]
        );
        assert_eq!(
            rebuild_targets(&[base, "# BR2_PACKAGE_NEPTRON_CAPTURE is not set\n"]),
            ["neptron-reconfigure"],
            "a later fragment wins"
        );
        assert_eq!(
            rebuild_targets(&["BR2_PACKAGE_NEPTRON_CONTROL=y\n"]),
            Vec::<String>::new(),
            "a symbol with the package name as a prefix is another symbol"
        );
        assert!(rebuild_targets(&[]).is_empty());
    }

    /// Every package of `os/package` that builds from this tree is in the
    /// rebuild list and copies its whole source again on each rsync.
    #[test]
    fn every_local_package_is_rebuilt_and_copies_its_source_again() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../os/package");
        let mut local = Vec::new();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            let Ok(mk) = std::fs::read_to_string(dir.join(&name).join(format!("{name}.mk"))) else {
                continue;
            };
            if mk.lines().any(|l| l.ends_with("_SITE_METHOD = local")) {
                let var = name.to_uppercase().replace('-', "_");
                for hook in [
                    format!("{var}_PRE_RSYNC_HOOKS += NEPTRON_LOCAL_SOURCE_CLEAR"),
                    format!("{var}_POST_RSYNC_HOOKS += NEPTRON_LOCAL_SOURCE_TOUCH"),
                ] {
                    assert!(mk.lines().any(|l| l == hook), "{name}.mk lacks {hook:?}");
                }
                local.push(name);
            }
        }
        local.sort();
        let mut listed: Vec<String> = LOCAL_PACKAGES
            .iter()
            .map(|(p, _)| (*p).to_owned())
            .collect();
        listed.sort();
        assert_eq!(local, listed);
        for (package, symbol) in LOCAL_PACKAGES {
            let config = std::fs::read_to_string(dir.join(package).join("Config.in")).unwrap();
            assert!(
                config
                    .lines()
                    .any(|l| l.trim() == format!("config {symbol}")),
                "{package}: {symbol}"
            );
        }
    }

    /// The hooks of `local-source.mk`, run by make on folders that stand
    /// for a build folder with a stale copy. `cp -pR` keeps the file times
    /// as the rsync of Buildroot does.
    #[test]
    fn the_rsync_hooks_replace_a_stale_copy_and_give_it_a_new_time() {
        let root = std::env::temp_dir().join(format!("xtask-local-source-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root); // A folder of an earlier run may not exist.
        let (src, dest) = (root.join("src"), root.join("build"));
        let write = |path: &Path, text: &str| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
        write(&src.join("crates/types/lib.rs"), "new");
        write(&src.join(".cargo/config.toml"), "checkout");
        write(&src.join("Cargo.toml"), "[workspace]");
        for file in ["crates/types/lib.rs", "Cargo.toml"] {
            std::fs::File::options()
                .write(true)
                .open(src.join(file))
                .unwrap()
                .set_modified(old)
                .unwrap();
        }
        write(&dest.join("crates/types/lib.rs"), "old");
        write(&dest.join("crates/types/removed.rs"), "old");
        write(&dest.join("target/release/neptron.d"), "dep-info");
        write(&dest.join("VENDOR/serde/lib.rs"), "vendored");
        write(&dest.join(".cargo/config.toml"), "checkout and vendor");
        write(&dest.join(".stamp_configured"), "");
        let built = std::fs::metadata(dest.join("target/release/neptron.d"))
            .unwrap()
            .modified()
            .unwrap();
        let mk =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../os/package/neptron/local-source.mk");
        let makefile = format!(
            "include {}\nqstrip = $(strip $(subst \",,$(1)))\nSRCDIR = {}\n\
             {}/.stamp_rsynced:\n\
             \t$(NEPTRON_LOCAL_SOURCE_CLEAR)\n\
             \tcp -pR {}/crates {}/Cargo.toml $(@D)/\n\
             \t$(NEPTRON_LOCAL_SOURCE_TOUCH)\n",
            mk.display(),
            src.display(),
            dest.display(),
            src.display(),
            src.display(),
        );
        write(&root.join("Makefile"), &makefile);
        let out = std::process::Command::new("make")
            .arg("-s")
            .arg("-C")
            .arg(&root)
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        let read = |p: &str| std::fs::read_to_string(dest.join(p)).unwrap();
        assert_eq!(read("crates/types/lib.rs"), "new");
        assert!(!dest.join("crates/types/removed.rs").exists());
        assert_eq!(read("target/release/neptron.d"), "dep-info");
        assert_eq!(read("VENDOR/serde/lib.rs"), "vendored");
        assert_eq!(read(".cargo/config.toml"), "checkout and vendor");
        assert!(dest.join(".stamp_configured").exists());
        for file in ["crates/types/lib.rs", "Cargo.toml"] {
            let time = std::fs::metadata(dest.join(file))
                .unwrap()
                .modified()
                .unwrap();
            assert!(time >= built, "{file} is older than the build output");
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}
