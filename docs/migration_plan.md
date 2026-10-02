# Flash/Animate → IR → Rive + macroquad 迁移计划（Phase 0 审计）

> 本文由 `docs/animate_to_rive_macroquad_migration_agent_prompt.md` 的 Phase 0 产出。
> 分支 `player_preview`，基线 `cargo check --bins` 通过（仅 warning）。
> 本阶段**未修改任何代码**。

---

## A. 当前架构

Cargo workspace（`Cargo.toml`）：

- 根 crate `lambda_dx_pad_preview`（edition 2024），4 个 bin：
  - `lambda_dx_pad_preview` → `src/main.rs`（音游 pad 预览）
  - `lambda_dx_player_ui` → `src/player_ui_main.rs`（纯 macroquad 前端播放器）
  - `macroanimate_test` → `src/macroanimate_test.rs`
  - `autoplay_gen` → `src/autoplay_gen/main.rs`（headless 全 Perfect 生成器）
- workspace member：`lnmai-core`（Lean FFI 判定内核，`backend-lean` 默认）。
- excluded：`lnmai-rust`（纯 Rust 内核 shim，需 `backend-rust`）。
- 本地 path 依赖：`macroanimate = { path = "macroanimate" }`。

图形/运行时全部基于 **macroquad 0.4**（`default-features = false`），音频 `rodio`，
UI 面板 `egui-macroquad`，JPEG 用 `image`。

`macroanimate`（本地 crate，v0.2.1）是当前唯一的“Flash 资产解析 + 渲染”层：

- `xfl/`：`DOMDocument.xml` + `LIBRARY/*.xml` 解析（`parse.rs`）、时间轴求值
  （`eval.rs`，按 frame 采样成 `DrawPart`）、矢量/位图/9-slice 渲染（`mod.rs`）。
- `sparrow_atlas.rs`：Animate/Starling Sparrow XML 图集解析。
- `texture_atlas.rs`：Animate texture atlas（Starling `SI`/`ASI`）解析 + mesh 绘制。

游戏侧代码：

- `src/app/`：`anim.rs`（加载 `ui` / `player_ui` 两个 XFL 工程，thread-local + 热重载）、
  `chart.rs` / `maidata.rs` / `maichart.rs`（谱面）、`audio.rs`、`params.rs`、
  `pad_svg.rs`、`guide.rs`、`slide/`、`slide_render.rs`、`ui.rs`（`mask.frag` shader）。
- `src/player/`：真正的 pad 音游预览。`engine.rs`、`state.rs`、`layout.rs`、
  `render/`（`notes.rs`、`pad.rs`、`ring.rs`、`hit_fx.rs`、`feedback.rs`、`slide.rs`、
  `textures.rs`、`progress.rs`、`touch.rs`、`skin.rs`）、`audio`、`video`、
  `export_video`、`hud`、`sfx`、`cues`、`autoplay`。
- `src/player_ui/`：纯 macroquad 前端。
  - `mod.rs`：boot + `UiCtx` + 每帧主循环；`draw_flash()` 走 Flash 页面模式。
  - `flash.rs`：XFL 页面的 hit region / 动作映射（`PAGES`、`Hit`、`Action`）。
  - `pages/`：**另一套自绘 UI**（`start` / `song_select` / `settings` / `gameplay` / `pause`）。
  - `library.rs`（曲库扫描 / 封面）、`anim.rs`（缓动）、`draw.rs`（绘图原语）、
    `state.rs`、`theme.rs`、`input.rs`、`perf.rs`。
- `src/core/`：`none.rs`（`backend-none` 判定桩）。

离线工具 / 参考资料（不参与 Rust 构建）：

- `tools/make_player_ui_xfl.py`、`make_notes_xfl.py`、`make_tap_hit_xfl.py`、
  `fix_xfl_symbol_instances.py`：**用脚本生成 XFL XML**。
- `XFLProcessor/`（git-ignored）：AS3 写的 `.dat` 资源还原工具，离线。
- `pure/`（git-ignored）：旧 worktree。

---

## B. 当前 Flash/Animate 依赖

**运行期依赖 = `macroanimate` 的 XFL 运行时**：

- `macroanimate::XflAsset::load()` 在启动时解析：
  - `assets/ui/`（`EFFECT_PROJECT`）→ 特效库，按名字取 `ui.get("tap_perfect")`。
  - `assets/player_ui/`（`PLAYER_PROJECT`）→ 前端页面，`ui.get("UI/page_start")`。
