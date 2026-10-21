# 判定核心重写计划

基线：SDEZ170 官游实现（/home/pingfanh/project/JudgementCode）为唯一权威。

目标：交付一个不依赖 Lean 运行时的 Rust 判定核心，行为按官游源码逐条对齐。

## 1. 结论

现有 Lean 与 Rust 判定层不是 SDEZ170 语义。`LnmaiCore/Judge.lean` 头部明确写着按
MajdataPlay 的 `NoteDrop.Judge`、`TouchDrop.Judge`、`SlideBase.Judge` 转写；
Rust 的 `crates/lnmai-core/src/judge.rs` 又逐行镜像了这个 Lean 版本。两者内部一致，
但一致的是一套非官游规则。

因此不能只“参考 JudgementCode 修正几个阈值”。判定子系统需要按官游重写，
包括时间基准、15 档判定表、Hold 松手统计、Touch 输入占用、Slide 路径推进、
同帧处理顺序和计分累计。解析器、谱面 IR、会话管理、JSON/FFI、seek 框架可以保留。

“当前 Lean 版没有问题”只能理解为它在旧规格内自洽；以官游为绝对权威时，
它不能作为正确性基线，现有证明也不能证明官游行为。

建议的最终形态：

- Rust 成为唯一运行时权威，直接供 lambdaDX 使用。
- Lean 版本保留为历史实现/差分参考，不再作为运行时依赖。
- 不把官游语义先移植回 Lean 再证明；那样会把工作量翻倍，而且现有 Lean 证明绑定的是旧语义。

## 2. 官游基线中的关键事实

这些是移植时最容易“想当然简化”的地方，必须原样保留。

### 2.1 两套时间偏移

官游同时存在两个独立偏移：

- 谱面偏移 `AdjustTiming`：
  `GetAdjustMSec() = (AdjustTiming - 20 + 36) / 10 * 16.666666f`。
  默认 `AdjustTiming = Normal = 20`，结果为约 `60ms`。
  `NotesReader.calcBPMList()` 用这个值作为 BPM 时间轴起点，所以所有音符时间
  默认整体后移约 60ms。
- 判定偏移 `JudgeTiming`：
  `GetJudgeTimingFrame() = (JudgeTiming - 20) / 10f`。
  判定时减去 `JudgeTimingFrame * 16.666666f`。

`NoteJudge.JudgeAdjustMs` 固定为 `50f`，并加到 6x15 判定表的每一项上。
`GetJudgeTiming` 对 `SlideOut` 先加 50ms，返回前再从引用参数中减 50ms。

结论：不能把 50ms 和 60ms 相互抵消后写死成“零中心窗口”。必须保留
`AppearMsec`、`JudgeTimingDiffMsec`、`JudgeAdjustMs` 的原始含义，否则显示时差、
边界归属和 Slide 的 `lastWaitTime` 都会偏。

### 2.2 15 档判定表与比较方向

官游 `NoteJudge` 是 6 张 15 项表：

| 类型 | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Tap | -9 | -6 | -5 | -4 | -3 | -2 | -1 | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 9 |
| HoldOut | -9 | -6 | -5 | -4 | -3 | -2 | -1 | 0 | 1 | 4 | 8 | 9 | 10 | 11 | 14 |
| SlideOut | -36 | -26 | -22 | -18 | -14 | -14 | -14 | 0 | 14 | 14 | 14 | 16 | 22 | 26 | 36 |
| Touch | -9 | -9 | -9 | -9 | -9 | -9 | -9 | 0 | 9 | 10.5 | 12 | 13 | 14 | 15 | 18 |
| ExTap | -9 | -9 | -9 | -9 | -9 | -9 | -9 | 0 | 9 | 9 | 9 | 9 | 9 | 9 | 9 |
| Break | -9 | -6 | -5 | -4 | -3 | -2 | -1 | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 9 |

每项实际值为 `系数 * 16.666666f + 50f`。比较规则：

- 早侧使用严格 `<`。
- 晚侧使用 `<=`。
- 索引 7 不单独作为边界。
- `Critical` 条件是 `> 表[6]` 且 `<= 表[8]`。
- `TooFast` 是 `< 表[0]`，`TooLate` 是 `> 表[14]`。
- `GetJudgeTiming` 返回的是 15 档 `ETiming`，不存在 `End`。

EX 音符不是“直接 Perfect”。`TapNote`/`StarNote` 在 EX 时使用 `ExTap` 表，
其宽窗口表现为几乎总是 `Critical`；`Break` 的 EX 同样使用 `ExTap`。

### 2.3 各音符的时差来源

