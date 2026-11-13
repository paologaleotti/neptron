//! Tests of the network share files in `os/`: the SMB server
//! configuration, the init scripts of the network and the share, the mount
//! options of the share partition, and the kernel options of both images.
//! `docs/spec/network-share.md` is the spec.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::Arch;
use super::defconfig::parse;

fn os_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../os")
}

fn read(rel: &str) -> String {
    let path = os_dir().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn defconfig(arch: Arch) -> BTreeMap<String, String> {
    parse(&read(&format!("configs/{}", arch.defconfig())))
}

const OVERLAY: &str = "board/common/rootfs-overlay";
const SHARE_SCRIPT: &str = "board/common/rootfs-overlay/etc/init.d/S45neptron-share";
const NETWORK_SCRIPT: &str = "board/common/rootfs-overlay/etc/init.d/S40neptron-network";
const STORAGE_SCRIPT: &str = "board/common/rootfs-overlay/etc/init.d/S06neptron-storage";

/// The sections of a ksmbd.conf file: section name to its parameters, with
/// the names in lower case. Lines that start with `;` or `#` are comments.
fn ksmbd_sections(text: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut current = String::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            current = name.to_lowercase();
            sections.entry(current.clone()).or_default();
        } else if let Some((key, value)) = line.split_once('=') {
            let previous = sections
                .entry(current.clone())
                .or_default()
                .insert(key.trim().to_lowercase(), value.trim().to_owned());
            assert!(previous.is_none(), "{key} twice in [{current}]");
        } else {
            panic!("not a ksmbd.conf line: {line}");
        }
    }
    sections
}

#[test]
fn no_variant_starts_the_ifup_dhcp_that_waits_at_boot() {
    for rel in [
        "configs/neptron_pc_x86_64_defconfig",
        "configs/neptron_qemu_aarch64_defconfig",
        "configs/neptron_pc_x86_64_dev.fragment",
        "configs/neptron_pc_x86_64_no15khz.fragment",
    ] {
        assert!(
            !parse(&read(rel)).contains_key("BR2_SYSTEM_DHCP"),
            "{rel}: S40neptron-network starts DHCP"
        );
    }
}

#[test]
fn the_server_shares_only_the_share_partition_to_guests_over_smb2_or_newer() {
    let sections = ksmbd_sections(&read(&format!("{OVERLAY}/etc/ksmbd/ksmbd.conf")));
    assert_eq!(
        sections.keys().map(String::as_str).collect::<Vec<_>>(),
        ["global", "share"],
        "one share, named share"
    );
    let global = &sections["global"];
    for (key, value) in [
        ("netbios name", "NEPTRON"),
        ("server min protocol", "SMB2_10"),
        ("map to guest", "bad user"),
        ("guest account", "nobody"),
        ("bind interfaces only", "no"),
        ("max connections", "16"),
    ] {
        assert_eq!(global.get(key).map(String::as_str), Some(value), "{key}");
    }
    let share = &sections["share"];
    for (key, value) in [
        ("path", "/share"),
        ("guest ok", "yes"),
        ("read only", "no"),
        ("store dos attributes", "no"),
    ] {
        assert_eq!(share.get(key).map(String::as_str), Some(value), "{key}");
    }
    for (name, params) in &sections {
        for key in [
            "wide links",
            "follow symlinks",
            "root directory",
            "force user",
            "force group",
            "interfaces",
            "server max protocol",
        ] {
            assert!(!params.contains_key(key), "[{name}] sets {key}");
        }
    }
    assert!(
        !os_dir()
            .join(OVERLAY)
            .join("etc/ksmbd/ksmbdpwd.db")
            .exists(),
        "no user database: every client is a guest"
    );
    let subauth = read(&format!("{OVERLAY}/etc/ksmbd/ksmbd.subauth"));
    let fields: Vec<&str> = subauth.trim_end_matches('\n').split(':').collect();
    assert_eq!(fields.len(), 3, "{subauth}");
    for field in fields {
        assert!(
            !field.is_empty() && field.parse::<u32>().is_ok(),
            "{subauth}"
        );
    }
    assert_eq!(
        read(&format!("{OVERLAY}/etc/neptron/mdns.d/smb.service")),
        "type _smb._tcp\nport 445\n"
    );
}

