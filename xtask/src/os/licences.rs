//! Tests of what the images say about their licences: the licence fields
//! of the NEPTRON packages and the notes of `os/board/*/LICENCES.txt`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The value of `<prefix>_<key> = ...` in a package makefile, with the
/// lines that end with a backslash joined.
fn mk_value(mk: &str, prefix: &str, key: &str) -> String {
    let start = format!("{prefix}_{key} = ");
    let mut lines = mk.lines().skip_while(|l| !l.starts_with(&start));
    let mut value = String::new();
    let mut line = lines.next().map(|l| &l[start.len()..]);
    while let Some(text) = line {
        let (text, more) = match text.trim_end().strip_suffix('\\') {
            Some(head) => (head, true),
            None => (text, false),
        };
        value.push_str(text.trim());
        value.push(' ');
        line = if more { lines.next() } else { None };
    }
    value.trim_end().to_owned()
}

/// The crate folders that `manifest` (a `Cargo.toml` path relative to the
/// root) reaches through path dependencies of its `[dependencies]` and
/// target dependency tables, optional ones included.
fn path_crates(manifest: &str) -> BTreeSet<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![root().join(manifest).parent().unwrap().to_path_buf()];
    while let Some(dir) = stack.pop() {
        let dir = std::fs::canonicalize(&dir).unwrap();
        if !seen.insert(dir.clone()) {
            continue;
        }
        let text = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        let mut normal = false;
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') {
                normal = line == "[dependencies]"
                    || (line.starts_with("[target.") && line.ends_with(".dependencies]"));
                continue;
            }
            if !normal {
                continue;
            }
            if let Some((_, rest)) = line.split_once("path = \"") {
                let rel = rest.split('"').next().unwrap();
                stack.push(dir.join(rel));
            }
        }
    }
    seen
}

fn links_switchres(manifest: &str) -> bool {
    path_crates(manifest)
        .iter()
        .any(|d| d.ends_with("crates/neptron-switchres-sys"))
}

/// The Rust packages of `os/package`: package folder, makefile prefix and
/// the `Cargo.toml` of the program.
const RUST_PACKAGES: [(&str, &str, &str); 3] = [
    ("neptron", "NEPTRON", "apps/neptron/Cargo.toml"),
    (
        "neptron-launch",
        "NEPTRON_LAUNCH",
        "apps/neptron-launch/Cargo.toml",
    ),
    ("neptronctl", "NEPTRONCTL", "apps/neptronctl/Cargo.toml"),
];

const GPL_3_0_SHA256: &str = "3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986";

#[test]
fn a_program_that_links_switchres_is_gpl_3_0_or_later_in_its_package() {
    let mut gpl = Vec::new();
    for (package, prefix, manifest) in RUST_PACKAGES {
        let mk = read(&format!("os/package/{package}/{package}.mk"));
        let licence = mk_value(&mk, prefix, "LICENSE");
        let files = mk_value(&mk, prefix, "LICENSE_FILES");
        let files: Vec<&str> = files.split_whitespace().collect();
        for file in &files {
            assert!(root().join(file).is_file(), "{package}: no file {file}");
        }
        assert!(
            licence.contains("MIT or Apache-2.0"),
            "{package}: {licence}"
        );
        assert!(files.contains(&"LICENSE-MIT") && files.contains(&"LICENSE-APACHE"));
        if links_switchres(manifest) {
            gpl.push(package);
            assert!(
                licence.starts_with("GPL-3.0+ (binary)"),
                "{package} links Switchres: {licence}"
            );
            assert!(
                licence.contains("GPL-2.0+ (Switchres"),
                "{package}: {licence}"
            );
            assert!(files.contains(&"LICENSE-GPL-3.0"), "{package}");
            assert!(files.contains(&"vendor/switchres/LICENSE"), "{package}");
        } else {
            assert!(!licence.contains("GPL"), "{package}: {licence}");
        }
    }
    assert_eq!(gpl, ["neptron", "neptronctl"], "ADR 0002");
    let text = std::fs::read(root().join("LICENSE-GPL-3.0")).unwrap();
    assert_eq!(crate::sha256::hex(&text), GPL_3_0_SHA256, "the GPL 3 text");
}

