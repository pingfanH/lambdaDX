# MCP 与轻量 Agent 实现计划

状态: **P0–P3 已实现并验收; P4 实施中; P5 待实现**。
更新日期: 2026-10-21。工作分支: `agent`。设计调查基线: `62df204`;
实施基线: `9af3500`。下文未勾选项描述后续目标, 不代表已有网络或 Agent 入口。

用户已确认: 同时支持外部 MCP 客户端控制正在运行的编辑器, 以及编辑器内置轻量 Agent;
首要用途是理解、检查和修改谱面。
库的一手资料、版本与待验证限制见 [依赖研究](MCP_AGENT_RESEARCH.md)。
现有行为另见 [Simai 编辑](EDITOR_SIMAI_EDITING.md)、[文件保存](EDITOR_FILES.md)、
[工作台](EDITOR_WORKBENCH_DESIGN.md)、[交互规范](EDITOR_INTERACTION_SPEC.md)。

## 1. 目标与首版范围

首版形成一个完整工作流:

1. 读取当前谱面和用户显式选择的范围, 解释其结构与 Simai 语法。
2. 确定性地检查数据、时值、区域和 Slide 形状, 对问题定位。
3. 根据自然语言生成有范围限制的修改提案, 展示具体差异。
4. 用户在编辑器确认后, 原子应用为一个可撤销步骤。
5. 保持现有原文保留、预览核心同步、保存和自动保存行为可理解。

MCP 本身不需要模型、API Key 或内置 Agent; 用户可以使用自己的外部客户端。
内置 Agent 不需要开启 MCP 监听, 两者只共享工具契约和执行逻辑。

首版不做: 通用 Shell/文件管理、浏览器代理、任意网络工具、后台自主运行、
多 Agent 协作、插件系统、向量数据库、全自动作谱、自动下载媒体、直接控制判定核心。
不把现有确定性 Simai 补全改为模型补全。Autoplay/模板的写工具后置。

## 2. 代码调查与必须保留的约束

| 现状 | 来源 | 设计影响 |
| --- | --- | --- |
| 编辑器是独立 bin, 通过 `#[path]` 包含共享模块, 不是现成 library | `editor/src/main.rs`, `Cargo.toml` | 不为接入 Agent 抽取整个播放器或编辑器 |
| 文档、选择、历史和草稿由主线程拥有 | `editor/src/state.rs`, `edit_ops.rs` | 后台只持有不可变快照和请求通道, 不包一把全局状态锁 |
| 音符时间和时长为小节, 小节 1 对应播放 0 秒 | `src/app/types.rs` | 工具字段显式写单位, 不把播放核心用的秒数当原文 |
| Slide 总跨度包括启动延迟 | `Slide.slide_duration`, `core_chart.rs` | 对外分别说明延迟与运动时长, 转换集中实现 |
| Simai 原文与 ChartDoc 同时存在, 草稿可能尚未应用 | `simai_editor.rs`, `simai_editor/format.rs` | 查询说明读取哪种版本; 写提案遇到未应用草稿时拒绝覆盖 |
| 格式保留逻辑会重新解析验证 | `format::Document::update` | 格式预检不是纯字符串操作, 需要避开主线程与核心竞争 |
| 现有 `document_revision` 在 `load_chart` 时增加, 并非每次编辑增加 | `src/player/state.rs` | 不能直接作为写入的并发版本号 |
| 核心更新已有单 worker 与过期结果丢弃机制 | `editor/src/runtime/sync.rs` | 复用/小幅扩展调度, 不为每个工具另开核心会话 |
| Simai 解析经现有 FFI, 不是独立纯 Rust parser | `src/app/maidata.rs` | 后台解析仍可能竞争核心, 必须测试而非假设无阻塞 |
| 图形历史主要保存 ChartDoc, 文本另有历史 | `edit_ops.rs`, `simai_editor/editing.rs` | 外部事务须额外保存原文状态, 撤销不能只恢复 AST |
| 保存含原子写入和格式/难度保护, 自动保存延迟执行 | `io.rs`, `io/autosave.rs` | 不从工具直接 `fs::write`; 应用确认须明确自动保存后果 |
| 自由分屏保存分区身份和配置, 不保存独立文档 | `ui/workbench/` | Agent 是可停靠分区, 多个视图不能各启动一个 Agent |

`io::validate` 可复用, 但尚不足以独立验证不受信任的外部参数。
还需验证 note 类型与区域的组合、Slide 起终点和连接段、字段范围、批量上限等。
核心编译成功也不等于手感合理或玩家可完成, 检查结果必须区分确定错误与建议。

