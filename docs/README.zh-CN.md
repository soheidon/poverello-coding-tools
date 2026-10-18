[English](../README.md) | [日本語](README.ja.md) | [中文(简体)](README.zh-CN.md) | [中文(繁體)](README.zh-TW.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Español](README.es.md)

# Poverello Coding Tools

**以 Claude Code / Claude Desktop 作为编程框架，将推理请求路由至第三方 LLM API，并将外部模型用作 Google Antigravity 的规划器与审查器。**

Poverello Coding Tools（应用内显示名称为 **Poverello**）是一款面向 AI 辅助软件开发的 Windows 配套应用程序，支持五种互补的工作流：

1. **面向 Claude Code / Claude Desktop 的第三方推理网关** — 保留 Claude 的代码仓库探索、工具调用、文件编辑和测试执行能力，同时将推理请求路由至第三方提供商。
2. **Claude Code 专用本地 LLM（Ollama）** — 使用本地模型离线运行 Claude Code。
3. **面向 Google Antigravity 的 MCP 规划器与审查器** — 通过 `poverello/plan`、`poverello/revise` 和 `poverello/review` MCP 工具，将实现规划、计划修订和实现后审查委托给外部模型。
4. **多智能体开发 Orchestrator** — 执行当前支持的有界 Planner–Reviewer 工作流，并记录标准化 token 用量和基于已核验供应商费率的成本估算。
5. **Antigravity 规则管理** — 为 Antigravity 配置、预览并安全同步 Poverello Direct Coding 规则（`pov-direct.md`）。

> **最新动态：** 请参阅 [NEWS.md（发行说明）](../NEWS.md)，了解各版本的功能、实现细节和变更记录。

---

## 五大主要工作流

### 1. Claude Code / Claude Desktop 配合第三方推理网关

```text
Claude Code / Claude Desktop
             ↓
  Poverello 3P Gateway
             ↓
Anthropic Direct / DeepSeek / Kimi Code / OpenRouter / MiniMax / MiMo / GLM
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
- **Auto-Converge 说明**：独立的完整 Auto-Converge 工作流在 v0.24.6 中已隐藏且不可用。本文所述当前支持的 Orchestrator 工作流与其不同；Auto-Converge 已延期至后续版本。

为保持兼容性，可执行文件和 MCP 工具标识仍使用 `anthro-bridge` 名称。

Antigravity 的规范命令为 `/pov-plan`、`/pov-revise` 和 `/pov-review`；旧的 `/anthro-*` 命令仍为兼容性保留。

**Antigravity 全局命令：**

- **`/pov-plan`** — 将实现规划委托给已配置的外部模型。
- **`/pov-revise`** — 根据新的反馈或约束条件修订现有计划。
- **`/pov-review`** — 在提交前，针对已批准的计划审查已完成的实现，并给出明确的 READY / NOT READY 结论。

**推荐工作流：**

```text
/pov-plan → 实现与测试 → /pov-review → 提交
```

---

### 4. 多智能体开发 Orchestrator（有界工作流与成本）

Orchestrator 使用可配置的角色配置和迭代上限执行当前支持的 Planner–Reviewer 工作流。独立的完整 Auto-Converge 工作流在 v0.24.6 中不可用。创建新的主计划始终需要用户明确确认。`%APPDATA%\Poverello\runs\` 中的运行日志会记录可用的标准化用量、审计元数据和基于已核验费率的成本估算。估算不等同于供应商账单；不支持的路由不会计算成本。

### 5. Antigravity 规则管理（Poverello Direct Coding）

可在 GUI 中配置、预览并同步 `pov-direct.md`。应用会保留用户自定义的 frontmatter 和注释，并提供经过验证的应用、备份及回滚保护。

## 支持的提供商

| 提供商 | 连接方式 | 支持的系列 | 推理强度控制 |
|---|---|---|---|
| **Anthropic** | 直连 API | Claude Opus 5.5、Sonnet 5.5、Haiku 4.5、Sonnet 3.7 | Adaptive thinking / 推理强度 |
| **DeepSeek** | 直连 API | `deepseek-flash`（V4.1 Flash 1017）、`deepseek-v4-pro`（V4 Pro 0813） | Normal / Thinking · Low / High / Max |
| **GLM (Z.ai)** | 直连 API | GLM-5.3、GLM-5.3-Flash、GLM-5.3-FlashX、GLM-5.2 | GLM-5.3: Low / High / Max · GLM-5.2: Normal / Thinking |
| **Kimi Code** | 直连 API | kimi-for-coding、kimi-for-coding-highspeed | 思考模式 |
| **MiniMax** | 直连 API | MiniMax M3、M2.7 | 模型专属 |
| **Kimi / Moonshot** | 直连 API | Kimi K2.x、Kimi K3 | Thinking / 推理强度 |
| **MiMo / Xiaomi** | 直连 API | MiMo V2.6 Flash、Pro、Pro-UltraSpeed（兼容 V2.5） | Normal / Thinking |
| **Ollama Local** | 本地回环 | Gemma 4、Qwen 2.5、Llama 3.3、DeepSeek-R1 | Thinking（1024 预算）/ Disabled |
| **OpenRouter** | 多配置文件网关 | 请参见下方 OpenRouter 章节 | 模型专属 / 配置文件专属 |
| **多提供商网关** | 自定义路由 | 任意符合条件的提供商组合 | 按路由配置 |

### Anthropic（直连）

通过 `https://api.anthropic.com` 和 `ANTHROPIC_API_KEY` 直接使用官方 API。支持 `claude-opus-5-5`、`claude-sonnet-5-5`、`claude-haiku-4-5`、`claude-3-7-sonnet`、`claude-3-5-sonnet` 和 `claude-3-5-haiku`，并支持上游模型提供的 adaptive thinking 与推理强度控制。

