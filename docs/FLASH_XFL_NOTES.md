# Flash / XFL 知识笔记

> 本文记录本仓库在「用 Adobe Animate（Flash）制作 UI、并让 Rust 运行时解析 XFL
> 直接渲染」这条路上积累的格式知识与踩坑经验。属于参考资料，不是设计文档；
> 玩家 UI 的设计与页面结构见 `tools/make_player_ui_xfl.py` 与 `src/player_ui/`。

- 相关资产：`assets/player_ui/`（玩家 UI）、`assets/notes/`（音符贴图）
- 相关生成器：`tools/make_player_ui_xfl.py`、`tools/make_notes_xfl.py`
- 相关运行时：`macroanimate/`（XFL 解析 + 渲染）、`src/player_ui/mod.rs`、`src/player_ui/flash.rs`

## 1. 为什么用 Flash / Animate

- UI 是**矢量**的：同一份资源可在任意分辨率下清晰渲染，适合 1280×760 设计尺寸 +
  运行时按窗口缩放。
- 时间轴天然表达**页面转场/循环/逐帧动画**（按钮弹起、加载扫光、暂停面板进出）。
- 文本保持**可编辑**（`DOMStaticText`），文案不烧进图片。
- 资产是纯文本 XFL，可被脚本生成、被程序 diff、被运行时解析——避免绑定 Animate 二进制。

## 2. XFL 目录结构（未压缩 FLA）

Animate 的「未压缩 FLA」就是一个**文件夹**：

```
assets/player_ui/
├── player_ui.xfl        # 项目标记/桩文件，内容通常是 "PROXY-CS5"
├── main.xfl             # <DOMFlashFile>：列出本工程包含的所有 XML 文件路径
├── DOMDocument.xml      # 主文档：画布尺寸/帧率/背景色/符号 Include 列表/场景时间轴
├── PublishSettings.xml  # 导出设置
├── MobileSettings.xml   # 移动端设置（可为空壳）
├── META-INF/metadata.xml
├── text_slots.json      # 本项目自加的「动态文本槽位」表（非 Flash 格式）
├── LIBRARY/
│   └── UI/              # 每个 symbol 一个 .xml
│       ├── page_start.xml
│       ├── ui_btn_primary.xml
│       └── …
└── bin/                 # Animate 生成的缓存/发布产物（可 gitignore）
```

要点：
- `main.xfl`（`<DOMFlashFile>`）与 `DOMDocument.xml` 里的 `<Include>` 列表**必须同步**；
  新增符号时两个列表都要加，否则 Animate 打开时看不到该符号。
- `player_ui.xfl` 只是个标记文件（`PROXY-CS5`），标明「这是一个 XFL 工程」。
- 库中每个符号独立成文件，符号之间用 `name`（如 `UI/ui_btn_primary`）互相引用，
  **不是用 href**（`href` 只在 `<Include>` 里用）。

## 3. 关键 XML 概念

### 3.1 DOMDocument（主文档）

```xml
<DOMDocument backgroundColor="#171719" width="1280" height="760"
             frameRate="60" xflVersion="23.0" …>
  <symbols>
    <Include href="UI/page_start.xml" itemID="…"/>
    …
  </symbols>
  <timelines>…</timelines>
</DOMDocument>
```

- `width/height` 是设计尺寸（本工程 1280×760），运行时不改文档尺寸，而是整体缩放。
- `<Include>` 的 `itemID` 必须唯一且稳定；生成器按固定规则分配。

### 3.2 DOMSymbolItem（符号定义）

```xml
<DOMSymbolItem name="UI/ui_btn_primary" itemID="…" symbolType="graphic"
               scaleGridLeft="8" scaleGridTop="8"
               scaleGridRight="172" scaleGridBottom="38">
  <timeline>
    <DOMTimeline name="ui_btn_primary">
      <layers>
        <DOMLayer name="fill" color="#9933CC">
          <frames>
            <DOMFrame index="0" keyMode="9728">…</DOMFrame>
          </frames>
        </DOMLayer>
      </layers>
    </DOMTimeline>
  </timeline>
</DOMSymbolItem>
```

- `symbolType`：`graphic` / `movieclip` / `button`。本工程主要用 graphic（由外层
  movieclip 控制播放）。
- `scaleGrid*`：见第 4 节（9-slice）。

### 3.3 时间轴：Timeline → Layer → Frame → Element

