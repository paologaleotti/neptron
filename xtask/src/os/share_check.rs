//! `cargo xtask os share-check`: the tier T3 check of the share partition.
//! Each case boots its own copy of the x86-64 disk image, with the writes of
//! the guest kept in the copy, one boot after the other.
//!
//! `rewrite`, a stick that is written again:
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
//! `offer`, an empty partition 4 with no marker, which the script may not
//! format by its own rules (the host writes its entry, type 7 at the
//! planned start):
//!
//! 1. `offered`: `/share` is a RAM folder and `storage.toml` offers the
//!    format. The guest writes the request file as the frontend does.
//! 2. `formatted`: the script formats, mounts and removes the request; the
//!    guest writes a file to the share.
//! 3. `kept`: a normal boot mounts with no format. The guest plants a
//!    request for the partition, which now holds a filesystem.
//! 4. `refused`: the script removes the request, formats nothing and says
//!    why; the file is there.
//!
//! `retype`, the same with a partition 4 of type 0x83:
//!
//! 1. `retype-offered`: the offer names type 83; the guest writes the
//!    request.
//! 2. `retyped`: the script sets type 7 with sfdisk, formats and mounts.
//! 3. `retype-again`: the disk still boots after sfdisk wrote its MBR, and
//!    the share mounts with no format.
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
    /// The cases to run, in order.
    pub cases: Vec<Case>,
}

impl ShareCheckArgs {
    /// Parses `[--arch x86_64] [--timeout SECS] [--images DIR] [--case NAME]...`.
    pub fn parse(args: &[&str]) -> Result<Self, OsError> {
        let mut out = Self {
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            images: None,
            cases: Vec::new(),
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
                "--case" => {
                    let case = CASES
                        .into_iter()
                        .find(|c| c.name() == value)
                        .ok_or_else(|| {
                            OsError::Usage(format!(
                                "unknown case {value:?}; the cases are rewrite, offer and retype"
                            ))
                        })?;
                    out.cases.push(case);
                }
                other => return Err(OsError::Usage(format!("unknown argument {other:?}"))),
            }
        }
        if out.cases.is_empty() {
            out.cases = CASES.to_vec();
        }
        Ok(out)
    }
}

/// A disk copy and the boots on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// A stick that is written again over its share.
    Rewrite,
    /// An empty partition 4 with no marker, formatted on the request.
    Offer,
    /// An empty partition 4 of type 0x83, given type 7 and formatted on the
    /// request.
    Retype,
}

/// Every case, in the order of a full run.
pub const CASES: [Case; 3] = [Case::Rewrite, Case::Offer, Case::Retype];

impl Case {
    /// The name in the output, on the command line and in the folder name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Rewrite => "rewrite",
            Self::Offer => "offer",
            Self::Retype => "retype",
        }
    }

    /// The boots of the case, in order.
    pub fn steps(self) -> &'static [Step] {
        match self {
            Self::Rewrite => &[Step::Fresh, Step::Rewritten, Step::Again],
            Self::Offer => &[Step::Offered, Step::Formatted, Step::Kept, Step::Refused],
            Self::Retype => &[Step::RetypeOffered, Step::Retyped, Step::RetypeAgain],
        }
    }

    /// The MBR type of the empty partition 4 that the host writes before
    /// the first boot, `None` for none.
    pub fn partition_4_type(self) -> Option<u8> {
        match self {
            Self::Rewrite => None,
            Self::Offer => Some(0x07),
            Self::Retype => Some(0x83),
        }
    }
}

/// One boot of a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The first boot on empty free space.
    Fresh,
    /// The first boot after the image is written again over the copy.
    Rewritten,
    /// A normal boot after that.
    Again,
    /// The offer of a format of the empty partition 4 of type 7.
    Offered,
    /// The boot after the request: formatted and mounted.
    Formatted,
    /// A normal boot; the guest plants a request.
    Kept,
    /// The request for a partition with a filesystem formats nothing.
    Refused,
    /// The offer of a format of the empty partition 4 of type 0x83.
    RetypeOffered,
    /// The boot after that request: type 7, formatted and mounted.
    Retyped,
    /// A normal boot after the type change.
    RetypeAgain,
}

