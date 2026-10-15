import { describe, expect, it } from "vitest";
import { calculateDashboardCardCount, getVisibleOpenRouterProfiles } from "./dashboardTiles";
import { buildTiles } from "./components/ProviderTiles";

function provider() {
  return { profiles: undefined };
}

describe("dashboard tile card rules", () => {
  it("counts each non-OpenRouter provider once", () => {
    expect(calculateDashboardCardCount({
      providers: {
        deepseek: provider(),
        mimo: provider(),
        minimax: provider(),
        kimi: provider(),
      },
    })).toBe(4);
  });

  it("uses one fallback card for absent or empty OpenRouter profiles", () => {
    expect(calculateDashboardCardCount({
      providers: { openrouter: provider() },
    })).toBe(1);
    expect(calculateDashboardCardCount({
      providers: { openrouter: { profiles: [] } },
    })).toBe(1);
  });

  it("does not use fallback when all existing profiles are hidden", () => {
    const profiles = [{ hidden: true }, { hidden: true }];
    expect(getVisibleOpenRouterProfiles(profiles)).toEqual([]);
    expect(calculateDashboardCardCount({
      providers: { openrouter: { profiles } },
    })).toBe(0);
  });

  it("keeps profiles without hidden true visible", () => {
    expect(getVisibleOpenRouterProfiles([{}, { hidden: false }, { hidden: true }])).toEqual([
      {},
      { hidden: false },
    ]);
  });

  it("counts the seven-card fixture used by the dashboard", () => {
    const config = {
      active_provider: "deepseek",
      active_openrouter_profile_id: null,
      providers: {
        deepseek: provider(),
        mimo: provider(),
        minimax: provider(),
        kimi: provider(),
        openrouter: {
          profiles: [
            { hidden: false },
            {},
            { hidden: false },
            { hidden: true },
          ],
        },
      },
      server: { host: "127.0.0.1", port: 4000, enable_cors: false },
    };

    expect(calculateDashboardCardCount(config)).toBe(7);
    expect(buildTiles(config as never)).toHaveLength(7);
  });

  it("excludes hidden direct providers from the card count", () => {
    expect(calculateDashboardCardCount({
      providers: {
        deepseek: { hidden: true },
        mimo: provider(),
        minimax: provider(),
        kimi: provider(),
      },
    })).toBe(3);
  });

  it("omits a hidden direct provider tile", () => {
    const config = {
      active_provider: "deepseek",
      active_openrouter_profile_id: null,
      providers: {
        deepseek: { hidden: true, display_name: "DeepSeek", models: {} },
        mimo: { display_name: "MiMo", models: {} },
        minimax: { display_name: "MiniMax", models: {} },
        kimi: { display_name: "Kimi", models: {} },
        openrouter: { profiles: [] },
      },
      server: { host: "127.0.0.1", port: 4000, enable_cors: false },
    };

    const tiles = buildTiles(config as never);
    // 3 visible direct providers + 1 OpenRouter fallback tile.
    expect(tiles).toHaveLength(4);
    expect(tiles.some((t) => t.providerId === "deepseek")).toBe(false);
  });

  it("returns null for fallback and an empty array for all-hidden profiles", () => {
    expect(getVisibleOpenRouterProfiles(undefined)).toBeNull();
    expect(getVisibleOpenRouterProfiles([])).toBeNull();
    expect(getVisibleOpenRouterProfiles([{ hidden: true }])).toEqual([]);
  });

  it("counts Ollama Local card when visible on dashboard", () => {
    const config = {
      providers: {
        deepseek: provider(),
        mimo: provider(),
      },
      claude_code: {
        third_party_provider: {
          show_on_dashboard: true,
          provider: "ollama",
          model: "qwen2.5-coder:32b",
        },
      },
    };

    expect(calculateDashboardCardCount(config as never)).toBe(3);
  });

  it("omits Ollama Local card when show_on_dashboard is false", () => {
    const config = {
      providers: {
        deepseek: provider(),
        mimo: provider(),
      },
      claude_code: {
        third_party_provider: {
          show_on_dashboard: false,
          provider: "ollama",
          model: "qwen2.5-coder:32b",
        },
      },
    };

    expect(calculateDashboardCardCount(config as never)).toBe(2);
  });

  it("sets active state based on claude_code.active_route", () => {
    const configOllamaActive = {
      active_provider: "deepseek",
      active_openrouter_profile_id: null,
      providers: {
        deepseek: { display_name: "DeepSeek", models: {} },
        mimo: { display_name: "MiMo", models: {} },
      },
      claude_code: {
        active_route: "ollama",
        third_party_provider: {
          show_on_dashboard: true,
          provider: "ollama",
          model: "gemma4:latest",
        },
      },
      server: { host: "127.0.0.1", port: 4000, enable_cors: false },
    };

    const tilesOllama = buildTiles(configOllamaActive as never);
    expect(tilesOllama).toHaveLength(3);
    const ollamaTile = tilesOllama.find((t) => t.providerId === "ollama");
    const deepseekTile = tilesOllama.find((t) => t.providerId === "deepseek");
    expect(ollamaTile?.isActive).toBe(true);
    expect(deepseekTile?.isActive).toBe(false);

    const configGatewayActive = {
      ...configOllamaActive,
      claude_code: {
        ...configOllamaActive.claude_code,
        active_route: "gateway",
      },
    };
    const tilesGateway = buildTiles(configGatewayActive as never);
    expect(tilesGateway.find((t) => t.providerId === "ollama")?.isActive).toBe(false);
    expect(tilesGateway.find((t) => t.providerId === "deepseek")?.isActive).toBe(true);
  });
});

