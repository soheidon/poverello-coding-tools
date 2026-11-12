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

      expect(mockInvoke).toHaveBeenCalledWith("set_project_plan_slots", {
        surface: "codex",
        slots: [
          {
            slot_id: "primary",
            repo_path: "C:\\mock\\other-repo",
            plan_path: ".plan/other.md",
          },
        ],
      });
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
        if (cmd === "get_plan_role_assignment") {
          if (args?.repoPath === "C:\\mock\\repo-b") {
            return Promise.resolve({ planner_id: "custom_planner_b", reviser_id: null });
          }
          return Promise.resolve({ planner_id: "default_planner_a", reviser_id: null });
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

      // Role assignment must be queried for repo-b, not repo-a
      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith("get_plan_role_assignment", {
          surface: "codex",
          repoPath: "C:\\mock\\repo-b",
        });
      });

      expect(onRolesChanged).toHaveBeenCalledWith(
        expect.objectContaining({ plannerRole: "custom_planner_b" })
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

      // Assert slot labels exist
      expect(screen.getByTestId("slot-label-primary")).toHaveTextContent("planManager.slotPrimary:");
      expect(screen.getByTestId("slot-label-secondary")).toHaveTextContent("planManager.slotSecondary:");

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
});
