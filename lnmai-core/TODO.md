# Remaining work toward complete Rust–Lean equivalence

The complete runtime pipeline is implemented, but whole-program equivalence
has not been proved. `../verification/SPEC_MAPPING.md` identifies the current
boundary. Do not mark this task complete solely because a corpus or test
suite passes.

`../verification/SOURCE_AUDIT.md` records module coverage, concrete source
findings, current proof builds and remaining gaps.

## Current checkpoint (2026-10-19)

The current Rust, LLBC and generated-Lean batch is complete and ready to
commit. A fresh extraction check passed with
`rtk env AENEAS_HOME=/Users/pingfanh/bin/aeneas
CHARON_HOME=/Users/pingfanh/bin/charon bash
lnmai-core-rs/tools/regen_aeneas.sh --check`: the six permitted formatter
external declarations are the only external declarations, no runtime axioms
are present, `byte_exact` is `true`, and the LLBC, generated Lean, partition
files and extraction manifest match the Rust sources.

The full Lean build under `verification` completed all 17,541 tasks.
`Verification.Audit` audited 8,054 theorems. The audit uses only
`propext`, `Classical.choice` and `Quot.sound`; no `sorryAx` or runtime axiom
was introduced. `rtk git diff --check` also passed.

This closes the extraction and proof-audit obligations for the current batch,
but it does not establish whole-runtime Rust–Lean equivalence. The remaining
representation, parser, lifecycle, scheduler, session, FFI and ABI
obligations listed below are still open. The historical checkpoint notes
below preserve earlier evidence; statements there that an independent audit
was pending refer to those earlier checkpoints and are superseded by this
section.

Tests remain intentionally deferred for this checkpoint. After the remaining
equivalence work is complete, run `rtk tools/validate_all.sh` and include the
registered chart cases:

- `tools/assets/011996 Usagi Flap`
- `tools/assets/011951 サイエンス`
- `tools/assets/蛙石`

The three cases are registered but have not been executed in this checkpoint;
testing remains deferred until the remaining equivalence obligations are
complete.

## Current work order

Finish the module-by-module implementation alignment and equivalence proofs
before running tests, as requested by the user. Rust extraction and Lean
equivalence-proof compilation belong to this first phase. Unit, differential,
real-chart and ABI acceptance runs are deferred until that phase is complete.
Prioritize parser, complete lifecycles, scheduler and judgments. Defer further
count/combo-display proofs until those priority paths are covered.
The interrupted Koishi diagnostic is not a passing acceptance result.
The deferred corpus includes every difficulty from `011996 Usagi Flap`,
`011951 サイエンス` and `蛙石` (10 additional cases, 48 total). Both frontend
comparison and complete runtime playthroughs use this same corpus. Registration
is complete; these newly added cases have not been executed. The Lean
real-chart acceptance also includes all ten, retaining AP and no-missing-note
requirements.

## Required acceptance for each kernel/runtime change

- Run `tools/validate_all.sh` from the parent checkout. It checks formatting,
  Clippy, debug/release tests, extraction freshness, Lean regressions and real
  charts, equivalence proofs with axiom audit, all differential tests and C ABI.
- Keep LLBC and generated Lean synchronized. Never edit generated Lean or add
  `sorry`/custom axioms to make a claimed equivalence theorem pass.
- Record whether evidence is a universal theorem, a bounded theorem or a
  finite executable comparison. Preserve all theorem preconditions.

## Formal work still required

1. Relate the actual `Integer`/`Natural`/`Rat` runtime representations and
   operations to Lean arithmetic, including promotion/normalization, division
   by zero, Euclidean division and saturating natural subtraction.
2. Prove exact-time adapters, threshold clamping and runtime score/accuracy
   implementations. Shared finite tables alone do not prove the adapters.
3. Extend extraction/verification to Simai parsing, shape tables, timing,
   normalization, lowering, validation and error/JSON representation.
4. Prove chart loading, storage/map order, input state and every note lifecycle
   transition. Cover Hold head/tail events, EX grades, parent/dependent slides,
   touch groups, score/audio/render event order and all settings.
5. Prove scheduler/frame composition, default tactic generation, seek/replay
   and restoration for arbitrary traces.
6. Prove JSON/session/FFI semantics and ownership/ABI obligations. The native
   protocol's modulo-64-bit representation must match Lean separately from
   unbounded gameplay arithmetic.

## Kernel additions in this working revision

The current batch adds the extracted `connected_slide_metadata` kernel and
wires runtime normalization through it. `ConnectedSlideMetadata.apply_semantics`
proves the source-group presence/reset branches, connected slide-kind choice,
parent selection, group flags and next-chain state under the explicit BigNat
adapter contract. It is included in `Verification.Audit`; the concrete
Natural representation, timing adapters and whole normalization composition
remain open.