describe("gateway profiles dashboard rules", () => {
  it("counts visible gateway profiles and excludes hidden ones", () => {
    const config = {
      providers: {
        deepseek: provider(),
      },
      gateway_profiles: [
        { hidden: false },
        { hidden: true },
        { hidden: false },
      ],
    };

    expect(calculateDashboardCardCount(config as never)).toBe(3); // 1 direct provider + 2 visible gateway profiles
  });

  it("builds gateway profile tiles with resolved routes and active state", () => {
    const config = {
      active_provider: "gateway_profile",
      active_gateway_profile_id: "gp-1",
      providers: {
        deepseek: { display_name: "DeepSeek", models: {} },
        openrouter: {
          profiles: [
            {
              id: "or-p1",
              display_name: "OR Set 1",
              models: {
                "claude-sonnet-5": {
                  upstream_model: "anthropic/claude-3.7-sonnet",
                  thinking_mode: "thinking",
                  reasoning_effort: "high",
                },
              },
            },
          ],
        },
      },
      gateway_profiles: [
        {
          id: "gp-1",
          display_name: "Multi 1",
          hidden: false,
          routes: {
            "claude-opus-5": {
              provider: "anthropic",
              model: "claude-opus-5-5",
              thinking_mode: "thinking",
            },
            "claude-sonnet-5": {
              provider: "openrouter",
              profile_id: "or-p1",
            },
            "claude-haiku-4-5": {
              provider: "zai",
              model: "glm-5.2",
              thinking_mode: "normal",
            },
          },
        },
        {
          id: "gp-hidden",
          display_name: "Hidden Profile",
          hidden: true,
          routes: {},
        },
      ],
    };

    const tiles = buildTiles(config as never);
    const gpTile = tiles.find((t) => t.providerId === "gateway_profile");
    expect(gpTile).toBeDefined();
    expect(gpTile?.profileId).toBe("gp-1");
    expect(gpTile?.displayName).toBe("Multi 1");
    expect(gpTile?.isActive).toBe(true);
    expect(gpTile?.opusUpstream).toBe("claude-opus-5-5");
    expect(gpTile?.opusThinkingMode).toBe("thinking");
    expect(gpTile?.sonnetUpstream).toBe("anthropic/claude-3.7-sonnet");
    expect(gpTile?.sonnetThinkingMode).toBe("thinking");
    expect(gpTile?.sonnetReasoningEffort).toBe("high");
    expect(gpTile?.haikuUpstream).toBe("glm-5.2");
    expect(gpTile?.haikuThinkingMode).toBe("normal");

    // Hidden profile is not rendered
    expect(tiles.some((t) => t.profileId === "gp-hidden")).toBe(false);
  });
});

// ProviderTiles uses the same shared profile helper as calculateDashboardCardCount.

describe("shared dashboard profile rules", () => {
  it("keeps a visible profile fixture aligned with the tile builder", () => {
    const profiles = [{ hidden: false }, {}, { hidden: true }];
    expect(getVisibleOpenRouterProfiles(profiles)).toHaveLength(2);
  });
});
