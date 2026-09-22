# LambdaDX Pad 预览（独立提取版）

从 `Mai2Chart/demo/macroquad_sim` 的 `lambda_dx_player` bin 中，单独抽取出
**pad 预览**（分层圆盘 + 音符飞行 + 触摸高亮 + 判定文字），做成一个可独立编译运行的
macroquad 小程序。目标是在不启动编辑器、曲库、裁决引擎与 egui 前端的前提下，
完整复现 player 里 pad 的**视觉与运动逻辑**。

---

## 1. 快速开始

```bash
cargo run            # 打开 1280x760 窗口，显示内置默认曲目
cargo build --release

# 命令行指定谱面：可以是 .maichart 目录（含 maidata.txt / chart.json）或谱面文件
cargo run -- ~/.maichart/324_Jack-the-Ripper◆      # 目录优先读 maidata.txt
cargo run -- ./maidata.txt                          # simai 文本
cargo run -- ./chart.json ./track.mp3 --diff 4     # maichart JSON + 音频 + 难度
cargo run -- --help
```

命令行参数（`src/app/cli.rs`）：

| 参数 | 作用 |
|---|---|
| `CHART`（位置参数 1） | 谱面目录（含 `maidata.txt` / `chart.json`）或谱面文件（`.txt` / `.json`） |
| `AUDIO`（位置参数 2） | 音频文件（mp3/wav），覆盖自动检测 |
| `-c, --chart <PATH>` | 同位置参数 1 |
| `-a, --audio <PATH>` | 同位置参数 2 |
| `-d, --diff <N>` | 指定难度编号（否则用 `MAICHART_DIFF` 或最高难度） |
| `-h, --help` | 打印帮助（不会开窗） |

音频选择优先级：`--audio` > 谱面目录里的 `track.mp3`/`track.wav`/`demo.*` >
内置 `assets`。谱面加载见 `chart::load_chart_from_path`（目录优先 `maidata.txt`，
其次 `chart.json`；文件按扩展名：`.txt` 走 simai，`.json` 先 maichart 再内部格式）。

操作：

| 输入 | 作用 |
|---|---|
| 鼠标左键 / 触摸 | 命中 pad 分区高亮（蓝），并触发判定文字 |
| `1`–`8` | 模拟 A 环 8 条轨道的按键打击 |
| `T` | 模拟中心 C 区打击 |
| `Space` | 播放 / 暂停（暂停时用 `timeline_view_time` 定格画面） |
| `R` | 从头播放 |
| `Home` | 回到 0 秒 |
| `←` / `→` | 后退 / 前进 1 秒（seek） |
| `↑` / `↓` | 播放速度 ±0.1x |
| `A` | 开关音频 |

环境变量：`MAI2_UI_SCALE`（0.7–2.4 缩放）、`MAI2_MOBILE_UI=1`（移动端尺寸）、
`MAICHART_DIFF=1..5`（选择难度，默认取最高难）。

默认曲目为 **Jack-the-Ripper◆**（`assets/charts/jack_ripper/`，来自
`~/.maichart/324_Jack-the-Ripper◆`），载入 `chart.json` 与 `track.mp3`。

### 依赖

`macroquad 0.4`、`serde`/`serde_json`、`hound`（WAV 解码）、`minimp3`（MP3 解码）、
`roxmltree`（解析 `pad.svg`）、`rodio`（BGM 播放）。

### 资源（`assets/`）

| 文件 | 用途 |
|---|---|
| `charts/jack_ripper/chart.json` | **默认曲目**（maichart 格式，见 §5.5） |
| `charts/jack_ripper/track.mp3` | **默认曲目 BGM** |
| `pad.svg` | 分区多边形定义（`<g id="touch">` + `<g id="center">`） |
| `generated_chart.json` | 备用谱面（旧版“秒”格式，载入时自动迁移为“小节”） |
| `demo.mp3` | 备用 BGM |
| `Skins/classic/*.png` | tap/hold/touch/slide/star/wifi 皮肤 |
| `Sfx/answer.wav` | 判定点音效（tap / hold 头 / hold 尾） |
| `mask.frag` | touch-hold 进度环的着色器 |

谱面加载优先级（未指定命令行路径时）：默认 maichart `chart.json` → 输出目录的
`latest_chart.json` → `generated_chart.json` → 内置 `fallback_chart()`。命令行给定路径时
见 §5.6（目录优先 `maidata.txt`）。

---

## 2. 目录结构

