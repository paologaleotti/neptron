//! Tests of the network files in `os/`: the DHCP client of every image.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::defconfig::parse;

fn os_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../os")
}

fn read(rel: &str) -> String {
    let path = os_dir().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

const NETWORK_SCRIPT: &str = "board/common/rootfs-overlay/etc/init.d/S40neptron-network";

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
        let status = Command::new("sh")
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
            .status()
            .unwrap();
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

/// Reads one line from a FIFO; the open blocks until a writer opens it.
fn read_fifo(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
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
