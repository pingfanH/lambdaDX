# FX 音符格式（Simai 内联音频效果）

> 状态：**设计稿，尚未实现**。
> 本文定义在 `lambdaDX` 中新增的 **FX 音符**：一种与 Hold / TouchHold 行为一致、
> 但会在按住期间对 BGM 叠加音频效果（AudioEffect）的音符。
> 效果**直接写在 simai 谱面里**，不依赖外部 JSON（方案 C：扩展 vendored 的
> Lean core `lnmai-core`，让解析/判定/评级全部在核心内完成）。
>
> 效果参数语义参考 `kson` / `audio-effect-test`（见
> `kson-rs/audio-effect-test/docs/kson-audio-format.md`），但**格式不同**：那边是
> JSON，本仓库是 simai 内联。

## 0. 背景与决策

- 目标：像官方 SDVX 一样，FX 音符按住时对 BGM 施加效果；谱师在谱面里直接指定
  效果类型与参数。
- 决策（方案 C）：**不放外部 JSON、不在 lambdaDX 侧单独维护一份效果定义**，
  而是把 `fx` 音符类型与效果串进 `lnmai-core`：
  - `RawNoteKind` 新增 `fxHold` / `fxTouchHold`；
  - 解析出的效果类型 + 参数随音符进入 `ChartSpec` / `GameState`；
  - 判定、计分、评级与 Hold 相同，只有**音符头判定**是 EX（见 §5）。
- 音频 DSP 在 lambdaDX 侧实现（`src/app/audio_fx/`），从 `lnmai-core` 拿到
  “当前生效的效果 + 参数”，叠加到 BGM 上。

## 1. 语法总览

```
<note-head>[@<effect>[<params>]][<timing>]
```

- **note head**：
  - `<lane>fx`：`lane` 为数字（1..8）→ `fxhold`（长条，按键轨）。
  - `<area>fx`：`area` 为触屏区 `A1..E8`（如 `A1fx`、`A3fx`）→ `fxtouchhold`。
- **`@<effect>`**：绑定音频效果。`<effect>` 为效果类型名或其别名（§3）。
- **`<params>`**：紧跟在效果名后，**必须是圆括号** `( ... )`，多个参数用 `;` 分隔，
  可以省略整个括号（全部用默认值）。
- **`<timing>`**：与 Hold 完全一致——方括号 `[n:m]` 指定**起始拍位与持续长度**
  （长度 = 一小节 × m/n）。省略时默认一个音符增量。
- `/` 分隔同时出现的多个音符（每个都能独立带 `@效果`）。
- `,` 分隔时间轴上的前后段。

### 1.1 示例

```text
1fx@sidechain(1:2;10ms;50ms;1:16;1>5)     # 1 轨 fxhold，位置/长度由紧随的 [n:m] 决定
1fx@gate[4:1]                             # 4 拍处一个 gate 效果，长度 1/4 小节
1fx@w[8:1]/2fx@w[8:1]                     # 左右同时 wobble
A3fx@sc(wl=1:4;r=60)                      # A3 触屏 fx，命名参数 + 别名
```

> 说明：`@效果` 与 `[n:m]` 的顺序固定为 **先效果后 timing**；`[n:m]` 是
> 位置 + 长度规格，不是效果参数。

## 2. 位置与长度

与现有 Hold 相同（`LnmaiCore/Simai/Timing.lean` 的 `parseNdMicros`）：

- `[n:m]`：`n` = 细分位置，`m` = 时值分子，时长 = `一小节 × m / n`。
- 例：`[4:1]` = 一小节 × 1/4 = 一拍；`[8:1]` = 1/8 小节。
- 省略 `[n:m]`：长度取默认音符增量（`noteTimingIncrement`）。
- FX 效果的**作用窗口 = 音符的 `[起始, 起始+长度]`**；按住期间持续生效，
  松开（或音符结束时）立即停止叠加。

## 3. 效果类型与别名

| 规范名 (`type`) | 别名 | 本阶段 | 说明 |
| --- | --- | --- | --- |
| `gate` | `g` | ✅ | 周期性衰减（断续节奏） |
| `bit_crusher` | `bc` | ✅ | 降位深 / 采样保持 |
| `wobble` | `w` | ✅ | 滤波中心频率上下摆动 |
| `sidechain` | `sc` | ✅ | 侧链压缩，周期性压低音量 |
| `high_pass_filter` | `hpf` / `hp` | ✅ | 高通滤波 |
| `retrigger` | `rt` | ⬜ 后续 | 片段循环回放 |
| `flanger` | `fl` | ⬜ 后续 | 短延迟调制 |
| `pitch_shift` | `ps` | ⬜ 后续 | 移调 |
| `phaser` | `ph` | ⬜ 后续 | 全通扫频 |
| `tapestop` | `ts` | ⬜ 后续 | 磁带停止 |
| `echo` | `ec` | ⬜ 后续 | 延迟回声 |
| `switch_audio` | — | ⬜ 后续 | 切换音频文件 |
| `low_pass_filter` | `lpf` / `lp` | ⬜ 后续 | 低通滤波 |
| `peaking_filter` | `pkf` | ⬜ 后续 | 峰值均衡 |

