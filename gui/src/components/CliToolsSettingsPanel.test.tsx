import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import CliToolsSettingsPanel from "./CliToolsSettingsPanel";
import { LanguageContext } from "../i18n";
import type { NodePrerequisiteStatus, CliToolInfo, CliAuthInfo, CodexReviewSettings } from "../types";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

const dummyPrereqReady: NodePrerequisiteStatus = {
  node_installed: true,
  node_version: "22.2.0",
  node_major_version: 22,
  node_path: "C:\\Program Files\\nodejs\\node.exe",
  npm_installed: true,
  npm_version: "10.7.0",
  npm_path: "C:\\Program Files\\nodejs\\npm.cmd",
  node_npm_ready: true,
  gemini_node_ready: true,
  message: null,
};

const dummyPrereqOutdated: NodePrerequisiteStatus = {
  node_installed: true,
  node_version: "18.19.0",
  node_major_version: 18,
  node_path: "C:\\Program Files\\nodejs\\node.exe",
  npm_installed: true,
  npm_version: "10.2.0",
  npm_path: "C:\\Program Files\\nodejs\\npm.cmd",
  node_npm_ready: true,
  gemini_node_ready: false,
  message: "Node.js v18.19.0 found. Gemini CLI requires Node.js >= 20.",
};

const dummyPrereqMissing: NodePrerequisiteStatus = {
  node_installed: false,
  node_version: null,
  node_major_version: null,
  node_path: null,
  npm_installed: false,
  npm_version: null,
  npm_path: null,
  node_npm_ready: false,
  gemini_node_ready: false,
  message: "Node.js is not installed or not found on PATH.",
};

const dummyCliTools: CliToolInfo[] = [
  {
    name: "codex",
    display_name: "Codex CLI",
    installed: true,
    path: "C:\\Users\\Sohei\\AppData\\Roaming\\npm\\codex.cmd",
    installed_version: "0.163.0",
    latest_version: null,
    latest_checked_at: null,
    package_name: "@openai/codex",
    is_preview: false,
    update_available: false,
    check_id: null,
    prerequisite_error: null,
  },
  {
    name: "dsh",
    display_name: "DeepSeek Harness",
    installed: false,
    path: null,
    installed_version: null,
    latest_version: null,
    latest_checked_at: null,
    package_name: "@deepseek-ai/dsh",
    is_preview: true,
    update_available: false,
    check_id: null,
    prerequisite_error: null,
  },
  {
    name: "gemini",
    display_name: "Gemini CLI",
    installed: false,
    path: null,
    installed_version: null,
    latest_version: null,
    latest_checked_at: null,
    package_name: "@google/gemini-cli",
    is_preview: false,
    update_available: false,
    check_id: null,
    prerequisite_error: null,
  },
];

const dummyAuthInfo: CliAuthInfo[] = [
  { name: "codex", status: "authenticated", auth_type: "ChatGPT Plus", message: null, can_launch_login: true },
  { name: "dsh", status: "unknown", auth_type: null, message: "Configure API keys via Web UI", can_launch_login: true },
  { name: "gemini", status: "unknown", auth_type: null, message: "Interactive authentication only", can_launch_login: true },
];