fn notes(board: &str) -> String {
    let text = read(&format!("os/board/{board}/LICENCES.txt"));
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The non-commercial condition covers the three cores and the image that
/// holds them, and reads as no restriction on the other programs
/// (ADR 0019).
#[test]
fn the_notes_limit_sale_by_the_cores_only_and_name_switchres() {
    for board in ["pc", "rpi"] {
        let flat = notes(board);
        for fact in [
            "This image contains programs under different licences.",
            "Three libretro cores, FinalBurn Neo, Snes9x and Genesis Plus GX, may not be sold or used commercially under their own licences.",
            "So this image, which contains them, may not be sold.",
            "Every other program in this image keeps its own licence, and its rights are not reduced.",
            "to redistribute the rest under its own terms",
            "commit 09272ac24640daee45f5be15b1b856ecd220dda2",
            "Chris Kennedy, Antonio Giner, Alexandre Wodarczyk and Gil Delescluse",
            "So these two programs are under the GNU General Public License 3.0 or later.",
            "ADR 0019",
        ] {
            assert!(flat.contains(fact), "{board}: LICENCES.txt misses {fact:?}");
        }
        for gone in [
            "Nobody may sell it",
            "The NEPTRON programs are under the MIT licence or the Apache License 2.0, at your option:",
        ] {
            assert!(
                !flat.contains(gone),
                "{board}: LICENCES.txt still says {gone:?}"
            );
        }
    }
    let readme = read("vendor/switchres/README.md");
    assert!(readme.contains("`09272ac24640daee45f5be15b1b856ecd220dda2`"));
}

#[test]
fn each_rust_program_writes_its_crate_notices_and_the_c_tools_ship_their_licence() {
    for (package, prefix, _) in RUST_PACKAGES {
        let mk = read(&format!("os/package/{package}/{package}.mk"));
        let call = format!("\t$(call NEPTRON_WORKSPACE_NOTICES,{package},--package {package}");
        let install = mk
            .split(&format!("define {prefix}_INSTALL_TARGET_CMDS"))
            .nth(1)
            .and_then(|rest| rest.split("endef").next())
            .unwrap_or_else(|| panic!("{package}: no install step"));
        assert!(install.contains(&call), "{package}: {install}");
    }
    let neptron = read("os/package/neptron/neptron.mk");
    assert!(neptron.contains("--package neptron --features $(NEPTRON_FEATURES))"));
    let files = mk_value(&neptron, "NEPTRON", "LICENSE_FILES");
    assert!(
        files.contains("crates/neptron-ui/assets/fonts/spleen/LICENSE"),
        "ADR 0004"
    );
    assert!(mk_value(&neptron, "NEPTRON", "LICENSE").contains("BSD-2-Clause (Spleen font)"));
    let external = read("os/external.mk");
    for fact in [
        "--edges normal,no-proc-macro --prefix none $(2)",
        "rust-notices",
        "--vendor $(@D)/VENDOR",
        "--std $(HOST_RUST_BIN_DIR)",
        "--out $(NEPTRON_NOTICES_DIR)/$(1).txt",
        "NEPTRON_NOTICES_DIR = $(BASE_DIR)/neptron-notices",
    ] {
        assert!(external.contains(fact), "external.mk misses {fact:?}");
    }
    for (package, prefix) in [
        ("neptron-capture", "NEPTRON_CAPTURE"),
        ("neptron-vkms-probe", "NEPTRON_VKMS_PROBE"),
    ] {
        let mk = read(&format!("os/package/{package}/{package}.mk"));
        assert_eq!(
            mk_value(&mk, prefix, "LICENSE_FILES"),
            "LICENSE-MIT LICENSE-APACHE"
        );
        assert!(mk.contains(&format!(
            "{prefix}_POST_RSYNC_HOOKS += NEPTRON_WORKSPACE_LICENCE_FILES"
        )));
    }
    let gpgx = read("os/package/libretro-genesis-plus-gx/libretro-genesis-plus-gx.mk");
    let licence = mk_value(&gpgx, "LIBRETRO_GENESIS_PLUS_GX", "LICENSE");
    assert!(licence.contains("LGPL-2.1+ (Nuked OPN2"), "{licence}");
    for board in ["pc", "rpi"] {
        let flat = notes(board);
        for fact in [
            "Spleen font by Frederic Cambus, under the BSD 2-Clause licence",
            "`legal/neptron-rust/<program>.txt` on this partition",
            "`/usr/share/licenses/neptron-rust`",
            "The Nuked OPN2 core, the NTSC filter and Blip Buffer: the GNU Lesser General Public License 2.1 or later.",
        ] {
            assert!(flat.contains(fact), "{board}: LICENCES.txt misses {fact:?}");
        }
        let post_image = read(&format!("os/board/{board}/post-image.sh"));
        assert!(post_image.contains(
            "cp -r \"$TARGET_DIR/usr/share/licenses/neptron-rust\" \"$part/legal/neptron-rust\""
        ));
        let post_build = read(&format!("os/board/{board}/post-build.sh"));
        assert!(post_build.contains("board/common/install-licences.sh\" \"$target\""));
    }
}

/// A folder that is removed when the test ends, also when it fails.
struct TempDir(PathBuf);

impl TempDir {
    fn create(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("xtask-licences-{name}-{}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).unwrap();
        }
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // A folder left in the system temp folder does no harm.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs install-licences.sh on a fake Buildroot output in `dir`.
fn install_licences(dir: &TempDir) -> std::process::Output {
    std::process::Command::new("sh")
        .arg(root().join("os/board/common/install-licences.sh"))
        .arg(dir.0.join("target"))
        .arg(dir.0.join("LICENCES.txt"))
        .env("BASE_DIR", &dir.0)
        .output()
        .unwrap()
}

#[test]
fn the_image_gets_the_legal_info_texts_and_the_notices_of_each_rust_program() {
    let dir = TempDir::create("install");
    dir.write("LICENCES.txt", "notes\n");
    dir.write("legal-info/manifest.csv", "PACKAGE,VERSION\n");
    dir.write("legal-info/licenses/busybox-1/LICENSE", "GPL\n");
    dir.write("target/usr/share/licenses/stale/LICENSE", "old\n");
    dir.write("target/usr/bin/neptron", "");
    dir.write("target/usr/bin/neptronctl", "");
    dir.write("neptron-notices/neptron.txt", "Rust crates in neptron\n");
    let out = install_licences(&dir);
    assert!(!out.status.success(), "neptronctl has no notices");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("/usr/bin/neptronctl has no Rust crate notices"),
        "{stderr}"
    );

    dir.write(
        "neptron-notices/neptronctl.txt",
        "Rust crates in neptronctl\n",
    );
    dir.write("neptron-notices/neptron-launch.txt", "not in this image\n");
    let out = install_licences(&dir);
    assert!(out.status.success(), "{out:?}");
    let licenses = dir.0.join("target/usr/share/licenses");
    let read = |rel: &str| std::fs::read_to_string(licenses.join(rel)).unwrap();
    assert_eq!(read("LICENCES.txt"), "notes\n");
    assert_eq!(read("manifest.csv"), "PACKAGE,VERSION\n");
    assert_eq!(read("busybox-1/LICENSE"), "GPL\n");
    assert_eq!(read("neptron-rust/neptron.txt"), "Rust crates in neptron\n");
    assert_eq!(
        read("neptron-rust/neptronctl.txt"),
        "Rust crates in neptronctl\n"
    );
    assert!(!licenses.join("neptron-rust/neptron-launch.txt").exists());
    assert!(!licenses.join("stale").exists());

    std::fs::remove_file(dir.0.join("legal-info/manifest.csv")).unwrap();
    assert!(!install_licences(&dir).status.success());
}
