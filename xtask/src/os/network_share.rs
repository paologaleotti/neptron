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
        // The fixed account works with exactly the rights of a guest.
        ("force user", "nobody"),
        ("force group", "nobody"),
    ] {
        assert_eq!(share.get(key).map(String::as_str), Some(value), "{key}");
    }
    for (name, params) in &sections {
        for key in [
            "wide links",
            "follow symlinks",
            "root directory",
            "interfaces",
            "server max protocol",
        ] {
            assert!(!params.contains_key(key), "[{name}] sets {key}");
        }
    }
    assert!(!global.contains_key("force user"), "only the share forces");
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
        "etc/ksmbd/ksmbdpwd.db",
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

/// The guest share is for the local network only: no kernel of an image
/// has IPv6, and both kernel checks fail the build when it comes back.
#[test]
fn no_kernel_has_ipv6() {
    let pi = parse(&read("configs/neptron_rpi_arm64_defconfig"));
    for (fragments, rel) in [
        (
            &defconfig(Arch::X86_64)["BR2_LINUX_KERNEL_CONFIG_FRAGMENT_FILES"],
            "board/pc/network.config",
        ),
        (
            &defconfig(Arch::Aarch64)["BR2_LINUX_KERNEL_CONFIG_FRAGMENT_FILES"],
            "board/qemu/network.config",
        ),
        (
            &pi["BR2_LINUX_KERNEL_CONFIG_FRAGMENT_FILES"],
            "board/rpi/network.config",
        ),
    ] {
        assert!(fragments.contains(rel), "{rel} is not in the defconfig");
        assert_eq!(
            parse(&read(rel)).get("CONFIG_IPV6").map(String::as_str),
            Some("n"),
            "{rel}"
        );
    }
    for rel in [
        "board/pc/check-kernel-config.sh",
        "board/rpi/check-kernel-config.sh",
    ] {
        let check = read(rel);
        for line in [
            "if has IPV6 y; then",
            "elif has IPV6 m && ! disables_ipv6 \"$cmdline\"; then",
        ] {
            assert!(check.lines().any(|l| l == line), "{rel} misses {line}");
        }
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
    assert!(storage.contains("mount -t exfat -o \"$SHARE_MOUNT_OPTIONS\" \"$share\" \"$SHARE\""));
    assert!(storage.contains("[ \"$1\" = share ] && options=$SHARE_RAM_OPTIONS"));
    assert!(storage.contains("chown -R \"$SHARE_OWNER:$SHARE_OWNER\" \"$SHARE\""));
    assert!(storage.lines().any(|l| l == "SHARE=$R/share"));
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
        self.run_args(script, &[action])
    }

    /// Runs `script` with `args` as [`run`](Self::run) does, with umask 0,
    /// as mdev runs its commands: the script must set its own umask.
    fn run_args(&self, script: &str, args: &[&str]) -> (bool, String) {
        self.run_env(script, args, &[])
    }

    /// Runs `script` as [`run_args`](Self::run_args) does, with `env` on top
    /// of the test paths.
    fn run_env(&self, script: &str, args: &[&str], env: &[(&str, &str)]) -> (bool, String) {
        let action = args.join("-");
        let path = format!(
            "{}:{}",
            self.path("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let out_path = self.path(&format!("{action}.out"));
        let out = std::fs::File::create(&out_path).unwrap();
        let child = Command::new("sh")
            .arg("-c")
            .arg("umask 000; exec sh \"$@\"")
            .arg("sh")
            .arg(os_dir().join(script))
            .args(args)
            .env("PATH", path)
            .env("LOG", self.path("log"))
            .env("NEPTRON_STATE", self.path("state"))
            .env("NEPTRON_PROC_NET", self.path("net"))
            .env("NEPTRON_RUN", self.path("run"))
            .env("NEPTRON_SYS_NET", self.path("sys"))
            .env("NEPTRON_PROC", self.path("proc"))
            .env("NEPTRON_DEV_MARK", self.path("development"))
            .env("NEPTRON_CONSOLE", self.path("console"))
            .env("NEPTRON_ADDRESS", self.path("bin/net-address"))
            .envs(env.iter().copied())
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

/// A filter that does not load keeps the share off: the server and mDNS
/// never start open to every address.
#[test]
fn a_filter_that_does_not_load_keeps_the_share_off() {
    let dir = TempDir::create("nofilter");
    let done = dir.path("done");
    mkfifo(&done);
    dir.fake(
        "iptables-legacy-restore",
        &format!(
            "echo \"iptables-legacy-restore $*\" >> \"$LOG\"; echo done > \"{}\"; exit 1",
            done.display()
        ),
    );
    dir.logging_fakes(&["ksmbd.mountd", "start-stop-daemon"]);
    let (ok, out) = dir.run(SHARE_SCRIPT, "start");
    assert!(ok, "{out}");
    assert_eq!(read_fifo(&done), "done\n");
    // The job prints its line after the fake ends; wait for it with a
    // bound, as the other waits of this file do.
    let path = dir.path("start.out");
    let out = bounded("the line of the share job", move || {
        loop {
            let text = std::fs::read_to_string(&path).unwrap();
            if text.contains("the share stays off") {
                return text;
            }
            std::thread::yield_now();
        }
    })
    .unwrap_or_else(|message| panic!("{message}"));
    assert!(out.contains("the packet filter did not load"), "{out}");
    assert_eq!(
        dir.log(),
        "iptables-legacy-restore /etc/neptron/share-filter.rules\n",
        "nothing starts"
    );
}

/// The filter rules that the check expects, for the fixtures below.
const FILTER_RULES: &str = "board/common/rootfs-overlay/etc/neptron/share-filter.rules";

/// Builds a fake target with the share filter, the tool that loads it and
/// the init script, changes it with `change`, and runs
/// check-share-filter.sh on it.
fn check_share_filter(name: &str, change: impl FnOnce(&Path)) -> std::process::Output {
    let dir = TempDir::create(&format!("filter-{name}"));
    let target = dir.path("target");
    for (rel, from) in [
        ("etc/neptron/share-filter.rules", FILTER_RULES),
        ("etc/init.d/S45neptron-share", SHARE_SCRIPT),
    ] {
        let path = target.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, read(from)).unwrap();
    }
    std::fs::create_dir_all(target.join("usr/sbin")).unwrap();
    std::fs::write(target.join("usr/sbin/xtables-legacy-multi"), "").unwrap();
    std::os::unix::fs::symlink(
        "xtables-legacy-multi",
        target.join("usr/sbin/iptables-legacy-restore"),
    )
    .unwrap();
    change(&target);
    Command::new("sh")
        .arg(os_dir().join("board/common/check-share-filter.sh"))
        .arg(&target)
        .output()
        .unwrap()
}

/// Replaces `from` with `to` in the file `rel` of a fake target.
fn edit(target: &Path, rel: &str, from: &str, to: &str) {
    let path = target.join(rel);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(from), "{rel} has no {from}");
    std::fs::write(&path, text.replacen(from, to, 1)).unwrap();
}

/// The build fails when the image loses the address limit of the share:
/// a changed rule, no filter file, no tool to load it, no load in the
/// init script, or the package script that flushes it.
#[test]
fn the_filter_check_refuses_an_image_without_the_address_limit() {
    let ok = check_share_filter("ok", |_| ());
    assert!(ok.status.success(), "{ok:?}");
    const RULES: &str = "etc/neptron/share-filter.rules";
    let comment = check_share_filter("comment", |t| {
        edit(t, RULES, "*filter", "# A comment.\n\n*filter")
    });
    assert!(
        comment.status.success(),
        "comments change no rule: {comment:?}"
    );
    type Change = Box<dyn FnOnce(&Path)>;
    let cases: [(&str, Change, &str); 9] = [
        (
            "nodrop",
            Box::new(|t| edit(t, RULES, "-A neptron-lan -j DROP\n", "")),
            "are not the ones",
        ),
        (
            "any",
            Box::new(|t| {
                edit(
                    t,
                    RULES,
                    "-A neptron-lan -s 127.0.0.0/8",
                    "-A neptron-lan -s 0.0.0.0/0 -j ACCEPT\n-A neptron-lan -s 127.0.0.0/8",
                )
            }),
            "are not the ones",
        ),
        (
            "wide",
            Box::new(|t| edit(t, RULES, "10.0.0.0/8", "10.0.0.0/1")),
            "are not the ones",
        ),
        (
            "nojump",
            Box::new(|t| {
                edit(
                    t,
                    RULES,
                    "-A INPUT -p tcp -m tcp --dport 445 -j neptron-lan\n",
                    "",
                )
            }),
            "are not the ones",
        ),
        (
            "accept",
            Box::new(|t| {
                edit(
                    t,
                    RULES,
                    "-A INPUT -p tcp",
                    "-A INPUT -j ACCEPT\n-A INPUT -p tcp",
                )
            }),
            "are not the ones",
        ),
        (
            "norules",
            Box::new(|t| std::fs::remove_file(t.join(RULES)).unwrap()),
            "has no /etc/neptron/share-filter.rules",
        ),
        (
            "notool",
            Box::new(|t| std::fs::remove_file(t.join("usr/sbin/iptables-legacy-restore")).unwrap()),
            "has no /usr/sbin/iptables-legacy-restore",
        ),
        (
            "noload",
            Box::new(|t| {
                edit(
                    t,
                    "etc/init.d/S45neptron-share",
                    "if ! iptables-legacy-restore /etc/neptron/share-filter.rules; then",
                    "if false; then",
                )
            }),
            "does not load the filter",
        ),
        (
            "s35",
            Box::new(|t| std::fs::write(t.join("etc/init.d/S35iptables"), "").unwrap()),
            "S35iptables",
        ),
    ];
    for (name, change, fault) in cases {
        let out = check_share_filter(name, change);
        assert!(!out.status.success(), "{name}");
        let text = String::from_utf8_lossy(&out.stderr).into_owned();
        assert!(text.contains(fault), "{name}: {text}");
    }
}

/// Every image limits the share to the local network with the packet
/// filter: the package, the kernel options and both kernel checks, and the
/// post-build step that checks the image.
#[test]
fn every_image_filters_the_share_to_the_local_network() {
    let filter = [
        "CONFIG_NETFILTER",
        "CONFIG_NETFILTER_XTABLES",
        "CONFIG_NETFILTER_XTABLES_LEGACY",
        "CONFIG_IP_NF_IPTABLES",
        "CONFIG_IP_NF_IPTABLES_LEGACY",
        "CONFIG_IP_NF_FILTER",
    ];
    for (defconfig, fragment) in [
        (
            "configs/neptron_pc_x86_64_defconfig",
            "board/pc/network.config",
        ),
        (
            "configs/neptron_qemu_aarch64_defconfig",
            "board/qemu/network.config",
        ),
        (
            "configs/neptron_rpi_arm64_defconfig",
            "board/rpi/network.config",
        ),
    ] {
        let options = parse(&read(defconfig));
        assert_eq!(
            options.get("BR2_PACKAGE_IPTABLES").map(String::as_str),
            Some("y"),
            "{defconfig}"
        );
        assert!(
            !options.contains_key("BR2_PACKAGE_IPTABLES_NFTABLES_DEFAULT"),
            "{defconfig}: the share loads the legacy tables"
        );
        let kernel = parse(&read(fragment));
        for option in filter {
            assert_eq!(
                kernel.get(option).map(String::as_str),
                Some("y"),
                "{fragment} {option}"
            );
        }
    }
    let names = filter.map(|o| o.trim_start_matches("CONFIG_")).join(" ");
    for rel in [
        "board/pc/check-kernel-config.sh",
        "board/rpi/check-kernel-config.sh",
    ] {
        let line = format!("FILTER_ON=\"{names}\"");
        assert!(read(rel).lines().any(|l| l == line), "{rel} misses {line}");
    }
    let post_build = read("board/common/post-build.sh");
    assert!(post_build.contains("\"$target/etc/init.d/S35iptables\""));
    assert!(post_build.contains("\"$(dirname \"$0\")/check-share-filter.sh\" \"$target\""));
    for dev in [
        "configs/neptron_pc_x86_64_dev.fragment",
        "configs/neptron_pc_x86_64_no15khz.fragment",
        "configs/neptron_rpi_arm64_dev.fragment",
    ] {
        let fragment = parse(&read(dev));
        assert!(!fragment.contains_key("BR2_PACKAGE_IPTABLES"), "{dev}");
    }
}

#[test]
fn the_off_switch_keeps_the_server_and_mdns_down() {
    let dir = TempDir::create("off");
    dir.logging_fakes(&[
        "iptables-legacy-restore",
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
    dir.logging_fakes(&["iptables-legacy-restore"]);
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
    assert!(
        !dir.log().contains("ksmbd.mountd"),
        "the script returned before the server started"
    );
    write_fifo(&gate, "go\n");
    assert_eq!(read_fifo(&done), "done\n");
    assert_eq!(
        dir.log(),
        "iptables-legacy-restore /etc/neptron/share-filter.rules\n\
         ksmbd.mountd \n\
         start-stop-daemon --start --quiet --exec /usr/sbin/mdnsd -- /etc/neptron/mdns.d\n",
        "the filter is in place before the server listens"
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
        dir.fake("pidof", "exit 1");
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
        assert_eq!(out.contains("still runs after"), waits > 0, "{name}: {out}");
    }
}

/// A restart must not start the daemons while the old ones still run: the
/// lock of ksmbd.mountd and the port of mdnsd are still taken.
#[test]
fn restart_starts_the_daemons_only_after_the_old_ones_ended() {
    let dir = TempDir::create("restart");
    let done = dir.path("done");
    mkfifo(&done);
    dir.logging_fakes(&[
        "ksmbd.control",
        "usleep",
        "sync",
        "iptables-legacy-restore",
        "ksmbd.mountd",
    ]);
    // The old daemons end after three looks.
    dir.fake(
        "pidof",
        "n=$(cat \"$LOG.pidof\" 2>/dev/null || echo 0); n=$((n + 1)); echo $n > \"$LOG.pidof\"\n\
         echo \"pidof $*\" >> \"$LOG\"; [ $n -le 3 ]",
    );
    dir.fake(
        "start-stop-daemon",
        &format!(
            "echo \"start-stop-daemon $*\" >> \"$LOG\"\n\
             case \"$*\" in *--start*) echo done > \"{}\" ;; esac",
            done.display()
        ),
    );
    std::fs::create_dir_all(dir.path("net")).unwrap();
    let (ok, out) = dir.run(SHARE_SCRIPT, "restart");
    assert!(ok, "{out}");
    assert_eq!(read_fifo(&done), "done\n");
    let log = dir.log();
    let lines: Vec<&str> = log.lines().map(str::trim_end).collect();
    assert_eq!(
        lines,
        [
            "start-stop-daemon --stop --quiet --exec /usr/sbin/mdnsd",
            "ksmbd.control --shutdown",
            "pidof mdnsd ksmbd.mountd",
            "usleep 100000",
            "pidof mdnsd ksmbd.mountd",
            "usleep 100000",
            "pidof mdnsd ksmbd.mountd",
            "usleep 100000",
            "pidof mdnsd ksmbd.mountd",
            // The check for the line that says a daemon still runs.
            "pidof mdnsd ksmbd.mountd",
            "sync",
            "iptables-legacy-restore /etc/neptron/share-filter.rules",
            "ksmbd.mountd",
            "start-stop-daemon --start --quiet --exec /usr/sbin/mdnsd -- /etc/neptron/mdns.d",
        ],
        "{out}"
    );
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
        "etc/ksmbd/ksmbdpwd.db",
        "etc/neptron/mdns.d/smb.service",
        "etc/init.d/S50mdnsd",
        "etc/init.d/S35iptables",
    ] {
        let file = target.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "").unwrap();
    }
    std::fs::write(
        target.join("etc/init.d/S45neptron-share"),
        read(SHARE_SCRIPT),
    )
    .unwrap();
    std::fs::write(
        target.join("etc/neptron/share-filter.rules"),
        read(FILTER_RULES),
    )
    .unwrap();
    std::fs::create_dir_all(target.join("usr/sbin")).unwrap();
    std::fs::write(target.join("usr/sbin/iptables-legacy-restore"), "").unwrap();
    std::fs::write(target.join("etc/mdev.conf"), MDEV_CONF).unwrap();
    udhcpc_files(&target);
    let file = target.join("etc/network/interfaces");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, interfaces).unwrap();
    if read_only {
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444)).unwrap();
    }
    let out = run_post_build(dir);
    assert!(!target.join("etc/init.d/S50mdnsd").exists());
    assert!(!target.join("etc/init.d/S35iptables").exists());
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        std::fs::read_to_string(&file).unwrap(),
    )
}