#[test]
fn both_images_and_both_variants_have_the_share() {
    for arch in [Arch::Aarch64, Arch::X86_64] {
        let options = defconfig(arch);
        for name in ["BR2_PACKAGE_KSMBD_TOOLS", "BR2_PACKAGE_MDNSD"] {
            assert_eq!(
                options.get(name).map(String::as_str),
                Some("y"),
                "{name} {arch:?}"
            );
        }
        assert!(
            options["BR2_ROOTFS_POST_BUILD_SCRIPT"]
                .contains("$(BR2_EXTERNAL_NEPTRON_PATH)/board/common/post-build.sh"),
            "{arch:?} does not remove the init script of mdnsd"
        );
        assert!(
            options["BR2_ROOTFS_OVERLAY"]
                .starts_with("$(BR2_EXTERNAL_NEPTRON_PATH)/board/common/rootfs-overlay"),
            "{arch:?}"
        );
    }
    for rel in [
        "configs/neptron_pc_x86_64_dev.fragment",
        "configs/neptron_pc_x86_64_no15khz.fragment",
    ] {
        let fragment = parse(&read(rel));
        for name in [
            "BR2_PACKAGE_KSMBD_TOOLS",
            "BR2_PACKAGE_MDNSD",
            "BR2_ROOTFS_POST_BUILD_SCRIPT",
        ] {
            assert!(!fragment.contains_key(name), "{rel} changes {name}");
        }
        if let Some(overlay) = fragment.get("BR2_ROOTFS_OVERLAY") {
            assert!(overlay.contains("board/common/rootfs-overlay"), "{rel}");
        }
    }
    let post_build = read("board/common/post-build.sh");
    assert!(post_build.contains("rm -f \"$target/etc/init.d/S50mdnsd\""));
    for path in [
        "etc/init.d/S45neptron-share",
        "etc/ksmbd/ksmbd.conf",
        "etc/ksmbd/ksmbd.subauth",
        "etc/neptron/mdns.d/smb.service",
    ] {
        assert!(post_build.contains(path), "{path}");
        assert!(os_dir().join(OVERLAY).join(path).is_file(), "{path}");
    }
}

#[test]
fn both_kernels_build_in_the_smb_server_with_no_smb1_rdma_or_kerberos() {
    let pc = &defconfig(Arch::X86_64)["BR2_LINUX_KERNEL_CONFIG_FRAGMENT_FILES"];
    let qemu = &defconfig(Arch::Aarch64)["BR2_LINUX_KERNEL_CONFIG_FRAGMENT_FILES"];
    for (fragments, rel) in [
        (pc, "board/pc/network.config"),
        (qemu, "board/qemu/network.config"),
    ] {
        assert!(fragments.contains(rel), "{rel} is not in the defconfig");
        let options = parse(&read(rel));
        assert_eq!(options["CONFIG_SMB_SERVER"], "y", "{rel}");
        assert_eq!(options["CONFIG_NETWORK_FILESYSTEMS"], "y", "{rel}");
        for off in [
            "CONFIG_SMB_INSECURE_SERVER",
            "CONFIG_SMB_SERVER_SMBDIRECT",
            "CONFIG_SMB_SERVER_KERBEROS5",
        ] {
            assert_eq!(options[off], "n", "{rel} {off}");
        }
    }
    let check = read("board/pc/check-kernel-config.sh");
    for line in [
        "SHARE_OFF=\"SMB_INSECURE_SERVER SMB_SERVER_SMBDIRECT SMB_SERVER_KERBEROS5\"",
        "SHARE_ON=\"SMB_SERVER\"",
    ] {
        assert!(check.lines().any(|l| l == line), "the check misses {line}");
    }
}

