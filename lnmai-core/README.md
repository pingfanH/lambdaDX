# Rust gameplay core

This workspace implements the Lean `LnmaiCore` parser, gameplay state machine,
score calculation, seek/replay, JSON interface and native C ABI. The Lean
specification is in `../LnmaiCore`; the proof package is in `../verification`.

## Architecture and verification boundary

- `crates/lnmai-core` is the runtime implementation. Timeline values use exact
  `Integer` arithmetic, note identifiers/counts/scores use `Natural`, and
  accuracy and parser timing use exact `Rat`. These types preserve Lean's
  unbounded arithmetic, Euclidean division and total division-by-zero behavior.
- `crates/lnmai-core-verify` contains an Aeneas-compatible kernel using machine
  integers. The runtime shares its enums, area operations, grade conversions
  and finite judgment tables. Unbounded inputs pass through explicit adapters;
  score arithmetic is implemented separately with exact natural numbers.
- Charon and Aeneas extract **the kernel**, not the whole runtime. Kernel
  equivalence theorems carry explicit overflow preconditions where needed.
  They do not establish correctness of every adapter, the big-integer library,
  parser, scheduler, seek implementation or FFI.
- Differential tests compare the Rust implementation with executable Lean
  oracles. Passing these finite tests is evidence of matching behavior, not a
  proof of equivalence for every possible input. Whole-program formal
  equivalence is still incomplete; see `TODO.md` and
  `../verification/SPEC_MAPPING.md`.

## Build and full validation

Run commands from the parent `lnmai-core` checkout unless shown otherwise.

```bash
cargo test --manifest-path lnmai-core-rs/Cargo.toml --workspace --locked
lake build LnmaiCore Proofs simai-parser-cli runtime-step-cli

export AENEAS_HOME=/path/to/aeneas
export CHARON_HOME=/path/to/charon
bash verification/tools/wire_aeneas.sh
bash lnmai-core-rs/tools/regen_aeneas.sh --check
(cd verification && lake build)
bash lnmai-core-rs/tools/run_differential.sh --release
bash lnmai-core-rs/tools/run_native_smoke.sh

# All required stages, with individual logs and failure propagation:
bash lnmai-core-rs/tools/validate_all.sh
```

Lean is pinned to `leanprover/lean4:v4.30.0`. The extraction environment uses
Aeneas `ac74e1b`, Charon `0.1.210` at
`a535e914f74db4fd9e6be7048f4233270d8945c0`, and its pinned Rust nightly.
The backend is wired through an ignored `.lake/local/aeneas` symlink, with no
user-specific absolute path in the committed Lake configuration.

`regen_aeneas.sh` updates both the LLBC artifact and generated Lean source;
`--check` extracts to a temporary directory and rejects stale artifacts. The
LLBC includes the extraction target: regenerate and recheck proofs after a
target or toolchain change. Generated Lean must never be edited by hand.

## Test coverage

The 19 differential tests are explicitly ignored by ordinary `cargo test`.
`run_differential.sh` enables them and fails if either Lean oracle is absent.
`LNMAI_LEAN_PARSER_CLI` and `LNMAI_LEAN_RUNTIME_CLI` can override their paths.

Coverage includes all frontend representations, parse/decode errors, timing
and grade boundaries, large signed integers/naturals/rationals, autoplay,
settings, seek and resume, JSON state restoration, native records and errors.
The direct arithmetic oracle covers signed integers, naturals and rationals
with boundary and seeded inputs up to 2048 bits, including division by zero.
The fixture corpus contains 48 chart difficulties, including all four in
`011996 Usagi Flap`, all four in `011951 サイエンス`, and both in `蛙石`.
Every runtime difficulty
is played in classic/modern mode, with autoplay enabled and disabled; each
frame's output is compared, with full state at checkpoints and completion.
Synthetic runtime scenarios compare full state every frame.

Lean real-chart acceptance also checks AP and missing completed notes. Hold
head notifications do not count as completed Hold notes; only their tails do.
All ten difficulties from the three added songs are registered in this Lean
acceptance stage as well. Their execution remains deferred until code
equivalence work is complete.

## C interfaces

`include/lnmai_ffi.h` exposes JSON functions; free returned strings with
`lnmai_string_free`. `include/lnmai_native.h` exposes ABI version 3, including
its eight native operations and owned buffer release functions. The C smoke
test compiles against both headers and exercises success and failure paths.
Native word conversion deliberately retains the low 64 bits, matching Lean.

Proof builds load Mathlib and Aeneas and can take several minutes per module
on a 16 GB machine. Keep proof builds out of the Rust edit/test loop and avoid
multiple overlapping proof builds.