- Tap / Break / Star / Hold 头 / Touch：
  `diff = NotesManager.GetCurrentMsec() - AppearMsec`。
- Slide：
  `diff = NotesManager.GetCurrentMsec() - TailMsec + lastWaitTime`。

Slide 的时差不是从音符头时间算，而是从尾时间加当前等待时间算。这里必须按
`SlideRoot.Judge()` 原样移植。

### 2.4 Slide 的特殊处理

`GetSlideJudgeTiming` 先算 `num = lastWaitTime / 4f`，把
`SlideOut` 表的索引 6/8 边界向两侧各扩展 `num`：

- 扩展区间内直接返回 `Critical`。
- 该分支直接返回，不对引用参数减 50ms。
- 区间外回落到 `GetJudgeTiming(..., SlideOut)`，此时才走正常加减 50ms 的路径。

`SlideRoot` 在判定为 `TooFast` 后会把结果改写成 `FastGood`。
超时时如果最后一个判定区已被触及，`TooLate` 会改写成 `LateGood`，
否则保持 Miss。`SlideFan` 的三路触摸有各自的一套判定区与超时处理。

### 2.5 Hold 的松手统计

`JudgeHoldTotal` 用“有效区间内的松手时间比例”判等级：

- 普通 Hold：头忽略 `50 + 50 = 100ms`，尾忽略 `150 + 50 = 200ms`。
- Touch Hold：头忽略 `200 + 50 = 250ms`，尾忽略 `150 + 50 = 200ms`。
- `holdTime = TailMsec - AppearMsec`。
- 若 `holdTime <= headIgnore + tailIgnore`，直接返回头部等级。
- 否则 `holdTime -= headIgnore + tailIgnore`。
- `bodyOn == false` 直接取 5x15 表的第 5 行。
- 否则 `percent = ceil(holdReleaseTime / holdTime * 100)`，上限 100，
  按 `[0, 33, 67, 95, 100]` 找到第一个 `>= percent` 的档位。

`HoldReleaseTime` 的累计依赖 `PushList[4]` 的四帧历史和
`HoldReleaseCorrectTime` 的补回逻辑。当前 Lean/Rust 的 5x15 分档表与官游基本同构，
但松手累计只有 2 帧 grace + 简单累加，没有官游的四帧历史和补回；这是 Hold 的主要差异。

### 2.6 Touch 与输入

官游 Touch 没有“过早直接忽略”的分支。只要 `IsNoteCheckTimeStart` 通过，
按下就会调用 `GetJudgeTiming(Touch)`；过早结果是 `TooFast`（Miss），
并且会占用本帧输入。

触摸面板不是简单的“每帧按下/抬起”：

- `GameInputLog` 保存 Now/Last/Old 三帧位图。
- `TriggerOn` 是 `!Last && Now`，并且只在新硬件帧生效。
- `KillOnOffOn` 会抑制 `Old && !Last && Now` 的 on-off-on 抖动。
- `InputManager.GameInputFrameOnceInput[35]` 记录本帧已被哪个判定占用。
- 中心区 C 对应 `C1`/`C2` 两个区域，判定成功时两者都置占用。

这些规则决定了同帧多个 Touch、Touch 与 Slide、Touch 与 Hold 的竞争结果。

### 2.7 同帧顺序

`GameCtrl.UpdateNotes()` 先遍历活动普通音符/长条/触摸，再遍历活动 Slide。
每个列表内部按注册顺序执行，而不是按全局时间重新排序。
`GameScoreList.SetResult` 用 `NoteData.isJudged` 做去重。

`IsUsedThisFrame` 的置位发生在判定过程中，所以同一帧的前一个音符可以“抢走”
后一个音符的输入。这个顺序必须作为可测试的规范固定下来。

### 2.8 计分

官游用 `NoteScore.GetJudgeScore` 的 6 张 15 项分数表，再加
`JudgeResultSt.UpdateScore` 与 `GameScoreList.CalcUpdate`：

- Break 的 base 与 BreakBonus 分开累计。
- `AchivementIgnoreBreak` 按 `_allPerfectScore` 归一。
- Break 额外分按 `_breakBonusScore` 归一。
- DX 分按 `Critical=3, Perfect=2, Great=1, Good/Miss=0`。
- 达成率使用 C# `decimal`，最后才做显示舍入。
- `DispJudge` 只影响 Critical/Perfect 的显示合并与 FAST/LATE 统计，不影响原始判定。

当前 Rust 的逐事件 fold 逻辑在多数情况下能得到相同总分，但它不是官游的
“先存每音符结果，再整体 CalcUpdate”模型。为了保证显示、断线和重算一致，
建议改成官游的每音符结果表 + 整体重算。

