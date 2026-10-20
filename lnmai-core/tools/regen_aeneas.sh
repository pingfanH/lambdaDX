#!/usr/bin/env bash
# Extract the Rust kernel. --check compares without changing artifacts.
# Use AENEAS_HOME/CHARON_HOME, AENEAS_BIN/CHARON_BIN or executables on PATH.
set -euo pipefail

mode=write
if [[ "${1:-}" == --check ]]; then mode=check; shift; fi
if [[ $# != 0 ]]; then echo 'usage: regen_aeneas.sh [--check]' >&2; exit 2; fi
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$here/.." && pwd)"
if [[ -n "${CHARON_HOME:-}" ]]; then export PATH="$CHARON_HOME:$PATH"; fi
charon_bin="${CHARON_BIN:-$(command -v charon || true)}"
aeneas_bin="${AENEAS_BIN:-${AENEAS_HOME:+$AENEAS_HOME/bin/aeneas}}"
aeneas_bin="${aeneas_bin:-$(command -v aeneas || true)}"
for exe in "$charon_bin" "$aeneas_bin"; do
  if [[ ! -x "$exe" || -d "$exe" ]]; then
    echo 'Set AENEAS_HOME and CHARON_HOME, or AENEAS_BIN and CHARON_BIN.' >&2
    exit 1
  fi
done

out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
llbc="$out/lnmai_core_verify.llbc"
echo '[1/3] charon: extracting the Rust kernel'
(cd "$here/crates/lnmai-core-verify" && "$charon_bin" cargo --preset=aeneas --dest-file "$llbc" -- --locked)
# Destination is bookkeeping, not part of the extracted program.
python3 - "$llbc" <<'PY'
import json, pathlib, sys
p = pathlib.Path(sys.argv[1])
model = json.loads(p.read_text())
if model['has_errors']:
    raise SystemExit('Charon reported extraction errors')
model['translated']['options']['dest_file'] = None
# Charon serializes this name lookup table in hash-map iteration order.
# Sort the entries, preserving every key/value, so repeat extractions compare
# the program rather than a randomized bookkeeping order.
model['translated']['short_names'].sort(
    key=lambda entry: json.dumps(entry, sort_keys=True)
)
p.write_text(json.dumps(model, ensure_ascii=False, separators=(',', ':')))
PY
echo '[2/3] aeneas: generating the Lean model'
"$aeneas_bin" -backend lean -dest "$out" "$llbc"
generated="$out/LnmaiCoreVerify.lean"
destination="$repo/verification/Verification"
test -f "$generated"
# Preserve literal CR/LF and other Unicode characters. Text-mode newline
# conversion would silently change the extracted whitespace predicate.
python3 "$repo/verification/tools/normalize_generated.py" "$generated"
python3 "$repo/verification/tools/check_generated_axioms.py" "$generated"
# Optional Source-routed modules come only from this full extraction. The
# partitioner rejects leftover extension files from another stage.
if [[ "$mode" == check ]]; then
  echo '[3/3] checking generated model freshness'
  python3 "$repo/verification/tools/partition_generated.py" \
    "$generated" "$destination" --check
  python3 - "$here/lnmai_core_verify.llbc" "$llbc" <<'PY'
import json, sys
a, b = (json.load(open(p)) for p in sys.argv[1:])
if a != b:
    raise SystemExit('lnmai_core_verify.llbc is stale or was extracted for a different target')
PY
else
  echo '[3/3] installing extracted artifacts'
  python3 "$repo/verification/tools/partition_generated.py" \
    "$generated" "$destination"
  # Publish LLBC only after every partition and the full-order manifest verify.
  cp "$llbc" "$here/lnmai_core_verify.llbc"
fi
echo 'Aeneas artifacts match the Rust source.'
