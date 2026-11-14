//! Tests of `S06neptron-storage`, the init script that mounts the state and
//! the share partition. The script runs on the host with every path below
//! a test folder, which the tests give as its second argument: a fake
//! sysfs, a fake disk file and fake commands for every program that touches
//! a device or the kernel. The fakes write their calls to a log; the tests
//! read the log, the status file `run/neptron/storage.toml` and the storage
//! log `state/log/storage.log`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SCRIPT: &str = "board/common/rootfs-overlay/etc/init.d/S06neptron-storage";

fn script_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../os")
        .join(SCRIPT)
}

fn script() -> String {
    std::fs::read_to_string(script_path()).unwrap()
}

/// The fake disk: partition 3 (state) ends at sector 10240, so the planned
/// start of partition 4 is 10240: on a 1 MiB boundary, not on a 4 MiB one.
/// 65 MiB of free space follow, just over the 64 MiB limit, so the scan for
/// an old share is short.
const STATE_START: u64 = 4096;
const STATE_SECTORS: u64 = 6144;
const PLANNED: u64 = 10_240;
const FREE_SECTORS: u64 = 133_120;
const DISK_SECTORS: u64 = PLANNED + FREE_SECTORS;
/// The start that the fake sfdisk takes when its input gives none: the
/// first free sector rounded up to 4 MiB, as libfdisk does on a device
/// with an optimal I/O size of 4 MiB.
const GRAIN_4_MIB_START: u64 = 16_384;

/// The fake commands. Each reads its behaviour from `FAKE_*` variables;
/// `FAKE_ROOT` is the test folder.
const FAKES: &[(&str, &str)] = &[
    ("mountpoint", "echo 8:2"),
    (
        "e2fsck",
        "echo \"e2fsck $*\" >> \"$LOG\"; exit \"${FAKE_E2FSCK_EXIT:-0}\"",
    ),
    (
        "mount",
        "echo \"mount $*\" >> \"$LOG\"\n\
         case \"$*\" in\n\
         *'-t exfat'*) exit \"${FAKE_MOUNT_EXFAT_EXIT:-0}\" ;;\n\
         *'-t ext4'*) exit \"${FAKE_MOUNT_EXT4_EXIT:-0}\" ;;\n\
         esac",
    ),
    (
        "logger",
        "echo \"logger $*\" >> \"$LOG\"; exit \"${FAKE_LOGGER_EXIT:-0}\"",
    ),
    ("sync", "echo sync >> \"$LOG\""),
    (
        "dd",
        "if [ -n \"$FAKE_DD_LOG\" ]; then echo \"dd $*\" >> \"$LOG\"; fi\n\
         exec /bin/dd \"$@\"",
    ),
    ("chown", "echo \"chown $*\" >> \"$LOG\""),
    ("date", "echo DATE"),
    (
        "mkfs.exfat",
        "echo \"mkfs.exfat $*\" >> \"$LOG\"; exit \"${FAKE_MKFS_EXIT:-0}\"",
    ),
    (
        "blockdev",
        "echo \"blockdev $*\" >> \"$LOG\"; exit \"${FAKE_FLUSH_EXIT:-0}\"",
    ),
    (
        "blkid",
        "if [ -n \"$FAKE_BLKID_ERROR\" ]; then echo \"$FAKE_BLKID_ERROR\" >&2; exit 4; fi\n\
         if [ -n \"$FAKE_BLKID\" ]; then echo \"$FAKE_BLKID\"; exit 0; fi\n\
         exit 2",
    ),
    (
        "usleep",
        "echo \"usleep $*\" >> \"$LOG\"\n\
         n=$(($(cat \"$FAKE_ROOT/usleeps\" 2>/dev/null || echo 0) + 1))\n\
         echo \"$n\" > \"$FAKE_ROOT/usleeps\"\n\
         if [ -n \"$FAKE_NODE_AT\" ] && [ \"$n\" -ge \"$FAKE_NODE_AT\" ]; then\n\
         \t: > \"$FAKE_ROOT/dev/sda4\"\n\
         fi",
    ),
    (
        "sfdisk",
        "R=$FAKE_ROOT\n\
         case \" $* \" in\n\
         *' --dump '*)\n\
         \tn=$(cat \"$R/dump-fails\" 2>/dev/null || echo 0)\n\
         \t[ \"$FAKE_DUMP_ALWAYS_FAILS\" = yes ] && n=1\n\
         \tif [ \"$n\" -gt 0 ]; then\n\
         \t\techo $((n - 1)) > \"$R/dump-fails\"\n\
         \t\techo 'dump fails' >> \"$LOG\"\n\
         \t\techo 'sfdisk: cannot open /dev/sda: Input/output error' >&2; exit 1\n\
         \tfi\n\
         \techo dump >> \"$LOG\"; cat \"$R/table\"; exit 0 ;;\n\
         *' --part-type '*' 4 7 '*)\n\
         \techo \"sfdisk-set-type $*\" >> \"$LOG\"\n\
         \tif [ \"${FAKE_RETYPE_WRITES:-yes}\" = yes ]; then\n\
         \t\tsed -i.bak 's/^\\(.*sda4 : .*type=\\)[0-9a-fA-F]*$/\\17/' \"$R/table\"\n\
         \tfi\n\
         \tif [ -n \"$FAKE_RETYPE_ALSO\" ]; then sed -i.bak \"$FAKE_RETYPE_ALSO\" \"$R/table\"; fi\n\
         \tif [ -n \"$FAKE_RETYPE_BOOT_CODE\" ]; then printf X | /bin/dd of=\"$R/dev/sda\" bs=1 seek=100 conv=notrunc 2>/dev/null; fi\n\
         \texit \"${FAKE_RETYPE_EXIT:-0}\" ;;\n\
         *' --part-type '*) grep -q '4 : .*type=7' \"$R/table\" && echo ' 7'; exit 0 ;;\n\
         esac\n\
         read -r line\n\
         echo \"sfdisk $* <<< $line\" >> \"$LOG\"\n\
         start=${line%%,*}\n\
         [ -n \"$start\" ] || start=$GRAIN_START\n\
         start=${FAKE_P4_START:-$start}\n\
         if [ \"${FAKE_SFDISK_WRITES:-yes}\" = yes ]; then\n\
         \tprintf '%s/dev/sda4 : start=%12s, size=%12s, type=%s\\n' \"$R\" \
         \"$start\" $((DISK_SECTORS - start)) \"${FAKE_P4_TYPE:-${line##*,}}\" >> \"$R/table\"\n\
         \tif [ -n \"$FAKE_DUMP_FAILS_AFTER_WRITE\" ]; then echo \"$FAKE_DUMP_FAILS_AFTER_WRITE\" > \"$R/dump-fails\"; fi\n\
         fi\n\
         if [ -n \"$FAKE_SFDISK_SAYS\" ]; then echo \"$FAKE_SFDISK_SAYS\" >&2; fi\n\
         exit \"${FAKE_SFDISK_EXIT:-0}\"",
    ),
    (
        "addpart",
        "R=$FAKE_ROOT\n\
         echo \"addpart $*\" >> \"$LOG\"\n\
         n=$(($(cat \"$R/addpart-calls\" 2>/dev/null || echo 0) + 1))\n\
         echo \"$n\" > \"$R/addpart-calls\"\n\
         if [ \"$n\" -le \"${FAKE_ADDPART_FAILS:-0}\" ]; then\n\
         \techo 'addpart: failed to add partition: Device or resource busy' >&2; exit 1\n\
         fi\n\
         if [ \"${FAKE_ADDPART_ADDS:-yes}\" != yes ]; then exit 0; fi\n\
         add_partition \"$2\" \"$3\" \"$4\"\n\
         if [ -z \"$FAKE_NODE_AT\" ]; then : > \"$R/dev/sda$2\"; fi",
    ),
];

/// Adds partition `$1` (start `$2`, size `$3`) to the fake sysfs, as the
/// kernel does. The fake addpart and the test setup share it.
const ADD_PARTITION: &str = "#!/bin/sh\n\
    R=$FAKE_ROOT\n\
    dir=$R/sys/devices/blk/sda/sda$1\n\
    mkdir -p \"$dir\" \"$R/sys/class/block\"\n\
    echo \"$1\" > \"$dir/partition\"\n\
    echo \"$2\" > \"$dir/start\"\n\
    echo \"$3\" > \"$dir/size\"\n\
    ln -sf \"$dir\" \"$R/sys/class/block/sda$1\"\n";

/// One run of the script in its own folder.
struct Run {
    dir: PathBuf,
    env: Vec<(String, String)>,
}

