//! Simai 内联 FX（AudioEffect）参数模型与解析。
//!
//! 谱面里写 `1fx@sidechain(1:2;10ms;50ms;1:16;1>5)`，核心把 `@` 之后的
//! `type(params)` 原样透传（见 [`docs/FX_NOTE_FORMAT.md`]）；本模块负责把这个原始
//! 串解析成结构化参数，并换算成 DSP 可直接使用的 [`ResolvedFx`]。
//!
//! 参数语义参考 `kson` / `audio-effect-test`（`parameter.rs` / `effects.rs`），
//! 但把分数分隔符由 `/` 换成 `:`（`/` 仍兼容）。单位规则见决策 D10：**不做按类型
//! 隐式化简**，每个参数有自己的默认单位；裸数字按该参数默认单位解释，也可显式写
//! 后缀（`1ms`、`2s`），后缀须与参数量纲同类。

use serde::{Deserialize, Serialize};

/// 本阶段实现的 5 个效果类型（`FX_NOTE_FORMAT.md` §3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FxKind {
    Gate,
    BitCrusher,
    Wobble,
    Sidechain,
    HighPassFilter,
}

impl FxKind {
    /// 规范名 / 别名 → 类型（`FX_NOTE_FORMAT.md` §3）。
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "gate" | "g" => Some(Self::Gate),
            "bit_crusher" | "bc" => Some(Self::BitCrusher),
            "wobble" | "w" => Some(Self::Wobble),
            "sidechain" | "sc" => Some(Self::Sidechain),
            "high_pass_filter" | "hpf" | "hp" => Some(Self::HighPassFilter),
            _ => None,
        }
    }

    pub fn canonical_name(self) -> &'static str {
        match self {
            Self::Gate => "gate",
            Self::BitCrusher => "bit_crusher",
            Self::Wobble => "wobble",
            Self::Sidechain => "sidechain",
            Self::HighPassFilter => "high_pass_filter",
        }
    }

    /// 参数声明顺序 = 位置参数顺序（`FX_NOTE_FORMAT.md` §6）。
    pub fn spec(self) -> &'static [ParamSpec] {
        match self {
            Self::Gate => &GATE_SPEC,
            Self::BitCrusher => &BIT_CRUSHER_SPEC,
            Self::Wobble => &WOBBLE_SPEC,
            Self::Sidechain => &SIDECHAIN_SPEC,
            Self::HighPassFilter => &HIGH_PASS_SPEC,
        }
    }
}

/// 参数的默认量纲；决定裸数字的解释方式（D10）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamUnit {
    /// 时值，以「小节」为单位（受 BPM 影响），如 `1/4`。
    Measure,
    /// 时值，固定秒。
    Seconds,
    /// 时值，固定毫秒。
    Millis,
    /// 频率 Hz。
    Hz,
    /// 比率/百分比（`50%` → 0.5）。
    Rate,
    /// 采样数。
    Sample,
    /// 普通浮点。
    Float,
    /// 开关（`on`/`off`）。
    Switch,
}

pub struct ParamSpec {
    pub name: &'static str,
    pub unit: ParamUnit,
    pub default: &'static str,
}

