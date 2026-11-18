import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import AgentOptCliSettingsCard from "./AgentOptCliSettingsCard";
import { translations as ja } from "../i18n/lang/ja";

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const cli = {
  installed: false,
  version: null,
  supported_version: false,
  cli_path: null,
  node_version: "v24.13.0",
  node_supported: true,
  npm_available: true,
  pinned_version: "0.1.0",
  error: null,
};

describe("AgentOptCliSettingsCard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_antigravity_nerf_engine_overview") return { cli };
      if (command === "install_agent_opt_cli") return { ...cli, installed: true, supported_version: true, version: "0.1.0" };
      throw new Error(`Unexpected command: ${command}`);
    });
  });

  it("only reads status on load and installs only after explicit confirmation", async () => {
    render(<AgentOptCliSettingsCard />);
    await waitFor(() => expect(screen.getByTestId("agentopt-cli-install")).toBeInTheDocument());
    expect(invokeMock).toHaveBeenCalledWith("get_antigravity_nerf_engine_overview");
    expect(invokeMock).not.toHaveBeenCalledWith("install_agent_opt_cli");

    fireEvent.click(screen.getByTestId("agentopt-cli-install"));
    expect(screen.getByTestId("agentopt-cli-install-confirm")).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("install_agent_opt_cli");
    fireEvent.click(screen.getByRole("button", { name: "Confirm install 0.1.0" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("install_agent_opt_cli"));
  });

  it("provides localized AgentOpt strings in Japanese", () => {
    expect(ja["agentOpt.cliTitle"]).toBe("AgentOpt CLI（固定 v{version}）");
    expect(ja["agentOpt.installCli"]).toBe("AgentOpt {version}をインストール");
  });
});
