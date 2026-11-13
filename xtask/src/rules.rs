//! Checks of the crate boundaries: the dependency rules and the `unsafe`
//! allow-list of the monorepo layout, and the headless feature set.
//!
//! The checks run on a [`Workspace`] model built from `cargo metadata`, so
//! that the tests can build small models by hand.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::load::LoadPolicy;
use crate::repo::{self, Repo, RepoError};

/// Crates that may contain `unsafe`, in addition to every `-sys` crate. Every
/// other workspace crate carries `#![forbid(unsafe_code)]` in every target root.
pub const UNSAFE_ALLOWED: &[&str] = &[
    "neptron-libretro",
    "neptron-kms",
    "neptron-gfx",
    "neptron-audio",
    "neptron-input",
    "neptron-testcore",
];

/// Crates that bring SDL3, EGL, GL, KMS or evdev code. The headless build
/// (default features) must not contain any of them.
pub const HARDWARE_CRATES: &[&str] = &[
    "sdl3",
    "sdl3-sys",
    "khronos-egl",
    "glutin",
    "glow",
    "drm",
    "drm-ffi",
    "drm-sys",
    "gbm",
    "gbm-sys",
    "evdev",
    "librashader",
];

/// Feature names that turn on a hardware backend. The headless build must not
/// turn on one of them in any workspace crate.
pub const HARDWARE_FEATURES: &[&str] = &["kms", "gles", "sdl3", "evdev", "window"];

/// The frontend has one frame loop on the main thread and no async runtime.
pub const ASYNC_RUNTIMES: &[&str] = &["tokio", "async-std", "smol", "async-executor"];

