import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import McpSettingPanel, { isGpt61SolModel } from "./McpSettingPanel";
import type { GatewayConfig, AntigravityMcpInfo, AntigravityCommandsInfo } from "../types";
import { MODEL_CAPABILITIES } from "../modelCapabilities";
import { translations as en } from "../i18n/lang/en";
import { translations as ja } from "../i18n/lang/ja";
import { translations as de } from "../i18n/lang/de";
import { translations as es } from "../i18n/lang/es";
import { translations as fr } from "../i18n/lang/fr";
import { translations as ko } from "../i18n/lang/ko";
import { translations as zhCN } from "../i18n/lang/zh-CN";
import { translations as zhTW } from "../i18n/lang/zh-TW";

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

const dummyConfig: GatewayConfig = {
  active_provider: "deepseek",
  providers: {
    deepseek: {
      display_name: "DeepSeek",
      upstream_url: "https://api.deepseek.com",
      api_key_env: "DEEPSEEK_API_KEY",
      default_model: "deepseek-chat",
      force_anthropic_version: null,
      supports_count_tokens: true,
      supports_vision: false,
      supports_video: false,
      supports_thinking: true,
      model_map: {},
      visible_models: ["deepseek-chat", "deepseek-reasoner"],
    },
  },
  server: {
    host: "127.0.0.1",
    port: 4000,
    enable_cors: true,
  },
};

