import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import PlanFileManagerCard from "./PlanFileManagerCard";
import { LanguageContext } from "../i18n";
import type { PlanFileDetails } from "../types";

const mockInvoke = vi.fn((cmd: string, args?: unknown) => undefined as any);
let lastConfiguredLocation: string | null = "C:\\mock\\repo";
vi.mock("@tauri-apps/api/core", () => ({
  invoke: async (cmd: string, args?: unknown) => {
    const res = await (args !== undefined ? mockInvoke(cmd, args) : mockInvoke(cmd));
    if (cmd === "get_workflow_role_assignment" && res === undefined) {
      const legacy = await mockInvoke("get_plan_role_assignment", { ...(args as any), repoPath: lastConfiguredLocation });
      return { assignment: legacy ?? null, legacy_conflict: false, conflict_sources: [] };
    }
    if (cmd === "set_workflow_role_assignment" && res === undefined) {
      return mockInvoke("set_plan_role_assignment", { ...(args as any), repoPath: lastConfiguredLocation });
    }
    if (cmd === "get_plan_manager_location") {
      if (res !== undefined) {
        lastConfiguredLocation = res;
        return res;
      }
      lastConfiguredLocation = "C:\\mock\\repo";
      return "C:\\mock\\repo";
    }
    if (cmd === "get_project_plan_slots" && res === undefined) {
      const repo = (args as any)?.repoPath || lastConfiguredLocation;
      if (repo === null) {
        return [{ slot_id: "primary", repo_path: null, plan_path: null }];
      }
      return [
        { slot_id: "primary", repo_path: repo, plan_path: ".plan/V0.26.3.md" },
      ];
    }
    return res;
  },
}));

const mockPlanDetailsEmpty: PlanFileDetails = {
  repo_root: "C:\\mock\\repo",
  plans_dir: "C:\\mock\\repo\\.plan",
  active_plan_path: null,
  active_plan_relative_path: null,
  plans: [],
};

const mockPlanDetailsWithPlans: PlanFileDetails = {
  repo_root: "C:\\mock\\repo",
  plans_dir: "C:\\mock\\repo\\.plan",
  active_plan_path: "C:\\mock\\repo\\.plan\\V0.26.3.md",
  active_plan_relative_path: ".plan/V0.26.3.md",
  plans: [
    {
      path: "C:\\mock\\repo\\.plan\\V0.26.3.md",
      relative_path: ".plan/V0.26.3.md",
      title: "Plan Manager & Role Separation",
      status: "APPROVED",
      last_modified_ms: 1700000000000,
      is_active: true,
    },
    {
      path: "C:\\mock\\repo\\.plan\\feature-xyz.md",
      relative_path: ".plan/feature-xyz.md",
      title: "Feature XYZ",
      status: "PLANNED",
      last_modified_ms: 1699000000000,
      is_active: false,
    },
  ],
};

