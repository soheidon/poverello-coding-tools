import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import AntigravitySkillsRulesPanel from "./AntigravitySkillsRulesPanel";
import { LanguageContext } from "../i18n";
import type {
  AntigravityCommandsInfo,
  ReviewerPolicyConfig,
  WorkflowSkillsPolicyStatus,
  GlobalRuleItemInfo,
  PovNerfCommandsInfo,
  PovDirectStatus,
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

const dummyReviewerPolicy: ReviewerPolicyConfig = {
  schema_version: 1,
  policy: "codex_plus_deepseek",
};

const dummyPolicyStatus: WorkflowSkillsPolicyStatus = {
  compliant: true,
  details: [],
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
      skill_name: "pov-nerf-balanced",
      slash_command: "/pov-nerf-balanced",
      target_path: "C:\\skills\\pov-nerf-balanced\\SKILL.md",
      state: "not_installed",
      permitted_actions: ["install"],
      diagnostic_reason: null,
    },
    {
      skill_id: "lightweight",
      skill_name: "pov-nerf-lightweight",
      slash_command: "/pov-nerf-lightweight",
      target_path: "C:\\skills\\pov-nerf-lightweight\\SKILL.md",
      state: "installed",
      permitted_actions: ["remove"],
      diagnostic_reason: null,
    },
  ],
  legacy_single_skill: null,
};

const dummyDirectStatus: PovDirectStatus = {
  rule_path: "C:\\Users\\User\\.gemini\\config\\rules\\pov-direct.md",
  skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-direct\\SKILL.md",
  skill_present: true,
  state: "managed_synced",
  settings_state: "embedded_valid",
  settings: {
    auto_activation: false,
    web_research: "requested_only",
    response_verbosity: "detailed",
    repository_exploration: "detailed",
    verification: "broad",
  },
  preview_text: "Preview",
  expanded_byte_size: 1000,
  byte_limit: 24000,
  can_apply: true,
  available_operations: ["apply"],
  block_reason: null,
  content_hash: "directhash",
  legacy_sidecar_present: false,
};

describe("AntigravitySkillsRulesPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (
        cmd === "get_antigravity_commands_status" ||
        cmd === "install_antigravity_command" ||
        cmd === "update_antigravity_command" ||
        cmd === "remove_antigravity_command" ||
        cmd === "install_all_antigravity_commands"
      ) {
        return dummyCommandsInfo;
      }
      if (cmd === "get_reviewer_policy") return dummyReviewerPolicy;
      if (cmd === "get_workflow_skills_policy_status") return dummyPolicyStatus;
      if (cmd === "get_antigravity_global_rules_status") return dummyGlobalRules;
      if (cmd === "get_pov_nerf_commands_status") return dummyNerfStatus;
      if (cmd === "get_pov_direct_status") return dummyDirectStatus;
      return null;
    });
  });

  it("renders all 5 cards correctly", async () => {
    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <AntigravitySkillsRulesPanel />
      </LanguageContext.Provider>
    );

    // 1. Workflow Skills Card
    await waitFor(() => {
      expect(screen.getByText("antigravity.workflowCommandsSectionHeader")).toBeInTheDocument();
    });
    expect(screen.getByText("/pov-plan-only")).toBeInTheDocument();
    expect(screen.getByText("/pov-build-review")).toBeInTheDocument();
    expect(screen.getByText("/pov-full-cycle")).toBeInTheDocument();

    // 2. Reviewer Policy Card
    expect(screen.getByText("antigravity.reviewerPolicyTitle")).toBeInTheDocument();
    expect(screen.getByText("antigravity.reviewerPolicyCodexPlusDeepseek")).toBeInTheDocument();
    expect(screen.getByText("antigravity.reviewerPolicyCodexOnly")).toBeInTheDocument();

    // 3. Coding Behavior Card (Direct Coding)
    expect(screen.getAllByRole("heading", { name: /Poverello Direct Coding/i }).length).toBeGreaterThan(0);

    // 4. Task Presets Card (Nerf)
    expect(screen.getByText("antigravity.taskPresetsHeader")).toBeInTheDocument();
    expect(screen.getByText("/pov-nerf-balanced")).toBeInTheDocument();
    expect(screen.getByText("/pov-nerf-lightweight")).toBeInTheDocument();

    // 5. Global Rules Card
    expect(screen.getByText("antigravity.globalRulesFixedHeader")).toBeInTheDocument();
    expect(screen.getByText("codex-cli.md")).toBeInTheDocument();
    expect(screen.getByText("deepseek-harness.md")).toBeInTheDocument();
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

    // pov-plan-only is not_installed -> click Install
    const installBtns = screen.getAllByRole("button", { name: "antigravity.commandBtnInstall" });
    await user.click(installBtns[0]);
    expect(invokeMock).toHaveBeenCalledWith("install_antigravity_command", { name: "pov-plan-only" });

    // pov-build-review is outdated -> click Update
    const updateBtn = screen.getByRole("button", { name: "antigravity.commandBtnUpdate" });
    await user.click(updateBtn);
    expect(invokeMock).toHaveBeenCalledWith("update_antigravity_command", { name: "pov-build-review" });

    // pov-full-cycle is installed -> click Remove
    const removeBtns = screen.getAllByRole("button", { name: "antigravity.commandBtnRemove" });
    await user.click(removeBtns[0]);
    expect(invokeMock).toHaveBeenCalledWith("remove_antigravity_command", { name: "pov-full-cycle" });
  });

  it("handles reviewer policy switching when compliant", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <AntigravitySkillsRulesPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByText("antigravity.reviewerPolicyTitle")).toBeInTheDocument();
    });

    const codexOnlyRadio = screen.getByDisplayValue("codex_only");
    await user.click(codexOnlyRadio);

    expect(invokeMock).toHaveBeenCalledWith("set_reviewer_policy", {
      config: { schema_version: 1, policy: "codex_only", content_hash: null },
      expectedHash: null,
    });
  });

  it("disables reviewer policy options and shows warning when not compliant", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_antigravity_commands_status") return dummyCommandsInfo;
      if (cmd === "get_reviewer_policy") return dummyReviewerPolicy;
      if (cmd === "get_workflow_skills_policy_status") {
        return { compliant: false, details: [dummyCommandsInfo.commands![0]] };
      }
      if (cmd === "get_antigravity_global_rules_status") return dummyGlobalRules;
      if (cmd === "get_pov_nerf_commands_status") return dummyNerfStatus;
      if (cmd === "get_pov_direct_status") return dummyDirectStatus;
      return null;
    });

    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <AntigravitySkillsRulesPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByText(/antigravity\.reviewerPolicyUpdateRequiredNotice/i)).toBeInTheDocument();
    });

    const codexOnlyRadio = screen.getByDisplayValue("codex_only");
    expect(codexOnlyRadio).toBeDisabled();

    const codexDeepSeekRadio = screen.getByDisplayValue("codex_plus_deepseek");
    expect(codexDeepSeekRadio).toBeDisabled();
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

    // codex-cli is not_installed -> click Install
    const installBtn = screen.getByRole("button", { name: "antigravity.globalRulesBtnInstall" });
    await user.click(installBtn);
    expect(invokeMock).toHaveBeenCalledWith("install_antigravity_global_rule", { id: "codex_cli" });

    // deepseek-harness is installed -> click Remove
    const removeBtn = screen.getByRole("button", { name: "antigravity.globalRulesBtnRemove" });
    await user.click(removeBtn);
    expect(invokeMock).toHaveBeenCalledWith("remove_antigravity_global_rule", { id: "deepseek_harness" });
  });

  it("disables global rule remove button when can_remove is false", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_antigravity_commands_status") return dummyCommandsInfo;
      if (cmd === "get_reviewer_policy") return dummyReviewerPolicy;
      if (cmd === "get_workflow_skills_policy_status") return dummyPolicyStatus;
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
      if (cmd === "get_pov_direct_status") return dummyDirectStatus;
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
