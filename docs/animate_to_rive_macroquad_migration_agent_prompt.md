# Animate/Flash → IR → Rive + macroquad 迁移 Agent 提示词

你现在负责把一个原本基于 Adobe Animate/Flash 思维的动画、UI、Vector VFX 系统，逐步迁移成：

    Adobe Animate/Flash
            ↓
      Import / Parse
            ↓
     Animation IR
       ↙       ↘
    Rive      macroquad Native
      ↓             ↓
 UI / Vector    Sprite / Particle
 VFX / Animation Shader / PostFX
            ↓
        macroquad

你的目标不是“兼容 Flash”，而是**逐步把原系统迁移到新的 IR + Rive + macroquad 架构，并最终摆脱 Adobe Animate 运行时/编辑依赖**。

---

## 0. 总原则

必须遵守：

1. **一步一步迁移，每一步都必须可编译、可运行、可验证。**
2. **能直接替换就直接替换，不要为了兼容旧架构长期保留两套系统。**
3. Adobe Animate/Flash 只允许作为“输入资源来源”：
   - 可以读取 FLA/XFL/Animate 导出的资源；
   - 可以解析其 Library、Timeline、Symbol、Shape、Bitmap、Text 等；
   - 可以把这些内容转换成 IR；
   - IR 再转换成 Rive 工程/资源。
4. **一旦进入 IR，就不再要求用户重新切换回 Flash/Animate。**
5. 不要设计成：
       FLA → Flash → Rive → Flash → Rive
   这种循环工作流。
6. 目标工作流必须最终变成：
       Flash/Animate（仅作为历史资源输入）
           ↓
       IR
           ↓
       Rive / macroquad
           ↓
       游戏运行时
7. 如果发现某个 Flash 功能无法合理映射到 Rive：
   - 不要为了 1:1 兼容而污染 Rive；
   - 判断它属于：
     A. Rive
     B. Sprite
     C. Particle
     D. Shader
     E. Post Processing
     F. Rust/Game Logic
   - 然后直接迁移到最合适的目标系统。
8. **允许改变原设计。**
   如果原 Flash 架构不适合实时游戏，就直接重新实现，不要机械翻译。
9. 不要为了“以后可能需要”过度抽象。
10. 每一步完成后都必须说明：
    - 改了什么
    - 为什么这样改
    - 如何验证
    - 下一步是什么
11. 用户偏好直接修改代码，不要长篇理论。除非遇到架构决策，否则优先给：
    - 问题
    - 修改位置
    - 修改后的代码
    - 验证命令

---

# 1. 最终目标架构

最终系统应该接近：

    ┌─────────────────────────────┐
    │       Animation IR          │
    │                             │
    │ Timeline / Symbol / Shape   │
    │ Bitmap / Text / Transform   │
    │ Tween / Mask / Instance     │
    └──────────────┬──────────────┘
                   │
          ┌────────┴────────┐
          ↓                 ↓
        Rive             Native VFX
          │                 │
    ┌─────┼─────┐      ┌────┼────────┐
    │     │     │      │    │        │
   UI   Vector  Anim  Sprite Particle Shader
    │     VFX          │      │        │
    └─────┴────────────┴──────┴────────┘
                       ↓
                    macroquad
                       ↓
              Post Processing
                       ↓
                    Screen

Rive 的职责：

- UI
- Vector VFX
- Timeline Animation
- Tween
- Morph
- Bone / 2D Rig
- State Machine
- 可复用组件
- 少量简单图片元素
- 文本动画

macroquad 原生职责：

- Sprite
- Sprite Sheet
- 大量重复图片
- Particle
- Shader
- RenderTexture
- Bloom
- Blur
- Distortion
- Screen Flash
- 后处理
- Camera
- 大量实例化对象
- 音游实时逻辑

Rust/Game Logic 职责：

- 输入
- 谱面
- 音频时间
- 状态
- 数据
- 事件
- UI Layout
- VFX 生命周期
- 资源管理

---

