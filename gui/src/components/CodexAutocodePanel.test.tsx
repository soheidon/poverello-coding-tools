import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import CodexAutocodePanel from "./CodexAutocodePanel";
import { LanguageContext } from "../i18n";
import type { CodexReviewSettings, CliToolInfo } from "../types";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

const dummyExternalReviewSettings: CodexReviewSettings = {
  schema_version: 5,
  review_mode: "external_only",
  build_reviewers: ["deepseek"],
  plan_reviewers: null,
  reviewers: {
    codex: false,
    gemini: false,
    deepseek: true,
  },
  priority_reviewer: "deepseek",
  gemini_cli_enabled: false,
  codex: {
    model: "gpt-6-luna",
    reasoning_effort: "medium",
  },
  deepseek: {
    model: "deepseek-flash",
    reasoning_effort: "high",
  },
};

const dummyCliTools: CliToolInfo[] = [
  {
    name: "codex",
    display_name: "Codex CLI",
    installed: true,
    path: null,
    installed_version: "0.1.0",
    update_available: false,
    latest_version: null,
    latest_checked_at: null,
    package_name: "@openai/codex",
    is_preview: false,
    check_id: null,
    prerequisite_error: null,
  },
  {
    name: "dsh",
    display_name: "DeepSeek Harness",
    installed: true,
    path: null,
    installed_version: "1.0.0",
    update_available: false,
    latest_version: null,
    latest_checked_at: null,
    package_name: "@deepseek-ai/dsh",
    is_preview: true,
    check_id: null,
    prerequisite_error: null,
  },
];

