[English](../README.md) | [日本語](README.ja.md) | [中文(简体)](README.zh-CN.md) | [中文(繁體)](README.zh-TW.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Español](README.es.md)

# Poverello Coding Tools

**使用 Claude Code / Claude Desktop 作为开发环境，将推理请求路由至已配置的云端或本地模型，并管理 Google Antigravity 的外部模型与规则。**

Poverello Coding Tools（应用内显示名称为 **Poverello**）是一款面向 AI 辅助软件开发的 Windows 配套应用程序，支持四种互补的工作流：

1. **面向 Claude Code / Claude Desktop 的第三方推理网关** — 保留 Claude 的代码仓库探索、工具调用、文件编辑和测试执行能力，同时将推理请求路由至第三方提供商。
2. **Claude Code 专用本地 LLM（Ollama）** — 使用本地模型离线运行 Claude Code，零 API 成本。
3. **面向 Google Antigravity 的 MCP 规划器与审查器** — 通过 `poverello/plan`、`poverello/revise` 和 `poverello/review` MCP 工具，将实现规划、计划修订和实现后审查委托给外部模型。
4. **Antigravity Autocode 与 Skills & Rules** — 在 Antigravity Autocode 标签页配置外部审查和编码行为；在 Settings 中管理工作流命令、CLI Tools、Task Presets、Global Rules 和 Nerf Skills。

> [!TIP]
> **最新动态：** 请参阅 [NEWS.md（发行说明）](../NEWS.md)，了解各版本的功能、实现细节和变更记录。

---

## 四大主要工作流

### 1. Claude Code / Claude Desktop 配合第三方推理网关

```text
Claude Code / Claude Desktop
             ↓
  Poverello 3P Gateway
             ↓
已配置的云端或本地模型
```

- **框架与模型分离**：保留 Claude 的代理工具能力，同时将推理请求路由至第三方提供商。
- **动态多配置文件路由**：通过图形界面动态切换活跃提供商、OpenRouter 配置文件和模型路由。
- **配置指南**：[Claude Desktop / Cowork 第三方推理网关配置](THIRD_PARTY_INFERENCE.md)

### 2. Claude Code 专用本地 LLM（Ollama）

```text
Claude Code CLI
      ↓ (Loopback ANTHROPIC_BASE_URL)
Ollama Local (127.0.0.1:11434/v1)
      ↓
Gemma 4 / Qwen / Llama 3 / DeepSeek-R1（本地设备端）
```

- **完全免费、离线与隐私**: 无需 API 密钥，无用量限制，无云端数据传输。
- **独立路由与模型发现**: 独立的 Claude Code 路由（`active_route: "ollama"`），支持 `/api/tags` 本地模型自动发现。
- **思考模式与上下文微调**: 支持为每个别名配置思考预算（1024 标记）与上下文窗口（`num_ctx`）。
- **完全隔离**: Claude Desktop、Cowork on 3P 和 Google Antigravity MCP 继续独立使用云端模型。
- **配置指南**: [Ollama Local 指南](OLLAMA_LOCAL.zh-CN.md)

### 3. Antigravity + MCP 规划器与审查器

```text
Antigravity
    ↓ stdio
anthro-bridge.exe --mcp-server
    ↓
已配置的外部模型（规划器 / 审查器）
    ↓
实现计划 / 审查结论
    ↓
Antigravity 使用订阅容量
执行实现与测试
```

- **规划与执行分离**：外部模型负责生成高层次计划或审查结论；Antigravity 订阅容量负责执行耗费大量 token 的代码编辑任务。
- **实时图形界面配置**：切换规划器或审查器的提供商、模型或推理强度，下次调用时立即生效。
- **配置指南**：[Google Antigravity + Poverello MCP 配置](ANTIGRAVITY_MCP.zh-CN.md)

为保持兼容性，可执行文件和 MCP 工具标识仍使用 `anthro-bridge` 名称。

Antigravity 的规范命令为 `/pov-mcp-plan`、`/pov-mcp-revise` 和 `/pov-mcp-review`；旧的 `/anthro-*` 命令仍为兼容性保留。

**Antigravity 全局命令：**

- **`/pov-mcp-plan`** — 将实现规划委托给已配置的外部模型。
- **`/pov-mcp-revise`** — 根据新的反馈或约束条件修订现有计划。
- **`/pov-mcp-review`** — 在提交前，针对已批准的计划审查已完成的实现，并给出明确的 READY / NOT READY 结论。

自主工作流：`/pov-full-cycle`、`/pov-plan-only`、`/pov-build-review`（在 **设置 > Antigravity Skills & Rules** 中安装和修复；在 **Antigravity Autocode** 中设置 Reviewer Policy）。

**推荐工作流：**

```text
/pov-mcp-plan → 实现与测试 → /pov-mcp-review → 提交
```