#[test]
fn the_share_partition_belongs_to_the_guest_and_runs_nothing() {
    let storage = read(STORAGE_SCRIPT);
    let value = |name: &str| {
        storage
            .lines()
            .find_map(|l| l.strip_prefix(&format!("{name}=")))
            .unwrap_or_else(|| panic!("no {name}"))
            .trim_matches('"')
            .to_owned()
    };
    assert_eq!(value("SHARE_OWNER"), "65534", "the user nobody");
    let partition: Vec<String> = value("SHARE_MOUNT_OPTIONS")
        .replace("$SHARE_OWNER", "65534")
        .split(',')
        .map(str::to_owned)
        .collect();
    let ram: Vec<String> = value("SHARE_RAM_OPTIONS")
        .replace("$SHARE_OWNER", "65534")
        .split(',')
        .map(str::to_owned)
        .collect();
    for options in [&partition, &ram] {
        for option in ["uid=65534", "gid=65534", "nosuid", "nodev", "noexec"] {
            assert!(options.iter().any(|o| o == option), "{option}: {options:?}");
        }
    }
    for option in ["iocharset=utf8", "dmask=0022", "fmask=0133"] {
        assert!(partition.iter().any(|o| o == option), "{option}");
    }
    assert!(storage.contains("mount -t exfat -o \"$SHARE_MOUNT_OPTIONS\" \"$share\" /share"));
    assert!(storage.contains("[ \"$1\" = /share ] && options=$SHARE_RAM_OPTIONS"));
    assert!(storage.contains("chown -R \"$SHARE_OWNER:$SHARE_OWNER\" /share"));
    let passwd_owner = read(&format!("{OVERLAY}/etc/ksmbd/ksmbd.conf"));
    assert!(passwd_owner.contains("guest account = nobody"));
}

/// A folder that is removed when the test ends, also when it fails.
struct TempDir(PathBuf);

impl TempDir {
    fn create(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("xtask-share-{name}-{}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).unwrap();
        }
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }

    /// Writes an executable fake command to `bin/<name>`.
    fn fake(&self, name: &str, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        let path = self.path("bin").join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// Fakes that append their name and arguments to `log`.
    fn logging_fakes(&self, names: &[&str]) {
        for name in names {
            self.fake(name, &format!("echo \"{name} $*\" >> \"$LOG\""));
        }
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.path("log")).unwrap_or_default()
    }

    /// Runs `script` with `action`, the fakes first on the PATH, and the
    /// test paths in the environment; returns whether it succeeded and its
    /// output. The output goes to a file, not a pipe: a background job of
    /// the script keeps the console, and a pipe would wait for it.
    fn run(&self, script: &str, action: &str) -> (bool, String) {
        let path = format!(
            "{}:{}",
            self.path("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let out_path = self.path(&format!("{action}.out"));
        let out = std::fs::File::create(&out_path).unwrap();
        let child = Command::new("sh")
            .arg(os_dir().join(script))
            .arg(action)
            .env("PATH", path)
            .env("LOG", self.path("log"))
            .env("NEPTRON_STATE", self.path("state"))
            .env("NEPTRON_PROC_NET", self.path("net"))
            .env("NEPTRON_RUN", self.path("run"))
            .env("NEPTRON_SYS_NET", self.path("sys"))
            .stdin(Stdio::null())
            .stdout(out.try_clone().unwrap())
            .stderr(out)
            .spawn()
            .unwrap();
        let pid = child.id();
        let mut child = child;
        let status = bounded(&format!("{script} {action} to return"), move || {
            child.wait().unwrap()
        })
        .unwrap_or_else(|message| {
            // The script hangs: stop it, so the test ends.
            let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
            panic!("{message}")
        });
        (
            status.success(),
            std::fs::read_to_string(&out_path).unwrap(),
        )
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // A folder left in the system temp folder does no harm.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn mkfifo(path: &Path) {
    let status = Command::new("mkfifo").arg(path).status().unwrap();
    assert!(status.success());
}

/// How long a test waits for a script or a fake that it synchronises with
/// through a FIFO. A passing run takes milliseconds; the bound turns a
/// regression that blocks into a failure with a message.
const FIFO_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

/// Runs `work` on its own thread and returns its result, or a message when
/// it does not finish within [`FIFO_WAIT`]. The thread of a blocked open
/// stays; the test fails, and the process ends.
fn bounded<T: Send + 'static>(
    what: &str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        // The receiver is gone only after a timeout, which already failed.
        let _ = tx.send(work());
    });
    rx.recv_timeout(FIFO_WAIT)
        .map_err(|_| format!("no answer within {} s: {what}", FIFO_WAIT.as_secs()))
}

/// Reads a FIFO to its end; the open blocks until a writer opens it.
fn read_fifo(path: &Path) -> String {
    let owned = path.to_owned();
    bounded(&format!("a writer of {}", path.display()), move || {
        std::fs::read_to_string(owned).unwrap()
    })
    .unwrap_or_else(|message| panic!("{message}"))
}

