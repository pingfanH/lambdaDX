use std::path::PathBuf;
use std::process::Command;

/// Forwards the Lean/FFI linker response file produced by `lnmai-core`'s build
/// script to this package's own binaries.
///
/// Cargo propagates `rustc-link-search`/`rustc-link-lib` from dependency build
/// scripts, but it does **not** propagate `cargo:rustc-link-arg`. Without this
/// forwarding step the final binary is linked without any of the Lean objects
/// and every `lnmai_*`/`lean_*` symbol is reported as undefined.
fn main() {
    if std::env::var_os("CARGO_FEATURE_BACKEND_LEAN").is_none() {
        return;
    }

    // `lnmai-core`'s build script writes `lnmai-core-link.rsp` next to the
    // profile directory (e.g. `target/debug/`); mirror that same location here.
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("missing OUT_DIR"));
    let profile_dir = out_dir
        .ancestors()
        .nth(3)
        .expect("OUT_DIR should live under <target>/<profile>/build/<pkg>/out");
    let rsp = profile_dir.join("lnmai-core-link.rsp");
    println!("cargo:rustc-link-arg=@{}", rsp.display());

    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-arg=-Wl,-syslibroot");
        println!("cargo:rustc-link-arg={}", sdk_path());
    }
}

fn sdk_path() -> String {
    let output = Command::new("xcrun")
        .args(["--sdk", "macosx", "--show-sdk-path"])
        .output()
        .expect("failed to query sdk path");

    String::from_utf8(output.stdout).unwrap().trim().to_string()
}
