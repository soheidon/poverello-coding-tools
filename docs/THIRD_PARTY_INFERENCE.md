[English](THIRD_PARTY_INFERENCE.md) | [日本語](THIRD_PARTY_INFERENCE.ja.md) | [中文(简体)](THIRD_PARTY_INFERENCE.zh-CN.md) | [中文(繁體)](THIRD_PARTY_INFERENCE.zh-TW.md) | [한국어](THIRD_PARTY_INFERENCE.ko.md) | [Français](THIRD_PARTY_INFERENCE.fr.md) | [Deutsch](THIRD_PARTY_INFERENCE.de.md) | [Español](THIRD_PARTY_INFERENCE.es.md)

[← Back to Poverello README](../README.md)

# Using Poverello with Claude Desktop / Cowork on 3P

Poverello can be used as a local Anthropic-compatible gateway for
Claude Desktop / Cowork on 3P.

Claude Desktop / Cowork on 3P supports third-party inference through the
in-app configuration window.

Official documentation:

- [https://claude.com/docs/cowork/3p/installation](https://claude.com/docs/cowork/3p/installation)
- [https://claude.com/docs/cowork/3p/configuration](https://claude.com/docs/cowork/3p/configuration)

## 1. Start Poverello

Start Poverello first and make sure the gateway is running.

By default, Poverello listens on:

```text
http://127.0.0.1:4000
```

Keep Poverello running while using Claude Desktop / Cowork on 3P.

## 2. Enable Developer Mode in Claude Desktop

Open Claude Desktop.

On Windows, open the application menu in the upper-left corner.

Then select:

```text
Help → Troubleshooting → Enable Developer Mode
```

After Developer Mode is enabled, a new `Developer` menu will appear.

## 3. Open third-party inference settings

Open:

```text
Developer → Configure third-party inference
```

This opens the third-party inference configuration window.

## 4. Configure Connection

In the `Connection` section, select:

```text
Gateway
```

Then enter the following values.

| Field                 | Value                                      |
| --------------------- | ------------------------------------------ |
| Gateway base URL      | `http://127.0.0.1:4000`                    |
| Gateway API key       | `sk-local-gateway`                         |
| Gateway auth scheme   | `bearer`                                   |
| Gateway extra headers | Leave blank unless you need custom headers |

The `Gateway API key` must match the local API key configured in Poverello.

## 5. Configure Identity & Models

In the `Identity & Models` section, add the model IDs that Claude Desktop should show in the model picker.

Example:

```text
claude-opus-5
claude-sonnet-5
claude-haiku-4-5
```

You can also give each model a display label.

Example:

| Model ID            | Display label   |
| ------------------- | --------------- |
| `claude-opus-5`  | `Gateway Opus`  |
| `claude-sonnet-5`  | `Gateway Pro`   |
| `claude-haiku-4-5` | `Gateway Flash` |

The first model in the list is used as the default picker entry.

For each model, expand the row and confirm that `Model ID` is exactly the model name you want Claude Desktop to send to Poverello.

Only enable `Offer 1M-context variant` if your upstream provider and selected model actually support the extended context window.

## 6. Example configuration

The above settings correspond to the following third-party inference configuration:

```json
{
  "inferenceProvider": "gateway",
  "inferenceGatewayBaseUrl": "http://127.0.0.1:4000",
  "inferenceGatewayApiKey": "sk-local-gateway",
  "inferenceGatewayAuthScheme": "bearer",
  "inferenceModels": [
    {
      "name": "claude-opus-5",
      "labelOverride": "Gateway Opus"
    },
    {
      "name": "claude-sonnet-5",
      "labelOverride": "Gateway Pro"
    },
    {
      "name": "claude-haiku-4-5",
      "labelOverride": "Gateway Flash"
    }
  ]
}
```

## 7. Apply and restart Claude Desktop

After configuring the gateway and model list, apply the settings locally.

Restart Claude Desktop if prompted.

Once Claude Desktop restarts, requests from Cowork on 3P will be sent to Poverello, which routes them to the upstream provider and reasoning settings configured in the Poverello GUI.

For specific provider thinking parameters and model capabilities, see: [Provider Details & Model Behaviors](PROVIDERS.md).

## Direct DeepSeek API

When Poverello is configured with the Direct DeepSeek provider, requests from Claude Desktop / Cowork on 3P are routed through the Anthropic Messages-compatible DeepSeek endpoint.

The canonical DeepSeek API IDs are `deepseek-flash` and `deepseek-v4-pro`; the current official pricing page lists them as DeepSeek-V4.1-Flash-1017 and DeepSeek-V4-Pro-0813, respectively. These are upstream API IDs; the Claude-facing model names in `inferenceModels` are local picker entries and are mapped by Poverello to the selected provider profile.

Both models support tool calls and a 1M-token context window, with up to 384K output tokens. Flash supports image input; Pro does not. DeepSeek's retired `deepseek-v4-flash` and `deepseek-v4-flash-vision-exp` IDs remain upstream aliases to Flash. Poverello's built-in Direct DeepSeek defaults use the canonical IDs. Official sources differ on Pro routing and billing: the current pricing page lists separate Pro rates and continued API service after September 14, 2026, while the September 10 announcement described temporary Flash routing and Flash rates until V4.1-Pro launches. Poverello cannot verify individual request routing or charges; its displayed Pro prices reflect the published schedule, not an invoice or guarantee. See [current pricing](https://api-docs.deepseek.com/quick_start/pricing/) and the [September 10 announcement](https://api-docs.deepseek.com/news/news260910/).

For current per-token pricing and peak/off-peak rules, see [DeepSeek's official pricing](https://api-docs.deepseek.com/quick_start/pricing/) and the [Provider Details guide](PROVIDERS.md#deepseek). Poverello's bundled holiday calendar affects the local peak/off-peak estimate; the coverage status and fallback behavior are described in the Provider Details guide.

## Xiaomi MiMo Provider

When Poverello is configured to use the Direct MiMo provider, inference requests from Claude Desktop are routed to Xiaomi's MiMo API.

### MiMo-V2.6 models

| Model ID | Display name | Multimodal |
| --- | --- | --- |
| `mimo-v2.6-flash` | MiMo-V2.6-Flash | Image + Video |
| `mimo-v2.6-pro` | MiMo-V2.6-Pro | Image + Video |
| `mimo-v2.6-pro-ultraspeed` | MiMo-V2.6-Pro-UltraSpeed | Image + Video |

All V2.6 models support a 1,000,000-token context window.

### Normal / Thinking

MiMo uses a simple Normal/Thinking toggle. There are no reasoning effort levels (Low/Medium/High).

| Setting | Upstream parameter | Behavior |
| --- | --- | --- |
| Normal | `thinking: {"type": "disabled"}` | Standard inference |
| Thinking | `thinking: {"type": "enabled"}` | Extended thinking |

### Default routes

The built-in Direct MiMo preset routes:

| Gateway model | Upstream model | Thinking |
| --- | --- | --- |
| `claude-opus-5` | `mimo-v2.6-pro` | Thinking |
| `claude-sonnet-5` | `mimo-v2.6-pro` | Normal |
| `claude-haiku-4-5` | `mimo-v2.6-flash` | Thinking |

### V2.5 backward compatibility

Saved `mimo-v2.5`, `mimo-v2.5-pro`, and `mimo-v2.5-pro-ultraspeed` configurations are preserved and continue to work. Only untouched factory-default routes are automatically migrated to V2.6 on startup. Customized routes, thinking mode settings, and custom `default_model` values are never overwritten.

---

> [!NOTE]
> **Claude Desktop / Cowork on 3P uses Cloud Providers via the Gateway**
>
> Claude Desktop and Cowork on 3P send inference requests to the Poverello Gateway (`http://127.0.0.1:4000`), which routes to your configured upstream cloud provider (DeepSeek, MiMo, Kimi, MiniMax, OpenRouter).
>
> The **Ollama Local** backend is strictly dedicated to **Claude Code CLI** and is completely isolated from Claude Desktop and MCP. For local LLM setup with Claude Code, please see the [Ollama Local Guide](OLLAMA_LOCAL.md).

---

## MiniMax-M3 Thinking Mode

MiniMax-M3 supports turning extended thinking on or off through the Anthropic-compatible API.

| Setting | Upstream API parameter | Behavior |
| --- | --- | --- |
| Thinking ON | `thinking: {"type": "adaptive"}` | Enables extended thinking |
| Thinking OFF | `thinking: {"type": "disabled"}` | Disables extended thinking |
| Default (unset) | *omitted* | API default: thinking disabled |

Poverello displays a Thinking/Normal toggle on the MiniMax-M3 row in Settings. MiniMax-M2.x models (such as `MiniMax-M2.7-highspeed`) do not support disabling thinking and remain fixed to Thinking-only mode.

## Laguna S/XS 2.1 (OpenRouter) Configuration

By default, Poverello routes OpenRouter `claude-opus-5` and `claude-sonnet-5` to `poolside/laguna-s-2.1`, and `claude-haiku-4-5` to `poolside/laguna-xs-2.1`.

### Thinking behavior

Third-party tests have observed that Laguna S 2.1 thinking behavior can be affected by system-prompt structure. Reports indicate that including a clear professional persona and explicit acceptance criteria substantially reduced unnecessary reasoning. Based on these results, the default for Laguna Opus (`claude-opus-5`) was changed to Normal (thinking off); Thinking can be enabled again in Settings.

### Known limitation: no usable output at the token limit

Under certain conditions—especially when the model reaches its per-turn token limit during reasoning—Laguna S 2.1 may return reasoning content only, with no text output or tool call and `stop_reason` set to `max_tokens`. The client may then be unable to continue the conversation. Poverello detects this and logs a warning. If “Reasoning-only response reached the per-turn token limit” appears repeatedly, consider increasing the maximum output tokens or turning thinking off for that model.

These observations are based on third-party community tests. Results may vary by provider configuration and model revision.

## Notes

Poverello is an unofficial Anthropic-compatible local gateway.

It is not affiliated with Anthropic, Moon Bridge, or any upstream model provider.

Claude Desktop / Cowork on 3P is configured through Claude's third-party inference settings. Menu labels and configuration fields may change as Anthropic updates Claude Desktop.
