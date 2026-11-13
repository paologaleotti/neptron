//! Tests that pin the Raspberry Pi image: the defconfig and its development
//! fragment, the kernel pin and its fragments, the checks of the kernel,
//! `config.txt` and `cmdline.txt`, the SD card layout, and the licence
//! notes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use super::Board;
use super::defconfig::parse;

fn os_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../os")
}

fn read(rel: &str) -> String {
    let path = os_dir().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn defconfig() -> BTreeMap<String, String> {
    parse(&read(&format!("configs/{}", Board::RpiArm64.defconfig())))
}

fn dev_fragment() -> BTreeMap<String, String> {
    parse(&read(&format!(
        "configs/{}",
        Board::RpiArm64.dev_fragment().unwrap()
    )))
}

/// The pinned commit of the Raspberry Pi fork, from the tarball location.
fn kernel_commit() -> String {
    let location = &defconfig()["BR2_LINUX_KERNEL_CUSTOM_TARBALL_LOCATION"];
    let commit = location
        .strip_prefix("$(call github,raspberrypi,linux,")
        .and_then(|rest| rest.split_once(')'))
        .map(|(commit, _)| commit.to_owned())
        .unwrap_or_else(|| panic!("{location} is not a commit of raspberrypi/linux"));
    assert_eq!(
        location,
        &format!("$(call github,raspberrypi,linux,{commit})/linux-{commit}.tar.gz")
    );
    commit
}

/// A folder that is removed when the test ends, also when it fails.
struct TempDir(PathBuf);

impl TempDir {
    fn create(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("xtask-rpi-{name}-{}", std::process::id()));
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

fn run_sh(script: &Path, args: &[&Path]) -> Output {
    Command::new("sh").arg(script).args(args).output().unwrap()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn the_pi_image_makes_the_os_decisions_of_the_pc_image() {
    let pi = defconfig();
    let pc = parse(&read(&format!("configs/{}", Board::Pc.defconfig())));
    for name in [
        "BR2_TOOLCHAIN_BUILDROOT_GLIBC",
        "BR2_TOOLCHAIN_BUILDROOT_CXX",
        "BR2_KERNEL_HEADERS_AS_KERNEL",
        "BR2_PACKAGE_HOST_LINUX_HEADERS_CUSTOM_6_18",
        "BR2_INIT_BUSYBOX",
        "BR2_ROOTFS_DEVICE_CREATION_DYNAMIC_MDEV",
        "BR2_DOWNLOAD_FORCE_CHECK_HASHES",
        "BR2_TARGET_GENERIC_GETTY",
        "BR2_TARGET_GENERIC_REMOUNT_ROOTFS_RW",
        "BR2_TARGET_ROOTFS_EROFS",
        "BR2_ROOTFS_USERS_TABLES",
        "BR2_ROOTFS_POST_FAKEROOT_SCRIPT",
        "BR2_PACKAGE_BUSYBOX_CONFIG_FRAGMENT_FILES",
        "BR2_PACKAGE_E2FSPROGS",
        "BR2_PACKAGE_EXFATPROGS",
        "BR2_PACKAGE_UTIL_LINUX_BINARIES",
        "BR2_PACKAGE_UTIL_LINUX_PARTX",
        "BR2_PACKAGE_SDL3_KMSDRM",
        "BR2_PACKAGE_SDL3_OPENGLES",
        "BR2_PACKAGE_MESA3D_OPENGL_EGL",
        "BR2_PACKAGE_MESA3D_OPENGL_ES",
        "BR2_PACKAGE_ALSA_UTILS_AMIXER",
        "BR2_PACKAGE_LIBRETRO_FCEUMM",
        "BR2_PACKAGE_LIBRETRO_SNES9X",
        "BR2_PACKAGE_LIBRETRO_GENESIS_PLUS_GX",
        "BR2_PACKAGE_NEPTRON",
        "BR2_PACKAGE_NEPTRON_LAUNCH",
        "BR2_PACKAGE_NEPTRONCTL",
        "BR2_PACKAGE_KSMBD_TOOLS",
        "BR2_PACKAGE_MDNSD",
    ] {
        assert!(pi.contains_key(name), "{name} missing in the Pi defconfig");
        assert_eq!(pi.get(name), pc.get(name), "{name} differs from the PC");
    }
    assert_eq!(pi["BR2_aarch64"], "y");
    assert_eq!(pi["BR2_cortex_a53"], "y");
    // The common post-build step (the network share files) runs first.
    assert!(
        pi["BR2_ROOTFS_POST_BUILD_SCRIPT"].starts_with(
            "$(BR2_EXTERNAL_NEPTRON_PATH)/board/common/post-build.sh \
             $(BR2_EXTERNAL_NEPTRON_PATH)/board/rpi/post-build.sh"
        ),
        "{}",
        pi["BR2_ROOTFS_POST_BUILD_SCRIPT"]
    );
    assert_eq!(
        pi["BR2_ROOTFS_POST_IMAGE_SCRIPT"],
        "$(BR2_EXTERNAL_NEPTRON_PATH)/board/rpi/post-image.sh"
    );
    // The release image: no network login, no control commands, and no
    // test tool of the QEMU display.
    for absent in [
        "BR2_PACKAGE_DROPBEAR",
        "BR2_PACKAGE_NEPTRON_CONTROL",
        "BR2_PACKAGE_NEPTRON_VKMS_PROBE",
        "BR2_PACKAGE_NEPTRON_CAPTURE",
        "BR2_SYSTEM_DHCP",
        "BR2_PACKAGE_LINUX_FIRMWARE",
    ] {
        assert!(!pi.contains_key(absent), "{absent} is in the Pi defconfig");
    }
}

#[test]
fn mesa_gives_gles_on_vc4_without_llvm() {
    let pi = defconfig();
    assert_eq!(pi["BR2_PACKAGE_MESA3D_GALLIUM_DRIVER_VC4"], "y");
    assert_eq!(pi["BR2_PACKAGE_MESA3D_GALLIUM_DRIVER_SOFTPIPE"], "y");
    for absent in [
        "BR2_PACKAGE_MESA3D_LLVM",
        "BR2_PACKAGE_MESA3D_GALLIUM_DRIVER_LLVMPIPE",
        "BR2_PACKAGE_MESA3D_GALLIUM_DRIVER_IRIS",
    ] {
        assert!(!pi.contains_key(absent), "{absent}");
    }
}

#[test]
fn the_kernel_is_the_pi_fork_at_a_pinned_commit_with_its_hash() {
    let pi = defconfig();
    assert_eq!(pi["BR2_LINUX_KERNEL_CUSTOM_TARBALL"], "y");
    assert_eq!(pi["BR2_LINUX_KERNEL_DEFCONFIG"], "bcm2711");
    assert!(!pi.contains_key("BR2_LINUX_KERNEL_CUSTOM_VERSION_VALUE"));
    let commit = kernel_commit();
    assert!(
        commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()),
        "{commit}"
    );
    let tarball = format!("linux-{commit}.tar.gz");
    for pkg in ["linux", "linux-headers"] {
        let hashes = read(&format!("board/rpi/patches/{pkg}/{pkg}.hash"));
        assert!(hashes.contains("rpi-6.18.y"), "{pkg}: the branch is named");
        assert!(hashes.contains(&commit), "{pkg}: the commit is named");
        let pinned = hashes.lines().any(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            f.len() == 3 && f[0] == "sha256" && f[1].len() == 64 && f[2] == tarball
        });
        assert!(pinned, "{pkg}: no sha256 for {tarball}");
        for licence in [
            "COPYING",
            "LICENSES/preferred/GPL-2.0",
            "LICENSES/exceptions/Linux-syscall-note",
        ] {
            assert!(
                hashes.lines().any(|l| l.ends_with(&format!("  {licence}"))),
                "{pkg}: {licence}"
            );
        }
    }
    let notes = read("board/rpi/LICENCES.txt");
    assert!(notes.contains(&commit), "LICENCES.txt names the kernel");
}

#[test]
fn the_device_trees_and_overlays_come_from_the_kernel_not_the_firmware() {
    let pi = defconfig();
    assert_eq!(pi["BR2_LINUX_KERNEL_DTS_SUPPORT"], "y");
    assert_eq!(pi["BR2_LINUX_KERNEL_DTB_OVERLAY_SUPPORT"], "y");
    assert_eq!(
        pi["BR2_LINUX_KERNEL_INTREE_DTS_NAME"],
        "broadcom/bcm2710-rpi-3-b broadcom/bcm2710-rpi-3-b-plus broadcom/bcm2710-rpi-zero-2-w"
    );
    assert_eq!(
        pi["BR2_LINUX_KERNEL_INTREE_DTSO_NAMES"],
        "overlays/vc4-kms-v3d overlays/disable-bt"
    );
    assert_eq!(pi["BR2_PACKAGE_RPI_FIRMWARE"], "y");
    assert_eq!(pi["BR2_PACKAGE_RPI_FIRMWARE_BOOTCODE_BIN"], "y");
    assert_eq!(pi["BR2_PACKAGE_RPI_FIRMWARE_VARIANT_PI"], "y");
    assert_eq!(pi["BR2_PACKAGE_RPI_FIRMWARE_INSTALL_DTB_OVERLAYS"], "n");
    assert_eq!(pi["BR2_PACKAGE_RPI_FIRMWARE_CMDLINE_FILE"], "");
    assert!(!pi.contains_key("BR2_PACKAGE_RPI_FIRMWARE_CONFIG_FILE"));
    let post_image = read("board/rpi/post-image.sh");
    for name in pi["BR2_LINUX_KERNEL_INTREE_DTS_NAME"].split_whitespace() {
        let dtb = name.rsplit('/').next().unwrap();
        assert!(post_image.contains(dtb), "post-image.sh misses {dtb}");
    }
    for overlay in ["vc4-kms-v3d.dtbo", "disable-bt.dtbo"] {
        assert!(
            post_image.contains(&format!("\"$part/overlays/{overlay}\"")),
            "post-image.sh misses {overlay}"
        );
    }
}

#[test]
fn each_kernel_fragment_holds_its_concern() {
    let pi = defconfig();
    let fragments = &pi["BR2_LINUX_KERNEL_CONFIG_FRAGMENT_FILES"];
    for (file, option, value) in [
        ("system.config", "CONFIG_ARM64_4K_PAGES", "y"),
        ("system.config", "CONFIG_BCM2835_WDT", "y"),
        ("video.config", "CONFIG_DRM_VC4", "y"),
        ("video.config", "CONFIG_DRM_V3D", "y"),
        ("video.config", "CONFIG_MEDIA_SUPPORT", "n"),
        ("console.config", "CONFIG_EXPERT", "y"),
        ("console.config", "CONFIG_VT", "n"),
        ("input.config", "CONFIG_INPUT_EVDEV", "y"),
        ("input.config", "CONFIG_USB_HID", "y"),
        ("input.config", "CONFIG_INPUT_UINPUT", "y"),
        ("sound.config", "CONFIG_SND", "y"),
        ("sound.config", "CONFIG_SND_SOC", "y"),
        ("sound.config", "CONFIG_SND_BCM2835", "y"),
        ("sound.config", "CONFIG_SND_USB_AUDIO", "y"),
        ("storage.config", "CONFIG_MMC_BCM2835", "y"),
        ("storage.config", "CONFIG_EROFS_FS", "y"),
        ("storage.config", "CONFIG_EROFS_FS_ZIP", "y"),
        ("storage.config", "CONFIG_EXT4_FS", "y"),
        ("storage.config", "CONFIG_EXFAT_FS", "y"),
        ("storage.config", "CONFIG_VFAT_FS", "y"),
        ("network.config", "CONFIG_USB_NET_SMSC95XX", "y"),
        ("network.config", "CONFIG_USB_LAN78XX", "y"),
        ("network.config", "CONFIG_SMB_SERVER", "y"),
        ("network.config", "CONFIG_SMB_SERVER_KERBEROS5", "n"),
        ("network.config", "CONFIG_WLAN", "n"),
        ("network.config", "CONFIG_BT", "n"),
    ] {
        let rel = format!("board/rpi/{file}");
        assert!(
            fragments.contains(&format!("$(BR2_EXTERNAL_NEPTRON_PATH)/{rel}")),
            "{rel} is not in the defconfig"
        );
        assert_eq!(
            parse(&read(&rel)).get(option).map(String::as_str),
            Some(value),
            "{rel} {option}"
        );
    }
    // The kernel sets no mode of its own (the safety model, layer 8).
    let video = parse(&read("board/rpi/video.config"));
    for option in [
        "CONFIG_FB",
        "CONFIG_DRM_FBDEV_EMULATION",
        "CONFIG_FRAMEBUFFER_CONSOLE",
        "CONFIG_DRM_SIMPLEDRM",
        "CONFIG_SYSFB_SIMPLEFB",
        "CONFIG_DRM_PANIC",
        "CONFIG_DRM_CLIENT_LOG",
        "CONFIG_LOGO",
    ] {
        assert_eq!(video[option], "n", "{option}");
    }
    let pc_check = read("board/pc/check-kernel-config.sh");
    let pi_check = read("board/rpi/check-kernel-config.sh");
    let options = |text: &str| {
        text.lines()
            .skip_while(|l| !l.starts_with("OPTIONS="))
            .take_while(|l| !l.starts_with("SHARE_OFF="))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(
        options(&pi_check),
        options(&pc_check),
        "both kernels refuse the same mode options"
    );
    assert!(pi_check.contains("POWER_ON=\"BCM2835_WDT\""));
}

/// Runs the kernel check of the Pi on a fake output: a Buildroot config
/// (`custom` gives the custom tarball line) and the `.config` of
/// `linux-custom` (no text for no file).
fn check_kernel_config(name: &str, custom: bool, config: Option<&str>) -> Output {
    let dir = TempDir::create(&format!("kconfig-{name}"));
    let br2 = dir.0.join("br2.config");
    let line = if custom {
        "BR2_LINUX_KERNEL_CUSTOM_TARBALL=y\n"
    } else {
        "BR2_LINUX_KERNEL_CUSTOM_VERSION_VALUE=\"6.18.63\"\n"
    };
    std::fs::write(&br2, format!("BR2_aarch64=y\n{line}")).unwrap();
    let build = dir.0.join("build");
    std::fs::create_dir_all(build.join("linux-custom")).unwrap();
    if let Some(text) = config {
        std::fs::write(build.join("linux-custom/.config"), text).unwrap();
    }
    run_sh(
        &os_dir().join("board/rpi/check-kernel-config.sh"),
        &[&br2, &build],
    )
}

#[test]
fn the_kernel_check_refuses_a_mode_option_and_needs_the_watchdog() {
    let good = "# CONFIG_VT is not set\n# CONFIG_FB is not set\nCONFIG_DRM_VC4=y\n\
                CONFIG_SMB_SERVER=y\nCONFIG_BCM2835_WDT=y\n";
    let ok = check_kernel_config("ok", true, Some(good));
    assert!(ok.status.success(), "{ok:?}");
    let missing = check_kernel_config("missing", true, None);
    assert!(!missing.status.success());
    assert!(stderr(&missing).contains("is missing"));
    let other = check_kernel_config("other", false, Some(good));
    assert!(!other.status.success());
    assert!(stderr(&other).contains("custom tarball"), "{other:?}");
    for (name, text, fault) in [
        ("fb", "CONFIG_FB=y\n", "CONFIG_FB is on"),
        (
            "fbdev",
            "CONFIG_DRM_FBDEV_EMULATION=y\n",
            "CONFIG_DRM_FBDEV_EMULATION is on",
        ),
        ("vt", "CONFIG_VT=y\n", "CONFIG_VT is on"),
        ("panic", "CONFIG_DRM_PANIC=y\n", "CONFIG_DRM_PANIC is on"),
        (
            "simpledrm",
            "CONFIG_DRM_SIMPLEDRM=m\n",
            "CONFIG_DRM_SIMPLEDRM is on",
        ),
        (
            "nowdt",
            "# CONFIG_BCM2835_WDT is not set\n",
            "CONFIG_BCM2835_WDT is not built in",
        ),
        (
            "wdtmodule",
            "CONFIG_BCM2835_WDT=m\n",
            "CONFIG_BCM2835_WDT is not built in",
        ),
        (
            "vc4module",
            "CONFIG_DRM_VC4=m\n",
            "CONFIG_DRM_VC4 is not built in",
        ),
        (
            "smbmodule",
            "CONFIG_SMB_SERVER=m\n",
            "CONFIG_SMB_SERVER is not built in",
        ),
        (
            "kerberos",
            "CONFIG_SMB_SERVER_KERBEROS5=y\n",
            "CONFIG_SMB_SERVER_KERBEROS5 is on",
        ),
    ] {
        let text = good
            .lines()
            .filter(|l| {
                let option = text.split(['=', ' ']).find(|w| w.starts_with("CONFIG_"));
                !option.is_some_and(|o| l.contains(o))
            })
            .chain(text.lines())
            .collect::<Vec<_>>()
            .join("\n");
        let out = check_kernel_config(name, true, Some(&text));
        assert!(!out.status.success(), "{name}");
        assert!(stderr(&out).contains(fault), "{name}: {}", stderr(&out));
    }
    let post_build = read("board/rpi/post-build.sh");
    for check in ["check-kernel-config.sh", "check-kernel-patches.sh"] {
        assert!(
            post_build.contains(&format!("\"$board/{check}\"")),
            "{check}"
        );
    }
}

/// Runs the kernel patch check on `br2` (text of a Buildroot config) with
/// a copy of the Pi patch directory, plus the files of `extra`.
fn check_kernel_patches(name: &str, br2: &str, extra: &[&str]) -> Output {
    let dir = TempDir::create(&format!("patches-{name}"));
    let config = dir.0.join("br2.config");
    std::fs::write(&config, br2).unwrap();
    let external = dir.0.join("os");
    for pkg in ["linux", "linux-headers"] {
        let rel = format!("board/rpi/patches/{pkg}/{pkg}.hash");
        let path = external.join(&rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::copy(os_dir().join(&rel), path).unwrap();
    }
    for rel in extra {
        let path = external.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }
    run_sh(
        &os_dir().join("board/rpi/check-kernel-patches.sh"),
        &[&config, &external],
    )
}

/// The Pi kernel takes no patch, so the 15 kHz patch set of the PC (open
/// licence question, OP-070) can never reach it.
#[test]
fn no_kernel_patch_reaches_the_pi_kernel() {
    let pi = defconfig();
    assert!(!pi.contains_key("BR2_LINUX_KERNEL_PATCH"));
    assert!(!dev_fragment().contains_key("BR2_LINUX_KERNEL_PATCH"));
    assert_eq!(
        pi["BR2_GLOBAL_PATCH_DIR"],
        "$(BR2_EXTERNAL_NEPTRON_PATH)/board/rpi/patches"
    );
    let patches = os_dir().join("board/rpi/patches");
    let mut files: Vec<String> = Vec::new();
    for pkg in std::fs::read_dir(&patches).unwrap() {
        let pkg = pkg.unwrap().path();
        for file in std::fs::read_dir(&pkg).unwrap() {
            let file = file.unwrap().path();
            files.push(file.strip_prefix(&patches).unwrap().display().to_string());
        }
    }
    files.sort();
    assert_eq!(
        files,
        ["linux-headers/linux-headers.hash", "linux/linux.hash"]
    );

    // The check on the real defconfig and patch directory.
    let real = run_sh(
        &os_dir().join("board/rpi/check-kernel-patches.sh"),
        &[
            &os_dir().join("configs/neptron_rpi_arm64_defconfig"),
            &os_dir(),
        ],
    );
    assert!(real.status.success(), "{real:?}");

    let good = "BR2_GLOBAL_PATCH_DIR=\"$(BR2_EXTERNAL_NEPTRON_PATH)/board/rpi/patches\"\n\
                BR2_LINUX_KERNEL_PATCH=\"\"\n";
    let ok = check_kernel_patches("ok", good, &[]);
    assert!(ok.status.success(), "{ok:?}");
    let pc = read(&format!("configs/{}", Board::Pc.defconfig()));
    let pc_patch = pc
        .lines()
        .find(|l| l.starts_with("BR2_LINUX_KERNEL_PATCH="))
        .unwrap();
    for (name, br2, extra, fault) in [
        (
            "pcpatch",
            format!("{good}{pc_patch}\n"),
            &[][..],
            "BR2_LINUX_KERNEL_PATCH is",
        ),
        (
            "pcdir",
            "BR2_GLOBAL_PATCH_DIR=\"$(BR2_EXTERNAL_NEPTRON_PATH)/patches\"\n".to_owned(),
            &[],
            "is not board/rpi/patches",
        ),
        (
            "both",
            "BR2_GLOBAL_PATCH_DIR=\"$(BR2_EXTERNAL_NEPTRON_PATH)/board/rpi/patches \
             $(BR2_EXTERNAL_NEPTRON_PATH)/patches\"\n"
                .to_owned(),
            &[],
            "'$(BR2_EXTERNAL_NEPTRON_PATH)/patches' is not board/rpi/patches",
        ),
        (
            "nodir",
            "BR2_LINUX_KERNEL_PATCH=\"\"\n".to_owned(),
            &[],
            "BR2_GLOBAL_PATCH_DIR is not set",
        ),
        (
            "file",
            good.to_owned(),
            &["board/rpi/patches/linux/01_linux_15khz.patch"][..],
            "01_linux_15khz.patch is in the patch directory of linux",
        ),
        (
            "versiondir",
            good.to_owned(),
            &["board/rpi/patches/linux/6.18.62/0001.patch"][..],
            "6.18.62 is in the patch directory of linux",
        ),
        (
            "hidden",
            good.to_owned(),
            &["board/rpi/patches/linux-headers/.0001.patch"][..],
            ".0001.patch is in the patch directory of linux-headers",
        ),
    ] {
        let out = check_kernel_patches(name, &br2, extra);
        assert!(!out.status.success(), "{name}");
        assert!(stderr(&out).contains(fault), "{name}: {}", stderr(&out));
    }
}

/// The lines of a file that are not comments or blank.
fn entries(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

#[test]
fn config_txt_holds_the_keys_of_the_spec_each_with_its_reason() {
    let text = read("board/rpi/config.txt");
    assert_eq!(
        entries(&text),
        [
            "arm_64bit=1",
            "kernel=Image",
            "disable_splash=1",
            "disable_fw_kms_setup=1",
            "dtoverlay=vc4-kms-v3d,composite",
            "dtparam=audio=on",
            "core_freq_fixed=1",
            "[pi02]",
            "enable_tvout=1",
            "[all]",
        ]
    );
    assert!(text.starts_with("# Written by the NEPTRON image."));
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.contains('=') && !line.starts_with('#') {
            // A model filter such as [pi02] may sit between the reason
            // and its key.
            let above = lines[..i].iter().rev().find(|l| !l.starts_with('['));
            assert!(
                above.is_some_and(|l| l.starts_with('#')),
                "{line} has no reason above it"
            );
        }
    }
    let dev = read("board/rpi/config-dev.txt");
    assert_eq!(entries(&dev), ["enable_uart=1", "dtoverlay=disable-bt"]);
    for line in entries(&text).into_iter().chain(entries(&dev)) {
        assert!(!line.contains("dpi"), "no GPIO DAC overlay: {line}");
        assert!(
            !line.contains("video="),
            "no mode from the firmware: {line}"
        );
    }
    let cmdline = read("board/rpi/cmdline.txt");
    assert_eq!(
        cmdline,
        "root=PARTUUID=@SIGNATURE@-02 rootfstype=erofs ro rootwait quiet \
         console=serial0,115200 panic=10\n"
    );
    assert!(!cmdline.contains("video="), "the kernel sets no mode");
    let post_image = read("board/rpi/post-image.sh");
    assert!(post_image.contains("cat \"$board/config-dev.txt\" >> \"$part/config.txt\""));
    assert!(post_image.contains("signature=4e505052"), "release");
    assert!(post_image.contains("signature=4e505044"), "development");
}

/// Runs `cmdline.sh` on the real template.
fn cmdline(signature: &str, variant: &str) -> Output {
    Command::new("sh")
        .arg(os_dir().join("board/rpi/cmdline.sh"))
        .arg(os_dir().join("board/rpi/cmdline.txt"))
        .arg(signature)
        .arg(variant)
        .output()
        .unwrap()
}

/// The release kernel is quiet on its console. The development kernel
/// prints its whole log on the serial port from the start, so a first boot
/// that stops early can be read there.
#[test]
fn only_the_release_kernel_command_line_is_quiet() {
    let release = cmdline("4e505052", "release");
    assert!(release.status.success(), "{release:?}");
    assert_eq!(
        String::from_utf8(release.stdout).unwrap(),
        "root=PARTUUID=4e505052-02 rootfstype=erofs ro rootwait quiet \
         console=serial0,115200 panic=10\n"
    );
    let dev = cmdline("4e505044", "dev");
    assert!(dev.status.success(), "{dev:?}");
    assert_eq!(
        String::from_utf8(dev.stdout).unwrap(),
        "root=PARTUUID=4e505044-02 rootfstype=erofs ro rootwait \
         console=serial0,115200 panic=10\n"
    );
    let bad = cmdline("4e505052", "debug");
    assert!(!bad.status.success());
    assert!(stderr(&bad).contains("not 'debug'"), "{bad:?}");
    let post_image = read("board/rpi/post-image.sh");
    assert!(
        post_image.contains(
            "\"$board/cmdline.sh\" \"$board/cmdline.txt\" \"$signature\" \"$variant\" \
             > \"$part/cmdline.txt\""
        ),
        "post-image.sh writes cmdline.txt through cmdline.sh"
    );
    assert!(post_image.contains("[ \"$dev\" = yes ] && variant=dev"));
}

/// The partitions of the SD card image, as (name, type, image, size).
fn genimage_partitions(text: &str) -> Vec<(String, String, String, String)> {
    let mut out = Vec::new();
    let mut current: Option<(String, String, String, String)> = None;
    for line in text.lines().map(str::trim) {
        if let Some(name) = line
            .strip_prefix("partition ")
            .and_then(|r| r.strip_suffix(" {"))
        {
            current = Some((name.to_owned(), String::new(), String::new(), String::new()));
        } else if line == "}" {
            if let Some(p) = current.take() {
                out.push(p);
            }
        } else if let Some(p) = current.as_mut()
            && let Some((key, value)) = line.split_once(" = ")
        {
            let value = value.trim_matches('"').to_owned();
            match key {
                "partition-type" => p.1 = value,
                "image" => p.2 = value,
                "size" => p.3 = value,
                _ => {}
            }
        }
    }
    out
}

#[test]
fn the_sd_card_has_the_pc_layout_and_leaves_room_for_the_share() {
    let text = read("board/rpi/genimage.cfg");
    let parts = genimage_partitions(&text);
    let expected = [
        ("boot", "0xc", "boot.vfat", ""),
        ("root", "0x83", "rootfs.erofs", "384M"),
        ("state", "0x83", "state.ext4", ""),
    ];
    assert_eq!(parts.len(), expected.len(), "{parts:?}");
    for (part, (name, kind, image, size)) in parts.iter().zip(expected) {
        assert_eq!(
            (
                part.0.as_str(),
                part.1.as_str(),
                part.2.as_str(),
                part.3.as_str()
            ),
            (name, kind, image, size)
        );
    }
    assert!(text.contains("partition-table-type = \"mbr\""));
    assert!(text.contains("disk-signature = @SIGNATURE@"));
    assert!(text.contains("offset = 4M"));
    assert!(text.contains("label = \"NEPTRONBOOT\""));
    assert!(text.contains("extraargs = \"-F 32\""), "FAT32");
    assert!(text.contains("size = 128M"), "the boot partition");
    assert!(
        !text.contains("partition share"),
        "the first boot makes the share"
    );
    let post_image = read("board/rpi/post-image.sh");
    assert!(post_image.contains("truncate -s 256M \"$images/state.ext4\""));
    assert!(post_image.contains("mkfs.ext4\" -q -L neptron-state"));
    // 4 MiB offset, 128 MiB boot, 384 MiB root, 256 MiB state: the share
    // starts at 772 MiB, a 4 MiB boundary.
    let share_start_mib = 4 + 128 + 384 + 256;
    assert_eq!(share_start_mib % 4, 0);
    for file in [
        "bootcode.bin",
        "start.elf",
        "fixup.dat",
        "config.txt",
        "cmdline.txt",
        "Image",
        "bcm2710-rpi-3-b.dtb",
        "bcm2710-rpi-3-b-plus.dtb",
        "bcm2710-rpi-zero-2-w.dtb",
        "overlays",
        "LICENCES.txt",
        "legal",
    ] {
        assert!(
            text.contains(&format!(
                "file {file} {{\n\t\t\timage = \"boot-part/{file}\""
            )),
            "the boot partition misses {file}"
        );
    }
}

/// Runs one function of the storage script, defined in the script text,
/// with `body` after it.
fn storage_function(name: &str, body: &str) -> String {
    let script = read("board/common/rootfs-overlay/etc/init.d/S06neptron-storage");
    let start = script
        .find(&format!("\n{name}() {{\n"))
        .unwrap_or_else(|| panic!("no function {name}"));
    let end = script[start..].find("\n}\n").unwrap() + start + 3;
    let out = Command::new("sh")
        .arg("-c")
        .arg(format!("{}\n{body}", &script[start..end]))
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    String::from_utf8(out.stdout).unwrap()
}

/// The storage script finds the partitions of an SD card by their number in
/// sysfs and reads partition 4 of `sfdisk --dump` under the name
/// `mmcblk0p4`, so the share partition of a Pi needs no change in it.
#[test]
fn the_storage_script_finds_mmcblk0p3_and_reads_mmcblk0p4() {
    let dir = TempDir::create("mmc");
    let disk = dir.0.join("mmcblk0");
    for n in 1..=3 {
        let part = disk.join(format!("mmcblk0p{n}"));
        std::fs::create_dir_all(&part).unwrap();
        std::fs::write(part.join("partition"), format!("{n}\n")).unwrap();
    }
    std::fs::create_dir_all(disk.join("queue")).unwrap();
    let state = storage_function(
        "partition",
        &format!("DEV=/dev; partition '{}' 3", disk.display()),
    );
    assert_eq!(state, "/dev/mmcblk0p3\n");
    let table = "label: dos\n\
                 /dev/mmcblk0p1 : start=        8192, size=      262144, type=c, bootable\n\
                 /dev/mmcblk0p2 : start=      270336, size=      786432, type=83\n\
                 /dev/mmcblk0p3 : start=     1056768, size=      524288, type=83\n\
                 /dev/mmcblk0p4 : start=     1581056, size=    60000000, type=7\n";
    let entry = storage_function(
        "entry_of_table",
        &format!("table='{table}'; entry_of_table"),
    );
    assert_eq!(entry, "1581056 60000000 7\n");
    let none = storage_function(
        "entry_of_table",
        &format!(
            "table='{}'; entry_of_table",
            table.lines().take(4).collect::<Vec<_>>().join("\n")
        ),
    );
    assert_eq!(none, "", "mmcblk0p3 is not partition 4");
}

#[test]
fn the_dev_variant_adds_ssh_the_hot_folder_and_the_serial_console() {
    let dev = dev_fragment();
    assert_eq!(dev["BR2_PACKAGE_DROPBEAR"], "y");
    assert_eq!(dev["BR2_PACKAGE_NEPTRON_CONTROL"], "y");
    let release_overlay = &defconfig()["BR2_ROOTFS_OVERLAY"];
    assert_eq!(
        dev["BR2_ROOTFS_OVERLAY"],
        format!(
            "{release_overlay} $(BR2_EXTERNAL_NEPTRON_PATH)/board/pc/dev-overlay \
             $(BR2_EXTERNAL_NEPTRON_PATH)/board/rpi/dev-overlay"
        )
    );
    assert!(
        !dev.keys()
            .any(|k| k.starts_with("BR2_PACKAGE_LINUX_FIRMWARE")),
        "the Ethernet chips of the Pi 3 need no firmware"
    );
    let post_build = read("board/rpi/post-build.sh");
    for line in [
        "grep -qx 'BR2_PACKAGE_DROPBEAR=y' \"$BR2_CONFIG\"",
        "key=/work/dev-key/authorized_keys",
        "\"$ext/pc/dev-overlay\" \"$ext/rpi/dev-overlay\"",
        "build it in a clean output",
    ] {
        assert!(post_build.contains(line), "post-build.sh misses {line:?}");
    }
    assert!(read("board/rpi/dev-overlay/etc/neptron/development").contains("Raspberry Pi"));
}

/// The System panel turns a Pi off through the launcher (`--power`) and
/// BusyBox `poweroff`, as on the PC. A Pi has no ACPI: the watchdog driver
/// halts the firmware.
#[test]
fn the_power_off_of_a_pi_goes_through_the_shared_init_table_and_the_watchdog() {
    let overlay = &defconfig()["BR2_ROOTFS_OVERLAY"];
    assert_eq!(
        overlay,
        "$(BR2_EXTERNAL_NEPTRON_PATH)/board/common/rootfs-overlay \
         $(BR2_EXTERNAL_NEPTRON_PATH)/board/pc/rootfs-overlay"
    );
    let inittab = read("board/pc/rootfs-overlay/etc/inittab");
    assert!(inittab.contains("::respawn:/usr/libexec/neptron/launch"));
    assert!(inittab.contains("::shutdown:/usr/libexec/neptron/stop"));
    let launch = read("board/common/rootfs-overlay/usr/libexec/neptron/launch");
    assert!(launch.contains("neptron-launch --power"));
    assert_eq!(
        parse(&read("board/rpi/system.config"))["CONFIG_BCM2835_WDT"],
        "y"
    );
}

#[test]
fn the_licence_notes_correct_the_firmware_label_and_say_what_may_be_shared() {
    let notes = read("board/rpi/LICENCES.txt");
    for line in [
        "063bcab6c8a90efb0d19f69d88cbbc7ec79cab68",
        "`bootcode.bin`",
        "`start.elf`",
        "`fixup.dat`",
        "They are not open source. Their licence is `legal/LICENCE.broadcom`.",
        "Buildroot labels the package `BSD-3-Clause` in `legal/manifest.csv`.",
        "That label is wrong for these terms",
        "Redistribution and use in binary form, without modification.",
        "running or using a Raspberry",
        "Copyright (c) 2006, Broadcom Corporation.",
        "Copyright (c) 2015, Raspberry Pi (Trading) Ltd",
        "DISCLAIMER.  THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND",
        "What may be distributed",
        "OP-054",
        "no Wi-Fi or Bluetooth firmware",
    ] {
        assert!(notes.contains(line), "LICENCES.txt misses {line:?}");
    }
    assert!(
        !notes.contains("OPEN LICENCE QUESTION"),
        "the Pi image holds no 15 kHz patch"
    );
    let post_image = read("board/rpi/post-image.sh");
    assert!(post_image.contains("cp \"$board/LICENCES.txt\" \"$part/LICENCES.txt\""));
    assert!(post_image.contains("\"$part/legal/LICENCE.broadcom\""));
    let post_build = read("board/rpi/post-build.sh");
    assert!(post_build.contains("\"$target/usr/share/licenses/LICENCES.txt\""));
}

#[test]
fn every_external_path_of_the_pi_image_exists() {
    for options in [defconfig(), dev_fragment()] {
        for (name, value) in options {
            for part in value.split_whitespace() {
                if let Some(rel) = part.strip_prefix("$(BR2_EXTERNAL_NEPTRON_PATH)/") {
                    let path = os_dir().join(rel);
                    assert!(path.exists(), "{name}: {} is missing", path.display());
                }
            }
        }
    }
    use std::os::unix::fs::PermissionsExt;
    for script in [
        "post-build.sh",
        "post-image.sh",
        "check-kernel-config.sh",
        "check-kernel-patches.sh",
        "cmdline.sh",
    ] {
        let mode = std::fs::metadata(os_dir().join("board/rpi").join(script))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "{script} is not executable");
    }
}
