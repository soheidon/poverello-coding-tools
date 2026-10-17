[English](ANTIGRAVITY_MCP.md) | [日本語](ANTIGRAVITY_MCP.ja.md) | [中文(简体)](ANTIGRAVITY_MCP.zh-CN.md) | [中文(繁體)](ANTIGRAVITY_MCP.zh-TW.md) | [한국어](ANTIGRAVITY_MCP.ko.md) | [Français](ANTIGRAVITY_MCP.fr.md) | [Deutsch](ANTIGRAVITY_MCP.de.md) | [Español](ANTIGRAVITY_MCP.es.md)

[← Back to Poverello README](../README.md)

# Using Poverello MCP with Google Antigravity

Poverello does not require a separate MCP server executable. The installed `anthro-bridge.exe` provides both the desktop application and the MCP server. Antigravity starts the MCP mode by launching the same executable with `--mcp-server`.

The canonical MCP server key in Antigravity is `poverello`, exposing tools as `poverello/plan`, `poverello/revise`, and `poverello/review`. The executable identity `anthro-bridge.exe`, legacy server key `anthro-bridge`, and legacy tool names like `anthro-bridge/plan` remain supported for backward compatibility.

```text
Normal launch
anthro-bridge.exe
→ Poverello desktop app / 3P Gateway

MCP launch
anthro-bridge.exe --mcp-server
→ headless stdio MCP server for Antigravity (server key: poverello)
```

This allows agentic environments such as Google Antigravity to delegate architectural and implementation planning to external LLMs (e.g., DeepSeek V4, MiMo, Kimi, MiniMax, GLM, or OpenRouter models) via `poverello/plan` (or legacy `anthro-bridge/plan`), while performing the actual token-intensive code edits, terminal commands, builds, and tests using Antigravity's subscription-backed model allocation.

---

## 1. What This Workflow Does

```text
Antigravity
    ↓ stdio
anthro-bridge.exe --mcp-server
    ↓
Configured external planner model
    ↓
Structured implementation plan returned
    ↓
Antigravity executes edits, builds, and tests
using subscription-backed capacity
```

- **External API**: Responsible only for generating the implementation plan based on the relevant repository context. Billed by the respective provider.
- **Antigravity Subscription**: Performs the heavy file-reading, code-editing, tool-calling, and test-running loops.
- **Separation of Concerns**: You get high-quality external reasoning for architecture and plans without exhausting external API tokens on routine code generation.

---

## 2. Requirements

1. **Poverello** installed on Windows.
2. Provider authentication configured in Poverello or system environment variables for the provider you want to use for planning.
3. **Google Antigravity** installed and running.

---

## 3. Configure the MCP Server in Antigravity

### Method 1 — GUI Configuration via Poverello (Recommended)

1. Open Poverello and go to **Settings** (`[Settings]` tab) > **Antigravity** in the left sub-navigation.
2. In the **Antigravity Integration** card:
   - **Target Executable**: By default, the currently running `anthro-bridge.exe` path is shown. If you wish to use a different binary (e.g. portable or custom build), click **Change** (`antigravity.btnChangeExe`) and select the executable.
   - **Register / Update**: Click **Update Antigravity Configuration** (`antigravity.btnUpdate`) to safely register or update the `poverello` entry in `%USERPROFILE%\.gemini\config\mcp_config.json`. Existing `anthro-bridge` entries and all other MCP server configurations in that file remain preserved.
   - **Remove**: Click **Remove Configuration** (`antigravity.btnRemove`) if you wish to unregister the server from Antigravity.
   - **Open Folder**: Click **Open Settings Folder** (`antigravity.btnOpenFolder`) to inspect the directory in Windows Explorer.

---

### Method 2 — Manual Configuration (Advanced)

1. In Poverello **Settings > Antigravity**, click **Open Settings Folder** to open `%USERPROFILE%\.gemini\config\` in Windows Explorer.
2. Open or create `mcp_config.json` and add the `poverello` entry under `mcpServers`:

```json
{
  "mcpServers": {
    "poverello": {
      "command": "C:\\Users\\<USER>\\AppData\\Local\\Poverello\\anthro-bridge.exe",
      "args": ["--mcp-server"]
    }
  }
}
```

For development builds, point directly to the release build executable:
```json
{
  "mcpServers": {
    "poverello": {
      "command": "C:\\Users\\<USER>\\path\\to\\poverello-coding-tools\\gui\\src-tauri\\target\\release\\anthro-bridge.exe",
      "args": ["--mcp-server"]
    }
  }
}
```

> [!TIP]
> You do **not** need to write provider API keys into Antigravity's `mcp_config.json`. The MCP server leverages Poverello's credential resolution (reading from Windows user environment variables like `ANTHROPIC_API_KEY`, `DEEPSEEK_API_KEY`, `ZAI_API_KEY`, `OPENROUTER_API_KEY`, `MOONSHOT_API_KEY`, `MINIMAX_API_KEY`, `XIAOMI_API_KEY`, or saved application settings).

---

## 4. Verify the MCP Connection

In Antigravity's **Installed MCP Servers** view, confirm that `poverello` (and `anthro-bridge` if configured) is recognized:

```text
poverello
  3 tools enabled
  - plan
  - revise
  - review
