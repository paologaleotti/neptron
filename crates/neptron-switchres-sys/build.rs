//! Compiles the vendored Switchres calculation sources and the shim.

#![forbid(unsafe_code)]

use std::path::PathBuf;

/// The calculation sources. The other upstream files hold display backends
/// and are not vendored.
const SWITCHRES_SOURCES: &[&str] = &["modeline.cpp", "monitor.cpp", "log.cpp"];
const SWITCHRES_HEADERS: &[&str] = &["modeline.h", "monitor.h", "log.h"];
const SHIM_SOURCE: &str = "shim/neptron_switchres.cpp";
const SHIM_HEADER: &str = "shim/neptron_switchres.h";

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let vendor = manifest.join("../../vendor/switchres");

    // The shim calls Switchres, so its archive must come first on the link
    // line.
    base_build(&vendor)
        .include(manifest.join("shim"))
        .file(manifest.join(SHIM_SOURCE))
        .warnings_into_errors(true)
        .compile("neptron_switchres_shim");

    // Warnings are off for the vendored files: they stay unchanged, so a
    // warning there is not ours to fix.
    let mut switchres = base_build(&vendor);
    switchres.warnings(false).extra_warnings(false).flag("-w");
    for source in SWITCHRES_SOURCES {
        switchres.file(vendor.join(source));
    }
    switchres.compile("switchres");

    println!("cargo:rerun-if-changed={SHIM_SOURCE}");
    println!("cargo:rerun-if-changed={SHIM_HEADER}");
    for file in SWITCHRES_SOURCES.iter().chain(SWITCHRES_HEADERS) {
        println!("cargo:rerun-if-changed={}", vendor.join(file).display());
    }
}

/// The settings that both archives share.
///
/// `-ffp-contract=off` stops the compiler from fusing a multiply and an add
/// into one instruction. GCC fuses by default on aarch64 and Clang does not,
/// so without it the same source gives different doubles on Linux and macOS.
/// The sources use no fast-math option and no function of libm that rounds
/// in a way the platform chooses (only floor, ceil, round, roundf and fabs).
fn base_build(vendor: &std::path::Path) -> cc::Build {
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++11")
        .flag("-ffp-contract=off")
        .include(vendor);
    build
}
