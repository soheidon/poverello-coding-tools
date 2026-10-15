import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import McpSettingPanel from "./McpSettingPanel";
import type { GatewayConfig, AntigravityMcpInfo, AntigravityCommandsInfo } from "../types";
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

  it("triggers install_all_antigravity_commands on Install All click", async () => {
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
      if (cmd === "install_all_antigravity_commands") {
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



    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: /antigravity\.btnInstallAll/i }));
    });

    expect(invokeMock).toHaveBeenCalledWith("install_all_antigravity_commands");
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
          plan_command: { name: "pov-plan", slash_command: "/pov-plan", status: "not_installed", skill_path: "", error: null },
          revise_command: { name: "pov-revise", slash_command: "/pov-revise", status: "invalid", skill_path: "", error: "Modified by user" },
          review_command: { name: "pov-review", slash_command: "/pov-review", status: "not_installed", skill_path: "", error: null },
        };
      }
      if (cmd === "install_all_antigravity_commands") {
        return {
          skills_dir: "C:\\skills",
          plan_command: { name: "pov-plan", slash_command: "/pov-plan", status: "not_installed", skill_path: "", error: null },
          revise_command: { name: "pov-revise", slash_command: "/pov-revise", status: "invalid", skill_path: "", error: "Modified by user" },
          review_command: { name: "pov-review", slash_command: "/pov-review", status: "not_installed", skill_path: "", error: null },
          install_all_results: [
            { name: "pov-plan", outcome: "not_attempted", message: "Skipped because another target failed preflight" },
            { name: "pov-revise", outcome: "refused", message: "Modified by user" },
            { name: "pov-review", outcome: "not_attempted", message: "Skipped because another target failed preflight" },
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
    expect(resultPanel.textContent).toContain("pov-plan");
    expect(resultPanel.textContent).toContain("antigravity.commandInstallOutcomeNotAttempted");
    expect(resultPanel.textContent).toContain("pov-revise");
    expect(resultPanel.textContent).toContain("antigravity.commandInstallOutcomeRefused");
    expect(resultPanel.textContent).toContain("Modified by user");
    expect(resultPanel.textContent).toContain("pov-review");
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
            name: "pov-plan",
            slash_command: "/pov-plan",
            status: "not_installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-plan\\SKILL.md",
            error: null,
          },
          revise_command: {
            name: "pov-revise",
            slash_command: "/pov-revise",
            status: "not_installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-revise\\SKILL.md",
            error: null,
          },
          review_command: {
            name: "pov-review",
            slash_command: "/pov-review",
            status: "not_installed",
            skill_path: "C:\\Users\\User\\.gemini\\config\\skills\\pov-review\\SKILL.md",
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
      return null;
    });

    render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/antigravity\.header/i)).toBeInTheDocument();
    });

    expect(screen.getByText(/antigravity\.commandPlanTitle/i)).toBeInTheDocument();
    expect(screen.getByText(/antigravity\.commandReviseTitle/i)).toBeInTheDocument();
    expect(screen.getByText(/antigravity\.commandReviewTitle/i)).toBeInTheDocument();
    expect(screen.getByText(/antigravity\.legacyNoticeTitle/i)).toBeInTheDocument();
    expect(screen.getByText(/antigravity\.legacyNoticeDesc/i)).toBeInTheDocument();
    expect(screen.getByText(/antigravity\.mcpLegacyPreserved/i)).toBeInTheDocument();

    const exeInput = screen.getByDisplayValue("C:\\Users\\User\\AppData\\Local\\Poverello\\anthro-bridge.exe");
    expect(exeInput).toBeInTheDocument();
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

    it("renders 5 reasoning effort choices for GPT-6 Astra and thinking only for Astra Pro", async () => {
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

      // Astra: effort select is available with 5 options
      const selects = screen.getAllByRole("combobox");
      const effortSelect = selects[3];
      const effortOptions = Array.from(effortSelect.querySelectorAll("option")).map((o) => o.value);
      expect(effortOptions).toEqual(["low", "medium", "high", "xhigh", "max"]);

      unmount();

      // Astra Pro: thinking_only
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
      // Only profile select and model select are rendered (no thinking mode toggle, no effort select)
      const astraProSelects = screen.getAllByRole("combobox");
      expect(astraProSelects).toHaveLength(2);
    });
  });

  describe("Antigravity Nerf Commands (/pov-nerf-balanced & /pov-nerf-lightweight)", () => {
    it("renders both Balanced and Lightweight cards with initial not_installed status", async () => {
      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_pov_nerf_commands_status") {
          return {
            skills_dir: "C:\\skills",
            skills: [
              {
                skill_id: "balanced",
                skill_name: "pov-nerf-balanced",
                slash_command: "/pov-nerf-balanced",
                target_path: "C:\\skills\\pov-nerf-balanced\\SKILL.md",
                state: "not_installed",
                permitted_actions: ["install"],
                diagnostic_reason: null,
              },
              {
                skill_id: "lightweight",
                skill_name: "pov-nerf-lightweight",
                slash_command: "/pov-nerf-lightweight",
                target_path: "C:\\skills\\pov-nerf-lightweight\\SKILL.md",
                state: "not_installed",
                permitted_actions: ["install"],
                diagnostic_reason: null,
              },
            ],
            legacy_single_skill: null,
          };
        }
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_mcp_status") return { status: "not_configured" };
        if (cmd === "get_antigravity_commands_status") return {
          skills_dir: "C:\\skills",
          plan_command: { name: "anthro-plan", slash_command: "/anthro-plan", status: "installed", skill_path: "", error: null },
          revise_command: { name: "anthro-revise", slash_command: "/anthro-revise", status: "installed", skill_path: "", error: null },
          review_command: { name: "anthro-review", slash_command: "/anthro-review", status: "installed", skill_path: "", error: null },
        };
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText(/antigravity\.nerfSectionHeader/i)).toBeInTheDocument();
      });

      expect(screen.getByText(/antigravity\.nerfBalancedTitle/i)).toBeInTheDocument();
      expect(screen.getByText(/antigravity\.nerfLightweightTitle/i)).toBeInTheDocument();
      const notInstalledBadges = screen.getAllByText(/antigravity\.commandStatusNotInstalled/i);
      expect(notInstalledBadges.length).toBeGreaterThanOrEqual(2);
    });

    it("allows installing Balanced command independently", async () => {
      let installCalledWith: any = null;
      let statusCalled = 0;
      invokeMock.mockImplementation(async (cmd: string, args?: any) => {
        if (cmd === "get_pov_nerf_commands_status") {
          statusCalled++;
          return {
            skills_dir: "C:\\skills",
            skills: [
              {
                skill_id: "balanced",
                skill_name: "pov-nerf-balanced",
                slash_command: "/pov-nerf-balanced",
                target_path: "C:\\skills\\pov-nerf-balanced\\SKILL.md",
                state: statusCalled > 1 ? "installed" : "not_installed",
                permitted_actions: statusCalled > 1 ? ["remove"] : ["install"],
                diagnostic_reason: null,
              },
              {
                skill_id: "lightweight",
                skill_name: "pov-nerf-lightweight",
                slash_command: "/pov-nerf-lightweight",
                target_path: "C:\\skills\\pov-nerf-lightweight\\SKILL.md",
                state: "not_installed",
                permitted_actions: ["install"],
                diagnostic_reason: null,
              },
            ],
            legacy_single_skill: null,
          };
        }
        if (cmd === "install_pov_nerf_command") {
          installCalledWith = args;
          return {
            outcome: "success",
            skill: {
              skill_id: args?.skillId,
              skill_name: `pov-nerf-${args?.skillId}`,
              slash_command: `/pov-nerf-${args?.skillId}`,
              target_path: `C:\\skills\\pov-nerf-${args?.skillId}\\SKILL.md`,
              state: "installed",
              permitted_actions: ["remove"],
              diagnostic_reason: null,
            },
            message: null,
          };
        }
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_mcp_status") return { status: "not_configured" };
        if (cmd === "get_antigravity_commands_status") return {
          skills_dir: "C:\\skills",
          plan_command: { name: "anthro-plan", slash_command: "/anthro-plan", status: "installed", skill_path: "", error: null },
          revise_command: { name: "anthro-revise", slash_command: "/anthro-revise", status: "installed", skill_path: "", error: null },
          review_command: { name: "anthro-review", slash_command: "/anthro-review", status: "installed", skill_path: "", error: null },
        };
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText(/antigravity\.nerfBalancedTitle/i)).toBeInTheDocument();
      });

      const installBtns = screen.getAllByRole("button", { name: /antigravity\.commandBtnInstall/i });
      await act(async () => {
        installBtns[0].click();
      });

      expect(installCalledWith).toEqual({ skillId: "balanced" });
      await waitFor(() => {
        expect(screen.getAllByText(/antigravity\.commandStatusInstalled/i).length).toBeGreaterThanOrEqual(1);
      });
    });

    it("displays legacy warning when legacy_single_skill is present", async () => {
      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_pov_nerf_commands_status") {
          return {
            skills_dir: "C:\\skills",
            skills: [
              {
                skill_id: "balanced",
                skill_name: "pov-nerf-balanced",
                slash_command: "/pov-nerf-balanced",
                target_path: "C:\\skills\\pov-nerf-balanced\\SKILL.md",
                state: "not_installed",
                permitted_actions: ["install"],
                diagnostic_reason: null,
              },
              {
                skill_id: "lightweight",
                skill_name: "pov-nerf-lightweight",
                slash_command: "/pov-nerf-lightweight",
                target_path: "C:\\skills\\pov-nerf-lightweight\\SKILL.md",
                state: "not_installed",
                permitted_actions: ["install"],
                diagnostic_reason: null,
              },
            ],
            legacy_single_skill: {
              present: true,
              path: "C:\\skills\\pov-nerf\\SKILL.md",
              message: "A legacy /pov-nerf Skill was detected. It is left untouched.",
            },
          };
        }
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_mcp_status") return { status: "not_configured" };
        if (cmd === "get_antigravity_commands_status") return {
          skills_dir: "C:\\skills",
          plan_command: { name: "anthro-plan", slash_command: "/anthro-plan", status: "installed", skill_path: "", error: null },
          revise_command: { name: "anthro-revise", slash_command: "/anthro-revise", status: "installed", skill_path: "", error: null },
          review_command: { name: "anthro-review", slash_command: "/anthro-review", status: "installed", skill_path: "", error: null },
        };
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText(/A legacy \/pov-nerf Skill was detected/i)).toBeInTheDocument();
      });
      expect(screen.getByText(/C:\\skills\\pov-nerf\\SKILL\.md/i)).toBeInTheDocument();
    });

    it("handles modified skill state safely without action buttons", async () => {
      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_pov_nerf_commands_status") {
          return {
            skills_dir: "C:\\skills",
            skills: [
              {
                skill_id: "balanced",
                skill_name: "pov-nerf-balanced",
                slash_command: "/pov-nerf-balanced",
                target_path: "C:\\skills\\pov-nerf-balanced\\SKILL.md",
                state: "modified",
                permitted_actions: [],
                diagnostic_reason: "File content or digest does not match the canonical template.",
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
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_mcp_status") return { status: "not_configured" };
        if (cmd === "get_antigravity_commands_status") return {
          skills_dir: "C:\\skills",
          plan_command: { name: "anthro-plan", slash_command: "/anthro-plan", status: "installed", skill_path: "", error: null },
          revise_command: { name: "anthro-revise", slash_command: "/anthro-revise", status: "installed", skill_path: "", error: null },
          review_command: { name: "anthro-review", slash_command: "/anthro-review", status: "installed", skill_path: "", error: null },
        };
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText(/antigravity\.nerfStatusModified/i)).toBeInTheDocument();
      });
      expect(screen.getByText(/File content or digest does not match the canonical template/i)).toBeInTheDocument();
    });

    it("handles invalid skill state safely with diagnostic reason and no action buttons", async () => {
      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_pov_nerf_commands_status") {
          return {
            skills_dir: "C:\\skills",
            skills: [
              {
                skill_id: "balanced",
                skill_name: "pov-nerf-balanced",
                slash_command: "/pov-nerf-balanced",
                target_path: "C:\\skills\\pov-nerf-balanced\\SKILL.md",
                state: "invalid",
                permitted_actions: [],
                diagnostic_reason: "Target component is a symlink",
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
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_mcp_status") return { status: "not_configured" };
        if (cmd === "get_antigravity_commands_status") return {
          skills_dir: "C:\\skills",
          plan_command: { name: "anthro-plan", slash_command: "/anthro-plan", status: "installed", skill_path: "", error: null },
          revise_command: { name: "anthro-revise", slash_command: "/anthro-revise", status: "installed", skill_path: "", error: null },
          review_command: { name: "anthro-review", slash_command: "/anthro-review", status: "installed", skill_path: "", error: null },
        };
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText(/Target component is a symlink/i)).toBeInTheDocument();
      });
      expect(screen.getByText(/antigravity\.commandStatusInvalid/i)).toBeInTheDocument();
    });

    it("fails closed on same-mounted component on refresh failure after valid status and recovers on retry without remount", async () => {
      let failStatus = false;
      let installCount = 0;

      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_pov_nerf_commands_status") {
          if (failStatus) {
            throw new Error("I/O error reading skills folder");
          }
          return {
            skills_dir: "C:\\skills",
            skills: [
              {
                skill_id: "balanced",
                skill_name: "pov-nerf-balanced",
                slash_command: "/pov-nerf-balanced",
                target_path: "C:\\skills\\pov-nerf-balanced\\SKILL.md",
                state: installCount > 0 ? "installed" : "not_installed",
                permitted_actions: installCount > 0 ? ["remove"] : ["install"],
                diagnostic_reason: null,
              },
              {
                skill_id: "lightweight",
                skill_name: "pov-nerf-lightweight",
                slash_command: "/pov-nerf-lightweight",
                target_path: "C:\\skills\\pov-nerf-lightweight\\SKILL.md",
                state: "not_installed",
                permitted_actions: ["install"],
                diagnostic_reason: null,
              },
            ],
            legacy_single_skill: null,
          };
        }
        if (cmd === "install_pov_nerf_command") {
          installCount++;
          failStatus = true;
          return {
            outcome: "success",
            skill: {
              skill_id: "balanced",
              skill_name: "pov-nerf-balanced",
              slash_command: "/pov-nerf-balanced",
              target_path: "C:\\skills\\pov-nerf-balanced\\SKILL.md",
              state: "installed",
              permitted_actions: ["remove"],
              diagnostic_reason: null,
            },
            message: null,
          };
        }
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_mcp_status") return { status: "not_configured" };
        if (cmd === "get_antigravity_commands_status") return {
          skills_dir: "C:\\skills",
          plan_command: { name: "anthro-plan", slash_command: "/anthro-plan", status: "installed", skill_path: "", error: null },
          revise_command: { name: "anthro-revise", slash_command: "/anthro-revise", status: "installed", skill_path: "", error: null },
          review_command: { name: "anthro-review", slash_command: "/anthro-review", status: "installed", skill_path: "", error: null },
        };
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      // Step 1: Initial valid load shows cards and install buttons
      await waitFor(() => {
        expect(screen.getByText(/antigravity\.nerfBalancedTitle|\/pov-nerf-balanced/i)).toBeInTheDocument();
      });

      const installButtons = screen.getAllByRole("button", { name: /antigravity\.commandBtnInstall|インストール|Install/i });
      expect(installButtons.length).toBeGreaterThan(0);

      // Step 2: Trigger mutation; its subsequent refresh fails on the SAME mounted component
      await act(async () => {
        await userEvent.click(installButtons[0]);
      });

      // Stale cards and action buttons must be cleared/hidden, error and localized retry must be visible
      await waitFor(() => {
        expect(screen.getByText(/I\/O error reading skills folder/i)).toBeInTheDocument();
      });

      expect(screen.queryByText(/antigravity\.nerfBalancedTitle|\/pov-nerf-balanced/i)).not.toBeInTheDocument();
      expect(screen.queryByText(/antigravity\.nerfLightweightTitle|\/pov-nerf-lightweight/i)).not.toBeInTheDocument();
      // Only the 3 general commands Remove buttons exist; Nerf remove button is unmounted
      expect(screen.getAllByRole("button", { name: /antigravity\.commandBtnRemove|削除|Remove/i })).toHaveLength(3);

      const retryBtn = screen.getByRole("button", { name: /antigravity\.nerfRetryStatus|再試行|Retry/i });
      expect(retryBtn).toBeInTheDocument();

      // Step 3: Trigger Retry on same mounted component; fresh valid response restores controls
      failStatus = false;
      await act(async () => {
        await userEvent.click(retryBtn);
      });

      await waitFor(() => {
        expect(screen.queryByText(/I\/O error reading skills folder/i)).not.toBeInTheDocument();
        expect(screen.getByText(/antigravity\.nerfBalancedTitle|\/pov-nerf-balanced/i)).toBeInTheDocument();
      });

      // 3 general commands + 1 balanced Nerf command = 4 Remove buttons
      expect(screen.getAllByRole("button", { name: /antigravity\.commandBtnRemove|削除|Remove/i })).toHaveLength(4);
    });

    it("discards stale responses when status refreshes race: newer request response wins even if older finishes later", async () => {
      type Deferred<T> = {
        promise: Promise<T>;
        resolve: (val: T) => void;
        reject: (err: unknown) => void;
      };
      function createDeferred<T>(): Deferred<T> {
        let resolve!: (val: T) => void;
        let reject!: (err: unknown) => void;
        const promise = new Promise<T>((res, rej) => {
          resolve = res;
          reject = rej;
        });
        return { promise, resolve, reject };
      }

      const deferred1 = createDeferred<unknown>();
      const deferred2 = createDeferred<unknown>();
      let callCount = 0;

      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_pov_nerf_commands_status") {
          callCount++;
          if (callCount === 1) {
            // First mount: return immediate error to show retry control
            throw new Error("Initial failure to show retry");
          }
          if (callCount === 2) {
            return deferred1.promise;
          }
          return deferred2.promise;
        }
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_mcp_status") return { status: "not_configured" };
        if (cmd === "get_antigravity_commands_status") return {
          skills_dir: "C:\\skills",
          plan_command: { name: "anthro-plan", slash_command: "/anthro-plan", status: "not_installed", skill_path: "", error: null },
          revise_command: { name: "anthro-revise", slash_command: "/anthro-revise", status: "not_installed", skill_path: "", error: null },
          review_command: { name: "anthro-review", slash_command: "/anthro-review", status: "not_installed", skill_path: "", error: null },
        };
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText(/Initial failure to show retry/i)).toBeInTheDocument();
      });

      const retryBtn = screen.getByRole("button", { name: /antigravity\.nerfRetryStatus|再試行|Retry/i });

      // Click Retry once -> triggers callCount 2 (deferred1)
      await act(async () => {
        await userEvent.click(retryBtn);
      });
      expect(callCount).toBe(2);

      // Click Retry second time -> triggers callCount 3 (deferred2)
      await act(async () => {
        await userEvent.click(retryBtn);
      });
      expect(callCount).toBe(3);

      // Resolve Call 2 FIRST with fresh valid status
      await act(async () => {
        deferred2.resolve({
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
              state: "not_installed",
              permitted_actions: ["install"],
              diagnostic_reason: null,
            },
          ],
          legacy_single_skill: null,
        });
      });

      await waitFor(() => {
        expect(screen.queryByText(/Initial failure to show retry/i)).not.toBeInTheDocument();
        expect(screen.getByText(/antigravity\.nerfBalancedTitle|\/pov-nerf-balanced/i)).toBeInTheDocument();
      });

      // Now resolve Call 1 SECOND with a stale failure (inverted completion order)
      await act(async () => {
        deferred1.reject(new Error("Stale slow network error from request 1"));
      });

      // The stale response must NOT overwrite the newer state
      expect(screen.queryByText(/Stale slow network error from request 1/i)).not.toBeInTheDocument();
      expect(screen.getByText(/antigravity\.nerfBalancedTitle|\/pov-nerf-balanced/i)).toBeInTheDocument();
      expect(screen.getByRole("button", { name: /antigravity\.commandBtnRemove|削除|Remove/i })).toBeInTheDocument();
    });

    it("disables operation buttons and prevents operation execution while status is loading", async () => {
      type Deferred<T> = {
        promise: Promise<T>;
        resolve: (val: T) => void;
        reject: (err: unknown) => void;
      };
      function createDeferred<T>(): Deferred<T> {
        let resolve!: (val: T) => void;
        let reject!: (err: unknown) => void;
        const promise = new Promise<T>((res, rej) => {
          resolve = res;
          reject = rej;
        });
        return { promise, resolve, reject };
      }

      const deferred = createDeferred<unknown>();
      let callCount = 0;
      let installCommandCalls = 0;

      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_pov_nerf_commands_status") {
          callCount++;
          if (callCount === 1) {
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
                  state: "not_installed",
                  permitted_actions: ["install"],
                  diagnostic_reason: null,
                },
              ],
              legacy_single_skill: null,
            };
          }
          return deferred.promise;
        }
        if (cmd === "remove_pov_nerf_command") {
          return { outcome: "success", skill: null, message: null };
        }
        if (cmd === "install_pov_nerf_command") {
          installCommandCalls++;
          return { outcome: "success", skill: null, message: null };
        }
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_mcp_status") return { status: "not_configured" };
        if (cmd === "get_antigravity_commands_status") return {
          skills_dir: "C:\\skills",
          plan_command: { name: "anthro-plan", slash_command: "/anthro-plan", status: "not_installed", skill_path: "", error: null },
          revise_command: { name: "anthro-revise", slash_command: "/anthro-revise", status: "not_installed", skill_path: "", error: null },
          review_command: { name: "anthro-review", slash_command: "/anthro-review", status: "not_installed", skill_path: "", error: null },
        };
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText(/antigravity\.nerfBalancedTitle|\/pov-nerf-balanced/i)).toBeInTheDocument();
      });

      const removeButtons = screen.getAllByRole("button", { name: /antigravity\.commandBtnRemove|削除|Remove/i });
      const nerfRemoveBtn = removeButtons[removeButtons.length - 1];
      const installButtons = screen.getAllByRole("button", { name: /antigravity\.commandBtnInstall|インストール|Install/i });
      const nerfInstallBtn = installButtons[installButtons.length - 1];

      expect(nerfRemoveBtn).toBeEnabled();
      expect(nerfInstallBtn).toBeEnabled();

      // Trigger remove operation; its post-operation status refresh hangs on deferred promise
      await act(async () => {
        await userEvent.click(nerfRemoveBtn);
      });

      // While status refresh is loading, buttons must be disabled
      expect(nerfInstallBtn).toBeDisabled();

      // Attempting to click disabled install button does not execute install
      await act(async () => {
        await userEvent.click(nerfInstallBtn);
      });
      expect(installCommandCalls).toBe(0);

      // Resolve the pending status refresh
      await act(async () => {
        deferred.resolve({
          skills_dir: "C:\\skills",
          skills: [
            {
              skill_id: "balanced",
              skill_name: "pov-nerf-balanced",
              slash_command: "/pov-nerf-balanced",
              target_path: "C:\\skills\\pov-nerf-balanced\\SKILL.md",
              state: "not_installed",
              permitted_actions: ["install"],
              diagnostic_reason: null,
            },
            {
              skill_id: "lightweight",
              skill_name: "pov-nerf-lightweight",
              slash_command: "/pov-nerf-lightweight",
              target_path: "C:\\skills\\pov-nerf-lightweight\\SKILL.md",
              state: "not_installed",
              permitted_actions: ["install"],
              diagnostic_reason: null,
            },
          ],
          legacy_single_skill: null,
        });
      });

      await waitFor(() => {
        const refreshedInstalls = screen.getAllByRole("button", { name: /antigravity\.commandBtnInstall|インストール|Install/i });
        expect(refreshedInstalls[refreshedInstalls.length - 1]).toBeEnabled();
      });
    });

    it("retains operation error when operation fails but subsequent status refresh succeeds, without showing retry", async () => {
      invokeMock.mockImplementation(async (cmd: string) => {
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
                state: "not_installed",
                permitted_actions: ["install"],
                diagnostic_reason: null,
              },
            ],
            legacy_single_skill: null,
          };
        }
        if (cmd === "remove_pov_nerf_command") {
          return {
            outcome: "refused",
            skill: null,
            message: "Target file modified by external editor; refusing removal",
          };
        }
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_mcp_status") return { status: "not_configured" };
        if (cmd === "get_antigravity_commands_status") return {
          skills_dir: "C:\\skills",
          plan_command: { name: "anthro-plan", slash_command: "/anthro-plan", status: "not_installed", skill_path: "", error: null },
          revise_command: { name: "anthro-revise", slash_command: "/anthro-revise", status: "not_installed", skill_path: "", error: null },
          review_command: { name: "anthro-review", slash_command: "/anthro-review", status: "not_installed", skill_path: "", error: null },
        };
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText(/antigravity\.nerfBalancedTitle|\/pov-nerf-balanced/i)).toBeInTheDocument();
      });

      const removeButtons = screen.getAllByRole("button", { name: /antigravity\.commandBtnRemove|削除|Remove/i });
      const nerfRemoveBtn = removeButtons[removeButtons.length - 1];

      await act(async () => {
        await userEvent.click(nerfRemoveBtn);
      });

      // Operation error remains visible
      await waitFor(() => {
        expect(screen.getByText(/Target file modified by external editor; refusing removal/i)).toBeInTheDocument();
      });

      // Status refresh succeeded, so status Retry button is NOT displayed
      expect(screen.queryByRole("button", { name: /antigravity\.nerfRetryStatus|再試行|Retry/i })).not.toBeInTheDocument();
    });

    it("retains operation error when operation invoke rejects with diagnostic error but subsequent status refresh succeeds, without showing retry", async () => {
      invokeMock.mockImplementation(async (cmd: string) => {
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
                state: "not_installed",
                permitted_actions: ["install"],
                diagnostic_reason: null,
              },
            ],
            legacy_single_skill: null,
          };
        }
        if (cmd === "remove_pov_nerf_command") {
          throw new Error("IPC transport error: connection broken during removal");
        }
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_mcp_status") return { status: "not_configured" };
        if (cmd === "get_antigravity_commands_status") return {
          skills_dir: "C:\\skills",
          plan_command: { name: "anthro-plan", slash_command: "/anthro-plan", status: "not_installed", skill_path: "", error: null },
          revise_command: { name: "anthro-revise", slash_command: "/anthro-revise", status: "not_installed", skill_path: "", error: null },
          review_command: { name: "anthro-review", slash_command: "/anthro-review", status: "not_installed", skill_path: "", error: null },
        };
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText(/antigravity\.nerfBalancedTitle|\/pov-nerf-balanced/i)).toBeInTheDocument();
      });

      const removeButtons = screen.getAllByRole("button", { name: /antigravity\.commandBtnRemove|削除|Remove/i });
      const nerfRemoveBtn = removeButtons[removeButtons.length - 1];

      await act(async () => {
        await userEvent.click(nerfRemoveBtn);
      });

      // Operation error from rejected invocation remains visible
      await waitFor(() => {
        expect(screen.getByText(/IPC transport error: connection broken during removal/i)).toBeInTheDocument();
      });

      // Status refresh succeeded, so status Retry button is absent
      expect(screen.queryByRole("button", { name: /antigravity\.nerfRetryStatus|再試行|Retry/i })).not.toBeInTheDocument();

      // Cards remain visible because status refresh succeeded
      expect(screen.getByText(/antigravity\.nerfBalancedTitle|\/pov-nerf-balanced/i)).toBeInTheDocument();
    });

    it("displays both operation and status errors when both fail; retry re-runs only status refresh and keeps operation error", async () => {
      let removeCalls = 0;
      let failStatus = false;

      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_pov_nerf_commands_status") {
          if (failStatus) {
            throw new Error("Disk read failure during status inspection");
          }
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
                state: "not_installed",
                permitted_actions: ["install"],
                diagnostic_reason: null,
              },
            ],
            legacy_single_skill: null,
          };
        }
        if (cmd === "remove_pov_nerf_command") {
          removeCalls++;
          failStatus = true;
          return {
            outcome: "refused",
            skill: null,
            message: "Pre-delete recheck failed: target file identity changed",
          };
        }
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_mcp_status") return { status: "not_configured" };
        if (cmd === "get_antigravity_commands_status") return {
          skills_dir: "C:\\skills",
          plan_command: { name: "anthro-plan", slash_command: "/anthro-plan", status: "not_installed", skill_path: "", error: null },
          revise_command: { name: "anthro-revise", slash_command: "/anthro-revise", status: "not_installed", skill_path: "", error: null },
          review_command: { name: "anthro-review", slash_command: "/anthro-review", status: "not_installed", skill_path: "", error: null },
        };
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText(/antigravity\.nerfBalancedTitle|\/pov-nerf-balanced/i)).toBeInTheDocument();
      });

      const removeButtons = screen.getAllByRole("button", { name: /antigravity\.commandBtnRemove|削除|Remove/i });
      const nerfRemoveBtn = removeButtons[removeButtons.length - 1];

      await act(async () => {
        await userEvent.click(nerfRemoveBtn);
      });

      // Both diagnostics must be displayed
      await waitFor(() => {
        expect(screen.getByText(/Pre-delete recheck failed: target file identity changed/i)).toBeInTheDocument();
        expect(screen.getByText(/Disk read failure during status inspection/i)).toBeInTheDocument();
      });

      // Retry button is visible for status error
      const retryBtn = screen.getByRole("button", { name: /antigravity\.nerfRetryStatus|再試行|Retry/i });
      expect(retryBtn).toBeInTheDocument();
      expect(removeCalls).toBe(1);

      // Now configure status refresh to succeed on retry
      failStatus = false;
      await act(async () => {
        await userEvent.click(retryBtn);
      });

      // After retry: status error disappears, but operation error remains
      await waitFor(() => {
        expect(screen.queryByText(/Disk read failure during status inspection/i)).not.toBeInTheDocument();
        expect(screen.getByText(/Pre-delete recheck failed: target file identity changed/i)).toBeInTheDocument();
      });

      // Retry button disappears because status fetch succeeded
      expect(screen.queryByRole("button", { name: /antigravity\.nerfRetryStatus|再試行|Retry/i })).not.toBeInTheDocument();

      // Operation (remove_pov_nerf_command) was NOT re-executed
      expect(removeCalls).toBe(1);
    });

    it("discards stale successful status response when preceded by a newer failing status request", async () => {
      type Deferred<T> = {
        promise: Promise<T>;
        resolve: (val: T) => void;
        reject: (err: unknown) => void;
      };
      function createDeferred<T>(): Deferred<T> {
        let resolve!: (val: T) => void;
        let reject!: (err: unknown) => void;
        const promise = new Promise<T>((res, rej) => {
          resolve = res;
          reject = rej;
        });
        return { promise, resolve, reject };
      }

      const deferredA = createDeferred<unknown>();
      const deferredB = createDeferred<unknown>();
      let callCount = 0;

      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_pov_nerf_commands_status") {
          callCount++;
          if (callCount === 1) {
            // Initial mount failure to reveal retry button
            throw new Error("Initial failure to show retry");
          }
          if (callCount === 2) {
            // Request A
            return deferredA.promise;
          }
          // Request B
          return deferredB.promise;
        }
        if (cmd === "get_mcp_config") return {};
        if (cmd === "get_antigravity_mcp_status") return { status: "not_configured" };
        if (cmd === "get_antigravity_commands_status") return {
          skills_dir: "C:\\skills",
          plan_command: { name: "anthro-plan", slash_command: "/anthro-plan", status: "not_installed", skill_path: "", error: null },
          revise_command: { name: "anthro-revise", slash_command: "/anthro-revise", status: "not_installed", skill_path: "", error: null },
          review_command: { name: "anthro-review", slash_command: "/anthro-review", status: "not_installed", skill_path: "", error: null },
        };
        return {};
      });

      render(<McpSettingPanel config={dummyConfig} refreshConfig={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText(/Initial failure to show retry/i)).toBeInTheDocument();
      });

      const retryBtn = screen.getByRole("button", { name: /antigravity\.nerfRetryStatus|再試行|Retry/i });

      // Click Retry 1st time -> triggers Request A (callCount = 2, deferredA)
      await act(async () => {
        await userEvent.click(retryBtn);
      });
      expect(callCount).toBe(2);

      // Click Retry 2nd time -> triggers Request B (callCount = 3, deferredB)
      await act(async () => {
        await userEvent.click(retryBtn);
      });
      expect(callCount).toBe(3);

      // Settle Request B FIRST with failure
      await act(async () => {
        deferredB.reject(new Error("Newer error from request B"));
      });

      await waitFor(() => {
        expect(screen.getByText(/Newer error from request B/i)).toBeInTheDocument();
      });

      // Cards/actions remain unavailable, and the loading indicator is cleared while A remains pending
      expect(screen.queryByText(/antigravity\.nerfBalancedTitle|\/pov-nerf-balanced/i)).not.toBeInTheDocument();
      expect(screen.queryByText(/antigravity\.nerfLightweightTitle|\/pov-nerf-lightweight/i)).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: /antigravity\.commandBtnRemove|削除|Remove/i })).not.toBeInTheDocument();
      // Only the 3 general commands install buttons exist; Nerf install button is absent
      expect(screen.getAllByRole("button", { name: /antigravity\.commandBtnInstall/i })).toHaveLength(3);
      expect(screen.queryByText("...")).not.toBeInTheDocument();
      expect(screen.getByRole("button", { name: /antigravity\.nerfRetryStatus|再試行|Retry/i })).toBeEnabled();

      // Settle Request A SECOND with a stale success
      await act(async () => {
        deferredA.resolve({
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
              state: "not_installed",
              permitted_actions: ["install"],
              diagnostic_reason: null,
            },
          ],
          legacy_single_skill: null,
        });
      });

      // The stale success of Request A cannot replace B's error, restore cards/actions, or change loading back to active
      expect(screen.getByText(/Newer error from request B/i)).toBeInTheDocument();
      expect(screen.queryByText(/antigravity\.nerfBalancedTitle|\/pov-nerf-balanced/i)).not.toBeInTheDocument();
      expect(screen.queryByText(/antigravity\.nerfLightweightTitle|\/pov-nerf-lightweight/i)).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: /antigravity\.commandBtnRemove|削除|Remove/i })).not.toBeInTheDocument();
      expect(screen.getAllByRole("button", { name: /antigravity\.commandBtnInstall/i })).toHaveLength(3);
      expect(screen.queryByText("...")).not.toBeInTheDocument();
      expect(screen.getByRole("button", { name: /antigravity\.nerfRetryStatus|再試行|Retry/i })).toBeEnabled();
    });

    it("contains antigravity.nerfRetryStatus in all eight locale dictionaries exactly once and renders it on retry control", () => {
      const locales = [
        { code: "en", map: en },
        { code: "ja", map: ja },
        { code: "de", map: de },
        { code: "es", map: es },
        { code: "fr", map: fr },
        { code: "ko", map: ko },
        { code: "zh-CN", map: zhCN },
        { code: "zh-TW", map: zhTW },
      ];
      for (const { code, map } of locales) {
        const val = (map as Record<string, string>)["antigravity.nerfRetryStatus"];
        expect(val, `Missing or empty antigravity.nerfRetryStatus in ${code}`).toBeTruthy();
        expect(typeof val).toBe("string");
        expect(val.trim().length).toBeGreaterThan(0);
      }
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
});
