# 工作台 UI 预览

这是独立的原生 Rust bin `lambda_dx_ui_preview`, 用来确认
[工作台设计](EDITOR_WORKBENCH_DESIGN.md)的风格与布局, 不替换现有 editor。

## 启动

在项目根目录执行:

```sh
cargo run --no-default-features --features ui-preview --bin lambda_dx_ui_preview
```

不需要 Lean / lnmai-core。源码位于 `editor/ui_preview/`,
停靠组件和图标字体依赖只在 `ui-preview` feature 下启用。

## 绘制性能

已修复预览整体持续掉帧的问题: 原 egui 绘制器让所有绘制批次反复覆盖
同一组 GPU 缓冲, 在 macOS 上会频繁等待前一批绘制完成。
现在为批次复用独立缓冲, 并轮换三组顶点/索引缓冲池; 不降低分辨率、不关闭透明效果。
本机 `1500x940` HiDPI、dev 构建下, Chart 从约 11-14 FPS 恢复到约 60 FPS,
Autoplay 也约 60 FPS。正式 editor/player 在重新编译后使用同一修复。
依赖补丁和测量记录见 [LOCAL_PATCH.md](../vendor/egui-miniquad/LOCAL_PATCH.md)。

## 可以体验

- 炭灰色半透明顶栏与菜单, 不使用纯黑底色, 保持紧凑的文字菜单和图标按钮。普通按钮没有常驻底色/边框,
  仅在悬停、按下时提供轻微反馈; 工具选中使用图标变色或细小标记, 不使用灰色方块底。
- 全局只保留一行: File / Edit / View / Playback、工作区、播放控制、布局与设置。
  Chart / Autoplay / Combined 三套工作区各自保留布局和视图状态。
  窄窗口将工作区合并为下拉菜单, 时值和速度仍可在 Playback 菜单中调整, 不另起一行。
- 各分区只保留一条标题栏, 吸附、细分、缩放、Pad 传感器、属性动作和撤销/重做
  都并入对应标题行。窄分区把次要控件收进工具菜单, 不叠加第二条工具栏。
- 每个分屏只有一个编辑器, 不包含标签组, 不提供拖到中心合并标签。
  拖分隔线调整比例, 标题左侧的类型图标替换当前编辑器。
- 标题右侧的区域菜单或标题空白处右键, 可向右/下拆分、交换区域、关闭区域;
  不允许关闭最后一个区域。布局锁定时禁用这些结构操作和类型替换。
- 标题右侧图标或双击标题空白处最大化/还原; 布局菜单可记住本次会话的布局并恢复。
- View > Add editor area 在活动区域右侧创建独立分屏; 关闭区域不删除 note。
  模板与历史可从区域类型菜单进入, 也可以添加为独立区域。
- Chart 左侧为画布内悬浮的紧凑工具条, 使用炭灰色半透明材质, 不再占满整列;
  包含选择工具及 Tap / Hold / Slide / Touch 贴图工具。
  空白处点击放置示例 note, 选择工具点击空白取消选择, 拖 note 调整时间和 lane。
- Properties 修改选中 note 的时值、lane、长度; 模板面板可插入三种示例图案。
- Autoplay 使用四条复用轨道, 选中片段后下方展开对应传感器区间;
  拖片段移动, 拖区间两端调整 Press / Release 范围。点击空白取消选择。
  传感器详情区域可独立上下滚动, 不会带动轨道时间位置。
- 播放、循环、速度、时值输入、总览定位; 所有区域共享同一个演示时钟。
- 普通滚轮平移时间轴, Command/Ctrl + 滚轮以播放线为锚点缩放;
  分区标题行可调细分、吸附、倍率、跟随播放、定位所选 note。
- Settings 集中预览透明浮层、note 尺寸、网格、吸附、Pad 传感器和布局设置。

预览快捷键:

| 操作 | 快捷键 |
| --- | --- |
| 播放 / 暂停 | Space |
| 设置窗口 | F1 |
| 下一个 / 上一个 note | Tab / Shift+Tab |
| 复制选中 note | Shift+D |
| 删除 | Delete / Backspace |
| 撤销 / 重做 | Command/Ctrl+Z / Command/Ctrl+Shift+Z |
| 退出最大化并取消选择 | Esc |

快捷键只覆盖上述原型操作, 不代表正式编辑器的完整操作方案。

## 边界

- 使用内置示例数据, 不是当前打开的真实谱面。所有数据和布局只保存在内存。
- 不连接核心、不产生判定、不播放音频、不读取或保存真实谱面;
  File 菜单中保存和导出明确禁用。正式编辑器的保存功能不受影响。
- Pad 和轨道用于展示视觉层级与联动, 没有复刻完整 slide 几何、判定或 autoplay 生成器。
- Hold / Slide 示例放置使用预设长度; 正式的多阶段放置与完整键盘操作尚未迁入。
- Autoplay 原型的片段移动会同步移动示例 note, 不作为正式事件数据模型。
- 玻璃外观使用中性炭灰半透明底色与低对比细亮边, **尚无真实磨砂模糊**。
  不透出桌面, 不增加实时模糊开销; 画布保持不透明。关闭透明选项后改为同色实底。
- 只支持窗内停靠与分屏, 不创建跨屏原生子窗口。
- 确认视觉和布局后再接入正式编辑器, 不直接把原型作为生产实现。

## 可选启动参数

```sh
MAI2_UI_PREVIEW_WORKSPACE=autoplay cargo run --no-default-features --features ui-preview --bin lambda_dx_ui_preview
MAI2_UI_PREVIEW_WINDOW=1280x720 cargo run --no-default-features --features ui-preview --bin lambda_dx_ui_preview
```

- `MAI2_UI_PREVIEW_WORKSPACE`: `chart`(默认)、`autoplay`、`combined`。
- `MAI2_UI_PREVIEW_WINDOW`: 逻辑窗口尺寸, 默认 `1500x940`。
- `MAI2_UI_PREVIEW_SETTINGS=1`: 启动时显示设置窗口。
- `MAI2_UI_PREVIEW_SHOT=/tmp/workbench.png`: 稳定渲染数帧后截图并退出, 仅用于样稿检查。
