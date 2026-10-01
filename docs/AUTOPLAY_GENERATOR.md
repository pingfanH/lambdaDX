# 想法：按游戏机制生成「理论上必然 Perfect」的 autoplay

> 状态：**想法 / 待验证**。本文只记录设想与约束，不含实现。

## 动机

当前 autoplay 直接使用 lnmai-core 的 `default_tactic`
（`player::engine::JudgeEngine::default_tactic` → `player::autoplay::tick`）。
它在多数情况下可用，但对某些滑条会出现「默认 autoplay 做不到、谱面本身却无误」的情况：

- 密集 `^` 链（每段弧短于一帧，例如 `7^2^3…>6[8:1]`）；
- 普通滑条的判定受**时间相位**影响（同一 star 在 `サイエンス1` Miss、在 `サイエンス2` Perfect，仅差一拍）。

结论：默认 tactic 的采样密度/时机并不保证与内核判定窗口完全吻合。
我们需要一个**理论上必然全 Perfect** 的 autoplay 生成器，作为：

1. 判定正确性的**参照基准**；
2. 判断「问题在内核/步进/窗口，而非谱面」的依据。

## 术语（按游戏机制）

| 术语 | 含义 |
|---|---|
| 星星 / slide | 一条滑条 note |
| 生命周期 | 一个 slide 从**出生**（head 可被触发）到**判定结束**（尾判定完成）的全过程 |
| 判定区块 block | 一条（或一组重叠的）slide 在时间上切出的判定单元 |
| 分区 segment | slide 路径上的判定段（A/B/C 等 sensor 区） |
| 时间块 time block | 把一个 block 的可用总时长固定切分，如总时长 10s、每块 0.1s |
| 重叠单位 unit | 同一时刻重叠在一起的多条 slide，作为一个枚举单位联合处理 |
| ex / 保护套 | 带 ex 标记的 note；非 ex（普通）tap/hold 的 great/good 窗口更紧 |

## 目标

- 输入：谱面（含 slide 的全部区块/分区时间信息）。
- 输出：一串带时间戳的输入事件（autoplay tactic），使**每个 slide、每个区块都 Perfect**。
- 约束：区块按时间顺序触发；同一重叠单位内联合枚举，互不破坏 Perfect。

## 枚举模型

- 每条 slide 的总时长切成固定大小的时间块（**统一换算成秒**计算，例：总时长 10s、每块 0.1s）。
  总时长、已占用时间、剩余时间都以秒为单位参与运算。
- 每个 slide 的判定区块**按顺序**触发。
- 枚举当前 slide 时：
  - 下一个区块的可枚举时间 = **总时长 − 前面所有区块占用的时间总和**；
  - 当前面的枚举导致剩余区块**可分配的时间块数量 ≤ 1** 时：
    - 还剩 1 个时间块 ⇒ 剩下所有区块在**这一个时间块内一起划完**；
    - 没有剩余时间块 ⇒ 剩下区块**直接跟上一个区块一起划完**。
- 终止条件：当某个区块的启动时机能让**该区块内的 slide 全部 Perfect**，该区块视为结束，继续下一区块。

伪代码：

```text
for unit in overlapping_units:          # 重叠单位
    time_blocks = split(total_duration, block = 0.1s)
    free = len(time_blocks)
    for slide in unit.slides:
        for block in slide.blocks_in_order:
            remaining = free - sum(consumed_blocks)
            if remaining <= 1:
                assign(all_remaining_blocks, time_blocks[last])   # 或并入上一个
                break
            t = pick_start_time(block, time_blocks)
            if all_perfect(block, t):
                consumed_blocks += block.span
            else:
                backtrack_or_adjust(...)                          # 重新枚举
```

## 补充约束

枚举 slide 时还需额外识别：

> **区块内某条 slide 的 A 区区块的触发时间，是否与区块内某个非 ex 的 tap/hold 的 great/good 判定时间重合。**

- 若重合，可能把该 tap/hold 从 perfect 拉成 great/good，或反过来干扰 slide 判定；
- 枚举时应把这类时刻视为**禁放窗口 / 冲突窗口**，避开或延后 A 区区块的触发。

**ex 的定义**：只要判定落在**除 miss 以外的任意判定区间**，就直接算 perfect。
因此与 ex note 的时间重合不会造成降级；冲突只发生在**非 ex 的 normal tap/hold** 上，
所以枚举时只需排除「非 ex」的 great/good 窗口。

### 回溯与复杂度

给某个区块选定起始时间后，如果它无法让区块内 slide 全 Perfect，就要换这个区块的其它候选时间；
若本区块所有候选都不行，则回退到**上一个区块**改时间再往下重试——这个过程就是**回溯**。

- 候选空间 = 每个区块可选时间块数的乘积，最坏是 `∏(每区块候选数)`，即**指数级**。
- 因此需要剪枝：先枚举约束最紧（可用窗口最小、与其它 note 冲突最多）的区块；
  候选窗口预先按判定窗口裁好；一旦发现冲突立刻回退，不留到后面。

## 修改建议（针对当前实现）

1. **步进粒度**：`player::engine::step_judge_engine` 现在是「帧内事件跨度 > 5ms 才逐事件」的混合策略
   （`FRAME_INPUT_BATCH_SPAN_US`，可用 `MAI2_DEBUG_FORCE_BATCH` / `MAI2_DEBUG_FORCE_PEREVENT` 覆盖）。
   严格枚举生成器更希望内核**按事件时间精确步进**，或至少暴露一个稳定模式。
2. **判定窗口**：需要内核暴露「每个 slide 每个分区」的判定窗口（起止时间），枚举器才能判断能否 Perfect。
3. **重叠关系**：需要谱面/内核给出 slide 的重叠关系（同一时刻的多 slide 归为一个 unit）。
4. **ex 冲突窗口**：需要 tap/hold 的 great/good 窗口信息，用于上述冲突检测。
5. **定位分工**：可把现有 `default_tactic` 视为「快速近似」，新生成器作为「理论基准 / 校验」，
   两者对不上的地方即需要排查内核或步进。

## 未决问题

- 时间块粒度（0.1s）与 BPM 的关系：是否应按拍子而非固定秒数切分（但计算仍换算成秒）。
- 回溯枚举的剪枝策略与可接受的最坏复杂度。

## 已确认

- **「一起划完」内核支持**：同一时间块内触发多个分区是可以的，剩余区块可直接在最后一个
  （或上一个）时间块里一起划完。
- **ex**：判定落在除 miss 以外的任意区间都算 perfect，故 ex 的时间重合无需规避；
  只有非 ex 的 normal tap/hold 的 great/good 窗口需要避开。
</content>
