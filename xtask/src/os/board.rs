//! The images that `os build` makes. Two of them are also the emulated
//! targets of QEMU ([`Arch`]); the Raspberry Pi image runs on hardware
//! only.

use super::{Arch, OsError};

/// An image of `os build`, named by the value of `--arch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Board {
    /// The aarch64 QEMU test image (`--arch aarch64`).
    QemuAarch64,
    /// The x86-64 PC image, which QEMU boots too (`--arch x86_64`).
    Pc,
    /// The 64-bit Raspberry Pi image for the Pi 3 B and 3 B+
    /// (`--arch rpi-arm64`). QEMU does not boot it.
    RpiArm64,
}

impl Board {
    /// Parses the name that `--arch` takes in `os build` and the bench
    /// commands: a QEMU target, or `rpi-arm64`.
    pub fn parse(name: &str) -> Result<Self, OsError> {
        match name {
            RPI_ARM64 => Ok(Self::RpiArm64),
            other => Arch::parse(other)
                .map(Self::of_arch)
                .map_err(|_| OsError::BadBoard(other.to_owned())),
        }
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
            Self::QemuAarch64 | Self::RpiArm64 => Arch::Aarch64,
            Self::Pc => Arch::X86_64,
        }
    }

    /// The name of `--arch`, of the output volume and of the images
    /// directory.
    pub fn name(self) -> &'static str {
        match self {
            Self::QemuAarch64 | Self::Pc => self.arch().name(),
            Self::RpiArm64 => RPI_ARM64,
        }
    }

    /// The Buildroot defconfig, in `os/configs`.
    pub fn defconfig(self) -> &'static str {
        match self {
            Self::QemuAarch64 | Self::Pc => self.arch().defconfig(),
            Self::RpiArm64 => "neptron_rpi_arm64_defconfig",
        }
    }

    /// The configuration fragment of the development variant, in
    /// `os/configs`, for an image that has one.
    pub fn dev_fragment(self) -> Option<&'static str> {
        match self {
            Self::QemuAarch64 => None,
            Self::Pc => Some("neptron_pc_x86_64_dev.fragment"),
            Self::RpiArm64 => Some("neptron_rpi_arm64_dev.fragment"),
        }
    }

    /// The disk image file that the post-image step writes, for an image
    /// that goes to a USB stick or an SD card.
    pub fn disk_image(self) -> Option<&'static str> {
        match self {
            Self::QemuAarch64 => None,
            Self::Pc => Some("neptron-x86_64.img"),
            Self::RpiArm64 => Some("neptron-rpi-arm64.img"),
        }
    }

    /// The Buildroot output whose toolchain and sysroot `os push` uses:
    /// the output that built the development image of the board. The PC
    /// development image of the bench was built in the release output
    /// (`os/README.md`, "The output now holds the development packages").
    pub fn push_output(self) -> String {
        match self {
            Self::QemuAarch64 | Self::Pc => self.name().to_owned(),
            Self::RpiArm64 => format!("{RPI_ARM64}{}", super::build::DEV_SUFFIX),
        }
    }
}

/// The `--arch` name of the Raspberry Pi image.
const RPI_ARM64: &str = "rpi-arm64";

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Board; 3] = [Board::QemuAarch64, Board::Pc, Board::RpiArm64];

    #[test]
    fn names_parse_and_round_trip() {
        for board in ALL {
            assert_eq!(Board::parse(board.name()).unwrap(), board);
        }
        assert_eq!(Board::parse("arm64").unwrap(), Board::QemuAarch64);
        assert_eq!(Board::parse("amd64").unwrap(), Board::Pc);
        let err = Board::parse("rpi-arm32").unwrap_err();
        assert!(err.to_string().contains("rpi-arm32"), "{err}");
        assert!(err.to_string().contains("rpi-arm64"), "{err}");
    }

    #[test]
    fn the_pi_image_is_an_aarch64_image_of_its_own() {
        assert_eq!(Board::RpiArm64.arch(), Arch::Aarch64);
        assert_eq!(Board::RpiArm64.name(), "rpi-arm64");
        assert_eq!(Board::RpiArm64.defconfig(), "neptron_rpi_arm64_defconfig");
        assert_eq!(Board::RpiArm64.disk_image(), Some("neptron-rpi-arm64.img"));
        assert_eq!(Board::RpiArm64.push_output(), "rpi-arm64-dev");
        assert!(
            Arch::parse("rpi-arm64").is_err(),
            "QEMU cannot boot the Pi image"
        );
    }

    #[test]
    fn the_qemu_targets_keep_their_names_and_outputs() {
        for arch in [Arch::Aarch64, Arch::X86_64] {
            let board = Board::of_arch(arch);
            assert_eq!(board.arch(), arch);
            assert_eq!(board.name(), arch.name());
            assert_eq!(board.defconfig(), arch.defconfig());
            assert_eq!(board.push_output(), arch.name());
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
        for (board, dir) in [(Board::Pc, "pc"), (Board::RpiArm64, "rpi")] {
            let text = std::fs::read_to_string(board_dir.join(dir).join("genimage.cfg")).unwrap();
            let image = board.disk_image().unwrap();
            assert!(
                text.contains(&format!("image {image} {{")),
                "{dir}: {image}"
            );
        }
    }
}
