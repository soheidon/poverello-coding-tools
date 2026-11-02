# Configuration Reference

This document describes the structure and settings available in Poverello's configuration (`config.json`).

Configuration files are stored under:
- **Stable**: `%APPDATA%\Poverello\config.json`
- **Development**: `%APPDATA%\Poverello Dev\config.json`

---

## 1. Top-Level Structure

```json
{
  "active_provider": "deepseek",
  "active_openrouter_profile": "openrouter_default",
  "active_gateway_profile_id": "profile_default",
  "gateway_profiles": [
    {
      "id": "profile_default",
      "name": "Custom Multi-Provider",
      "routes": {
        "claude-opus-5": { "provider": "anthropic", "model": "claude-opus-5-5" },
        "claude-sonnet-5": { "provider": "deepseek", "model": "deepseek-flash" },
        "claude-haiku-4-5": { "provider": "zai", "model": "glm-5.3-flash" }
      }
    }
  ],
  "providers": {
    "anthropic": { ... },
    "deepseek": { ... },
    "minimax": { ... },
    "kimi": { ... },
    "mimo": { ... },
    "zai": { ... },
    "openrouter": { ... }
  },
  "claude_code": {
    "active_route": "gateway",
    "third_party_provider": {
      "provider": "ollama",
      "api_base_url": "http://127.0.0.1:11434/v1",
      "default_model": "gemma4:latest",
      "thinking_mode": "enabled",
      "context_window_tokens": 131072,
      "show_on_dashboard": true,
      "models": {
        "opus": "gemma4:latest",
        "sonnet": "gemma4:latest",
        "haiku": "gemma4:latest"
      },
      "thinking_modes": {
        "opus": "enabled",
        "sonnet": "enabled",
        "haiku": "enabled"
      },
      "context_windows": {
        "opus": 131072,
        "sonnet": 131072,
        "haiku": 131072
      }
    },
    "auto_compact": {
      "enabled": true,
      "mode": "auto",
      "trigger_percent": 90
    }
  },
  "mcp": {
    "provider": "deepseek",
    "profile_id": "",
    "model": "deepseek-v4-pro",
    "thinking_mode": "thinking",
    "reasoning_effort": "high",
    "targets": { ... }
  },
  "timezone": "Asia/Tokyo",
  "normalize_response_model": true,
  "non_vision_image_policy": "fallback_to_text"
}
```

---

## 2. Provider and Route Configuration

Each provider configuration maps Anthropic route names (`claude-opus-5`, `claude-sonnet-5`, `claude-haiku-4-5`) to provider-specific upstream models and reasoning parameters.

### Canonical Routes

- `opus`: Mapped to heavy reasoning / high-intelligence upstream models.
- `sonnet`: Mapped to balanced general-purpose coding models.
- `haiku`: Mapped to fast, lightweight auxiliary models.

### Reasoning and Thinking Fields

- `thinking`: Boolean flag or provider-specific parameter.
- `thinking_mode`: Used by providers like MiMo (`thinking`, `normal`).
- `reasoning_effort`: Controls effort levels (`low`, `medium`, `high`, `xhigh`, `max`).

---

## 3. OpenRouter Profiles

OpenRouter supports multiple profiles, each with its own independent configuration:

```json
{
  "profiles": [
    {
      "id": "openrouter_default",
      "display_name": "GPT-5.6 Balanced",
      "models": {
        "opus": "openai/gpt-5.6-sol",
        "sonnet": "openai/gpt-5.6-terra",
        "haiku": "openai/gpt-5.6-luna"
      },
      "thinking_modes": {
        "opus": "thinking",
        "sonnet": "thinking",
        "haiku": "thinking"
      },
      "reasoning_efforts": {
        "opus": "high",
        "sonnet": "high",
        "haiku": "high"
      },
      "hidden": false
    }
  ]
}
```

---

## 4. Multi-Provider Gateway Profiles

When `active_provider` is set to `"gateway_profile"`, the proxy resolves model routes using the profile referenced by `active_gateway_profile_id`:

