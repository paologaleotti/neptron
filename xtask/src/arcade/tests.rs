//! The DAT text here is written for the tests: invented sets, sizes and
//! CRCs in the shape of the FinalBurn Neo DAT.

use super::*;

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

const DAT: &str = r#"<?xml version="1.0"?>
<!DOCTYPE datafile PUBLIC "-//Test//DTD ROM Management Datafile//EN" "http://www.logiqx.com/Dats/datafile.dtd">

<datafile>
	<header>
		<name>Test DAT</name>
		<version>9.9.9</version>
	</header>
	<game name="starpatj" cloneof="starpat" romof="starpat" sourcefile="invented/d_star.cpp">
		<description>Star Patrol &amp; Friends (Japan)</description>
		<year>1990</year>
		<manufacturer>Nobody Ltd.</manufacturer>
		<rom name="sp1.bin" merge="sp1.bin" size="1024" crc="1234abcd"/>
		<rom name="spj2.bin" size="2048" crc="FFFF0000"/>
		<video type="raster" orientation="vertical" width="224" height="288" aspectx="3" aspecty="4"/>
		<driver status="preliminary"/>
	</game>
	<game name="starpat" romof="zbios" sourcefile="invented/d_star.cpp">
		<comment>ignored</comment>
		<description>Star Patrol &amp; Friends (World)</description>
		<year>199?</year>
		<manufacturer>Nobody Ltd.</manufacturer>
		<rom name="sp1.bin" size="1024" crc="1234abcd"/>
		<rom name="sp2.bin" size="2048" status="nodump"/>
		<sample name="ignored"/>
		<video type="raster" orientation="vertical" width="224" height="288" aspectx="3" aspecty="4"/>
		<driver status="good"/>
	</game>
	<game isbios="yes" name="zbios" sourcefile="invented/d_bios.cpp">
		<description>Zed BIOS</description>
		<year>1989</year>
		<manufacturer>Nobody Ltd.</manufacturer>
		<rom name="z1.bin" size="128" crc="000000aa"/>
	</game>
	<game name="diskgame" sourcefile="invented/d_disk.cpp">
		<description>Disk Game</description>
		<year>1994</year>
		<manufacturer>Nobody Ltd.</manufacturer>
		<rom name="d1.bin" size="16" crc="00000051"/>
		<disk name="diskgame" sha1="0123456789ABCDEF0123456789abcdef01234567" />
		<disk name="nosha" />
		<driver status="good"/>
	</game>
</datafile>
"#;

#[test]
fn a_dat_gives_every_field_of_the_index() {
    let index = index_of_dat(DAT, "Test.dat", COMMIT).expect("index");
    assert_eq!(index.source.core, "fbneo");
    assert_eq!(index.source.commit, COMMIT);
    assert_eq!(index.source.dat_name, "Test.dat");
    assert_eq!(index.source.dat_version, "9.9.9");
    assert_eq!(index.source.licence, LICENCE);
    let names: Vec<&str> = index.sets().iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["diskgame", "starpat", "starpatj", "zbios"]);

    let clone = index.find("starpatj").expect("clone");
    assert_eq!(clone.title, "Star Patrol & Friends");
    assert_eq!(clone.details, "Japan");
    assert_eq!(clone.parent.as_deref(), Some("starpat"));
    assert_eq!(clone.romof.as_deref(), Some("starpat"));
    assert!(clone.flags.vertical && clone.flags.not_working && clone.flags.preliminary);
    assert_eq!(clone.orientation, Orientation::Vertical);
    assert_eq!((clone.width, clone.height), (224, 288));
    assert_eq!(clone.board, "invented/d_star.cpp");
    assert_eq!(clone.roms[0].merge.as_deref(), Some("sp1.bin"));
    assert_eq!(clone.roms[1].crc, Some(0xffff_0000));

    let parent = index.find("starpat").expect("parent");
    assert_eq!(parent.year, "199?");
    assert!(!parent.flags.not_working && !parent.flags.bios);
    assert_eq!(parent.roms.len(), 2, "samples are not ROMs");
    assert_eq!(parent.roms[1].crc, None);
    assert_eq!(parent.roms[1].size, 2048);

    assert!(parent.disks.is_empty());

    let disks = &index.find("diskgame").expect("disk game").disks;
    assert_eq!(
        disks,
        &[
            DiskRecord {
                name: "diskgame".to_owned(),
                sha1: Some("0123456789abcdef0123456789abcdef01234567".to_owned()),
            },
            DiskRecord {
                name: "nosha".to_owned(),
                sha1: None,
            },
        ]
    );

    let bios = index.find("zbios").expect("bios");
    assert!(bios.flags.bios);
    assert_eq!(bios.orientation, Orientation::Horizontal);
    assert_eq!(index.bios_of(clone).map(|s| s.name.as_str()), Some("zbios"));
}

