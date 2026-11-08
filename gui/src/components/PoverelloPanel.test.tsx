import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import PoverelloPanel from "./PoverelloPanel";
import { LanguageContext } from "../i18n";

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

describe("PoverelloPanel Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_reviewer_policy") {
        return {
          schema_version: 3,
          policy: "none",
        };
      }
      if (command === "get_workflow_skills_policy_status") {
        return {
          compliant: true,
          details: [],
        };
      }
      if (command === "get_pov_direct_status") {
        return {
          state: "managed_synced",
          can_apply: true,
          settings: {
            auto_activation: true,
            web_research: "requested_or_necessary",
            response_verbosity: "concise",
            repository_exploration: "minimal",
            verification: "focused",
          },
        };
      }
      return undefined;
    });
  });

  it("renders both ExternalReviewCard and AntigravityRulesPanel inside the top-level panel", async () => {
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <PoverelloPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("poverello-panel")).toBeInTheDocument();
    });

    expect(screen.getByTestId("reviewer-policy-card")).toBeInTheDocument();
    expect(screen.getByTestId("antigravity-rules-panel")).toBeInTheDocument();

    // Verify title for coding settings is Antigravity Nerf Settings
    expect(screen.getByRole("heading", { name: "Antigravity Nerf Settings" })).toBeInTheDocument();
    // Verify title for external review is Select Implementation Review AI
    expect(screen.getByRole("heading", { name: /Select Implementation Review AI|External Review Configuration/ })).toBeInTheDocument();
  });
});