describe("McpSettingPanel - Antigravity Integration", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_mcp_config") {
        return {
          provider: "deepseek",
          model: "deepseek-v4-pro",
          thinking_mode: "thinking",
          reasoning_effort: "high",
        };
      }
      if (cmd === "get_antigravity_mcp_status") {
        return {
          status: "not_configured",
          config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
          config_dir: "C:\\Users\\User\\.gemini\\config",
          registered_command: null,
          registered_args: null,
          error: null,
        } as AntigravityMcpInfo;
      }
      if (cmd === "get_antigravity_commands_status") {
        return {
          skills_dir: "C:\\Users\\User\\.gemini\\config\\skills",
          plan_command: {
            name: "anthro-plan",
            slash_command: "/anthro-plan",
            status: "not_installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\anthro-plan\\SKILL.md",
            error: null,
          },
          revise_command: {
            name: "anthro-revise",
            slash_command: "/anthro-revise",
            status: "not_installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\anthro-revise\\SKILL.md",
            error: null,
          },
          review_command: {
            name: "anthro-review",
            slash_command: "/anthro-review",
            status: "not_installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\anthro-review\\SKILL.md",
            error: null,
          },
        } as AntigravityCommandsInfo;
      }
      if (cmd === "select_executable_dialog") {
        return "C:\\Users\\User\\AppData\\Local\\Poverello\\anthro-bridge.exe";
      }
      if (cmd === "configure_antigravity_mcp") {
        return {
          status: "configured",
          config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
          config_dir: "C:\\Users\\User\\.gemini\\config",
          registered_command: "C:\\Users\\User\\AppData\\Local\\Poverello\\anthro-bridge.exe",
          registered_args: ["--mcp-server"],
          error: null,
        } as AntigravityMcpInfo;
      }
      return {};
    });
  });

  it("renders Not configured state with empty selection, disabled update button, and change button when expanded", async () => {
    render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/antigravity\.header/i)).toBeInTheDocument();
    });

    expect(screen.getAllByText(/antigravity\.statusNotConfigured/i).length).toBeGreaterThan(0);

    // Update button is disabled because no executable is selected yet
    const updateBtn = screen.getByRole("button", { name: /antigravity\.btnUpdate/i });
    expect(updateBtn).toBeDisabled();
    expect(screen.getByRole("button", { name: /antigravity\.btnChangeExe/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /antigravity\.btnOpenFolder/i })).toBeInTheDocument();
  });

  it("renders Configured state with registered path, disabled update button, and Remove button when expanded", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_mcp_config") return {};
      if (cmd === "get_antigravity_commands_status") {
        return {
          skills_dir: "",
          plan_command: { name: "anthro-plan", status: "not_installed" },
          revise_command: { name: "anthro-revise", status: "not_installed" },
        };
      }
      if (cmd === "get_antigravity_mcp_status") {
        return {
          status: "configured",
          config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
          config_dir: "C:\\Users\\User\\.gemini\\config",
          registered_command: "C:\\Users\\User\\AppData\\Local\\Poverello\\anthro-bridge.exe",
          registered_args: ["--mcp-server"],
          error: null,
        } as AntigravityMcpInfo;
      }
      return null;
    });

    render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getAllByText(/antigravity\.statusConfigured/i).length).toBeGreaterThan(0);
    });



    // Update button is disabled because selectedExePath matches registered_command
    expect(screen.getByRole("button", { name: /antigravity\.btnUpdate/i })).toBeDisabled();
    expect(screen.getByRole("button", { name: /antigravity\.btnRemove/i })).toBeInTheDocument();
  });

  it("switches to Mismatch state and enables Update button after selecting a different exe", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_mcp_config") {
        return {
          provider: "deepseek",
          model: "deepseek-v4-pro",
          thinking_mode: "thinking",
          reasoning_effort: "high",
        };
      }
      if (cmd === "get_antigravity_commands_status") {
        return {
          skills_dir: "",
          plan_command: { name: "anthro-plan", status: "not_installed" },
          revise_command: { name: "anthro-revise", status: "not_installed" },
        };
      }
      if (cmd === "get_antigravity_mcp_status") {
        return {
          status: "configured",
          config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
          config_dir: "C:\\Users\\User\\.gemini\\config",
          registered_command: "C:\\Users\\User\\AppData\\Local\\Poverello\\anthro-bridge.exe",
          registered_args: ["--mcp-server"],
          error: null,
        } as AntigravityMcpInfo;
      }
      if (cmd === "select_executable_dialog") {
        return "C:\\Users\\User\\dev\\anthro-bridge\\gui\\src-tauri\\target\\release\\anthro-bridge.exe";
      }
      return null;
    });

    render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getAllByText(/antigravity\.statusConfigured/i).length).toBeGreaterThan(0);
    });

    // Click Change button
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /antigravity\.btnChangeExe/i }));
    });

    // Status becomes mismatch (outdated key), Update button enabled
    expect(screen.getAllByText(/antigravity\.statusOutdated/i).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: /antigravity\.btnUpdate/i })).toBeEnabled();
  });

  it("renders Invalid state with error message", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_mcp_config") {
        return {
          provider: "deepseek",
          model: "deepseek-v4-pro",
          thinking_mode: "thinking",
          reasoning_effort: "high",
        };
      }
      if (cmd === "get_antigravity_commands_status") {
        return {
          skills_dir: "",
          plan_command: { name: "anthro-plan", status: "not_installed" },
          revise_command: { name: "anthro-revise", status: "not_installed" },
        };
      }
      if (cmd === "get_antigravity_mcp_status") {
        return {
          status: "invalid",
          config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
          config_dir: "C:\\Users\\User\\.gemini\\config",
          registered_command: null,
          registered_args: null,
          error: "Syntax error on line 1",
        } as AntigravityMcpInfo;
      }
      return null;
    });

    render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getAllByText(/antigravity\.statusInvalid/i).length).toBeGreaterThan(0);
    });
  });

  it("triggers open_antigravity_mcp_config_folder on Open Settings Folder click", async () => {
    render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/antigravity\.header/i)).toBeInTheDocument();
    });



    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /antigravity\.btnOpenFolder/i }));
    });

    expect(invokeMock).toHaveBeenCalledWith("open_antigravity_mcp_config_folder");
  });

  it("triggers configure_antigravity_mcp with chosen exePath on update click", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_mcp_config") {
        return {
          provider: "deepseek",
          model: "deepseek-v4-pro",
          thinking_mode: "thinking",
          reasoning_effort: "high",
        };
      }
      if (cmd === "get_antigravity_commands_status") {
        return {
          skills_dir: "",
          plan_command: { name: "anthro-plan", status: "not_installed" },
          revise_command: { name: "anthro-revise", status: "not_installed" },
          review_command: { name: "anthro-review", status: "not_installed" },
        };
      }
      if (cmd === "get_antigravity_mcp_status") {
        return {
          status: "not_configured",
          config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
          config_dir: "C:\\Users\\User\\.gemini\\config",
          registered_command: null,
          registered_args: null,
          error: null,
        } as AntigravityMcpInfo;
      }
      if (cmd === "select_executable_dialog") {
        return "C:\\Users\\User\\AppData\\Local\\Poverello\\anthro-bridge.exe";
      }
      if (cmd === "configure_antigravity_mcp") {
        return {
          status: "configured",
          config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
          config_dir: "C:\\Users\\User\\.gemini\\config",
          registered_command: "C:\\Users\\User\\AppData\\Local\\Poverello\\anthro-bridge.exe",
          registered_args: ["--mcp-server"],
          error: null,
        } as AntigravityMcpInfo;
      }
      return null;
    });

    render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getAllByText(/antigravity\.statusNotConfigured/i).length).toBeGreaterThan(0);
    });

    // Select exe
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /antigravity\.btnChangeExe/i }));
    });

    // Update button enabled -> click
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /antigravity\.btnUpdate/i }));
    });

    expect(invokeMock).toHaveBeenCalledWith("configure_antigravity_mcp", {
      exePath: "C:\\Users\\User\\AppData\\Local\\Poverello\\anthro-bridge.exe",
    });

    await waitFor(() => {
      expect(screen.getAllByText(/antigravity\.statusConfigured/i).length).toBeGreaterThan(0);
    });
  });

  // ── Antigravity Commands Tests ──
  it("renders Commands Not Installed state with Install buttons and InstallAll button", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_mcp_config") {
        return {
          provider: "deepseek",
          model: "deepseek-v4-pro",
          thinking_mode: "thinking",
          reasoning_effort: "high",
        };
      }
      if (cmd === "get_antigravity_mcp_status") {
        return {
          status: "configured",
          config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
          config_dir: "C:\\Users\\User\\.gemini\\config",
          registered_command: "C:\\Users\\User\\AppData\\Local\\Poverello\\anthro-bridge.exe",
          registered_args: ["--mcp-server"],
          error: null,
        } as AntigravityMcpInfo;
      }
      if (cmd === "get_antigravity_commands_status") {
        return {
          skills_dir: "C:\\Users\\User\\.gemini\\config\\skills",
          plan_command: {
            name: "anthro-plan",
            slash_command: "/anthro-plan",
            status: "not_installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\anthro-plan\\SKILL.md",
            error: null,
          },
          revise_command: {
            name: "anthro-revise",
            slash_command: "/anthro-revise",
            status: "not_installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\anthro-revise\\SKILL.md",
            error: null,
          },
          review_command: {
            name: "anthro-review",
            slash_command: "/anthro-review",
            status: "not_installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\anthro-review\\SKILL.md",
            error: null,
          },
        };
      }
      if (cmd === "get_pov_nerf_commands_status") {
        return {
          skills_dir: "C:\\skills",
          skills: [
            {
              skill_id: "balanced",
              skill_name: "pov-nerf-balanced",
              slash_command: "/pov-nerf-balanced",
              target_path: "C:\\skills\\pov-nerf-balanced\\SKILL.md",
              state: "installed",
              permitted_actions: ["remove"],
              diagnostic_reason: null,
            },
            {
              skill_id: "lightweight",
              skill_name: "pov-nerf-lightweight",
              slash_command: "/pov-nerf-lightweight",
              target_path: "C:\\skills\\pov-nerf-lightweight\\SKILL.md",
              state: "installed",
              permitted_actions: ["remove"],
              diagnostic_reason: null,
            },
          ],
          legacy_single_skill: null,
        };
      }
      return null;
    });

    render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/antigravity\.header/i)).toBeInTheDocument();
    });



    expect(screen.getAllByText(/antigravity\.commandStatusNotInstalled/i).length).toBe(3);
    expect(screen.getAllByRole("button", { name: /antigravity\.commandBtnInstall/i }).length).toBe(3);
    expect(screen.getByRole("button", { name: /antigravity\.btnInstallAll/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /antigravity\.btnOpenSkillsFolder/i })).toBeInTheDocument();
  });

  it("renders Commands Installed state with Remove buttons and no InstallAll button", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_mcp_config") {
        return {
          provider: "deepseek",
          model: "deepseek-v4-pro",
          thinking_mode: "thinking",
          reasoning_effort: "high",
        };
      }
      if (cmd === "get_antigravity_mcp_status") {
        return {
          status: "configured",
          config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
          config_dir: "C:\\Users\\User\\.gemini\\config",
          registered_command: "C:\\Users\\User\\AppData\\Local\\Poverello\\anthro-bridge.exe",
          registered_args: ["--mcp-server"],
          error: null,
        } as AntigravityMcpInfo;
      }
      if (cmd === "get_antigravity_commands_status") {
        return {
          skills_dir: "C:\\Users\\User\\.gemini\\config\\skills",
          plan_command: {
            name: "anthro-plan",
            slash_command: "/anthro-plan",
            status: "installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\anthro-plan\\SKILL.md",
            error: null,
          },
          revise_command: {
            name: "anthro-revise",
            slash_command: "/anthro-revise",
            status: "installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\anthro-revise\\SKILL.md",
            error: null,
          },
          review_command: {
            name: "anthro-review",
            slash_command: "/anthro-review",
            status: "installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\anthro-review\\SKILL.md",
            error: null,
          },
        };
      }
      return null;
    });

    render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/antigravity\.header/i)).toBeInTheDocument();
    });



    expect(screen.getAllByText(/antigravity\.commandStatusInstalled/i).length).toBe(3);
    expect(screen.getByRole("button", { name: /antigravity\.btnRemove/i })).toBeInTheDocument(); // MCP Remove button
    expect(screen.getAllByRole("button", { name: /antigravity\.commandBtnRemove/i }).length).toBe(3); // Commands Remove buttons
    expect(screen.queryByRole("button", { name: /antigravity\.btnInstallAll/i })).not.toBeInTheDocument();
  });

  it("triggers install_all_antigravity_mcp_commands on Install All click and does not install workflow skills", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_mcp_config") {
        return {
          provider: "deepseek",
          model: "deepseek-v4-pro",
          thinking_mode: "thinking",
          reasoning_effort: "high",
        };
      }
      if (cmd === "get_antigravity_mcp_status") {
        return {
          status: "configured",
          config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
          config_dir: "C:\\Users\\User\\.gemini\\config",
          registered_command: "C:\\Users\\User\\AppData\\Local\\Poverello\\anthro-bridge.exe",
          registered_args: ["--mcp-server"],
          error: null,
        } as AntigravityMcpInfo;
      }
      if (cmd === "get_antigravity_commands_status") {
        return {
          skills_dir: "C:\\Users\\User\\.gemini\\config\\skills",
          plan_command: {
            name: "pov-mcp-plan",
            slash_command: "/pov-mcp-plan",
            status: "not_installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-mcp-plan\\SKILL.md",
            error: null,
          },
          revise_command: {
            name: "pov-mcp-revise",
            slash_command: "/pov-mcp-revise",
            status: "not_installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-mcp-revise\\SKILL.md",
            error: null,
          },
          review_command: {
            name: "pov-mcp-review",
            slash_command: "/pov-mcp-review",
            status: "not_installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-mcp-review\\SKILL.md",
            error: null,
          },
        };
      }
      if (cmd === "install_all_antigravity_mcp_commands") {
        return {
          skills_dir: "C:\\Users\\User\\.gemini\\config\\skills",
          plan_command: {
            name: "pov-mcp-plan",
            slash_command: "/pov-mcp-plan",
            status: "installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-mcp-plan\\SKILL.md",
            error: null,
          },
          revise_command: {
            name: "pov-mcp-revise",
            slash_command: "/pov-mcp-revise",
            status: "installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-mcp-revise\\SKILL.md",
            error: null,
          },
          review_command: {
            name: "pov-mcp-review",
            slash_command: "/pov-mcp-review",
            status: "installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-mcp-review\\SKILL.md",
            error: null,
          },
        };
      }
      return null;
    });

    render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/antigravity\.header/i)).toBeInTheDocument();
    });

    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /antigravity\.btnInstallAll/i }));
    });

    expect(invokeMock).toHaveBeenCalledWith("install_all_antigravity_mcp_commands");
    expect(invokeMock).not.toHaveBeenCalledWith("install_all_antigravity_commands");
    expect(invokeMock).not.toHaveBeenCalledWith("install_antigravity_command", { name: "pov-plan-only" });
    expect(invokeMock).not.toHaveBeenCalledWith("install_antigravity_command", { name: "pov-build-review" });
    expect(invokeMock).not.toHaveBeenCalledWith("install_antigravity_command", { name: "pov-full-cycle" });
    await waitFor(() => {
      expect(screen.getAllByText(/antigravity\.commandStatusInstalled/i).length).toBe(3);
    });
  });

  it("shows every per-Skill outcome when Install All is refused during preflight", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_mcp_config") return {};
      if (cmd === "get_antigravity_mcp_status") return { status: "configured" };
      if (cmd === "get_antigravity_commands_status") {
        return {
          skills_dir: "C:\\skills",
          plan_command: { name: "pov-mcp-plan", slash_command: "/pov-mcp-plan", status: "not_installed", skill_path: "", error: null },
          revise_command: { name: "pov-mcp-revise", slash_command: "/pov-mcp-revise", status: "invalid", skill_path: "", error: "Modified by user" },
          review_command: { name: "pov-mcp-review", slash_command: "/pov-mcp-review", status: "not_installed", skill_path: "", error: null },
        };
      }
      if (cmd === "install_all_antigravity_mcp_commands") {
        return {
          skills_dir: "C:\\skills",
          plan_command: { name: "pov-mcp-plan", slash_command: "/pov-mcp-plan", status: "not_installed", skill_path: "", error: null },
          revise_command: { name: "pov-mcp-revise", slash_command: "/pov-mcp-revise", status: "invalid", skill_path: "", error: "Modified by user" },
          review_command: { name: "pov-mcp-review", slash_command: "/pov-mcp-review", status: "not_installed", skill_path: "", error: null },
          install_all_results: [
            { name: "pov-mcp-plan", outcome: "not_attempted", message: "Skipped because another target failed preflight" },
            { name: "pov-mcp-revise", outcome: "refused", message: "Modified by user" },
            { name: "pov-mcp-review", outcome: "not_attempted", message: "Skipped because another target failed preflight" },
          ],
        };
      }
      return null;
    });

    render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);
    const installAll = await screen.findByRole("button", { name: /antigravity\.btnInstallAll/i });
    expect(screen.getByText("Modified by user")).toBeInTheDocument();
    expect(screen.queryAllByRole("button", { name: /antigravity\.commandBtnUpdate/i })).toHaveLength(0);
    expect(screen.queryAllByRole("button", { name: /antigravity\.commandBtnRemove/i })).toHaveLength(0);
    await userEvent.click(installAll);

    const resultPanel = await screen.findByRole("status");
    expect(resultPanel.textContent).toContain("pov-mcp-plan");
    expect(resultPanel.textContent).toContain("antigravity.commandInstallOutcomeNotAttempted");
    expect(resultPanel.textContent).toContain("pov-mcp-revise");
    expect(resultPanel.textContent).toContain("antigravity.commandInstallOutcomeRefused");
    expect(resultPanel.textContent).toContain("Modified by user");
    expect(resultPanel.textContent).toContain("pov-mcp-review");
  });

  it("shows MCP warning when MCP is not configured", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_mcp_config") {
        return {
          provider: "deepseek",
          model: "deepseek-v4-pro",
          thinking_mode: "thinking",
          reasoning_effort: "high",
        };
      }
      if (cmd === "get_antigravity_mcp_status") {
        return {
          status: "not_configured",
          config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
          config_dir: "C:\\Users\\User\\.gemini\\config",
          registered_command: null,
          registered_args: null,
          error: null,
        } as AntigravityMcpInfo;
      }
      if (cmd === "get_antigravity_commands_status") {
        return {
          skills_dir: "C:\\Users\\User\\.gemini\\config\\skills",
          plan_command: {
            name: "anthro-plan",
            slash_command: "/anthro-plan",
            status: "installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\anthro-plan\\SKILL.md",
            error: null,
          },
          revise_command: {
            name: "anthro-revise",
            slash_command: "/anthro-revise",
            status: "installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\anthro-revise\\SKILL.md",
            error: null,
          },
          review_command: {
            name: "anthro-review",
            slash_command: "/anthro-review",
            status: "installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\anthro-review\\SKILL.md",
            error: null,
          },
        };
      }
      return null;
    });

    render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/antigravity\.header/i)).toBeInTheDocument();
    });



    expect(screen.getByText(/antigravity\.commandsMcpWarning/i)).toBeInTheDocument();
  });

  it("displays canonical /pov-* commands and shows legacy notice when legacy skills exist", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_mcp_config") {
        return {
          provider: "deepseek",
          model: "deepseek-v4-pro",
          thinking_mode: "thinking",
          reasoning_effort: "high",
        };
      }
      if (cmd === "get_antigravity_mcp_status") {
        return {
          status: "configured",
          legacy_status: "configured",
          config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
          config_dir: "C:\\Users\\User\\.gemini\\config",
          registered_command: null,
          registered_args: null,
          legacy_command: "C:\\Users\\User\\AppData\\Local\\Poverello\\anthro-bridge.exe",
          legacy_args: ["--mcp-server"],
          error: null,
        } as AntigravityMcpInfo;
      }
      if (cmd === "get_antigravity_commands_status") {
        return {
          skills_dir: "C:\\Users\\User\\.gemini\\config\\skills",
          plan_command: {
            name: "pov-mcp-plan",
            slash_command: "/pov-mcp-plan",
            status: "not_installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-mcp-plan\\SKILL.md",
            error: null,
          },
          revise_command: {
            name: "pov-mcp-revise",
            slash_command: "/pov-mcp-revise",
            status: "not_installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-mcp-revise\\SKILL.md",
            error: null,
          },
          review_command: {
            name: "pov-mcp-review",
            slash_command: "/pov-mcp-review",
            status: "not_installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-mcp-review\\SKILL.md",
            error: null,
          },
          legacy_plan_command: {
            name: "anthro-plan",
            slash_command: "/anthro-plan",
            status: "installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\anthro-plan\\SKILL.md",
            error: null,
          },
        } as AntigravityCommandsInfo;
      }
      if (cmd === "get_current_executable_path") {
        return "C:\\Users\\User\\AppData\\Local\\Poverello\\anthro-bridge.exe";
      }
      return null;
    });

    render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/antigravity\.header/i)).toBeInTheDocument();
    });

    expect(screen.getByText("/pov-mcp-plan")).toBeInTheDocument();
    expect(screen.getByText("/pov-mcp-revise")).toBeInTheDocument();
    expect(screen.getByText("/pov-mcp-review")).toBeInTheDocument();
    expect(screen.getByText(/antigravity\.legacyNoticeTitle/i)).toBeInTheDocument();
    expect(screen.getByText(/antigravity\.legacyNoticeDesc/i)).toBeInTheDocument();
    expect(screen.getByText(/antigravity\.mcpLegacyPreserved/i)).toBeInTheDocument();

    const exeInput = screen.getByDisplayValue("C:\\Users\\User\\AppData\\Local\\Poverello\\anthro-bridge.exe");
    expect(exeInput).toBeInTheDocument();
  });

  it("does not render workflow skills (/pov-plan-only, /pov-build-review, /pov-full-cycle) in McpSettingPanel", async () => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_mcp_config") {
        return {
          provider: "deepseek",
          model: "deepseek-chat",
        };
      }
      if (cmd === "get_antigravity_mcp_status") {
        return {
          status: "configured",
          config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
          config_dir: "C:\\Users\\User\\.gemini\\config",
          registered_command: "C:\\Users\\User\\AppData\\Local\\Poverello\\anthro-bridge.exe",
          registered_args: ["--mcp-server"],
          error: null,
        } as AntigravityMcpInfo;
      }
      if (cmd === "get_antigravity_commands_status") {
        return {
          skills_dir: "C:\\Users\\User\\.gemini\\config\\skills",
          commands: [
            { name: "pov-mcp-plan", slash_command: "/pov-mcp-plan", status: "installed", skill_path: "", error: null },
            { name: "pov-mcp-revise", slash_command: "/pov-mcp-revise", status: "installed", skill_path: "", error: null },
            { name: "pov-mcp-review", slash_command: "/pov-mcp-review", status: "installed", skill_path: "", error: null },
            { name: "pov-full-cycle", slash_command: "/pov-full-cycle", status: "installed", skill_path: "", error: null },
            { name: "pov-plan-only", slash_command: "/pov-plan-only", status: "outdated", skill_path: "", error: null },
            { name: "pov-build-review", slash_command: "/pov-build-review", status: "outdated", skill_path: "", error: null },
          ],
          plan_command: { name: "pov-mcp-plan", slash_command: "/pov-mcp-plan", status: "installed", skill_path: "", error: null },
          revise_command: { name: "pov-mcp-revise", slash_command: "/pov-mcp-revise", status: "installed", skill_path: "", error: null },
          review_command: { name: "pov-mcp-review", slash_command: "/pov-mcp-review", status: "installed", skill_path: "", error: null },
        } as AntigravityCommandsInfo;
      }
      return null;
    });

    render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("/pov-mcp-plan")).toBeInTheDocument();
    });

    expect(screen.getByText("/pov-mcp-revise")).toBeInTheDocument();
    expect(screen.getByText("/pov-mcp-review")).toBeInTheDocument();

    expect(screen.queryByText("/pov-plan-only")).not.toBeInTheDocument();
    expect(screen.queryByText("/pov-build-review")).not.toBeInTheDocument();
    expect(screen.queryByText("/pov-full-cycle")).not.toBeInTheDocument();
  });
});

