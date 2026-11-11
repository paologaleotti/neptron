//! Compiles the C shim for the variadic libretro log callback.

#![forbid(unsafe_code)]

fn main() {
    println!("cargo::rerun-if-changed=src/log_shim.c");
    cc::Build::new()
        .file("src/log_shim.c")
        .std("c11")
        .warnings(true)
        .extra_warnings(true)
        .compile("neptron_libretro_log");
}
