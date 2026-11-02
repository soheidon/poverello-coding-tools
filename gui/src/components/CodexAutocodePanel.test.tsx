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
  schema_version: 4,
  review_mode: "external_only",
  reviewers: {
    codex: false,
    gemini: false,
    deepseek: true,
  },
  priority_reviewer: "deepseek",
  gemini_cli_enabled: false,
};

const dummySelfReviewSettings: CodexReviewSettings = {
  schema_version: 4,
  review_mode: "codex_self_only",
  reviewers: {
    codex: false,
    gemini: false,
    deepseek: false,
  },
  priority_reviewer: null,
  gemini_cli_enabled: false,
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
  {
    name: "gemini",
    display_name: "Gemini CLI",
    installed: false,
    path: null,
    installed_version: null,
    update_available: false,
    latest_version: null,
    latest_checked_at: null,
    package_name: "@google/gemini-cli",
    is_preview: false,
    check_id: null,
    prerequisite_error: null,
  },
];

describe("CodexAutocodePanel Component (v0.26.2)", () => {
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
        return [
          { name: "codex", status: "authenticated", auth_type: "ChatGPT Plus", message: null, can_launch_login: true },
          { name: "gemini", status: "unknown", auth_type: null, message: "Interactive login only", can_launch_login: true },
          { name: "dsh", status: "unknown", auth_type: null, message: "Local Web UI config only", can_launch_login: true },
        ];
      }
      return undefined;
    });
  });

  it("renders 4 review modes and initializes to external_only when set", async () => {
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("codex-review-settings-card")).toBeInTheDocument();
      expect(screen.getByTestId("radio-review-mode-codex_self_only")).toBeInTheDocument();
      expect(screen.getByTestId("radio-review-mode-external_only")).toBeInTheDocument();
      expect(screen.getByTestId("radio-review-mode-codex_plus_external")).toBeInTheDocument();
      expect(screen.getByTestId("radio-review-mode-none")).toBeInTheDocument();
    });

    expect(screen.getByTestId("radio-review-mode-external_only")).toHaveAttribute("aria-checked", "true");
    expect(screen.getByTestId("working-agent-item")).toHaveTextContent("Codex");
    expect(screen.queryByTestId("checkbox-reviewer-codex")).toBeNull();
    expect(screen.queryByTestId("radio-priority-codex")).toBeNull();
    expect(screen.getByTestId("checkbox-reviewer-deepseek")).toBeInTheDocument();
    expect(screen.getByTestId("radio-priority-deepseek")).toBeInTheDocument();
  });

  it("switches to codex_self_only mode, disables external reviewer buttons, and saves schema v4 cleanly", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("radio-review-mode-codex_self_only")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("radio-review-mode-codex_self_only"));
    expect(screen.getByTestId("radio-review-mode-codex_self_only")).toHaveAttribute("aria-checked", "true");

    // In codex_self_only mode, external reviewer toggle is disabled
    expect(screen.getByTestId("checkbox-reviewer-deepseek")).toBeDisabled();

    // Save button is enabled
    const saveBtn = screen.getByTestId("btn-save-review-settings");
    expect(saveBtn).not.toBeDisabled();
    await user.click(saveBtn);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
        settings: expect.objectContaining({
          schema_version: 4,
          review_mode: "codex_self_only",
          unconfirmed_migration: false,
        }),
      });
    });
  });

  it("toggles accessible info popovers for Review Settings and Priority Reviewer", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-info-review-settings")).toBeInTheDocument();
      expect(screen.getByTestId("btn-info-priority")).toBeInTheDocument();
    });

    // Initially closed
    expect(screen.queryByTestId("btn-info-review-settings-popover")).toBeNull();
    expect(screen.queryByTestId("btn-info-priority-popover")).toBeNull();

    // Click Review Settings info icon
    await user.click(screen.getByTestId("btn-info-review-settings"));
    expect(screen.getByTestId("btn-info-review-settings-popover")).toBeInTheDocument();

    // Click Priority info icon
    await user.click(screen.getByTestId("btn-info-priority"));
    expect(screen.getByTestId("btn-info-priority-popover")).toBeInTheDocument();

    // Press Escape to close all popovers
    await user.keyboard("{Escape}");
    expect(screen.queryByTestId("btn-info-review-settings-popover")).toBeNull();
    expect(screen.queryByTestId("btn-info-priority-popover")).toBeNull();
  });

  it("displays unconfirmed migration banner when unconfirmed_migration is true and clears on save", async () => {
    mockInvoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_codex_review_settings") {
        return {
          schema_version: 4,
          review_mode: "external_only",
          reviewers: { codex: false, gemini: false, deepseek: true },
          priority_reviewer: "deepseek",
          gemini_cli_enabled: false,
          unconfirmed_migration: true,
        };
      }
      if (command === "save_codex_review_settings") {
        const payload = args as { settings: CodexReviewSettings };
        return payload.settings;
      }
      if (command === "get_cli_tools_status") return dummyCliTools;
      if (command === "get_cli_auth_status") return [];
      return undefined;
    });

    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("unconfirmed-migration-banner")).toBeInTheDocument();
    });

    // Click save
    await user.click(screen.getByTestId("btn-save-review-settings"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
        settings: expect.objectContaining({
          unconfirmed_migration: false,
          schema_version: 4,
        }),
      });
    });
  });

  it("renders Gemini with restricted modern models (2.5-pro and 2.5-flash only) when gemini_cli_enabled is true", async () => {
    mockInvoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_codex_review_settings") {
        return {
          schema_version: 4,
          review_mode: "external_only",
          reviewers: { codex: false, gemini: false, deepseek: true },
          priority_reviewer: "deepseek",
          gemini_cli_enabled: true,
        };
      }
      if (command === "save_codex_review_settings") {
        const payload = args as { settings: CodexReviewSettings };
        return payload.settings;
      }
      if (command === "get_cli_tools_status") return dummyCliTools;
      if (command === "get_cli_auth_status") return [];
      return undefined;
    });

    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("checkbox-reviewer-gemini")).toBeInTheDocument();
    });

    // Check Gemini
    await user.click(screen.getByTestId("checkbox-reviewer-gemini"));

    // Gemini model settings appear
    expect(screen.getByTestId("gemini-model-settings")).toBeInTheDocument();
    expect(screen.getByTestId("gemini-model-gemini-2.5-pro")).toHaveClass("active");
    expect(screen.getByTestId("gemini-model-gemini-2.5-flash")).not.toHaveClass("active");

    // Verify deprecated models are NOT present
    expect(screen.queryByTestId("gemini-model-gemini-2.0-flash")).toBeNull();
    expect(screen.queryByTestId("gemini-model-gemini-2.0-flash-thinking-exp")).toBeNull();

    // Select Gemini as priority and flash model
    await user.click(screen.getByTestId("radio-priority-gemini"));
    await user.click(screen.getByTestId("gemini-model-gemini-2.5-flash"));
    expect(screen.getByTestId("gemini-model-gemini-2.5-flash")).toHaveClass("active");

    // Save
    await user.click(screen.getByTestId("btn-save-review-settings"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
        settings: expect.objectContaining({
          schema_version: 4,
          review_mode: "external_only",
          reviewers: { codex: false, deepseek: true, gemini: true },
          priority_reviewer: "gemini",
          gemini: { model: "gemini-2.5-flash" },
        }),
      });
    });
  });

  it("renders DeepSeek Harness without unsupported effort buttons and without headless notice", async () => {
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("deepseek-model-settings")).toBeInTheDocument();
    });

    // Unsupported effort buttons must NOT be present
    expect(screen.queryByTestId("deepseek-effort-high")).toBeNull();
    expect(screen.queryByTestId("deepseek-effort-max")).toBeNull();
    expect(screen.queryByTestId("deepseek-model-select")).toBeNull();

    // Headless profile notice is NOT present
    expect(screen.getByTestId("deepseek-model-settings")).not.toHaveTextContent("dshFixedProfileNotice");
  });

  it("disables save button when 0 reviewers are selected in external_only mode", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("checkbox-reviewer-deepseek")).toBeInTheDocument();
    });

    // Uncheck deepseek (all 0 unchecked in external_only mode)
    await user.click(screen.getByTestId("checkbox-reviewer-deepseek"));

    // Warning is visible
    expect(screen.getByTestId("zero-external-reviewers-warning")).toBeInTheDocument();

    // Save button must be disabled
    const saveBtn = screen.getByTestId("btn-save-review-settings");
    expect(saveBtn).toBeDisabled();
  });

  it("reloads review settings on reload button click", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-reload-review-settings")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("btn-reload-review-settings"));
    expect(mockInvoke).toHaveBeenCalledWith("get_codex_review_settings");
  });

  it("supports Codex Self-Review as Priority Reviewer in codex_plus_external mode with dynamic order caption", async () => {
    mockInvoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_codex_review_settings") {
        return {
          schema_version: 4,
          review_mode: "codex_plus_external",
          reviewers: { codex: false, gemini: false, deepseek: true },
          priority_reviewer: "codex",
          gemini_cli_enabled: false,
        };
      }
      if (command === "save_codex_review_settings") {
        const payload = args as { settings: CodexReviewSettings };
        return payload.settings;
      }
      if (command === "get_cli_tools_status") return dummyCliTools;
      if (command === "get_cli_auth_status") return [];
      return undefined;
    });

    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("radio-priority-codex")).toBeInTheDocument();
      expect(screen.getByTestId("radio-priority-deepseek")).toBeInTheDocument();
    });

    // Codex is initially the priority reviewer
    expect(screen.getByTestId("radio-priority-codex")).toHaveAttribute("aria-checked", "true");
    expect(screen.getByTestId("radio-priority-deepseek")).toHaveAttribute("aria-checked", "false");

    // Caption explains arbitration rule
    const desc = screen.getByTestId("priority-order-description");
    expect(desc).toHaveTextContent(
      "After all selected reviewers have returned verdicts, the priority reviewer determines the final outcome if verdicts disagree. Review failure, incomplete reviews, or unresolved blocking findings cannot be overridden by priority."
    );

    // Save with Codex priority
    await user.click(screen.getByTestId("btn-save-review-settings"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
        settings: expect.objectContaining({
          schema_version: 4,
          review_mode: "codex_plus_external",
          priority_reviewer: "codex",
        }),
      });
    });

    // Reload with Codex priority maintained
    await user.click(screen.getByTestId("btn-reload-review-settings"));
    expect(screen.getByTestId("radio-priority-codex")).toHaveAttribute("aria-checked", "true");

    // Click DeepSeek as priority reviewer
    await user.click(screen.getByTestId("radio-priority-deepseek"));
    expect(screen.getByTestId("radio-priority-deepseek")).toHaveAttribute("aria-checked", "true");
    expect(screen.getByTestId("radio-priority-codex")).toHaveAttribute("aria-checked", "false");

    // Caption consistently explains arbitration rule without order/sequence wording
    expect(desc).toHaveTextContent(
      "After all selected reviewers have returned verdicts, the priority reviewer determines the final outcome if verdicts disagree. Review failure, incomplete reviews, or unresolved blocking findings cannot be overridden by priority."
    );

    // Click Save
    await user.click(screen.getByTestId("btn-save-review-settings"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
        settings: expect.objectContaining({
          schema_version: 4,
          review_mode: "codex_plus_external",
          reviewers: { codex: false, gemini: false, deepseek: true },
          priority_reviewer: "deepseek",
        }),
      });
    });

    // Update mock return to simulate persistence
    mockInvoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_codex_review_settings") {
        return {
          schema_version: 4,
          review_mode: "codex_plus_external",
          reviewers: { codex: false, gemini: false, deepseek: true },
          priority_reviewer: "deepseek",
          gemini_cli_enabled: false,
        };
      }
      return undefined;
    });

    // Reload and verify DeepSeek priority is maintained
    await user.click(screen.getByTestId("btn-reload-review-settings"));
    await waitFor(() => {
      expect(screen.getByTestId("radio-priority-deepseek")).toHaveAttribute("aria-checked", "true");
    });
  });

  it("renders DeepSeek model selector and reasoning effort buttons and persists them", async () => {
    let savedSettings: CodexReviewSettings | null = null;
    mockInvoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_codex_review_settings") {
        return savedSettings || {
          schema_version: 4,
          review_mode: "codex_plus_external",
          reviewers: { codex: false, gemini: false, deepseek: true },
          priority_reviewer: "codex",
          gemini_cli_enabled: false,
          deepseek: {
            model: "deepseek-flash",
            reasoning_effort: "high",
          },
        };
      }
      if (command === "save_codex_review_settings") {
        const payload = args as { settings: CodexReviewSettings };
        savedSettings = payload.settings;
        return payload.settings;
      }
      if (command === "get_cli_tools_status") return dummyCliTools;
      if (command === "get_cli_auth_status") return [];
      return undefined;
    });

    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("deepseek-model-settings")).toBeInTheDocument();
    });

    expect(screen.getByTestId("codex-deepseek-model-deepseek-flash")).toHaveClass("active");
    expect(screen.getByTestId("codex-deepseek-model-deepseek-v4-pro")).not.toHaveClass("active");

    // Check effort buttons
    expect(screen.getByTestId("codex-deepseek-effort-low")).toBeInTheDocument();
    expect(screen.getByTestId("codex-deepseek-effort-high")).toHaveClass("active");
    expect(screen.getByTestId("codex-deepseek-effort-max")).not.toHaveClass("active");

    // Select deepseek-v4-pro
    await user.click(screen.getByTestId("codex-deepseek-model-deepseek-v4-pro"));
    expect(screen.getByTestId("codex-deepseek-model-deepseek-v4-pro")).toHaveClass("active");
    expect(screen.getByTestId("codex-deepseek-model-deepseek-flash")).not.toHaveClass("active");

    // Click max effort
    await user.click(screen.getByTestId("codex-deepseek-effort-max"));
    expect(screen.getByTestId("codex-deepseek-effort-max")).toHaveClass("active");

    // Save
    await user.click(screen.getByTestId("btn-save-review-settings"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
        settings: expect.objectContaining({
          deepseek: {
            model: "deepseek-v4-pro",
            reasoning_effort: "max",
          },
        }),
      });
    });

    // Reload
    await user.click(screen.getByTestId("btn-reload-review-settings"));
    await waitFor(() => {
      expect(screen.getByTestId("codex-deepseek-model-deepseek-v4-pro")).toHaveClass("active");
      expect(screen.getByTestId("codex-deepseek-effort-max")).toHaveClass("active");
    });
  });
});