The same batch adds the extracted `connected_slide_lifecycle` kernel and
wires the normalization timing step through it. `ConnectedSlideLifecycle.apply_semantics`
proves optional judge-time shifts, parent/group timing selection, next-chain
state and end-of-group reset under the explicit BigInt adapter contract. The
proof is included in `Verification.Audit`; arbitrary-precision refinement
and whole normalization composition remain open.

`Hold`, `HoldModern`, `Rotations`, `Time`, `Constants` and `ScoreExtra` contain
new obligations for the formerly missing pure functions. Their completion
requires a passing fresh proof build and axiom audit, not just the presence of
source files. Modern-slide arithmetic now uses Euclidean division; negative
wait times are included in the stated bounded range.

Runtime storage and slide symmetry now re-export their extracted implementations.
`Storage*` proofs cover arbitrary vector contents and require value-preserving
cloning and pure callback specifications; `Symmetry` covers every machine
rotation argument, including noncanonical representatives modulo eight.

Runtime slide checkpoints now also use the extracted `lifecycle::SlideArea<P>`
with arbitrary-precision progress values as the opaque parameter.
`SlideHistory`, `SlideAreaLoops`, `SlideArea` and `SlideAreaTrace` compiled
successfully, covering malformed histories, independent one-target legacy
seeding, defaults, cloning, updates, observations and finite input traces.
The runtime now also delegates ordered queue traversal to the extracted
`slide_queue` module. That extraction and library-only typecheck succeeded;
the queue proofs passed combined-source elaboration, while a fresh
separate-module axiom audit is still pending.
This does not cover the scheduler that selects checkpoint updates.

The parser now uses the extracted character classifiers and bracket-aware
top-level splitter. `Tokenize` passed combined-source elaboration and axiom
checks, including loop termination, bounds and complete output equivalence
for character sequences with length below `Usize.max`. UTF-8/string conversion
and the rest of parsing/normalization/lowering remain open.

The runtime-used sanitizer and all thirteen slide-command scans now have
accepted extracted-code proofs in `SlideParserText` (18 principal axiom
checks, `/tmp/lnmai-parser-text-proof-accepted-v1.json`). Filtering before
command scanning, safe indices/pushes and character order are included.
UTF-8 decoding/String encoding and the concrete diagnostic/numeric adapters
remain open. The parent-chain, timing-route, ordered-parsing and queue-metric
proofs now also passed combined-source Lean elaboration and all 50 new
principal axiom checks. Together with Text, the current batch has 68 accepted
principal reports, with only standard logical axioms. The audit requires these
new theorems; a fresh separate-module audit remains required. Concrete
runtime adapters and whole scheduling remain open. Natural scanning and timing
aggregation have the additional accepted proofs described below; their atomic
numeric, String and closure contracts remain explicit.

The following accepted batch adds the runtime-used `natural_parser`,
`timing_aggregate` and `slide_schedule` kernels. The finalized extraction
matches the formatted Rust source (`/tmp/lnmai-natural-schedule-extraction-4.log`);
library-only Cargo checks, formatting and whitespace checks passed.
`NaturalParser`, `TimingAggregate` and `SlideSchedule` passed combined-source
elaboration and all 57 principal axiom checks, with only standard logical
axioms. Diagnostics and matching source hashes are recorded in
`/tmp/lnmai-natural-schedule-proof-accepted-v8.json` and its `.sources.json`.
They are registered in the required axiom audit and deferred validation script.
This does not discharge the concrete BigUint/Rat, character, closure,
lifecycle/parent, container or capacity contracts. The preceding
68-report acceptance belongs to its recorded source-hash snapshot, before
this re-extraction; it is not a fresh audit of the current Generated module.
`SlideScheduleParents` adds seven composition theorems using the actual
extracted parent kernels to fill the scheduler's three parent fields. All 64
principal reports (57 + 7) passed combined-source elaboration and axiom checks
in `/tmp/lnmai-natural-schedule-parents-proof-accepted-v9.json`, with matching
source hashes. The independent-module audit in
`/tmp/lnmai-natural-schedule-independent-audit-1.log` completed fifteen modules
without failure before its driver was held at a module boundary for the new
judgment proof work. That superseded driver has been stopped. The independent
audit of the installed judgment extraction in
`/tmp/lnmai-judgment-independent-audit-1.log` compiled Generated and Areas before
its driver was stopped at the Bridge module boundary to batch further changes.
Completed artifacts are retained; the whole audit is not yet complete. Concrete facade wiring,
metadata, point operations, cloning and capacity contracts remain explicit.

