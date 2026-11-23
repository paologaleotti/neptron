//! The crate folders here are written for the tests: invented crates in
//! the shape of a `cargo vendor` folder.

use super::*;

/// A folder that is removed when the test ends, also when it fails.
struct TempDir(PathBuf);

impl TempDir {
    fn create(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("xtask-notices-{name}-{}", std::process::id()));
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

fn id(name: &str, version: &str) -> CrateId {
    CrateId {
        name: name.to_owned(),
        version: version.to_owned(),
    }
}

fn manifest(name: &str, version: &str, licence: &str) -> String {
    format!(
        "[package]\nedition = \"2021\"\nname = \"{name}\"\nversion = \"{version}\"\n\
         description = \"\"\"\nA crate.\n\"\"\"\n{licence}\n\n[dependencies.libc]\nversion = \"0.2\"\n"
    )
}

const TREE: &str = "\
neptron v0.1.0 (/build/neptron-0.1.0/apps/neptron)
drm v0.15.0
drm-ffi v0.9.1
drm-ffi v0.9.1 (*)
libloading v0.8.9
libloading v0.9.0
neptron-kms v0.1.0 (/build/neptron-0.1.0/crates/neptron-kms) (*)
sdl3-sys v0.7.3+SDL-3.4.20

toml v1.2.1+spec-1.1.0 (*)
";

#[test]
fn the_tree_gives_each_external_crate_once_and_no_workspace_crate() {
    let ids = parse_tree(TREE).unwrap();
    assert_eq!(
        ids,
        [
            id("drm", "0.15.0"),
            id("drm-ffi", "0.9.1"),
            id("libloading", "0.8.9"),
            id("libloading", "0.9.0"),
            id("sdl3-sys", "0.7.3+SDL-3.4.20"),
            id("toml", "1.2.1+spec-1.1.0"),
        ]
    );
}

#[test]
fn a_line_that_is_not_a_crate_is_an_error_with_its_number() {
    for (text, line) in [
        ("drm v0.15.0\nwarning: something\n", 2),
        ("drm\n", 1),
        ("drm 0.15.0\n", 1),
    ] {
        match parse_tree(text) {
            Err(NoticesError::BadLine { line: got, .. }) => assert_eq!(got, line, "{text:?}"),
            other => panic!("{text:?}: {other:?}"),
        }
    }
}

#[test]
fn a_crate_is_found_by_name_and_version_in_both_folder_layouts() {
    let dir = TempDir::create("find");
    dir.write(
        "libloading/Cargo.toml",
        &manifest("libloading", "0.9.0", ""),
    );
    dir.write(
        "libloading-0.8.9/Cargo.toml",
        &manifest("libloading", "0.8.9", ""),
    );
    assert_eq!(
        find_crate(&dir.0, &id("libloading", "0.9.0")).unwrap(),
        dir.0.join("libloading")
    );
    assert_eq!(
        find_crate(&dir.0, &id("libloading", "0.8.9")).unwrap(),
        dir.0.join("libloading-0.8.9")
    );
    assert!(matches!(
        find_crate(&dir.0, &id("libloading", "0.7.0")),
        Err(NoticesError::NotVendored { .. })
    ));
    assert!(matches!(
        find_crate(&dir.0, &id("nix", "0.29.0")),
        Err(NoticesError::NotVendored { .. })
    ));
}

#[test]
fn a_crate_gives_its_licence_field_and_every_licence_notice_and_copyright_file() {
    let dir = TempDir::create("read");
    dir.write(
        "a/Cargo.toml",
        &manifest(
            "a",
            "1.0.0",
            "license = \"MIT OR Apache-2.0\"\nlicense-file = \"legal/TERMS.txt\"",
        ),
    );
    dir.write("a/LICENSE-MIT", "Copyright (c) A\r\nMIT text\r\n");
    dir.write("a/LICENSE-APACHE", "Apache text\n");
    dir.write("a/NOTICE", "Notice of a\n");
    dir.write("a/COPYRIGHT.md", "Copyright of a\n");
    dir.write("a/legal/TERMS.txt", "Terms\n");
    dir.write("a/README.md", "not a licence\n");
    dir.write("a/src/LICENSE", "not at the top\n");
    let c = read_crate(&dir.0.join("a"), id("a", "1.0.0")).unwrap();
    assert_eq!(c.expression, "MIT OR Apache-2.0");
    let names: Vec<&str> = c.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "COPYRIGHT.md",
            "LICENSE-APACHE",
            "LICENSE-MIT",
            "NOTICE",
            "legal/TERMS.txt"
        ]
    );
    assert_eq!(c.files[2].text, "Copyright (c) A\nMIT text\n");
}

#[test]
fn a_crate_with_no_licence_field_and_no_file_is_an_error() {
    let dir = TempDir::create("none");
    dir.write("b/Cargo.toml", &manifest("b", "1.0.0", ""));
    assert!(matches!(
        read_crate(&dir.0.join("b"), id("b", "1.0.0")),
        Err(NoticesError::NoLicence { .. })
    ));
    dir.write(
        "c/Cargo.toml",
        &manifest(
            "c",
            "1.0.0",
            "license = \"MIT\"\nauthors = [\"Ann <a@example.org>\", \"Bo\"]",
        ),
    );
    let c = read_crate(&dir.0.join("c"), id("c", "1.0.0")).unwrap();
    assert!(c.files.is_empty());
    let text = render("p", &[c], &[]);
    assert!(
        text.contains(
            "- c 1.0.0: MIT (the crate ships no licence file; its authors: Ann <a@example.org>, Bo)"
        ),
        "{text}"
    );
}

