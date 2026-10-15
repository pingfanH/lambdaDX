# Native FFI、seek 与 autoplay 所有权

## 播放器语义

- 时间轴拖动和提交共用 `seek_runtime_to`，调用核心
  `Session::seek_with_events(target_us, events, keep_held)`。
- seek 开始新的练习片段：清除判定、combo、评分及旧视觉反馈。
  分母及最大 DX 分数只统计目标时间后仍需结算的 note，不把已跳过 note 判为 Miss。
- 保留尾部跨越目标的 hold/touch-hold 和 slide；连续 slide 保留整条 parent
  所有权链。过去的星星头和已结束的 arc 不重复计分。恰好结束于目标时仍保留。
- 复用加载时缓存的不可变初始运行状态、输入计划和原始 runtime note ID，
  不重新解析、降级谱面或构造整张谱面的判定队列/几何，也不更换 session。
  普通 note 的队列游标与共享 frontier 按目标时间直接定位；仅用临时局部运行视图
  恢复跨目标长音符、连续 slide 的父子链及其头部判定窗口内的竞争 note。
  目标之前的输入保持状态通过轻量扫描恢复，不对无关历史反复运行整张谱面的
  scheduler。开销仍与谱面/输入大小及跨目标长音符的恢复区间有关，并非常数时间。
  保留判定配置；向后跳转必须 seek，普通 step 拒绝倒退。
- 拖动时恢复核心进度和 autoplay 保持输入/近期触摸反馈，不发送历史判定或音效；
  松手才重启音频。直传模式使用原始事件，模拟触摸模式使用归一化传感器事件。
  暂停、恢复和变速不清评分。失败的 seek 不提交核心/播放时钟变更。
- 结束时间已过的 slide 在 seek 时进入终止状态，清空其检查点并隐藏轨道，
  避免延迟判定在下一帧污染已重置的评分；保留索引以维护连续 slide 的连接。
- 恢复期间同一 slide/track 的重复渲染命令合并，只返回最终进度与隐藏边界，
  不返回历史判定反馈。目标时刻的事件与恰好到达的 Hold 尾部留给恢复后的帧结算。

目前这些能力由默认 `backend-lean` 提供；Rust 后端显式返回不支持 seek/
分组生成的错误，不能保证该语义。

## 分组缓存（version 1）

缓存仍是 JSON 文件，但**运行时 Rust ↔ Lean 通信不使用 JSON**。

`AutoplayPlan { version, chartFingerprint, groups }`：

- `groups[].id`：普通 note 的 runtime note ID，或连续 slide 的根 ID。
- `runtimeNoteIndices`：此组包含的全部核心 ID（含 slide head、各 arc）。
- `parts`：每个 note/head/body 的事件；body 可按 wifi track 再分区。
  `role` 为普通 note=0、slide head=1、slide body=2。
- 每个 part 保存完整按下/移动/释放事件，不从扁平事件时间反推所有权。
- `plan.group_for_note(id)` 找到整组；该组所有 `parts[].events`
  即该 slide 或 note 的完整输入。
- `plan.events()` 按时间稳定合并；同时间保持 group/part/event 顺序。
- `plan.retained(ids)` 整组保留/删除，递归移除失去依赖的组。
- 加载缓存时验证版本、原谱面文本+难度指纹、所有权、组覆盖和依赖。

生成器当前禁用跨 note 的整 arc/分支 skip 优化，以避免删掉早期组后
后期组失去所依赖的输入；以后启用时须显式记录依赖。
旧裸事件数组仍可加载；首次非零 seek 切换到核心默认所有权计划。

## ABI 3

声明：`lnmai-core/native/lnmai_native.h`；C 隔离 Lean 对象布局，Rust
通过 `src/native.rs` 的 RAII 结果对象解码。

- 固定宽度整数，时间为有符号 i64 微秒；输入仅在调用期间借用。
- `lnmai_v3_call` 的 operation：0 create、1 load、2 step、3 seek、
  4 autoplay plan、5 chart、6 slide bindings、7 free handle。
- seek 使用 `time_us` 传目标、`events/event_count` 传排序事件、`level_index`
  传 `keep_held`（0/1）；空事件使用核心默认计划。输出含用于恢复进度的 render records。
- 输出为 tagged records（tag + 8 个 u64），有符号量使用补码。
- 检查 ABI version、struct size、保留字段、指针和输入枚举；负 seek 拒绝。
- 输出分配完成后才提交状态；结果必须用 `lnmai_v3_free_result` 释放。
- Lean registry 互斥锁保证会话更新串行化；兼容 JSON 接口只用于调试/比较。
- Rust build 自动解析 Lean toolchain headers 并编译 C bridge。
  外部 Lean 工程独立构建时也须向 C 编译器提供 Lean include 路径
  （macOS 还需要 SDK 和 Clang resource headers）。

## 验证

`cargo test -p lnmai-core --test native_seek_test` 比较 native/JSON
的完整 step 结果，并覆盖清零、重复重播、倒退拒绝、剩余分母、跨目标
长音符保留、连续 slide 与 wifi 所有权。
`cargo test -p lnmai-core --test seek_crossing_test` 覆盖跨目标长音符的
autoplay 续播和评分、session 身份保留，以及已结束 slide 的延迟判定隔离。
`cargo test -p lnmai-core --test seek_restore_test` 对照正常顺播的跨目标
Hold/TouchHold/Slide/连续 Slide/Wifi 状态，并覆盖恰好落在 Hold 尾部的 seek。
`cargo test -p lnmai-core --test seek_performance_test -- --ignored --nocapture --test-threads=1`
检查 1280 个 runtime note 的密集谱面反复前后 seek，防止完整历史重放造成秒级阻塞。
`cargo test --bin lambda_dx_pad_preview dense_scrubbing_stays_interactive_in_both_autoplay_modes -- --ignored --nocapture --test-threads=1`
覆盖播放器实际 `scrub_to` 路径，分别检查模拟触摸及直传核心模式的拖动耗时。
