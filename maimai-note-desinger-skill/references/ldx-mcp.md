# LambdaDX 工具工作流

内置 Agent 和外部 MCP 使用相同的工具 schema 与配置库。构建编辑器：

```sh
cargo run --locked --bin lambda_dx_editor --features editor-agent,editor-mcp
```

外部客户端先构建独立 stdio bridge：

```sh
cargo build --locked -p lambdadx-automation --features stdio-bridge --bin lambdadx_mcp
```

在编辑器设置中开启 MCP，复制实例 ID 并授权只读/提案权限。客户端的 MCP server command 指向构建后的 `lambdadx_mcp`，args 为 `["--instance", "编辑器实例ID"]`；bridge 从本机私有发现记录获取 endpoint 和认证信息。

外部客户端可读取资源 `lambdadx://reference/note-design-skill` 获得本技能入口，无需将整个目录装入系统提示。也可将本目录安装为客户端技能。

## 检索示例

```json
{"schema_version":1,"query":"交互","limit":5}
```

将它传给 `library_search`；读取命中详情：

```json
{"schema_version":1,"revision":"从搜索结果复制","id":"interaction","max_bytes":4096}
```

将它传给 `library_read`。读取参考谱时增加 `part`，其值为命中的 references[].id。翻页时复制 next_offset、file_revision；文本为 UTF-8，字节游标和谱面小节不是同一坐标。

## 谱面编辑

| 工具 | 用途 |
| --- | --- |
| chart_get_context | 获取状态、不可变快照、选中 ID 和有界 selected_notes；本轮已有新鲜 editor_context 时复用 |
| chart_query_notes | 读取范围内音符、精确 ID、时长和滑条结构，必要时翻页 |
| chart_read_simai | 从快照读取原文；这是 Unicode 字符索引，区别于 library_read 的字节游标 |
| chart_check | 分页结构诊断；不验证音乐性、两手手法或完整核心运行 |
| chart_propose_edit | 创建带快照约束的类型化提案，不应用或保存 |
| chart_proposal_status | 查询本会话提案状态 |
| editor_focus_range | 有独立权限时定位范围，不选择或移动音符 |

选区信息足够时直接提交 typed edits，不必重复查询或读取整首 Simai。selection_notes_truncated 表示详情不完整，补查使用同一 target，note_ids 每批最多 256 个，limit 为 1..128，翻页复制 next_offset。需要处理段落衔接、手法或大范围任务时自主扩读，保留完整推理与工具能力。

提案的 operations、音符结构和新增 ID 规则以当前工具 schema 为准，不从旧示例猜参数。改已有音符必须先拿精确 ID；新增、删除和移动共同组成一次完整编辑，避免提交诊断探针。

DOCUMENT_CHANGED 时重取快照并复核当前内容。UNAPPLIED_DRAFT 或不同步时检查当前草稿和来源，不自行覆盖用户文本。FORMAT_PRESERVATION_FAILED 表示该提案没有通过保真校验，不证明某处永远不能编辑。库的 DOCUMENT_CHANGED 则要求重建索引或重新搜索/读取，不涉及修改谱面。
