import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import PlanFileManagerCard from "./PlanFileManagerCard";
import { LanguageContext } from "../i18n";
import type { PlanFileDetails } from "../types";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: async (cmd: string, args?: unknown) => {
    const res = await mockInvoke(cmd, args);
    if (cmd === "get_plan_manager_location" && res === undefined) {
      return "C:\\mock\\repo";
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
  });

  it("renders warning banner when no active plan is specified", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(mockPlanDetailsEmpty);
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
    });

    expect(screen.getByText(".plan/V0.26.3.md")).toBeInTheDocument();
    expect(screen.getByText("Plan Manager & Role Separation")).toBeInTheDocument();
    expect(screen.getByTestId("active-plan-badge")).toHaveTextContent("[APPROVED]");
  });

  it("allows selecting an existing plan by input and clicking confirm", async () => {
    const user = userEvent.setup();
    const onActivePlanChanged = vi.fn();

    mockInvoke.mockImplementation((cmd: string, args: any) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(mockPlanDetailsWithPlans);
      }
      if (cmd === "set_active_plan") {
        return Promise.resolve({
          ...mockPlanDetailsWithPlans,
          active_plan_relative_path: args.relativePath,
        });
      }
      return Promise.resolve();
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <PlanFileManagerCard
          surface="codex"
          onActivePlanChanged={onActivePlanChanged}
        />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("plan-file-input")).not.toBeDisabled();
    });

    const input = screen.getByTestId("plan-file-input");
    await user.clear(input);
    await user.type(input, "feature-xyz.md");

    const confirmBtn = screen.getByTestId("btn-confirm-plan");
    expect(confirmBtn).not.toBeDisabled();
    await user.click(confirmBtn);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("set_active_plan", {
        relativePath: ".plan/feature-xyz.md",
        repoPath: "C:\\mock\\repo",
      });
    });
  });

  it("creates a new plan via confirm button when file does not exist", async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation((cmd: string, args: any) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(mockPlanDetailsWithPlans);
      }
      if (cmd === "create_plan_file") {
        return Promise.resolve({
          ...mockPlanDetailsWithPlans,
          active_plan_relative_path: `.plan/${args.request.name}`,
        });
      }
      return Promise.resolve();
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <PlanFileManagerCard surface="codex" />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("plan-file-input")).not.toBeDisabled();
    });

    const input = screen.getByTestId("plan-file-input");
    await user.clear(input);
    await user.type(input, "V0.26.4-next"); // omits .md, will auto-complete

    const confirmBtn = screen.getByTestId("btn-confirm-plan");
    await user.click(confirmBtn);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("create_plan_file", {
        request: {
          name: "V0.26.4-next.md",
          folder: ".plan",
          set_as_active: true,
        },
        repoPath: "C:\\mock\\repo",
      });
    });
  });

  it("disables file input and confirm button when no folder is selected", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(mockPlanDetailsEmpty);
      }
      return Promise.resolve();
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <PlanFileManagerCard surface="codex" />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("plan-file-input")).toBeInTheDocument();
    });

    expect(screen.getByTestId("plan-file-input")).toBeDisabled();
    expect(screen.getByTestId("btn-confirm-plan")).toBeDisabled();
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
      expect(screen.getByTestId("btn-select-folder")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("btn-select-folder"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("select_plan_file_dialog", {
        repoPath: "C:\\mock\\repo",
        surface: "codex",
        dialogTitle: expect.any(String),
      });
      expect(mockInvoke).toHaveBeenCalledWith("set_active_plan", {
        relativePath: ".plan/feature-xyz.md",
        repoPath: "C:\\mock\\repo",
      });
    });

    expect(onActivePlanChanged).toHaveBeenCalledWith(".plan/feature-xyz.md");
  });

  it("rejects path traversal in file input and prevents mutation", async () => {
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
      expect(screen.getByTestId("plan-file-input")).not.toBeDisabled();
    });

    const input = screen.getByTestId("plan-file-input");
    await user.clear(input);
    await user.type(input, "../secret.md");

    const confirmBtn = screen.getByTestId("btn-confirm-plan");
    await user.click(confirmBtn);

    expect(mockInvoke).not.toHaveBeenCalledWith("set_active_plan", expect.anything());
    expect(mockInvoke).not.toHaveBeenCalledWith("create_plan_file", expect.anything());
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
    expect(screen.getByTestId("btn-select-folder")).toBeInTheDocument();
    expect(screen.getByTestId("btn-refresh-plans")).toBeDisabled();
    expect(mockInvoke).not.toHaveBeenCalledWith("list_available_plans", expect.anything());
  });

  it("clears selected folder and details when activeRepo changes", async () => {
    let currentRepo = "C:\\mock\\repo1";
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_available_plans") {
        return Promise.resolve(currentRepo === "C:\\mock\\repo1" ? mockPlanDetailsWithPlans : mockPlanDetailsEmpty);
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

  describe("unconfigured plan folder location dialog opening", () => {
    it.each(["codex" as const, "antigravity" as const])(
      "calls select_plan_root_dialog when unconfigured on %s and cancels gracefully",
      async (surface) => {
        const user = userEvent.setup();
        mockInvoke.mockImplementation((cmd: string) => {
          if (cmd === "get_plan_manager_location") {
            return Promise.resolve(null);
          }
          if (cmd === "select_plan_root_dialog") {
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

        await user.click(screen.getByTestId("btn-select-folder"));

        await waitFor(() => {
          expect(mockInvoke).toHaveBeenCalledWith("select_plan_root_dialog", { surface });
        });
        expect(mockInvoke).not.toHaveBeenCalledWith("select_plan_folder_dialog", expect.anything());
        expect(mockInvoke).not.toHaveBeenCalledWith("list_available_plans", expect.anything());
        expect(screen.queryByTestId("no-location-warning")).not.toBeInTheDocument();
      }
    );

    it.each(["codex" as const, "antigravity" as const])(
      "calls select_plan_root_dialog on %s, updates root, removes warning, and loads plans with selected root only",
      async (surface) => {
        const user = userEvent.setup();
        const selectedRepo = "C:\\mock\\new-repo";
        mockInvoke.mockImplementation((cmd: string, args?: any) => {
          if (cmd === "get_plan_manager_location") {
            return Promise.resolve(null);
          }
          if (cmd === "select_plan_root_dialog") {
            return Promise.resolve(selectedRepo);
          }
          if (cmd === "list_available_plans") {
            return Promise.resolve({
              repo_root: selectedRepo,
              plans_dir: `${selectedRepo}\\.plan`,
              active_plan_path: null,
              active_plan_relative_path: null,
              plans: [],
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

        await user.click(screen.getByTestId("btn-select-folder"));

        await waitFor(() => {
          expect(mockInvoke).toHaveBeenCalledWith("select_plan_root_dialog", { surface });
        });
        expect(mockInvoke).not.toHaveBeenCalledWith("select_plan_folder_dialog", expect.anything());

        // Warning banner is cleared once root is configured
        await waitFor(() => {
          expect(screen.queryByTestId("no-location-warning")).not.toBeInTheDocument();
        });

        // Exactly one list_available_plans call with the selected repo root, never with null
        await waitFor(() => {
          expect(mockInvoke).toHaveBeenCalledWith("list_available_plans", {
            repoPath: selectedRepo,
            folderPath: null,
          });
        });

        const listCalls = mockInvoke.mock.calls.filter(([cmd]) => cmd === "list_available_plans");
        expect(listCalls).toHaveLength(1);
        expect(listCalls[0][1]).toEqual({
          repoPath: selectedRepo,
          folderPath: null,
        });

        // Does not redundantly call set_plan_manager_location from frontend
        expect(mockInvoke).not.toHaveBeenCalledWith("set_plan_manager_location", expect.anything());
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
              plans: [],
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
          expect(screen.getByTestId("btn-select-folder")).toBeInTheDocument();
          expect(screen.getByTestId("no-active-plan-warning")).toBeInTheDocument();
        });

        onActivePlanChanged.mockClear();

        await user.click(screen.getByTestId("btn-select-folder"));

        await waitFor(() => {
          expect(mockInvoke).toHaveBeenCalledWith("select_plan_file_dialog", {
            repoPath: configuredRepo,
            surface,
            dialogTitle: expect.any(String),
          });
          expect(mockInvoke).toHaveBeenCalledWith("set_active_plan", {
            relativePath: ".plan/my-plan.md",
            repoPath: configuredRepo,
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
          expect(screen.getByTestId("btn-select-folder")).toBeInTheDocument();
          expect(screen.getByTestId("active-plan-badge")).toBeInTheDocument();
        });

        onActivePlanChanged.mockClear();

        await user.click(screen.getByTestId("btn-select-folder"));

        await waitFor(() => {
          expect(mockInvoke).toHaveBeenCalledWith("select_plan_file_dialog", {
            repoPath: configuredRepo,
            surface,
            dialogTitle: expect.any(String),
          });
        });

        expect(mockInvoke).not.toHaveBeenCalledWith("set_active_plan", expect.anything());
        expect(onActivePlanChanged).not.toHaveBeenCalled();
      }
    );
  });
});
