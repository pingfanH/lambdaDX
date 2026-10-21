# SDEZ170 C# oracle

This harness compiles the unmodified official NoteJudge.cs and NoteScore.cs
from JudgementCode with a one-method UnityEngine.Mathf shim, then recomputes
the golden vectors consumed by the Rust test.

Source hashes:

- NoteJudge.cs: 44514e6021dbed4d09d9ee612d0e14f3f01ea3bb6fbb7504cbdc298287ee3024
- NoteScore.cs: 6771662684770a5e3dfe137ac1b088c28bffb2be44c23ca220d4b9ff97f36b53

Run from the repository root:

    nix-shell -p dotnet-sdk_8 --run "dotnet run --project tools/sdez170_csharp_oracle -- lnmai-core/crates/lnmai-core/tests/golden/sdez170_vectors.json lnmai-core/crates/lnmai-core/tests/golden/sdez170_vectors.json"
