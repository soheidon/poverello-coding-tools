[English](README.md) | [日本語](docs/README.ja.md) | [中文(简体)](docs/README.zh-CN.md) | [中文(繁體)](docs/README.zh-TW.md) | [한국어](docs/README.ko.md) | [Français](docs/README.fr.md) | [Deutsch](docs/README.de.md) | [Español](docs/README.es.md)

# Poverello Coding Tools

**Use Claude Code / Claude Desktop as the coding harness, route inference to configured cloud or local models, and manage external models and rules for Google Antigravity.**

Poverello Coding Tools (shown in the app as **Poverello**) is a Windows companion application for AI-assisted software development. It supports five core workflows:

1. **3P Gateway for Claude Code / Claude Desktop** — Keep Claude's repository exploration, tool use, file editing, and test execution while routing inference to third-party cloud providers.
2. **Dedicated Local LLMs for Claude Code (Ollama)** — Run Claude Code completely offline with zero API costs using local Ollama models.
3. **MCP Planner & Reviewer for Google Antigravity** — Delegate implementation planning and post-implementation review to external models via the canonical `poverello/plan`, `poverello/revise`, and `poverello/review` MCP tools (with `anthro-bridge/*` maintained for backward compatibility).
4. **Antigravity Autocode and Skills & Rules** — Configure external review and coding behavior in the Antigravity Autocode tab; manage Antigravity Workflow Skills and Global Rules under Settings, with Antigravity Nerf Settings managed directly in the GUI. Shared CLI Tools have their own Settings page.
5. **Codex Autocode and Skills & Rules** — Configure reviewers for Codex Workflow Skills in the Codex Autocode tab; manage the three native Codex Skills and Poverello-managed `AGENTS.md` rules under Settings. Shared CLI Tools are managed separately under Settings.

> [!TIP]
> **What's New:** For version-by-version feature updates, implementation details, and changelogs, see [NEWS.md (Release Notes)](NEWS.md).

The next release target is **v0.27.3** (not yet published). Each Autocode surface has its own primary project-plan slot and can add one optional second slot. Selecting a Markdown plan associates it with the repository containing that file; Antigravity and Codex keep their selections independent. The plan-folder setting only supplies the file picker’s starting directory when no plan is active. Removing the optional slot clears its Poverello association without deleting project or plan files. The selected plan's full path and status are shown, with a separate action to open it.

---

## Five Main Workflows

### 1. Claude Code / Claude Desktop with 3P Gateway

```text
Claude Code / Claude Desktop
             ↓
  Poverello 3P Gateway
             ↓
Configured cloud or local model
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

The executable retains the `anthro-bridge.exe` name, and the legacy `anthro-bridge`
MCP server key and `/anthro-*` skills remain supported for backward compatibility.

**Global Antigravity commands:**

- **`/pov-mcp-plan`** (legacy `/anthro-plan`) — Delegate implementation planning to the configured external model.
- **`/pov-mcp-revise`** (legacy `/anthro-revise`) — Revise an existing plan based on new feedback or constraints.
- **`/pov-mcp-review`** (legacy `/anthro-review`) — Review a completed implementation against the approved plan before commit, with explicit READY / NOT READY verdicts.

Antigravity Workflow Skills: `/pov-full-cycle`, `/pov-plan-only`, `/pov-build-review` (installed and repaired under **Settings > Antigravity Skills & Rules**; reviewer policy is configured in **Antigravity Autocode**). Codex has separate native Skills with the same command names under **Settings > Codex Skills & Rules**.

**Recommended workflow:**

```text
/pov-mcp-plan → Implementation & Tests → /pov-mcp-review → Commit
```

### 4. Antigravity Autocode and Skills & Rules
```text
Antigravity Autocode tab
  • External Review policy / reviewer configuration
  • Antigravity Nerf Settings / Direct Coding

Settings > Antigravity Skills & Rules
  • Workflow Commands / repair
  • Global Rules

Settings > CLI Tools
  • Shared Node.js / npm and CLI management
