# Player UI 优化计划

> 目标:在保持现有黑底 + 青色强调、贴纸式动效风格的前提下,把 `player_ui` 从
> “能跑通的前端”打磨成随手可用的播放器/试玩前端,并补齐评分闭环与输入可达性。
>
> 现状入口:`src/player_ui_main.rs` → `src/player_ui/mod.rs`(页面派发 + 转场)
> 页面:`src/player_ui/pages/{start,song_select,settings,gameplay,pause}.rs`
> 状态机:`src/player_ui/state.rs`(`Page = {Start, SongSelect, Settings, Gameplay, Pause}`)
> 主题/原语:`theme.rs` / `draw.rs` / `anim.rs` / `input.rs` / `library.rs` / `perf.rs`

图例:`[ ]` 未做 · `[~]` 部分 · `[x]` 已有基础

---

## P0 — 闭环与可用性(先做)

- [ ] **结算页(Result)** — 目前 `Page` 没有结算页,歌曲放完没有分数/评级闭环。
      新增 `Page::Result` + `pages/result.rs`:读取 `app.pad` 的 combo / score /
      dx_score / 判定计数,展示 曲名·难度·评级(SS/S/A…)、精度、最佳记录,
      提供「重玩 / 返回选歌 / 下一首」。`state.rs` 里检测 `song_time >= duration`
      触发切页。
- [ ] **游玩内 HUD:Combo / Score / 精度 / 判定统计** — `pages/gameplay.rs::draw_hud`
      目前只有进度条与播放控制;在顶部 HUD 或画面侧边加入 Combo 数、Score、
      精度、判定(Perfect/Great/… )计数。数据源:`app.pad.combo()`、
      `app.pad.core_score`(lnmai-core 读数)。
- [ ] **键盘 / 手柄导航** — 菜单目前仅鼠标(`input.widget` + 命中)。加入焦点导航:
      ↑/↓/←/→ 移动焦点、Enter 确认、Esc/Backspace 返回;焦点环 + 高亮。
      页面各自维护焦点索引,或统一在 `pages/mod.rs` 做一层「可聚焦控件表」。
- [ ] **Toast 队列** — `pages/mod.rs::toast` 只显示 `app.error`。加入 info/success/warn
      队列 + 淡出 + 堆叠;`state.rs` 用 `VecDeque<Toast>` 承载,替换现有单 error。
- [ ] **加载 / 错误态** — 选歌与音频解码是后台线程(`loading_song` / `preparing`),
      目前只有 gameplay 内的不定进度条。补:选歌列表骨架屏、解码进度、失败可重试、
      曲库扫描进度。

## P1 — 曲库与设置

- [ ] **曲库增强** — `library.rs::scan` 一次性扫盘,列表无搜索/排序/筛选。
      加:标题/艺术家搜索框、按难度/标题排序、难度筛选、收藏、最近游玩;
      大曲库虚拟滚动(仅绘制可视行)。
- [ ] **封面加载** — `library.rs::pump_covers` 每帧预算内加载。加:邻近曲目预取、
      占位图、失败兜底图标、切歌时避免重复解码。
- [ ] **设置扩展** — `pages/settings.rs` 现有 audio/gameplay/display 三类。
      补:音量(总/BGM/SFX 分离)、判定偏移(judge offset)、note 皮肤/主题、
      背景视频开关与参数、按键说明页;每类「恢复默认」+ 全量保存到磁盘。
- [ ] **设置持久化** — 复用 `app::params`(F1 面板同源)或独立 `player_ui` 配置文件;
      启动时载入、退出/改动时写回。

## P2 — 体验完善

- [ ] **开播引导** — 进入游玩前 3-2-1 倒计时 / 或 await-first-input,避免一进就漏首拍。
- [ ] **窗口失焦自动暂停** — 失焦(`miniquad` focus 事件)时暂停并标记。
- [ ] **i18n 与文案统一** — 现有中英混排(「开始 · 选歌」「AUTO」「PLAY」…);
      集中到一处字符串表,可选语言切换。
- [ ] **转场性能** — `pages/mod.rs::capture_transition` 每次转场
      `get_screen_data()` 读回整帧(可能 GPU 停顿)。改为可选/降频,或用更便宜的
      遮罩动效替代;`MAI2_UI_TRANS_HOLD` 保留用于截图。
- [ ] **移动/小屏布局** — `state.rs` 有 `mobile_ui` 开关;补齐小屏重排、更大点击热区、
      安全区内边距、触屏滚动手势。
- [ ] **无障碍/可读性** — 高对比模式、判定色盲友好配色、字号档位、减少动效选项。

## P3 — 打磨

- [ ] **视觉一致性** — 统一间距/圆角/命中盒(`ctx.scale` 下的 `CUT`/`PADDING`);
      hover/active/focus 三态齐全;鼠标指针随可点区域变化。
- [ ] **性能** — `draw::text_width` 每帧重复测量 → 文本度量缓存;减少每帧分配;
      预载下一首封面/音频。
- [ ] **计分细节** — 精度公式与评级阈值、DX 分数的展示口径确认;断连(0 combo)提示。
- [ ] **调试面板** — 把 F1 `params_panel` 的能力(流速/尺寸/判定贴图)在 player UI
      内做只读摘要或「高级」入口,便于现场调参而不必开 preview。

---

## 设计原则(保持)

- 表面:`VOID/PANEL/PANEL_ALT/RAISED`;分隔线 1px `BORDER`;强调色单一 `ACCENT`。
- 动效:列表错峰入场(`D_CARD`+`STAGGER`)、弹簧弹入(`D_POP`)、按下下沉
  1px、页面斜切扫过(`D_SWEEP`);统一走 `anim.rs`。
- 缩放:一切尺寸乘 `ctx.scale`;文字走 `draw::text*` 与字体缓存。
- 命中:UI 控件优先于 pad 感应区(`pages/mod.rs` 已保证「控件先拒后 pad」)。

## 验证

- 截图:`MAI2_*` 环境变量 + `get_screen_data().export_png`(见 preview/editor 的用法)。
- 转场冻结:`MAI2_UI_TRANS_HOLD=0.5`。
- 性能:`perf.rs::time` 打点(已有 `transition.readback/upload`)。
- 每个 P0/P1 项要求:能截图复核 + 有对应单测(状态机/数值),不靠肉眼回归。
