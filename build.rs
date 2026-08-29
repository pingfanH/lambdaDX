fn main() {
    // The lnmai-core Lean runtime's link args are emitted by the build scripts
    // of lnmai-core-rs/lnmai-core-ffi, but `cargo:rustc-link-arg` only applies
    // to the crate that owns the build script. Re-run the same build here so the
    // args reach this package's binaries (e.g. lambda_dx_player).
    let manifest_dir =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    lnmai_core_ffi::build::build(manifest_dir.join("lnmai-core-rs/lnmai-core-ffi"));
}
