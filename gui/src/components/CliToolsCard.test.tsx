import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import CliToolsCard from "./CliToolsCard";
import { LanguageContext } from "../i18n";
import type { CliToolInfo } from "../types";

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

const dummyCliTools: CliToolInfo[] = [
  {
    name: "codex",
    display_name: "Codex CLI",
    installed: true,
    path: "C:\\Users\\Sohei\\AppData\\Roaming\\npm\\codex.cmd",
    installed_version: "0.163.0",
    latest_version: null,
    package_name: "@openai/codex",
    is_preview: false,
    update_available: false,
  },
  {
    name: "dsh",
    display_name: "DeepSeek Harness",
    installed: false,
    path: null,
    installed_version: null,
    latest_version: null,
    package_name: "@deepseek-ai/dsh",
    is_preview: true,
    update_available: false,
  },
];

describe("CliToolsCard Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    invokeMock.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_cli_tools_status") {
        return dummyCliTools;
      }
      if (command === "install_cli_tool") {
        const payload = args as { name: string };
        return {
          name: payload.name,
          display_name: payload.name === "codex" ? "Codex CLI" : "DeepSeek Harness",
          installed: true,
          path: `C:\\dummy\\${payload.name}.cmd`,
          installed_version: "1.0.0",
          latest_version: null,
          package_name: payload.name === "codex" ? "@openai/codex" : "@deepseek-ai/dsh",
          is_preview: payload.name === "dsh",
          update_available: false,
        };
      }
      return undefined;
    });
  });

  it("renders CLI tools with install states, paths, and preview notices", async () => {
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CliToolsCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("cli-tools-card")).toBeInTheDocument();
    });

    // Codex CLI
    const codexItem = screen.getByTestId("cli-tool-item-codex");
    expect(codexItem).toHaveTextContent("Codex CLI");
    expect(codexItem).toHaveTextContent("0.163.0");
    expect(codexItem).toHaveTextContent("C:\\Users\\Sohei\\AppData\\Roaming\\npm\\codex.cmd");
    expect(screen.getByTestId("btn-install-codex")).toHaveTextContent("Update");

    // DeepSeek Harness
    const dshItem = screen.getByTestId("cli-tool-item-dsh");
    expect(dshItem).toHaveTextContent("DeepSeek Harness");
    expect(dshItem.textContent).toMatch(/Not installed|antigravity\.cliToolNotInstalled/);
    expect(dshItem.textContent).toMatch(/Developer Preview|antigravity\.cliToolPreviewNotice/);
    expect(screen.getByTestId("btn-install-dsh").textContent).toMatch(/Install|antigravity\.cliToolInstallBtn/);
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

    // Modal should be open
    const modal = screen.getByTestId("cli-confirm-modal");
    expect(modal).toBeInTheDocument();
    expect(modal).toHaveTextContent("@deepseek-ai/dsh@latest");
    expect(modal.textContent).toMatch(/Developer Preview|antigravity\.cliToolPreviewNotice/);

    // Click Proceed
    await user.click(screen.getByTestId("cli-confirm-proceed"));

    expect(invokeMock).toHaveBeenCalledWith("install_cli_tool", { name: "dsh" });
    await waitFor(() => {
      expect(screen.queryByTestId("cli-confirm-modal")).not.toBeInTheDocument();
      expect(screen.getByTestId("cli-success-msg")).toBeInTheDocument();
    });
  });

  it("cancels install confirmation when Cancel is clicked", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <CliToolsCard />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-install-codex")).toBeInTheDocument();
    });

    // Click Update on Codex
    await user.click(screen.getByTestId("btn-install-codex"));
    expect(screen.getByTestId("cli-confirm-modal")).toBeInTheDocument();

    // Click Cancel
    await user.click(screen.getByTestId("cli-confirm-cancel"));
    expect(screen.queryByTestId("cli-confirm-modal")).not.toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("install_cli_tool", expect.anything());
  });
});