### GLM / Z.ai（直连）

通过 Z.ai 官方 Anthropic Messages 兼容 API（`https://api.z.ai/api/anthropic/v1/messages`）和 `ZAI_API_KEY` 直连。GLM-5.3 系列可选择推理强度（`low`、`high`、`max`）；GLM-5.2 提供 Normal / Thinking 切换。

### DeepSeek（直连）

内置 **Direct DeepSeek** 预设路由：Opus 5 → V4.1 Flash / Max · Sonnet 5 → V4.1 Flash / High · Haiku 4.5 → V4.1 Flash / Low。

- `deepseek-flash` — DeepSeek-V4.1-Flash-1017 的规范稳定 API ID；上下文窗口 1M，最多输出 384K token。
- `deepseek-v4-pro` — API ID；当前官方价格页面将其列为 DeepSeek-V4-Pro-0813。上下文窗口 1M，最多输出 384K token。
- Flash 支持视觉输入，Pro 不支持。DeepSeek 仍接受旧 ID `deepseek-v4-flash` 和 `deepseek-v4-flash-vision-exp`，并将其作为 Flash 别名按 Flash 价格计费。

### Kimi Code（直连）

专用编程专家 API（`KIMI_CODE_API_KEY`），与 Moonshot Kimi 独立分开：

- `kimi-for-coding` — 完整质量的编程模型。
- `kimi-for-coding-highspeed` — 低延迟变体。

### MiMo / Xiaomi（直连）

内置 **Direct MiMo** 预设路由：
- Opus 5 → `mimo-v2.6-pro` / Thinking
- Sonnet 5 → `mimo-v2.6-pro` / Normal
- Haiku 4.5 → `mimo-v2.6-flash` / Thinking
- 默认模型：`mimo-v2.6-flash`

模型：`mimo-v2.6-flash`、`mimo-v2.6-pro`、`mimo-v2.6-pro-ultraspeed`（可选）。三者均支持 100 万 token 上下文窗口及原生多模态能力（文本、图像、视频）。

**Normal / Thinking**：MiMo 采用简单的 Normal/Thinking 切换 —— 不支持推理强度级别。

**V2.5 向下兼容**：已保存的 `mimo-v2.5`、`mimo-v2.5-pro`、`mimo-v2.5-pro-ultraspeed` 路由将继续正常运行。未修改的旧版默认配置将在启动时自动迁移至 V2.6。

### OpenRouter

支持多个命名配置文件。单一下拉列表中的当前 OpenAI 模型目录（15 个模型）：

| 模型 ID | 显示名称 |
|---|---|
| `openai/gpt-6.1-sol` | GPT-6.1 Sol |
| `openai/gpt-6.1-sol-pro` | GPT-6.1 Sol Pro |
| `openai/gpt-6-sol` | GPT-6 Sol |
| `openai/gpt-6-sol-pro` | GPT-6 Sol Pro |
| `openai/gpt-6-luna` | GPT-6 Luna |
| `openai/gpt-6-luna-pro` | GPT-6 Luna Pro |
| `openai/gpt-6-astra` | GPT-6 Astra |
| `openai/gpt-6-astra-pro` | GPT-6 Astra Pro |
| `openai/gpt-astra-latest` | GPT Astra Latest |
| `openai/gpt-5.6-sol` | GPT-5.6 Sol |
| `openai/gpt-5.6-sol-pro` | GPT-5.6 Sol Pro |
| `openai/gpt-5.6-terra` | GPT-5.6 Terra |
| `openai/gpt-5.6-terra-pro` | GPT-5.6 Terra Pro |
| `openai/gpt-5.6-luna` | GPT-5.6 Luna |
| `openai/gpt-5.6-luna-pro` | GPT-5.6 Luna Pro |

