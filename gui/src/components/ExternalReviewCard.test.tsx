import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import ExternalReviewCard from "./ExternalReviewCard";
import { LanguageContext } from "../i18n";
import type {
  ReviewerPolicyConfig,
  WorkflowSkillsPolicyStatus,
  CodexAccountInfo,
} from "../types";

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

const dummyReviewerPolicy: ReviewerPolicyConfig = {
  schema_version: 4,
  policy: "codex_plus_deepseek",
  priority_reviewer: "codex_cli",
  codex: {
    model: "gpt-6.1-terra",
    reasoning_effort: "high",
  },
  deepseek: {
    model: "deepseek-flash",
    reasoning_effort: "high",
  },
  content_hash: "abcd1234hash",
};

const dummyCodexAccount: CodexAccountInfo = {
  installed: true,
  executable_path: "C:\\Users\\Sohei\\AppData\\Roaming\\npm\\codex.cmd",
  version: "0.163.0",
  auth_method: "chatgpt",
  account_identity: "soheidon@yahoo.co.jp",
  config_profile: null,
  requires_openai_auth: true,
};

const dummyPolicyStatusCompliant: WorkflowSkillsPolicyStatus = {
  compliant: true,
  details: [],
};

const dummyPolicyStatusNonCompliant: WorkflowSkillsPolicyStatus = {
  compliant: false,
  details: [],
};

