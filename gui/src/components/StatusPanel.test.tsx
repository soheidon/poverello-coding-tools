import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import StatusPanel from "./StatusPanel";
import { LanguageProvider } from "../i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "check_all_api_keys") {
      return {
        kimi: { set: true },
        deepseek: { set: true },
        kimi_code: { set: false },
        glm: { set: true },
        minimax: { set: true },
        mimo: { set: true },
        anthropic: { set: true },
        openrouter: { set: true },
      };
    }
    if (cmd === "read_config") {
      return {
        active_provider: "deepseek",
        providers: {
          kimi: { display_name: "Kimi" },
          deepseek: { display_name: "DeepSeek" },
          kimi_code: { display_name: "Kimi Code" },
          glm: { display_name: "GLM" },
          minimax: { display_name: "MiniMax" },
          mimo: { display_name: "MiMo" },
          anthropic: { display_name: "Anthropic" },
          openrouter: { display_name: "OpenRouter" },
        },
      };
    }
    return null;
  }),
}));

vi.mock("./OpenRouterModelSelector", () => ({
  getOpenRouterModelsCached: vi.fn().mockResolvedValue({ models: [] }),
  parsePerMillionUsd: vi.fn().mockReturnValue(null),
}));

describe("StatusPanel Component", () => {
  it("renders Port 4000 and Gateway URL with status-card-compact and API key card with standard flex", async () => {
    const { container } = render(
      <LanguageProvider>
        <StatusPanel
          health={{
            reachable: false,
            port_listening: false,
            checked_at: new Date().toISOString(),
            error: null,
            managed_child_running: false,
            managed_child_pid: null,
            diagnostic: "Not listening",
          }}
          healthError={null}
          healthLoading={false}
        />
      </LanguageProvider>
    );

    const cards = container.querySelectorAll(".status-card");
    expect(cards).toHaveLength(3);

    // Card 1: Port 4000
    expect(cards[0]).toHaveClass("status-card-compact");

    // Card 2: Gateway URL
    expect(cards[1]).toHaveClass("status-card-compact");
    const urlValue = cards[1].querySelector(".status-card-value");
    expect(urlValue).toHaveStyle({ whiteSpace: "nowrap" });

    // Card 3: API keys
    expect(cards[2]).not.toHaveClass("status-card-compact");

    // All 8 API keys should appear
    expect(await screen.findByText(/Kimi: ✓/)).toBeInTheDocument();
    expect(await screen.findByText(/DeepSeek: ✓/)).toBeInTheDocument();
    expect(await screen.findByText(/Kimi Code: ✗/)).toBeInTheDocument();
    expect(await screen.findByText(/GLM: ✓/)).toBeInTheDocument();
    expect(await screen.findByText(/MiniMax: ✓/)).toBeInTheDocument();
    expect(await screen.findByText(/MiMo: ✓/)).toBeInTheDocument();
    expect(await screen.findByText(/Anthropic: ✓/)).toBeInTheDocument();
    expect(await screen.findByText(/OpenRouter: ✓/)).toBeInTheDocument();
  });
});