```
src/
├── main.rs                     # 入口：加载资源 + 主循环
├── app/                        # ← 从原项目复制的“基础层”（近原样）
│   ├── mod.rs                  #   模块声明 + window_conf 再导出
│   ├── types.rs                #   数据类型、常量、时间换算、径向运动公式
│   ├── types/zone.rs           #   PadZone（A1..A8/B1..B8/C/D1..D8/E1..E8）
│   ├── pad_svg.rs              #   pad.svg 解析、SVG↔屏幕坐标、多边形填充/描边
│   ├── slide/mod.rs            #   滑条路径模块
│   ├── slide/path.rs           #   各 slide 形状（Q/P/S/Z/Wifi…）的路径点生成
│   ├── slide/segmentation.rs   #   把路径采样成 trail bar 并对 zone 分段
│   ├── slide_render.rs         #   滑条绘制（trail tiles + star）
│   ├── chart.rs                #   谱面加载 + 格式迁移 + fallback
│   ├── cli.rs                  #   命令行参数解析（谱面/音频/难度）
│   ├── maichart.rs             #   .maichart/chart.json 解析 → ChartDoc
│   ├── maidata.rs              #   simai maidata.txt → ChartDoc
│   ├── beat_format.rs          #   beat 格式读写（兼容）
│   ├── audio.rs                #   MP3/WAV 解码、变速 PCM、rodio BGM
│   ├── platform.rs             #   资源路径解析
│   └── ui.rs                   #   window_conf、9-slice hold、mask material
├── simai/                      # ← 内置纯 Rust simai 解析器（maisimai, MIT）
│   ├── mod.rs
│   ├── model.rs                #   SimaiNote / SimaiChart / SimaiFile
│   └── parser.rs               #   maidata.txt 解析
└── player/                     # ← 本项目新写的“预览层”
    ├── mod.rs
    ├── state.rs                #   PadPreviewState（精简版 PlayerState）
    ├── cues.rs                 #   判定点音效时间轴（tap/hold 头尾）
    ├── layout.rs               #   面板布局 + pad 几何
    ├── render/                 #   渲染，按职责细分
    │   ├── mod.rs              #   draw_pad_panel 总编排 + 运动模型总说明
    │   ├── scale.rs            #   ui_scale
    │   ├── pad.rs              #   背景 / 圆盘 / 分区 / A 环指示点
    │   ├── timing.rs           #   每音符的 NoteTiming（dt、可见性）
    │   ├── notes.rs            #   遍历谱面 + 按类型分发
    │   ├── ring.rs             #   tap / hold（径向飞行）
    │   ├── touch.rs            #   touch / touch-hold（整段淡入+移动）
    │   ├── slide.rs            #   slide 适配层
    │   ├── feedback.rs         #   判定文字覆盖层
    │   └── textures.rs         #   皮肤纹理加载
    └── input/
        ├── mod.rs
        ├── pointer.rs          #   触摸/鼠标 → 分区命中、active 集合
        ├── keyboard.rs         #   轨道键 + 播放热键
        └── hit.rs              #   简易“判定”文字
```

设计原则：**基础层（`app/`）尽量原样复制**，保证几何与渲染与上游一致；
**预览层（`player/`）重新拆分**，把原来 1200 行的 `player/ui.rs` 拆成
`render/*` 若干小模块，把 `player/input.rs` 拆成 `input/*`。

---

## 3. 实现路径（Extraction Path）

原 `lambda_dx_player` 的 `src/player/player.rs` 主循环做了很多事，我们只保留与 pad
预览相关的部分：

```
原 bin 主循环                          本预览主循环 (main.rs)
─────────────────────────────         ──────────────────────────
clear_background                       clear_background
compute_layout / compute_pad_geom      player::layout::compute_layout / compute_pad_geom
handle_global_hotkeys                  player::input::handle_global_hotkeys
handle_lane_input                      player::input::handle_lane_input
collect_pointer_events                 player::input::collect_pointer_events
handle_touch_controls                  player::input::handle_touch_controls
service_audio                          app::audio::service_audio
step_judge_engine          ← 删除（裁决引擎/FFI）
draw_layout → draw_pad_panel           player::render::draw_pad_panel
tick_feedback                          app.tick_feedback
egui_ui / egui draw        ← 删除（前端）
finalize_playback_start                app.finalize_playback_start
next_frame                             next_frame
```

**裁剪掉的东西**：曲库/选曲、设置页、暂停模态、模拟器编辑、slide 轨迹编辑、
模板系统、裁决引擎（`lnmai-core` FFI）、autoplay、SFX、波形 FFT。

**保留并复制的基础层**：`types`、`pad_svg`、`slide/path`、`slide/segmentation`、
`slide_render`、`chart`、`beat_format`、`platform`，以及 `ui.rs` 中的
`window_conf` / `draw_hold_9slice_segment` / `load_mask_material`。

**新写的预览层**：`PadPreviewState`（只保留 `draw_pad_panel` 与音频真正读取的字段）、
`render/*`、`input/*`、`layout.rs`。原 `draw_pad_panel` 中与编辑/裁决相关的整段分支
（如 slide 轨迹编辑、slide 判定色带）被移除以解耦。

### 数据流

```
assets → chart::load_generated_chart ─┐
assets → audio::load_audio_pcm_from_assets ─┤→ PadPreviewState
assets → pad_svg::PadSvgDef::from_svg_str ─┘
                                            │
              ┌─────────────────────────────┘
              ▼
 每帧: layout → input → audio.service → render → feedback.tick
```

---

## 4. 坐标系与 Pad 几何

- 全部使用**屏幕像素**绘制。
- `PadGeom { cx, cy, outer_r }` 描述圆盘；`layout::compute_pad_geom` 取面板较短边
  的 `0.42` 作为 `outer_r`。
- `pad_svg.rs` 维护 SVG `viewBox`（背景圆心 `422.9, 348.8`，半径 `326.57`）与屏幕
  圆的仿射映射：
  - `svg_to_screen`：`scale = outer_r / 326.57`，把 SVG 点平移到 `(cx, cy)`。
  - `screen_to_svg`：其逆变换，用于命中测试。
