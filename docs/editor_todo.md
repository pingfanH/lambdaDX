# 编辑器 TODO — 相对原版 macroquad_sim 的缺口

> 对比基准:原版 `Mai2Chart/demo/macroquad_sim/src/app`(~15.7k 行)
> 移植版:`lambdaDX/editor/*` + `src/editor_main.rs`(~1.8k 行)
>
> **现状**:单选框选、时间轴放置 Tap/Hold/Touch/星头 Slide、拖拽移动、删除、
> 播放控制、缩放/擦洗、波形金字塔、自动播放 + lnmai-core 判定预览。

图例:`[ ]` 未实现 · `[~]` 部分实现 · `[x]` 已完成

---

## P0 — 阻塞日常编辑

- [ ] **保存 / 另存为** — JSON + `latest_chart.json`。原版 `chart.rs:207`。移植版无任何写路径。
- [ ] **Simai 导出** — 含分数/休止符换算。原版 `simai_io.rs:1255`。
      `beat_format::chart_to_json`、`platform::write_output_text` 管道已存在但未接线。
- [ ] **撤销 / 重做入口** — 机制已存在(`editor/state.rs:71` `push_history/undo/redo`),
      仅缺快捷键/按钮接线,低成本高收益。
- [ ] **多选 + 框选(橡皮筋)** — `selected: HashSet<u64>` 已支持,现仅塞 1 个 id
      (`editor/timeline.rs:142`)。原版 `input.rs:1990/2024`。
- [ ] **复制 / 粘贴 / 副本** — 含粘贴幽灵预览。原版 `input.rs:200/2065`。
- [ ] **音符属性编辑** — Type/Time/Lane/Duration + Break/Ex/Each/Star/Tapless。
      原版 `egui_ui/mod.rs:711`。现侧栏是绑死 `value=0.5` 的假滑块。
- [ ] **Break / Ex / ExBreak 快捷键** — B / X / N。原版 `input.rs:453-545`。
- [ ] **原生打开文件 + 难度切换** — 文件对话框、音频自动识别、level 循环。
      原版 `simai_io.rs:1867/1863`,`egui_ui/mod.rs:270`。

## P1 — 核心功能缺失

- [ ] **录音模式(整块)** — 开始/停止(R)、lane 键 1–8/T 录制、pad 触摸录制、
      zone 记录、音符分类、录音吸附、录音速度、RecordingDoc 保存。
      原版 `input.rs:585/656`,`state.rs:510/1034/1048`。
- [ ] **Slide 轨迹编辑** — E 进入、点 pad 追加航点、中键进入、
      形状键 Q/P/S/Z/V/W + 1–8、延时点/头尾拖拽、双击星头追加子 slide。
      原版 `input.rs:243/680/1155/1381`。
- [ ] **形状自动匹配与生成** — Line/VShape 匹配、形状→航点、形状合法性校验。
      原版 `slide_match.rs:37`,`simai_io.rs:781`,`types.rs:204`。
- [ ] **模板 / 场景隔离系统(整块)** — 建/删/改名/插入/移动实例、进入隔离与回写、
      版本传播、面包屑、场景抽屉、模板音符保护。原版 `template.rs` 全文件,
      `egui_ui/mod.rs:1203`。
- [ ] **隐藏 / 取消隐藏** — H / Shift+H,隐藏音符 30% 透明。原版 `input.rs:66`。
- [ ] **Hold / Slide 时长拖拽** — 头/身/尾分段拖拽改时长。原版 `input.rs:1697`。
- [ ] **Grab(G)/ Scale(S)模式** — 整体抓取/缩放 + 方向键微调。
      原版 `input.rs:1525/1550/1650`。
- [ ] **右键删除** — 时间轴最近音符。原版 `input.rs:1119`。

## P2 — 体验完善

- [ ] **进度条点击拖拽 + 时间标签** — `Xs / Ys (xZoom)`。
      原版 `input.rs:979`,`draw_timeline.rs:1784`。现仅 overview 擦洗。
- [ ] **播放到底自动循环** — 原版 `state.rs:731`。
- [ ] **中键拖动平移时间轴** — 原版 `input.rs:913`。
- [ ] **框选到边缘自动滚动** — 原版 `input.rs:922`;选择锚定时间随滚动保持 `state.rs:57`。
- [ ] **频谱 / 节拍可视化** — 原版 FFT 低频节拍条 `draw_timeline.rs:152`;
      移植版仅波形金字塔。
- [ ] **波形阈值 `[`/`]` 调整** — 原版 `input.rs:553`。
- [ ] **音频偏移 UI** — 字段存在但无控件,±0.05s 步进。原版 `egui_ui/mod.rs:401`。
- [ ] **音量滑条 / 静音** — 移植版仅开关。
- [ ] **Toast 通知系统** — 四级提示 + 进度条。原版 `toast.rs`;
      移植版 `status` 字段从不渲染。
- [ ] **Chart Info 面板** — Title/Artist/BPM/Offset。原版 `egui_ui/mod.rs:794`。
- [ ] **主工具栏** — File/Simai/路径导入行/运输/速度/偏移/状态读数。
      原版 `egui_ui/mod.rs:200-447`。

## P3 — 锦上添花

- [ ] **变速 PCM 缓存 / 预热缓存** — 原版 `audio.rs:228/233`。
- [ ] **`is_each` 自动重算** — 原版 `state.rs:577`。
- [ ] **Pad 触控反馈计时闪烁** — 原版 `state.rs:973`。
- [ ] **侧栏工具图标系统补全** — 原版 `egui_ui/mod.rs:1495`。
- [ ] **快捷键帮助窗口** — 原版也没有,可选。
- [ ] **节拍器** — 原版也没有,可选。

---

## 布局改动

- [x] **竖向时间轴布局选项** — 仿原版:左侧竖向 timeline(时间向上流向判定线,
      lanes 为列),右侧 pad/inspector 信息面板;header 按钮运行时切换。
      见 `editor/state.rs`(`EditorLayout`)、`editor/egui/mod.rs`(header 按钮 +
      `SidePanel::right`)、`editor/timeline.rs`(`handle_input_vertical` /
      `draw_macroquad_vertical`)、`src/editor_main.rs`(派发 + 侧栏背景)。
      可用 `MAI2_EDITOR_LAYOUT=vertical` 直接以竖向启动。
- [x] **竖向宽度比例 + 窗口默认宽度** — 竖向布局下右侧信息面板宽 = 可用宽度的 1/3,
      时间轴 = 2× 信息面板(超出部分不受 SIDEBAR_MAX 限制以保持 2:1);
      窗口默认 1600×900 且可缩放,缩放时比例自适应。见 `editor/egui/mod.rs`
      `vertical_sidebar_width`、`src/editor_main.rs` `editor_window_conf`。

---

## 移植版已领先(无需回补)

自动播放 + lnmai-core 判定引擎、Combo/Score/DX、Hit FX、判定文本、
F1 参数面板、视频背景、波形金字塔。

---

## 建议落地顺序

1. P0 的「撤销入口 + 保存/导出 + 多选」— 成本最低、解锁后续工作。
2. P1 的「录音 + Slide 轨迹 + 模板」三块大功能。
3. P2 / P3 穿插优化。
