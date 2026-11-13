//! The images that `os build` makes. Each is also an emulated target of
//! QEMU ([`Arch`]).

use super::{Arch, OsError};

/// An image of `os build`, named by the value of `--arch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Board {
    /// The aarch64 QEMU test image (`--arch aarch64`).
    QemuAarch64,
    /// The x86-64 PC image, which QEMU boots too (`--arch x86_64`).
    Pc,
}

impl Board {
    /// Parses the name that `--arch` takes in `os build`.
    pub fn parse(name: &str) -> Result<Self, OsError> {
        Arch::parse(name).map(Self::of_arch)
    }

    /// The image of an emulated target.
    pub fn of_arch(arch: Arch) -> Self {
        match arch {
            Arch::Aarch64 => Self::QemuAarch64,
            Arch::X86_64 => Self::Pc,
        }
    }

    /// The CPU architecture: it names the Buildroot cross compiler and the
    /// Rust target of the guest programs.
    pub fn arch(self) -> Arch {
        match self {
            Self::QemuAarch64 => Arch::Aarch64,
            Self::Pc => Arch::X86_64,
        }
    }

    /// The name of `--arch`, of the output volume and of the images
    /// directory.
    pub fn name(self) -> &'static str {
        self.arch().name()
    }

    /// The Buildroot defconfig, in `os/configs`.
    pub fn defconfig(self) -> &'static str {
        self.arch().defconfig()
    }

    /// The configuration fragment of the development variant, in
    /// `os/configs`, for an image that has one.
    pub fn dev_fragment(self) -> Option<&'static str> {
        match self {
            Self::QemuAarch64 => None,
            Self::Pc => Some("neptron_pc_x86_64_dev.fragment"),
        }
    }

    /// The disk image file that the post-image step writes, for an image
    /// that goes to a USB stick or an SD card.
    pub fn disk_image(self) -> Option<&'static str> {
        match self {
            Self::QemuAarch64 => None,
            Self::Pc => Some("neptron-x86_64.img"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Board; 2] = [Board::QemuAarch64, Board::Pc];

    #[test]
    fn names_parse_and_round_trip() {
        for board in ALL {
            assert_eq!(Board::parse(board.name()).unwrap(), board);
        }
        assert_eq!(Board::parse("arm64").unwrap(), Board::QemuAarch64);
        assert_eq!(Board::parse("amd64").unwrap(), Board::Pc);
        let err = Board::parse("riscv64").unwrap_err();
        assert!(err.to_string().contains("riscv64"), "{err}");
    }

    #[test]
    fn the_qemu_targets_keep_their_names_and_outputs() {
        for arch in [Arch::Aarch64, Arch::X86_64] {
            let board = Board::of_arch(arch);
            assert_eq!(board.arch(), arch);
            assert_eq!(board.name(), arch.name());
            assert_eq!(board.defconfig(), arch.defconfig());
        }
        assert_eq!(Board::Pc.disk_image(), Some("neptron-x86_64.img"));
        assert_eq!(Board::QemuAarch64.dev_fragment(), None);
    }

    #[test]
    fn every_defconfig_and_dev_fragment_is_a_file_in_os_configs() {
        let configs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../os/configs");
        for board in ALL {
            for name in std::iter::once(board.defconfig()).chain(board.dev_fragment()) {
                let path = configs.join(name);
                assert!(path.is_file(), "{} is missing", path.display());
            }
        }
    }

    #[test]
    fn every_disk_image_is_the_image_of_its_genimage_file() {
        let board_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../os/board");
        let text = std::fs::read_to_string(board_dir.join("pc/genimage.cfg")).unwrap();
        let image = Board::Pc.disk_image().unwrap();
        assert!(text.contains(&format!("image {image} {{")), "pc: {image}");
    }
}
