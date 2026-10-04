# FX 音符实现计划（fxnote）

> 输入规格：[`FX_NOTE_FORMAT.md`](./FX_NOTE_FORMAT.md)（Simai 内联 FX 音符，
> 状态：设计稿）。
> 本文件把设计稿拆成可执行的分阶段任务：每一阶段给出**文件、函数、接口草图、
> 测试与验收**。实现分支：`fxnote`（基线 `player_preview`）。

## 实现进度

| 阶段 | 内容 | 状态 |
| --- | --- | --- |
| A | `src/app/fx.rs` 参数模型 + 解析 + 单测 | ✅ 完成（9 测） |
| B | 两份 `shared/rust_ffi_types.rs` 增 `fx_effect` | ✅ 完成 |
| C | `Note.fx` + `maidata::simple_note` 映射 | ✅ 完成 |
| E | `src/app/audio_fx/` DSP + `FxSource` + `BgmPlayer` | ✅ 完成（4 测） |
| F | player 接线（`PadPreviewState::update_fx_state`） | ✅ 完成，**当前交付测试点** |
| D | 外部 Lean 核心解析 | ⬜ 待开工（外部 `fxnote` 分支） |
| G | 集成测试 / 手工验收 | 🔶 单测已绿，谱面级待 D |

> F 阶段交付时：Lean 核心还不会解析 `@`，故用 `LAMBDADX_FX_TEST`（见 §8 F1b）
> 手动验收 5 个效果的听感与启停。

## 0. 范围与非目标

**目标**：谱面里写 `1fx@gate[4:1]`、`A3fx@sc(wl=1:4;r=60)` 这类音符，按住时
对 BGM 叠加音频效果；解析、判定、计分、评级与 Hold 一致，仅音符头判定为 EX。

**本阶段实现 5 个效果**：`gate`、`bit_crusher`、`wobble`、`sidechain`、
`high_pass_filter`（与 `FX_NOTE_FORMAT.md` §6 一致）。

**非目标（本阶段）**：
- 其余 9 个效果（`retrigger` / `flanger` / `pitch_shift` / `phaser` /
  `tapestop` / `echo` / `switch_audio` / `low_pass_filter` / `peaking_filter`）。
- 外部 JSON 定义（方案 C 明确排除）。
- `key_sound`（FX tap 按键音）——与 AudioEffect 是两套东西，见 kson 文档 §3.4。

**关键前提**：Simai 解析器**不在本仓库**。`src/app/maidata.rs` 只把
`lnmai-core` 的 FFI token 映射到渲染模型。Lean 工程由 `.cargo/config.toml`
的 `LNMAI_CORE_LEAN_PROJECT` 指向外部 checkout：

```
/Users/pingfanh/project/Mai2Chart/demo/macroquad_sim/lnmai-core-rs/lnmai-core-ffi/lnmai-core
```

因此任务分成**本仓库侧（A/B/C/E/F/G）**与**外部 Lean 侧（D，另行开工）**。
按用户要求，D 阶段先不动。

---

## 1. 架构与数据流

```
Simai 文本
  │  (外部 Lean 核心)
  ├─ Tokenize → Normalize → Lower(ChartSpec) → Lifecycle/Scheduler(GameState)
  │        └─ 新增：fxEffect 串随 Hold/TouchHold 一路带出；头部判定 EX
  ▼
FFI JSON（shared/rust_ffi_types.rs，两份拷贝）
  ├─ RawNoteToken.fxEffect         ← 解析出的 `type(params)` 原始串
  └─ HoldChartNote/TouchHoldChartNote.fxEffect（autoplay/调试用，可选）
  ▼
lambdaDX
  ├─ app/maidata.rs::simple_note → Note { fx: Option<FxEffect> }   (B/C)
  ├─ app/fx.rs：把 `type(params)` 解析成 AudioEffect + 参数        (A)
  ├─ app/audio_fx/：5 个效果 DSP + FxSource                        (E)
  └─ app/audio.rs::BgmPlayer：BGM 外面包 FxSource，读原子当前效果   (E)
  ▼
player（每帧）
  └─ update_fx_state()：按住 + 窗口内 → 设置 FxControl            (F)
```