- 分区多边形保留 SVG 形状，仅缩放/平移，因此**绘制与命中测试共用同一变换**。
- 所有“环上”位置（tap 落点、星标、slide 起点）由统一公式给出：

  ```
  angle  = -π/2 + PAD_ROTATION_RAD + (zone-1) · 2π/8      // PAD_ROTATION_RAD = π/8
  target = spawn_center + (cos angle, sin angle) · (outer_r + TAP_TARGET_OFFSET)
  ```

  其中 `spawn_center` 取 SVG C 区质心（`pad_visual_center`），即音符的“出生点”。

---

## 5. 时间系统

谱面里音符的 `time` / `hold_duration` / `slide_duration` 单位是**小节（measure）**，
`1.0 = 第一拍`。渲染前统一换算为**秒**：

| 函数（`app/types.rs`） | 作用 |
|---|---|
| `measure_to_secs(m, bpms)` | 小节 → 秒（支持多 BPM 分段） |
| `secs_to_measure(t, bpms)` | 秒 → 小节（逆变换） |
| `mdur_to_secs(d, start_m, bpms)` | 以 `start_m` 为起点的小节时长 → 秒 |
| `note_secs(note, bpms)` | 音符头时间（秒） |
| `hold_tail_time(note, bpms)` | Hold 尾时间（秒） |
| `slide_end_time(note, bpms)` | Slide 尾时间（秒，取最长子 slide 的 span） |

每帧对每个音符计算（`render/timing.rs::compute`）：

```
dt        = note_head_seconds - current_t     // >0 还在接近
dt_scaled = dt / play_speed                   // 音乐时间，与倍速无关
```

- `current_t`：播放时为 `song_time()`（由 `mode_wall_anchor` + `mode_song_offset`
  按墙钟外推）；暂停时为 `timeline_view_time`（定格）。
- 除以 `play_speed` 让 0.5x/2.0x 下“看起来一样快”，因为音频时钟本身也在变速。

### 可见性剔除

一个音符只有在下面条件内才绘制（`NoteTiming::visible`）：

```
slide_tail_dt >= -disappear_time   // 尾判后短暂保留
dt_scaled     <= lead_time         // 进入 NOTE_VISIBLE_DISTANCE 之前不出现
```

`lead_time = (NOTE_OUTER_DISTANCE - NOTE_VISIBLE_DISTANCE) / speed = 6.075 / speed`，
所以音符不会在屏幕中间“凭空出现”。

---

## 5.5 默认谱面与 maichart 加载器（`app/maichart.rs`）

默认曲目来自 `~/.maichart/324_Jack-the-Ripper◆`，其 `chart.json` 是一种
**序列化的 simai AST**（不是内部 `ChartDoc` 格式），本模块在运行时把它转成
`ChartDoc`，无需离线转换工具。

### 5.5.1 时间点 `TimePoint { split, beat }`

位置与时长用同一结构，但位置要 +1、时长不用：

| 用途 | 公式 | 例子 |
|---|---|---|
| 绝对位置 | `measure = 1 + beat / split` | `{4,0}` → 第 1 拍；`{8,413}` → `1+413/8` |
| 时长 | `measure = beat / split` | `{4,5}` → `5/4` 小节；`{8,17}` → `17/8` 小节 |

依据 simai 记谱规范（<https://w.atwiki.jp/simai/pages/1003.html>）：
`[divider:multiplier]` 表示 `multiplier/divider` 个**全音符**，而一个全音符
就是一个小节，所以时长（小节）= `beat/split`。

两个关键交叉验证：

- `[2:1]` = 一个二分音符 = `1/2` 小节；
- slide 追踪前默认等待 **1 拍**（当前 BPM） = `1/4` 小节，正好等于 chart.json 里
  每个 slide 固定出现的 `prepareTime {split:4, beat:1}`。

若把时长误当成“拍数”再除以 4（把 `{4,5}` 当成 `5/16` 小节），所有 hold/slide
长度都会缩到 `1/4` —— 这正是此前的 bug（slide 时值错误）。

### 5.5.2 音符映射

| chart.json | 内部 `Note` |
|---|---|
| `taps` | `NoteType::Tap`，`lane = button(1..8)` |
| `holds` | `NoteType::Hold`，`hold_duration = duration(holdTime)` |
| `slides` | `NoteType::Slide`，`slide = build_slides(parts)` |
| `touches` | `NoteType::Touch`，`lane = 8 + button`（B 环） |
| `toucheHolds` | `NoteType::Hold`，B 环 + duration |

标志位：`isBreak`→`is_break`、`isEx`→`is_ex`；
slide 的 `hindHead`（无头）→ `is_tapless = true`。

> 注意 `is_star` 不是“有星星头”，而是 **double star**（同一 `(time, lane)`
> 上有多个 slide 共用一个头，如 simai `1-3/1-5`）。渲染层用它选择
> `star_double*` 皮肤，所以普通单 slide 必须为 `false`，否则所有星星头都会变成
> double。`mark_double_stars` 在转换后按同头分组自动标记。

### 5.5.3 Slide 形状

每个 slide 由 `parts[]` 组成，每个 part 的 `fragments[]` 是一段形状，字符映射：

