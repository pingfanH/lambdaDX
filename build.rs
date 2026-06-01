use std::env;
use std::path::PathBuf;

fn main() {
    // When ffi feature is enabled in lnmai-core-rs, we need to propagate the link settings
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let lnmai_core_rs_dir = manifest_dir.join("lnmai-core-rs");
    
    // Check if lnmai-core-ffi exists and has a build function
    let ffi_build = lnmai_core_rs_dir.join("lnmai-core-ffi").join("src").join("build.rs");
    if ffi_build.exists() {
        // Call the FFI build to get the link settings
        lnmai_core_rs::build::build(lnmai_core_rs_dir.join("lnmai-core-ffi"));
    }
}