**核心原则**：音符类型只决定「效果作用窗口」，真正出声由「是否按住」决定
（dry/wet 即时切换）。详见 `FX_NOTE_FORMAT.md` §5 与 kson 文档 §3.3。

---

## 2. 先决决策（避免返工）

| # | 决策 | 取值 / 理由 | 备选 |
| --- | --- | --- | --- |
| D1 | 核心如何携带效果 | **核心只透传原始串** `fxEffect: String`，参数解析全部放 lambdaDX `src/app/fx.rs`。理由：把参数模型（`EffectParameter`、范围/过渡）留在 Rust 一处，Lean 侧改动最小 | 结构化（`AudioEffect` 的 Lean 镜像）：改动大、易与 Rust 漂移 |
| D2 | lambdaDX 的音符类型 | **沿用 `NoteType::Hold/TouchHold` + 新增 `Note.fx: Option<FxEffect>`**。理由：`NoteType` 有 ~10 处 `match`（`ring.rs`/`timing.rs`/`cues.rs`/`main.rs`…），新增 enum 变体会全部触发编译错误且无意义 | 按设计稿新增 `NoteType::FxHold/FxTouchHold`（改动面大） |
| D3 | 判定在哪 | 核心内完成，lambdaDX 不重复判定。EX 头部用现有 `is_ex` 机制（设计稿 §5：`judgeTap(isEX=true)`） | — |
| D4 | `fx_enable` 来源 | 本地：`song_time ∈ [head, tail]` 且对应 zone **当前按住**（指针/键盘/autoplay）。见 §6 F1 | 读核心 `GameState.active_holds`（light step 不返回，需额外拉全量 state，成本高） |
| D5 | fx 解析失败（未知类型 / 语法错误） | **该音符退化为普通 Hold，不报错**（已确认）。便于玩家仍能打谱，不因编辑失误整谱失败 | 整谱报错 |
| D6 | 未知参数名 | **忽略**（设计稿 §4.1，便于跨效果复用） | 报错 |
| D7 | 计分类别 | **并入 Hold**（设计稿 §5「与 Hold 同权重」）。不新增 `Fx` 计分桶 | 单独 `Fx` 桶 |
| D8 | 参数位置+命名混用 | **允许**：带 `名=` 的按命名，其余按声明顺序依次填未命名位置参数 | 只允许其一 |
| D9 | `:` 与 `/` | `:` 是规范写法（`1:4`）；同时接受 `/` 作为兼容（kson 语法）。见 A2 | 仅 `:` |
| D10 | 裸数字的单位 | **不做按类型隐式化简**。每个参数有自己的**默认单位**；裸数字按该参数默认单位解释，也可显式写后缀（`1ms`、`2s`），后缀必须与该参数量纲同类。见 A2 | 全局按类型固定 |

> D1/D2 是相对设计稿 §7.3 的**偏离**，其余沿用设计稿。

---

## 3. 阶段 A：lambdaDX 参数模型（新增 `src/app/fx.rs`）

**不依赖核心**，可独立完成并测试。这是整条链路的“词汇层”。

### A1 数据类型

```rust
// src/app/fx.rs
pub enum FxKind { Gate, BitCrusher, Wobble, Sidechain, HighPassFilter }

/// 与 kson 的 EffectParameterValue 对应，但用 `:` 作分数。
pub enum ParamValue {
    Length { lo: f32, hi: f32, tempo: bool }, // 拍(tempo)或秒
    Rate   { lo: f32, hi: f32 },              // 0..1（50% → 0.5）
    Freq   { lo: f32, hi: f32 },              // Hz
    Sample { lo: i32, hi: i32 },
    Float  { lo: f32, hi: f32 },
    Switch(bool),                             // on/off
}
pub struct Param { pub value: ParamValue, pub transition: bool } // `a>b` → transition

pub struct AudioEffect {
    pub kind: FxKind,
    /// 按声明顺序保存已给出的位置参数；命名参数解析后并入。
    pub params: Vec<(String, Param)>, // 规范名 → Param
}
```

