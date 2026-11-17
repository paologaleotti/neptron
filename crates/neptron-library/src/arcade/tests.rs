//! Every set, ROM, size and CRC here is invented for the tests.

use super::*;

fn source() -> IndexSource {
    IndexSource {
        core: "fbneo".to_owned(),
        commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        dat_name: "Test DAT.dat".to_owned(),
        dat_version: "9.9.9".to_owned(),
        licence: "test licence".to_owned(),
    }
}

fn rom(name: &str, size: u64, crc: Option<u32>) -> RomRecord {
    RomRecord {
        name: name.to_owned(),
        size,
        crc,
        merge: None,
    }
}

fn sample() -> ArcadeIndex {
    let mut bios = SetRecord::new("zbios".to_owned(), "Zed BIOS".to_owned());
    bios.flags.bios = true;
    bios.roms = vec![rom("z1.bin", 128, Some(0x0000_00aa))];
    let mut parent = SetRecord::new("starpat".to_owned(), "Star Patrol".to_owned());
    parent.romof = Some("zbios".to_owned());
    parent.flags.vertical = true;
    parent.orientation = Orientation::Vertical;
    parent.width = 224;
    parent.height = 288;
    parent.year = "1990".to_owned();
    parent.maker = "Nobody Ltd.".to_owned();
    parent.board = "invented/d_star.cpp".to_owned();
    parent.details = "World".to_owned();
    parent.roms = vec![
        rom("sp1.bin", 1024, Some(0x1234_abcd)),
        rom("sp2.bin", 2048, None),
    ];
    let mut clone = SetRecord::new("starpatj".to_owned(), "Star Patrol".to_owned());
    clone.parent = Some("starpat".to_owned());
    clone.romof = Some("starpat".to_owned());
    clone.flags.preliminary = true;
    clone.flags.not_working = true;
    clone.details = "Japan".to_owned();
    let mut merged = rom("sp1.bin", 1024, Some(0x1234_abcd));
    merged.merge = Some("sp1.bin".to_owned());
    clone.roms = vec![merged, rom("spj2.bin", 2048, Some(0xffff_0000))];
    ArcadeIndex::from_sets(source(), vec![clone, bios, parent]).expect("valid sets")
}

#[test]
fn an_index_survives_the_text_form() {
    let index = sample();
    let text = encode(&index).expect("encodes");
    assert_eq!(decode(&text).expect("decodes"), index);
}

#[test]
fn the_text_form_follows_the_spec() {
    let text = encode(&sample()).expect("encodes");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "neptron-arcade-index\t1");
    assert_eq!(
        lines[1],
        "core\tfbneo\t0123456789abcdef0123456789abcdef01234567"
    );
    assert_eq!(lines[2], "dat\tTest DAT.dat\t9.9.9");
    assert_eq!(lines[3], "licence\ttest licence");
    assert_eq!(
        lines[4],
        "set\tstarpat\t-\tzbios\tv\t1990\tNobody Ltd.\tinvented/d_star.cpp\tv\t224\t288\tStar Patrol\tWorld"
    );
    assert_eq!(lines[5], "rom\tsp1.bin\t1024\t1234abcd\t-\t-");
    assert_eq!(lines[6], "rom\tsp2.bin\t2048\t-\t-\td");
    assert_eq!(
        lines[7],
        "set\tstarpatj\tstarpat\tstarpat\tnp\t\t\t\th\t0\t0\tStar Patrol\tJapan"
    );
    assert_eq!(lines[8], "rom\tsp1.bin\t1024\t1234abcd\tsp1.bin\t-");
    assert!(
        lines[10].starts_with("set\tzbios\t-\t-\tb\t"),
        "sorted by name"
    );
}

#[test]
fn the_lookup_ignores_ascii_case() {
    let index = sample();
    assert_eq!(
        index.find("StarPatJ").map(|s| s.details.as_str()),
        Some("Japan")
    );
    assert_eq!(
        index.find("STARPAT").map(|s| s.name.as_str()),
        Some("starpat")
    );
    assert!(index.find("starpa").is_none());
    assert!(index.find("").is_none());
}

#[test]
fn the_bios_is_found_through_the_romof_chain() {
    let index = sample();
    let clone = index.find("starpatj").expect("clone");
    assert_eq!(index.bios_of(clone).map(|s| s.name.as_str()), Some("zbios"));
    let bios = index.find("zbios").expect("bios");
    assert!(
        index.bios_of(bios).is_none(),
        "a BIOS has no BIOS of its own"
    );
    assert_eq!(index.bios_count(), 1);
}

