[English](THIRD_PARTY_INFERENCE.md) | [日本語](THIRD_PARTY_INFERENCE.ja.md) | [中文(简体)](THIRD_PARTY_INFERENCE.zh-CN.md) | [中文(繁體)](THIRD_PARTY_INFERENCE.zh-TW.md) | [한국어](THIRD_PARTY_INFERENCE.ko.md) | [Français](THIRD_PARTY_INFERENCE.fr.md) | [Deutsch](THIRD_PARTY_INFERENCE.de.md) | [Español](THIRD_PARTY_INFERENCE.es.md)

# 在 Claude Desktop / Cowork on 3P 中使用 Poverello

Poverello 可作为 Claude Desktop / Cowork on 3P 的本地
Anthropic 兼容网关使用。

Claude Desktop / Cowork on 3P 通过应用内配置窗口支持第三方推理。

官方文档：

- [https://claude.com/docs/cowork/3p/installation](https://claude.com/docs/cowork/3p/installation)
- [https://claude.com/docs/cowork/3p/configuration](https://claude.com/docs/cowork/3p/configuration)

## 1. 启动 Poverello

首先启动 Poverello，确保网关正在运行。

默认情况下，Poverello 监听：

```text
http://127.0.0.1:4000
```

在使用 Claude Desktop / Cowork on 3P 期间，请保持 Poverello 运行。

## 2. 在 Claude Desktop 中启用开发者模式

打开 Claude Desktop。

在 Windows 上，点击左上角的应用菜单。

然后选择：

```text
Help → Troubleshooting → Enable Developer Mode
```

启用开发者模式后，会出现一个新的 `Developer` 菜单。

## 3. 打开第三方推理设置

打开：

```text
Developer → Configure third-party inference
```

这将打开第三方推理配置窗口。

## 4. 配置 Connection

在 `Connection` 部分中，选择：

```text
Gateway
```

然后输入以下值。

| 字段                    | 值                        |
| --------------------- | ------------------------ |
| Gateway base URL      | `http://127.0.0.1:4000` |
| Gateway API key       | `sk-local-gateway`       |
| Gateway auth scheme   | `bearer`                 |
| Gateway extra headers | 通常留空                      |

`Gateway API key` 必须与 Poverello 中配置的本地 API 密钥一致。

## 5. 配置 Identity & Models

在 `Identity & Models` 部分中，添加要在模型选择器中显示的模型 ID。

示例：

```text
claude-sonnet-4-6
claude-haiku-4-5
```

您还可以为每个模型设置显示名称。

示例：

| Model ID            | 显示名称          |
| ------------------- | ------------- |
| `claude-sonnet-4-6` | `Gateway Pro` |
| `claude-haiku-4-5`  | `Gateway Flash` |

列表中的第一个模型将作为默认选择项。

展开每个模型行，确认 `Model ID` 与您希望发送到 Poverello 的模型名称完全一致。

仅当上游提供商和所选模型确实支持扩展上下文窗口时，才启用 `Offer 1M-context variant`。

## 6. 配置示例

以上设置对应以下第三方推理配置：

```json
{
  "inferenceProvider": "gateway",
  "inferenceGatewayBaseUrl": "http://127.0.0.1:4000",
  "inferenceGatewayApiKey": "sk-local-gateway",
  "inferenceGatewayAuthScheme": "bearer",
  "inferenceModels": [
    {
      "name": "claude-sonnet-4-6",
      "labelOverride": "Gateway Pro"
    },
    {
      "name": "claude-haiku-4-5",
      "labelOverride": "Gateway Flash"
    }
  ]
}
```

## 7. 应用并重启 Claude Desktop

配置好网关和模型列表后，在本地应用设置。

如有提示，请重启 Claude Desktop。

重启后，Cowork on 3P 的请求将发送到 Poverello。Poverello 随后将请求路由到您在 Poverello 中配置的上游提供商。

## Direct DeepSeek API

将 Poverello 配置为 Direct DeepSeek 提供商后，Claude Desktop / Cowork on 3P 的请求会路由到 DeepSeek 的 Anthropic Messages 兼容端点。

DeepSeek 规范 API 模型 ID 为 `deepseek-flash` 和 `deepseek-v4-pro`；当前官方价格页面分别将其列为 DeepSeek-V4.1-Flash-1017 和 DeepSeek-V4-Pro-0813。这些是上游 API ID。`inferenceModels` 中面向 Claude 的名称只是本地模型选择项，由 Poverello 映射到所选提供商配置。

两款模型都支持工具调用、1M token 上下文和最高 384K 输出 token。只有 Flash 支持图像输入。DeepSeek 仍将已退役的 `deepseek-v4-flash` 和 `deepseek-v4-flash-vision-exp` ID 作为 Flash 的上游别名接受。Poverello 内置的 Direct DeepSeek 默认配置使用规范 ID。官方资料对 Pro 路由和计费的说明不一致：当前价格页面列出单独的 Pro 费率，并说明 API 服务在 2026 年 9 月 14 日后继续提供；而 9 月 10 日公告曾称 V4.1-Pro 发布前会临时路由至 V4.1 Flash 并按 Flash 费率计费。Poverello 无法核实每个请求的路由或账单；显示的是公开费率，并非账单或保证。请参阅[当前价格页面](https://api-docs.deepseek.com/quick_start/pricing/)和[9 月 10 日公告](https://api-docs.deepseek.com/news/news260910/)。

当前 token 单价及高峰／非高峰规则请参阅 [DeepSeek 官方价格表](https://api-docs.deepseek.com/quick_start/pricing/)和[提供商详情指南](PROVIDERS.md#deepseek)。Poverello 随附的节假日日历会影响本地高峰／非高峰估算；覆盖状态和回退行为也见提供商指南。

## Xiaomi MiMo 提供商

当 Poverello 配置为使用 Direct MiMo 提供商时，来自 Claude Desktop 的推理请求将被路由到小米的 MiMo API。

### MiMo-V2.6 模型

| Model ID | 显示名称 | 多模态 |
| --- | --- | --- |
| `mimo-v2.6-flash` | MiMo-V2.6-Flash | Image + Video |
| `mimo-v2.6-pro` | MiMo-V2.6-Pro | Image + Video |
| `mimo-v2.6-pro-ultraspeed` | MiMo-V2.6-Pro-UltraSpeed | Image + Video |

所有 V2.6 模型支持 1,000,000 token 上下文窗口。

### 普通 / 思考

MiMo 使用简单的普通／思考切换开关，没有推理努力级别（Low / Medium / High）。

| 设置 | 上游 API 参数 | 行为 |
| --- | --- | --- |
| 普通 | `thinking: {"type": "disabled"}` | 标准推理 |
| 思考 | `thinking: {"type": "enabled"}` | 扩展思考 |

### 默认路由

Direct MiMo 预设的内置路由：

| 网关模型 | 上游模型 | 思考 |
| --- | --- | --- |
| `claude-opus-5` | `mimo-v2.6-pro` | 思考 |
| `claude-sonnet-5` | `mimo-v2.6-pro` | 普通 |
| `claude-haiku-4-5` | `mimo-v2.6-flash` | 思考 |

### V2.5 向后兼容性

已保存的 `mimo-v2.5`、`mimo-v2.5-pro` 和 `mimo-v2.5-pro-ultraspeed` 配置将被保留并继续有效。启动时仅自动迁移未经修改的出厂默认路由。自定义路由、思考模式设置以及自定义 `default_model` 值不会被覆盖。

## MiniMax-M3 思考模式

MiniMax-M3 支持通过 Anthropic 兼容 API 开关思考（扩展思考）功能。

| 设置 | 上游 API 参数 | 行为 |
| ---- | ------------ | ---- |
| 思考开启 | `thinking: {"type": "adaptive"}` | 启用扩展思考 |
| 思考关闭 | `thinking: {"type": "disabled"}` | 禁用扩展思考 |
| 默认（未设置） | *（省略）* | API 默认值：思考关闭 |

在 Poverello 设置界面中，MiniMax-M3 行会显示「思考／通常」切换开关。

MiniMax-M2.x 系列模型（`MiniMax-M2.7-highspeed`）不支持关闭思考功能，固定为「仅思考」模式。

> [!NOTE]
> **Claude Desktop / Cowork on 3P 通过网关使用云端提供商**
>
> Claude Desktop 和 Cowork on 3P 将推理请求发送至 Poverello 网关（`http://127.0.0.1:4000`），并路由到您配置的上游云端提供商（DeepSeek、MiMo、Kimi、MiniMax、OpenRouter）。
>
> **Ollama Local** 后端是 **Claude Code CLI 专用** 的，与 Claude Desktop 和 MCP 完全隔离。有关 Claude Code 的本地 LLM 配置，请参阅 [Ollama Local 指南](OLLAMA_LOCAL.zh-CN.md)。

---

## Laguna S/XS 2.1（OpenRouter）配置

默认情况下，Poverello 将 OpenRouter 的 `claude-opus-5` 和 `claude-sonnet-5` 路由到 `poolside/laguna-s-2.1`，将 `claude-haiku-4-5` 路由到 `poolside/laguna-xs-2.1`。

### 思考行为

第三方测试观察到，系统提示词的结构可能影响 Laguna S 2.1 的思考行为。有报告称，加入明确的职业角色和清晰的验收标准可显著减少不必要的推理。基于这些测试结果，Laguna Opus（`claude-opus-5`）的默认模式已改为普通（关闭思考）；可在设置中重新启用 Thinking。

### 已知限制：达到 token 上限时没有可用输出

在某些情况下，尤其是模型在推理阶段达到单轮 token 上限时，Laguna S 2.1 可能只返回推理内容，没有文本输出或工具调用，并将 `stop_reason` 设为 `max_tokens`。Poverello 会检测此情况并在日志面板记录警告。如果反复出现“Reasoning-only response reached the per-turn token limit”，可考虑提高最大输出 token 数或关闭该模型的 Thinking。

这些观察来自第三方社区测试，结果可能因提供商配置和模型版本而异。

## 注意事项

Poverello 是一个非官方的 Anthropic 兼容本地网关。

与 Anthropic、Moon Bridge 或任何上游模型提供商无关联。

Claude Desktop / Cowork on 3P 通过 Claude 的第三方推理设置进行配置。菜单标签和配置字段可能随 Anthropic 更新 Claude Desktop 而发生变化。
