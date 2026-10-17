[English](README.md) | [日本語](docs/README.ja.md) | [中文(简体)](docs/README.zh-CN.md) | [中文(繁體)](docs/README.zh-TW.md) | [한국어](docs/README.ko.md) | [Français](docs/README.fr.md) | [Deutsch](docs/README.de.md) | [Español](docs/README.es.md)

# Poverello Coding Tools

**Use Claude Code / Claude Desktop as the coding harness, route inference to third-party LLM APIs, orchestrate multi-agent planner–reviewer convergence with cost-estimate tracking, and manage external models and rules for Google Antigravity.**

Poverello Coding Tools (shown in the app as **Poverello**) is a Windows companion application for AI-assisted software development. It supports five core workflows:

1. **3P Gateway for Claude Code / Claude Desktop** — Keep Claude's repository exploration, tool use, file editing, and test execution while routing inference to third-party cloud providers.
2. **Dedicated Local LLMs for Claude Code (Ollama)** — Run Claude Code completely offline with zero API costs using local Ollama models.
3. **MCP Planner & Reviewer for Google Antigravity** — Delegate implementation planning and post-implementation review to external models via the canonical `poverello/plan`, `poverello/revise`, and `poverello/review` MCP tools (with `anthro-bridge/*` maintained for backward compatibility).
4. **Multi-Agent Development Orchestrator** — Execute bounded Planner–Reviewer convergence workflows across CLI and API agents, track normalized token usage and rate-verified cost estimates (in nano-dollars), and maintain auditable dispatch journals.
5. **Antigravity Rules Management** — Configure, preview, and safely synchronize lightweight Poverello Direct Coding rules (`pov-direct.md`) for Antigravity.

> [!TIP]
> **What's New / 新実装の詳細はこちら**: For detailed specifications and changelogs of new implementations, see [NEWS.md (Release Notes)](NEWS.md). バージョンごとの新機能や詳細な実装仕様については [NEWS.md](NEWS.md) をご覧ください。

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
- **Orchestrator Integration**: Can be paired with the Multi-Agent Orchestrator (Workflow 4) for autonomous convergence, normalized usage tracking, and rate-based cost estimates.

The executable retains the `anthro-bridge.exe` name, and the legacy `anthro-bridge`
MCP server key and `/anthro-*` skills remain supported for backward compatibility.

**Global Antigravity commands:**

- **`/pov-plan`** (legacy `/anthro-plan`) — Delegate implementation planning to the configured external model.
- **`/pov-revise`** (legacy `/anthro-revise`) — Revise an existing plan based on new feedback or constraints.
- **`/pov-review`** (legacy `/anthro-review`) — Review a completed implementation against the approved plan before commit, with explicit READY / NOT READY verdicts.

**Recommended workflow:**

```text
/pov-plan → Implementation & Tests → /pov-review → Commit
```

### 4. Multi-Agent Development Orchestrator (Convergence & Cost Engine)

```text
Task / Requirement
        ↓
+-------------------------------------------------------------+
| Poverello Orchestrator                                      |
|                                                             |
|  [Planner Profile]  ⇄  [Reviewer Profile]                   |
|  (Generate / Revise)     (Verify / Audit Verdicts)          |
|                                                             |
|  • Bounded convergence loop (configurable turn limit)        |
|  • Full dispatch journals (%APPDATA%\Poverello\runs\*.json) |
|  • Integer nano-dollar cost estimates & provenance checks    |
|  • Fail-closed status badges (Complete / Partial / N/A)     |
+-------------------------------------------------------------+
        ↓
Approved Plan / Audit Log
        ↓
Execution via Antigravity / Claude Code Harness
```

- **Autonomous Planner–Reviewer Convergence**: Configure independent LLM profiles for planning and reviewing (e.g. Claude Opus 5.5 / Sonnet 5.5, DeepSeek V4.1, GLM-5.3, or local Ollama). The Orchestrator automates iterative generation, review, and revision loops with strict criteria enforcement until convergence or maximum turn limits.
- **Safety Invariants & Human-in-the-Loop**: Approved in-scope additions can auto-converge; proposing a new primary plan always requires explicit human confirmation.
- **Auditable Run Journals**: `%APPDATA%\Poverello\runs\<run_id>.json` records run configuration, an optional task prompt, dispatch metadata and payload digests, available normalized usage and cost estimates, and Planner–Reviewer convergence audit metadata. Full adapter prompt envelopes and model responses are not stored in the run journal.
- **Normalized Usage & Rate-Verified Cost Estimates**:
  - When an adapter returns supported, parseable usage, the journal records normalized token metrics (`input_tokens`, `output_tokens`, `raw_total_tokens`, `cache_read_tokens`, `cache_write_tokens`; serialized as `cacheWriteTokens`). Missing or unsupported usage remains unknown, not zero. Provider-native cache-creation usage maps to `cache_write_tokens`.
  - Calculates estimates in integer nano-dollars (`u64 amount_nanos`, 1 USD = $10^9$ nanos, serialized as decimal strings to prevent JavaScript floating-point truncation). These are estimates from supported rate schedules, not provider invoices or actual billed amounts; floating-point math is strictly forbidden.
  - Enforces schedule provenance verification: rate schedules require exact matches across schedule ID, provider, model, and currency against verified rate tables (e.g., bundled Anthropic Opus 5.5 and Sonnet 5.5 rates). Mismatched or unknown rates fail closed as unpriced dispatches.
  - Current bundled estimates are limited to qualifying Planner–Reviewer convergence dispatches using Anthropic Direct Opus 5.5 or Sonnet 5.5 with explicitly zero cache buckets. Other providers, routes, and execution paths remain unpriced/unavailable; this is not evidence of zero cost.
  - Fail-closed status badges in the UI indicate whether estimate coverage is `Complete`, a valid subtotal is `Partial`, or estimates are `Unavailable`.
  - Recalculates dynamically from immutable dispatch records on every `list_journals()` read, preventing stale saved summaries from bypassing verification.

