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
fn builtin_catalog_has_the_mvp_systems() {
    let catalog = Catalog::builtin().expect("the built-in definitions are valid");
    let folders: Vec<&str> = catalog
        .systems()
        .iter()
        .map(|s| s.folder.as_str())
        .collect();
    assert_eq!(folders, ["nes", "snes", "megadrive", "arcade", "neogeo"]);
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
fn builtin_arcade_systems_run_fbneo_from_zip_sets() {
    let catalog = Catalog::builtin().expect("the built-in definitions are valid");
    for folder in ["arcade", "fbneo", "fba", "FBA", "neogeo"] {
        let system = catalog
            .system_for_folder(folder)
            .and_then(|i| catalog.systems().get(i))
            .unwrap_or_else(|| panic!("{folder} is a system folder"));
        assert_eq!(system.extensions, ["zip"], "{folder}");
        assert!(system.always_progressive, "{folder}");
        let core = catalog.core_for(&system.folder).expect("the core");
        assert_eq!(core.id, "fbneo");
        assert_eq!(core.library, "fbneo_libretro.so");
    }
    assert_eq!(
        catalog.system_for_folder("fbneo"),
        catalog.system_for_folder("arcade")
    );
    assert_ne!(
        catalog.system_for_folder("neogeo"),
        catalog.system_for_folder("arcade")
    );
    for folder in ["mame", "mame2003", "mame2003-plus", "mame-libretro"] {
        assert_eq!(catalog.system_for_folder(folder), None, "{folder}");
    }
    for console in ["nes", "snes", "megadrive"] {
        assert!(!catalog.system(console).unwrap().always_progressive);
    }
    assert!(catalog.is_bios_set("arcade", "neogeo"));
    assert!(catalog.is_bios_set("neogeo", "neogeo"));
    assert!(!catalog.is_bios_set("nes", "neogeo"));
}

#[test]
fn builtin_systems_accept_zip_but_not_7z() {
    let catalog = Catalog::builtin().expect("the built-in definitions are valid");
    for system in catalog.systems() {
        assert!(system.accepts_extension("zip"), "{}", system.folder);
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
fn always_progressive_defaults_to_false() {
    let c = catalog(&ok_systems(), &ok_cores()).unwrap();
    assert!(!c.system("nes").unwrap().always_progressive);
    let text = format!("{NES}always_progressive = true\n");
    let c = catalog(&[src("nes", &text)], &[src("nescore", CORE_NES)]).unwrap();
    assert!(c.system("nes").unwrap().always_progressive);
    let text = format!("{NES}always_progressive = false\n");
    let c = catalog(&[src("nes", &text)], &[src("nescore", CORE_NES)]).unwrap();
    assert!(!c.system("nes").unwrap().always_progressive);
}

#[test]
fn always_progressive_must_be_a_boolean() {
    let text = format!("{NES}always_progressive = \"yes\"\n");
    let err = catalog(&[src("nes", &text)], &[src("nescore", CORE_NES)]).unwrap_err();
    assert!(matches!(err, CatalogError::Parse { .. }), "{err:?}");
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

#[test]
fn core_options_are_read_sorted_by_key() {
    let text = format!("{CORE_NES}\n[options]\nzeta = \"1\"\nalpha = \"top/bottom\"\n");
    let catalog = catalog(
        &ok_systems(),
        &[src("nescore", &text), src("mdcore", CORE_MD)],
    )
    .expect("valid options");
    let core = catalog.core_for("nes").expect("the core");
    assert_eq!(
        core.options,
        [
            ("alpha".to_owned(), "top/bottom".to_owned()),
            ("zeta".to_owned(), "1".to_owned())
        ]
    );
    let md = catalog.core_for("megadrive").expect("the core");
    assert!(md.options.is_empty(), "no table gives no options");
}

#[test]
fn core_option_keys_may_hold_a_hyphen() {
    let text = format!("{CORE_NES}\n[options]\nnes-diagnostic-input = \"Start + L + R\"\n");
    let catalog = catalog(
        &ok_systems(),
        &[src("nescore", &text), src("mdcore", CORE_MD)],
    )
    .expect("a key with `-` is valid");
    let core = catalog.core_for("nes").expect("the core");
    assert_eq!(
        core.options,
        [(
            "nes-diagnostic-input".to_owned(),
            "Start + L + R".to_owned()
        )]
    );
}

#[test]
fn the_internal_resolution_of_a_core_is_read_sorted_by_scale() {
    let table = "[internal_resolution]\noption = \"gl_res\"\nnative_lines = 240\nvalues = { 4 = \"4x\", 1 = \"1x\", 2 = \"2x\" }\n";
    let text = format!("{CORE_NES}\n{table}");
    let catalog = catalog(
        &ok_systems(),
        &[src("nescore", &text), src("mdcore", CORE_MD)],
    )
    .expect("a valid internal resolution");
    let core = catalog.core_for("nes").expect("the core");
    assert_eq!(
        core.internal_resolution,
        Some(InternalResolution {
            option: "gl_res".to_owned(),
            native_lines: 240,
            values: vec![
                (1, "1x".to_owned()),
                (2, "2x".to_owned()),
                (4, "4x".to_owned())
            ],
        })
    );
    let md = catalog.core_for("megadrive").expect("the core");
    assert_eq!(md.internal_resolution, None, "no table gives none");
}

#[test]
fn a_bad_internal_resolution_is_rejected() {
    for table in [
        "option = \"a b\"\nnative_lines = 240\nvalues = { 1 = \"1x\" }",
        "option = \"res\"\nnative_lines = 0\nvalues = { 1 = \"1x\" }",
        "option = \"res\"\nnative_lines = 1025\nvalues = { 1 = \"1x\" }",
        "option = \"res\"\nnative_lines = 240\nvalues = { 2 = \"2x\" }",
        "option = \"res\"\nnative_lines = 240\nvalues = {}",
        "option = \"res\"\nnative_lines = 240\nvalues = { 0 = \"0x\", 1 = \"1x\" }",
        "option = \"res\"\nnative_lines = 240\nvalues = { 17 = \"17x\", 1 = \"1x\" }",
        "option = \"res\"\nnative_lines = 240\nvalues = { x = \"2x\", 1 = \"1x\" }",
        "option = \"res\"\nnative_lines = 240\nvalues = { 1 = \"a|b\" }",
        "option = \"res\"\nnative_lines = 240\nvalues = { 1 = \"1x\" }\nextra = 1",
        "option = \"res\"\nvalues = { 1 = \"1x\" }",
    ] {
        let text = format!("{CORE_NES}\n[internal_resolution]\n{table}\n");
        let result = catalog(
            &ok_systems(),
            &[src("nescore", &text), src("mdcore", CORE_MD)],
        );
        assert!(result.is_err(), "{table} must be refused");
    }
}

#[test]
fn bad_core_options_are_rejected() {
    for table in [
        "\"bad key\" = \"1\"",
        "\"a=b\" = \"1\"",
        "\"a;b\" = \"1\"",
        "\"a.b\" = \"1\"",
        "\"\" = \"1\"",
        "key = \"\"",
        "key = \"a|b\"",
        "key = \"a;b\"",
        "key = 1",
    ] {
        let text = format!("{CORE_NES}\n[options]\n{table}\n");
        let result = catalog(
            &ok_systems(),
            &[src("nescore", &text), src("mdcore", CORE_MD)],
        );
        assert!(
            matches!(
                result,
                Err(CatalogError::BadName { .. } | CatalogError::Parse { .. })
            ),
            "{table}: {result:?}"
        );
    }
}

/// The `option` lines of a core trace: each key with the values the core
/// announces.
fn announced(trace: &str) -> Vec<(String, Vec<String>)> {
    trace
        .lines()
        .filter_map(|line| line.strip_prefix("option "))
        .filter_map(|rest| {
            let (key, rest) = rest.split_once(" = ")?;
            let values = rest.split_once('[')?.1.split_once(']')?.0;
            Some((
                key.to_owned(),
                values.split('|').map(str::to_owned).collect(),
            ))
        })
        .collect()
}

/// The `name` of the `system` line of a core trace: the core's
/// `library_name`.
fn library_name(trace: &str) -> Option<&str> {
    trace
        .lines()
        .find_map(|line| line.strip_prefix("system name=\""))
        .and_then(|rest| rest.split_once('"'))
        .map(|(name, _)| name)
}

#[test]
fn builtin_core_options_are_announced_by_the_cores() {
    let traces = [
        (
            "fceumm",
            include_str!("../../../../tests/traces/fceumm.trace"),
        ),
        (
            "snes9x",
            include_str!("../../../../tests/traces/snes9x.trace"),
        ),
        (
            "genesis_plus_gx",
            include_str!("../../../../tests/traces/genesis_plus_gx.trace"),
        ),
        (
            "fbneo",
            include_str!("../../../../tests/traces/fbneo.trace"),
        ),
    ];
    let catalog = Catalog::builtin().expect("the built-in definitions are valid");
    for core in catalog.cores() {
        assert!(
            traces.iter().any(|(id, _)| *id == core.id),
            "{} has no core trace to check its options against",
            core.id
        );
    }
    for (id, trace) in traces {
        let core = catalog
            .cores()
            .iter()
            .find(|c| c.id == id)
            .expect("the core");
        assert!(!core.options.is_empty(), "{id} has default options");
        assert_eq!(library_name(trace), Some(core.name.as_str()), "{id}");
        let announced = announced(trace);
        for (key, value) in &core.options {
            let values = announced
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v)
                .unwrap_or_else(|| panic!("{id} does not announce {key}"));
            assert!(values.contains(value), "{id}: {key} has no value {value}");
        }
    }
}

#[test]
fn bios_sets_belong_to_the_core_of_the_system() {
    let arcade = "format = 1\nfolder = \"arcade\"\naliases = []\nname = \"Arcade\"\n\
                  extensions = [\"zip\"]\n";
    let fbneo = "format = 1\nid = \"fbneo\"\nname = \"FB\"\nlibrary = \"fbneo_libretro.so\"\n\
                 systems = [\"arcade\"]\nlicence = \"MIT\"\n";
    let c = catalog(
        &[src("nes", NES), src("arcade", arcade)],
        &[src("nescore", CORE_NES), src("fbneo", fbneo)],
    )
    .unwrap();
    assert!(c.is_bios_set("arcade", "neogeo"));
    assert!(c.is_bios_set("arcade", "NEOGEO"));
    assert!(c.is_bios_set("arcade", "pgm"));
    assert!(!c.is_bios_set("arcade", "mslug"));
    assert!(!c.is_bios_set("arcade", "neogeo2"));
    assert!(!c.is_bios_set("nes", "neogeo"), "another core");
    assert!(!c.is_bios_set("psx", "neogeo"), "an unknown system");
}