The runtime now calls the shared `judgment_adapter` kernel for exact clamping,
tap/touch/slide judgments, hold percentages and tail judgments, slide timeout
and fast/late counting. Library-only Cargo checking and a staged real extraction
passed. All fifteen `JudgmentAdapter` theorems passed combined-source elaboration
and axiom checking, with no `sorryAx` or nonstandard axioms. The evidence and
65 matching source/artifact hashes are recorded in
`/tmp/lnmai-judgment-proof-accepted-v3.json` and its `.sources.json`/`.evidence.json`.
This bundle uses all thirteen new declarations verbatim from the real extraction;
existing declarations are unchanged apart from order. The full Generated/LLBC
artifacts are now installed and the fresh independent audit is still pending.
Only individual integer-operation laws are assumed, rather than whole judgment
formulas. The actual Integer/BigInt representation and runtime trait instance
remain unproved.

Tap and SlideHead now use the shared extracted `TapState` and `tap_transition`.
Their fresh extraction and library-only typecheck passed. `TapLifecycle`
single-step and finite-frame composition proofs passed combined-source Lean acceptance;
actual unbounded time/judgment/event adapters remain explicit boundaries.

`SlideQueueModel` has compiled separately. Extracted traversal, finite-frame
composition and `SlideQueueReferenceTrace` now elaborate together, including
every intermediate reference queue, ordered command list and command-free
replay. The new `VecEquality`, `SlideAreaEquality` and `StorageEquality`
proofs passed the same combined-source check. Clone/container hypotheses
remain; this does not replace the fresh separate-module audit.

The default-value source pass corrected the difference between Lean's derived
`Inhabited.default` and structure field initializers. Rust `Default` now
matches the former; `with_field_defaults()` preserves the latter for input,
game state, click cursors and the global registry. The corrected kernel was
re-extracted and the runtime library typechecked; tests remain deferred.
The unused floating-point parser at `src/simai.rs` was removed; all Cargo
targets use the exact parser in `crates/lnmai-core/src/simai/`.

`Shape` and `ShapeSolver` cover the shared canonical-shape solver.
`AreasPublic` and `TypesPublic` add public defaults, arrays, clones, equality
and order. Unbounded area adapter composition remains conditional on the
actual runtime producing the exact natural residue/index.
`ShapeAdapters` also passed combined-source elaboration and individual axiom
checks for four unbounded-endpoint composition theorems. They explicitly
assume the runtime conversion to `usize` preserves representable values and
fails only above its maximum. Actual bigint conversion remains unproved.

`IntegerAdapters`, `AccuracyAdapters` and `RuntimeScore` passed combined-source
elaboration and principal-theorem axiom checks. They establish the decoded
Euclidean-division algorithm, five rational accuracy formulas and ordered
event-fold/trace composition under explicit representation laws. Actual
bigint/rational operations and unextracted runtime callers remain open.

Runtime grade indexing, enumeration and fixed-array reads/writes now delegate
to the extracted `grade_counts` kernel. Fresh extraction, library-only Rust
typechecking and the Generated module compilation passed. `GradeCounts`
passed combined-source elaboration and axiom checks for arbitrary contents;
its read/write theorems need no Clone law, and decoded increments assume
the addition law. `RuntimeCounts` five-table routing/update composition also
passed combined-source elaboration and principal axiom checks, retaining its
explicit arithmetic/decoding laws.
The previous full proof-build driver was stopped at a module boundary after
fresh extraction invalidated its plan; a fresh full audit is still required.

Runtime JSON decoding now includes Syntax/IR, autoplay, `FrameInput`, public
FFI results and event/position types. The session registry and native output
commit order are aligned with Lean in source; serialization and session/ABI
proof obligations remain open.
The source inventory covers 75 derived encoders, 71 derived decoders, 26
concrete custom instances and 50 structure field sequences. Numeric parsing
now accepts Lean-style digit separators, and runtime time constants share
the kernel values. Combo-state classification uses judgment counts, including
the stricter Break requirement for AP+. These remain source/implementation
evidence until their representation and composition proofs are discharged.

Tap/SlideHead, Touch and Hold/TouchHold runtime steps now share extracted
control flow (`tap_transition`, `touch_transition`/`finish_touch_judgment`,
`hold_head_action`/`hold_after_head_action`/`hold_body_action`). Their
equivalence modules passed combined-source elaboration and every requested
axiom check (accepted diagnostics and source hashes in
`/tmp/lnmai-lifecycle-proof-accepted-v1.json` and its `.sources.json`), so
the modules are wired into `Verification/Audit.lean` and the validation
script. Exact-time arithmetic, raw judgment tables and event constructors
remain explicit adapter boundaries, and the unbounded counter/bigint
representations stay outside the kernel.

