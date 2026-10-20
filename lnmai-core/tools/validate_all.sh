#!/usr/bin/env bash
# Reproducible acceptance suite. Every stage is required; no skip-pass paths.
set -euo pipefail
export LEAN_NUM_THREADS="${LEAN_NUM_THREADS:-2}"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$here/.." && pwd)"
logs="${LNMAI_VALIDATION_LOG_DIR:-$(mktemp -d /tmp/lnmai-validation.XXXXXX)}"
mkdir -p "$logs"
logs="$(cd "$logs" && pwd)"
echo "Validation logs: $logs"

run() {
  local name="$1" directory="$2"
  shift 2
  echo "[$name] running"
  if (cd "$directory" && "$@") >"$logs/$name.log" 2>&1; then
    echo "[$name] passed"
  else
    local status=$?
    tail -n 80 "$logs/$name.log" >&2
    echo "[$name] failed (exit $status); see $logs/$name.log" >&2
    return "$status"
  fi
}

source_manifest() {
  python3 - <<'PY'
import hashlib
import json
import subprocess
from pathlib import Path

names = subprocess.check_output([
    "git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"
]).decode().split("\0")
manifest = {}
for name in sorted(set(names) - {""}):
    path = Path(name)
    if path.is_file():
        manifest[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    elif not path.exists():
        manifest[name] = "deleted"
print(json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True))
PY
}

run source-state "$repo" git status --short --branch
run source-manifest "$repo" source_manifest
run rust-version "$here" rustc --version
run lean-version "$repo" lake env lean --version
run formatting "$here" cargo fmt --all -- --check
run rust-libraries "$here" cargo check --workspace --lib --locked
run extraction-freshness "$repo" bash lnmai-core-rs/tools/regen_aeneas.sh --check
# Complete the extracted-code proofs and axiom audit before executing tests.
# The LnmaiCore aggregate imports executable regressions, so build it only
# in the acceptance phase below. Proofs import individual reference modules.
if [[ -n "${AENEAS_HOME:-}" ]]; then
  run aeneas-backend "$repo" bash verification/tools/wire_aeneas.sh
