#!/usr/bin/env bash
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cmp "$here/include/lnmai_native.h" "$here/../native/lnmai_native.h"
cd "$here"
cargo build -p lnmai_core --release --locked
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
"${CC:-cc}" -std=c11 -Wall -Wextra -Werror -Iinclude tests/native_smoke.c \
  -Ltarget/release -llnmai_core -Wl,-rpath,"$here/target/release" -o "$out/smoke"
"$out/smoke"
