# Editor UI 优化计划

> 目标:让编辑器 UI 对齐作者设计 `Editor.svg` / `Editor-1.fla`,并补齐日常编辑
> 所需的交互(选择/缩放/吸附/循环/保存闭环),保持与 `player_ui` 一致的暗色 +
> 强调色语言。
>
> 代码结构:`editor/src/`(自包含模块树)
> - 入口/主循环:`main.rs`、`runtime.rs`
> - 状态/编辑:`state.rs`、`edit_ops.rs`、`io.rs`
> - UI:`ui/{mod,toolbar,sidebar,inspector,tools,layout,style,widgets}.rs`
> - 时间轴:`timeline/{mod,geometry,input,notes,vertical,horizontal,overview,waveform}.rs`
> - 主题:`theme.rs`
>
> 设计源:`assets/editor_ui/source/Editor-1.svg`(舞台 1570×856)、
> `assets/editor_ui/components.json`(86 个元件)、`assets/Editor-1.fla`。
> 参考实现(旧):`Mai2Chart/demo/macroquad_sim/src/app/egui_ui/*`。

图例:`[x]` 已对齐 · `[~]` 部分/风格未对齐 · `[ ]` 未实现 · `[-]` 有意偏离(按需求)

---

## 与 Editor.svg 元件对照(缺口清单)

> 这是本计划的核心:下表逐项列出设计元件 vs 当前实现,`[ ]`/`[~]` 即“未参考
> Editor.svg 优化”的部分。

### UI/Header(顶栏)
- [x] `ui_menu_background` — 顶栏底(已用 `theme::BG_TOOLBAR`)。
- [~] `ui_brand` — 设计是**矢量 logo**;当前用文字 “Lambda DX”。→ 可选矢量/图标。
- [-] `ui_title`(“LambdaDX Editor”,居中标题)— 按需求不实现。
- [-] `ui_window_chrome`(红黄绿窗控)— 按需求不实现。
- [x] `ui_save` / `ui_load` / `ui_open` / `ui_export` / `ui_grid` / `ui_audio`
      — 已在顶栏(`ui/toolbar.rs`)。
- [ ] 顶栏缺:撤销/重做按钮、“未保存”指示、快捷键帮助(`?`)。

### UI/Timeline
- [~] `ui_grid` — 网格/小节线/车道线/判定线已画,但**未逐像素对齐设计**
      (线色、刻度间距、小节数字位置、拍线高亮、判定红线)。`timeline/vertical.rs`。

### UI/Transport
- [~] `ui_progress` — 底部进度条 + 时间/缩放读数已做(`timeline/overview.rs`),
      样式未对齐设计(圆角/颜色/手柄/刻度)。

### UI/Preview(右侧面板)
- [x] `ui_sidebar_background` — 面板底 #222(`main.rs` 用 macroquad 画)。
- [x] `ui_collapse` — 右缘折叠手柄(`ui/mod.rs::draw_panel_handle`)。
- [x] `ui_cover` / `ui_pad` / `ui_preview_panel` — 由实时 pad 渲染器绘制
      (`player::render::pad::draw_pad_sim`:disc + cover + pad)。
- [~] `Sensors/ui_sensor_01..33`(33 个感应区)— 由实时 pad 的 zone 绘制覆盖;
      需确认编辑器预览是否与设计的区域样式一致(开/关、描边)。

### UI/Inspector
- [~] `ui_inspector` + `ui_detail_00..20`(21 个细节元件)— 已有 “Note Properties”
      (Type/Time/Lane/Duration + Break/Ex/Each/Star/Tapless)与 “Chart Info”
      (Title/Artist/BPM/Offset)。**未对齐/缺失**:
  - [ ] 齿轮(设置)图标、`collapse` 三角、行分隔线的精确样式。
  - [ ] 设计中的额外字段行(逐项对照 `components.json` 的 21 个 detail 确认;
        候选:Slide 延时/时长、BPM/Offset **可编辑**、Chart 元数据)。
  - [ ] 21 个 detail 的组件映射表(需从 DOMDocument/`source/` 反查)。

