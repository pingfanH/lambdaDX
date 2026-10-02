# 会话总结：运行时修复 + autoplay_gen

> 本文是阶段性开发记录（不是设计文档）。autoplay_gen 的设计与设想见
> [`AUTOPLAY_GENERATOR.md`](./AUTOPLAY_GENERATOR.md)。

- 仓库：`/Users/pingfanh/project/lambdaDX`
- 分支：本地 `player_preview`（由 `master` 改名），上游 `origin/player_preview`
- 相关 bin：`lambda_dx_pad_preview`、`autoplay_gen`

## 一、player 运行时修复

### 滑条步进（hybrid stepping）

- 新增 `player::engine::step_engine_events`（`pub(crate)`），替代单纯按帧步进。
  - 一帧内事件跨度 ≤ `FRAME_INPUT_BATCH_SPAN_US`（**5ms**）→ 整批用 `step(now, events)`；
  - 跨度更大 → 逐事件在各自 `tp` 步进，最后再 `step(now, &[])`。
- 目的：密集 `^` 链（每段弧短于一帧）能逐事件判定，普通滑条仍保持批量帧步进。
- 回归测试：`sparse_and_dense_slides_both_complete`。
- 调试开关（env）：
  - `MAI2_DEBUG_FORCE_BATCH=1` 强制批量；
  - `MAI2_DEBUG_FORCE_PEREVENT=1` 强制逐事件。
- 相关提交：`0cb6882 Judge sub-frame slide arcs per event, batch ordinary frames`、
  `835d988 Add debug overrides for slide frame stepping`。

### 其它运行时修复

- **暂停语义**：修正暂停/恢复时的时间推进与输入处理。
- **重启**：修正重启后引擎步进状态（重新从谱面起点推进）。
- **wifi 轨道进度**：进度改取「最小已行进距离」，避免多指先后导致进度跳动。
- **低速音频**：改用 rodio `Source::speed` 做变速，低倍速时不至于异常。

### 测试谱面 / 资源

- 新增 `assets/charts/サイエンス1/`、`サイエンス2/` 与测试资源
  （`8b2fbba Add サイエンス test songs and update test chart/UI assets`）。
- `assets/charts/test/maidata.txt` 后续有改动（未提交，见 `git status`）。

> 说明：以上运行时条目部分来自本会话改动，部分来自相邻会话；提交 hash 只标注了
> 能直接对应到的滑条步进两条。

## 二、autoplay_gen

### 目标

生成「理论上必然全 Perfect」的 autoplay tactic，作为判定正确性的参照基准，
并交给 pad preview 显示。完整设想、术语、枚举模型见
[`AUTOPLAY_GENERATOR.md`](./AUTOPLAY_GENERATOR.md)。

### 关键结论

- **事件模型是瓶颈**：每条 segment 只 hold 一个 zone 无法驱动一条 runtime arc；
  必须按 arc 的**完整 zone 序列**生成事件（`verify::build_events_from_spec`）。
- 内核判定受**时间相位**影响：同一 star 在不同谱面/拍点下可能 Perfect 或
  Miss / FastGreat。
- `all_perfect()` 原先只看 Miss（假阳性），已收紧为「每个已判定 arc 都属于 Perfect 家族」。

### 已实现

- `src/autoplay_gen/`：`model.rs`（SlidePlan）、`planner.rs`（分块 + 重叠单位）、
  `report.rs`（调度 + A 区冲突）、`verify.rs`（事件生成 + 内核重放）、`search.rs`（枚举）。
- `player::engine::chart_spec()` 暴露 lowered chart：`slides[].judge_queues`、
  `start_timing`、`length`、`judge_at`、`logical_slide_id`、`slide_heads[].timing/slot`。
- `--spec`：按完整 zone 序列生成事件；对 `test/`、`サイエンス2/` 均 `misses: 0`。
- `--default-tactic`：内核自带 tactic 走同一验证链路。
- `search.rs` baseline-first 局部化枚举（贪心降 `bad()`）：
  仅重定时失败 + 重叠的 slide；`MAI2_DEBUG_FORCE_BATCH=1` 下曾修复一个失败谱面
  （`work slides: [0]`、`arcs re-timed: 1`、全非 Miss）。
- **缓存 + 预览链路**：验证通过后写
  `out/autoplay_gen/<title>_lv<level>_<hash>.json`（`<hash>` 为谱面路径短哈希），
  并可用 `--autoplay-tactic <file>` / env `MAI2_AUTOPLAY_TACTIC` 让 preview **原样重放**。
- `VerifyResult.grades` + `print_verify` 打印每个 arc 的判定等级。

### 验证观察（autotest）

`assets/charts/autotest/maidata.txt = (210){8},,1h[4:1],,,,1>4[4:1],,1>4[4:1]`

- 结果：`arcs 2 judged 2 misses 0 non-perfect 1`，`grades: 0:Perfect 1:FastGreat`。
- 内核默认 tactic 也得到 arc1 `FastGreat`；`FORCE_PEREVENT` 不变；
  均匀偏移（±48ms @2ms）不变。
- 推断：需要 `fast` 模式（rush 早期区、最后区落在 `judge_at`）+ 更细的每条 track
  间隔枚举，而非只平移整条 arc。

### 本轮修复（autotest 已全 Perfect）

`search.rs` 改用 `Vec<ArcTiming>` 并枚举 `{offset × fast}`。定位并修复两个根因
（详见 [`AUTOPLAY_GENERATOR.md` 的「根因与修复」](./AUTOPLAY_GENERATOR.md#根因与修复autotest)）：

1. **末区 hold 零长度**：末区 down 与 up 同一时间戳被同帧批处理抵消，
   `wasOn` 不成立 → queue 不清 → `LateGood`。改为末区按住至少一帧。
2. **重叠同形滑条的前置消费**：后一条从 `headTiming-50ms` 起 checkable，
   被前一条相同手势顺带消费、提前判定。策略：整体后移前一条，使共享完成时刻
   落在两条 `judgeAt` 中点（差分各 ±半拍，仍在 14 帧 perfect 窗口内）。

结果：`autotest` → `all judged arcs Perfect`，缓存 `out/autoplay_gen/TEST_lv2.json`；
`test/`、`サイエンス1/2/` 无回归（misses: 0，已判定 arc 全 Perfect）。

缓存键已加入谱面路径哈希，`サイエンス1/2` 不再互相覆盖。待办见
[`AUTOPLAY_GENERATOR.md` 的「接下来的任务」](./AUTOPLAY_GENERATOR.md#接下来的任务)：
A 区与非 ex note 冲突窗口、`fast`/track 间隔、回归测试。

## 三、Git 状态（截至本记录）

- 已提交：`1f96c6f`（缓存 tactic + preview 文件驱动）、`1976dc9`（等级诊断 + head 偏移）
  及更早的 autoplay_gen / player_ui 系列提交。
- 未提交改动包含：`assets/charts/test/maidata.txt`、`assets/player_ui/` 多份 XML、
  `src/autoplay_gen/verify.rs`、`src/player_ui/*`、`tools/make_player_ui_xfl.py`；
  未跟踪：`assets/charts/autotest/`、`assets/RECOVER_player_ui/`。
- 注意：有时会遇到 `macroanimate/src/xfl/mod.rs` 的并发改动导致 `cargo test` 临时编译失败，
  与 autoplay_gen 无关；`cargo build --bin autoplay_gen --bin lambda_dx_pad_preview` 正常。
