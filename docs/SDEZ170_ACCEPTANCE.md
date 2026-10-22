# SDEZ170 Rust 判定核心验收

分支：hdd

权威实现：/home/pingfanh/project/JudgementCode 的 SDEZ170 NoteJudge.cs、
NoteScore.cs、Monitor 目录和 SlideManager.cs。

## 后端选择

lambdaDX 默认使用旧 scheduler。只有显式设置 `MAI2_JUDGE_MODE=sdez` 时，
`JudgeEngine::load` 才会创建 SDEZ170 Rust 后端；未设置、设置为 `legacy`，
或使用其他值时都会使用旧 scheduler。
- 纯 Rust 构建不需要 Lean 动态库；lnmai-core 为 vendored Rust 核心，
  lnmai-core-lean 仅保留为旧实现参考。

## 已实现

- 官游 6x15 判定表、GetJudgeTiming、GetSlideJudgeTiming、ConvertJudge。
- AdjustTiming 默认 +60ms、JudgeAdjustMs 50ms、15 档 ETiming。
- Tap、Break、EX、Hold、BreakHold、Touch、TouchHold。
- SlideRoot、SlideFan/WiFi、连接 Slide 合并、HitArea 顺序推进。
- lastWaitTime 初始化/递减、TooFast -> FastGood、TooLate -> LateGood。
- 35 位触摸面板三帧日志、KillOnOffOn、本帧占用、C1/C2。
- Touch 链 ceil(count*0.51)、TouchHold 链。
- 普通音符按注册/时间顺序执行，Slide 在普通音符之后。
- SDEZ 分数表、BreakBonus、Combo、DX 分。
- 进程内 FFI、typed SdezSession、seek 重建快进、原生 autoplay。

## 权威源码哈希

- NoteJudge.cs: 44514e6021dbed4d09d9ee612d0e14f3f01ea3bb6fbb7504cbdc298287ee3024
- NoteScore.cs: 6771662684770a5e3dfe137ac1b088c28bffb2be44c23ca220d4b9ff97f36b53

## 验证结果

核心：

- cargo test -p lnmai_core
- 116 个单元测试通过。
- 官方 C# oracle 黄金向量 5670 判定 + 1200 Slide + 800 Hold + 全分数表通过。
- 三张真实谱面自动 Critical 全量完成：jack_ripper L5、pandora paraboxxx L6、
  credit L5。
- 真实谱面差分：旧 scheduler 与 SDEZ 的理论分/实得分/Combo 差异完全由
  SlideHead 被旧核心额外计为 Tap 解释。

lambdaDX：

- cargo test -p lambda_dx_pad_preview --bin lambda_dx_pad_preview
- 198 通过，1 个既有 effects 参考测试因缺少 Node.js bundle 忽略。
- 22 个 player::engine 测试全部通过。
- seek、autoplay、Wifi 三轨进度、Touch 位置、Hold 头尾、Slide 隐藏全部通过。

wrapper：

- cargo test -p lnmai-core-rust
- 3 个测试通过，包括 typed SdezSession roundtrip。

## 官方 C# oracle

tools/sdez170_csharp_oracle 直接编译未修改的官方 NoteJudge.cs 和 NoteScore.cs。
Program.cs 只提供 UnityEngine.Mathf.CeilToInt 的 shim，并重算黄金向量。

重新生成：

    nix-shell -p dotnet-sdk_8 --run "dotnet run --project tools/sdez170_csharp_oracle -- lnmai-core/crates/lnmai-core/tests/golden/sdez170_vectors.json lnmai-core/crates/lnmai-core/tests/golden/sdez170_vectors.json"

## Simai 到 SlideType 的桥接

JudgementCode 是 MA2/SDEZ 谱面语义，Simai 是另一套记谱。当前映射：

- Line -> Straight
- Circle -> Circle L/R（方向由 symmetry 与起始槽决定）
- V -> Corner
- P/Q -> Curve L/R
- PP/QQ -> Skip L/R
- S/Z -> Thunder L/R
- Turn -> Bend L/R
- Wifi -> Fan

HitArea/PushDistance/ReleaseDistance 全部来自官方 SlideManager.cs 提取表，
由 tools/extract_slide_hit_areas.py 生成 slide_hit_areas.json。