| 字符 | SlideShape | 字符 | SlideShape |
|---|---|---|---|
| `-` | Line | `s` | S |
| `<` | Left | `z` | Z |
| `>` | Right | `p` | P |
| `^` | Caret | `q` | Q |
| `v` | VShape | `w` | Wifi |

`prepareTime` → `slide_start_delay`，`moveTime` → 移动时长，
`slide_duration = prepare + move`；`middleButton` 作为中间路点。

### 5.5.4 难度选择

`--diff N` 或 `MAICHART_DIFF=1..5` 选择 `difficulty`，否则取最高难度。本曲共 4 个难度
（5.0 / 9.0 / 12.2 / 14.1）。

### 5.5.5 关键函数

| 函数 | 作用 |
|---|---|
| `load_default` | 读取 `assets/charts/jack_ripper/chart.json` 并转换 |
| `from_bytes` | 去 BOM + serde 解析 + `convert` |
| `convert` | 组装 `ChartDoc`（taps/holds/slides/touches） |
| `measure_of` / `duration_of` | 混合基数 → 小节位置 / 时长 |
| `build_bpms` | BPM keyframes → 排序后的 `BpmChange` 列表 |
| `build_slides` | parts/fragments → `Vec<Slide>` |
| `mark_double_stars` | 同 `(time, lane)` 多 slide 时标记 `is_star`（double star） |
| `shape_of` | 形状字符 → `SlideShape` |
| `ring_lane` / `sensor_lane` | button → lane |
| `select_difficulty` | 依据 `--diff` / `MAICHART_DIFF` 或最高难度 |

---

## 5.6 读取 maidata.txt（simai 文本）

除了 maichart 的 `chart.json`，也支持直接读 **simai `maidata.txt`**。

**解析器**：`src/simai/` 内置了纯 Rust 的 simai 解析器（`maisimai`，MIT，源自
MaiConverter），零外部依赖，只保留了 `model.rs` + `parser.rs`（导出器不需要）。
`parse_file(text)` → `SimaiFile { title, artist, first, levels, charts }`，其中
`charts: Vec<(inote_N, SimaiChart)>`。

**转换**：`src/app/maidata.rs`：

| Simai | 内部 `Note` |
|---|---|
| `SimaiNote::Tap` | `NoteType::Tap`，`lane = button + 1` |
| `SimaiNote::Hold` | `NoteType::Hold`，`hold_duration = duration` |
| `SimaiNote::Slide` | `NoteType::Slide`，`pattern`→`SlideShape`，`chain`→segments，`*` 拆成多个子 slide |
| `TouchTap` / `TouchHold` | 传感器 → `PadZone`：A=1..8、B=9..16、C=17、D=18..25、E=26..33 |

要点：

- **slide 等待**：解析器记录 `delay_explicit`（是否写了 `[delay##duration]`）。
  未显式时补上 simai 规范默认的 **1 拍 = 0.25 小节**等待；显式时（即使是
  `[0##0.24]` 这种 delay=0）**直接采用 0**，不再补默认值。
  `slide_duration = wait + travel`，`slide_start_delay = wait`。
- **星星头去重**：解析器会为 `1-5` 同时产出星形 Tap 和 Slide，转换后按
  `(measure, lane)` 丢弃与 slide 头重合的星形 Tap（`drop_slide_star_taps`）。
- **double star**：slide 的 `is_star` 一律先置 false，再由
  `maichart::mark_double_stars` 按同头分组标记。
- **难度**：`&inote_N=` 按 N 升序排列，`--diff N` / `MAICHART_DIFF` 取第 N 个（1 起），
  默认取最难。这样与 chart.json 的难度编号一致。

验证：本曲 `maidata.txt`（最难 `inote_5`）转换后 839 个音符，与 `chart.json`（diff 4）
完全一致。

---

## 6. 音符运动逻辑详解（核心）

### 6.1 A/B 环（tap / hold / slide 头，`zone <= 8`）

用统一的径向飞行公式 `note_radial_motion(time_until_hit, speed, outer_r, target_offset)`
（`app/types.rs`）：

```
distance = NOTE_OUTER_DISTANCE - dt_scaled · speed      // = 4.8 - dt·speed_scaled
若 distance < NOTE_VISIBLE_DISTANCE(-1.275) → 不可见

scale    = clamp(distance · 0.4 + 0.51, 0, 1)           // 生长
progress = clamp((distance - NOTE_LOCK_DISTANCE) / (NOTE_OUTER_DISTANCE - NOTE_LOCK_DISTANCE), 0, 1)
                                                         // 1.225 → 4.8 之间从 0 到 1
target_r = outer_r + TAP_TARGET_OFFSET                   // 目标环半径
lock_r   = target_r · NOTE_LOCK_DISTANCE / NOTE_OUTER_DISTANCE
radius   = lock_r + (target_r - lock_r) · progress
```

分两段：

1. **锁定生长**：`distance` 从 4.8 降到 1.225 期间，`progress = 0`，音符钉在
   `lock_r`（内环），只有 `scale` 从 0.51 长到 1.0。
2. **向外飞行**：`distance` 从 1.225 到 0，`progress` 0→1，音符从 `lock_r`
   滑到 `target_r`；`dt = 0` 时恰好落在目标环上。

