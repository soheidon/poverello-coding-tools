# Development & Verification Guide

This document contains instructions for building, developing, testing, and verifying Poverello.

---

## 1. Project Structure

```text
poverello-coding-tools/
├── README.md
├── SPEC.md
├── docs/
│   ├── ANTIGRAVITY_MCP.md
│   ├── CONFIGURATION.md
│   ├── OLLAMA_LOCAL.md
│   ├── DEVELOPMENT.md
│   ├── PROVIDERS.md
│   └── THIRD_PARTY_INFERENCE.md
├── gui/
│   ├── src/
│   │   ├── components/
│   │   │   ├── AntigravitySkillsRulesPanel.tsx
│   │   │   ├── AgentOptPanel.tsx
│   │   │   ├── AgentOptConfigEditor.tsx
│   │   │   ├── CodexAutocodePanel.tsx
│   │   │   ├── CodexSkillsRulesPanel.tsx
│   │   │   ├── CliToolsSettingsPanel.tsx
│   │   │   ├── ExternalReviewCard.tsx
│   │   │   ├── CliToolsCard.tsx
│   │   │   ├── AntigravityRulesPanel.tsx
│   │   │   └── McpSettingPanel.tsx
│   │   ├── config/
│   │   ├── hooks/
│   │   └── i18n/
│   ├── src-tauri/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── agent_opt.rs
│   │   │   ├── pov_direct.rs
│   │   │   ├── plan_manager.rs
│   │   │   ├── main.rs
│   │   │   ├── proxy.rs
│   │   │   ├── openrouter.rs
│   │   │   ├── reviewer_policy.rs
│   │   │   ├── cli_tools.rs
│   │   │   ├── codex_skills.rs
│   │   │   ├── codex_global_rules.rs
│   │   │   ├── codex_review_settings.rs
│   │   │   ├── antigravity_global_rules.rs
│   │   │   └── model_capabilities.rs
│   │   └── resources/
│   │       ├── codex-skills/
│   │       │   ├── pov-plan-only/
│   │       │   ├── pov-build-review/
│   │       │   └── pov-full-cycle/
│   │       ├── codex-global-rules/
│   │       │   └── managed-block.md
│   │       ├── antigravity-global-rules/
│   │       │   ├── codex-cli.md
│   │       │   └── deepseek-harness.md
│   │       └── antigravity-skills/
│   │           ├── pov-mcp-plan/SKILL.md
│   │           ├── pov-mcp-review/SKILL.md
│   │           ├── pov-mcp-revise/SKILL.md
│   │           ├── pov-full-cycle/SKILL.md
│   │           ├── pov-plan-only/SKILL.md
│   │           └── pov-build-review/SKILL.md
│   └── package.json
├── mcp-server/
│   ├── src/
│   │   ├── lib.rs
│   │   ├── main.rs
│   │   └── provider/
│   └── Cargo.toml
├── scripts/
│   └── verify_doc_parity.ps1
└── LICENSE
```

---

## 2. Running in Development Mode

```bash
cd gui
npm install
npm run tauri dev
```

### Building the Development Variant

On Windows, use a single Rust build job to avoid intermittent compiler memory termination:

```powershell
cd gui
$env:CARGO_BUILD_JOBS = "1"
npm run tauri:build:dev
Remove-Item Env:CARGO_BUILD_JOBS
```

Development builds use:
- Window title: `Poverello Dev`
- Application identity: `com.soheidon.anthro-bridge.dev`
- Port: `4000`
- Data directory: `%APPDATA%\Poverello Dev\`
- Single Binary MCP Server: `target\release\anthro-bridge.exe --mcp-server`

### Standalone MCP Server for Testing

For development and standalone testing, the `mcp-server` crate can still be built as `anthro-bridge-mcp-server.exe`:

```bash
cd mcp-server
cargo build --release
```

Production Antigravity integration uses `anthro-bridge.exe --mcp-server`.
The `anthro-bridge` executable, MCP server name, and application bundle ID are retained compatibility identifiers; the product is Poverello Coding Tools and the app is displayed as Poverello.

---

## 3. Automated Verification

### Frontend Typecheck & Tests

```bash
cd gui
npm run build
npx tsc --noEmit
npx vitest run
```

### Rust Tests

```bash
# GUI / Tauri backend tests
cd gui/src-tauri
cargo check
cargo test

