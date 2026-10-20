# MCP 与轻量 Agent 依赖研究

设计研究日期: 2026-10-20; Cargo 探针更新: 2026-10-21。
本文前半记录原设计的一手资料; 第 6 节记录实际发布版与本机编译结果。
网络源码来自两个项目的 main 分支，因此版本会继续变化；真正实施时应锁定发布版本、
git revision 和 Cargo.lock，再重新验证。

## 1. 读取基线

| 项目 | 读取 revision | 仓库声明版本 | Rust / 许可证 | 入口 |
| --- | --- | --- | --- | --- |
| modelcontextprotocol/rust-sdk / rmcp | [74d7895](https://github.com/modelcontextprotocol/rust-sdk/tree/74d789515fdee63ef46a1635b801364aad6695e4) | 3.6.0 | edition 2024, rust-version = 1.88, Apache-2.0 | [Cargo.toml](https://github.com/modelcontextprotocol/rust-sdk/blob/74d789515fdee63ef46a1635b801364aad6695e4/crates/rmcp/Cargo.toml) |
| jeremychone/rust-genai / genai | [06eca21](https://github.com/jeremychone/rust-genai/tree/06eca2124811a3384f1c5e4d30e0a27c3a0b5fcb) | 0.7.1-WIP | edition 2024, MIT OR Apache-2.0 | [Cargo.toml](https://github.com/jeremychone/rust-genai/blob/06eca2124811a3384f1c5e4d30e0a27c3a0b5fcb/Cargo.toml) |

本项目当前使用 Rust 1.98.0-nightly，但这不是新依赖的 MSRV 证明。实施前必须至少
用项目声明的最低 Rust 版本或明确记录无法支持的原因。

## 2. rmcp 事实

### 2.1 依赖和 feature

rmcp 的 workspace 3.6.0 声明 rust-version = 1.88、edition 2024、Apache-2.0。
核心依赖包括 Tokio、serde、serde_json、futures、thiserror、tracing、tokio-util
和 indexmap。

相关 feature 以该 revision 的 [crates/rmcp/Cargo.toml](https://github.com/modelcontextprotocol/rust-sdk/blob/74d789515fdee63ef46a1635b801364aad6695e4/crates/rmcp/Cargo.toml)
为准：

| Feature | 作用 | 计划用途 |
| --- | --- | --- |
| server | server 能力和 tool 系统，并带 async-rw、schemars、宏辅助依赖 | 编辑器内置 MCP server |
| client | client 侧基础能力 | stdio 代理或测试 client |
| transport-io | server-side stdio | 只有真正需要 stdio server 时开启 |
| transport-child-process | client-side stdio，拉起子进程 | 可选通用 MCP client，不是首选代理路径 |
| transport-streamable-http-server | Streamable HTTP server、session 和 worker | 编辑器 loopback MCP server |
| transport-streamable-http-client-reqwest | Streamable HTTP client + reqwest | stdio 代理连接编辑器 |
| schemars | JSON Schema 生成 | typed tool 参数/输出 |
| macros | tool/prompt 宏 | 可用，但共享目录仍是唯一 schema 来源 |
| reqwest / reqwest-native-tls / reqwest-tls-no-provider | HTTP 客户端 TLS 选择 | client 代理按需选择一个 |

rmcp 默认 feature 当前是 base64、macros、server。HTTP/TLS 组合、根工作区 feature
统一和平台支持必须用探针验证；不要把该表当作最终依赖集合。

### 2.2 Server、client 和传输

官方 README 列出两组内置配对：server-side stdio 与 TokioChildProcess client，
以及 Streamable HTTP client 与 StreamableHttpService server。官方 server 示例使用
StreamableHttpService、LocalSessionManager、axum nest_service 和 CancellationToken。

来源: [counter_streamhttp.rs](https://github.com/modelcontextprotocol/rust-sdk/blob/74d789515fdee63ef46a1635b801364aad6695e4/examples/servers/src/counter_streamhttp.rs)
和 [rmcp README](https://github.com/modelcontextprotocol/rust-sdk/blob/74d789515fdee63ef46a1635b801364aad6695e4/crates/rmcp/README.md)。

官方 client 示例使用 StreamableHttpClientTransport::from_uri，再用
ClientConfig::serve_with_lifecycle 选择 ClientLifecycleMode::Auto，在 Discover 和旧
Initialize 生命周期间协商。

来源: [streamable_http.rs](https://github.com/modelcontextprotocol/rust-sdk/blob/74d789515fdee63ef46a1635b801364aad6695e4/examples/clients/src/streamable_http.rs)。

SDK 提供传输和 session 管理，但不提供 LambdaDX 的状态桥。loopback 绑定、Bearer
token、Origin/Host 检查、实例发现、连接数限制和有界主线程队列仍是本项目责任。

### 2.3 Tool、resource、prompt、结果和错误

官方 SDK 使用 tool_router、tool 和 tool_handler 宏；serde Deserialize 加 schemars
JsonSchema 的参数字段、类型和文档可用于生成 inputSchema。工具可返回
CallToolResult，内容可以是文本、图片、音频或嵌入资源，并支持 structuredContent
和 outputSchema。

来源: [README Tools](https://github.com/modelcontextprotocol/rust-sdk/blob/74d789515fdee63ef46a1635b801364aad6695e4/README.md#tools)。

错误有两层：工具已运行但业务失败时返回 CallToolResult::error；协议无法路由、参数
无法处理或 session 无效时返回 McpError。LambdaDX 的 DOCUMENT_CHANGED、
VALIDATION_FAILED、UNAPPLIED_DRAFT 属于工具级错误；未知字段、无法反序列化和未知
方法才映射协议错误。不能用成功文本包裹失败。

resource 需要 ServerHandler 的 list_resources/read_resource；prompt 需要 prompt
router 或显式处理。它们不会自动从共享工具目录生成。首版只读谱面片段先走 tool，
静态 Simai 说明再考虑 resource。

### 2.4 协议、生命周期和取消

该 revision 的 README 描述稳定协议为 2026-07-28，并兼容 2025-11-25 及更早版本。
协议版本和 SDK revision 不是一回事，必须对目标客户端做协商集成测试。server 示例
使用 CancellationToken graceful shutdown，client 示例使用 client.cancel()。

SDK 的取消能力不等于本项目的提案语义：取消未开始的提案应阻止入队；已经开始的
主线程原子提交不能伪装成已撤回；断连不能让写请求在稍后无人确认时继续执行。

### 2.5 rmcp 不负责的部分

它不负责文档快照、Simai 原文保留、差异预览、人工审批、撤销载荷、主线程调度、
模型调用循环、权限策略、秘密存储或哪个实例可连接。这些属于共享 typed host 和
editor adapter；MCP 层只做协议转换。

## 3. genai 事实

### 3.1 Chat 与工具调用

genai 的 ChatRequest 是 provider-neutral 请求，包含 system、Vec<ChatMessage>、
可选 Vec<Tool>、previous_response_id 和 store。工具定义可用 JSON Schema 配置。
官方 streaming 示例使用 ChatOptions::with_capture_tool_calls(true)。

来源: [chat_request.rs](https://github.com/jeremychone/rust-genai/blob/06eca2124811a3384f1c5e4d30e0a27c3a0b5fcb/src/chat/chat_request.rs)
和 [c21-tooluse-streaming.rs](https://github.com/jeremychone/rust-genai/blob/06eca2124811a3384f1c5e4d30e0a27c3a0b5fcb/examples/c21-tooluse-streaming.rs)。

ToolCall 的关键字段是 call_id: String、fn_name: String、fn_arguments:
serde_json::Value，也可能带 provider-specific thought_signatures。ToolResponse
至少需要原始 call ID 和字符串化内容，也可以附带 function name。

来源: [tool_call.rs](https://github.com/jeremychone/rust-genai/blob/06eca2124811a3384f1c5e4d30e0a27c3a0b5fcb/src/chat/tool/tool_call.rs)
和 [tool_response.rs](https://github.com/jeremychone/rust-genai/blob/06eca2124811a3384f1c5e4d30e0a27c3a0b5fcb/src/chat/tool/tool_response.rs)。

Agent runner 必须保存每个 call ID 和原始参数，不能用 tool name 作为关联键，也不应
把 Value 先转成不稳定字符串再猜字段。调用 host 前要执行 schema、未知字段、权限
和大小验证；模型返回的 JSON 是不受信任输入。

### 3.2 流式 tool call

ChatStreamEvent 至少包含 Start、文本 Chunk、reasoning/signature chunk、ToolCallChunk、
End(StreamEnd) 和 Heartbeat。开启 capture 后，StreamEnd 可保存合并的 captured_content，
并提供 captured_tool_calls()、captured_into_tool_calls() 和续写工具使用消息的辅助。

来源: [chat_stream.rs](https://github.com/jeremychone/rust-genai/blob/06eca2124811a3384f1c5e4d30e0a27c3a0b5fcb/src/chat/chat_stream.rs)
和 [ChatRequest::append_tool_use_from_stream_end](https://github.com/jeremychone/rust-genai/blob/06eca2124811a3384f1c5e4d30e0a27c3a0b5fcb/src/chat/chat_request.rs)。

实现规则：

1. 可实时显示文本，但不能执行半截参数。
2. 以完整 stream end 或明确聚合并成功解析 JSON 的 call 为执行边界。
3. 同轮多个 call 按 call ID 保持顺序，逐个生成对应 ToolResponse。
4. 下一轮同时保留完整 assistant tool-use 消息和所有 tool response。
5. provider reasoning/thought signature 保留顺序，不能统一丢弃。
6. stream 出错、取消或只有半截参数时，不能假设工具已经执行，也不要盲目重试。

官方示例明确展示：stream 结束后提取多个 ToolCall，为每个调用构造 ToolResponse，
再把 assistant tool calls 和 responses 追加到下一轮。不同 provider 的 signature、
function name 和 response 格式可能不同，需 fake fixture 加至少一次真实 provider 烟测。

### 3.3 Feature、TLS 和取消边界

genai 当前默认 feature 是 rustls-tls 和 system-proxy。native-tls 与 rustls-tls 互斥；
源码明确要求使用 native TLS 时设 default-features = false 再启用 native-tls，否则
触发 compile error。关闭两者则允许注入自定义 reqwest client。

genai 是异步 chat/tool transport，不是 LambdaDX Agent 状态机。它没有提案审批、工具
权限、预算、快照、主线程队列或 UI。Stop 应由上层使用取消 token 或 select 停止等待并
丢弃 stream，同时从 bridge 撤销未开始的工具调用；实现前要测试 drop 后的连接释放。

### 3.4 genai 不负责的部分

它不提供 MCP 协议、LambdaDX 权限裁剪、人工确认、多轮预算、幂等 operation ID、修改
差异、结构检查、编辑器 UI 或会话持久化；它只是 provider-neutral 的消息和 transport 层。

## 4. 推荐组合与待验证

P0 可以先尝试以下最小组合，但不是直接合入的配置：

    rmcp = { version = 3.6, default-features = false, features = [server, macros, schemars, transport-streamable-http-server] }
    genai = { version = 0.7, default-features = false, features = [rustls-tls] }

genai 的仓库版本仍是 WIP，rmcp 的 HTTP server 会带入 Tokio/Axum 相关依赖，根工作区
已有平台和 reqwest feature 可能统一。实施前分别验证 feature-off、MCP-only、stdio
client、agent-only、both、现有 editor bin 和最低 Rust 版本。

必须完成的探针：

- 锁定 rmcp 发布版或 revision，测试目标客户端的协议版本协商。
- 确认 genai 0.7 发布 API 是否与 main 相同，并验证至少一个工具调用 provider。
- 用 fake transport 验证 inputSchema、outputSchema、structured content、isError、
  session、取消和断连。
- 用 fake provider 验证多个 tool call、参数聚合、call ID、thought signature、错误、
  drop/select 取消和重复 operation ID。
- 验证 StreamableHttpService 的 session manager 与 LambdaDX logical session 的关系。
- 测量快照、预检、队列和核心更新延迟，不能只凭 async 宣称非阻塞。

## 5. 结论

rmcp 适合作为 MCP protocol adapter，genai 适合作为 provider-neutral chat/tool
transport。两者都不应成为 LambdaDX 领域层或编辑器状态拥有者。推荐顺序仍是：
feature-off 的共享 typed contract 与 fake host，接着只读查询，再做提案/确认/撤销闭环，
最后接 loopback MCP 和内置 Agent。

## Sources

- https://github.com/modelcontextprotocol/rust-sdk/tree/74d789515fdee63ef46a1635b801364aad6695e4
- https://github.com/jeremychone/rust-genai/tree/06eca2124811a3384f1c5e4d30e0a27c3a0b5fcb
- https://modelcontextprotocol.io/specification/2026-07-28

## 6. P0 发布版与实测记录 (2026-10-21)

实际 manifest: `crates/lambdadx-automation/Cargo.toml`; 解析结果提交到根 Cargo.lock。
以锁定发布版为准, 不把 main 的 0.7.1-WIP 当作 genai 发布版。

| 依赖 | 精确版本 | 启用条件 / features |
| --- | --- | --- |
| schemars | 1.2.2 | 共享 DTO/schema 始终使用 |
| rmcp | 3.6.0 | mcp; default-features=false, server/schemars/transport-streamable-http-server |
| genai | 0.7.0 | agent; default-features=false, rustls-tls; 不启用 system-proxy |
| Tokio | 1.53.1 | mcp 或 agent; rt-multi-thread/time/sync/net |

stdio-bridge 额外启用 rmcp 的 client、transport-io、
transport-streamable-http-client-reqwest、reqwest; 尚无可执行代理。
共享 crate 不依赖 ChartDoc、GPU、编辑器或 Lean。mcp/agent runtime 与 editor host
明确只支持桌面; 默认关闭, 原有移动/wasm 构建不启用新功能。

实际运行并通过:

- Rust 1.88.0: `cargo +1.88.0 check -p lambdadx-automation --locked`。
- Rust 1.88.0: 同上加 `--all-features`, 包括实际 transitive dependencies。
- Stable 1.96.0: `cargo +stable check -p lambdadx-automation --all-features`。
- 项目 1.98.0-nightly: mcp、agent、mcp+agent、stdio-bridge 分别 cargo check。
- cargo tree 确认未启用 editor-automation 时根包不包含共享 crate/Tokio/rmcp/genai;
  共享 crate 无 feature 时仅依赖 serde、serde_json、schemars。

这些结果验证版本/features/MSRV 与依赖隔离, 没有验证真实服务/API 行为。
P3 仍须做 HTTP 鉴权、Origin/Host、真实 rmcp client、断连/取消和 stdio 集成测试。
P4 仍须核对 0.7.0 chat/tool API、fake provider 循环与真实 provider 烟测。
尚未发送模型请求, 也没有宣称 provider 兼容性。
