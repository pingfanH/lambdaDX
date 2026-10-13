# Native FFI、seek 与 autoplay 所有权

## 播放器语义

- 所有时间轴提交统一调用 `PadPreviewState::seek_to`，再调用核心 `Session::seek(target_us)`。
- seek 开始新的练习片段：清除判定、combo、评分、输入保持状态和视觉反馈。
  分母及最大 DX 分数只统计保留 note，不把已跳过 note 判为 Miss。
- note 头时刻早于目标时整条跳过，包括跨目标的 hold/touch-hold；连续 slide
  以 parent 链根为单位整组跳过。目标等于头时刻时保留。
- 使用加载时缓存的完整谱面重建，保留判定配置和原始 runtime note ID。
  向后跳转必须 seek，普通 step 拒绝倒退。
- 拖动只预览，不步进核心或 autoplay；松手只提交一次。
  暂停、恢复和变速不清评分。失败的 seek 不提交核心/播放时钟变更。

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
整组删除、连续 slide 与 wifi 所有权。