- 解析能力（`macroanimate/src/xfl/`）：
  - Document / Scene / Library Symbol（Graphic / MovieClip / Button）/ Include。
  - Layer / Frame / keyframe / `tweenType="motion"`（线性插值）。
  - Element：Bitmap / Symbol Instance / Shape / Group / Static Text。
  - Shape：`fills` / `strokes` / `edges`（twips），矢量按多边形三角扇绘制。
  - 位图：内嵌 `bin/*.dat`（`dat.rs` 解码 PNG/JPEG/BMP 等）或外部图片。
  - 9-slice（`scale9Grid`，symbol 级）、Static Text（不栅格化，交给调用方字体）。
- 运行期消费点，仅两处：
  - `src/player/render/hit_fx.rs:31` `draw_flash()` → 播放 `tap_perfect` 类特效。
  - `src/player_ui/mod.rs:277` `draw_flash()` → 播放 `player_ui` 页面 + 控件状态叠加 + 文本槽。
- **没有** SWF 运行时、**没有** ActionScript 执行、**没有** Flash Player 依赖。
- `tools/*.py` 在构建前/离线生成 XFL（这是要迁移掉的“编辑器/创作链路”）。

资产：

| 路径 | 内容 | 状态 |
|---|---|---|
| `assets/ui/` | XFL 特效工程（tap_perfect、TapPerfect、Hex、ring/flash/spark shape、Tween） | 运行期使用 |
| `assets/effects/tap_hit/` | 256×256 XFL 命中特效（tap_hit + 3 shape） | 新，未被引用 |
| `assets/player_ui/` | XFL UI 页面 + ~40 个 `UI/ui_*` 控件 + `text_slots.json` | 运行期使用（`MAI2_UI_FLASH`） |
| `assets/notes/` | 由 `make_notes_xfl.py` 生成的 XFL 音符皮肤 | 需确认渲染路径 |
| `assets/Skins/classic/` | 音符 PNG 皮肤 | 运行期使用 |
| `assets/charts/` | 歌曲（maidata.txt + 音频 + 封面 + pv） | 运行期使用 |
| `assets/mjdataplay/` | MajdataView 参考导出（Animate 工程） | git-ignored，参考 |
| `assets/RECOVER_player_ui/` | 损坏/恢复的 player_ui 备份 | untracked，参考 |
| `assets/Sfx/`、`demo.mp3`、`bg.mp4`、`mask.frag`、`pad.svg` | 音效/音乐/视频/后处理 shader/pad 矢量 | 运行期使用 |

---

## C. 当前 Animation / VFX / UI 架构

### Animation

- **没有统一 Animation IR，也没有统一 Animation Clock。**
- XFL 播放是“按帧号采样”：调用方用各自的时钟算出 `frame`，再 `clip.draw(frame, xf)`。
  - `player_ui/mod.rs`：`FxMode::{Loop, Once, Reverse}` + `flash_frame()`，时钟是
    `flash_start` / `flash_enter`（`get_time()`）。
  - `hit_fx.rs`：自己的 `app.fx_clock()` + 每个 hit 的 `started` / `duration` 算帧。
- 自绘页面用 `player_ui/anim.rs`（easing + `Smooth`）与 `pages/*` 的 `page_born`。
- 音频时钟在 `player::audio` / `PadPreviewState`，与动画时钟**未打通**。

### VFX

- `player/render/hit_fx.rs`：两条路径 —— Animate clip 或**程序化** ring + sparks + flash。
- `player/render/feedback.rs`（判定反馈）、`ring.rs`、`slide.rs`、`touch.rs` 等
  都是 macroquad 直接绘图（`draw_circle` / `draw_line` / mesh）。
- `mask.frag` + `app/ui.rs` 的 `load_mask_material`：页面切换的遮罩 wipe（后处理雏形）。
- 没有粒子系统、没有统一 `vfx.spawn(...)` API、没有 Composite VFX。

### UI

- **两套并行 UI**：
  1. 自绘 Rust 页面 `src/player_ui/pages/*`（默认）。
  2. Flash XFL 页面 `src/player_ui/flash.rs` + `draw_flash()`（`MAI2_UI_FLASH=1` 启用）。