const GATE_SPEC: [ParamSpec; 3] = [
    ParamSpec { name: "wave_length", unit: ParamUnit::Measure, default: "1/4" },
    ParamSpec { name: "rate", unit: ParamUnit::Rate, default: "70%" },
    ParamSpec { name: "mix", unit: ParamUnit::Rate, default: "0%>90%" },
];
const BIT_CRUSHER_SPEC: [ParamSpec; 2] = [
    ParamSpec { name: "reduction", unit: ParamUnit::Sample, default: "0samples-30samples" },
    ParamSpec { name: "mix", unit: ParamUnit::Rate, default: "0%>100%" },
];
const WOBBLE_SPEC: [ParamSpec; 5] = [
    ParamSpec { name: "wave_length", unit: ParamUnit::Measure, default: "1/12" },
    ParamSpec { name: "lo_freq", unit: ParamUnit::Hz, default: "500hz" },
    ParamSpec { name: "hi_freq", unit: ParamUnit::Hz, default: "20000hz" },
    ParamSpec { name: "q", unit: ParamUnit::Float, default: "1.414" },
    ParamSpec { name: "mix", unit: ParamUnit::Rate, default: "0%>50%" },
];
const SIDECHAIN_SPEC: [ParamSpec; 5] = [
    ParamSpec { name: "period", unit: ParamUnit::Measure, default: "1/4" },
    ParamSpec { name: "hold_time", unit: ParamUnit::Millis, default: "50ms" },
    ParamSpec { name: "attack_time", unit: ParamUnit::Millis, default: "10ms" },
    ParamSpec { name: "release_time", unit: ParamUnit::Measure, default: "1/16" },
    ParamSpec { name: "ratio", unit: ParamUnit::Float, default: "1>5" },
];
const HIGH_PASS_SPEC: [ParamSpec; 5] = [
    ParamSpec { name: "v", unit: ParamUnit::Float, default: "0" },
    ParamSpec { name: "freq", unit: ParamUnit::Hz, default: "80hz-2khz" },
    ParamSpec { name: "q", unit: ParamUnit::Float, default: "1.414" },
    ParamSpec { name: "delay", unit: ParamUnit::Float, default: "0" },
    ParamSpec { name: "mix", unit: ParamUnit::Rate, default: "50%" },
];

/// 一个参数值，支持单值 / 区间（`lo-hi`）；
/// `transition` 表示 `lo>hi` 过渡写法（DSP 目前取 `hi` 作为稳定值）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub value: ParamValue,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub transition: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParamValue {
    /// `lo`/`hi` 为小节数（`tempo=true`）或秒（`tempo=false`）。
    Length { lo: f32, hi: f32, tempo: bool },
    Rate { lo: f32, hi: f32 },
    Freq { lo: f32, hi: f32 },
    Sample { lo: f32, hi: f32 },
    Float { lo: f32, hi: f32 },
    Switch { lo: bool, hi: bool },
}

/// 解析后的效果：类型 + 规范名参数（按声明顺序，缺省已补默认值）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioEffect {
    pub kind: FxKind,
    pub params: Vec<(String, Param)>,
}

impl AudioEffect {
    pub fn param(&self, name: &str) -> Option<&Param> {
        self.params.iter().find(|(n, _)| n == name).map(|(_, p)| p)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FxError {
    Empty,
    UnclosedParen,
    UnknownEffect,
    BadValue,
}

impl std::fmt::Display for FxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            FxError::Empty => "empty fx effect",
            FxError::UnclosedParen => "unclosed '(' in fx effect",
            FxError::UnknownEffect => "unknown fx effect type",
            FxError::BadValue => "bad fx parameter value",
        };
        f.write_str(s)
    }
}

/// 解析 `type(params)`（`@` 与 `[timing]` 已由核心剥离）。
///
/// - 未知类型 → `Err(UnknownEffect)`（调用方按 D5 退化为普通 Hold）。
/// - 未知参数名 → 忽略（D6）。
/// - 位置参数按声明顺序填入未被命名参数占用的槽位（D8）。
pub fn parse_fx_effect(text: &str) -> Result<AudioEffect, FxError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(FxError::Empty);
    }
    let (name, inner) = match text.find('(') {
        Some(i) => {
            let rest = &text[i + 1..];
            let inner = rest.strip_suffix(')').ok_or(FxError::UnclosedParen)?;
            (&text[..i], Some(inner.trim()))
        }
        None => (text, None),
    };
    let kind = FxKind::from_name(name.trim()).ok_or(FxError::UnknownEffect)?;

    let mut named: Vec<(&str, &str)> = Vec::new();
    let mut positional: Vec<&str> = Vec::new();
    if let Some(inner) = inner.filter(|s| !s.is_empty()) {
        for part in inner.split(';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if let Some((k, v)) = part.split_once('=') {
                named.push((normalize_param_name(k.trim()), v.trim()));
            } else {
                positional.push(part);
            }
        }
    }

    let mut params = Vec::new();
    let mut pos = 0usize;
    for spec in kind.spec() {
        let raw = if let Some((_, v)) = named.iter().find(|(k, _)| *k == spec.name) {
            *v
        } else if pos < positional.len() {
            let v = positional[pos];
            pos += 1;
            v
        } else {
            spec.default
        };
        let value = parse_param(raw, spec.unit)?;
        params.push((spec.name.to_string(), value));
    }

    Ok(AudioEffect { kind, params })
}