### A2 解析器

入口：`pub fn parse_fx_effect(text: &str) -> Result<AudioEffect, FxError>`，
`text` = `type(params)`（已由核心剥离 `@` 与 `[n:m]`）。

- 效果名/别名表（设计稿 §3）：
  `gate|g`、`bit_crusher|bc`、`wobble|w`、`sidechain|sc`、
  `high_pass_filter|hpf|hp`。未知 → `Err`。
- 参数名别名（设计稿 §4.2）：`wl → wave_length`、`r → rate`（其余按规范名）。
- 顶层分号切分 `(` `)` 内内容（值里可能出现 `>`、`-`、`:`、`%`、`=`，但不含 `;`
  与括号）。
- 值解析（设计稿 §4.3，`:` 换 `/`）：
  - `1:4` → `Length{lo=hi=0.25, tempo=true}`；`/` 同样接受。
  - `10ms` / `2s` → `Length{tempo=false}`。
  - `50%` → `Rate{0.5}`。
  - `80hz` / `2khz` → `Freq`（`kHz` ×1000）。
  - `30samples` → `Sample`。
  - `on`/`off` → `Switch`。
  - `2.0` → `Float`。
  - `a-b` → 区间（lo..hi）；`a>b` → 过渡（`transition=true`）。
- 每个效果**按声明顺序**的默认值（设计稿 §6）表驱动，缺省参数补默认。
- 解析结果带 `(kind, params)`；提供 `resolve(&self, bpm, beat_len) -> ResolvedFx`
  把拍值换算成秒（`secs = beats * 240 / bpm`）、频率等，供 DSP 直接使用。

### A3 测试（`#[cfg(test)]` in fx.rs）

- 每个效果：无参数 → 默认值正确。
- `@sc(1:2;10ms;50ms;1:16;1>5)` 位置参数按序落位。
- `@sc(wl=1:4;r=60)` 命名参数 → `wave_length`/`rate`。
- `@hpf(80hz-2khz)` 区间；`@gate(0%>90%)` 过渡。
- `@g` / `@bc` / `@w` / `@hp` 别名。
- 未知类型 → `Err`；未知参数名 → 忽略且不报错。
- `1:4` 与 `1/4` 等价。

**验收**：`cargo test fx::` 全绿，不触碰其它模块。

---

## 4. 阶段 B：FFI 类型同步（本仓库内，不碰外部 Lean）

两份文件必须同步（内容有少量已知差异，改动要各写一次）：

- `lnmai-core/shared/rust_ffi_types.rs`（Lean 后端）
- `lnmai-rust/shared/rust_ffi_types.rs`（纯 Rust 后端 mirror）

改动：

1. `RawNoteToken` 新增：
   ```rust
   /// `@type(params)` 原始串（不含 `@`）。None = 非 FX。
   #[serde(default, skip_serializing_if = "Option::is_none")]
   pub fx_effect: Option<String>,
   ```
2. `HoldChartNote` / `TouchHoldChartNote` 新增同名字段（autoplay/调试可见）。
3. `NormalizedHold` / `NormalizedTouchHold` 可选加 `fx_effect`（若核心也在
   normalized 层暴露；本阶段可只走 token→ChartSpec）。
4. 可选：`JudgeEventKind` 是否新增 `Fx` 变体——按 D7 **不加**。
5. **注意**：纯 Rust 端口 `lnmai-rust/port` 是外部机器路径（本仓库只提交 stub），
   `backend-rust` 下 FX 不会生效，除非同步更新该端口。计划内先只保证
   `backend-lean`（默认）编译与行为。

**验收**：`cargo check`（backend-lean）通过；JSON 反序列化对缺省字段兼容
（`#[serde(default)]`）。

---

## 5. 阶段 C：lambdaDX 映射与状态