- `DOMFrame index="N" keyMode="9728"` 是关键帧（9728 = 空关键帧之外的普通关键帧）。
- 帧里的 `<elements>` 可含 `DOMShape` / `DOMSymbolInstance` / `DOMStaticText`。
- **嵌套实例的帧坐标**：子 movieclip 实例播放到第几帧 = `父当前帧 + 实例 firstFrame`，
  由子符号自身总帧数决定循环（`single frame` 定格 / `loop` 循环 / `play once` 播放一次）。
  运行时按 `<DOMSymbolInstance loop="…">` 解析。

### 3.4 形状：FillStyle / StrokeStyle / Edge

```xml
<DOMShape>
  <fills><FillStyle index="1"><SolidColor color="#52D6E8"/></FillStyle></fills>
  <edges><Edge fillStyle1="1"
    edges="!0 0S2|3440 0!3440 0|3600 160!3600 160|3600 920"/></edges>
</DOMShape>
```

- `edges` 是路径编码：
  - `!` 开始一段子路径（moveTo）；段内 `|` 分隔点对；`x0 y0|x1 y1` 为直线。
  - `S`/`Q` 平滑/二次曲线（后跟样式标志数字，再跟控制点与终点）；
    `B` 三次曲线。运行时把曲线**离散化成折线**后绘制。
  - 多点路径自动闭合成多边形。
- **单位陷阱**：`edges` 里的坐标是 **twips（1/20 像素）**，而所有
  `matrix`（tx/ty/缩放）是**像素**。解析形状后要除以 20，矩阵不要除。
- 填充/描边用 `fillStyle0/1`、`strokeStyle` 索引到同形状的 `FillStyle`/`StrokeStyle`；
  一个 `<SolidColor>` 可出现在 fills 或 strokes 任意层。

### 3.5 实例：DOMSymbolInstance

```xml
<DOMSymbolInstance libraryItemName="UI/ui_btn_primary" firstFrame="0" loop="loop">
  <matrix><Matrix a="1" b="0" c="0" d="1" tx="56" ty="424"/></matrix>
  <color><Color alphaMultiplier="0.5"/></color>
</DOMSymbolInstance>
```

- `matrix`：`a b c d tx ty`（像素；`tx/ty` 是画布坐标）。
- `firstFrame`：起始帧偏移。
- `loop`：`single frame`（空格可有可无）/ `loop` / `play once`。
- `<Color>`：alpha 乘子（运行时用于淡入淡出）。

## 4. 九宫格（9-slice）——本项目踩的关键坑

目标：按钮/滑条等可拉伸符号，拉伸时四角不形变、中段拉伸。

**结论：Animate 的 9-slice 是「符号级」属性，不是「实例级」。**

- 正确位置：`<DOMSymbolItem>` 根节点上的四个属性
  `scaleGridLeft/Top/Right/Bottom`（单位：符号本地像素）。
  ```xml
  <DOMSymbolItem name="UI/ui_btn_primary" symbolType="graphic"
      scaleGridLeft="8" scaleGridTop="8"
      scaleGridRight="172" scaleGridBottom="38">
  ```
- **实例级** `<scale9Grid>` 无效：Animate 打开工程再保存时会**直接删掉**它
  （曾用它给 `page_start` 写实例网格，被 Animate 重存时全部清除）。
- 所以生成器把网格写在**符号**上，运行时对**该符号的所有实例**统一应用。
  实例级 `<scale9Grid>` 若存在则覆盖符号级（保留兼容）。
- 本工程取值：`ui_btn_*` → `(8, 8, 172, 38)`；`ui_slider` → `(6, 0, 294, 36)`。
- 运行时实现：`macroanimate/src/xfl/parse.rs`（符号级读到 `XflAtlas.scale9`，
  实例级读到 `Element::Instance.scale9`）→ `eval.rs` 决定用哪份 →
  `mod.rs` 的 `draw_nine_slice` 做矢量切片。

> 参考来源：`SC2FLA-FOSS-Edition/lib/fla/dom/symbol_item.py`（grep.app）中
> symbol item 的 `scaleGrid*` 字段；Coherent Prysm 文档「enable 9-slice
> property on any symbol」印证是符号级。

## 5. JSFL / Animate 自动化的坑（重要）

Animate 2024 通过 `mcp-for-animate` CEP 桥接执行 JSFL，**非常不稳定**：