fn normalize_param_name(name: &str) -> &str {
    match name {
        "wl" => "wave_length",
        "r" => "rate",
        other => other,
    }
}

/// 解析单个参数：支持 `a>b` 过渡、`a-b` 区间、单值。
fn parse_param(raw: &str, unit: ParamUnit) -> Result<Param, FxError> {
    let raw = raw.trim();
    if let Some((a, b)) = raw.split_once('>') {
        let lo = parse_scalar(a.trim(), unit)?;
        let hi = parse_scalar(b.trim(), unit)?;
        return Ok(Param { value: combine(lo, hi)?, transition: true });
    }
    if let Some((a, b)) = split_range(raw) {
        let lo = parse_scalar(a.trim(), unit)?;
        let hi = parse_scalar(b.trim(), unit)?;
        return Ok(Param { value: combine(lo, hi)?, transition: false });
    }
    Ok(Param { value: parse_scalar(raw, unit)?, transition: false })
}

/// 区间分隔符 `-`，但避免吃掉负号（本阶段参数无负值，仍保守处理）。
fn split_range(raw: &str) -> Option<(&str, &str)> {
    let idx = raw[1..].find('-').map(|i| i + 1)?;
    Some((&raw[..idx], &raw[idx + 1..]))
}

fn combine(lo: ParamValue, hi: ParamValue) -> Result<ParamValue, FxError> {
    Ok(match (lo, hi) {
        (ParamValue::Length { lo: a, hi: _, tempo: t1 }, ParamValue::Length { lo: _, hi: b, tempo: t2 }) => {
            ParamValue::Length { lo: a, hi: b, tempo: t1 || t2 }
        }
        (ParamValue::Rate { lo: a, .. }, ParamValue::Rate { hi: b, .. }) => {
            ParamValue::Rate { lo: a, hi: b }
        }
        (ParamValue::Freq { lo: a, .. }, ParamValue::Freq { hi: b, .. }) => {
            ParamValue::Freq { lo: a, hi: b }
        }
        (ParamValue::Sample { lo: a, .. }, ParamValue::Sample { hi: b, .. }) => {
            ParamValue::Sample { lo: a, hi: b }
        }
        (ParamValue::Float { lo: a, .. }, ParamValue::Float { hi: b, .. }) => {
            ParamValue::Float { lo: a, hi: b }
        }
        (ParamValue::Switch { lo: a, .. }, ParamValue::Switch { hi: b, .. }) => {
            ParamValue::Switch { lo: a, hi: b }
        }
        _ => return Err(FxError::BadValue),
    })
}

/// 解析单个标量；显式单位优先，否则按参数默认单位（D10）。
fn parse_scalar(raw: &str, unit: ParamUnit) -> Result<ParamValue, FxError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(FxError::BadValue);
    }
    if raw.eq_ignore_ascii_case("on") {
        return Ok(ParamValue::Switch { lo: true, hi: true });
    }
    if raw.eq_ignore_ascii_case("off") {
        return Ok(ParamValue::Switch { lo: false, hi: false });
    }

    // `1:4` / `1/4` → 小节分数。
    if let Some((a, b)) = raw.split_once(':').or_else(|| raw.split_once('/')) {
        let n: f32 = a.trim().parse().map_err(|_| FxError::BadValue)?;
        let d: f32 = b.trim().parse().map_err(|_| FxError::BadValue)?;
        if d == 0.0 {
            return Err(FxError::BadValue);
        }
        let v = n / d;
        return Ok(ParamValue::Length { lo: v, hi: v, tempo: true });
    }

    if let Some(v) = strip_suffix_f32(raw, "ms") {
        return Ok(ParamValue::Length { lo: v / 1000.0, hi: v / 1000.0, tempo: false });
    }
    if let Some(v) = strip_suffix_f32(raw, "samples") {
        return Ok(ParamValue::Sample { lo: v, hi: v });
    }
    if let Some(v) = strip_suffix_f32(raw, "khz") {
        return Ok(ParamValue::Freq { lo: v * 1000.0, hi: v * 1000.0 });
    }
    if let Some(v) = strip_suffix_f32(raw, "hz") {
        return Ok(ParamValue::Freq { lo: v, hi: v });
    }
    if let Some(v) = strip_suffix_f32(raw, "%") {
        return Ok(ParamValue::Rate { lo: v / 100.0, hi: v / 100.0 });
    }
    if let Some(v) = strip_suffix_f32(raw, "s") {
        return Ok(ParamValue::Length { lo: v, hi: v, tempo: false });
    }

    let n: f32 = raw.parse().map_err(|_| FxError::BadValue)?;
    Ok(match unit {
        ParamUnit::Measure => ParamValue::Length { lo: n, hi: n, tempo: true },
        ParamUnit::Seconds => ParamValue::Length { lo: n, hi: n, tempo: false },
        ParamUnit::Millis => ParamValue::Length { lo: n / 1000.0, hi: n / 1000.0, tempo: false },
        ParamUnit::Hz => ParamValue::Freq { lo: n, hi: n },
        ParamUnit::Rate => ParamValue::Rate { lo: n / 100.0, hi: n / 100.0 },
        ParamUnit::Sample => ParamValue::Sample { lo: n, hi: n },
        ParamUnit::Float => ParamValue::Float { lo: n, hi: n },
        ParamUnit::Switch => return Err(FxError::BadValue),
    })
}