#[test]
fn a_romof_loop_ends() {
    let mut a = SetRecord::new("a".to_owned(), "A".to_owned());
    a.romof = Some("b".to_owned());
    let mut b = SetRecord::new("b".to_owned(), "B".to_owned());
    b.romof = Some("a".to_owned());
    let index = ArcadeIndex::from_sets(source(), vec![a, b]).expect("valid");
    let a = index.find("a").expect("a");
    assert!(index.bios_of(a).is_none());
}

#[test]
fn a_bad_set_name_or_a_duplicate_is_refused() {
    let upper = SetRecord::new("Star".to_owned(), "x".to_owned());
    assert!(matches!(
        ArcadeIndex::from_sets(source(), vec![upper]),
        Err(IndexError::SetName { set }) if set == "Star"
    ));
    let empty = SetRecord::new(String::new(), "x".to_owned());
    assert!(ArcadeIndex::from_sets(source(), vec![empty]).is_err());
    let twice = vec![
        SetRecord::new("a".to_owned(), "x".to_owned()),
        SetRecord::new("a".to_owned(), "y".to_owned()),
    ];
    assert!(matches!(
        ArcadeIndex::from_sets(source(), twice),
        Err(IndexError::Duplicate { set }) if set == "a"
    ));
}

#[test]
fn the_encoder_names_the_set_and_the_field_of_a_tab() {
    let mut set = SetRecord::new("a".to_owned(), "Tab\there".to_owned());
    set.year = "1990".to_owned();
    let index = ArcadeIndex::from_sets(source(), vec![set]).expect("valid");
    let err = encode(&index).expect_err("a tab");
    assert!(
        matches!(&err, IndexError::Separator { set, field } if set == "a" && *field == "title")
    );
    assert!(err.to_string().contains("`title` of the set `a`"), "{err}");

    let mut set = SetRecord::new("a".to_owned(), "x".to_owned());
    set.roms = vec![rom("line\nbreak", 1, Some(1))];
    let index = ArcadeIndex::from_sets(source(), vec![set]).expect("valid");
    assert!(matches!(
        encode(&index),
        Err(IndexError::Separator {
            field: "rom name",
            ..
        })
    ));

    let mut index = sample();
    index.source.dat_name = "a\rb".to_owned();
    assert!(matches!(
        encode(&index),
        Err(IndexError::Separator {
            field: "dat name",
            ..
        })
    ));
}

#[test]
fn the_encoder_refuses_a_name_that_reads_as_none() {
    let mut set = SetRecord::new("a".to_owned(), "x".to_owned());
    set.parent = Some("-".to_owned());
    let index = ArcadeIndex::from_sets(source(), vec![set]).expect("valid");
    assert!(matches!(
        encode(&index),
        Err(IndexError::Reserved {
            field: "cloneof",
            ..
        })
    ));
    let mut set = SetRecord::new("a".to_owned(), "x".to_owned());
    set.roms = vec![rom("", 1, Some(1))];
    let index = ArcadeIndex::from_sets(source(), vec![set]).expect("valid");
    assert!(matches!(
        encode(&index),
        Err(IndexError::Reserved {
            field: "rom name",
            ..
        })
    ));
}

const HEAD: &str = "neptron-arcade-index\t1\ncore\tfbneo\tabc\ndat\td.dat\t1\n";

#[test]
fn another_magic_or_a_newer_version_is_refused_with_both_names() {
    let err = decode("other-file\t1\n").expect_err("magic");
    assert!(
        err.to_string().contains("`other-file`") && err.to_string().contains(MAGIC),
        "{err}"
    );
    let err = decode("neptron-arcade-index\t2\n").expect_err("version");
    assert!(
        err.to_string().contains("`2`") && err.to_string().contains("up to 1"),
        "{err}"
    );
    assert!(matches!(
        decode("neptron-arcade-index\t0\n"),
        Err(IndexError::Version { .. })
    ));
    assert!(matches!(
        decode("neptron-arcade-index\n"),
        Err(IndexError::Version { .. })
    ));
    assert!(matches!(decode(""), Err(IndexError::Magic { .. })));
}