### C1 `src/app/types.rs`

```rust
// Note 新增（保持 serde 兼容）
#[serde(default, skip_serializing_if = "Option::is_none")]
pub fx: Option<crate::app::fx::AudioEffect>,
```
- `NoteType` **不变**（D2）。
- 新增便捷判定：`impl Note { pub fn is_fx(&self) -> bool { self.fx.is_some() } }`。

### C2 `src/app/maidata.rs`

- `simple_note()`：
  - 核心若发出 `RawNoteKind::Hold`（带 `fx_effect` / `is_ex`）→ 构建 `NoteType::Hold`
    + `fx: Some(parse_fx_effect(...)?)`。
  - 若核心按设计稿发出 `RawNoteKind::FxHold/FxTouchHold`，则在 `match` 里新增分支，
    同样映射到 `Hold`/`TouchHold` + `fx`。
  - `TouchHold` 同理。
  - 解析失败：返回 `None` + 记一条状态（或走 `Result`，见 §10 待确认）。
- `build_notes()` 对 fx 音符走普通 hold 分支（不改 slide 分组逻辑）。
- 测试：加一份 `assets/charts/fxtest/maidata.txt`，断言
  `notes.iter().any(|n| n.fx.is_some() && n.note_type == Hold)`。

### C3 其它构造 Note 的地方

- `src/app/maichart.rs`（编辑器/模板路径）与 `src/app/chart.rs`：保持
  `fx: None`（`..Default::default()` 已覆盖，无需改）。
- `src/main.rs` 的 `match note.note_type`：因 `NoteType` 不变，无需改。

### C4 音频 cue（`src/player/cues.rs`）

- fx hold 头/尾与普通 hold 相同（`NoteType::Hold/TouchHold`），**无需改**。若将来
  要给 fx 头换 Ex 音，再在 `CueEvent` 里加 `is_fx`。

### C5 渲染

- `ring.rs` / `timing.rs` / `touch.rs` 复用 Hold/TouchHold 分支，**无需改**
  （外观同 Hold，设计稿 §5）。

**验收**：带 `fx` 的谱面能加载、渲染、cue 正常；`fx` 暂不影响声音。

---

## 6. 阶段 D：外部 Lean 核心（**推迟，另行开工**）

> 以下为落地清单，基于 `FX_NOTE_FORMAT.md` §7。开工前需先确认工程路径与
> `lake build` 可用。**本阶段先不做。**

### D1 `LnmaiCore/Simai/Tokenize.lean`

- `applyInlineDirective`（约 :164–:193）：只把**段首前导指令**当 BPM/除数/hspeed
  指令；`@` 之后的 `(...)` 属于音符本体，不得吞掉。
- `expandTokenList`（约 :456–:458）：对 fx token 放宽 `K @ c m` 拒绝逻辑。
- `inferKind`（约 :97）：`<digit>fx` → fxHold（或 Hold+fx），
  `<area>fx` → fxTouchHold（或 TouchHold+fx）。
- `mkRawToken`（约 :198）：显式覆盖 `isBreak`（`bit_crusher` 含 `b`）、
  `isHanabi`（`fx` 含 `f`）、`isEX`（`fx` 含 `x`）的启发式误判。
- 从 token 剥离 `@type(params)`，存 `fxEffect`；`head[timing]` 走原逻辑。

### D2 `Timing.lean`：不变（`[n:m]` 语义复用）。

### D3 `Normalize.lean`：把 `fxEffect` 落到 `ChartSpec` 的 hold/touch-hold。

### D4 `ChartLoader.lean`：`buildHold` / `buildTouchHold` 带 `fxEffect`；
若新增 kind，加分支。头部按 EX。

### D5 `Lifecycle.lean`：fx hold/touchhold 头部走 `judgeTap(isEX=true)`
（若沿用 `is_ex` 则可能无需改，需实测确认）。

### D6 `Types.lean` / `Scheduler.lean`：`JudgeEventKind`/`NoteType` 若加 fx 分支；
计分按 D7 并入 Hold。