## 3. 现状评估

现有实现的优点是工程结构完整：解析、IR、加载、会话、JSON、C ABI、seek 都在，
Rust 版也有差分测试和证明基础设施。问题集中在判定语义层。

差距矩阵：

| 维度 | 当前 Lean/Rust | SDEZ170 | 处理 |
| --- | --- | --- | --- |
| 判定表 | 按音符类型拆成对称窗口 | 6x15 表，索引 7 空置，早晚比较方向不同 | 重写 |
| 时间基准 | 时差以 0 为中心 | 谱面默认 +60ms，判定表再 +50ms | 重写 |
| EX | `isEX` 直接 Perfect | 使用 ExTap 表，宽 Critical | 重写 |
| Touch | 过早返回 None | 窗口内按下必落 15 档表，窗口外由 start 检查挡住 | 重写 |
| Slide | 动态扩展 + classic 模式 | `lastWaitTime/4` 扩展，尾时间基准 | 重写 |
| Hold | 5x15 表近似，2 帧 grace 简单累加 | `JudgeHoldTotal` + 4 帧松手历史 + CorrectTime 补回 | 重写 |
| Classic | 有 classic slide/hold 分支 | SDEZ170 没有对应分支 | 默认移除，旧实现隔离 |
| 输入 | 时间戳事件、click/hold | 60Hz 面板位图、三帧日志、去抖、本帧占用 | 重写 |
| 触摸组 | 严格多数 | `ceil(count * 0.51)`，EndCount/ChainJudge | 重写 |
| 调度 | 队列/前沿模型 | 活动列表顺序，普通音符先于 Slide | 重写 |
| 计分 | 逐事件 fold | 每音符结果表 + CalcUpdate | 重写/校验 |
| 等级枚举 | 反向 15 档 `JudgeGrade` | 官游顺序 `ETiming` | 新增 `ETiming`，保留兼容映射 |
| Slide 几何 | 简化判定区队列 | SlideManager HitArea/PushDistance/ReleaseDistance | 移植表数据 |
| 解析/IR/会话/FFI | 完整 | 不涉及 | 保留 |

结论是“底层结构可用，判定实现需重写”。不需要推翻整个项目，但需要把
`judge`、`lifecycle`、`scheduler`、`input_model`、`runtime_score` 视为替换对象。

## 4. 目标架构

### 4.1 模块划分

在 `lnmai-core-rs` 中新增 SDEZ 语义模块，旧模块保留在 `legacy` feature 下：

- `sdez::timing`：`AdjustTiming`、`JudgeTiming`、`GetAdjustMSec`、帧时钟。
- `sdez::judge`：6x15 表、`GetJudgeTiming`、`GetSlideJudgeTiming`、`ConvertJudge`、`JudgeHoldTotal`。
- `sdez::input`：35 位面板状态、三帧日志、去抖、按钮 down/push、本帧占用。
- `sdez::note`：Tap/Break/Star/EX、Hold/BreakHold、Touch/TouchHold。
- `sdez::slide`：SlideRoot/SlideFan、HitArea 推进、`lastWaitTime`。
- `sdez::score`：`JudgeResultSt`、`CalcUpdate`、BreakBonus、DX、达成率。
- `sdez::runtime`：活动列表、注册顺序、同帧调度、自动游玩。

FFI/JSON 继续放在 `lnmai-core`，只做类型适配，不把判定规则写进 FFI 层。

### 4.2 数值模型

官游使用 `float`（f32）做判定时间，`double` 累计 Hold 松手时间，`decimal` 做达成率。
为了边界一致，建议：

- 判定时间、音符时间、阈值、`lastWaitTime`：`f32`。
- 帧计数器：`u64`。
- `HoldReleaseTime`/`HoldReleaseCorrectTime`：`f64`，传给 `JudgeHoldTotal` 时转 `f32`。
- 分数与计数：`u32`/`u64`。
- 达成率：`i128` 定点（建议 1e-6 或 1e-9 精度），只在显示时舍入；不使用 f64 累计。
- 谱面解析仍可保留有理数，在“生成运行时音符时间”这一步一次性转 f32，并加 `GetAdjustMSec`。

不要继续在判定热路径使用微秒整数或有理数。它会让“表值 * 16.666666f + 50f”的
舍入行为与官游不同，边界测试会对不上。

### 4.3 帧与输入模型

官游是帧驱动：三帧触摸日志、四帧松手历史、`IsUsedThisFrame` 都以帧为单位。
因此判定核心按 60Hz 逻辑帧推进，host 每帧提供：

