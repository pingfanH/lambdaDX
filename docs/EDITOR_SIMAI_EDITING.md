# Simai 模态编辑与代码补全

状态：首版已接入，限 Simai 文本分区。标准模式为默认值，谱面画布和 Autoplay 键位不变。

## 使用入口

在 Simai 标题栏的设置菜单，或编辑器设置中的 Simai 设置里，选择
`标准 / Vim / Helix`。同处可开关代码补全、自动候选、括号自动配对。
这些选项随工作区布局保存，重启恢复。标题栏显示模式及待完成命令；搜索时显示查询。

模式切换不修改谱面正文。重新读取正文会清除临时搜索、候选和寄存器，但不清空共享文档历史。
打开另一谱面时重置文档历史；自动应用成功和可视化同步不会清除正在使用的编辑模式或历史。

## 模块与数据流

- `simai_editor/editing.rs`：字符范围、行与单词移动、括号对象、撤销事务接口。
- `simai_editor/history.rs`：共享文档历史，将谱面变化与精确 Simai 正文、已应用原文和选区一同保存/恢复。
- `simai_editor/modal/`：Vim 操作符优先、Helix 选择优先的命令状态。
- `simai_editor/modal/search.rs`：按字符位置进行字面搜索、反向搜索和循环匹配。
- `simai_editor/completion.rs`：原文上下文、候选、片段字段与括号配对。
- `simai_editor/input.rs`：事件归属和统一事务；可打印命令只从 Text 事件执行，避免 Key/Text 重复执行。
- `ui/simai/editing.rs`：egui 光标适配、焦点事件过滤、候选浮层和可见光标。
- 文本修改继续走 `changed()`、实时应用与原文保留保存；不绕过谱面校验。
- 选项随布局保存；当前模式、命令前缀、寄存器、搜索、候选不序列化。

## 实施阶段

1. 字符安全的文本操作、移动和撤销事务。
2. Vim Normal/Insert/Visual/Visual Line 与 Helix Normal/Insert/Select。
3. Simai 上下文补全、括号配对、片段字段。
4. 输入归属、分区设置、补全浮层、模式状态。
5. 单元/UI 测试、布局重启测试、真实窗口截图与构建。

## 支持的键位

| Area | Vim | Helix |
| --- | --- | --- |
| Move | `hjkl`, arrows, `w/b/e`, `0/^/$`, `gg/G`, `2gg/3G`, counts | `hjkl`, arrows, `w/b/e`, `gh/gl`, `gg/ge`, `2gg/3G`, counts |
| Insert | `i/a/I/A/o/O`, Esc returns to Normal | `i/a/I/A/o/O`, Esc returns to Normal |
| Select | `v/V` | `v` extends selection; `x` selects a line |
| Operate | `d/c/y` followed by motion; `dd/cc/yy` | `d/c/y` applies to the current selection |
| Paste | `p/P` | `p/P` |
| History | `u`, Ctrl+R | `u/U` |
| Find | `/` / `?`, Enter, `n/N`, `f/F/t/T`, `;` / `,` | `/` / `?`, Enter, `n/N`, `f/F/t/T`, `;` / `,` |
| Brackets | `%`, `di[` / `ci[` and corresponding pairs | `%`, `mi[` / `ma[` and corresponding pairs |
| Words | `diw/ciw/daw`, `iW/aW` | `miw/maw`, `miW/maW` 后 `d/c/y` |

`/` 向后搜索，`?` 向前搜索；Enter 确认，`n` 沿原方向继续，`N` 沿相反方向继续，
`2n` 等计数可跳过多个匹配。匹配使用黄色背景显示，搜索不修改正文。
Vim 搜索后光标停在匹配首字符；Helix 选择匹配文本。字符查找后的 `;` 重复，`,` 反向重复。
`iw` 选择当前单词，`aw` 还包含一侧空白；`iW/aW` 将连续非空白字符视为整个单词。

## 光标与选区