对 **tap** 而言到此不停：`dt < 0` 后继续按同速外飞（`note_radial_motion_continue`
只放开上界、不放开下界，因此接近段与原来完全一致），直到 `disappear_time`（0.18s）
被剔除。这模拟 MajdataView 里 tap 越过判定线后飞出屏幕的效果。hold 头与 slide 头
仍用钳制版 `note_radial_motion`，停在环上。

方向由 lane 固定：`angle = -π/2 + π/8 + (zone-1)·π/4`。绘制时
`px = spawn.x + cos·radius`，`py = spawn.y + sin·radius`。

`render/ring.rs::draw` 负责这项；`draw_tap` 画皮肤（或回退粉色圆），
命中瞬间（`|dt| <= HIT_WINDOW`）再叠一圈白环。

### 6.2 Hold 头尾同速模型

Hold 的头和尾**用同一套径向模型**，只是各自代入自己的时间：
头部用 `dt_scaled`，尾部用 `tail_dt_scaled`（= `(hold_tail_time - t)/speed_scale`）。

```
head_r = note_radial_motion(dt_scaled, speed, ...)        // 不可见时回退 lock_r
tail_r = note_radial_motion(tail_dt_scaled, speed, ...)   // 不可见时回退 lock_r
tail_r = min(tail_r, head_r)                              // 尾不允许超过头
```

因为两者用**同一个 `speed`**，径向速度天然一致：接近阶段身体整体平移、长度不变；
头在命中的一刻停在判定环上；尾在 Hold 结束的一刻刚好到达判定环。

> 早期版本用的是“尾钉在 `lock_r`、最后 `drain_t` 秒才排空”的 drain 模型。它对长
> hold 速度恰好一致，但对短 hold（本曲多数 `[4:1]` 只有 1 拍）尾速度 = 距离/持有
> 时长，会明显快于头部 → “头尾速度不一致”。改为共享模型后不再存在该分支。

身体用 `draw_hold_9slice_segment`（`app/ui.rs`）从 `head` 画到 `tail`，9-slice 保证
头尾帽在“头尾重合”（刚出生）时仍是自然大小，而不是被压成一个点。头到 `lock_r`
之前整体按 `head_motion.scale` 缩放，之后尺寸固定。

**出生时的最小 body 长度**：头尾出生同在 `lock_r`，原始 body 长度为 0，看起来只有
一坨帽。`hold_tail_radius`（`ring.rs`）给尾端加一个向内（朝出生中心）的额外长度
`min_body = HOLD_SPAWN_BODY_WIDTH_FRAC × 身体宽度`（默认 `0.5`），并让该偏移**随尾端
接近判定环而衰减**（系数 `1 - tail_progress`）：

- 尾端还钉在 `lock_r`（出生/前半程）时偏移是常数 → **尾端不动，只有头在飞**，bar 是
  被“拉长”而不是整体往前窜（这就是之前“出生后整体突然前移”的原因）；
- 尾端到达判定环时偏移为 0 → 尾端仍精确落在判定环上。

想更长/更短改 `HOLD_SPAWN_BODY_WIDTH_FRAC`（`1.0` = 与宽度相同，`0.0` = 恢复只有帽）；
想限制偏移最大不超过 `min_body` 也行（当前即如此，尾端不会越过中心）。

**消失时机**：hold 的 `disappear_time = 0`（`render/timing.rs::compute`）—— 尾端一到达
判定环（`tail_dt < 0`）就在下一帧被剔除，不像 tap 那样越过判定环后还继续飞一小段。

### 6.3 Touch / Touch-Hold（`zone > 8`，B/C/D/E 屏区）

Touch **不用**径向模型，而是 MajdataView 的“整段时长”模型
（`touch_whole_duration`）：

```
whole   = 3.2093857 · touch_speed^(-0.9549622)   // 可见总时长
raw     = (whole - dt_scaled) / whole            // 剩余比例
progress= smoothstep(clamp(raw, 0, 1))           // 缓动
```

分两阶段（`render/touch.rs::draw`）：

- **阶段 1**（`progress < TOUCH_GROW_FRAC = 0.25`）：原地淡入（alpha 0→255），不动。
- **阶段 2**：完全不透明，四根三角臂从 `TOUCH_START_DIST(30)` 向内移动到
  `TOUCH_END_DIST(10)`，中心点静止。位移
  `dist = (30 + (10-30)·move_progress) · TOUCH_SCALE · scale`。

Touch-Hold（`draw_touch_hold`）额外：

- 四张**旋转 45°** 的贴图（`TOUCHHOLD_CROSS_BASE=86`，`TOUCHHOLD_SCALE=0.6`），
  起始/结束距离 `30 → 19`；
- 一个进度环：0/0.19 的 `BORDER_BASE=170` 贴图先用全透明画一遍“幽灵环”，
  再在 `mask.frag` 材质下用 `progress` uniform 顺时针扫描出已进行的部分
  （`hold_progress = clamp((current_t - ns) / hold_duration, 0, 1)`）。

### 6.4 Slide

Slide 委托给 `app/slide_render.rs::draw_slide`（`render/slide.rs` 是适配层，
在 `app/pad_svg.rs` 与 `app/slide/path.rs` 之上）：

1. **路径构建**：起点在 A 环（lane 1–8）或 zone 质心（其余），然后逐段按
   `SlideShape`（Q/QQ/P/PP/Left/Right/Caret/Z/S/Wifi/Line）调用
   `slide/path.rs` 生成屏幕点。`segmentation::build` 以
   `SLIDE_TILE_SPACING·scale` 为间距沿路径采样成 `SlideBar`（位置 + 旋转 + 所属 zone）。