- `current_time_ms: f32`（来自音频时钟，单调）
- `button_state: u16`（9 位）
- `panel_state: u64`（35 位）
- `panel_frame_id: u32`（触摸硬件帧号，用于复现 `SetNewTouchPanel`）

渲染可以继续按显示器刷新率插值；判定只在 60Hz 逻辑帧发生。
这样既保留官游语义，也避免 lambdaDX 的变帧率改变判定结果。

### 4.4 FFI 兼容

新增 `judgeMode = "sdez170"`，并新增可选字段：

- `adjustTiming`、`judgeTiming`
- `autoplayMode`（Critical/Perfect/Great/Good/Miss/Random）
- `buttonState`、`panelState`、`panelFrameId`
- `etiming`（官游 15 档）与 `judgeBox`（Miss/Good/Great/Perfect/Critical）
- `judgeDiffRaw`、`judgeDiffAdjusted`

旧字段 `grade` 继续输出，做兼容映射。C ABI 版本升到 4，旧客户端仍可读旧字段。

## 5. 分阶段计划

### Phase 0：冻结基线与官游 oracle（2-3 天）

- 记录 JudgementCode 的 commit、文件哈希。
- 用 .NET 建一个只包含 `NoteJudge.cs`、`NoteScore.cs` 与必要 `Mathf` stub 的控制台程序，
  直接执行官游纯函数。
- 生成黄金向量：
  - 6 种 `EJudgeType` x 时差扫描（-400ms 到 +400ms，步长 0.001ms 与随机浮点）。
  - `GetSlideJudgeTiming`：`lastWaitTime` 0 到 2000ms。
  - `JudgeHoldTotal`：长度、松手时间、bodyOn、TouchHold 的笛卡尔组合。
  - `NoteScore` 全部 6x15。
- 黄金向量提交为 JSON，测试不依赖 .NET 运行时。

验收：oracle 输出与手工计算的关键点一致；每条向量包含 `etiming` 和返回后的 `diff`。

### Phase 1：纯判定内核（3-4 天）

- 实现 `ETiming`、`JudgeBox`、6x15 表、`GetJudgeTiming`、`GetSlideJudgeTiming`、
  `ConvertJudge`、`JudgeHoldTotal`、`GetJudgeScore`。
- 逐位复现 f32 运算顺序，禁止编译器重排导致的差异。
- 边界测试覆盖每个阈值的 `t-ε`、`t`、`t+ε`。

验收：黄金向量 100% 一致；边界无“差一档”。

### Phase 2：时间轴与谱面偏移（2-3 天）

- 实现 `AdjustTiming`/`JudgeTiming` 设置与默认值。
- 在运行时音符生成阶段应用 `GetAdjustMSec`，只加一次，避免重复偏移。
- 实现 `IsNoteCheckTimeStart`、`JudgeToolate`、自动游玩触发点。
- 建立 60Hz 逻辑帧时钟与 host 时间输入。

验收：默认设置下，音符时间比原始谱面时间晚 60ms；判定显示时差符合官游 `ref` 语义。

### Phase 3：输入层（4-5 天）

- 35 位面板状态、三帧日志、`TriggerOn`/`StateOn`、`KillOnOffOn`。
- 按钮 down/push 区分、本帧占用位图、C1/C2 双区域。
- 把旧 `TimedInputBatch` 事件适配为帧状态；lambdaDX 触摸适配为面板位图。
- 测试 on-off-on 抖动、跨两帧按下、同帧竞争。

验收：输入真值表与官游 `GameInputLog`、`InputManager` 一致。

### Phase 4：音符生命周期（8-10 天）

按官游类逐个移植 `NoteCheck`/`Judge`/`EndNote`：

- Tap、Break、Star、BreakStar、EX。
- Hold、BreakHold，包括 `PushList[4]` 与 `HoldReleaseCorrectTime`。
- TouchB/C、TouchHoldC，包括触摸组与 C1/C2。
- SlideRoot、SlideFan、连接 Slide、`lastWaitTime` 递减、TooFast/TooLate 改写。

Slide 需要先完成 HitArea 表移植：

- 从 `SlideManager.cs` 提取 `GetSlideHitArea` 的 HitPoints、PushDistance、ReleaseDistance。
- 按 (SlideType, StartButton, EndButton) 生成静态表。
- 校验 `lastWaitTime = (arrive - shoot) * lastHitArea.PushDistance / totalDistance`。

验收：脚本化帧轨迹逐帧输出与官游一致；Hold 松手时间轨迹精确到帧。

### Phase 5：调度与同帧顺序（4-5 天）

