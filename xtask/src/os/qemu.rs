//! `cargo xtask qemu`: the arguments, and the QEMU command line they give.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::host::Accel;
use super::{Arch, OsError};

/// Guest memory, in MiB. The machine runs one guest at a time with 2 GB.
pub const GUEST_MEMORY_MIB: u32 = 2048;
/// Guest CPUs.
pub const GUEST_CPUS: u32 = 2;
/// The 9p mount tag of the shared host directory. The guest init script
/// mounts it at `/mnt/host`.
pub const SHARE_TAG: &str = "neptron";
/// The firmware config entry that carries the script to run.
pub const RUN_ENTRY: &str = "opt/neptron/run";
/// The firmware config entry that carries the boot check script. The guest
/// runs it while the launcher runs, not instead of it.
pub const CHECK_ENTRY: &str = "opt/neptron/check";
/// The firmware config entry that carries the nonce of the run markers.
pub const NONCE_ENTRY: &str = "opt/neptron/nonce";
/// The firmware config entry that carries the guest deadline, in seconds.
pub const TIMEOUT_ENTRY: &str = "opt/neptron/timeout";
/// Time the host adds to the guest deadline for the boot and the shutdown.
pub const HOST_MARGIN: Duration = Duration::from_secs(30);

/// What `cargo xtask qemu` does in the guest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Job {
    /// Boot to the console shell for a person at the terminal.
    Interactive,
    /// Run a command line, given as separate words.
    Command(Vec<String>),
    /// Run a shell script file from the host.
    Script(PathBuf),
}

/// The arguments of `cargo xtask qemu`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QemuArgs {
    /// The target to boot.
    pub arch: Arch,
    /// An images directory that replaces the build output.
    pub images: Option<PathBuf>,
    /// The longest time the command may run in the guest.
    pub timeout: Duration,
    /// A host directory to share read-only at `/mnt/host`.
    pub share: Option<PathBuf>,
    /// What to do in the guest.
    pub job: Job,
}

impl QemuArgs {
    /// Parses `[--arch A] [--images DIR] [--timeout SECS] [--share DIR]
    /// [--script FILE | -- command...]`.
    pub fn parse(args: &[&str], default_arch: Arch) -> Result<Self, OsError> {
        let mut out = Self {
            arch: default_arch,
            images: None,
            timeout: DEFAULT_TIMEOUT,
            share: None,
            job: Job::Interactive,
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
                "--images" => out.images = Some(PathBuf::from(value()?)),
                "--share" => out.share = Some(PathBuf::from(value()?)),
                "--timeout" => {
                    let text = value()?;
                    match text.parse::<u64>() {
                        Ok(secs) if secs > 0 => out.timeout = Duration::from_secs(secs),
                        _ => {
                            return Err(OsError::BadNumber {
                                flag: "--timeout",
                                value: text.to_owned(),
                            });
                        }
                    }
                }
                "--script" => out.set_job(Job::Script(PathBuf::from(value()?)))?,
                "--" => {
                    let words: Vec<String> = it.map(|s| (*s).to_owned()).collect();
                    if words.is_empty() {
                        return Err(OsError::Usage("no command after --".to_owned()));
                    }
                    out.set_job(Job::Command(words))?;
                    break;
                }
                other => return Err(OsError::Usage(format!("unknown argument {other:?}"))),
            }
        }
        Ok(out)
    }

    fn set_job(&mut self, job: Job) -> Result<(), OsError> {
        if self.job != Job::Interactive {
            return Err(OsError::Usage(
                "give either --script or a command after --, not both".to_owned(),
            ));
        }
        self.job = job;
        Ok(())
    }
}

/// A command run: what the guest gets through the firmware config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestJob {
    /// The firmware config entry of the script: [`RUN_ENTRY`] or
    /// [`CHECK_ENTRY`].
    pub entry: &'static str,
    /// The script file that the guest runs.
    pub script: PathBuf,
    /// The nonce that the guest puts in its markers.
    pub nonce: String,
    /// The guest stops the command after this many seconds and powers off.
    pub timeout_secs: u64,
}

