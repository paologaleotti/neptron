//! Reads Buildroot defconfig files, so that tests can check the two QEMU
//! targets against each other and against the pinned hashes.

use std::collections::BTreeMap;

/// The options of a defconfig: name to value, with quotes removed. An option
/// written as `# NAME is not set` maps to `n`.
pub fn parse(text: &str) -> BTreeMap<String, String> {
    let mut options = BTreeMap::new();
    for line in text.lines().map(str::trim) {
        if let Some(name) = line
            .strip_prefix("# ")
            .and_then(|rest| rest.strip_suffix(" is not set"))
        {
            options.insert(name.to_owned(), "n".to_owned());
        } else if let Some((name, value)) = line.split_once('=')
            && !line.starts_with('#')
        {
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(value);
            options.insert(name.to_owned(), value.to_owned());
        }
    }
    options
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::Arch;
    use std::path::{Path, PathBuf};

    fn os_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../os")
    }

    fn defconfig(arch: Arch) -> BTreeMap<String, String> {
        let path = os_dir().join("configs").join(arch.defconfig());
        parse(&std::fs::read_to_string(&path).unwrap())
    }

    #[test]
    fn parse_reads_values_quotes_and_unset_options() {
        let o = parse("BR2_A=y\nBR2_B=\"x y\"\n# BR2_C is not set\n# a comment\n\n");
        assert_eq!(o["BR2_A"], "y");
        assert_eq!(o["BR2_B"], "x y");
        assert_eq!(o["BR2_C"], "n");
        assert_eq!(o.len(), 3);
    }

    /// Options that must be equal in both QEMU targets, because they are
    /// decisions of the OS layer and not of the architecture.
    const SHARED: [&str; 12] = [
        "BR2_TOOLCHAIN_BUILDROOT_GLIBC",
        "BR2_INIT_BUSYBOX",
        "BR2_ROOTFS_DEVICE_CREATION_DYNAMIC_MDEV",
        "BR2_LINUX_KERNEL_CUSTOM_VERSION_VALUE",
        "BR2_PACKAGE_HOST_LINUX_HEADERS_CUSTOM_6_18",
        "BR2_LINUX_KERNEL_CONFIG_FRAGMENT_FILES",
        "BR2_ROOTFS_OVERLAY",
        "BR2_GLOBAL_PATCH_DIR",
        "BR2_DOWNLOAD_FORCE_CHECK_HASHES",
        "BR2_TARGET_GENERIC_GETTY",
        "BR2_TARGET_ROOTFS_EXT2_4",
        "BR2_TARGET_ROOTFS_EXT2_SIZE",
    ];

    #[test]
    fn both_targets_make_the_same_os_decisions() {
        let a = defconfig(Arch::Aarch64);
        let x = defconfig(Arch::X86_64);
        for name in SHARED {
            assert!(a.contains_key(name), "{name} missing in aarch64");
            assert_eq!(a.get(name), x.get(name), "{name} differs");
        }
        assert_eq!(a["BR2_aarch64"], "y");
        assert_eq!(x["BR2_x86_64"], "y");
    }

    #[test]
    fn the_kernel_is_6_18_lts_with_a_pinned_hash() {
        let version = &defconfig(Arch::Aarch64)["BR2_LINUX_KERNEL_CUSTOM_VERSION_VALUE"];
        assert!(version.starts_with("6.18."), "{version}");
        let tarball = format!("linux-{version}.tar.xz");
        for pkg in ["linux", "linux-headers"] {
            let path = os_dir()
                .join("patches")
                .join(pkg)
                .join(format!("{pkg}.hash"));
            let hashes = std::fs::read_to_string(&path).unwrap();
            let pinned = hashes.lines().any(|l| {
                let f: Vec<&str> = l.split_whitespace().collect();
                f.len() == 3 && f[0] == "sha256" && f[1].len() == 64 && f[2] == tarball
            });
            assert!(pinned, "{} has no sha256 for {tarball}", path.display());
        }
    }

    #[test]
    fn each_kernel_fragment_holds_its_concern() {
        let fragments = &defconfig(Arch::Aarch64)["BR2_LINUX_KERNEL_CONFIG_FRAGMENT_FILES"];
        for (file, option) in [
            ("video.config", "CONFIG_DRM_VKMS"),
            ("input.config", "CONFIG_INPUT_UINPUT"),
            ("sound.config", "CONFIG_SND_DUMMY"),
            ("host.config", "CONFIG_FW_CFG_SYSFS"),
        ] {
            let rel = format!("board/qemu/{file}");
            assert!(fragments.contains(&rel), "{rel} is not in the defconfig");
            let options = parse(&std::fs::read_to_string(os_dir().join(&rel)).unwrap());
            assert_eq!(options.get(option).map(String::as_str), Some("y"), "{rel}");
        }
        let video =
            parse(&std::fs::read_to_string(os_dir().join("board/qemu/video.config")).unwrap());
        assert_eq!(video["CONFIG_DRM_FBDEV_EMULATION"], "n");
        assert_eq!(video["CONFIG_FRAMEBUFFER_CONSOLE"], "n");
    }

    #[test]
    fn every_external_path_exists() {
        for arch in [Arch::Aarch64, Arch::X86_64] {
            for (name, value) in defconfig(arch) {
                for part in value.split_whitespace() {
                    if let Some(rel) = part.strip_prefix("$(BR2_EXTERNAL_NEPTRON_PATH)/") {
                        let path = os_dir().join(rel);
                        assert!(path.exists(), "{name}: {} is missing", path.display());
                    }
                }
            }
        }
    }
}