/// An error while the rule checker reads the workspace.
#[derive(Debug, thiserror::Error)]
pub enum RulesError {
    /// `cargo metadata` failed.
    #[error(transparent)]
    Metadata(#[from] RepoError),
    /// The output of `cargo metadata` is not JSON.
    #[error("cannot parse the output of cargo metadata: {0}")]
    Parse(#[from] serde_json::Error),
    /// The output of `cargo metadata` has no expected field.
    #[error("cargo metadata has no field {0:?}")]
    MissingField(&'static str),
}

/// Kind of a dependency edge, as Cargo reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepKind {
    /// `[dependencies]`
    Normal,
    /// `[dev-dependencies]`
    Dev,
    /// `[build-dependencies]`
    Build,
}

/// One declared dependency of a workspace crate.
#[derive(Debug, Clone)]
pub struct Dep {
    /// Package name of the dependency.
    pub name: String,
    /// Section it is declared in.
    pub kind: DepKind,
}

/// One workspace crate.
#[derive(Debug, Clone)]
pub struct Package {
    /// Package name.
    pub name: String,
    /// Directory that holds `Cargo.toml`.
    pub dir: PathBuf,
    /// Declared dependencies.
    pub deps: Vec<Dep>,
    /// Source roots of all targets (lib, bins, tests, build script, ...).
    pub target_roots: Vec<PathBuf>,
}

/// One package in the resolved graph.
#[derive(Debug, Clone, Default)]
pub struct Node {
    /// Package name.
    pub name: String,
    /// Package ids of its dependencies.
    pub deps: Vec<String>,
    /// Features that the build turns on for this package.
    pub features: Vec<String>,
}

/// The workspace: its crates and the resolved graph for default features.
#[derive(Debug, Clone, Default)]
pub struct Workspace {
    /// Workspace members.
    pub members: Vec<Package>,
    /// Resolved graph of the default (headless) build, by package id.
    pub resolve: BTreeMap<String, Node>,
    /// Package ids of the workspace members, by name.
    pub member_ids: BTreeMap<String, String>,
}

/// True when a crate may contain `unsafe`.
pub fn unsafe_allowed(name: &str) -> bool {
    name.ends_with("-sys") || UNSAFE_ALLOWED.contains(&name)
}

/// Reads the workspace, prints every violation and returns how many there are.
pub fn run(repo: &Repo, policy: &LoadPolicy) -> Result<usize, RulesError> {
    let ws = load(&repo.root, policy)?;
    let violations = check(&ws, &|p| std::fs::read_to_string(p));
    for v in &violations {
        eprintln!("rule violation: {v}");
    }
    if violations.is_empty() {
        eprintln!("rules: {} crates, no violations", ws.members.len());
    }
    Ok(violations.len())
}

/// Reads the workspace model with `cargo metadata` in `root`.
pub fn load(root: &Path, policy: &LoadPolicy) -> Result<Workspace, RulesError> {
    let json = repo::output(
        policy
            .cargo(&["metadata", "--format-version", "1"])
            .current_dir(root),
    )?;
    from_metadata(&serde_json::from_str(&json)?)
}

/// Runs every check and returns the violations, one line each.
pub fn check(ws: &Workspace, read: &dyn Fn(&Path) -> io::Result<String>) -> Vec<String> {
    let mut v = Vec::new();
    check_dependency_rules(ws, &mut v);
    check_unsafe(ws, read, &mut v);
    check_headless(ws, &mut v);
    check_async(ws, &mut v);
    check_readmes(ws, &mut v);
    v
}

/// True when the crate root `source` carries `#![forbid(unsafe_code)]` among
/// the inner attributes at its head. An attribute anywhere else (in a
/// module, a comment or a string) does not count.
pub fn forbids_unsafe(source: &str) -> bool {
    inner_attributes(source).iter().any(|attr| {
        let flat: String = attr.chars().filter(|c| !c.is_whitespace()).collect();
        flat.strip_prefix("forbid(")
            .and_then(|rest| rest.strip_suffix(')'))
            .is_some_and(|lints| lints.split(',').any(|l| l == "unsafe_code"))
    })
}

/// Builds the model from the JSON output of `cargo metadata --format-version 1`.
pub fn from_metadata(meta: &Value) -> Result<Workspace, RulesError> {
    let member_list: BTreeSet<&str> = meta["workspace_members"]
        .as_array()
        .ok_or(RulesError::MissingField("workspace_members"))?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let packages = meta["packages"]
        .as_array()
        .ok_or(RulesError::MissingField("packages"))?;
    let mut ws = Workspace::default();
    let mut names = BTreeMap::new();
    for pkg in packages {
        let id = str_field(pkg, "id")?;
        let name = str_field(pkg, "name")?;
        names.insert(id.to_owned(), name.to_owned());
        if member_list.contains(id) {
            ws.member_ids.insert(name.to_owned(), id.to_owned());
            ws.members.push(parse_package(pkg)?);
        }
    }
    for node in meta["resolve"]["nodes"].as_array().into_iter().flatten() {
        let id = str_field(node, "id")?;
        ws.resolve.insert(
            id.to_owned(),
            Node {
                name: names.get(id).cloned().unwrap_or_default(),
                deps: strings(&node["deps"], Some("pkg")),
                features: strings(&node["features"], None),
            },
        );
    }
    Ok(ws)
}

const NORMAL: &[DepKind] = &[DepKind::Normal];
const NORMAL_BUILD: &[DepKind] = &[DepKind::Normal, DepKind::Build];
const ALL: &[DepKind] = &[DepKind::Normal, DepKind::Build, DepKind::Dev];

/// Crates that may use async runtimes because they never ship.
const ASYNC_EXEMPT: &[&str] = &["xtask"];

/// A crate may depend only on the listed crates.
struct OnlyDeps {
    krate: &'static str,
    kinds: &'static [DepKind],
    allowed: &'static [&'static str],
    why: &'static str,
}

const ONLY_DEPS: &[OnlyDeps] = &[
    OnlyDeps {
        krate: "neptron-safety",
        kinds: NORMAL,
        allowed: &["neptron-types"],
        why: "the validator depends only on the shared types crate",
    },
    OnlyDeps {
        krate: "neptron-modeline",
        kinds: NORMAL,
        allowed: &["neptron-types", "neptron-switchres-sys"],
        why: "the solver depends only on the shared types and the Switchres bindings",
    },
    OnlyDeps {
        krate: "kms-spike",
        kinds: NORMAL,
        allowed: &["drm", "neptron-types", "neptron-safety"],
        why: "the KMS spike depends on drm, neptron-types and neptron-safety only",
    },
];

/// A crate may depend on no other workspace crate.
struct NoWorkspaceDeps {
    krate: &'static str,
    kinds: &'static [DepKind],
    why: &'static str,
}

const NO_WORKSPACE_DEPS: &[NoWorkspaceDeps] = &[
    NoWorkspaceDeps {
        krate: "neptron-types",
        kinds: NORMAL_BUILD,
        why: "the shared types sit at the bottom of the graph",
    },
    NoWorkspaceDeps {
        krate: "neptron-ui",
        kinds: ALL,
        why: "the UI is sealed: snapshot in, scene and intents out",
    },
    NoWorkspaceDeps {
        krate: "neptron-libretro-sys",
        kinds: ALL,
        why: "raw bindings depend on nothing of ours",
    },
    NoWorkspaceDeps {
        krate: "neptron-switchres-sys",
        kinds: NORMAL_BUILD,
        why: "the Switchres crate holds the C++ build and the raw calls, nothing else",
    },
];

/// A crate must not depend on another crate.
struct ForbiddenEdge {
    from: &'static str,
    to: &'static str,
    kinds: &'static [DepKind],
    why: &'static str,
}

const FORBIDDEN_EDGES: &[ForbiddenEdge] = &[ForbiddenEdge {
    from: "neptron-video",
    to: "neptron-kms",
    kinds: ALL,
    why: "the engine uses the Display trait, never the KMS backend",
}];

/// Only the listed crates may depend on a crate.
struct OnlyDependents {
    krate: &'static str,
    kinds: &'static [DepKind],
    allowed: &'static [&'static str],
    /// Apply the rule to library crates under `crates/` only.
    libraries_only: bool,
    why: &'static str,
}

