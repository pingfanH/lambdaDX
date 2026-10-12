# Editor UI TODO

> 配套计划:`docs/EDITOR_UI_PLAN.md`。勾选即完成(需可截图/可测试复核)。
> 涉及:`editor/src/ui/*`、`editor/src/timeline/*`、`editor/src/theme.rs`、`editor/src/main.rs`。

## 与 Editor.svg 对齐(缺口)

- [x] `ui_grid`:线色/拍高亮/判定线按 svg 取样(`GRID_MINOR/LINE/BEAT/LANE/LANE_CENTER`,
      判定线 1px `#FF3B50`,去掉红色小节线);小节号/刻度间距随缩放。
- [x] `ui_progress`:进度条对齐设计(10px `#2B2B2D` + 左端 1px 红起点 + 刻度/播放头)。
- [~] `ui_inspector` + `ui_detail_00..20`:齿轮图标、collapse 三角、分隔线(`#2A2A2D`)、
      标签/取值色(`#8D8D94`/`#BDBDC2`)已对齐;逐项对照 `components.json` 补齐映射待做。
- [x] Inspector 缺失字段:Slide 延时/时长(`slide_start_delay`/`slide_duration`,写回全部子滑条);
      BPM/Offset 可编辑(改 BPM 同步 `bpms[0]`);Chart 元数据 Title/Artist 文本可编辑。
- [ ] `ui_brand`:矢量/图标化(可选)。
- [x] Tools:8 个工具 + 图标 + 阴影/选中态,并**按 svg 复刻浮动面板**
      (38×268 `#252529` 阴影,4/2/2 分组;新增 TouchHold/Break)。
- [ ] `ui_cover`/`Sensors`:确认预览面板 cover 与 33 感应区样式与设计一致。
- [-] `ui_title` / `ui_window_chrome`:按需求不实现(标注)。

## P0 — 闭环

- [x] 顶栏 Undo/Redo 按钮(含灰化)。
- [x] `dirty` 未保存指示(顶栏红点 + 保存清空)+ 载入/打开/退出前确认(Save/Discard/Cancel);
      载入成功清空 `dirty`(顺带修复之前切图后红点不消的 bug)。
- [x] `?` 快捷键帮助浮层(顶栏 `?` 按钮 / 按 `?` / `F1` 切换)。
- [x] Inspector 多选聚合显示(相同/混合)。
- [x] 框选贴边自动滚动(横向平移 / 纵向 seek);hover 音符信息(进度条右读数)。
- [x] 右键删除最近音符。
- [ ] Hold/Slide 头/尾时长拖拽手柄。
- [x] 方向键微调选中音符(±1 格;←→ 时间、↑↓ 轨道)。

## P1 — 效率与视图

- [ ] 吸附选择器 UI(`snap_division`:1/4、1/8、1/16、1/32)。
- [ ] 缩放控件 `+/-/适配` + 百分比读数;中键平移。
- [ ] 循环 A/B;播放速度控件;节拍器;跟随播放头开关。
- [ ] 音频偏移 UI;音量/静音;波形阈值 `[`/`]`。

## P2 — 内容与工程

- [ ] Toast 通知系统(替换 status 单点)。
- [ ] Chart Info 可编辑(Title/Artist/BPM/Offset)。
- [ ] Save As / 最近文件 / 自动保存 / 拖放导入。
- [ ] Simai 导出(见 `docs/editor_todo.md` P0)。
- [ ] 模板 / 场景系统(见 `docs/editor_todo.md` P1)。

## P3 — 打磨

- [ ] 侧栏拖拽调宽;全屏;DPI 缩放;面板悬浮/固定。
- [ ] 文本度量缓存;note 绘制批次优化。
- [ ] 截图黄金测试(`MAI2_EDITOR_SHOT` / `MAI2_EDITOR_SHOT_AT`)。

## 验证清单

- [ ] 截图复核(竖向默认 + `MAI2_EDITOR_LAYOUT=horizontal`)。
- [ ] `cargo test --bin lambda_dx_editor` 全绿。
- [ ] 与 `assets/editor_ui/preview.png` 逐区域对照并记录差异。
