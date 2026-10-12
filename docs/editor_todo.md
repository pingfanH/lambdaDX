# 编辑器 TODO — 相对原版 macroquad_sim 的缺口

> 对比基准:原版 `Mai2Chart/demo/macroquad_sim/src/app`(~15.7k 行)
> 移植版:`lambdaDX/editor/src/**`(自包含模块树;入口 `main.rs`)
>
> **现状**:单选框选、时间轴放置 Tap/Hold/Touch/星头 Slide、拖拽移动、删除、
> 播放控制、缩放/擦洗、波形金字塔、自动播放 + lnmai-core 判定预览。

图例:`[ ]` 未实现 · `[~]` 部分实现 · `[x]` 已完成

> UI 专项计划另见 `docs/EDITOR_UI_PLAN.md` / `docs/EDITOR_UI_TODO.md`
> (含与 `Editor.svg` 的元件级缺口对照)。本文件聚焦“相对原版 macroquad_sim 的功能缺口”。

---

## P0 — 阻塞日常编辑

- [~] **保存** — `⌘/Ctrl+S` 或 header `SAVE` 按钮写出 `output/editor_chart.json`
      (pretty JSON,format C)。**另存为/自定义路径**尚未做。
      `editor/src/timeline/input.rs::save_chart`(经 `editor/src/io.rs`)、`editor/src/ui/toolbar.rs`。
- [ ] **Simai 导出** — 含分数/休止符换算。原版 `simai_io.rs:1255`。
      移植版 `app/` 目前**没有** Simai writer,需新写,工作量较大。
- [x] **撤销 / 重做入口** — `⌘/Ctrl+Z` 撤销、`⌘/Ctrl+Shift+Z` 或 `⌘/Ctrl+Y` 重做。
      `editor/src/timeline/input.rs::handle_edit_shortcuts`。
- [x] **多选 + 框选(橡皮筋)** — Shift+点选切换、空白拖拽框选;`Shift` 保留旧选区。
      `editor/src/timeline/input.rs`(两种布局)、`geometry.rs::paint_marquee`。
- [x] **复制 / 粘贴 / 剪贴板** — `⌘/Ctrl+C` 复制、`⌘/Ctrl+X` 剪切、`⌘/Ctrl+V`
      在播放头处粘贴(保留相对时间与 lane)。`editor/src/edit_ops.rs::copy_selection/paste_at`。
      (原版的「粘贴幽灵跟随光标」尚未做,当前是就地粘贴。)
- [x] **音符属性编辑** — 侧栏 INSPECTOR → NOTE 显示/编辑选区首个音符的
      TIME / LANE / DUR + Break / Ex / Each / Star / Tapless 复选框(每次编辑一步撤销)。
      `editor/src/ui/inspector.rs::note_properties`。
- [x] **Break / Ex / ExBreak 快捷键** — B / X / N(作用于整个选区,可撤销)。
      `editor/src/timeline/input.rs::handle_edit_shortcuts`。
- [x] **原生打开文件 + 难度切换** — 侧栏 `CHART`:路径输入框 + `OPEN…`
      (macOS 经 `osascript` 选文件夹)+ `LOAD` + `◀/▶` 难度循环。
      `PadPreviewState::load_chart` 原地换谱面(重建 chart/cue/音频/引擎/autoplay,
      纹理材质复用),无需重启。见 `editor/src/ui/toolbar.rs`(难度切换)、
      `player/state.rs::load_chart`、`editor/src/runtime.rs::load_chart_into` /
      `editor/src/io.rs::pick_chart_path`。
      (原生对话框目前仅 macOS;其它平台可用路径输入或 `MAI2_EDITOR_CHART`。)

## P1 — 核心功能缺失

- [ ] **录音模式(整块)** — 开始/停止(R)、lane 键 1–8/T 录制、pad 触摸录制、
      zone 记录、音符分类、录音吸附、录音速度、RecordingDoc 保存。
      原版 `input.rs:585/656`,`state.rs:510/1034/1048`。
- [~] **Slide 轨迹编辑** — 中键/三指点 slide 轨迹进入(选中并定位到该子 slide);
      进入后 pad 预览**只显示这条 slide**(`app.edit_solo_note = editing_slide`,其余 note 隐藏、
      不做时间裁剪,整条轨迹可见);点 pad A 区或按 `1–8` 追加航点
      (`editing_slide_idx` 指定子 slide);形状键 `Q/P/S/Z/V/W`(⇧=QQ/PP,
      `,`/`.`/`;`=</>/^,`-`=line)+ `1–8` 追加一个新 segment
      (含 V 过中心、`validate_slide_shape` 校验);`⌫` 撤最后一个航点(空段丢弃);
      `Esc` 退出。延时点/头尾拖拽、双击星头追加子 slide 未做。
      `editor/src/edit_ops.rs::{append_slide_waypoint,pop_slide_waypoint,apply_slide_shape}`、
      `editor/src/timeline/input.rs::{handle_edit_shortcuts,slide_trail_at}`
      (原版 `input.rs:243/291/680/1155/1381`)。
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

- [x] **竖向时间轴为默认布局** — 仿设计 SVG:左侧竖向 timeline(时间向上流向
      判定线,lanes 为列),右侧 pad/inspector 信息面板;顶栏按钮运行时切换布局。
      见 `editor/src/state.rs`(`EditorLayout`,默认 `Vertical`)、
      `editor/src/ui/`(`layout.rs` 面板分栏、`toolbar.rs` 切换按钮、
      `sidebar.rs` 右侧面板)、`editor/src/timeline/{input,vertical}.rs`。
      可用 `MAI2_EDITOR_LAYOUT=horizontal` 启动为横向布局。
- [x] **工具面板移到时间轴上方** — 工具调色板不再单独占一列,而是横排在时间轴
      上方(`editor/src/ui/tools.rs`、`main.rs`/`ui/mod.rs` 布局)。
- [x] **顶栏软件工具条** — Save/Load/Open/Export/Grid/Audio + 难度/预览模式/布局,
      不依赖 mac 原生菜单栏(`editor/src/ui/toolbar.rs`、`editor/src/io.rs`)。
- [x] **右侧面板宽度比例** — 信息面板与时间轴列等宽(各占内容宽的一半减去中缝手柄);
      窗口默认 1600×900 且可缩放,比例自适应。
      见 `editor/src/ui/layout.rs::sidebar_width`、`editor/src/main.rs`。

## 移植版已领先(无需回补)

自动播放 + lnmai-core 判定引擎、Combo/Score/DX、Hit FX、判定文本、
F1 参数面板、视频背景、波形金字塔。

---

## 建议落地顺序

1. P0 的「撤销入口 + 保存/导出 + 多选」— 成本最低、解锁后续工作。
2. P1 的「录音 + Slide 轨迹 + 模板」三块大功能。
3. P2 / P3 穿插优化。