#[test]
fn the_index_of_a_dat_survives_the_text_form() {
    let index = index_of_dat(DAT, "Test.dat", COMMIT).expect("index");
    let text = arcade::encode(&index).expect("encodes");
    assert_eq!(arcade::decode(&text).expect("decodes"), index);
}

#[test]
fn a_value_the_index_cannot_take_names_the_set() {
    let cases = [
        (
            DAT.replace("orientation=\"vertical\"", "orientation=\"sideways\""),
            "video orientation",
        ),
        (
            DAT.replace("status=\"good\"", "status=\"imperfect\""),
            "driver status",
        ),
        (DAT.replace("size=\"128\"", "size=\"big\""), "rom size"),
        (DAT.replace("crc=\"000000aa\"", "crc=\"aa\""), "rom crc"),
        (
            DAT.replace("status=\"nodump\"", "status=\"baddump\""),
            "rom status",
        ),
        (DAT.replace("isbios=\"yes\"", "isbios=\"maybe\""), "isbios"),
        (DAT.replace("sha1=\"0123", "sha1=\"x123"), "disk sha1"),
        (
            DAT.replace("<disk name=\"nosha\" />", "<disk />"),
            "disk name",
        ),
    ];
    for (text, field) in cases {
        match index_of_dat(&text, "Test.dat", COMMIT) {
            Err(ArcadeError::Value { field: got, .. }) => assert_eq!(got, field),
            other => panic!("expected a bad {field}, got {other:?}"),
        }
    }
}

#[test]
fn a_tab_in_a_description_stops_the_encoder() {
    let text = DAT.replace("Zed BIOS", "Zed&#9;BIOS");
    let index = index_of_dat(&text, "Test.dat", COMMIT).expect("index");
    let err = arcade::encode(&index).expect_err("a tab");
    assert!(
        err.to_string().contains("`title` of the set `zbios`"),
        "{err}"
    );
}

#[test]
fn a_file_that_is_not_a_dat_is_refused() {
    assert!(matches!(
        index_of_dat("<other/>", "x", COMMIT),
        Err(ArcadeError::Shape(_))
    ));
    assert!(matches!(
        index_of_dat(
            "<datafile><header><version>1</version></header></datafile>",
            "x",
            COMMIT
        ),
        Err(ArcadeError::Shape("game element"))
    ));
    assert!(matches!(
        index_of_dat("<datafile>", "x", COMMIT),
        Err(ArcadeError::Xml(_))
    ));
    let twice = DAT.replace("name=\"starpatj\"", "name=\"starpat\"");
    assert!(matches!(
        index_of_dat(&twice, "x", COMMIT),
        Err(ArcadeError::Index(IndexError::Duplicate { .. }))
    ));
}

#[test]
fn the_arguments_need_a_commit_and_two_paths() {
    let ok =
        parse_args(&["--commit", &COMMIT.to_ascii_uppercase(), "a.dat", "out.idx"]).expect("valid");
    assert_eq!(
        ok,
        (
            COMMIT.to_owned(),
            PathBuf::from("a.dat"),
            PathBuf::from("out.idx")
        )
    );
    for args in [
        &["a.dat", "out.idx"][..],
        &["--commit", "abc", "a.dat", "out.idx"],
        &["--commit", COMMIT, "a.dat"],
        &["--commit", COMMIT, "a", "b", "c"],
        &["--commit"],
        &["--commit", COMMIT, "--other", "a", "b"],
    ] {
        assert!(
            matches!(parse_args(args), Err(ArcadeError::Usage(_))),
            "{args:?}"
        );
    }
}