describe("PlanFileManagerCard Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    lastConfiguredLocation = "C:\\mock\\repo";
  });

  it("renders warning banner when no active plan is specified", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(mockPlanDetailsEmpty);
      }
      if (cmd === "get_project_plan_slots") {
        return Promise.resolve([
          { slot_id: "primary", repo_path: "C:\\mock\\repo", plan_path: null },
        ]);
      }
      return Promise.resolve();
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <PlanFileManagerCard surface="codex" />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("no-active-plan-warning")).toBeInTheDocument();
    });

    expect(screen.getByTestId("no-active-plan-warning")).toHaveTextContent(/planManager\.warningTitle/i);
  });

  it("renders active plan details and status badge when active plan exists", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(mockPlanDetailsWithPlans);
      }
      return Promise.resolve();
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <PlanFileManagerCard surface="codex" />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("active-plan-badge")).toBeInTheDocument();
      expect(screen.getByTestId("active-plan-path-display")).toHaveAttribute(
        "aria-label",
        "C:\\mock\\repo\\.plan\\V0.26.3.md — Plan Manager & Role Separation"
      );
    });

    expect(screen.getByTestId("active-plan-path-display")).toHaveTextContent("C:\\mock\\repo\\.plan\\V0.26.3.md");
    expect(screen.getByTestId("btn-open-plan")).toBeInTheDocument();
    expect(screen.getByTestId("btn-open-plan")).not.toBeDisabled();
    expect(screen.getByTestId("active-plan-badge")).toHaveTextContent("[planManager.status.approved]");
    expect(screen.getByTestId("active-plan-path-container")).toBeInTheDocument();
    expect(screen.queryByTestId("btn-create-plan")).not.toBeInTheDocument();
    expect(screen.queryByTestId("plan-file-input")).not.toBeInTheDocument();
  });

  it("renders static selectable path display that does not trigger open_path on click", async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(mockPlanDetailsWithPlans);
      }
      return Promise.resolve();
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <PlanFileManagerCard surface="codex" />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("active-plan-path-display")).toBeInTheDocument();
    });

    const pathDisplay = screen.getByTestId("active-plan-path-display");
    expect(pathDisplay).toHaveTextContent("C:\\mock\\repo\\.plan\\V0.26.3.md");

    // Clicking the path field itself is a no-op (static text, not a link or button)
    await user.click(pathDisplay);
    expect(mockInvoke).not.toHaveBeenCalledWith("open_path", expect.anything());
  });

  it("invokes open_path with the operational path when clicking the dedicated open button", async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(mockPlanDetailsWithPlans);
      }
      return Promise.resolve();
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <PlanFileManagerCard surface="codex" />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-open-plan")).not.toBeDisabled();
    });

    const openBtn = screen.getByTestId("btn-open-plan");
    await user.click(openBtn);

    expect(mockInvoke).toHaveBeenCalledWith("open_path", {
      path: "C:\\mock\\repo\\.plan\\V0.26.3.md",
    });
  });

  it("disables dedicated open button and displays empty message when no plan is selected", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(mockPlanDetailsEmpty);
      }
      if (cmd === "get_project_plan_slots") {
        return Promise.resolve([
          { slot_id: "primary", repo_path: "C:\\mock\\repo", plan_path: null },
        ]);
      }
      return Promise.resolve();
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <PlanFileManagerCard surface="codex" />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-open-plan")).toBeInTheDocument();
    });

    expect(screen.getByTestId("btn-open-plan")).toBeDisabled();
    expect(screen.queryByTestId("active-plan-path-display")).not.toBeInTheDocument();
    expect(screen.getByTestId("empty-plan-path-field")).toBeInTheDocument();
    expect(screen.queryByTestId("btn-create-plan")).not.toBeInTheDocument();
    expect(screen.getByTestId("btn-refresh-plans")).toBeDisabled();
  });

  it("selects a plan file via select_plan_file_dialog, sets it as active, and reloads parent folder", async () => {
    const user = userEvent.setup();
    const onActivePlanChanged = vi.fn();

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(mockPlanDetailsEmpty);
      }
      if (cmd === "select_plan_file_dialog") {
        return Promise.resolve(".plan/feature-xyz.md");
      }
      if (cmd === "set_active_plan") {
        return Promise.resolve({
          ...mockPlanDetailsWithPlans,
          active_plan_path: "C:\\mock\\repo\\.plan\\feature-xyz.md",
          active_plan_relative_path: ".plan/feature-xyz.md",
        });
      }
      return Promise.resolve();
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <PlanFileManagerCard surface="codex" onActivePlanChanged={onActivePlanChanged} />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-select-plan-file")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("btn-select-plan-file"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("select_plan_file_dialog", {
        repoPath: "C:\\mock\\repo",
        surface: "codex",
        dialogTitle: expect.any(String),
        slotId: "primary",
      });
      expect(mockInvoke).toHaveBeenCalledWith("set_active_plan", {
        relativePath: ".plan/feature-xyz.md",
        repoPath: "C:\\mock\\repo",
        surface: "codex",
        slotId: "primary",
      });
    });

    expect(onActivePlanChanged).toHaveBeenCalledWith(".plan/feature-xyz.md");
  });

  it("does not render create plan input or mode in PlanFileManagerCard", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(mockPlanDetailsWithPlans);
      }
      return Promise.resolve();
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <PlanFileManagerCard surface="codex" />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("active-plan-path-container")).toBeInTheDocument();
    });

    expect(screen.queryByTestId("btn-create-plan")).not.toBeInTheDocument();
    expect(screen.queryByTestId("plan-file-input")).not.toBeInTheDocument();
    expect(screen.queryByTestId("btn-confirm-plan")).not.toBeInTheDocument();
    expect(screen.queryByTestId("create-plan-mode")).not.toBeInTheDocument();
  });

  it("keeps roles unselected on antigravity surface until explicitly chosen", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(mockPlanDetailsWithPlans);
      }
      return Promise.resolve();
    });

    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <PlanFileManagerCard
          surface="antigravity"
          plannerRole=""
          reviserRole=""
        />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("planner-role-antigravity")).toBeInTheDocument();
    });

    expect(screen.getByTestId("planner-role-none")).toBeChecked();
    expect(screen.getByTestId("reviser-role-none")).toBeChecked();

    // Available options on Antigravity surface: only Antigravity and Codex CLI (no unselected, no deepseek for planner/reviser)
    expect(screen.getByTestId("planner-role-codex")).toBeInTheDocument();
    expect(screen.queryByTestId("planner-role-deepseek")).not.toBeInTheDocument();
    expect(screen.getByTestId("planner-role-none")).toBeInTheDocument();

    expect(screen.getByTestId("reviser-role-codex")).toBeInTheDocument();
    expect(screen.queryByTestId("reviser-role-deepseek")).not.toBeInTheDocument();
    expect(screen.getByTestId("reviser-role-none")).toBeInTheDocument();

    expect(screen.getByTestId("planner-role-codex")).not.toBeChecked();
    expect(screen.getByTestId("reviser-role-codex")).not.toBeChecked();

    // Plan review section has inherit option and 3 toggleable reviewers
    expect(screen.getByTestId("plan-reviewer-inherit-toggle")).toBeInTheDocument();
    expect(screen.getByTestId("plan-reviewer-inherit")).toBeChecked();
    expect(screen.getByTestId("plan-reviewer-antigravity-toggle")).toBeInTheDocument();
    expect(screen.getByTestId("plan-reviewer-codex-toggle")).toBeInTheDocument();
    expect(screen.getByTestId("plan-reviewer-deepseek-toggle")).toBeInTheDocument();
  });

  it("restores persisted roles when saving a selection fails", async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") return Promise.resolve(mockPlanDetailsWithPlans);
      if (cmd === "get_plan_role_assignment") return Promise.resolve({ planner_id: null, reviser_id: null });
      if (cmd === "set_plan_role_assignment") return Promise.reject(new Error("disk full"));
      return Promise.resolve();
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <PlanFileManagerCard surface="antigravity" />
      </LanguageContext.Provider>
    );
    await waitFor(() => expect(screen.getByTestId("planner-role-antigravity")).toBeInTheDocument());
    await user.click(screen.getByTestId("planner-role-antigravity"));
    await waitFor(() => expect(screen.getByTestId("planner-role-none")).toBeChecked());
    expect(screen.getByText("Error: disk full")).toBeInTheDocument();
  });

  it("handles independent role selections and invokes onRolesChanged for Planner and Reviser combinations", async () => {
    const user = userEvent.setup();
    const onRolesChanged = vi.fn();

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(mockPlanDetailsWithPlans);
      }
      return Promise.resolve();
    });

    const { rerender } = render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <PlanFileManagerCard
          surface="antigravity"
          plannerRole="antigravity"
          reviserRole="antigravity"
          onRolesChanged={onRolesChanged}
        />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("planner-role-codex")).toBeInTheDocument();
    });

    // Select Planner: Codex CLI
    await user.click(screen.getByTestId("planner-role-codex"));
    expect(onRolesChanged).toHaveBeenCalledWith(
      { plannerRole: "codex" }
    );

    // Rerender with Planner=codex, Reviser=antigravity
    rerender(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <PlanFileManagerCard
          surface="antigravity"
          plannerRole="codex"
          reviserRole="antigravity"
          onRolesChanged={onRolesChanged}
        />
      </LanguageContext.Provider>
    );

    // Select Reviser: Codex CLI
    await user.click(screen.getByTestId("reviser-role-codex"));
    expect(onRolesChanged).toHaveBeenCalledWith(
      { reviserRole: "codex" }
    );

    // Switch Planner back to Antigravity
    rerender(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <PlanFileManagerCard
          surface="antigravity"
          plannerRole="codex"
          reviserRole="codex"
          onRolesChanged={onRolesChanged}
        />
      </LanguageContext.Provider>
    );
    await user.click(screen.getByTestId("planner-role-antigravity"));
    expect(onRolesChanged).toHaveBeenCalledWith(
      { plannerRole: "antigravity" }
    );
  });

  it("handles multi-select and inherit mode for Plan Review", async () => {
    const user = userEvent.setup();
    const onRolesChanged = vi.fn();
    const onPlanReviewersChanged = vi.fn();

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(mockPlanDetailsWithPlans);
      }
      return Promise.resolve();
    });

    const { rerender } = render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <PlanFileManagerCard
          surface="antigravity"
          plannerRole="antigravity"
          reviserRole="antigravity"
          planReviewers={null}
          onRolesChanged={onRolesChanged}
          onPlanReviewersChanged={onPlanReviewersChanged}
        />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("plan-reviewer-inherit-toggle")).toBeInTheDocument();
    });

    // Default: inherit mode checked
    expect(screen.getByTestId("plan-reviewer-inherit")).toBeChecked();
    expect(screen.getByTestId("plan-reviewer-codex-only")).not.toBeChecked();
    expect(screen.getByTestId("plan-reviewer-deepseek-only")).not.toBeChecked();

    // 1. Click Codex CLI: breaks inherit, sets planReviewers: ["codex"]
    await user.click(screen.getByTestId("plan-reviewer-codex-toggle"));
    expect(onPlanReviewersChanged).toHaveBeenCalledWith(["codex"]);
    expect(onRolesChanged).toHaveBeenCalledWith(
      expect.objectContaining({
        planReviewers: ["codex"],
        planReviewPolicy: "codex_only",
      })
    );

    // Rerender with planReviewers: ["codex"]
    rerender(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <PlanFileManagerCard
          surface="antigravity"
          plannerRole="antigravity"
          reviserRole="antigravity"
          planReviewers={["codex"]}
          onRolesChanged={onRolesChanged}
          onPlanReviewersChanged={onPlanReviewersChanged}
        />
      </LanguageContext.Provider>
    );
    expect(screen.getByTestId("plan-reviewer-inherit")).not.toBeChecked();
    expect(screen.getByTestId("plan-reviewer-codex-only")).toBeChecked();
    expect(screen.getByTestId("plan-reviewer-deepseek-only")).not.toBeChecked();

    // 2. Click DeepSeek Harness: adds deepseek -> ["codex", "deepseek"]
    await user.click(screen.getByTestId("plan-reviewer-deepseek-toggle"));
    expect(onPlanReviewersChanged).toHaveBeenCalledWith(["codex", "deepseek"]);
    expect(onRolesChanged).toHaveBeenCalledWith(
      expect.objectContaining({
        planReviewers: ["codex", "deepseek"],
        planReviewPolicy: "codex_plus_deepseek",
      })
    );

    // Rerender with planReviewers: ["codex", "deepseek"]
    rerender(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <PlanFileManagerCard
          surface="antigravity"
          plannerRole="antigravity"
          reviserRole="antigravity"
          planReviewers={["codex", "deepseek"]}
          onRolesChanged={onRolesChanged}
          onPlanReviewersChanged={onPlanReviewersChanged}
        />
      </LanguageContext.Provider>
    );
    expect(screen.getByTestId("plan-reviewer-codex-only")).toBeChecked();
    expect(screen.getByTestId("plan-reviewer-deepseek-only")).toBeChecked();

    // 3. Click "Use Implementation Review Settings" (inherit): resets to null
    await user.click(screen.getByTestId("plan-reviewer-inherit-toggle"));
    expect(onPlanReviewersChanged).toHaveBeenCalledWith(null);
    expect(onRolesChanged).toHaveBeenCalledWith(
      expect.objectContaining({
        planReviewers: null,
      })
    );
  });

  it("restricts choices on codex surface and does not show antigravity option", async () => {
    const user = userEvent.setup();
    const onRolesChanged = vi.fn();

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(mockPlanDetailsWithPlans);
      }
      return Promise.resolve();
    });

    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <PlanFileManagerCard
          surface="codex"
          plannerRole=""
          reviserRole=""
          onRolesChanged={onRolesChanged}
        />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("planner-role-codex-session")).toBeInTheDocument();
    });

    // Codex surface offers Codex Session and an explicit unselected choice.
    expect(screen.getByTestId("planner-role-codex-session")).toBeInTheDocument();
    expect(screen.getByTestId("planner-role-none")).toBeInTheDocument();
    expect(screen.getByTestId("planner-role-none")).toBeChecked();
    expect(screen.queryByTestId("planner-role-antigravity")).not.toBeInTheDocument();
    expect(screen.getByTestId("reviser-role-codex-session")).toBeInTheDocument();
    expect(screen.queryByTestId("reviser-role-antigravity")).not.toBeInTheDocument();
    expect(screen.getByTestId("reviser-role-none")).toBeChecked();

    // Plan reviewer group is present with inherit, codex session, and deepseek, but no antigravity
    expect(screen.getByTestId("plan-reviewer-inherit-toggle")).toBeInTheDocument();
    expect(screen.getByTestId("plan-reviewer-codex-toggle")).toBeInTheDocument();
    expect(screen.getByTestId("plan-reviewer-deepseek-toggle")).toBeInTheDocument();
    expect(screen.queryByTestId("plan-reviewer-antigravity-toggle")).not.toBeInTheDocument();

    // Click deepseek in plan review
    await user.click(screen.getByTestId("plan-reviewer-deepseek-toggle"));
    expect(onRolesChanged).toHaveBeenCalledWith(
      expect.objectContaining({
        planReviewers: ["deepseek"],
      })
    );
  });

  it("opens active plan file in system editor via open_path", async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(mockPlanDetailsWithPlans);
      }
      return Promise.resolve();
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <PlanFileManagerCard surface="codex" />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-open-plan")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("btn-open-plan"));

    expect(mockInvoke).toHaveBeenCalledWith("open_path", {
      path: "C:\\mock\\repo\\.plan\\V0.26.3.md",
    });
  });

  it("does not repeatedly call list_available_plans on mount or re-render (no infinite loop)", async () => {
    let callCount = 0;
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        callCount++;
        return Promise.resolve(mockPlanDetailsWithPlans);
      }
      return Promise.resolve();
    });

    const { rerender } = render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <PlanFileManagerCard
          surface="antigravity"
          onActivePlanChanged={() => {}}
        />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-refresh-plans")).toBeInTheDocument();
    });

    // Re-render parent with new inline callbacks to simulate parent state updates
    rerender(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <PlanFileManagerCard
          surface="antigravity"
          onActivePlanChanged={() => {}}
        />
      </LanguageContext.Provider>
    );

    // Give time to ensure no loop occurs
    await new Promise((r) => setTimeout(r, 100));

    expect(callCount).toBe(1);
    // Button shows refresh and is not blinking loading
    expect(screen.getByTestId("btn-refresh-plans")).toHaveTextContent(/planManager\.refresh|再読込/i);
    expect(screen.getByTestId("btn-refresh-plans")).not.toBeDisabled();
  });

  it("only displays loading text during explicit manual refresh click", async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(mockPlanDetailsWithPlans);
      }
      return Promise.resolve();
    });

    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <PlanFileManagerCard surface="antigravity" />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-refresh-plans")).not.toBeDisabled();
    });

    const refreshBtn = screen.getByTestId("btn-refresh-plans");
    expect(refreshBtn).toHaveTextContent(/planManager\.refresh|再読込/i);
    expect(refreshBtn).not.toBeDisabled();

    // Setup delayed promise for the second (manual) call
    let refreshPromiseResolved = false;
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return new Promise((resolve) => {
          setTimeout(() => {
            refreshPromiseResolved = true;
            resolve(mockPlanDetailsWithPlans);
          }, 80);
        });
      }
      return Promise.resolve();
    });

    await user.click(refreshBtn);

    // During manual refresh, button shows loading and is disabled
    expect(screen.getByTestId("btn-refresh-plans")).toHaveTextContent(/planManager\.loading|更新中/i);
    expect(screen.getByTestId("btn-refresh-plans")).toBeDisabled();

    await waitFor(() => {
      expect(refreshPromiseResolved).toBe(true);
      expect(screen.getByTestId("btn-refresh-plans")).toHaveTextContent(/planManager\.refresh|再読込/i);
      expect(screen.getByTestId("btn-refresh-plans")).not.toBeDisabled();
    });
  });

  it("does not display the location warning when no repository root is configured", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_plan_manager_location") {
        return Promise.resolve(null);
      }
      return Promise.resolve();
    });

    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <PlanFileManagerCard surface="codex" repoPath={null} />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.queryByTestId("no-location-warning")).not.toBeInTheDocument();
    });
    expect(screen.getByTestId("btn-select-plan-file")).toBeInTheDocument();
    expect(screen.getByTestId("btn-refresh-plans")).toBeDisabled();
    expect(mockInvoke).not.toHaveBeenCalledWith("list_available_plans", expect.anything());
  });

  it("clears selected folder and details when activeRepo changes", async () => {
    let currentRepo = "C:\\mock\\repo1";
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(currentRepo === "C:\\mock\\repo1" ? mockPlanDetailsWithPlans : mockPlanDetailsEmpty);
      }
      if (cmd === "get_project_plan_slots") {
        return Promise.resolve([
          {
            slot_id: "primary",
            repo_path: currentRepo,
            plan_path: currentRepo === "C:\\mock\\repo1" ? ".plan/V0.26.3.md" : null,
          },
        ]);
      }
      return Promise.resolve();
    });

    const { rerender } = render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <PlanFileManagerCard surface="codex" repoPath="C:\\mock\\repo1" />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("active-plan-badge")).toBeInTheDocument();
    });

    // Re-render with new repoPath
    currentRepo = "C:\\mock\\repo2";
    rerender(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <PlanFileManagerCard surface="codex" repoPath="C:\\mock\\repo2" />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.queryByTestId("active-plan-badge")).not.toBeInTheDocument();
      expect(screen.getByTestId("no-active-plan-warning")).toBeInTheDocument();
    });
  });

  describe("unconfigured plan folder location and slot selection", () => {
    it.each(["codex" as const, "antigravity" as const])(
      "calls select_plan_file_dialog directly without calling select_plan_root_dialog when unconfigured on %s",
      async (surface) => {
        const user = userEvent.setup();
        mockInvoke.mockImplementation((cmd: string) => {
          if (cmd === "get_plan_manager_location") {
            return Promise.resolve(null);
          }
          if (cmd === "select_plan_file_dialog") {
            return Promise.resolve(null);
          }
          return Promise.resolve();
        });

        render(
          <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
            <PlanFileManagerCard surface={surface} repoPath={null} />
          </LanguageContext.Provider>
        );

        await waitFor(() => {
          expect(screen.queryByTestId("no-location-warning")).not.toBeInTheDocument();
        });

        await user.click(screen.getByTestId("btn-select-plan-file"));

        await waitFor(() => {
          expect(mockInvoke).toHaveBeenCalledWith(
            "select_plan_file_dialog",
            expect.objectContaining({ surface, slotId: "primary" })
          );
        });
        expect(mockInvoke).not.toHaveBeenCalledWith("select_plan_root_dialog", expect.anything());
        expect(mockInvoke).not.toHaveBeenCalledWith("list_available_plans", expect.anything());
        expect(screen.queryByTestId("no-location-warning")).not.toBeInTheDocument();
      }
    );

    it.each(["codex" as const, "antigravity" as const])(
      "renders no project-folder picker and selecting plan directly binds repo on %s",
      async (surface) => {
        const user = userEvent.setup();
        const selectedRepo = "C:\\mock\\new-repo";
        const selectedPlan = ".plan/new-feature.md";
        mockInvoke.mockImplementation((cmd: string) => {
          if (cmd === "get_plan_manager_location") {
            return Promise.resolve(null);
          }
          if (cmd === "select_plan_file_dialog") {
            return Promise.resolve({
              repo_path: selectedRepo,
              relative_path: selectedPlan,
              surface,
              slot_id: "primary",
            });
          }
          if (cmd === "set_active_plan") {
            return Promise.resolve({
              repo_root: selectedRepo,
              plans_dir: `${selectedRepo}\\.plan`,
              active_plan_path: `${selectedRepo}\\${selectedPlan}`,
              active_plan_relative_path: selectedPlan,
              plans: [
                {
                  filename: "new-feature.md",
                  relative_path: selectedPlan,
                  abs_path: `${selectedRepo}\\${selectedPlan}`,
                  is_active: true,
                },
              ],
            });
          }
          if (cmd === "list_available_plans") {
            return Promise.resolve({
              repo_root: selectedRepo,
              plans_dir: `${selectedRepo}\\.plan`,
              active_plan_path: `${selectedRepo}\\${selectedPlan}`,
              active_plan_relative_path: selectedPlan,
              plans: [
                {
                  filename: "new-feature.md",
                  relative_path: selectedPlan,
                  abs_path: `${selectedRepo}\\${selectedPlan}`,
                  is_active: true,
                },
              ],
            });
          }
          if (cmd === "get_plan_role_assignment") {
            return Promise.resolve({ planner_id: null, reviser_id: null });
          }
          return Promise.resolve();
        });

        render(
          <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
            <PlanFileManagerCard surface={surface} repoPath={null} />
          </LanguageContext.Provider>
        );

        await waitFor(() => {
          expect(screen.queryByTestId("no-location-warning")).not.toBeInTheDocument();
        });

        // Folder selection button does not exist
        expect(screen.queryByTestId("btn-select-project-primary")).not.toBeInTheDocument();

        // Clicking plan file button selects plan and binds repo
        await user.click(screen.getByTestId("btn-select-plan-file"));

        await waitFor(() => {
          expect(mockInvoke).toHaveBeenCalledWith(
            "select_plan_file_dialog",
            expect.objectContaining({ surface, slotId: "primary" })
          );
        });

        await waitFor(() => {
          expect(mockInvoke).toHaveBeenCalledWith("set_active_plan", {
            relativePath: selectedPlan,
            repoPath: selectedRepo,
            surface,
            slotId: "primary",
          });
        });

        // Does not call folder selection dialogs
        expect(mockInvoke).not.toHaveBeenCalledWith("select_project_folder_dialog", expect.anything());
        expect(mockInvoke).not.toHaveBeenCalledWith("select_plan_root_dialog", expect.anything());
      }
    );

    it.each(["codex" as const, "antigravity" as const])(
      "opens select_plan_file_dialog when root is configured on %s, sets active plan, and reloads parent folder",
      async (surface) => {
        const user = userEvent.setup();
        const configuredRepo = "C:\\mock\\existing-repo";
        const onActivePlanChanged = vi.fn();

        let activePlan: string | null = null;
        mockInvoke.mockImplementation((cmd: string, args?: any) => {
          if (cmd === "get_plan_manager_location") {
            return Promise.resolve(configuredRepo);
          }
          if (cmd === "get_project_plan_slots") {
            return Promise.resolve([
              { slot_id: "primary", repo_path: configuredRepo, plan_path: activePlan },
            ]);
          }
          if (cmd === "list_available_plans") {
            return Promise.resolve({
              repo_root: configuredRepo,
              plans_dir: `${configuredRepo}\\.plan`,
              active_plan_path: activePlan ? `${configuredRepo}\\${activePlan.replace(/\//g, "\\")}` : null,
              active_plan_relative_path: activePlan,
              plans: [],
            });
          }
          if (cmd === "select_plan_file_dialog") {
            return Promise.resolve(".plan/my-plan.md");
          }
          if (cmd === "set_active_plan") {
            activePlan = args?.relativePath || ".plan/my-plan.md";
            return Promise.resolve({
              repo_root: configuredRepo,
              plans_dir: `${configuredRepo}\\.plan`,
              active_plan_path: `${configuredRepo}\\.plan\\my-plan.md`,
              active_plan_relative_path: ".plan/my-plan.md",
              plans: [
                {
                  path: `${configuredRepo}\\.plan\\my-plan.md`,
                  relative_path: ".plan/my-plan.md",
                  title: "My Plan",
                  status: "APPROVED",
                  last_modified_ms: 1700000000000,
                  is_active: true,
                },
              ],
            });
          }
          return Promise.resolve();
        });

        render(
          <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
            <PlanFileManagerCard surface={surface} onActivePlanChanged={onActivePlanChanged} />
          </LanguageContext.Provider>
        );

        await waitFor(() => {
          expect(screen.getByTestId("btn-select-plan-file")).toBeInTheDocument();
          expect(screen.getByTestId("no-active-plan-warning")).toBeInTheDocument();
        });

        onActivePlanChanged.mockClear();

        await user.click(screen.getByTestId("btn-select-plan-file"));

        await waitFor(() => {
          expect(mockInvoke).toHaveBeenCalledWith("select_plan_file_dialog", {
            repoPath: configuredRepo,
            surface,
            dialogTitle: expect.any(String),
            slotId: "primary",
          });
          expect(mockInvoke).toHaveBeenCalledWith("set_active_plan", {
            relativePath: ".plan/my-plan.md",
            repoPath: configuredRepo,
            surface,
            slotId: "primary",
          });
          expect(screen.getByTestId("active-plan-badge")).toBeInTheDocument();
        });
        expect(onActivePlanChanged).toHaveBeenCalledWith(".plan/my-plan.md");
        expect(mockInvoke).not.toHaveBeenCalledWith("select_plan_root_dialog", expect.anything());
      }
    );

    it.each(["codex" as const, "antigravity" as const])(
      "preserves active plan and UI selection when select_plan_file_dialog is cancelled on %s",
      async (surface) => {
        const user = userEvent.setup();
        const configuredRepo = "C:\\mock\\existing-repo";
        const onActivePlanChanged = vi.fn();

        mockInvoke.mockImplementation((cmd: string) => {
          if (cmd === "get_plan_manager_location") {
            return Promise.resolve(configuredRepo);
          }
          if (cmd === "get_project_plan_slots") {
            return Promise.resolve([
              { slot_id: "primary", repo_path: configuredRepo, plan_path: ".plan/V0.26.3.md" },
            ]);
          }
          if (cmd === "list_available_plans") {
            return Promise.resolve(mockPlanDetailsWithPlans);
          }
          if (cmd === "select_plan_file_dialog") {
            return Promise.resolve(null);
          }
          return Promise.resolve();
        });

        render(
          <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
            <PlanFileManagerCard surface={surface} onActivePlanChanged={onActivePlanChanged} />
          </LanguageContext.Provider>
        );

        await waitFor(() => {
          expect(screen.getByTestId("btn-select-plan-file")).toBeInTheDocument();
          expect(screen.getByTestId("active-plan-badge")).toBeInTheDocument();
        });

        onActivePlanChanged.mockClear();

        await user.click(screen.getByTestId("btn-select-plan-file"));

        await waitFor(() => {
          expect(mockInvoke).toHaveBeenCalledWith("select_plan_file_dialog", {
            repoPath: configuredRepo,
            surface,
            dialogTitle: expect.any(String),
            slotId: "primary",
          });
        });

        expect(mockInvoke).not.toHaveBeenCalledWith("set_active_plan", expect.anything());
        expect(onActivePlanChanged).not.toHaveBeenCalled();
      }
    );
  });

  describe("Phase 14 & Phase 15 Autocode Plan Header, Path Presentation, and Selection UX", () => {
    const testCases: Array<{ surface: "codex" | "antigravity" }> = [
      { surface: "codex" },
      { surface: "antigravity" },
    ];

    testCases.forEach(({ surface }) => {
      it(`renders compact header, concise status, no-wrap refresh, normal Windows path in static bordered field on ${surface}`, async () => {
        const mockVerbosePlanDetails: PlanFileDetails = {
          repo_root: "C:\\Users\\Sohei\\dev\\project",
          plans_dir: "C:\\Users\\Sohei\\dev\\project\\.plan",
          active_plan_path: "C:\\Users\\Sohei\\dev\\project\\.plan\\V0.27.2.md",
          active_plan_relative_path: ".plan/V0.27.2.md",
          plans: [
            {
              path: "C:\\Users\\Sohei\\dev\\project\\.plan\\V0.27.2.md",
              relative_path: ".plan/V0.27.2.md",
              title: "Very Long Detailed Plan Title That Should Not Overflow Compact Header",
              status: "APPROVED — Phase 12 implementation reviewed and approved by Codex Desktop (APPROVED_WITH_NON_BLOCKING_FINDINGS)",
              last_modified_ms: 1700000000000,
              is_active: true,
            },
          ],
        };

        mockInvoke.mockImplementation((cmd: string) => {
          if (cmd === "list_available_plans") {
            return Promise.resolve(mockVerbosePlanDetails);
          }
          if (cmd === "get_plan_manager_location") {
            return Promise.resolve("C:\\Users\\Sohei\\dev\\project");
          }
          if (cmd === "get_project_plan_slots") {
            return Promise.resolve([
              {
                slot_id: "primary",
                repo_path: "C:\\Users\\Sohei\\dev\\project",
                plan_path: ".plan/V0.27.2.md",
              },
            ]);
          }
          return Promise.resolve();
        });

        render(
          <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
            <PlanFileManagerCard surface={surface} repoPath="C:\\Users\\Sohei\\dev\\project" />
          </LanguageContext.Provider>
        );

        await waitFor(() => {
          expect(screen.getByTestId("active-plan-badge")).toBeInTheDocument();
        });

        // 1. Heading and concise status (no verbose narration)
        expect(screen.getByText("planManager.title")).toBeInTheDocument();
        const badge = screen.getByTestId("active-plan-badge");
        expect(badge).toHaveTextContent("[planManager.status.approved]");
        expect(badge.textContent).not.toContain("APPROVED_WITH_NON_BLOCKING_FINDINGS");
        expect(badge).toHaveAttribute("title", "planManager.status.approved");

        // 2. Refresh button has flex-shrink: 0 and white-space: nowrap
        const refreshBtn = screen.getByTestId("btn-refresh-plans");
        expect(refreshBtn).toHaveStyle({ flexShrink: "0", whiteSpace: "nowrap" });

        // 3. Normal Windows path rendered inside bordered static field beside picker button on Row 2 (not a link)
        expect(screen.queryByTestId("active-plan-url-row")).not.toBeInTheDocument();
        const pathContainer = screen.getByTestId("active-plan-path-container");
        expect(pathContainer).toBeInTheDocument();

        const pathDisplay = screen.getByTestId("active-plan-path-display");
        expect(pathDisplay).toHaveTextContent("C:\\Users\\Sohei\\dev\\project\\.plan\\V0.27.2.md");
        expect(pathDisplay.textContent).not.toContain("file:///");
        expect(pathDisplay.textContent).not.toContain("\\\\?\\");
        expect(pathDisplay).toHaveAttribute(
          "aria-label",
          "C:\\Users\\Sohei\\dev\\project\\.plan\\V0.27.2.md — Very Long Detailed Plan Title That Should Not Overflow Compact Header"
        );

        // 4. Phase 16: Row 2 Layout Ordering and Buttons
        const selectBtn = screen.getByTestId("btn-select-plan-file");
        const openBtn = screen.getByTestId("btn-open-plan");
        expect(selectBtn).toBeInTheDocument();
        expect(openBtn).toBeInTheDocument();
        expect(openBtn).not.toBeDisabled();
        expect(openBtn).toHaveTextContent("planManager.btnOpenPlanFile");

        // Verify single-row order: active-plan-path-container precedes btn-select-plan-file which precedes btn-open-plan
        expect(pathContainer.compareDocumentPosition(selectBtn) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
        expect(selectBtn.compareDocumentPosition(openBtn) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

        // Verify selector button label key
        expect(selectBtn).toHaveTextContent("planManager.planFileButtonLabel");

        // Verify open button contains no leading arrow glyph or icon span
        expect(openBtn.textContent).not.toContain("↗");
        expect(openBtn.textContent).not.toContain("→");
        expect(openBtn.querySelector("span[aria-hidden='true']")).toBeNull();

        // 5. Create button and confirm button are completely absent
        expect(screen.queryByTestId("btn-confirm-plan")).not.toBeInTheDocument();
        expect(screen.queryByTestId("btn-create-plan")).not.toBeInTheDocument();
      });

      it(`places directory box before selector and open buttons with no arrow on ${surface}`, async () => {
        mockInvoke.mockImplementation((cmd: string) => {
          if (cmd === "list_available_plans") {
            return Promise.resolve(mockPlanDetailsWithPlans);
          }
          if (cmd === "get_plan_manager_location") {
            return Promise.resolve("C:\\mock\\repo");
          }
          return Promise.resolve();
        });

        render(
          <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
            <PlanFileManagerCard surface={surface} repoPath="C:\\mock\\repo" />
          </LanguageContext.Provider>
        );

        await waitFor(() => {
          expect(screen.getByTestId("active-plan-path-container")).toBeInTheDocument();
        });

        const pathContainer = screen.getByTestId("active-plan-path-container");
        const selectBtn = screen.getByTestId("btn-select-plan-file");
        const openBtn = screen.getByTestId("btn-open-plan");

        // Verify exact Row 2 DOM ordering
        expect(pathContainer.compareDocumentPosition(selectBtn) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
        expect(selectBtn.compareDocumentPosition(openBtn) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

        // Verify selector button label
        expect(selectBtn).toHaveTextContent("planManager.planFileButtonLabel");

        // Verify open button text and lack of arrow icon
        expect(openBtn).toHaveTextContent("planManager.btnOpenPlanFile");
        expect(openBtn.textContent).not.toContain("↗");
        expect(openBtn.textContent).not.toContain("→");
        expect(openBtn.querySelector("span[aria-hidden='true']")).toBeNull();
      });

      it(`normalizes Windows extended-length prefix \\\\?\\ on ${surface} while preserving operational path for open_path`, async () => {
        const user = userEvent.setup();
        const rawExtendedPath = "\\\\?\\C:\\Users\\Sohei\\dev\\project\\.plan\\V0.27.2.md";
        const mockExtendedPlanDetails: PlanFileDetails = {
          repo_root: "C:\\Users\\Sohei\\dev\\project",
          plans_dir: "C:\\Users\\Sohei\\dev\\project\\.plan",
          active_plan_path: rawExtendedPath,
          active_plan_relative_path: ".plan/V0.27.2.md",
          plans: [
            {
              path: rawExtendedPath,
              relative_path: ".plan/V0.27.2.md",
              title: "Extended Path Plan",
              status: "IMPLEMENTED",
              last_modified_ms: 1700000000000,
              is_active: true,
            },
          ],
        };

        mockInvoke.mockImplementation((cmd: string) => {
          if (cmd === "list_available_plans") {
            return Promise.resolve(mockExtendedPlanDetails);
          }
          if (cmd === "get_plan_manager_location") {
            return Promise.resolve("C:\\Users\\Sohei\\dev\\project");
          }
          if (cmd === "get_project_plan_slots") {
            return Promise.resolve([
              {
                slot_id: "primary",
                repo_path: "C:\\Users\\Sohei\\dev\\project",
                plan_path: ".plan/V0.27.2.md",
              },
            ]);
          }
          return Promise.resolve();
        });

        render(
          <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
            <PlanFileManagerCard surface={surface} repoPath="C:\\Users\\Sohei\\dev\\project" />
          </LanguageContext.Provider>
        );

        await waitFor(() => {
          expect(screen.getByTestId("active-plan-path-display")).toBeInTheDocument();
        });

        const pathDisplay = screen.getByTestId("active-plan-path-display");
        // Displayed path strips \\?\ and starts with C:\
        expect(pathDisplay).toHaveTextContent("C:\\Users\\Sohei\\dev\\project\\.plan\\V0.27.2.md");
        expect(pathDisplay.textContent).not.toContain("\\\\?\\");
        expect(pathDisplay.textContent).not.toContain("file:///");

        // Clicking the path display does not call open_path (static text)
        await user.click(pathDisplay);
        expect(mockInvoke).not.toHaveBeenCalledWith("open_path", expect.anything());

        // Clicking dedicated open button invokes open_path with the exact raw operational path
        const openBtn = screen.getByTestId("btn-open-plan");
        await user.click(openBtn);
        expect(mockInvoke).toHaveBeenCalledWith("open_path", {
          path: rawExtendedPath,
        });
      });

      it(`falls back to bounded neutral localized label for unknown status on ${surface}`, async () => {
        const mockUnknownStatusPlanDetails: PlanFileDetails = {
          repo_root: "C:\\Users\\Sohei\\dev\\project",
          plans_dir: "C:\\Users\\Sohei\\dev\\project\\.plan",
          active_plan_path: "C:\\Users\\Sohei\\dev\\project\\.plan\\custom.md",
          active_plan_relative_path: ".plan/custom.md",
          plans: [
            {
              path: "C:\\Users\\Sohei\\dev\\project\\.plan\\custom.md",
              relative_path: ".plan/custom.md",
              title: "Custom Plan",
              status: "SOME_ARBITRARY_UNRECOGNIZED_STATUS_STRING",
              last_modified_ms: 1700000000000,
              is_active: true,
            },
          ],
        };

        mockInvoke.mockImplementation((cmd: string) => {
          if (cmd === "list_available_plans") {
            return Promise.resolve(mockUnknownStatusPlanDetails);
          }
          if (cmd === "get_plan_manager_location") {
            return Promise.resolve("C:\\Users\\Sohei\\dev\\project");
          }
          if (cmd === "get_project_plan_slots") {
            return Promise.resolve([
              {
                slot_id: "primary",
                repo_path: "C:\\Users\\Sohei\\dev\\project",
                plan_path: ".plan/custom.md",
              },
            ]);
          }
          return Promise.resolve();
        });

        render(
          <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
            <PlanFileManagerCard surface={surface} repoPath="C:\\Users\\Sohei\\dev\\project" />
          </LanguageContext.Provider>
        );

        await waitFor(() => {
          expect(screen.getByTestId("active-plan-badge")).toBeInTheDocument();
        });

        const badge = screen.getByTestId("active-plan-badge");
        expect(badge).toHaveTextContent("[planManager.status.unknown]");
        expect(badge.textContent).not.toContain("SOME_ARBITRARY_UNRECOGNIZED_STATUS_STRING");
        expect(badge).toHaveAttribute("title", "planManager.status.unknown");
      });

      it(`disables open plan button and shows empty placeholder when active plan is unset on ${surface}`, async () => {
        mockInvoke.mockImplementation((cmd: string) => {
          if (cmd === "list_available_plans") {
            return Promise.resolve(mockPlanDetailsEmpty);
          }
          if (cmd === "get_plan_manager_location") {
            return Promise.resolve("C:\\mock\\repo");
          }
          if (cmd === "get_project_plan_slots") {
            return Promise.resolve([
              {
                slot_id: "primary",
                repo_path: "C:\\mock\\repo",
                plan_path: null,
              },
            ]);
          }
          return Promise.resolve();
        });

        render(
          <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
            <PlanFileManagerCard surface={surface} repoPath="C:\\mock\\repo" />
          </LanguageContext.Provider>
        );

        await waitFor(() => {
          expect(screen.getByTestId("btn-open-plan")).toBeInTheDocument();
        });

        expect(screen.getByTestId("btn-open-plan")).toBeDisabled();
        expect(screen.getByTestId("empty-plan-path-field")).toBeInTheDocument();
        expect(screen.queryByTestId("active-plan-path-display")).not.toBeInTheDocument();
        expect(screen.queryByTestId("btn-create-plan")).not.toBeInTheDocument();
      });

      it(`removes leading file icon from plan file select button on ${surface}`, async () => {
        mockInvoke.mockImplementation((cmd: string) => {
          if (cmd === "list_available_plans") {
            return Promise.resolve(mockPlanDetailsWithPlans);
          }
          if (cmd === "get_plan_manager_location") {
            return Promise.resolve("C:\\mock\\repo");
          }
          return Promise.resolve();
        });

        render(
          <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
            <PlanFileManagerCard surface={surface} repoPath="C:\\mock\\repo" />
          </LanguageContext.Provider>
        );

        await waitFor(() => {
          expect(screen.getByTestId("btn-select-plan-file")).toBeInTheDocument();
        });

        const selectBtn = screen.getByTestId("btn-select-plan-file");
        expect(selectBtn.textContent).not.toMatch(/[\u{1F300}-\u{1F9FF}]/u);
        expect(selectBtn).toHaveTextContent("planManager.planFileButtonLabel");
      });
    });
  });

  describe("V0.27.3 Project-Scoped Plan Slots Management", () => {
    it("renders permanent primary slot and allows adding/removing a second slot", async () => {
      const user = userEvent.setup();
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "get_project_plan_slots") {
          return Promise.resolve([
            { slot_id: "primary", repo_path: "C:\\mock\\repo", plan_path: ".plan/V0.26.3.md" },
          ]);
        }
        if (cmd === "list_available_plans") {
          return Promise.resolve(mockPlanDetailsWithPlans);
        }
        if (cmd === "set_project_plan_slots") {
          return Promise.resolve();
        }
        return Promise.resolve();
      });

      render(
        <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" repoPath="C:\\mock\\repo" />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("project-plan-slot-primary")).toBeInTheDocument();
      });

      // Primary slot exists and secondary slot does not exist yet
      expect(screen.queryByTestId("project-plan-slot-secondary")).not.toBeInTheDocument();
      expect(screen.getByTestId("btn-add-slot")).toBeInTheDocument();

      // Click add second slot
      await user.click(screen.getByTestId("btn-add-slot"));

      await waitFor(() => {
        expect(screen.getByTestId("project-plan-slot-secondary")).toBeInTheDocument();
      });

      // After adding, set_project_plan_slots should be called with both slots
      expect(mockInvoke).toHaveBeenCalledWith("set_project_plan_slots", {
        surface: "codex",
        slots: [
          { slot_id: "primary", repo_path: "C:\\mock\\repo", plan_path: ".plan/V0.26.3.md" },
          { slot_id: "secondary", repo_path: null, plan_path: null },
        ],
      });

      // Add button is hidden when max (2) slots are active
      expect(screen.queryByTestId("btn-add-slot")).not.toBeInTheDocument();

      // Secondary slot has a remove button
      const removeBtn = screen.getByTestId("btn-remove-slot-secondary");
      expect(removeBtn).toBeInTheDocument();
      expect(removeBtn).toHaveAttribute("title", "planManager.removeSlotTooltip");

      // Primary slot does NOT have a remove button (permanent)
      expect(screen.queryByTestId("btn-remove-slot-primary")).not.toBeInTheDocument();

      // Click remove on secondary slot
      await user.click(removeBtn);

      await waitFor(() => {
        expect(screen.queryByTestId("project-plan-slot-secondary")).not.toBeInTheDocument();
      });

      // set_project_plan_slots should be called with only primary slot
      expect(mockInvoke).toHaveBeenCalledWith("set_project_plan_slots", {
        surface: "codex",
        slots: [
          { slot_id: "primary", repo_path: "C:\\mock\\repo", plan_path: ".plan/V0.26.3.md" },
        ],
      });

      // Add button reappears
      expect(screen.getByTestId("btn-add-slot")).toBeInTheDocument();
    });

    it("allows selecting plan file for a slot and directly binds enclosing repo", async () => {
      const user = userEvent.setup();
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "get_project_plan_slots") {
          return Promise.resolve([
            { slot_id: "primary", repo_path: "C:\\mock\\repo", plan_path: null },
          ]);
        }
        if (cmd === "select_plan_file_dialog") {
          return Promise.resolve({
            repo_path: "C:\\mock\\other-repo",
            relative_path: ".plan/other.md",
            surface: "codex",
            slot_id: "primary",
          });
        }
        if (cmd === "set_active_plan") {
          return Promise.resolve({
            rel_path: ".plan/other.md",
            abs_path: "C:\\mock\\other-repo\\.plan\\other.md",
            file_name: "other.md",
            title: "Other Plan",
            active_plan_relative_path: ".plan/other.md",
            plans: [],
          });
        }
        if (cmd === "set_project_plan_slots") {
          return Promise.resolve();
        }
        if (cmd === "list_available_plans") {
          return Promise.resolve(mockPlanDetailsEmpty);
        }
        return Promise.resolve();
      });

      render(
        <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" repoPath="C:\\mock\\repo" />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("btn-select-plan-file")).toBeInTheDocument();
      });

      // No separate project folder button exists
      expect(screen.queryByTestId("btn-select-project-primary")).not.toBeInTheDocument();

      await user.click(screen.getByTestId("btn-select-plan-file"));

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith(
          "select_plan_file_dialog",
          expect.objectContaining({ surface: "codex", slotId: "primary" })
        );
      });

      expect(mockInvoke).toHaveBeenCalledWith(
        "set_project_plan_slots",
        expect.objectContaining({
          surface: "codex",
          activeSlotId: "primary",
          activeRepoPath: "C:\\mock\\other-repo",
          slots: [
            {
              slot_id: "primary",
              repo_path: "C:\\mock\\other-repo",
              plan_path: ".plan/other.md",
            },
          ],
        })
      );
    });

    it("verifies cross-surface independence and stale async write protection", async () => {
      const user = userEvent.setup();

      // Test stale async protection: slow plan picker should not resurrect removed secondary slot
      let resolvePlanPicker: (val: any) => void;
      const planPickerPromise = new Promise<any>((resolve) => {
        resolvePlanPicker = resolve;
      });

      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "get_project_plan_slots") {
          return Promise.resolve([
            { slot_id: "primary", repo_path: "C:\\mock\\repo", plan_path: ".plan/V0.26.3.md" },
            { slot_id: "secondary", repo_path: null, plan_path: null },
          ]);
        }
        if (cmd === "select_plan_file_dialog") {
          return planPickerPromise;
        }
        if (cmd === "list_available_plans") {
          return Promise.resolve(mockPlanDetailsWithPlans);
        }
        if (cmd === "set_project_plan_slots") {
          return Promise.resolve();
        }
        return Promise.resolve();
      });

      render(
        <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" repoPath="C:\\mock\\repo" />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("project-plan-slot-secondary")).toBeInTheDocument();
      });

      // 1. Click select plan on secondary slot (starts in-flight async picker)
      const selectPlanBtn = screen.getByTestId("btn-select-plan-file-secondary");
      await user.click(selectPlanBtn);

      // 2. While picker is in-flight, user clicks remove secondary slot
      const removeBtn = screen.getByTestId("btn-remove-slot-secondary");
      await user.click(removeBtn);

      await waitFor(() => {
        expect(screen.queryByTestId("project-plan-slot-secondary")).not.toBeInTheDocument();
      });

      // 3. In-flight plan picker finishes later
      resolvePlanPicker!({
        repo_path: "C:\\mock\\stale-secondary-repo",
        relative_path: ".plan/stale.md",
        surface: "codex",
        slot_id: "secondary",
      });
      await new Promise((r) => setTimeout(r, 50));

      // 4. Stale slot must NOT be resurrected
      expect(screen.queryByTestId("project-plan-slot-secondary")).not.toBeInTheDocument();
    });

    it("resolves distinct Codex project root to the matching slot and loads its plan and roles", async () => {
      const onActivePlanChanged = vi.fn();
      const onRolesChanged = vi.fn();

      mockInvoke.mockImplementation((cmd: string, args?: any) => {
        if (cmd === "get_project_plan_slots") {
          return Promise.resolve([
            { slot_id: "primary", repo_path: "C:\\mock\\repo-a", plan_path: ".plan/plan-a.md" },
            { slot_id: "secondary", repo_path: "C:\\mock\\repo-b", plan_path: ".plan/plan-b.md" },
          ]);
        }
        if (cmd === "list_available_plans") {
          const repo = args?.repoPath;
          return Promise.resolve({
            repo_root: repo,
            plans_dir: `${repo}\\.plan`,
            active_plan_path: `${repo}\\.plan\\plan-${repo?.includes("b") ? "b" : "a"}.md`,
            active_plan_relative_path: `.plan/plan-${repo?.includes("b") ? "b" : "a"}.md`,
            plans: [
              {
                path: `${repo}\\.plan\\plan-${repo?.includes("b") ? "b" : "a"}.md`,
                relative_path: `.plan/plan-${repo?.includes("b") ? "b" : "a"}.md`,
                title: `Plan ${repo?.includes("b") ? "B" : "A"}`,
                status: "APPROVED",
                last_modified_ms: 1700000000000,
                is_active: true,
              },
            ],
          });
        }
        if (cmd === "get_workflow_role_assignment") {
          return Promise.resolve({ assignment: { planner_id: "shared_planner", reviser_id: null }, legacy_conflict: false, conflict_sources: [] });
        }
        return Promise.resolve();
      });

      // Thread running in repo-b: repoPath prop matches secondary slot
      render(
        <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
          <PlanFileManagerCard
            surface="codex"
            repoPath="C:\\mock\\repo-b"
            onActivePlanChanged={onActivePlanChanged}
            onRolesChanged={onRolesChanged}
          />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("active-plan-path-display-secondary")).toBeInTheDocument();
      });

      // Role assignment is surface-wide and does not change with the selected repository.
      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith("get_workflow_role_assignment", { surface: "codex" });
      });

      expect(onRolesChanged).toHaveBeenCalledWith(
        expect.objectContaining({ plannerRole: "shared_planner" })
      );
      expect(onActivePlanChanged).toHaveBeenCalledWith(".plan/plan-b.md");
    });

    it("shows unmatched workspace warning and does not guess another slot's plan when repoPath matches no slot", async () => {
      const onActivePlanChanged = vi.fn();

      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "get_project_plan_slots") {
          return Promise.resolve([
            { slot_id: "primary", repo_path: "C:\\mock\\repo-a", plan_path: ".plan/plan-a.md" },
            { slot_id: "secondary", repo_path: "C:\\mock\\repo-b", plan_path: ".plan/plan-b.md" },
          ]);
        }
        return Promise.resolve();
      });

      // Thread running in repo-c: does not match any configured slot
      render(
        <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
          <PlanFileManagerCard
            surface="codex"
            repoPath="C:\\mock\\repo-c"
            onActivePlanChanged={onActivePlanChanged}
          />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("unmatched-workspace-warning")).toBeInTheDocument();
      });

      // Active plan must NOT silently guess repo-a or repo-b
      expect(onActivePlanChanged).toHaveBeenCalledWith(null);
    });

    it("updates slot repo_path and plan_path when selecting a plan in another repository", async () => {
      const user = userEvent.setup();
      let savedSlots: any[] = [];

      mockInvoke.mockImplementation((cmd: string, args?: any) => {
        if (cmd === "get_project_plan_slots") {
          return Promise.resolve([
            { slot_id: "primary", repo_path: "C:\\mock\\old-repo", plan_path: ".plan/old-plan.md" },
          ]);
        }
        if (cmd === "select_plan_file_dialog") {
          return Promise.resolve({
            repo_path: "C:\\mock\\brand-new-repo",
            relative_path: ".plan/new-plan.md",
            surface: "codex",
            slot_id: "primary",
          });
        }
        if (cmd === "set_active_plan") {
          return Promise.resolve({
            repo_root: "C:\\mock\\brand-new-repo",
            plans_dir: "C:\\mock\\brand-new-repo\\.plan",
            active_plan_path: "C:\\mock\\brand-new-repo\\.plan\\new-plan.md",
            active_plan_relative_path: ".plan/new-plan.md",
            plans: [],
          });
        }
        if (cmd === "list_available_plans") {
          return Promise.resolve({
            repo_root: "C:\\mock\\brand-new-repo",
            plans_dir: "C:\\mock\\brand-new-repo\\.plan",
            active_plan_path: "C:\\mock\\brand-new-repo\\.plan\\new-plan.md",
            active_plan_relative_path: ".plan/new-plan.md",
            plans: [],
          });
        }
        if (cmd === "set_project_plan_slots") {
          savedSlots = args?.slots;
          return Promise.resolve(args?.slots);
        }
        return Promise.resolve();
      });

      render(
        <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" repoPath="C:\\mock\\old-repo" />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("active-plan-path-display")).toBeInTheDocument();
      });

      // No folder picker button exists
      expect(screen.queryByTestId("btn-select-project-primary")).not.toBeInTheDocument();

      await user.click(screen.getByTestId("btn-select-plan-file"));

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith(
          "select_plan_file_dialog",
          expect.objectContaining({ surface: "codex", slotId: "primary" })
        );
      });

      await waitFor(() => {
        expect(savedSlots.length).toBeGreaterThan(0);
      });

      expect(savedSlots[0]).toEqual({
        slot_id: "primary",
        repo_path: "C:\\mock\\brand-new-repo",
        plan_path: ".plan/new-plan.md",
      });
    });

    it("renders single-row plan-centric layout with slot labels, no folder picker buttons, and remove on secondary row", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "get_project_plan_slots") {
          return Promise.resolve([
            { slot_id: "primary", repo_path: "C:\\mock\\repo-1", plan_path: ".plan/v1.md" },
            { slot_id: "secondary", repo_path: null, plan_path: null },
          ]);
        }
        if (cmd === "list_available_plans") {
          return Promise.resolve(mockPlanDetailsWithPlans);
        }
        return Promise.resolve();
      });

      render(
        <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" repoPath="C:\\mock\\repo-1" />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("project-plan-slot-primary")).toBeInTheDocument();
        expect(screen.getByTestId("project-plan-slot-secondary")).toBeInTheDocument();
      });

      // Assert path-only rows have NO Project 1 / Project 2 slot labels
      expect(screen.queryByTestId("slot-label-primary")).not.toBeInTheDocument();
      expect(screen.queryByTestId("slot-label-secondary")).not.toBeInTheDocument();

      // Assert no project folder picker buttons exist anywhere
      expect(screen.queryByTestId("btn-select-project-primary")).not.toBeInTheDocument();
      expect(screen.queryByTestId("btn-select-project-secondary")).not.toBeInTheDocument();

      // Assert primary slot row has plan select and open buttons, but NO remove button
      const primarySlot = screen.getByTestId("project-plan-slot-primary");
      expect(primarySlot).toContainElement(screen.getByTestId("btn-select-plan-file"));
      expect(primarySlot).toContainElement(screen.getByTestId("btn-open-plan"));
      expect(primarySlot.querySelector('[data-testid="btn-remove-slot-secondary"]')).toBeNull();

      // Assert secondary slot row has plan select, open button, and remove button
      const secondarySlot = screen.getByTestId("project-plan-slot-secondary");
      expect(secondarySlot).toContainElement(screen.getByTestId("btn-select-plan-file-secondary"));
      expect(secondarySlot).toContainElement(screen.getByTestId("btn-open-plan-secondary"));
      expect(secondarySlot).toContainElement(screen.getByTestId("btn-remove-slot-secondary"));

      // Secondary empty plan path field displays placeholder directly
      expect(screen.getByTestId("empty-plan-path-field-secondary")).toHaveTextContent("planManager.emptyPlanPlaceholder");
    });
  });

  describe("Antigravity unconfigured surface role-assignment error suppression", () => {
    it("shows shared-role settings errors independently of the selected plan on antigravity", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "list_available_plans") {
          return Promise.resolve(mockPlanDetailsEmpty);
        }
        if (cmd === "get_project_plan_slots") {
          return Promise.resolve([
            { slot_id: "primary", repo_path: "C:\\mock\\repo", plan_path: null },
          ]);
        }
        if (cmd === "get_workflow_role_assignment") {
          return Promise.reject("No active plan file is configured for antigravity surface");
        }
        return Promise.resolve();
      });

      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="antigravity" />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("no-active-plan-warning")).toBeInTheDocument();
      });

      expect(screen.getByTestId("btn-select-plan-file")).toBeInTheDocument();
      expect(screen.getByText("No active plan file is configured for antigravity surface")).toBeInTheDocument();
    });

    it("keeps generic missing-surface error visible on codex surface", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "list_available_plans") {
          return Promise.resolve(mockPlanDetailsEmpty);
        }
        if (cmd === "get_project_plan_slots") {
          return Promise.resolve([
            { slot_id: "primary", repo_path: "C:\\mock\\repo", plan_path: null },
          ]);
        }
        if (cmd === "get_workflow_role_assignment") {
          return Promise.reject("No active plan file is configured for codex surface");
        }
        return Promise.resolve();
      });

      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(
          screen.getByText("No active plan file is configured for codex surface")
        ).toBeInTheDocument();
      });
    });

    it("keeps slot-specific missing-plan error and genuine load errors visible on antigravity", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "list_available_plans") {
          return Promise.resolve(mockPlanDetailsEmpty);
        }
        if (cmd === "get_project_plan_slots") {
          return Promise.resolve([
            { slot_id: "primary", repo_path: "C:\\mock\\repo", plan_path: null },
          ]);
        }
        if (cmd === "get_plan_role_assignment") {
          return Promise.reject('No active plan file is configured for antigravity surface slot "primary"');
        }
        return Promise.resolve();
      });

      const { unmount } = render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="antigravity" />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(
          screen.getByText('No active plan file is configured for antigravity surface slot "primary"')
        ).toBeInTheDocument();
      });

      unmount();

      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "list_available_plans") {
          return Promise.resolve(mockPlanDetailsEmpty);
        }
        if (cmd === "get_project_plan_slots") {
          return Promise.resolve([
            { slot_id: "primary", repo_path: "C:\\mock\\repo", plan_path: null },
          ]);
        }
        if (cmd === "get_plan_role_assignment") {
          return Promise.reject(new Error("Failed to parse .plan/.active-plan.json"));
        }
        return Promise.resolve();
      });

      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="antigravity" />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByText(/Failed to parse \.plan\/\.active-plan\.json/)).toBeInTheDocument();
      });
    });

    it("keeps both project plan rows without active-slot selection buttons", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "get_project_plan_slots") return Promise.resolve([
          { slot_id: "primary", repo_path: "C:\\mock\\repo-a", plan_path: ".plan/one.md" },
          { slot_id: "secondary", repo_path: "C:\\mock\\repo-b", plan_path: ".plan/two.md" },
        ]);
        if (cmd === "get_selected_project_plan_context") return Promise.resolve({
          surface: "codex", repoRoot: "C:\\mock\\repo-a", slotId: "primary", planPath: ".plan/one.md", resolved: true,
        });
        if (cmd === "get_surface_plan_slots") return Promise.resolve({
          repo_root: "C:\\mock\\repo-a", surface: "codex", active_slot_id: "primary", active_plan_path: ".plan/one.md",
          slots: { primary: ".plan/one.md" }, is_unresolved: false,
        });
        if (cmd === "list_available_plans") return Promise.resolve(mockPlanDetailsWithPlans);
        if (cmd === "get_plan_role_assignment") return Promise.resolve({ planner_id: "codex_session", reviser_id: "codex_session" });
        return Promise.resolve();
      });

      render(
        <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" repoPath="C:\\mock\\repo-a" />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("project-plan-slot-primary")).toBeInTheDocument();
        expect(screen.getByTestId("project-plan-slot-secondary")).toBeInTheDocument();
      });
      expect(screen.queryByTestId("btn-activate-slot-primary")).not.toBeInTheDocument();
      expect(screen.queryByTestId("btn-activate-slot-secondary")).not.toBeInTheDocument();
      expect(screen.getByTestId("btn-select-plan-file")).toBeInTheDocument();
      expect(screen.getByTestId("btn-select-plan-file-secondary")).toBeInTheDocument();
      expect(screen.getByTestId("btn-open-plan")).toBeInTheDocument();
      expect(screen.getByTestId("btn-open-plan-secondary")).toBeInTheDocument();
    });
    it("pins the project workflow context through its plan-file picker", async () => {
      const user = userEvent.setup();
      const onPinned = vi.fn();
      const onContextChanged = vi.fn();
      mockInvoke.mockImplementation((cmd: string, args?: any) => {
        if (cmd === "get_project_plan_slots") return Promise.resolve([
          { slot_id: "primary", repo_path: "C:\\mock\\repo-a", plan_path: ".plan/one.md" },
          { slot_id: "secondary", repo_path: "C:\\mock\\repo-b", plan_path: ".plan/two.md" },
        ]);
        if (cmd === "get_selected_project_plan_context") return Promise.resolve({
          surface: "codex", repoRoot: "C:\\mock\\repo-a", slotId: "primary", planPath: ".plan/one.md", resolved: true,
        });
        if (cmd === "get_surface_plan_slots") return Promise.resolve({
          repo_root: "C:\\mock\\repo-a", surface: "codex", active_slot_id: "primary", active_plan_path: ".plan/one.md",
          slots: { primary: ".plan/one.md" }, is_unresolved: false,
        });
        if (cmd === "list_available_plans") return Promise.resolve(mockPlanDetailsWithPlans);
        if (cmd === "get_plan_role_assignment") return Promise.resolve({ planner_id: "codex_session", reviser_id: "codex_session" });
        if (cmd === "select_plan_file_dialog") return Promise.resolve({
          relative_path: ".plan/two.md", repo_path: "C:\\mock\\repo-b", slot_id: "secondary", surface: "codex",
        });
        if (cmd === "set_active_plan") return Promise.resolve(mockPlanDetailsWithPlans);
        if (cmd === "pin_workflow_target") return Promise.resolve({
          repository: args.repoPath, surface: args.surface, slot_id: args.slotId, plan_path: ".plan/two.md",
          sha256_hash: "abc123", file_identity: "id-2",
        });
        if (cmd === "set_project_plan_slots") return Promise.resolve(args.slots);
        return Promise.resolve();
      });

      render(
        <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
          <PlanFileManagerCard
            surface="codex"
            repoPath="C:\\mock\\repo-a"
            onPinnedWorkflowTargetChanged={onPinned}
            onActivePlanContextChanged={onContextChanged}
          />
        </LanguageContext.Provider>
      );

      await waitFor(() => expect(screen.getByTestId("btn-select-plan-file-secondary")).toBeInTheDocument());
      await user.click(screen.getByTestId("btn-select-plan-file-secondary"));
      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith("pin_workflow_target", {
          repoPath: "C:\\mock\\repo-b", surface: "codex", slotId: "secondary",
        });
        expect(onPinned).toHaveBeenCalledWith(expect.objectContaining({
          repository: "C:\\mock\\repo-b", slot_id: "secondary", plan_path: ".plan/two.md",
        }));
      });
      expect(onContextChanged).toHaveBeenCalledWith({
        surface: "codex", repoRoot: "C:\\mock\\repo-b", slotId: "secondary", planPath: ".plan/two.md", resolved: true,
      });
      expect(screen.queryByTestId("btn-activate-slot-primary")).not.toBeInTheDocument();
      expect(screen.queryByTestId("btn-activate-slot-secondary")).not.toBeInTheDocument();
    });

    it("resolves secondary plan while keeping Planner/Reviser shared", async () => {
      const user = userEvent.setup();
      let sharedRole = { planner_id: null as string | null, reviser_id: null as string | null };

      mockInvoke.mockImplementation((cmd: string, args?: any) => {
        if (cmd === "get_project_plan_slots") {
          return Promise.resolve([
            { slot_id: "primary", repo_path: "C:\\mock\\primary-repo", plan_path: ".plan/primary.md" },
            { slot_id: "secondary", repo_path: "C:\\mock\\asr-composer", plan_path: ".plan/v0.3.0.md" },
          ]);
        }
        if (cmd === "get_selected_project_plan_context") {
          return Promise.resolve({
            surface: "codex",
            repoRoot: "C:\\mock\\asr-composer",
            slotId: "secondary",
            planPath: ".plan/v0.3.0.md",
            resolved: true,
          });
        }
        if (cmd === "get_surface_plan_slots") {
          return Promise.resolve({
            repo_root: args?.repoPath,
            surface: "codex",
            active_slot_id: "secondary",
            active_plan_path: ".plan/v0.3.0.md",
            slots: { secondary: ".plan/v0.3.0.md" },
            is_unresolved: false,
          });
        }
        if (cmd === "list_available_plans") {
          return Promise.resolve({
            ...mockPlanDetailsWithPlans,
            active_plan_path: ".plan/v0.3.0.md",
            plans: [
              {
                name: "v0.3.0.md",
                path: `${args?.repoPath}\\.plan\\v0.3.0.md`,
                relative_path: ".plan/v0.3.0.md",
                title: "v0.3.0",
                status: null,
                last_modified_ms: 1700000000000,
                is_active: true,
              },
            ],
          });
        }
        if (cmd === "get_workflow_role_assignment") {
          return Promise.resolve({ assignment: sharedRole, legacy_conflict: false, conflict_sources: [] });
        }
        if (cmd === "set_workflow_role_assignment") {
          sharedRole = {
            planner_id: args?.assignment?.planner_id ?? null,
            reviser_id: args?.assignment?.reviser_id ?? null,
          };
          return Promise.resolve({ assignment: sharedRole, legacy_conflict: false, conflict_sources: [] });
        }
        return Promise.resolve();
      });

      const { unmount } = render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" />
        </LanguageContext.Provider>
      );

      // Role assignment is loaded once for the surface, not from the selected repository.
      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith("get_workflow_role_assignment", { surface: "codex" });
      });
      expect(screen.getByTestId("planner-role-none")).toBeChecked();
      expect(screen.getByTestId("reviser-role-none")).toBeChecked();

      await user.click(screen.getByTestId("planner-role-codex-session"));
      await user.click(screen.getByTestId("reviser-role-codex-session"));

      await waitFor(() => {
        expect(sharedRole).toEqual({
          planner_id: "codex_session",
          reviser_id: "codex_session",
        });
      });

      // Reload and verify persisted roles remain selected
      unmount();
      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" />
        </LanguageContext.Provider>
      );
      await waitFor(() => {
        expect(screen.getByTestId("planner-role-codex-session")).toBeChecked();
        expect(screen.getByTestId("reviser-role-codex-session")).toBeChecked();
      });
    });

    it("reverts to last confirmed role assignment when both save and reload fail", async () => {
      const user = userEvent.setup();
      let getCallCount = 0;
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "get_project_plan_slots") {
          return Promise.resolve([
            { slot_id: "primary", repo_path: "C:\\mock\\repo", plan_path: ".plan/v0.3.0.md" },
          ]);
        }
        if (cmd === "list_available_plans") return Promise.resolve(mockPlanDetailsWithPlans);
        if (cmd === "get_plan_role_assignment") {
          getCallCount += 1;
          if (getCallCount === 1) {
            return Promise.resolve({ planner_id: null, reviser_id: null });
          }
          return Promise.reject(new Error("readback I/O error"));
        }
        if (cmd === "set_plan_role_assignment") {
          return Promise.reject(new Error("permission denied"));
        }
        return Promise.resolve();
      });

      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" repoPath="C:\\mock\\repo" />
        </LanguageContext.Provider>
      );

      await waitFor(() => expect(screen.getByTestId("planner-role-none")).toBeChecked());
      await user.click(screen.getByTestId("planner-role-codex-session"));

      await waitFor(() => {
        expect(screen.getByText(/permission denied.*failed to reload saved role assignment.*readback I\/O error/)).toBeInTheDocument();
      });
      // Must NOT leave unpersisted "Codex (Session)" appearing saved
      expect(screen.getByTestId("planner-role-none")).toBeChecked();
      expect(screen.getByTestId("planner-role-codex-session")).not.toBeChecked();
    });

    it("does not fall back to primary or overwrite parent active plan while selected context is delayed", async () => {
      const user = userEvent.setup();
      const onActivePlanChanged = vi.fn();
      let resolveContext!: (value: any) => void;
      const contextPromise = new Promise((resolve) => {
        resolveContext = resolve;
      });
      const roleReads: string[] = [];
      const roleWrites: Array<{ surface: string; planner_id: string | null; reviser_id: string | null }> = [];
      const slotReads: string[] = [];

      mockInvoke.mockImplementation((cmd: string, args?: any) => {
        if (cmd === "get_project_plan_slots") {
          return Promise.resolve([
            { slot_id: "primary", repo_path: "C:\\mock\\poverello", plan_path: ".plan/V0.28.1.md" },
            { slot_id: "secondary", repo_path: "C:\\mock\\ASR-Composer", plan_path: ".plan/v0.3.0.md" },
          ]);
        }
        if (cmd === "get_selected_project_plan_context") {
          return contextPromise;
        }
        if (cmd === "get_surface_plan_slots") {
          slotReads.push(args?.repoPath);
          const isPrimary = args?.repoPath === "C:\\mock\\poverello";
          return Promise.resolve({
            repo_root: args?.repoPath,
            surface: "codex",
            active_slot_id: isPrimary ? "primary" : "secondary",
            active_plan_path: isPrimary ? ".plan/V0.28.1.md" : ".plan/v0.3.0.md",
            slots: isPrimary ? { primary: ".plan/V0.28.1.md" } : { secondary: ".plan/v0.3.0.md" },
            is_unresolved: false,
          });
        }
        if (cmd === "list_available_plans") {
          return Promise.resolve({
            ...mockPlanDetailsWithPlans,
            active_plan_path: args?.repoPath === "C:\\mock\\poverello" ? ".plan/V0.28.1.md" : ".plan/v0.3.0.md",
            active_plan_relative_path: args?.repoPath === "C:\\mock\\poverello" ? ".plan/V0.28.1.md" : ".plan/v0.3.0.md",
            plans: [],
          });
        }
        if (cmd === "get_workflow_role_assignment") {
          roleReads.push(args?.surface);
          return Promise.resolve({ assignment: { planner_id: null, reviser_id: null }, legacy_conflict: false, conflict_sources: [] });
        }
        if (cmd === "set_workflow_role_assignment") {
          roleWrites.push({
            surface: args?.surface,
            planner_id: args?.assignment?.planner_id ?? null,
            reviser_id: args?.assignment?.reviser_id ?? null,
          });
          return Promise.resolve(args?.assignment);
        }
        return Promise.resolve();
      });

      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" onActivePlanChanged={onActivePlanChanged} />
        </LanguageContext.Provider>
      );

      // Flush get_project_plan_slots microtasks while get_selected_project_plan_context is still pending
      await waitFor(() => {
        expect(screen.getByTestId("project-plan-slot-secondary")).toBeInTheDocument();
      });

      expect(slotReads).toEqual([]);
      expect(roleReads).toEqual(["codex"]);
      expect(onActivePlanChanged).not.toHaveBeenCalledWith(".plan/V0.28.1.md");

      // Now resolve selected project context to ASR-Composer secondary slot
      resolveContext({
        surface: "codex",
        repoRoot: "C:\\mock\\ASR-Composer",
        slotId: "secondary",
        planPath: ".plan/v0.3.0.md",
        resolved: true,
      });

      await waitFor(() => {
        expect(slotReads).toEqual(["C:\\mock\\ASR-Composer"]);
      });
      expect(roleReads).toEqual(["codex"]);
      expect(onActivePlanChanged).not.toHaveBeenCalledWith(".plan/V0.28.1.md");
      expect(screen.getByTestId("planner-role-none")).toBeChecked();
      expect(screen.getByTestId("reviser-role-none")).toBeChecked();

      await user.click(screen.getByTestId("planner-role-codex-session"));
      await user.click(screen.getByTestId("reviser-role-codex-session"));

      await waitFor(() => {
        expect(roleWrites.length).toBe(2);
      });
      expect(roleWrites.every((w) => w.surface === "codex")).toBe(true);
    });

    it("shows one shared role assignment for all registered projects", async () => {
      const user = userEvent.setup();
      const registryState = {
        schema_version: 1,
        projects: [
          { project_id: "proj_poverello", display_name: "Project 1", repo_root: "C:\\mock\\poverello", plan_path: ".plan/V0.28.1.md", created_unix: 1, updated_unix: 1 },
          { project_id: "proj_asr", display_name: "Project 2", repo_root: "C:\\mock\\ASR-Composer", plan_path: ".plan/v0.3.0.md", created_unix: 1, updated_unix: 1 },
        ],
        pending_conflicts: [],
        migrated_from_legacy: true,
      };
      const roleReads: any[] = [];
      mockInvoke.mockImplementation((cmd: string, args?: any) => {
        if (cmd === "get_project_registry") return Promise.resolve(registryState);
        if (cmd === "get_workflow_role_assignment") {
          roleReads.push(args);
          return Promise.resolve({ assignment: { planner_id: "codex_session", reviser_id: null }, legacy_conflict: false, conflict_sources: [] });
        }
        if (cmd === "set_workflow_role_assignment") return Promise.resolve();
        return Promise.resolve();
      });

      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("plan-path-display-proj_poverello")).toHaveTextContent("C:\\mock\\poverello\\.plan\\V0.28.1.md");
        expect(screen.getByTestId("plan-path-display-proj_asr")).toHaveTextContent("C:\\mock\\ASR-Composer\\.plan\\v0.3.0.md");
        expect(roleReads).toHaveLength(1);
      });
      expect(screen.queryByTestId("unresolved-project-context-warning")).not.toBeInTheDocument();
      expect(mockInvoke).not.toHaveBeenCalledWith("get_selected_project_plan_context", expect.anything());
      expect(screen.getByTestId("planner-role-codex-session")).toBeChecked();
      expect(screen.queryByTestId("project-planner-role-proj_asr-codex_session")).not.toBeInTheDocument();
      expect(screen.queryByTestId("project-planner-role-proj_poverello-codex_session")).not.toBeInTheDocument();
      expect(roleReads).toEqual([{ surface: "codex" }]);
      await user.click(screen.getByTestId("planner-role-none"));
      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith("set_workflow_role_assignment", {
          surface: "codex",
          assignment: { planner_id: null, reviser_id: null },
        });
      });
    });

    it("requires one explicit shared save when legacy project role assignments conflict", async () => {
      const user = userEvent.setup();
      mockInvoke.mockImplementation((cmd: string, args?: any) => {
        if (cmd === "get_project_registry") return Promise.resolve({
          schema_version: 1,
          projects: [
            { project_id: "proj_a", display_name: "A", repo_root: "C:\\mock\\a", plan_path: ".plan/a.md", created_unix: 1, updated_unix: 1 },
            { project_id: "proj_b", display_name: "B", repo_root: "C:\\mock\\b", plan_path: ".plan/b.md", created_unix: 1, updated_unix: 1 },
          ], pending_conflicts: [], migrated_from_legacy: true,
        });
        if (cmd === "get_workflow_role_assignment") return Promise.resolve({
          assignment: null, legacy_conflict: true, conflict_sources: ["A", "B"],
        });
        if (cmd === "set_workflow_role_assignment") return Promise.resolve({
          assignment: args.assignment, legacy_conflict: false, conflict_sources: [],
        });
        return Promise.resolve();
      });
      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" />
        </LanguageContext.Provider>
      );
      expect(await screen.findByTestId("shared-role-legacy-conflict")).toBeInTheDocument();
      expect(screen.getByTestId("planner-role-none")).toBeChecked();
      expect(screen.getByTestId("reviser-role-none")).toBeChecked();
      await user.click(screen.getByTestId("planner-role-codex-session"));
      await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("set_workflow_role_assignment", {
        surface: "codex", assignment: { planner_id: "codex_session", reviser_id: null },
      }));
      expect(screen.queryByTestId("shared-role-legacy-conflict")).not.toBeInTheDocument();
    });

    it("renders identical path-only plan rows on both surfaces without project labels, active selectors, or verbatim prefixes", async () => {
      const user = userEvent.setup();
      let registryState = {
        schema_version: 1,
        projects: [
          {
            project_id: "proj_poverello",
            display_name: "Project 1",
            repo_root: "\\\\?\\C:\\mock\\poverello",
            plan_path: ".plan/V0.28.1.md",
            created_unix: 1,
            updated_unix: 1,
          },
          {
            project_id: "proj_asr",
            display_name: "Project 2",
            repo_root: "C:\\mock\\ASR-Composer",
            plan_path: ".plan/v0.3.0.md",
            created_unix: 1,
            updated_unix: 1,
          },
        ],
        pending_conflicts: [
          {
            conflict_id: "plan-conflict-proj_asr",
            kind: "plan_path_conflict",
            repo_root: "C:\\mock\\ASR-Composer",
            surface: null,
            description: "Multiple distinct plan paths found for ASR-Composer",
            options: [
              { source: "codex:secondary", label: ".plan/v0.3.0.md (codex)", plan_path: ".plan/v0.3.0.md" },
            ],
          },
        ],
        migrated_from_legacy: true,
      };

      mockInvoke.mockImplementation((cmd: string, args?: any) => {
        if (cmd === "get_project_registry") return Promise.resolve(registryState);
        if (cmd === "resolve_project_migration_conflict") {
          registryState = { ...registryState, pending_conflicts: [] };
          return Promise.resolve(registryState);
        }
        if (cmd === "get_project_role_assignment") {
          return Promise.resolve({ planner_id: null, reviser_id: null });
        }
        if (cmd === "get_project_plan_slots") {
          return Promise.resolve([
            { slot_id: "primary", repo_path: "\\\\?\\C:\\mock\\poverello", plan_path: ".plan/V0.28.1.md" },
            { slot_id: "secondary", repo_path: "C:\\mock\\ASR-Composer", plan_path: ".plan/v0.3.0.md" },
          ]);
        }
        if (cmd === "get_selected_project_plan_context") {
          return Promise.resolve({
            surface: args?.surface,
            repoRoot: null,
            slotId: null,
            planPath: null,
            resolved: false,
          });
        }
        if (cmd === "get_plan_role_assignment") {
          return Promise.resolve({ planner_id: null, reviser_id: null });
        }
        return Promise.resolve();
      });

      const { unmount } = render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("plan-path-display-proj_poverello")).toBeInTheDocument();
        expect(screen.getByTestId("plan-path-display-proj_asr")).toBeInTheDocument();
      });
      // Clean Windows paths without \\?\ prefix
      expect(screen.getByTestId("plan-path-display-proj_poverello")).toHaveTextContent("C:\\mock\\poverello\\.plan\\V0.28.1.md");
      expect(screen.getByTestId("plan-path-display-proj_asr")).toHaveTextContent("C:\\mock\\ASR-Composer\\.plan\\v0.3.0.md");
      expect(screen.queryByText(/\\\\\?\\/)).not.toBeInTheDocument();

      // Path-only rows: no active selector, project cards, or priority labels.
      expect(screen.queryByTestId("slot-label-primary")).not.toBeInTheDocument();
      expect(screen.queryByTestId("slot-label-secondary")).not.toBeInTheDocument();
      expect(screen.queryByText(/Project 1|Project 2|プロジェクト 1|プロジェクト 2/)).not.toBeInTheDocument();
      expect(screen.queryByTestId("shared-active-project-select")).not.toBeInTheDocument();
      expect(screen.queryByTestId("unified-project-list")).not.toBeInTheDocument();
      expect(screen.queryByTestId("btn-select-project-proj_asr")).not.toBeInTheDocument();
      expect(screen.queryByTestId("btn-rename-project-proj_asr")).not.toBeInTheDocument();

      // File-picker & open controls remain available on both rows
      expect(screen.getByTestId("btn-select-plan-file-proj_poverello")).toBeInTheDocument();
      expect(screen.getByTestId("btn-open-plan-proj_poverello")).toBeInTheDocument();
      expect(screen.getByTestId("btn-select-plan-file-proj_asr")).toBeInTheDocument();
      expect(screen.getByTestId("btn-open-plan-proj_asr")).toBeInTheDocument();
      expect(screen.getByTestId("btn-remove-project-proj_asr")).toBeInTheDocument();
      expect(screen.getByTestId("btn-add-project-plan")).toBeInTheDocument();
      expect(screen.queryByTestId("btn-remove-slot-secondary")).not.toBeInTheDocument();
      expect(screen.queryByTestId("unresolved-project-context-warning")).not.toBeInTheDocument();

      // Conflict banner rendered and resolvable
      expect(screen.getByTestId("migration-conflict-banner")).toBeInTheDocument();
      await user.click(screen.getByTestId("btn-resolve-conflict-plan-conflict-proj_asr"));
      await waitFor(() => {
        expect(screen.queryByTestId("migration-conflict-banner")).not.toBeInTheDocument();
      });

      unmount();

      // Antigravity surface renders the exact same path-only rows
      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="antigravity" />
        </LanguageContext.Provider>
      );
      await waitFor(() => {
        expect(screen.getByTestId("plan-path-display-proj_poverello")).toHaveTextContent("C:\\mock\\poverello\\.plan\\V0.28.1.md");
        expect(screen.getByTestId("plan-path-display-proj_asr")).toHaveTextContent("C:\\mock\\ASR-Composer\\.plan\\v0.3.0.md");
      });
      expect(screen.queryByTestId("slot-label-primary")).not.toBeInTheDocument();
      expect(screen.queryByTestId("shared-active-project-select")).not.toBeInTheDocument();
    });

    it("updates only the chosen project's plan path without selecting or pinning a project", async () => {
      const user = userEvent.setup();
      let registryState: any = {
        schema_version: 1,
        projects: [
          { project_id: "proj_b", display_name: "Project 2", repo_root: "C:\\mock\\b", plan_paths: { codex: ".plan/b.md", antigravity: ".plan/ag-b.md" }, created_unix: 1, updated_unix: 1 },
          { project_id: "proj_a", display_name: "Project 1", repo_root: "C:\\mock\\a", plan_paths: { codex: ".plan/a.md", antigravity: ".plan/ag-a.md" }, created_unix: 1, updated_unix: 1 },
        ],
        pending_conflicts: [],
        migrated_from_legacy: false,
      };
      const writes: any[] = [];
      mockInvoke.mockImplementation((cmd: string, args?: any) => {
        if (cmd === "get_project_registry") return Promise.resolve(registryState);
        if (cmd === "get_project_role_assignment") return Promise.resolve({ planner_id: null, reviser_id: null });
        if (cmd === "select_plan_file_dialog") {
          expect(args).toEqual({ repoPath: "C:\\mock\\a", surface: "codex", dialogTitle: expect.any(String) });
          return Promise.resolve({ repo_path: "C:\\mock\\a", relative_path: ".plan/new-a.md", surface: "codex" });
        }
        if (cmd === "set_project_plan_path") {
          writes.push(args);
          registryState = { ...registryState, projects: registryState.projects.map((project: any) => project.project_id === args.projectId ? { ...project, plan_paths: { ...project.plan_paths, [args.surface]: args.planPath } } : project) };
          return Promise.resolve(registryState);
        }
        return Promise.resolve();
      });

      const view = render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" />
        </LanguageContext.Provider>
      );
      await waitFor(() => expect(screen.getByTestId("plan-path-display-proj_a")).toHaveTextContent("C:\\mock\\a\\.plan\\a.md"));
      await user.click(screen.getByTestId("btn-select-plan-file-proj_a"));
      await waitFor(() => expect(screen.getByTestId("plan-path-display-proj_a")).toHaveTextContent("C:\\mock\\a\\.plan\\new-a.md"));
      expect(screen.getByTestId("plan-path-display-proj_b")).toHaveTextContent("C:\\mock\\b\\.plan\\b.md");
      expect(writes).toEqual([{ projectId: "proj_a", planPath: ".plan/new-a.md", surface: "codex" }]);
      view.unmount();
      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="antigravity" />
        </LanguageContext.Provider>
      );
      await waitFor(() => expect(screen.getByTestId("plan-path-display-proj_a")).toHaveTextContent("C:\\mock\\a\\.plan\\ag-a.md"));
      for (const forbidden of ["set_active_plan", "pin_workflow_target", "set_project_plan_slots", "get_selected_project_plan_context"]) {
        expect(mockInvoke).not.toHaveBeenCalledWith(forbidden, expect.anything());
      }
    });

    it("switches the clicked row to another registered repository without leaving duplicate rows", async () => {
      const user = userEvent.setup();
      let registryState: any = {
        schema_version: 1,
        projects: [
          { project_id: "proj_a", display_name: "Project 1", repo_root: "C:\\mock\\a", plan_path: ".plan/a.md", created_unix: 1, updated_unix: 1 },
          { project_id: "proj_b", display_name: "Project 2", repo_root: "C:\\mock\\b", plan_path: ".plan/b.md", created_unix: 1, updated_unix: 1 },
        ],
        pending_conflicts: [],
        migrated_from_legacy: false,
      };
      mockInvoke.mockImplementation((cmd: string, args?: any) => {
        if (cmd === "get_project_registry") return Promise.resolve(registryState);
        if (cmd === "get_project_role_assignment") return Promise.resolve({ planner_id: null, reviser_id: null });
        if (cmd === "select_plan_file_dialog") return Promise.resolve({ repo_path: "C:\\mock\\b", relative_path: ".plan/new-b.md", surface: "codex" });
        if (cmd === "replace_project_plan_association") {
          registryState = {
            ...registryState,
            projects: registryState.projects
              .map((project: any) => project.project_id === args.sourceProjectId
                ? { ...project, plan_paths: { codex: null, antigravity: project.plan_path } }
                : project.repo_root === args.targetRepoPath
                  ? { ...project, plan_paths: { codex: args.targetPlanPath, antigravity: project.plan_path } }
                  : project),
          };
          return Promise.resolve(registryState);
        }
        return Promise.resolve();
      });
      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" />
        </LanguageContext.Provider>
      );
      await waitFor(() => expect(screen.getByTestId("btn-select-plan-file-proj_a")).toBeInTheDocument());
      await user.click(screen.getByTestId("btn-select-plan-file-proj_a"));
      await waitFor(() => expect(screen.getByTestId("plan-path-display-proj_b")).toHaveTextContent("C:\\mock\\b\\.plan\\new-b.md"));
      expect(screen.queryByTestId("project-registry-confirm")).not.toBeInTheDocument();
      expect(screen.queryByTestId("empty-plan-path-proj_a")).not.toBeInTheDocument();
      expect(screen.getByTestId("plan-path-display-proj_b")).toHaveTextContent("C:\\mock\\b\\.plan\\new-b.md");
      expect(registryState.projects).toHaveLength(2);
      expect(mockInvoke).toHaveBeenCalledWith("replace_project_plan_association", {
        sourceProjectId: "proj_a",
        targetRepoPath: "C:\\mock\\b",
        targetPlanPath: ".plan/new-b.md",
        surface: "codex",
      });
      expect(screen.getByText(/planManager\.planAssociationSwitched/)).toBeInTheDocument();
      expect(mockInvoke).not.toHaveBeenCalledWith("set_active_plan", expect.anything());
      expect(mockInvoke).not.toHaveBeenCalledWith("pin_workflow_target", expect.anything());
    });

    it("replaces an existing row with a newly selected repository instead of adding a third", async () => {
      const user = userEvent.setup();
      let registryState: any = {
        schema_version: 1,
        projects: [
          { project_id: "proj_asr", display_name: "Project 1", repo_root: "C:\\mock\\ASR-Composer", plan_path: ".plan/v0.3.0.md", created_unix: 1, updated_unix: 1 },
          { project_id: "proj_pov", display_name: "Project 2", repo_root: "C:\\mock\\poverello", plan_path: ".plan/V0.28.1.md", created_unix: 1, updated_unix: 1 },
        ],
        pending_conflicts: [],
        migrated_from_legacy: false,
      };
      mockInvoke.mockImplementation((cmd: string, args?: any) => {
        if (cmd === "get_project_registry") return Promise.resolve(registryState);
        if (cmd === "get_project_role_assignment") return Promise.resolve({ planner_id: null, reviser_id: null });
        if (cmd === "select_plan_file_dialog") return Promise.resolve({ repo_path: "C:\\mock\\NCVROC", relative_path: ".plan/v0.34.1.md", surface: "codex" });
        if (cmd === "replace_project_plan_association") {
          registryState = {
            ...registryState,
            projects: [
              ...registryState.projects.map((project: any) => project.project_id === args.sourceProjectId
                ? { ...project, plan_paths: { codex: null, antigravity: project.plan_path } }
                : project),
              { project_id: "proj_ncvroc", display_name: "Project 3", repo_root: args.targetRepoPath, plan_paths: { codex: args.targetPlanPath }, created_unix: 3, updated_unix: 3 },
            ],
          };
          return Promise.resolve(registryState);
        }
        return Promise.resolve();
      });
      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" />
        </LanguageContext.Provider>
      );
      await waitFor(() => expect(screen.getByTestId("btn-select-plan-file-proj_asr")).toBeInTheDocument());
      await user.click(screen.getByTestId("btn-select-plan-file-proj_asr"));
      await waitFor(() => expect(screen.getByTestId("plan-path-display-proj_ncvroc")).toHaveTextContent("C:\\mock\\NCVROC\\.plan\\v0.34.1.md"));
      expect(screen.queryByTestId("project-registry-confirm")).not.toBeInTheDocument();
      expect(mockInvoke).not.toHaveBeenCalledWith("register_project_in_registry", expect.anything());
      expect(screen.queryByTestId("empty-plan-path-proj_asr")).not.toBeInTheDocument();
      expect(screen.getByTestId("plan-path-display-proj_pov")).toBeInTheDocument();
      expect(registryState.projects).toHaveLength(3);
      expect(mockInvoke).toHaveBeenCalledWith("replace_project_plan_association", {
        sourceProjectId: "proj_asr",
        targetRepoPath: "C:\\mock\\NCVROC",
        targetPlanPath: ".plan/v0.34.1.md",
        surface: "codex",
      });
    });

    it("adds a third repository only through the explicit add action", async () => {
      const user = userEvent.setup();
      let registryState: any = {
        schema_version: 1,
        projects: [
          { project_id: "proj_a", display_name: "Project 1", repo_root: "C:\\mock\\a", plan_path: ".plan/a.md", created_unix: 1, updated_unix: 1 },
          { project_id: "proj_b", display_name: "Project 2", repo_root: "C:\\mock\\b", plan_path: ".plan/b.md", created_unix: 1, updated_unix: 1 },
        ],
        pending_conflicts: [],
        migrated_from_legacy: false,
      };
      mockInvoke.mockImplementation((cmd: string, args?: any) => {
        if (cmd === "get_project_registry") return Promise.resolve(registryState);
        if (cmd === "get_project_role_assignment") return Promise.resolve({ planner_id: null, reviser_id: null });
        if (cmd === "select_plan_file_dialog") {
          expect(args).toEqual({ surface: "codex", dialogTitle: expect.any(String) });
          return Promise.resolve({ repo_path: "C:\\mock\\c", relative_path: ".plan/c.md", surface: "codex" });
        }
        if (cmd === "register_project_in_registry") {
          registryState = {
            ...registryState,
          projects: [...registryState.projects, { project_id: "proj_c", display_name: "Project 3", repo_root: args.repoPath, plan_paths: { [args.surface]: args.planPath }, created_unix: 3, updated_unix: 3 }],
          };
          return Promise.resolve(registryState);
        }
        return Promise.resolve();
      });

      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" />
        </LanguageContext.Provider>
      );
      await waitFor(() => expect(screen.getByTestId("btn-add-project-plan")).toBeInTheDocument());
      await user.click(screen.getByTestId("btn-add-project-plan"));

      await waitFor(() => expect(screen.getByTestId("plan-path-display-proj_c")).toHaveTextContent("C:\\mock\\c\\.plan\\c.md"));
      expect(screen.getByTestId("plan-path-display-proj_a")).toHaveTextContent("C:\\mock\\a\\.plan\\a.md");
      expect(screen.getByTestId("plan-path-display-proj_b")).toHaveTextContent("C:\\mock\\b\\.plan\\b.md");
      expect(mockInvoke).toHaveBeenCalledWith("register_project_in_registry", { repoPath: "C:\\mock\\c", planPath: ".plan/c.md", surface: "codex" });
      expect(screen.getByText(/planManager\.planRepositoryAdded/)).toBeInTheDocument();
      for (const forbidden of ["set_active_plan", "pin_workflow_target", "set_project_plan_slots"]) {
        expect(mockInvoke).not.toHaveBeenCalledWith(forbidden, expect.anything());
      }
    });

    it("removes the selected surface association immediately and hides the empty project row", async () => {
      const user = userEvent.setup();
      let registryState: any = {
        schema_version: 1,
        projects: [
          { project_id: "proj_a", display_name: "Project 1", repo_root: "C:\\mock\\a", plan_path: ".plan/a.md", created_unix: 1, updated_unix: 1 },
          { project_id: "proj_b", display_name: "Project 2", repo_root: "C:\\mock\\b", plan_path: ".plan/b.md", created_unix: 1, updated_unix: 1 },
        ],
        pending_conflicts: [],
        migrated_from_legacy: false,
      };
      mockInvoke.mockImplementation((cmd: string, args?: any) => {
        if (cmd === "get_project_registry") return Promise.resolve(registryState);
        if (cmd === "get_project_role_assignment") return Promise.resolve({ planner_id: null, reviser_id: null });
        if (cmd === "unregister_project_from_registry") {
          registryState = { ...registryState, projects: registryState.projects.map((project: any) => project.project_id === args.projectId ? { ...project, plan_paths: { codex: null, antigravity: project.plan_path } } : project) };
          return Promise.resolve(registryState);
        }
        return Promise.resolve();
      });
      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" />
        </LanguageContext.Provider>
      );
      await waitFor(() => expect(screen.getByTestId("btn-remove-project-proj_a")).toBeInTheDocument());
      await user.click(screen.getByTestId("btn-remove-project-proj_a"));
      await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("unregister_project_from_registry", { projectId: "proj_a", surface: "codex" }));
      expect(screen.queryByTestId("empty-plan-path-proj_a")).not.toBeInTheDocument();
      expect(screen.queryByTestId("plan-path-display-proj_a")).not.toBeInTheDocument();
      expect(screen.getByTestId("plan-path-display-proj_b")).toBeInTheDocument();
      expect(screen.getByText(/planManager\.planAssociationRemoved/)).toBeInTheDocument();
    });

    it("does not write when the plan picker is cancelled", async () => {
      const user = userEvent.setup();
      const registryState = {
        schema_version: 1,
        projects: [{ project_id: "proj_a", display_name: "Project 1", repo_root: "C:\\mock\\a", plan_path: ".plan/a.md", created_unix: 1, updated_unix: 1 }],
        pending_conflicts: [],
        migrated_from_legacy: false,
      };
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "get_project_registry") return Promise.resolve(registryState);
        if (cmd === "get_project_role_assignment") return Promise.resolve({ planner_id: null, reviser_id: null });
        if (cmd === "select_plan_file_dialog") return Promise.resolve(null);
        return Promise.resolve();
      });
      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" />
        </LanguageContext.Provider>
      );
      await waitFor(() => expect(screen.getByTestId("btn-select-plan-file-proj_a")).toBeInTheDocument());
      await user.click(screen.getByTestId("btn-select-plan-file-proj_a"));
      expect(mockInvoke).not.toHaveBeenCalledWith("register_project_in_registry", expect.anything());
      expect(mockInvoke).not.toHaveBeenCalledWith("set_project_plan_path", expect.anything());
    });

    it("rejects a malformed relative plan path before writing the registry", async () => {
      const user = userEvent.setup();
      const registryState = {
        schema_version: 1,
        projects: [{ project_id: "proj_a", display_name: "Project 1", repo_root: "C:\\mock\\a", plan_path: ".plan/a.md", created_unix: 1, updated_unix: 1 }],
        pending_conflicts: [],
        migrated_from_legacy: false,
      };
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "get_project_registry") return Promise.resolve(registryState);
        if (cmd === "get_project_role_assignment") return Promise.resolve({ planner_id: null, reviser_id: null });
        if (cmd === "select_plan_file_dialog") return Promise.resolve({ repo_path: "C:\\mock\\other", relative_path: "../outside.md", surface: "codex" });
        return Promise.resolve();
      });
      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" />
        </LanguageContext.Provider>
      );
      await waitFor(() => expect(screen.getByTestId("btn-select-plan-file-proj_a")).toBeInTheDocument());
      await user.click(screen.getByTestId("btn-select-plan-file-proj_a"));
      expect(await screen.findByText(/planManager\.planSelectionFailed/)).toBeInTheDocument();
      expect(mockInvoke).not.toHaveBeenCalledWith("register_project_in_registry", expect.anything());
      expect(mockInvoke).not.toHaveBeenCalledWith("set_project_plan_path", expect.anything());
      expect(screen.getByTestId("plan-path-display-proj_a")).toHaveTextContent("C:\\mock\\a\\.plan\\a.md");
    });

    it("shows actionable feedback and preserves the last registry view when persistence fails", async () => {
      const user = userEvent.setup();
      const registryState = {
        schema_version: 1,
        projects: [{ project_id: "proj_a", display_name: "Project 1", repo_root: "C:\\mock\\a", plan_path: ".plan/a.md", created_unix: 1, updated_unix: 1 }],
        pending_conflicts: [],
        migrated_from_legacy: false,
      };
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "get_project_registry") return Promise.resolve(registryState);
        if (cmd === "get_project_role_assignment") return Promise.resolve({ planner_id: null, reviser_id: null });
        if (cmd === "select_plan_file_dialog") return Promise.resolve({ repo_path: "C:\\mock\\new", relative_path: ".plan/new.md", surface: "codex" });
        if (cmd === "replace_project_plan_association") return Promise.reject(new Error("write failed"));
        return Promise.resolve();
      });
      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" />
        </LanguageContext.Provider>
      );
      await waitFor(() => expect(screen.getByTestId("btn-select-plan-file-proj_a")).toBeInTheDocument());
      await user.click(screen.getByTestId("btn-select-plan-file-proj_a"));
      expect(await screen.findByText(/planManager\.planSelectionFailed/)).toBeInTheDocument();
      expect(screen.getByTestId("plan-path-display-proj_a")).toHaveTextContent("C:\\mock\\a\\.plan\\a.md");
      expect(screen.queryByTestId("plan-path-display-proj_new")).not.toBeInTheDocument();
      expect(screen.queryByText(/planManager\.planRepositoryAdded/)).not.toBeInTheDocument();
    });

    it("offers a targeted mirror resync after sync failure and clears the warning on success", async () => {
      const user = userEvent.setup();
      const failedRegistry: any = {
        schema_version: 1,
        projects: [
          { project_id: "proj_asr", display_name: "ASR", repo_root: "C:\\mock\\ASR-Composer", plan_paths: { codex: ".plan/v0.5.0.md" }, created_unix: 1, updated_unix: 1 },
          { project_id: "proj_pov", display_name: "Poverello", repo_root: "C:\\mock\\poverello", plan_paths: { codex: ".plan/V0.28.3.md" }, created_unix: 1, updated_unix: 1 },
        ],
        pending_conflicts: [],
        migrated_from_legacy: false,
        mirror_sync_error: "proj_asr (codex): write failed; proj_pov (antigravity): other surface failed",
      };
      mockInvoke.mockImplementation((cmd: string, args?: any) => {
        if (cmd === "get_project_registry") return Promise.resolve(failedRegistry);
        if (cmd === "get_project_role_assignment") return Promise.resolve({ planner_id: null, reviser_id: null });
        if (cmd === "resync_repo_active_plan_mirror") {
          expect(args).toEqual({ projectId: "proj_asr", surface: "codex" });
          return Promise.resolve({ ...failedRegistry, mirror_sync_error: null });
        }
        return Promise.resolve();
      });

      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" />
        </LanguageContext.Provider>
      );

      expect(await screen.findByText(/mirror sync failed/)).toBeInTheDocument();
      expect(screen.queryByTestId("btn-resync-plan-mirror-proj_pov")).not.toBeInTheDocument();
      await user.click(screen.getByTestId("btn-resync-plan-mirror-proj_asr"));
      await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("resync_repo_active_plan_mirror", {
        projectId: "proj_asr",
        surface: "codex",
      }));
      await waitFor(() => expect(screen.queryByText(/mirror sync failed/)).not.toBeInTheDocument());
      expect(screen.getByText("Plan selection mirror resynchronized")).toBeInTheDocument();
      expect(screen.getByTestId("plan-path-display-proj_pov")).toBeInTheDocument();
    });

    it("shows one shared role assignment read error without project-specific editors", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "get_project_registry") return Promise.resolve({
          schema_version: 1,
          projects: [{ project_id: "proj_read_error", display_name: "Project", repo_root: "C:\\mock\\read-error", plan_path: ".plan/a.md", created_unix: 1, updated_unix: 1 }],
          pending_conflicts: [],
          migrated_from_legacy: false,
        });
        if (cmd === "get_workflow_role_assignment") return Promise.reject(new Error("role read failed"));
        return Promise.resolve();
      });
      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <PlanFileManagerCard surface="codex" />
        </LanguageContext.Provider>
      );
      await waitFor(() => expect(screen.getByText("Error: role read failed")).toBeInTheDocument());
      expect(screen.queryByTestId("project-role-settings-proj_read_error")).not.toBeInTheDocument();
      expect(screen.getByTestId("planner-role-group")).toBeInTheDocument();
    });
  });
});