/// Writes `text` to a FIFO; the open blocks until a reader opens it.
fn write_fifo(path: &Path, text: &'static str) {
    let owned = path.to_owned();
    bounded(&format!("a reader of {}", path.display()), move || {
        std::fs::write(owned, text).unwrap()
    })
    .unwrap_or_else(|message| panic!("{message}"))
}

#[test]
fn the_off_switch_keeps_the_server_and_mdns_down() {
    let dir = TempDir::create("off");
    dir.logging_fakes(&[
        "ksmbd.mountd",
        "start-stop-daemon",
        "ksmbd.control",
        "usleep",
        "sync",
    ]);
    std::fs::create_dir_all(dir.path("state")).unwrap();
    std::fs::write(dir.path("state/network-share-off"), "").unwrap();
    let (ok, out) = dir.run(SHARE_SCRIPT, "start");
    assert!(ok, "{out}");
    assert!(out.contains("network-share-off is there"), "{out}");
    assert_eq!(dir.log(), "", "nothing started");
}

#[test]
fn the_share_starts_in_the_background_and_never_delays_the_boot() {
    let dir = TempDir::create("start");
    let gate = dir.path("gate");
    let done = dir.path("done");
    mkfifo(&gate);
    mkfifo(&done);
    // The server blocks until the test opens the gate: the script must
    // return before that.
    dir.fake(
        "ksmbd.mountd",
        &format!(
            "read -r _ < \"{}\"; echo \"ksmbd.mountd $*\" >> \"$LOG\"",
            gate.display()
        ),
    );
    dir.fake(
        "start-stop-daemon",
        &format!(
            "echo \"start-stop-daemon $*\" >> \"$LOG\"; echo done > \"{}\"",
            done.display()
        ),
    );
    let (ok, out) = dir.run(SHARE_SCRIPT, "start");
    assert!(ok, "{out}");
    assert_eq!(
        dir.log(),
        "",
        "the script returned before the server started"
    );
    write_fifo(&gate, "go\n");
    assert_eq!(read_fifo(&done), "done\n");
    assert_eq!(
        dir.log(),
        "ksmbd.mountd \nstart-stop-daemon --start --quiet --exec /usr/sbin/mdnsd -- /etc/neptron/mdns.d\n"
    );
}

/// A line of /proc/net/tcp6 for a socket on local port 445 in `state`.
fn smb_socket(state: &str) -> String {
    format!(
        "   0: 00000000000000000000000000000000:01BD 00000000000000000000000000000000:0000 {state} 00000000:00000000 00:00000000 00000000     0        0 1 1 0000000000000000 100 0 0 10 0\n"
    )
}

const TCP_HEADER: &str = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n";

#[test]
fn stop_closes_the_server_waits_for_its_sockets_and_syncs() {
    for (name, tcp6, waits) in [
        ("closed", String::new(), 0),
        ("time-wait", smb_socket("06"), 0),
        ("listening", smb_socket("0A"), 50),
        ("established", smb_socket("01"), 50),
    ] {
        let dir = TempDir::create(&format!("stop-{name}"));
        dir.logging_fakes(&["start-stop-daemon", "ksmbd.control", "usleep", "sync"]);
        std::fs::create_dir_all(dir.path("net")).unwrap();
        std::fs::write(dir.path("net/tcp"), TCP_HEADER).unwrap();
        std::fs::write(dir.path("net/tcp6"), format!("{TCP_HEADER}{tcp6}")).unwrap();
        let (ok, out) = dir.run(SHARE_SCRIPT, "stop");
        assert!(ok, "{name}: {out}");
        let log = dir.log();
        let lines: Vec<&str> = log.lines().map(str::trim_end).collect();
        assert_eq!(
            lines.first().copied(),
            Some("start-stop-daemon --stop --quiet --exec /usr/sbin/mdnsd"),
            "{name}"
        );
        assert_eq!(
            lines.get(1).copied(),
            Some("ksmbd.control --shutdown"),
            "{name}"
        );
        assert_eq!(
            lines.iter().filter(|l| l.starts_with("usleep")).count(),
            waits,
            "{name}"
        );
        assert_eq!(
            lines.last().copied(),
            Some("sync"),
            "{name}: the data reaches the disk last"
        );
        assert_eq!(
            out.contains("still has sockets"),
            waits > 0,
            "{name}: {out}"
        );
    }
}