### D7 `Frontend.lean`（FFI JSON 出口）：输出 `fxEffect` 字段，与阶段 B 对齐。

**验收**：`lake build`；用 `assets/charts/fxtest/maidata.txt` 打印 token/chart，
`fxEffect` 与 `is_ex` 正确；跑 `lnmai-core-verify` / 现有测试无回归。

---

## 7. 阶段 E：音频 DSP（新增 `src/app/audio_fx/`）

> 参考实现：`/Users/pingfanh/project/kson-rs/audio-effect-test/src/sources/`
> 与 `kson-rodio-sources/`。lambdaDX 用 **rodio 0.19**，需做 API 适配。

### E1 rodio 0.21 → 0.19 适配表

| 0.21/0.22（参考） | 0.19（本仓库） |
| --- | --- |
| `channels() -> ChannelCount` | `channels() -> u16` |
| `sample_rate() -> SampleRate` | `sample_rate() -> u32` |
| `channels().get()` | `channels()` |
| `sample_rate().get()` | `sample_rate()` |
| `current_span_len()` | `current_frame_len()` |
| `rodio::source::UniformSourceIterator` | 存在，可用 |
| `rodio::source::Delay` | 存在，可用 |

### E2 模块结构

```
src/app/audio_fx/
  mod.rs          // FxControl, FxSource, Dsp（5 个效果状态机）
  biquad.rs       // Biquad（LowPass/HighPass）+ 通道状态
```

> 与最初的 7 文件拆分不同：5 个效果的状态机都很小，合并进 `mod.rs` 更易维护；
> 只把复用的 `Biquad` 单独成文件。（实现偏差，已确认可接受。）

5 个效果的移植要点：
- `gate`：按 `wave_length` 周期、`rate` 占空比乘增益；`mix` 干湿。
- `bit_crusher`：`reduction` 个采样保持 + `lerp(mix)`。
- `wobble`：三角波驱动 biquad 中心频率（`lo_freq..hi_freq` 对数），`q`。
- `sidechain`：`period` 内 attack/hold/release 包络压 `ratio`。
- `hpf`：biquad `HighPass`，`freq`/`q`/`mix`。

默认值取自设计稿 §6（与 kson `effects.rs` 一致；注意 kson 用 `1/4`，本仓库用
`1:4`）。

### E3 `FxSource` 设计（关键）

参考实现是**嵌套 Source**（`biquad(wobble(...))`）。但实时切换效果需要**保留
BGM 播放位置**，重建 Source 链会丢状态。因此 lambdaDX 采用**单层内联 DSP**：

```rust
pub struct FxSource<S: Source<Item = f32>> {
    input: S,
    control: Arc<FxControl>,      // 与 player 共享
    active: Option<ActiveFx>,     // 当前 DSP 状态机
    span_left: usize,             // 每帧/每 span 轮询一次控制
}
```

- `FxControl`：`Mutex<Option<Arc<ResolvedFx>>>`（`ResolvedFx` 是 Copy 的小结构）。
- `next()`：每 `current_frame_len()`（或每 N 帧，如 512）`try_lock` 一次，把
  `ResolvedFx` 复制进 `active`；`active` 改变时重置对应 DSP 状态（biquad 复位）。
  逐采样只做纯计算，无锁。
- `None` → **dry 透传**（`input.next()` 原样返回），不重建 `Sink`、不重置位置。

### E4 `src/app/audio.rs::BgmPlayer`

- `play()` 里把 `SamplesBuffer::new(...)` 换成
  `FxSource::new(SamplesBuffer::new(...), self.fx_control.clone()).speed(speed)`。
  （注意 0.19 `Sink::append` 需要 `Send + 'static`；`Arc<Mutex<..>>` 满足，
  `SamplesBuffer<f32>` 满足。）
- `BgmPlayer` 持有 `fx_control: Arc<FxControl>`，暴露
  `pub fn set_active_fx(&self, fx: Option<Arc<ResolvedFx>>)`。