fn strip_suffix_f32(raw: &str, suffix: &str) -> Option<f32> {
    let lower = raw.to_ascii_lowercase();
    if lower.ends_with(suffix) {
        lower[..lower.len() - suffix.len()].trim().parse::<f32>().ok()
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Resolved（DSP 视图）
// ---------------------------------------------------------------------------

/// 换算成 DSP 直接可用的数值（秒 / Hz / 比率）。
/// 区间/过渡取稳定端：时值取 `lo`，其余取 `hi`。
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedFx {
    Gate {
        wave_length: f32,
        rate: f32,
        mix: f32,
    },
    BitCrusher {
        reduction: f32,
        mix: f32,
    },
    Wobble {
        wave_length: f32,
        lo_freq: f32,
        hi_freq: f32,
        q: f32,
        mix: f32,
    },
    Sidechain {
        period: f32,
        hold_time: f32,
        attack_time: f32,
        release_time: f32,
        ratio: f32,
    },
    HighPassFilter {
        freq: f32,
        q: f32,
        mix: f32,
    },
}

impl ResolvedFx {
    pub fn kind(&self) -> FxKind {
        match self {
            Self::Gate { .. } => FxKind::Gate,
            Self::BitCrusher { .. } => FxKind::BitCrusher,
            Self::Wobble { .. } => FxKind::Wobble,
            Self::Sidechain { .. } => FxKind::Sidechain,
            Self::HighPassFilter { .. } => FxKind::HighPassFilter,
        }
    }
}

/// 把 [`AudioEffect`] 换算成 DSP 参数。`bpm` 用于把小节时值转秒。
pub fn resolve(effect: &AudioEffect, bpm: f32) -> ResolvedFx {
    let bpm = if bpm > 1.0 { bpm } else { 120.0 };
    let lo = |name: &str| effect.param(name).map(|p| length_secs_lo(&p.value, bpm)).unwrap_or(0.0);
    let hi = |name: &str| effect.param(name).map(|p| length_secs_hi(&p.value, bpm)).unwrap_or(0.0);
    let rate = |name: &str| effect.param(name).map(|p| rate_hi(&p.value)).unwrap_or(0.0);
    let freq_lo = |name: &str| effect.param(name).map(|p| freq_lo(&p.value)).unwrap_or(0.0);
    let freq_hi = |name: &str| effect.param(name).map(|p| freq_hi(&p.value)).unwrap_or(0.0);
    let float_hi = |name: &str| effect.param(name).map(|p| float_hi(&p.value)).unwrap_or(0.0);

    match effect.kind {
        FxKind::Gate => ResolvedFx::Gate {
            wave_length: lo("wave_length").max(1e-3),
            rate: rate("rate").clamp(0.0, 1.0),
            mix: rate("mix").clamp(0.0, 1.0),
        },
        FxKind::BitCrusher => ResolvedFx::BitCrusher {
            reduction: float_hi("reduction").max(1.0),
            mix: rate("mix").clamp(0.0, 1.0),
        },
        FxKind::Wobble => ResolvedFx::Wobble {
            wave_length: lo("wave_length").max(1e-3),
            lo_freq: freq_lo("lo_freq").max(1.0),
            hi_freq: freq_hi("hi_freq").max(1.0),
            q: float_hi("q").max(0.01),
            mix: rate("mix").clamp(0.0, 1.0),
        },
        FxKind::Sidechain => ResolvedFx::Sidechain {
            period: lo("period").max(1e-3),
            hold_time: hi("hold_time").max(0.0),
            attack_time: hi("attack_time").max(0.0),
            release_time: hi("release_time").max(0.0),
            ratio: float_hi("ratio").max(1.0),
        },
        FxKind::HighPassFilter => ResolvedFx::HighPassFilter {
            freq: freq_hi("freq").max(1.0),
            q: float_hi("q").max(0.01),
            mix: rate("mix").clamp(0.0, 1.0),
        },
    }
}

fn length_secs_lo(v: &ParamValue, bpm: f32) -> f32 {
    match v {
        ParamValue::Length { lo, tempo, .. } => {
            if *tempo { lo * 240.0 / bpm } else { *lo }
        }
        ParamValue::Float { lo, .. } => *lo,
        ParamValue::Rate { lo, .. } => *lo,
        ParamValue::Sample { lo, .. } => *lo / 44100.0,
        ParamValue::Freq { lo, .. } => 1.0 / lo.max(1e-6),
        ParamValue::Switch { .. } => 0.0,
    }
}
fn length_secs_hi(v: &ParamValue, bpm: f32) -> f32 {
    match v {
        ParamValue::Length { hi, tempo, .. } => {
            if *tempo { hi * 240.0 / bpm } else { *hi }
        }
        ParamValue::Float { hi, .. } => *hi,
        ParamValue::Rate { hi, .. } => *hi,
        ParamValue::Sample { hi, .. } => *hi / 44100.0,
        ParamValue::Freq { hi, .. } => 1.0 / hi.max(1e-6),
        ParamValue::Switch { .. } => 0.0,
    }
}
fn rate_hi(v: &ParamValue) -> f32 {
    match v {
        ParamValue::Rate { hi, .. } => *hi,
        ParamValue::Float { hi, .. } => *hi,
        ParamValue::Switch { hi, .. } => f32::from(u8::from(*hi)),
        _ => 0.0,
    }
}
fn freq_lo(v: &ParamValue) -> f32 {
    match v {
        ParamValue::Freq { lo, .. } => *lo,
        _ => 0.0,
    }
}
fn freq_hi(v: &ParamValue) -> f32 {
    match v {
        ParamValue::Freq { hi, .. } => *hi,
        _ => 0.0,
    }
}
fn float_hi(v: &ParamValue) -> f32 {
    match v {
        ParamValue::Float { hi, .. } => *hi,
        ParamValue::Sample { hi, .. } => *hi,
        ParamValue::Rate { hi, .. } => *hi,
        ParamValue::Freq { hi, .. } => *hi,
        ParamValue::Length { hi, .. } => *hi,
        ParamValue::Switch { hi, .. } => f32::from(u8::from(*hi)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(e: &AudioEffect, name: &str) -> Param {
        *e.param(name).expect("param")
    }

    #[test]
    fn each_effect_defaults() {
        let g = parse_fx_effect("gate").unwrap();
        assert_eq!(g.kind, FxKind::Gate);
        assert_eq!(p(&g, "wave_length").value, ParamValue::Length { lo: 0.25, hi: 0.25, tempo: true });
        assert_eq!(p(&g, "rate").value, ParamValue::Rate { lo: 0.7, hi: 0.7 });

        let bc = parse_fx_effect("bit_crusher").unwrap();
        assert_eq!(p(&bc, "reduction").value, ParamValue::Sample { lo: 0.0, hi: 30.0 });

        let w = parse_fx_effect("wobble").unwrap();
        assert_eq!(p(&w, "lo_freq").value, ParamValue::Freq { lo: 500.0, hi: 500.0 });
        assert_eq!(p(&w, "hi_freq").value, ParamValue::Freq { lo: 20000.0, hi: 20000.0 });

        let sc = parse_fx_effect("sidechain").unwrap();
        assert_eq!(p(&sc, "hold_time").value, ParamValue::Length { lo: 0.05, hi: 0.05, tempo: false });
        assert_eq!(p(&sc, "release_time").value, ParamValue::Length { lo: 0.0625, hi: 0.0625, tempo: true });
        assert_eq!(p(&sc, "ratio").value, ParamValue::Float { lo: 1.0, hi: 5.0 });

        let hpf = parse_fx_effect("high_pass_filter").unwrap();
        assert_eq!(p(&hpf, "freq").value, ParamValue::Freq { lo: 80.0, hi: 2000.0 });
    }

    #[test]
    fn positional_params_in_order() {
        let sc = parse_fx_effect("sidechain(1:2;10ms;50ms;1:16;1>5)").unwrap();
        assert_eq!(p(&sc, "period").value, ParamValue::Length { lo: 0.5, hi: 0.5, tempo: true });
        assert_eq!(p(&sc, "hold_time").value, ParamValue::Length { lo: 0.01, hi: 0.01, tempo: false });
        assert_eq!(p(&sc, "attack_time").value, ParamValue::Length { lo: 0.05, hi: 0.05, tempo: false });
        assert_eq!(p(&sc, "release_time").value, ParamValue::Length { lo: 0.0625, hi: 0.0625, tempo: true });
        assert_eq!(p(&sc, "ratio").value, ParamValue::Float { lo: 1.0, hi: 5.0 });
        assert!(p(&sc, "ratio").transition);
    }

    #[test]
    fn named_params_and_aliases() {
        let g = parse_fx_effect("g(wl=1:4;r=60)").unwrap();
        assert_eq!(p(&g, "wave_length").value, ParamValue::Length { lo: 0.25, hi: 0.25, tempo: true });
        assert_eq!(p(&g, "rate").value, ParamValue::Rate { lo: 0.6, hi: 0.6 });
    }

    #[test]
    fn aliases_resolve() {
        for (text, kind) in [
            ("g", FxKind::Gate),
            ("bc", FxKind::BitCrusher),
            ("w", FxKind::Wobble),
            ("hp", FxKind::HighPassFilter),
            ("hpf", FxKind::HighPassFilter),
        ] {
            assert_eq!(parse_fx_effect(text).unwrap().kind, kind);
        }
    }

    #[test]
    fn fraction_slash_equivalent() {
        let a = parse_fx_effect("gate(1:4)").unwrap();
        let b = parse_fx_effect("gate(1/4)").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn ranges_and_transitions() {
        let hpf = parse_fx_effect("hpf(80hz-2khz)").unwrap();
        assert_eq!(p(&hpf, "freq").value, ParamValue::Freq { lo: 80.0, hi: 2000.0 });
        assert!(!p(&hpf, "freq").transition);

        let g = parse_fx_effect("gate(mix=0%>90%)").unwrap();
        assert_eq!(p(&g, "mix").value, ParamValue::Rate { lo: 0.0, hi: 0.9 });
        assert!(p(&g, "mix").transition);
    }

    #[test]
    fn unknown_type_errors_unknown_param_ignored() {
        assert_eq!(parse_fx_effect("nope").unwrap_err(), FxError::UnknownEffect);
        let g = parse_fx_effect("gate(wat=123;rate=50%)").unwrap();
        assert_eq!(p(&g, "rate").value, ParamValue::Rate { lo: 0.5, hi: 0.5 });
        assert!(g.param("wat").is_none());
    }

    #[test]
    fn resolve_converts_measures_and_freq() {
        let g = parse_fx_effect("gate(wave_length=1:4;rate=70%;mix=90%)").unwrap();
        match resolve(&g, 120.0) {
            ResolvedFx::Gate { wave_length, rate, mix } => {
                assert!((wave_length - 0.5).abs() < 1e-6);
                assert!((rate - 0.7).abs() < 1e-6);
                assert!((mix - 0.9).abs() < 1e-6);
            }
            other => panic!("unexpected {other:?}"),
        }

        let hpf = parse_fx_effect("hpf").unwrap();
        match resolve(&hpf, 120.0) {
            ResolvedFx::HighPassFilter { freq, q, .. } => {
                assert!((freq - 2000.0).abs() < 1e-3);
                assert!((q - 1.414).abs() < 1e-3);
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