# 2. 时间系统必须统一

这是音游项目，不能让 Rive、Particle、Shader 各自维护时间。

建立统一 Animation Clock：

    Audio Clock
         ↓
    Game Clock
         ↓
    Animation Clock
         ↓
    ┌────┼─────┐
    ↓    ↓     ↓
   Rive Sprite Particle
         ↓
      Shader

必须支持：

- 正常播放
- 暂停
- 倍速
- 倒放（如果架构允许）
- Seek
- Replay
- 视频导出
- 固定时间步长测试

不要使用多个互相独立的 dt 作为唯一时间来源。

优先支持：

    seek(time)

而不是只有：

    update(dt)

这样以后才能精确同步音游判定和动画。

---

# 3. Animation IR

必须建立一个与 Rive 解耦的中间表示。

推荐结构：

    animation_ir/
    ├── document
    ├── library
    ├── symbol
    ├── timeline
    ├── layer
    ├── instance
    ├── shape
    ├── path
    ├── bitmap
    ├── text
    ├── transform
    ├── keyframe
    ├── tween
    ├── mask
    └── metadata

不要让 IR 直接依赖 Rive API。

IR 应该能够表达：

- Document
- Symbol
- MovieClip
- Graphic
- Instance
- Layer
- Frame
- Keyframe
- Transform
- Shape
- Path
- Fill
- Stroke
- Bitmap
- Text
- Mask
- Tween
- Label
- Animation
- Asset Reference

---

# 4. Flash/Animate 输入

允许输入：

- FLA
- XFL
- Animate 导出的 XFL
- Library 资源
- SVG
- PNG/JPG/WebP
- 其他可以可靠解析的 Animate 资源

但设计上必须：

    Flash Input
         ↓
    Parser
         ↓
    IR

解析层和 IR 分离。

禁止：

    Parser → 直接调用 Rive API

正确：

    Parser → IR → Rive Exporter

---

# 5. 不追求 1:1 Flash 兼容

遇到功能时，按照下面优先级判断：

## A. 直接映射到 Rive

例如：

- Shape
- Path
- Transform
- Tween
- Morph
- Bone
- Timeline
- Text
- Component
- State

## B. 转成 Sprite

例如：

- 复杂逐帧手绘动画
- Rive 不适合表达的大量 raster frame
- 原始序列帧

## C. 转成 Particle

例如：

- 大量 Spark
- Dust
- Burst
- Trail

## D. 转成 Shader

例如：

- Glow
- Distortion
- Color grading
- UV effect
- Wave
- Dissolve

## E. 转成 Post Processing

例如：

- Bloom
- Blur
- Chromatic Aberration
- Screen Flash
- Distortion

## F. 转成 Rust Logic

例如：

- ActionScript
- 游戏逻辑
- 输入
- 数据绑定
- 事件
- 资源管理

---

# 6. MovieClip 的处理

不要强行把每一个 MovieClip 都转换成一个独立文件。

应该根据复用关系决定：

    Symbol
       ↓
    reusable Rive Component / Artboard

嵌套结构应该尽量保留。

例如：

    HitEffect
      ├── Ring
      ├── Star
      └── Flash

如果全部适合 Rive：

    HitEffect.riv

如果部分不适合：

    HitEffect
      ├── Rive: Ring / Star
      ├── Particle: Spark
      └── Shader: Flash/Glow

---

# 7. UI 迁移

UI 不要把 Layout 全塞进 Rive。

推荐：

    Rust
      ↓
    Layout / Data / Logic
      ↓
    Rive
      ↓
    Visual Animation

Rive 负责：

- Button 状态
- Hover
- Press
- Selected
- Disabled
- Visual Transition
- 动态装饰
- Icon Animation
- Vector Animation

Rust 负责：

- 尺寸
- 位置
- Scroll
- 输入
- 数据
- 页面切换
- 业务逻辑

如果 Rive 的 Data Binding 合适，可以使用它，但不要为了 Data Binding 把所有游戏逻辑搬进去。

---

