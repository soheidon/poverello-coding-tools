import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import AntigravitySkillsRulesPanel from "./AntigravitySkillsRulesPanel";
import { LanguageContext } from "../i18n";
import type {
  AntigravityCommandsInfo,
  GlobalRuleItemInfo,
  PovNerfCommandsInfo,
} from "../types";

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

const dummyCommandsInfo: AntigravityCommandsInfo = {
  skills_dir: "C:\\Users\\User\\.gemini\\config\\skills",
  plan_command: {
    name: "pov-mcp-plan",
    slash_command: "/pov-mcp-plan",
    status: "installed",
    skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-mcp-plan\\SKILL.md",
  },
  revise_command: {
    name: "pov-mcp-revise",
    slash_command: "/pov-mcp-revise",
    status: "installed",
    skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-mcp-revise\\SKILL.md",
  },
  review_command: {
    name: "pov-mcp-review",
    slash_command: "/pov-mcp-review",
    status: "installed",
    skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-mcp-review\\SKILL.md",
  },
  commands: [
    {
      name: "pov-plan-only",
      slash_command: "/pov-plan-only",
      status: "not_installed",
      skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-plan-only\\SKILL.md",
    },
    {
      name: "pov-build-review",
      slash_command: "/pov-build-review",
      status: "outdated",
      skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-build-review\\SKILL.md",
    },
    {
      name: "pov-full-cycle",
      slash_command: "/pov-full-cycle",
      status: "installed",
      skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-full-cycle\\SKILL.md",
    },
  ],
};

const dummyGlobalRules: GlobalRuleItemInfo[] = [
  {
    id: "codex_cli",
    file_name: "codex-cli.md",
    path: "C:\\Users\\User\\.gemini\\config\\rules\\codex-cli.md",
    status: "not_installed",
    can_remove: true,
    error: null,
  },
  {
    id: "deepseek_harness",
    file_name: "deepseek-harness.md",
    path: "C:\\Users\\User\\.gemini\\config\\rules\\deepseek-harness.md",
    status: "installed",
    can_remove: true,
    error: null,
  },
];

const dummyNerfStatus: PovNerfCommandsInfo = {
  skills_dir: "C:\\Users\\User\\.gemini\\config\\skills",
  skills: [
    {
      skill_id: "balanced",
      slash_command: "/pov-nerf-balanced",
      skill_name: "pov-nerf-balanced",
      state: "installed",
      target_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-nerf-balanced\\SKILL.md",
      permitted_actions: ["remove"],
      diagnostic_reason: null,
    },
    {
      skill_id: "lightweight",
      slash_command: "/pov-nerf-lightweight",
      skill_name: "pov-nerf-lightweight",
      state: "not_installed",
      target_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-nerf-lightweight\\SKILL.md",
      permitted_actions: ["install"],
      diagnostic_reason: null,
    },
  ],
  legacy_single_skill: null,
};

describe("AntigravitySkillsRulesPanel Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_antigravity_commands_status") return dummyCommandsInfo;
      if (command === "install_antigravity_command") return dummyCommandsInfo;
      if (command === "update_antigravity_command") return dummyCommandsInfo;
      if (command === "remove_antigravity_command") return dummyCommandsInfo;
      if (command === "get_cli_tools_status") return [];
      if (command === "get_antigravity_global_rules_status") return dummyGlobalRules;
      if (command === "get_pov_nerf_commands_status") return dummyNerfStatus;
      return null;
    });
  });

  it("renders 3 distribution cards and excludes external review & direct coding", async () => {
    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <AntigravitySkillsRulesPanel />
      </LanguageContext.Provider>
    );

    // 1. Workflow Skills Card
    await waitFor(() => {
      expect(screen.getByText("antigravity.workflowCommandsSectionHeader")).toBeInTheDocument();
      expect(screen.getByText("/pov-plan-only")).toBeInTheDocument();
    });
    expect(screen.getByText("/pov-build-review")).toBeInTheDocument();
    expect(screen.getByText("/pov-full-cycle")).toBeInTheDocument();

    // 2. CLI Tools Card must NOT be in Antigravity Skills & Rules panel (relocated to dedicated CLI Tools panel)
    expect(screen.queryByTestId("cli-tools-card")).not.toBeInTheDocument();

    // 2. Task Presets Card (Nerf)
    expect(screen.getByText("antigravity.taskPresetsHeader")).toBeInTheDocument();
    expect(screen.getByText("/pov-nerf-balanced")).toBeInTheDocument();
    expect(screen.getByText("/pov-nerf-lightweight")).toBeInTheDocument();

    // 4. Global Rules Card
    expect(screen.getByText("antigravity.globalRulesFixedHeader")).toBeInTheDocument();
    expect(screen.getByText("codex-cli.md")).toBeInTheDocument();
    expect(screen.getByText("deepseek-harness.md")).toBeInTheDocument();

    // Excluded from settings
    expect(screen.queryByText("External Review Configuration")).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: /Customize Coding Settings|Poverello Direct Coding/i })).not.toBeInTheDocument();
  });

  it("handles workflow skill actions (Install, Update, Remove)", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <AntigravitySkillsRulesPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByText("/pov-plan-only")).toBeInTheDocument();
    });

    const installBtns = screen.getAllByRole("button", { name: "antigravity.commandBtnInstall" });
    await user.click(installBtns[0]);
    expect(invokeMock).toHaveBeenCalledWith("install_antigravity_command", { name: "pov-plan-only" });

    const updateBtn = screen.getByRole("button", { name: "antigravity.commandBtnUpdate" });
    await user.click(updateBtn);
    expect(screen.getByTestId("skill-repair-modal")).toBeInTheDocument();
    await user.click(screen.getByTestId("skill-repair-proceed"));
    expect(invokeMock).toHaveBeenCalledWith("update_antigravity_command", { name: "pov-build-review" });

    const removeBtns = screen.getAllByRole("button", { name: "antigravity.commandBtnRemove" });
    await user.click(removeBtns[0]);
    expect(invokeMock).toHaveBeenCalledWith("remove_antigravity_command", { name: "pov-full-cycle" });
  });

  it("handles global rule Install and Remove", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <AntigravitySkillsRulesPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByText("codex-cli.md")).toBeInTheDocument();
    });

    const installBtn = screen.getByRole("button", { name: "antigravity.globalRulesBtnInstall" });
    await user.click(installBtn);
    expect(invokeMock).toHaveBeenCalledWith("install_antigravity_global_rule", { id: "codex_cli" });

    const removeBtn = screen.getByRole("button", { name: "antigravity.globalRulesBtnRemove" });
    await user.click(removeBtn);
    expect(invokeMock).toHaveBeenCalledWith("remove_antigravity_global_rule", { id: "deepseek_harness" });
  });

  it("disables global rule remove button when can_remove is false", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_antigravity_commands_status") return dummyCommandsInfo;
      if (cmd === "get_antigravity_global_rules_status") {
        return [
          {
            id: "deepseek_harness",
            file_name: "deepseek-harness.md",
            path: "/path/to/rules/deepseek-harness.md",
            status: "installed",
            can_remove: false,
            error: null,
          },
        ];
      }
      if (cmd === "get_pov_nerf_commands_status") return dummyNerfStatus;
      return null;
    });

    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <AntigravitySkillsRulesPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByText("deepseek-harness.md")).toBeInTheDocument();
    });

    const removeBtn = screen.getByRole("button", { name: "antigravity.globalRulesBtnRemove" });
    expect(removeBtn).toBeDisabled();
  });
});