# MCP Server tests
cd mcp-server
cargo test
```

### Multilingual Documentation Parity

```powershell
pwsh -File scripts/verify_doc_parity.ps1
```

### Signed Windows Application Updates

- Stable release builds use `npm run tauri:build:stable`; the Windows NSIS build script requires configured updater signing material and verifies the final installer signature and generated updater manifest. Do not substitute a development or unsigned build for a release artifact.
- The production updater endpoint is GitHub Releases `releases/latest/download/latest.json`. A published release must include the versioned NSIS installer, its matching `.exe.sig`, and `latest.json`. Ensure the manifest URL and versioned `v<version>` tag match the signed installer and release notes. Draft releases are not served by the `latest` endpoint.
- Development builds have updater checks disabled. Do not enable production update behavior in a development build or point it at the production endpoint for staging.
- Run `npm run test:updater` for updater manifest/signature verification tests. Keep signing private-key material and passwords out of command output, logs, and release notes.
- Local production updater signing material (do not commit key or password contents):
  - Key directory: `%USERPROFILE%\.tauri\poverello-release-keys\` (`C:\Users\Sohei\.tauri\poverello-release-keys\`)
  - Private key path: `%USERPROFILE%\.tauri\poverello-release-keys\production.key`
  - Password file path: `%USERPROFILE%\.tauri\poverello-release-keys\.production-password`
  - Public key path: `%USERPROFILE%\.tauri\poverello-release-keys\production.key.pub`
  - Public key ID: `86A10C13103638D7` (`public-key-info.json`)

---

## 4. Manual Verification Checklist

Before creating a release, verify the following:

- [ ] Window controls (minimize, maximize/restore, close) operate smoothly on the custom titlebar.
- [ ] Titlebar workspace tabs (`Poverello` and `MCP`) switch views correctly and cleanly fuse with the header.
- [ ] Dragging works from the title and central spacer; tabs and buttons do not drag the window.
- [ ] Opening Settings and clicking a titlebar tab closes Settings and navigates to the target workspace.
- [ ] Every built-in provider and OpenRouter profile saves and loads routes reliably.
- [ ] Gateway start/stop functions reliably and reports health on `http://127.0.0.1:4000/health`.
- [ ] Copied Claude Code launch commands contain correct base URL and context window parameters.
- [ ] Poverello MCP server connects cleanly to Google Antigravity and executes `poverello/plan`.
- [ ] Codex Autocode reviewer selection and priority persist across navigation; enabling Gemini does not select or launch it.
- [ ] Priority Reviewer arbitrates only valid conflicting verdicts; failed/incomplete reviews and unresolved blocking findings cannot be overridden, and all findings remain visible.
- [ ] DeepSeek reviewer configuration uses the intended headless overlay; documentation does not claim OS-level filesystem/network isolation from snapshot or tool settings alone.
- [ ] Reviewer diagnostic JSONL contains no prompts, response bodies, or credentials; retention/rotation and export filtering match the implementation.
- [ ] Historical Codex Skill template updates distinguish verified releases from unknown/modified files and preserve backups; tests use synthetic directories, not the user's installed Skills.
- [ ] Settings > Codex Skills & Rules preserves content outside Poverello's marked `AGENTS.md` block and reports when `AGENTS.override.md` shadows the managed rules.
- [ ] Configure, save, edit, browse, and reset the plan-folder picker preference independently for Antigravity and Codex. With no Active Plan, confirm it controls only the picker start directory and does not restrict selection to that directory or define the selected plan's repository.
- [ ] On both Autocode surfaces, select a `.md` plan outside the configured picker directory and confirm it associates with the enclosing Git repository; verify the displayed full path/status and that the open action opens the selected file.
- [ ] Confirm each surface keeps its project-plan rows independent, always shows the primary row, allows at most one optional secondary row, and persists both associations across restart. Verify matching project workspace context uses the corresponding row when available and ambiguous/unmatched context does not silently select another project's plan.
- [ ] Remove the optional second row and confirm only its Poverello association is cleared; the project directory and plan file remain unchanged. Confirm the plan-file selection button has no leading file icon on either surface.
- [ ] In Antigravity AgentOpt, verify each project slot has an independent target path, manual target override, enable/disable control, and state. Verify FAST / BALANCED / STRICT persist per slot, Custom opens the editor, and failures do not show unconfirmed success.
- [ ] In Settings > Antigravity Skills & Rules, verify `pov-direct.md` install/repair status. Confirm recognized managed files can be repaired and protected or unrecognized user content is never overwritten.
- [ ] Confirm the picker starts in the active plan's parent folder; with no active plan, it starts in the configured picker preference. Canceling the picker leaves all plan associations unchanged.
- [ ] Settings > CLI Tools shows Node.js/npm prerequisites separately from CLI installation and authentication status; verify that Node.js is not installed automatically.
- [ ] Non-interactive CLI and Node.js/npm status probes do not show console windows, repeated tab navigation reuses bounded cached results, explicit refresh invalidates the relevant cache, and user-requested login remains interactive. Verify that initialization errors such as `0xC0000142` are surfaced rather than treated as success.
- [ ] For a release-candidate Windows build, verify updater runtime status is configured only in the stable signed build, `latest.json` points to the exact versioned `.exe` and matching `.sig`, the manifest and signatures verify offline, and development builds cannot check or install production updates. Do not treat a draft release as discoverable through the `/releases/latest/` endpoint.
