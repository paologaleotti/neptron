//! The two emulated targets and what differs between them.

use super::OsError;

/// The architecture of an emulated target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    /// 64-bit Arm on the QEMU `virt` machine. Runs at native speed on an
    /// Apple Silicon Mac.
    Aarch64,
    /// x86-64 on the QEMU `q35` machine. Runs at native speed on a Linux
    /// x86-64 host with KVM.
    X86_64,
}

impl Arch {
    /// Parses the name that `--arch` takes.
    pub fn parse(name: &str) -> Result<Self, OsError> {
        match name {
            "aarch64" | "arm64" => Ok(Self::Aarch64),
            "x86_64" | "x86-64" | "amd64" => Ok(Self::X86_64),
            other => Err(OsError::BadArch(other.to_owned())),
        }
    }

    /// The architecture of a host, from a value of `std::env::consts::ARCH`.
    pub fn of_host(host_arch: &str) -> Option<Self> {
        match host_arch {
            "aarch64" => Some(Self::Aarch64),
            "x86_64" => Some(Self::X86_64),
            _ => None,
        }
    }

    /// The default target on a host: the host's own architecture, else aarch64.
    pub fn default_for(host_arch: &str) -> Self {
        Self::of_host(host_arch).unwrap_or(Self::Aarch64)
    }

    /// The canonical name, as in the defconfig and the cache paths.
    pub fn name(self) -> &'static str {
        match self {
            Self::Aarch64 => "aarch64",
            Self::X86_64 => "x86_64",
        }
    }

    /// The Buildroot defconfig of the QEMU target, in `os/configs`.
    pub fn defconfig(self) -> String {
        format!("neptron_qemu_{}_defconfig", self.name())
    }

    /// The kernel file in the Buildroot images directory.
    pub fn kernel_image(self) -> &'static str {
        match self {
            Self::Aarch64 => "Image",
            Self::X86_64 => "bzImage",
        }
    }

    /// The QEMU system emulator for this target.
    pub fn qemu_binary(self) -> &'static str {
        match self {
            Self::Aarch64 => "qemu-system-aarch64",
            Self::X86_64 => "qemu-system-x86_64",
        }
    }

    /// The serial console device of the guest kernel.
    pub fn console(self) -> &'static str {
        match self {
            Self::Aarch64 => "ttyAMA0",
            Self::X86_64 => "ttyS0",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_parse_and_round_trip() {
        for arch in [Arch::Aarch64, Arch::X86_64] {
            assert_eq!(Arch::parse(arch.name()).unwrap(), arch);
        }
        assert_eq!(Arch::parse("arm64").unwrap(), Arch::Aarch64);
        assert_eq!(Arch::parse("amd64").unwrap(), Arch::X86_64);
    }

    #[test]
    fn unknown_name_is_rejected_with_the_name() {
        let err = Arch::parse("riscv64").unwrap_err();
        assert!(err.to_string().contains("riscv64"), "{err}");
    }

    #[test]
    fn default_follows_the_host() {
        assert_eq!(Arch::default_for("aarch64"), Arch::Aarch64);
        assert_eq!(Arch::default_for("x86_64"), Arch::X86_64);
        assert_eq!(Arch::default_for("riscv64"), Arch::Aarch64);
    }

    #[test]
    fn defconfig_names_match_the_files_in_os_configs() {
        let configs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../os/configs");
        for arch in [Arch::Aarch64, Arch::X86_64] {
            let path = configs.join(arch.defconfig());
            assert!(path.is_file(), "{} is missing", path.display());
        }
    }
}
