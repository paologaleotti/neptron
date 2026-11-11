use super::format::encode as encode_raw;
use super::*;
use crate::index::{Game, MAX_TEXT_BYTES, SystemGames};
use crate::test_dir::TestDir;

fn catalog() -> Catalog {
    Catalog::builtin().expect("the built-in definitions are valid")
}

fn decode(bytes: &[u8]) -> Result<Index, CacheError> {
    super::decode(bytes, &catalog())
}

fn sample() -> Index {
    let games = |system: &str, names: &[&str]| {
        SystemGames::new(
            system.to_owned(),
            names
                .iter()
                .map(|n| Game::new((*n).to_owned(), format!("{system}/{n}.rom")))
                .collect(),
        )
    };
    Index::from_systems(vec![
        games("nes", &["Contra", "Mega Man 2", "Écran"]),
        games("megadrive", &["Sonic", "Gunstar Heroes"]),
    ])
}

/// A small xorshift generator, so the random tests repeat exactly.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Replaces the payload of a cache file and writes a new length and
/// checksum, so the bytes pass the header checks and reach the parser.
fn with_payload(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&CACHE_VERSION.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(&super::crc32::crc32(payload).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

#[test]
fn round_trip_keeps_the_index() {
    let index = sample();
    assert_eq!(decode(&encode(&index)).expect("a valid cache"), index);
    assert_eq!(
        decode(&encode(&Index::new())).expect("a valid cache"),
        Index::new()
    );
}

#[test]
fn header_has_the_documented_layout() {
    let bytes = encode(&sample());
    assert_eq!(&bytes[..8], b"NEPTRIDX");
    assert_eq!(bytes[8..12], 1u32.to_le_bytes());
    assert_eq!(
        bytes[12..20],
        ((bytes.len() - HEADER_BYTES) as u64).to_le_bytes()
    );
    assert_eq!(bytes, encode_raw(&sample()));
}

#[test]
fn every_truncation_is_rejected() {
    let bytes = encode(&sample());
    for len in 0..bytes.len() {
        let err = decode(&bytes[..len]).expect_err("a truncated cache");
        if len < HEADER_BYTES {
            assert!(matches!(err, CacheError::Truncated { .. }), "{len}: {err}");
        } else {
            assert!(matches!(err, CacheError::Length { .. }), "{len}: {err}");
        }
    }
}

#[test]
fn every_single_bit_flip_is_rejected() {
    let bytes = encode(&sample());
    for i in 0..bytes.len() {
        for bit in 0..8 {
            let mut damaged = bytes.clone();
            damaged[i] ^= 1 << bit;
            assert!(decode(&damaged).is_err(), "byte {i} bit {bit}");
        }
    }
}

#[test]
fn version_mismatch_is_rejected() {
    let mut bytes = encode(&sample());
    bytes[8..12].copy_from_slice(&2u32.to_le_bytes());
    let err = decode(&bytes).expect_err("another version");
    assert!(matches!(
        err,
        CacheError::Version {
            found: 2,
            expected: 1
        }
    ));
}

#[test]
fn trailing_bytes_are_rejected() {
    let mut bytes = encode(&sample());
    bytes.push(0);
    assert!(matches!(decode(&bytes), Err(CacheError::Length { .. })));
    let mut payload = encode(&sample())[HEADER_BYTES..].to_vec();
    payload.push(0);
    assert!(matches!(
        decode(&with_payload(&payload)),
        Err(CacheError::Malformed {
            what: "trailing bytes",
            ..
        })
    ));
}

#[test]
fn random_bytes_never_panic() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..2_000 {
        let len = rng.below(200);
        let bytes: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
        assert!(decode(&bytes).is_err());
        // The same bytes behind a valid header reach the parser. Ok or Err
        // are both correct; the test is that the call returns.
        let _ = decode(&with_payload(&bytes));
    }
}

#[test]
fn damaged_payload_with_a_valid_checksum_never_panics() {
    let payload = encode(&sample())[HEADER_BYTES..].to_vec();
    let mut rng = Rng(42);
    for i in 0..payload.len() {
        for value in [0x00, 0xFF, 0x7F, 0x80, rng.next() as u8] {
            let mut damaged = payload.clone();
            damaged[i] = value;
            // Ok or Err are both correct here; the test is that it returns.
            if let Ok(index) = decode(&with_payload(&damaged)) {
                assert!(index.total() <= sample().total() + 1);
            }
        }
    }
    for len in 0..payload.len() {
        assert!(decode(&with_payload(&payload[..len])).is_err(), "{len}");
    }
}

#[test]
fn huge_counts_are_rejected_before_allocation() {
    let mut payload = u32::MAX.to_le_bytes().to_vec();
    assert!(matches!(
        decode(&with_payload(&payload)),
        Err(CacheError::Malformed {
            what: "system count",
            ..
        })
    ));
    payload = 1u32.to_le_bytes().to_vec();
    payload.extend_from_slice(&3u32.to_le_bytes());
    payload.extend_from_slice(b"nes");
    payload.extend_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        decode(&with_payload(&payload)),
        Err(CacheError::Malformed {
            what: "game count",
            ..
        })
    ));
}

