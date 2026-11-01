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
  schema_version: 3,
  policy: "codex_plus_deepseek",
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

  it("renders reviewer policy card with mode radios, summary, and collapsed accordions", async () => {
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ExternalReviewCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("reviewer-policy-card")).toBeInTheDocument();
    });

    // Policy radio options
    expect(screen.getByTestId("reviewer-policy-none")).toBeInTheDocument();
    expect(screen.getByTestId("reviewer-policy-codex-only")).toBeInTheDocument();
    expect(screen.getByTestId("reviewer-policy-codex-plus-deepseek")).toBeInTheDocument();
    expect(screen.getByTestId("reviewer-policy-codex-plus-deepseek")).toBeChecked();

    // Summary block
    const summary = screen.getByTestId("reviewer-config-summary");
    expect(summary).toBeInTheDocument();
    expect(summary.textContent).toContain("soheidon@yahoo.co.jp");
    expect(summary.textContent).toContain("chatgpt");
    expect(summary.textContent).toMatch(/default|antigravity\.configProfileDefault/);

    // Accordions collapsed by default
    expect(screen.getByTestId("codex-accordion-toggle")).toBeInTheDocument();
    expect(screen.getByTestId("deepseek-accordion-toggle")).toBeInTheDocument();
    expect(screen.queryByTestId("codex-model-select")).not.toBeInTheDocument();
    expect(screen.queryByTestId("deepseek-model-select")).not.toBeInTheDocument();
  });

  it("expands accordions and modifies model and effort without writing preset", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ExternalReviewCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("codex-accordion-toggle")).toBeInTheDocument();
    });

    // Expand Codex accordion
    await user.click(screen.getByTestId("codex-accordion-toggle"));
    expect(screen.getByTestId("codex-model-select")).toBeInTheDocument();

    // Change Codex model
    await user.selectOptions(screen.getByTestId("codex-model-select"), "gpt-6.1-sol");
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

  it("shows disabled summary when policy is none and hides accordions", async () => {
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
      expect(screen.getByTestId("reviewer-policy-none")).toBeChecked();
    });

    const summary = screen.getByTestId("reviewer-config-summary");
    expect(summary.textContent).toMatch(/External review is disabled|antigravity\.effectiveReviewDisabled/);
    expect(screen.queryByTestId("codex-accordion-toggle")).not.toBeInTheDocument();
    expect(screen.queryByTestId("deepseek-accordion-toggle")).not.toBeInTheDocument();
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

    expect(screen.getByTestId("reviewer-policy-none")).toBeDisabled();
    expect(screen.getByTestId("reviewer-policy-codex-only")).toBeDisabled();
    expect(screen.getByTestId("reviewer-policy-codex-plus-deepseek")).toBeDisabled();
  });
});