- 二者的布局/交互/状态逻辑在 `flash.rs` 与各 page 间重复，正是提示词要消除的“两套系统”。

---

## D. 需要迁移的模块

| 模块 | 现状 | 目标 |
|---|---|---|
| XFL 解析（`macroanimate/src/xfl/parse.rs`） | 运行期解析 + 直接求值渲染 | 拆成 Parser → **Animation IR**（不含渲染） |
| XFL 求值（`eval.rs`） | 直接产出 `DrawPart` 给 macroquad | IR 的 timeline 采样器 |
| XFL 渲染（`mod.rs`） | macroquad 矢量/位图/9-slice | 删除或降级为 native 备用 |
| `assets/ui/`、`effects/tap_hit/` 特效 | Animate 工程 | IR → Rive / Particle / Shader |
| `assets/player_ui/` 页面/控件 | Animate 工程 | IR → Rive（视觉）+ Rust 布局 |
| `tools/make_*_xfl.py` | 手写 XFL 生成 | IR 生成 / MCP 工具 |
| `player/render/hit_fx.rs` | Flash + 程序化双路径 | 统一 `VfxInstance` / CompositeVfx |
| `player_ui/flash.rs` + `draw_flash()` | Flash UI 分叉 | 合并为 Rive visual + Rust layout |
| 时间源 | 多个独立时钟 | 统一 Animation Clock（seek-based） |

---

## E. Rive 候选模块

- UI 控件与视觉：按钮/标签/开关/滑条/面板/Hover/Press/Selected/Disabled，页面切换动画。
- 矢量 VFX：`ui` 工程里的 ring / spark / flash / Hex / `tap_perfect` / Tween 动画。
- Timeline 动画、Tween、Morph、2D 骨骼/rig、State Machine。
- 简单文本动画（不含动态数值文本——那仍归 Rust）。
- 少量简单图片元素。

## F. macroquad Native 候选模块

- Sprite / SpriteSheet：`assets/Skins/classic/*`、`assets/notes/` 音符、`texture_atlas` 图集。
- 大量重复图片 / 实例化音符。
- Particle：sparks、dust、burst、trail（含 `tap_hit` 的 spark 部分）。
- Shader：`mask.frag`、glow、flash、distortion、wave、dissolve。
- PostFX：Bloom、Blur、Chromatic Aberration、Screen Flash、切换 wipe。
- Camera / RenderTexture / 视频背景（`bg.mp4`、`player/video.rs`）。
- 音游实时逻辑与判定可视化。

## G. 需要删除的旧模块（Phase 8，验证通过后）

- `macroanimate` 的**运行期渲染**（保留 parser 作 importer）。
- `src/player_ui/flash.rs`、`player_ui/mod.rs::draw_flash` 的分叉逻辑。
- `player/render/hit_fx.rs` 中旧 Flash 路径（procedural fallback 可暂留）。
- `tools/make_*_xfl.py`（被 IR / MCP 工具取代）。
- 重复资源与备份：`assets/RECOVER_player_ui/`、`assets/mjdataplay/`（参考后清理）。
- 旧 worktree `pure/`、离线 `XFLProcessor/`（视需要保留为 importer 参考）。

---

## H. 风险

1. **Rive Rust runtime 与 macroquad GPU 上下文互操作**（最高风险）。
   macroquad 走 miniquad/OpenGL；Rive renderer 走自己的 GPU 抽象（wgpu/Metal/GL）。
   需要 RenderTexture/Surface 互通方案，或把 Rive 渲到离屏再 blit。Phase 3 必须先做可行性探针。
2. **`.riv` 写入器**。~~Rive 的 `.riv` 是二进制格式，官方没有稳定的公开 writer。~~
   **已解决**：改用 OpenRive 的 MCP server 创作并 `export_riv`（Phase 2 已跑通）。
   注意 OpenRive 监听随机 loopback 端口，导出工具需自动发现（已实现）。
3. **CJK 文本**。当前 XFL 文本由 macroquad + 系统字体渲染，配合 `text_slots.json` 动态槽。
   Rive 内文本需要内嵌字体，动态数值文本应留在 Rust（避免把数据绑定变成游戏逻辑）。
4. **解析完整度**。当前 parser 不支持 mask、filter、motion guide、button 状态、ActionScript、
   音频时间轴、非线性缓动（motion tween 仅线性）。批量迁移前要实测资产覆盖率。