/// How the guest boots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Boot {
    /// QEMU loads the kernel of the images directory, and the root
    /// filesystem `rootfs.ext4` is a virtio disk.
    Kernel,
    /// The firmware boots a disk image from a USB stick on an xHCI
    /// controller, as a PC boots the image. x86-64 only.
    UsbDisk {
        /// The disk image.
        image: PathBuf,
        /// The firmware.
        firmware: Firmware,
    },
}

/// The firmware of a disk boot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Firmware {
    /// The default BIOS of QEMU (SeaBIOS).
    Bios,
    /// UEFI (EDK II): the code and the variable store flash images.
    Uefi {
        /// The code flash image, read-only.
        code: PathBuf,
        /// The variable store flash image. `-snapshot` keeps it unchanged.
        vars: PathBuf,
    },
}

/// Everything QEMU needs for one boot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QemuPlan {
    /// The target.
    pub arch: Arch,
    /// How the guest CPU runs.
    pub accel: Accel,
    /// The directory with the kernel and `rootfs.ext4`.
    pub images: PathBuf,
    /// How the guest boots.
    pub boot: Boot,
    /// The command run, or `None` for an interactive boot.
    pub job: Option<GuestJob>,
    /// The host directory shared over 9p.
    pub share: Option<PathBuf>,
}

impl QemuPlan {
    /// The QEMU arguments, without the program name.
    pub fn args(&self) -> Vec<String> {
        let mut a: Vec<String> = Vec::new();
        let mut push = |items: &[&str]| a.extend(items.iter().map(|s| (*s).to_owned()));
        let cpu = match self.accel {
            Accel::Hvf | Accel::Kvm => "host",
            Accel::Tcg => "max",
        };
        let (machine, blk_device) = match self.arch {
            Arch::Aarch64 => ("virt", "virtio-blk-device"),
            Arch::X86_64 => ("q35", "virtio-blk-pci"),
        };
        push(&[
            "-machine",
            machine,
            "-accel",
            self.accel.name(),
            "-cpu",
            cpu,
        ]);
        push(&["-smp", &GUEST_CPUS.to_string()]);
        push(&["-m", &GUEST_MEMORY_MIB.to_string()]);
        match &self.boot {
            Boot::Kernel => {
                push(&[
                    "-kernel",
                    &path_str(&self.images.join(self.arch.kernel_image())),
                ]);
                push(&["-append", &self.kernel_cmdline()]);
                let drive = format!(
                    "file={},if=none,format=raw,id=root",
                    path_str(&self.images.join(ROOTFS))
                );
                push(&[
                    "-drive",
                    &drive,
                    "-device",
                    &format!("{blk_device},drive=root"),
                ]);
            }
            Boot::UsbDisk { image, firmware } => {
                if let Firmware::Uefi { code, vars } = firmware {
                    let code = format!(
                        "if=pflash,format=raw,unit=0,readonly=on,file={}",
                        path_str(code)
                    );
                    let vars = format!("if=pflash,format=raw,unit=1,file={}", path_str(vars));
                    push(&["-drive", &code, "-drive", &vars]);
                }
                let drive = format!("file={},if=none,format=raw,id=usb", path_str(image));
                push(&[
                    "-drive",
                    &drive,
                    "-device",
                    "qemu-xhci,id=xhci",
                    "-device",
                    "usb-storage,bus=xhci.0,drive=usb,bootindex=0",
                ]);
            }
        }
        // The base image stays unchanged: guest writes go to a temporary overlay.
        push(&["-snapshot", "-nic", "none", "-no-reboot"]);
        // Without a display card vkms is the only DRM device, card0. The q35
        // machine adds a VGA card unless it is told not to.
        push(&["-vga", "none"]);
        match &self.job {
            None => push(&["-nographic"]),
            Some(job) => {
                push(&["-display", "none"]);
                push(&["-serial", "stdio", "-monitor", "none"]);
                let entries = [
                    format!("name={},file={}", job.entry, path_str(&job.script)),
                    format!("name={NONCE_ENTRY},string={}", job.nonce),
                    format!("name={TIMEOUT_ENTRY},string={}", job.timeout_secs),
                ];
                for entry in &entries {
                    push(&["-fw_cfg", entry]);
                }
            }
        }
        if let Some(share) = &self.share {
            let fs = format!(
                "local,path={},mount_tag={SHARE_TAG},security_model=none,readonly=on",
                path_str(share)
            );
            push(&["-virtfs", &fs]);
        }
        a
    }

