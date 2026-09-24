use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn build() {
    // The Lean project itself is supplied by the pinned lnmai-core flake input.
    // This Rust wrapper only consumes its sanitized linker response file, but it
    // can also fall back to a local Lake checkout for non-Nix development.
    println!("cargo:rerun-if-env-changed=LNMAI_CORE_ARTIFACTS");
    println!("cargo:rerun-if-env-changed=LNMAI_CORE_LEAN_PROJECT");

    let lake_args = match env::var("LNMAI_CORE_ARTIFACTS")
        .ok()
        .filter(|value| !value.is_empty())
    {
        Some(artifacts) => {
            let lake_rsp_path =
                PathBuf::from(artifacts).join("share/lnmai-core/ffi-link.rsp");
            if !lake_rsp_path.exists() {
                panic!(
                    "missing sanitized Nix-built Lean response file at {}; build lnmai-core .#ffi-artifacts first",
                    lake_rsp_path.display()
                );
            }

            println!("cargo:rerun-if-changed={}", lake_rsp_path.display());
            let lake_rsp =
                fs::read_to_string(&lake_rsp_path).expect("failed to read Lake link response file");
            parse_rsp(&lake_rsp)
        }
        None => {
            let lean_project = find_lean_project();
            run_lake_build(&lean_project);
            let lake_rsp_path = lean_project.join(".lake/build/bin/lnmai-core.rsp");
            println!("cargo:rerun-if-changed={}", lake_rsp_path.display());
            let lake_rsp = fs::read_to_string(&lake_rsp_path)
                .unwrap_or_else(|error| panic!(
                    "failed to read Lake link response file at {}: {error}",
                    lake_rsp_path.display()
                ));
            parse_rsp(&lake_rsp)
        }
    };

    let resolved = resolve_system_libraries(&lake_args);

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("missing OUT_DIR"));
    let linker_rsp_path = out_dir.join("lnmai-link.rsp");
    let linker_rsp = build_link_rsp(&lake_args, &resolved);
    fs::write(&linker_rsp_path, linker_rsp).expect("failed to write linker rsp");

    if let Some(parent) = resolved.lean_toolchain_lib_dir.parent() {
        println!("cargo:rustc-link-search=native={}", parent.display());
    }
    println!(
        "cargo:rustc-link-search=native={}",
        resolved.lean_toolchain_lib_dir.display()
    );
    println!("cargo:rustc-link-arg=@{}", linker_rsp_path.display());
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-arg=-Wl,-syslibroot");
        println!("cargo:rustc-link-arg={}", sdk_path());
    }
}

fn find_lean_project() -> PathBuf {
    if let Ok(path) = env::var("LNMAI_CORE_LEAN_PROJECT") {
        let path = PathBuf::from(path);
        if path.join("lakefile.toml").exists() {
            return path;
        }
        panic!(
            "LNMAI_CORE_LEAN_PROJECT does not point at a Lake project: {}",
            path.display()
        );
    }

    // Resolve relative to the Cargo workspace root rather than the calling
    // crate's manifest dir so every crate in the workspace links the same Lake
    // project.
    //
    // Prefer the `lnmai-core-ffi` submodule: it is a git clone of
    // Neuron-Group/lnmai-core whose FFI schema matches `shared/rust_ffi_types.rs`
    // (slide heads/bodies, `headTiming`). A sibling `../lnmai-core` checkout is
    // only used as a fallback because it may be a pre-refactor snapshot that
    // emits the legacy `timing` field and fails deserialization.
    let workspace_root = workspace_root();
    let candidates = [
        workspace_root.join("lnmai-core-rs/lnmai-core-ffi/lnmai-core"),
        workspace_root.join("../lnmai-core"),
        workspace_root.join("lnmai-core-lean"),
        workspace_root.join("../lnmai-core-lean"),
    ];
    for candidate in candidates {
        if candidate.join("lakefile.toml").exists() {
            return fs::canonicalize(&candidate).unwrap_or(candidate);
        }
    }

    panic!(
        "LNMAI_CORE_ARTIFACTS is not set and no local Lean project was found under {}; run through `nix build`/`nix run`, or set LNMAI_CORE_LEAN_PROJECT to a Lake checkout of Neuron-Group/lnmai-core",
        workspace_root.display()
    );
}

fn workspace_root() -> PathBuf {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));
    let mut dir = manifest_dir.as_path();
    loop {
        let cargo_toml = dir.join("Cargo.toml");
        if let Ok(content) = fs::read_to_string(&cargo_toml) {
            if content.lines().any(|line| line.trim() == "[workspace]") {
                return dir.to_path_buf();
            }
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => return manifest_dir,
        }
    }
}