5. **时间系统统一**。`fx_clock` / `flash_*` / 页面 `page_born` / 音频时钟各自为政，需收敛为
   一个 `Animation Clock`，并原生支持 `seek(time)` 而非只有 `update(dt)`。
6. **性能**。Rive + macroquad 双渲染器、多实例、移动端（Android/macOS/Windows）预算需提前定。
7. **两套 UI 并行**期间点：迁移中若同时维护自绘页与 Rive 页，成本翻倍——应按页面逐个替换后立即删旧页。

---

## I. Phase 0 → Phase 8 迁移计划

### Phase 0（本文件）— 审计
- 产出 `docs/migration_plan.md`（A–I）。**不改代码**。
- 验证：`cargo check --bins` 通过（已确认）。
- commit：`migration/phase-00-audit`。

### Phase 1 — Animation IR
- 新建 workspace member（如 `animation_ir/`）：Document / Symbol / Timeline / Layer /
  Frame / Keyframe / Transform / Shape / Path / Fill / Stroke / Bitmap / Text / Instance /
  Mask / Tween / Label / Animation / AssetReference。
- 把 `macroanimate` 的 parse 结果映射进 IR（Parser 与 IR 解耦，IR 不依赖 Rive）。
- 先写测试：XFL → IR → JSON snapshot。
- 验证：`cargo test -p animation_ir`；对 `assets/ui`、`assets/player_ui`、`assets/effects/tap_hit` 快照。
- commit：`migration/phase-01-ir`。

**已完成（Phase 1 落地）**：

- 新增 `animation_ir/` crate（workspace member），`src/lib.rs` 定义 IR，`src/from_xfl.rs`
  把 `macroanimate::XflAtlas` 转成 IR（只读 parser 模型，IR 本身不依赖 Rive/renderer）。
- 覆盖：`SymbolKind`、`LoopMode`、`AnimTarget`、`Timeline`/`Layer`/`Frame`/`TweenKind`、
  `Element`（Bitmap / Instance / Shape / Group / Text）、`ShapePath`/`Stroke`/`Color`、
  `Text`/`TextAlign`、`Matrix`、`Bitmap`、`Animation`。
- 未覆盖（延后，parser 也尚未产出）：Mask、filter、motion guide、Button 状态、ActionScript、
  音频时间轴、非线性缓动。IR 结构预留了扩展位（`TweenKind`、`Element` 为 enum）。
- 测试：`animation_ir/tests/xfl_ir.rs`（5 项）+ `examples/snapshot.rs`。
  实测：`player_ui` 44 symbols / 157 shape / 84 instance / 72 text；
  `ui` 9 symbols / 178 instance；`tap_hit` 4 symbols。
- 验证：`cargo test -p animation_ir` 全绿；`cargo check --workspace` 通过。


### Phase 2 — IR → Rive Export
- 先确定 `.riv` 产出策略（风险 H2），再实现最小导出：
  Shape / Transform / Timeline / Bitmap / Text / Animation（先覆盖 ~20% 常用功能）。
- 验证：导出物能在 Rive 编辑器打开并播放（人工）+ 单测快照。
- commit：`migration/phase-02-rive-export`。

**已完成（Phase 2 落地；写入策略已确定）**：

- **`.riv` 写入策略 = 经 OpenRive MCP 创作**，不再需要手写二进制 writer（消除风险 H2）。
  `OpenRive.app`（Electrobun 桌面版）内置 MCP server：
  - 随机 loopback 端口、Streamable HTTP、端点 `/api/mcp`、loopback 无需鉴权。
  - 30 个工具，覆盖 IR 所需：`create_project` / `add_artboard` / `add_path` /
    `add_shape` / `add_text` / `add_group` / `set_properties` / `add_timeline` /
    `add_keyframes` / `add_state_machine` / `add_property` / `add_transition` /
    `add_listener` / `export_riv` / `import_riv` / `inspect_riv` / `get_project` …
- 新增 `tools/openrive_mcp.py`：stdlib MCP 客户端（自动发现端口 + initialize + tools/call）。
- 新增 `tools/ir_to_rive.py`：IR JSON → OpenRive 工程 → `.riv`。
  - shape → `add_path`（XFL 曲线已被 parser 打成多边形）；instance → Rive group +
    内联子几何 + 按 XFL 补间加 x/y/rotation/scale/opacity 关键帧；text → `add_text`；
    group 递归；bitmap 暂跳过并计数。