### UI/Tools(工具栏)
- [x] 已按设计复刻为**左侧浮动面板 + 阴影**(`ui_tools::draw`:38×268 `#252529`,
      阴影 dy3/blur4,圆角 4),浮在时间轴左上角(设计偏移 12/72)。
- [x] `ui_button_00..07`(8 个工具按钮):Select/Tap/Hold/Touch/TouchHold/Slide/
      Break/Eraser,30×30、圆角 3、选中 `#477DAD`、4/2/2 分组分隔线。
- [x] 工具按钮的选中/悬停态与设计阴影对齐。

### 其他
- [ ] `UI/page_editor` 整页裁切/留白与设计的 1570×856 栅格对齐(DESIGN_W/H 已在
      `ui/layout.rs`,但比例是响应式而非固定设计比例)。

---

## P0 — 编辑闭环与设计对齐

- [ ] **顶栏完善**:Undo/Redo 按钮 + 灰化态;`dirty` 星标;`?` 快捷键帮助浮层;
      状态文本(toast)统一。
- [ ] **Inspector 对齐设计**:齿轮/collapse 三角/分隔线;补齐字段;
      多选时显示聚合(相同值 vs 混合)。
- [ ] **时间轴网格对齐 `ui_grid`**:抽出网格样式常量(线色/粗细/小节号/拍高亮/
      判定线),提供“贴合设计”的默认值;`timeline/vertical.rs`。
- [ ] **Transport 对齐 `ui_progress`** + 保留时间/缩放/音符数读数。
- [ ] **选择/编辑交互**:框选贴边自动滚动;右键删除最近音符;hover 显示音符信息;
      Hold/Slide **头/尾时长拖拽手柄**;方向键微调(±1 格)。
- [ ] **Tools 定案**:确定 8 个工具集合与图标;记录“浮动面板→横条”的有意偏离。

## P1 — 效率与视图

- [ ] **吸附选择器 UI**:`snap_division`(1/4、1/8、1/16、1/32)下拉 + 快捷;Grid
      开/关已有,补“吸附到拍/小节”指示。
- [ ] **缩放控件**:`+ / - / 适配`、百分比读数;中键平移;`overview` 拖动同步。
- [ ] **播放**:循环 A/B、播放速度、节拍器、跟随播放头开关(已有自动跟随,补
      手动)。
- [ ] **音频**:偏移(Offset)UI、音量/静音、波形阈值 `[`/`]`。

## P2 — 内容与工程

- [ ] **模板/场景系统**(见 `docs/editor_todo.md` P1)。
- [ ] **Simai 导出**(P0 缺口)。
- [ ] **Save As / 最近文件 / 自动保存 / 拖放导入**。
- [ ] **Chart Info 可编辑**(Title/Artist/BPM/Offset)。
- [ ] **Toast 通知系统**(status 字段目前只在顶栏)。

## P3 — 打磨

- [ ] 侧栏可拖拽调宽;全屏;DPI 缩放;面板悬浮/固定。
- [ ] 性能:文本度量缓存;note 绘制批次;已有 culling(`timeline/*.rs`)。
- [ ] 截图/黄金测试:`MAI2_EDITOR_SHOT` / `MAI2_EDITOR_SHOT_AT` + 固定布局。

---

## 设计原则(保持)

- 表面/线条/文本/强调色全部走 `editor/src/theme.rs`(对齐设计取样值)。
- 尺寸走设计常量(`TOOLBAR_H`/`TOOLS_STRIP_H`/`OVERVIEW_H`/`PADDING`),必要时
  与 `ui/layout.rs` 的响应式比例组合。
- 两套布局:竖向为默认(仿设计),横向为旧移植布局,仅时间轴朝向不同。

## 验证

- 截图:`MAI2_EDITOR_SHOT=… MAI2_EDITOR_SHOT_AT=…`(+`MAI2_EDITOR_LAYOUT=vertical|horizontal`)。
- 单测:`cargo test --bin lambda_dx_editor`(布局比例、几何、编辑操作)。
- 与 `assets/editor_ui/preview.png`(Animate 导出)逐区域对照,记录差异。
