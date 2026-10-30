[English](README.md) | [日本語](docs/README.ja.md) | [中文(简体)](docs/README.zh-CN.md) | [中文(繁體)](docs/README.zh-TW.md) | [한국어](docs/README.ko.md) | [Français](docs/README.fr.md) | [Deutsch](docs/README.de.md) | [Español](docs/README.es.md)

# Poverello Coding Tools

> **v0.25.2 transition:** Auto-Converge remains in the codebase but is hidden from normal app navigation. Production dispatch remains disabled, and Stage 2 isolation remains unverified. This is not a release-readiness claim.

**Use Claude Code / Claude Desktop as the coding harness, route inference to third-party LLM APIs, orchestrate bounded multi-agent workflows with cost-estimate tracking, and manage external models and rules for Google Antigravity.**

Poverello Coding Tools (shown in the app as **Poverello**) is a Windows companion application for AI-assisted software development. It supports five core workflows:

1. **3P Gateway for Claude Code / Claude Desktop** — Keep Claude's repository exploration, tool use, file editing, and test execution while routing inference to third-party cloud providers.
2. **Dedicated Local LLMs for Claude Code (Ollama)** — Run Claude Code completely offline with zero API costs using local Ollama models.
3. **MCP Planner & Reviewer for Google Antigravity** — Delegate implementation planning and post-implementation review to external models via the canonical `poverello/plan`, `poverello/revise`, and `poverello/review` MCP tools (with `anthro-bridge/*` maintained for backward compatibility).
4. **Multi-Agent Development Orchestrator** — Run supported bounded Planner–Reviewer plan/review workflows across CLI and API agents, track normalized token usage and rate-verified cost estimates (in nano-dollars), and maintain auditable dispatch journals.
5. **Antigravity Skills & Rules Management** — Centrally configure Reviewer Policy for workflow skills (Codex CLI alone vs. Codex CLI + DeepSeek Harness composite), manage protected Global Rules standard templates (`codex-cli.md`, `deepseek-harness.md`), tune Direct Coding behavior (`pov-direct.md`), and manage Nerf skills.

> [!TIP]
> **What's New:** For version-by-version feature updates, implementation details, and changelogs, see [NEWS.md (Release Notes)](NEWS.md).

---

## Five Main Workflows

### 1. Claude Code / Claude Desktop with 3P Gateway

```text
Claude Code / Claude Desktop
             ↓
  Poverello 3P Gateway
             ↓
Anthropic Direct / DeepSeek / Kimi Code / OpenRouter / MiniMax / MiMo / GLM
```

- **Harness & Model Separation**: Keep Claude's agentic tooling while routing inference to third-party providers or official Anthropic endpoints.
- **Dynamic Multi-Profile Routing & Multi-Provider Gateway**: Switch active providers, OpenRouter profiles, or configure Multi-Provider Gateway profiles that map each canonical Claude route independently to different providers from the GUI.
- **Setup Guide**: [Claude Desktop / Cowork 3P Gateway Setup](docs/THIRD_PARTY_INFERENCE.md)

### 2. Claude Code with Dedicated Local LLMs (Ollama)

```text
Claude Code CLI
      ↓ (Loopback ANTHROPIC_BASE_URL)
Ollama Local (127.0.0.1:11434/v1)
      ↓
Gemma 4 / Qwen / Llama 3 / DeepSeek-R1 (Local On-Device)
```

- **Free, Offline & Private**: Run Claude Code without API keys, usage limits, or cloud data transfer.
- **Dedicated Routing & Model Discovery**: Switch between Gateway and Ollama (`active_route: "ollama"`) with automatic model discovery from `/api/tags`.
- **Reasoning & Context Tuning**: Full support for thinking token budgets (1024 tokens) and context window overrides (`num_ctx`) per alias.
- **Total Isolation**: Claude Desktop, Cowork on 3P, and Google Antigravity MCP continue to use cloud models independently.
- **Setup Guide**: [Ollama Local Setup Guide](docs/OLLAMA_LOCAL.md)

