import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { LanguageContext } from "../i18n";
import App from "../App";
import { DEFAULT_ORCHESTRATOR_PROFILES, DEFAULT_ORCHESTRATOR_QUICK_SLOTS, DEFAULT_VALIDATION_GATES, DEFAULT_ITERATION_LIMITS } from "../config/orchestratorPresets";

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    title: vi.fn().mockResolvedValue("Poverello"),
    isMaximized: vi.fn().mockResolvedValue(false),
    onResized: vi.fn().mockResolvedValue(() => {}),
    minimize: vi.fn().mockResolvedValue(undefined),
    toggleMaximize: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined),
    setSize: vi.fn().mockResolvedValue(undefined),
    innerSize: vi.fn().mockResolvedValue({ width: 800, height: 600 }),
    outerSize: vi.fn().mockResolvedValue({ width: 800, height: 600 }),
    scaleFactor: vi.fn().mockResolvedValue(1),
  }),
}));

describe("Settings Sub-Navigation Integration", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "is_first_run") return false;
      if (cmd === "read_config") {
        return {
          server: { host: "127.0.0.1", port: 4000, enable_cors: true },
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
        };
      }
      if (cmd === "get_orchestrator_config") {
        return {
          profiles: DEFAULT_ORCHESTRATOR_PROFILES,
          assignments: {},
          quickSlots: DEFAULT_ORCHESTRATOR_QUICK_SLOTS,
          validationGates: DEFAULT_VALIDATION_GATES,
          iterationLimits: DEFAULT_ITERATION_LIMITS,
          authorizedCustomGates: [],
        };
      }
      if (cmd === "get_health") {
        return { reachable: true, port_listening: true, managed_child_running: false };
      }
      if (cmd === "resolve_claude_code_auto_compact") {
        return { enabled: false, window_size: 262144, pct_override: 90 };
      }
      if (cmd === "get_mcp_config") {
        return { provider: "deepseek", model: "deepseek-chat" };
      }
      if (cmd === "get_antigravity_mcp_status") {
        return { status: "not_configured" };
      }
      if (cmd === "get_pov_direct_status") {
        return {
          rule_path: "C:\\Users\\Sohei\\.gemini\\config\\rules\\pov-direct.md",
          skill_path: "C:\\Users\\Sohei\\.gemini\\config\\skills\\pov-direct\\SKILL.md",
          skill_present: true,
          state: "managed_synced",
          settings_state: "embedded_valid",
          settings: {
            auto_activation: false,
            web_research: "requested_only",
            response_verbosity: "detailed",
            repository_exploration: "detailed",
            verification: "broad",
          },
          preview_text: "Automatic activation: Disabled (manual only: activated via @pov-direct mention; /pov-direct invokes canonical Skill only)\nWeb research: When requested only\nResponse verbosity: detailed\nRepository exploration: detailed\nVerification: broad",
          expanded_byte_size: 3820,
          byte_limit: 24000,
          can_apply: true,
          available_operations: ["apply"],
          block_reason: null,
          content_hash: "abc123hash",
          legacy_sidecar_present: false,
        };
      }
      if (cmd === "get_antigravity_commands_status") {
        return { skills_dir: "", plan_command: { name: "anthro-plan", status: "not_installed" }, revise_command: { name: "anthro-revise", status: "not_installed" } };
      }
      if (cmd === "find_claude_configs") return [];
      if (cmd === "get_pricing_custom_models") return [];
      if (cmd === "get_timezone_setting") return "local";
      if (cmd === "get_raw_config") return { content: "", encoding_used: "UTF-8" };
      if (cmd === "list_logs") return [];
      if (cmd === "get_managed_log") return "";
      if (cmd === "get_user_language") return "ja";
      return null;
    });
  });

  it("navigates to Settings and switches between General, Claude Desktop, Antigravity, Antigravity Rules, and Orchestrator", async () => {
    const user = userEvent.setup();
    render(
      <LanguageContext.Provider value={{ lang: "ja", setLang: vi.fn() }}>
        <App />
      </LanguageContext.Provider>
    );

    // Wait for app load and switch to Settings tab (in test environment, translation keys are returned)
    const settingsTopTab = await screen.findByRole("button", { name: /header\.settings/i });
    await user.click(settingsTopTab);

    // Sidebar items should be visible
    const generalNav = await screen.findByRole("button", { name: /settings\.nav\.general/i });
    const claudeNav = screen.getByRole("button", { name: /settings\.nav\.claudeDesktop/i });
    const antigravityNav = screen.getByRole("button", { name: /settings\.nav\.antigravity/i });
    const rulesNav = screen.getByRole("button", { name: /Antigravity Rules/i });
    const orchestratorNav = screen.getByRole("button", { name: "Orchestrator" });

    expect(generalNav).toBeInTheDocument();
    expect(claudeNav).toBeInTheDocument();
    expect(antigravityNav).toBeInTheDocument();
    expect(rulesNav).toBeInTheDocument();
    expect(orchestratorNav).toBeInTheDocument();

    // 1. Initial sub-tab: General
    expect(generalNav).toHaveClass("active");
    expect(screen.getByText(/language\.header/i)).toBeInTheDocument();
    expect(screen.getByText(/^peakValley\.pricingDisplayTimezone$/i)).toBeInTheDocument();
    expect(screen.queryByText(/apiKeyPanel\.header/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/claudeConfig\.header/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/antigravity\.header/i)).not.toBeInTheDocument();
    expect(screen.queryAllByRole("heading", { name: /Poverello Direct Coding/i })).toHaveLength(0);

    // 2. Click Claude Desktop
    await user.click(claudeNav);
    expect(claudeNav).toHaveClass("active");
    expect(generalNav).not.toHaveClass("active");
    expect(screen.getByText(/apiKeyPanel\.header/i)).toBeInTheDocument();
    expect(screen.getByText(/claudeConfig\.header/i)).toBeInTheDocument();
    expect(screen.queryByText(/language\.header/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/antigravity\.header/i)).not.toBeInTheDocument();
    expect(screen.queryAllByRole("heading", { name: /Poverello Direct Coding/i })).toHaveLength(0);

    // 3. Click Antigravity
    await user.click(antigravityNav);
    expect(antigravityNav).toHaveClass("active");
    expect(claudeNav).not.toHaveClass("active");
    expect(screen.getByText(/antigravity\.header/i)).toBeInTheDocument();
    expect(screen.queryByText(/claudeConfig\.header/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/apiKeyPanel\.header/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/language\.header/i)).not.toBeInTheDocument();
    expect(screen.queryAllByRole("heading", { name: /Poverello Direct Coding/i })).toHaveLength(0);

    // 4. Click Antigravity Rules
    await user.click(rulesNav);
    expect(rulesNav).toHaveClass("active");
    expect(antigravityNav).not.toHaveClass("active");
    expect((await screen.findAllByRole("heading", { name: /Poverello Direct Coding/i })).length).toBeGreaterThan(0);
    expect(screen.queryByText(/claudeConfig\.header/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/apiKeyPanel\.header/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/language\.header/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/antigravity\.header/i)).not.toBeInTheDocument();

    // Verify non-default persisted values from backend are loaded rather than component defaults (§47)
    const autoActivationCheckbox = screen.getByRole("checkbox");
    expect(autoActivationCheckbox).not.toBeChecked();

    const requestedOnlyRadio = screen.getAllByRole("radio").find((r) => (r as HTMLInputElement).value === "requested_only");
    expect(requestedOnlyRadio).toBeChecked();

    expect(screen.getByText(/Automatic activation: Disabled/i)).toBeInTheDocument();
    expect(screen.getByText(/Web research: When requested only/i)).toBeInTheDocument();
    expect(screen.getByText(/Response verbosity: detailed/i)).toBeInTheDocument();
    expect(screen.getByText(/Repository exploration: detailed/i)).toBeInTheDocument();
    expect(screen.getByText(/Verification: broad/i)).toBeInTheDocument();

    expect(screen.queryByText(/Automatic activation: Enabled/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Web research: When requested or necessary/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Response verbosity: concise/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Repository exploration: minimal/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Verification: focused/i)).not.toBeInTheDocument();

    // 5. Click Orchestrator
    await user.click(orchestratorNav);
    expect(orchestratorNav).toHaveClass("active");
    expect(rulesNav).not.toHaveClass("active");
    expect(await screen.findByRole("heading", { name: "orchestrator.settings.profiles" })).toBeInTheDocument();
    expect(screen.queryAllByRole("heading", { name: /Poverello Direct Coding/i })).toHaveLength(0);

    // 6. Return to General
    await user.click(generalNav);
    expect(generalNav).toHaveClass("active");
    expect(orchestratorNav).not.toHaveClass("active");
    expect(rulesNav).not.toHaveClass("active");
    expect(screen.getByText(/language\.header/i)).toBeInTheDocument();
    expect(screen.queryByText(/apiKeyPanel\.header/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/antigravity\.header/i)).not.toBeInTheDocument();
    expect(screen.queryAllByRole("heading", { name: /Poverello Direct Coding/i })).toHaveLength(0);
    expect(screen.queryByText("orchestrator.settings.title")).not.toBeInTheDocument();
  });
});
