import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import CodexSkillsRulesPanel from "./CodexSkillsRulesPanel";
import { LanguageContext } from "../i18n";
import type { CodexSkillsStatus, CodexGlobalRulesStatus } from "../types";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

const dummySkillsStatus: CodexSkillsStatus = {
  skills_dir: "C:\\Users\\Sohei\\.agents\\skills",
  skills: [
    {
      name: "pov-plan-only",
      display_name: "Plan Only (/pov-plan-only)",
      description: "計画作成と独立計画レビュー専用Skill",
      state: "Installed",
      path: "C:\\Users\\Sohei\\.agents\\skills\\pov-plan-only",
      discovery_verified: false,
      detail_message: null,
      has_backup: false,
    },
    {
      name: "pov-build-review",
      display_name: "Build & Review (/pov-build-review)",
      description: "承認済み計画に基づく実装・テストSkill",
      state: "UpdateAvailable",
      path: "C:\\Users\\Sohei\\.agents\\skills\\pov-build-review",
      discovery_verified: false,
      detail_message: "Template update available",
      has_backup: false,
    },
    {
      name: "pov-full-cycle",
      display_name: "Full Cycle (/pov-full-cycle)",
      description: "フルサイクル管理Skill",
      state: "NotInstalled",
      path: "C:\\Users\\Sohei\\.agents\\skills\\pov-full-cycle",
      discovery_verified: false,
      detail_message: null,
      has_backup: false,
    },
  ],
  resolution_error: null,
};

const dummyRulesStatus: CodexGlobalRulesStatus = {
  codex_home: "C:\\Users\\Sohei\\.codex",
  agents_md_path: "C:\\Users\\Sohei\\.codex\\AGENTS.md",
  state: "Installed",
  is_shadowed: false,
  shadowed_by: null,
  detail_message: null,
  resolution_error: null,
};