# 8. VFX 迁移

建立统一 VFX API。

例如：

    vfx.spawn("perfect")
    vfx.spawn("critical")
    vfx.spawn("slide_hit")

VFX Manager 内部决定：

    perfect
       ↓
    Rive + Particle + Shader

游戏代码不应该知道它具体用了什么 Renderer。

建议抽象：

    VfxInstance
        ├── RiveVfx
        ├── SpriteVfx
        ├── ParticleVfx
        ├── ShaderVfx
        └── CompositeVfx

---

# 9. Composite VFX

这是核心能力。

一个特效可以由多个系统组成：

    Perfect
      │
      ├── Rive
      │    ├── Ring
      │    ├── Text
      │    └── Star
      │
      ├── Particle
      │    └── Spark
      │
      └── PostFX
           └── Flash

必须让这些组件共享：

- 时间
- Transform
- Color
- Intensity
- Lifetime
- Trigger

---

# 10. Timeline 2.0

不要让 Rive 成为整个 Timeline 系统。

建立自己的 Timeline：

    AnimationTimeline
      │
      ├── RiveTrack
      ├── SpriteTrack
      ├── ParticleTrack
      ├── ShaderTrack
      ├── CameraTrack
      └── EventTrack

例如：

    0ms       100ms       200ms       300ms

    Rive      ───────────────────────
    Sprite            ───────
    Particle                   ─────────
    Shader        ───
    Event                  ●

这样 Rive 是 Timeline 的一个 Renderer，而不是整个动画系统。

---

# 11. Rive Renderer 集成

优先研究：

- Rive Rust runtime
- Rive renderer
- wgpu / OpenGL / WebGL / Metal 等后端
- macroquad 使用的图形上下文
- 是否可以共享 GPU 资源
- RenderTexture / Texture / Surface 的互操作

不要一开始写完整 Rive Renderer。

第一阶段只验证：

    .riv
      ↓
    Rust
      ↓
    Rive Runtime
      ↓
    macroquad
      ↓
    屏幕

最小成功标准：

1. 加载 .riv
2. 找到 Artboard
3. 播放 Animation
4. Seek
5. 绘制到 macroquad
6. 60/120 FPS 稳定
7. 能创建多个实例

---

# 12. 性能策略

必须建立性能预算。

目标设备：

- Desktop
- Android
- macOS
- Windows

默认目标：

- 60 FPS
- 高端设备 120 FPS

不要假设所有动画都应该进入 Rive。

性能原则：

    简单 Vector Animation
        → Rive

    大量相同 Sprite
        → Sprite batching

    大量粒子
        → Particle system

    全屏效果
        → Shader/PostFX

    复杂逐帧 Raster
        → Sprite Sheet

    UI
        → Rive + Rust Layout

---

# 13. 资源格式

建议最终：

    assets/
    ├── rive/
    │   ├── ui/
    │   └── vfx/
    │
    ├── textures/
    │   ├── sprites/
    │   └── atlases/
    │
    ├── particles/
    │
    ├── shaders/
    │
    └── animation/
        └── *.manim

其中 .manim 可以是自己的 Manifest：

    {
      "name": "perfect",
      "rive": "rive/vfx/perfect.riv",
      "sprites": [],
      "particles": [],
      "shaders": [],
      "duration": 0.5
    }

不要把所有资源打包成一个巨大文件。

---

# 14. Editor 方向

如果项目已经存在编辑器：

不要重新制作一个完整 Flash。

优先：

    Existing Editor
         ↓
    Timeline
         ↓
    Animation IR
         ↓
    Rive / Native

如果后续需要自定义 Timeline，可以逐步替换。

编辑器应该最终能够：

- 导入 Animate/Flash
- 显示 IR
- 编辑 Timeline
- 预览 Rive
- 预览 Sprite
- 预览 Particle
- 预览 Shader
- 导出最终资源

---

# 15. MCP / AI 工作流

最终目标是：

    AI
     ↓
    MCP
     ↓
    Animation IR / Rive
     ↓
    Asset
     ↓
    macroquad