impl Run {
    /// A disk with partitions 1 to 3 in the table and in the kernel, the
    /// state partition last, and free space after it.
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("xtask-storage-{name}-{}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).unwrap();
        }
        let run = Run {
            dir,
            env: Vec::new(),
        };
        let bin = run.path("bin");
        std::fs::create_dir_all(&bin).unwrap();
        for (name, body) in FAKES {
            write_exec(&bin.join(name), &format!("#!/bin/sh\n{body}\n"));
        }
        write_exec(&bin.join("add_partition"), ADD_PARTITION);
        for dir in ["dev", "sys/dev/block", "usr/share/neptron/systems"] {
            std::fs::create_dir_all(run.path(dir)).unwrap();
        }
        std::fs::write(run.path("usr/share/neptron/systems/nes.toml"), "").unwrap();
        std::fs::create_dir_all(run.path("sys/devices/blk/sda")).unwrap();
        std::fs::write(
            run.path("sys/devices/blk/sda/size"),
            format!("{DISK_SECTORS}\n"),
        )
        .unwrap();
        let disk = std::fs::File::create(run.path("dev/sda")).unwrap();
        disk.set_len(DISK_SECTORS * 512).unwrap();
        run.add_partition(1, 2048, 1024);
        run.add_partition(2, 3072, 1024);
        run.add_partition(3, STATE_START, STATE_SECTORS);
        std::os::unix::fs::symlink(
            run.path("sys/devices/blk/sda/sda2"),
            run.path("sys/dev/block/8:2"),
        )
        .unwrap();
        let root = run.dir.display().to_string();
        std::fs::write(
            run.path("table"),
            format!(
                "label: dos\n\n\
                 {root}/dev/sda1 : start=        2048, size=        1024, type=c, bootable\n\
                 {root}/dev/sda2 : start=        3072, size=        1024, type=83\n\
                 {root}/dev/sda3 : start=        4096, size=        6144, type=83\n"
            ),
        )
        .unwrap();
        run
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.dir.join(rel)
    }

    fn set(mut self, name: &str, value: &str) -> Self {
        self.env.push((name.to_owned(), value.to_owned()));
        self
    }

    /// Adds partition `number` to the fake kernel, with its device node.
    fn add_partition(&self, number: u32, start: u64, sectors: u64) {
        let status = Command::new(self.path("bin/add_partition"))
            .args([number.to_string(), start.to_string(), sectors.to_string()])
            .env("FAKE_ROOT", &self.dir)
            .status()
            .unwrap();
        assert!(status.success());
        std::fs::write(self.path(&format!("dev/sda{number}")), "").unwrap();
    }

    /// Writes partition 4 to the fake table on the disk.
    fn table_partition_4(&self, start: u64, sectors: u64, kind: &str) {
        let mut table = std::fs::read_to_string(self.path("table")).unwrap();
        table.push_str(&format!(
            "{}/dev/sda4 : start={start:>12}, size={sectors:>12}, type={kind}\n",
            self.dir.display()
        ));
        std::fs::write(self.path("table"), table).unwrap();
    }

    /// Partition 4 at the planned start, in the table and in the kernel.
    fn with_partition_4(self) -> Self {
        self.table_partition_4(PLANNED, FREE_SECTORS, "7");
        self.add_partition(4, PLANNED, FREE_SECTORS);
        self
    }

    /// Puts an exFAT boot sector at `sector` of the fake disk.
    fn exfat_at(&self, sector: u64) {
        use std::io::{Seek, SeekFrom, Write};
        let mut disk = std::fs::OpenOptions::new()
            .write(true)
            .open(self.path("dev/sda"))
            .unwrap();
        disk.seek(SeekFrom::Start(sector * 512 + 3)).unwrap();
        disk.write_all(b"EXFAT   ").unwrap();
    }

    /// Runs the script with `start` and the test folder, and removes the
    /// folder.
    fn start(self) -> Outcome {
        self.boot(&[])
    }

    /// Writes the marker as the script writes it for partition 4 at the
    /// planned start over all the free space.
    fn write_marker(&self) {
        std::fs::create_dir_all(self.path("state")).unwrap();
        std::fs::write(
            self.path("state/share-pending"),
            format!("partition 4 made at DATE\nstart={PLANNED}\nsize={FREE_SECTORS}\n"),
        )
        .unwrap();
    }

    /// One boot: runs the script with `start`, the test folder, and the
    /// variables of the run and `extra`, and returns what it did. The
    /// folder stays, so a second boot sees the disk and the state partition
    /// of the first; the call logs start empty.
    fn boot(&self, extra: &[(&str, &str)]) -> Outcome {
        for file in ["log", "usleeps", "addpart-calls", "dump-fails"] {
            // A file that is not there is the state wanted.
            let _ = std::fs::remove_file(self.path(file));
        }
        let path = format!(
            "{}:{}",
            self.path("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let out = Command::new("sh")
            .arg(script_path())
            .arg("start")
            .arg(&self.dir)
            .env("PATH", path)
            .env("LOG", self.path("log"))
            .env("FAKE_ROOT", &self.dir)
            .env("GRAIN_START", GRAIN_4_MIB_START.to_string())
            .env("DISK_SECTORS", DISK_SECTORS.to_string())
            .envs(self.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .envs(extra.iter().copied())
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let read = |rel: &str| std::fs::read_to_string(self.path(rel)).unwrap_or_default();
        Outcome {
            ok: out.status.success(),
            console: String::from_utf8_lossy(&out.stdout).into_owned()
                + &String::from_utf8_lossy(&out.stderr),
            log: read("log"),
            status: read("run/neptron/storage.toml"),
            state_log: read("state/log/storage.log"),
            marker: self.path("state/share-pending").exists(),
            marker_text: read("state/share-pending"),
            request: self.path("state/share-format-request").exists(),
        }
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        // A folder left in the system temp folder does no harm.
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// What a run of the script did.
#[derive(Debug)]
struct Outcome {
    ok: bool,
    console: String,
    log: String,
    status: String,
    state_log: String,
    marker: bool,
    marker_text: String,
    request: bool,
}

impl Outcome {
    fn calls(&self, prefix: &str) -> usize {
        self.log.lines().filter(|l| l.starts_with(prefix)).count()
    }

    /// The position of the first log line that starts with `prefix`.
    fn first(&self, prefix: &str) -> usize {
        self.log
            .lines()
            .position(|l| l.starts_with(prefix))
            .unwrap_or_else(|| panic!("no {prefix}: {self:#?}"))
    }

    /// The share partition is mounted, formatted or not.
    fn assert_share_mounted(&self, formatted: bool) {
        assert!(self.ok, "{self:#?}");
        assert_eq!(
            self.status, "state = \"partition\"\nshare = \"partition\"\n",
            "{self:#?}"
        );
        assert_eq!(self.calls("mount -t exfat"), 1, "{self:#?}");
        assert_eq!(
            self.calls("mkfs.exfat"),
            usize::from(formatted),
            "{self:#?}"
        );
        assert!(!self.marker, "the marker goes after the mount: {self:#?}");
        assert!(
            self.log.contains("logger -t neptron-storage ")
                && self.log.contains("sda4 mounted at /share"),
            "{self:#?}"
        );
        assert!(
            self.state_log.contains("sda4 mounted at /share"),
            "{self:#?}"
        );
    }

    /// `/share` is a RAM folder with `reason` in the status file, and the
    /// reason is in syslog, on the console, and in the storage log when
    /// `/state` is on the disk.
    fn assert_share_ram(&self, reason: &str) -> String {
        self.assert_share_ram_with(reason, 0)
    }

    /// As [`Self::assert_share_ram`], with `mkfs` calls of mkfs.exfat.
    fn assert_share_ram_with(&self, reason: &str, mkfs: usize) -> String {
        assert!(self.ok, "{self:#?}");
        let share_reason = self
            .status
            .lines()
            .find_map(|l| l.strip_prefix("share_reason = \""))
            .and_then(|l| l.strip_suffix('"'))
            .unwrap_or_else(|| panic!("no share reason: {self:#?}"))
            .to_owned();
        assert!(share_reason.contains(reason), "{reason}: {self:#?}");
        assert!(self.status.contains("share = \"ram\"\n"), "{self:#?}");
        assert!(
            self.log
                .lines()
                .any(|l| l == format!("logger -t neptron-storage {share_reason}")),
            "syslog misses the reason: {self:#?}"
        );
        assert!(
            self.console
                .contains(&format!("neptron-storage: {share_reason}")),
            "{self:#?}"
        );
        let state_ram = self.status.contains("state = \"ram\"");
        if !state_ram {
            assert!(
                self.state_log.lines().any(|l| l == share_reason),
                "the storage log misses the reason: {self:#?}"
            );
        }
        assert_eq!(self.calls("mount -t exfat"), 0, "{self:#?}");
        assert_eq!(self.calls("mkfs.exfat"), mkfs, "{self:#?}");
        assert_eq!(
            self.calls("mount -t tmpfs"),
            1 + usize::from(state_ram),
            "{self:#?}"
        );
        share_reason
    }
}

fn write_exec(path: &Path, text: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn the_script_parses_in_every_posix_shell_of_the_host() {
    let mut checked = Vec::new();
    for shell in ["sh", "dash", "busybox"] {
        let mut cmd = Command::new(shell);
        if shell == "busybox" {
            cmd.arg("sh");
        }
        match cmd.arg("-n").arg(script_path()).output() {
            Ok(out) => {
                assert!(out.status.success(), "{shell}: {out:?}");
                checked.push(shell);
            }
            Err(_) => eprintln!("{shell} -n skipped: {shell} is not on this host"),
        }
    }
    assert!(checked.contains(&"sh"), "{checked:?}");
}

#[test]
fn other_actions_do_nothing() {
    let run = Run::new("stop");
    let out = Command::new("sh")
        .arg(script_path())
        .arg("stop")
        .arg(&run.dir)
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", run.path("bin").display()),
        )
        .env("LOG", run.path("log"))
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(!run.path("log").exists());
    // A folder left in the system temp folder does no harm.
    let _ = std::fs::remove_dir_all(&run.dir);
}

/// Init passes the kernel command line to its children as variables, so
/// no variable may move the paths of the script; only its second argument,
/// which init never gives, does.
#[test]
fn only_the_second_argument_moves_the_paths() {
    let text = script();
    assert!(text.contains("\nR=${2:-}\n"), "{text}");
    assert!(!text.contains("NEPTRON_STORAGE_ROOT"), "{text}");
    let mut names: Vec<String> = Vec::new();
    let bytes: Vec<char> = text.chars().collect();
    for (i, c) in bytes.iter().enumerate() {
        if *c != '$' {
            continue;
        }
        let rest: String = bytes[i + 1..]
            .iter()
            .skip_while(|c| **c == '{')
            .take_while(|c| c.is_ascii_uppercase() || **c == '_' || c.is_ascii_digit())
            .collect();
        if rest.starts_with(|c: char| c.is_ascii_uppercase()) && !names.contains(&rest) {
            names.push(rest);
        }
    }
    for name in &names {
        assert!(
            text.contains(&format!("\n{name}=")),
            "{name} comes from the environment"
        );
    }
}

/// A flag that a variable of init could set is set by the script first.
#[test]
fn a_variable_from_init_cannot_open_the_share_path() {
    let out = Run::new("env-flag")
        .set("state_writable", "yes")
        .set("NEPTRON_STORAGE_ROOT", "/nonexistent")
        .set("FAKE_MOUNT_EXT4_EXIT", "32")
        .start();
    out.assert_share_ram("the state partition is not mounted, so no share partition is made");
    assert_eq!(out.calls("sfdisk"), 0, "{out:#?}");
}

/// sfdisk gets the planned start, never wipes a signature and never asks:
/// the free space can hold the share of an older image. Every line goes to
/// syslog.
#[test]
fn sfdisk_gets_the_planned_start_and_wipes_nothing() {
    let text = script();
    let sfdisk: String = text
        .split("| sfdisk ")
        .nth(1)
        .and_then(|rest| rest.split(" 2>&1)").next())
        .expect("the sfdisk call")
        .to_owned();
    for option in [
        "--force",
        "--no-reread",
        "--no-tell-kernel",
        "--wipe never",
        "--wipe-partitions never",
        "--lock=nonblock",
        "--append \"$disk_dev\"",
    ] {
        assert!(sfdisk.contains(option), "{option}: {sfdisk}");
    }
    assert!(text.contains("printf '%s,,7\\n' \"$planned\" | sfdisk "));
    assert!(!text.contains("--wipe always"));
    assert!(!text.contains("wipefs"));
    assert_eq!(text.matches("mkfs.exfat -L NEPTRON").count(), 1);
    assert!(text.contains("\tlogger -t neptron-storage \"$1\" 2>/dev/null\n"));
    let echoes: Vec<&str> = text
        .lines()
        .filter(|l| l.trim_start().starts_with("echo \"neptron-storage"))
        .collect();
    assert_eq!(
        echoes,
        ["\techo \"neptron-storage: $1\""],
        "only log() echoes"
    );
}

#[test]
fn empty_free_space_becomes_a_formatted_share() {
    let out = Run::new("fresh").start();
    out.assert_share_mounted(true);
    assert_eq!(out.calls("sfdisk"), 1, "{out:#?}");
    assert_eq!(out.calls("addpart "), 1, "{out:#?}");
    assert!(
        out.log.lines().any(|l| l.starts_with("addpart ")
            && l.ends_with(&format!("/dev/sda 4 {PLANNED} {FREE_SECTORS}"))),
        "{out:#?}"
    );
    assert!(
        out.log.contains(&format!(
            "logger -t neptron-storage making partition 4 at sector {PLANNED}"
        )),
        "{out:#?}"
    );
}

/// libfdisk aligns a start it chooses to the optimal I/O size of the
/// device; the fake sfdisk chooses 4 MiB. The script names the start, so
/// the entry is where the plan puts it.
#[test]
fn sfdisk_gets_the_start_and_does_not_choose_one() {
    let out = Run::new("grain").start();
    out.assert_share_mounted(true);
    assert!(
        out.log
            .lines()
            .any(|l| l.starts_with("sfdisk ") && l.ends_with(&format!("<<< {PLANNED},,7"))),
        "{out:#?}"
    );
}

/// The case of a stick written again over an older NEPTRON image: the old
/// share starts where partition 4 goes, and it keeps its files.
#[test]
fn an_old_exfat_share_at_the_planned_start_is_kept_as_it_is() {
    let run = Run::new("adopt").set("FAKE_BLKID", "TYPE=exfat");
    run.exfat_at(PLANNED);
    let out = run.start();
    out.assert_share_mounted(false);
    assert!(
        out.log
            .contains("partition 4 is made around it and keeps its files"),
        "{out:#?}"
    );
}

#[test]
fn sfdisk_failing_after_a_correct_write_still_gives_the_share() {
    let run = Run::new("sfdisk-late-fail")
        .set("FAKE_BLKID", "TYPE=exfat")
        .set("FAKE_SFDISK_EXIT", "1")
        .set("FAKE_SFDISK_SAYS", "sda: fsync device failed");
    run.exfat_at(PLANNED);
    let out = run.start();
    out.assert_share_mounted(false);
    assert!(
        out.log
            .contains("logger -t neptron-storage sfdisk exit 1: sda: fsync device failed"),
        "{out:#?}"
    );
    assert!(
        out.log.contains(&format!(
            "holds partition 4 as planned ({PLANNED} {FREE_SECTORS} 7), sfdisk exit 1"
        )),
        "{out:#?}"
    );
}

/// The table is read back from the device, not from the page cache that
/// sfdisk wrote: the cache of the disk is dropped first.
#[test]
fn the_cache_of_the_disk_is_dropped_before_the_read_back() {
    let out = Run::new("flush").start();
    out.assert_share_mounted(true);
    let sfdisk = out.first("sfdisk ");
    let flush = out.first("blockdev --flushbufs ");
    let dumps: Vec<usize> = out
        .log
        .lines()
        .enumerate()
        .filter(|(_, l)| *l == "dump")
        .map(|(i, _)| i)
        .collect();
    assert!(sfdisk < flush, "{out:#?}");
    assert!(dumps.iter().any(|d| *d > flush), "{out:#?}");
    assert!(
        out.log
            .contains("logger -t neptron-storage blockdev --flushbufs exit 0: the table is read back after a flush of the cache"),
        "{out:#?}"
    );
}

#[test]
fn a_flush_that_fails_is_logged_and_the_read_back_goes_on() {
    let out = Run::new("flush-fails").set("FAKE_FLUSH_EXIT", "1").start();
    out.assert_share_mounted(true);
    assert!(
        out.log.contains(
            "logger -t neptron-storage blockdev --flushbufs exit 1: the cache was not flushed, so the table may be read back from the cache"
        ),
        "{out:#?}"
    );
}

#[test]
fn a_read_back_that_fails_once_is_tried_again() {
    let out = Run::new("dump-once")
        .set("FAKE_DUMP_FAILS_AFTER_WRITE", "1")
        .start();
    out.assert_share_mounted(true);
    assert_eq!(out.calls("dump fails"), 1, "{out:#?}");
}

/// A table that cannot be read proves nothing: the marker stays, so the
/// next boot, where the kernel reads the table, can still format an empty
/// partition 4.
#[test]
fn a_table_that_cannot_be_read_back_keeps_the_marker() {
    let out = Run::new("dump-never")
        .set("FAKE_DUMP_FAILS_AFTER_WRITE", "99")
        .start();
    out.assert_share_ram(
        "the partition table cannot be read back after sfdisk (exit 0); the marker stays and the next boot uses partition 4",
    );
    assert!(out.marker, "{out:#?}");
    assert_eq!(out.calls("dump fails"), 5, "{out:#?}");
    assert_eq!(out.calls("addpart"), 0, "{out:#?}");
}

#[test]
fn sfdisk_that_writes_nothing_leaves_a_ram_folder_with_its_reason() {
    let out = Run::new("sfdisk-fail")
        .set("FAKE_SFDISK_WRITES", "no")
        .set("FAKE_SFDISK_EXIT", "1")
        .start();
    out.assert_share_ram("sfdisk exit 1, and the partition table has no partition 4");
    assert!(!out.marker, "no partition, no marker: {out:#?}");
    assert_eq!(out.calls("addpart"), 0, "{out:#?}");
}

fn other_partition_4(name: &str, start: u64, kind: &str) {
    let out = Run::new(name)
        .set("FAKE_P4_START", &start.to_string())
        .set("FAKE_P4_TYPE", kind)
        .start();
    out.assert_share_ram(&format!(
        "sfdisk exit 0, and the table holds partition 4 at sector {start} with {} sectors and type {kind}, not type 7 at sector {PLANNED}",
        DISK_SECTORS - start
    ));
    assert!(!out.marker, "{name}: {out:#?}");
    assert_eq!(out.calls("addpart"), 0, "{name}: {out:#?}");
}

#[test]
fn a_partition_4_at_another_start_is_left_untouched() {
    other_partition_4("plan-start", PLANNED + 2048, "7");
}

#[test]
fn a_partition_4_of_another_type_is_left_untouched() {
    other_partition_4("plan-type", PLANNED, "83");
}

#[test]
fn addpart_failing_once_is_tried_again() {
    let out = Run::new("addpart-once")
        .set("FAKE_ADDPART_FAILS", "1")
        .start();
    out.assert_share_mounted(true);
    assert_eq!(out.calls("addpart "), 2, "{out:#?}");
    assert_eq!(out.calls("usleep 500000"), 1, "{out:#?}");
    assert!(
        out.log.contains(
            "logger -t neptron-storage addpart try 1 of 5, exit 1: the kernel does not see partition 4: addpart: failed to add partition: Device or resource busy"
        ),
        "{out:#?}"
    );
}

/// A tool can exit with 0 and add nothing: only the kernel's list counts.
#[test]
fn a_partition_the_kernel_never_sees_is_named_and_not_silent() {
    let out = Run::new("addpart-silent")
        .set("FAKE_ADDPART_ADDS", "no")
        .start();
    out.assert_share_ram(
        "partition 4 is in the partition table, but the kernel does not see it after 5 tries of addpart (last exit 0); restart to use it",
    );
    assert_eq!(out.calls("addpart "), 5, "{out:#?}");
    assert_eq!(out.calls("usleep 500000"), 4, "{out:#?}");
    assert!(
        out.marker,
        "the next boot formats an empty partition 4: {out:#?}"
    );
}

#[test]
fn a_device_node_that_comes_late_is_waited_for() {
    let out = Run::new("node-late").set("FAKE_NODE_AT", "3").start();
    out.assert_share_mounted(true);
    assert_eq!(out.calls("usleep 100000"), 3, "{out:#?}");
}

#[test]
fn a_device_node_that_never_comes_is_a_reason() {
    let out = Run::new("node-never").set("FAKE_NODE_AT", "999").start();
    let reason = out.assert_share_ram("the kernel sees partition 4, but its device node");
    assert!(
        reason.ends_with("does not appear within 5000 ms"),
        "{out:#?}"
    );
    assert_eq!(out.calls("usleep 100000"), 50, "{out:#?}");
}

#[test]
fn a_partition_4_in_the_table_but_not_in_the_kernel_is_added() {
    let run = Run::new("table-only").set("FAKE_BLKID", "TYPE=exfat");
    run.table_partition_4(PLANNED, FREE_SECTORS, "7");
    let out = run.start();
    out.assert_share_mounted(false);
    assert_eq!(out.calls("sfdisk"), 0, "{out:#?}");
    assert_eq!(out.calls("addpart "), 1, "{out:#?}");
}

#[test]
fn a_partition_4_the_kernel_sees_is_mounted_at_once() {
    let out = Run::new("known")
        .set("FAKE_BLKID", "TYPE=exfat")
        .with_partition_4()
        .start();
    out.assert_share_mounted(false);
    assert_eq!(out.calls("sfdisk"), 0, "{out:#?}");
    assert_eq!(out.calls("addpart"), 0, "{out:#?}");
    assert_eq!(out.calls("usleep"), 0, "{out:#?}");
}

#[test]
fn too_little_free_space_is_a_reason() {
    let run = Run::new("small");
    std::fs::write(
        run.path("sys/devices/blk/sda/size"),
        format!("{}\n", PLANNED + 1000),
    )
    .unwrap();
    run.start()
        .assert_share_ram("1000 free sectors after the state partition, less than 131072");
}

#[test]
fn an_old_share_at_another_start_is_a_reason() {
    let run = Run::new("old-elsewhere");
    run.exfat_at(PLANNED + 4096);
    let out = run.start();
    out.assert_share_ram(&format!(
        "an old exFAT share starts at sector {}, not at {PLANNED}",
        PLANNED + 4096
    ));
    assert_eq!(out.calls("sfdisk"), 0, "{out:#?}");
}

#[test]
fn no_partition_3_is_a_reason_for_both() {
    let run = Run::new("no-state");
    std::fs::remove_dir_all(run.path("sys/devices/blk/sda/sda3")).unwrap();
    let out = run.start();
    out.assert_share_ram("the boot disk has no partition 3, so no share partition is made");
    assert!(
        out.status
            .contains("state_reason = \"the boot disk has no partition 3"),
        "{out:#?}"
    );
}

#[test]
fn a_root_on_a_whole_disk_is_a_reason_for_both() {
    let run = Run::new("whole-disk");
    std::fs::remove_file(run.path("sys/devices/blk/sda/sda2/partition")).unwrap();
    run.start()
        .assert_share_ram("the root filesystem is not on a partition of a disk");
}

#[test]
fn a_state_partition_that_does_not_mount_stops_the_share() {
    let out = Run::new("state-unmounted")
        .set("FAKE_MOUNT_EXT4_EXIT", "32")
        .start();
    out.assert_share_ram("the state partition is not mounted, so no share partition is made");
    assert!(
        out.status
            .contains("does not mount; /state is a RAM folder"),
        "{out:#?}"
    );
}

#[test]
fn a_share_that_does_not_mount_is_a_reason() {
    let run = Run::new("exfat-mount")
        .set("FAKE_BLKID", "TYPE=exfat")
        .set("FAKE_MOUNT_EXFAT_EXIT", "32");
    run.exfat_at(PLANNED);
    let out = run.start();
    assert!(out.status.contains("share = \"ram\""), "{out:#?}");
    assert!(out.status.contains("sda4 does not mount\""), "{out:#?}");
    assert!(out.state_log.contains("sda4 does not mount"), "{out:#?}");
}

#[test]
fn an_unclean_probe_is_a_reason() {
    Run::new("unclean")
        .set("FAKE_BLKID_ERROR", "blkid: read error")
        .with_partition_4()
        .start()
        .assert_share_ram("was left untouched: blkid could not probe it cleanly");
}

/// The format rule that keeps the user's data: an empty partition 4 that
/// this system did not make is never formatted, at any boot.
#[test]
fn an_empty_partition_4_with_no_marker_is_named_as_such() {
    Run::new("no-marker")
        .with_partition_4()
        .start()
        .assert_share_ram("sda4 holds no file system that blkid finds and has no /state/share-pending, so it is left untouched");
}

#[test]
fn a_damaged_share_is_a_reason() {
    let run = Run::new("backup-boot-sector").with_partition_4();
    run.write_marker();
    let mut sectors = vec![0_u8; 13 * 512];
    sectors[12 * 512 + 3..12 * 512 + 11].copy_from_slice(b"EXFAT   ");
    std::fs::write(run.path("dev/sda4"), sectors).unwrap();
    run.start()
        .assert_share_ram("it holds an exFAT backup boot sector (sector 12)");
}

fn state_ram(name: &str, var: &str, value: &str, reason: &str) {
    let out = Run::new(name).set(var, value).start();
    let state_reason = out
        .status
        .lines()
        .find_map(|l| l.strip_prefix("state_reason = \""))
        .and_then(|l| l.strip_suffix('"'))
        .unwrap_or_else(|| panic!("{name}: {out:#?}"));
    assert!(state_reason.ends_with(reason), "{name}: {out:#?}");
    assert!(
        out.log
            .lines()
            .any(|l| l == format!("logger -t neptron-storage {state_reason}")),
        "{name}: {out:#?}"
    );
    assert_eq!(out.state_log, "", "no state partition to write: {out:#?}");
}

#[test]
fn e2fsck_errors_are_a_reason_in_syslog() {
    state_ram(
        "state-e2fsck",
        "FAKE_E2FSCK_EXIT",
        "8",
        "left errors (exit 8); /state is a RAM folder",
    );
}

#[test]
fn a_state_mount_failure_is_a_reason_in_syslog() {
    state_ram(
        "state-ext4",
        "FAKE_MOUNT_EXT4_EXIT",
        "32",
        "sda3 does not mount; /state is a RAM folder",
    );
}

/// `/var/log` is a RAM folder and a release image has no SSH, so the lines
/// of each boot also go to the state partition, written directly: syslog
/// may not run yet.
#[test]
fn the_lines_of_each_boot_go_to_the_state_partition_without_syslog() {
    let out = Run::new("state-log").set("FAKE_LOGGER_EXIT", "1").start();
    out.assert_share_mounted(true);
    let lines: Vec<&str> = out.state_log.lines().collect();
    assert_eq!(lines.first().copied(), Some("== boot DATE"), "{out:#?}");
    assert!(
        lines
            .iter()
            .any(|l| l.ends_with("sda3 mounted at /state (e2fsck exit 0)")),
        "the lines before the state mount are kept: {out:#?}"
    );
    assert!(
        lines.iter().any(|l| l.starts_with("making partition 4")),
        "{out:#?}"
    );
}

#[test]
fn the_storage_log_keeps_the_last_lines_only() {
    let run = Run::new("state-log-bound");
    std::fs::create_dir_all(run.path("state/log")).unwrap();
    let old: String = (0..1000).map(|i| format!("old line {i}\n")).collect();
    std::fs::write(run.path("state/log/storage.log"), old).unwrap();
    let out = run.start();
    let lines: Vec<&str> = out.state_log.lines().collect();
    assert_eq!(lines.len(), 300, "{out:#?}");
    assert!(lines.contains(&"== boot DATE"), "{out:#?}");
    assert!(!lines.contains(&"old line 0"), "{out:#?}");
    assert!(
        lines.last().unwrap().ends_with("sda4 mounted at /share"),
        "{out:#?}"
    );
}

// The format rules: each test fails when its rule is gone from the script.

/// A refusal to format partition 4: `/share` is a RAM folder with
/// `reason`, and the marker stays.
fn format_refused(run: Run, reason: &str) {
    let out = run.start();
    out.assert_share_ram(reason);
    assert!(out.marker, "a refusal keeps the marker: {out:#?}");
}

/// An empty partition 4 at `start` of `kind`, in the table and in the
/// kernel, with the marker of the planned partition.
fn marked_partition_4(name: &str, start: u64, kind: &str) -> Run {
    let run = Run::new(name);
    run.table_partition_4(start, DISK_SECTORS - start, kind);
    run.add_partition(4, start, DISK_SECTORS - start);
    run.write_marker();
    run
}

#[test]
fn a_marked_partition_4_of_another_type_is_not_formatted() {
    format_refused(
        marked_partition_4("format-type", PLANNED, "83"),
        "its type or its start is not those of the share partition of this image",
    );
}

#[test]
fn a_marked_partition_4_at_another_start_is_not_formatted() {
    format_refused(
        marked_partition_4("format-start", PLANNED + 2048, "7"),
        "its type or its start is not those of the share partition of this image",
    );
}

#[test]
fn a_marked_partition_4_that_does_not_read_is_not_formatted() {
    let run = marked_partition_4("format-read", PLANNED, "7");
    // dd cannot read a folder, as it cannot read a bad stick.
    std::fs::remove_file(run.path("dev/sda4")).unwrap();
    std::fs::create_dir(run.path("dev/sda4")).unwrap();
    format_refused(run, "its first 16 MiB do not read cleanly");
}

fn backup_boot_sector(sector: usize) {
    let run = marked_partition_4(&format!("format-backup-{sector}"), PLANNED, "7");
    let mut bytes = vec![0_u8; (sector + 1) * 512];
    bytes[sector * 512 + 3..sector * 512 + 11].copy_from_slice(b"EXFAT   ");
    std::fs::write(run.path("dev/sda4"), bytes).unwrap();
    format_refused(
        run,
        &format!(
            "it holds an exFAT backup boot sector (sector {sector}), so it is a damaged share"
        ),
    );
}

#[test]
fn an_exfat_backup_boot_sector_at_sector_12_stops_a_format() {
    backup_boot_sector(12);
}

#[test]
fn an_exfat_backup_boot_sector_at_sector_24_stops_a_format() {
    backup_boot_sector(24);
}

#[test]
fn an_exfat_backup_boot_sector_at_sector_48_stops_a_format() {
    backup_boot_sector(48);
}

#[test]
fn an_exfat_backup_boot_sector_at_sector_96_stops_a_format() {
    backup_boot_sector(96);
}

#[test]
fn a_failed_format_is_a_reason_and_keeps_the_marker() {
    let run = marked_partition_4("format-fails", PLANNED, "7").set("FAKE_MKFS_EXIT", "1");
    let out = run.start();
    out.assert_share_ram_with("mkfs.exfat failed on ", 1);
    assert!(out.marker, "the next boot tries again: {out:#?}");
}

#[test]
fn a_marker_that_cannot_be_written_stops_the_partition() {
    let run = Run::new("marker-unwritable");
    // A folder in the place of the marker file: the write fails, also for
    // root.
    std::fs::create_dir_all(run.path("state/share-pending")).unwrap();
    let out = run.start();
    out.assert_share_ram("share-pending cannot be written, so no share partition is made");
    assert_eq!(out.calls("sfdisk"), 0, "{out:#?}");
}

#[test]
fn a_table_that_cannot_be_read_before_sfdisk_stops_the_partition() {
    let out = Run::new("table-unreadable")
        .set("FAKE_DUMP_ALWAYS_FAILS", "yes")
        .start();
    out.assert_share_ram("cannot be read, so no share partition is made");
    assert_eq!(out.calls("sfdisk"), 0, "{out:#?}");
    assert!(!out.marker, "{out:#?}");
}

/// Boot 1 cannot read the table back, so the marker stays; at boot 2 the
/// kernel has read the table, and partition 4 is there.
fn second_boot_after_an_unread_table(name: &str, boot_2: &[(&str, &str)]) -> Outcome {
    let run = Run::new(name);
    let first = run.boot(&[("FAKE_DUMP_FAILS_AFTER_WRITE", "99")]);
    first.assert_share_ram("the partition table cannot be read back after sfdisk");
    assert!(first.marker, "{first:#?}");
    run.add_partition(4, PLANNED, FREE_SECTORS);
    run.boot(boot_2)
}

#[test]
fn a_kept_marker_lets_the_next_boot_format_an_empty_partition_4() {
    let second = second_boot_after_an_unread_table("two-boots-empty", &[]);
    second.assert_share_mounted(true);
    assert_eq!(second.calls("sfdisk"), 0, "{second:#?}");
}

#[test]
fn a_kept_marker_never_formats_a_partition_4_with_a_filesystem() {
    let second =
        second_boot_after_an_unread_table("two-boots-filesystem", &[("FAKE_BLKID", "TYPE=exfat")]);
    second.assert_share_mounted(false);
}

/// A read back that differs from the plan removes the marker; the next
/// boot names only the fact, as for any empty partition 4 with no marker.
#[test]
fn after_a_table_off_the_plan_the_next_boot_states_the_fact() {
    let run = Run::new("off-plan-two-boots");
    let first = run.boot(&[("FAKE_P4_START", &(PLANNED + 2048).to_string())]);
    first.assert_share_ram("not type 7 at sector");
    assert!(!first.marker, "{first:#?}");
    run.add_partition(4, PLANNED + 2048, DISK_SECTORS - PLANNED - 2048);
    let second = run.boot(&[]);
    let reason = second
        .assert_share_ram("holds no file system that blkid finds and has no /state/share-pending, so it is left untouched");
    assert!(!reason.contains("not made by this system"), "{second:#?}");
}

/// A partition 4 with a filesystem never needs the format permission
/// again: the marker goes even when the mount fails.
#[test]
fn a_filesystem_on_partition_4_removes_the_marker_also_with_no_mount() {
    let run = marked_partition_4("signature-marker", PLANNED, "7")
        .set("FAKE_BLKID", "TYPE=exfat")
        .set("FAKE_MOUNT_EXFAT_EXIT", "32");
    let out = run.start();
    assert!(out.status.contains("sda4 does not mount\""), "{out:#?}");
    assert!(!out.marker, "{out:#?}");
    assert!(
        out.log.contains("logger -t neptron-storage partition 4 holds a filesystem, so /state/share-pending is removed"),
        "{out:#?}"
    );
}

/// The marker names the partition it allows to format: the planned start
/// and size.
#[test]
fn the_marker_names_the_planned_start_and_size() {
    let out = Run::new("marker-text")
        .set("FAKE_DUMP_FAILS_AFTER_WRITE", "99")
        .start();
    assert_eq!(
        out.marker_text,
        format!("partition 4 made at DATE\nstart={PLANNED}\nsize={FREE_SECTORS}\n"),
        "{out:#?}"
    );
}

fn marker_mismatch(name: &str, marker: &str) {
    let run = marked_partition_4(name, PLANNED, "7");
    std::fs::write(run.path("state/share-pending"), marker).unwrap();
    format_refused(
        run,
        &format!(
            "was left untouched: /state/share-pending does not name start {PLANNED} and size {FREE_SECTORS}"
        ),
    );
}

#[test]
fn a_marker_of_another_size_allows_no_format() {
    marker_mismatch(
        "marker-size",
        &format!(
            "partition 4 made at DATE\nstart={PLANNED}\nsize={}\n",
            FREE_SECTORS - 2048
        ),
    );
}

/// A marker of an older image names no partition, so it allows no format.
#[test]
fn a_marker_that_names_no_partition_allows_no_format() {
    marker_mismatch("marker-old", "partition 4 made at DATE\n");
}

// The format offer and the format request of the user. Each test fails
// when its rule is gone from the script.

impl Run {
    /// An empty partition 4 at `start` of `kind` to the end of the disk, in
    /// the table and in the kernel, with no marker.
    fn empty_partition_4(name: &str, start: u64, kind: &str) -> Self {
        let run = Run::new(name);
        run.table_partition_4(start, DISK_SECTORS - start, kind);
        run.add_partition(4, start, DISK_SECTORS - start);
        run
    }

    /// The device path of partition 4, as the script names it.
    fn sda4(&self) -> String {
        format!("{}/dev/sda4", self.dir.display())
    }

    /// Writes the format request as the frontend writes it.
    fn write_request(&self, device: &str, start: u64, size: u64, kind: &str) {
        std::fs::create_dir_all(self.path("state")).unwrap();
        std::fs::write(
            self.path("state/share-format-request"),
            format!("device={device}\nstart={start}\nsize={size}\ntype={kind}\n"),
        )
        .unwrap();
    }

    /// The request of the offer for the planned partition 4.
    fn request_planned(&self, kind: &str) {
        self.write_request(&self.sda4(), PLANNED, FREE_SECTORS, kind);
    }
}

impl Outcome {
    /// The value of `key` in the status file, without quotes.
    fn status_value(&self, key: &str) -> Option<String> {
        self.status
            .lines()
            .find_map(|l| l.strip_prefix(&format!("{key} = ")))
            .map(|v| v.trim_matches('"').to_owned())
    }

    /// The offer of the status file: device, start, size, type.
    fn offer(&self) -> Option<(String, String, String, String)> {
        let fix = self.status_value("share_fix")?;
        assert_eq!(fix, "format", "{self:#?}");
        let get = |key| {
            self.status_value(key)
                .unwrap_or_else(|| panic!("no {key}: {self:#?}"))
        };
        Some((
            get("share_fix_device"),
            get("share_fix_start"),
            get("share_fix_size"),
            get("share_fix_type"),
        ))
    }

    /// `/share` is a RAM folder and no format is offered; the log says why
    /// when `why` is given.
    fn assert_no_offer(&self, why: Option<&str>) {
        assert!(self.status.contains("share = \"ram\""), "{self:#?}");
        assert_eq!(self.offer(), None, "{self:#?}");
        assert!(!self.status.contains("share_fix"), "{self:#?}");
        if let Some(why) = why {
            assert!(
                self.log
                    .contains(&format!("cannot be offered a format: {why}")),
                "{why}: {self:#?}"
            );
        }
    }

    /// The share partition was formatted on the request and mounted.
    fn assert_formatted_on_request(&self) {
        assert!(self.ok, "{self:#?}");
        assert_eq!(self.calls("mount -t exfat"), 1, "{self:#?}");
        assert_eq!(self.calls("mkfs.exfat"), 1, "{self:#?}");
        assert!(
            self.state_log.contains("sda4 mounted at /share"),
            "{self:#?}"
        );
        assert!(!self.request, "the request is removed: {self:#?}");
        assert_eq!(
            self.status, "state = \"partition\"\nshare = \"partition\"\nshare_format = \"done\"\n",
            "{self:#?}"
        );
        assert!(
            self.log
                .contains("is formatted exFAT on the request of the user"),
            "{self:#?}"
        );
    }

    /// The request was refused with `why`, the request file is gone, and
    /// nothing was formatted.
    fn assert_request_refused(&self, why: &str) {
        assert!(self.ok, "{self:#?}");
        assert!(!self.request, "the request is removed: {self:#?}");
        assert_eq!(self.calls("mkfs.exfat"), 0, "{self:#?}");
        assert_eq!(
            self.status_value("share_format").as_deref(),
            Some("refused"),
            "{self:#?}"
        );
        let reason = self
            .status_value("share_format_reason")
            .unwrap_or_else(|| panic!("no reason: {self:#?}"));
        assert!(reason.contains(why), "{why}: {self:#?}");
        assert!(
            self.state_log
                .contains(&format!("the format request is refused: {reason}")),
            "{self:#?}"
        );
    }
}

#[test]
fn an_empty_partition_4_with_no_marker_is_offered_a_format() {
    let run = Run::new("offer-no-marker").with_partition_4();
    let out = run.boot(&[]);
    out.assert_share_ram("holds no file system that blkid finds and has no /state/share-pending");
    assert_eq!(
        out.offer(),
        Some((
            run.sda4(),
            PLANNED.to_string(),
            FREE_SECTORS.to_string(),
            "7".to_owned()
        )),
        "{out:#?}"
    );
    assert!(
        out.state_log.contains("is offered in Settings > System"),
        "{out:#?}"
    );
}

#[test]
fn an_empty_partition_4_of_another_type_is_offered_a_format() {
    let run = Run::empty_partition_4("offer-type", PLANNED, "83");
    let out = run.start();
    out.assert_share_ram("holds no file system that blkid finds and has no /state/share-pending");
    let (_, start, _, kind) = out.offer().expect("an offer");
    assert_eq!((start.as_str(), kind.as_str()), ("10240", "83"), "{out:#?}");
}

#[test]
fn an_empty_partition_4_at_another_start_is_offered_a_format() {
    let start = PLANNED + 2048;
    let run = Run::empty_partition_4("offer-start", start, "7");
    let out = run.start();
    let (_, offered, size, _) = out.offer().expect("an offer");
    assert_eq!(offered, start.to_string(), "{out:#?}");
    assert_eq!(size, (DISK_SECTORS - start).to_string(), "{out:#?}");
}

#[test]
fn an_empty_partition_4_with_a_marker_of_another_size_is_offered_a_format() {
    let run = marked_partition_4("offer-marker", PLANNED, "7");
    std::fs::write(
        run.path("state/share-pending"),
        format!("start={PLANNED}\nsize=1\n"),
    )
    .unwrap();
    let out = run.start();
    out.assert_share_ram("/state/share-pending does not name start");
    assert!(out.offer().is_some(), "{out:#?}");
}

#[test]
fn a_failed_format_is_offered_again() {
    let run = marked_partition_4("offer-mkfs", PLANNED, "7").set("FAKE_MKFS_EXIT", "1");
    let out = run.start();
    out.assert_share_ram_with("mkfs.exfat failed on ", 1);
    assert!(out.offer().is_some(), "{out:#?}");
}

#[test]
fn a_damaged_share_is_never_offered_a_format() {
    for sector in [12_usize, 96] {
        let run = Run::new(&format!("no-offer-backup-{sector}")).with_partition_4();
        let mut bytes = vec![0_u8; (sector + 1) * 512];
        bytes[sector * 512 + 3..sector * 512 + 11].copy_from_slice(b"EXFAT   ");
        std::fs::write(run.path("dev/sda4"), bytes).unwrap();
        run.start().assert_no_offer(Some(&format!(
            "it holds an exFAT backup boot sector (sector {sector})"
        )));
    }
}

#[test]
fn a_partition_4_that_does_not_read_is_never_offered_a_format() {
    let run = Run::new("no-offer-read").with_partition_4();
    // dd cannot read a folder, as it cannot read a bad stick.
    std::fs::remove_file(run.path("dev/sda4")).unwrap();
    std::fs::create_dir(run.path("dev/sda4")).unwrap();
    run.start()
        .assert_no_offer(Some("its first 16 MiB do not read cleanly"));
}

#[test]
fn an_unclean_probe_is_never_offered_a_format() {
    Run::new("no-offer-unclean")
        .set("FAKE_BLKID_ERROR", "blkid: read error")
        .with_partition_4()
        .start()
        .assert_no_offer(None);
}

#[test]
fn a_filesystem_that_does_not_mount_is_never_offered_a_format() {
    let run = Run::new("no-offer-mount")
        .set("FAKE_BLKID", "TYPE=exfat")
        .set("FAKE_MOUNT_EXFAT_EXIT", "32")
        .with_partition_4();
    let out = run.start();
    assert!(out.status.contains("sda4 does not mount\""), "{out:#?}");
    out.assert_no_offer(None);
}

#[test]
fn an_old_share_inside_an_empty_partition_4_is_never_offered_a_format() {
    let run = Run::new("no-offer-inside").with_partition_4();
    run.exfat_at(PLANNED + 4096);
    run.start().assert_no_offer(Some(&format!(
        "an exFAT boot sector starts at sector {} of the disk, before or inside it",
        PLANNED + 4096
    )));
}

#[test]
fn an_extended_partition_4_is_never_offered_a_format() {
    for kind in ["5", "f", "85"] {
        Run::empty_partition_4(&format!("no-offer-extended-{kind}"), PLANNED, kind)
            .start()
            .assert_no_offer(Some(&format!("it is an extended partition (type {kind})")));
    }
}

#[test]
fn a_partition_4_the_kernel_and_the_table_disagree_on_is_never_offered_a_format() {
    let run = Run::new("no-offer-disagree");
    run.table_partition_4(PLANNED, FREE_SECTORS - 2048, "7");
    run.add_partition(4, PLANNED, FREE_SECTORS);
    run.start().assert_no_offer(Some(&format!(
        "the partition table names start {PLANNED} and size {}, the kernel start {PLANNED} and size {FREE_SECTORS}",
        FREE_SECTORS - 2048
    )));
}

#[test]
fn a_partition_4_over_the_state_partition_is_never_offered_a_format() {
    let start = STATE_START + 2048;
    Run::empty_partition_4("no-offer-overlap", start, "7")
        .start()
        .assert_no_offer(Some("it starts before the end of the state partition"));
}

#[test]
fn a_table_that_is_not_mbr_is_never_offered_a_format() {
    let run = Run::new("no-offer-gpt").with_partition_4();
    let table = std::fs::read_to_string(run.path("table"))
        .unwrap()
        .replace("label: dos", "label: gpt");
    std::fs::write(run.path("table"), table).unwrap();
    run.start()
        .assert_no_offer(Some("the partition table is not an MBR table"));
}

#[test]
fn with_no_state_partition_mounted_no_format_is_offered() {
    Run::new("no-offer-state")
        .set("FAKE_MOUNT_EXT4_EXIT", "32")
        .with_partition_4()
        .start()
        .assert_no_offer(Some("the state partition is not mounted"));
}

/// The states with no partition 4, which a format cannot fix.
#[test]
fn states_with_no_partition_4_have_no_offer() {
    let small = Run::new("no-offer-small");
    std::fs::write(
        small.path("sys/devices/blk/sda/size"),
        format!("{}\n", PLANNED + 1000),
    )
    .unwrap();
    let elsewhere = Run::new("no-offer-elsewhere");
    elsewhere.exfat_at(PLANNED + 4096);
    let no_state = Run::new("no-offer-no-3");
    std::fs::remove_dir_all(no_state.path("sys/devices/blk/sda/sda3")).unwrap();
    let whole = Run::new("no-offer-whole");
    std::fs::remove_file(whole.path("sys/devices/blk/sda/sda2/partition")).unwrap();
    for run in [
        small,
        elsewhere,
        no_state,
        whole,
        Run::new("no-offer-unreadable").set("FAKE_DUMP_ALWAYS_FAILS", "yes"),
        Run::new("no-offer-unmounted").set("FAKE_MOUNT_EXT4_EXIT", "32"),
    ] {
        run.start().assert_no_offer(None);
    }
}

#[test]
fn a_partition_with_a_filesystem_has_no_offer() {
    let out = Run::new("no-offer-fs")
        .set("FAKE_BLKID", "TYPE=exfat")
        .with_partition_4()
        .start();
    out.assert_share_mounted(false);
    assert!(!out.status.contains("share_fix"), "{out:#?}");
}

#[test]
fn a_request_for_the_offered_partition_formats_and_mounts_it() {
    let run = Run::new("request-good").with_partition_4();
    run.request_planned("7");
    let out = run.start();
    out.assert_formatted_on_request();
    assert_eq!(out.calls("sfdisk-set-type"), 0, "{out:#?}");
    // The request goes before anything is done for the share.
    let removed = out.first("logger -t neptron-storage format request of the user for ");
    assert!(removed < out.first("mkfs.exfat"), "{out:#?}");
    assert!(
        out.log.lines().nth(removed - 1) == Some("sync"),
        "the removal is synced: {out:#?}"
    );
}

#[test]
fn a_request_for_a_partition_4_of_another_type_sets_type_7_then_formats() {
    let run = Run::empty_partition_4("request-type", PLANNED, "83");
    run.request_planned("83");
    let out = run.boot(&[]);
    out.assert_formatted_on_request();
    assert_eq!(out.calls("sfdisk-set-type"), 1, "{out:#?}");
    let set = out.first("sfdisk-set-type");
    assert!(
        out.log.lines().nth(set).is_some_and(|l| l
            .contains("--no-reread --no-tell-kernel --lock=nonblock --part-type")
            && l.ends_with("/dev/sda 4 7")),
        "{out:#?}"
    );
    let flush = out
        .log
        .lines()
        .skip(set)
        .position(|l| l.starts_with("blockdev --flushbufs"));
    assert!(flush.is_some(), "{out:#?}");
    assert!(set < out.first("mkfs.exfat"), "{out:#?}");
    let table = std::fs::read_to_string(run.path("table")).unwrap();
    assert!(
        table.contains(&format!(
            "start={PLANNED:>12}, size={FREE_SECTORS:>12}, type=7"
        )),
        "{table}"
    );
}

#[test]
fn a_type_change_that_does_not_reach_the_table_formats_nothing() {
    let run = Run::empty_partition_4("request-type-lost", PLANNED, "83")
        .set("FAKE_RETYPE_WRITES", "no")
        .set("FAKE_RETYPE_EXIT", "0");
    run.request_planned("83");
    run.start()
        .assert_request_refused("after the type change the table holds partition 4 as");
}

#[test]
fn a_request_whose_numbers_do_not_match_the_disk_formats_nothing() {
    for (name, start, size, kind, why) in [
        (
            "request-size",
            PLANNED,
            FREE_SECTORS - 1,
            "7",
            "it names start",
        ),
        (
            "request-start",
            PLANNED + 2048,
            FREE_SECTORS,
            "7",
            "it names start",
        ),
        (
            "request-type-other",
            PLANNED,
            FREE_SECTORS,
            "83",
            "and type 83",
        ),
        ("request-bad-number", PLANNED, 0, "x", "no valid type"),
    ] {
        let run = Run::new(name).with_partition_4();
        run.write_request(&run.sda4(), start, size, kind);
        let out = run.start();
        out.assert_request_refused(why);
        out.assert_share_ram(
            "holds no file system that blkid finds and has no /state/share-pending",
        );
        assert!(out.offer().is_some(), "the offer stays: {out:#?}");
    }
}

#[test]
fn a_request_with_no_valid_numbers_formats_nothing() {
    let run = Run::new("request-garbage").with_partition_4();
    std::fs::create_dir_all(run.path("state")).unwrap();
    std::fs::write(
        run.path("state/share-format-request"),
        format!("device={}\nstart=10240; mkfs\n", run.sda4()),
    )
    .unwrap();
    run.start()
        .assert_request_refused("it names no valid start and size");
}

#[test]
fn a_request_for_another_device_formats_nothing() {
    let run = Run::new("request-device").with_partition_4();
    run.write_request("/dev/sdb4", PLANNED, FREE_SECTORS, "7");
    run.start()
        .assert_request_refused("it names /dev/sdb4, not partition 4 of the boot disk");
}

#[test]
fn a_request_for_a_partition_that_now_holds_a_filesystem_formats_nothing() {
    let run = Run::new("request-filesystem")
        .set("FAKE_BLKID", "TYPE=exfat")
        .with_partition_4();
    run.request_planned("7");
    let out = run.start();
    assert!(!out.request, "{out:#?}");
    assert_eq!(out.calls("mkfs.exfat"), 0, "{out:#?}");
    assert!(
        out.status
            .starts_with("state = \"partition\"\nshare = \"partition\"\n"),
        "{out:#?}"
    );
    assert_eq!(
        out.status_value("share_format").as_deref(),
        Some("refused"),
        "{out:#?}"
    );
    assert!(
        out.status_value("share_format_reason")
            .is_some_and(|r| r.contains("it holds a filesystem or a partition table")),
        "{out:#?}"
    );
}

#[test]
fn a_request_for_a_damaged_share_formats_nothing() {
    let run = Run::new("request-backup").with_partition_4();
    let mut bytes = vec![0_u8; 49 * 512];
    bytes[48 * 512 + 3..48 * 512 + 11].copy_from_slice(b"EXFAT   ");
    std::fs::write(run.path("dev/sda4"), bytes).unwrap();
    run.request_planned("7");
    run.start()
        .assert_request_refused("it holds an exFAT backup boot sector (sector 48)");
}

#[test]
fn a_request_with_an_unclean_probe_formats_nothing() {
    let run = Run::new("request-unclean")
        .set("FAKE_BLKID_ERROR", "blkid: read error")
        .with_partition_4();
    run.request_planned("7");
    run.start()
        .assert_request_refused("blkid could not probe it cleanly");
}

#[test]
fn a_request_with_no_partition_4_formats_nothing() {
    let run = Run::new("request-no-4");
    std::fs::write(
        run.path("sys/devices/blk/sda/size"),
        format!("{}\n", PLANNED + 1000),
    )
    .unwrap();
    run.request_planned("7");
    run.start()
        .assert_request_refused("no partition 4 reached the checks: 1000 free sectors");
}

#[test]
fn a_request_whose_format_fails_is_offered_again_and_not_kept() {
    let run = Run::new("request-mkfs")
        .set("FAKE_MKFS_EXIT", "1")
        .with_partition_4();
    run.request_planned("7");
    let out = run.start();
    out.assert_share_ram_with("mkfs.exfat failed on ", 1);
    assert!(!out.request, "{out:#?}");
    assert_eq!(
        out.status_value("share_format").as_deref(),
        Some("failed"),
        "{out:#?}"
    );
    assert!(out.offer().is_some(), "{out:#?}");
}

#[test]
fn a_request_that_cannot_be_removed_formats_nothing() {
    let run = Run::new("request-stuck").with_partition_4();
    std::fs::create_dir_all(run.path("state/share-format-request")).unwrap();
    let out = run.start();
    assert_eq!(out.calls("mkfs.exfat"), 0, "{out:#?}");
    assert!(
        out.log
            .contains("share-format-request cannot be removed, so the format request is not used"),
        "{out:#?}"
    );
    assert_eq!(out.status_value("share_format"), None, "{out:#?}");
}

#[test]
fn a_request_is_used_once() {
    let run = Run::new("request-once")
        .set("FAKE_MKFS_EXIT", "1")
        .with_partition_4();
    run.request_planned("7");
    let first = run.boot(&[]);
    assert_eq!(first.calls("mkfs.exfat"), 1, "{first:#?}");
    let second = run.boot(&[]);
    assert_eq!(second.calls("mkfs.exfat"), 0, "{second:#?}");
    assert_eq!(second.status_value("share_format"), None, "{second:#?}");
}

#[test]
fn with_no_request_nothing_is_formatted() {
    let run = Run::new("request-none").with_partition_4();
    for _ in 0..2 {
        let out = run.boot(&[]);
        assert_eq!(out.calls("mkfs.exfat"), 0, "{out:#?}");
        assert!(out.offer().is_some(), "{out:#?}");
        assert_eq!(out.status_value("share_format"), None, "{out:#?}");
    }
}

/// An old share at the planned start can run into a partition 4 that
/// starts later: the scan starts at the planned start, not at partition 4.
#[test]
fn an_old_share_before_partition_4_is_never_offered_a_format() {
    let run = Run::empty_partition_4("no-offer-before", PLANNED + 2048, "83");
    run.exfat_at(PLANNED);
    run.start().assert_no_offer(Some(&format!(
        "an exFAT boot sector starts at sector {PLANNED} of the disk"
    )));
}

/// Sectors in one MiB.
const MIB_SECTORS: u64 = 2048;

impl Run {
    /// A disk whose free space is an empty partition 4 of type 7 at the
    /// planned start with `sectors`, in the table and in the kernel, with
    /// no marker.
    fn with_large_partition_4(name: &str, sectors: u64) -> Self {
        let run = Run::new(name);
        std::fs::write(
            run.path("sys/devices/blk/sda/size"),
            format!("{}\n", PLANNED + sectors),
        )
        .unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(run.path("dev/sda"))
            .unwrap()
            .set_len((PLANNED + sectors) * 512)
            .unwrap();
        run.table_partition_4(PLANNED, sectors, "7");
        run.add_partition(4, PLANNED, sectors);
        run
    }
}

/// The offer runs the scan of the request, so it never promises what the
/// request refuses.
#[test]
fn the_offer_and_the_request_scan_the_same_window() {
    let sectors = 512 * MIB_SECTORS;
    let run = Run::with_large_partition_4("scan-same-window", sectors);
    run.exfat_at(PLANNED + 300 * MIB_SECTORS);
    let offer = run.boot(&[]);
    offer.assert_no_offer(Some(&format!(
        "an exFAT boot sector starts at sector {} of the disk",
        PLANNED + 300 * MIB_SECTORS
    )));
    run.write_request(&run.sda4(), PLANNED, sectors, "7");
    run.boot(&[]).assert_request_refused(&format!(
        "an exFAT boot sector starts at sector {} of the disk",
        PLANNED + 300 * MIB_SECTORS
    ));
}

impl Run {
    /// A disk whose last partition is an empty partition 4 of type 7 at
    /// `start` with `sectors`, in the table and in the kernel, with no
    /// marker. The disk ends with partition 4.
    fn with_partition_4_at(name: &str, start: u64, sectors: u64) -> Self {
        let run = Run::new(name);
        std::fs::write(
            run.path("sys/devices/blk/sda/size"),
            format!("{}\n", start + sectors),
        )
        .unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(run.path("dev/sda"))
            .unwrap()
            .set_len((start + sectors) * 512)
            .unwrap();
        run.table_partition_4(start, sectors, "7");
        run.add_partition(4, start, sectors);
        run
    }
}

/// The space between the planned start and a partition 4 far after it
/// would cost minutes of scan, and a share there could run into
/// partition 4: such a partition is neither offered nor formatted.
#[test]
fn a_partition_4_far_past_the_planned_start_cannot_be_checked() {
    let gap = 4096 * MIB_SECTORS + MIB_SECTORS;
    let start = PLANNED + gap;
    let run = Run::with_partition_4_at("gap-too-large", start, 64 * MIB_SECTORS)
        .set("FAKE_DD_LOG", "yes");
    let offer = run.boot(&[]);
    offer.assert_no_offer(Some(&format!(
        "it cannot be checked, because it starts {gap} sectors after the planned start of the share, more than 8388608"
    )));
    run.write_request(&run.sda4(), start, 64 * MIB_SECTORS, "7");
    let request = run.boot(&[]);
    request.assert_request_refused("it cannot be checked");
    for out in [&offer, &request] {
        assert_eq!(
            out.log
                .lines()
                .filter(|l| l.starts_with("dd ") && l.contains("bs=1 "))
                .count(),
            0,
            "no scan runs: {out:#?}"
        );
    }
}

/// Within the bound, the scan covers the whole space before partition 4.
#[test]
fn an_old_share_in_the_gap_before_partition_4_is_found() {
    let start = PLANNED + 300 * MIB_SECTORS;
    let run = Run::with_partition_4_at("gap-share", start, 64 * MIB_SECTORS);
    run.exfat_at(start - MIB_SECTORS);
    run.start().assert_no_offer(Some(&format!(
        "an exFAT boot sector starts at sector {} of the disk",
        start - MIB_SECTORS
    )));
}

#[test]
fn the_request_scan_ends_after_4_gib_of_partition_4() {
    let sectors = 4097 * MIB_SECTORS;
    for (name, at, refused) in [
        ("scan-request-last", PLANNED + 4095 * MIB_SECTORS, true),
        ("scan-request-after", PLANNED + 4096 * MIB_SECTORS, false),
    ] {
        let run = Run::with_large_partition_4(name, sectors);
        run.exfat_at(at);
        run.write_request(&run.sda4(), PLANNED, sectors, "7");
        let out = run.boot(&[]);
        if refused {
            out.assert_request_refused(&format!("starts at sector {at} of the disk"));
        } else {
            assert_eq!(out.calls("mkfs.exfat"), 1, "{name}: {out:#?}");
        }
    }
}

/// One process reads each step of the scan: the boot cost is the number
/// of steps.
#[test]
fn a_boot_name_is_one_dd_that_seeks_to_its_byte() {
    let text = script();
    let body: String = text
        .split("boot_name() {")
        .nth(1)
        .and_then(|rest| rest.split("\n}").next())
        .expect("boot_name")
        .to_owned();
    assert_eq!(body.matches("dd ").count(), 1, "{body}");
    assert!(
        body.contains("bs=1 skip=$(($2 * 512 + 3)) count=8"),
        "{body}"
    );
}

impl Run {
    /// Writes `bytes` at byte `at` of the partition 4 device.
    fn write_sda4(&self, at: u64, bytes: &[u8]) {
        use std::io::{Seek, SeekFrom, Write};
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.path("dev/sda4"))
            .unwrap();
        file.seek(SeekFrom::Start(at)).unwrap();
        file.write_all(bytes).unwrap();
    }
}

/// A file system whose head is gone keeps backup copies of its header
/// further in; each one stops the offer.
#[test]
fn an_ext_backup_superblock_stops_the_offer() {
    // The primary superblock, and the backup of block group 1 for blocks of
    // 1, 2 and 4 KiB.
    for at in [1024_u64, 8_389_632, 33_554_432, 134_217_728] {
        let run = Run::with_large_partition_4(&format!("no-offer-ext-{at}"), 512 * MIB_SECTORS);
        run.write_sda4(at + 56, &[0x53, 0xef]);
        run.start().assert_no_offer(Some(&format!(
            "it holds an ext2, ext3 or ext4 superblock at byte {at}"
        )));
    }
}

#[test]
fn a_fat32_backup_boot_sector_stops_the_offer() {
    let run = Run::new("no-offer-fat32").with_partition_4();
    run.write_sda4(6 * 512 + 82, b"FAT32   ");
    run.start()
        .assert_no_offer(Some("it holds a FAT32 backup boot sector (sector 6)"));
}

#[test]
fn an_ntfs_backup_boot_sector_stops_the_offer() {
    for sector_bytes in [512_u64, 4096] {
        let run = Run::new(&format!("no-offer-ntfs-{sector_bytes}")).with_partition_4();
        run.write_sda4(FREE_SECTORS * 512 - sector_bytes + 3, b"NTFS    ");
        run.start().assert_no_offer(Some(&format!(
            "it holds an NTFS backup boot sector in its last {sector_bytes} bytes"
        )));
    }
}

#[test]
fn a_btrfs_superblock_mirror_stops_the_offer() {
    let run = Run::new("no-offer-btrfs").with_partition_4();
    run.write_sda4(64 * 1024 * 1024 + 64, b"_BHRfS_M");
    run.start()
        .assert_no_offer(Some("it holds a btrfs superblock mirror at 64 MiB"));
}

/// sfdisk --part-type writes the whole MBR sector: a change of anything
/// but the type of partition 4 stops the format.
#[test]
fn a_type_change_that_changes_another_entry_formats_nothing() {
    for (name, also) in [
        (
            "retype-other-entry",
            "s/size=        1024, type=c/size=        2048, type=c/",
        ),
        (
            "retype-entry-3",
            "s/size=        6144, type=83/size=        6144, type=82/",
        ),
    ] {
        let run = Run::empty_partition_4(name, PLANNED, "83").set("FAKE_RETYPE_ALSO", also);
        run.request_planned("83");
        let out = run.start();
        out.assert_request_refused(
            "the partition table changed in more than the type of partition 4",
        );
    }
}

/// After a type change that failed its read back, the normal rules must
/// not go on: with a marker of the planned partition and type 7 now in
/// the table, they would format under a refused request.
#[test]
fn a_refused_type_change_stops_the_normal_rules() {
    let run = marked_partition_4("retype-then-rules", PLANNED, "83").set(
        "FAKE_RETYPE_ALSO",
        "s/size=        1024, type=c/size=        2048, type=c/",
    );
    run.request_planned("83");
    let out = run.start();
    out.assert_request_refused("the partition table changed in more than the type of partition 4");
    out.assert_share_ram("the partition table changed in more than the type of partition 4");
}

#[test]
fn a_partition_4_past_the_end_of_the_disk_is_never_offered_a_format() {
    let run = Run::new("no-offer-past-end");
    run.table_partition_4(PLANNED, FREE_SECTORS + 2048, "7");
    run.add_partition(4, PLANNED, FREE_SECTORS + 2048);
    run.start()
        .assert_no_offer(Some("it ends after the end of the disk"));
}

/// Partition 4 must overlap no other entry of the table, not only the
/// state partition.
#[test]
fn a_partition_4_that_overlaps_another_entry_is_never_offered_a_format() {
    let run = Run::new("no-offer-overlap-1").with_partition_4();
    let table = std::fs::read_to_string(run.path("table")).unwrap().replace(
        "start=        2048, size=        1024, type=c",
        &format!("start={:>12}, size=        1024, type=c", PLANNED + 4096),
    );
    std::fs::write(run.path("table"), table).unwrap();
    let sda1 = format!("{}/dev/sda1", run.dir.display());
    run.start()
        .assert_no_offer(Some(&format!("it overlaps {sda1} of the partition table")));
}

/// Linux refuses a seek past the end of a block device, and dd then reads
/// every byte up to the offset: at boot that never ends. No read of the
/// offer or of the request may start or end past its device.
#[test]
fn no_read_goes_past_the_end_of_its_device() {
    let sectors = 100 * MIB_SECTORS;
    let run = Run::with_large_partition_4("reads-in-bounds", sectors).set("FAKE_DD_LOG", "yes");
    let offer = run.boot(&[]);
    assert!(offer.offer().is_some(), "{offer:#?}");
    run.write_request(&run.sda4(), PLANNED, sectors, "7");
    let request = run.boot(&[]);
    assert_eq!(request.calls("mkfs.exfat"), 1, "{request:#?}");
    let sda = format!("{}/dev/sda", run.dir.display());
    let sda4 = run.sda4();
    let mut reads = 0;
    for out in [&offer, &request] {
        for line in out.log.lines().filter(|l| l.starts_with("dd ")) {
            let arg = |key: &str| {
                line.split(' ')
                    .find_map(|w| w.strip_prefix(key))
                    .map(str::to_owned)
            };
            let (Some(input), Some(skip), Some(count)) = (arg("if="), arg("skip="), arg("count="))
            else {
                continue;
            };
            let bs: u64 = arg("bs=").and_then(|b| b.parse().ok()).expect(line);
            let end = (skip.parse::<u64>().unwrap() + count.parse::<u64>().unwrap()) * bs;
            let size = if input == sda {
                (PLANNED + sectors) * 512
            } else if input == sda4 {
                sectors * 512
            } else {
                continue;
            };
            assert!(
                end <= size,
                "{line}: ends at {end}, the device has {size} bytes"
            );
            reads += 1;
        }
    }
    assert!(reads > 50, "the scan and the checks read: {reads}");
}

/// The marker of the planned partition opens the normal format rules. A
/// type change that a refused request left must not open them with fewer
/// checks than the request: they run the same checks.
#[test]
fn the_normal_rules_check_what_a_request_checks() {
    let ext = marked_partition_4("rules-ext", PLANNED, "7");
    ext.write_sda4(8_389_632 + 56, &[0x53, 0xef]);
    let out = ext.start();
    out.assert_share_ram(
        "was left untouched: it holds an ext2, ext3 or ext4 superblock at byte 8389632",
    );
    assert!(out.marker, "{out:#?}");

    let inside = marked_partition_4("rules-inside", PLANNED, "7");
    inside.exfat_at(PLANNED + 4096);
    inside.start().assert_share_ram(&format!(
        "was left untouched: an exFAT boot sector starts at sector {} of the disk",
        PLANNED + 4096
    ));
}

/// The first boot scans the free space once: the scan before the
/// partition is made covers the window of the format checks.
#[test]
fn the_first_boot_scans_the_free_space_once() {
    let out = Run::new("first-boot-one-scan")
        .set("FAKE_DD_LOG", "yes")
        .start();
    out.assert_share_mounted(true);
    let steps = out
        .log
        .lines()
        .filter(|l| l.starts_with("dd ") && l.contains("/dev/sda bs=1 ") && l.ends_with("count=8"))
        .count();
    assert_eq!(steps as u64, FREE_SECTORS / MIB_SECTORS, "{out:#?}");
}

/// sfdisk --part-type writes the whole MBR sector: the boot code in bytes
/// 0 to 439 must read back the same.
#[test]
fn a_type_change_that_changes_the_boot_code_formats_nothing() {
    let run = Run::empty_partition_4("retype-boot-code", PLANNED, "83")
        .set("FAKE_RETYPE_BOOT_CODE", "yes");
    run.request_planned("83");
    let out = run.start();
    out.assert_request_refused("the boot code of the MBR (bytes 0 to 439) changed");
    out.assert_share_ram("the boot code of the MBR (bytes 0 to 439) changed");
}