**GPT-6 Astra**：105 万上下文 · 推理强度：`low / medium / high / xhigh / max`。
**GPT-6 Astra Pro**：105 万上下文 · 始终开启 Pro 推理（`reasoning.mode = pro`），不支持用户自选强度。
**GPT Astra Latest**：追踪 Astra 系列最新模型的别名。

内置 **OpenRouter: chatGPT** 预设：Opus 5 → GPT-6 Astra / max · Sonnet 5 → GPT-6 Astra / high · Haiku 4.5 → GPT-6 Astra / medium。

同时支持：**OpenRouter: Gemini**（Gemini 3.8 Flash · 推理强度 `low / medium / high`）、**OpenRouter: Poolside**、**OpenRouter: Tencent**、**OpenRouter: InclusionAI**、**OpenRouter: StepFun**。

---

## DeepSeek API 价格（核验于 2026-10-18）

| 模型 ID | 峰时输入（缓存未命中 / 命中） | 非峰时输入（缓存未命中 / 命中） | 峰时输出 | 非峰时输出 |
|---|---:|---:|---:|---:|
| `deepseek-flash` | \$0.30（\$0.006） | \$0.15（\$0.003） | \$1.20 | \$0.60 |
| `deepseek-v4-pro` | \$1.32（\$0.044） | \$0.66（\$0.022） | \$3.96 | \$1.98 |

DeepSeek 价格单位均为每百万 token。工作日 UTC 01:00–04:00、06:00–10:00 为峰时（中国法定节假日除外）；周末及节假日全天按非峰时计价。Poverello 随附经核验的 2026 年中国节假日和调休工作日，并会提示次年日历缺失。其他提供商的价格是应用内置参考值，可能变化。Orchestrator 成本估算目前仅覆盖符合条件且缓存用量明确为零的 Anthropic Direct 调用，不估算 DeepSeek 费用。对于 Pro，当前官方价格页面列出单独费率，并说明 API 服务在 2026 年 9 月 14 日后继续提供；但 9 月 10 日公告曾称在 V4.1-Pro 发布前会临时路由至 V4.1 Flash 并按 Flash 费率计费。Poverello 无法核实每个请求的路由或账单；显示的是公开费率，并非账单或保证。请参阅[当前价格页面](https://api-docs.deepseek.com/quick_start/pricing/)和[9 月 10 日公告](https://api-docs.deepseek.com/news/news260910/)。

---

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
2. 选择 **MCP** 标签页，并在 **Settings > Antigravity > MCP Plan Settings** 中配置模型。
3. 在 Antigravity 的 MCP 配置中注册 `anthro-bridge.exe`，并附加参数 `["--mcp-server"]`（或在 Poverello 中点击 **Configure Automatically**）。
4. 使用 `/pov-plan` 制定计划，使用 `/pov-revise` 更新计划，使用 `/pov-review` 在提交前审查实现。
5. 请参阅完整的 [Antigravity MCP 配置指南](ANTIGRAVITY_MCP.zh-CN.md)。

### 工作流 4：多智能体开发 Orchestrator

1. 打开顶部导航栏中的 **Orchestrator** 标签页。
2. 选择或自定义 **Planner** 和 **Reviewer** 角色配置。
3. 设置所选受支持 Orchestrator 工作流的任务目标和迭代上限。
4. 查看实时进度、审计结论、成本估算状态，以及 `%APPDATA%\Poverello\runs\` 中的运行日志。

### 工作流 5：Antigravity 规则管理

1. 打开顶部导航栏中的 **Antigravity Rules** 标签页。
2. 选择规则模式并检查预览。
3. 点击 **Apply Settings**，将 `pov-direct.md` 安全地应用到 Antigravity。
4. 自定义注释、frontmatter 字段和备份会得到保留。

---

## API 密钥

| 提供商 | 环境变量 |
|---|---|
| Anthropic | `ANTHROPIC_API_KEY` |
| DeepSeek | `DEEPSEEK_API_KEY` |
| GLM / Z.ai | `ZAI_API_KEY` |
| Kimi Code | `KIMI_CODE_API_KEY` |
| Kimi / Moonshot | `MOONSHOT_API_KEY` |
| MiniMax | `MINIMAX_API_KEY` |
| MiMo / Xiaomi | `XIAOMI_API_KEY` |
| OpenRouter | `OPENROUTER_API_KEY` |

---

## 文档

- [发行说明 (NEWS.md)](../NEWS.md) — 各版本功能与实现细节
- [Claude Desktop / Cowork 第三方推理网关配置](THIRD_PARTY_INFERENCE.md)
- [Google Antigravity + Poverello MCP 配置](ANTIGRAVITY_MCP.zh-CN.md)
- [配置参考（`config.json`）](CONFIGURATION.md)
- [提供商详情与推理强度控制](PROVIDERS.md)
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