### 3. Antigravity with MCP Planner & Reviewer

```text
Antigravity
    ↓ stdio
anthro-bridge.exe --mcp-server
    ↓
Configured external model (planner / reviewer)
    ↓
Implementation plan / Review verdict
    ↓
Antigravity implements and tests
using subscription-backed capacity
```

- **Planning vs. Execution Split**: External models generate the high-level plan or review verdict; Antigravity subscription capacity executes token-intensive code edits.
- **Live GUI Configuration**: Switching the planner or reviewer provider, model, or reasoning effort takes effect immediately on the next invocation.
- **Setup Guide**: [Google Antigravity + Poverello MCP Setup](docs/ANTIGRAVITY_MCP.md)
- **Orchestrator Integration**: Can be paired with the Multi-Agent Orchestrator (Workflow 4) for supported bounded tasks, normalized usage tracking, and rate-based cost estimates.

The executable retains the `anthro-bridge.exe` name, and the legacy `anthro-bridge`
MCP server key and `/anthro-*` skills remain supported for backward compatibility.

**Global Antigravity commands:**

- **`/pov-mcp-plan`** (legacy `/anthro-plan`) — Delegate implementation planning to the configured external model.
- **`/pov-mcp-revise`** (legacy `/anthro-revise`) — Revise an existing plan based on new feedback or constraints.
- **`/pov-mcp-review`** (legacy `/anthro-review`) — Review a completed implementation against the approved plan before commit, with explicit READY / NOT READY verdicts.

Complete autonomous workflows: `/pov-full-cycle`, `/pov-plan-only`, `/pov-build-review` (managed under **Settings > Antigravity Skills & Rules** with Reviewer Policy synchronization).

**Recommended workflow:**

```text
/pov-mcp-plan → Implementation & Tests → /pov-mcp-review → Commit
```

### 4. Multi-Agent Development Orchestrator (Bounded Workflows & Cost Estimates)

```text
Task / Requirement
        ↓
+-------------------------------------------------------------+
| Poverello Orchestrator                                      |
|                                                             |
|  [Planner Profile]  ⇄  [Reviewer Profile]                   |
|  (Generate / Revise)     (Verify / Audit Verdicts)          |
|                                                             |
|  • Bounded plan/review loop (configurable turn limit)         |
|  • Full dispatch journals (%APPDATA%\Poverello\runs\*.json) |
|  • Integer nano-dollar cost estimates & provenance checks    |
|  • Fail-closed status badges (Complete / Partial / N/A)     |
+-------------------------------------------------------------+
        ↓
Approved Plan / Audit Log
        ↓
Separate, user-controlled implementation via your coding harness
```

- **Bounded Orchestrator Workflows**: Configure agent profiles and iteration limits for the supported Orchestrator workflows.
- **Safety Invariants & Human-in-the-Loop**: Actions requiring approval remain subject to the workflow's explicit approval gates; proposing a new primary plan always requires explicit human confirmation.
- **Auditable Run Journals**: `%APPDATA%\Poverello\runs\<run_id>.json` records run configuration, an optional task prompt, dispatch metadata and payload digests, available normalized usage and cost estimates, and Planner–Reviewer convergence audit metadata. Full adapter prompt envelopes and model responses are not stored in the run journal.
- **Normalized Usage & Rate-Verified Cost Estimates**:
  - When an adapter returns supported, parseable usage, the journal records normalized token metrics (`input_tokens`, `output_tokens`, `raw_total_tokens`, `cache_read_tokens`, `cache_write_tokens`; serialized as `cacheWriteTokens`). Missing or unsupported usage remains unknown, not zero. Provider-native cache-creation usage maps to `cache_write_tokens`.
  - Calculates estimates in integer nano-dollars (`u64 amount_nanos`, 1 USD = $10^9$ nanos, serialized as decimal strings to prevent JavaScript floating-point truncation). These are estimates from supported rate schedules, not provider invoices or actual billed amounts; floating-point math is strictly forbidden.
  - Enforces schedule provenance verification: rate schedules require exact matches across schedule ID, provider, model, and currency against verified rate tables (e.g., bundled Anthropic Opus 5.5 and Sonnet 5.5 rates). Mismatched or unknown rates fail closed as unpriced dispatches.
  - Current bundled estimates are limited to qualifying Planner–Reviewer convergence dispatches using Anthropic Direct Opus 5.5 or Sonnet 5.5 with explicitly zero cache buckets. Other providers, routes, and execution paths remain unpriced/unavailable; this is not evidence of zero cost.
  - Fail-closed status badges in the UI indicate whether estimate coverage is `Complete`, a valid subtotal is `Partial`, or estimates are `Unavailable`.
  - Recalculates dynamically from immutable dispatch records on every `list_journals()` read, preventing stale saved summaries from bypassing verification.

