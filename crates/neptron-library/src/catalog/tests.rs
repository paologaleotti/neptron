use super::*;

const NES: &str = r#"
format = 1
folder = "nes"
aliases = []
name = "NES"
extensions = ["nes"]
"#;

const MD: &str = r#"
format = 1
folder = "megadrive"
aliases = ["genesis"]
name = "Mega Drive"
extensions = ["md", "bin"]
"#;

const CORE_NES: &str = r#"
format = 1
id = "nescore"
name = "Nes Core"
library = "nescore_libretro.so"
systems = ["nes"]
licence = "MIT"
"#;

const CORE_MD: &str = r#"
format = 1
id = "mdcore"
name = "MD Core"
library = "mdcore_libretro.so"
systems = ["megadrive"]
licence = "MIT"
"#;

fn src<'a>(name: &'a str, text: &'a str) -> Source<'a> {
    Source { name, text }
}

fn catalog(systems: &[Source<'_>], cores: &[Source<'_>]) -> Result<Catalog, CatalogError> {
    Catalog::from_sources(systems, cores)
}

fn ok_systems() -> [Source<'static>; 2] {
    [src("nes", NES), src("megadrive", MD)]
}

fn ok_cores() -> [Source<'static>; 2] {
    [src("nescore", CORE_NES), src("mdcore", CORE_MD)]
}

#[test]
fn builtin_catalog_has_the_three_mvp_systems() {
    let catalog = Catalog::builtin().expect("the built-in definitions are valid");
    let folders: Vec<&str> = catalog
        .systems()
        .iter()
        .map(|s| s.folder.as_str())
        .collect();
    assert_eq!(folders, ["nes", "snes", "megadrive"]);
    assert_eq!(catalog.system_for_folder("genesis"), Some(2));
    assert_eq!(
        catalog.core_for("nes").map(|c| c.id.as_str()),
        Some("fceumm")
    );
    assert_eq!(
        catalog.core_for("snes").map(|c| c.id.as_str()),
        Some("snes9x")
    );
    assert_eq!(
        catalog.core_for("megadrive").map(|c| c.library.as_str()),
        Some("genesis_plus_gx_libretro.so")
    );
    assert!(catalog.cores().iter().all(|c| !c.licence.is_empty()));
}

#[test]
fn builtin_files_have_no_archive_extensions() {
    let catalog = Catalog::builtin().expect("the built-in definitions are valid");
    for system in catalog.systems() {
        assert!(!system.accepts_extension("zip"), "{}", system.folder);
        assert!(!system.accepts_extension("7z"), "{}", system.folder);
    }
}

#[test]
fn folder_and_extension_match_ignore_ascii_case() {
    let c = catalog(&ok_systems(), &ok_cores()).unwrap();
    assert_eq!(c.system_for_folder("NES"), Some(0));
    assert_eq!(c.system_for_folder("Genesis"), Some(1));
    assert_eq!(c.system_for_folder("snes"), None);
    let md = c.system("megadrive").unwrap();
    assert!(md.accepts_extension("MD"));
    assert!(md.accepts_extension("bin"));
    assert!(!md.accepts_extension("smd"));
    assert!(!md.accepts_extension(""));
}

#[test]
fn bad_toml_is_a_parse_error() {
    let err = catalog(&[src("nes", "format = ")], &[]).unwrap_err();
    assert!(matches!(err, CatalogError::Parse { ref source_name, .. } if source_name == "nes"));
}

#[test]
fn unknown_key_is_rejected() {
    let text = format!("{NES}\nbios = []\n");
    let err = catalog(&[src("nes", &text)], &[]).unwrap_err();
    assert!(matches!(err, CatalogError::Parse { .. }), "{err:?}");
}

#[test]
fn missing_key_is_rejected() {
    let text = NES.replace("aliases = []", "");
    let err = catalog(&[src("nes", &text)], &[]).unwrap_err();
    assert!(matches!(err, CatalogError::Parse { .. }), "{err:?}");
}

#[test]
fn other_format_is_rejected() {
    let text = NES.replace("format = 1", "format = 2");
    let err = catalog(&[src("nes", &text)], &[]).unwrap_err();
    assert_eq!(
        err,
        CatalogError::Format {
            source_name: "nes".into(),
            found: 2,
            expected: 1
        }
    );
}

