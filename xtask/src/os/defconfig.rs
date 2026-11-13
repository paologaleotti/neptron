//! Reads Buildroot defconfig files, so that tests can check the aarch64
//! QEMU test image and the x86-64 PC image against each other and against
//! the pinned hashes.

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

    /// Options that must be equal in both targets, because they are
    /// decisions of the OS layer and not of the architecture or the board.
    const SHARED: [&str; 10] = [
        "BR2_TOOLCHAIN_BUILDROOT_GLIBC",
        "BR2_TOOLCHAIN_BUILDROOT_CXX",
        "BR2_INIT_BUSYBOX",
        "BR2_ROOTFS_DEVICE_CREATION_DYNAMIC_MDEV",
        "BR2_LINUX_KERNEL_CUSTOM_VERSION_VALUE",
        "BR2_PACKAGE_HOST_LINUX_HEADERS_CUSTOM_6_18",
        "BR2_GLOBAL_PATCH_DIR",
        "BR2_DOWNLOAD_FORCE_CHECK_HASHES",
        "BR2_TARGET_GENERIC_GETTY",
        "BR2_TARGET_ROOTFS_EXT2_4",
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

    fn fragment(rel: &str) -> BTreeMap<String, String> {
        parse(&std::fs::read_to_string(os_dir().join(rel)).unwrap())
    }

    #[test]
    fn each_kernel_fragment_of_the_qemu_image_holds_its_concern() {
        let fragments = &defconfig(Arch::Aarch64)["BR2_LINUX_KERNEL_CONFIG_FRAGMENT_FILES"];
        for (file, option) in [
            ("video.config", "CONFIG_DRM_VKMS"),
            ("input.config", "CONFIG_INPUT_UINPUT"),
            ("sound.config", "CONFIG_SND_DUMMY"),
            ("host.config", "CONFIG_FW_CFG_SYSFS"),
        ] {
            let rel = format!("board/qemu/{file}");
            assert!(fragments.contains(&rel), "{rel} is not in the defconfig");
            assert_eq!(
                fragment(&rel).get(option).map(String::as_str),
                Some("y"),
                "{rel}"
            );
        }
        let video = fragment("board/qemu/video.config");
        assert_eq!(video["CONFIG_DRM_FBDEV_EMULATION"], "n");
        assert_eq!(video["CONFIG_FRAMEBUFFER_CONSOLE"], "n");
    }

    #[test]
    fn each_kernel_fragment_of_the_pc_image_holds_its_concern() {
        let fragments = &defconfig(Arch::X86_64)["BR2_LINUX_KERNEL_CONFIG_FRAGMENT_FILES"];
        for (file, option, value) in [
            ("pc/video.config", "CONFIG_DRM_I915", "y"),
            ("pc/video.config", "CONFIG_DRM_AMDGPU", "m"),
            ("pc/video.config", "CONFIG_DRM_VKMS", "m"),
            ("pc/console.config", "CONFIG_VT", "n"),
            ("pc/input.config", "CONFIG_INPUT_EVDEV", "y"),
            ("pc/sound.config", "CONFIG_SND_HDA_INTEL", "y"),
            ("pc/sound.config", "CONFIG_SND_DUMMY", "m"),
            ("pc/storage.config", "CONFIG_EROFS_FS", "y"),
            ("pc/storage.config", "CONFIG_EXFAT_FS", "y"),
            ("pc/storage.config", "CONFIG_USB_STORAGE", "y"),
            ("pc/network.config", "CONFIG_E1000E", "y"),
            ("qemu/host.config", "CONFIG_FW_CFG_SYSFS", "y"),
        ] {
            let rel = format!("board/{file}");
            assert!(fragments.contains(&rel), "{rel} is not in the defconfig");
            assert_eq!(
                fragment(&rel).get(option).map(String::as_str),
                Some(value),
                "{rel} {option}"
            );
        }
        // The kernel sets no mode of its own (the safety model, layer 8).
        let video = fragment("board/pc/video.config");
        for option in [
            "CONFIG_DRM_FBDEV_EMULATION",
            "CONFIG_FRAMEBUFFER_CONSOLE",
            "CONFIG_FB",
            "CONFIG_DRM_SIMPLEDRM",
            "CONFIG_SYSFB_SIMPLEFB",
            "CONFIG_DRM_PANIC",
            "CONFIG_DRM_EFIDRM",
            "CONFIG_DRM_VESADRM",
            "CONFIG_DRM_CLIENT_LOG",
        ] {
            assert_eq!(video[option], "n", "{option}");
        }
        assert_eq!(fragment("board/pc/console.config")["CONFIG_EXPERT"], "y");
        let check =
            std::fs::read_to_string(os_dir().join("board/pc/check-kernel-config.sh")).unwrap();
        for name in [
            "VT",
            "FB",
            "DRM_FBDEV_EMULATION",
            "FRAMEBUFFER_CONSOLE",
            "DRM_SIMPLEDRM",
            "SYSFB_SIMPLEFB",
            "DRM_PANIC",
            "DRM_EFIDRM",
            "DRM_VESADRM",
            "DRM_CLIENT_LOG",
        ] {
            let listed = check
                .split(|c: char| c.is_whitespace() || c == '"' || c == '=' || c == '\\')
                .any(|w| w == name);
            assert!(listed, "the check misses {name}");
        }
        let post_build = std::fs::read_to_string(os_dir().join("board/pc/post-build.sh")).unwrap();
        assert!(post_build.contains("check-kernel-config.sh"));
    }

    /// A folder that is removed when the test ends, also when it fails.
    struct TempDir(PathBuf);

    impl TempDir {
        fn create(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("xtask-{name}-{}", std::process::id()));
            if dir.exists() {
                std::fs::remove_dir_all(&dir).unwrap();
            }
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            // A folder left in the system temp folder does no harm.
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Sources the Mesa cache script with `/state` at a folder of a temp
    /// directory and the mount table `mounts` (no file for `None`), and
    /// returns the cache folder and the disable flag that it exports, and
    /// whether the cache folder exists after.
    fn mesa_cache(name: &str, mounts: Option<&str>, state_is_file: bool) -> (String, String, bool) {
        let script = os_dir().join("board/common/rootfs-overlay/usr/libexec/neptron/mesa-cache");
        let dir = TempDir::create(&format!("mesa-{name}"));
        let state = dir.0.join("state");
        if state_is_file {
            std::fs::write(&state, "not a folder").unwrap();
        } else {
            std::fs::create_dir_all(&state).unwrap();
        }
        let table = dir.0.join("mounts");
        if let Some(mounts) = mounts {
            let text = mounts.replace("STATE", state.to_str().unwrap());
            std::fs::write(&table, text).unwrap();
        }
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(r#". "$0"; echo "$MESA_SHADER_CACHE_DIR|$MESA_SHADER_CACHE_DISABLE|$MESA_SHADER_CACHE_MAX_SIZE""#)
            .arg(&script)
            .env("NEPTRON_STATE", &state)
            .env("NEPTRON_MOUNTS", &table)
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        assert!(out.stderr.is_empty(), "{out:?}");
        let made = state.join("cache/mesa").is_dir();
        let line = String::from_utf8(out.stdout).unwrap();
        let line = line.trim().replace(state.to_str().unwrap(), "STATE");
        let mut parts = line.split('|');
        let cache = parts.next().unwrap_or_default().to_owned();
        let disable = parts.next().unwrap_or_default().to_owned();
        assert_eq!(
            parts.next(),
            Some(if cache.is_empty() { "" } else { "64M" })
        );
        (cache, disable, made)
    }

    #[test]
    fn the_mesa_cache_is_on_the_state_partition_or_off() {
        let disk = "/dev/sda2 / erofs ro 0 0\n/dev/sda3 STATE ext4 rw,noatime 0 0\n";
        assert_eq!(
            mesa_cache("disk", Some(disk), false),
            ("STATE/cache/mesa".to_owned(), String::new(), true)
        );
        let read_only = "/dev/sda3 STATE ext4 ro,noatime 0 0\n";
        assert_eq!(
            mesa_cache("read-only", Some(read_only), false),
            (String::new(), "true".to_owned(), false),
            "a state partition mounted read-only gets no cache"
        );
        let remounted =
            "/dev/sda3 STATE ext4 rw 0 0\n/dev/sda3 STATE ext4 ro,errors=remount-ro 0 0\n";
        assert_eq!(
            mesa_cache("remounted", Some(remounted), false),
            (String::new(), "true".to_owned(), false),
            "the last line of the folder is the mount in use"
        );
        let ram = "/dev/sda2 / erofs ro 0 0\nstate STATE tmpfs rw 0 0\n";
        assert_eq!(
            mesa_cache("ram", Some(ram), false),
            (String::new(), "true".to_owned(), false),
            "a RAM folder gets no cache"
        );
        assert_eq!(
            mesa_cache("none", None, false),
            (String::new(), "true".to_owned(), false),
            "no mount table"
        );
        assert_eq!(
            mesa_cache("blocked", Some(disk), true),
            (String::new(), "true".to_owned(), false),
            "a folder that cannot be made turns the cache off and stops nothing"
        );
        let launch = |rel: &str| std::fs::read_to_string(os_dir().join(rel)).unwrap();
        for rel in [
            "board/common/rootfs-overlay/usr/libexec/neptron/launch",
            "board/pc/dev-overlay/usr/libexec/neptron/launch",
        ] {
            assert!(
                launch(rel).contains(". /usr/libexec/neptron/mesa-cache\n"),
                "{rel} does not source the Mesa cache script"
            );
        }
    }

    #[test]
    fn the_dev_variant_has_the_realtek_ethernet_firmware_and_no_other() {
        let dev = fragment("configs/neptron_pc_x86_64_dev.fragment");
        assert_eq!(dev["BR2_PACKAGE_LINUX_FIRMWARE"], "y");
        let firmware: Vec<_> = dev
            .iter()
            .filter(|(k, v)| k.starts_with("BR2_PACKAGE_LINUX_FIRMWARE_") && *v == "y")
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(firmware, ["BR2_PACKAGE_LINUX_FIRMWARE_RTL_8169"]);
        // r8169 asks for its firmware when the port opens, after the root
        // filesystem mounts, so the file in /lib/firmware is enough.
        assert_eq!(fragment("board/pc/network.config")["CONFIG_R8169"], "y");
        let release = defconfig(Arch::X86_64);
        assert!(
            !release
                .keys()
                .any(|k| k.starts_with("BR2_PACKAGE_LINUX_FIRMWARE")),
            "the release image starts no network and needs no firmware"
        );
    }

    #[test]
    fn the_pc_image_boots_with_grub_on_bios_and_uefi_and_has_no_login() {
        let x = defconfig(Arch::X86_64);
        for option in [
            "BR2_TARGET_GRUB2_I386_PC",
            "BR2_TARGET_GRUB2_X86_64_EFI",
            "BR2_TARGET_ROOTFS_EROFS",
            "BR2_PACKAGE_MESA3D_GALLIUM_DRIVER_IRIS",
            "BR2_PACKAGE_UTIL_LINUX_BINARIES",
            "BR2_PACKAGE_EXFATPROGS",
            "BR2_PACKAGE_NEPTRON",
            "BR2_PACKAGE_NEPTRON_LAUNCH",
            "BR2_PACKAGE_NEPTRONCTL",
        ] {
            assert_eq!(x.get(option).map(String::as_str), Some("y"), "{option}");
        }
        assert_eq!(x["BR2_TARGET_GENERIC_GETTY"], "n");
        assert_eq!(x["BR2_TARGET_GENERIC_REMOUNT_ROOTFS_RW"], "n");
        for absent in [
            "BR2_PACKAGE_DROPBEAR",
            "BR2_PACKAGE_NEPTRON_CONTROL",
            "BR2_SYSTEM_DHCP",
        ] {
            assert!(
                !x.contains_key(absent),
                "{absent} is in the release defconfig"
            );
        }
        let inittab =
            std::fs::read_to_string(os_dir().join("board/pc/rootfs-overlay/etc/inittab")).unwrap();
        let entries: Vec<&str> = inittab.lines().filter(|l| !l.starts_with('#')).collect();
        assert!(
            !entries
                .iter()
                .any(|l| l.contains("getty") || l.contains("/bin/sh")),
            "{inittab}"
        );
        assert!(
            inittab.contains("::respawn:/usr/libexec/neptron/launch"),
            "{inittab}"
        );
        let grub = std::fs::read_to_string(os_dir().join("board/pc/grub.cfg")).unwrap();
        assert!(grub.contains("set timeout=0"), "{grub}");
        assert!(grub.contains("root=PARTUUID=@SIGNATURE@-02"), "{grub}");
        let genimage = std::fs::read_to_string(os_dir().join("board/pc/genimage.cfg")).unwrap();
        assert!(
            genimage.contains("disk-signature = @SIGNATURE@"),
            "{genimage}"
        );
        assert!(
            !genimage.contains("partition share {"),
            "the first boot makes the share partition"
        );
        let post_image = std::fs::read_to_string(os_dir().join("board/pc/post-image.sh")).unwrap();
        for signature in ["4e505452", "4e505444", "4e50544e", "4e505445"] {
            assert!(
                post_image.contains(&format!("signature={signature}")),
                "{signature}"
            );
        }
    }

    /// Runs check-kernel-config.sh on a fake output: `version` in the
    /// Buildroot config (none for no line), and a `.config` for each folder
    /// of `kernels` (version, text; no text for no file).
    fn check_kernel_config(
        name: &str,
        version: Option<&str>,
        kernels: &[(&str, Option<&str>)],
    ) -> std::process::Output {
        let script = os_dir().join("board/pc/check-kernel-config.sh");
        let dir = std::env::temp_dir().join(format!("xtask-kconfig-{name}-{}", std::process::id()));
        let build = dir.join("build");
        std::fs::create_dir_all(&build).unwrap();
        let br2 = dir.join("br2.config");
        let line = version.map_or(String::new(), |v| {
            format!("BR2_LINUX_KERNEL_CUSTOM_VERSION_VALUE=\"{v}\"\n")
        });
        std::fs::write(&br2, format!("BR2_x86_64=y\n{line}")).unwrap();
        for (kernel, text) in kernels {
            let folder = build.join(format!("linux-{kernel}"));
            std::fs::create_dir_all(&folder).unwrap();
            if let Some(text) = text {
                std::fs::write(folder.join(".config"), text).unwrap();
            }
        }
        let out = std::process::Command::new("sh")
            .arg(&script)
            .arg(&br2)
            .arg(&build)
            .output()
            .unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        out
    }

    #[test]
    fn the_kernel_check_reads_the_configured_kernel_and_every_mode_option() {
        let off = "# CONFIG_VT is not set\n# CONFIG_FB is not set\nCONFIG_DRM_I915=y\n";
        let ok = check_kernel_config("off", Some("6.18.63"), &[("6.18.63", Some(off))]);
        assert!(ok.status.success(), "{ok:?}");
        let missing = check_kernel_config("missing", Some("6.18.63"), &[("6.18.63", None)]);
        assert!(!missing.status.success());
        assert!(String::from_utf8_lossy(&missing.stderr).contains("is missing"));
        let no_version = check_kernel_config("noversion", None, &[("6.18.63", Some(off))]);
        assert!(!no_version.status.success());
        assert!(String::from_utf8_lossy(&no_version.stderr).contains("no kernel version"));
        let module = check_kernel_config(
            "module",
            Some("6.18.63"),
            &[("6.18.63", Some("CONFIG_DRM_SIMPLEDRM=m\n"))],
        );
        assert!(!module.status.success());
        assert!(String::from_utf8_lossy(&module.stderr).contains("CONFIG_DRM_SIMPLEDRM is on"));
        let builtin = check_kernel_config(
            "builtin",
            Some("6.18.63"),
            &[("6.18.63", Some("CONFIG_VT=y\nCONFIG_DRM_PANIC=y\n"))],
        );
        let text = String::from_utf8_lossy(&builtin.stderr).into_owned();
        assert!(!builtin.status.success());
        assert!(
            text.contains("CONFIG_VT is on") && text.contains("CONFIG_DRM_PANIC is on"),
            "{text}"
        );
        // An older kernel folder stays in a reused output: a clean old
        // configuration must not hide an option of the configured kernel.
        let stale = check_kernel_config(
            "stale",
            Some("6.18.63"),
            &[("6.18.62", Some(off)), ("6.18.63", Some("CONFIG_FB=y\n"))],
        );
        assert!(!stale.status.success());
        assert!(String::from_utf8_lossy(&stale.stderr).contains("CONFIG_FB is on"));
        let only_old = check_kernel_config("onlyold", Some("6.18.63"), &[("6.18.62", Some(off))]);
        assert!(
            !only_old.status.success(),
            "the configured kernel has no configuration"
        );
    }

    /// The core packages, their make prefix, and the library that
    /// `data/cores` names.
    const CORE_PACKAGES: [(&str, &str, &str); 3] = [
        ("libretro-fceumm", "LIBRETRO_FCEUMM", "fceumm"),
        ("libretro-snes9x", "LIBRETRO_SNES9X", "snes9x"),
        (
            "libretro-genesis-plus-gx",
            "LIBRETRO_GENESIS_PLUS_GX",
            "genesis_plus_gx",
        ),
    ];

    #[test]
    fn both_targets_build_the_three_cores() {
        for arch in [Arch::Aarch64, Arch::X86_64] {
            let options = defconfig(arch);
            for (_, prefix, _) in CORE_PACKAGES {
                let name = format!("BR2_PACKAGE_{prefix}");
                assert_eq!(options.get(&name).map(String::as_str), Some("y"), "{name}");
            }
        }
        let menu = std::fs::read_to_string(os_dir().join("Config.in")).unwrap();
        for (package, _, _) in CORE_PACKAGES {
            assert!(
                menu.contains(&format!("package/{package}/Config.in")),
                "{package}"
            );
        }
    }

    #[test]
    fn each_core_is_pinned_hashed_and_installed_under_its_library_name() {
        for (package, prefix, core) in CORE_PACKAGES {
            let dir = os_dir().join("package").join(package);
            let mk = std::fs::read_to_string(dir.join(format!("{package}.mk"))).unwrap();
            let version = mk
                .lines()
                .find_map(|l| l.strip_prefix(&format!("{prefix}_VERSION = ")))
                .unwrap();
            assert!(
                version.len() == 40 && version.bytes().all(|b| b.is_ascii_hexdigit()),
                "{package}: {version} is not a commit"
            );
            let hashes = std::fs::read_to_string(dir.join(format!("{package}.hash"))).unwrap();
            let tarball = format!("{package}-{version}.tar.gz");
            let pinned = hashes.lines().any(|l| {
                let f: Vec<&str> = l.split_whitespace().collect();
                f.len() == 3 && f[0] == "sha256" && f[1].len() == 64 && f[2] == tarball
            });
            assert!(pinned, "{package}: no sha256 for {tarball}");
            let files = mk
                .lines()
                .find_map(|l| l.strip_prefix(&format!("{prefix}_LICENSE_FILES = ")))
                .unwrap();
            for file in files.split_whitespace() {
                assert!(
                    hashes.lines().any(|l| l.ends_with(&format!("  {file}"))),
                    "{package}: no hash for the licence file {file}"
                );
            }
            let library = format!("{core}_libretro.so");
            assert!(
                mk.contains(&format!("$(TARGET_DIR)/usr/lib/libretro/{library}")),
                "{package}"
            );
            let data = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../data/cores")
                .join(format!("{core}.toml"));
            let data = std::fs::read_to_string(data).unwrap();
            assert!(data.contains(&format!("library = \"{library}\"")), "{core}");
        }
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