2. **trail tiles（分段消耗）**：沿采样点铺贴图。`render/slide.rs` 按星星进度算出
   `hidden_until_bar = floor(star_t · bar_count)`（`consumed_bars`），把**星星已经经过
   的 bar 隐藏**，于是 trail 是“被星星吃掉”的——跟随星星头部逐段消失，而不是整条一起
   留到最后。星星到达终点时 `hidden_until_bar = bar_count`，trail 全部消失。
3. **星标**：
   - 头（`current_t < ns`）：与 tap 同一径向飞行，边飞边转（`rotation = progress·2π`）；
   - 判定后（`ns <= current_t <= slide_end`）：沿路径行走，位置由
     `star_dist_along = star_t · total_len` 在路径上插值，方向取该段切线。
4. Wifi 特殊：三条直轨，每条 11 张贴图，且单独飞星。

slide 的 `disappear_time = 0`（`timing.rs::compute`）：星星到达尾端后整个 slide 立即
消失，不做 0.3s 的停留。

第 1 步里的 Q/P/QQ/PP 由「直线 → B/AD 弧 → 直线」拼成。直线与圆弧的接缝原本是
硬折角（直线近似沿半径、而圆弧切线垂直于半径，夹角接近 90°），看起来很不自然。
现在在两条接缝处用 `fillet_corner` 做**切线连续（G1）的二次贝塞尔圆角**：沿入边方向
起、沿出边方向止，圆角半径 `clamp(arc_radius·0.3, 8·scale, 40·scale)` 并按相邻线段
长度的一半截断。Left/Right/Caret 的弧两端本就与被连接点重合，无需圆角。

轨迹淡入用 `slide_fade_in`（默认 `3.926913 / note_speed` 秒）配合
`slide_start_delay` 换算出的 `fade_in_s`。

### 6.5 判定文字覆盖层

`render/feedback.rs::draw` 遍历 `app.judge_feedback`，把标签（`PERFECT`/`GOOD`）
画在 `zone` 对应锚点：环区在 `outer_r + TAP_TARGET_OFFSET` 处、屏区在 zone 质心；
最后 0.2 s 做 alpha 淡出。标签由 `input/hit.rs::judge_label_for_zone` 生成：
找同 zone 最近音符，`|Δt|<=0.12s → Perfect`，`<=0.25s → Good`，否则无。

---

## 7. 渲染管线（绘制顺序）

`render/mod.rs::draw_pad_panel` 固定顺序：

1. `pad::draw_panel_background` — 面板底 + “Pad View”；
2. `pad::draw_pad_disc` — 深色圆盘；
3. 计算 `spawn_cx = pad_visual_center(pad)`；
4. `pad::draw_spawn_dot` — 中心出生点白点；
5. `pad::draw_zones` — 每个分区多边形（active 蓝 / feedback 橙 / 普通灰）+ 标签；
6. `pad::draw_ring_indicators` — 8 个白点 + 连接弧；
7. `notes::draw_notes` — 遍历谱面：slide → ring/touch 分发；
8. `feedback::draw` — 判定文字。

`notes::draw_notes` 的每音符顺序刻意与原 player 一致：**先画 slide（trail 在星标下），
再走 zone 分支**，因此 ring lane 上的 slide 由 slide 渲染器画头星，ring 分支只补
命中环。

---

## 8. 输入系统

- `pointer::collect_pointer_events`：把 macroquad 的 `touches()` 与鼠标合成统一的
  `PointerEvent`。有触摸时不再发鼠标事件，避免双触发。
- `pointer::handle_touch_controls`：
  - `Started`：`pad_svg.hit_test(position, pad)` 命中测试，写入 `active_pointer_zones`；
  - `Moved/Stationary`：用 `sampled_motion_path`（每 ~4px 采一点）补间，防止快速
    拖动跨帧跳过分区；
  - `Ended/Cancelled`：移除该指针。
  - 每次 zone 变化：`push_feedback`（橙色脉冲）并按需 `push_judgement`（判定文字）。
- `keyboard::handle_lane_input`：`1`–`8`/`T` 映射为合成的 pointer id，与触摸走同一
  `update_pointer_zone`。
- `keyboard::handle_global_hotkeys`：Space/R/Home/方向键/A（见 §1 操作表）。

命中测试 `PadSvgDef::hit_test` 用射线法 `point_in_polygon`（`app/pad_svg.rs`）。

---

## 9. 音频

`app/audio.rs`：

| 函数 | 作用 |
|---|---|
| `load_audio_pcm_from_assets` | 依次尝试 `charts/jack_ripper/track.mp3`、`demo.wav`、`demo.mp3`，解码为 16bit 立体声 PCM |
| `load_wav_pcm_from_bytes` | `hound` 解 WAV |
| `load_mp3_pcm_from_bytes` | `minimp3` 解 MP3，并裁掉 ~40ms 编码器延迟 |
| `normalize_to_44100` | 线性插值重采样到 44.1kHz |
| `service_audio` | 若 `pending_audio_start` 则按当前速度构建并播放 BGM |
| `build_speed_pcm` | 变速 PCM：1.0x 直接切片；否则按 `speed` 重采样（seek 偏移生效） |
| `BgmPlayer` | rodio 封装：单 Sink，可停可换；`play_once` 用 `play_raw` 混播一次性音效，不影响 BGM |
| `SfxBuffer` / `load_answer_sfx` | 解码 `Sfx/answer.wav` 为一次性音效 |