Scheduler click ownership now also runs through the extracted kernel
(`consume_click`, `tap_queue_has_unused_click`, `touch_input_available`),
with `Verification/SchedulerClicks.lean` relating the runtime's comparison
and counter updates to `consumeButtonThenFallbackSensor`,
`tapFamilyQueueHasUnusedClick` and `hasUnusedTouchInputAt`. The remaining
scheduler surface is the frame loop itself: note iteration order, shared
touch-group state, and the audio/render command order.

Slide/WiFi now shares the extracted generic `slide_lifecycle` state and
control flow. `SlideLifecycle` passed combined-source Lean elaboration and
thirteen principal axiom checks, covering single steps, output gating and
arbitrary finite frame traces. The accepted source hashes and diagnostics are
in `/tmp/lnmai-slide-lifecycle-proof-accepted-v2.json` and its `.sources.json`.
Its concrete runtime time, raw-judgment, queue-snapshot, event and render
adapters remain open; the theorem is not full runtime extraction.

The extracted slide command selectors, scalar bar-count/wait tables and their
parser composition passed combined-source elaboration. The shape-solver bridge
now invokes the extracted solver rather than assuming whole-function agreement.
`SchedulerFrame` also passed for driver ordering, conditional frame composition
and first-match touch-group search/registration. All 50 principal axiom reports
in the combined batch passed; evidence and source hashes are preserved in
`/tmp/lnmai-parser-scheduler-proof-accepted-v16.json` and its `.sources.json`.

The character/error/just-type frontend passed combined-source elaboration and
all 24 principal axiom checks together with the previous 50 theorems. The
accepted diagnostics and source hashes are in
`/tmp/lnmai-frontend-proof-accepted-v14.json` and its `.sources.json`. Complete
diagnostic values, command precedence and fallible-operation order are covered.
Whole-function reader, just-type and solver assumptions have been discharged;
text decoding and primitive representation laws remain open. Sanitization and
command observations are discharged by the later `SlideParserText` batch.

Text sanitization/command scanning and connected-slide parent loops have now
passed the combined-source checks recorded above. Judgment adapter control flow
has also passed its fifteen checks. Slide traversal/parent/judgment composition now derives that phase's result;
the later traversal batch also derives Tap and ordinary Hold.
Remaining work includes parser/table payloads, normalization/lowering, Maidata
segmentation, exact numeric timing and complete runtime frame/lifecycle
composition. Every new kernel must be called by the
runtime and proved against the original Lean definition. Representation and
callback contracts are obligations, not completed proofs of their callers.

## Batched composition checkpoint (2026-10-17)

The composition batch passed combined-source Lean elaboration: 76 current
modules, 93 principal reports, 2744 transitively audited theorems and 185
matching source/artifact hashes. Evidence is preserved in
`/tmp/lnmai-composition-proof-accepted-v6.json` with its `.sources.json`,
`.evidence.json` and `.lean` snapshots. This is not an independent-module build.

The nine composition modules contribute 67 principal reports. They connect
exact judgment kernels to Tap/SlideHead/Touch/Hold/Slide lifecycles, Slide
traversal and parent operations, score folding, frame finalization, and finite
frame histories. `TapScheduleModel` adds 11 reports for the original queue
recursion and sufficient fuel; `QueueMaintenanceModel` adds 15 for the
original Hold/Touch queue normalization and replacement behavior.

The later traversal batch passed combined-source checks with 51 source modules,
144 principal reports, 2028 transitively audited theorems and 149 matching
hashes (/tmp/lnmai-traversal-proof-accepted-v23.json and adjacent snapshots).
It proves actual extracted Tap/Hold/queue loops, judgment composition, Tap's
inter-zone map_accum and finite frame histories. The frame theorem now retains
only Touch and TouchHold as complete-pass premises. Atomic operations,
unbounded numeric/container representations, allocation bounds, actual runtime
facades and initialization remain open. Uniform capacity is still a premise,
not a proved reachable-state invariant.

HoldScheduleQueueJudgments subsequently derives the Hold update_queue law from
slot/replace/normalize/store points and installs it in SchedulerFrameTraversals.
This new connection and the full current Audit passed combined-source v24:
90 source modules, 150 principal reports, 3146 transitively audited theorems,
202 matching hashes, and only standard logical axioms. Evidence is preserved
in /tmp/lnmai-traversal-proof-accepted-v24.json and its adjacent snapshots.
A complete independent-module build is still required. The Tap/Hold/queue
kernels, callers and real extraction were installed in checkpoint 2a3333c.
Next runtime changes are staged separately: Touch head and automatic traversal,
TouchHold traversal, and parser normalization control flow. They have no
accepted extracted-code proof yet. Defer tests and validate_all.sh until code
equivalence is closed, including the already registered three requested charts.