## 3. 总体架构

```text
外部 MCP 客户端 -- Streamable HTTP -------------------------+
外部 stdio 客户端 -- lambdadx_mcp 代理 -- Streamable HTTP --+ |
                                                         v v
                                              MCP adapter (rmcp)
                                                         |
编辑器 Agent 分区 -- Agent runner (genai) ------------------+
                                                         |
                                 共享工具目录 / typed request / 策略检查
                                                         |
                                   有界请求队列 + 取消 / 截止时间
                                                         |
                       主线程 EditorHost -- 不可变快照 --> 预检 worker
                               |                         |
                               +<------ 候选与差异 ------+
                               |
                         用户确认 + 最终冲突检查
                               |
                  现有编辑历史 / Simai 原文 / ChartSync / 保存流程
```

只引入一个小型共享 crate, 不立即拆分领域模型。建议结构:

```text
crates/lambdadx-automation/
  Cargo.toml
  src/lib.rs
  src/contract.rs          # 不含 GPU/Lean/编辑器状态的 DTO 与错误
  src/catalog.rs           # 工具名、描述、schema、权限分类
  src/bridge.rs            # 有界请求与响应、取消、截止时间
  src/mcp/mod.rs           # rmcp server, Resources/Tools 映射
  src/mcp/http.rs          # loopback 启停、鉴权、连接限制
  src/mcp/proxy.rs         # stdio 到 HTTP 的固定代理
  src/agent/mod.rs         # 一次运行的状态机与预算
  src/agent/provider.rs    # genai 对话/工具结果转换与能力检查
  src/agent/policy.rs      # 暴露给模型的工具子集与上下文策略
  src/bin/lambdadx_mcp.rs  # 仅启动代理, 不启动图形窗口
editor/src/automation/
  mod.rs                   # 主线程 EditorHost 入口
  snapshot.rs              # 文档身份、快照令牌、冲突检查
  query.rs                 # 谱面范围读取与确定性检查
  proposal.rs              # 候选构造、验证、差异与生命周期
  commit.rs                # 确认后原子应用、历史与状态协调
editor/src/agent/
  mod.rs                   # 会话拥有者、后台 runtime 生命周期
  settings.rs              # provider/model/endpoint/凭据引用
editor/src/ui/automation.rs # 共享提案差异与人工确认, 不依赖模型
editor/src/ui/agent.rs     # 对话、提案、确认与运行状态
```

这是职责划分, 不要求首个提交创建所有空文件。
共享 crate 不依赖 `ChartDoc`、macroquad、egui、Lean。EditorHost 负责 DTO 转换。
先保留领域实现的位置, 只为确有两个调用方的工具接口增加抽象。

### 编译与运行隔离

- 根包提供独立的可选 feature: `editor-mcp`、`editor-agent`, 都启用共享 host。
- 共享 crate 的 `mcp`、`agent`、`stdio-bridge` feature 分开, 不开启不需要的传输。
- stdio bin 用 `required-features`, 可独立构建, 不拉播放器/Lean/窗口依赖。
- 不启用这些 feature 时, 当前编辑器构建和启动不需要 Tokio、模型或服务器。
- Agent/MCP 默认关闭; 后台共用一个受管理的 Tokio runtime, 不给每个分区创建 runtime。
- 初期只支持桌面, 明确 wasm/mobile 不支持; 构建条件不能破坏原有 targets。
- 依赖选择 registry 稳定版本并提交 lockfile; 实施前用小型探针验证 features/MSRV。

## 4. 共享工具契约

所有请求使用 `schema_version = 1`。以窄工具为入口, 不提供任意 Rust 表达式、
JSON Pointer 或完整 ChartDoc 替换。模型和 MCP adapter 都不能跳过 host 校验。

### 身份与时间

- 编辑器实例有 `instance_id`, 当前打开文档有 `document_id`。重载、切难度、
  模板隔离进出都使旧文档身份或上下文失效; 不用路径单独标识一个活文档。
- 每次读取返回 host 生成的 opaque `snapshot_token`。分页必须使用同一快照。
- note ID 对外为字符串, 避免 JSON/JavaScript 对 `u64` 的精度损失。
- 谱面范围使用 `[start_measure, end_measure)`, `start_measure >= 1`。
- 位置叫 `measure`, 时长叫 `duration_measures`; 不使用含糊的 `time`。
- 如返回 `playback_seconds`, 明确为音符播放时钟, 音频 offset 单列。
- Slide 使用 `delay_measures` 与 `motion_duration_measures`, host 转为现有总跨度。
- Touch 使用 `A1`、`B1`、`C`、`D1`、`E1` 等区域名; Ring lane 仍为 `1..8`。
- DTO 的数值进入现有 `f32` 模型前做有限性/可表示范围检查; 不主动对齐网格,
  应用结果返回实际存储值。需要精确表达 Simai 分数时走后续原文范围工具。
