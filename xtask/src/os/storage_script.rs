//! Tests of `S06neptron-storage`, the init script that mounts the state and
//! the share partition. The script runs on the host with every path below
//! a test folder (`NEPTRON_STORAGE_ROOT`): a fake sysfs, a fake disk file
//! and fake commands for every program that touches a device or the kernel.
//! The fakes write their calls to a log; the tests read the log and the
//! status file `run/neptron/storage.toml`.

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

/// The fake disk: partition 3 (state) ends at sector 8192, so the planned
/// start of partition 4 is 8192, and 128 MiB of free space follow.
const STATE_START: u64 = 4096;
const STATE_SECTORS: u64 = 4096;
const PLANNED: u64 = 8192;
const FREE_SECTORS: u64 = 262_144;
const DISK_SECTORS: u64 = PLANNED + FREE_SECTORS;

/// The fake commands. Each reads its behaviour from `FAKE_*` variables.
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
    ("logger", "echo \"logger $*\" >> \"$LOG\""),
    ("sync", "echo sync >> \"$LOG\""),
    ("chown", "echo \"chown $*\" >> \"$LOG\""),
    ("date", "echo DATE"),
    ("mkfs.exfat", "echo \"mkfs.exfat $*\" >> \"$LOG\""),
    (
        "blkid",
        "if [ -n \"$FAKE_BLKID_ERROR\" ]; then echo \"$FAKE_BLKID_ERROR\" >&2; exit 4; fi\n\
         if [ -n \"$FAKE_BLKID\" ]; then echo \"$FAKE_BLKID\"; exit 0; fi\n\
         exit 2",
    ),
    (
        "usleep",
        "echo \"usleep $*\" >> \"$LOG\"\n\
         n=$(($(cat \"$NEPTRON_STORAGE_ROOT/usleeps\" 2>/dev/null || echo 0) + 1))\n\
         echo \"$n\" > \"$NEPTRON_STORAGE_ROOT/usleeps\"\n\
         if [ -n \"$FAKE_NODE_AT\" ] && [ \"$n\" -ge \"$FAKE_NODE_AT\" ]; then\n\
         \t: > \"$NEPTRON_STORAGE_ROOT/dev/sda4\"\n\
         fi",
    ),
    (
        "sfdisk",
        "R=$NEPTRON_STORAGE_ROOT\n\
         case \" $* \" in\n\
         *' --dump '*) cat \"$R/table\"; exit 0 ;;\n\
         *' --part-type '*) grep -q '4 : .*type=7' \"$R/table\" && echo ' 7'; exit 0 ;;\n\
         esac\n\
         cat > /dev/null\n\
         echo \"sfdisk $*\" >> \"$LOG\"\n\
         if [ \"${FAKE_SFDISK_WRITES:-yes}\" = yes ]; then\n\
         \tprintf '%s/dev/sda4 : start=%12s, size=%12s, type=%s\\n' \"$R\" \
         \"${FAKE_P4_START:-8192}\" \"${FAKE_P4_SECTORS:-262144}\" \"${FAKE_P4_TYPE:-7}\" >> \"$R/table\"\n\
         fi\n\
         if [ -n \"$FAKE_SFDISK_SAYS\" ]; then echo \"$FAKE_SFDISK_SAYS\" >&2; fi\n\
         exit \"${FAKE_SFDISK_EXIT:-0}\"",
    ),
    (
        "partx",
        "R=$NEPTRON_STORAGE_ROOT\n\
         echo \"partx $*\" >> \"$LOG\"\n\
         n=$(($(cat \"$R/partx-calls\" 2>/dev/null || echo 0) + 1))\n\
         echo \"$n\" > \"$R/partx-calls\"\n\
         if [ \"$n\" -le \"${FAKE_PARTX_FAILS:-0}\" ]; then\n\
         \techo 'partx: /dev/sda: error adding partition 4' >&2; exit 1\n\
         fi\n\
         if [ \"${FAKE_PARTX_ADDS:-yes}\" != yes ]; then exit 0; fi\n\
         add_partition 4 \"${FAKE_P4_START:-8192}\" \"${FAKE_P4_SECTORS:-262144}\"\n\
         if [ -z \"$FAKE_NODE_AT\" ]; then : > \"$R/dev/sda4\"; fi",
    ),
];