```json
{
  "active_provider": "gateway_profile",
  "active_gateway_profile_id": "profile_default",
  "gateway_profiles": [
    {
      "id": "profile_default",
      "name": "Custom Multi-Provider",
      "routes": {
        "claude-opus-5": {
          "provider": "anthropic",
          "model": "claude-opus-5-5",
          "thinking_mode": "thinking",
          "reasoning_effort": "high"
        },
        "claude-sonnet-5": {
          "provider": "deepseek",
          "model": "deepseek-flash",
          "thinking_mode": "thinking",
          "reasoning_effort": "high"
        },
        "claude-haiku-4-5": {
          "provider": "zai",
          "model": "glm-5.3-flash"
        }
      }
    }
  ]
}
```

- Every gateway profile must define all three canonical routes: `claude-opus-5`, `claude-sonnet-5`, and `claude-haiku-4-5`.
- Routes may independently point to any reviewed provider (`anthropic`, `deepseek`, `zai`, `minimax`, `kimi`, `mimo`, `openrouter`).
- OpenRouter routes require an additional `"profile_id"` specifying the target OpenRouter profile.

---

## 5. Claude Code Context Management

Poverello can manage context capacity to trigger proactive compaction in Claude Code.

### Modes

1. **Auto (`mode: "auto"`)**: Automatically detects the minimum context window among the three canonical routes and applies an override percentage.
```json
{
  "claude_code": {
    "auto_compact": {
      "enabled": true,
      "mode": "auto",
      "trigger_percent": 90
    }
  }
}
```

2. **Manual (`mode: "manual"`)**: Explicit token window and trigger percentage.
```json
{
  "claude_code": {
    "auto_compact": {
      "enabled": true,
      "mode": "manual",
      "window_tokens": 240000,
      "trigger_percent": 90
    }
  }
}
```

3. **Claude Default (`mode: "claude_default"`)**: Disables context variable injection and leaves compaction to Claude Code.

---

## 6. Claude Code Routing & Local LLM (Ollama Local)

Claude Code CLI routing is governed independently from global provider settings.

### Route Selector (`claude_code.active_route`)

- `"gateway"`: Claude Code connects to Poverello Gateway (`http://127.0.0.1:4000`), routing to the global `active_provider`.
- `"ollama"`: Claude Code connects directly to the local Ollama loopback endpoint (`http://127.0.0.1:11434/v1`).

### Third-Party Provider Configuration (`claude_code.third_party_provider`)

| Field | Type | Default | Description |
|---|---|---|---|
| `provider` | string | `"ollama"` | Local provider identifier. |
| `api_base_url` | string | `"http://127.0.0.1:11434/v1"` | Loopback base URL for Ollama. |
| `default_model` | string | `"gemma4:latest"` | Default model ID for unmapped routes. |
| `thinking_mode` | string | `"enabled"` | Global thinking mode (`"enabled"` = budget 1024, `"disabled"` = budget 0). |
| `context_window_tokens` | integer | `131072` | Context window size passed via `options.num_ctx`. |
| `show_on_dashboard` | boolean | `true` | Controls visibility of the Ollama tile on the Dashboard. |
| `models` | object | `{ "opus": "...", "sonnet": "...", "haiku": "..." }` | Per-alias model mappings. |
| `thinking_modes` | object | `{ "opus": "...", ... }` | Per-alias thinking mode overrides. |
| `context_windows` | object | `{ "opus": 131072, ... }` | Per-alias context window overrides. |

### Backward Compatibility & Migration

- Legacy Plan 32 configurations containing `third_party_provider.enabled = true` and no `active_route` are automatically promoted to `active_route = "ollama"` at runtime and upon configuration save.
- If `enabled = false` or missing, it is promoted to `active_route = "gateway"`.
- `show_on_dashboard` defaults to `true` if not specified.
- Ollama is **never** added to global `providers` or `active_provider`.

---

## 7. MCP Planning Configuration

The `mcp` section configures the model used when external agents call `poverello/plan`:

```json
{
  "mcp": {
    "provider": "deepseek",
    "profile_id": "",
    "model": "deepseek-v4-pro",
    "thinking_mode": "thinking",
    "reasoning_effort": "high",
    "targets": {
      "deepseek": {
        "model": "deepseek-v4-pro",
        "thinking_mode": "thinking",
        "reasoning_effort": "high"
      },
      "openrouter:gemini": {
        "model": "google/gemini-3.7-flash",
        "thinking_mode": "normal",
        "reasoning_effort": "high"
      }
    }
  }
}
```