/// The last lines of the udhcpc script that Buildroot's busybox package
/// installs (`/usr/share/udhcpc/default.script` of the Pi image of
/// 2026-11-14, Buildroot 2026.08.1): the loop that runs the hooks with the
/// event `$ACTION` as their one argument.
const UDHCPC_SCRIPT_END: &str = "HOOK_DIR=\"$0.d\"\nfor hook in \"${HOOK_DIR}/\"*; do\n    \
                                 [ -f \"${hook}\" -a -x \"${hook}\" ] || continue\n    \
                                 \"${hook}\" \"$ACTION\"\ndone\n\nexit 0\n";

const ADDRESS_SCRIPT: &str = "board/common/rootfs-overlay/usr/libexec/neptron/net-address";
const ADDRESS_HOOK: &str =
    "board/common/rootfs-overlay/usr/share/udhcpc/default.script.d/neptron-address";

/// Puts the udhcpc script of the busybox package and the two address
/// scripts of the overlay, executable, in the image folder `target`.
fn udhcpc_files(target: &Path) {
    use std::os::unix::fs::PermissionsExt;
    for (rel, text) in [
        (
            "usr/share/udhcpc/default.script",
            UDHCPC_SCRIPT_END.to_owned(),
        ),
        (
            "usr/share/udhcpc/default.script.d/neptron-address",
            read(ADDRESS_HOOK),
        ),
        ("usr/libexec/neptron/net-address", read(ADDRESS_SCRIPT)),
        ("usr/bin/flock", String::new()),
    ] {
        let file = target.join(rel);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, text).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// The address files come from a hook of udhcpc, so the post-build step
/// fails when the udhcpc script of the image runs no hooks or the hook is
/// not executable, and it removes the address script of an older build,
/// which wrote the share file once at boot.
#[test]
fn the_post_build_checks_the_udhcpc_hook_and_removes_the_old_address_script() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::create("udhcpc-hook");
    let (ok, err, _) = post_build(&dir, "auto lo\niface lo inet loopback\n", false);
    assert!(ok, "{err}");
    let target = dir.path("target");
    let old = target.join("etc/init.d/S41neptron-address");
    std::fs::write(&old, "#!/bin/sh\n").unwrap();
    let again = run_post_build(&dir);
    assert!(again.status.success(), "{again:?}");
    assert!(!old.exists(), "the address script of an older build stays");

    let hook = target.join("usr/share/udhcpc/default.script.d/neptron-address");
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o644)).unwrap();
    let out = run_post_build(&dir);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("no executable udhcpc hook"),
        "{out:?}"
    );
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();

    std::fs::write(
        target.join("usr/share/udhcpc/default.script"),
        "#!/bin/sh\nexit 0\n",
    )
    .unwrap();
    let out = run_post_build(&dir);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("runs the hooks"),
        "{out:?}"
    );
    std::fs::write(
        target.join("usr/share/udhcpc/default.script"),
        UDHCPC_SCRIPT_END,
    )
    .unwrap();

    std::fs::remove_file(target.join("usr/bin/flock")).unwrap();
    let out = run_post_build(&dir);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("no /usr/bin/flock"),
        "{out:?}"
    );
}