- `stop()` 时清空 `fx_control`。

### E5 测试

- 5 个效果各写纯函数级测试（喂直流/正弦，断言增益/频率响应形状）。
- `FxSource`：`None` 透传逐样本相等；切换后下一 span 生效。
- 不引入 `num-traits` 除非必要（`ResolvedFx` 用 f32 即可）。

---

## 8. 阶段 F：player 接线

### F1 每帧状态（新增，`src/player/engine.rs` 或 `state.rs`）

`update_fx_state(app)`，在 `step_judge_engine` 之后、`tick_frames` 之前调用：

1. `t = app.song_time()`。
2. 找当前**按住且窗口内**的 fx 音符：
   - 窗口：`[note_secs(head), hold_tail_time]`（`app::types`）。
   - 按住：zone 在「当前按下集合」里。
     按下集合来源：
     - 指针：`state.active_pointer_zones`（`HashMap<u64, Vec<PadZone>>`）。
     - 键盘：`input/keyboard.rs` 的按键映射（A1..A8 等）。
     - autoplay：`autoplay_click_held` / `autoplay_explicit_held`。
3. 命中多个时定义优先级：**取 head 最晚的一个**（后触发覆盖，已确认）。
4. `resolve` 成 `ResolvedFx`（用音符所在 BPM 换算拍值）→ `bgm_player.set_active_fx(...)`。
5. 无命中 → `set_active_fx(None)`（立即切干声）。

> 备选：核心 light step 不返回 active holds，故本地判定；若未来核心在
> `RuntimeStepLightResult` 暴露 `active_fx`，可改为直接读。

#### F1b 调试直达（D 阶段落地前的手动验收）

Lean 核心尚未输出 `fxEffect`，因此 F 阶段用环境变量手动注入效果：

```bash
LAMBDADX_FX_TEST='gate'                 cargo run --bin lambda_dx_pad_preview
LAMBDADX_FX_TEST='sidechain(1:4;50ms)'  cargo run --bin lambda_dx_pad_preview
```

按住任意 pad 区（键盘 `1`–`8` / `T`，或触摸）即叠加该效果，松开立即切干声。
效果串按 [`src/app/fx.rs`] 的语法解析（`type(params)`），BPM 固定 120 仅用于时值换算。
此路径仅用于验收 DSP；正常谱面路径仍走 F1。

### F2 暂停 / 拖拽 / 重启 / 切歌

- 暂停、`stop_audio_if_any`、`restart`、seek 都要 `set_active_fx(None)` 并重置
  `hold_head_hits` 逻辑附近的状态。见 `state.rs` 的 `pending_audio_start` /
  `audio_seek_offset` 生命周期。

### F3 autoplay

- autoplay 时 fx 同样由按住驱动；`autoplay_*_held` 已建，F1 直接复用。
- `src/player/autoplay.rs` / `autoplay_gen` 无需改判定（核心照常）。

### F4 渲染（可选）

- 可选：fx hold 用不同描边/图标（本阶段不做，外观同 Hold）。

**验收**：测试谱面按住 fx hold 能听到效果、松手立刻消失；暂停/重启无残留。

---

## 9. 阶段 G：测试与验收

### G1 单元测试
- `fx.rs`（A3）。
- `audio_fx`（E5）。
- `maidata.rs`：fx 谱面映射（C2）。

### G2 集成
- 新增 `assets/charts/fxtest/maidata.txt`，含 5 个效果各一个 hold，例如：
  ```text
  1fx@gate[4:1],2fx@bc[4:1],3fx@w[4:1],4fx@sc[4:1],5fx@hpf[4:1],
  ```
- `cargo test`（backend-lean）全绿。

### G3 手动验收清单
- [ ] 按住 fx hold → 对应效果可闻；松开 → 立即干声。
- [ ] dry 路径与改动前逐样本一致（无效果时无差异）。
- [ ] 5 个效果默认参数可闻、参数生效。
- [ ] 暂停/恢复/拖拽/重启/切歌后无卡住的效果。
- [ ] 非 fx 谱面零回归（`test/`、`サイエンス1/2/`、`autotest`）。
- [ ] `cargo build` / `cargo test` / `cargo clippy` 通过。