- EACH、派生星头和模板展开来源只读; 不允许模型随意写派生标志。
- 查询排序固定为位置、note ID, 不以 HashSet 顺序或屏幕像素作为契约。

### 首批工具

| 工具 | 输入与输出 | 是否改文档 |
| --- | --- | --- |
| `chart_get_context` | 文档摘要、BPM、来源格式、难度槽、选区、草稿/核心/自动保存状态、快照令牌 | 否 |
| `chart_query_notes` | 指定快照、小节范围/ID/类型过滤、分页; 返回显式单位的 note DTO | 否 |
| `chart_read_simai` | 指定快照和原文行/字符范围; 返回真实原文、范围映射和是否为未应用草稿 | 否 |
| `chart_check` | 指定快照/范围与检查级别; 返回规则 ID、严重度、note ID/位置及限制说明 | 否 |
| `chart_propose_edit` | 目标文档/快照、`operation_id`、说明、批量 typed edits; 返回提案 ID、差异、校验结果 | 只建提案 |
| `chart_proposal_status` | 返回提案的等待确认/已应用/拒绝/过期/失败状态及应用后的快照 | 否 |
| `editor_focus_range` | 显式范围或 note ID; 在主线程定位/高亮, 不更改谱面 | 仅视图 |

首版没有模型可调用的 `approve`、`apply`、`save`、`undo`、`open_file`。
应用与撤销由编辑器 UI/用户现有命令完成; 不能让模型批准自己的请求。
`chart_propose_edit` 创建会话状态, MCP tool annotations 不冒充完全只读;
annotations 只是描述, 不能代替服务端权限检查。

第一组 edit 操作:

- `add_notes`: host 分配 ID, 支持 Tap/Hold/Touch/TouchHold 与完整 Slide DTO。
- `update_notes`: 指定明确 ID 和允许字段, 未提供字段保留原值。
- `delete_notes`: 只删除列出的主谱面 note, 同步清理其关联数据。
- `transform_notes`: 对明确 ID 集合移动小节、旋转区域/轨迹; 复用现有几何规则。

不隐式使用会变化的当前选区, 不默默截断批量、吸附时间或把越界位置 clamp 成另一位置。
一次请求有任一操作无效, 整个提案失败; 不留下半张已修改的谱面。
更新/删除首版仅针对主谱面原始音符。模板定义、展开 note 和 Autoplay 编辑返回明确不支持。
精确 Simai 文本范围替换作为后续阶段, 不用整段重写替代结构化编辑。

建议初始限制: 一页 128 个音符、一次最多 256 个 edit/受影响音符、
单提案输入 256 KiB、原文响应 16 KiB、单工具结果 64 KiB。超限返回分页/缩小范围提示。
这些是项目默认值, P0/P1 依据实测调整; 不通过静默裁剪制造不完整上下文。

### 结果与错误

成功结果包含文档身份、快照、适用范围与必要 warnings。
共享错误至少包括 `NO_DOCUMENT`、`DOCUMENT_CHANGED`、`EDITOR_BUSY`、
`UNAPPLIED_DRAFT`、`INVALID_ARGUMENT`、`UNSUPPORTED_OPERATION`、
`VALIDATION_FAILED`、`FORMAT_PRESERVATION_FAILED`、`PERMISSION_DENIED`、
`QUEUE_FULL`、`CANCELLED`、`DEADLINE_EXCEEDED`、`CORE_UNAVAILABLE`。

MCP 协议/参数错误按协议返回; 工具执行错误返回可读内容和 `isError`,
不伪装为成功文本。对内置 Agent 使用同一错误分类, 不依赖分析字符串判断。

### 交互示例

用户提出“把这几个音符向后移动一拍”, Agent 先确认明确 ID 和当前 4/4 小节语义,
将一拍换算为 `delta_measures = 0.25`, 然后提交以下形状的请求。
这是目标契约示意, P0 以实际生成的 JSON Schema 为准:

```json
{
  "schema_version": 1,
  "document_id": "doc_example",
  "snapshot_token": "snapshot_example",
  "operation_id": "move_example",
  "summary": "Move the specified notes one quarter-measure later",
  "edits": [
    {
      "kind": "transform_notes",
      "note_ids": ["101", "102"],
      "delta_measures": 0.25,
      "lane_rotation": 0
    }
  ]
}
```

返回提案 ID 和差异不代表已修改文档; 用户确认后才变为 Applied。
若“一拍”指别的细分或范围不明确, Agent 应先澄清, 不能只靠当前网格猜测。

## 5. 提案、并发与可撤销事务

### 快照不是现有 document_revision

P0 不要求把所有 UI 写路径重构成新命令系统。采用按需快照与最终比较:

1. 主线程在安全帧点捕获 canonical authored ChartDoc、原文文档、草稿状态、来源和难度。
2. host 保存快照与令牌, worker/客户端只能引用令牌, 不能自行构造有效版本。
3. 写提案生成前及应用前, 核对文档 incarnation 和实际 authored 数据/原文是否仍与基线相同。
4. 摘要/hash 可以加速比较, 不能只相信客户端版本号或未覆盖全部字段的散列。
5. 若期间修改又完整恢复到相同内容, 可按内容等价接受; 文档重载/切换仍使旧令牌失效。
6. 播放头、鼠标悬停、界面主题变化不使谱面快照失效; 文档内容与草稿变化必须失效。

这样不遗漏仍在直接改 ChartDoc 的旧 UI 路径, 也不每帧 clone 全谱/序列化全谱。
按请求限流捕获快照, 限制保留数量和总字节。长期统一编辑 revision 另做渐进优化。

### 提案生命周期

`Preparing -> AwaitingApproval -> Applying -> Applied`。
旁路终态为 `Rejected`、`Expired`、`Cancelled`、`Failed`。
每个提案绑定连接/运行拥有者、文档、快照、候选内容和差异, 不能审批后替换 payload。
首版全编辑器最多一个写提案等待确认, 其他请求返回忙; 读查询可继续。
逻辑会话结束、运行取消、文档切换、超时或撤销权限会撤销未应用提案。
HTTP 连接复用/短暂重连不等于逻辑会话结束, 不能因一次正常响应结束就丢弃提案。
首版提案建议 10 分钟过期, 每个实例限制快照/终态记录数量与总内存,
清理后查询明确返回已过期, 不无限保留完整候选。
已开始的原子提交不可假装撤回; 返回真实 Applied, 用户可使用一次撤销恢复。

### 预检与提交

1. 完整解析 DTO, 验证参数/权限/范围/身份, 拒绝未知字段与不完整 Slide。
2. 使用基线快照构造候选, 不改变 live ChartDoc、选择或 dirty 状态。
3. 复用领域操作和验证; 对有 Simai 原文基线的文档执行 `Document::update`
   及必要的解析等价验证。原生 JSON 仍保留完整 authored 数据, 不强迫转为 Simai 保存。
4. worker 返回稳定候选、原文、结构与原文差异、关联删除列表和 warnings。
5. UI 显示精确 ID/小节范围、增删改计数及必要原文 diff, 用户可定位、确认或拒绝。
6. 主线程最终检查 idle、身份、内容、原文、权限和自动保存授权, 然后一次提交。
7. 只 push 一次历史, 原子替换经过验证的候选与原文, 协调选择/固定对象/Slide 目标。
8. 使用现有 ChartSync 更新派生视觉与判定核心; 不在提交路径同步重建核心。

事务历史必须保存 **ChartDoc + 格式文档/原文基线**。为外部事务增加必要的历史载荷,
不要让未参与本次功能的 UI 写路径全部迁移。一次 Undo/Redo 必须同时恢复音符和原文,
尤其是注释、换行、`[1:2]`、`[2:9]` 与未改动的 Slide 记法。
确认提交后应协调/清空旧文本输入撤销基线, 防止旧字符串历史覆盖新文档;
拒绝提案不得影响文本光标、模态命令或输入历史。

格式保留失败时拒绝提案, 不悄悄规范化整张谱面。
核心异步更新失败与文档提交是两种状态: 显示已应用但预览核心失败, 不悄悄保存或伪装回滚。

### 忙碌、核心与自动保存

- 未应用/冲突 Simai 草稿、IME、拖拽、键盘事务、文件对话框、模板隔离时不接受写提案。
- 首版涉及 FFI/格式重新解析的写预检仅在暂停且 idle 时调度; 忙时不自动暂停或取消用户编辑。
- `chart_check` 的结构规则可检查快照; 核心级检查显式请求, 在可用后端与安全调度下执行。
- 后台 Tokio 不直接调用 native core。预检需复用/扩展既有串行任务通道, 防止
  parser、seek、playback、build/free 互相争抢。必须包含取消/过期结果丢弃与延迟测试。