#[test]
fn dhcp_starts_on_each_wired_port_in_the_background() {
    let dir = TempDir::create("dhcp");
    for iface in ["eth0", "enp3s0", "lo", "sit0", "wlan0"] {
        std::fs::create_dir_all(dir.path("sys").join(iface)).unwrap();
    }
    let wired = ["eth0", "enp3s0"];
    for iface in wired {
        mkfifo(&dir.path(&format!("started-{iface}")));
    }
    dir.logging_fakes(&["ip"]);
    dir.fake("hostname", "echo neptron");
    // Each client writes its arguments to the FIFO of its port, and the
    // open blocks until the test reads it: the script must return before
    // that, as a boot with no cable does.
    dir.fake(
        "udhcpc",
        &format!(
            "args=\"$*\"\nwhile [ $# -gt 0 ]; do [ \"$1\" = -i ] && iface=$2; shift; done\n\
             echo \"udhcpc $args\" > \"{}/started-$iface\"",
            dir.0.display()
        ),
    );
    let (ok, out) = dir.run(NETWORK_SCRIPT, "start");
    assert!(ok, "{out}");
    for iface in wired {
        assert!(
            out.contains(&format!("DHCP on {iface} in the background")),
            "{out}"
        );
        let line = read_fifo(&dir.path(&format!("started-{iface}")));
        assert_eq!(
            line,
            format!(
                "udhcpc -f -R -S -i {iface} -x hostname:neptron -p {}/dhcp/{iface}.pid\n",
                dir.path("run").display()
            )
        );
        assert!(dir.log().contains(&format!("ip link set dev {iface} up\n")));
    }
    for other in ["lo", "sit0", "wlan0"] {
        assert!(!out.contains(other), "{out}");
        assert!(!dir.path(&format!("started-{other}")).exists(), "{other}");
    }
    let (ok, out) = dir.run(NETWORK_SCRIPT, "stop");
    assert!(ok, "{out}");
}

/// Runs the common post-build step on a target folder that holds the files
/// of the network share and `interfaces` as `etc/network/interfaces`.
fn post_build(dir: &TempDir, interfaces: &str, read_only: bool) -> (bool, String, String) {
    use std::os::unix::fs::PermissionsExt;
    let target = dir.path("target");
    for path in [
        "etc/init.d/S45neptron-share",
        "etc/ksmbd/ksmbd.conf",
        "etc/ksmbd/ksmbd.subauth",
        "etc/neptron/mdns.d/smb.service",
        "etc/init.d/S50mdnsd",
    ] {
        let file = target.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "").unwrap();
    }
    let file = target.join("etc/network/interfaces");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, interfaces).unwrap();
    if read_only {
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444)).unwrap();
    }
    let out = Command::new("sh")
        .arg(os_dir().join("board/common/post-build.sh"))
        .arg(&target)
        .output()
        .unwrap();
    assert!(!target.join("etc/init.d/S50mdnsd").exists());
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        std::fs::read_to_string(&file).unwrap(),
    )
}

/// Buildroot does not install ifupdown-scripts again when BR2_SYSTEM_DHCP
/// changes, so an output built before keeps the DHCP stanza of eth0: a
/// second DHCP client, and a boot that waits for it.
#[test]
fn the_post_build_leaves_only_the_loopback_in_interfaces() {
    let old = "# interface file auto-generated by buildroot\n\nauto lo\niface lo inet loopback\n\n\
               auto eth0\niface eth0 inet dhcp\n  pre-up /etc/network/nfs_check\n  wait-delay 15\n";
    let dir = TempDir::create("interfaces");
    let (ok, err, interfaces) = post_build(&dir, old, false);
    assert!(ok, "{err}");
    assert_eq!(interfaces, "auto lo\niface lo inet loopback\n");
    let dir = TempDir::create("interfaces-ro");
    let (ok, err, _) = post_build(&dir, old, true);
    assert!(!ok, "a file that cannot be written fails the build");
    assert!(err.contains("etc/network/interfaces"), "{err}");
}
