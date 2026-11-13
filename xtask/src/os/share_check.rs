//! `cargo xtask os share-check`: the tier T3 check of the share partition
//! on a stick that is written again. It boots a copy of the x86-64 disk
//! image three times, with the writes of the guest kept in the copy:
//!
//! 1. `fresh`: the free space after the state partition is empty. The
//!    first boot makes partition 4, formats it exFAT and mounts it; the
//!    guest writes a file to the share.
//! 2. `rewritten`: the host writes the image again over the start of the
//!    copy, as a new image written to a used stick. The table has no
//!    partition 4 and the state partition is new, but the old share is on
//!    the disk at the planned start. The boot must make partition 4 around
//!    it, mount it with no format, and the file must be there.
//! 3. `again`: a normal boot with partition 4 in the table.
//!
//! Each boot checks `/run/neptron/storage.toml`, `/proc/mounts` and the
//! lines of the storage script in `/var/log/messages`.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use super::boot_check::{X86_64_DISK, disk_with_share_room};
use super::host::{self, Cache};
use super::qemu::{Job, QemuArgs};
use super::{Arch, OsError, transcript};
use crate::repo::Repo;

/// The arguments of `os share-check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareCheckArgs {
    /// The guest deadline of each boot.
    pub timeout_secs: u64,
    /// An images directory that replaces the build output.
    pub images: Option<PathBuf>,
}

impl ShareCheckArgs {
    /// Parses `[--arch x86_64] [--timeout SECS] [--images DIR]`.
    pub fn parse(args: &[&str]) -> Result<Self, OsError> {
        let mut out = Self {
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            images: None,
        };
        let mut it = args.iter();
        while let Some(&arg) = it.next() {
            let value = it
                .next()
                .copied()
                .ok_or_else(|| OsError::Usage(format!("{arg} needs a value")))?;
            match arg {
                "--arch" if Arch::parse(value)? == Arch::X86_64 => {}
                "--arch" => {
                    return Err(OsError::Usage(
                        "share-check boots the PC disk image: --arch x86_64 only".to_owned(),
                    ));
                }
                "--timeout" => {
                    out.timeout_secs = match value.parse::<u64>() {
                        Ok(secs) if secs > 0 => secs,
                        _ => {
                            return Err(OsError::BadNumber {
                                flag: "--timeout",
                                value: value.to_owned(),
                            });
                        }
                    }
                }
                "--images" => out.images = Some(PathBuf::from(value)),
                other => return Err(OsError::Usage(format!("unknown argument {other:?}"))),
            }
        }
        Ok(out)
    }
}

/// The boots of the check, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The first boot on empty free space.
    Fresh,
    /// The first boot after the image is written again over the copy.
    Rewritten,
    /// A normal boot after that.
    Again,
}

/// The boots of the check, in order.
pub const STEPS: [Step; 3] = [Step::Fresh, Step::Rewritten, Step::Again];

impl Step {
    /// The name in the output and in the file names.
    pub fn name(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Rewritten => "rewritten",
            Self::Again => "again",
        }
    }
}

/// The guest script of `step`. `nonce` is the text of the file that the
/// first boot writes to the share and the later boots read.
pub fn guest_script(step: Step, nonce: &str) -> String {
    let mut s = String::from(
        r#"fail() {
	echo "share-check: FAIL STEP: $1"
	echo "share-check: /run/neptron/storage.toml:"; cat /run/neptron/storage.toml
	echo "share-check: storage lines in /var/log/messages:"; grep neptron-storage /var/log/messages
	exit 1
}
logged() {
	grep -q "neptron-storage: $1" /var/log/messages
}
grep -qx 'share = "partition"' /run/neptron/storage.toml || fail "/share is not the share partition"
grep -q '^/dev/sda4 /share exfat ' /proc/mounts || fail "/share is not partition 4 as exFAT"
logged '/dev/sda4 mounted at /share' || fail "/var/log/messages has no line of the storage script"
grep -q '/dev/sda4 mounted at /share' /state/log/storage.log || fail "/state/log/storage.log has no line of this boot"
"#,
    );
    let file = format!("/share/{FILE}");
    let read_back = format!(
        "[ \"$(cat {file})\" = \"{nonce}\" ] || fail \"{file} of the first boot is not there\"\n"
    );
    match step {
        Step::Fresh => {
            s.push_str(
                "logged '/dev/sda4 holds no filesystem; formatting it exFAT' || fail \"the empty space was not formatted\"\n",
            );
            s.push_str(FLUSHED);
            s.push_str(&format!(
                "echo \"{nonce}\" > {file} && sync || fail \"{file} cannot be written\"\n"
            ));
        }
        Step::Rewritten => {
            s.push_str(
                "logged 'an old exFAT share starts at sector' || fail \"the old share was not found at the planned start\"\n\
                 logged 'formatting it exFAT' && fail \"the old share was formatted\"\n\
                 [ ! -e /state/share-pending ] || fail \"the marker /state/share-pending stays after the mount\"\n",
            );
            s.push_str(FLUSHED);
            s.push_str(&read_back);
        }
        Step::Again => {
            s.push_str(
                "logged 'making partition 4' && fail \"partition 4 was made again\"\n\
                 logged 'formatting it exFAT' && fail \"the share was formatted\"\n",
            );
            s.push_str(&read_back);
        }
    }
    s.push_str("grep neptron-storage /var/log/messages\necho \"share-check: STEP PASS\"\nexit 0\n");
    s.replace("STEP", step.name())
}