AI 不应该操作 Adobe Animate 作为必经步骤。

例如：

    “创建一个 Perfect 判定特效”

应该可以直接：

    MCP
      ↓
    create_animation
      ↓
    create_rive_component
      ↓
    add_shape
      ↓
    add_animation
      ↓
    add_particle
      ↓
    export

而不是：

    AI
      ↓
    打开 Animate
      ↓
    操作 GUI
      ↓
    导出 FLA
      ↓
    再导入 Rive

Animate 只负责历史资源迁移。

---

# 16. 迁移阶段

必须严格按照以下阶段推进。

## Phase 0：代码和资源审计

先不要改代码。

分析：

- 当前项目结构
- Animate/Flash 资源
- FLA/XFL
- Library
- Timeline
- MovieClip
- Symbol
- UI
- VFX
- Sprite
- Shader
- 当前 macroquad renderer
- 当前资源加载
- 当前动画系统

输出：

    migration_plan.md

包含：

- 当前架构
- 目标架构
- 迁移列表
- 风险
- 哪些进入 Rive
- 哪些进入 macroquad
- 哪些直接删除

---

## Phase 1：Animation IR

实现最小 IR：

- Document
- Symbol
- Timeline
- Layer
- Frame
- Transform
- Shape
- Bitmap
- Text
- Instance

先写测试。

目标：

    FLA/XFL
      ↓
    Parser
      ↓
    IR
      ↓
    snapshot/json
      ↓
    测试

此阶段暂时不要接 Rive。

---

## Phase 2：Rive Export

实现：

    IR
      ↓
    Rive Exporter
      ↓
    Rive project/assets

要求：

- Shape
- Transform
- Timeline
- Bitmap
- Text
- Animation

先支持 20% 常用功能。

不要一次做完整 Flash。

---

## Phase 3：Rive Runtime

实现：

    .riv
      ↓
    Rust
      ↓
    macroquad

先做：

- Load
- Artboard
- Animation
- Seek
- Draw

然后做：

- Multiple instances
- State Machine
- Inputs
- Data Binding

---

## Phase 4：Native VFX

实现：

- Sprite
- SpriteSheet
- Particle
- Shader
- PostFX

统一接入：

    AnimationTimeline

---

## Phase 5：Composite VFX

实现：

    Rive
    +
    Sprite
    +
    Particle
    +
    Shader

形成：

    CompositeVfx

然后把一个真实 Flash 特效完整迁移。

建议第一例：

    Perfect / Hit Effect

---

## Phase 6：UI

迁移一个真实页面。

例如：

    Song Select

实现：

- Rust Layout
- Rive Visual
- Data Binding
- Button State
- Animation
- Transition

验证：

- 键鼠
- 触摸
- 窗口缩放
- 移动端

---

## Phase 7：批量迁移

建立：

    animate-importer

流程：

    FLA/XFL
       ↓
    Parser
       ↓
    IR
       ↓
    Classifier
       ↓
    ┌───────────────┐
    ↓               ↓
    Rive          Native
    ↓               ↓
    .riv        Sprite/Particle/
                Shader/PostFX

然后批量迁移资源。

---

## Phase 8：删除旧系统

只有新系统已经通过验证之后：

删除：

- Flash runtime dependency
- 旧 Animation Runtime
- 不再使用的转换器
- 重复资源
- 旧 UI renderer
- 旧 VFX renderer

不要为了兼容旧代码长期保留旧架构。

---

# 17. 每一个 Phase 的执行方式

你必须采用：

    Inspect
      ↓
    Plan
      ↓
    Implement
      ↓
    Build
      ↓
    Test
      ↓
    Benchmark
      ↓
    Commit
      ↓
    Next Phase

每个阶段结束必须有明确结果。

禁止：

    一次修改几十个模块
    ↓
    最后一起编译
    ↓
    不知道哪里坏了

---

# 18. 每一步都必须可回滚

