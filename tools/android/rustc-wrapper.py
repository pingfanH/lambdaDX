#!/usr/bin/env python3
"""Adapt cargo-quad-apk's generated entry point to Rust 2024."""
import os
from pathlib import Path
import sys


# Only patch the packager's temporary file, never Cargo's registry or sources.
for argument in sys.argv[2:]:
    path = Path(argument)
    if path.name.startswith("__cargo_apk_") and path.suffix == ".tmp" and path.is_file():
        source = path.read_text()
        updated = source.replace("#[no_mangle]", "#[unsafe(no_mangle)]")
        if updated != source:
            path.write_text(updated)

command = sys.argv[1:]
previous = os.environ.get("LDX_ANDROID_RUSTC_WRAPPER")
if previous:
    command.insert(0, previous)
os.execvp(command[0], command)