/// Writes `image` over the start of `disk` and keeps the rest of `disk`,
/// as a new image written with `dd` to a used stick. Fails when the image
/// has a partition 4: the check needs a table with no share partition.
pub fn rewrite_image(image: &Path, disk: &Path) -> Result<(), OsError> {
    let io = |path: &Path| {
        let path = path.to_owned();
        move |source| OsError::Io { path, source }
    };
    let mut source = std::fs::File::open(image).map_err(io(image))?;
    let mut target = std::fs::OpenOptions::new()
        .write(true)
        .read(true)
        .open(disk)
        .map_err(io(disk))?;
    std::io::copy(&mut source, &mut target).map_err(io(disk))?;
    target.flush().map_err(io(disk))?;
    let mut entry = [0_u8; 16];
    target
        .seek(SeekFrom::Start(MBR_ENTRY_4_OFFSET))
        .map_err(io(disk))?;
    target.read_exact(&mut entry).map_err(io(disk))?;
    if entry != [0; 16] {
        return Err(OsError::Usage(format!(
            "{} has a partition 4 in its table; the share check needs an image with none",
            image.display()
        )));
    }
    Ok(())
}

/// `cargo xtask os share-check`.
pub fn run(args: &[&str]) -> Result<ExitCode, OsError> {
    let args = ShareCheckArgs::parse(args)?;
    let repo = Repo::discover()?;
    let cache = Cache::from_env()?;
    let images = match (&args.images, std::env::var_os(host::IMAGES_ENV)) {
        (Some(dir), _) => dir.clone(),
        (None, Some(dir)) if !dir.is_empty() => PathBuf::from(dir),
        _ => cache.images(Arch::X86_64),
    };
    let image = images.join(X86_64_DISK);
    if !image.is_file() {
        return Err(OsError::NoImage {
            missing: image,
            arch: Arch::X86_64.name(),
            kernel: Arch::X86_64.kernel_image(),
        });
    }
    let out_dir = repo.root.join("target").join("os").join("share-check");
    let disk = disk_with_share_room(&image, &out_dir)?;
    let nonce = transcript::new_nonce();
    let mut passed = 0;
    for step in STEPS {
        if step == Step::Rewritten {
            rewrite_image(&image, &disk)?;
        }
        let script = out_dir.join(format!("{}.sh", step.name()));
        super::write_file(&script, &guest_script(step, &nonce))?;
        let qemu = QemuArgs {
            arch: Arch::X86_64,
            images: Some(images.clone()),
            timeout: Duration::from_secs(args.timeout_secs),
            share: None,
            disk: Some(disk.clone()),
            ssh_port: None,
            beside_launcher: false,
            job: Job::Script(script),
        };
        let mut console = Vec::new();
        let code = super::boot(&qemu, &mut console)?;
        let text = String::from_utf8_lossy(&console);
        for line in text.lines().filter(|l| is_report_line(l)) {
            println!("{line}");
        }
        if code != Some(0) {
            println!(
                "share-check {}: the guest script exited with {code:?}",
                step.name()
            );
            break;
        }
        passed += 1;
    }
    let ok = passed == STEPS.len();
    println!(
        "share-check x86_64: {passed} of {} boots pass: {}",
        STEPS.len(),
        if ok { "PASS" } else { "FAIL" }
    );
    Ok(super::exit_code(ok))
}

/// The guest deadline of each boot when `--timeout` is not given: a boot
/// under CPU emulation takes about a minute.
const DEFAULT_TIMEOUT_SECS: u64 = 300;
/// The check of a boot that made partition 4: the table was read back
/// from the device.
const FLUSHED: &str = "logged 'blockdev --flushbufs exit 0: the table is read back after a flush of the cache' || fail \"the table was not read back after a flush of the cache\"\n";
/// The file that the first boot writes to the share.
const FILE: &str = "share-check.txt";
/// The byte offset of the fourth primary entry of an MBR.
const MBR_ENTRY_4_OFFSET: u64 = 446 + 3 * 16;