    /// The kernel command line. `panic=-1` turns a panic into a reboot, and
    /// `-no-reboot` turns that into a QEMU exit, so a failed boot ends at once.
    pub fn kernel_cmdline(&self) -> String {
        format!(
            "root=/dev/vda rootwait console={} quiet panic=-1",
            self.arch.console()
        )
    }
}

/// The script that runs `words` as one command in the guest shell.
pub fn command_script(words: &[String]) -> String {
    let quoted: Vec<String> = words.iter().map(|w| shell_quote(w)).collect();
    format!("{}\n", quoted.join(" "))
}

/// Quotes `word` for a POSIX shell.
pub fn shell_quote(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:,+@%".contains(c));
    if plain {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

/// The root filesystem image in the images directory.
pub const ROOTFS: &str = "rootfs.ext4";

/// Long enough for a test program; `--timeout` raises it.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// QEMU splits option values on commas, so a comma in a path is doubled.
fn path_str(path: &Path) -> String {
    path.display().to_string().replace(',', ",,")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(arch: Arch, accel: Accel, run: bool) -> QemuPlan {
        QemuPlan {
            arch,
            accel,
            images: PathBuf::from("/img"),
            boot: Boot::Kernel,
            job: run.then(|| GuestJob {
                entry: RUN_ENTRY,
                script: PathBuf::from("/tmp/run.sh"),
                nonce: "0123456789abcdef".to_owned(),
                timeout_secs: 120,
            }),
            share: None,
        }
    }

    fn joined(p: &QemuPlan) -> String {
        p.args().join(" ")
    }

    #[test]
    fn no_arguments_is_an_interactive_boot() {
        let a = QemuArgs::parse(&[], Arch::Aarch64).unwrap();
        assert_eq!(a.job, Job::Interactive);
        assert_eq!(a.timeout, Duration::from_secs(120));
    }

    #[test]
    fn command_after_double_dash_keeps_its_words() {
        let a = QemuArgs::parse(
            &[
                "--arch",
                "x86_64",
                "--timeout",
                "30",
                "--",
                "ls",
                "-l",
                "--",
                "x",
            ],
            Arch::Aarch64,
        )
        .unwrap();
        assert_eq!(a.arch, Arch::X86_64);
        assert_eq!(a.timeout, Duration::from_secs(30));
        assert_eq!(
            a.job,
            Job::Command(vec!["ls".into(), "-l".into(), "--".into(), "x".into()])
        );
    }

    #[test]
    fn bad_arguments_are_rejected() {
        for bad in [
            &["--timeout", "0"][..],
            &["--timeout"],
            &["--"],
            &["--script", "a.sh", "--", "ls"],
            &["--bogus"],
        ] {
            assert!(QemuArgs::parse(bad, Arch::Aarch64).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn aarch64_on_a_mac_uses_hvf_and_virtio_mmio() {
        let s = joined(&plan(Arch::Aarch64, Accel::Hvf, true));
        assert!(
            s.starts_with("-machine virt -accel hvf -cpu host -smp 2 -m 2048"),
            "{s}"
        );
        assert!(s.contains("-kernel /img/Image"), "{s}");
        assert!(s.contains("-device virtio-blk-device,drive=root"), "{s}");
        assert!(
            s.contains("console=ttyAMA0 quiet panic=-1 -drive file=/img/rootfs.ext4"),
            "{s}"
        );
    }

    #[test]
    fn x86_64_without_kvm_is_emulated_on_q35() {
        let s = joined(&plan(Arch::X86_64, Accel::Tcg, true));
        assert!(s.starts_with("-machine q35 -accel tcg -cpu max"), "{s}");
        assert!(s.contains("-kernel /img/bzImage"), "{s}");
        assert!(s.contains("virtio-blk-pci,drive=root"), "{s}");
        assert!(s.contains("console=ttyS0"), "{s}");
        assert!(s.contains("-vga none"), "{s}");
    }

    #[test]
    fn a_run_passes_the_script_and_never_changes_the_image() {
        let s = joined(&plan(Arch::Aarch64, Accel::Hvf, true));
        assert!(s.contains("-snapshot"), "{s}");
        assert!(s.contains("-no-reboot"), "{s}");
        assert!(s.contains("-serial stdio -monitor none"), "{s}");
        assert!(
            s.contains("-fw_cfg name=opt/neptron/run,file=/tmp/run.sh"),
            "{s}"
        );
        assert!(
            s.contains("-fw_cfg name=opt/neptron/nonce,string=0123456789abcdef"),
            "{s}"
        );
        assert!(
            s.contains("-fw_cfg name=opt/neptron/timeout,string=120"),
            "{s}"
        );
        assert!(!s.contains("-nographic"), "{s}");
    }

    #[test]
    fn an_interactive_boot_has_the_console_on_the_terminal() {
        let s = joined(&plan(Arch::Aarch64, Accel::Hvf, false));
        assert!(s.contains("-nographic"), "{s}");
        assert!(s.contains("-vga none"), "{s}");
        assert!(!s.contains("-fw_cfg"), "{s}");
    }

    #[test]
    fn a_share_is_read_only_9p_and_commas_are_escaped() {
        let mut p = plan(Arch::Aarch64, Accel::Hvf, true);
        p.share = Some(PathBuf::from("/a,b"));
        let s = joined(&p);
        assert!(
            s.contains(
                "-virtfs local,path=/a,,b,mount_tag=neptron,security_model=none,readonly=on"
            ),
            "{s}"
        );
    }

    #[test]
    fn a_bios_disk_boot_is_a_usb_stick_and_no_kernel() {
        let mut p = plan(Arch::X86_64, Accel::Tcg, true);
        p.boot = Boot::UsbDisk {
            image: PathBuf::from("/img/neptron-x86_64.img"),
            firmware: Firmware::Bios,
        };
        let s = joined(&p);
        assert!(
            s.contains(
                "-drive file=/img/neptron-x86_64.img,if=none,format=raw,id=usb \
                 -device qemu-xhci,id=xhci -device usb-storage,bus=xhci.0,drive=usb,bootindex=0"
            ),
            "{s}"
        );
        assert!(!s.contains("-kernel"), "{s}");
        assert!(!s.contains("-append"), "{s}");
        assert!(!s.contains("pflash"), "{s}");
        assert!(s.contains("-snapshot"), "{s}");
        assert!(s.contains("-vga none"), "{s}");
    }

    #[test]
    fn a_uefi_disk_boot_adds_the_two_flash_images() {
        let mut p = plan(Arch::X86_64, Accel::Tcg, true);
        p.boot = Boot::UsbDisk {
            image: PathBuf::from("/img/disk.img"),
            firmware: Firmware::Uefi {
                code: PathBuf::from("/q/edk2-x86_64-code.fd"),
                vars: PathBuf::from("/t/vars.fd"),
            },
        };
        let s = joined(&p);
        assert!(
            s.contains(
                "-drive if=pflash,format=raw,unit=0,readonly=on,file=/q/edk2-x86_64-code.fd \
                 -drive if=pflash,format=raw,unit=1,file=/t/vars.fd"
            ),
            "{s}"
        );
    }

    #[test]
    fn a_check_job_uses_the_check_entry() {
        let mut p = plan(Arch::X86_64, Accel::Tcg, true);
        if let Some(job) = &mut p.job {
            job.entry = CHECK_ENTRY;
        }
        let s = joined(&p);
        assert!(
            s.contains("-fw_cfg name=opt/neptron/check,file=/tmp/run.sh"),
            "{s}"
        );
        assert!(!s.contains("opt/neptron/run"), "{s}");
    }

    #[test]
    fn words_are_quoted_for_the_shell() {
        assert_eq!(shell_quote("modetest"), "modetest");
        assert_eq!(shell_quote("-M"), "-M");
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
        assert_eq!(shell_quote("$HOME"), "'$HOME'");
        assert_eq!(
            command_script(&["echo".into(), "a b".into()]),
            "echo 'a b'\n"
        );
    }
}