#[test]
fn folder_must_equal_file_name() {
    let err = catalog(&[src("famicom", NES)], &[]).unwrap_err();
    assert!(matches!(err, CatalogError::NameMismatch { ref id, .. } if id == "nes"));
}

#[test]
fn bad_names_are_rejected() {
    for (field, value) in [
        ("folder = \"nes\"", "folder = \"NES\""),
        ("folder = \"nes\"", "folder = \".nes\""),
        ("folder = \"nes\"", "folder = \"a/b\""),
        ("aliases = []", "aliases = [\"\"]"),
        ("extensions = [\"nes\"]", "extensions = [\".nes\"]"),
        ("extensions = [\"nes\"]", "extensions = [\"NES\"]"),
        ("extensions = [\"nes\"]", "extensions = [\"\"]"),
    ] {
        let text = NES.replace(field, value);
        let name = if value.starts_with("folder") {
            "x"
        } else {
            "nes"
        };
        let err = catalog(&[src(name, &text)], &[]).unwrap_err();
        assert!(
            matches!(err, CatalogError::BadName { .. }),
            "{value}: {err:?}"
        );
    }
}

#[test]
fn empty_fields_are_rejected() {
    let text = NES.replace("name = \"NES\"", "name = \" \"");
    let err = catalog(&[src("nes", &text)], &[]).unwrap_err();
    assert!(matches!(err, CatalogError::Empty { field: "name", .. }));
    let text = NES.replace("extensions = [\"nes\"]", "extensions = []");
    let err = catalog(&[src("nes", &text)], &[]).unwrap_err();
    assert!(matches!(
        err,
        CatalogError::Empty {
            field: "extensions",
            ..
        }
    ));
    let text = CORE_NES.replace("systems = [\"nes\"]", "systems = []");
    let err = catalog(&[src("nes", NES)], &[src("nescore", &text)]).unwrap_err();
    assert!(matches!(
        err,
        CatalogError::Empty {
            field: "systems",
            ..
        }
    ));
    let text = CORE_NES.replace("licence = \"MIT\"", "licence = \"\"");
    let err = catalog(&[src("nes", NES)], &[src("nescore", &text)]).unwrap_err();
    assert!(matches!(
        err,
        CatalogError::Empty {
            field: "licence",
            ..
        }
    ));
}

#[test]
fn library_must_be_a_file_name() {
    let text = CORE_NES.replace("nescore_libretro.so", "../x.so");
    let err = catalog(&[src("nes", NES)], &[src("nescore", &text)]).unwrap_err();
    assert!(matches!(err, CatalogError::BadName { .. }), "{err:?}");
}

#[test]
fn alias_that_equals_another_folder_is_rejected() {
    let md = MD.replace("\"genesis\"", "\"nes\"");
    let err = catalog(&[src("nes", NES), src("megadrive", &md)], &ok_cores()).unwrap_err();
    assert_eq!(
        err,
        CatalogError::DuplicateFolder {
            folder: "nes".into()
        }
    );
}

#[test]
fn duplicate_core_is_rejected() {
    let err = catalog(
        &ok_systems(),
        &[src("nescore", CORE_NES), src("nescore", CORE_NES)],
    )
    .unwrap_err();
    assert_eq!(
        err,
        CatalogError::DuplicateCore {
            id: "nescore".into()
        }
    );
}

#[test]
fn core_with_unknown_system_is_rejected() {
    let text = CORE_NES.replace("systems = [\"nes\"]", "systems = [\"psx\"]");
    let err = catalog(&ok_systems(), &[src("nescore", &text)]).unwrap_err();
    assert_eq!(
        err,
        CatalogError::UnknownSystem {
            core: "nescore".into(),
            system: "psx".into()
        }
    );
}

#[test]
fn every_system_needs_exactly_one_core() {
    let err = catalog(&ok_systems(), &[src("nescore", CORE_NES)]).unwrap_err();
    assert_eq!(
        err,
        CatalogError::CoreCount {
            system: "megadrive".into(),
            cores: 0
        }
    );
    let both = CORE_MD.replace(
        "systems = [\"megadrive\"]",
        "systems = [\"megadrive\", \"nes\"]",
    );
    let err = catalog(
        &ok_systems(),
        &[src("nescore", CORE_NES), src("mdcore", &both)],
    )
    .unwrap_err();
    assert_eq!(
        err,
        CatalogError::CoreCount {
            system: "nes".into(),
            cores: 2
        }
    );
}