describe("CliToolsSettingsPanel Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders both Node prerequisites and CLI tools cards when prerequisites are met", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_node_prerequisites_status") return dummyPrereqReady;
      if (cmd === "get_cli_tools_status") return dummyCliTools;
      if (cmd === "get_cli_auth_status") return dummyAuthInfo;
      return null;
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CliToolsSettingsPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("cli-tools-settings-panel")).toBeInTheDocument();
      expect(screen.getByTestId("node-prerequisites-card")).toBeInTheDocument();
      expect(screen.getByTestId("cli-tools-card")).toBeInTheDocument();
    });

    // Node & npm version badges
    expect(screen.getByTestId("node-version-badge")).toHaveTextContent("v22.2.0");
    expect(screen.getByTestId("npm-version-badge")).toHaveTextContent("v10.7.0");
    expect(screen.queryByTestId("gemini-node-min-warning")).toBeNull();
  });

  it("warns about Node.js < 20 requirement for Gemini CLI when outdated", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_node_prerequisites_status") return dummyPrereqOutdated;
      if (cmd === "get_cli_tools_status") return dummyCliTools;
      if (cmd === "get_cli_auth_status") return dummyAuthInfo;
      return null;
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CliToolsSettingsPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("gemini-node-min-warning")).toBeInTheDocument();
      expect(screen.getByTestId("gemini-node-min-warning")).toHaveTextContent("v18.19.0");
    });
  });

  it("handles missing Node.js and npm and opens download page on click", async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_node_prerequisites_status") return dummyPrereqMissing;
      if (cmd === "get_cli_tools_status") return dummyCliTools;
      if (cmd === "get_cli_auth_status") return dummyAuthInfo;
      if (cmd === "open_node_download_page") return null;
      return null;
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CliToolsSettingsPanel />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("node-not-installed-badge")).toBeInTheDocument();
      expect(screen.getByTestId("npm-not-installed-badge")).toBeInTheDocument();
    });

    const downloadBtn = screen.getByTestId("btn-open-node-download");
    await user.click(downloadBtn);
    expect(mockInvoke).toHaveBeenCalledWith("open_node_download_page");
  });

  describe("GeminiReviewerOptionCard in Settings > CLI Tools (§12.27)", () => {
    it("renders Gemini Reviewer Option card and toggles ON without selecting gemini reviewer", async () => {
      const user = userEvent.setup();
      const currentSettings: CodexReviewSettings = {
        schema_version: 4,
        review_mode: "external_only",
        reviewers: { codex: true, gemini: false, deepseek: false },
        priority_reviewer: "codex",
        gemini_cli_enabled: false,
      };

      mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === "get_node_prerequisites_status") return dummyPrereqReady;
        if (cmd === "get_cli_tools_status") return dummyCliTools;
        if (cmd === "get_cli_auth_status") return dummyAuthInfo;
        if (cmd === "get_codex_review_settings") return currentSettings;
        if (cmd === "save_codex_review_settings") {
          const payload = args as { settings: CodexReviewSettings };
          return payload.settings;
        }
        return null;
      });

      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <CliToolsSettingsPanel />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("gemini-reviewer-option-card")).toBeInTheDocument();
        expect(screen.getByTestId("toggle-gemini-option-settings")).toBeInTheDocument();
        expect(screen.getByTestId("gemini-option-badge")).toHaveTextContent("Disabled");
      });

      const toggle = screen.getByTestId("toggle-gemini-option-settings");
      expect(toggle).not.toBeChecked();

      // Click toggle to enable
      await user.click(toggle);

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
          settings: {
            schema_version: 4,
        review_mode: "external_only",
            reviewers: { codex: true, gemini: false, deepseek: false },
            priority_reviewer: "codex",
            gemini_cli_enabled: true,
          },
        });
        expect(screen.getByTestId("gemini-option-success-msg")).toBeInTheDocument();
        expect(screen.getByTestId("gemini-option-badge")).toHaveTextContent("✓ Active");
      });
    });

    it("directly disables Gemini option without confirmation modal when Gemini is unselected and not priority", async () => {
      const user = userEvent.setup();
      const currentSettings: CodexReviewSettings = {
        schema_version: 4,
        review_mode: "external_only",
        reviewers: { codex: true, gemini: false, deepseek: false },
        priority_reviewer: "codex",
        gemini_cli_enabled: true,
      };

      mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === "get_node_prerequisites_status") return dummyPrereqReady;
        if (cmd === "get_cli_tools_status") return dummyCliTools;
        if (cmd === "get_cli_auth_status") return dummyAuthInfo;
        if (cmd === "get_codex_review_settings") return currentSettings;
        if (cmd === "save_codex_review_settings") {
          const payload = args as { settings: CodexReviewSettings };
          return payload.settings;
        }
        return null;
      });

      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <CliToolsSettingsPanel />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("toggle-gemini-option-settings")).toBeChecked();
      });

      const toggle = screen.getByTestId("toggle-gemini-option-settings");
      await user.click(toggle);

      // No confirmation modal should be opened
      expect(screen.queryByTestId("gemini-disable-confirm-modal")).toBeNull();

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
          settings: {
            schema_version: 4,
        review_mode: "external_only",
            reviewers: { codex: true, gemini: false, deepseek: false },
            priority_reviewer: "codex",
            gemini_cli_enabled: false,
          },
        });
      });
    });

    it("shows confirmation modal when disabling Gemini while selected, handles cancel, and handles confirm when Gemini is not priority", async () => {
      const user = userEvent.setup();
      const currentSettings: CodexReviewSettings = {
        schema_version: 4,
        review_mode: "external_only",
        reviewers: { codex: true, gemini: true, deepseek: false },
        priority_reviewer: "codex",
        gemini_cli_enabled: true,
      };

      mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === "get_node_prerequisites_status") return dummyPrereqReady;
        if (cmd === "get_cli_tools_status") return dummyCliTools;
        if (cmd === "get_cli_auth_status") return dummyAuthInfo;
        if (cmd === "get_codex_review_settings") return currentSettings;
        if (cmd === "save_codex_review_settings") {
          const payload = args as { settings: CodexReviewSettings };
          return payload.settings;
        }
        return null;
      });

      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <CliToolsSettingsPanel />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("toggle-gemini-option-settings")).toBeChecked();
      });

      // Click toggle -> opens confirmation modal because gemini is selected
      await user.click(screen.getByTestId("toggle-gemini-option-settings"));
      expect(screen.getByTestId("gemini-disable-confirm-modal")).toBeInTheDocument();

      // Cancel button leaves state unchanged
      await user.click(screen.getByTestId("btn-cancel-disable-gemini"));
      expect(screen.queryByTestId("gemini-disable-confirm-modal")).toBeNull();
      expect(mockInvoke).not.toHaveBeenCalledWith("save_codex_review_settings", expect.anything());

      // Click toggle again and confirm
      await user.click(screen.getByTestId("toggle-gemini-option-settings"));
      expect(screen.getByTestId("gemini-disable-confirm-modal")).toBeInTheDocument();
      // Not priority, so no priority prompt
      expect(screen.queryByTestId("gemini-disable-priority-resolution")).toBeNull();

      await user.click(screen.getByTestId("btn-confirm-disable-gemini"));

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
          settings: {
            schema_version: 4,
        review_mode: "external_only",
            reviewers: { codex: true, gemini: false, deepseek: false },
            priority_reviewer: "codex",
            gemini_cli_enabled: false,
          },
        });
      });
    });

    it("handles priority resolution in confirmation modal when Gemini is priority: auto-assigns sole remaining reviewer", async () => {
      const user = userEvent.setup();
      const currentSettings: CodexReviewSettings = {
        schema_version: 4,
        review_mode: "external_only",
        reviewers: { codex: true, gemini: true, deepseek: false },
        priority_reviewer: "gemini",
        gemini_cli_enabled: true,
      };

      mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === "get_node_prerequisites_status") return dummyPrereqReady;
        if (cmd === "get_cli_tools_status") return dummyCliTools;
        if (cmd === "get_cli_auth_status") return dummyAuthInfo;
        if (cmd === "get_codex_review_settings") return currentSettings;
        if (cmd === "save_codex_review_settings") {
          const payload = args as { settings: CodexReviewSettings };
          return payload.settings;
        }
        return null;
      });

      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <CliToolsSettingsPanel />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("toggle-gemini-option-settings")).toBeChecked();
      });

      await user.click(screen.getByTestId("toggle-gemini-option-settings"));
      expect(screen.getByTestId("gemini-disable-confirm-modal")).toBeInTheDocument();
      expect(screen.getByTestId("gemini-disable-priority-resolution")).toBeInTheDocument();
      expect(screen.getByTestId("gemini-disable-priority-resolution")).toHaveTextContent("Codex CLI");

      const confirmBtn = screen.getByTestId("btn-confirm-disable-gemini");
      expect(confirmBtn).not.toBeDisabled();
      await user.click(confirmBtn);

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
          settings: {
            schema_version: 4,
        review_mode: "external_only",
            reviewers: { codex: true, gemini: false, deepseek: false },
            priority_reviewer: "codex", // auto-reassigned to the sole remaining reviewer
            gemini_cli_enabled: false,
          },
        });
      });
    });

    it("handles priority resolution in confirmation modal when Gemini is priority: requires radio choice among multiple remaining reviewers before enabling confirm", async () => {
      const user = userEvent.setup();
      const currentSettings: CodexReviewSettings = {
        schema_version: 4,
        review_mode: "external_only",
        reviewers: { codex: true, gemini: true, deepseek: true },
        priority_reviewer: "gemini",
        gemini_cli_enabled: true,
      };

      mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === "get_node_prerequisites_status") return dummyPrereqReady;
        if (cmd === "get_cli_tools_status") return dummyCliTools;
        if (cmd === "get_cli_auth_status") return dummyAuthInfo;
        if (cmd === "get_codex_review_settings") return currentSettings;
        if (cmd === "save_codex_review_settings") {
          const payload = args as { settings: CodexReviewSettings };
          return payload.settings;
        }
        return null;
      });

      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <CliToolsSettingsPanel />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("toggle-gemini-option-settings")).toBeChecked();
      });

      await user.click(screen.getByTestId("toggle-gemini-option-settings"));
      expect(screen.getByTestId("gemini-disable-confirm-modal")).toBeInTheDocument();
      expect(screen.getByTestId("radio-group-new-priority")).toBeInTheDocument();

      const confirmBtn = screen.getByTestId("btn-confirm-disable-gemini");
      // Initially disabled because 2 remaining reviewers and none chosen yet
      expect(confirmBtn).toBeDisabled();

      // Select deepseek
      await user.click(screen.getByTestId("radio-priority-deepseek"));
      expect(confirmBtn).not.toBeDisabled();

      await user.click(confirmBtn);

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
          settings: {
            schema_version: 4,
        review_mode: "external_only",
            reviewers: { codex: true, gemini: false, deepseek: true },
            priority_reviewer: "deepseek", // chosen priority
            gemini_cli_enabled: false,
          },
        });
      });
    });

    it("handles priority resolution in confirmation modal when Gemini is priority: shows zero-reviewer warning when 0 remaining reviewers", async () => {
      const user = userEvent.setup();
      const currentSettings: CodexReviewSettings = {
        schema_version: 4,
        review_mode: "external_only",
        reviewers: { codex: false, gemini: true, deepseek: false },
        priority_reviewer: "gemini",
        gemini_cli_enabled: true,
      };

      mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === "get_node_prerequisites_status") return dummyPrereqReady;
        if (cmd === "get_cli_tools_status") return dummyCliTools;
        if (cmd === "get_cli_auth_status") return dummyAuthInfo;
        if (cmd === "get_codex_review_settings") return currentSettings;
        if (cmd === "save_codex_review_settings") {
          const payload = args as { settings: CodexReviewSettings };
          return payload.settings;
        }
        return null;
      });

      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <CliToolsSettingsPanel />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("toggle-gemini-option-settings")).toBeChecked();
      });

      await user.click(screen.getByTestId("toggle-gemini-option-settings"));
      expect(screen.getByTestId("gemini-disable-confirm-modal")).toBeInTheDocument();
      expect(screen.getByTestId("zero-reviewers-warning")).toBeInTheDocument();

      const confirmBtn = screen.getByTestId("btn-confirm-disable-gemini");
      expect(confirmBtn).not.toBeDisabled();
      await user.click(confirmBtn);

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith("save_codex_review_settings", {
          settings: {
            schema_version: 4,
        review_mode: "external_only",
            reviewers: { codex: false, gemini: false, deepseek: false },
            priority_reviewer: null,
            gemini_cli_enabled: false,
          },
        });
      });
    });

    it("handles save failure gracefully by rolling back state and displaying error", async () => {
      const user = userEvent.setup();
      const currentSettings: CodexReviewSettings = {
        schema_version: 4,
        review_mode: "external_only",
        reviewers: { codex: true, gemini: false, deepseek: false },
        priority_reviewer: "codex",
        gemini_cli_enabled: false,
      };

      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === "get_node_prerequisites_status") return dummyPrereqReady;
        if (cmd === "get_cli_tools_status") return dummyCliTools;
        if (cmd === "get_cli_auth_status") return dummyAuthInfo;
        if (cmd === "get_codex_review_settings") return currentSettings;
        if (cmd === "save_codex_review_settings") {
          throw new Error("Disk write error");
        }
        return null;
      });

      render(
        <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
          <CliToolsSettingsPanel />
        </LanguageContext.Provider>
      );

      await waitFor(() => {
        expect(screen.getByTestId("toggle-gemini-option-settings")).toBeInTheDocument();
      });

      const toggle = screen.getByTestId("toggle-gemini-option-settings");
      await user.click(toggle);

      await waitFor(() => {
        expect(screen.getByTestId("gemini-option-error")).toBeInTheDocument();
        expect(screen.getByTestId("gemini-option-error")).toHaveTextContent("Disk write error");
        expect(screen.getByTestId("gemini-option-badge")).toHaveTextContent("Disabled");
      });
    });
  });
});