播放起始时机：`start_playback_at` 只把时钟冻结并置 `playback_pending`，等主循环画完
第一帧后 `finalize_playback_start` 才请求音频，避免加载卡顿吃掉谱面时间。

### 9.1 判定点音效（cue）

没有裁决引擎，所以音效由**时间轴**驱动：`player/cues.rs::CueTrack` 在载入时把谱面里的
**tap 头、hold 头、hold 尾、slide 星星头**时间排成有序事件表，并维护一个前进游标。每帧
`PadPreviewState::tick_cues()` 把游标推进到 `song_time()`，跨过的事件就播放一次
`Sfx/answer.wav`（`BgmPlayer::play_once`）。

- `reset(t)`：在播放/seek/重播时把游标重定位到 `t` 之后，**不补发**，避免从头播放时
  一次性把所有音效炸出来；
- `take_due(t)`：返回本帧跨过的事件数；时间倒流（seek 后退）自动重同步；
- 暂停时不推进；恢复播放时以当前 `mode_song_offset` 重置。

想给不同 cue 用不同音效，改 `Cue` 分支即可（目前三者共用一个音效）。

---

## 10. 函数作用清单

### `player/state.rs` — `PadPreviewState`

原 `PlayerState` 的精简版，只保留渲染与音频读取的字段。关键方法：

| 方法 | 作用 |
|---|---|
| `new` | 初始化（含 `BgmPlayer`，读取 `MAI2_UI_SCALE`/`MAI2_MOBILE_UI`） |
| `current_speed` | 播放倍速；Idle 时为 0（时钟不走） |
| `song_time` | 由墙钟锚点外推当前秒；`playback_pending` 时冻结 |
| `toggle_play` | 播放/暂停，暂停时记录 `mode_song_offset` |
| `start_playback_at` | 从指定秒重播，冻结时钟待首帧 |
| `finalize_playback_start` | 首帧后释放时钟并请求音频 |
| `seek_audio_to` | 记录 seek；播放中触发重建音频，暂停中静音 |
| `set_play_speed` / `nudge_play_speed` | 改速并在播放中重锚时钟+重建音频 |
| `tick_feedback` | 清理过期的 pad/judge 反馈 |
| `push_feedback` / `push_judgement` | 添加橙色脉冲 / 判定文字 |
| `tick_cues` | 播放本帧跨过的 tap/hold 判定点音效 |
| `play_answer` | 播放一次 `Sfx/answer.wav` |
| `reset_cues` | 播放/seek/重播时重定位 cue 游标（不补发） |

### `player/cues.rs`

| 函数 | 作用 |
|---|---|
| `CueTrack::from_chart` | 谱面 → 有序 cue 事件表（tap 头 / hold 头 / hold 尾 / slide 星星头） |
| `CueTrack::reset` | 重定位游标到 `t` 之后，不触发 |
| `CueTrack::take_due` | 推进到 `t`，返回跨过的事件数（倒流自动重同步） |

### `player/layout.rs`

| 函数 | 作用 |
|---|---|
| `compute_layout` | header + pad 面板矩形 |
| `compute_pad_geom` | 面板内切圆 → `PadGeom`（`outer_r = min(w,h)·0.42`） |

### `player/render/*`

| 文件 / 函数 | 作用 |
|---|---|
| `mod::draw_pad_panel` | **渲染总入口**，按固定顺序编排全部绘制 |
| `scale::ui_scale` | 统一缩放系数 |
| `pad::draw_panel_background` | 面板底 + 标题 |
| `pad::draw_pad_disc` | 深色圆盘 |
| `pad::draw_spawn_dot` | 出生点白点 |
| `pad::draw_zones` | 分区多边形填充/描边/标签（active/feedback 着色） |
| `pad::draw_ring_indicators` | A 环 8 白点 + 连接弧 |
| `timing::compute` | 生成 `NoteTiming`（`dt`/`dt_scaled`/尾判/速度/lead/消失时间） |
| `timing::NoteTiming::visible` | 可见性剔除 |
| `notes::draw_notes` | 遍历谱面 + 分发到 ring/touch/slide |
| `ring::draw` | tap/hold 径向飞行绘制 |
| `ring::draw_tap` | tap 贴图（含 Ex）或回退图形 |
| `ring::draw_hold` | hold 头尾同速飞行 + 9-slice 身体 |
| `ring::hold_tail_radius` | 尾端半径：向内的最小长度偏移，随尾端接近判定环衰减 |
| `touch::draw` | touch 整段淡入/内移；hold 转 `draw_touch_hold` |
| `touch::draw_touch_hold` | 4 斜向贴图 + `mask.frag` 进度环 |
| `slide::draw` | 选贴图、按星星进度隐藏已过的 trail bar，并调用 `slide_render::draw_slide` |
| `slide::consumed_bars` | 星星进度 → 该隐藏的 trail bar 数量 |
| `feedback::draw` | 判定文字覆盖层 |
| `textures::load_note_textures` | 按候选路径加载全部皮肤（缺失回退图形） |