#[test]
fn unknown_lines_and_later_fields_are_skipped() {
    let text = format!(
        "{HEAD}future\tsomething\nset\ta\t-\t-\t-\t1990\tM\tb.cpp\th\t320\t240\tA\t\tlater field\nrom\tr\t4\t0000000f\t-\t-\tlater\n"
    );
    let index = decode(&text).expect("decodes");
    let a = index.find("a").expect("a");
    assert_eq!(a.title, "A");
    assert_eq!(a.details, "");
    assert_eq!(a.roms, vec![rom("r", 4, Some(15))]);
}

#[test]
fn a_set_flag_that_this_reader_does_not_know_is_skipped() {
    let text = format!("{HEAD}set\ta\t-\t-\tbvxp\t\t\t\th\t0\t0\tA\t\n");
    let flags = decode(&text).expect("decodes").sets()[0].flags;
    assert!(flags.bios && flags.vertical && flags.preliminary && !flags.not_working);
}

#[test]
fn every_broken_line_is_refused_with_its_number() {
    let set = "set\ta\t-\t-\t-\t\t\t\th\t0\t0\tA\t";
    let cases = [
        (format!("{HEAD}{set}\n{set}\n"), "set order"),
        (
            format!("{HEAD}set\tb\t-\t-\t-\t\t\t\th\t0\t0\tB\t\n{set}\n"),
            "set order",
        ),
        (
            format!("{HEAD}rom\tr\t1\t00000001\t-\t-\n"),
            "rom before any set",
        ),
        (
            format!("{HEAD}set\tA\t-\t-\t-\t\t\t\th\t0\t0\tA\t\n"),
            "set name",
        ),
        (
            format!("{HEAD}set\ta\t\t-\t-\t\t\t\th\t0\t0\tA\t\n"),
            "parent",
        ),
        (
            format!("{HEAD}set\ta\t-\t-\tvb\t\t\t\th\t0\t0\tA\t\n"),
            "set flags",
        ),
        (
            format!("{HEAD}set\ta\t-\t-\tbb\t\t\t\th\t0\t0\tA\t\n"),
            "set flags",
        ),
        (
            format!("{HEAD}set\ta\t-\t-\t\t\t\t\th\t0\t0\tA\t\n"),
            "set flags",
        ),
        (
            format!("{HEAD}set\ta\t-\t-\t-\t\t\t\tx\t0\t0\tA\t\n"),
            "orientation",
        ),
        (
            format!("{HEAD}set\ta\t-\t-\t-\t\t\t\th\t-1\t0\tA\t\n"),
            "width",
        ),
        (
            format!("{HEAD}set\ta\t-\t-\t-\t\t\t\th\t0\t0\tA\n"),
            "details",
        ),
        (
            format!("{HEAD}{set}\nrom\tr\tx\t00000001\t-\t-\n"),
            "rom size",
        ),
        (
            format!("{HEAD}{set}\nrom\tr\t1\t0000001\t-\t-\n"),
            "rom crc",
        ),
        (
            format!("{HEAD}{set}\nrom\tr\t1\t0000000F\t-\t-\n"),
            "rom crc",
        ),
        (format!("{HEAD}{set}\nrom\tr\t1\t-\t-\t-\n"), "rom crc"),
        (
            format!("{HEAD}{set}\nrom\tr\t1\t00000001\t-\td\n"),
            "rom crc",
        ),
        (
            format!("{HEAD}{set}\nrom\tr\t1\t00000001\t-\tx\n"),
            "rom flags",
        ),
        (
            format!("{HEAD}{set}\nrom\t-\t1\t00000001\t-\t-\n"),
            "rom name",
        ),
    ];
    for (text, what) in cases {
        match decode(&text) {
            Err(IndexError::Line { line, what: got }) => {
                assert_eq!(got, what, "{text:?}");
                assert!(line >= 4, "{text:?}");
            }
            other => panic!("{text:?} gave {other:?}, expected bad {what}"),
        }
    }
}

#[test]
fn the_header_lines_must_come_before_the_sets() {
    let set = "set\ta\t-\t-\t-\t\t\t\th\t0\t0\tA\t\n";
    let no_core = format!("neptron-arcade-index\t1\ndat\td\t1\n{set}");
    assert!(matches!(
        decode(&no_core),
        Err(IndexError::MissingHeader { what: "core" })
    ));
    let no_dat = format!("neptron-arcade-index\t1\ncore\tfbneo\tabc\n{set}");
    assert!(matches!(
        decode(&no_dat),
        Err(IndexError::MissingHeader { what: "dat" })
    ));
    assert!(matches!(
        decode("neptron-arcade-index\t1\n"),
        Err(IndexError::MissingHeader { .. })
    ));
}