#[test]
fn the_command_writes_the_index_file() {
    let dir = std::env::temp_dir().join(format!("xtask-arcade-{}", std::process::id()));
    // A leftover of an earlier run with the same process id.
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("folder");
    let dat = dir.join("Test.dat");
    fs::write(&dat, DAT).expect("dat");
    let out = dir.join("share/arcade/fbneo.idx");
    let (dat_arg, out_arg) = (dat.to_string_lossy(), out.to_string_lossy());
    run(&["--commit", COMMIT, &dat_arg, &out_arg]).expect("runs");
    let index = arcade::read(&out).expect("reads");
    assert_eq!(index.sets().len(), 4);
    assert_eq!(index.source.dat_name, "Test.dat");
    assert!(!dir.join("share/arcade/fbneo.idx.tmp").exists());
    let missing = dir.join("none.dat");
    let err = run(&["--commit", COMMIT, &missing.to_string_lossy(), &out_arg]).expect_err("no DAT");
    assert!(err.to_string().contains("none.dat"), "{err}");
    // The folder is temporary; one left behind does no harm.
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_frontend_package_installs_the_index_of_the_core_dat() {
    let os = Path::new(env!("CARGO_MANIFEST_DIR")).join("../os");
    let mk = fs::read_to_string(os.join("package/neptron/neptron.mk")).expect("the package");
    let step = mk
        .split("define NEPTRON_INSTALL_ARCADE_INDEX")
        .nth(1)
        .and_then(|rest| rest.split("endef").next())
        .expect("the index step");
    for part in [
        "cargo run --offline --locked",
        "--package xtask -- arcade-index",
        "--commit $(LIBRETRO_FBNEO_VERSION)",
        "\"$(NEPTRON_ARCADE_DAT)\"",
        "$(TARGET_DIR)/usr/share/neptron/arcade/fbneo.idx",
    ] {
        assert!(step.contains(part), "the step misses {part:?}");
    }
    assert!(mk.contains(
        "NEPTRON_ARCADE_DAT = $(LIBRETRO_FBNEO_DIR)/dats/FinalBurn Neo (ClrMame Pro XML, Arcade only).dat"
    ));
    assert!(mk.contains("NEPTRON_DEPENDENCIES += libretro-fbneo"));
    let install = mk
        .split("define NEPTRON_INSTALL_TARGET_CMDS")
        .nth(1)
        .and_then(|rest| rest.split("endef").next())
        .expect("the install step");
    let wipe = install
        .find("rm -rf $(TARGET_DIR)/usr/share/neptron")
        .expect("the wipe");
    let index = install
        .find("$(NEPTRON_INSTALL_ARCADE_INDEX)")
        .expect("the index");
    assert!(
        wipe < index,
        "the step that empties the folder fills it again after"
    );

    let core = fs::read_to_string(os.join("package/libretro-fbneo/libretro-fbneo.mk"))
        .expect("the core package");
    assert!(
        !core.contains("arcade-index"),
        "the core package makes no index"
    );
    assert!(!core.contains("DEPENDENCIES += neptron"));
    for board in ["pc", "rpi"] {
        let notes = fs::read_to_string(os.join(format!("board/{board}/LICENCES.txt")))
            .expect("the licence notes");
        assert!(
            notes.contains("/usr/share/neptron/arcade/fbneo.idx"),
            "{board}"
        );
    }
}

/// Runs the image check on a target folder with the core and the index
/// as given.
fn check_image(label: &str, core: bool, index: Option<&str>) -> std::process::Output {
    let target =
        std::env::temp_dir().join(format!("xtask-arcade-image-{label}-{}", std::process::id()));
    // A leftover of an earlier run with the same process id.
    let _ = fs::remove_dir_all(&target);
    if core {
        let so = target.join("usr/lib/libretro/fbneo_libretro.so");
        fs::create_dir_all(so.parent().expect("a folder")).expect("folder");
        fs::write(&so, b"").expect("core");
    }
    if let Some(text) = index {
        let idx = target.join("usr/share/neptron/arcade/fbneo.idx");
        fs::create_dir_all(idx.parent().expect("a folder")).expect("folder");
        fs::write(&idx, text).expect("index");
    }
    fs::create_dir_all(&target).expect("folder");
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../os/board/common/check-arcade-index.sh");
    let out = std::process::Command::new("sh")
        .arg(script)
        .arg(&target)
        .output()
        .expect("sh runs");
    // The folder is temporary; one left behind does no harm.
    let _ = fs::remove_dir_all(&target);
    out
}

#[test]
fn the_image_check_fails_for_a_core_with_no_index() {
    let missing = check_image("missing", true, None);
    assert!(!missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("no arcade index"),
        "{missing:?}"
    );
    assert!(
        !check_image("empty", true, Some("")).status.success(),
        "an empty file"
    );
    assert!(
        check_image("both", true, Some("neptron-arcade-index\t1\n"))
            .status
            .success()
    );
    assert!(check_image("no-core", false, None).status.success());
    let post_build = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../os/board/common/post-build.sh"),
    )
    .expect("the post-build step");
    assert!(post_build.contains("\"$(dirname \"$0\")/check-arcade-index.sh\" \"$target\""));
}
