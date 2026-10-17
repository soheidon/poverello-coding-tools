# Provider Details & Model Behaviors

This document details supported model providers, reasoning parameters, capability translations, and local inference backends in Poverello.

---

## 1. Supported Providers Overview

### Cloud Providers (Claude Desktop, Claude Code Gateway Route, MCP)

| Provider | Endpoint | Native / OpenRouter | Reasoning Control |
|---|---|---|---|
| **Anthropic** | `https://api.anthropic.com` | Native | Adaptive thinking / Reasoning effort |
| **DeepSeek** | `https://api.deepseek.com/anthropic` | Native | Low / High / Max |
| **GLM (Z.ai)** | `https://api.z.ai/api/anthropic/v1/messages` | Native | GLM-5.3: Low / High / Max · GLM-5.2: Normal / Thinking |
| **MiniMax** | `https://api.minimax.io/anthropic` | Native | Model-specific |
| **Kimi / Moonshot** | `https://api.moonshot.cn/anthropic` | Native | Thinking / Reasoning effort |
| **MiMo / Xiaomi** | `https://api.xiaomimimo.com/anthropic` | Native | Thinking mode (`thinking` / `normal`) |
| **OpenRouter** | `https://openrouter.ai/api/v1` | Multi-profile Gateway | Vendor / Model specific |
| **Gateway Profiles** | Per-route endpoint | Multi-Provider Gateway | Per-route provider settings |

### Dedicated Local Inference Backend (Claude Code Dedicated Route)

| Provider | Endpoint | Target Harness | Key Features |
|---|---|---|---|
| **Ollama Local** | `http://127.0.0.1:11434/v1` | Claude Code CLI | Zero-cloud offline privacy, thinking budget override (1024), context window override (`num_ctx`), dynamic loopback model discovery via `/api/tags` |

> [!NOTE]
> **Isolation Notice**: Ollama Local is an independent local backend configured strictly for Claude Code CLI (`claude_code.active_route: "ollama"`). It does not affect Claude Desktop, Cowork on 3P, or Google Antigravity MCP (which continue using configured cloud providers). See [Ollama Local Guide](OLLAMA_LOCAL.md) for full details.

---

## 2. Direct Provider Details

### Anthropic

- **Endpoint**: `https://api.anthropic.com` (official Messages API)
- **Supported Models**: `claude-opus-5-5`, `claude-sonnet-5-5`, `claude-haiku-4-5`, `claude-3-7-sonnet`, `claude-3-5-sonnet`, `claude-3-5-haiku`.
- **Authentication**: `ANTHROPIC_API_KEY` (or Poverello GUI Settings).
- **Reasoning Control**: Native adaptive thinking and reasoning effort supported by official Anthropic models.
- **Diagnostics**: Streaming and non-streaming detection of `stop_reason: "max_tokens"` with reasoning but no generated text, returning descriptive provider errors.

### DeepSeek

- **Supported Models**: `deepseek-v4-pro` (V4-Pro-0813), `deepseek-flash` (DeepSeek V4.1 Flash), `deepseek-v4-flash` (V4-Flash-0731 legacy), `deepseek-v4-flash-vision-exp` (V4-Flash-Vision-Exp).
- **Reasoning Control**: DeepSeek models support reasoning effort levels (`low`, `high`, `max`).

### GLM / Z.ai

- **Endpoint**: `https://api.z.ai/api/anthropic/v1/messages` (official Anthropic Messages-compatible API)
- **Supported Models**:
  - `glm-5.3`: Flagship reasoning model with always-on thinking.
  - `glm-5.3-flash`: High-performance reasoning model with always-on thinking.
  - `glm-5.3-flashx`: Ultra-fast reasoning variant with always-on thinking.
  - `glm-5.2`: General coding model supporting both Normal and Thinking modes.
- **Reasoning Control**:
  - GLM-5.3 series: Always-on thinking (`thinking: {"type": "enabled"}`) with selectable effort levels (`low`, `high`, `max`).
  - GLM-5.2: Toggle between `thinking` and `normal` (`thinking: {"type": "disabled"}`).