- 应用不主动调用 Save。若当前自动保存开启, UI 确认必须明确这次应用也允许
  按现有规则写回当前来源; 未取得此授权不应用, 不靠隐藏修改 auto_save 规避。
- 来源或自动保存设置在确认前变化时重新确认。取消提案不触发 dirty 或自动保存。
- 保留现有跨格式限制; 含不能用 Simai 保存的内容时显示真实保存目标/限制。

## 6. MCP 实现

### 首选传输

编辑器启用后, 在 `127.0.0.1` 随机可用端口提供 `/mcp` Streamable HTTP。
MCP adapter 只做协议与 typed tool request 转换, 不直接拥有编辑器状态。
HTTP 鉴权和网络 IO 在后台 runtime; 主线程按帧非阻塞取请求/投递结果。
有界队列, 写事务串行, 超载立即返回, 不允许无限积压。

提供可选 `lambdadx_mcp --instance <id>` stdio 代理:

- stdin/stdout 仅 MCP JSON-RPC, 日志到 stderr。
- 连接用户指定的已运行实例, 不在代理中复制领域逻辑、加载另一个谱面或开启图形窗口。
- 多实例必须明确选择, 不默认连接第一个窗口或最后使用文件。
- 编辑器未运行时说明原因, 首版不自动启动并打开文档。
- 代理仅转发白名单 Tools/Resources, 保留结构化结果、错误和取消语义。
- 工具列表来自同一目录, 不再维护第二套手写 schema。

不采用自制 socket/JSON-RPC 协议, 不实现旧 HTTP+SSE 传输。
不把外部 MCP 客户端连接功能扩成内置 Agent 可任意连接第三方 MCP 的通用宿主。

### MCP 能力范围

首批 Tools 见第 4 节。最小 Resources 为静态 Simai 语法/时间单位说明;
可提供带快照令牌的只读片段 URI, 返回内容必须与 Tools 一致并经过同一权限检查。
不把整个音频、所有文件或所有谱面隐式暴露为 resource。
Prompts 可后置, 例如解释选区/检查节奏, 不影响首版闭环。
取消必须从 MCP request context 映射到 bridge; 超时和断连的写请求不能稍后执行。

### 本机安全

- 默认关闭, 启用时显示 endpoint、实例 ID、连接数、权限及关闭/撤销入口。
- 每次启用产生高熵本机 token, 每个 HTTP 请求检查 Bearer token。
- 仅 loopback 监听; 不允许通过配置悄悄绑定 `0.0.0.0`。
- 校验 Host 与 Origin, 拒绝未授权网页 Origin; 无 Origin 的本机客户端仍必须认证。
- endpoint/token 仅写入用户私有运行记录, Unix 权限 `0600`、目录 `0700`,
  Windows 需等效用户 ACL。不写入仓库、日志或聊天正文; 退出删除过期记录。
- 本机发现文件用于已知实例的连接, 不能成为让代理携带 token 访问任意 URL 的入口。
- 设置 read-only 与 proposal 权限; 默认 read-only, 用户在编辑器显式启用提案。
- tool annotations/客户端根目录声明均不是授权依据。
- 首版不承诺远程 OAuth/多人权限; 远程部署必须另行设计, 不能直接暴露这个本机端口。

## 7. 轻量 Agent 实现

使用 `genai` 作为模型适配, 自己实现小型受控运行状态机, 不引入大型 Agent 框架。
模型选择、HTTP 请求、工具调用循环与 UI 相互独立。

### 一次运行

`Idle -> Requesting -> Streaming -> ExecutingTools -> Requesting`。
可进入 `AwaitingApproval`、`Completed`、`Failed` 或 `Cancelled`。
单会话同一时间一个 run; Stop 取消网络请求、排队工具和未应用提案。
不把中断后的 run 自动恢复成后台任务。

1. 以用户当前消息 + 显式选区摘要开始, 按需调用查询工具, 不发送整首谱面和音频。
2. 模型只能看到设置允许的工具子集。工具名、参数和返回结果仍由 host 逐项验证。
3. 流式展示普通文本, 但 **仅在完整 tool call 参数收齐并验证后执行**。
4. 同轮多个 tool calls 按顺序处理, 不并发写; 每个 call ID 都有对应结果,
   包括无效、跳过或取消, 不把 provider 的调用 ID 丢失。