每个 Phase 完成后创建 Git commit。

推荐：

    migration/phase-00-audit
    migration/phase-01-ir
    migration/phase-02-rive-export
    migration/phase-03-rive-runtime
    migration/phase-04-native-vfx
    migration/phase-05-composite-vfx
    migration/phase-06-ui
    migration/phase-07-bulk-import
    migration/phase-08-remove-flash

---

# 19. 验收标准

最终必须达到：

## 输入

可以：

    FLA/XFL
       ↓
    IR

## 编辑/生成

可以：

    IR
       ↓
    Rive

不需要重新打开 Flash。

## Runtime

可以：

    Rive
    Sprite
    Particle
    Shader
    PostFX

统一运行在：

    macroquad

## Timeline

所有动画共享：

    Animation Clock

## VFX

支持：

    Composite VFX

## UI

支持：

    Rust Logic
    +
    Rive Visual

## AI

可以通过 MCP：

    创建 / 修改 / 导出 Rive / IR 资源

## 最终状态

Adobe Animate 不再是运行时依赖。

它只作为：

    历史资源
    ↓
    Importer
    ↓
    IR
    ↓
    新系统

---

# 20. Agent 的工作纪律

每次开始工作：

1. 先读取当前项目状态。
2. 不要假设已有架构。
3. 搜索实际代码。
4. 找到真实入口。
5. 修改最少的文件。
6. 编译/测试。
7. 出错就先修当前错误，不继续堆新功能。
8. 不要为了“未来扩展”创建大量抽象。
9. 不要引入用户没有要求的框架。
10. 不要擅自替换已经能工作的模块。
11. 如果必须替换，直接替换，并删除旧实现。
12. 不要保留无意义的 compatibility layer。
13. 不要创建 Flash → Rive → Flash 的循环依赖。
14. 不要把 Rive 当成所有 VFX 的容器。
15. 不要把 Particle / Shader / PostFX 强行塞入 Rive。
16. 不要把游戏逻辑塞进 Rive。
17. 音频时间永远是音游动画同步的重要参考。
18. 每完成一个阶段都必须给出验证结果。
19. 如果发现当前设计不合理，直接提出替换方案并实施，而不是继续沿着错误架构堆代码。
20. **优先让系统真正运行，再完善边缘功能。**

---

# 21. Agent 首次执行任务

现在不要直接开始大规模迁移。

第一步只做：

    1. 检查项目目录
    2. 检查 Cargo workspace
    3. 找到当前 Animate/Flash 资源
    4. 找到当前动画系统
    5. 找到 macroquad renderer
    6. 找到资源加载系统
    7. 找到 UI 系统
    8. 找到 VFX 系统
    9. 找到现有 Timeline/Animation 代码
    10. 搜索现有 Rive / Flash / FLA / XFL 相关代码
    11. 检查当前依赖
    12. 检查测试
    13. 检查构建方式

然后只输出：

    A. 当前架构
    B. 当前 Flash/Animate 依赖
    C. 当前 Animation/VFX/UI 架构
    D. 需要迁移的模块
    E. Rive 候选模块
    F. macroquad Native 候选模块
    G. 需要删除的旧模块
    H. 风险
    I. Phase 0 → Phase 8 实际迁移计划

**不要在第一次执行中修改代码。**

等审计完成后，再从 Phase 1 开始。

---

# 最终原则

不要问：

    “怎么把 Flash 原封不动搬到 Rive？”

而应该问：

    “Flash 中哪些东西值得保留？”

然后：

    值得保留的视觉语义
            ↓
          Animation IR
            ↓
       ┌────┴────┐
       ↓         ↓
     Rive      Native
       ↓         ↓
      UI/VFX   Sprite/
      Anim     Particle/
               Shader/
               PostFX
            ↓
         macroquad

目标不是：

    “让 Flash 永远活着”

目标是：

    **把现有 Flash/Animate 资产迁移出来，
    建立一个不依赖 Flash 的、
    面向 Rive + macroquad 的现代动画系统。**
