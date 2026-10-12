# Player UI TODO

> 配套计划:`docs/PLAYER_UI_PLAN.md`。勾选即完成(需可截图/可测试复核)。
> 涉及目录:`src/player_ui/`(`mod.rs`、`pages/*.rs`、`state.rs`、`library.rs`、
> `theme.rs`、`draw.rs`、`anim.rs`、`input.rs`、`perf.rs`)。

## P0 — 闭环与可用性

- [ ] `Page::Result` + `pages/result.rs`:曲名/难度/评级/精度/判定计数/最佳记录,
      「重玩 / 返回选歌 / 下一首」。
- [ ] `state.rs`:歌曲结束(或 core 报 finished)自动切 `Page::Result`。
- [ ] `pages/gameplay.rs` HUD 增加 Combo / Score / 精度 / 判定统计。
- [ ] 键盘焦点导航:方向键移动、Enter 确认、Esc 返回;焦点环绘制。
- [ ] 手柄支持(可选但同类):方向/确认/返回映射到焦点导航。
- [ ] Toast 队列(info/success/warn)+ 淡出/堆叠,替换单 `app.error`。
- [ ] 选歌加载骨架屏 + 解码进度 + 失败重试。
- [ ] 曲库扫描进度与空库引导。

## P1 — 曲库与设置

- [ ] 搜索框(标题/艺术家)。
- [ ] 排序(标题/BPM/难度/最近)与难度筛选。
- [ ] 收藏 / 最近游玩。
- [ ] 大曲库虚拟滚动(仅画可视行)。
- [ ] 封面预取 + 占位/失败兜底图 + 去重解码。
- [ ] 设置新增:音量(总/BGM/SFX)、判定偏移、皮肤/主题、背景视频参数、按键说明。
- [ ] 设置分节「恢复默认」+ 全量持久化(载入/写回)。

## P2 — 体验

- [ ] 开播倒计时 / await-first-input。
- [ ] 窗口失焦自动暂停。
- [ ] 文案 i18n 字符串表 + 语言切换。
- [ ] 转场读回(`capture_transition`)降频/可关;或换便宜动效。
- [ ] 小屏/移动重排(`mobile_ui`)、更大热区、安全区。
- [ ] 无障碍:高对比、色盲友好判定色、字号档、减少动效。

## P3 — 打磨

- [ ] 统一间距/圆角/命中盒;hover/active/focus 三态;指针样式。
- [ ] 文本度量缓存,消除每帧重复 `measure_text`。
- [ ] 减少每帧堆分配;预载下一首封面/音频。
- [ ] 精度公式/评级阈值/DX 口径复核;0-combo 提示。
- [ ] player UI 内置 F1 参数的只读摘要/高级入口。

## 验证清单(每项需满足)

- [ ] 截图复核 `MAI2_*_SHOT`(必要时 `MAI2_UI_TRANS_HOLD` 冻结转场)。
- [ ] 状态机/数值逻辑有单测(`cargo test --bin lambda_dx_player_ui`)。
- [ ] 不破坏现有 89+b 项测试与 `cargo check --bins`。
