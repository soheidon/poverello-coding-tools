import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import CliToolsCard from "./CliToolsCard";
import { LanguageContext } from "../i18n";
import type { CliToolInfo } from "../types";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

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
    prerequisite_error: "Node.js >= 20 is required (found: v18.0.0).",
  },
];

describe("CliToolsCard Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_cli_tools_status") {
        return dummyCliTools;
      }
      if (command === "get_cli_auth_status") {
        return [
          {
            name: "codex",
            status: "authenticated",
            auth_type: "ChatGPT Plus",
            message: null,
            can_launch_login: true,
            account_identity: "soheidon@yahoo.co.jp",
            codex_home: "C:\\Users\\Sohei\\.codex",
            last_checked_at: "2026-11-01T15:00:00.000Z",
          },
          { name: "dsh", status: "unknown", auth_type: null, message: "Configure API keys via local Web UI", can_launch_login: true },
          { name: "gemini", status: "unknown", auth_type: null, message: "Interactive authentication only", can_launch_login: true },
        ];
      }
      if (command === "launch_cli_auth") {
        return null;
      }
      if (command === "check_cli_tools_latest_version") {
        return [
          {
            ...dummyCliTools[0],
            latest_version: "0.164.0",
            update_available: true,
            check_id: "check_valid_123",
          },
          {
            ...dummyCliTools[1],
            latest_version: "0.2.1-rc.5",
            update_available: false,
            check_id: "check_valid_123",
          },
          dummyCliTools[2],
        ];
      }
      if (command === "update_cli_tool") {
        const payload = args as { name: string; checkId: string };
        return {
          name: payload.name,
          display_name: "Codex CLI",
          installed: true,
          path: `C:\\dummy\\${payload.name}.cmd`,
          installed_version: "0.164.0",
          latest_version: "0.164.0",
          latest_checked_at: null,
          package_name: "@openai/codex",
          is_preview: false,
          update_available: false,
          check_id: null,
          prerequisite_error: null,
        };
      }
      if (command === "install_cli_tool") {
        const payload = args as { name: string };
        return {
          name: payload.name,
          display_name: payload.name === "codex" ? "Codex CLI" : payload.name === "dsh" ? "DeepSeek Harness" : "Gemini CLI",
          installed: true,
          path: `C:\\dummy\\${payload.name}.cmd`,
          installed_version: "1.0.0",
          latest_version: null,
          latest_checked_at: null,
          package_name: payload.name === "codex" ? "@openai/codex" : payload.name === "dsh" ? "@deepseek-ai/dsh" : "@google/gemini-cli",
          is_preview: payload.name === "dsh",
          update_available: false,
          prerequisite_error: null,
        };
      }
      return undefined;
    });
  });

  it("renders 3 CLI tools with install states, paths, and prerequisite notices", async () => {
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CliToolsCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("cli-tools-card")).toBeInTheDocument();
    });

    // Codex CLI (installed and up to date, no update button rendered initially)
    const codexItem = screen.getByTestId("cli-tool-item-codex");
    expect(codexItem).toHaveTextContent("Codex CLI");
    expect(codexItem).toHaveTextContent("0.163.0");
    expect(codexItem).toHaveTextContent("C:\\Users\\Sohei\\AppData\\Roaming\\npm\\codex.cmd");
    expect(screen.queryByTestId("btn-install-codex")).toBeNull();
    expect(screen.queryByTestId("btn-update-codex")).toBeNull();

    // DeepSeek Harness
    const dshItem = screen.getByTestId("cli-tool-item-dsh");
    expect(dshItem).toHaveTextContent("DeepSeek Harness");
    expect(dshItem.textContent).toMatch(/Not installed|cliToolNotInstalled/);
    expect(dshItem.textContent).not.toMatch(/Developer Preview|cliToolPreviewNotice/);
    expect(screen.queryByTestId("preview-badge")).toBeNull();
    expect(screen.getByTestId("btn-install-dsh").textContent).toMatch(/Install|cliToolInstallBtn/);

    // Gemini CLI with prerequisite error
    const geminiItem = screen.getByTestId("cli-tool-item-gemini");
    expect(geminiItem).toHaveTextContent("Gemini CLI");
    expect(geminiItem).toHaveTextContent("Node.js >= 20 is required");
    expect(screen.getByTestId("btn-install-gemini")).toBeDisabled();
  });

  it("checks latest versions on Check for Updates button click and allows update", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CliToolsCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-check-latest")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("btn-check-latest"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("check_cli_tools_latest_version");
      expect(screen.getByTestId("update-badge-codex").textContent).toMatch(/0\.164\.0/);
      expect(screen.getByTestId("btn-update-codex")).toBeInTheDocument();
    });

    // Click Update on Codex CLI
    await user.click(screen.getByTestId("btn-update-codex"));
    expect(screen.getByTestId("cli-confirm-modal")).toBeInTheDocument();
    expect(screen.getByTestId("cli-confirm-modal")).toHaveTextContent("@openai/codex@0.164.0");

    // Proceed with update
    await user.click(screen.getByTestId("cli-confirm-proceed"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("update_cli_tool", { name: "codex", checkId: "check_valid_123" });
    });
  });

  it("opens confirmation modal on click and proceeds with install on confirmation", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CliToolsCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-install-dsh")).toBeInTheDocument();
    });

    // Click Install on DeepSeek Harness
    await user.click(screen.getByTestId("btn-install-dsh"));

    expect(screen.getByTestId("cli-confirm-modal")).toBeInTheDocument();
    expect(screen.getByTestId("cli-confirm-modal")).toHaveTextContent("@deepseek-ai/dsh@latest");

    // Proceed
    await user.click(screen.getByTestId("cli-confirm-proceed"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("install_cli_tool", { name: "dsh" });
    });
  });

  it("renders auth status badges, Gemini sandbox notice, and launches auth modal", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CliToolsCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      // Codex auth badge
      expect(screen.getByTestId("auth-badge-codex")).toHaveTextContent("ChatGPT Plus");
      // DSH auth badge
      expect(screen.getByTestId("auth-badge-dsh")).toHaveTextContent("cliTools.authUnknown");
      // DSH auth message
      expect(screen.getByTestId("auth-message-dsh")).toHaveTextContent("Configure API keys via local Web UI");
      // Gemini sandbox warning
      expect(screen.getByTestId("gemini-sandbox-warning")).toBeInTheDocument();
    });

    // Click launch auth on Codex
    const launchCodexBtn = screen.getByTestId("btn-launch-auth-codex");
    await user.click(launchCodexBtn);

    // Modal appears
    expect(screen.getByTestId("cli-auth-modal")).toBeInTheDocument();
    expect(screen.getByTestId("cli-auth-modal")).toHaveTextContent("Codex CLI");

    // Cancel modal
    await user.click(screen.getByTestId("cli-auth-cancel"));
    expect(screen.queryByTestId("cli-auth-modal")).toBeNull();

    // Click launch auth on DeepSeek
    const launchDshBtn = screen.getByTestId("btn-launch-auth-dsh");
    await user.click(launchDshBtn);
    expect(screen.getByTestId("cli-auth-modal")).toBeInTheDocument();
    expect(screen.getByTestId("cli-auth-modal")).toHaveTextContent("DeepSeek Harness");

    // Proceed
    await user.click(screen.getByTestId("cli-auth-proceed"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("launch_cli_auth", { toolName: "dsh" });
    });
  });

  it("renders Codex account identity, effective Codex Home, and supports re-checking auth status", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CliToolsCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("auth-badge-codex")).toHaveTextContent("ChatGPT Plus");
      expect(screen.getByTestId("codex-account-identity")).toHaveTextContent("soheidon@yahoo.co.jp");
      expect(screen.getByTestId("codex-effective-home")).toHaveTextContent("C:\\Users\\Sohei\\.codex");
      expect(screen.getByTestId("btn-recheck-auth-codex")).toBeInTheDocument();
    });

    // Click Re-check button
    const recheckBtn = screen.getByTestId("btn-recheck-auth-codex");
    await user.click(recheckBtn);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_cli_auth_status");
    });
  });
});
