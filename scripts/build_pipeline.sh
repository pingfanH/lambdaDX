#!/usr/bin/env bash
set -euo pipefail

if ! command -v git >/dev/null 2>&1; then
  echo "error: git is required" >&2
  exit 1
fi

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "${repo_root}" ]]; then
  echo "error: run this command from inside the LambdaDX repository" >&2
  exit 1
fi

cd "${repo_root}"

echo "==> syncing submodules"
git submodule sync --recursive
git submodule update --init --recursive

lean_project="${repo_root}/lnmai-core-rs/lnmai-core-ffi/lnmai-core"
lean_toolchain_file="${lean_project}/lean-toolchain"

if [[ ! -f "${lean_toolchain_file}" ]]; then
  echo "error: missing Lean toolchain file at ${lean_toolchain_file}" >&2
  exit 1
fi

if ! command -v elan >/dev/null 2>&1; then
  echo "error: elan is required; run through nix develop or nix run" >&2
  exit 1
fi

lean_toolchain="$(tr -d '[:space:]' < "${lean_toolchain_file}")"
if elan toolchain list | grep -Fq "${lean_toolchain}"; then
  echo "==> using installed Lean toolchain ${lean_toolchain}"
else
  echo "==> installing Lean toolchain ${lean_toolchain}"
  elan toolchain install "${lean_toolchain}"
fi

echo "==> building Lean FFI"
(
  cd "${lean_project}"
  lake build LnmaiCore LnmaiCore.FFI
)

if [[ "$#" -eq 0 ]]; then
  set -- --bin lambda_dx_player
fi

echo "==> cargo build $*"
cargo build "$@"