- 首个真实产物：`assets/effects/tap_hit` → `assets/rive/vfx/tap_hit.riv`
  （256×256 artboard，10 个 group、Timeline fps 24、60 条 keyed 属性）。
- 验证：`file` 识别 magic `RIVE`；`inspect_riv` 确认 artboard/节点/时间轴/关键帧结构。
- 未覆盖：bitmap 图片导入（MCP 无该工具）、mask/filter、非线性缓动、State Machine
  行为（输入/过渡）、scene 主时间轴 label。


### Phase 3 — Rive Runtime（macroquad）
- 可行性探针：`.riv` → Rust runtime → RenderTexture → macroquad 上屏；
  最小成功标准：加载 / 找 Artboard / 播放 / Seek / 绘制 / 稳定帧率 / 多实例。
- 再做 State Machine / Inputs / Data Binding。
- 验证：一个独立 bin 或 `macroanimate_test` 式探针 + 截图。
- commit：`migration/phase-03-rive-runtime`。

### Phase 4 — Native VFX
- 统一 `AnimationTimeline`（RiveTrack/SpriteTrack/ParticleTrack/ShaderTrack/CameraTrack/EventTrack）
  与 Animation Clock（支持 pause/倍速/seek/replay/固定步长）。
- 实现 Sprite / SpriteSheet / Particle / Shader / PostFX（从 `Skins`、`mask.frag` 起步）。
- 验证：`cargo test` + 视觉截图 harness（现有 `MAI2_UI_SHOT`）。
- commit：`migration/phase-04-native-vfx`。

### Phase 5 — Composite VFX
- `VfxInstance`：RiveVfx / SpriteVfx / ParticleVfx / ShaderVfx / CompositeVfx，
  共享 time / transform / color / intensity / lifetime / trigger。
- 首个真实迁移：`tap_hit` / `tap_perfect`（Rive ring+star+text + Particle spark + PostFX flash）。
- 对外只暴露 `vfx.spawn("perfect")`。
- 验证：对 `assets/effects/tap_hit/` 与 `assets/ui` 逐帧对比截图。
- commit：`migration/phase-05-composite-vfx`。

### Phase 6 — UI
- 迁移一个真实页面（建议 `song_select`）：Rust 负责尺寸/位置/滚动/输入/数据/页面切换，
  Rive 负责视觉/状态/过渡。
- 验证：键鼠 + 触摸 + 窗口缩放 + 移动端；替换后立即删除对应自绘页。
- commit：`migration/phase-06-ui`。

### Phase 7 — 批量迁移
- `animate-importer`：FLA/XFL → Parser → IR → Classifier →（Rive | Native）。
- 批量迁移 `assets/ui`、`assets/effects`、`assets/player_ui` 剩余资产。
- 验证：清单化覆盖报告（哪些进 Rive、哪些进 Sprite/Particle/Shader/PostFX）。
- commit：`migration/phase-07-bulk-import`。

### Phase 8 — 删除旧系统
- 删除 Flash runtime 依赖（`macroanimate` 渲染层）、`flash.rs`、`make_*_xfl.py`、
  旧 UI/VFX renderer、重复资源。
- 验证：全量测试 + 手动回归；确认 Adobe Animate 不再是运行期依赖。
- commit：`migration/phase-08-remove-flash`。

---

## 已确认决策（用户）

1. **macroquad 原生优先**：凡 macroquad 能直接表达的（sprite / spritesheet / particle /
   shader / postfx / 大量实例化音符），直接走 macroquad 原生渲染，不进 Rive。
   分类优先级见 E / F。
2. **Rive 创作经 MCP**：`.riv` 的生成/编辑通过 MCP 连接 Rive（openrive）完成，
   不再依赖手写 `.riv` writer。注意：当前会话未检测到 Rive MCP，Phase 2 前需配置。

## 下一步（Phase 1 起点建议）

1. 先做 Phase 2 的 **Rive 写入策略调研**（风险 H2），因为它决定 IR 的 shape/animation 表达粒度。
2. 同时做 Phase 3 的 **macroquad × Rive 可行性探针**（风险 H1）。
3. 二者有结论后，再落地 Phase 1 的 IR crate，避免 IR 结构返工。