- 普通/插入模式光标是字符间的位置；Vim/Helix 的块状光标指向当前字符。
- 选区尾端为排他边界，块状光标不会画到该边界右侧的下一字符；反向选区保留移动方向。
- 块状光标绘制、滚动跟随和 Alt+Enter 定位使用同一字符索引；模态下隐藏多余的细竖线光标。
- 连续上下移动会记住目标列，经过短行后恢复原列；水平移动、鼠标重新定位或编辑会重设目标列。
- Esc 离开可视选择时停在活动端字符，不会向右跳一格。

Ctrl+Space 打开候选，Up/Down 选择，Tab/Enter 接受，Esc 关闭。
片段内 Tab/Shift+Tab 跳转字段。输入法组合及注释中不弹补全，粘贴不会自动配对。

补全使用当前光标附近的原文，不要求整张谱面已解析成功：

- `1h[` 提供 Hold 时值，`[`、`{`、`(` 自动配对且已有闭括号不会重复插入。
- `{` 提供常用节拍细分，`(` 提供 BPM，`<H` 提供 Hi-Speed。
- Ring Note 后提供 Hold、Slide、Break、EX、星星和无头 Slide。
- `A/B/D/E` 后提供 Touch 区域，Touch 后提供 Hold 和烟花。
- Slide 形状后按起点筛选终点；例如 `1w` 只提供 `5`。
- 接受含参数的片段后选中首个字段；Tab/Shift+Tab 切字段，末字段后回到片段末尾。
- Up/Down 或鼠标选择候选，Enter/Tab 或鼠标点击接受；Ctrl+Space 手动打开。

普通输入、剪切粘贴、输入法、模式命令、补全和可视化修改共用文档历史；Primary+Z 撤销，
Primary+Shift+Z / Primary+Y 重做。Vim 的 u/Ctrl+R 和 Helix 的 u/U 使用同一历史，
在 Simai 中也能撤回可视化编辑，反向同样有效。连续输入在 0.7 秒内合并，
移动光标、切换焦点或可视化操作会分组；应用已记录的文本不新增重复撤销步。
撤销同时恢复谱面、原文和选区；未完成或解析失败的草稿按原样保留，不会强行写入谱面。
恢复时废弃未完成的后台解析任务；新的编辑会清空重做分支。模板隔离编辑仍使用自己的谱面历史。

模态 Insert 直接输入空格；标准模式保留原有 Space 播放设置。
Primary+S、Alt+Enter 保持原有保存和定位语义。
Esc 依次关闭候选/片段/搜索，再退出 Insert 或选区模式，不移走文本焦点。

## 验收

- 模式命令不触发谱面放置或播放；Key/Text 跨帧时只执行一次。
- Unicode 注释、CRLF、文件末尾及空行的范围有效。
- 绘制坐标测试检查 Helix 块状光标与选中字形对齐，Alt+Enter 测试检查判定单元格不偏移。
- 单次命令/补全可撤销，普通输入、输入法、鼠标补全和可视化编辑共用历史。
- 标准/Vim/Helix 均能撤销/重做可视化修改；交替编辑保持顺序，不重复记录应用步骤。
- 注释编辑、无效草稿和紧凑放置记录可恢复；撤销后旧解析结果不能重新应用。
- 文本和可视化编辑交替后，未修改的 Simai 原文仍保留。
- 选项经过真实布局写盘/重载测试，旧布局默认标准模式。
- 补全只替换目标范围，Slide 终点经过既有形状和几何规则筛选。

验证命令：

```sh
rtk cargo test --bin lambda_dx_editor simai -- --nocapture
rtk cargo test --bin lambda_dx_editor -- --nocapture
rtk cargo build --bin lambda_dx_editor
```

## 未支持

这是内嵌编辑子集，不是完整 Vim/Helix 运行时。不支持 Vimscript、插件、宏、
多选区、完整 Ex 命令或 LSP/AI 补全。
搜索为字面匹配而非正则表达式。片段只维护一组顺序字段，不做联动占位替换。
不支持的命令不执行，不会转发为谱面放置快捷键。