### 5. Antigravity Skills & Rules Management

```text
Poverello GUI (Settings > Antigravity Skills & Rules)
          ↓ (Reviewer Policy / Template Guard / Verified Rollback)
%USERPROFILE%\.gemini\config\rules\
  • codex-cli.md / deepseek-harness.md (Standard Global Rule Templates)
  • pov-direct.md (Direct Coding Behavior)
~/.gemini/antigravity/skills/
  • pov-full-cycle, pov-plan-only, pov-build-review (Policy-Synchronized Workflow Skills)
```

- **Centralized Reviewer Policy**: Switch autonomous workflow skills (`/pov-full-cycle`, `/pov-plan-only`, `/pov-build-review`) between `codex_only` (independent Codex CLI alone) and `codex_plus_deepseek` (Codex CLI + DeepSeek Harness composite review) with automatic SHA-256 validation and policy synchronization.
- **Protected Global Rules Templates**: One-click install and safe removal of standard Global Rules templates (`codex-cli.md`, `deepseek-harness.md`) verified by SHA-256 digests, with platform-guarded removal (`can_remove`) on Windows.
- **Scope & Direction Control (Poverello Direct Coding)**: Configure lightweight direct-coding behavior for Antigravity, disabling unsolicited multi-agent planning or expanded refactoring for localized implementation tasks.
- **Single-File Integrity**: Safely manages the YAML `trigger` condition and bounded managed marker block while strictly preserving user-defined frontmatter, custom descriptions, and skill inclusions.
- **Fail-Safe Application**: Protected by pre-replacement SHA-256 validation, external deletion conflict checks, collision-resistant unpruned backups, verified read-back rollback, and legacy sidecar folder guards.

---

## Supported Providers

| Provider | Connection | Supported Families | Reasoning Controls |
|---|---|---|---|
| **Anthropic** | Direct API | Claude Opus 5.5, Sonnet 5.5, Haiku 4.5, 3.7 Sonnet | Adaptive thinking / effort |
| **DeepSeek** | Direct API | `deepseek-flash` (V4.1 Flash 1017), `deepseek-v4-pro` (V4 Pro 0813) | Normal / Thinking · Low / High / Max |
| **Kimi Code** | Direct API | kimi-for-coding, kimi-for-coding-highspeed | Thinking mode |
| **MiniMax** | Direct API | MiniMax M3, M2.7 | Model-specific |
| **Kimi / Moonshot** | Direct API | Kimi K2.x, Kimi K3 | Thinking / Reasoning effort |
| **GLM (Z.ai)** | Direct API | GLM-5.3, GLM-5.3-Flash, GLM-5.3-FlashX, GLM-5.2 | GLM-5.3: Low / High / Max · GLM-5.2: Normal / Thinking |
| **MiMo / Xiaomi** | Direct API | MiMo V2.6 Flash, Pro, Pro-UltraSpeed (V2.5 backward-compatible) | Normal / Thinking |
| **Ollama Local** | Local Loopback | Gemma 4, Qwen 2.5, Llama 3.3, DeepSeek-R1 | Thinking (1024 budget) / Disabled |
| **OpenRouter** | Multi-profile Gateway | See OpenRouter section below | Model-specific / Profile-specific |
| **Multi-Provider Gateway** | Custom Routes | Any combination of above eligible providers | Per-route provider controls |

