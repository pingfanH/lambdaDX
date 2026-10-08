# Editor-1 Flash 文档

主文件：`../Editor-1.fla`。可编辑 XFL：`editor_ui.xfl`。

舞台尺寸为 1570 × 856，60 FPS，单帧静态界面。源图为 Mai2Chart 的 `Editor-1.svg`，组织方式参考 `assets/player_ui` 的独立库元件和页面组合结构。

## 元件结构

`UI/page_editor` 组合六个独立图层：背景、顶部菜单、时间网格、进度条、预览面板、浮动工具栏。

- `UI/Header`：品牌、窗口标题、窗口控制区、Save / Load / Open / Export / Grid / Audio 菜单。
- `UI/Timeline`：时间网格及刻度。
- `UI/Transport`：底部进度条及时间显示。
- `UI/Preview`：预览面板背景、收起按钮、圆形封面、预览盘。
- `UI/Preview/Sensors`：33 个独立感应区域元件。
- `UI/Inspector`：属性面板及 21 个独立细节元件。
- `UI/Tools`：浮动面板、阴影、8 个独立工具按钮。

共有 86 个 MovieClip 元件。文字沿用源 SVG 的字形轮廓，以保持字体外观；图形使用独立可编辑矢量绘制对象。封面图片和模糊阴影使用位图。实例位置和图层堆叠已保留。此文档复刻静态外观，尚未绑定按钮事件或编辑器业务逻辑。

## 文件

- `LIBRARY/`：独立元件源文件。
- `components.json`：元件与原始 SVG 绘制元素的映射、注册点。
- `source/Editor-1.svg`：原图副本。
- `source/reference.png`：原图的 SVG 渲染基准。
- `preview.png`：Adobe Animate 导出的实际舞台预览。
- `animate_validation.txt`：Animate 保存和库元件检查结果。
- `verification.json`：像素比较和引用完整性验证。

已使用 Adobe Animate 2024 打开 XFL 并另存为 FLA。与 SVG 基准的比较用于发现图形填充、裁剪和位置错误；SVG 与 Animate 渲染器在抗锯齿、图片缩放和阴影处理上仍有细微差异，并非逐像素完全一致。