---

## 10. 文件清单

**新增**
- `src/app/fx.rs`（A）
- `src/app/audio_fx/mod.rs` + `src/app/audio_fx/biquad.rs`（E）
- `assets/charts/fxtest/maidata.txt`（G2，待 D 阶段加入，避免当前核心解析失败）
- `docs/FX_NOTE_PLAN.md`（本文件）

**修改（本仓库）**
- `lnmai-core/shared/rust_ffi_types.rs`、`lnmai-rust/shared/rust_ffi_types.rs`（B）
- `src/app/types.rs`（C1）
- `src/app/maidata.rs`（C2）
- `src/app/beat_format.rs`（`fx: None` 兼容）
- `src/app/audio.rs`（E4）
- `src/player/state.rs`（F：`update_fx_state` + 调试直达）
- `src/main.rs`（F：每帧调用）
- `src/app/mod.rs`（注册 `fx`、`audio_fx`）
- `Cargo.toml`（**未新增依赖**，未用 `num-traits`）

**修改（外部 Lean，推迟）**
- `LnmaiCore/Simai/Tokenize.lean`、`Normalize.lean`、`Simai/Frontend.lean`、
  `ChartLoader.lean`、`Lifecycle.lean`、`Types.lean`、`Scheduler.lean`（D）

---

## 11. 风险

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| Lean 核心在外部仓库 | 无法在本分支端到端验证解析 | 先用 `fx.rs` 单测锁参数语义；核心 D 阶段单独开工 |
| `backend-rust` 端口是 stub | rust 后端下 FX 无效 | 明确只保证 `backend-lean`；端口更新另议 |
| 逐采样加 DSP 的性能 | 低配机音频卡顿 | 单层无锁 `next()`；span 级轮询控制；只在内联状态机上做乘加 |
| rodio 0.19 API 差异 | 编译/行为偏差 | E1 适配表 + 逐效果单测 |
| 同时按住多个 fx | 行为未定义 | D 阶段前先定优先级（§12） |
| 参数单位歧义（`1` 是秒/拍/浮点） | 听感错 | 按效果参数类型固定（A2 表驱动），单测覆盖 |

---

## 12. 已确认决策（开工前拍板结果）

1. **同时按住多个 fx 音符**：**取后触发**（head 最晚的那个生效，后覆盖前）。见 F1。
2. **fx 解析失败**：**该音符退化为普通 Hold**（静默，不整谱报错）。见 D5、C2。
3. **单位规则**：**不按类型隐式化简**。每个参数有默认单位，裸数字用默认单位；
   可在参数内显式指定同类单位（如默认 ms，可写 `1ms`，也可写其它同类单位）。
   见 D10、A2。
4. **fx 计入 DX/评级权重**：**是**，并入 Hold（D7 确认）。
5. **纯 Rust 端口（`backend-rust`）**：**暂不同步**（只保证 `backend-lean`）。
6. **外部 Lean 工程**：在外部 checkout 新建分支 **`fxnote`**（与本仓库同名）；
   提交/回灌方式待 D 阶段开工时再定。

> 执行顺序：A → B → C → E → F（**到 F 时先交付可测版本给用户**）→ D → G 穿插。


---

## 13. 建议的执行顺序

1. **阶段 A**（`fx.rs`）——独立、可测、无依赖。← 建议从这里开始。
2. **阶段 B**（FFI 类型）——小改动，为 C/D 铺路。
3. **阶段 C**（映射，`fx` 暂不发声）——确保谱面加载/渲染/判定无回归。
4. **阶段 E**（DSP）——独立可测（喂样本），不依赖核心。
5. **阶段 F**（接线）——打通“按住→出声”。
6. **阶段 D**（Lean 核心）——最后开工，端到端联调。
7. **阶段 G** 全程穿插。