const ONLY_DEPENDENTS: &[OnlyDependents] = &[
    OnlyDependents {
        krate: "neptron-ui",
        kinds: ALL,
        allowed: &["neptron", "xtask"],
        libraries_only: false,
        why: "only the app binary (and the screenshot tooling) depends on the UI",
    },
    OnlyDependents {
        krate: "neptron-modeline",
        kinds: NORMAL_BUILD,
        allowed: &["neptron-video"],
        libraries_only: true,
        why: "neptron-video is the single crate that joins solver, safety and display",
    },
    OnlyDependents {
        krate: "neptron-switchres-sys",
        kinds: NORMAL_BUILD,
        allowed: &["neptron-modeline"],
        libraries_only: false,
        why: "neptron-modeline is the safe wrapper of the raw Switchres calls",
    },
];

impl Package {
    /// True when the crate is a library under `crates/`.
    fn is_library_crate(&self) -> bool {
        self.dir
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|n| n == "crates")
    }

    fn depends_on(&self, name: &str, kinds: &[DepKind]) -> bool {
        self.deps
            .iter()
            .any(|d| d.name == name && kinds.contains(&d.kind))
    }
}

fn check_dependency_rules(ws: &Workspace, v: &mut Vec<String>) {
    let names: BTreeSet<&str> = ws.members.iter().map(|p| p.name.as_str()).collect();
    let mut deny = |p: &Package, dep: &str, why: &str| {
        v.push(format!("deps: {} must not depend on {dep} ({why})", p.name));
    };
    for p in &ws.members {
        for rule in ONLY_DEPS.iter().filter(|r| r.krate == p.name) {
            for d in p.deps.iter().filter(|d| rule.kinds.contains(&d.kind)) {
                if !rule.allowed.contains(&d.name.as_str()) {
                    deny(p, &d.name, rule.why);
                }
            }
        }
        for rule in NO_WORKSPACE_DEPS.iter().filter(|r| r.krate == p.name) {
            for d in p.deps.iter().filter(|d| rule.kinds.contains(&d.kind)) {
                if names.contains(d.name.as_str()) {
                    deny(p, &d.name, rule.why);
                }
            }
        }
        for rule in FORBIDDEN_EDGES.iter().filter(|r| r.from == p.name) {
            if p.depends_on(rule.to, rule.kinds) {
                deny(p, rule.to, rule.why);
            }
        }
        for rule in ONLY_DEPENDENTS {
            let exempt = p.name == rule.krate
                || rule.allowed.contains(&p.name.as_str())
                || (rule.libraries_only && !p.is_library_crate());
            if !exempt && p.depends_on(rule.krate, rule.kinds) {
                deny(p, rule.krate, rule.why);
            }
        }
    }
}

fn check_unsafe(ws: &Workspace, read: &dyn Fn(&Path) -> io::Result<String>, v: &mut Vec<String>) {
    for p in ws.members.iter().filter(|p| !unsafe_allowed(&p.name)) {
        for root in &p.target_roots {
            match read(root) {
                Ok(text) if forbids_unsafe(&text) => {}
                Ok(_) => v.push(format!(
                    "unsafe: {} is not on the unsafe allow-list, so {} must start with \
                     #![forbid(unsafe_code)] among its inner attributes",
                    p.name,
                    root.display()
                )),
                Err(e) => v.push(format!("unsafe: cannot read {}: {e}", root.display())),
            }
        }
    }
}

/// Uses the features that Cargo resolves for the default build, so a hardware
/// feature is found however it is turned on: a `default` list, a chain of
/// features, or `features = [...]` on a dependency entry.
fn check_headless(ws: &Workspace, v: &mut Vec<String>) {
    for (member, id) in &ws.member_ids {
        let Some(node) = ws.resolve.get(id) else {
            continue;
        };
        for f in node
            .features
            .iter()
            .filter(|f| HARDWARE_FEATURES.contains(&f.as_str()))
        {
            v.push(format!(
                "headless: the default build turns on the hardware feature {f:?} of {member}"
            ));
        }
    }
    let mut seen = BTreeSet::new();
    for node in ws.resolve.values() {
        if HARDWARE_CRATES.contains(&node.name.as_str()) && seen.insert(&node.name) {
            v.push(format!(
                "headless: the default build contains the hardware crate {}; put it behind a feature",
                node.name
            ));
        }
    }
}