- 活动普通音符列表先于活动 Slide 列表。
- 列表内按注册/谱面顺序执行。
- `isJudged` 去重、`IsUsedThisFrame` 置位、同帧多音符竞争。
- 自动游玩模式、Touch 链、Slide 父子结束传播。

验收：构造同帧多音符用例，判定归属与官游一致。

### Phase 6：计分、会话与 FFI（4-5 天）

- `JudgeResultSt` 每音符结果表、`SetResult`、`CalcUpdate`。
- BreakBonus、达成率、DX、Combo、FAST/LATE、Life、TrackSkip。
- JSON v4、C ABI v4、seek/replay 重建。
- lambdaDX 适配层切换。

验收：全音符类型、全档位计分与官游 decimal 结果在显示精度内一致。

### Phase 7：验证与切换（4-6 天）

- 纯函数黄金向量、合成帧轨迹、现有 48 谱面差分。
- 官游 AP 用例：自动 Critical 全连、无 Miss、所有音符完成。
- lambdaDX 以 `backend-sdez` 运行真实谱面，和旧后端做差异报告。
- 性能与确定性测试。

验收通过后，把默认后端切到 Rust/SDEZ，Lean 标记为 legacy。

总工作量：单人全职约 5-7 周；Slide 几何表提取是最大不确定项。

## 6. 验收标准

- 纯判定函数：黄金向量 100% 匹配，包括返回后的 `diff`。
- 每个阈值：`t-ε`、`t`、`t+ε` 的结果与官游一致。
- Hold：随机 10k 组长度/松手时间，`JudgeHoldTotal` 完全一致。
- Slide：随机 10k 组 `lastWaitTime`/命中轨迹，结果完全一致。
- 输入：三帧日志与去抖真值表全部通过。
- 运行时：同一输入轨迹产生同一输出哈希；Linux x86_64 与 aarch64 一致。
- 真实谱面：自动 Critical 模式下 AP、无 Miss、无未完成音符。
- 计分：各档位、BreakBonus、达成率、DX 与官游一致。
- 集成：lambdaDX 使用纯 Rust 后端可完整游玩，不需要 Lean 动态库。
- 性能：典型谱面 p99 每帧 < 1ms（目标，不作为正确性条件）。

## 7. 主要风险

1. **50ms/60ms 偏移被误简化**。这是最高风险项。必须保留官游的 `AppearMsec`、
   `JudgeAdjustMs`、`GetAdjustMSec` 全链路，并用显示时差测试锁定。
2. **f32 边界**。C# 与 Rust 都应是 IEEE-754 单精度；禁止在判定路径改用 f64，
   禁止 fast-math。所有表值按官游表达式计算。
3. **SlideManager 几何数据缺失**。当前核心没有 PushDistance/ReleaseDistance 表，
   这是官游 Slide 判定的必要条件，需要单独提取和验证。
4. **Touch 硬件帧语义**。官游只在新硬件帧更新触摸日志；如果 host 每渲染帧都送状态，
   会改变 on-off-on 抑制结果。需要 `panelFrameId` 或等价的“本帧有新数据”标志。
5. **同帧顺序**。普通音符先于 Slide、列表内按注册顺序，直接影响输入归属。
6. **计分精度**。C# decimal 与定点实现需要专门对齐，尤其是 BreakBonus 归一化。
7. **Lean 证明失效**。现有证明证明的是旧语义；在 SDEZ 语义下不能继续宣称等价。
   应明确标注为 legacy，不阻塞 Rust 交付。

## 8. 交付物

- `docs/SDEZ170_JUDGEMENT_SPEC.md`：逐函数规范与源码行号。
- `tests/golden/sdez170/*.json`：官游黄金向量。
- `crates/lnmai-core/src/sdez/*`：新的判定实现。
- `crates/lnmai-core/src/legacy/*`：旧实现隔离。
- `tools/judge-oracle/`：C# oracle 生成器。
- `docs/SDEZ170_DIFF_REPORT.md`：旧后端与新后端的差异分类。
- FFI v4 头文件与 JSON schema 文档。

## 9. 建议的第一批提交

1. 只加文档、C# oracle 和黄金向量，不改运行时代码。
2. 加 `sdez::timing` 与 `sdez::judge`，先用测试消费黄金向量。
3. 加 `sdez::input`，保持旧后端默认不变。
4. 加 `sdez::note` 的 Tap/Hold/Touch，逐类替换。
5. 完成 Slide 几何表后再接 `sdez::slide`。
6. 最后切换默认后端并删除 Lean 运行时依赖。

这样每个提交都能独立验证，不会出现“大爆炸式替换后无法定位差异”的局面。
