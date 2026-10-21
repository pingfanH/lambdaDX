# 编辑器内置 Agent

内置 Agent 可以按需读取当前谱面、解释或检查音符, 并生成需要人工确认的修改提案。
提案审核沿用编辑器的差异窗口, 应用后可用一次 Undo 撤销。模型没有批准、保存、
打开文件或执行 Shell 的工具。实现状态和测试记录见 [MCP 与 Agent 计划](MCP_AGENT_PLAN.md)。

## 启动与分区

只使用内置 Agent 不需要启动 MCP 服务:

```sh
cargo run --locked --bin lambda_dx_editor --features editor-agent
```

需要同时使用外部 MCP 客户端时启用 `--features editor-agent,editor-mcp`。
MCP 监听与 Agent 分别开关, 默认都关闭。未启用 Agent feature 的构建仍保留布局中的
Agent 分区, 显示功能未启用, 不启动模型连接。

通过分区标题的类型菜单选择 Agent, 或使用“视图 > 布局”的添加分区入口。
每个编辑器实例只有一个会话。工作区切换、分区最大化与恢复不复制或重启会话;
关闭最后一个 Agent 分区会停止运行并撤销待审提案。

## 模型连接

打开设置（F1 或 Primary+,）, 展开“内置 Agent · 模型连接”:

1. 勾选“启用内置 Agent（本次编辑器会话）”。每次启动都需要重新启用。
2. 选择 Provider, 填写 Model、Base URL 和凭据**环境变量名**。此处不要填写密钥。
3. 按需允许定位视图、创建提案。只读查询默认可用; 提案权限不会授予自动应用权限。
4. 点击“确认 endpoint 与权限并保存配置”。连接或权限有未确认的更改时不能发送。

目前实际验收通过的组合:

| 配置项 | 值 |
| --- | --- |
| Provider | `OpenAICompatible` |
| Base URL | `https://api.commandcode.ai/provider/v1` |
| Model | `deepseek/deepseek-v4.1-flash` |
| 凭据环境变量名 | `LDX_AGENT_SECRET` |

模型名中的 `/` 是端点要求的实际拼写。OpenAI 与 Ollama 的配置入口已实现,
尚未对其真实服务/模型完成工具调用验收; 其他兼容端点也需要分别验证。

凭据必须在启动编辑器的进程环境中。编辑器**不会自动加载 `.env`**。如果已经在自己的
`.env` 中配置上述变量, 可以在终端加载后从同一终端启动:

```sh
set -a
source .env
set +a
cargo run --locked --bin lambda_dx_editor --features editor-agent
```

`LDX_AGENT_PROVIDER` 与 `LDX_AGENT_MODEL` 用于手动网络测试; 编辑器界面中的
Base URL 与 Model 仍需按设置填写。运行开始时只读取所填写的凭据变量,
不会扫描其他凭据。配置文件为应用缓存目录下的 `editor_agent.json`, 只保存模型、
endpoint、环境变量名与权限; 聊天正文、密钥、运行和提案不随配置或布局保存。

Base URL 接受 HTTPS 或本机 loopback HTTP, 不接受 URL 中的凭据、query 或 fragment。
网络请求禁用自动重定向与环境代理。发送按钮会向所选 endpoint 发起真实模型请求。

## 发送、审核和停止

先在谱面中选择音符, 在 Agent 分区输入任务, 再点击“发送到所选 endpoint”。
例如“检查选中音符所在小节, 给出修改建议”, 或“把选中音符向后移动一拍;
这里一拍指 0.25 小节”。消息会附带明确选中的 note ID, 不默认发送整首谱面或音频;
模型通过工具按需读取内容。分区显示流式文本、工具名/调用 ID/结果、运行状态和 usage。
provider 未报告完整 usage 时显示未知。

成功创建提案后进入 `AwaitingApproval`, 停止模型循环。检查精确 ID、位置和增删改差异,
再确认或拒绝。启用自动保存时, 审核还需要明确授权按原有自动保存规则写回来源。
确认前若谱面、草稿、来源或难度变化, host 会执行最终冲突检查并拒绝过期提案。
应用后的核心预览状态与保存状态仍由现有编辑器流程处理。

审核结果和应用后的快照会加入会话, 需要进一步解释时再次发送消息。审核不会自动
唤醒旧模型运行。点击 Stop、清空会话、关闭最后一个 Agent 分区或重载文档,
会取消网络/排队工具和未应用提案。Stop 不会撤回已经原子应用的修改, 此时使用 Undo。
文档重载会清空会话; 普通编辑通过快照冲突检查保护提案。

默认每次发送最多 8 轮模型调用、24 次工具调用、120 秒活动时间; 单次网络 60 秒、
单工具 30 秒。等待人工审核不继续请求模型。显式选区最多 256 个 ID, 会话上限
256 KiB; 超限时缩小范围或清空会话。流断开、参数不完整或 finish reason 不成功时
不执行工具, 不会盲目重试。认证、模型/工具不支持、限流和超时显示错误后,
修正配置或任务再显式发送。

## 验证范围

自动测试不需要 API Key。共享 crate 的 fake provider 与真实 TCP SSE fixture
覆盖流式多工具参数、结果 ID、错误、预算、取消和事件队列背压。编辑器测试使用真实
host 验证无 MCP 的 Agent、HTTP MCP 查询/提案、人工应用、单步撤销和断连取消。
布局测试覆盖旧配置与 Agent 分区恢复, 会话测试覆盖审核后不自动续跑和过期审核退出。

显式网络测试 `live_agent_smoke` 默认忽略。需要运行时, 在环境中提供
`LDX_AGENT_PROVIDER`（endpoint URL）、`LDX_AGENT_MODEL` 与 `LDX_AGENT_SECRET`, 然后执行:

```sh
cargo test --locked --bin lambda_dx_editor --features editor-agent live_agent_smoke -- --ignored --nocapture
```

它在内存谱面上请求真实模型读取两个指定 note, 生成各后移 `0.25` 小节的提案并验证差异,
停在人工审核前, 不保存文件。已对上表组合通过此测试。尚未完成实际图形窗口
端到端操作、跨平台与第二 provider 验收; 构建检查和基线限制详见计划中的 P4 记录。
