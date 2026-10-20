#!/usr/bin/env bash
# Run the Rust-vs-Lean differential tests.
#
# Requires the Lean CLIs to be built in the parent `lnmai-core` checkout:
#   lake build simai-parser-cli runtime-step-cli
#
# Override paths with LNMAI_LEAN_PARSER_CLI / LNMAI_LEAN_RUNTIME_CLI.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$here/.." && pwd)"
bin="$repo/.lake/build/bin"

export LNMAI_LEAN_PARSER_CLI="${LNMAI_LEAN_PARSER_CLI:-$bin/simai-parser-cli}"
export LNMAI_LEAN_RUNTIME_CLI="${LNMAI_LEAN_RUNTIME_CLI:-$bin/runtime-step-cli}"

for f in "$LNMAI_LEAN_PARSER_CLI" "$LNMAI_LEAN_RUNTIME_CLI"; do
  if [ ! -x "$f" ]; then
    echo "missing Lean CLI: $f" >&2
    echo "run: (cd '$repo' && lake build simai-parser-cli runtime-step-cli)" >&2
    exit 1
  fi
done

cd "$here"
profile=()
if [[ "${1:-}" == --release ]]; then
  profile=(--release)
  shift
fi
cargo test --locked "${profile[@]}" -p lnmai_core --test differential -- --ignored --nocapture "$@"