impl Step {
    /// The name in the output and in the file names.
    pub fn name(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Rewritten => "rewritten",
            Self::Again => "again",
            Self::Offered => "offered",
            Self::Formatted => "formatted",
            Self::Kept => "kept",
            Self::Refused => "refused",
            Self::RetypeOffered => "retype-offered",
            Self::Retyped => "retyped",
            Self::RetypeAgain => "retype-again",
        }
    }

    /// True when `/share` must be partition 4 after the boot.
    fn mounts(self) -> bool {
        !matches!(self, Self::Offered | Self::RetypeOffered)
    }
}

/// The guest script of `step`. `nonce` is the text of the file that a boot
/// writes to the share and the later boots read.
pub fn guest_script(step: Step, nonce: &str) -> String {
    let mut s = String::from(PRELUDE);
    if step.mounts() {
        s.push_str(MOUNTED);
    }
    let file = format!("/share/{FILE}");
    let write = format!("echo \"{nonce}\" > {file} && sync || fail \"{file} cannot be written\"\n");
    let read_back = format!(
        "[ \"$(cat {file})\" = \"{nonce}\" ] || fail \"{file} of the first boot is not there\"\n"
    );
    match step {
        Step::Fresh => {
            s.push_str(
                "logged '/dev/sda4 holds no filesystem; formatting it exFAT' || fail \"the empty space was not formatted\"\n",
            );
            s.push_str(FLUSHED);
            s.push_str(&write);
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
        Step::Again | Step::RetypeAgain => {
            s.push_str(
                "logged 'making partition 4' && fail \"partition 4 was made again\"\n\
                 logged 'formatting it exFAT' && fail \"the share was formatted\"\n",
            );
            s.push_str(&read_back);
        }
        Step::Offered | Step::RetypeOffered => {
            let kind = if step == Step::Offered { "7" } else { "83" };
            s.push_str(OFFERED);
            s.push_str(&format!(
                "status 'share_fix_type = \"{kind}\"' || fail \"the offer does not name type {kind}\"\n"
            ));
            s.push_str(WRITE_REQUEST);
        }
        Step::Formatted | Step::Retyped => {
            s.push_str(FORMATTED);
            if step == Step::Retyped {
                s.push_str(
                    "logged 'sfdisk --part-type /dev/sda 4 7 exit' || fail \"the type was not set\"\n\
                     [ \"$(sfdisk --part-type /dev/sda 4 | tr -d ' ')\" = 7 ] || fail \"partition 4 is not type 7 in the table\"\n",
                );
            }
            s.push_str(&write);
        }
        Step::Kept => {
            s.push_str(
                "logged 'formatting it exFAT' && fail \"the share was formatted\"\n\
                 logged 'is formatted exFAT on the request' && fail \"the share was formatted\"\n\
                 grep -q '^share_format' /run/neptron/storage.toml && fail \"a request result with no request\"\n",
            );
            s.push_str(&read_back);
            s.push_str(PLANT_REQUEST);
        }
        Step::Refused => {
            s.push_str(
                "logged 'is formatted exFAT on the request' && fail \"the share was formatted\"\n\
                 logged 'formatting it exFAT' && fail \"the share was formatted\"\n\
                 status 'share_format = \"refused\"' || fail \"the request was not refused\"\n\
                 grep -q '^share_format_reason = \".*it holds a filesystem or a partition table\"$' /run/neptron/storage.toml || fail \"the refusal does not name the filesystem\"\n\
                 [ ! -e /state/share-format-request ] || fail \"the request file stays\"\n",
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

/// Writes an MBR entry 4 of `kind` into `disk`: from the first 1 MiB
/// boundary after partition 3, where the storage script plans the share,
/// to the end of the disk. The space stays as it is (zeros in a fresh
/// copy), so partition 4 is empty and no marker names it. Returns the start
/// and the size, in sectors.
pub fn add_partition_4(disk: &Path, kind: u8) -> Result<(u64, u64), OsError> {
    let io = |source| OsError::Io {
        path: disk.to_owned(),
        source,
    };
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .read(true)
        .open(disk)
        .map_err(io)?;
    let mut mbr = [0_u8; 512];
    file.read_exact(&mut mbr).map_err(io)?;
    let entry_3 = &mbr[MBR_ENTRY_3_OFFSET as usize..MBR_ENTRY_3_OFFSET as usize + 16];
    let word =
        |bytes: &[u8]| u64::from(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
    let state_end = word(&entry_3[8..12]) + word(&entry_3[12..16]);
    let disk_sectors = file.metadata().map_err(io)?.len() / SECTOR_BYTES;
    let start = state_end.div_ceil(ALIGN_SECTORS) * ALIGN_SECTORS;
    if state_end == 0 || start >= disk_sectors {
        return Err(OsError::Usage(format!(
            "{} has no partition 3 with room after it",
            disk.display()
        )));
    }
    let size = disk_sectors - start;
    let too_large = |_| OsError::Usage(format!("{} is too large for an MBR entry", disk.display()));
    let mut entry = [0_u8; 16];
    // CHS fields that say "use the LBA fields", as libfdisk writes them
    // for a partition past the CHS range.
    entry[1..4].copy_from_slice(&[0xfe, 0xff, 0xff]);
    entry[4] = kind;
    entry[5..8].copy_from_slice(&[0xfe, 0xff, 0xff]);
    entry[8..12].copy_from_slice(&u32::try_from(start).map_err(too_large)?.to_le_bytes());
    entry[12..16].copy_from_slice(&u32::try_from(size).map_err(too_large)?.to_le_bytes());
    file.seek(SeekFrom::Start(MBR_ENTRY_4_OFFSET)).map_err(io)?;
    file.write_all(&entry).map_err(io)?;
    file.flush().map_err(io)?;
    Ok((start, size))
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
    let nonce = transcript::new_nonce();
    let total: usize = args.cases.iter().map(|c| c.steps().len()).sum();
    let mut passed = 0;
    'cases: for case in &args.cases {
        let out_dir = repo
            .root
            .join("target")
            .join("os")
            .join("share-check")
            .join(case.name());
        let disk = disk_with_share_room(&image, &out_dir)?;
        if let Some(kind) = case.partition_4_type() {
            let (start, size) = add_partition_4(&disk, kind)?;
            println!(
                "share-check {}: partition 4 of type {kind:x} at sector {start} with {size} sectors, empty, no marker",
                case.name()
            );
        }
        for &step in case.steps() {
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
                    "share-check {} {}: the guest script exited with {code:?}",
                    case.name(),
                    step.name()
                );
                break 'cases;
            }
            passed += 1;
        }
    }
    let ok = passed == total;
    println!(
        "share-check x86_64: {passed} of {total} boots pass: {}",
        if ok { "PASS" } else { "FAIL" }
    );
    Ok(super::exit_code(ok))
}

/// The guest deadline of each boot when `--timeout` is not given: a boot
/// under CPU emulation takes about a minute.
const DEFAULT_TIMEOUT_SECS: u64 = 300;
/// The helpers of every guest script.
const PRELUDE: &str = r#"fail() {
	echo "share-check: FAIL STEP: $1"
	echo "share-check: /run/neptron/storage.toml:"; cat /run/neptron/storage.toml
	echo "share-check: storage lines in /var/log/messages:"; grep neptron-storage /var/log/messages
	exit 1
}
logged() {
	grep -q "neptron-storage: $1" /var/log/messages
}
status() {
	grep -qx "$1" /run/neptron/storage.toml
}
# The lines of this boot in the storage log: those after its last
# "== boot" line, so a line of an earlier boot never passes a check.
this_boot() {
	awk '/^== boot / { n = NR } { line[NR] = $0 } END { if (n) for (i = n + 1; i <= NR; i++) print line[i] }' /state/log/storage.log
}
"#;
/// The checks of a boot that mounts partition 4 at `/share`.
const MOUNTED: &str = r#"status 'share = "partition"' || fail "/share is not the share partition"
grep -q '^/dev/sda4 /share exfat ' /proc/mounts || fail "/share is not partition 4 as exFAT"
logged '/dev/sda4 mounted at /share' || fail "/var/log/messages has no line of the storage script"
this_boot | grep -q '/dev/sda4 mounted at /share' || fail "/state/log/storage.log has no line of this boot"
"#;
/// The checks of a boot that offers the format of an empty partition 4.
const OFFERED: &str = r#"status 'share = "ram"' || fail "/share is not a RAM folder"
grep -q '^share /share tmpfs ' /proc/mounts || fail "/share is not a RAM folder in /proc/mounts"
grep -q '^/dev/sda4 ' /proc/mounts && fail "partition 4 is mounted"
logged 'formatting it exFAT' && fail "partition 4 was formatted with no request"
logged 'is empty and has no /state/share-pending, so it is left untouched' || fail "the reason is not that of an empty partition 4 with no marker"
status 'share_fix = "format"' || fail "no format is offered"
status 'share_fix_device = "/dev/sda4"' || fail "the offer does not name /dev/sda4"
status "share_fix_start = $(cat /sys/class/block/sda4/start)" || fail "the offer does not name the start of partition 4"
status "share_fix_size = $(cat /sys/class/block/sda4/size)" || fail "the offer does not name the size of partition 4"
this_boot | grep -q 'is offered in Settings > System' || fail "/state/log/storage.log has no line of the offer"
"#;
/// Writes the request file from the offer, as the frontend writes it: a
/// temporary file, a sync, a rename.
const WRITE_REQUEST: &str = r#"value() {
	sed -n "s/^$1 = \"\{0,1\}\([^\"]*\)\"\{0,1\}$/\1/p" /run/neptron/storage.toml
}
printf 'device=%s\nstart=%s\nsize=%s\ntype=%s\n' "$(value share_fix_device)" "$(value share_fix_start)" "$(value share_fix_size)" "$(value share_fix_type)" > /state/share-format-request.tmp &&
	sync && mv /state/share-format-request.tmp /state/share-format-request && sync || fail "the request cannot be written"
cat /state/share-format-request
"#;
/// The checks of a boot that formatted partition 4 on the request.
const FORMATTED: &str = r#"logged 'format request of the user for /dev/sda4' || fail "the request was not read"
logged '/dev/sda4 is formatted exFAT on the request of the user' || fail "partition 4 was not formatted on the request"
status 'share_format = "done"' || fail "storage.toml does not say that the format is done"
grep -q '^share_fix' /run/neptron/storage.toml && fail "a format is still offered"
[ ! -e /state/share-format-request ] || fail "the request file stays"
"#;
/// Plants a request for partition 4, which holds a filesystem now.
const PLANT_REQUEST: &str = r#"printf 'device=/dev/sda4\nstart=%s\nsize=%s\ntype=7\n' "$(cat /sys/class/block/sda4/start)" "$(cat /sys/class/block/sda4/size)" > /state/share-format-request && sync || fail "the request cannot be planted"
"#;
/// The check of a boot that made partition 4: the table was read back
/// from the device.
const FLUSHED: &str = "logged 'blockdev --flushbufs exit 0: the table is read back after a flush of the cache' || fail \"the table was not read back after a flush of the cache\"\n";
/// The file that a boot writes to the share.
const FILE: &str = "share-check.txt";
/// The byte offset of the third primary entry of an MBR.
const MBR_ENTRY_3_OFFSET: u64 = 446 + 2 * 16;
/// The byte offset of the fourth primary entry of an MBR.
const MBR_ENTRY_4_OFFSET: u64 = 446 + 3 * 16;
/// The sector of the MBR numbers, in bytes.
const SECTOR_BYTES: u64 = 512;
/// The storage script starts partition 4 on the next 1 MiB boundary, in
/// sectors.
const ALIGN_SECTORS: u64 = 2048;

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
        assert_eq!(a.cases, CASES);
        let a = ShareCheckArgs::parse(&["--case", "offer", "--case", "retype"]).unwrap();
        assert_eq!(a.cases, [Case::Offer, Case::Retype]);
        let a = ShareCheckArgs::parse(&["--arch", "x86_64", "--timeout", "60", "--images", "/i"])
            .unwrap();
        assert_eq!(a.timeout_secs, 60);
        assert_eq!(a.images, Some(PathBuf::from("/i")));
        for bad in [
            &["--arch", "aarch64"][..],
            &["--timeout", "0"],
            &["--images"],
            &["--bless", "x"],
            &["--case", "other"],
        ] {
            assert!(ShareCheckArgs::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn every_boot_checks_the_mount_and_the_syslog_lines() {
        let steps = CASES.iter().flat_map(|c| c.steps()).filter(|s| s.mounts());
        for &step in steps {
            let s = guest_script(step, "n0nce");
            for part in [
                "status 'share = \"partition\"'",
                "grep -q '^/dev/sda4 /share exfat ' /proc/mounts",
                "grep -q \"neptron-storage: $1\" /var/log/messages",
                "logged '/dev/sda4 mounted at /share'",
                "this_boot | grep -q '/dev/sda4 mounted at /share'",
                "awk '/^== boot / { n = NR }",
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
    fn the_offer_boots_write_the_request_and_never_format() {
        for (step, kind) in [(Step::Offered, "7"), (Step::RetypeOffered, "83")] {
            let s = guest_script(step, "n0nce");
            for part in [
                "status 'share = \"ram\"'",
                "logged 'formatting it exFAT' && fail",
                "status 'share_fix = \"format\"'",
                &format!("status 'share_fix_type = \"{kind}\"'"),
                "mv /state/share-format-request.tmp /state/share-format-request && sync",
            ] {
                assert!(s.contains(part), "{}: {part}: {s}", step.name());
            }
            assert!(!s.contains(MOUNTED), "{s}");
        }
    }

    #[test]
    fn the_request_boots_format_once_and_then_refuse() {
        let formatted = guest_script(Step::Formatted, "n0nce");
        assert!(formatted.contains(FORMATTED), "{formatted}");
        assert!(formatted.contains("echo \"n0nce\" > /share/share-check.txt"));
        let retyped = guest_script(Step::Retyped, "n0nce");
        assert!(retyped.contains("sfdisk --part-type /dev/sda 4 | tr -d ' ')\" = 7 ]"));
        let kept = guest_script(Step::Kept, "n0nce");
        assert!(kept.contains(PLANT_REQUEST), "{kept}");
        assert!(kept.contains("= \"n0nce\" ]"), "{kept}");
        let refused = guest_script(Step::Refused, "n0nce");
        for part in [
            "logged 'is formatted exFAT on the request' && fail",
            "status 'share_format = \"refused\"'",
            "it holds a filesystem or a partition table",
            "[ ! -e /state/share-format-request ]",
            "= \"n0nce\" ]",
        ] {
            assert!(refused.contains(part), "{part}: {refused}");
        }
    }

    #[test]
    fn partition_4_is_written_at_the_first_1_mib_boundary_after_partition_3() {
        let dir = std::env::temp_dir().join(format!("xtask-share-p4-{}", std::process::id()));
        // A leftover of an earlier run with the same process id; absence is fine.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let disk = dir.join("disk.img");
        let mut bytes = vec![0_u8; 20_000 * 512];
        let entry_3 = 446 + 32;
        bytes[entry_3 + 4] = 0x83;
        bytes[entry_3 + 8..entry_3 + 12].copy_from_slice(&4096_u32.to_le_bytes());
        bytes[entry_3 + 12..entry_3 + 16].copy_from_slice(&6000_u32.to_le_bytes());
        std::fs::write(&disk, &bytes).unwrap();
        assert_eq!(add_partition_4(&disk, 0x83).unwrap(), (10_240, 9_760));
        let written = std::fs::read(&disk).unwrap();
        let entry_4 = &written[446 + 48..446 + 64];
        assert_eq!(entry_4[4], 0x83);
        assert_eq!(&entry_4[8..12], &10_240_u32.to_le_bytes());
        assert_eq!(&entry_4[12..16], &9_760_u32.to_le_bytes());
        assert_eq!(&written[..446 + 48], &bytes[..446 + 48], "the rest stays");

        std::fs::write(&disk, vec![0_u8; 4096]).unwrap();
        let err = add_partition_4(&disk, 7).unwrap_err();
        assert!(err.to_string().contains("no partition 3"), "{err}");
        std::fs::remove_dir_all(&dir).unwrap();
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

#[cfg(test)]
mod this_boot {
    use super::*;
    use std::process::Command;

    /// The storage log check reads only the lines after the last boot line.
    #[test]
    fn only_the_lines_of_the_last_boot_count() {
        let script = guest_script(Step::Again, "n0nce");
        let function: String = script
            .split("this_boot() {")
            .nth(1)
            .and_then(|rest| rest.split("\n}").next())
            .expect("the this_boot function")
            .replace("/state/log/storage.log", "\"$1\"");
        let dir = std::env::temp_dir().join(format!("xtask-this-boot-{}", std::process::id()));
        // A leftover of an earlier run with the same process id; absence is fine.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("storage.log");
        let run = |text: &str| {
            std::fs::write(&log, text).unwrap();
            let out = Command::new("sh")
                .arg("-c")
                .arg(format!("f() {{{function}\n}}; f \"$0\""))
                .arg(&log)
                .output()
                .unwrap();
            assert!(out.status.success(), "{out:?}");
            String::from_utf8(out.stdout).unwrap()
        };
        assert_eq!(
            run("== boot 1\n/dev/sda4 mounted at /share\n== boot 2\nno share\n"),
            "no share\n"
        );
        assert_eq!(run("== boot 1\na\nb\n"), "a\nb\n");
        assert_eq!(run("no boot line\n"), "");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