fn check_async(ws: &Workspace, v: &mut Vec<String>) {
    for (member, id) in &ws.member_ids {
        if ASYNC_EXEMPT.contains(&member.as_str()) {
            continue;
        }
        let mut seen = BTreeSet::new();
        let mut todo = vec![id.as_str()];
        while let Some(id) = todo.pop() {
            if !seen.insert(id) {
                continue;
            }
            if let Some(node) = ws.resolve.get(id) {
                if ASYNC_RUNTIMES.contains(&node.name.as_str()) {
                    v.push(format!(
                        "deps: {member} pulls in the async runtime {} (no async runtime in the frontend)",
                        node.name
                    ));
                }
                todo.extend(node.deps.iter().map(String::as_str));
            }
        }
    }
}

fn check_readmes(ws: &Workspace, v: &mut Vec<String>) {
    for p in &ws.members {
        if !p.dir.join("README.md").is_file() {
            v.push(format!("readme: {} has no README.md", p.name));
        }
    }
}

/// The inner attributes at the head of a crate root, without `#![` and `]`.
/// The head ends at the first token that is not whitespace, a comment or an
/// inner attribute.
fn inner_attributes(source: &str) -> Vec<String> {
    let src = source.strip_prefix('\u{feff}').unwrap_or(source);
    let mut rest = skip_shebang(src);
    let mut attrs = Vec::new();
    loop {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix("//") {
            rest = after.split_once('\n').map_or("", |(_, tail)| tail);
        } else if rest.starts_with("/*") {
            match skip_block_comment(rest) {
                Some(tail) => rest = tail,
                None => return attrs,
            }
        } else if let Some(after) = rest.strip_prefix("#!") {
            let Some(body) = after.trim_start().strip_prefix('[') else {
                return attrs;
            };
            match bracket_body(body) {
                Some((inner, tail)) => {
                    attrs.push(inner.to_owned());
                    rest = tail;
                }
                None => return attrs,
            }
        } else {
            return attrs;
        }
    }
}

/// A `#!` line that does not start an attribute is a shebang.
fn skip_shebang(src: &str) -> &str {
    match src.strip_prefix("#!") {
        Some(after) if !after.trim_start().starts_with('[') => {
            after.split_once('\n').map_or("", |(_, tail)| tail)
        }
        _ => src,
    }
}

/// Skips a block comment that starts `src`; block comments nest in Rust.
fn skip_block_comment(src: &str) -> Option<&str> {
    let mut depth = 0usize;
    let mut i = 0;
    let b = src.as_bytes();
    while i + 1 < b.len() {
        match (b[i], b[i + 1]) {
            (b'/', b'*') => {
                depth += 1;
                i += 2;
            }
            (b'*', b'/') => {
                depth -= 1;
                i += 2;
                if depth == 0 {
                    return Some(&src[i..]);
                }
            }
            _ => i += 1,
        }
    }
    None
}

/// Splits `src` (the text after an opening `[`) at its matching `]`, and skips
/// brackets inside string literals.
fn bracket_body(src: &str) -> Option<(&str, &str)> {
    let b = src.as_bytes();
    let mut depth = 1usize;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' => i = skip_string(b, i + 1)?,
            b'r' if matches!(b.get(i + 1), Some(b'#' | b'"')) => i = skip_raw_string(b, i + 1)?,
            b'[' => {
                depth += 1;
                i += 1;
            }
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&src[..i], &src[i + 1..]));
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

/// Returns the index after the closing quote of a string whose body starts at `i`.
fn skip_string(b: &[u8], mut i: usize) -> Option<usize> {
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => return Some(i + 1),
            _ => i += 1,
        }
    }
    None
}

/// Returns the index after a raw string whose hashes start at `i`.
fn skip_raw_string(b: &[u8], mut i: usize) -> Option<usize> {
    let hashes = b[i..].iter().take_while(|&&c| c == b'#').count();
    i += hashes;
    if b.get(i) != Some(&b'"') {
        return Some(i);
    }
    i += 1;
    while i < b.len() {
        if b[i] == b'"'
            && b[i + 1..]
                .iter()
                .take(hashes)
                .filter(|&&c| c == b'#')
                .count()
                == hashes
        {
            return Some(i + 1 + hashes);
        }
        i += 1;
    }
    None
}

fn parse_package(pkg: &Value) -> Result<Package, RulesError> {
    let manifest = PathBuf::from(str_field(pkg, "manifest_path")?);
    let dir = manifest
        .parent()
        .map(Path::to_path_buf)
        .ok_or(RulesError::MissingField("manifest_path"))?;
    let deps = pkg["dependencies"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|d| {
            let kind = match d["kind"].as_str() {
                None => DepKind::Normal,
                Some("dev") => DepKind::Dev,
                Some("build") => DepKind::Build,
                Some(_) => return None,
            };
            Some(Dep {
                name: d["name"].as_str()?.to_owned(),
                kind,
            })
        })
        .collect();
    let target_roots = strings(&pkg["targets"], Some("src_path"))
        .into_iter()
        .map(PathBuf::from)
        .collect();
    Ok(Package {
        name: str_field(pkg, "name")?.to_owned(),
        dir,
        deps,
        target_roots,
    })
}