```

- External Review and Antigravity Nerf Settings are configured in Antigravity Autocode. Settings > Antigravity Skills & Rules manages Antigravity Workflow Skills, Global Rules, and the plan-folder picker start-directory preference for Antigravity Autocode; shared CLI Tools have a separate Settings page.

### 5. Codex Autocode and Skills & Rules

```text
Codex Workflow Skills
       ↓ configured reviewers
Codex in-session review / optional external DeepSeek Harness or Gemini CLI
       ↓
Codex Autocode review settings

Settings > Codex Skills & Rules
  • /pov-plan-only, /pov-build-review, /pov-full-cycle
  • Poverello-managed instructions in CODEX_HOME/AGENTS.md
Settings > CLI Tools
  • Shared Codex CLI, Gemini CLI, DeepSeek Harness, Node.js / npm status
```

- The three Codex Workflow Skills are installed under the Codex Skills directory. Settings provides install, update, removal, backup recovery, and preview controls.
- Settings > Codex Skills & Rules provides a separate plan-folder picker start-directory preference for Codex Autocode, with an editable path, folder picker, and reset control.
- Poverello manages only its marked section in `CODEX_HOME/AGENTS.md`; unrelated user instructions are preserved. A higher-priority `AGENTS.override.md` may override it.
- Codex Autocode configures selected reviewers and their priority. Gemini CLI is optional and disabled by default; enabling it in **Settings > CLI Tools** does not select it or start a review. On Windows, Gemini review remains unavailable when its read-only sandbox cannot be verified.
- CLI installation state and authentication state are reported separately. Node.js and npm are detected as prerequisites; Poverello does not install them automatically. Since v0.26.1, non-interactive CLI and Node.js/npm status checks run without opening console windows and reuse short-lived cached results across tab changes; process initialization failures such as `0xC0000142` are reported explicitly. User-requested login remains interactive.
- Since v0.26.2, Codex Autocode distinguishes Codex's in-session work from external reviewers, applies priority only to conflicting valid verdicts, and preserves all findings. Reviewer-specific DeepSeek model and effort settings, JSONL diagnostics, and protected Skill updates are available. DeepSeek's snapshot and tool configuration do not claim Windows OS-level filesystem or network isolation; where that safety boundary is required but unverified, treat DeepSeek review as unavailable.
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

### Workflow 4: Antigravity Autocode and Skills & Rules
1. Configure External Review and Antigravity Nerf Settings in **Antigravity Autocode**. The selected policy controls reviewer use by Workflow Skills.
2. Under **Settings > Antigravity Skills & Rules**, manage Antigravity Workflow Skills and Global Rules. Skill repair requires confirmation and preserves a backup. Manage shared CLI Tools under **Settings > CLI Tools**.
3. Auto-Converge remains hidden; production dispatch is disabled and Stage 2 isolation is unverified.

### Workflow 5: Codex Autocode and Skills & Rules
1. In **Settings > CLI Tools**, check Node.js/npm and the Codex CLI / optional reviewer CLI status. Installation and authentication are separate states; Node.js is not installed automatically.
2. In **Settings > Codex Skills & Rules**, install or update `/pov-plan-only`, `/pov-build-review`, and `/pov-full-cycle`, and manage Poverello's section of `CODEX_HOME/AGENTS.md`.
3. In **Codex Autocode**, choose reviewers and a priority reviewer. Gemini CLI must first be enabled in **Settings > CLI Tools**; enabling it does not select it. Gemini review is unavailable on Windows when a verified read-only sandbox is absent.

## Documentation
- [Provider details and inference controls](docs/PROVIDERS.md)


- [Release Notes (NEWS.md)](NEWS.md) — Version-by-version feature updates and implementation details.
- [Claude Desktop / Cowork 3P Gateway Setup](docs/THIRD_PARTY_INFERENCE.md)
- [Ollama Local Setup & CLI Configuration](docs/OLLAMA_LOCAL.md)
- [Google Antigravity + Poverello MCP Setup](docs/ANTIGRAVITY_MCP.md)
- [Configuration Reference (`config.json`)](docs/CONFIGURATION.md)
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
