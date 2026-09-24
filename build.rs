fn main() {
    // The lnmai-core Lean runtime's link args are emitted by the build scripts
    // of lnmai-core, but `cargo:rustc-link-arg` only applies to the crate that
    // owns the build script. Re-run the same build here so the
    // args reach this package's binaries (e.g. lambda_dx_player).
    lnmai_core::build::build();
}
