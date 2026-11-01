import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import CodexAutocodePanel from "./CodexAutocodePanel";
import { LanguageContext } from "../i18n";
import type { CodexReviewSettings, ReviewersSelection, CliToolInfo } from "../types";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

const dummyReviewSettings: CodexReviewSettings = {
  schema_version: 2,
  reviewers: {
    codex: true,
    gemini: false,
    deepseek: false,
  },
  priority_reviewer: "codex",
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

describe("CodexAutocodePanel Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_codex_review_settings") {
        return dummyReviewSettings;
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

  it("renders Review Settings card with 2-column compact grids when Gemini option is disabled (§12.27)", async () => {
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("codex-review-settings-card")).toBeInTheDocument();
      // §12.27: Disclosure button, option container, and summary badge must NOT exist in Codex Autocode
      expect(screen.queryByTestId("btn-toggle-options")).toBeNull();
      expect(screen.queryByTestId("gemini-option-container")).toBeNull();
      expect(screen.queryByTestId("gemini-summary-badge")).toBeNull();
      expect(screen.getByTestId("reviewer-selection-grid")).toBeInTheDocument();
      expect(screen.getByTestId("reviewer-status-row")).toBeInTheDocument();
      expect(screen.queryByTestId("reviewer-status-grid")).toBeNull();
      expect(screen.getByTestId("priority-selection-section")).toBeInTheDocument();
      expect(screen.getByTestId("checkbox-reviewer-codex")).toBeInTheDocument();
      expect(screen.getByTestId("checkbox-reviewer-deepseek")).toBeInTheDocument();
      expect(screen.queryByTestId("checkbox-reviewer-gemini")).toBeNull();
      expect(screen.queryByTestId("reviewer-status-item-gemini")).toBeNull();
      expect(screen.queryByTestId("radio-priority-gemini")).toBeNull();
    });

    const codexCheckbox = screen.getByTestId("checkbox-reviewer-codex");
    const deepseekCheckbox = screen.getByTestId("checkbox-reviewer-deepseek");

    expect(codexCheckbox.getAttribute("aria-checked")).toBe("true");
    expect(codexCheckbox).toHaveClass("active");
    expect(deepseekCheckbox.getAttribute("aria-checked")).toBe("false");
    expect(deepseekCheckbox).not.toHaveClass("active");

    // Priority buttons: codex is active (only 1 active reviewer), deepseek disabled
    const radioCodex = screen.getByTestId("radio-priority-codex");
    const radioDeepseek = screen.getByTestId("radio-priority-deepseek");

    expect(radioCodex.getAttribute("aria-checked")).toBe("true");
    expect(radioCodex).toHaveClass("active");
    expect(radioDeepseek).toBeDisabled();

    expect(screen.getByTestId("btn-save-review-settings")).not.toBeDisabled();
  });

  it("renders 3 candidates in 3-column layout when Gemini option is enabled in review settings (§12.27)", async () => {
    mockInvoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_codex_review_settings") {
        return {
          schema_version: 2,
          reviewers: { codex: true, gemini: false, deepseek: false },
          priority_reviewer: "codex",
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
      // §12.27: Zero option controls in Codex Autocode
      expect(screen.queryByTestId("btn-toggle-options")).toBeNull();
      expect(screen.queryByTestId("gemini-option-container")).toBeNull();
      expect(screen.queryByTestId("gemini-summary-badge")).toBeNull();

      // Gemini candidate rendered in 3-column grid
      const grid = screen.getByTestId("reviewer-selection-grid");
      expect(grid).toHaveClass("cols-3");
      expect(screen.getByTestId("checkbox-reviewer-codex")).toBeInTheDocument();
      expect(screen.getByTestId("checkbox-reviewer-gemini")).toBeInTheDocument();
      expect(screen.getByTestId("checkbox-reviewer-deepseek")).toBeInTheDocument();
      expect(screen.getByTestId("reviewer-status-item-gemini")).toBeInTheDocument();
    });

    const geminiCheckbox = screen.getByTestId("checkbox-reviewer-gemini");
    expect(geminiCheckbox.getAttribute("aria-checked")).toBe("false");

    // Select Gemini as reviewer and set as priority
    await user.click(geminiCheckbox);
    expect(geminiCheckbox.getAttribute("aria-checked")).toBe("true");

    const radioGemini = screen.getByTestId("radio-priority-gemini");
    expect(radioGemini).not.toBeDisabled();
    await user.click(radioGemini);
    expect(radioGemini.getAttribute("aria-checked")).toBe("true");

    // Save review settings
    await user.click(screen.getByTestId("btn-save-review-settings"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
        settings: {
          schema_version: 2,
          reviewers: { codex: true, deepseek: false, gemini: true },
          priority_reviewer: "gemini",
          gemini_cli_enabled: true,
        },
      });
    });
  });

  it("renders Gemini as selected and priority directly when loaded from canonical settings (§12.27)", async () => {
    mockInvoke.mockImplementation(async (command: string) => {
      if (command === "get_codex_review_settings") {
        return {
          schema_version: 2,
          reviewers: { codex: true, gemini: true, deepseek: false },
          priority_reviewer: "gemini",
          gemini_cli_enabled: true,
        };
      }
      if (command === "get_cli_tools_status") return dummyCliTools;
      if (command === "get_cli_auth_status") return [];
      return undefined;
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.queryByTestId("btn-toggle-options")).toBeNull();
      expect(screen.queryByTestId("gemini-option-container")).toBeNull();
      expect(screen.getByTestId("checkbox-reviewer-gemini")).toBeInTheDocument();
      expect(screen.getByTestId("checkbox-reviewer-gemini").getAttribute("aria-checked")).toBe("true");
      expect(screen.getByTestId("radio-priority-gemini").getAttribute("aria-checked")).toBe("true");
    });
  });

  it("updates priority when toggling reviewers and enables save", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("checkbox-reviewer-deepseek")).toBeInTheDocument();
    });

    // Check deepseek (now codex and deepseek are checked)
    await user.click(screen.getByTestId("checkbox-reviewer-deepseek"));

    // Priority radio section
    expect(screen.getByTestId("priority-selection-section")).toBeInTheDocument();
    const radioCodex = screen.getByTestId("radio-priority-codex");
    const radioDeepseek = screen.getByTestId("radio-priority-deepseek");
    expect(radioCodex.getAttribute("aria-checked")).toBe("true");
    expect(radioDeepseek.getAttribute("aria-checked")).toBe("false");
    expect(radioDeepseek).not.toBeDisabled();

    // Change priority to deepseek
    await user.click(radioDeepseek);
    expect(radioDeepseek.getAttribute("aria-checked")).toBe("true");
    expect(radioCodex.getAttribute("aria-checked")).toBe("false");

    // Save button should be enabled
    const saveBtn = screen.getByTestId("btn-save-review-settings");
    expect(saveBtn).not.toBeDisabled();

    // Click save
    await user.click(saveBtn);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
        settings: {
          schema_version: 2,
          reviewers: { codex: true, deepseek: true, gemini: false },
          priority_reviewer: "deepseek",
          gemini_cli_enabled: false,
        },
      });
    });
  });

  it("requires confirmation modal when 0 reviewers are selected", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("checkbox-reviewer-codex")).toBeInTheDocument();
    });

    // Uncheck codex (all 0 unchecked)
    await user.click(screen.getByTestId("checkbox-reviewer-codex"));

    const saveBtn = screen.getByTestId("btn-save-review-settings");
    expect(saveBtn).not.toBeDisabled();

    // Clicking save should trigger modal
    await user.click(saveBtn);
    expect(screen.getByTestId("modal-confirm-zero-reviewers")).toBeInTheDocument();

    // Cancel modal
    await user.click(screen.getByTestId("modal-zero-reviewers-cancel"));
    expect(screen.queryByTestId("modal-confirm-zero-reviewers")).toBeNull();
    expect(mockInvoke).not.toHaveBeenCalledWith("save_codex_review_settings", expect.anything());

    // Click save again and proceed
    await user.click(saveBtn);
    await user.click(screen.getByTestId("modal-zero-reviewers-proceed"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
        settings: {
          schema_version: 2,
          reviewers: { codex: false, deepseek: false, gemini: false },
          priority_reviewer: null,
          gemini_cli_enabled: false,
        },
      });
    });
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

  it("maps reviewer 'deepseek' to CLI catalog ID 'dsh', renders in single row without auth details, and handles navigation to CLI tools", async () => {
    mockInvoke.mockImplementation(async (command: string) => {
      if (command === "get_codex_review_settings") {
        return {
          schema_version: 2,
          reviewers: { codex: true, gemini: false, deepseek: false },
          priority_reviewer: "codex",
          gemini_cli_enabled: true,
        };
      }
      if (command === "get_cli_tools_status") return dummyCliTools;
      if (command === "get_cli_auth_status") return [];
      return undefined;
    });

    const user = userEvent.setup();
    const onNavigateMock = vi.fn();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexAutocodePanel onNavigateToCliTools={onNavigateMock} />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      // Reviewer deepseek should show installed version from 'dsh' (1.0.0)
      const dshInstallStatus = screen.getByTestId("reviewer-install-status-deepseek");
      expect(dshInstallStatus).toHaveTextContent("1.0.0");

      // Reviewer codex should show installed version (0.1.0)
      const codexInstallStatus = screen.getByTestId("reviewer-install-status-codex");
      expect(codexInstallStatus).toHaveTextContent("0.1.0");

      // Reviewer gemini is rendered when gemini_cli_enabled is true
      const geminiInstallStatus = screen.getByTestId("reviewer-install-status-gemini");
      expect(geminiInstallStatus).toHaveTextContent("antigravity.cliToolNotInstalled");

      // Auth details are excluded from single status row (§12.21.2)
      expect(screen.queryByTestId("reviewer-auth-status-codex")).toBeNull();
    });

    const navBtn = screen.getByTestId("btn-nav-cli-tools");
    expect(navBtn).toBeInTheDocument();
    await user.click(navBtn);
    expect(onNavigateMock).toHaveBeenCalledTimes(1);
  });

  it("distinguishes status retrieval failure from Not installed (§12.20.7, §12.20.9)", async () => {
    mockInvoke.mockImplementation(async (command: string) => {
      if (command === "get_codex_review_settings") {
        return dummyReviewSettings;
      }
      if (command === "get_cli_tools_status") {
        throw new Error("CLI tool detection failed");
      }
      if (command === "get_cli_auth_status") {
        return [];
      }
      return undefined;
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CodexAutocodePanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      const codexStatus = screen.getByTestId("reviewer-install-status-codex");
      expect(codexStatus).toHaveTextContent("codexSkillsRules.statusUnavailable");
      expect(codexStatus).not.toHaveTextContent("antigravity.cliToolNotInstalled");
    });
  });
});
