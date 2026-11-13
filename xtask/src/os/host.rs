//! Facts about the host: where its tools are, which accelerator QEMU can use,
//! and where the shared caches live.

use std::ffi::OsStr;
use std::fs::{self, File, TryLockError};
use std::path::{Path, PathBuf};

use super::{Arch, OsError};

/// Environment variable that moves the cache directory from `~/.cache/neptron`.
pub const CACHE_ENV: &str = "NEPTRON_CACHE_DIR";
/// Environment variable that names an images directory to boot, for example
/// one fetched from CI, in place of the local build output.
pub const IMAGES_ENV: &str = "NEPTRON_OS_IMAGES";

/// How QEMU runs the guest CPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accel {
    /// The macOS hypervisor: native speed for an aarch64 guest on Apple Silicon.
    Hvf,
    /// The Linux hypervisor: native speed when guest and host match.
    Kvm,
    /// Software emulation: correct, but slow.
    Tcg,
}

impl Accel {
    /// Picks the fastest accelerator for `guest` on a host with this OS and
    /// CPU (values of `std::env::consts`).
    pub fn choose(guest: Arch, host_os: &str, host_arch: &str, kvm_available: bool) -> Self {
        let native = Arch::of_host(host_arch) == Some(guest);
        match host_os {
            "macos" if native => Self::Hvf,
            "linux" if native && kvm_available => Self::Kvm,
            _ => Self::Tcg,
        }
    }

    /// The value of QEMU's `-accel` option.
    pub fn name(self) -> &'static str {
        match self {
            Self::Hvf => "hvf",
            Self::Kvm => "kvm",
            Self::Tcg => "tcg",
        }
    }
}

/// The directories that NEPTRON shares between worktrees on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cache {
    /// The root, `~/.cache/neptron` by default.
    pub root: PathBuf,
}

impl Cache {
    /// Finds the cache root from `NEPTRON_CACHE_DIR` or `HOME`.
    pub fn from_env() -> Result<Self, OsError> {
        Self::resolve(
            std::env::var_os(CACHE_ENV).as_deref(),
            std::env::var_os("HOME").as_deref(),
        )
    }

    /// Builds the cache root from the raw values of `NEPTRON_CACHE_DIR` and `HOME`.
    pub fn resolve(cache_env: Option<&OsStr>, home: Option<&OsStr>) -> Result<Self, OsError> {
        match (cache_env, home) {
            (Some(dir), _) if !dir.is_empty() => Ok(Self {
                root: PathBuf::from(dir),
            }),
            (_, Some(home)) if !home.is_empty() => Ok(Self {
                root: Path::new(home).join(".cache").join("neptron"),
            }),
            _ => Err(OsError::NoHome),
        }
    }

    /// Where `os build` puts the images of `arch` for QEMU.
    pub fn images(&self, arch: Arch) -> PathBuf {
        self.images_dir(arch.name())
    }

    /// The images directory of the variant `name`, for example
    /// `x86_64-dev`.
    pub fn images_dir(&self, name: &str) -> PathBuf {
        self.root.join("buildroot").join("images").join(name)
    }

    /// The lock that allows one Buildroot build at a time on this machine.
    pub fn build_lock(&self) -> PathBuf {
        self.root.join("buildroot.lock")
    }

    /// The lock that allows one QEMU guest at a time on this machine.
    pub fn qemu_lock(&self) -> PathBuf {
        self.root.join("qemu.lock")
    }
}

/// A held machine-wide lock. The OS releases it when the file closes.
#[derive(Debug)]
pub struct MachineLock {
    _file: File,
}

impl MachineLock {
    /// Takes the lock at `path`, and waits with a message when another
    /// process holds it.
    pub fn take(path: &Path, what: &str) -> Result<Self, OsError> {
        let lock_err = |source| OsError::Io {
            path: path.to_path_buf(),
            source,
        };
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(lock_err)?;
        }
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .map_err(lock_err)?;
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                eprintln!("xtask: another {what} is running, waiting for it to end");
                file.lock().map_err(lock_err)?;
            }
            Err(TryLockError::Error(source)) => return Err(lock_err(source)),
        }
        Ok(Self { _file: file })
    }
}

/// Finds `tool` in the directories of a `PATH` value.
pub fn find_tool(tool: &str, path_var: Option<&OsStr>) -> Option<PathBuf> {
    let path_var = path_var?;
    std::env::split_paths(path_var)
        .map(|dir| dir.join(tool))
        .find(|candidate| candidate.is_file())
}

/// Fails with an install hint when `tool` is not on the `PATH`.
pub fn require_tool(tool: &str, hint: &'static str) -> Result<PathBuf, OsError> {
    find_tool(tool, std::env::var_os("PATH").as_deref()).ok_or_else(|| OsError::ToolMissing {
        tool: tool.to_owned(),
        hint,
    })
}

/// Whether this host offers KVM to the current user.
pub fn kvm_available() -> bool {
    File::options()
        .read(true)
        .write(true)
        .open("/dev/kvm")
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_runs_aarch64_on_hvf_and_x86_on_tcg() {
        assert_eq!(
            Accel::choose(Arch::Aarch64, "macos", "aarch64", false),
            Accel::Hvf
        );
        assert_eq!(
            Accel::choose(Arch::X86_64, "macos", "aarch64", false),
            Accel::Tcg
        );
    }

    #[test]
    fn linux_uses_kvm_only_when_present_and_native() {
        assert_eq!(
            Accel::choose(Arch::X86_64, "linux", "x86_64", true),
            Accel::Kvm
        );
        assert_eq!(
            Accel::choose(Arch::X86_64, "linux", "x86_64", false),
            Accel::Tcg
        );
        assert_eq!(
            Accel::choose(Arch::Aarch64, "linux", "x86_64", true),
            Accel::Tcg
        );
    }

    #[test]
    fn cache_env_wins_over_home() {
        let c = Cache::resolve(Some(OsStr::new("/c")), Some(OsStr::new("/h"))).unwrap();
        assert_eq!(c.root, PathBuf::from("/c"));
        let c = Cache::resolve(None, Some(OsStr::new("/h"))).unwrap();
        assert_eq!(c.root, PathBuf::from("/h/.cache/neptron"));
        assert_eq!(
            c.images(Arch::X86_64),
            PathBuf::from("/h/.cache/neptron/buildroot/images/x86_64")
        );
        assert_eq!(
            c.images_dir("x86_64-dev"),
            PathBuf::from("/h/.cache/neptron/buildroot/images/x86_64-dev")
        );
    }

    #[test]
    fn empty_values_count_as_unset() {
        let c = Cache::resolve(Some(OsStr::new("")), Some(OsStr::new("/h"))).unwrap();
        assert_eq!(c.root, PathBuf::from("/h/.cache/neptron"));
        assert!(matches!(
            Cache::resolve(None, Some(OsStr::new(""))),
            Err(OsError::NoHome)
        ));
    }

    #[test]
    fn missing_tool_is_not_found() {
        let dir = std::env::temp_dir();
        let path = std::env::join_paths([&dir]).unwrap();
        assert_eq!(find_tool("neptron-no-such-tool", Some(&path)), None);
        assert_eq!(find_tool("sh", None), None);
    }

    #[test]
    fn present_tool_is_found() {
        let dir = std::env::temp_dir().join(format!("xtask-tool-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("fake-qemu"), "").unwrap();
        let path = std::env::join_paths([Path::new("/nonexistent"), &dir]).unwrap();
        assert_eq!(
            find_tool("fake-qemu", Some(&path)),
            Some(dir.join("fake-qemu"))
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
