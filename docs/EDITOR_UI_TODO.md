# Editor UI TODO

> 配套计划:`docs/EDITOR_UI_PLAN.md`。勾选即完成(需可截图/可测试复核)。
> 涉及:`editor/src/ui/*`、`editor/src/timeline/*`、`editor/src/theme.rs`、`editor/src/main.rs`。

## 与 Editor.svg 对齐(缺口)

- [ ] `ui_grid`:时间轴网格样式对齐设计(线色/粗细/小节号/拍高亮/判定线)。
- [ ] `ui_progress`:底部进度条样式对齐设计(圆角/色/手柄/刻度)。
- [ ] `ui_inspector` + `ui_detail_00..20`:齿轮图标、collapse 三角、分隔线;
      逐项对照 `components.json` 补齐 21 个细节元件映射。
- [ ] Inspector 缺失字段:Slide 延时/时长;BPM/Offset 可编辑;Chart 元数据。
- [ ] `ui_brand`:矢量/图标化(可选)。
- [ ] Tools:确定 8 个工具集合 + 图标 + 阴影/选中态(记录“浮动→横条”偏离)。
- [ ] `ui_cover`/`Sensors`:确认预览面板 cover 与 33 感应区样式与设计一致。
- [-] `ui_title` / `ui_window_chrome`:按需求不实现(标注)。

## P0 — 闭环

- [ ] 顶栏 Undo/Redo 按钮(含灰化)。
- [ ] `dirty` 未保存指示 + 载入/退出前提示。
- [ ] `?` 快捷键帮助浮层。
- [ ] Inspector 多选聚合显示(相同/混合)。
- [ ] 框选贴边自动滚动;hover 音符信息。
- [ ] 右键删除最近音符。
- [ ] Hold/Slide 头/尾时长拖拽手柄。
- [ ] 方向键微调选中音符(±1 格)。

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