/// The post-build step of every image runs the filter check: an image
/// without the filter of the share fails the build.
#[test]
fn the_post_build_fails_an_image_without_the_share_filter() {
    let dir = TempDir::create("post-build-filter");
    let (ok, err, _) = post_build(&dir, "auto lo\niface lo inet loopback\n", false);
    assert!(ok, "{err}");
    let rules = dir.path("target").join("etc/neptron/share-filter.rules");
    std::fs::write(&rules, "*filter\nCOMMIT\n").unwrap();
    let out = run_post_build(&dir);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("are not the ones"),
        "{out:?}"
    );
}

/// The lock of the address script is a flock. Every image builds BusyBox
/// with the common fragment, which keeps the applet on.
#[test]
fn every_image_keeps_the_flock_applet_of_busybox() {
    let fragment = parse(&read("board/common/busybox.config"));
    assert_eq!(fragment.get("CONFIG_FLOCK").map(String::as_str), Some("y"));
    for rel in [
        "configs/neptron_pc_x86_64_defconfig",
        "configs/neptron_rpi_arm64_defconfig",
        "configs/neptron_qemu_aarch64_defconfig",
    ] {
        let options = parse(&read(rel));
        assert!(
            options["BR2_PACKAGE_BUSYBOX_CONFIG_FRAGMENT_FILES"]
                .contains("$(BR2_EXTERNAL_NEPTRON_PATH)/board/common/busybox.config"),
            "{rel}"
        );
        assert!(
            !options.contains_key("BR2_PACKAGE_BUSYBOX_CONFIG"),
            "{rel} replaces the Buildroot base configuration of BusyBox"
        );
    }
    for rel in [
        "configs/neptron_pc_x86_64_dev.fragment",
        "configs/neptron_pc_x86_64_no15khz.fragment",
        "configs/neptron_rpi_arm64_dev.fragment",
    ] {
        let options = parse(&read(rel));
        for key in [
            "BR2_PACKAGE_BUSYBOX_CONFIG",
            "BR2_PACKAGE_BUSYBOX_CONFIG_FRAGMENT_FILES",
        ] {
            assert!(!options.contains_key(key), "{rel} changes {key}");
        }
    }
    assert!(read(ADDRESS_SCRIPT).contains("flock -n 9"));
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
    // Root writes a read-only file, so this case proves nothing there.
    let uid = Command::new("id").arg("-u").output().unwrap();
    if String::from_utf8_lossy(&uid.stdout).trim() == "0" {
        return;
    }
    let dir = TempDir::create("interfaces-ro");
    let (ok, err, _) = post_build(&dir, old, true);
    assert!(!ok, "a file that cannot be written fails the build");
    assert!(err.contains("etc/network/interfaces"), "{err}");
}

/// The last lines of the mdev.conf that the busybox package installs.
const MDEV_CONF: &str = "input/ts[0-9]\t\troot:root 600\n\n# load modules\n\
                         $MODALIAS=.*\troot:root 660 @modprobe \"$MODALIAS\"\n";

/// The rule that common/post-build.sh appends to /etc/mdev.conf: `*` runs
/// the script when a port is added and when it is removed.
const NET_RULE: &str = "SUBSYSTEM=net;(eth|en).*\troot:root 600 */usr/libexec/neptron/net-hotplug";

fn run_post_build(dir: &TempDir) -> std::process::Output {
    // Buildroot gives its configuration to every post-build step.
    let config = dir.path("br2.config");
    std::fs::write(&config, "BR2_x86_64=y\n").unwrap();
    Command::new("sh")
        .arg(os_dir().join("board/common/post-build.sh"))
        .arg(dir.path("target"))
        .env("BR2_CONFIG", &config)
        .output()
        .unwrap()
}

/// A wired port that appears after the boot (the USB Ethernet of a Pi, a
/// USB adapter) gets its DHCP client from mdev. The post-build step
/// appends the rule once, replaces the line of an older build (mdev takes
/// the first rule that matches), ends the last line of the file first, and
/// fails when the image has no mdev.conf.
#[test]
fn the_post_build_adds_the_mdev_rule_of_network_ports_once() {
    let dir = TempDir::create("mdev");
    let (ok, err, _) = post_build(&dir, "auto lo\niface lo inet loopback\n", false);
    assert!(ok, "{err}");
    let conf = dir.path("target/etc/mdev.conf");
    let text = std::fs::read_to_string(&conf).unwrap();
    assert_eq!(text, format!("{MDEV_CONF}{NET_RULE}\n"));
    let again = run_post_build(&dir);
    assert!(again.status.success(), "{again:?}");
    assert_eq!(std::fs::read_to_string(&conf).unwrap(), text, "one copy");

    let older = "SUBSYSTEM=net;(eth|en).*\troot:root 600 @/usr/libexec/neptron/net-hotplug";
    std::fs::write(&conf, format!("{MDEV_CONF}{older}\n")).unwrap();
    let replaced = run_post_build(&dir);
    assert!(replaced.status.success(), "{replaced:?}");
    assert_eq!(
        std::fs::read_to_string(&conf).unwrap(),
        text,
        "the older rule goes"
    );

    let unterminated = MDEV_CONF.trim_end_matches('\n');
    std::fs::write(&conf, unterminated).unwrap();
    let ended = run_post_build(&dir);
    assert!(ended.status.success(), "{ended:?}");
    assert_eq!(
        std::fs::read_to_string(&conf).unwrap(),
        format!("{unterminated}\n{NET_RULE}\n"),
        "the rule is a line of its own"
    );

    std::fs::remove_file(&conf).unwrap();
    let missing = run_post_build(&dir);
    assert!(!missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("no /etc/mdev.conf"),
        "{missing:?}"
    );
    // The rule runs the hotplug script of the common overlay, which is in
    // every image.
    let hotplug = os_dir().join(HOTPLUG);
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(&hotplug).unwrap().permissions().mode();
    assert_eq!(mode & 0o111, 0o111, "net-hotplug is not executable");
}

