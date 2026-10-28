import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import TitleBar from "./TitleBar";
import { version } from "../../package.json";
import tauriConf from "../../src-tauri/tauri.conf.json";

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    title: vi.fn().mockResolvedValue("Poverello"),
    isMaximized: vi.fn().mockResolvedValue(false),
    onResized: vi.fn().mockResolvedValue(() => {}),
    minimize: vi.fn().mockResolvedValue(undefined),
    toggleMaximize: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined),
  }),
}));

describe("TitleBar Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders branding, supported top navigation tabs, and version info", () => {
    render(
      <TitleBar
        activeTab="gateway"
        onTabChange={vi.fn()}
      />
    );

    // App Branding
    expect(screen.getByText("Poverello")).toBeInTheDocument();

    // Tabs
    expect(screen.getByRole("button", { name: /Gateway for Claude Desktop/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /MCP for Antigravity/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Antigravity Rules/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Orchestrator/i })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /header\.settings/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Auto-Converge/i })).not.toBeInTheDocument();

    // Version
    expect(screen.getByText(`v${version}`)).toBeInTheDocument();

    // No old right-side button or close toggle
    expect(screen.queryByRole("button", { name: /header\.settingsClose/i })).not.toBeInTheDocument();
  });

  it("triggers onTabChange when each tab is clicked", async () => {
    const user = userEvent.setup();
    const onTabChange = vi.fn();
    render(
      <TitleBar
        activeTab="gateway"
        onTabChange={onTabChange}
      />
    );

    const mcpTab = screen.getByRole("button", { name: /MCP for Antigravity/i });
    await user.click(mcpTab);
    expect(onTabChange).toHaveBeenCalledWith("mcp");

    const settingsTab = screen.getByRole("button", { name: /header\.settings/i });
    await user.click(settingsTab);
    expect(onTabChange).toHaveBeenCalledWith("settings");

    const gatewayTab = screen.getByRole("button", { name: /Gateway for Claude Desktop/i });
    await user.click(gatewayTab);
    expect(onTabChange).toHaveBeenCalledWith("gateway");
  });

  it("keeps the Auto-Converge tab hidden even when its retained route is active", () => {
    render(
      <TitleBar
        activeTab="autoConverge"
        onTabChange={vi.fn()}
      />
    );

    expect(screen.queryByRole("button", { name: /Auto-Converge/i })).not.toBeInTheDocument();
  });

  it("applies active class to supported tabs while keeping them visible", () => {
    const { rerender } = render(
      <TitleBar
        activeTab="gateway"
        onTabChange={vi.fn()}
      />
    );

    const gatewayBtn = screen.getByRole("button", { name: /Gateway for Claude Desktop/i });
    const mcpBtn = screen.getByRole("button", { name: /MCP for Antigravity/i });
    const settingsBtn = screen.getByRole("button", { name: /header\.settings/i });

    expect(gatewayBtn).toHaveClass("titlebar-tab-active");
    expect(mcpBtn).not.toHaveClass("titlebar-tab-active");
    expect(settingsBtn).not.toHaveClass("titlebar-tab-active");

    // Switch to settings
    rerender(
      <TitleBar
        activeTab="settings"
        onTabChange={vi.fn()}
      />
    );

    expect(gatewayBtn).not.toHaveClass("titlebar-tab-active");
    expect(mcpBtn).not.toHaveClass("titlebar-tab-active");
    expect(settingsBtn).toHaveClass("titlebar-tab-active");

    // All tabs remain in the document
    expect(gatewayBtn).toBeInTheDocument();
    expect(mcpBtn).toBeInTheDocument();
    expect(settingsBtn).toBeInTheDocument();
  });

  it("renders window controls (minimize, maximize, close)", () => {
    render(
      <TitleBar
        activeTab="gateway"
        onTabChange={vi.fn()}
      />
    );

    expect(screen.getByRole("button", { name: /最小化/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /最大化/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /閉じる/i })).toBeInTheDocument();
  });

  it("keeps tauri.conf.json version consistent with package.json", () => {
    expect(tauriConf.version).toBe(version);
  });
});

