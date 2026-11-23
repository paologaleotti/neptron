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