fn system_entry(id: &[u8]) -> Vec<u8> {
    let mut p = (id.len() as u32).to_le_bytes().to_vec();
    p.extend_from_slice(id);
    p.extend_from_slice(&0u32.to_le_bytes());
    p
}

fn systems_payload(entries: &[&[u8]]) -> Vec<u8> {
    let mut p = (entries.len() as u32).to_le_bytes().to_vec();
    for id in entries {
        p.extend_from_slice(&system_entry(id));
    }
    p
}

#[test]
fn bad_system_ids_are_rejected() {
    for entries in [&[&b"\xff\xfe"[..]][..], &[b"nes", b"nes"]] {
        let err = decode(&with_payload(&systems_payload(entries))).expect_err("a bad id");
        assert!(
            matches!(
                err,
                CacheError::Malformed {
                    what: "system id",
                    ..
                }
            ),
            "{entries:?}: {err}"
        );
    }
    for id in [&b""[..], b"psx", b"NES"] {
        let err = decode(&with_payload(&systems_payload(&[id]))).expect_err("an unknown id");
        assert!(matches!(err, CacheError::UnknownSystem { .. }), "{err}");
    }
    assert!(decode(&with_payload(&systems_payload(&[b"nes", b"snes"]))).is_ok());
}

#[test]
fn more_systems_than_the_catalog_are_refused_by_the_count() {
    // 40,000 distinct empty systems: a valid checksum, and a quadratic
    // duplicate check would take seconds. The count check refuses the
    // file before the first id is read.
    let ids: Vec<Vec<u8>> = (0..40_000).map(|i| format!("s{i}").into_bytes()).collect();
    let refs: Vec<&[u8]> = ids.iter().map(Vec::as_slice).collect();
    let err = decode(&with_payload(&systems_payload(&refs))).expect_err("too many systems");
    assert!(
        matches!(err, CacheError::Malformed { what: "system count", offset } if offset == HEADER_BYTES + 4),
        "{err}"
    );
    let four = systems_payload(&[b"nes", b"snes", b"megadrive", b"nes"]);
    let err = decode(&with_payload(&four)).expect_err("four systems");
    assert!(
        matches!(
            err,
            CacheError::Malformed {
                what: "system count",
                ..
            }
        ),
        "{err}"
    );
}

fn one_game(system: &str, name: &str, path: &str) -> Vec<u8> {
    let mut p = 1u32.to_le_bytes().to_vec();
    let put = |p: &mut Vec<u8>, text: &str| {
        p.extend_from_slice(&(text.len() as u32).to_le_bytes());
        p.extend_from_slice(text.as_bytes());
    };
    put(&mut p, system);
    p.extend_from_slice(&1u32.to_le_bytes());
    put(&mut p, name);
    put(&mut p, path);
    p
}