- `fl.importFile`、`fl.runScript`、对 DOM 做 `for..in` 都可能直接**挂起** Animate。
- `setFillColor` 常被忽略；要用 `setCustomFill` / `setCustomStroke`。
- `addNewRectangle(bbox, roundness, suppressFill, suppressStroke)` 可用。
- 布尔运算 `union` 可用，但 `punch`/`intersect` 不可靠。
- `addItemToDocument` 会把符号 bbox **居中**到给定点（不是左上角）。
- 结论：**不要指望用 JSFL 自动搭 UI**。本工程改用「脚本直接写 XFL 文本」的
  生成器路线（见第 6 节），Animate 只用于人工查看/微调。

其它环境问题：
- Animate 可能「进程存在但不开窗」（无崩溃日志）。可通过 `/tmp/animate_recover.sh`
  重启，CEP 连接用 `/tmp/cdp_connect.js`。
- 用 JSFL 打开并保存工程后，Animate 会**重排/美化 XML**，并可能丢弃它不认的
  自定义内容（如实例级 `<scale9Grid>`）。因此 **XFL 以生成器为准**，人工改动要
  回填到生成器，避免下次重生成被覆盖。

## 6. 本仓库的资产管线（generator-owned）

**单一事实来源是 Python 生成器，XFL 是产物：**

```bash
python3 tools/make_player_ui_xfl.py    # 重新生成 assets/player_ui/
python3 tools/make_notes_xfl.py        # 重新生成 assets/notes/
```

约定：
- 符号命名空间 `UI/<name>`；页面/组合是 `UI/page_*`。
- **原子（atoms）只做视觉，不烘焙文案**；文案由页面/组合层用静态文本或文本槽提供，
  便于复用同一按钮/面板。
- 运行时通过 `ui.get("UI/page_start")` 之类的字符串引用符号。
- `text_slots.json`（生成）记录动态文本槽位，运行时按页面坐标匹配后跳过其占位符，
  由 `slot_value` 绘制实时数值（分数、进度等）。

校验（不需要开 Animate）：
```bash
cargo run --manifest-path macroanimate/Cargo.toml --example xfl_dump -- \
    assets/player_ui UI/page_start 0
```
输出会列出该帧所有部件及变换；可用来确认某个实例是否被识别为 `slice`（9-slice）、
`text` 等。

## 7. macroanimate 运行时行为

- 解析：`macroanimate/src/xfl/{model,parse,eval}.rs`。
  - `XflAtlas` 缓存符号时间轴与**符号级** `scale9` 表。
  - `Element::Instance` 带 `matrix`/`firstFrame`/`loop`/`alpha`/`scale9`。
  - `DOMStaticText` 解析成 `TextRun`/`TextDraw`。
- 渲染：`macroanimate/src/xfl/mod.rs`。
  - `draw_matrix` 支持对整棵实例树施加变换。
  - `draw_nine_slice` / `draw_paths_with`：矢量 9-slice 与路径绘制（含 2 点线段描边）。
  - 文本不画进矢量，而是收集成 `text_draws` / `text_draws_matrix`，由上层用
    macroquad overlay 绘制（保证字体一致、文本清晰）。

## 8. 运行时调试开关（dev harness）

在 `src/player_ui/mod.rs`，用于无 Animate 地检查 Flash UI：

| 环境变量 | 作用 |
| --- | --- |
| `MAI2_UI_FLASH=1\|start\|select\|settings\|game\|pause` | 以 Flash UI 模式启动并停在指定页面 |
| `MAI2_UI_FLASH_CLICK="px,py"` | 在页面坐标系合成一次点击，验证命中与跳转 |
| `MAI2_UI_FLASH_DUMP=1` | 打印当前帧的文本绘制列表 |
| `MAI2_UI_FLASH_TRACE=1` | 打印当前页面/帧，观察转场播放 |
| `MAI2_UI_SHOT=path.png` + `MAI2_UI_SHOT_AT=秒` | 运行若干秒后截图 |

无窗口 headless 运行示例：
```bash
MAI2_UI_FLASH=start MAI2_UI_SHOT=/tmp/x.png MAI2_UI_SHOT_AT=2.0 \
  cargo run --quiet --bin lambda_dx_player_ui \
  --no-default-features --features backend-none
```

## 9. 已知问题 / 待办

- Animate 在本机偶发无法启动（进程在、无窗口）；工作方式是「改生成器 → 重生成 →
  用运行时/harness 验证」，而非在 Animate 里操作。
- 页面转场、滚动、toast、缩放/DPI、冒烟测试等仍在推进中。
- 若将来有更多「会被整体拉伸」的符号（面板、toast、header），同样应在**符号**上
  加 `scaleGrid*`，而不是实例。