describe("McpSettingPanel - Thinking-only and Capability-driven MCP UI", () => {
  const kimiCodeConfig: GatewayConfig = {
    active_provider: "kimi-code",
    providers: {
      "kimi-code": {
        display_name: "Kimi Code",
        upstream_url: "https://api.kimi.com/coding",
        api_key_env: "KIMI_CODE_API_KEY",
        default_model: "kimi-for-coding",
        force_anthropic_version: null,
        supports_count_tokens: false,
        supports_vision: true,
        supports_video: true,
        supports_thinking: true,
        model_map: {},
        visible_models: ["kimi-for-coding", "kimi-for-coding-highspeed"],
        models: {
          "claude-opus-5": {
            upstream_model: "kimi-for-coding",
            thinking_mode: "thinking_only",
          },
        },
      },
      deepseek: {
        display_name: "DeepSeek",
        upstream_url: "https://api.deepseek.com",
        api_key_env: "DEEPSEEK_API_KEY",
        default_model: "deepseek-v4-pro",
        force_anthropic_version: null,
        supports_count_tokens: true,
        supports_vision: false,
        supports_video: false,
        supports_thinking: true,
        model_map: {},
        visible_models: ["deepseek-v4-pro", "deepseek-v4-flash"],
        models: {
          "claude-opus-5": {
            upstream_model: "deepseek-v4-pro",
            thinking_mode: "thinking",
            reasoning_effort: "high",
          },
        },
      },
    },
    server: {
      host: "127.0.0.1",
      port: 4000,
      enable_cors: true,
    },
  };

  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_mcp_config") {
        return {
          provider: "kimi-code",
          model: "kimi-for-coding",
          thinking_mode: "thinking_only",
        };
      }
      if (cmd === "get_antigravity_mcp_status") {
        return {
          status: "configured",
          config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
          config_dir: "C:\\Users\\User\\.gemini\\config",
          registered_command: "C:\\path\\anthro-bridge.exe",
          registered_args: ["--mcp-server"],
          error: null,
        };
      }
      if (cmd === "get_antigravity_commands_status") {
        return {
          skills_dir: "C:\\Users\\User\\.gemini\\config\\skills",
          plan_command: { name: "anthro-plan", slash_command: "/anthro-plan", status: "installed" },
          revise_command: { name: "anthro-revise", slash_command: "/anthro-revise", status: "installed" },
          review_command: { name: "anthro-review", slash_command: "/anthro-review", status: "installed" },
        };
      }
      return {};
    });
  });

  it("renders Kimi Code with thinking_only summary and no dropdowns for thinking mode or reasoning effort", async () => {
    render(<McpSettingPanel config={kimiCodeConfig} refreshConfig={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("Kimi Code")).toBeInTheDocument();
    });

    // Check summary text does NOT say Normal
    expect(screen.getByText("Kimi for Coding (Thinking only)")).toBeInTheDocument();
    expect(screen.queryByText("Kimi for Coding (Normal)")).not.toBeInTheDocument();

    // Expand Kimi Code row
    await act(async () => {
      screen.getByText("Kimi Code").click();
    });

    // Italic "Thinking only" text should be present
    expect(screen.getAllByText("Thinking only").length).toBeGreaterThan(0);

    // Thinking mode select and effort select should NOT be rendered for thinking_only
    const selects = screen.getAllByRole("combobox");
    expect(selects).toHaveLength(1);
    expect(selects[0]).toHaveValue("kimi-for-coding");
  });

  it("regression: renders DeepSeek with toggleable thinking mode and reasoning effort", async () => {
    render(<McpSettingPanel config={kimiCodeConfig} refreshConfig={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("DeepSeek")).toBeInTheDocument();
    });

    // Expand DeepSeek row
    await act(async () => {
      screen.getByText("DeepSeek").click();
    });

    // DeepSeek should have Model, Thinking Mode, and Reasoning Effort dropdowns
    const selects = screen.getAllByRole("combobox");
    expect(selects.length).toBeGreaterThanOrEqual(3);
  });
});