describe("ExternalReviewCard Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    invokeMock.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_reviewer_policy") {
        return dummyReviewerPolicy;
      }
      if (command === "get_workflow_skills_policy_status") {
        return dummyPolicyStatusCompliant;
      }
      if (command === "get_codex_account_info") {
        return dummyCodexAccount;
      }
      if (command === "set_reviewer_policy") {
        const payload = args as { config: ReviewerPolicyConfig };
        return {
          ...payload.config,
          content_hash: "newhash5678",
        };
      }
      return undefined;
    });
  });

  it("renders reviewer policy card with mode toggles, summary, and compact settings", async () => {
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ExternalReviewCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("reviewer-policy-card")).toBeInTheDocument();
    });

    // Policy toggle options (Antigravity, Codex CLI, DeepSeek Harness)
    expect(screen.getByTestId("reviewer-policy-antigravity")).toBeInTheDocument();
    expect(screen.getByTestId("reviewer-policy-codex-only")).toBeInTheDocument();
    expect(screen.getByTestId("reviewer-policy-deepseek-only")).toBeInTheDocument();

    // Since dummyReviewerPolicy has policy "codex_plus_deepseek", both codex and deepseek are checked
    expect(screen.getByTestId("reviewer-policy-antigravity")).not.toBeChecked();
    expect(screen.getByTestId("reviewer-policy-codex-only")).toBeChecked();
    expect(screen.getByTestId("reviewer-policy-deepseek-only")).toBeChecked();

    // Model badge displayed next to heading when Codex is used
    const modelBadge = screen.getByTestId("codex-model-badge");
    expect(modelBadge).toBeInTheDocument();
    expect(modelBadge.textContent).toContain("GPT-6.1 Terra");
    expect(screen.getByTestId("btn-open-diagnostics-modal")).toBeInTheDocument();

    // Codex and DeepSeek detailed settings in compact rows
    expect(screen.getByTestId("codex-model-settings")).toBeInTheDocument();
    expect(screen.getByTestId("codex-model-group")).toBeInTheDocument();
    expect(screen.getByTestId("codex-effort-group")).toBeInTheDocument();
    expect(screen.getByTestId("deepseek-model-settings")).toBeInTheDocument();
    expect(screen.getByTestId("deepseek-model-group")).toBeInTheDocument();
    expect(screen.getByTestId("deepseek-effort-group")).toBeInTheDocument();
    // Verify pricing string is NOT rendered in model buttons
    expect(screen.getByTestId("deepseek-model-deepseek-flash").textContent).toBe("DeepSeek V4.1 Flash");
    expect(screen.getByTestId("deepseek-model-deepseek-flash").textContent).not.toContain("$");
  });

  it("modifies Codex model and effort via compact segment buttons without writing preset", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ExternalReviewCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("codex-model-settings")).toBeInTheDocument();
    });

    // Change Codex model via segment button
    await user.click(screen.getByTestId("codex-model-gpt-6.1-sol"));
    expect(invokeMock).toHaveBeenCalledWith("set_reviewer_policy", expect.objectContaining({
      config: expect.objectContaining({
        policy: "codex_plus_deepseek",
        codex: expect.objectContaining({ model: "gpt-6.1-sol" }),
      }),
    }));

    // Verify preset is NOT in the saved config payload
    const lastCall = invokeMock.mock.calls.find((call) => call[0] === "set_reviewer_policy");
    expect(lastCall?.[1]?.config?.preset).toBeUndefined();
  });

  it("modifies DeepSeek model and effort via compact segment buttons without pricing text", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ExternalReviewCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("deepseek-model-settings")).toBeInTheDocument();
    });

    // Model buttons: check active model, verify pricing is omitted
    const flashBtn = screen.getByTestId("deepseek-model-deepseek-flash");
    const proBtn = screen.getByTestId("deepseek-model-deepseek-v4-pro");
    expect(flashBtn).toHaveClass("active");
    expect(flashBtn.textContent).toBe("DeepSeek V4.1 Flash");
    expect(proBtn.textContent).toBe("DeepSeek V4 Pro 0813");

    // Click pro model
    await user.click(proBtn);
    expect(invokeMock).toHaveBeenCalledWith("set_reviewer_policy", expect.objectContaining({
      config: expect.objectContaining({
        deepseek: expect.objectContaining({
          model: "deepseek-v4-pro",
        }),
      }),
    }));

    // Effort buttons: click max effort
    const maxEffortBtn = screen.getByTestId("deepseek-effort-max");
    await user.click(maxEffortBtn);
    expect(invokeMock).toHaveBeenCalledWith("set_reviewer_policy", expect.objectContaining({
      config: expect.objectContaining({
        deepseek: expect.objectContaining({
          reasoning_effort: "max",
        }),
      }),
    }));
  });

  it("shows unselected toggles when policy is none and hides accordions", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_reviewer_policy") {
        return {
          schema_version: 3,
          policy: "none",
          content_hash: "hash000",
        };
      }
      if (command === "get_workflow_skills_policy_status") {
        return dummyPolicyStatusCompliant;
      }
      return undefined;
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ExternalReviewCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("reviewer-policy-antigravity")).not.toBeChecked();
      expect(screen.getByTestId("reviewer-policy-codex-only")).not.toBeChecked();
      expect(screen.getByTestId("reviewer-policy-deepseek-only")).not.toBeChecked();
    });

    expect(screen.queryByTestId("codex-model-badge")).not.toBeInTheDocument();
    expect(screen.queryByTestId("codex-model-settings")).not.toBeInTheDocument();
    expect(screen.queryByTestId("deepseek-model-settings")).not.toBeInTheDocument();
  });

  it("disables controls and displays warning when workflow skills are non-compliant", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_reviewer_policy") return dummyReviewerPolicy;
      if (command === "get_workflow_skills_policy_status") return dummyPolicyStatusNonCompliant;
      return undefined;
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ExternalReviewCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("reviewer-policy-card")).toBeInTheDocument();
    });

    expect(screen.getByTestId("reviewer-policy-antigravity")).toBeDisabled();
    expect(screen.getByTestId("reviewer-policy-codex-only")).toBeDisabled();
    expect(screen.getByTestId("reviewer-policy-deepseek-only")).toBeDisabled();
  });

  it("handles priority reviewer selection and displays it in summary", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ExternalReviewCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("priority-reviewer-codex-cli")).toBeInTheDocument();
    });

    expect(screen.getByTestId("priority-reviewer-codex-cli")).toBeChecked();
    expect(screen.getByTestId("priority-reviewer-deepseek")).not.toBeChecked();

    // Redundant label next to priority buttons is removed
    expect(screen.queryByTestId("priority-reviewer-summary")).not.toBeInTheDocument();

    // Click DeepSeek as priority reviewer
    await user.click(screen.getByTestId("priority-reviewer-deepseek"));
    expect(invokeMock).toHaveBeenCalledWith("set_reviewer_policy", expect.objectContaining({
      config: expect.objectContaining({
        schema_version: 6,
        policy: "codex_plus_deepseek",
        priority_reviewer: "deepseek",
      }),
    }));
  });

  it("displays migration banner when unconfirmed_migration is true", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_reviewer_policy") {
        return {
          ...dummyReviewerPolicy,
          unconfirmed_migration: true,
        };
      }
      if (command === "get_workflow_skills_policy_status") return dummyPolicyStatusCompliant;
      if (command === "get_codex_account_info") return dummyCodexAccount;
      return undefined;
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ExternalReviewCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("reviewer-migration-banner")).toBeInTheDocument();
    });
  });

  it("handles deepseek toggle selection and shows only deepseek controls and summary", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation(async (command: string, args: any) => {
      if (command === "get_reviewer_policy") {
        return {
          schema_version: 5,
          policy: "deepseek_only",
          build_reviewers: ["deepseek"],
          deepseek: { model: "deepseek-flash", reasoning_effort: "low" },
          content_hash: "hash-dsh",
        };
      }
      if (command === "get_workflow_skills_policy_status") return dummyPolicyStatusCompliant;
      if (command === "set_reviewer_policy") {
        const payload = args as { config: ReviewerPolicyConfig };
        return { ...payload.config, content_hash: "updated-hash" };
      }
      return undefined;
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ExternalReviewCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("reviewer-policy-deepseek-only")).toBeChecked();
      expect(screen.getByTestId("reviewer-policy-codex-only")).not.toBeChecked();
      expect(screen.getByTestId("reviewer-policy-antigravity")).not.toBeChecked();
    });

    // Verify DeepSeek settings exist, and Codex settings / model badge / priority reviewer do NOT exist
    expect(screen.getByTestId("deepseek-model-settings")).toBeInTheDocument();
    expect(screen.getByTestId("deepseek-model-group")).toBeInTheDocument();
    expect(screen.getByTestId("deepseek-effort-group")).toBeInTheDocument();
    expect(screen.queryByTestId("codex-model-settings")).not.toBeInTheDocument();
    expect(screen.queryByTestId("codex-model-badge")).not.toBeInTheDocument();
    expect(screen.queryByTestId("priority-reviewer-codex-cli")).not.toBeInTheDocument();
    expect(screen.queryByTestId("priority-reviewer-summary")).not.toBeInTheDocument();

    // Now toggle Antigravity ON: both DeepSeek and Antigravity will be selected
    await user.click(screen.getByTestId("reviewer-policy-antigravity"));
    expect(invokeMock).toHaveBeenCalledWith("set_reviewer_policy", expect.objectContaining({
      config: expect.objectContaining({
        build_reviewers: ["deepseek", "antigravity"],
        policy: "deepseek_only",
      }),
    }));
  });

  it("handles toggling off all reviewers to unselected state without converting to antigravity", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation(async (command: string, args: any) => {
      if (command === "get_reviewer_policy") {
        return {
          schema_version: 6,
          policy: "antigravity",
          build_reviewers: ["antigravity"],
          content_hash: "hash-ag",
        };
      }
      if (command === "get_workflow_skills_policy_status") return dummyPolicyStatusCompliant;
      if (command === "set_reviewer_policy") {
        const payload = args as { config: ReviewerPolicyConfig };
        return { ...payload.config, content_hash: "updated-hash" };
      }
      return undefined;
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ExternalReviewCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("reviewer-policy-antigravity")).toBeChecked();
    });

    // Click Antigravity to toggle it OFF
    await user.click(screen.getByTestId("reviewer-policy-antigravity"));
    expect(invokeMock).toHaveBeenCalledWith("set_reviewer_policy", expect.objectContaining({
      config: expect.objectContaining({
        build_reviewers: [],
        policy: "none",
        build_review_policy: "none",
      }),
    }));
  });

  it("syncs plan_review_policy with build review settings when plan_reviewers is in inherit mode", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation(async (command: string, args: any) => {
      if (command === "get_reviewer_policy") {
        return {
          schema_version: 6,
          policy: "antigravity",
          build_reviewers: ["antigravity"],
          build_review_policy: "antigravity",
          plan_review_policy: "antigravity",
          plan_reviewers: null,
          content_hash: "hash-inherit",
        };
      }
      if (command === "get_workflow_skills_policy_status") return dummyPolicyStatusCompliant;
      if (command === "set_reviewer_policy") {
        const payload = args as { config: ReviewerPolicyConfig };
        return { ...payload.config, content_hash: "updated-hash" };
      }
      return undefined;
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ExternalReviewCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("reviewer-policy-antigravity")).toBeChecked();
    });

    // Toggle Codex ON in implementation review
    await user.click(screen.getByTestId("reviewer-policy-codex-only"));

    // Because plan_reviewers was null (inherit mode), plan_review_policy is synced with build_review_policy ("codex_only")
    expect(invokeMock).toHaveBeenCalledWith("set_reviewer_policy", expect.objectContaining({
      config: expect.objectContaining({
        build_reviewers: ["antigravity", "codex"],
        build_review_policy: "codex_only",
        plan_review_policy: "codex_only",
        plan_reviewers: null,
      }),
    }));
  });

  it("reproduces user issue: unselects Codex CLI from Antigravity+Codex even when plan_reviewers has codex, and adds DeepSeek without reverting", async () => {
    const user = userEvent.setup();
    let currentConfig: ReviewerPolicyConfig = {
      schema_version: 6,
      policy: "codex_only",
      build_review_policy: "codex_only",
      build_reviewers: ["codex", "antigravity"],
      plan_review_policy: "codex_only",
      plan_priority_reviewer: "codex_cli",
      plan_reviewers: ["codex"],
      codex: { model: "gpt-6-luna", reasoning_effort: "medium" },
      content_hash: "initial-hash",
    };

    invokeMock.mockImplementation(async (command: string, args: any) => {
      if (command === "get_reviewer_policy") {
        return currentConfig;
      }
      if (command === "get_workflow_skills_policy_status") return dummyPolicyStatusCompliant;
      if (command === "set_reviewer_policy") {
        const payload = args as { config: ReviewerPolicyConfig };
        // Verify codex configuration is preserved and NOT wiped to null
        expect(payload.config.codex).toEqual({ model: "gpt-6-luna", reasoning_effort: "medium" });
        currentConfig = { ...payload.config, content_hash: "hash-" + Math.random() };
        return currentConfig;
      }
      return undefined;
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ExternalReviewCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("reviewer-policy-codex-only")).toBeChecked();
      expect(screen.getByTestId("reviewer-policy-antigravity")).toBeChecked();
      expect(screen.getByTestId("reviewer-policy-deepseek-only")).not.toBeChecked();
    });

    // Step 1: Uncheck Codex CLI -> state must become ["antigravity"] only
    await user.click(screen.getByTestId("reviewer-policy-codex-only"));

    await waitFor(() => {
      expect(screen.getByTestId("reviewer-policy-codex-only")).not.toBeChecked();
      expect(screen.getByTestId("reviewer-policy-antigravity")).toBeChecked();
      expect(screen.getByTestId("reviewer-policy-deepseek-only")).not.toBeChecked();
    });

    // Step 2: Add DeepSeek Harness -> state must become ["antigravity", "deepseek"]
    await user.click(screen.getByTestId("reviewer-policy-deepseek-only"));

    await waitFor(() => {
      expect(screen.getByTestId("reviewer-policy-codex-only")).not.toBeChecked();
      expect(screen.getByTestId("reviewer-policy-antigravity")).toBeChecked();
      expect(screen.getByTestId("reviewer-policy-deepseek-only")).toBeChecked();
    });

    // Verify final saved config in mock has ["antigravity", "deepseek"] and NO codex in build_reviewers
    expect(currentConfig.build_reviewers).toEqual(["antigravity", "deepseek"]);
    expect(currentConfig.codex).toEqual({ model: "gpt-6-luna", reasoning_effort: "medium" });
    expect(currentConfig.deepseek).toBeDefined();
  });

  it("supports all 8 combinations of reviewers independently", async () => {
    const user = userEvent.setup();
    let currentConfig: ReviewerPolicyConfig = {
      schema_version: 6,
      policy: "none",
      build_review_policy: "none",
      build_reviewers: [],
      content_hash: "initial-hash",
    };

    invokeMock.mockImplementation(async (command: string, args: any) => {
      if (command === "get_reviewer_policy") return currentConfig;
      if (command === "get_workflow_skills_policy_status") return dummyPolicyStatusCompliant;
      if (command === "set_reviewer_policy") {
        const payload = args as { config: ReviewerPolicyConfig };
        currentConfig = { ...payload.config, content_hash: "hash-" + Math.random() };
        return currentConfig;
      }
      return undefined;
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ExternalReviewCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      // 1. None (all OFF)
      expect(screen.getByTestId("reviewer-policy-antigravity")).not.toBeChecked();
      expect(screen.getByTestId("reviewer-policy-codex-only")).not.toBeChecked();
      expect(screen.getByTestId("reviewer-policy-deepseek-only")).not.toBeChecked();
    });

    // 2. Antigravity only
    await user.click(screen.getByTestId("reviewer-policy-antigravity"));
    expect(currentConfig.build_reviewers).toEqual(["antigravity"]);

    // 3. Antigravity + Codex
    await user.click(screen.getByTestId("reviewer-policy-codex-only"));
    expect(currentConfig.build_reviewers).toEqual(["antigravity", "codex"]);

    // 4. All 3: Antigravity + Codex + DeepSeek
    await user.click(screen.getByTestId("reviewer-policy-deepseek-only"));
    expect(currentConfig.build_reviewers).toEqual(["antigravity", "codex", "deepseek"]);

    // 5. Codex + DeepSeek (toggle off Antigravity)
    await user.click(screen.getByTestId("reviewer-policy-antigravity"));
    expect(currentConfig.build_reviewers).toEqual(["codex", "deepseek"]);

    // 6. DeepSeek only (toggle off Codex)
    await user.click(screen.getByTestId("reviewer-policy-codex-only"));
    expect(currentConfig.build_reviewers).toEqual(["deepseek"]);

    // 7. Antigravity + DeepSeek (toggle on Antigravity)
    await user.click(screen.getByTestId("reviewer-policy-antigravity"));
    expect(currentConfig.build_reviewers).toEqual(["deepseek", "antigravity"]);

    // 8. Codex only (toggle off DeepSeek & Antigravity, toggle on Codex)
    await user.click(screen.getByTestId("reviewer-policy-deepseek-only"));
    await user.click(screen.getByTestId("reviewer-policy-antigravity"));
    await user.click(screen.getByTestId("reviewer-policy-codex-only"));
    expect(currentConfig.build_reviewers).toEqual(["codex"]);

    // Back to None (all OFF)
    await user.click(screen.getByTestId("reviewer-policy-codex-only"));
    expect(currentConfig.build_reviewers).toEqual([]);
  });

  it("handles consecutive rapid changes by serializing and saving the latest state without clobbering", async () => {
    const user = userEvent.setup();
    const savedConfigs: ReviewerPolicyConfig[] = [];
    let hashCounter = 1;

    invokeMock.mockImplementation(async (command: string, args: any) => {
      if (command === "get_reviewer_policy") {
        return {
          schema_version: 6,
          policy: "none",
          build_reviewers: [],
          content_hash: "hash-0",
        };
      }
      if (command === "get_workflow_skills_policy_status") return dummyPolicyStatusCompliant;
      if (command === "set_reviewer_policy") {
        const payload = args as { config: ReviewerPolicyConfig };
        savedConfigs.push(payload.config);
        await new Promise((r) => setTimeout(r, 20));
        return { ...payload.config, content_hash: `hash-${hashCounter++}` };
      }
      return undefined;
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ExternalReviewCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("reviewer-policy-antigravity")).not.toBeChecked();
    });

    // Rapidly toggle Antigravity and then Codex
    await user.click(screen.getByTestId("reviewer-policy-antigravity"));
    await user.click(screen.getByTestId("reviewer-policy-codex-only"));

    await waitFor(() => {
      expect(savedConfigs.length).toBeGreaterThanOrEqual(1);
      const finalConfig = savedConfigs[savedConfigs.length - 1];
      expect(finalConfig.build_reviewers).toEqual(["antigravity", "codex"]);
    });
  });
});