#[test]
fn game_paths_must_stay_inside_the_system_folder() {
    // An empty path makes the game smaller than any valid game, so the
    // game count check refuses it first.
    let err = decode(&with_payload(&one_game("nes", "a", ""))).expect_err("empty path");
    assert!(
        matches!(
            err,
            CacheError::Malformed {
                what: "game count",
                ..
            }
        ),
        "{err}"
    );
    let err = decode(&with_payload(&one_game("nes", "abcd", ""))).expect_err("empty path");
    assert!(
        matches!(
            err,
            CacheError::Malformed {
                what: "game path",
                ..
            }
        ),
        "{err}"
    );
    for path in [
        "/nes/a.nes",
        "/etc/passwd",
        "nes",
        "nes/",
        "nes//a.nes",
        "nes/../a.nes",
        "nes/sub/../../../a.nes",
        "nes/./a.nes",
        "../nes/a.nes",
        "snes/a.nes",
        "nes/a\0.nes",
    ] {
        let err = decode(&with_payload(&one_game("nes", "a", path))).expect_err(path);
        assert!(
            matches!(
                err,
                CacheError::Malformed {
                    what: "game path",
                    ..
                }
            ),
            "{path}: {err}"
        );
    }
    for (system, path) in [
        ("nes", "nes/a.nes"),
        ("nes", "NES/sub/a..b.nes"),
        ("megadrive", "genesis/a.md"),
    ] {
        assert!(
            decode(&with_payload(&one_game(system, "a", path))).is_ok(),
            "{path}"
        );
    }
}

#[test]
fn empty_game_name_is_rejected() {
    let err = decode(&with_payload(&one_game("nes", "", "nes/.nes"))).expect_err("no name");
    assert!(
        matches!(
            err,
            CacheError::Malformed {
                what: "game name",
                ..
            }
        ),
        "{err}"
    );
}

#[test]
fn over_long_string_is_rejected() {
    let long = vec![b'a'; MAX_TEXT_BYTES + 1];
    let err = decode(&with_payload(&systems_payload(&[&long]))).expect_err("a long id");
    assert!(
        matches!(
            err,
            CacheError::Malformed {
                what: "system id",
                ..
            }
        ),
        "{err}"
    );
}

#[test]
fn foreign_file_is_rejected() {
    let err = decode(b"PK\x03\x04 this is a zip file, not a cache").expect_err("foreign");
    assert!(matches!(err, CacheError::BadMagic));
}

#[test]
fn write_then_load_gives_the_index() {
    let dir = TestDir::new("cache");
    let path = dir.path().join("library.cache");
    write(&path, &sample()).expect("the cache is written");
    assert!(!dir.path().join("library.cache.tmp").exists());
    match load(&path, &catalog()) {
        CacheLoad::Loaded(index) => assert_eq!(index, sample()),
        other => panic!("expected a loaded cache, got {other:?}"),
    }
    write(&path, &Index::new()).expect("the cache is replaced");
    assert!(matches!(load(&path, &catalog()), CacheLoad::Loaded(i) if i.total() == 0));
}

#[test]
fn missing_file_is_missing() {
    let dir = TestDir::new("cache-missing");
    assert!(matches!(
        load(&dir.path().join("library.cache"), &catalog()),
        CacheLoad::Missing
    ));
}

#[test]
fn damaged_file_is_discarded_and_deleted() {
    let dir = TestDir::new("cache-damaged");
    let path = dir.path().join("library.cache");
    let mut bytes = encode(&sample());
    let last = bytes.len() - 1;
    bytes[last] ^= 0x55;
    fs::write(&path, &bytes).expect("write the damaged cache");
    assert!(matches!(
        load(&path, &catalog()),
        CacheLoad::Discarded(CacheError::Checksum { .. })
    ));
    assert!(!path.exists());
    assert!(matches!(load(&path, &catalog()), CacheLoad::Missing));
}