- 本阶段只实现前 5 个（与测试工具的重点一致）。
- 未知/未实现类型：**解析报错**（避免静默掉落音效）。

## 4. 参数

### 4.1 写法

- **位置参数**：按该效果**声明顺序**依次给出，例
  `@sidechain(1:2;10ms;50ms;1:16;1>5)`。
- **命名参数**：`名字=值`，例 `@sc(wl=1:4;r=60)`；可与位置参数混用（未确认，
  见 §8）。
- 省略的参数使用**默认值**。
- 未知参数名：**忽略**（便于谱面跨效果复用）。

### 4.2 参数名别名

| 规范名 | 别名 |
| --- | --- |
| `wave_length` | `wl` |
| `rate` | `r` |
| 其余 | 待定，见 §8 |

### 4.3 参数值格式

沿用 kson 的值语义，但把 `/` 换成 `:`（`:` 与 simai 的 `:` 一致，避免和路径冲突）：

| 写法 | 解析结果 | 含义 |
| --- | --- | --- |
| `1:4`、`1:8` | `Length(..., tempo=true)` | 以拍为单位的时值（受 BPM 影响） |
| `10ms`、`2s` | `Length(..., tempo=false)` | 固定时间 |
| `1` | `Length(1.0s)` 或 `Float` | 见下 |
| `30samples` | `Sample` | 采样数 |
| `50%` | `Rate(0.5..=0.5)` | 百分比 |
| `80hz` / `2khz` | `Freq` | 频率 |
| `2.0` | `Float` | 普通浮点数 |
| `on` / `off` | `Switch` | 开关 |
| `a-b` | 范围，如 `80hz-2khz` | 取值范围 |
| `a>b` | 过渡，如 `0%>100%`、`off>on` | 起始值线性过渡到结束值 |

- **单位化简**：`1ms` 可省略单位写成 `1`；`1:4` 也可写成 `1`（视参数类型取合适
  默认单位）。实现时优先按“带单位”解析，纯数字再按参数类型推断。
- `:` 是 `/` 的等价写法（`1:2` = `1/2`）。

## 5. 行为语义

| 方面 | 行为 |
| --- | --- |
| 外观 | `fxhold` 同 Hold 长条；`fxtouchhold` 同 TouchHold |
| 音符头判定 | **EX**：`judgeTap(isEX=true)` → 恒 `Perfect`（无 fast/late 细分）；fxtouchhold 头部同样按 EX 处理 |
| 尾部判定 | 与 Hold 相同（`judgeHoldEnd`） |
| 计分 | 与 Hold 同权重；是否单独成 `Fx` 计分类别待定（§8） |
| 评级 | 计入评级（需要评级） |
| 音频 | 按住且在音符窗口内 → 叠加对应效果；松开/结束 → 立即切干声 |
| 自动演奏 | 生效条件与 `fx_enable` 等价（是否按住） |

> 与 kson 一致：音符类型只决定“效果作用窗口”，真正出声由“是否按住”决定。
> 因此 tap 形式（长度 0）几乎听不到效果；FX 的实际意义在 hold。

## 6. 各效果参数（本阶段 5 个）

> 参数顺序 = 结构体声明顺序，即**位置参数顺序**。默认值取自 kson。

### Gate (`gate` / `g`)

| # | 参数 | 默认 | 作用 |
| --- | --- | --- | --- |
| 1 | `wave_length` | `1/4` | 开合周期（拍） |
| 2 | `rate` | `70%` | 占空比 |
| 3 | `mix` | `0%>90%` | 干湿比 |

### BitCrusher (`bit_crusher` / `bc`)

| # | 参数 | 默认 | 作用 |
| --- | --- | --- | --- |
| 1 | `reduction` | `0samples-30samples` | 采样保持数量（samples） |
| 2 | `mix` | `0%>100%` | 干湿比 |

### Wobble (`wobble` / `w`)

| # | 参数 | 默认 | 作用 |
| --- | --- | --- | --- |
| 1 | `wave_length` | `1/12` | 摆动周期（拍） |
| 2 | `lo_freq` | `500hz` | 频率下限 |
| 3 | `hi_freq` | `20000hz` | 频率上限 |
| 4 | `q` | `1.414` | 品质因数 |
| 5 | `mix` | `0%>50%` | 干湿比 |

### Sidechain (`sidechain` / `sc`)

| # | 参数 | 默认 | 作用 |
| --- | --- | --- | --- |
| 1 | `period` | `1/4` | 压缩周期（拍） |
| 2 | `hold_time` | `50ms` | 保持时间 |
| 3 | `attack_time` | `10ms` | 起音时间 |
| 4 | `release_time` | `1/16` | 释放时间 |
| 5 | `ratio` | `1>5` | 压缩比 |