- Target configurations are stored per-provider/profile in `targets`.
- Switching the active target in the GUI automatically mirrors its values to top-level fields for backwards compatibility.

---

## 8. Orchestrator Journal & Cost Tracking

The Orchestrator persists workflow runs to `%APPDATA%\Poverello\runs\<run_id>.json`.

### Run Cost Summary Schema

For dispatch records with available usage and supported pricing provenance, the journal summary aggregates the resulting estimates. The current bundled estimate applies only to qualifying Anthropic Direct Opus 5.5 / Sonnet 5.5 Planner–Reviewer convergence dispatches with explicit zero cache buckets; other providers and execution paths remain unpriced/unavailable. Example serialized journal fragment (field names use the actual camelCase wire format):

```json
{
  "costSummary": {
    "currency": "USD",
    "totalCost": {
      "currency": "USD",
      "amountNanos": "26000000"
    },
    "status": "complete",
    "pricedDispatches": 2,
    "unpricedDispatches": 0,
    "schedules": [
      "anthropic:claude-opus-5-5:2026-10:uncached"
    ],
    "method": "bundled_exact_rate_v1"
  }
}
```

- **Fixed-point integer precision**: All amounts are measured in integer nano-dollars (`1 USD = 1,000,000,000 nanos`) and serialized as strings to preserve full 64-bit integer precision. Floating-point numbers are never used.
- **Provenance verification**: Only dispatches with verified rate schedules matching the exact provider, model, and currency contribute to the serialized `pricedDispatches` and `totalCost` fields (Rust fields: `priced_dispatches` and `total_cost`).
- **Fail-closed status**:
  - `"complete"`: All dispatches are priced with valid schedule provenance.
  - `"partial"`: At least one dispatch is priced, with unpriced dispatches counted and numeric subtotal preserved.
  - `"unavailable"`: No dispatches could be verified or priced; `totalCost` is omitted from the serialized summary because the value is unavailable.
- **Dynamic re-aggregation**: `list_journals()` dynamically re-aggregates summaries on every read, preventing stale on-disk summaries from bypassing verification rules.

---

## 9. Codex Workflow Review Settings

Codex Autocode settings are stored separately from the application `config.json`, under the effective Codex home:

- `CODEX_HOME` when set and non-empty.
- Otherwise `%USERPROFILE%\.codex` on Windows (or `$HOME/.codex` on other platforms).
- Review settings file: `pov-review-settings.json`.

The current review-settings schema is version 4. The default uses `codex_self_only`; Codex is the work agent and is not launched as a separate `codex` process for self-review. External reviewer selections default to off, Gemini is disabled, and the default DeepSeek model/effort are `deepseek-flash` / `high`. The four modes are `codex_self_only`, `external_only`, `codex_plus_external`, and `none`. Enabling Gemini only makes it an available candidate; it does not select or launch it. A selected reviewer must have a valid priority when arbitration is needed.

All required reviewers must return a valid verdict before Priority Reviewer can resolve a disagreement. An incomplete, unavailable, failed, timed-out, or missing review cannot be overridden. Unresolved blocking findings, safety violations, and required-test failures cannot be overridden by priority. Non-priority findings are retained in the report.

Installation state and CLI authentication state are separate. Review settings do not store CLI credentials. The optional Gemini reviewer remains unavailable on Windows when its read-only sandbox cannot be verified.

Codex Workflow Skills are installed under the Codex Skills directory (`$HOME/.agents/skills`, with platform-specific home resolution). The three managed skills are `pov-plan-only`, `pov-build-review`, and `pov-full-cycle`. Updates distinguish verified official historical templates from unknown or modified files; updates to changed files require a review and backup, and unknown entries remain protected. Global instructions are managed in `AGENTS.md` under the effective Codex home. Poverello updates only its marked block and preserves unrelated content; a higher-priority `AGENTS.override.md` can shadow that file.

Reviewer diagnostics are stored as JSONL at `%USERPROFILE%\.gemini\antigravity-ide\logs\reviewer-diagnostics.jsonl` on Windows (or the equivalent home directory on other platforms). Events contain run metadata, status, and error categories, not prompts or response bodies. The log is size-bounded and rotates; the in-app export applies additional filtering. A DSH `read-only` configuration overlay and tool restrictions are application-level controls, not a Windows OS-enforced filesystem/network boundary.