/// The strings of a JSON array, or of one field of each object in it.
fn strings(array: &Value, field: Option<&str>) -> Vec<String> {
    array
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| match field {
            Some(f) => item[f].as_str(),
            None => item.as_str(),
        })
        .map(str::to_owned)
        .collect()
}

fn str_field<'a>(v: &'a Value, key: &'static str) -> Result<&'a str, RulesError> {
    v[key].as_str().ok_or(RulesError::MissingField(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORBID: &str = "#![forbid(unsafe_code)]";

    fn pkg(name: &str, dir: &str, deps: &[(&str, DepKind)]) -> Package {
        Package {
            name: name.into(),
            dir: PathBuf::from(dir),
            deps: deps
                .iter()
                .map(|(n, k)| Dep {
                    name: (*n).into(),
                    kind: *k,
                })
                .collect(),
            target_roots: vec![PathBuf::from(format!("{dir}/src/lib.rs"))],
        }
    }

    fn lib(name: &str, deps: &[(&str, DepKind)]) -> Package {
        pkg(name, &format!("/ws/crates/{name}"), deps)
    }

    fn ws(members: Vec<Package>) -> Workspace {
        Workspace {
            members,
            ..Workspace::default()
        }
    }

    fn deps_violations(ws: &Workspace) -> Vec<String> {
        let mut v = Vec::new();
        check_dependency_rules(ws, &mut v);
        v
    }

    fn base() -> Vec<Package> {
        [
            "neptron-types",
            "neptron-safety",
            "neptron-modeline",
            "neptron-switchres-sys",
            "neptron-ui",
            "neptron-video",
            "neptron-kms",
        ]
        .iter()
        .map(|n| lib(n, &[]))
        .collect()
    }

    #[test]
    fn clean_graph_passes() {
        let mut m = base();
        m.push(lib(
            "neptron-video",
            &[
                ("neptron-types", DepKind::Normal),
                ("neptron-safety", DepKind::Normal),
                ("neptron-modeline", DepKind::Normal),
            ],
        ));
        m.push(pkg(
            "neptron",
            "/ws/apps/neptron",
            &[
                ("neptron-ui", DepKind::Normal),
                ("neptron-kms", DepKind::Normal),
            ],
        ));
        assert_eq!(deps_violations(&ws(m)), Vec::<String>::new());
    }

    #[test]
    fn safety_may_depend_on_types_only() {
        let mut m = base();
        m.push(lib(
            "neptron-safety",
            &[
                ("neptron-types", DepKind::Normal),
                ("serde", DepKind::Normal),
                ("proptest", DepKind::Dev),
            ],
        ));
        let v = deps_violations(&ws(m));
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("neptron-safety must not depend on serde"));
    }

    #[test]
    fn modeline_wraps_switchres_sys_only() {
        let mut m = base();
        m.push(lib(
            "neptron-modeline",
            &[
                ("neptron-types", DepKind::Normal),
                ("neptron-switchres-sys", DepKind::Normal),
            ],
        ));
        m.push(lib("neptron-switchres-sys", &[("cc", DepKind::Build)]));
        assert!(deps_violations(&ws(m)).is_empty());
        let mut m = base();
        m.push(lib("neptron-modeline", &[("neptron-kms", DepKind::Normal)]));
        assert_eq!(deps_violations(&ws(m)).len(), 1);
    }

    #[test]
    fn switchres_sys_is_a_leaf_used_by_modeline_only() {
        let mut m = base();
        m.push(lib(
            "neptron-switchres-sys",
            &[("neptron-types", DepKind::Normal)],
        ));
        m.push(lib(
            "neptron-video",
            &[("neptron-switchres-sys", DepKind::Normal)],
        ));
        let v = deps_violations(&ws(m));
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(
            v.iter()
                .any(|l| l.starts_with("deps: neptron-switchres-sys"))
        );
        assert!(v.iter().any(|l| l.starts_with("deps: neptron-video")));
    }

    #[test]
    fn ui_depends_on_no_workspace_crate() {
        let mut m = base();
        m.push(lib(
            "neptron-ui",
            &[
                ("neptron-types", DepKind::Dev),
                ("embedded-graphics", DepKind::Normal),
            ],
        ));
        let v = deps_violations(&ws(m));
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("neptron-ui must not depend on neptron-types"));
    }

    #[test]
    fn only_the_app_depends_on_ui() {
        let mut m = base();
        m.push(lib("neptron-session", &[("neptron-ui", DepKind::Normal)]));
        m.push(pkg(
            "neptronctl",
            "/ws/apps/neptronctl",
            &[("neptron-ui", DepKind::Dev)],
        ));
        m.push(pkg(
            "xtask",
            "/ws/xtask",
            &[("neptron-ui", DepKind::Normal)],
        ));
        let v = deps_violations(&ws(m));
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(v.iter().any(|l| l.contains("neptron-session")));
        assert!(v.iter().any(|l| l.contains("neptronctl")));
    }

    #[test]
    fn video_never_depends_on_kms() {
        let mut m = base();
        m.push(lib("neptron-video", &[("neptron-kms", DepKind::Normal)]));
        let v = deps_violations(&ws(m));
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("neptron-video must not depend on neptron-kms"));
    }

    #[test]
    fn only_video_joins_the_solver_among_libraries() {
        let mut m = base();
        m.push(lib(
            "neptron-session",
            &[("neptron-modeline", DepKind::Normal)],
        ));
        m.push(lib("neptron-kms", &[("neptron-modeline", DepKind::Normal)]));
        m.push(pkg(
            "neptronctl",
            "/ws/apps/neptronctl",
            &[("neptron-modeline", DepKind::Normal)],
        ));
        let v = deps_violations(&ws(m));
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(!v.iter().any(|l| l.contains("neptronctl")));
    }

    #[test]
    fn kms_spike_dependencies_are_closed() {
        let mut m = base();
        m.push(pkg(
            "kms-spike",
            "/ws/tools/kms-spike",
            &[
                ("drm", DepKind::Normal),
                ("neptron-safety", DepKind::Normal),
                ("neptron-modeline", DepKind::Normal),
            ],
        ));
        let v = deps_violations(&ws(m));
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("kms-spike must not depend on neptron-modeline"));
    }

    fn unsafe_violations(m: Vec<Package>, text: &'static str) -> Vec<String> {
        let mut v = Vec::new();
        check_unsafe(&ws(m), &|_| Ok(text.to_owned()), &mut v);
        v
    }

    #[test]
    fn crate_outside_allow_list_needs_forbid() {
        let v = unsafe_violations(vec![lib("neptron-ui", &[])], "//! UI\n");
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("neptron-ui"));
        let ok = unsafe_violations(
            vec![lib("neptron-ui", &[])],
            "//! UI\n\n#![forbid(unsafe_code)]\n",
        );
        assert!(ok.is_empty());
    }

    #[test]
    fn every_target_root_is_checked() {
        let mut p = lib("neptron-ui", &[]);
        p.target_roots
            .push(PathBuf::from("/ws/crates/neptron-ui/tests/all.rs"));
        let mut v = Vec::new();
        let read = |path: &Path| {
            Ok(if path.ends_with("lib.rs") {
                FORBID.to_owned()
            } else {
                String::new()
            })
        };
        check_unsafe(&ws(vec![p]), &read, &mut v);
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("tests/all.rs"));
    }

    #[test]
    fn allow_list_and_sys_crates_need_no_forbid() {
        let mut m: Vec<Package> = UNSAFE_ALLOWED.iter().map(|n| lib(n, &[])).collect();
        m.push(lib("neptron-libretro-sys", &[]));
        m.push(lib("neptron-switchres-sys", &[]));
        assert!(unsafe_violations(m, "").is_empty());
        assert!(unsafe_allowed("neptron-testcore"));
        assert!(!unsafe_allowed("neptron-modeline"));
        assert!(!unsafe_allowed("neptron-sysinfo"));
    }

    #[test]
    fn forbid_at_the_head_counts() {
        assert!(forbids_unsafe(FORBID));
        assert!(forbids_unsafe(
            "//! Doc.\n//!\n//! More.\n\n#![forbid(unsafe_code)]\n"
        ));
        assert!(forbids_unsafe(
            "/* a /* nested */ comment */\n#![deny(missing_docs)]\n#![forbid(unsafe_code)]\n"
        ));
        assert!(forbids_unsafe("#![forbid( missing_docs , unsafe_code )]"));
        assert!(forbids_unsafe(
            "#![doc = \"a ] in a string\"]\n#![forbid(unsafe_code)]"
        ));
        assert!(forbids_unsafe(
            "#!/usr/bin/env run\n#![forbid(unsafe_code)]"
        ));
        assert!(forbids_unsafe("\u{feff}#! [forbid(unsafe_code)]"));
    }

    #[test]
    fn forbid_in_a_comment_does_not_count() {
        assert!(!forbids_unsafe("// #![forbid(unsafe_code)]\n"));
        assert!(!forbids_unsafe("/* \n#![forbid(unsafe_code)]\n*/\n"));
        assert!(!forbids_unsafe(
            "/* unterminated\n#![forbid(unsafe_code)]\n"
        ));
    }

    #[test]
    fn forbid_inside_an_item_does_not_count() {
        assert!(!forbids_unsafe("mod inner {\n#![forbid(unsafe_code)]\n}\n"));
        assert!(!forbids_unsafe(
            "const S: &str = r#\"\n#![forbid(unsafe_code)]\n\"#;\n"
        ));
        assert!(!forbids_unsafe(
            "const S: &str = \"\n#![forbid(unsafe_code)]\n\";\n"
        ));
        assert!(!forbids_unsafe("fn main() {}\n#![forbid(unsafe_code)]\n"));
    }

    #[test]
    fn other_forms_do_not_count() {
        assert!(!forbids_unsafe("#![deny(unsafe_code)]"));
        assert!(!forbids_unsafe("#![cfg_attr(test, forbid(unsafe_code))]"));
        assert!(!forbids_unsafe("#[forbid(unsafe_code)]\nfn f() {}"));
        assert!(!forbids_unsafe("#![forbid(unsafe_code_x)]"));
        assert!(!forbids_unsafe("#![forbid(unsafe_code)"));
    }

    fn headless_violations(ws: &Workspace) -> Vec<String> {
        let mut v = Vec::new();
        check_headless(ws, &mut v);
        v
    }

    fn member_with_features(name: &str, features: &[&str]) -> Workspace {
        let mut w = ws(vec![lib(name, &[])]);
        w.member_ids.insert(name.into(), "id".into());
        w.resolve.insert(
            "id".into(),
            Node {
                name: name.into(),
                deps: vec![],
                features: features.iter().map(|f| (*f).into()).collect(),
            },
        );
        w
    }

    #[test]
    fn hardware_features_stay_off_in_the_default_build() {
        let ok = member_with_features("neptron-ipc", &["default", "control"]);
        assert!(headless_violations(&ok).is_empty());
        let bad = member_with_features("neptron-kms", &["kms"]);
        let v = headless_violations(&bad);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("\"kms\" of neptron-kms"));
    }

    #[test]
    fn hardware_crate_in_default_build_fails() {
        let mut w = ws(vec![]);
        w.resolve.insert(
            "a".into(),
            Node {
                name: "neptron-kms".into(),
                deps: vec!["b".into()],
                features: vec![],
            },
        );
        w.resolve.insert(
            "b".into(),
            Node {
                name: "drm".into(),
                ..Node::default()
            },
        );
        let v = headless_violations(&w);
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("drm"));
    }

    fn node(name: &str, deps: &[&str]) -> Node {
        Node {
            name: name.into(),
            deps: deps.iter().map(|d| (*d).into()).collect(),
            features: vec![],
        }
    }

    #[test]
    fn async_runtime_is_found_through_the_graph() {
        let mut w = ws(vec![]);
        w.member_ids.insert("neptron-ipc".into(), "ipc".into());
        w.member_ids.insert("xtask".into(), "x".into());
        w.resolve
            .insert("ipc".into(), node("neptron-ipc", &["mid"]));
        w.resolve.insert("mid".into(), node("some-crate", &["rt"]));
        w.resolve.insert("rt".into(), node("tokio", &[]));
        w.resolve.insert("x".into(), node("xtask", &["rt"]));
        let mut v = Vec::new();
        check_async(&w, &mut v);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("neptron-ipc pulls in the async runtime tokio"));
    }

    #[test]
    fn metadata_is_parsed() {
        let meta: Value = serde_json::from_str(
            r#"{
              "workspace_members": ["path+file:///ws/crates/neptron-ui#0.1.0"],
              "packages": [
                {"id": "path+file:///ws/crates/neptron-ui#0.1.0", "name": "neptron-ui",
                 "manifest_path": "/ws/crates/neptron-ui/Cargo.toml",
                 "dependencies": [{"name": "serde", "kind": null}, {"name": "cc", "kind": "build"}],
                 "targets": [{"kind": ["lib"], "src_path": "/ws/crates/neptron-ui/src/lib.rs"}]},
                {"id": "registry+https://x#serde@1.0.0", "name": "serde",
                 "manifest_path": "/r/serde/Cargo.toml", "dependencies": [], "targets": []}
              ],
              "resolve": {"nodes": [
                {"id": "path+file:///ws/crates/neptron-ui#0.1.0",
                 "deps": [{"pkg": "registry+https://x#serde@1.0.0"}], "features": ["default"]},
                {"id": "registry+https://x#serde@1.0.0", "deps": [], "features": ["std"]}
              ]}
            }"#,
        )
        .unwrap();
        let w = from_metadata(&meta).unwrap();
        assert_eq!(w.members.len(), 1);
        let p = &w.members[0];
        assert_eq!(p.name, "neptron-ui");
        assert_eq!(p.dir, PathBuf::from("/ws/crates/neptron-ui"));
        assert!(p.is_library_crate());
        assert_eq!(p.deps.len(), 2);
        assert_eq!(p.deps[1].kind, DepKind::Build);
        assert_eq!(p.target_roots.len(), 1);
        let serde = &w.resolve["registry+https://x#serde@1.0.0"];
        assert_eq!(serde.name, "serde");
        assert_eq!(serde.features, vec!["std".to_owned()]);
    }

    #[test]
    fn broken_metadata_is_an_error() {
        let meta: Value = serde_json::from_str(r#"{"packages": []}"#).unwrap();
        assert!(matches!(
            from_metadata(&meta),
            Err(RulesError::MissingField("workspace_members"))
        ));
    }

    #[test]
    fn the_real_workspace_passes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let policy = LoadPolicy::resolve(false, None, None).unwrap();
        let w = load(&root, &policy).unwrap();
        assert!(w.members.len() > 20);
        let v = check(&w, &|p| std::fs::read_to_string(p));
        assert_eq!(v, Vec::<String>::new());
    }

    /// The number of `pub const <name>: <type> = <number>;` in a source
    /// file of the workspace.
    fn const_value(rel: &str, name: &str) -> i64 {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(rel);
        let text = std::fs::read_to_string(&path).unwrap();
        let prefix = format!("pub const {name}:");
        let line = text
            .lines()
            .find(|l| l.trim_start().starts_with(&prefix))
            .unwrap_or_else(|| panic!("{rel} has no {name}"));
        let value = line.split('=').nth(1).unwrap().trim().trim_end_matches(';');
        value.parse().unwrap_or_else(|_| panic!("{rel}: {line}"))
    }

    /// The frontend and the launcher are two binaries, and no crate may
    /// depend on both, so this test reads their sources.
    #[test]
    fn the_frontend_and_the_launcher_agree_on_the_exit_codes() {
        for (frontend, launcher, name) in [
            (
                "apps/neptron/src/relaunch.rs",
                "apps/neptron-launch/src/backoff.rs",
                "RELAUNCH_EXIT_CODE",
            ),
            (
                "apps/neptron/src/display_wait.rs",
                "apps/neptron-launch/src/backoff.rs",
                "NO_DISPLAY_EXIT_CODE",
            ),
            (
                "apps/neptron/src/power.rs",
                "apps/neptron-launch/src/backoff.rs",
                "REBOOT_EXIT_CODE",
            ),
            (
                "apps/neptron/src/power.rs",
                "apps/neptron-launch/src/backoff.rs",
                "POWER_OFF_EXIT_CODE",
            ),
        ] {
            assert_eq!(
                const_value(frontend, name),
                const_value(launcher, name),
                "{name}"
            );
        }
        let codes: Vec<i64> = [
            "RELAUNCH_EXIT_CODE",
            "NO_DISPLAY_EXIT_CODE",
            "REBOOT_EXIT_CODE",
            "POWER_OFF_EXIT_CODE",
        ]
        .iter()
        .map(|name| const_value("apps/neptron-launch/src/backoff.rs", name))
        .collect();
        for (i, code) in codes.iter().enumerate() {
            assert!(
                (3..126).contains(code),
                "{code} is 0, 1, 2 or a code of a shell or a signal"
            );
            assert!(!codes[i + 1..].contains(code), "{code} is used twice");
        }
    }

    /// The string of `pub const <name>: &str = "<text>";` in a source file
    /// of the workspace.
    fn const_text(rel: &str, name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(rel);
        let text = std::fs::read_to_string(&path).unwrap();
        let prefix = format!("pub const {name}: &str = \"");
        let line = text
            .lines()
            .find_map(|l| l.trim_start().strip_prefix(&prefix))
            .unwrap_or_else(|| panic!("{rel} has no {name}"));
        line.strip_suffix("\";")
            .unwrap_or_else(|| panic!("{rel}: {line}"))
            .to_owned()
    }

    #[test]
    fn the_frontend_and_the_launcher_agree_on_the_failed_power_variable() {
        for name in ["POWER_FAILED_ENV", "REBOOT_FAILED", "POWER_OFF_FAILED"] {
            assert_eq!(
                const_text("apps/neptron/src/power.rs", name),
                const_text("apps/neptron-launch/src/power.rs", name),
                "{name}"
            );
        }
    }

    #[test]
    fn the_frontend_and_the_launcher_agree_on_the_power_variable() {
        let frontend = const_text("apps/neptron/src/power.rs", "POWER_ENV");
        assert_eq!(
            frontend,
            const_text("apps/neptron-launch/src/power.rs", "POWER_ENV")
        );
        assert_eq!(frontend, "NEPTRON_POWER");
        assert_eq!(
            const_text("apps/neptron-launch/src/power.rs", "POWER_ENV_ON"),
            "1",
            "the value that neptron::power::control_from takes"
        );
    }
}