/// Adds partition `$1` (start `$2`, size `$3`) to the fake sysfs, as the
/// kernel does. The fake partx and the test setup share it.
const ADD_PARTITION: &str = "#!/bin/sh\n\
    R=$NEPTRON_STORAGE_ROOT\n\
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
    /// state partition first, and free space after it.
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
        std::fs::create_dir_all(run.path("dev")).unwrap();
        std::fs::create_dir_all(run.path("sys/dev/block")).unwrap();
        std::fs::create_dir_all(run.path("usr/share/neptron/systems")).unwrap();
        std::fs::write(run.path("usr/share/neptron/systems/nes.toml"), "").unwrap();
        std::fs::write(
            run.path("sys/devices/blk/sda/size").tap_dir(),
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
                 {root}/dev/sda3 : start=        4096, size=        4096, type=83\n"
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
            .env("NEPTRON_STORAGE_ROOT", &self.dir)
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

    /// Runs the script with `start` and returns what it did.
    fn start(self) -> Outcome {
        let path = format!(
            "{}:{}",
            self.path("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let out = Command::new("sh")
            .arg(script_path())
            .arg("start")
            .env("PATH", path)
            .env("LOG", self.path("log"))
            .env("NEPTRON_STORAGE_ROOT", &self.dir)
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let read = |rel: &str| std::fs::read_to_string(self.path(rel)).unwrap_or_default();
        let outcome = Outcome {
            ok: out.status.success(),
            console: String::from_utf8_lossy(&out.stdout).into_owned()
                + &String::from_utf8_lossy(&out.stderr),
            log: read("log"),
            status: read("run/neptron/storage.toml"),
            marker: self.path("state/share-pending").exists(),
        };
        // A folder left in the system temp folder does no harm.
        let _ = std::fs::remove_dir_all(&self.dir);
        outcome
    }
}

/// What a run of the script did.
#[derive(Debug)]
struct Outcome {
    ok: bool,
    console: String,
    log: String,
    status: String,
    marker: bool,
}

impl Outcome {
    fn calls(&self, prefix: &str) -> usize {
        self.log.lines().filter(|l| l.starts_with(prefix)).count()
    }

    /// The share partition is mounted and no format ran.
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
    }

    /// `/share` is a RAM folder with `reason` in the status file, and the
    /// reason is in syslog and on the console.
    fn assert_share_ram(&self, reason: &str) {
        assert!(self.ok, "{self:#?}");
        let share_reason = self
            .status
            .lines()
            .find_map(|l| l.strip_prefix("share_reason = \""))
            .and_then(|l| l.strip_suffix('"'))
            .unwrap_or_else(|| panic!("no share reason: {self:#?}"));
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
        assert_eq!(self.calls("mount -t exfat"), 0, "{self:#?}");
        assert_eq!(self.calls("mkfs.exfat"), 0, "{self:#?}");
        assert_eq!(
            self.calls("mount -t tmpfs"),
            1 + self.status.matches("state = \"ram\"").count(),
            "{self:#?}"
        );
    }
}

trait TapDir {
    /// Makes the parent folder of the path and returns the path.
    fn tap_dir(self) -> Self;
}

impl TapDir for PathBuf {
    fn tap_dir(self) -> Self {
        std::fs::create_dir_all(self.parent().unwrap()).unwrap();
        self
    }
}

fn write_exec(path: &Path, text: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn the_script_is_posix_sh_and_starts_nothing_on_other_actions() {
    for shell in ["sh", "dash"] {
        let Ok(out) = Command::new(shell).arg("-n").arg(script_path()).output() else {
            continue;
        };
        assert!(out.status.success(), "{shell}: {out:?}");
    }
    let run = Run::new("stop");
    let path = run.path("bin");
    let out = Command::new("sh")
        .arg(script_path())
        .arg("stop")
        .env("PATH", format!("{}:/usr/bin:/bin", path.display()))
        .env("LOG", run.path("log"))
        .env("NEPTRON_STORAGE_ROOT", &run.dir)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(!run.path("log").exists());
    let _ = std::fs::remove_dir_all(&run.dir);
}

/// sfdisk never wipes a signature and never asks: the free space can hold
/// the share of an older image. Every line goes to syslog.
#[test]
fn sfdisk_wipes_nothing_and_every_line_goes_to_syslog() {
    let text = script();
    let sfdisk: String = text
        .split("output=$(echo ',,7' | sfdisk ")
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
    assert_eq!(out.calls("partx --add --nr 4"), 1, "{out:#?}");
    assert!(
        out.log
            .contains("logger -t neptron-storage making partition 4 at sector 8192"),
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
        out.log
            .contains("holds partition 4 as planned (8192 262144 7), sfdisk exit 1"),
        "{out:#?}"
    );
}

#[test]
fn sfdisk_that_writes_nothing_leaves_a_ram_folder_with_its_reason() {
    let out = Run::new("sfdisk-fail")
        .set("FAKE_SFDISK_WRITES", "no")
        .set("FAKE_SFDISK_EXIT", "1")
        .start();
    out.assert_share_ram("sfdisk exit 1, and the partition table has no partition 4");
    assert!(!out.marker, "no partition, no marker: {out:#?}");
    assert_eq!(out.calls("partx"), 0, "{out:#?}");
}

#[test]
fn a_partition_4_other_than_planned_is_left_untouched() {
    for (name, start, kind) in [("start", PLANNED + 2048, "7"), ("type", PLANNED, "83")] {
        let out = Run::new(&format!("plan-{name}"))
            .set("FAKE_P4_START", &start.to_string())
            .set("FAKE_P4_TYPE", kind)
            .start();
        out.assert_share_ram(&format!(
            "sfdisk exit 0, and the table holds partition 4 at sector {start} with 262144 sectors and type {kind}, not type 7 at sector 8192"
        ));
        assert!(!out.marker, "{name}: {out:#?}");
        assert_eq!(out.calls("partx"), 0, "{name}: {out:#?}");
    }
}

#[test]
fn partx_failing_once_is_tried_again() {
    let out = Run::new("partx-once").set("FAKE_PARTX_FAILS", "1").start();
    out.assert_share_mounted(true);
    assert_eq!(out.calls("partx --add --nr 4"), 2, "{out:#?}");
    assert_eq!(out.calls("usleep 500000"), 1, "{out:#?}");
    assert!(
        out.log.contains(
            "logger -t neptron-storage partx try 1 of 5, exit 1: the kernel does not see partition 4: partx: /dev/sda: error adding partition 4"
        ),
        "{out:#?}"
    );
}

/// partx can exit with 0 and add nothing: only the kernel's list counts.
#[test]
fn a_partition_the_kernel_never_sees_is_named_and_not_silent() {
    let out = Run::new("partx-silent")
        .set("FAKE_PARTX_ADDS", "no")
        .start();
    out.assert_share_ram(
        "partition 4 is in the partition table, but the kernel does not see it after 5 tries of partx (last exit 0); restart to use it",
    );
    assert_eq!(out.calls("partx --add --nr 4"), 5, "{out:#?}");
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
    out.assert_share_ram("the kernel sees partition 4, but its device node");
    assert!(
        out.status.contains("does not appear within 5000 ms"),
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
    assert_eq!(out.calls("partx --add --nr 4"), 1, "{out:#?}");
}

#[test]
fn a_partition_4_the_kernel_sees_is_mounted_at_once() {
    let run = Run::new("known").set("FAKE_BLKID", "TYPE=exfat");
    run.table_partition_4(PLANNED, FREE_SECTORS, "7");
    run.add_partition(4, PLANNED, FREE_SECTORS);
    let out = run.start();
    out.assert_share_mounted(false);
    assert_eq!(out.calls("sfdisk"), 0, "{out:#?}");
    assert_eq!(out.calls("partx"), 0, "{out:#?}");
    assert_eq!(out.calls("usleep"), 0, "{out:#?}");
}

#[test]
fn every_way_to_a_ram_share_names_its_reason() {
    let small = Run::new("small");
    std::fs::write(
        small.path("sys/devices/blk/sda/size"),
        format!("{}\n", PLANNED + 1000),
    )
    .unwrap();
    small
        .start()
        .assert_share_ram("1000 free sectors after the state partition, less than 131072");

    let other = Run::new("old-elsewhere");
    other.exfat_at(PLANNED + 4096);
    let out = other.start();
    out.assert_share_ram("an old exFAT share starts at sector 12288, not at 8192");
    assert_eq!(out.calls("sfdisk"), 0, "{out:#?}");

    let no_state = Run::new("no-state");
    std::fs::remove_dir_all(no_state.path("sys/devices/blk/sda/sda3")).unwrap();
    let out = no_state.start();
    out.assert_share_ram("the boot disk has no partition 3, so no share partition is made");
    assert!(
        out.status
            .contains("state_reason = \"the boot disk has no partition 3"),
        "{out:#?}"
    );

    let whole = Run::new("whole-disk");
    std::fs::remove_file(whole.path("sys/devices/blk/sda/sda2/partition")).unwrap();
    whole
        .start()
        .assert_share_ram("the root filesystem is not on a partition of a disk");

    let unmounted = Run::new("state-unmounted").set("FAKE_MOUNT_EXT4_EXIT", "32");
    let out = unmounted.start();
    out.assert_share_ram("the state partition is not mounted, so no share partition is made");
    assert!(
        out.status
            .contains("does not mount; /state is a RAM folder"),
        "{out:#?}"
    );

    let broken = Run::new("state-errors").set("FAKE_E2FSCK_EXIT", "4");
    let out = broken.start();
    out.assert_share_ram("the state partition is not mounted");
    assert!(out.status.contains("left errors (exit 4)"), "{out:#?}");

    let no_mount = Run::new("exfat-mount")
        .set("FAKE_BLKID", "TYPE=exfat")
        .set("FAKE_MOUNT_EXFAT_EXIT", "32");
    no_mount.exfat_at(PLANNED);
    let out = no_mount.start();
    assert!(out.status.contains("share_reason = \""), "{out:#?}");
    assert!(out.status.contains("sda4 does not mount\""), "{out:#?}");
    assert!(out.log.contains("logger -t neptron-storage "), "{out:#?}");

    let unclean = Run::new("unclean").set("FAKE_BLKID_ERROR", "blkid: read error");
    unclean.add_partition(4, PLANNED, FREE_SECTORS);
    unclean
        .start()
        .assert_share_ram("was left untouched: blkid could not probe it cleanly");

    let foreign = Run::new("no-marker");
    foreign.table_partition_4(PLANNED, FREE_SECTORS, "7");
    foreign.add_partition(4, PLANNED, FREE_SECTORS);
    foreign
        .start()
        .assert_share_ram("this system did not make it (no /state/share-pending)");

    let damaged = Run::new("backup-boot-sector");
    damaged.table_partition_4(PLANNED, FREE_SECTORS, "7");
    damaged.add_partition(4, PLANNED, FREE_SECTORS);
    std::fs::create_dir_all(damaged.path("state")).unwrap();
    std::fs::write(damaged.path("state/share-pending"), "partition 4 made\n").unwrap();
    let mut sector = vec![0_u8; 13 * 512];
    sector[12 * 512 + 3..12 * 512 + 11].copy_from_slice(b"EXFAT   ");
    std::fs::write(damaged.path("dev/sda4"), sector).unwrap();
    damaged
        .start()
        .assert_share_ram("it holds an exFAT backup boot sector (sector 12)");
}

/// The state partition part names its reasons the same way.
#[test]
fn every_way_to_a_ram_state_names_its_reason_in_syslog() {
    for (name, var, value, reason) in [
        (
            "e2fsck",
            "FAKE_E2FSCK_EXIT",
            "8",
            "left errors (exit 8); /state is a RAM folder",
        ),
        (
            "ext4",
            "FAKE_MOUNT_EXT4_EXIT",
            "32",
            "sda3 does not mount; /state is a RAM folder",
        ),
    ] {
        let out = Run::new(&format!("state-{name}")).set(var, value).start();
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
    }
    let out = Run::new("state-ok").start();
    assert!(
        out.log.contains("logger -t neptron-storage ")
            && out.log.contains("sda3 mounted at /state (e2fsck exit 0)"),
        "{out:#?}"
    );
}