describe("CodexAutocodePanel Component (v0.26.2 Unified Layout)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_codex_review_settings") {
        return dummyExternalReviewSettings;
      }
      if (command === "save_codex_review_settings") {
        const payload = args as { settings: CodexReviewSettings };
        return payload.settings;
      }
      if (command === "get_cli_tools_status") {
        return dummyCliTools;
      }
      if (command === "get_cli_auth_status") {
        return [];
      }
      if (command === "list_available_plans") {
        return {
          repo_root: "C:\\mock\\repo",
          plans_dir: "C:\\mock\\repo\\.plan",
          selected_folder: ".plan",
          active_plan_path: "C:\\mock\\repo\\.plan\\V0.26.4.md",
          active_plan_relative_path: ".plan/V0.26.4.md",
          plans: [],
        };
      }
      return undefined;
    });
  });

  it("renders independent toggles for Codex (Session) and DeepSeek Harness matching Antigravity layout and header", async () => {
    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("codex-review-settings-card")).toBeInTheDocument();
      expect(screen.getByTestId("reviewer-toggle-codex")).toBeInTheDocument();
      expect(screen.getByTestId("reviewer-toggle-deepseek")).toBeInTheDocument();
    });

    // Check heading and unified layout
    expect(screen.getByText(/Select Implementation Review AI|実装レビューAIの選択/)).toBeInTheDocument();
    expect(screen.getByText(/Multi-select supported|複数選択可能/)).toBeInTheDocument();

    // Verify deprecated review modes and redundant status boxes are NOT rendered
    expect(screen.queryByTestId("radio-review-mode-codex_self_only")).toBeNull();
    expect(screen.queryByTestId("radio-review-mode-external_only")).toBeNull();
    expect(screen.queryByTestId("working-agent-item")).toBeNull();
    expect(screen.queryByTestId("reviewer-status-row")).toBeNull();

    // DeepSeek is initially active
    expect(screen.getByTestId("reviewer-policy-deepseek-only")).toBeChecked();
    expect(screen.getByTestId("reviewer-policy-codex-only")).not.toBeChecked();
    expect(screen.getByTestId("deepseek-model-settings")).toBeInTheDocument();
  });

  it("supports toggling Codex (Session) and DeepSeek independently across all 4 combinations and saves schema_version 5", async () => {
    let currentSettings = { ...dummyExternalReviewSettings };
    mockInvoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_codex_review_settings") return currentSettings;
      if (command === "save_codex_review_settings") {
        const payload = args as { settings: CodexReviewSettings };
        currentSettings = { ...payload.settings };
        return payload.settings;
      }
      if (command === "list_available_plans") {
        return {
          repo_root: "C:\\mock\\repo",
          plans_dir: "C:\\mock\\repo\\.plan",
          selected_folder: ".plan",
          active_plan_path: null,
          active_plan_relative_path: null,
          plans: [],
        };
      }
      return undefined;
    });

    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("reviewer-toggle-codex")).toBeInTheDocument();
    });

    // 1. Initial state: DeepSeek only (OFF/ON)
    expect(screen.getByTestId("reviewer-policy-deepseek-only")).toBeChecked();
    expect(screen.getByTestId("reviewer-policy-codex-only")).not.toBeChecked();

    // 2. Uncheck DeepSeek -> 0 reviewers (OFF/OFF) -> none
    await user.click(screen.getByTestId("reviewer-toggle-deepseek"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
        settings: expect.objectContaining({
          schema_version: 5,
          review_mode: "none",
          build_reviewers: [],
          priority_reviewer: null,
        }),
      });
    });
    expect(screen.getByTestId("reviewer-policy-deepseek-only")).not.toBeChecked();
    expect(screen.getByTestId("reviewer-policy-codex-only")).not.toBeChecked();

    // 3. Check Codex -> Codex only (ON/OFF) -> codex_self_only
    await user.click(screen.getByTestId("reviewer-toggle-codex"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
        settings: expect.objectContaining({
          schema_version: 5,
          review_mode: "codex_self_only",
          build_reviewers: ["codex"],
          priority_reviewer: "codex",
        }),
      });
    });
    expect(screen.getByTestId("reviewer-policy-codex-only")).toBeChecked();
    expect(screen.getByTestId("codex-model-badge")).toBeInTheDocument();

    // 4. Check DeepSeek -> Both ON (ON/ON) -> codex_plus_external
    await user.click(screen.getByTestId("reviewer-toggle-deepseek"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
        settings: expect.objectContaining({
          schema_version: 5,
          review_mode: "codex_plus_external",
          build_reviewers: ["codex", "deepseek"],
          priority_reviewer: "codex",
        }),
      });
    });
    expect(screen.getByTestId("reviewer-policy-codex-only")).toBeChecked();
    expect(screen.getByTestId("reviewer-policy-deepseek-only")).toBeChecked();

    // Priority Reviewer section appears when both are active
    expect(screen.getByTestId("priority-selection-section")).toBeInTheDocument();
  });

  it("renders DeepSeek model settings only when DeepSeek is active and updates model/effort", async () => {
    let currentSettings = { ...dummyExternalReviewSettings };
    mockInvoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_codex_review_settings") return currentSettings;
      if (command === "save_codex_review_settings") {
        const payload = args as { settings: CodexReviewSettings };
        currentSettings = { ...payload.settings };
        return payload.settings;
      }
      return undefined;
    });

    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("deepseek-model-settings")).toBeInTheDocument();
    });

    expect(screen.getByTestId("codex-deepseek-model-deepseek-flash")).toHaveClass("active");
    expect(screen.getByTestId("codex-deepseek-effort-high")).toHaveClass("active");

    // Click max effort
    await user.click(screen.getByTestId("codex-deepseek-effort-max"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
        settings: expect.objectContaining({
          deepseek: expect.objectContaining({
            reasoning_effort: "max",
          }),
        }),
      });
    });

    // Switch to deepseek-v4-pro
    await user.click(screen.getByTestId("codex-deepseek-model-deepseek-v4-pro"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
        settings: expect.objectContaining({
          deepseek: expect.objectContaining({
            model: "deepseek-v4-pro",
          }),
        }),
      });
    });
  });

  it("renders Priority Reviewer only when both Codex and DeepSeek are active and allows selecting priority", async () => {
    let currentSettings: CodexReviewSettings = {
      ...dummyExternalReviewSettings,
      review_mode: "codex_plus_external",
      build_reviewers: ["codex", "deepseek"],
      priority_reviewer: "codex",
    };
    mockInvoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_codex_review_settings") return currentSettings;
      if (command === "save_codex_review_settings") {
        const payload = args as { settings: CodexReviewSettings };
        currentSettings = { ...payload.settings };
        return payload.settings;
      }
      return undefined;
    });

    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("priority-selection-section")).toBeInTheDocument();
      expect(screen.getByTestId("radio-priority-codex")).toBeInTheDocument();
      expect(screen.getByTestId("radio-priority-deepseek")).toBeInTheDocument();
    });

    expect(screen.getByTestId("radio-priority-codex")).toHaveClass("active");

    // Click DeepSeek as priority
    await user.click(screen.getByTestId("radio-priority-deepseek"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
        settings: expect.objectContaining({
          priority_reviewer: "deepseek",
        }),
      });
    });
  });

  it("does not render regular save button and automatically saves", async () => {
    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("codex-review-settings-card")).toBeInTheDocument();
    });

    expect(screen.queryByTestId("btn-save-review-settings")).not.toBeInTheDocument();
  });

  it("displays unconfirmed migration banner and clears on confirm migration button click", async () => {
    let currentSettings: CodexReviewSettings = {
      ...dummyExternalReviewSettings,
      unconfirmed_migration: true,
    };

    mockInvoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_codex_review_settings") {
        return currentSettings;
      }
      if (command === "save_codex_review_settings") {
        const payload = args as { settings: CodexReviewSettings };
        currentSettings = { ...payload.settings };
        return currentSettings;
      }
      return undefined;
    });

    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("unconfirmed-migration-banner")).toBeInTheDocument();
      expect(screen.getByTestId("btn-confirm-migration")).toBeInTheDocument();
    });

    // 1. Changing reviewer does NOT clear unconfirmed_migration
    await user.click(screen.getByTestId("reviewer-toggle-codex"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
        settings: expect.objectContaining({
          unconfirmed_migration: true,
        }),
      });
      expect(screen.getByTestId("unconfirmed-migration-banner")).toBeInTheDocument();
    });

    // 2. Explicitly confirming migration clears unconfirmed_migration
    await user.click(screen.getByTestId("btn-confirm-migration"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
        settings: expect.objectContaining({
          unconfirmed_migration: false,
        }),
      });
      expect(screen.queryByTestId("unconfirmed-migration-banner")).not.toBeInTheDocument();
    });
  });

  it("displays save error and reloads persisted settings on save failure", async () => {
    let saveFailedOnce = true;
    mockInvoke.mockImplementation(async (command: string, _args?: unknown) => {
      if (command === "get_codex_review_settings") {
        return dummyExternalReviewSettings;
      }
      if (command === "save_codex_review_settings") {
        if (saveFailedOnce) {
          saveFailedOnce = false;
          throw new Error("Disk full or permission denied");
        }
        return dummyExternalReviewSettings;
      }
      return undefined;
    });

    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("reviewer-toggle-codex")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("reviewer-toggle-codex"));

    await waitFor(() => {
      expect(screen.getByTestId("review-settings-error")).toBeInTheDocument();
      expect(screen.getByTestId("review-settings-error").textContent).toContain("Disk full or permission denied");
    });

    // It reloaded the settings to reconcile GUI with disk
    expect(mockInvoke).toHaveBeenCalledWith("get_codex_review_settings");
  });

  it("handles consecutive rapid changes by serializing and saving the latest state", async () => {
    const savedStates: CodexReviewSettings[] = [];
    mockInvoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_codex_review_settings") {
        return dummyExternalReviewSettings;
      }
      if (command === "save_codex_review_settings") {
        const payload = args as { settings: CodexReviewSettings };
        savedStates.push(payload.settings);
        // Introduce small artificial latency to exercise concurrency
        await new Promise((r) => setTimeout(r, 20));
        return payload.settings;
      }
      return undefined;
    });

    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("codex-deepseek-model-deepseek-flash")).toBeInTheDocument();
      expect(screen.getByTestId("codex-deepseek-model-deepseek-v4-pro")).toBeInTheDocument();
    });

    // Rapidly click two different models
    await user.click(screen.getByTestId("codex-deepseek-model-deepseek-v4-pro"));
    await user.click(screen.getByTestId("codex-deepseek-model-deepseek-flash"));

    await waitFor(() => {
      expect(savedStates.length).toBeGreaterThanOrEqual(1);
      const lastSaved = savedStates[savedStates.length - 1];
      expect(lastSaved.deepseek?.model).toBe("deepseek-flash");
    });
  });

  it("reloads review settings on reload button click", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-reload-review-settings")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("btn-reload-review-settings"));
    expect(mockInvoke).toHaveBeenCalledWith("get_codex_review_settings");
  });
});