```

---

## 5. Configure Planner and Reviewer Models in Poverello

Poverello clearly separates model selection from detailed parameter management:

1. **Top-Level `MCP` Tab (`MCP for Antigravity`)**:
   - Displays available providers (DeepSeek, OpenRouter, MiniMax, MiMo (V2.6), Kimi, GLM) and profiles.
   - Click a provider card to switch the active planner and reviewer destination immediately.
2. **`Settings` > `Antigravity`**:
   - **MCP Plan Settings** card: Configure model selection, Thinking mode, and Reasoning Effort per provider/profile. Both `plan` and `review` share the same target configuration.
   - **Antigravity Integration** card: Manage MCP server registration and Antigravity Commands (Global Skills).

> [!NOTE]
> The Poverello MCP server reads current configuration dynamically on every `plan()` and `review()` tool invocation. You do **not** need to restart the MCP server or Antigravity when changing providers or model settings in the GUI.

---

## 6. Antigravity Commands (`/pov-plan`, `/pov-revise` & `/pov-review`) (Recommended)

From **Settings > Antigravity > Antigravity Integration**, you can install Global Skills to use slash commands across all Antigravity workspaces (legacy `/anthro-*` commands remain supported for existing workspaces):

- Click **Install All Commands** (`antigravity.btnInstallAll`) or click **Install** (`antigravity.commandBtnInstall`) next to individual commands.

### Create a new implementation plan:
```text
/pov-plan <description of the task or feature>
```
*Gathers repository context, invokes canonical `poverello/plan` (or legacy `anthro-bridge/plan`), and stops cleanly after presenting the plan without making code edits or running build commands.*

### Revise an existing implementation plan:
```text
/pov-revise <feedback or updated requirements to incorporate>
```
*Identifies the current implementation plan (from active context or `implementation_plan.md`), passes the plan and feedback to canonical `poverello/revise` (or legacy fallback), and updates the plan while preserving unaffected sections.*

### Review implementation against approved plan:
```text
/pov-review
```
*Gathers the approved plan, `git status --short`, `git diff HEAD`, review-relevant untracked files, and test results, invokes canonical `poverello/review` (or legacy `anthro-bridge/review`), and presents a 3-tier verdict (`Approved`, `Approved with recommendations`, `Not approved`) along with commit readiness (`READY` / `NOT READY`). Read-only; never commits or modifies files.*

> [!IMPORTANT]
> When executing through `/pov-plan`, `/pov-revise`, or `/pov-review` (or legacy `/anthro-*`), the command workflow owns the tool call. Workspace rules will not trigger additional duplicate calls.

---

## 7. Automating Planning via Workspace Rules

Place a workspace rule like [`.agents/rules/deepseek-planner.md`](../.agents/rules/deepseek-planner.md) in your project to automate external planner invocation for complex tasks:

```markdown
---
trigger: model_decision
description: Use for implementation, debugging, architectural changes, or multi-file code changes where an external implementation plan would be useful. Do not use for trivial text-only edits.
---

# DeepSeek Planner Rule

For non-trivial implementation tasks in this repository:

1. If the current task is being executed through the `/pov-plan` or `/pov-revise` command (or legacy `/anthro-plan` / `/anthro-revise`), do NOT invoke `poverello/plan` (or `anthro-bridge/plan`) separately. The command workflow owns the planner call.
2. First inspect the repository yourself and identify the files and code relevant to the task.
3. Summarize only the context necessary for implementation planning.
4. Call the canonical `poverello/plan` MCP tool (or legacy `anthro-bridge/plan` if `poverello` is not yet configured) exactly once with:
   - the user's task;
   - the relevant repository context;
   - important constraints.
   Note: "Exactly once" means duplicate planner calls are prohibited once a successful usable result is obtained. If the tool call itself fails or returns an unusable response (e.g. transport or decoding error), exactly 1 recovery retry is permitted.
5. Use the returned DeepSeek plan as the basis for implementation.
6. Perform file edits, builds, and tests yourself.
7. Do not call `poverello/plan` (or `anthro-bridge/plan`) again unless the implementation encounters a major unresolved problem.
8. Do not ask the user to repeat this workflow.
9. Do not use the planner for trivial tasks such as a one-word text change unless planning would materially help.
```

### Triggering Policy:
- **Trivial / localized tasks** (e.g., fixing a typo, one-line edits, small syntax adjustments): Do not trigger the planner.
- **Non-trivial tasks** (architectural changes, multi-file features, complex debugging): Antigravity inspects repository context, invokes `poverello/plan` once, and executes the implementation based on the returned plan.

---

## 8. Typical Automated Workflow

```text
User: "Refactor feature X to support multiple profiles."
    ↓
Antigravity reads files and summarizes the codebase context
    ↓
Antigravity triggers poverello/plan tool call (exactly once)
    ↓
Poverello sends prompt to configured external model
    ↓
Antigravity receives structured implementation plan
    ↓
User reviews/approves plan
    ↓
Antigravity modifies files, executes tests, and verifies changes
```

---

## 9. Important Notes

- **Independent Operation**: The MCP server operates completely independently of the Poverello 3P Gateway. The 3P Gateway does not need to be running for MCP calls to work.
- **Separate Billing**: Calls to `poverello/plan`, `poverello/revise`, and `poverello/review` (or legacy `anthro-bridge/*`) incur external API costs billed by the chosen provider. Subsequent file editing, tool execution, and testing use Antigravity's subscription capacity.
- **Dynamic Configuration**: Switching the active MCP provider, profile, or model parameters in the Poverello GUI takes effect immediately on the next `plan()`, `revise()`, or `review()` invocation.

---
> [!NOTE]
> **MCP Planning & Reviewing Uses Cloud Providers**
>
> Antigravity MCP uses the active cloud model provider configured in Poverello (`mcp.provider` / `mcp.targets`). The **Ollama Local** backend is strictly dedicated to **Claude Code CLI** and does not affect MCP operations. For Claude Code local inference, see the [Ollama Local Guide](OLLAMA_LOCAL.md).