fn is_report_line(line: &str) -> bool {
    line.contains("share-check") || line.contains("neptron-storage")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_parse_and_only_x86_64_is_taken() {
        let a = ShareCheckArgs::parse(&[]).unwrap();
        assert_eq!(a.timeout_secs, 300);
        assert_eq!(a.images, None);
        let a = ShareCheckArgs::parse(&["--arch", "x86_64", "--timeout", "60", "--images", "/i"])
            .unwrap();
        assert_eq!(a.timeout_secs, 60);
        assert_eq!(a.images, Some(PathBuf::from("/i")));
        for bad in [
            &["--arch", "aarch64"][..],
            &["--timeout", "0"],
            &["--images"],
            &["--bless", "x"],
        ] {
            assert!(ShareCheckArgs::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn every_boot_checks_the_mount_and_the_syslog_lines() {
        for step in STEPS {
            let s = guest_script(step, "n0nce");
            for part in [
                "grep -qx 'share = \"partition\"' /run/neptron/storage.toml",
                "grep -q '^/dev/sda4 /share exfat ' /proc/mounts",
                "grep -q \"neptron-storage: $1\" /var/log/messages",
                "logged '/dev/sda4 mounted at /share'",
                "grep -q '/dev/sda4 mounted at /share' /state/log/storage.log",
            ] {
                assert!(s.contains(part), "{}: {part}: {s}", step.name());
            }
            assert!(
                s.contains(&format!("share-check: FAIL {}: $1", step.name())),
                "{s}"
            );
            assert!(
                s.ends_with(&format!(
                    "echo \"share-check: {} PASS\"\nexit 0\n",
                    step.name()
                )),
                "{s}"
            );
        }
    }

    #[test]
    fn the_first_boot_formats_and_the_later_boots_keep_the_file() {
        let fresh = guest_script(Step::Fresh, "n0nce");
        assert!(fresh.contains("formatting it exFAT' || fail"), "{fresh}");
        assert!(fresh.contains(FLUSHED), "{fresh}");
        assert!(guest_script(Step::Rewritten, "n0nce").contains(FLUSHED));
        assert!(!guest_script(Step::Again, "n0nce").contains(FLUSHED));
        assert!(
            fresh.contains("echo \"n0nce\" > /share/share-check.txt && sync"),
            "{fresh}"
        );
        let rewritten = guest_script(Step::Rewritten, "n0nce");
        for part in [
            "logged 'an old exFAT share starts at sector' || fail",
            "logged 'formatting it exFAT' && fail",
            "[ ! -e /state/share-pending ]",
            "[ \"$(cat /share/share-check.txt)\" = \"n0nce\" ]",
        ] {
            assert!(rewritten.contains(part), "{part}: {rewritten}");
        }
        let again = guest_script(Step::Again, "n0nce");
        assert!(
            again.contains("logged 'making partition 4' && fail"),
            "{again}"
        );
        assert!(again.contains("= \"n0nce\" ]"), "{again}");
    }

    #[test]
    fn the_image_is_written_over_the_start_and_the_rest_stays() {
        let dir = std::env::temp_dir().join(format!("xtask-share-check-{}", std::process::id()));
        // A leftover of an earlier run with the same process id; absence is fine.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let image = dir.join("image.img");
        let disk = dir.join("disk.img");
        let mut bytes = vec![1_u8; 1024];
        bytes[494..510].fill(0);
        std::fs::write(&image, &bytes).unwrap();
        std::fs::write(&disk, vec![9_u8; 4096]).unwrap();
        rewrite_image(&image, &disk).unwrap();
        let written = std::fs::read(&disk).unwrap();
        assert_eq!(written.len(), 4096);
        assert_eq!(&written[..1024], &bytes[..]);
        assert!(
            written[1024..].iter().all(|b| *b == 9),
            "the old share stays"
        );

        bytes[498] = 7;
        std::fs::write(&image, &bytes).unwrap();
        let err = rewrite_image(&image, &disk).unwrap_err();
        assert!(err.to_string().contains("has a partition 4"), "{err}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn report_lines_are_those_of_the_check_and_of_the_storage_script() {
        assert!(is_report_line("share-check: fresh PASS"));
        assert!(is_report_line(
            "Nov 13 16:21:34 neptron user.notice neptron-storage: /dev/sda4 mounted at /share"
        ));
        assert!(!is_report_line("[    1.234] usb 2-1: new device"));
    }
}