fn run_lake_build(lean_project: &Path) {
    println!(
        "cargo:rerun-if-changed={}",
        lean_project.join("lakefile.toml").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        lean_project.join("lean-toolchain").display()
    );

    let rsp_path = lean_project.join(".lake/build/bin/lnmai-core.rsp");
    if !rsp_path.exists() {
        // First build of this Lean project: fetch the mathlib cache so the
        // subsequent `lake build` does not try to compile mathlib from source.
        println!(
            "cargo:warning=lnmai-core: {} missing, running `lake exe cache get` first",
            rsp_path.display()
        );
        let cache_status = Command::new("lake")
            .args(["exe", "cache", "get"])
            .current_dir(lean_project)
            .status()
            .expect("failed to invoke `lake exe cache get`; install elan/lake or run through `nix develop`");
        if !cache_status.success() {
            panic!("`lake exe cache get` failed with status {cache_status}");
        }
    }

    let status = Command::new("lake")
        .args(["build", "lnmai-core", "+LnmaiCore.FFI:c.o"])
        .current_dir(lean_project)
        .status()
        .expect("failed to invoke `lake build`; install elan/lake or run through `nix develop`");
    if !status.success() {
        panic!("`lake build` failed with status {status}");
    }
}

fn sdk_path() -> String {
    let output = Command::new("xcrun")
        .args(["--sdk", "macosx", "--show-sdk-path"])
        .output()
        .expect("failed to query sdk path");

    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn parse_rsp(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.trim_matches('"').to_owned())
        .collect()
}

fn build_link_rsp(lake_args: &[String], resolved: &ResolvedLibraries) -> String {
    let mut out = String::new();
    let mut index = 0;
    let mut saw_uv = false;
    while index < lake_args.len() {
        let arg = &lake_args[index];

        if arg == "-fuse-ld=lld" {
            index += 1;

            continue;
        }

        if arg == "--sysroot" {
            index += 2;
            continue;
        }

        if is_excluded_object(arg) {
            index += 1;
            continue;
        }

        if arg == "-L" {
            let path = lake_args.get(index + 1).expect("-L without path");
            if path.ends_with("/lib/glibc") {
                index += 2;
                continue;
            }
            out.push_str(&quote_arg("-L"));
            out.push('\n');
            out.push_str(&quote_arg(path));
            out.push('\n');
            index += 2;
            continue;
        }

        if let Some(lib) = arg.strip_prefix("-l") {
            match lib {
                "c++" => {
                    if let Some(path) = &resolved.libcxx {
                        out.push_str(&quote_arg(dynamic_linker_switch()));
                        out.push('\n');
                        out.push_str(&quote_arg(&path.display().to_string()));
                        out.push('\n');
                    } else {
                        out.push_str(&quote_arg(arg));
                        out.push('\n');
                    }
                }
                "c++abi" => {
                    if let Some(path) = &resolved.libcxxabi {
                        out.push_str(&quote_arg(dynamic_linker_switch()));
                        out.push('\n');
                        out.push_str(&quote_arg(&path.display().to_string()));
                        out.push('\n');
                    } else {
                        out.push_str(&quote_arg(arg));
                        out.push('\n');
                    }
                }
                "gmp" => {
                    if let Some(path) = &resolved.gmp {
                        out.push_str(&quote_arg(dynamic_linker_switch()));
                        out.push('\n');
                        out.push_str(&quote_arg(&path.display().to_string()));
                        out.push('\n');
                    } else {
                        out.push_str(&quote_arg(arg));
                        out.push('\n');
                    }
                }
                "uv" => {
                    saw_uv = true;
                    if let Some(path) = &resolved.uv {
                        out.push_str(&quote_arg(dynamic_linker_switch()));
                        out.push('\n');
                        out.push_str(&quote_arg(&path.display().to_string()));
                        out.push('\n');
                    } else {
                        out.push_str(&quote_arg(arg));
                        out.push('\n');
                    }
                }
                "c" | "pthread_nonshared" | "dl" | "m" | "rt" | "pthread" => {}
                ":ld.so" | "c_nonshared" => {}
                _ => {
                    out.push_str(&quote_arg(arg));
                    out.push('\n');
                }
            }
            index += 1;
            continue;
        }

        out.push_str(&quote_arg(arg));
        out.push('\n');
        index += 1;
    }

    if cfg!(target_os = "linux") && saw_uv {
        // On recent glibc, pthread_atfork is provided through libc's linker
        // script via libc_nonshared.a. Re-emitting -lc after libuv keeps
        // ld.bfd from missing that symbol when Rust's earlier -lc has
        // already been scanned.
        out.push_str(&quote_arg("-lc"));
        out.push('\n');
    }

    out
}

fn is_excluded_object(arg: &str) -> bool {
    if !arg.ends_with(".c.o.export") {
        return false;
    }

    if arg.contains("/.lake/packages/") {
        return false;
    }

    let is_main = arg.ends_with("/.lake/build/ir/Main.c.o.export");
    let is_app = arg.contains("/.lake/build/ir/Apps/");

    is_main || is_app
}

fn quote_arg(arg: &str) -> String {
    format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\""))
}

struct ResolvedLibraries {
    lean_toolchain_lib_dir: PathBuf,
    libcxx: Option<PathBuf>,
    libcxxabi: Option<PathBuf>,
    gmp: Option<PathBuf>,
    uv: Option<PathBuf>,
}