### HighPassFilter (`high_pass_filter` / `hpf` / `hp`)

| # | 参数 | 默认 | 作用 |
| --- | --- | --- | --- |
| 1 | `v` | `0` | 值（强度/混合系数） |
| 2 | `freq` | `80hz-2khz` | 截止频率 |
| 3 | `q` | `1.414` | 品质因数 |
| 4 | `delay` | `0` | 延迟补偿 |
| 5 | `mix` | `50%` | 干湿比 |

## 7. 实现要点

### 7.1 与现有 tokenizer 的冲突（务必注意）

`LnmaiCore/Simai/Tokenize.lean`：

- `applyInlineDirective`（:164–:193）会把段内任何 `(...)`/`{...}`/`<H...>` 当成
  BPM/除数/hspeed 指令**吞掉**，且会丢弃其前面的音符文本 → 与 `@type(params)`
  冲突。需改为：**只把段首的前导指令**当指令；`@` 之后的圆括号属于音符本体。
- `expandTokenList`（:456–:458）对已知 kind 会拒绝含 `K @ c m` 的 token，
  以防把乱码当音符 → fx 的效果名/参数里必然含 `@` 与 `c`（`bit_crusher`、
  `sidechain` 等），需对 fx 放宽。
- `inferKind`（:97）目前：数字开头 → tap/slide/hold；触屏区开头 → touch/touchHold。
  需新增：`<digit>fx` → `fxHold`，`<area>fx` → `fxTouchHold`。
- `mkRawToken`（:198）的启发式会误判：`isBreak = contains 'b'`（`bit_crusher` 命中）、
  `isHanabi = contains 'f'`（`fx` 自身命中）、`isEX = contains 'x'`（`fx` 命中）→
  需对 fx 显式覆盖这些 flag。
- 效果串需从 token 中剥离后再走原有 head/timing 解析，效果本身存进
  `RawNoteToken` 的新字段。

### 7.2 解析流程建议

1. `inferKind` 识别 `<head>fx`；
2. 在 `mkRawToken` 里定位 `@`：`head@type(params)[timing]`，
   把 `type(params)` 复制到新字段 `fxEffect`，`head` + `[timing]` 走原逻辑；
3. `(params)` 用“顶层分号”切分（注意值里可能出现 `>`、`-`、`:`，但不含 `;`）；
4. 归一化时（`Normalize.lean`）把 `fxEffect` 落到 `ChartSpec`；
5. `ChartLoader.lean` 的 `buildHold` / `buildTouchHold` 新增 `isFx` / `fxEffect`；
6. `Lifecycle.lean` 的音符头判定按 EX 处理；
7. `Types.lean` 的 `JudgeEventKind` / `NoteType` 增加 fx 分支；
8. `Scheduler.lean` 的计分与事件音频命令带上效果串；
9. FFI 类型（两处 `shared/rust_ffi_types.rs`）同步。

### 7.3 lambdaDX 侧

- `src/app/types.rs`：`NoteType` 增 `FxHold` / `FxTouchHold`；`Note` 增 `fx_effect`。
- `src/app/maidata.rs`：`simple_note` 处理新 kind；新增效果串 → `FxEffect` 的解析。
- 新增 `src/app/fx.rs`：`AudioEffect` / `EffectParameter` 的可移植版本 + 解析。
- 新增 `src/app/audio_fx/`：从 vendored `rodio` Source 移植 5 个效果
  （注意 rodio **0.19** API：`channels()->u16`、`sample_rate()->u32`、
  `current_frame_len`；参考实现是 0.21 的 `ChannelCount`/`SampleRate`/
  `current_span_len`）。可能需要 `num-traits` 依赖。
- `src/app/audio.rs`：`BgmPlayer` 用 `FxSource` 包裹 BGM 的 `SamplesBuffer`，
  通过原子变量读当前效果 id + 参数；**dry = 透传，不重建 Sink**。
- `src/player/`：fx 的渲染/时间调度复用 hold / touchhold 分支，仅判定与音频不同。

## 8. 待确认问题

1. **计分类别**：fx 是否单独成 `Fx` 计分桶，还是并入 Hold？
2. **fxtouchhold 头部**：确认同样按 EX 判定（恒 Perfect）。
3. **位置参数 + 命名参数混用** 是否允许？
4. **未知参数名** 忽略还是报错？
5. **效果别名 / 参数别名** 完整表：目前仅确认 `sc`、`g`、`hpf`、`w`（推测）、
   `wl=wave_length`、`r=rate`；其余待定。
6. **单位化简约规则**：`1` 何时解释为秒、何时为拍、何时为浮点，需按参数类型固定。
7. 未实现的 9 个效果是“报错”还是“忽略并退化为普通 hold”？