### `player/input/*`

| 文件 / 函数 | 作用 |
|---|---|
| `pointer::collect_pointer_events` | 触摸/鼠标 → `PointerEvent` 列表 |
| `pointer::sampled_motion_path` | 指针位移补间采样 |
| `pointer::update_pointer_zone` | 更新 active 集合 + 反馈 |
| `pointer::handle_touch_controls` | 指针事件路由到分区 |
| `keyboard::handle_lane_input` | `1`–`8`/`T` 轨道键 |
| `keyboard::handle_global_hotkeys` | 播放/seek/倍速/音频热键 |
| `hit::judge_label_for_zone` | 简易判定文字 |

### `app/*`（复制的上游基础层，节选关键函数）

| 函数 | 作用 |
|---|---|
| `types::note_radial_motion` | **径向飞行核心公式**（§6.1） |
| `types::note_lock_radius` / `note_lead_time` / `note_flight_speed` | 锁定半径 / 提前量 / 有效飞行速度 |
| `types::touch_whole_duration` | touch 整段可见时长公式 |
| `types::measure_to_secs` 等 | 小节 ↔ 秒换算 |
| `pad_svg::PadSvgDef::from_svg_str` | 解析 `pad.svg`（touch + center 图层） |
| `pad_svg::hit_test` | 屏幕点 → `PadZone`（射线法） |
| `pad_svg::svg_to_screen` / `screen_to_svg` | SVG ↔ 屏幕变换 |
| `pad_svg::draw_polygon_fill` / `draw_polygon_lines` | 凹多边形三角化填充 / 描边 |
| `slide_render::draw_slide` | slide trail + 星标绘制 |
| `slide::path::slide_shape_*` | 各形状的屏幕路径点 |
| `slide::segmentation::build` | 路径 → trail bar + judge 分段 |
| `ui::draw_hold_9slice_segment` | hold 身体 9-slice |
| `ui::load_mask_material` | `mask.frag` 材质（进度环） |
| `chart::load_generated_chart` | 谱面加载（默认 maichart → latest → generated → fallback） |
| `chart::load_chart_from_path` | 命令行路径加载：目录取 `chart.json`，先 maichart 再内部格式 |
| `chart::find_audio_in_dir` | 谱面目录里找 `track.mp3/wav`、`demo.*` |
| `audio::load_audio_from_path` / `decode_audio_bytes` | 按扩展名解码本地音频并归一化 |
| `cli::parse` / `parse_env` / `help` | 命令行解析（谱面/音频/难度） |
| `maichart::*` | `.maichart/chart.json` → `ChartDoc`（见 §5.5） |
| `maidata::from_maidata` | simai `maidata.txt` → `ChartDoc`（见 §5.6） |
| `simai::parse_file` / `simai::parse_chart_text` | 内置纯 Rust simai 解析器 |
| `audio::*` | 解码 / 变速 / 播放（§9） |

---

## 11. 与原始 player 的差异

| 原始角色 | 本预览 |
|---|---|
| `PlayerState`（约 1400 行，编辑器+裁决+曲库） | `PadPreviewState`（约 250 行） |
| `player/ui.rs::draw_pad_panel`（1200 行，含编辑/判定色带） | `render/*` 拆分，移除编辑与判定色带 |
| `player/input.rs`（触摸+编辑+时间轴） | `input/*`（仅触摸+轨道键+播放热键） |
| `lnmai-core` 裁决引擎、`engine.rs` | 删除；判定文字改由 `input/hit.rs` 近似 |
| egui 前端 / 选曲 / 模板 / SFX | 删除 |

**修复的上游问题**：`player/ui.rs` 在组装 `SlideTextures` 时无条件传入
`star_ex_fallback = star_ex_tex`，而 `slide_render::draw_slide` 用
`star_ex.or(star_ex_fallback)` 取贴图，导致**所有** slide 头都叠了 Ex 环。
本项目的 `render/slide.rs` 只在 `note.is_ex` 为真时才提供 Ex 贴图，
`star_ex_fallback` 留空。

另外，Q/P/QQ/PP 路径的直线↔圆弧接缝是硬折角，本项目在 `slide/path.rs` 用
`fillet_corner` 做了 G1 圆角（见 §6.4）；tap 越过判定环后继续外飞（见 §6.1）。

**保持一致的**：`pad.svg` 几何与坐标变换、分区多边形、径向飞行/Hold 头尾同速/整段时长
等运动公式、贴图与绘制层级、时间换算与可变 BPM。

---

## 12. 可扩展点

- 想换谱面：把新的 `.maichart/<曲目>` 目录放到 `assets/charts/` 并改
  `maichart::DEFAULT_ASSET`，或用 `MAICHART_DIFF` 切换难度；也支持替换
  `assets/generated_chart.json`（旧“秒”格式与 beat 格式）。
- 想真正裁决：把 `input/hit.rs` 换成 `lnmai-core` FFI 调用，并在
  `render/touch.rs` 恢复判定色带与 `slide_judge` 覆盖。
- 想加波形/时间轴：复用原项目的 `rustfft` 波形与 `egui-macroquad` 时间轴绘制。
- 移动端：`MAI2_MOBILE_UI=1` 走移动布局，资源通过 `platform::load_asset_bytes`
  兼容 Android/iOS 打包路径。