#[test]
fn a_file_is_read_with_a_size_limit() {
    let dir = crate::test_dir::TestDir::new("arcade-read");
    let path = dir.path().join("fbneo.idx");
    std::fs::write(&path, encode(&sample()).expect("encodes")).expect("write");
    assert_eq!(read(&path).expect("reads"), sample());
    std::fs::write(&path, [0xff, 0xfe]).expect("write");
    assert!(matches!(
        read(&path),
        Err(IndexError::NotText { offset: 0 })
    ));
    let missing = dir.path().join("none.idx");
    let err = read(&missing).expect_err("missing");
    assert!(matches!(&err, IndexError::Io { .. }));
    assert!(err.to_string().contains("none.idx"), "{err}");
    let file = std::fs::File::create(&path).expect("create");
    file.set_len(MAX_INDEX_BYTES + 1).expect("grow");
    assert!(matches!(read(&path), Err(IndexError::TooLarge { .. })));
}

#[test]
fn a_description_gives_a_title_and_details() {
    let cases = [
        (
            "Star Patrol - Mission One (XYZ-0001 ~ XYZ-0002)",
            ("Star Patrol - Mission One", "XYZ-0001 ~ XYZ-0002"),
        ),
        ("Star Patrol", ("Star Patrol", "")),
        (
            "Star Patrol (Japan) (bootleg)",
            ("Star Patrol (Japan)", "bootleg"),
        ),
        ("Game (World (rev 1))", ("Game", "World (rev 1)")),
        ("  Game (x)  ", ("Game", "x")),
        ("(only a group)", ("(only a group)", "")),
        ("Broken)", ("Broken)", "")),
        ("Game [hack]", ("Game [hack]", "")),
        ("Game (\u{00e9}t\u{00e9})", ("Game", "\u{00e9}t\u{00e9}")),
        ("", ("", "")),
    ];
    for (description, expected) in cases {
        assert_eq!(split_description(description), expected, "{description:?}");
    }
}

/// Every prefix of a valid file gives an index or an error, never a panic.
/// A cut before the `dat` line is whole is an error; a later cut can be a
/// valid smaller index, with no set past the cut.
#[test]
fn every_cut_prefix_reads_or_fails_cleanly() {
    let text = encode(&sample()).expect("encodes");
    let full = sample();
    for end in (0..=text.len()).filter(|&i| text.is_char_boundary(i)) {
        match decode(&text[..end]) {
            Ok(index) => assert!(index.sets().len() <= full.sets().len(), "{end}"),
            Err(e) => assert!(!e.to_string().is_empty()),
        }
    }
    // Up to the tab before the DAT version, the `dat` line is missing or
    // short of a field. A cut inside a header value after that is still a
    // valid, shorter value.
    let dat = text.find("\ndat\t").expect("a dat line") + 1;
    let version = dat + text[dat..].match_indices('\t').nth(1).expect("two tabs").0;
    for end in 0..=version {
        assert!(decode(&text[..end]).is_err(), "a cut header at {end}");
    }
}

/// A small xorshift generator, so the random test repeats exactly.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// Random bytes, alone and after a valid header, never make the reader
/// panic. The bytes come from a small alphabet of the format's own
/// characters, so many reach the field parsers.
#[test]
fn random_bytes_never_panic() {
    const ALPHABET: &[u8] = b"\t\n-0123456789abcdefvbnpdhsetromcorelicence\xff";
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..3_000 {
        let len = (rng.next() % 300) as usize;
        let bytes: Vec<u8> = (0..len)
            .map(|_| ALPHABET[(rng.next() % ALPHABET.len() as u64) as usize])
            .collect();
        let text = String::from_utf8_lossy(&bytes);
        assert!(decode(&text).is_err(), "no magic, no index");
        // Ok or Err are both correct here; the test is that it returns.
        let _ = decode(&format!("{HEAD}{text}"));
        let _ = decode(&format!(
            "{HEAD}set\ta\t-\t-\t-\t\t\t\th\t0\t0\tA\t\n{text}"
        ));
    }
}
