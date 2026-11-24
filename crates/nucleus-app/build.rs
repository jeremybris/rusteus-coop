//! Build script: sets the main-thread stack size of the launcher executables.
//!
//! Parsing and running handler scripts recurses deeply, and `RUST_MIN_STACK` only applies to
//! threads Rust spawns. `.cargo/config.toml` raises the stack for the GNU toolchain through a
//! linker flag; this does the same for MSVC, where the default is 1 MB.

fn main() {
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        println!("cargo:rustc-link-arg-bins=/STACK:67108864");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