### 4. Antigravity Autocode 与 Skills & Rules 管理
```text
Antigravity Autocode tab
  • External Review policy / reviewer configuration
  • Customize Coding Settings / Direct Coding

Settings > Antigravity Skills & Rules
  • Workflow Commands / repair
  • CLI Tools / Task Presets
  • Global Rules / Nerf skills
```

- External Review 和 Customize Coding Settings 在 Antigravity Autocode 中配置。Settings 管理三个 Workflow Skill、CLI Tools、Task Presets、Global Rules 和 Nerf Skills。
## 安装

从 [Releases](https://github.com/soheidon/poverello-coding-tools/releases) 页面下载当前最新已发布的 Windows 安装程序并运行。

Windows 安装程序支持 8 种语言。Poverello 保留现有应用标识，并支持从 Anthro Bridge 原位升级。首次启动时，程序设计为将 `%APPDATA%\Anthro Bridge`（开发版为 `%APPDATA%\Anthro Bridge Dev`）中的设置和运行数据复制到对应的 Poverello 数据目录，同时保留原目录作为恢复副本。真实 Windows 环境中的安装、数据迁移、升级、重新安装、取消和卸载流程尚未验证；升级前请备份重要数据。

---

## 快速开始

### 工作流 1：面向 Claude Code / Claude Desktop 的第三方推理网关

1. 打开 Poverello **Settings > API Key**，为所需提供商配置 API 密钥。
2. 在仪表盘上选择提供商或 OpenRouter 配置文件。
3. 点击 **Start Gateway**（运行于 `http://127.0.0.1:4000`）。
4. 连接 Claude Code 或 Claude Desktop：
   - **Claude Code**：在 Settings 中点击 **Copy Claude Code launch command**，将命令粘贴到 PowerShell 中执行。
   - **Claude Desktop / Cowork**：请参照 [Claude Desktop 第三方推理配置指南](THIRD_PARTY_INFERENCE.md)。

### 工作流 2：Claude Code 专用本地 LLM（Ollama）

1. 确认 Ollama 正在本地运行（`http://127.0.0.1:11434`）。
2. 在 Poverello 仪表盘选择 **Ollama Local**，或在 **Settings > Claude Code / Local LLMs** 中配置模型。
3. 点击 **Copy Claude Code launch command** 并粘贴到 PowerShell。
4. Claude Code 将直接连接到本地 Ollama 实例。

### 工作流 3：面向 Google Antigravity 的 MCP 规划器与审查器

1. 在 Poverello 中为所选规划器/审查器模型配置 API 密钥。
2. 选择 **MCP** 选项卡（或 **设置 > Antigravity MCP**），并在 **设置 > Antigravity MCP > MCP Plan 详细设置** 中配置模型。
3. 在 Antigravity 的 MCP 配置中注册 `anthro-bridge.exe`，并附加参数 `["--mcp-server"]`，服务键为 `poverello`（或在 Poverello 中点击 **自动配置**）。
4. 使用 `/pov-mcp-plan` 制定计划，使用 `/pov-mcp-revise` 更新计划，使用 `/pov-mcp-review` 在提交前审查实现。
4. 请参阅完整的 [Antigravity MCP 配置指南](ANTIGRAVITY_MCP.zh-CN.md)。

### 工作流 4：Antigravity Autocode 与 Skills & Rules
1. 在 **Antigravity Autocode** 中配置 External Review 和 Customize Coding Settings。所选策略控制工作流 Skill 是否调用审查器。
2. 在 **Settings > Antigravity Skills & Rules** 中管理 Workflow Commands、CLI Tools、Task Presets、Global Rules 和 Nerf Skills。修复 Skill 需确认并保留备份。
3. Auto-Converge 仍隐藏，生产 dispatch 禁用，Stage 2 隔离未验证。
## 文档

- [发行说明 (NEWS.md)](../NEWS.md) — 各版本功能与实现细节
- [Claude Desktop / Cowork 第三方推理网关配置](THIRD_PARTY_INFERENCE.md)
- [Google Antigravity + Poverello MCP 配置](ANTIGRAVITY_MCP.zh-CN.md)
- [配置参考（`config.json`）](CONFIGURATION.md)
- [开发与验证指南](DEVELOPMENT.md)

---

## 故障排查

### 端口 4000 已被占用
```powershell
netstat -ano | findstr :4000
taskkill /PID <PID> /F
```

### 升级后设置被还原
重新启动应用程序以执行迁移。配置文件存储于 `%APPDATA%\Poverello\config.json`。

### MCP 规划器调用失败
请确保已为 **MCP** 标签页中所选提供商设置 API 密钥，或已在 Windows 用户环境变量中导出相应密钥（例如 `DEEPSEEK_API_KEY`、`OPENROUTER_API_KEY`）。MCP 功能无需运行第三方推理网关。

---

## 许可证

MIT 许可证。详见 [LICENSE](../LICENSE)。