describe("CodexSkillsRulesPanel Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_codex_skills_status") {
        return dummySkillsStatus;
      }
      if (command === "get_codex_global_rules_status") {
        return dummyRulesStatus;
      }
      if (command === "open_codex_skills_folder") {
        return undefined;
      }
      if (command === "open_codex_home_folder") {
        return undefined;
      }
      if (command === "get_codex_skill_template") {
        const payload = args as { name: string };
        return `---
name: ${payload.name}
description: Skill description
---
# Template content for ${payload.name}`;
      }
      if (command === "get_codex_global_rules_template") {
        return `<!-- BEGIN POVERELLO MANAGED RULES -->
# Global Rules Content
<!-- END POVERELLO MANAGED RULES -->`;
      }
      if (command === "install_codex_skill") {
        return {
          ...dummySkillsStatus.skills[2],
          state: "Installed",
        };
      }
      if (command === "update_codex_skill") {
        return {
          ...dummySkillsStatus.skills[1],
          state: "Installed",
        };
      }
      if (command === "remove_codex_skill") {
        return {
          ...dummySkillsStatus.skills[0],
          state: "NotInstalled",
        };
      }
      if (command === "install_or_update_codex_global_rules") {
        return {
          ...dummyRulesStatus,
          state: "Installed",
        };
      }
      if (command === "remove_codex_global_rules") {
        return {
          ...dummyRulesStatus,
          state: "NotInstalled",
        };
      }
      return undefined;
    });
  });

  it("renders Codex Workflow Skills card and Global Rules card", async () => {
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexSkillsRulesPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("codex-skills-card")).toBeInTheDocument();
      expect(screen.getByTestId("codex-global-rules-card")).toBeInTheDocument();
    });

    expect(screen.getByTestId("skill-item-pov-plan-only")).toHaveTextContent("Plan Only (/pov-plan-only)");
    expect(screen.getByTestId("skill-status-pov-plan-only").textContent).toMatch(/Installed|stateInstalled/i);

    expect(screen.getByTestId("skill-item-pov-build-review").textContent).toMatch(/Update Available|stateUpdateAvailable/i);
    expect(screen.getByTestId("skill-item-pov-full-cycle").textContent).toMatch(/Not Installed|stateNotInstalled/i);

    expect(screen.getByTestId("rules-status").textContent).toMatch(/Active|rulesStateInstalled/i);
  });

  it("opens skills folder and codex home on button clicks", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexSkillsRulesPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-open-skills-folder")).toBeInTheDocument();
      expect(screen.getByTestId("btn-open-codex-home")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("btn-open-skills-folder"));
    expect(mockInvoke).toHaveBeenCalledWith("open_codex_skills_folder");

    await user.click(screen.getByTestId("btn-open-codex-home"));
    expect(mockInvoke).toHaveBeenCalledWith("open_codex_home_folder");
  });

  it("handles install workflow skill with confirmation modal", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexSkillsRulesPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-install-pov-full-cycle")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("btn-install-pov-full-cycle"));

    expect(screen.getByTestId("modal-confirm-install-skill")).toBeInTheDocument();
    await user.click(screen.getByTestId("modal-proceed-install-skill"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("install_codex_skill", { name: "pov-full-cycle" });
    });
  });

  it("handles remove workflow skill with confirmation modal", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexSkillsRulesPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-remove-pov-plan-only")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("btn-remove-pov-plan-only"));

    expect(screen.getByTestId("modal-confirm-remove-skill")).toBeInTheDocument();
    expect(screen.getByTestId("modal-confirm-remove-skill")).toHaveTextContent("quarantine");

    await user.click(screen.getByTestId("modal-proceed-remove-skill"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("remove_codex_skill", { name: "pov-plan-only" });
    });
  });

  it("displays warning banner when global rules are shadowed by AGENTS.override.md", async () => {
    mockInvoke.mockImplementation(async (command: string) => {
      if (command === "get_codex_skills_status") return dummySkillsStatus;
      if (command === "get_codex_global_rules_status") {
        return {
          ...dummyRulesStatus,
          state: "Shadowed",
          is_shadowed: true,
          shadowed_by: "C:\\Users\\Sohei\\.codex\\AGENTS.override.md",
        };
      }
      return undefined;
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexSkillsRulesPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("rules-shadowed-warning")).toBeInTheDocument();
    });

    expect(screen.getByTestId("rules-shadowed-warning").textContent).toMatch(/AGENTS\.override\.md/);
    expect(screen.getByTestId("rules-status").textContent).toMatch(/Shadowed|codexGlobalRulesStatusShadowed/i);
  });

  it("reloads skills and rules on reload button clicks", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <CodexSkillsRulesPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-reload-skills")).toBeInTheDocument();
      expect(screen.getByTestId("btn-reload-rules")).toBeInTheDocument();
    });

    expect(screen.getByTestId("btn-reload-skills").textContent).toMatch(/再読み込み|btnReload|Reload/);
    expect(screen.getByTestId("btn-reload-rules").textContent).toMatch(/再読み込み|btnReload|Reload/);

    await user.click(screen.getByTestId("btn-reload-skills"));
    expect(mockInvoke).toHaveBeenCalledWith("get_codex_skills_status");

    await user.click(screen.getByTestId("btn-reload-rules"));
    expect(mockInvoke).toHaveBeenCalledWith("get_codex_global_rules_status");
  });

  it("previews skill template and global rules template in modals", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexSkillsRulesPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-preview-pov-plan-only")).toBeInTheDocument();
      expect(screen.getByTestId("btn-preview-rules")).toBeInTheDocument();
    });

    // Preview skill
    await user.click(screen.getByTestId("btn-preview-pov-plan-only"));
    expect(mockInvoke).toHaveBeenCalledWith("get_codex_skill_template", { name: "pov-plan-only" });
    await waitFor(() => {
      expect(screen.getByTestId("modal-template-preview")).toBeInTheDocument();
      expect(screen.getByTestId("modal-preview-content")).toHaveTextContent("Template content for pov-plan-only");
    });

    // Close preview
    await user.click(screen.getByTestId("modal-preview-close"));
    expect(screen.queryByTestId("modal-template-preview")).toBeNull();

    // Preview rules
    await user.click(screen.getByTestId("btn-preview-rules"));
    expect(mockInvoke).toHaveBeenCalledWith("get_codex_global_rules_template");
    await waitFor(() => {
      expect(screen.getByTestId("modal-template-preview")).toBeInTheDocument();
      expect(screen.getByTestId("modal-preview-content")).toHaveTextContent("BEGIN POVERELLO MANAGED RULES");
    });
    await user.click(screen.getByTestId("modal-preview-close-x"));
    expect(screen.queryByTestId("modal-template-preview")).toBeNull();
  });

  it("protects conflicted skills by disabling update button", async () => {
    mockInvoke.mockImplementation(async (command: string) => {
      if (command === "get_codex_skills_status") {
        return {
          ...dummySkillsStatus,
          skills: [
            {
              name: "pov-plan-only",
              display_name: "Plan Only (/pov-plan-only)",
              description: "計画作成と独立計画レビュー専用Skill",
              state: "ProtectedConflict",
              path: "C:\\Users\\Sohei\\.agents\\skills\\pov-plan-only",
              discovery_verified: false,
              detail_message: "Skill files contain custom modifications and do not match any known official version. Direct update blocked.",
              has_backup: false,
            },
          ],
        };
      }
      if (command === "get_codex_global_rules_status") return dummyRulesStatus;
      return undefined;
    });

    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <CodexSkillsRulesPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("skill-item-pov-plan-only")).toBeInTheDocument();
    });

    // Update button should be disabled for ProtectedConflict
    const updateBtn = screen.getByTestId("btn-update-pov-plan-only");
    expect(updateBtn).toBeDisabled();
    expect(screen.getByTestId("skill-item-pov-plan-only")).toHaveTextContent("Direct update blocked");
  });

  it("localizes unmanaged rules notice message in Japanese", async () => {
    mockInvoke.mockImplementation(async (command: string) => {
      if (command === "get_codex_skills_status") return dummySkillsStatus;
      if (command === "get_codex_global_rules_status") {
        return {
          ...dummyRulesStatus,
          state: "NotInstalled",
          detail_message: "AGENTS.md exists but contains no Poverello managed rules block. Existing content will be preserved on install.",
        };
      }
      return undefined;
    });

    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <CodexSkillsRulesPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("codex-global-rules-card")).toBeInTheDocument();
    });

    expect(screen.getByTestId("codex-global-rules-card").textContent).toMatch(
      /rulesUnmanagedNotice|AGENTS\.md は存在しますが/
    );
    expect(screen.getByTestId("btn-open-codex-home").textContent).toMatch(
      /openCodexHomeBtn|Codexフォルダを開く/
    );
  });

  it("does not render Review Settings card and does not invoke review settings commands", async () => {
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexSkillsRulesPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("codex-skills-card")).toBeInTheDocument();
      expect(screen.getByTestId("codex-global-rules-card")).toBeInTheDocument();
    });

    // Review Settings card must NOT be present
    expect(screen.queryByTestId("codex-review-settings-card")).toBeNull();

    // Must never invoke review settings commands from Settings
    expect(mockInvoke).not.toHaveBeenCalledWith("get_codex_review_settings");
    expect(mockInvoke).not.toHaveBeenCalledWith("get_cli_tools_status");
    expect(mockInvoke).not.toHaveBeenCalledWith("save_codex_review_settings", expect.anything());
  });
});
