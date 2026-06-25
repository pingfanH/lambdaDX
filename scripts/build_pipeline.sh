#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "${repo_root}" ]]; then
  echo "error: run this command from inside the LambdaDX repository" >&2
  exit 1
fi

cd "${repo_root}"

if [[ "$#" -eq 0 ]]; then
  set -- .#player
fi

echo "==> nix build $*"
exec nix build "$@"