#[test]
fn old_version_is_discarded_and_deleted() {
    let dir = TestDir::new("cache-version");
    let path = dir.path().join("library.cache");
    let mut bytes = encode(&sample());
    bytes[8..12].copy_from_slice(&0u32.to_le_bytes());
    fs::write(&path, &bytes).expect("write the old cache");
    assert!(matches!(
        load(&path, &catalog()),
        CacheLoad::Discarded(CacheError::Version { found: 0, .. })
    ));
    assert!(!path.exists());
}

#[test]
fn unknown_system_is_discarded() {
    let dir = TestDir::new("cache-system");
    let path = dir.path().join("library.cache");
    let index = Index::from_systems(vec![SystemGames::new(
        "psx".into(),
        vec![Game::new(
            "Ridge Racer".into(),
            "psx/Ridge Racer.cue".into(),
        )],
    )]);
    write(&path, &index).expect("the cache is written");
    assert!(matches!(
        load(&path, &catalog()),
        CacheLoad::Discarded(CacheError::UnknownSystem { ref system }) if system == "psx"
    ));
    assert!(!path.exists());
}

#[test]
fn write_into_a_missing_folder_fails_and_leaves_nothing() {
    let dir = TestDir::new("cache-nofolder");
    let path = dir.path().join("absent/library.cache");
    assert!(matches!(
        write(&path, &sample()),
        Err(CacheError::Io { .. })
    ));
    assert!(!dir.path().join("absent").exists());
}

#[cfg(unix)]
#[test]
fn write_does_not_follow_a_link_at_the_temporary_path() {
    let dir = TestDir::new("cache-link");
    let target = dir.path().join("precious.nes");
    fs::write(&target, b"ROM").expect("write the target");
    let path = dir.path().join("library.cache");
    std::os::unix::fs::symlink(&target, temp_path(&path)).expect("plant the link");

    write(&path, &sample()).expect("the cache is written");

    assert_eq!(fs::read(&target).expect("read the target"), b"ROM");
    assert!(!temp_path(&path).exists());
    assert!(matches!(load(&path, &catalog()), CacheLoad::Loaded(i) if i == sample()));
}

#[test]
fn index_larger_than_the_limit_is_not_written() {
    let dir = TestDir::new("cache-large");
    let path = dir.path().join("library.cache");
    let long = "x".repeat(MAX_TEXT_BYTES - 16);
    let per_game = 2 * long.len();
    let count = MAX_CACHE_BYTES as usize / per_game + 1;
    let games = (0..count)
        .map(|i| Game::new(format!("{long}{i}"), format!("nes/{long}{i}")))
        .collect();
    let index = Index::from_systems(vec![SystemGames::new("nes".into(), games)]);
    assert!(matches!(
        write(&path, &index),
        Err(CacheError::TooLarge { .. })
    ));
    assert!(!path.exists());
}

#[test]
fn temp_path_is_next_to_the_file() {
    assert_eq!(
        temp_path(Path::new("/state/cache/library.cache")),
        Path::new("/state/cache/library.cache.tmp")
    );
}

#[test]
fn ten_thousand_games_round_trip() {
    let games = (0..10_000)
        .map(|i| {
            let name = format!("Game number {i:05} (USA)");
            Game::new(name.clone(), format!("snes/{name}.sfc"))
        })
        .collect();
    let index = Index::from_systems(vec![SystemGames::new("snes".into(), games)]);
    let t = std::time::Instant::now();
    let bytes = encode(&index);
    let encoded = t.elapsed();
    let t = std::time::Instant::now();
    let decoded = decode(&bytes).expect("a valid cache");
    let decoded_in = t.elapsed();
    eprintln!(
        "10,000-game cache: {} bytes, encode {encoded:?}, decode {decoded_in:?}",
        bytes.len()
    );
    assert_eq!(decoded, index);
}