5. 提案生成后返回结构化 AwaitingApproval, 不长时间占用一个工具请求等待点击。
6. 确认后 UI 发出新事件和应用快照; 用户需要解释或下一步时再继续模型调用。
   旧 run 不因一次迟到确认无限自动续跑。
7. 短期聊天保存在内存, 手动清空; 首版不自动持久化谱面正文和完整工具日志。

### 限额与失败

建议默认每次用户请求最多 8 轮模型调用、24 次工具调用、总 120 秒,
单次网络超时 60 秒、只保留受控大小的工具输出。限额在发请求/执行前检查。
等待人工确认不占模型活动时间, 受提案自身过期时间约束; 等待时不持续请求模型。
如果 provider 没有完整 usage, 显示未知/估计, 不宣称可以精确控制费用。
输出 token 上限按 provider 已验证能力设置; 本地限额仍独立生效。

处理工具名幻觉、无效 JSON、未知字段、不支持 tools、重复调用、
工具轮数耗尽、断流、鉴权失败、限流和 endpoint 超时。
不对可能已经执行的工具盲目重试。`operation_id` 在会话内幂等,
相同 ID 与相同参数返回原结果, 相同 ID 搭配不同参数报错, 不替换待审批内容。
断流的半截参数不执行; provider 同轮无效调用必须形成合法的错误工具结果。

### 配置与凭据

- 配置至少有 provider、model、可选 base URL、凭据引用和上下文/工具权限。
- 首版验证一个实际工具调用 provider, 再验证一个兼容 endpoint 或本地模型路径;
  支持列表按实测列出, 不把 SDK 声称支持误当所有模型都支持同一种 tool 行为。
- 环境变量/系统凭据引用优先, 不把明文 Key 写进工作区布局、谱面或 git。
- 仅保存非秘密配置和环境变量名; 不默认读取不相关凭据, endpoint 的变更须显式确认。
- 连接测试必须说明会产生联网请求; MCP-only 和 fake provider 测试不需要 Key。
- 不从谱面内容或模型 tool 参数修改 provider/base URL/密钥。
- 谱面、注释、工具输出都是不受信任的数据, 不能提升权限或授权保存。
- 请求内容仅发送给用户选定 endpoint。提示注入防护以工具能力和确认流程为主,
  不是只依靠一条 system prompt。

### UI 与设置归属

使用现有 egui 工作台新增 Agent 分区, 提供会话、输入、发送/停止、
工具运行记录、问题定位、提案 diff 和确认/拒绝。小面板中用紧凑可扫描布局。
在现有设置入口集中配置 MCP 启停与模型连接; 不增加启动营销页。

会话和运行属于编辑器实例, 分区只持有视图配置。关闭分区不偷偷继续运行;
运行中关闭时提供明确停止语义。分屏不复制 agent/token/runtime。
布局只持久化分区、宽度、滚动等安全视图字段, 不序列化密钥/运行/未批准提案。
增加分区时处理旧布局加载与版本兼容, 不能因未知枚举让用户所有分屏配置消失。

## 8. 分阶段实施与验收

每阶段独立提交, 都要能构建和测试。先打通窄闭环, 再扩大操作集合。

### P0: 契约与依赖可行性

- [x] 用最小 Cargo 探针核对 rmcp、genai、Tokio、schema 版本/features/MSRV。
- [x] 定义 DTO、共享工具目录、错误、实例/文档/快照令牌与权限模型。
- [x] 定义单实例 runtime、bridge 的容量、截止时间与关闭流程。
- [x] 可选依赖隔离: feature-off 依赖图不包含 automation/Tokio/rmcp/genai。
- [x] 独立 stdio bin 不链接图形/Lean (P3 独立构建及依赖图验证)。

验收: fake EditorHost 能列工具、读取快照、拒绝无效/过期请求, 没有网络与模型依赖。

共享 crate 位于 `crates/lambdadx-automation`。工具目录从同一 DTO 生成输入/输出 schema,
通过 OnceLock 只构建一次, 同时保存严格解析函数。当前七个工具为摘要、音符查询、
原文读取、结构检查、范围定位、创建提案与查询提案状态。输入拒绝未知字段,
音符 ID 使用规范十进制字符串, 权限区分 Read/Focus/Propose。

