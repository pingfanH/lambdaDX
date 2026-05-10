# Chart 保存格式方案

BPM=210, 4/4拍, 内部分辨率: 384 ticks/小节 (96 ticks/拍)

---

## 方案 A: 整数 tick

全局定义分辨率，所有时间用绝对 tick 整数表示。

```json
{
  "version": "0.4.0-tick",
  "title": "Jack-the-Ripper◆[SD]",
  "bpm": 210.0,
  "resolution": 384,
  "notes": [
    {"tick": 480, "lane": 6, "note_type": "tap"},
    {"tick": 528, "lane": 5, "note_type": "tap"},
    {"tick": 576, "lane": 4, "note_type": "tap"},
    {"tick": 624, "lane": 3, "note_type": "tap"},
    {"tick": 672, "lane": 4, "note_type": "tap"},
    {"tick": 720, "lane": 5, "note_type": "tap"},
    {"tick": 768, "lane": 2, "note_type": "tap"},
    {"tick": 816, "lane": 1, "note_type": "tap"},
    {"tick": 864, "lane": 7, "note_type": "tap"},
    {"tick": 912, "lane": 8, "note_type": "tap"},
    {"tick": 960, "lane": 1, "note_type": "slide", "slide_shape": "line",
     "slide_target": 5, "slide_duration_ticks": 144, "slide_delay_ticks": 96},
    {"tick": 1152, "lane": 3, "note_type": "hold", "hold_duration_ticks": 192}
  ]
}
```

**优点**: 简单精确，无浮点误差，类似 MIDI  
**缺点**: tick 数字本身不直观，需要心算换算

---

## 方案 C: 小节+拍+细分 (已选择)

每个音符用 measure/beat/division/offset 定位，时长用拍数分数表示。

```json
{
  "version": "0.4.0-beat",
  "title": "Jack-the-Ripper◆[SD]",
  "bpm": 210.0,
  "notes": [
    {"measure": 1, "beat": 2, "division": 8, "offset": 0, "lane": 6, "note_type": "tap"},
    {"measure": 1, "beat": 2, "division": 8, "offset": 1, "lane": 5, "note_type": "tap"},
    {"measure": 1, "beat": 2, "division": 8, "offset": 2, "lane": 4, "note_type": "tap"},
    {"measure": 1, "beat": 2, "division": 8, "offset": 3, "lane": 3, "note_type": "tap"},
    {"measure": 1, "beat": 2, "division": 8, "offset": 4, "lane": 4, "note_type": "tap"},
    {"measure": 1, "beat": 2, "division": 8, "offset": 5, "lane": 5, "note_type": "tap"},
    {"measure": 1, "beat": 2, "division": 8, "offset": 6, "lane": 2, "note_type": "tap"},
    {"measure": 1, "beat": 2, "division": 8, "offset": 7, "lane": 1, "note_type": "tap"},
    {"measure": 1, "beat": 3, "division": 8, "offset": 0, "lane": 7, "note_type": "tap"},
    {"measure": 1, "beat": 3, "division": 8, "offset": 1, "lane": 8, "note_type": "tap"},
    {"measure": 1, "beat": 3, "division": 4, "offset": 0, "lane": 1, "note_type": "slide",
     "slide_shape": "line", "slide_target": 5,
     "slide_duration": [3, 2], "slide_start_delay": [1, 1]},
    {"measure": 2, "beat": 1, "division": 1, "offset": 0, "lane": 3, "note_type": "hold",
     "hold_duration": [2, 1]}
  ]
}
```

**字段说明**:
- `measure`: 小节号 (1-indexed)
- `beat`: 小节内第几拍 (1-indexed, 4/4拍下 1~4)
- `division`: 拍内细分数 (1=拍头, 2=二分, 4=四分, 8=八分, 16=十六分)
- `offset`: 细分位置 (0-indexed, 范围 0 ~ division-1)
- `hold_duration`: [分子, 分母] 表示持续拍数 (如 [3,2] = 1.5拍)
- `slide_duration`: [分子, 分母] 滑键总时长(拍)
- `slide_start_delay`: [分子, 分母] 滑键延迟(拍)

**优点**: 音乐含义明确，人类可读  
**缺点**: 冗长，实现较复杂

---

## 方案 D: 按拍分组

同一拍的音符分组在一起，拍内用 grid 细分。

```json
{
  "version": "0.4.0-grouped",
  "title": "Jack-the-Ripper◆[SD]",
  "bpm": 210.0,
  "beats": [
    {
      "beat": 5,
      "grid": 8,
      "notes": [
        {"pos": 0, "lane": 6, "type": "tap"},
        {"pos": 1, "lane": 5, "type": "tap"},
        {"pos": 2, "lane": 4, "type": "tap"},
        {"pos": 3, "lane": 3, "type": "tap"},
        {"pos": 4, "lane": 4, "type": "tap"},
        {"pos": 5, "lane": 5, "type": "tap"},
        {"pos": 6, "lane": 2, "type": "tap"},
        {"pos": 7, "lane": 1, "type": "tap"}
      ]
    },
    {
      "beat": 6,
      "grid": 8,
      "notes": [
        {"pos": 0, "lane": 7, "type": "tap"},
        {"pos": 1, "lane": 8, "type": "tap"},
        {"pos": 2, "lane": 3, "type": "tap"},
        {"pos": 3, "lane": 4, "type": "tap"}
      ]
    }
  ]
}
```

**优点**: 紧凑，类似 Simai 逻辑，一眼看出密度  
**缺点**: Hold/Slide 跨拍表示复杂，结构大改