### Anthropic (Direct)

Direct official access via `https://api.anthropic.com` using `ANTHROPIC_API_KEY`:

- **Dedicated API Key**: Uses official Anthropic credentials stored safely in application settings or user environment variables.
- **Supported Models**: `claude-opus-5-5`, `claude-sonnet-5-5`, `claude-haiku-4-5`, `claude-3-7-sonnet`, `claude-3-5-sonnet`, `claude-3-5-haiku`.
- **Reasoning Controls**: Native adaptive thinking and reasoning effort controls supported by upstream models.
- **Token-Cap Diagnostics**: Streaming and non-streaming detection of `stop_reason: "max_tokens"` with reasoning-only output failing closed with clear diagnostic errors.

### DeepSeek (Direct)

Built-in **Direct DeepSeek** preset uses the official stable API ID `deepseek-flash` (currently DeepSeek-V4.1-Flash-1017): Opus 5 → Max · Sonnet 5 → High · Haiku 4.5 → Low. The current official pricing page lists `deepseek-v4-pro` as DeepSeek-V4-Pro-0813. Both IDs are available through DeepSeek's Anthropic-compatible endpoint; Flash supports vision, while Pro does not. The official catalog lists a 1M-token context window and up to 384K output tokens.

DeepSeek's official materials differ on Pro routing: the current pricing page lists V4-Pro-0813 and separate Pro rates and says API service continued after September 14, 2026, while the September 10 announcement described temporary routing to V4.1 Flash at Flash rates until V4.1-Pro launches. Poverello cannot verify routing or billing for an individual request; its displayed Pro prices are the published schedule, not an invoice or guarantee. See the [current pricing page](https://api-docs.deepseek.com/quick_start/pricing/) and [September 10 announcement](https://api-docs.deepseek.com/news/news260910/).

DeepSeek's retired IDs `deepseek-v4-flash` and `deepseek-v4-flash-vision-exp` remain accepted upstream and route to the current Flash model at Flash rates. New Poverello configurations should use the canonical IDs above.

### GLM / Z.ai (Direct)

Direct access using Z.ai's official Anthropic Messages-compatible API (`https://api.z.ai/api/anthropic/v1/messages`):