bridge 使用标准库有界队列: 容量 16、每帧最多一个请求、截止时间最长 30 秒。
取消、丢弃 caller、超时和 host 关闭均有显式错误; 已取消/过期的排队请求不会调用 host。
输入 256 KiB、结果 64 KiB、原文 UTF-8 16 KiB、单页 128、ID 过滤 256 个,
超限报错而非截断。可选 BackgroundRuntime 仅一个 worker, 显式关闭最多等待 1 秒;
P0/P1 不实例化它, 不打开端口、不读取凭据、不调用 provider。

锁定 rmcp 3.6.0、genai 0.7.0、schemars 1.2.2、Tokio 1.53.1。共享 crate 的
无 feature 与 all-features 已用真实 Rust 1.88.0 检查通过, all-features 也用 stable
1.96.0 检查通过; mcp、agent、mcp+agent、stdio-bridge 分别通过当前工具链检查。
这是 macOS 本机编译验证, 不代表已验证客户端协商、provider tools 或跨平台。

### P1: 编辑器只读工具

- [x] 接入主线程安全帧点, 按需抓取 canonical 文档/Simai/草稿状态。
- [x] 实现摘要、分页 note 查询、原文读取、结构检查和可选定位。
- [x] 未应用草稿/模板隔离/无核心时返回准确状态, 不误读另一份文档。
- [x] 测试大谱面、分页稳定性、取消、队列满和主线程处理耗时。

验收: fake caller 能查询用户实际打开的谱面, 不产生 dirty、history、save 或播放变化。

实现位于 editor/src/automation, 在 ChartSync 发布后、player tick 前处理请求。
快照只查询 authored 主谱 note, 原文读取保留 Unicode/CRLF 和未应用草稿, 不生成 Simai。
后续编辑不改变旧快照分页; 重载、源/难度切换、模板隔离变化使旧身份失效。
定位仅滚动并关闭跟随, 必须与 live chart/原文一致; 模板隔离与忙碌捕获返回错误。
结构检查涵盖重复 ID、数值/时值/区域、正 Hold 时长与 Slide 轨迹; 核心检查仍明确不支持。

快照最多 8 个、TTL 300 秒、单个估算 16 MiB/总估算 32 MiB。估算并非严格 heap 配额。
捕获/检查同步在主线程进行; 每帧一个请求不是耗时上限。过滤结果和诊断先收集再分页。
20,000 note 的本机 optimized-dev 测试: 捕获 3.446 ms、查询 0.287 ms、
结构检查 0.709 ms、1000 空闲帧 0.0368 ms; 这不是实际播放/UI 延迟保证。

隔离 worktree 排除其他 agent 的改动后, automation 11 项测试通过。共享 crate
Rust 1.88 测试: 无 feature 6 项、all-features 7 项通过。默认全部 bins 及 editor 的
editor-mcp、editor-agent、两者同时开启均 cargo check --locked 通过。
backend-none 的 editor 测试构建被既有 autoplay_editor/curve.rs 中 JudgeEngine::chart_spec
缺失阻挡, 未修改无关模块; P1 无实际窗口/播放端到端验收。

### P2: 提案到单步撤销闭环

- [x] 先实现明确 ID 的移动/旋转/属性更新, 再扩展添加/删除/完整 Slide。
- [x] 实现 candidate 预检、原文格式保留、差异、只由 UI 确认与生命周期。
- [x] 保存原文的事务历史, 协调 modal/text history 与选择状态。
- [x] 最终冲突检查、core worker 调度、自动保存授权。

验收: “将指定小节的选定音符旋转一格”能预览、确认、一次撤销/重做;
中途手动改谱/输入/切难度会拒绝过期提案, 原文不被重写。此阶段不需要模型。

验证: 共享 crate 7 项测试、editor automation 15 项测试通过。覆盖混合批量原子性、属性类型、幂等、会话隔离、冲突、自动保存授权、拒绝/取消/过期、快照淘汰后继续审核，以及真实 Simai 预检和一次 Undo/Redo 精确保留 CRLF、Unicode、注释与分数记法。编辑器审核窗口已接入；无真实 MCP client/provider smoke test。

### P3: MCP 外部入口

- [x] 实现 rmcp Streamable HTTP、loopback/token/Origin 防护与设置开关。
- [x] 实例发现、权限、断连取消、结构化结果及有限 Resources。
- [x] 独立 stdio 代理与日志隔离, 多实例指定。
- [x] 用真实 MCP client 跑初始化、list/call/read、错误和取消集成测试。

验收: 外部客户端可解释/检查谱面并提交提案, 但不能绕过编辑器确认修改或保存。

