
use std::path::PathBuf;
use lnmai_core_rs;

fn main() {
    lnmai_core_rs::build(PathBuf::from("lnmai-core-rs/lnmai-core-ffi"));
}