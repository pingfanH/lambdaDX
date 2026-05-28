use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));
    let lean_project = manifest_dir.join("lnmai-core-ffi").join("lnmai-core");

    println!("cargo:rerun-if-changed={}", lean_project.join("lakefile.toml").display());
    println!("cargo:rerun-if-changed={}", lean_project.join("lean-toolchain").display());
    println!("cargo:rerun-if-changed={}", lean_project.join("LnmaiCore").display());
    println!("cargo:rerun-if-changed={}", lean_project.join("include/lnmai_ffi.h").display());
    println!("cargo:rerun-if-changed={}", lean_project.join("include/lnmai_session.h").display());

    run_lake_build(&lean_project);

    let lake_rsp_path = lean_project.join(".lake/build/bin/lnmai-core.rsp");
    println!("cargo:rerun-if-changed={}", lake_rsp_path.display());
    let lake_rsp = fs::read_to_string(&lake_rsp_path).expect("failed to read Lake link response file");
    let lake_args = parse_rsp(&lake_rsp);

    let resolved = resolve_system_libraries(&lake_args);

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("missing OUT_DIR"));
    let linker_rsp_path = out_dir.join("lnmai-link.rsp");
    let linker_rsp = build_link_rsp(&lake_args, &resolved, &lean_project);
    fs::write(&linker_rsp_path, linker_rsp).expect("failed to write linker rsp");

    if let Some(parent) = resolved.lean_toolchain_lib_dir.parent() {
        println!("cargo:rustc-link-search=native={}", parent.display());
    }
    println!("cargo:rustc-link-search=native={}", resolved.lean_toolchain_lib_dir.display());
    println!("cargo:rustc-link-arg=@{}", linker_rsp_path.display());
    println!("cargo:rustc-link-arg=-Wl,--no-as-needed");
    println!("cargo:rustc-link-arg=-pthread");
    println!("cargo:rustc-link-arg=-ldl");
    println!("cargo:rustc-link-arg=-lm");
    println!("cargo:rustc-link-arg=-lutil");
    println!("cargo:rustc-link-arg=-lrt");
    println!("cargo:rustc-link-arg=-lc");
    println!("cargo:rustc-link-arg=-pthread");
    println!("cargo:rustc-link-lib=dylib=dl");
    println!("cargo:rustc-link-lib=dylib=m");
    println!("cargo:rustc-link-lib=dylib=util");
}

fn run_lake_build(lean_project: &Path) {
    let status = Command::new("lake")
        .args(["build", "LnmaiCore", "LnmaiCore.FFI"])
        .current_dir(lean_project)
        .status()
        .expect("failed to invoke lake");
    if !status.success() {
        panic!("lake build failed with status {status}");
    }
}

fn parse_rsp(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.trim_matches('"').to_owned())
        .collect()
}

fn build_link_rsp(lake_args: &[String], resolved: &ResolvedLibraries, lean_project: &Path) -> String {
    let mut out = String::new();
    let mut index = 0;
    while index < lake_args.len() {
        let arg = &lake_args[index];

        if is_excluded_object(arg, lean_project) {
            index += 1;
            continue;
        }

        if arg == "-L" {
            let path = lake_args.get(index + 1).expect("-L without path");
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

    out
}

fn is_excluded_object(arg: &str, _lean_project: &Path) -> bool {
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
        if candidates.iter().any(|candidate| name == candidate || name.starts_with(&format!("{candidate}."))) {
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
    let stdout = String::from_utf8(output.stdout).expect("compiler search dirs were not valid UTF-8");

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
    suffixes.iter().map(|suffix| format!("{base}{suffix}")).collect()
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