const HOTPLUG: &str = "board/common/rootfs-overlay/usr/libexec/neptron/net-hotplug";

/// Runs net-hotplug as mdev does (umask 0, `action` and `device` in the
/// environment) below the root `dir`; returns whether it succeeded, the
/// calls of the fake network script with its umask, and what went to the
/// fake logger.
fn hotplug(dir: &TempDir, action: &str, device: &str) -> (bool, String, String) {
    let calls = dir.path("calls");
    let logged = dir.path("logged");
    for file in [&calls, &logged] {
        // A file that is not there yet is the usual case.
        let _ = std::fs::remove_file(file);
    }
    let network = dir.path("etc/init.d/S40neptron-network");
    std::fs::create_dir_all(network.parent().unwrap()).unwrap();
    std::fs::write(
        &network,
        format!(
            "#!/bin/sh\necho \"$* umask $(umask)\" >> {}\necho \"neptron-network: $*\"\n",
            calls.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&network, std::fs::Permissions::from_mode(0o755)).unwrap();
    dir.fake(
        "logger",
        &format!("echo \"$*: $(cat)\" >> {}", logged.display()),
    );
    let path = format!(
        "{}:{}",
        dir.path("bin").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = Command::new("sh")
        .arg("-c")
        .arg("umask 000; exec sh \"$@\"")
        .arg("sh")
        .arg(os_dir().join(HOTPLUG))
        .arg(&dir.0)
        .env("ACTION", action)
        .env("MDEV", device)
        .env("PATH", path)
        .output()
        .unwrap();
    (
        out.status.success(),
        std::fs::read_to_string(&calls).unwrap_or_default(),
        std::fs::read_to_string(&logged).unwrap_or_default(),
    )
}

/// mdev runs net-hotplug for every network port, with umask 0. It runs the
/// network script with umask 022, only for a wired port and only after the
/// boot start: a start for an added port, and the stop of its client for a
/// removed port. Its lines go to syslog.
#[test]
fn net_hotplug_starts_an_added_port_and_stops_a_removed_one() {
    let dir = TempDir::create("hotplug");
    let none = (true, String::new(), String::new());
    assert_eq!(
        hotplug(&dir, "add", "eth0"),
        none,
        "before S40neptron-network ran"
    );
    std::fs::create_dir_all(dir.path("run/neptron/dhcp")).unwrap();
    assert_eq!(
        hotplug(&dir, "add", "eth0"),
        (
            true,
            "start umask 0022\n".to_owned(),
            "-t neptron-network: neptron-network: start\n".to_owned()
        )
    );
    assert_eq!(
        hotplug(&dir, "add", "enx001122334455").1,
        "start umask 0022\n"
    );
    assert_eq!(
        hotplug(&dir, "remove", "eth0"),
        (
            true,
            "port-gone eth0 umask 0022\n".to_owned(),
            "-t neptron-network: neptron-network: port-gone eth0\n".to_owned()
        )
    );
    for (action, device) in [
        ("change", "eth0"),
        ("add", "wlan0"),
        ("remove", "lo"),
        ("add", ""),
    ] {
        assert_eq!(hotplug(&dir, action, device), none, "{action} {device}");
    }
}

/// Both scripts set umask 022 before they make any file: mdev runs them
/// with umask 0.
#[test]
fn the_network_scripts_set_their_umask_first() {
    for rel in [HOTPLUG, NETWORK_SCRIPT, ADDRESS_SCRIPT] {
        let text = read(rel);
        let first = text
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && !l.starts_with('#'));
        assert_eq!(first, Some("umask 022"), "{rel}");
    }
    let dir = TempDir::create("dhcp-umask");
    std::fs::create_dir_all(dir.path("sys/eth0")).unwrap();
    dir.logging_fakes(&["ip", "udhcpc"]);
    dir.fake("hostname", "echo neptron");
    let (ok, out) = dir.run(NETWORK_SCRIPT, "start");
    assert!(ok, "{out}");
    use std::os::unix::fs::PermissionsExt;
    let mode = |rel: &str| {
        std::fs::metadata(dir.path(rel))
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    };
    assert_eq!(mode("run/dhcp"), 0o755);
    assert_eq!(mode("run/dhcp/eth0.pid"), 0o644);
}

/// Writes the command line of a udhcpc for `iface` as the fake /proc of
/// the network script shows it for `pid`.
fn fake_client(dir: &TempDir, pid: u32, iface: &str) {
    let proc = dir.path(&format!("proc/{pid}"));
    std::fs::create_dir_all(&proc).unwrap();
    std::fs::write(
        proc.join("cmdline"),
        format!("udhcpc\0-f\0-R\0-S\0-i\0{iface}\0-x\0hostname:neptron\0"),
    )
    .unwrap();
}

/// A child that runs until the test stops it, with its pid.
struct Process(Option<std::process::Child>);

impl Process {
    fn start() -> Self {
        Process(Some(
            Command::new("sh")
                .arg("-c")
                .arg("read line")
                .stdin(Stdio::piped())
                .spawn()
                .unwrap(),
        ))
    }

    fn pid(&self) -> u32 {
        self.0.as_ref().map_or(0, std::process::Child::id)
    }

    /// Whether the child ends within the wait of the tests.
    fn ended(&mut self) -> bool {
        let Some(mut child) = self.0.take() else {
            return true;
        };
        bounded("the child to end", move || child.wait().is_ok()).unwrap_or(false)
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            // The child may have ended already, which is what most tests
            // want; an error here leaves nothing to clean up.
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn dhcp_fakes(dir: &TempDir) {
    dir.logging_fakes(&["ip", "udhcpc", "usleep"]);
    dir.fake("hostname", "echo neptron");
}

/// A second start (a restart by hand, rcS run again, or mdev for another
/// port) must not start a second client on a port whose client still runs;
/// a pid file that names no udhcpc of that port is stale.
#[test]
fn dhcp_starts_no_second_client_and_replaces_a_stale_pid_file() {
    let dir = TempDir::create("dhcp-twice");
    for iface in ["eth0", "eth1", "eth2", "eth3"] {
        std::fs::create_dir_all(dir.path("sys").join(iface)).unwrap();
    }
    std::fs::create_dir_all(dir.path("run/dhcp")).unwrap();
    let live = Process::start();
    let other = Process::start();
    let reused = Process::start();
    // eth0: a running client of eth0. eth1: a pid that names no process.
    // eth2: a running client of eth3 (the file of another port). eth3: a
    // pid that a process other than udhcpc took.
    fake_client(&dir, live.pid(), "eth0");
    fake_client(&dir, other.pid(), "eth3");
    let proc = dir.path(&format!("proc/{}", reused.pid()));
    std::fs::create_dir_all(&proc).unwrap();
    std::fs::write(proc.join("cmdline"), "sh\0-c\0read line\0").unwrap();
    for (iface, pid) in [
        ("eth0", live.pid()),
        ("eth1", 999_999),
        ("eth2", other.pid()),
        ("eth3", reused.pid()),
    ] {
        std::fs::write(
            dir.path(&format!("run/dhcp/{iface}.pid")),
            format!("{pid}\n"),
        )
        .unwrap();
    }
    dhcp_fakes(&dir);
    let (ok, out) = dir.run(NETWORK_SCRIPT, "start");
    assert!(ok, "{out}");
    assert!(out.contains("eth0 already has a DHCP client"), "{out}");
    for iface in ["eth1", "eth2", "eth3"] {
        assert!(
            out.contains(&format!("DHCP on {iface} in the background")),
            "{out}"
        );
    }
    assert!(
        !out.contains("DHCP on eth0"),
        "a second client on eth0: {out}"
    );
}

/// A port that is unplugged loses its client and its pid file; plugged in
/// again, it gets a new client.
#[test]
fn a_removed_port_loses_its_client_and_gets_a_new_one_when_it_comes_back() {
    let dir = TempDir::create("dhcp-replug");
    std::fs::create_dir_all(dir.path("run/dhcp")).unwrap();
    let mut client = Process::start();
    fake_client(&dir, client.pid(), "eth0");
    std::fs::write(dir.path("run/dhcp/eth0.pid"), format!("{}\n", client.pid())).unwrap();
    dhcp_fakes(&dir);
    let (ok, out) = dir.run_args(NETWORK_SCRIPT, &["port-gone", "eth0"]);
    assert!(ok, "{out}");
    assert!(out.contains("the DHCP client of eth0 stops"), "{out}");
    assert!(client.ended(), "the client of the removed port ends");
    assert!(!dir.path("run/dhcp/eth0.pid").exists());
    assert!(!dir.path("run/dhcp.lock").exists());
    std::fs::create_dir_all(dir.path("sys/eth0")).unwrap();
    let (ok, out) = dir.run(NETWORK_SCRIPT, "start");
    assert!(ok, "{out}");
    assert!(out.contains("DHCP on eth0 in the background"), "{out}");
    let (ok, out) = dir.run_args(NETWORK_SCRIPT, &["port-gone", "wlan0"]);
    assert!(!ok, "only a wired port: {out}");
}

/// Two runs can overlap (the boot and a port that appears). A run waits for
/// the other one and gives up with a line when the lock stays; a run that
/// is stopped while it holds the lock frees it.
#[test]
fn overlapping_dhcp_runs_take_turns_and_free_the_lock() {
    let dir = TempDir::create("dhcp-lock");
    std::fs::create_dir_all(dir.path("sys/eth0")).unwrap();
    std::fs::create_dir_all(dir.path("run/dhcp.lock")).unwrap();
    dhcp_fakes(&dir);
    // The other run ends during the first pause.
    dir.fake(
        "usleep",
        &format!(
            "echo \"usleep $*\" >> \"$LOG\"\nrmdir {}",
            dir.path("run/dhcp.lock").display()
        ),
    );
    let (ok, out) = dir.run(NETWORK_SCRIPT, "start");
    assert!(ok, "{out}");
    assert!(out.contains("DHCP on eth0 in the background"), "{out}");
    assert!(dir.log().contains("usleep 100000\n"), "{}", dir.log());
    assert!(
        !dir.path("run/dhcp.lock").exists(),
        "the run frees the lock"
    );
    let pid = std::fs::read_to_string(dir.path("run/dhcp/eth0.pid")).unwrap();
    assert!(!pid.trim().is_empty(), "the pid file is written at once");

    // A lock that stays: the run gives up after its tries, 49 pauses
    // between 50 tries.
    std::fs::create_dir_all(dir.path("run/dhcp.lock")).unwrap();
    dir.fake("usleep", "echo \"usleep $*\" >> \"$LOG\"");
    std::fs::write(dir.path("log"), "").unwrap();
    std::fs::remove_file(dir.path("run/dhcp/eth0.pid")).unwrap();
    let (ok, out) = dir.run(NETWORK_SCRIPT, "start");
    assert!(ok, "{out}");
    assert!(out.contains("another run holds"), "{out}");
    assert!(!out.contains("DHCP on eth0"), "{out}");
    assert_eq!(dir.log().matches("usleep 100000").count(), 49);
    std::fs::remove_dir(dir.path("run/dhcp.lock")).unwrap();

    // A run stopped while it holds the lock: ip sends SIGTERM to it.
    dir.fake("ip", "kill -TERM \"$PPID\"");
    let (ok, out) = dir.run(NETWORK_SCRIPT, "start");
    assert!(!ok, "the run ends on the signal: {out}");
    assert!(
        !dir.path("run/dhcp.lock").exists(),
        "the trap frees the lock"
    );
}

/// One round of MD4: its function, its constant, the order of the words
/// and the four shifts.
type Md4Round = (fn(u32, u32, u32) -> u32, u32, [usize; 16], [u32; 4]);

/// MD4 (RFC 1320), for the NT hash of the published password.
fn md4(message: &[u8]) -> [u8; 16] {
    let mut data = message.to_vec();
    let bits = (message.len() as u64).wrapping_mul(8);
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bits.to_le_bytes());
    let mut h: [u32; 4] = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
    for block in data.chunks(64) {
        let x: Vec<u32> = block
            .chunks(4)
            .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
            .collect();
        let [mut a, mut b, mut c, mut d] = h;
        let rounds: [Md4Round; 3] = [
            (
                |x, y, z| (x & y) | (!x & z),
                0,
                [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
                [3, 7, 11, 19],
            ),
            (
                |x, y, z| (x & y) | (x & z) | (y & z),
                0x5a82_7999,
                [0, 4, 8, 12, 1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15],
                [3, 5, 9, 13],
            ),
            (
                |x, y, z| x ^ y ^ z,
                0x6ed9_eba1,
                [0, 8, 4, 12, 2, 10, 6, 14, 1, 9, 5, 13, 3, 11, 7, 15],
                [3, 9, 11, 15],
            ),
        ];
        for (f, k, order, shifts) in rounds {
            for (i, &j) in order.iter().enumerate() {
                let t = a
                    .wrapping_add(f(b, c, d))
                    .wrapping_add(x[j])
                    .wrapping_add(k)
                    .rotate_left(shifts[i % 4]);
                (a, b, c, d) = (d, t, b, c);
            }
        }
        h = [
            h[0].wrapping_add(a),
            h[1].wrapping_add(b),
            h[2].wrapping_add(c),
            h[3].wrapping_add(d),
        ];
    }
    let mut out = [0; 16];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    out
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(TABLE[(n >> (18 - 6 * i) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[test]
fn md4_and_base64_match_their_reference_vectors() {
    let hex = |b: [u8; 16]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    assert_eq!(hex(md4(b"")), "31d6cfe0d16ae931b73c59d7e0c089c0");
    assert_eq!(hex(md4(b"abc")), "a448017aaf21d8525fc10ae87aa6729d");
    assert_eq!(
        hex(md4(
            b"12345678901234567890123456789012345678901234567890123456789012345678901234567890"
        )),
        "e33b4ddc9c38f2199c3e7b164fcc0536"
    );
    assert_eq!(base64(b"Man"), "TWFu");
    assert_eq!(base64(b"Ma"), "TWE=");
    assert_eq!(base64(b"M"), "TQ==");
}

/// The published account of the share: Windows 11 24H2 refuses guest
/// sessions, and a login with a password can sign. The password is public.
const SHARE_USER: &str = "neptron";
const SHARE_PASSWORD: &str = "neptron";

#[test]
fn the_user_database_holds_the_published_account_only() {
    // ksmbd-tools 3.5.2 (adduser/user_admin.c) stores base64 of the MD4 of
    // the UTF-16LE password, one `name:hash` line per user.
    let utf16: Vec<u8> = SHARE_PASSWORD
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let line = format!("{SHARE_USER}:{}\n", base64(&md4(&utf16)));
    assert_eq!(line, "neptron:GVMrF6dHlJsKwpNZhfO/3Q==\n");
    assert_eq!(read(&format!("{OVERLAY}/etc/ksmbd/ksmbdpwd.db")), line);
}

#[test]
fn the_share_account_has_no_system_login() {
    let table = read("board/common/users.table");
    let lines: Vec<&str> = table
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .collect();
    assert_eq!(lines.len(), 1, "{table}");
    let fields: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(fields[0], SHARE_USER);
    assert_eq!(fields[4], "*", "no password: login is not allowed");
    assert_eq!(fields[5], "-", "no home folder");
    assert_eq!(fields[6], "/bin/false", "no shell");
    for arch in [Arch::Aarch64, Arch::X86_64] {
        let options = defconfig(arch);
        assert_eq!(
            options.get("BR2_ROOTFS_USERS_TABLES").map(String::as_str),
            Some("$(BR2_EXTERNAL_NEPTRON_PATH)/board/common/users.table"),
            "{arch:?}"
        );
        assert_eq!(
            options
                .get("BR2_ROOTFS_POST_FAKEROOT_SCRIPT")
                .map(String::as_str),
            Some("$(BR2_EXTERNAL_NEPTRON_PATH)/board/common/post-fakeroot.sh"),
            "{arch:?}: the check of the account runs after Buildroot makes the users"
        );
    }
}

/// Runs the post-fakeroot check on a target with these passwd and shadow
/// lines for the share account (none for `None`).
fn account_check(name: &str, passwd: Option<&str>, shadow: Option<&str>) -> (bool, String) {
    let dir = TempDir::create(&format!("account-{name}"));
    let etc = dir.path("target/etc");
    std::fs::create_dir_all(&etc).unwrap();
    let with = |base: &str, line: Option<&str>| match line {
        Some(line) => format!("{base}{line}\n"),
        None => base.to_owned(),
    };
    std::fs::write(
        etc.join("passwd"),
        with("root:x:0:0:root:/root:/bin/sh\n", passwd),
    )
    .unwrap();
    std::fs::write(
        etc.join("shadow"),
        with("root::10933:0:99999:7:::\n", shadow),
    )
    .unwrap();
    let out = Command::new("sh")
        .arg(os_dir().join("board/common/post-fakeroot.sh"))
        .arg(dir.path("target"))
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn the_post_fakeroot_check_refuses_a_share_account_that_can_log_in() {
    let passwd = "neptron:x:1000:1000:NEPTRON network share account:/:/bin/false";
    let shadow = "neptron:*:::::::";
    let (ok, err) = account_check("good", Some(passwd), Some(shadow));
    assert!(ok, "{err}");
    let nologin = "neptron:x:1000:1000::/:/sbin/nologin";
    assert!(account_check("nologin", Some(nologin), Some("neptron:!:::::::")).0);
    for (name, passwd, shadow, fault) in [
        ("nouser", None, Some(shadow), "no user neptron"),
        ("noshadow", Some(passwd), None, "no shadow entry"),
        (
            "shell",
            Some("neptron:x:1000:1000::/:/bin/sh"),
            Some(shadow),
            "a login shell",
        ),
        (
            "hash",
            Some(passwd),
            Some("neptron:$1$abcdefgh$0123456789abcdefghijkl:::::::"),
            "a password",
        ),
        ("empty", Some(passwd), Some("neptron::::::::"), "a password"),
    ] {
        let (ok, err) = account_check(name, passwd, shadow);
        assert!(!ok, "{name}");
        assert!(err.contains(fault), "{name}: {err}");
    }
}

/// The x86-64 and Pi images have a read-only EROFS root, so init cannot
/// make the mount point of the 9p share at boot. The common post-build
/// step makes it in every image, and the mount script only mounts.
#[test]
fn every_image_has_the_mount_point_of_the_host_share() {
    let dir = TempDir::create("mnt-host");
    let (ok, err, _) = post_build(&dir, "auto lo\niface lo inet loopback\n", false);
    assert!(ok, "{err}");
    let mount_point = super::trace::GUEST_SHARE.trim_start_matches('/');
    let made = dir.path("target").join(mount_point);
    assert!(made.is_dir(), "{} is not a directory", made.display());
    assert_eq!(std::fs::read_dir(&made).unwrap().count(), 0, "not empty");
    let again = run_post_build(&dir);
    assert!(again.status.success(), "{again:?}");

    for arch in [Arch::Aarch64, Arch::X86_64] {
        assert!(
            defconfig(arch)["BR2_ROOTFS_POST_BUILD_SCRIPT"]
                .contains("$(BR2_EXTERNAL_NEPTRON_PATH)/board/common/post-build.sh"),
            "{arch:?}"
        );
    }
    let mounts = read(&format!("{OVERLAY}/etc/init.d/S05neptron-mounts"));
    assert!(
        mounts.contains(&format!("neptron {}\n", super::trace::GUEST_SHARE)),
        "{mounts}"
    );
    assert!(!mounts.contains("mkdir"), "the root is read-only: {mounts}");
}

/// Every image runs the address script from a hook of udhcpc and from the
/// network script: both are in the common overlay, executable, and the
/// overlay of every image (PC, Raspberry Pi, QEMU test image, and the
/// development variants) starts with the common overlay.
#[test]
fn every_image_has_the_executable_udhcpc_hook_of_the_addresses() {
    use std::os::unix::fs::PermissionsExt;
    for rel in [ADDRESS_HOOK, ADDRESS_SCRIPT] {
        let mode = std::fs::metadata(os_dir().join(rel))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "{rel} is not executable");
    }
    assert!(
        read(ADDRESS_HOOK)
            .contains("exec \"${NEPTRON_ADDRESS:-/usr/libexec/neptron/net-address}\" \"$@\""),
        "the hook passes the event on"
    );
    assert!(
        read(NETWORK_SCRIPT)
            .contains("ADDRESS=${NEPTRON_ADDRESS:-/usr/libexec/neptron/net-address}")
    );
    for rel in [
        "configs/neptron_pc_x86_64_defconfig",
        "configs/neptron_pc_x86_64_dev.fragment",
        "configs/neptron_rpi_arm64_defconfig",
        "configs/neptron_rpi_arm64_dev.fragment",
        "configs/neptron_qemu_aarch64_defconfig",
    ] {
        let options = parse(&read(rel));
        assert!(
            options["BR2_ROOTFS_OVERLAY"]
                .starts_with("$(BR2_EXTERNAL_NEPTRON_PATH)/board/common/rootfs-overlay "),
            "{rel}"
        );
        if let Some(scripts) = options.get("BR2_ROOTFS_POST_BUILD_SCRIPT") {
            assert!(
                scripts.contains("$(BR2_EXTERNAL_NEPTRON_PATH)/board/common/post-build.sh"),
                "{rel}: the check of the hook does not run"
            );
        }
    }
    assert!(
        !os_dir()
            .join("board/pc/dev-overlay/etc/init.d/S41neptron-address")
            .exists(),
        "the boot script that wrote the share file once is back"
    );
}

/// A folder for the address script: a development image (for the console
/// line), and fakes of the commands it calls.
fn address_dir(name: &str) -> TempDir {
    let dir = TempDir::create(name);
    std::fs::write(dir.path("development"), "").unwrap();
    dir.fake("hostname", "echo neptron");
    dir.fake("flock", FLOCK);
    dir.logging_fakes(&["logger"]);
    dir
}

/// `flock -n FD` and `flock -u FD` as BusyBox and util-linux run them,
/// through the flock(2) call of Perl. The tests always use this shim, on
/// every host, because macOS has no flock command. The lock is on the open
/// file of the descriptor that the script holds, so it stays after this
/// process ends, as with the real command.
const FLOCK: &str = r#"case $1 in
-n) op='LOCK_EX|LOCK_NB' ;;
-u) op=LOCK_UN ;;
*) exit 2 ;;
esac
exec perl -e 'use Fcntl qw(:flock); open(my $fh, ">&=", $ARGV[0]) or exit 2; flock($fh, '"$op"') or exit 1; exit 0' "$2""#;

/// Runs a udhcpc event through the hook of the image, as the udhcpc script
/// does: the event as the argument, the port and the address in the
/// environment.
fn lease(dir: &TempDir, event: &str, port: &str, address: &str) {
    let script = os_dir().join(ADDRESS_SCRIPT);
    let (ok, out) = dir.run_env(
        ADDRESS_HOOK,
        &[event],
        &[
            ("interface", port),
            ("ip", address),
            ("NEPTRON_ADDRESS", script.to_str().unwrap()),
        ],
    );
    assert!(ok, "{event} {port} {address}: {out}");
}

fn address_script(dir: &TempDir, args: &[&str]) {
    let (ok, out) = dir.run_args(ADDRESS_SCRIPT, args);
    assert!(ok, "{args:?}: {out}");
}

fn network_toml(dir: &TempDir) -> String {
    std::fs::read_to_string(dir.path("run/network.toml")).unwrap()
}

fn inode(path: &Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).unwrap().ino()
}

/// The run file for the frontend with these ports and addresses.
fn toml_with(addresses: &[(&str, &str)]) -> String {
    let mut text = "version = 1\nhost_name = \"neptron\"\n".to_owned();
    for (port, address) in addresses {
        text.push_str(&format!(
            "\n[[address]]\nport = \"{port}\"\nipv4 = \"{address}\"\n"
        ));
    }
    text
}

/// The lock is free: a run that must write gets it and writes.
fn assert_lock_free(dir: &TempDir) {
    // A file that is not there is fine: the run writes it again.
    let _ = std::fs::remove_file(dir.path("run/network.toml"));
    let (ok, out) = dir.run_args(ADDRESS_SCRIPT, &["start"]);
    assert!(ok, "{out}");
    assert!(
        dir.path("run/network.toml").exists(),
        "the lock is still held"
    );
    assert!(!dir.path("run/address.dirty").exists());
}

/// No hidden temp file is left beside the run file.
fn no_temp_files(dir: &TempDir) {
    for entry in std::fs::read_dir(dir.path("run")).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        assert!(!name.ends_with(".new"), "run/{name}");
    }
}

/// A lease writes its address to the run file, with mode 0644 under the
/// umask 0 of udhcpc, a syslog line, and the console line of the
/// development image.
#[test]
fn a_lease_writes_the_address_to_the_run_file() {
    use std::os::unix::fs::PermissionsExt;
    let dir = address_dir("address-bound");
    lease(&dir, "bound", "eth0", "192.168.0.112");
    assert_eq!(network_toml(&dir), toml_with(&[("eth0", "192.168.0.112")]));
    let mode = std::fs::metadata(dir.path("run/network.toml"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o644);
    assert_eq!(
        std::fs::read_to_string(dir.path("console")).unwrap(),
        "neptron: address eth0 192.168.0.112\n"
    );
    assert!(
        dir.log()
            .contains("logger -t neptron-address addresses: eth0 192.168.0.112\n"),
        "{}",
        dir.log()
    );
    no_temp_files(&dir);
}

/// A renew with the same address writes nothing (same inode, no console
/// line); a renew with a new address writes the file again.
#[test]
fn a_renew_writes_only_a_changed_address() {
    let dir = address_dir("address-renew");
    lease(&dir, "bound", "eth0", "192.168.0.112");
    let run = dir.path("run/network.toml");
    let before = inode(&run);
    std::fs::remove_file(dir.path("console")).unwrap();
    lease(&dir, "renew", "eth0", "192.168.0.112");
    assert_eq!(inode(&run), before, "an unchanged renew wrote the run file");
    assert!(!dir.path("console").exists());

    lease(&dir, "renew", "eth0", "192.168.0.120");
    assert_ne!(inode(&run), before);
    assert_eq!(network_toml(&dir), toml_with(&[("eth0", "192.168.0.120")]));
    no_temp_files(&dir);
}

/// A lost lease (deconfig, a failed or refused lease) and a removed port
/// leave no address in the run file.
#[test]
fn a_lost_lease_or_a_removed_port_gives_no_address() {
    for event in ["deconfig", "leasefail", "nak", "port-gone"] {
        let dir = address_dir(&format!("address-{event}"));
        lease(&dir, "bound", "eth0", "192.168.0.112");
        if event == "port-gone" {
            address_script(&dir, &["port-gone", "eth0"]);
        } else {
            lease(&dir, event, "eth0", "");
        }
        assert_eq!(network_toml(&dir), toml_with(&[]), "{event}");
        assert_eq!(
            std::fs::read_to_string(dir.path("console")).unwrap(),
            "neptron: address none\n",
            "{event}"
        );
    }
}

/// Two ports: one table for each address, in the order of the port names;
/// a port that loses its lease leaves the other one.
#[test]
fn two_ports_give_one_table_each() {
    let dir = address_dir("address-two");
    lease(&dir, "bound", "eth0", "192.168.0.112");
    lease(&dir, "bound", "enx001122334455", "10.0.0.2");
    assert_eq!(
        network_toml(&dir),
        toml_with(&[("enx001122334455", "10.0.0.2"), ("eth0", "192.168.0.112")])
    );
    lease(&dir, "deconfig", "enx001122334455", "");
    assert_eq!(network_toml(&dir), toml_with(&[("eth0", "192.168.0.112")]));
}

/// The boot writes the run file with no lease, so the frontend always finds
/// it. Nothing goes to the share: an old neptron-ip.txt of an older image
/// stays as the user left it.
#[test]
fn the_boot_writes_the_run_file_and_nothing_goes_to_the_share() {
    let dir = address_dir("address-boot");
    std::fs::create_dir_all(dir.path("share")).unwrap();
    std::fs::write(dir.path("share/neptron-ip.txt"), "192.168.0.99\n").unwrap();
    address_script(&dir, &["start"]);
    assert_eq!(network_toml(&dir), toml_with(&[]));
    lease(&dir, "bound", "eth0", "192.168.0.112");
    lease(&dir, "deconfig", "eth0", "");
    assert_eq!(
        std::fs::read_to_string(dir.path("share/neptron-ip.txt")).unwrap(),
        "192.168.0.99\n"
    );
    assert!(
        !read(ADDRESS_SCRIPT).contains("/share"),
        "the address script names the share"
    );
}

/// The release image writes the run file and prints no console line.
#[test]
fn the_release_image_prints_no_console_line() {
    let dir = address_dir("address-release");
    std::fs::remove_file(dir.path("development")).unwrap();
    lease(&dir, "bound", "eth0", "192.168.0.112");
    assert!(!dir.path("console").exists());
    assert_eq!(network_toml(&dir), toml_with(&[("eth0", "192.168.0.112")]));
}

/// The values come from a DHCP server and from mdev: only a wired port name
/// and a dotted IPv4 address are kept.
#[test]
fn the_address_script_keeps_only_a_wired_port_and_an_ipv4_address() {
    let dir = address_dir("address-invalid");
    for (port, address) in [
        ("wlan0", "192.168.0.112"),
        ("eth0/../x", "192.168.0.112"),
        ("eth0", "192.168.0.112\"\nx = 1"),
        ("eth0", "192.168.0"),
        ("eth0", "1.2.3.4.5"),
        ("eth0", "1..2.3"),
        ("eth0", "1.2.3.4."),
        ("eth0", ".1.2.3"),
        ("eth0", "999.1.1.1"),
        ("eth0", "1.2.3.256"),
        ("eth0", "0001.2.3.4"),
        ("eth0", "1.2.3.-4"),
        ("eth0", ""),
    ] {
        lease(&dir, "bound", port, address);
        assert_eq!(network_toml(&dir), toml_with(&[]), "{port} {address}");
    }
    for address in ["0.0.0.0", "255.255.255.255", "10.0.2.15"] {
        lease(&dir, "bound", "eth0", address);
        assert_eq!(network_toml(&dir), toml_with(&[("eth0", address)]));
    }
    let (ok, out) = dir.run_args(ADDRESS_SCRIPT, &["reboot"]);
    assert!(!ok, "{out}");
}

/// A write of the run file that fails: the hook still exits 0, so udhcpc
/// goes on; syslog has the line, and no temp file is left.
#[test]
fn a_failed_write_of_the_run_file_is_logged_and_leaves_no_temp_file() {
    let dir = address_dir("address-write-fails");
    dir.fake(
        "mv",
        "case \"$*\" in *network.toml*) exit 1 ;; esac\nPATH=${PATH#*:} exec mv \"$@\"",
    );
    lease(&dir, "bound", "eth0", "192.168.0.112");
    assert!(
        dir.log().contains("network.toml cannot be written"),
        "{}",
        dir.log()
    );
    assert!(!dir.path("run/network.toml").exists());
    assert!(!dir.path("console").exists());
    assert!(
        dir.path("run/address.d/eth0").exists(),
        "the record is kept"
    );
    no_temp_files(&dir);
    // The next event writes the kept record.
    std::fs::remove_file(dir.path("bin/mv")).unwrap();
    assert_lock_free(&dir);
    assert_eq!(network_toml(&dir), toml_with(&[("eth0", "192.168.0.112")]));
}

/// udhcpc waits for its script, so an event never waits for the lock: a
/// second event while the first one writes records its port, leaves the
/// flag and returns at once. The holder writes again before it ends, so
/// the file holds both addresses.
#[test]
fn an_event_returns_while_the_writer_is_blocked() {
    let dir = address_dir("address-race");
    let reached = dir.path("reached");
    let gate = dir.path("gate");
    mkfifo(&reached);
    mkfifo(&gate);
    // The first look at the host name blocks until the test opens the gate.
    dir.fake(
        "hostname",
        &format!(
            "if [ ! -e \"{once}\" ]; then : > \"{once}\"; echo in > \"{reached}\"; read -r _ < \"{gate}\"; fi\necho neptron",
            once = dir.path("once").display(),
            reached = reached.display(),
            gate = gate.display()
        ),
    );
    std::thread::scope(|scope| {
        let first = scope.spawn(|| lease(&dir, "bound", "eth0", "192.168.0.112"));
        assert_eq!(read_fifo(&reached), "in\n");
        // The writer holds the lock and is blocked: this event must return.
        lease(&dir, "bound", "eth1", "192.168.0.113");
        assert!(dir.path("run/address.dirty").exists());
        assert!(!dir.path("run/network.toml").exists());
        write_fifo(&gate, "go\n");
        first.join().unwrap();
    });
    assert_eq!(
        network_toml(&dir),
        toml_with(&[("eth0", "192.168.0.112"), ("eth1", "192.168.0.113")])
    );
    assert_lock_free(&dir);
    assert!(!dir.path("run/address.dirty").exists());
}

/// The lock is a flock, which the kernel frees when its holder ends. A
/// lock file that a killed run left behind does not block the next event.
#[test]
fn a_lock_file_left_by_a_dead_run_does_not_block() {
    let dir = address_dir("address-old-lock");
    std::fs::create_dir_all(dir.path("run")).unwrap();
    std::fs::write(dir.path("run/address.lock"), "4242\n").unwrap();
    lease(&dir, "bound", "eth0", "192.168.0.112");
    assert_eq!(network_toml(&dir), toml_with(&[("eth0", "192.168.0.112")]));
    assert!(!dir.path("run/address.dirty").exists());
}

/// A lock file that cannot be opened (here a folder at its path), or a
/// flock that cannot run: the hook still exits 0, so udhcpc goes on, and
/// syslog says why nothing was written.
#[test]
fn a_lock_that_cannot_be_taken_is_logged_and_the_hook_exits_0() {
    let dir = address_dir("address-lock-dir");
    std::fs::create_dir_all(dir.path("run/address.lock")).unwrap();
    lease(&dir, "bound", "eth0", "192.168.0.112");
    assert!(
        dir.log().contains("address.lock cannot be opened"),
        "{}",
        dir.log()
    );
    assert!(!dir.path("run/network.toml").exists());

    let dir = address_dir("address-no-flock");
    dir.fake("flock", "exit 127");
    lease(&dir, "bound", "eth0", "192.168.0.112");
    assert!(dir.log().contains("flock cannot run"), "{}", dir.log());
    assert!(!dir.path("run/network.toml").exists());
}

/// The holder looks at the flag again after it freed the lock. An event
/// that comes after the last look of the inner loop and before the unlock
/// finds the lock taken and returns; that look writes its address.
#[test]
fn an_event_just_before_the_unlock_is_written_by_the_holder() {
    let dir = address_dir("address-unlock");
    let reached = dir.path("reached");
    let gate = dir.path("gate");
    mkfifo(&reached);
    mkfifo(&gate);
    // The first unlock blocks until the test opens the gate.
    dir.fake(
        "flock",
        &format!(
            "if [ \"$1\" = -u ] && [ ! -e \"{once}\" ]; then : > \"{once}\"; echo in > \"{reached}\"; read -r _ < \"{gate}\"; fi\n{FLOCK}",
            once = dir.path("once").display(),
            reached = reached.display(),
            gate = gate.display()
        ),
    );
    std::thread::scope(|scope| {
        let first = scope.spawn(|| lease(&dir, "bound", "eth0", "192.168.0.112"));
        assert_eq!(read_fifo(&reached), "in\n");
        assert_eq!(network_toml(&dir), toml_with(&[("eth0", "192.168.0.112")]));
        lease(&dir, "bound", "eth1", "192.168.0.113");
        assert!(dir.path("run/address.dirty").exists());
        write_fifo(&gate, "go\n");
        first.join().unwrap();
    });
    assert_eq!(
        network_toml(&dir),
        toml_with(&[("eth0", "192.168.0.112"), ("eth1", "192.168.0.113")])
    );
    assert!(!dir.path("run/address.dirty").exists());
}

/// A flag that cannot be removed would keep the holder in its loop: it
/// writes once, logs the fault and returns.
#[test]
fn a_flag_that_cannot_be_removed_ends_the_loop() {
    let dir = address_dir("address-flag");
    dir.fake(
        "rm",
        "case \"$*\" in *address.dirty*) exit 1 ;; esac\nPATH=${PATH#*:} exec rm \"$@\"",
    );
    lease(&dir, "bound", "eth0", "192.168.0.112");
    assert_eq!(network_toml(&dir), toml_with(&[("eth0", "192.168.0.112")]));
    assert!(
        dir.log().contains("address.dirty cannot be removed"),
        "{}",
        dir.log()
    );
    assert_eq!(dir.log().matches("addresses: ").count(), 1, "{}", dir.log());
}

/// The network script runs the address script at boot and when a port
/// goes; the stop leaves it to the releases of the clients.
#[test]
fn the_network_script_runs_the_address_script() {
    let dir = TempDir::create("address-network");
    dhcp_fakes(&dir);
    dir.logging_fakes(&["net-address"]);
    std::fs::create_dir_all(dir.path("sys/eth0")).unwrap();
    // The client runs in the background and logs after the script returns.
    // It writes to a FIFO after its log line, so the test can wait for it
    // before it empties the log.
    let started = dir.path("started-eth0");
    mkfifo(&started);
    dir.fake(
        "udhcpc",
        &format!(
            "echo \"udhcpc $*\" >> \"$LOG\"\necho done > \"{}\"",
            started.display()
        ),
    );
    let (ok, out) = dir.run(NETWORK_SCRIPT, "start");
    assert!(ok, "{out}");
    assert!(dir.log().contains("net-address start\n"), "{}", dir.log());
    read_fifo(&started);

    std::fs::write(dir.path("log"), "").unwrap();
    let (ok, out) = dir.run_args(NETWORK_SCRIPT, &["port-gone", "eth0"]);
    assert!(ok, "{out}");
    assert_eq!(dir.log(), "net-address port-gone eth0\n");

    std::fs::write(dir.path("log"), "").unwrap();
    let (ok, out) = dir.run(NETWORK_SCRIPT, "stop");
    assert!(ok, "{out}");
    assert!(!dir.log().contains("net-address"), "{}", dir.log());
}

/// A stopped client runs its deconfig event before it ends. The network
/// script waits until it ended (at most 10 pauses), so that late event
/// cannot remove the address of a new client of a port that came back with
/// the same name.
#[test]
fn a_stopped_client_is_waited_for() {
    for (name, ends_after, pauses) in [("ends", Some(3), 3), ("stays", None, 10)] {
        let dir = TempDir::create(&format!("dhcp-wait-{name}"));
        std::fs::create_dir_all(dir.path("run/dhcp")).unwrap();
        let client = Process::start();
        fake_client(&dir, client.pid(), "eth0");
        std::fs::write(dir.path("run/dhcp/eth0.pid"), format!("{}\n", client.pid())).unwrap();
        dhcp_fakes(&dir);
        // The fake /proc shows the client until the pause that ends it.
        let cmdline = dir.path(&format!("proc/{}/cmdline", client.pid()));
        let end = ends_after.map_or(String::new(), |n| {
            format!(
                "[ \"$(grep -c usleep \"$LOG\")\" -ge {n} ] && rm -f \"{}\"",
                cmdline.display()
            )
        });
        dir.fake("usleep", &format!("echo \"usleep $*\" >> \"$LOG\"\n{end}"));
        let (ok, out) = dir.run_args(NETWORK_SCRIPT, &["port-gone", "eth0"]);
        assert!(ok, "{name}: {out}");
        assert_eq!(
            dir.log().matches("usleep 100000").count(),
            pauses,
            "{name}: {}",
            dir.log()
        );
        assert_eq!(
            out.contains("still runs after 10 pauses"),
            ends_after.is_none(),
            "{name}: {out}"
        );
        assert!(!dir.path("run/dhcp/eth0.pid").exists(), "{name}");
    }
}