describe("McpSettingPanel - Direct DeepSeek Model Selection & Legacy Compatibility", () => {
  const directDeepSeekConfig: GatewayConfig = {
    active_provider: "deepseek",
    providers: {
      deepseek: {
        display_name: "DeepSeek",
        upstream_url: "https://api.deepseek.com",
        api_key_env: "DEEPSEEK_API_KEY",
        default_model: "deepseek-flash",
        force_anthropic_version: null,
        supports_count_tokens: true,
        supports_vision: false,
        supports_video: false,
        supports_thinking: true,
        model_map: {},
        visible_models: ["claude-opus-5", "claude-sonnet-5"],
        models: {
          "claude-opus-5": {
            upstream_model: "deepseek-flash",
            thinking_mode: "thinking",
            reasoning_effort: "max",
          },
          "claude-sonnet-4-6": {
            upstream_model: "deepseek-v4-pro",
            thinking_mode: "normal",
          },
          "claude-sonnet-4-5": {
            upstream_model: "deepseek-v4-flash",
            thinking_mode: "thinking",
          },
        },
      },
    },
    server: {
      host: "127.0.0.1",
      port: 4000,
      enable_cors: true,
    },
  };

  it("renders only standard Direct DeepSeek models with display names in normal dropdown", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_mcp_config") {
        return {
          provider: "deepseek",
          model: "deepseek-flash",
          thinking_mode: "thinking",
          reasoning_effort: "high",
        };
      }
      if (cmd === "get_antigravity_mcp_status") {
        return {
          status: "configured",
          config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
          config_dir: "C:\\Users\\User\\.gemini\\config",
          registered_command: "C:\\path\\anthro-bridge.exe",
          registered_args: ["--mcp-server"],
          error: null,
        };
      }
      if (cmd === "get_antigravity_commands_status") {
        return {
          skills_dir: "C:\\Users\\User\\.gemini\\config\\skills",
          plan_command: { name: "anthro-plan", slash_command: "/anthro-plan", status: "installed" },
          revise_command: { name: "anthro-revise", slash_command: "/anthro-revise", status: "installed" },
          review_command: { name: "anthro-review", slash_command: "/anthro-review", status: "installed" },
        };
      }
      return {};
    });

    render(<McpSettingPanel config={directDeepSeekConfig} refreshConfig={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("DeepSeek")).toBeInTheDocument();
    });

    // Expand DeepSeek row
    await act(async () => {
      screen.getByText("DeepSeek").click();
    });

    // Find the model select dropdown
    const selects = screen.getAllByRole("combobox");
    const modelSelect = selects[0];
    const options = Array.from(modelSelect.querySelectorAll("option")).map((o) => ({
      value: o.value,
      label: o.textContent,
    }));

    // Must only have standard models with display names
    expect(options).toEqual([
      { value: "deepseek-flash", label: "DeepSeek V4.1 Flash" },
      { value: "deepseek-v4-pro", label: "DeepSeek V4 Pro 0813" },
    ]);

    // Legacy models must NOT be present as normal options
    expect(options.some((o) => o.value === "deepseek-v4-flash")).toBe(false);
    expect(options.some((o) => o.value === "deepseek-v4-flash-vision-exp")).toBe(false);

    // Effort selector for deepseek-flash must have Low, High, Max (no Medium)
    const effortSelect = selects[2];
    const effortOptions = Array.from(effortSelect.querySelectorAll("option")).map((o) => o.textContent);
    expect(effortOptions).toEqual(["Low", "High", "Max"]);
    expect(effortOptions).not.toContain("Medium");
  });

  it("preserves saved legacy model as a selected-only option without breaking", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_mcp_config") {
        return {
          provider: "deepseek",
          model: "deepseek-v4-flash",
          thinking_mode: "thinking",
          reasoning_effort: "high",
        };
      }
      if (cmd === "get_antigravity_mcp_status") {
        return {
          status: "configured",
          config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
          config_dir: "C:\\Users\\User\\.gemini\\config",
          registered_command: "C:\\path\\anthro-bridge.exe",
          registered_args: ["--mcp-server"],
          error: null,
        };
      }
      if (cmd === "get_antigravity_commands_status") {
        return {
          skills_dir: "C:\\Users\\User\\.gemini\\config\\skills",
          plan_command: { name: "anthro-plan", slash_command: "/anthro-plan", status: "installed" },
          revise_command: { name: "anthro-revise", slash_command: "/anthro-revise", status: "installed" },
          review_command: { name: "anthro-review", slash_command: "/anthro-review", status: "installed" },
        };
      }
      return {};
    });

    render(<McpSettingPanel config={directDeepSeekConfig} refreshConfig={vi.fn()} />);

    // Header summary reflects loaded legacy model from get_mcp_config
    await waitFor(() => {
      expect(screen.getByText(/deepseek-v4-flash/i)).toBeInTheDocument();
    });

    // Expand DeepSeek row
    await act(async () => {
      screen.getByText("DeepSeek").click();
    });

    const selects = screen.getAllByRole("combobox");
    const modelSelect = selects[0];
    const options = Array.from(modelSelect.querySelectorAll("option")).map((o) => ({
      value: o.value,
      label: o.textContent,
    }));

    // Must contain the legacy saved model marked as legacy
    expect(options).toContainEqual({
      value: "deepseek-v4-flash",
      label: "deepseek-v4-flash (Legacy / saved)",
    });
    expect(modelSelect).toHaveValue("deepseek-v4-flash");
  });

  // ── Independent fixture for OpenAI reasoning models (§13 & §26) ────
  // Declared explicitly without deriving from MODEL_CAPABILITIES to ensure
  // independent verification of UI rendered options, defaults, and persistence.
  interface ExpectedReasoningModelFixture {
    readonly id: string;
    readonly expectedOptions: readonly string[];
    readonly defaultEffort: string;
    readonly supportsNone: boolean;
  }

  const INDEPENDENT_OPENAI_REASONING_FIXTURE: readonly ExpectedReasoningModelFixture[] = [
    // Group 1: 5 choices, no "none"
    {
      id: "openai/gpt-6.1-sol",
      expectedOptions: ["low", "medium", "high", "xhigh", "max"],
      defaultEffort: "medium",
      supportsNone: false,
    },
    {
      id: "openai/gpt-6.1-sol-pro",
      expectedOptions: ["low", "medium", "high", "xhigh", "max"],
      defaultEffort: "medium",
      supportsNone: false,
    },
    {
      id: "openai/gpt-6-astra",
      expectedOptions: ["low", "medium", "high", "xhigh", "max"],
      defaultEffort: "medium",
      supportsNone: false,
    },
    {
      id: "openai/gpt-6-astra-pro",
      expectedOptions: ["low", "medium", "high", "xhigh", "max"],
      defaultEffort: "medium",
      supportsNone: false,
    },
    {
      id: "openai/gpt-astra-latest",
      expectedOptions: ["low", "medium", "high", "xhigh", "max"],
      defaultEffort: "medium",
      supportsNone: false,
    },
    // Group 2: 6 choices, includes "none"
    {
      id: "openai/gpt-6-sol",
      expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
      defaultEffort: "medium",
      supportsNone: true,
    },
    {
      id: "openai/gpt-6-sol-pro",
      expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
      defaultEffort: "medium",
      supportsNone: true,
    },
    {
      id: "openai/gpt-6-luna",
      expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
      defaultEffort: "medium",
      supportsNone: true,
    },
    {
      id: "openai/gpt-6-luna-pro",
      expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
      defaultEffort: "medium",
      supportsNone: true,
    },
    {
      id: "openai/gpt-5.6-sol",
      expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
      defaultEffort: "medium",
      supportsNone: true,
    },
    {
      id: "openai/gpt-5.6-sol-pro",
      expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
      defaultEffort: "medium",
      supportsNone: true,
    },
    {
      id: "openai/gpt-5.6-terra",
      expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
      defaultEffort: "medium",
      supportsNone: true,
    },
    {
      id: "openai/gpt-5.6-terra-pro",
      expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
      defaultEffort: "medium",
      supportsNone: true,
    },
    {
      id: "openai/gpt-5.6-luna",
      expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
      defaultEffort: "medium",
      supportsNone: true,
    },
    {
      id: "openai/gpt-5.6-luna-pro",
      expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
      defaultEffort: "medium",
      supportsNone: true,
    },
  ] as const;

  describe("OpenRouter OpenAI Model Dropdown & Reasoning", () => {
    const openRouterConfig: GatewayConfig = {
      active_provider: "openrouter",
      active_openrouter_profile_id: "chatgpt",
      providers: {
        openrouter: {
          ...dummyConfig.providers.deepseek,
          display_name: "OpenRouter",
          profiles: [
            {
              id: "chatgpt",
              display_name: "OpenRouter: chatGPT",
              model_map: {},
              visible_models: [],
              models: {
                "claude-opus-5": {
                  upstream_model: "openai/gpt-5.6-sol",
                  thinking_mode: "thinking",
                  reasoning_effort: "high",
                },
                "claude-sonnet-5": {
                  upstream_model: "openai/gpt-5.6-terra",
                  thinking_mode: "thinking",
                  reasoning_effort: "high",
                },
                "claude-haiku-4-5": {
                  upstream_model: "openai/gpt-5.6-luna",
                  thinking_mode: "thinking",
                  reasoning_effort: "high",
                },
              },
            },
          ],
        },
      },
      server: {
        host: "127.0.0.1",
        port: 4000,
        enable_cors: true,
      },
    };

    it("lists all 15 OpenAI models with display names and no batch variants in OpenRouter chatGPT profile", async () => {
      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_mcp_config") {
          return {
            provider: "openrouter",
            profile_id: "chatgpt",
            model: "openai/gpt-5.6-sol",
            thinking_mode: "thinking",
            reasoning_effort: "high",
          };
        }
        if (cmd === "get_antigravity_mcp_status") {
          return { status: "configured", registered_command: "cmd", registered_args: [] };
        }
        if (cmd === "get_antigravity_commands_status") {
          return {
            skills_dir: "C:\\skills",
            plan_command: { name: "anthro-plan", status: "installed" },
            revise_command: { name: "anthro-revise", status: "installed" },
            review_command: { name: "anthro-review", status: "installed" },
          };
        }
        return {};
      });

      render(<McpSettingPanel config={openRouterConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText("OpenRouter")).toBeInTheDocument();
      });

      // Expand OpenRouter row
      await act(async () => {
        screen.getByText("OpenRouter").click();
      });

      const selects = screen.getAllByRole("combobox");
      // selects: [0: profileSelect, 1: modelSelect, 2: thinkingSelect, 3: effortSelect]
      const modelSelect = selects[1];
      const options = Array.from(modelSelect.querySelectorAll("option")).map((o) => ({
        value: o.value,
        label: o.textContent,
      }));

      expect(options).toEqual([
        { value: "openai/gpt-6.1-sol", label: "GPT-6.1 Sol" },
        { value: "openai/gpt-6.1-sol-pro", label: "GPT-6.1 Sol Pro" },
        { value: "openai/gpt-6-sol", label: "GPT-6 Sol" },
        { value: "openai/gpt-6-sol-pro", label: "GPT-6 Sol Pro" },
        { value: "openai/gpt-6-luna", label: "GPT-6 Luna" },
        { value: "openai/gpt-6-luna-pro", label: "GPT-6 Luna Pro" },
        { value: "openai/gpt-6-astra", label: "GPT-6 Astra" },
        { value: "openai/gpt-6-astra-pro", label: "GPT-6 Astra Pro" },
        { value: "openai/gpt-astra-latest", label: "GPT Astra Latest" },
        { value: "openai/gpt-5.6-sol", label: "GPT-5.6 Sol" },
        { value: "openai/gpt-5.6-sol-pro", label: "GPT-5.6 Sol Pro" },
        { value: "openai/gpt-5.6-terra", label: "GPT-5.6 Terra" },
        { value: "openai/gpt-5.6-terra-pro", label: "GPT-5.6 Terra Pro" },
        { value: "openai/gpt-5.6-luna", label: "GPT-5.6 Luna" },
        { value: "openai/gpt-5.6-luna-pro", label: "GPT-5.6 Luna Pro" },
      ]);

      expect(options.some((o) => o.value.toLowerCase().includes("batch"))).toBe(false);
      expect(modelSelect).toHaveValue("openai/gpt-5.6-sol");
    });

    it("keeps dashboard-hidden OpenRouter profiles selectable in MCP settings", async () => {
      const configWithHiddenProfile: GatewayConfig = {
        ...openRouterConfig,
        providers: {
          ...openRouterConfig.providers,
          openrouter: {
            ...openRouterConfig.providers.openrouter,
            profiles: [
              ...(openRouterConfig.providers.openrouter.profiles ?? []),
              {
                id: "hy3",
                display_name: "OpenRouter: Hy3",
                hidden: true,
                model_map: {},
                visible_models: [],
                models: {
                  "claude-opus-5": {
                    upstream_model: "tencent/hy3",
                    thinking_mode: "thinking",
                    reasoning_effort: "high",
                  },
                  "claude-sonnet-5": {
                    upstream_model: "tencent/hy3",
                    thinking_mode: "thinking",
                    reasoning_effort: "low",
                  },
                  "claude-haiku-4-5": {
                    upstream_model: "tencent/hy3",
                    thinking_mode: "normal",
                  },
                },
              },
            ],
          },
        },
      };

      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_mcp_config") {
          return {
            provider: "openrouter",
            profile_id: "chatgpt",
            model: "openai/gpt-5.6-sol",
            thinking_mode: "thinking",
            reasoning_effort: "high",
          };
        }
        if (cmd === "get_antigravity_mcp_status") {
          return { status: "configured", registered_command: "cmd", registered_args: [] };
        }
        if (cmd === "get_antigravity_commands_status") {
          return {
            skills_dir: "C:\\skills",
            plan_command: { name: "anthro-plan", status: "installed" },
            revise_command: { name: "anthro-revise", status: "installed" },
            review_command: { name: "anthro-review", status: "installed" },
          };
        }
        return {};
      });

      render(<McpSettingPanel config={configWithHiddenProfile} refreshConfig={vi.fn()} />);
      await waitFor(() => expect(screen.getByText("OpenRouter")).toBeInTheDocument());
      await act(async () => { screen.getByText("OpenRouter").click(); });

      const selects = screen.getAllByRole("combobox");
      const profileSelect = selects[0];
      const modelSelect = selects[1];
      expect(Array.from(profileSelect.querySelectorAll("option")).map((option) => option.value))
        .toContain("hy3");

      await userEvent.selectOptions(profileSelect, "hy3");
      expect(modelSelect).toHaveValue("tencent/hy3");
      expect(Array.from(modelSelect.querySelectorAll("option")).map((option) => option.value))
        .toContain("tencent/hy4-preview");
    });

    it("renders 5 reasoning effort choices for GPT-6 Astra and Astra Pro (§13)", async () => {
      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_mcp_config") {
          return {
            provider: "openrouter",
            profile_id: "chatgpt",
            model: "openai/gpt-6-astra",
            thinking_mode: "thinking",
            reasoning_effort: "high",
          };
        }
        if (cmd === "get_antigravity_mcp_status") {
          return { status: "configured", registered_command: "cmd", registered_args: [] };
        }
        if (cmd === "get_antigravity_commands_status") {
          return {
            skills_dir: "C:\\skills",
            plan_command: { name: "anthro-plan", status: "installed" },
            revise_command: { name: "anthro-revise", status: "installed" },
            review_command: { name: "anthro-review", status: "installed" },
          };
        }
        return {};
      });

      const { unmount } = render(<McpSettingPanel config={openRouterConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText("OpenRouter")).toBeInTheDocument();
      });

      await act(async () => {
        screen.getByText("OpenRouter").click();
      });

      // Astra: effort select is available with 5 options (no normal mode toggle)
      const selects = screen.getAllByRole("combobox");
      const effortSelect = selects[2];
      expect(effortSelect).toHaveValue("high");
      const effortOptions = Array.from(effortSelect.querySelectorAll("option")).map((o) => o.value);
      expect(effortOptions).toEqual(["low", "medium", "high", "xhigh", "max"]);

      unmount();

      // Astra Pro: thinking_only + effort select with 5 options per §13
      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_mcp_config") {
          return {
            provider: "openrouter",
            profile_id: "chatgpt",
            model: "openai/gpt-6-astra-pro",
            thinking_mode: "thinking_only",
            reasoning_effort: "",
          };
        }
        if (cmd === "get_antigravity_mcp_status") {
          return { status: "configured", registered_command: "cmd", registered_args: [] };
        }
        if (cmd === "get_antigravity_commands_status") {
          return {
            skills_dir: "C:\\skills",
            plan_command: { name: "anthro-plan", status: "installed" },
            revise_command: { name: "anthro-revise", status: "installed" },
            review_command: { name: "anthro-review", status: "installed" },
          };
        }
        return {};
      });

      render(<McpSettingPanel config={openRouterConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText("OpenRouter")).toBeInTheDocument();
      });

      await act(async () => {
        screen.getByText("OpenRouter").click();
      });

      expect(screen.getByText("Thinking only")).toBeInTheDocument();
      const astraProSelects = screen.getAllByRole("combobox");
      const astraProEffortSelect = astraProSelects[2];
      expect(astraProEffortSelect).toHaveValue("medium");
      const astraProEffortOptions = Array.from(astraProEffortSelect.querySelectorAll("option")).map((o) => o.value);
      expect(astraProEffortOptions).toEqual(["low", "medium", "high", "xhigh", "max"]);
    });

    it("verifies all 15 OpenAI models have identical options and defaults matching catalog in MCP settings (§13)", () => {
      const all15Ids = [
        "openai/gpt-6.1-sol",
        "openai/gpt-6.1-sol-pro",
        "openai/gpt-6-sol",
        "openai/gpt-6-sol-pro",
        "openai/gpt-6-luna",
        "openai/gpt-6-luna-pro",
        "openai/gpt-6-astra",
        "openai/gpt-6-astra-pro",
        "openai/gpt-astra-latest",
        "openai/gpt-5.6-sol",
        "openai/gpt-5.6-sol-pro",
        "openai/gpt-5.6-terra",
        "openai/gpt-5.6-terra-pro",
        "openai/gpt-5.6-luna",
        "openai/gpt-5.6-luna-pro",
      ];
      expect(all15Ids).toHaveLength(15);

      for (const id of all15Ids) {
        expect(isGpt61SolModel(id)).toBe(true);
        const caps = MODEL_CAPABILITIES[id];
        expect(caps).toBeDefined();
        expect(caps?.reasoning).toBeDefined();
        expect(caps?.reasoning?.defaultEffort).toBe("medium");
        expect(caps?.reasoningEffortOptions).toEqual(caps?.reasoning?.supportedEfforts);

        // Group 1: 5 choices, no none
        if (
          id.includes("gpt-6.1") ||
          id.includes("astra")
        ) {
          expect(caps?.reasoningEffortOptions).toEqual(["low", "medium", "high", "xhigh", "max"]);
          expect(caps?.reasoningEffortOptions).not.toContain("none");
        } else {
          // Group 2: 6 choices, includes none
          expect(caps?.reasoningEffortOptions).toEqual(["none", "low", "medium", "high", "xhigh", "max"]);
          expect(caps?.reasoningEffortOptions).toContain("none");
        }
      }
    });

    it("renders 5 reasoning effort choices and thinking only (no normal toggle) for GPT-6.1 Sol and Sol Pro, defaulting to medium", async () => {
      let updatedConfig: any = null;
      invokeMock.mockImplementation(async (cmd: string, args?: any) => {
        if (cmd === "get_mcp_config") {
          return {
            provider: "openrouter",
            profile_id: "chatgpt",
            model: "openai/gpt-6.1-sol",
            thinking_mode: "normal",
            reasoning_effort: "",
          };
        }
        if (cmd === "update_mcp_config") {
          updatedConfig = args?.mcp;
          return { success: true };
        }
        if (cmd === "get_antigravity_mcp_status") {
          return { status: "configured", registered_command: "cmd", registered_args: [] };
        }
        if (cmd === "get_antigravity_commands_status") {
          return {
            skills_dir: "C:\\skills",
            plan_command: { name: "anthro-plan", status: "installed" },
            revise_command: { name: "anthro-revise", status: "installed" },
            review_command: { name: "anthro-review", status: "installed" },
          };
        }
        return {};
      });

      const { unmount } = render(<McpSettingPanel config={openRouterConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText("OpenRouter")).toBeInTheDocument();
      });

      await act(async () => {
        screen.getByText("OpenRouter").click();
      });

      // Shows Thinking only (always on) label instead of normal/thinking combobox
      expect(screen.getByText("Thinking only")).toBeInTheDocument();

      // Comboboxes: profile select, model select, reasoning effort select (no thinking mode toggle)
      const selects = screen.getAllByRole("combobox");
      expect(selects).toHaveLength(3);

      const effortSelect = selects[2];
      expect(effortSelect).toHaveValue("medium");

      const expectedOptions = MODEL_CAPABILITIES["openai/gpt-6.1-sol"]?.reasoningEffortOptions ?? [];
      expect(expectedOptions).toContain("medium");

      const effortOptions = Array.from(effortSelect.querySelectorAll("option")).map((o) => o.value);
      expect(effortOptions).toEqual(expectedOptions);
      expect(effortOptions).not.toContain("normal");
      expect(effortOptions).not.toContain("off");
      expect(effortOptions).not.toContain("none");
      expect(effortOptions).not.toContain("minimal");

      // Changing effort saves the updated value
      await act(async () => {
        await userEvent.selectOptions(effortSelect, "xhigh");
      });

      expect(updatedConfig).toMatchObject({
        provider: "openrouter",
        model: "openai/gpt-6.1-sol",
        reasoning_effort: "xhigh",
      });

      unmount();

      // Test Pro model variant
      invokeMock.mockImplementation(async (cmd: string, args?: any) => {
        if (cmd === "get_mcp_config") {
          return {
            provider: "openrouter",
            profile_id: "chatgpt",
            model: "openai/gpt-6.1-sol-pro",
            thinking_mode: "thinking",
            reasoning_effort: "high",
          };
        }
        if (cmd === "update_mcp_config") {
          updatedConfig = args?.mcp;
          return { success: true };
        }
        if (cmd === "get_antigravity_mcp_status") {
          return { status: "configured", registered_command: "cmd", registered_args: [] };
        }
        if (cmd === "get_antigravity_commands_status") {
          return {
            skills_dir: "C:\\skills",
            plan_command: { name: "anthro-plan", status: "installed" },
            revise_command: { name: "anthro-revise", status: "installed" },
            review_command: { name: "anthro-review", status: "installed" },
          };
        }
        return {};
      });

      render(<McpSettingPanel config={openRouterConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText("OpenRouter")).toBeInTheDocument();
      });

      await act(async () => {
        screen.getByText("OpenRouter").click();
      });

      const proSelects = screen.getAllByRole("combobox");
      expect(proSelects).toHaveLength(3);
      const proEffortSelect = proSelects[2];
      expect(proEffortSelect).toHaveValue("high");

      const proExpectedOptions = MODEL_CAPABILITIES["openai/gpt-6.1-sol-pro"]?.reasoningEffortOptions ?? [];
      expect(proExpectedOptions).toContain("medium");
      const proEffortOptions = Array.from(proEffortSelect.querySelectorAll("option")).map((o) => o.value);
      expect(proEffortOptions).toEqual(proExpectedOptions);
      expect(proEffortOptions).not.toContain("normal");
      expect(proEffortOptions).not.toContain("off");
      expect(proEffortOptions).not.toContain("none");
      expect(proEffortOptions).not.toContain("minimal");

      await act(async () => {
        await userEvent.selectOptions(proEffortSelect, "max");
      });

      expect(updatedConfig).toMatchObject({
        provider: "openrouter",
        model: "openai/gpt-6.1-sol-pro",
        reasoning_effort: "max",
      });
    });

    describe("Table-driven actual UI rendering, options, defaults, persistence, and legacy boundary across all 15 models (§26)", () => {
      it.each(INDEPENDENT_OPENAI_REASONING_FIXTURE)(
        "renders actual effort selector with independent options, medium default, and persists valid effort for $id in MCP settings (§26)",
        async (fixture) => {
          let updatedConfig: any = null;
          invokeMock.mockReset();
          invokeMock.mockImplementation(async (cmd: string, args?: any) => {
            if (cmd === "get_mcp_config") {
              return {
                provider: "openrouter",
                profile_id: "chatgpt",
                model: fixture.id,
                thinking_mode: "thinking",
                reasoning_effort: "",
              };
            }
            if (cmd === "update_mcp_config") {
              updatedConfig = args?.mcp;
              return { success: true };
            }
            if (cmd === "get_antigravity_mcp_status") {
              return { status: "configured", registered_command: "cmd", registered_args: [] };
            }
            if (cmd === "get_antigravity_commands_status") {
              return {
                skills_dir: "C:\\skills",
                plan_command: { name: "anthro-plan", status: "installed" },
                revise_command: { name: "anthro-revise", status: "installed" },
                review_command: { name: "anthro-review", status: "installed" },
              };
            }
            return {};
          });

          const testConfig: GatewayConfig = {
            ...openRouterConfig,
            providers: {
              ...openRouterConfig.providers,
              openrouter: {
                ...openRouterConfig.providers.openrouter,
                profiles: (openRouterConfig.providers.openrouter.profiles ?? []).map((p) => ({
                  ...p,
                  models: {},
                })),
              },
            },
          };

          const { unmount } = render(<McpSettingPanel config={testConfig} refreshConfig={vi.fn()} />);

          await waitFor(() => {
            expect(screen.getByText("OpenRouter")).toBeInTheDocument();
          });

          await act(async () => {
            screen.getByText("OpenRouter").click();
          });

          // 1. The selected model is that exact ID and its effort selector is actually present.
          const selects = screen.getAllByRole("combobox");
          expect(selects).toHaveLength(3);
          const modelSelect = selects[1] as HTMLSelectElement;
          expect(modelSelect.value).toBe(fixture.id);

          const effortSelect = selects[2] as HTMLSelectElement;
          expect(effortSelect).toBeInTheDocument();

          // 2. The rendered option values exactly equal the independent expected matrix in §13.2,
          // including none only for models whose matrix includes it; no normal, off, or minimal option leaks.
          const optionValues = Array.from(effortSelect.querySelectorAll("option")).map((o) => o.value);
          expect(optionValues).toEqual(fixture.expectedOptions);
          expect(optionValues).not.toContain("normal");
          expect(optionValues).not.toContain("off");
          expect(optionValues).not.toContain("minimal");
          if (fixture.supportsNone) {
            expect(optionValues).toContain("none");
          } else {
            expect(optionValues).not.toContain("none");
          }

          // 3. With no usable persisted effort, the effective selected default is medium.
          expect(effortSelect).toHaveValue(fixture.defaultEffort);

          // 4. Changing to a valid effort persists the chosen value while retaining the exact model ID.
          const targetEffort = "high";
          await act(async () => {
            await userEvent.selectOptions(effortSelect, targetEffort);
          });

          expect(updatedConfig).toMatchObject({
            provider: "openrouter",
            model: fixture.id,
            reasoning_effort: targetEffort,
          });

          unmount();
        },
      );

      it.each(INDEPENDENT_OPENAI_REASONING_FIXTURE)(
        "resolves legacy normal/off boundary to none (when supported) or medium (otherwise) without persisting on render for $id (§26)",
        async (fixture) => {
          for (const legacyEffort of ["normal", "off"]) {
            invokeMock.mockReset();
            invokeMock.mockImplementation(async (cmd: string) => {
              if (cmd === "get_mcp_config") {
                return {
                  provider: "openrouter",
                  profile_id: "chatgpt",
                  model: fixture.id,
                  thinking_mode: "normal",
                  reasoning_effort: legacyEffort,
                };
              }
              if (cmd === "get_antigravity_mcp_status") {
                return { status: "configured", registered_command: "cmd", registered_args: [] };
              }
              if (cmd === "get_antigravity_commands_status") {
                return {
                  skills_dir: "C:\\skills",
                  plan_command: { name: "anthro-plan", status: "installed" },
                  revise_command: { name: "anthro-revise", status: "installed" },
                  review_command: { name: "anthro-review", status: "installed" },
                };
              }
              return {};
            });

            const testConfig: GatewayConfig = {
              ...openRouterConfig,
              providers: {
                ...openRouterConfig.providers,
                openrouter: {
                  ...openRouterConfig.providers.openrouter,
                  profiles: (openRouterConfig.providers.openrouter.profiles ?? []).map((p) => ({
                    ...p,
                    models: {},
                  })),
                },
              },
            };

            const { unmount } = render(<McpSettingPanel config={testConfig} refreshConfig={vi.fn()} />);

            await waitFor(() => {
              expect(screen.getByText("OpenRouter")).toBeInTheDocument();
            });

            await act(async () => {
              screen.getByText("OpenRouter").click();
            });

            const selects = screen.getAllByRole("combobox");
            expect(selects).toHaveLength(3);
            const effortSelect = selects[2] as HTMLSelectElement;

            // Legacy normal/off resolves to none for none-supporting models, and to medium for models that do not support none
            const expectedBoundaryEffort = fixture.supportsNone ? "none" : "medium";
            expect(effortSelect).toHaveValue(expectedBoundaryEffort);

            // Rendering alone must not persist a correction
            expect(invokeMock).not.toHaveBeenCalledWith("update_mcp_config", expect.anything());

            unmount();
          }
        },
      );
    });
  });

  describe("Direct MiMo Model Dropdown & Reasoning", () => {
    const mimoConfig: GatewayConfig = {
      active_provider: "mimo",
      providers: {
        mimo: {
          display_name: "MiMo",
          upstream_url: "https://api.xiaomimimo.com/anthropic",
          api_key_env: "XIAOMI_API_KEY",
          default_model: "mimo-v2.6-flash",
          force_anthropic_version: null,
          supports_count_tokens: false,
          supports_vision: true,
          supports_video: true,
          supports_thinking: true,
          model_map: {},
          visible_models: ["claude-opus-5", "claude-sonnet-5", "claude-haiku-4-5"],
          models: {
            "claude-opus-5": {
              upstream_model: "mimo-v2.6-pro",
              thinking_mode: "thinking",
              visible: true,
            },
            "claude-sonnet-5": {
              upstream_model: "mimo-v2.6-flash",
              thinking_mode: "thinking",
              visible: true,
            },
            "claude-haiku-4-5": {
              upstream_model: "mimo-v2.6-flash",
              thinking_mode: "normal",
              visible: true,
            },
          },
        },
      },
      server: {
        host: "127.0.0.1",
        port: 4000,
        enable_cors: true,
      },
    };

    it("lists MiMo V2.6 models with friendly display names and shows only Normal/Thinking toggle without effort select", async () => {
      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_mcp_config") {
          return {
            provider: "mimo",
            model: "mimo-v2.6-pro",
            thinking_mode: "thinking",
            reasoning_effort: "",
          };
        }
        if (cmd === "get_antigravity_mcp_status") {
          return { status: "configured", registered_command: "cmd", registered_args: [] };
        }
        if (cmd === "get_antigravity_commands_status") {
          return {
            skills_dir: "C:\\skills",
            plan_command: { name: "anthro-plan", status: "installed" },
            revise_command: { name: "anthro-revise", status: "installed" },
            review_command: { name: "anthro-review", status: "installed" },
          };
        }
        return {};
      });

      render(<McpSettingPanel config={mimoConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText("MiMo")).toBeInTheDocument();
      });

      await act(async () => {
        screen.getByText("MiMo").click();
      });

      const selects = screen.getAllByRole("combobox");
      // Model select and thinking mode select (no effort select)
      expect(selects).toHaveLength(2);

      const modelSelect = selects[0];
      const options = Array.from(modelSelect.querySelectorAll("option")).map((o) => ({
        value: o.value,
        label: o.textContent,
      }));

      expect(options).toEqual([
        { value: "mimo-v2.6-flash", label: "MiMo-V2.6-Flash" },
        { value: "mimo-v2.6-pro", label: "MiMo-V2.6-Pro" },
        { value: "mimo-v2.6-pro-ultraspeed", label: "MiMo-V2.6-Pro-UltraSpeed" },
      ]);
    });

    it("renders GLM row with 4 models and saves thinking mode and reasoning effort", async () => {
      const zaiConfig: GatewayConfig = {
        ...dummyConfig,
        active_provider: "zai",
        providers: {
          ...dummyConfig.providers,
          zai: {
            display_name: "GLM",
            upstream_url: "https://api.z.ai/api/anthropic",
            api_key_env: "ZAI_API_KEY",
            default_model: "glm-5.3",
            force_anthropic_version: null,
            supports_count_tokens: false,
            supports_vision: true,
            supports_video: false,
            supports_thinking: true,
            model_map: {},
            visible_models: ["glm-5.3", "glm-5.3-flash", "glm-5.3-flashx", "glm-5.2"],
          },
        },
      };

      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_mcp_config") {
          return {
            provider: "zai",
            model: "glm-5.3",
            thinking_mode: "thinking",
            reasoning_effort: "max",
          };
        }
        if (cmd === "update_mcp_config") {
          return {};
        }
        if (cmd === "get_antigravity_mcp_status") {
          return { status: "configured", registered_command: "cmd", registered_args: [] };
        }
        if (cmd === "get_antigravity_commands_status") {
          return {
            skills_dir: "C:\\skills",
            plan_command: { name: "anthro-plan", status: "installed" },
            revise_command: { name: "anthro-revise", status: "installed" },
            review_command: { name: "anthro-review", status: "installed" },
          };
        }
        return {};
      });

      render(<McpSettingPanel config={zaiConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText("GLM")).toBeInTheDocument();
      });

      await act(async () => {
        screen.getByText("GLM").click();
      });

      const selects = screen.getAllByRole("combobox");
      // Model select and reasoning effort select (thinking is fixed thinkingOnly)
      expect(selects.length).toBeGreaterThanOrEqual(2);

      const modelSelect = selects[0];
      const options = Array.from(modelSelect.querySelectorAll("option")).map((o) => ({
        value: o.value,
        label: o.textContent,
      }));

      expect(options).toEqual([
        { value: "glm-5.3", label: "GLM-5.3" },
        { value: "glm-5.3-flash", label: "GLM-5.3 Flash" },
        { value: "glm-5.3-flashx", label: "GLM-5.3 FlashX" },
        { value: "glm-5.2", label: "GLM-5.2" },
      ]);
    });
  });

  describe("Antigravity MCP executable target recovery (v0.24.4-r2b)", () => {
    const currentPoverelloExe = "C:\\Users\\User\\AppData\\Local\\Poverello\\anthro-bridge.exe";
    const staleLegacyExe = "C:\\Users\\User\\AppData\\Local\\Anthro Bridge\\anthro-bridge.exe";
    const canonicalExe = "C:\\Program Files\\Poverello\\poverello.exe";
    const customUserExe = "C:\\Custom\\Poverello\\poverello.exe";

    it("with registered_command absent, stale legacy_command present, and valid current executable returned, uses current executable not legacy path", async () => {
      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_commands_status") {
          return { skills_dir: "", plan_command: {}, revise_command: {}, review_command: {} };
        }
        if (cmd === "get_antigravity_mcp_status") {
          return {
            status: "not_configured",
            config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
            config_dir: "C:\\Users\\User\\.gemini\\config",
            registered_command: null,
            registered_args: null,
            legacy_status: "configured",
            legacy_command: staleLegacyExe,
            error: null,
          } as AntigravityMcpInfo;
        }
        if (cmd === "get_current_executable_path") {
          return currentPoverelloExe;
        }
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText(/antigravity\.header/i)).toBeInTheDocument();
      });

      // Target executable input should contain currentPoverelloExe, not staleLegacyExe
      const targetInput = await screen.findByDisplayValue(currentPoverelloExe);
      expect(targetInput).toBeInTheDocument();
      expect(screen.queryByDisplayValue(staleLegacyExe)).not.toBeInTheDocument();

      // Legacy preservation notice remains visible
      expect(screen.getByText(/antigravity\.mcpLegacyPreserved/i)).toBeInTheDocument();

      // Update button is enabled because target is populated and differs from registered_command (which is null)
      const updateBtn = screen.getByRole("button", { name: /antigravity\.btnUpdate/i });
      expect(updateBtn).not.toBeDisabled();
    });

    it("with canonical registered_command present, keeps canonical path without overriding from current executable", async () => {
      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_commands_status") {
          return { skills_dir: "", plan_command: {}, revise_command: {}, review_command: {} };
        }
        if (cmd === "get_antigravity_mcp_status") {
          return {
            status: "configured",
            config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
            config_dir: "C:\\Users\\User\\.gemini\\config",
            registered_command: canonicalExe,
            registered_args: ["--mcp-server"],
            legacy_status: "configured",
            legacy_command: staleLegacyExe,
            error: null,
          } as AntigravityMcpInfo;
        }
        if (cmd === "get_current_executable_path") {
          return currentPoverelloExe;
        }
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getAllByText(/antigravity\.statusConfigured/i).length).toBeGreaterThan(0);
      });

      // Canonical command remains selected; current executable does not override it
      expect(screen.getByDisplayValue(canonicalExe)).toBeInTheDocument();
      expect(screen.queryByDisplayValue(currentPoverelloExe)).not.toBeInTheDocument();
      expect(screen.queryByDisplayValue(staleLegacyExe)).not.toBeInTheDocument();

      // Update button is disabled because selected matches registered
      const updateBtn = screen.getByRole("button", { name: /antigravity\.btnUpdate/i });
      expect(updateBtn).toBeDisabled();
    });

    it("if current-executable discovery fails and only legacy path exists, target remains empty, update is disabled, and error is visible", async () => {
      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_commands_status") {
          return { skills_dir: "", plan_command: {}, revise_command: {}, review_command: {} };
        }
        if (cmd === "get_antigravity_mcp_status") {
          return {
            status: "not_configured",
            config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
            config_dir: "C:\\Users\\User\\.gemini\\config",
            registered_command: null,
            registered_args: null,
            legacy_status: "configured",
            legacy_command: staleLegacyExe,
            error: null,
          } as AntigravityMcpInfo;
        }
        if (cmd === "get_current_executable_path") {
          throw new Error("Failed to determine current executable path: access denied");
        }
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText(/Failed to determine current executable path/i)).toBeInTheDocument();
      });

      // Stale legacy path is NEVER used
      expect(screen.queryByDisplayValue(staleLegacyExe)).not.toBeInTheDocument();

      // Update button is disabled
      const updateBtn = screen.getByRole("button", { name: /antigravity\.btnUpdate/i });
      expect(updateBtn).toBeDisabled();
    });

    it("selecting an executable via 変更 and updating passes exactly the selected path to configure_antigravity_mcp", async () => {
      let configureArgs: any = null;
      invokeMock.mockImplementation(async (cmd: string, args?: any) => {
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_commands_status") {
          return { skills_dir: "", plan_command: {}, revise_command: {}, review_command: {} };
        }
        if (cmd === "get_antigravity_mcp_status") {
          return {
            status: "not_configured",
            config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
            config_dir: "C:\\Users\\User\\.gemini\\config",
            registered_command: null,
            registered_args: null,
            error: null,
          } as AntigravityMcpInfo;
        }
        if (cmd === "get_current_executable_path") {
          return currentPoverelloExe;
        }
        if (cmd === "select_executable_dialog") {
          return customUserExe;
        }
        if (cmd === "configure_antigravity_mcp") {
          configureArgs = args;
          return {
            status: "configured",
            config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
            config_dir: "C:\\Users\\User\\.gemini\\config",
            registered_command: customUserExe,
            registered_args: ["--mcp-server"],
            error: null,
          } as AntigravityMcpInfo;
        }
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByDisplayValue(currentPoverelloExe)).toBeInTheDocument();
      });

      // Click Change button
      const changeBtn = screen.getByRole("button", { name: /antigravity\.btnChangeExe/i });
      await act(async () => {
        changeBtn.click();
      });

      // Now customUserExe is displayed
      expect(screen.getByDisplayValue(customUserExe)).toBeInTheDocument();

      // Click Update button
      const updateBtn = screen.getByRole("button", { name: /antigravity\.btnUpdate/i });
      await act(async () => {
        updateBtn.click();
      });

      expect(configureArgs).toEqual({ exePath: customUserExe });
    });

    it("successful update refreshes status as canonical configured, clears operation error, and causes outdated warning to disappear", async () => {
      let agStatusState = "not_configured";
      let configuredCmd: string | null = null;

      invokeMock.mockImplementation(async (cmd: string, args?: any) => {
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_commands_status") {
          return { skills_dir: "", plan_command: {}, revise_command: {}, review_command: {} };
        }
        if (cmd === "get_antigravity_mcp_status") {
          return {
            status: agStatusState as any,
            config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
            config_dir: "C:\\Users\\User\\.gemini\\config",
            registered_command: configuredCmd,
            registered_args: configuredCmd ? ["--mcp-server"] : null,
            legacy_status: "configured",
            legacy_command: staleLegacyExe,
            error: null,
          } as AntigravityMcpInfo;
        }
        if (cmd === "get_current_executable_path") {
          return currentPoverelloExe;
        }
        if (cmd === "configure_antigravity_mcp") {
          agStatusState = "configured";
          configuredCmd = args.exePath;
          return {
            status: "configured",
            config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
            config_dir: "C:\\Users\\User\\.gemini\\config",
            registered_command: args.exePath,
            registered_args: ["--mcp-server"],
            legacy_status: "configured",
            legacy_command: staleLegacyExe,
            error: null,
          } as AntigravityMcpInfo;
        }
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByDisplayValue(currentPoverelloExe)).toBeInTheDocument();
      });

      // Initially, outdated warning is visible because selectedExePath !== registered_command (null)
      expect(screen.getAllByText(/antigravity\.statusOutdated/i).length).toBeGreaterThan(0);
      expect(screen.getByText(/antigravity\.commandsMcpWarning/i)).toBeInTheDocument();

      // Click Update button
      const updateBtn = screen.getByRole("button", { name: /antigravity\.btnUpdate/i });
      await act(async () => {
        updateBtn.click();
      });

      // After successful update:
      // 1. Status shows configured
      await waitFor(() => {
        expect(screen.getAllByText(/antigravity\.statusConfigured/i).length).toBeGreaterThan(0);
      });
      // 2. Outdated warning and commands MCP warning are gone
      expect(screen.queryByText(/antigravity\.statusOutdated/i)).not.toBeInTheDocument();
      expect(screen.queryByText(/antigravity\.commandsMcpWarning/i)).not.toBeInTheDocument();
      // 3. No error alert
      expect(screen.queryByText(/error/i)).not.toBeInTheDocument();
      // 4. Saved message appears
      expect(screen.getByText(/antigravity\.savedMessage/i)).toBeInTheDocument();
    });

    it("after successful removal, refreshed status reselects current executable as target, leaves legacy path unselected, enables Update button, and shows not configured status (§15)", async () => {
      let isRemoved = false;
      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_commands_status") {
          return { skills_dir: "", plan_command: {}, revise_command: {}, review_command: {} };
        }
        if (cmd === "get_antigravity_mcp_status") {
          if (!isRemoved) {
            return {
              status: "configured",
              config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
              config_dir: "C:\\Users\\User\\.gemini\\config",
              registered_command: canonicalExe,
              registered_args: ["--mcp-server"],
              legacy_status: "configured",
              legacy_command: staleLegacyExe,
              error: null,
            } as AntigravityMcpInfo;
          } else {
            return {
              status: "not_configured",
              config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
              config_dir: "C:\\Users\\User\\.gemini\\config",
              registered_command: null,
              registered_args: null,
              legacy_status: "configured",
              legacy_command: staleLegacyExe,
              error: null,
            } as AntigravityMcpInfo;
          }
        }
        if (cmd === "get_current_executable_path") {
          return currentPoverelloExe;
        }
        if (cmd === "remove_antigravity_mcp") {
          isRemoved = true;
          return {
            status: "not_configured",
            config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
            config_dir: "C:\\Users\\User\\.gemini\\config",
            registered_command: null,
            registered_args: null,
            legacy_status: "configured",
            legacy_command: staleLegacyExe,
            error: null,
          } as AntigravityMcpInfo;
        }
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByDisplayValue(canonicalExe)).toBeInTheDocument();
      });

      const removeBtn = screen.getByRole("button", { name: /antigravity\.btnRemove/i });
      expect(removeBtn).toBeInTheDocument();

      await act(async () => {
        removeBtn.click();
      });

      // 1. Selected target becomes the returned current executable
      await waitFor(() => {
        expect(screen.getByDisplayValue(currentPoverelloExe)).toBeInTheDocument();
      });
      // 2. Legacy path is NOT selected
      expect(screen.queryByDisplayValue(staleLegacyExe)).not.toBeInTheDocument();
      // 3. Update button becomes enabled
      const updateBtn = screen.getByRole("button", { name: /antigravity\.btnUpdate/i });
      expect(updateBtn).not.toBeDisabled();
      // 4. Status reflects canonical registration is absent
      expect(screen.getAllByText(/antigravity\.statusNotConfigured|antigravity\.statusOutdated/i).length).toBeGreaterThan(0);
    });

    it("after successful removal, if current-executable lookup fails, target remains empty, update is disabled, actionable error is shown, and legacy path is not selected (§15)", async () => {
      let isRemoved = false;
      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_commands_status") {
          return { skills_dir: "", plan_command: {}, revise_command: {}, review_command: {} };
        }
        if (cmd === "get_antigravity_mcp_status") {
          if (!isRemoved) {
            return {
              status: "configured",
              config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
              config_dir: "C:\\Users\\User\\.gemini\\config",
              registered_command: canonicalExe,
              registered_args: ["--mcp-server"],
              legacy_status: "configured",
              legacy_command: staleLegacyExe,
              error: null,
            } as AntigravityMcpInfo;
          } else {
            return {
              status: "not_configured",
              config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
              config_dir: "C:\\Users\\User\\.gemini\\config",
              registered_command: null,
              registered_args: null,
              legacy_status: "configured",
              legacy_command: staleLegacyExe,
              error: null,
            } as AntigravityMcpInfo;
          }
        }
        if (cmd === "get_current_executable_path") {
          if (isRemoved) {
            throw new Error("Failed to determine current executable path: access denied");
          }
          return currentPoverelloExe;
        }
        if (cmd === "remove_antigravity_mcp") {
          isRemoved = true;
          return {
            status: "not_configured",
            config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
            config_dir: "C:\\Users\\User\\.gemini\\config",
            registered_command: null,
            registered_args: null,
            legacy_status: "configured",
            legacy_command: staleLegacyExe,
            error: null,
          } as AntigravityMcpInfo;
        }
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByDisplayValue(canonicalExe)).toBeInTheDocument();
      });

      const removeBtn = screen.getByRole("button", { name: /antigravity\.btnRemove/i });
      await act(async () => {
        removeBtn.click();
      });

      // Target is empty and actionable error is shown
      await waitFor(() => {
        expect(screen.getByText(/Failed to determine current executable path: access denied/i)).toBeInTheDocument();
      });
      expect(screen.queryByDisplayValue(canonicalExe)).not.toBeInTheDocument();
      expect(screen.queryByDisplayValue(currentPoverelloExe)).not.toBeInTheDocument();
      expect(screen.queryByDisplayValue(staleLegacyExe)).not.toBeInTheDocument();

      // Update button is disabled
      const updateBtn = screen.getByRole("button", { name: /antigravity\.btnUpdate/i });
      expect(updateBtn).toBeDisabled();
    });

    it("if remove operation fails, canonical selection and status are preserved and operation error is visible (§15)", async () => {
      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_commands_status") {
          return { skills_dir: "", plan_command: {}, revise_command: {}, review_command: {} };
        }
        if (cmd === "get_antigravity_mcp_status") {
          return {
            status: "configured",
            config_path: "C:\\Users\\User\\.gemini\\config\\mcp_config.json",
            config_dir: "C:\\Users\\User\\.gemini\\config",
            registered_command: canonicalExe,
            registered_args: ["--mcp-server"],
            legacy_status: "configured",
            legacy_command: staleLegacyExe,
            error: null,
          } as AntigravityMcpInfo;
        }
        if (cmd === "get_current_executable_path") {
          return currentPoverelloExe;
        }
        if (cmd === "remove_antigravity_mcp") {
          throw new Error("Disk error: permission denied while removing server entry");
        }
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByDisplayValue(canonicalExe)).toBeInTheDocument();
      });

      const removeBtn = screen.getByRole("button", { name: /antigravity\.btnRemove/i });
      await act(async () => {
        removeBtn.click();
      });

      // Canonical selection and status are preserved
      await waitFor(() => {
        expect(screen.getByText(/Disk error: permission denied while removing server entry/i)).toBeInTheDocument();
      });
      expect(screen.getByDisplayValue(canonicalExe)).toBeInTheDocument();
      expect(screen.getAllByText(/antigravity\.statusConfigured/i).length).toBeGreaterThan(0);
      expect(screen.queryByDisplayValue(staleLegacyExe)).not.toBeInTheDocument();
    });
  });
});