验证: stdio-bridge 的 11 项测试通过。真实 rmcp client 完成 HTTP 初始化、工具/资源发现、读取、结构化结果、isError、权限、取消与 stdio relay；覆盖 token/Host/Origin 拒绝、8 会话上限及私有发现记录权限/清理。编辑器设置已接入启停、实例信息与权限撤销。代理独立构建通过，依赖图不包含图形/Lean。私有发现记录目前仅实现 Unix 权限，Windows 明确拒绝启用；未做实际图形窗口端到端测试。

### P4: 轻量 Agent 与工作台分区

- [ ] 首先用 fake provider 驱动完整 tool loop/流式事件/取消与预算。
- [ ] 用 genai 适配经过验证的 provider, 正确关联完整 tool calls/results。
- [ ] Agent 分区、设置、提案与工具记录, 旧布局兼容。
- [ ] 分别验证 HTTP 直连 MCP 与不启用 MCP 的内置 Agent 走同一 host。

验收: 自然语言“检查这几小节并给出修改建议”能引用实际 note;
“把选中音符移动一拍”给出明确小节单位和差异, 等用户确认, Stop 后无迟到写入。

### P5: 扩展与打磨

- [ ] 第二 provider/本地模型适配, 明确 tools 不支持时的只读聊天降级。
- [ ] 精确 Simai 范围替换, 明确 Unicode 字符/行坐标与原文字段, 保留原文回归测试。
- [ ] 更多确定性检查、MCP Prompts/Resources、可选人工导出日志。
- [ ] 按实际需求增加 Autoplay/模板工具, 单独设计归属/权限/历史, 不放宽通用任意写入。

## 9. 重点测试矩阵

| 层级 | 必测 |
| --- | --- |
| 契约 | schema 与目录一致、未知字段、ID 精度、数值溢出、单位、范围/大小上限 |
| 读取 | 当前源/难度、原文与草稿区分、稳定分页、模板来源、主谱与展开谱区别 |
| 修改 | 无效 lane/Touch/Slide、零长度 Hold、多 BPM、越界、关联数据清理、整批原子性 |
| 格式与历史 | 注释/CRLF/Unicode、`[1:2]`/`[2:9]`、连续 Slide、单次 Undo/Redo 精确恢复原文 |
| 并发 | 手动编辑/撤销/切源/难度/模板、UI 事务期间拒绝、过期 worker 不提交、幂等 operation ID |
| 生命周期 | 断连、Stop、超时、队列满、未批准提案过期、应用中取消的真实结果、退出清理 |
| 保存 | auto_save 开关与确认、拒绝不落盘、来源变化、写入失败不破坏旧文件 |
| MCP | 初始化协商、工具/资源发现、输入输出 schema、structuredContent、isError、取消、stdio 纯净 |
| 安全 | 无 token/错误 token、恶意 Origin/Host、非 loopback、连接限额、跨实例/跨会话提案 |
| Agent | 无工具文本、单/多工具、工具结果 ID、半截 JSON、重复/未知调用、预算、usage 缺失 |
| UI | 文本输入不穿透谱面快捷键、紧凑布局、无重叠、连接/错误/确认状态、旧分屏重启恢复 |
| 构建 | feature-off/mcp-only/agent-only/both、独立 stdio bin、现有 bins、支持桌面目标 |

自动测试使用 fake provider 与真实本机 rmcp client, 不依赖付费模型或访问公网。
真实 provider 烟测显式手动运行, 不把 Key 放入 fixture。

本轮实施前记录的完整编辑器基线为 533 通过/23 失败/3 忽略。失败涉及既有
maichart 难度、Autoplay 时值、Slide handles、UI 快捷键和 Jack Ripper 保存格式。
该记录不表示完整套件通过, 也不是可永久豁免的失败清单。
大谱面性能先测 P1 捕获/比较/提交耗时及播放延迟; 不能以“用了 async”替代测量。

## 10. 决策摘要与下一步

确定采用: 双入口、共享 typed tools、主线程拥有状态、只读优先、UI 审批提案、
保留原文的单步撤销、显式自动保存语义、rmcp + genai 可选集成。

P0 已验证共享 crate、依赖精确版本/features/MSRV 与隔离。后续仍须验证
Streamable HTTP + stdio 代理的真实客户端行为及 provider 流式 tools。

下一步为 **P3 MCP 外部入口**。P2 已在无模型情况下通过，继续接 MCP 与 Agent,
避免把领域编辑错误和模型行为混在一起排查。后续实施必须同步更新本文状态与使用文档。