fn resolve_system_libraries(rsp_args: &[String]) -> ResolvedLibraries {
    let lean_lib_dir = rsp_args
        .windows(2)
        .find_map(|window| {
            if window[0] == "-L" {
                Some(PathBuf::from(&window[1]))
            } else {
                None
            }
        })
        .expect("Lake response file did not contain a lean library search path");
    let lean_toolchain_lib_dir = lean_lib_dir
        .parent()
        .expect("lean lib directory should have a parent")
        .to_path_buf();

    let library_search_roots = library_search_roots();
    let dynamic_suffixes = dynamic_library_suffixes();
    let libcxx_candidates = library_candidates("libc++", &dynamic_suffixes);
    let libcxxabi_candidates = library_candidates("libc++abi", &dynamic_suffixes);
    let gmp_candidates = library_candidates("libgmp", &dynamic_suffixes);
    let uv_candidates = library_candidates("libuv", &dynamic_suffixes);

    ResolvedLibraries {
        lean_toolchain_lib_dir: lean_lib_dir,
        libcxx: find_joined_existing(&lean_toolchain_lib_dir, &libcxx_candidates),
        libcxxabi: find_joined_existing(&lean_toolchain_lib_dir, &libcxxabi_candidates),
        gmp: find_library_in_paths(&gmp_candidates, &library_search_roots)
            .or_else(|| find_pkg_config_library("gmp"))
            .or_else(|| find_pkg_config_library("gmpxx")),
        uv: find_library_in_paths(&uv_candidates, &library_search_roots)
            .or_else(|| find_pkg_config_library("libuv")),
    }
}

fn find_first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|path| path.exists()).cloned()
}

fn find_joined_existing(root: &Path, names: &[String]) -> Option<PathBuf> {
    let candidates = names.iter().map(|name| root.join(name)).collect::<Vec<_>>();
    find_first_existing(&candidates)
}

fn find_library_in_paths(candidates: &[String], roots: &[PathBuf]) -> Option<PathBuf> {
    for root in roots {
        if let Some(found) = find_library_recursive(root, candidates) {
            return Some(found);
        }
    }
    None
}

fn find_library_recursive(root: &Path, candidates: &[String]) -> Option<PathBuf> {
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_library_recursive(&path, candidates) {
                return Some(found);
            }
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if candidates
            .iter()
            .any(|candidate| name == candidate || name.starts_with(&format!("{candidate}.")))
        {
            return Some(path);
        }
    }
    None
}

fn library_search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    roots.extend(parse_compiler_library_roots());
    roots.extend(parse_env_library_roots());
    roots.sort();
    roots.dedup();
    roots
}

fn parse_compiler_library_roots() -> Vec<PathBuf> {
    let output = Command::new("cc")
        .arg("-print-search-dirs")
        .output()
        .expect("failed to query compiler search dirs");
    let stdout =
        String::from_utf8(output.stdout).expect("compiler search dirs were not valid UTF-8");

    let mut roots = Vec::new();
    for line in stdout.lines() {
        let Some(rest) = line.strip_prefix("libraries: =") else {
            continue;
        };
        roots.extend(split_search_path(rest));
    }
    roots
}

fn parse_env_library_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for key in ["LIBRARY_PATH", "LD_LIBRARY_PATH", "DYLD_LIBRARY_PATH"] {
        let Ok(value) = env::var(key) else {
            continue;
        };
        roots.extend(split_search_path(&value));
    }
    roots
}

fn split_search_path(value: &str) -> Vec<PathBuf> {
    env::split_paths(value)
        .filter(|path| path.exists())
        .collect()
}

fn find_pkg_config_library(package: &str) -> Option<PathBuf> {
    let output = Command::new("pkg-config")
        .args(["--libs-only-L", package])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    let candidates = library_candidates(library_base_name(package), &dynamic_library_suffixes());
    for token in stdout.split_whitespace() {
        let Some(path) = token.strip_prefix("-L") else {
            continue;
        };
        let lib_dir = PathBuf::from(path);
        if let Some(found) = find_library_recursive(&lib_dir, &candidates) {
            return Some(found);
        }
    }
    None
}

fn library_base_name(package: &str) -> &'static str {
    match package {
        "gmp" | "gmpxx" => "libgmp",
        "libuv" => "libuv",
        _ => unreachable!("unsupported pkg-config package"),
    }
}

fn library_candidates(base: &str, suffixes: &[&str]) -> Vec<String> {
    suffixes
        .iter()
        .map(|suffix| format!("{base}{suffix}"))
        .collect()
}

fn dynamic_library_suffixes() -> Vec<&'static str> {
    if cfg!(target_os = "macos") {
        vec![".dylib", ".so", ".so.1", ".so.1.0"]
    } else {
        vec![".so", ".so.1", ".so.1.0", ".dylib"]
    }
}

fn dynamic_linker_switch() -> &'static str {
    if cfg!(target_os = "macos") {
        "-Wl,-search_paths_first"
    } else {
        "-Wl,-Bdynamic"
    }
}
