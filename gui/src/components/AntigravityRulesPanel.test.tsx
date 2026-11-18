import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import AntigravityRulesPanel from "./AntigravityRulesPanel";
import { getAgentOptErrorPresentation } from "./agentOptErrorPresentation";
import * as i18n from "../i18n";
import { LanguageProvider } from "../i18n";
import { translations as ja } from "../i18n/lang/ja";
import { translations as en } from "../i18n/lang/en";
import { translations as de } from "../i18n/lang/de";
import { translations as es } from "../i18n/lang/es";
import { translations as fr } from "../i18n/lang/fr";
import { translations as ko } from "../i18n/lang/ko";
import { translations as zhCN } from "../i18n/lang/zh-CN";
import { translations as zhTW } from "../i18n/lang/zh-TW";
import type { ActivePlanContext, AgentOptWorkspaceState, NerfEngineOverview, PovDirectStatus } from "../types";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

describe("AntigravityRulesPanel Component", () => {
  const initialStatus: PovDirectStatus = {
    rule_path: "C:\\Users\\Sohei\\.gemini\\config\\rules\\pov-direct.md",
    skill_path: "C:\\Users\\Sohei\\.gemini\\config\\skills\\pov-direct\\SKILL.md",
    skill_present: true,
    state: "managed_synced",
    settings_state: "embedded_valid",
    settings: {
      auto_activation: false,
      web_research: "unrestricted",
      response_verbosity: "standard",
      repository_exploration: "standard",
      verification: "standard",
    },
    preview_text: "Automatic activation: Disabled\nVerification: standard",
    expanded_byte_size: 3450,
    byte_limit: 24000,
    can_apply: true,
    available_operations: ["apply"],
    block_reason: null,
    content_hash: "abc123hash",
    legacy_sidecar_present: false,
  };

  beforeEach(() => {
    vi.restoreAllMocks();
    vi.clearAllMocks();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_pov_direct_status") {
        return initialStatus;
      }
      if (cmd === "save_pov_direct_settings") {
        return initialStatus;
      }
      if (cmd === "open_antigravity_rules_folder") {
        return undefined;
      }
      throw new Error(`Unhandled mock command: ${cmd}`);
    });
  });

  it("renders Operation Mode segments, presets, and exact English descriptions", async () => {
    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <AntigravityRulesPanel />
      </LanguageProvider>
    );

    await waitFor(() => {
      expect(screen.getByText("Antigravity Nerf Settings")).toBeInTheDocument();
      expect(screen.queryByText("Poverello Direct Coding")).not.toBeInTheDocument();
      expect(screen.getByText("Preset")).toBeInTheDocument();
      expect(screen.queryByText("Operation Mode")).not.toBeInTheDocument();
    });

    // Initial status matches Gemini 3.8 Standard preset
    expect(
      screen.getByText(
        "Disables automatic activation (manual invocation only) and applies standard, unrestricted settings across all four axes."
      )
    ).toBeInTheDocument();

    // Switch to Gemini 3.7 Equivalent (Lightweight)
    const lightweightBtn = screen.getByRole("button", { name: "Gemini 3.7 Equivalent (Lightweight)" });
    await user.click(lightweightBtn);

    expect(
      screen.getByText(
        "A Poverello preset modeled after Gemini 3.7 Flash's lightweight coding behavior. Prioritizes direct implementation over expansive exploration and heavy verification (does not change the underlying model)."
      )
    ).toBeInTheDocument();

    // Click Custom preset
    const customBtn = screen.getByRole("button", { name: "Custom" });
    await user.click(customBtn);

    expect(
      screen.getByText(
        "Customized configuration with individual settings for web search, verbosity, exploration, verification, and auto-activation."
      )
    ).toBeInTheDocument();
  });

  it("toggles Advanced Details accordion and renders state indicators", async () => {
    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <AntigravityRulesPanel />
      </LanguageProvider>
    );

    await waitFor(() => {
      expect(screen.getByText("Antigravity Nerf Settings")).toBeInTheDocument();
      expect(screen.queryByText("Poverello Direct Coding")).not.toBeInTheDocument();
      expect(screen.getByText("Preset")).toBeInTheDocument();
      expect(screen.queryByText("Operation Mode")).not.toBeInTheDocument();
    });

    // Accordion is closed by default
    expect(screen.queryByText("Status: Managed and synced")).not.toBeInTheDocument();

    // Click accordion header
    const accordionBtn = screen.getByRole("button", { name: /Advanced Details/i });
    await user.click(accordionBtn);

    // Accordion content is now visible
    expect(screen.getByText("Status: Managed and synced")).toBeInTheDocument();
    // Rule Path is normalized to forward slashes for cross-locale display
    expect(screen.getByText("C:/Users/Sohei/.gemini/config/rules/pov-direct.md")).toBeInTheDocument();
    expect(screen.getByText(/3,450 \/ 24,000 bytes/)).toBeInTheDocument();
    expect(screen.getByText(/Automatic activation: Disabled/)).toBeInTheDocument();
  });

  it("triggers save_pov_direct_settings on Save & Apply click", async () => {
    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <AntigravityRulesPanel />
      </LanguageProvider>
    );

    await waitFor(() => {
      expect(screen.getByText("Antigravity Nerf Settings")).toBeInTheDocument();
      expect(screen.queryByText("Poverello Direct Coding")).not.toBeInTheDocument();
      expect(screen.getByText("Preset")).toBeInTheDocument();
      expect(screen.queryByText("Operation Mode")).not.toBeInTheDocument();
    });

    const saveButton = screen.getByRole("button", { name: "Save & Apply" });
    await user.click(saveButton);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "save_pov_direct_settings",
        expect.objectContaining({
          request: expect.objectContaining({
            operation: "apply",
            expected_state: "managed_synced",
            expected_content_hash: "abc123hash",
            settings: expect.objectContaining({
              auto_activation: false,
              web_research: "unrestricted",
            }),
          }),
        })
      );
    });
  });

  it("opens rules folder when Open Rules Folder is clicked", async () => {
    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <AntigravityRulesPanel />
      </LanguageProvider>
    );

    await waitFor(() => {
      expect(screen.getByText("Antigravity Nerf Settings")).toBeInTheDocument();
      expect(screen.queryByText("Poverello Direct Coding")).not.toBeInTheDocument();
      expect(screen.getByText("Preset")).toBeInTheDocument();
      expect(screen.queryByText("Operation Mode")).not.toBeInTheDocument();
    });

    const rulesActions = screen.getByTestId("poverello-rules-actions");
    expect(rulesActions).toContainElement(screen.getByRole("button", { name: "Open Rules Folder" }));
    expect(rulesActions).toContainElement(screen.getByRole("button", { name: "Reset to Defaults" }));
    expect(rulesActions).toContainElement(screen.getByRole("button", { name: "Save & Apply" }));

    const openFolderButton = screen.getByRole("button", { name: "Open Rules Folder" });
    await user.click(openFolderButton);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("open_antigravity_rules_folder");
    });
  });

  it("normalizes structured Rule Path to forward slashes while strictly preserving Rule Preview backslashes", async () => {
    const statusWithEscapes: PovDirectStatus = {
      ...initialStatus,
      rule_path: "C:\\Windows\\System32\\drivers\\etc\\hosts",
      preview_text: "Regex pattern: \\[A-Z\\]\\\\d+ and path C:\\sample\\path",
    };
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_pov_direct_status") return statusWithEscapes;
      return null;
    });

    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <AntigravityRulesPanel />
      </LanguageProvider>
    );

    await waitFor(() => {
      expect(screen.getByText("Antigravity Nerf Settings")).toBeInTheDocument();
      expect(screen.queryByText("Poverello Direct Coding")).not.toBeInTheDocument();
      expect(screen.getByText("Preset")).toBeInTheDocument();
      expect(screen.queryByText("Operation Mode")).not.toBeInTheDocument();
    });

    // Open Advanced Details
    const accordionBtn = screen.getByRole("button", { name: /Advanced Details/i });
    await user.click(accordionBtn);

    // Rule Path: normalized to forward slashes
    expect(screen.getByText("C:/Windows/System32/drivers/etc/hosts")).toBeInTheDocument();

    // Rule Preview: backslashes MUST be preserved as-is without alteration
    expect(screen.getByText("Regex pattern: \\[A-Z\\]\\\\d+ and path C:\\sample\\path")).toBeInTheDocument();
  });

  it("renders Direct Coding panel in Japanese when ja locale is active", async () => {
    const user = userEvent.setup();
    vi.spyOn(i18n, "useTranslation").mockImplementation(() => ({
      t: (key: string, params?: Record<string, string | number>) => {
        let text = (ja as Record<string, string>)[key] ?? key;
        if (params) {
          for (const [k, v] of Object.entries(params)) {
            text = text.replace(`{${k}}`, String(v));
          }
        }
        return text;
      },
      lang: "ja",
    }) as ReturnType<typeof i18n.useTranslation>);

    render(<AntigravityRulesPanel />);

    await waitFor(() => {
      expect(screen.getByText("Antigravityナーフ設定")).toBeInTheDocument();
      expect(screen.queryByText("Poverello Direct Coding")).not.toBeInTheDocument();
      expect(screen.getByText("プリセット")).toBeInTheDocument();
      expect(screen.queryByText("動作モード")).not.toBeInTheDocument();
    });

    const heading = within(screen.getByTestId("antigravity-rules-heading"));
    expect(heading.getByText("Antigravityナーフ設定")).toBeInTheDocument();
    expect(heading.getByText("Antigravityの過剰な探索・検証を抑え、コーディング作業を効率化します。")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "適用 / 同期" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "ルールフォルダを開く" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "デフォルトに戻す" })).toBeInTheDocument();
    expect(screen.getByText("プリセット")).toBeInTheDocument();
    expect(screen.queryByText("自動適用")).not.toBeInTheDocument();
    expect(screen.queryByText("Web検索")).not.toBeInTheDocument();
    expect(screen.queryByText("応答の冗長性")).not.toBeInTheDocument();
    expect(screen.queryByText("コード探索")).not.toBeInTheDocument();
    expect(screen.queryByText("検証範囲")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /詳細情報/i })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "カスタム" }));

    expect(screen.getByText("自動適用")).toBeInTheDocument();
    expect(screen.getByText("Web検索")).toBeInTheDocument();
    expect(screen.getByText("応答の冗長性")).toBeInTheDocument();
    expect(screen.getByText("コード探索")).toBeInTheDocument();
    expect(screen.getByText("検証範囲")).toBeInTheDocument();
  });

  it("renders Direct Coding panel in English when en locale is active", async () => {
    const user = userEvent.setup();
    vi.spyOn(i18n, "useTranslation").mockImplementation(() => ({
      t: (key: string, params?: Record<string, string | number>) => {
        let text = (en as Record<string, string>)[key] ?? key;
        if (params) {
          for (const [k, v] of Object.entries(params)) {
            text = text.replace(`{${k}}`, String(v));
          }
        }
        return text;
      },
      lang: "en",
    }) as ReturnType<typeof i18n.useTranslation>);

    render(<AntigravityRulesPanel />);

    await waitFor(() => {
      expect(screen.getByText("Antigravity Nerf Settings")).toBeInTheDocument();
      expect(screen.queryByText("Poverello Direct Coding")).not.toBeInTheDocument();
      expect(screen.getByText("Preset")).toBeInTheDocument();
      expect(screen.queryByText("Operation Mode")).not.toBeInTheDocument();
    });

    expect(screen.getByText("Reduce excessive exploration and verification in Antigravity to streamline coding tasks.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save & Apply" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Rules Folder" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Reset to Defaults" })).toBeInTheDocument();
    expect(screen.getByText("Preset")).toBeInTheDocument();
    expect(screen.queryByText("Auto Activation")).not.toBeInTheDocument();
    expect(screen.queryByText("Web Research")).not.toBeInTheDocument();
    expect(screen.queryByText("Response Verbosity")).not.toBeInTheDocument();
    expect(screen.queryByText("Repository Exploration")).not.toBeInTheDocument();
    expect(screen.queryByText("Verification")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Advanced Details/i })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Custom" }));

    expect(screen.getByText("Auto Activation")).toBeInTheDocument();
    expect(screen.getByText("Web Research")).toBeInTheDocument();
    expect(screen.getByText("Response Verbosity")).toBeInTheDocument();
    expect(screen.getByText("Repository Exploration")).toBeInTheDocument();
    expect(screen.getByText("Verification")).toBeInTheDocument();
  });

  it("shows Rows 1-5 only in Custom mode, preserves Custom while editing, and hides on built-in presets or Reset Defaults", async () => {
    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <AntigravityRulesPanel />
      </LanguageProvider>
    );

    await waitFor(() => {
      expect(screen.getByText("Preset")).toBeInTheDocument();
    });

    const assertRowsHidden = () => {
      expect(screen.queryByText("Auto Activation")).not.toBeInTheDocument();
      expect(screen.queryByText("Web Research")).not.toBeInTheDocument();
      expect(screen.queryByText("Response Verbosity")).not.toBeInTheDocument();
      expect(screen.queryByText("Repository Exploration")).not.toBeInTheDocument();
      expect(screen.queryByText("Verification")).not.toBeInTheDocument();
    };

    const assertRowsVisible = () => {
      expect(screen.getByText("Auto Activation")).toBeInTheDocument();
      expect(screen.getByText("Web Research")).toBeInTheDocument();
      expect(screen.getByText("Response Verbosity")).toBeInTheDocument();
      expect(screen.getByText("Repository Exploration")).toBeInTheDocument();
      expect(screen.getByText("Verification")).toBeInTheDocument();
    };

    // 1. Gemini 3.8 Standard (initial): Rows 1-5 hidden, chrome & Advanced Details visible
    assertRowsHidden();
    expect(screen.getByRole("button", { name: /Advanced Details/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save & Apply" })).toBeInTheDocument();

    // 2. Gemini 3.7 Equivalent (Lightweight): Rows 1-5 hidden
    await user.click(screen.getByRole("button", { name: "Gemini 3.7 Equivalent (Lightweight)" }));
    assertRowsHidden();
    expect(screen.getByRole("button", { name: /Advanced Details/i })).toBeInTheDocument();

    // 3. Custom selected: Rows 1-5 visible, Custom button active, Custom description shown
    const customBtn = screen.getByRole("button", { name: "Custom" });
    await user.click(customBtn);
    assertRowsVisible();
    expect(customBtn).toHaveClass("active");
    expect(
      screen.getByText(
        "Customized configuration with individual settings for web search, verbosity, exploration, verification, and auto-activation."
      )
    ).toBeInTheDocument();

    // 4. Editing individual settings while Custom is explicitly selected keeps Custom active
    //    even when edited to match a built-in preset's exact values (Gemini 3.8 Standard)
    await user.click(screen.getByRole("button", { name: "Disabled" }));
    await user.click(screen.getByRole("button", { name: "Unrestricted" }));
    const standardButtons = screen.getAllByRole("button", { name: "Standard" });
    expect(standardButtons).toHaveLength(3);
    for (const btn of standardButtons) {
      await user.click(btn);
    }
    expect(customBtn).toHaveClass("active");
    expect(screen.getByRole("button", { name: "Gemini 3.8 Standard" })).not.toHaveClass("active");
    expect(
      screen.getByText(
        "Customized configuration with individual settings for web search, verbosity, exploration, verification, and auto-activation."
      )
    ).toBeInTheDocument();
    assertRowsVisible();

    // 5. Selecting built-in preset clears Custom and hides Rows 1-5
    await user.click(screen.getByRole("button", { name: "Gemini 3.8 Standard" }));
    expect(customBtn).not.toHaveClass("active");
    assertRowsHidden();

    // 6. Re-open Custom, then Reset to Defaults -> returns to Standard and hides Rows 1-5
    await user.click(customBtn);
    assertRowsVisible();
    await user.click(screen.getByRole("button", { name: "Reset to Defaults" }));
    expect(screen.getByRole("button", { name: "Gemini 3.8 Standard" })).toHaveClass("active");
    expect(customBtn).not.toHaveClass("active");
    assertRowsHidden();
  });

  it("applies AgentOpt to both configured projects, independent of the active plan context", async () => {
    const user = userEvent.setup();
    vi.spyOn(i18n, "useTranslation").mockImplementation(() => ({
      t: (key: string, params?: Record<string, string | number>) => {
        let text = (en as Record<string, string>)[key] ?? key;
        if (params) {
          for (const [name, value] of Object.entries(params)) text = text.replace(`{${name}}`, String(value));
        }
        return text;
      },
      lang: "en",
    }) as ReturnType<typeof i18n.useTranslation>);
    const planInspection = {
      workspace_path: "C:\\repo-a",
      registered: true,
      state: "enabled" as const,
      antigravity_plugin_present: true,
      config_present: true,
      hooks_enabled: true,
      config: { raw_valid: true, schema_supported: true, enabled: true, mode: "BALANCED", raw: {}, error: null },
      unregistered_files_detected: false,
      detail: null,
    };
    const overview: NerfEngineOverview = {
      persisted_engine: "agentopt",
      workspace_mode: "follow_registered_projects",
      manual_workspace: null,
      active_plan_workspace: "C:\\repo-a",
      selected_workspace: "C:\\repo-a",
      registered_workspaces: ["C:\\repo-a", "C:\\repo-b"],
      workspace_targets: [
        { project_slot_id: "primary", repository_path: "\\\\?\\C:\\repo-a", workspace_path: "C:\\repo-a", plan_path: ".plan/one.md", use_agent_opt: true, manual_workspace: null, inspection: planInspection },
        { project_slot_id: "secondary", repository_path: "\\\\?\\C:\\repo-b", workspace_path: "C:\\repo-b", plan_path: ".plan/two.md", use_agent_opt: true, manual_workspace: null, inspection: { ...planInspection, workspace_path: "C:\\repo-b" } },
      ],
      target_resolution_errors: [],
      cli: {
        installed: true,
        version: "0.1.0",
        supported_version: true,
        cli_path: "agent-opt",
        node_version: "v22.0.0",
        node_supported: true,
        npm_available: true,
        pinned_version: "0.1.0",
        error: null,
      },
      workspace_inspection: planInspection,
      pov_direct_status: { ...initialStatus, state: "absent" },
      mismatch: {
        has_mismatch: false,
        reason: null,
        observed_rule_state: "absent",
        enabled_workspaces: ["C:\\repo-a", "C:\\repo-b"],
        selected_workspace_state: "enabled",
      },
      exclusivity_scope_note: "",
    };
    let activeOverview = overview;
    mockInvoke.mockImplementation(async (cmd: string, args?: any) => {
      if (cmd === "get_pov_direct_status") return initialStatus;
      if (cmd === "get_antigravity_nerf_engine_overview") return activeOverview;
      if (cmd === "apply_selected_antigravity_nerf_engine") {
        return { overview: activeOverview, changed: false, message: "Already active" };
      }
      if (cmd === "switch_antigravity_nerf_engine") {
        return { overview: { ...activeOverview, persisted_engine: args.targetEngine }, changed: true, message: "Switched" };
      }
      if (cmd === "set_agent_opt_project_preference") {
        const targets = activeOverview.workspace_targets.map((target) => target.project_slot_id === args.slotId
          ? { ...target, use_agent_opt: args.useAgentOpt, inspection: { ...target.inspection, state: (args.useAgentOpt ? "enabled" : "disabled") as AgentOptWorkspaceState, hooks_enabled: args.useAgentOpt, config: { raw_valid: true, schema_supported: true, enabled: args.useAgentOpt, mode: target.inspection.config?.mode ?? null, raw: target.inspection.config?.raw ?? null, error: null } } }
          : target);
        activeOverview = { ...activeOverview, workspace_targets: targets };
        return activeOverview;
      }
      if (cmd === "select_project_folder_dialog") return "C:\\manual-project-one";
      if (cmd === "set_agent_opt_project_manual_workspace") {
        const targets = activeOverview.workspace_targets.map((target) => target.project_slot_id === args.slotId
          ? { ...target, manual_workspace: args.manualWorkspace, workspace_path: args.manualWorkspace ?? target.repository_path, inspection: { ...target.inspection, workspace_path: args.manualWorkspace ?? target.repository_path } }
          : target);
        activeOverview = { ...activeOverview, workspace_targets: targets };
        return activeOverview;
      }
      throw new Error(`Unhandled mock command: ${cmd}`);
    });

    const context: ActivePlanContext = {
      surface: "antigravity",
      repoRoot: "C:\\repo-a",
      slotId: "primary",
      planPath: ".plan/one.md",
      resolved: true,
    };
    const { rerender } = render(
      <LanguageProvider>
        <AntigravityRulesPanel activePlanContext={context} />
      </LanguageProvider>
    );
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("apply_selected_antigravity_nerf_engine");
    });
    expect(screen.getByTestId("nerf-engine-selector")).toHaveClass("pov-engine-tabs");
    expect(screen.getByRole("tab", { name: "AgentOpt" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tab", { name: "Poverello Rules" })).toHaveAttribute("aria-selected", "false");
    expect(screen.getByRole("tabpanel")).toHaveAttribute("aria-labelledby", "engine-tab-agentopt");
    expect(screen.getByTestId("agentopt-project-primary")).toHaveTextContent(en["agentOpt.projectTitle"].replace("{number}", "1"));
    expect(screen.getByTestId("agentopt-project-secondary")).toHaveTextContent(en["agentOpt.projectTitle"].replace("{number}", "2"));
    expect(screen.getByTestId("agentopt-project-directory-primary")).toHaveValue("C:\\repo-a");
    expect(screen.getByTestId("agentopt-project-directory-secondary")).toHaveValue("C:\\repo-b");
    expect(screen.getByTestId("agentopt-project-directory-primary")).toHaveAttribute("readonly");
    expect(screen.getByTestId("agentopt-project-directory-secondary")).toHaveAttribute("readonly");
    expect(screen.getByTestId("agentopt-workspace-state-primary")).toHaveTextContent(en["agentOpt.state.enabled"]);
    expect(screen.getByTestId("agentopt-workspace-state-secondary")).toHaveTextContent(en["agentOpt.state.enabled"]);
    const primaryControls = screen.getByTestId("agentopt-project-controls-primary");
    expect(primaryControls.firstElementChild).toBe(screen.getByTestId("agentopt-project-directory-primary"));
    expect(primaryControls.children[1]).toContainElement(screen.getByTestId("agentopt-manual-select-primary"));
    expect(primaryControls.children[2]).toBe(screen.getByTestId("agentopt-project-action-primary"));
    expect(primaryControls.children[3]).toBe(screen.getByTestId("agentopt-workspace-state-primary"));
    expect(within(screen.getByTestId("agentopt-project-primary")).getByRole("button", { name: en["agentOpt.disableProject"] })).toBeInTheDocument();
    expect(within(screen.getByTestId("agentopt-project-secondary")).getByRole("button", { name: en["agentOpt.disableProject"] })).toBeInTheDocument();
    expect(screen.queryByTestId("agentopt-workspace-input-primary")).not.toBeInTheDocument();
    expect(screen.queryByTestId("agentopt-manual-workspace-editor-primary")).not.toBeInTheDocument();
    await user.click(screen.getByTestId("agentopt-project-directory-primary"));
    expect(mockInvoke).not.toHaveBeenCalledWith("select_project_folder_dialog");
    await user.click(screen.getByTestId("agentopt-manual-select-primary"));
    expect(mockInvoke).toHaveBeenCalledWith("select_project_folder_dialog");
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("set_agent_opt_project_manual_workspace", { slotId: "primary", manualWorkspace: "C:\\manual-project-one" }));
    expect(screen.getByTestId("agentopt-project-directory-primary")).toHaveValue("C:\\manual-project-one");
    expect(screen.getByTestId("agentopt-project-directory-secondary")).toHaveValue("C:\\repo-b");
    await user.click(screen.getByTestId("agentopt-manual-select-primary"));
    expect(screen.getByTestId("agentopt-manual-menu-primary")).toBeInTheDocument();
    await user.click(within(screen.getByTestId("agentopt-manual-menu-primary")).getByRole("button", { name: en["agentOpt.followProjectFolder"] }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("set_agent_opt_project_manual_workspace", { slotId: "primary", manualWorkspace: null }));
    expect(screen.getByTestId("agentopt-project-directory-primary")).toHaveValue("C:\\repo-a");
    await user.click(screen.getByTestId("agentopt-project-action-primary"));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("set_agent_opt_project_preference", { slotId: "primary", useAgentOpt: false }));
    expect(mockInvoke).not.toHaveBeenCalledWith("set_agent_opt_project_preference", { slotId: "secondary", useAgentOpt: false });
    await waitFor(() => expect(within(screen.getByTestId("agentopt-project-primary")).getByRole("button", { name: en["agentOpt.enableProject"] })).toBeInTheDocument());
    expect(screen.getByTestId("agentopt-workspace-state-primary")).toHaveTextContent(en["agentOpt.state.disabled"]);
    await user.click(screen.getByTestId("agentopt-project-action-primary"));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("set_agent_opt_project_preference", { slotId: "primary", useAgentOpt: true }));
    expect(screen.getByTestId("agentopt-workspace-state-primary")).toHaveTextContent(en["agentOpt.state.enabled"]);
    expect(mockInvoke.mock.calls.filter(([cmd]) => cmd === "apply_selected_antigravity_nerf_engine")).toHaveLength(1);

    rerender(
      <LanguageProvider>
        <AntigravityRulesPanel activePlanContext={{ ...context, planPath: ".plan/two.md" }} />
      </LanguageProvider>
    );
    expect(mockInvoke.mock.calls.filter(([cmd]) => cmd === "apply_selected_antigravity_nerf_engine")).toHaveLength(1);

    rerender(
      <LanguageProvider>
        <AntigravityRulesPanel activePlanContext={{ ...context, repoRoot: "C:\\repo-b", planPath: ".plan/other.md" }} />
      </LanguageProvider>
    );
    await waitFor(() => {
      expect(mockInvoke.mock.calls.filter(([cmd]) => cmd === "apply_selected_antigravity_nerf_engine")).toHaveLength(2);
    });
    screen.getByRole("tab", { name: "AgentOpt" }).focus();
    await user.keyboard("{ArrowLeft}");
    await waitFor(() => {
      expect(screen.getByRole("tab", { name: "Poverello Rules" })).toHaveFocus();
      expect(screen.getByRole("tab", { name: "Poverello Rules" })).toHaveAttribute("aria-selected", "true");
      expect(mockInvoke).toHaveBeenCalledWith("switch_antigravity_nerf_engine", { targetEngine: "poverello_rules" });
    });
  });

  it.each([
    { lang: "en" as const, translations: en, hasExistingFiles: true, nextAction: en["agentOpt.registerWorkspace"] },
    { lang: "ja" as const, translations: ja, hasExistingFiles: false, nextAction: ja["agentOpt.initializeWorkspace"] },
  ])("shows one clear target action for an unregistered AgentOpt workspace in $lang", async ({ lang, translations, hasExistingFiles, nextAction }) => {
    const user = userEvent.setup();
    vi.spyOn(i18n, "useTranslation").mockImplementation(() => ({
      t: (key: string, params?: Record<string, string | number>) => {
        let text = (translations as Record<string, string>)[key] ?? key;
        if (params) {
          for (const [name, value] of Object.entries(params)) {
            text = text.replace(`{${name}}`, String(value));
          }
        }
        return text;
      },
      lang,
    }) as ReturnType<typeof i18n.useTranslation>);
    const workspacePath = "\\\\?\\C:\\manual-workspace";
    const rawError = `Cannot switch to AgentOpt: Project manual workspace '${workspacePath}' is not registered. Register it explicitly first.`;
    const inspection = {
      workspace_path: workspacePath,
      registered: false,
      state: hasExistingFiles ? "disabled" as const : "not_initialized" as const,
      antigravity_plugin_present: hasExistingFiles,
      config_present: hasExistingFiles,
      config: hasExistingFiles
        ? { raw_valid: true, schema_supported: true, enabled: false, mode: "BALANCED", raw: {}, error: null }
        : null,
      unregistered_files_detected: hasExistingFiles,
      detail: hasExistingFiles ? null : "AgentOpt is not initialized for Antigravity in this workspace",
    };
    const overview: NerfEngineOverview = {
      persisted_engine: "poverello_rules",
      workspace_mode: "manual_override",
      manual_workspace: workspacePath,
      active_plan_workspace: workspacePath,
      selected_workspace: workspacePath,
      registered_workspaces: [],
      workspace_targets: [{ project_slot_id: "primary", repository_path: "C:\\repo", workspace_path: workspacePath, plan_path: null, use_agent_opt: false, manual_workspace: workspacePath, inspection }],
      target_resolution_errors: [rawError],
      cli: {
        installed: true,
        version: "0.1.0",
        supported_version: true,
        cli_path: "agent-opt",
        node_version: "v22.0.0",
        node_supported: true,
        npm_available: true,
        pinned_version: "0.1.0",
        error: null,
      },
      workspace_inspection: inspection,
      pov_direct_status: { ...initialStatus, state: "absent" },
      mismatch: {
        has_mismatch: false,
        reason: null,
        observed_rule_state: "absent",
        enabled_workspaces: [],
        selected_workspace_state: inspection.state,
      },
      exclusivity_scope_note: "",
    };
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_pov_direct_status") return initialStatus;
      if (cmd === "get_antigravity_nerf_engine_overview") return overview;
      if (cmd === "switch_antigravity_nerf_engine") throw rawError;
      throw new Error(`Unhandled mock command: ${cmd}`);
    });

    render(<AntigravityRulesPanel />);
    await waitFor(() => expect(screen.getByTestId("engine-btn-agentopt")).toBeInTheDocument());
    await user.click(screen.getByTestId("engine-btn-agentopt"));

    await waitFor(() => expect(screen.getByTestId("agentopt-target-section")).toBeInTheDocument());
    expect(screen.queryByTestId("poverello-rules-actions")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: translations["antigravityRules.actions.openFolder"] })).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByTestId("agentopt-project-primary")).toHaveTextContent(translations["agentOpt.projectTitle"].replace("{number}", "1"));
    expect(screen.getByTestId("agentopt-project-directory-primary")).toHaveValue("C:\\manual-workspace");
    expect(screen.getByRole("button", { name: nextAction })).toBeInTheDocument();
    if (lang === "ja") {
      expect(screen.getByTestId("agentopt-workspace-inspection")).toHaveTextContent(
        ja["agentOpt.detailNotInitialized"]
      );
      expect(screen.getByTestId("agentopt-workspace-state-primary")).toHaveTextContent(translations["agentOpt.state.not_initialized"]);
      await user.click(screen.getByRole("button", { name: nextAction }));
      expect(screen.getByTestId("agentopt-init-confirm")).toHaveTextContent(
        "このプロジェクトにAgentOptの設定を作成しますか？"
      );
      expect(screen.getByRole("button", { name: "このフォルダーに設定を作成" })).toBeInTheDocument();
      expect(mockInvoke).not.toHaveBeenCalledWith("init_agent_opt_workspace", expect.anything());
    }
  });

  it("provides explicit localized next-step translations in all supported locales", () => {
    const locales = [en, ja, de, es, fr, ko, zhCN, zhTW];
    const keys = ["agentOpt.workspaceNotRegistered", "agentOpt.workspaceNotRegisteredSteps", "agentOpt.legacyManualMigrationRequired", "agentOpt.legacyManualMigrationSteps", "agentOpt.manualSelect", "agentOpt.requiresAgentOptEngine"] as const;
    for (const locale of locales) {
      for (const key of keys) {
        expect(locale[key]).toBeTruthy();
        if (locale !== en) expect(locale[key]).not.toBe(en[key]);
      }
    }
  });

  it("preserves unrelated AgentOpt diagnostics instead of mislabeling them", () => {
    const diagnostic = "AgentOpt CLI process timed out after 30 seconds";
    const presented = getAgentOptErrorPresentation(diagnostic, (key) => en[key]);
    expect(presented).toEqual({ message: diagnostic, guidance: null });
  });

  it("reconciles AgentOpt when the active project changes despite a legacy manual setting", async () => {
    const manualOverview: NerfEngineOverview = {
      persisted_engine: "agentopt",
      workspace_mode: "manual_override",
      manual_workspace: "C:\\manual-workspace",
      active_plan_workspace: "C:\\repo-a",
      selected_workspace: "C:\\manual-workspace",
      registered_workspaces: ["C:\\manual-workspace"],
      workspace_targets: [],
      target_resolution_errors: [],
      cli: {
        installed: true,
        version: "0.1.0",
        supported_version: true,
        cli_path: "agent-opt",
        node_version: "v22.0.0",
        node_supported: true,
        npm_available: true,
        pinned_version: "0.1.0",
        error: null,
      },
      workspace_inspection: null,
      pov_direct_status: { ...initialStatus, state: "absent" },
      mismatch: {
        has_mismatch: false,
        reason: null,
        observed_rule_state: "absent",
        enabled_workspaces: ["C:\\manual-workspace"],
        selected_workspace_state: "enabled",
      },
      exclusivity_scope_note: "",
    };
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_pov_direct_status") return initialStatus;
      if (cmd === "get_antigravity_nerf_engine_overview") return manualOverview;
      if (cmd === "apply_selected_antigravity_nerf_engine") {
        return { overview: manualOverview, changed: false, message: "Already active" };
      }
      throw new Error(`Unhandled mock command: ${cmd}`);
    });
    const context: ActivePlanContext = {
      surface: "antigravity",
      repoRoot: "C:\\repo-a",
      slotId: "primary",
      planPath: ".plan/one.md",
      resolved: true,
    };
    const { rerender } = render(
      <LanguageProvider>
        <AntigravityRulesPanel activePlanContext={context} />
      </LanguageProvider>
    );
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_antigravity_nerf_engine_overview");
    });
    rerender(
      <LanguageProvider>
        <AntigravityRulesPanel activePlanContext={{ ...context, repoRoot: "C:\\repo-b", planPath: ".plan/two.md" }} />
      </LanguageProvider>
    );
    await waitFor(() => {
      expect(mockInvoke.mock.calls.filter(([cmd]) => cmd === "apply_selected_antigravity_nerf_engine")).toHaveLength(2);
    });
    expect(mockInvoke).not.toHaveBeenCalledWith("set_antigravity_agent_opt_workspace_selection", expect.anything());
  });

  it("edits per-project AgentOpt settings independently with validation, save, cancel, and error retention", async () => {
    const rawA: Record<string, unknown> = {
      enabled: true,
      mode: "FAST",
      context: { enabled: true, maxRankedFiles: 8, maxRepoMapTokens: 650, deduplicateReads: true, blockRedundantFullReadsAfter: 2, minQuerySpecificityForInjection: true },
      verification: { enabled: false, enforceOnStop: false, autoRunFastTestsOnStop: false, maxStopBlocksPerSession: 0, requireTestsWhenCodeModified: true, exemptDocOnlyChanges: true },
      failureDetection: { enabled: true, maxRepeatedCommandFailures: 2, maxIdenticalErrors: 2, maxRedundantReadsOfUnchangedFile: 2, detectEditOscillation: true },
      memory: { enabled: true, maxEntries: 50, minConfidenceToInject: 0.7, autoPruneContradictions: true },
    };
    const rawB: Record<string, unknown> = {
      ...rawA,
      mode: "BALANCED",
      context: { ...(rawA.context as Record<string, unknown>), maxRepoMapTokens: 700 },
    };
    let currentA = structuredClone(rawA);
    let failSave = false;
    let badPresetReadback = false;

    const makeOverview = (): NerfEngineOverview => ({
      persisted_engine: "agentopt",
      workspace_mode: "follow_registered_projects",
      manual_workspace: null,
      active_plan_workspace: "C:\\repo-a",
      selected_workspace: "C:\\repo-a",
      registered_workspaces: ["C:\\repo-a", "C:\\repo-b"],
      workspace_targets: [
        {
          project_slot_id: "primary",
          repository_path: "C:\\repo-a",
          workspace_path: "C:\\repo-a",
          plan_path: ".plan/one.md",
          use_agent_opt: true,
          manual_workspace: null,
          inspection: {
            workspace_path: "C:\\repo-a",
            registered: true,
            state: "enabled",
            antigravity_plugin_present: true,
            config_present: true,
            config: { raw_valid: true, schema_supported: true, enabled: true, mode: String(currentA.mode), raw: currentA, error: null },
            unregistered_files_detected: false,
            detail: null,
          },
        },
        {
          project_slot_id: "secondary",
          repository_path: "C:\\repo-b",
          workspace_path: "C:\\repo-b",
          plan_path: ".plan/two.md",
          use_agent_opt: true,
          manual_workspace: null,
          inspection: {
            workspace_path: "C:\\repo-b",
            registered: true,
            state: "enabled",
            antigravity_plugin_present: true,
            config_present: true,
            config: { raw_valid: true, schema_supported: true, enabled: true, mode: "BALANCED", raw: rawB, error: null },
            unregistered_files_detected: false,
            detail: null,
          },
        },
      ],
      target_resolution_errors: [],
      cli: {
        installed: true,
        version: "0.1.0",
        supported_version: true,
        cli_path: "agent-opt",
        node_version: "v24.0.0",
        node_supported: true,
        npm_available: true,
        pinned_version: "0.1.0",
        error: null,
      },
      workspace_inspection: null,
      pov_direct_status: { ...initialStatus, state: "absent" },
      mismatch: {
        has_mismatch: false,
        reason: null,
        observed_rule_state: "absent",
        enabled_workspaces: ["C:\\repo-a", "C:\\repo-b"],
        selected_workspace_state: "enabled",
      },
      exclusivity_scope_note: "",
    });

    mockInvoke.mockImplementation(async (cmd: string, args?: any) => {
      if (cmd === "get_pov_direct_status") return initialStatus;
      if (cmd === "get_antigravity_nerf_engine_overview") return makeOverview();
      if (cmd === "update_agent_opt_config") {
        if (failSave) throw new Error("Injected disk error");
        expect(args?.workspacePath).toBe("C:\\repo-a");
        expect(args?.patch?.enabled).toBeUndefined();
        if (badPresetReadback && args?.patch?.mode === "BALANCED") {
          return {
            ...makeOverview().workspace_targets![0].inspection,
            config: { ...makeOverview().workspace_targets![0].inspection.config!, raw_valid: false, raw: null },
          };
        }
        currentA = {
          ...currentA,
          ...args.patch,
          verification: { ...(currentA.verification as Record<string, unknown>), ...(args.patch?.verification ?? {}) },
        };
        return makeOverview().workspace_targets![0].inspection;
      }
      return null;
    });

    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <AntigravityRulesPanel />
      </LanguageProvider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("agentopt-config-presets-primary")).toBeInTheDocument();
      expect(screen.getByTestId("agentopt-config-presets-secondary")).toBeInTheDocument();
    });

    // Presets are visible before opening Custom; selecting Custom opens only Project 1.
    expect(screen.queryByTestId("agentopt-config-body-primary")).not.toBeInTheDocument();
    expect(screen.getByTestId("agentopt-config-mode-primary-FAST")).toHaveClass("active");
    await user.click(screen.getByTestId("agentopt-config-mode-primary-CUSTOM"));
    expect(screen.getByTestId("agentopt-config-body-primary")).toBeInTheDocument();
    expect(screen.queryByTestId("agentopt-config-body-secondary")).not.toBeInTheDocument();
    expect(screen.getByTestId("agentopt-cfg-primary-maxRankedFiles")).toHaveValue(8);
    expect(mockInvoke).not.toHaveBeenCalledWith("update_agent_opt_config", expect.anything());

    // Custom edits save as ordinary config values without changing the base mode.
    const rankedFilesInput = screen.getByTestId("agentopt-cfg-primary-maxRankedFiles");
    await user.clear(rankedFilesInput);
    await user.type(rankedFilesInput, "10");
    await user.click(screen.getByTestId("agentopt-config-save-primary"));
    await waitFor(() => expect(currentA.context).toMatchObject({ maxRankedFiles: 10 }));
    expect(currentA.mode).toBe("FAST");

    // Invalid value (0 for maxRepoMapTokens) blocks save and keeps draft
    const tokensInput = screen.getByTestId("agentopt-cfg-primary-maxRepoMapTokens");
    await user.clear(tokensInput);
    await user.type(tokensInput, "0");
    const updateCallsBeforeInvalidSave = mockInvoke.mock.calls.filter(([cmd]) => cmd === "update_agent_opt_config").length;
    await user.click(screen.getByTestId("agentopt-config-save-primary"));
    expect(screen.getByTestId("agentopt-config-error-primary")).toBeInTheDocument();
    expect(mockInvoke.mock.calls.filter(([cmd]) => cmd === "update_agent_opt_config")).toHaveLength(updateCallsBeforeInvalidSave);

    // Discard resets back to 650
    await user.click(screen.getByTestId("agentopt-config-cancel-primary"));
    expect((screen.getByTestId("agentopt-cfg-primary-maxRepoMapTokens") as HTMLInputElement).value).toBe("650");

    // A direct preset applies immediately and reports command failure without optimistic selection.
    failSave = true;
    await user.click(screen.getByTestId("agentopt-config-mode-primary-BALANCED"));
    await waitFor(() => expect(screen.getByTestId("agentopt-config-error-primary")).toHaveTextContent("Injected disk error"));
    expect(currentA.mode).toBe("FAST");
    expect(screen.getByTestId("agentopt-config-mode-primary-FAST")).toHaveClass("active");
    expect(screen.getByTestId("agentopt-config-mode-primary-BALANCED")).not.toHaveClass("active");

    // A successful command without a valid persisted read-back is not treated as success.
    failSave = false;
    badPresetReadback = true;
    await user.click(screen.getByTestId("agentopt-config-mode-primary-BALANCED"));
    await waitFor(() => expect(screen.getByTestId("agentopt-config-error-primary")).toHaveTextContent(/did not match|一致しません/i));
    expect(currentA.mode).toBe("FAST");
    expect(screen.getByTestId("agentopt-config-mode-primary-FAST")).toHaveClass("active");
    expect(screen.queryByTestId("agentopt-config-success-primary")).not.toBeInTheDocument();

    // Retry applies the built-in preset immediately, changing only its owned values.
    badPresetReadback = false;
    await user.click(screen.getByTestId("agentopt-config-mode-primary-BALANCED"));
    await waitFor(() => expect(currentA.mode).toBe("BALANCED"));
    expect(screen.getByTestId("agentopt-config-mode-primary-BALANCED")).toHaveClass("active");
    expect(currentA.context).toMatchObject({ maxRankedFiles: 10, maxRepoMapTokens: 650 });
    expect(currentA.verification).toMatchObject({ requireTestsWhenCodeModified: true, exemptDocOnlyChanges: true });
    const updateCalls = mockInvoke.mock.calls.filter(([cmd]) => cmd === "update_agent_opt_config");
    const balancedCall = updateCalls[updateCalls.length - 1]?.[1] as { patch?: Record<string, unknown> };
    expect(balancedCall.patch).toEqual({
      mode: "BALANCED",
      verification: { enabled: true, enforceOnStop: true, autoRunFastTestsOnStop: true, maxStopBlocksPerSession: 2 },
    });

    // Custom edits retain normal Save/Cancel behavior and disable direct presets while dirty.
    await user.click(screen.getByTestId("agentopt-config-mode-primary-CUSTOM"));
    const updatedTokensInput = screen.getByTestId("agentopt-cfg-primary-maxRepoMapTokens");
    await user.clear(updatedTokensInput);
    await user.type(updatedTokensInput, "900");
    expect(screen.getByTestId("agentopt-config-unsaved-primary")).toBeInTheDocument();
    expect(screen.getByTestId("agentopt-config-mode-primary-STRICT")).toBeDisabled();

    failSave = true;
    await user.click(screen.getByTestId("agentopt-config-save-primary"));
    await waitFor(() => {
      expect(screen.getByTestId("agentopt-config-error-primary")).toHaveTextContent("Injected disk error");
    });
    expect(screen.queryByTestId("agentopt-config-success-primary")).not.toBeInTheDocument();
    expect((screen.getByTestId("agentopt-cfg-primary-maxRepoMapTokens") as HTMLInputElement).value).toBe("900");
    expect(screen.getByTestId("agentopt-config-mode-primary-BALANCED")).toHaveClass("active");

    // Retry save succeeds and leaves Project 2 untouched
    failSave = false;
    await user.click(screen.getByTestId("agentopt-config-save-primary"));
    await waitFor(() => {
      expect(screen.getByTestId("agentopt-config-success-primary")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("agentopt-config-unsaved-primary")).not.toBeInTheDocument();
    expect(screen.getByTestId("agentopt-config-mode-primary-BALANCED")).toHaveClass("active");
    expect(currentA).toMatchObject({
      mode: "BALANCED",
      context: { maxRankedFiles: 10, maxRepoMapTokens: 900 },
      verification: {
        enabled: true,
        enforceOnStop: true,
        autoRunFastTestsOnStop: true,
        maxStopBlocksPerSession: 2,
        requireTestsWhenCodeModified: true,
        exemptDocOnlyChanges: true,
      },
    });
    await user.click(screen.getByTestId("agentopt-config-mode-primary-STRICT"));
    await waitFor(() => expect(currentA.mode).toBe("STRICT"));
    expect(currentA.verification).toMatchObject({ enabled: true, enforceOnStop: true, autoRunFastTestsOnStop: false, maxStopBlocksPerSession: 2 });

    await user.click(screen.getByTestId("agentopt-config-mode-primary-FAST"));
    await waitFor(() => expect(currentA.mode).toBe("FAST"));
    expect(currentA.verification).toMatchObject({ enabled: false, enforceOnStop: false, autoRunFastTestsOnStop: false, maxStopBlocksPerSession: 0 });
    // Project 2's saved configuration remains unchanged and opens only from its Custom action.
    await user.click(screen.getByTestId("agentopt-config-mode-secondary-CUSTOM"));
    expect((screen.getByTestId("agentopt-cfg-secondary-maxRepoMapTokens") as HTMLInputElement).value).toBe("700");
  });

  it("detects partial field persistence mismatch and surfaces/reconciles config.enabled mismatch with project toggle", async () => {
    const rawA: Record<string, unknown> = {
      enabled: false, // contradicts use_agent_opt: true
      mode: "FAST",
      context: { enabled: true, maxRankedFiles: 8, maxRepoMapTokens: 650, deduplicateReads: true, blockRedundantFullReadsAfter: 2, minQuerySpecificityForInjection: true },
      verification: { enabled: false, enforceOnStop: false, autoRunFastTestsOnStop: false, maxStopBlocksPerSession: 0, requireTestsWhenCodeModified: true, exemptDocOnlyChanges: true },
      failureDetection: { enabled: true, maxRepeatedCommandFailures: 2, maxIdenticalErrors: 2, maxRedundantReadsOfUnchangedFile: 2, detectEditOscillation: true },
      memory: { enabled: true, maxEntries: 50, minConfidenceToInject: 0.7, autoPruneContradictions: true },
    };
    let currentA = structuredClone(rawA);

    const makeOverview = (): NerfEngineOverview => ({
      persisted_engine: "agentopt",
      workspace_mode: "follow_registered_projects",
      manual_workspace: null,
      active_plan_workspace: "C:\\repo-a",
      selected_workspace: "C:\\repo-a",
      registered_workspaces: ["C:\\repo-a"],
      workspace_targets: [
        {
          project_slot_id: "primary",
          repository_path: "C:\\repo-a",
          workspace_path: "C:\\repo-a",
          plan_path: ".plan/one.md",
          use_agent_opt: true,
          manual_workspace: null,
          inspection: {
            workspace_path: "C:\\repo-a",
            registered: true,
            state: currentA.enabled ? "enabled" : "disabled",
            antigravity_plugin_present: true,
            config_present: true,
            config: { raw_valid: true, schema_supported: true, enabled: Boolean(currentA.enabled), mode: String(currentA.mode), raw: currentA, error: null },
            unregistered_files_detected: false,
            detail: null,
          },
        },
      ],
      target_resolution_errors: [],
      cli: {
        installed: true,
        version: "0.1.0",
        supported_version: true,
        cli_path: "agent-opt",
        node_version: "v24.0.0",
        node_supported: true,
        npm_available: true,
        pinned_version: "0.1.0",
        error: null,
      },
      workspace_inspection: null,
      pov_direct_status: { ...initialStatus, state: "absent" },
      mismatch: {
        has_mismatch: false,
        reason: null,
        observed_rule_state: "absent",
        enabled_workspaces: [],
        selected_workspace_state: "disabled",
      },
      exclusivity_scope_note: "",
    });

    mockInvoke.mockImplementation(async (cmd: string, args?: any) => {
      if (cmd === "get_pov_direct_status") return initialStatus;
      if (cmd === "get_antigravity_nerf_engine_overview") return makeOverview();
      if (cmd === "apply_selected_antigravity_nerf_engine") {
        currentA = { ...currentA, enabled: true };
        return { overview: makeOverview(), changed: true, message: "Reconciled" };
      }
      if (cmd === "update_agent_opt_config") {
        // Silently drop context.deduplicateReads change to verify full-field mismatch detection
        const patched = structuredClone(args.patch);
        patched.context.deduplicateReads = true;
        currentA = { ...currentA, ...patched };
        return makeOverview().workspace_targets![0].inspection;
      }
      return null;
    });

    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <AntigravityRulesPanel />
      </LanguageProvider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("agentopt-enabled-mismatch-primary")).toBeInTheDocument();
    });
    await user.click(screen.getByTestId("agentopt-reconcile-enabled-primary"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("apply_selected_antigravity_nerf_engine");
      expect(screen.queryByTestId("agentopt-enabled-mismatch-primary")).not.toBeInTheDocument();
    });

    await user.click(screen.getByTestId("agentopt-config-mode-primary-CUSTOM"));
    await user.click(screen.getByTestId("agentopt-cfg-primary-deduplicateReads"));
    await user.click(screen.getByTestId("agentopt-config-save-primary"));
    await waitFor(() => {
      expect(screen.getByTestId("agentopt-config-error-primary")).toHaveTextContent(
        /did not match|一致しません/i
      );
    });
    expect(screen.queryByTestId("agentopt-config-success-primary")).not.toBeInTheDocument();
  });
});
