# Editor 当前键盘操作

更新: 2026-10-15。Primary 在 macOS 是 Command, 在 Windows/Linux 是 Ctrl; Alt 在 macOS 是 Option。
这是当前接入版本的操作说明, 完整目标另见 [键盘方案](EDITOR_KEYBOARD_PLAN.md)。

## 播放与输入归属

| 按键 | 操作 |
| --- | --- |
| Space | 播放 / 暂停 |
| Alt+Space | 从头开始播放 |
| Alt+Enter | seek 到编辑光标, 保留播放或暂停状态 |
| Alt+Home | seek 到歌曲开头 |
| Alt+Left / Right | 后退 / 前进 1 秒 |
| Alt+Up / Down | 播放速度增加 / 减少 0.1 |
| A | 音乐开关 |
| O / P | 切换自动打歌 / 循环预览模式 |
| F1 / Primary+, | 设置 |
| F12 / ? | 快捷键帮助 |

Space 在画布空闲时生效。输入框、输入法组合、下拉菜单、对话框及未确认的放置/变换拥有输入时, 不执行播放快捷键。
先按 Enter 确认或 Esc 取消当前操作; 设置与帮助按 Esc 关闭, 然后按 Space。长按 Space 不会反复切换播放。
也可使用顶栏“播放 → 播放/暂停”。菜单仅拦截画布输入, 不临时隐藏时间轴或改变工具栏颜色。

## 光标、选择与工具

- F4 打开工具列表, Up/Down 选择, Enter 确认, Esc 取消。切换工具不清除 note 选择。
- 竖向时间轴: Up/Down 推后/提前时间, Left/Right 切 lane。横向: Right/Left 推后/提前时间, Down/Up 切 lane。
- 方向键或 Primary+方向键均可导航; 时间方向一次移动当前细分的一格, lane 方向一次移动一个 lane。加 Shift 后分别移动一个小节、四个 lane。支持长按, 同时按住时间与 lane 方向可连续斜向移动。
- 1..8 设置 lane。空闲画布导航同步播放时间, 保留播放/暂停状态; 光标在可见范围内移动时视图不动, 超出边界才继续滚动。编辑光标仍可移到歌曲末尾以外以继续放置, 此时播放器停在可播放范围内。放置草稿、变换与 Slide 分段编辑保留各自的方向键含义。
- J 打开定位框, 输入小节小数 (如 `2.5`) 或秒数 (如 `3s`), Enter 同时定位编辑光标、播放时钟和时间轴视图, 保留播放/暂停状态; 超出歌曲末尾时三者一起限制到末尾。
- Home/End 移动编辑光标到谱面起点/末尾; PageUp/PageDown 按页面移动。F8 切吸附, [ / ] 切细分。
- Tab/Shift+Tab 按时间跳到下一个/上一个 note, 同时值按 lane、稳定 ID 排序, 到头后循环; 支持按住连续切换, 同步播放时间, 仅在目标超出可见边界时滚动视图, 不自动选择。鼠标点击 note 会将其设为新的 Tab 参考, 点击空白则以点击位置为参考, 点击本身不因此触发 seek。光标位于两个 note 之间时从当前位置向前/后查找。F2 选择候选, Shift+F2 添加或移除候选。Primary+A 全选, Primary+Shift+A 清选择。
- 键盘光标指到 note 后, 右侧属性和 F5 直接显示该 note; Delete/Backspace/D 直接删除它, 无需先选中, 不影响其他选择。光标指向空白时不误删其他选择; 鼠标操作仍使用选择集合。
- Primary+Enter 设置范围锚点, 方向键、1..8 或 Tab 移动另一角, Enter 或再次 Primary+Enter 确认范围选择。Primary+Shift+Enter 开始或 Shift+Enter 确认可追加范围; Esc 取消并保留原选择。范围可跨屏, 越界时视图继续滚动; 框选时不会放置 note, 也不会切换工具或启动播放。
- Primary+Delete/Backspace 删除整个选择集合; 普通 Delete/Backspace/D 仍只删除光标指向的 note。选中的一组可直接使用 G/S/R、Shift+D 或 Primary+C/X 操作。
- 在空闲画布显式点击鼠标可回到鼠标操作; 未确认键盘事务不会被鼠标移动接管。

