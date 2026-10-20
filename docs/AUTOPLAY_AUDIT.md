# Autoplay 完整回放审计

更新: 2026-10-20。

`autoplay_audit` 是无窗口验收工具, 使用播放器真实的加载、Autoplay 编译、输入派发、核心判定与 Slide 渲染状态更新。不会修改原始测试谱面, 不伪造分数、判定或正常进度终点。

## 运行

```sh
rtk cargo run --bin autoplay_audit -- '/Users/pingfanh/.maichart/324_Jack-the-Ripper◆' '/Users/pingfanh/.maichart/011951 サイエンス' '/Users/pingfanh/.maichart/011996 Usagi Flap' --diff 5 --matrix --out out/autoplay_audit/final
```

默认每份谱面只测最高难度; `--diff N` 指定 `&inote_N`, 不遍历其他难度。

- 默认: 屏幕传感器输入、60 FPS、更新 Pad 输入反馈。
- `--buttons`: 保留 K 按钮和 A 传感器的区分。
- `--direct`: 同样的输入直接送核心, 不生成 Pad 反馈。
- `--stalled`: 每 47 帧插入约 183 ms 的长帧。
- `--matrix`: 组合以上三个开关, 每份谱面执行 8 次。
- `--out DIR`: 指定完整记录目录; 所有场景通过才返回 0。

## 严格通过条件

1. 所有返回的判定必须恰好为 `JudgeGrade::Perfect`; Fast/Late Perfect 子等级也不通过。
2. 所有应判定音符必须返回结果, Hold/TouchHold 的 Head/Tail 各一次。连接 Slide 只有末段产生独立尾判定, 但每段都必须返回进度。
3. 每条运行时 Slide、WiFi 每条分支都必须返回协议终点并完成映射进度。超时 HideAll 不能代替缺失的正常进度终点。
4. 每条原始进度、条带隐藏、整体隐藏指令, 都核对运行时 ID、谱面音符/子 Slide/段、桥接输出和应用后的渲染状态。不能用最终隐藏掩盖中间丢失的更新。
5. 计划事件必须全部消费, 实际发出的输入数量必须和编译后的事件流一致。

WiFi 的 `UpdateSlideTrackProgress.remaining` 名字容易误导: 现代 WiFi 是递增的 0..9 共享游标, Classic 是 0..8, 不是剩余判定区数量。普通/连接 Slide 是递减的队列长度, 以 0 结束。WiFi 的分支条带由 `HideSlideTrackBars` 独立控制, 共享游标不能强行完成其他分支。

## 记录文件

每个场景保存独立目录:

| 文件 | 内容 |
| --- | --- |
| `source.txt` | 原始 maidata 快照 |
| `chart.json` / `spec.json` | 前端谱面和核心运行时谱面 |
| `plan.json` | 带完整归属的 Autoplay 计划 |
| `events.json` | 按所有者合并重叠持有后的完整输入流 |
| `inputs.jsonl` | 每个显示帧实际发出的输入, 包括空帧 |
| `frames.jsonl` | 每个核心子步的完整判定、音频/渲染指令、分数、映射进度与渲染状态 |
| `judgments.json` | 按运行时音符 ID 汇集的全部判定 |
| `summary.json` | 统计、核对的进度指令数与所有失败原因 |

根目录 `index.json` 汇总全部场景及其记录路径。完整记录较大, 保存在 Git 忽略的 `out/` 中, 不提交个人谱面快照。

## 本轮修复

- 精确事件时间不再经 `f32` 秒往返; 同一微秒的 release/click/down 一批提交, 避免点击落到核心窗口以外。
- 按 part 保留持有归属, 编译为物理输入并集; Slide 释放不再打断同区 Hold。编辑器计划和 seek 后的保留计划使用同一编译器。
- 已有完整计划不再按显示帧额外合成传感器 Hold, 避免干扰 Slide 判定区遍历。
- 区分 WiFi 共享游标和独立分支前沿; 普通 Slide 的精确 HideSlideBars 也完整传到渲染层。
- 后段进度不再自动补满前段。正常结束和 Miss 都仍服从核心的 HideAll, 审计则另行要求正常进度终点。