fi
for module in Generated GeneratedTouchHoldBodyAdapter VecEquality Areas Bridge Types Convert Judge TypesExtra Hold HoldModern \
  Rotations Time Constants Score ScoreExtra Adapters AdapterJudgments JudgmentAdapter AdapterScore \
  IntegerAdapters AccuracyAdapters RuntimeScore RuntimeScoreJudgments GradeCounts RuntimeCounts \
  Storage StorageLoops StorageAccum StoragePublic StorageEquality Symmetry AreasPublic TypesPublic \
  Defaults Shape ShapeSolver ShapeAdapters Tokenize SlideParser SlideParserComposition SlideParserFrontend SlideParserText SlideParserSequence TimingParser SlideHistory SlideAreaLoops SlideArea SlideAreaTrace \
  NaturalParser StringSplit TimingNumberModel TimingNumberReference TimingNumber TimingAggregate \
  PrefixDirectivesModel PrefixAsciiTrim PrefixDirectivesReference PrefixDirectivesRanges PrefixDirectivesExtracted MaidataHeaderReference \
  SlideChainReference SlideChainModel MaidataScanModel TouchHoldRebuildModel TouchHoldRebuildReverse \
  SlideChainExtractedReads SlideChainExtractedTiming SlideChainExtractedLoops \
  SlideChainExtractedRender SlideChainExtractedEncode SlideChainExtracted \
  SlideChainWrapper SlideChainFrontend SlideChainPrefixFrontend MaidataScanFuel MaidataScan MaidataLineFields MaidataScanGroupsModel MaidataScanGroupsPlan MaidataScanGroups TouchHoldRebuildReverseExtracted TouchHoldRebuildUpsert TouchHoldRebuildTraversal \
  VecPrependModel VecPrependExtracted VecEraseModel VecEraseExtracted TouchHoldRebuildCapacity \
  TouchHoldBodyReads TouchHoldBodyTraits TouchHoldBodyFields TouchHoldBodyScans \
  TouchHoldBodyViewPoints TouchHoldBodyUpdates TouchHoldBodyErase TouchHoldBodyRebuild \
  TouchHoldBodyMajority TouchHoldBodyAdapter TouchHoldRebuildAdapter TouchHoldScheduleAdapter \
  SchedulerCounterLocal SchedulerRemainingFields \
  StorageLocalRefinement TapSchedulePassLocal TouchScheduleLocal TouchSchedulePassLocal \
  SchedulerFrameFieldsAt SchedulerFrameContractsAt SchedulerFramePipelineAt \
  TapLifecycle TouchLifecycle HoldLifecycle SlideLifecycle \
  LifecycleJudgments HoldJudgments SlideJudgments SlideQueueMetrics SchedulerClicks \
  SchedulerFrame SchedulerParents SlideSchedule SlideScheduleParents \
  SchedulerFrameSlide SchedulerFrameFinalization SchedulerFrameState SlideScheduleJudgments SchedulerFrameJudgments TapScheduleModel QueueMaintenanceModel \
  TapSchedule QueueMaintenance QueueMaintenanceComposition HoldScheduleModel HoldSchedule StorageRefinement \
  TapSchedulePass SchedulerFrameTap TapScheduleJudgments TapSchedulePassJudgments HoldScheduleJudgments SchedulerFrameHold HoldScheduleQueueJudgments SchedulerFrameTraversals \
  SlideAreaEquality SlideQueueModel SlideQueue SlideQueueTraversal SlideQueueTrace \
  StorageSensorRefinement NormalizationModel ConnectedSlideMetadata ConnectedSlideLifecycle NormalizationPoints Normalization NormalizationFrontend TouchAutomaticModel TouchAutomatic TouchAutomaticJudgments TouchAutomaticQueue TouchAutomaticGroups TouchScheduleModel TouchSchedule TouchScheduleJudgments TouchSchedulePass TouchSchedulePassJudgments SchedulerFrameTouch TouchHoldScheduleModel TouchHoldGroups TouchHoldSchedule TouchHoldScheduleJudgments TouchHoldScheduleTraversal TouchHoldScheduleQueueJudgments SchedulerFrameTouchJudgments SchedulerFrameTouchTraversals \
  SchedulerFrameTouchHold SchedulerFrameAllTraversals SchedulerFrameInitialization \
  FrameCommandsReference FrameCommandCollection FrameCommands FrameCommandFinalization FrameCommandState SchedulerParentFields SchedulerInitializationCapacity SchedulerFramePipeline SlideQueueReferenceTrace Audit; do
  run "proof-$module" "$repo/verification" lake build "Verification.$module"
done
run equivalence-proofs-and-axiom-audit "$repo/verification" lake build
run clippy "$here" cargo clippy --workspace --all-targets --locked -- -D warnings
run debug-tests "$here" cargo test --workspace --locked
run release-tests "$here" cargo test --workspace --release --locked
# Elaborating the full real-chart regressions in parallel exhausts memory on
# development machines. Build each expensive module before the aggregate.
for module in RuntimeJudgmentCounting KoishiOverlapTactic RealChartVerification11264 \
  RealChartVerification100524 RealChartVerification11358 \
  RealChartVerificationKoishiDISCO RealChartVerificationPandora \
  RealChartVerification7thSense '«RealChartVerification小石DISCO»'; do
  run "lean-$module" "$repo" lake build "Proofs.$module"
done
run lean-build "$repo" lake build LnmaiCore Proofs simai-parser-cli runtime-step-cli \
  real-chart-verification real-chart-verification-100524
run real-chart-acceptance "$repo" .lake/build/bin/real-chart-verification
run real-chart-100524 "$repo" .lake/build/bin/real-chart-verification-100524
run differential "$repo" bash lnmai-core-rs/tools/run_differential.sh --release
run native-c-abi "$repo" bash lnmai-core-rs/tools/run_native_smoke.sh
run whitespace "$repo" git diff --check
run source-manifest-final "$repo" source_manifest
run unchanged-sources "$repo" cmp "$logs/source-manifest.log" "$logs/source-manifest-final.log"
echo "All validation stages passed. Logs: $logs"
echo 'Formal coverage and remaining obligations are recorded in verification/SPEC_MAPPING.md.'