- **Authentication & eligibility**: Uses the configured `ZAI_API_KEY` (or key entered in Poverello) for this endpoint. Poverello does not identify key type, Coding Plan purchase history, account allowlisting, or billing eligibility. Z.ai's current account and endpoint requirements may vary; consult [Z.ai's setup guidance](https://zcode.z.ai/en/docs/configuration).
- **Output Validation**: Empty text / thinking-only responses (e.g. `stop_reason: "max_tokens"` exhausted inside thinking) fail closed with explicit provider errors across Orchestrator and MCP server.

### MiniMax

- **Supported Models**: `MiniMax-Text-01`, `MiniMax-VL-01`.
- **Reasoning Control**: Uses model-specific parameters.

### Kimi / Moonshot

- **Supported Models**: `kimi-k1.5`, `kimi-k1.5-preview`.
- **Reasoning Control**: Supports thinking toggle and reasoning effort.

### MiMo / Xiaomi

- **Supported Models**:
  - `mimo-v2.6-flash`: High-performance cost-efficient model with 1,000,000-token context window.
  - `mimo-v2.6-pro`: Flagship model for complex coding and agent tasks with 1,000,000-token context window.
  - `mimo-v2.6-pro-ultraspeed`: Ultra-fast variant offering up to 20x output speed with 1,000,000-token context window.
- **Reasoning Control**: Normal / Thinking mode toggle (`thinking: {"type": "disabled"}` or `thinking: {"type": "enabled"}`).
- **Multimodal**: Native text, image, and video understanding across all V2.6 models.

### Multi-Provider Gateway Profiles

- **Concept**: A Gateway profile maps each of the three standard Claude routes (`claude-opus-5`, `claude-sonnet-5`, `claude-haiku-4-5`) independently to different providers and models.
- **Example Route Mix**:
  - `claude-opus-5`: Anthropic Direct `claude-opus-5-5` (Deep architectural reasoning).
  - `claude-sonnet-5`: DeepSeek `deepseek-v4.1-flash` (Fast, high-quality code generation).
  - `claude-haiku-4-5`: GLM `glm-5.3-flash` (Lightweight summaries, classification).
- **Authentication**: Requires API keys configured for each participating provider. Unused providers require no keys.
- **Claude Code Integration**: Auto-compaction automatically computes safe context windows from the minimum capacity among the selected models in the active profile.

---

## 3. Dedicated Local LLM Backend (Ollama Local)

- **Endpoint**: `http://127.0.0.1:11434/v1` (native Anthropic-compatible `/v1/messages` loopback)
- **Supported Models**: Any local model installed in Ollama (e.g., `gemma4:latest`, `qwen2.5-coder:32b`, `deepseek-r1:32b`, `llama3.3:70b`).
- **Model Discovery**: Discovers installed models via loopback `GET http://127.0.0.1:11434/api/tags` (2-second bounded timeout, manual refresh).
- **Per-Alias Routing**: Allows distinct model mappings for `opus`, `sonnet`, and `haiku`.
- **Reasoning & Context**:
  - **Thinking Mode**: `enabled` (injects budget 1024) or `disabled` (`budget_tokens: 0`).
  - **Context Window**: Injects `options.num_ctx` per alias or uses global default.
- **Guide**: See [Ollama Local Guide](OLLAMA_LOCAL.md) for full configuration reference.

---

## 4. OpenRouter Provider Profiles

OpenRouter acts as a multi-profile gateway supporting hundreds of upstream models:
- **Poolside**: Laguna S / XS models.
- **InclusionAI**: Ring and Ling model families (Thinking and reasoning controls).
- **StepFun**: Step 3.5 and Step 3.7 (Low / Medium / High reasoning effort).
- **OpenAI**: GPT-5.6 Sol / Terra / Luna and Pro variants (Thinking and reasoning controls).
- **Google**: Gemini 3.1 Pro Preview and Gemini 3.7 Flash.

---

## 5. Response Model Normalization

Upstream APIs often return their native model name in JSON responses. When enabled, Poverello rewrites the response model field back to the Anthropic route requested by the client (`claude-opus-5`, `claude-sonnet-5`, `claude-haiku-4-5`). This ensures seamless compatibility with Claude Desktop and Claude Code.