fn file(name: &str, text: &str) -> LicenceFile {
    LicenceFile {
        name: name.to_owned(),
        text: text.to_owned(),
    }
}

#[test]
fn a_text_that_several_crates_ship_is_printed_once_with_their_names() {
    let apache = "Apache License\nVersion 2.0\n";
    let crates = [
        CrateLicence {
            id: id("a", "1.0.0"),
            expression: "MIT OR Apache-2.0".to_owned(),
            authors: String::new(),
            files: vec![
                file("LICENSE-APACHE", apache),
                file("LICENSE-MIT", "Copyright (c) A\n"),
            ],
        },
        CrateLicence {
            id: id("b", "2.0.0"),
            expression: "Apache-2.0".to_owned(),
            authors: String::new(),
            files: vec![file("LICENSE", apache)],
        },
    ];
    let std = [
        file("LICENSE-MIT", "Copyright (c) The Rust Project Developers\n"),
        file("LICENSE-APACHE", apache),
    ];
    let text = render("neptron", &crates, &std);
    assert!(text.starts_with("Rust crates in neptron\n======================\n"));
    assert!(text.contains("- a 1.0.0: MIT OR Apache-2.0 (texts 1, 2)\n"));
    assert!(text.contains("- b 2.0.0: Apache-2.0 (texts 1)\n"));
    assert!(text.contains("at your option (texts 3, 1)."), "{text}");
    assert_eq!(text.matches("Version 2.0").count(), 1, "{text}");
    assert!(text.contains(
        "[1] a 1.0.0, LICENSE-APACHE; b 2.0.0, LICENSE; Rust standard library, LICENSE-APACHE\n\nApache License\nVersion 2.0\n"
    ));
    assert!(text.contains("[2] a 1.0.0, LICENSE-MIT\n\nCopyright (c) A\n"));
    assert!(text.contains(
        "[3] Rust standard library, LICENSE-MIT\n\nCopyright (c) The Rust Project Developers\n"
    ));
}

#[test]
fn the_standard_library_texts_come_from_the_first_toolchain_folder_that_has_them() {
    let dir = TempDir::create("std");
    dir.write("half/LICENSE-MIT", "MIT\n");
    dir.write("rust/LICENSE-MIT", "MIT\n");
    dir.write("rust/LICENSE-APACHE", "Apache\n");
    dir.write("rust/COPYRIGHT", "Copyright\n");
    dir.write("reuse/MIT.txt", "MIT\n");
    dir.write("reuse/Apache-2.0.txt", "Apache\n");
    dir.write("reuse/LLVM-exception.txt", "Exception\n");
    let dirs = [
        PathBuf::new(),
        dir.0.join("missing"),
        dir.0.join("half"),
        dir.0.join("rust"),
    ];
    let files = read_std(&dirs).unwrap();
    let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["LICENSE-MIT", "LICENSE-APACHE", "COPYRIGHT"]);
    let reuse = read_std(&[dir.0.join("reuse")]).unwrap();
    let names: Vec<&str> = reuse.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["MIT.txt", "Apache-2.0.txt", "LLVM-exception.txt"]);
    assert!(matches!(
        read_std(&dirs[..3]),
        Err(NoticesError::NoStd { .. })
    ));
}

#[test]
fn the_command_writes_the_notices_of_the_listed_crates() {
    let dir = TempDir::create("run");
    dir.write(
        "vendor/drm/Cargo.toml",
        &manifest("drm", "0.15.0", "license = \"MIT\""),
    );
    dir.write("vendor/drm/LICENSE", "Copyright (c) drm authors\n");
    dir.write("std/LICENSE-MIT", "MIT\n");
    dir.write("std/LICENSE-APACHE", "Apache\n");
    dir.write("crates.txt", "neptronctl v0.1.0 (/b)\ndrm v0.15.0\n");
    let p = |rel: &str| dir.0.join(rel).to_string_lossy().into_owned();
    let (crates, vendor, std, out) = (p("crates.txt"), p("vendor"), p("std"), p("out/ctl.txt"));
    run(&[
        "--program",
        "neptronctl",
        "--crates",
        &crates,
        "--vendor",
        &vendor,
        "--std",
        "",
        "--std",
        &std,
        "--out",
        &out,
    ])
    .unwrap();
    let text = std::fs::read_to_string(dir.0.join("out/ctl.txt")).unwrap();
    assert!(text.contains("- drm 0.15.0: MIT (texts 1)"), "{text}");
    assert!(text.contains("Copyright (c) drm authors"));
    assert!(!text.contains("neptronctl 0.1.0"));
    for bad in [
        &["--program", "x"][..],
        &["--program"],
        &[
            "--program",
            "x",
            "--crates",
            "c",
            "--vendor",
            "v",
            "--out",
            "o",
        ],
        &["--nope", "x"],
    ] {
        assert!(matches!(run(bad), Err(NoticesError::Usage(_))), "{bad:?}");
    }
}