## 放置

1. F4 选工具, 方向键与 1..8 定位。也可用 T/H/C 等单键切工具, 但选中 note 时 G/S 优先用于变换。
2. Tap/Touch: Enter 放置。Hold/TouchHold: Enter 固定头部, 时间方向键设置尾端, Enter 完成; Esc 取消。
3. Star: Enter 固定头部, 时间方向键设置启动拍, Enter 进入轨迹阶段; 1..8 设置结束 lane, 时间方向键设置结束拍, Enter 完成。
4. Star 轨迹阶段默认直线; Q/P/S/Z/V/W 或 - / , / . / ; 选择形状。Shift+Q/P 为 QQ/PP。Shift+Enter 确认并继续下一连接段。

Hold 尾端可为零长度。Star 非法端点或早于启动拍的尾端不会提交, 可继续调整。Star 放置的 Esc 逐步返回, 再次 Esc 取消整条草稿。
一次完整 Hold/Star 放置只产生一次撤销; 确认事件不会额外放置另一个 note。

## 编辑与历史

- 选中单个 Star 后 E 进入/退出轨迹编辑; Tab 切同头子 slide, Primary+Left/Right 切连接段。
- 键盘轨迹修改使用形状键与 lane 键, Enter 提交, Esc 取消草稿。Primary+Shift+D 新建同头轨迹。
- 鼠标轨迹编辑保留原逻辑: 形状键后按 lane 立即修改路径。
- F7 打开光标指向的 Hold/Star (鼠标模式下为单个所选 note) 的手柄菜单, 方向键选项目、Enter 进入调整; 方向键调整, Enter 确认, Esc 取消。
- F5 打开光标指向的 note (鼠标模式下为单个所选 note) 的时间、Break、EX 草稿, Apply/Enter 提交, Esc 取消。
- G/S/R 进入移动/时间缩放/旋转。键盘操作后可用方向键调整、直接输入数值、Enter 确认、Esc 取消; 鼠标与滚轮操作仍保留。
- Shift+D 复制并进入移动, 确认点击不会触发普通放置。Shift+R 重复上次已确认变换/复制, 或以最近两次兼容放置的时间/lane 差继续放置。删除会从参考记录中移除已删除 note, 重新使用剩下最新两次放置; 不足两个、零位移或属性不兼容时不重复。撤销、重做及其他修改仍打断放置参考。
- Primary+C/X/V 复制/剪切/粘贴, 键盘粘贴锚定编辑光标。Primary+Z 撤销, Primary+Shift+Z 或 Primary+Y 重做。Delete/Backspace/D 在键盘光标模式删除指向 note, 在鼠标模式删除选择。
- Primary+S 保存到实际打开的文件和难度; Primary+Shift+S 另存为 `.txt` / `.json`; Primary+Shift+E 导出 Simai; Primary+O 打开文件或谱面文件夹。顶栏“文件”提供相同操作与重新载入、退出。无源文件时保存会先要求另存为, 失败或取消保留未保存状态。详见 [文件操作](EDITOR_FILES.md)。
- F3 搜索支持 Tools、Properties、Handles、Jump、Open chart、Save chart、Save As、Export Simai; 输入关键字, Up/Down 选择后 Enter 执行。

## 尚未完成

原方案中的 F6 完整面板焦点切换、快捷键重绑定、F3 全命令搜索/文件路径输入、Touch 区域环菜单和独立“仅星星头”命令尚未接通。
当前 F3 仅提供上述命令子集; J 暂不解析 `2+1/4` 这类分数写法。不要按完整设计提案推断这些功能已经可用。