### 5. Antigravity Rules Management (Poverello Direct Coding)

```text
Poverello GUI ("Antigravity Rules" Tab)
          ↓ (Verified Apply / Rollback)
%USERPROFILE%\.gemini\config\rules\pov-direct.md
          ↓
Google Antigravity adheres to direct coding rules
without expanding autonomous scope
```

- **Scope & Direction Control**: Configure lightweight direct-coding behavior for Antigravity, disabling unsolicited multi-agent planning or expanded refactoring for localized implementation tasks.
- **Single-File Integrity**: Safely manages the YAML `trigger` condition and bounded managed marker block while strictly preserving user-defined frontmatter, custom descriptions, and skill inclusions.
- **Fail-Safe Application**: Protected by pre-replacement SHA-256 validation, external deletion conflict checks, collision-resistant unpruned backups, verified read-back rollback, and legacy sidecar folder guards.

---

## Supported Providers

| Provider | Connection | Supported Families | Reasoning Controls |
|---|---|---|---|
| **Anthropic** | Direct API | Claude Opus 5.5, Sonnet 5.5, Haiku 4.5, 3.7 Sonnet | Adaptive thinking / effort |
| **DeepSeek** | Direct API | DeepSeek V4.1 Flash, V4 Pro 0813 | Normal / Low / High / Max |
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

Built-in **Direct DeepSeek** preset routes Opus 5 → V4.1 Flash / Max · Sonnet 5 → V4.1 Flash / High · Haiku 4.5 → V4.1 Flash / Low.

- `deepseek-v4.1-flash` — Current flagship reasoner ($0.27 / 1M input · $1.10 / 1M output).
- `deepseek-v4-pro-0813` — High-quality baseline without extended reasoning ($0.27 / 1M input · $1.10 / 1M output).

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

Supports multiple named profiles. Full OpenAI model catalog (single dropdown):

| Model ID | Display Name |
|---|---|
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

## Model Pricing (as of v0.23.0)

| Model | Input | Output |
|---|---|---|
| DeepSeek V4.1 Flash | \$0.27 / 1M | \$1.10 / 1M |
| DeepSeek V4 Pro 0813 | \$0.27 / 1M | \$1.10 / 1M |
| GPT-6 Astra / Astra Pro / Astra Latest | \$10 / 1M | \$50 / 1M |
| GPT-5.6 Sol / Terra / Luna | \$5 / 1M | \$25 / 1M |
| GPT-5.6 Sol Pro / Terra Pro / Luna Pro | \$5 / 1M | \$25 / 1M |
| Gemini 3.8 Flash (OpenRouter) | \$0.75 / 1M | \$3.75 / 1M |
| MiMo-V2.6-Flash | \$0.14 / 1M | \$0.28 / 1M |
| MiMo-V2.6-Pro | \$0.435 / 1M | \$0.87 / 1M |
| MiMo-V2.6-Pro-UltraSpeed | \$4.35 / 1M | \$8.70 / 1M |

---

## Installation

Download the latest currently published Windows installer from the [Releases](https://github.com/soheidon/poverello-coding-tools/releases) page and run it.

The Windows installer supports 8 languages. Poverello v0.24.1 keeps the existing application identity and includes an in-place upgrade path from Anthro Bridge. On first launch, it is designed to copy settings and run data from `%APPDATA%\Anthro Bridge` (or `%APPDATA%\Anthro Bridge Dev` for development builds) to the corresponding Poverello data folder while retaining the original folder as a recovery copy. Real Windows installation, migration, upgrade, reinstall, cancellation, and uninstall flows have not been verified; back up important data before upgrading.

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
2. Select the **MCP** tab and configure your model in **Settings > Antigravity > MCP Plan Settings**.
3. Register `anthro-bridge.exe` with `["--mcp-server"]` under server key `poverello` in Antigravity's MCP configuration (or click **Configure Automatically** in Poverello).
4. Use `/pov-plan` to design plans, `/pov-revise` to update plans, and `/pov-review` to review implementations before commit (legacy `/anthro-*` commands remain recognized).
5. Follow the complete [Antigravity MCP Setup Guide](docs/ANTIGRAVITY_MCP.md).

### Workflow 4: Multi-Agent Development Orchestrator

1. Open Poverello and switch to the **Orchestrator** tab in the top navigation bar.
2. Select or customize agent profiles for the **Planner** and **Reviewer** roles (or choose a built-in preset in Orchestrator Settings).
3. Set the convergence goal and iteration bounds.
4. Launch convergence: monitor live dispatch progress, audit verdicts, and cost-estimate coverage badges.
5. Review completed runs and dispatch logs stored under `%APPDATA%\Poverello\runs\`.

### Workflow 5: Antigravity Rules Management (Poverello Direct Coding)

1. Open Poverello and switch to the **Antigravity Rules** tab in the top navigation bar.
2. Select the desired rule mode and inspect the generated rule preview.
3. Click **Apply Settings** to safely write the rule to `%USERPROFILE%\.gemini\config\rules\pov-direct.md`.
4. Existing custom comments, frontmatter fields, and backups are safely preserved.

---

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

- [Release Notes (NEWS.md)](NEWS.md) — Detailed changelog and newly implemented features（新実装の詳細はこちら）
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