- **API Key and eligibility**: Uses the configured `ZAI_API_KEY` (or key entered in Poverello) for this endpoint. Poverello does not identify key type, Coding Plan purchase history, account allowlisting, or billing eligibility. Z.ai's current account and endpoint requirements may vary; consult [Z.ai's setup guidance](https://zcode.z.ai/en/docs/configuration) before use.
- **Models**:
  - `glm-5.3` — Flagship reasoning model with always-on thinking.
  - `glm-5.3-flash` — High-speed reasoning model with always-on thinking.
  - `glm-5.3-flashx` — Ultra-low latency reasoning variant with always-on thinking.
  - `glm-5.2` — Versatile general coding model supporting both Normal and Thinking modes.
- **Reasoning Controls**: Selectable reasoning effort (`low`, `high`, `max`) for the GLM-5.3 family; Normal / Thinking toggle for GLM-5.2.
- **Strict Response Validation**: Responses that contain only thinking blocks without final text (e.g., when reaching `max_tokens` mid-reasoning) fail closed with an explicit error in both Orchestrator and MCP workflows, preventing empty plan execution or silent failures.

### Kimi Code (Direct)

Dedicated coding-specialist API (`KIMI_CODE_API_KEY`), separate from Moonshot Kimi:

- `kimi-for-coding` — Full-quality coding model.
- `kimi-for-coding-highspeed` — Low-latency variant.

### MiMo / Xiaomi (Direct)

Built-in **Direct MiMo** preset routes:
- Opus 5 → `mimo-v2.6-pro` / Thinking
- Sonnet 5 → `mimo-v2.6-pro` / Normal
- Haiku 4.5 → `mimo-v2.6-flash` / Thinking
- Default model: `mimo-v2.6-flash`

Models: `mimo-v2.6-flash`, `mimo-v2.6-pro`, `mimo-v2.6-pro-ultraspeed` (selectable). All three support a 1M-token context window and native multimodal capabilities (text, image, video).

**Normal / Thinking**: MiMo uses a simple Normal/Thinking toggle — no reasoning effort levels.

**V2.5 backward compatibility**: Saved `mimo-v2.5`, `mimo-v2.5-pro`, and `mimo-v2.5-pro-ultraspeed` routes are preserved and continue to function. Untouched legacy defaults migrate to V2.6 automatically on startup.

### Dedicated Local LLM (Ollama Local)

Dedicated local inference backend for **Claude Code CLI**:
- Endpoint: `http://127.0.0.1:11434/v1` (native Anthropic messages loopback)
- Dynamic discovery of installed local models via loopback `/api/tags`
- Thinking budget support (1024 tokens) and context window (`num_ctx`) overrides per alias
- See [Ollama Local Guide](docs/OLLAMA_LOCAL.md) for detailed configuration

### OpenRouter

Supports multiple named profiles. Current OpenAI model catalog in the single dropdown (15 models):

| Model ID | Display Name |
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

**GPT-6 Astra**: 1.05M context · reasoning effort: `low / medium / high / xhigh / max`.
**GPT-6 Astra Pro**: 1.05M context · always-on Pro reasoning (`reasoning.mode = pro`), no user-selectable effort.
**GPT Astra Latest**: alias tracking the latest Astra family model.

Built-in **OpenRouter: chatGPT** preset: Opus 5 → GPT-6 Astra / max · Sonnet 5 → GPT-6 Astra / high · Haiku 4.5 → GPT-6 Astra / medium.

Also available: **OpenRouter: Gemini** (Gemini 3.8 Flash · reasoning effort `low / medium / high`), **OpenRouter: Poolside**, **OpenRouter: Tencent**, **OpenRouter: InclusionAI**, **OpenRouter: StepFun**.

---

## DeepSeek API Pricing Reference

Official Direct API prices in USD per 1M tokens (checked 2026-10-19). Input shows cache-miss rates; cache-hit rates are in parentheses. The V4 Pro row reflects the current published pricing table; see the routing and billing caveat above.

| Model ID | Peak input (miss / hit) | Off-peak input (miss / hit) | Peak output | Off-peak output |
|---|---:|---:|---:|---:|
| `deepseek-flash` | $0.30 ($0.006) | $0.15 ($0.003) | $1.20 | $0.60 |
| `deepseek-v4-pro` | $1.32 ($0.044) | $0.66 ($0.022) | $3.96 | $1.98 |

Peak periods are 01:00–04:00 and 06:00–10:00 UTC on weekdays, excluding Chinese public holidays; weekends and holidays are off-peak all day. Poverello includes verified 2026 holiday and make-up-workday dates and indicates when the following year's calendar is missing. Other provider prices are shown in the app's pricing view and may change; check the provider for current billing terms. Orchestrator cost estimates currently cover only eligible Anthropic Direct dispatches with explicit zero cache usage—DeepSeek rates are not presented as an Orchestrator cost estimate. [Official DeepSeek pricing](https://api-docs.deepseek.com/quick_start/pricing/)

---

## Installation

Download the latest currently published Windows installer from the [Releases](https://github.com/soheidon/poverello-coding-tools/releases) page and run it.

The Windows installer supports 8 languages. Poverello keeps the existing application identity and includes an in-place upgrade path from Anthro Bridge. On first launch, it is designed to copy settings and run data from `%APPDATA%\Anthro Bridge` (or `%APPDATA%\Anthro Bridge Dev` for development builds) to the corresponding Poverello data folder while retaining the original folder as a recovery copy. Real Windows installation, migration, upgrade, reinstall, cancellation, and uninstall flows have not been verified; back up important data before upgrading.

---

## Quick Start

### Workflow 1: 3P Gateway for Claude Code / Claude Desktop

1. Open Poverello **Settings > API Key** and configure an API key for your desired provider.
2. Select your provider or OpenRouter profile on the dashboard.
3. Click **Start Gateway** (runs on `http://127.0.0.1:4000`).
4. Connect Claude Code or Claude Desktop:
   - **Claude Code**: Click **Copy Claude Code launch command** in Settings and paste it into PowerShell.
   - **Claude Desktop / Cowork**: Follow the [Claude Desktop 3P Setup Guide](docs/THIRD_PARTY_INFERENCE.md).

### Workflow 2: Dedicated Local LLMs for Claude Code (Ollama)

1. Ensure Ollama is running locally (`http://127.0.0.1:11434`).
2. Select **Ollama Local** on the Poverello Dashboard (or configure models in **Settings > Claude Code / Local LLMs**).
3. Click **Copy Claude Code launch command** and paste it into PowerShell.
4. Claude Code connects directly to your local Ollama instance with full thinking and context window support.

### Workflow 3: MCP Planner & Reviewer for Google Antigravity

1. Configure an API key for your chosen planner/reviewer model in Poverello.
2. Select the **MCP** tab (or **Settings > Antigravity MCP**) and configure your model in **Settings > Antigravity MCP > MCP Plan Settings**.
3. Register `anthro-bridge.exe` with `["--mcp-server"]` under server key `poverello` in Antigravity's MCP configuration (or click **Configure Automatically** in Poverello).
4. Use `/pov-mcp-plan` to design plans, `/pov-mcp-revise` to update plans, and `/pov-mcp-review` to review implementations before commit (legacy `/anthro-*` commands remain recognized).
5. Follow the complete [Antigravity MCP Setup Guide](docs/ANTIGRAVITY_MCP.md).

### Workflow 4: Multi-Agent Development Orchestrator

**Access note:** v0.25.2 hides the Orchestrator entry from the Settings sub-navigation while retaining its implementation and internal routes. Start autonomous workflows in Antigravity with `/pov-full-cycle`, `/pov-plan-only`, or `/pov-build-review`.

### Workflow 5: Antigravity Skills & Rules Management

1. Open **Settings > Antigravity Skills & Rules**.
2. Select your Reviewer Policy (`codex_only` or `codex_plus_deepseek`) to synchronize workflow skills.
3. Install or manage protected standard Global Rules templates (`codex-cli.md`, `deepseek-harness.md`).
4. Configure Poverello Direct Coding parameters and click **Apply** to synchronize `pov-direct.md`.

## API Keys

| Provider | Environment Variable |
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

## Documentation

- [Release Notes (NEWS.md)](NEWS.md) — Version-by-version feature updates and implementation details.
- [Claude Desktop / Cowork 3P Gateway Setup](docs/THIRD_PARTY_INFERENCE.md)
- [Ollama Local Setup & CLI Configuration](docs/OLLAMA_LOCAL.md)
- [Google Antigravity + Poverello MCP Setup](docs/ANTIGRAVITY_MCP.md)
- [Configuration Reference (`config.json`)](docs/CONFIGURATION.md)
- [Provider Details & Reasoning Controls](docs/PROVIDERS.md)
- [Development & Verification Guide](docs/DEVELOPMENT.md)

---

## Troubleshooting

### Port 4000 Is Already in Use
```powershell
netstat -ano | findstr :4000
taskkill /PID <PID> /F
```

### Settings Revert After an Upgrade
Restart the application so migrations can run. Configuration is stored under `%APPDATA%\Poverello\config.json`. If migration reports a conflict, do not delete either data folder; back up both and resolve the conflict before restarting.

### MCP Planner Calls Fail
Ensure an API key is set for the provider selected under the **MCP** tab, or exported in your Windows user environment variables (e.g., `DEEPSEEK_API_KEY`, `OPENROUTER_API_KEY`). The 3P Gateway does not need to be running for MCP.

---

## License

MIT License. See [LICENSE](LICENSE).
