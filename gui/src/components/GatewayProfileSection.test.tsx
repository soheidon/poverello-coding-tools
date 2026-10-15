import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import GatewayProfileSection from "./GatewayProfileSection";
import type { GatewayConfig, GatewayProfile } from "../types";

const invokeMock = vi.mocked(invoke);
const confirmMock = vi.fn();

const testProfile: GatewayProfile = {
  id: "gp-test-1",
  display_name: "Test Profile 1",
  hidden: false,
  routes: {
    "claude-opus-5": {
      provider: "anthropic",
      model: "claude-opus-5-5",
      thinking_mode: "thinking",
      reasoning_effort: null,
    },
    "claude-sonnet-5": {
      provider: "deepseek",
      model: "deepseek-v4-flash",
      thinking_mode: "normal",
      reasoning_effort: null,
    },
    "claude-haiku-4-5": {
      provider: "zai",
      model: "glm-5.2",
      thinking_mode: "normal",
      reasoning_effort: null,
    },
  },
};

const testConfig: GatewayConfig = {
  active_provider: "gateway_profile",
  active_gateway_profile_id: "gp-test-1",
  active_openrouter_profile_id: null,
  gateway_profiles: [testProfile],
  providers: {
    deepseek: {
      display_name: "DeepSeek",
      upstream_url: "https://api.deepseek.com",
      api_key_env: "DEEPSEEK_API_KEY",
      default_model: "deepseek-chat",
      force_anthropic_version: null,
      supports_count_tokens: false,
      supports_vision: false,
      supports_video: false,
      supports_thinking: true,
      model_map: {},
      visible_models: [],
      models: {},
    },
    openrouter: {
      display_name: "OpenRouter",
      upstream_url: "https://openrouter.ai/api/v1",
      api_key_env: "OPENROUTER_API_KEY",
      default_model: "openrouter/auto",
      force_anthropic_version: null,
      supports_count_tokens: false,
      supports_vision: false,
      supports_video: false,
      supports_thinking: false,
      model_map: {},
      visible_models: [],
      models: {},
      profiles: [
        {
          id: "or-p1",
          display_name: "OR Profile 1",
          model_map: {},
          visible_models: [],
          models: {
            "claude-opus-5": {
              upstream_model: "anthropic/claude-3.7-sonnet",
              thinking_mode: "thinking",
            },
            "claude-sonnet-5": {
              upstream_model: "anthropic/claude-3.7-sonnet",
              thinking_mode: "thinking",
            },
            "claude-haiku-4-5": {
              upstream_model: "anthropic/claude-3.5-haiku",
              thinking_mode: "normal",
            },
          },
        },
      ],
    },
  },
  server: {
    host: "127.0.0.1",
    port: 4000,
    enable_cors: false,
  },
};

describe("GatewayProfileSection component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    confirmMock.mockReturnValue(true);
    vi.stubGlobal("confirm", confirmMock);
    invokeMock.mockResolvedValue({
      value: null,
      restartGateway: false,
      restartReason: "",
    });
  });

  it("renders profile list with active badge", () => {
    render(
      <GatewayProfileSection
        config={testConfig}
        gatewayRunning={false}
        refreshConfig={vi.fn().mockResolvedValue(undefined)}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    expect(screen.getByText("Multi-Provider Gateway Profiles")).toBeInTheDocument();
    expect(screen.getByText("Test Profile 1")).toBeInTheDocument();
    expect(screen.getByText("apiKeyPanel.badgeActive")).toBeInTheDocument();
  });

  it("creates a new profile with valid routes and calls save_gateway_profile", async () => {
    const refreshConfig = vi.fn().mockResolvedValue(undefined);
    render(
      <GatewayProfileSection
        config={testConfig}
        gatewayRunning={false}
        refreshConfig={refreshConfig}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    const addBtn = screen.getByRole("button", { name: /\+ Add Profile/i });
    fireEvent.click(addBtn);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_gateway_profile",
        expect.objectContaining({
          profile: expect.objectContaining({
            display_name: "Gateway Profile 2",
            routes: expect.objectContaining({
              "claude-opus-5": expect.objectContaining({ provider: "deepseek", model: "deepseek-v4-pro" }),
              "claude-sonnet-5": expect.objectContaining({ provider: "deepseek", model: "deepseek-v4-flash" }),
              "claude-haiku-4-5": expect.objectContaining({ provider: "deepseek", model: "deepseek-chat" }),
            }),
          }),
        }),
      );
      expect(refreshConfig).toHaveBeenCalled();
    });
  });

  it("uses the Rust profile_id field and only exposes allowlisted GLM models", async () => {
    render(
      <GatewayProfileSection
        config={testConfig}
        gatewayRunning={false}
        refreshConfig={vi.fn().mockResolvedValue(undefined)}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "apiKeyPanel.edit" }));
    const selects = await screen.findAllByRole("combobox");
    await userEvent.selectOptions(selects[0], "zai");
    expect(Array.from((selects[1] as HTMLSelectElement).options).map((option) => option.value)).toEqual([
      "glm-5.2",
      "glm-5.3",
      "glm-5.3-flash",
      "glm-5.3-flashx",
    ]);
    expect(screen.queryByRole("option", { name: "glm-5.2-flash" })).not.toBeInTheDocument();

    await userEvent.selectOptions(selects[0], "openrouter");
    const refreshedSelects = screen.getAllByRole("combobox");
    expect(refreshedSelects[1]).toHaveValue("or-p1");
    fireEvent.click(screen.getByRole("button", { name: "apiKeyPanel.save" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_gateway_profile",
        expect.objectContaining({
          profile: expect.objectContaining({
            routes: expect.objectContaining({
              "claude-opus-5": expect.objectContaining({ provider: "openrouter", profile_id: "or-p1" }),
            }),
          }),
        }),
      );
    });
  });

  it("toggles dashboard visibility via set_gateway_profile_hidden", async () => {
    const refreshConfig = vi.fn().mockResolvedValue(undefined);
    render(
      <GatewayProfileSection
        config={testConfig}
        gatewayRunning={false}
        refreshConfig={refreshConfig}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    const checkbox = screen.getByRole("checkbox");
    expect(checkbox).toBeChecked();

    fireEvent.click(checkbox);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_gateway_profile_hidden", {
        profileId: "gp-test-1",
        hidden: true,
      });
      expect(refreshConfig).toHaveBeenCalled();
    });
  });

  it("duplicates profile via duplicate_gateway_profile", async () => {
    const refreshConfig = vi.fn().mockResolvedValue(undefined);
    render(
      <GatewayProfileSection
        config={testConfig}
        gatewayRunning={false}
        refreshConfig={refreshConfig}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    const dupBtn = screen.getByRole("button", { name: /Duplicate/i });
    fireEvent.click(dupBtn);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("duplicate_gateway_profile", {
        profileId: "gp-test-1",
      });
      expect(refreshConfig).toHaveBeenCalled();
    });
  });

  it("deletes profile via delete_gateway_profile after confirmation", async () => {
    const refreshConfig = vi.fn().mockResolvedValue(undefined);
    render(
      <GatewayProfileSection
        config={testConfig}
        gatewayRunning={false}
        refreshConfig={refreshConfig}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    const delBtn = screen.getByRole("button", { name: /Delete/i });
    fireEvent.click(delBtn);

    expect(confirmMock).toHaveBeenCalled();
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("delete_gateway_profile", {
        profileId: "gp-test-1",
      });
      expect(refreshConfig).toHaveBeenCalled();
    });
  });

  it("expands edit area, allows modifying routes, and saves", async () => {
    const refreshConfig = vi.fn().mockResolvedValue(undefined);
    render(
      <GatewayProfileSection
        config={testConfig}
        gatewayRunning={false}
        refreshConfig={refreshConfig}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    const editBtn = screen.getByRole("button", { name: "apiKeyPanel.edit" });
    fireEvent.click(editBtn);

    // Edit area should be open showing 3 routes
    expect(screen.getByText("apiKeyPanel.gatewayPro")).toBeInTheDocument();
    expect(screen.getByText("apiKeyPanel.gatewayFlash")).toBeInTheDocument();
    expect(screen.getByText("apiKeyPanel.gatewayHaiku")).toBeInTheDocument();

    const saveBtn = screen.getByRole("button", { name: "apiKeyPanel.save" });
    fireEvent.click(saveBtn);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_gateway_profile",
        expect.objectContaining({
          profile: expect.objectContaining({
            id: "gp-test-1",
            display_name: "Test Profile 1",
          }),
        }),
      );
      expect(refreshConfig).toHaveBeenCalled();
    });
  });

  it("restarts gateway when save returns restartGateway=true", async () => {
    invokeMock.mockResolvedValueOnce({
      value: null,
      restartGateway: true,
      restartReason: "active_profile_updated",
    });
    const restartGateway = vi.fn().mockResolvedValue(undefined);

    render(
      <GatewayProfileSection
        config={testConfig}
        gatewayRunning={true}
        refreshConfig={vi.fn().mockResolvedValue(undefined)}
        restartGateway={restartGateway}
      />,
    );

    const editBtn = screen.getByRole("button", { name: "apiKeyPanel.edit" });
    fireEvent.click(editBtn);

    const saveBtn = screen.getByRole("button", { name: "apiKeyPanel.save" });
    fireEvent.click(saveBtn);

    await waitFor(() => {
      expect(restartGateway).toHaveBeenCalled();
    });
  });

  it("allows normal mode for glm-5.2, but hides normal and defaults to thinking for glm-5.3 series", async () => {
    render(
      <GatewayProfileSection
        config={testConfig}
        gatewayRunning={false}
        refreshConfig={vi.fn().mockResolvedValue(undefined)}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "apiKeyPanel.edit" }));
    const routeHaiku = screen.getByText("apiKeyPanel.gatewayHaiku").parentElement!;
    const selects = within(routeHaiku).getAllByRole("combobox");
    expect(selects[1]).toHaveValue("glm-5.2");
    const modeOptions = Array.from((selects[2] as HTMLSelectElement).options).map((o) => o.value);
    expect(modeOptions).toEqual(["normal", "thinking"]);

    // Change model to glm-5.3
    await userEvent.selectOptions(selects[1], "glm-5.3");
    const updatedSelects = within(routeHaiku).getAllByRole("combobox");
    const updatedModeOptions = Array.from((updatedSelects[2] as HTMLSelectElement).options).map((o) => o.value);
    expect(updatedModeOptions).toEqual(["thinking"]);
    expect(updatedSelects[2]).toHaveValue("thinking");

    // Change model to glm-5.3-flash
    await userEvent.selectOptions(updatedSelects[1], "glm-5.3-flash");
    const flashSelects = within(routeHaiku).getAllByRole("combobox");
    expect(Array.from((flashSelects[2] as HTMLSelectElement).options).map((o) => o.value)).toEqual(["thinking"]);
    expect(flashSelects[2]).toHaveValue("thinking");
  });

  it("reconciles normal thinking_mode to thinking when switching model from glm-5.2 to glm-5.3", async () => {
    render(
      <GatewayProfileSection
        config={testConfig}
        gatewayRunning={false}
        refreshConfig={vi.fn().mockResolvedValue(undefined)}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "apiKeyPanel.edit" }));
    const routeHaiku = screen.getByText("apiKeyPanel.gatewayHaiku").parentElement!;
    const selects = within(routeHaiku).getAllByRole("combobox");
    expect(selects[1]).toHaveValue("glm-5.2");
    expect(selects[2]).toHaveValue("normal");

    // Switch to glm-5.3
    await userEvent.selectOptions(selects[1], "glm-5.3");

    fireEvent.click(screen.getByRole("button", { name: "apiKeyPanel.save" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_gateway_profile",
        expect.objectContaining({
          profile: expect.objectContaining({
            routes: expect.objectContaining({
              "claude-haiku-4-5": expect.objectContaining({
                provider: "zai",
                model: "glm-5.3",
                thinking_mode: "thinking",
              }),
            }),
          }),
        }),
      );
    });
  });

  it("repairs a persisted invalid GLM normal mode when opening the profile editor", async () => {
    const configWithLegacyMode: GatewayConfig = JSON.parse(JSON.stringify(testConfig));
    configWithLegacyMode.gateway_profiles![0].routes["claude-haiku-4-5"] = {
      provider: "zai",
      model: "glm-5.3",
      thinking_mode: "normal",
      reasoning_effort: null,
    };

    render(
      <GatewayProfileSection
        config={configWithLegacyMode}
        gatewayRunning={false}
        refreshConfig={vi.fn().mockResolvedValue(undefined)}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "apiKeyPanel.edit" }));
    const routeHaiku = screen.getByText("apiKeyPanel.gatewayHaiku").parentElement!;
    expect(within(routeHaiku).getAllByRole("combobox")[2]).toHaveValue("thinking");

    fireEvent.click(screen.getByRole("button", { name: "apiKeyPanel.save" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_gateway_profile",
        expect.objectContaining({
          profile: expect.objectContaining({
            routes: expect.objectContaining({
              "claude-haiku-4-5": expect.objectContaining({
                provider: "zai",
                model: "glm-5.3",
                thinking_mode: "thinking",
              }),
            }),
          }),
        }),
      );
    });
  });

  it("handles GLM reasoning effort options (unset, low, high, max)", async () => {
    render(
      <GatewayProfileSection
        config={testConfig}
        gatewayRunning={false}
        refreshConfig={vi.fn().mockResolvedValue(undefined)}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "apiKeyPanel.edit" }));
    const routeHaiku = screen.getByText("apiKeyPanel.gatewayHaiku").parentElement!;
    const selects = within(routeHaiku).getAllByRole("combobox");
    const effortOptions = Array.from((selects[3] as HTMLSelectElement).options).map((o) => o.value);
    expect(effortOptions).toEqual(["", "low", "high", "max"]);

    // Select max
    await userEvent.selectOptions(selects[3], "max");

    fireEvent.click(screen.getByRole("button", { name: "apiKeyPanel.save" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_gateway_profile",
        expect.objectContaining({
          profile: expect.objectContaining({
            routes: expect.objectContaining({
              "claude-haiku-4-5": expect.objectContaining({
                provider: "zai",
                reasoning_effort: "max",
              }),
            }),
          }),
        }),
      );
    });
  });

  it("clears invalid provider-specific effort when switching providers", async () => {
    render(
      <GatewayProfileSection
        config={testConfig}
        gatewayRunning={false}
        refreshConfig={vi.fn().mockResolvedValue(undefined)}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "apiKeyPanel.edit" }));
    const routeHaiku = screen.getByText("apiKeyPanel.gatewayHaiku").parentElement!;
    let selects = within(routeHaiku).getAllByRole("combobox");

    // Set effort to max on GLM
    await userEvent.selectOptions(selects[3], "max");

    // Switch provider to DeepSeek (max is invalid for DeepSeek)
    await userEvent.selectOptions(selects[0], "deepseek");
    selects = within(routeHaiku).getAllByRole("combobox");
    expect(selects[3]).toHaveValue("");

    // Set effort to medium on DeepSeek
    await userEvent.selectOptions(selects[3], "medium");

    // Switch provider to zai (medium is invalid for GLM)
    await userEvent.selectOptions(selects[0], "zai");
    selects = within(routeHaiku).getAllByRole("combobox");
    expect(selects[3]).toHaveValue("");

    // Set effort to low on GLM (low is valid for both GLM and DeepSeek)
    await userEvent.selectOptions(selects[3], "low");
    await userEvent.selectOptions(selects[0], "deepseek");
    selects = within(routeHaiku).getAllByRole("combobox");
    expect(selects[3]).toHaveValue("low");
  });

  it("rejects GLM-specific invalid configurations on the frontend before save", async () => {
    const invalidGLMConfig: GatewayConfig = {
      ...testConfig,
      gateway_profiles: [
        {
          ...testProfile,
          routes: {
            ...testProfile.routes,
            "claude-haiku-4-5": {
              provider: "zai",
              model: "glm-5.3",
              thinking_mode: "normal",
              reasoning_effort: "medium",
            },
          },
        },
      ],
    };

    render(
      <GatewayProfileSection
        config={invalidGLMConfig}
        gatewayRunning={false}
        refreshConfig={vi.fn().mockResolvedValue(undefined)}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "apiKeyPanel.edit" }));
    fireEvent.click(screen.getByRole("button", { name: "apiKeyPanel.save" }));

    expect(
      screen.getByText("GLM route claude-haiku-4-5 has unsupported reasoning effort medium"),
    ).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("save_gateway_profile", expect.anything());
  });

  it("preserves DeepSeek effort options (unset, low, medium, high) and behavior", async () => {
    render(
      <GatewayProfileSection
        config={testConfig}
        gatewayRunning={false}
        refreshConfig={vi.fn().mockResolvedValue(undefined)}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "apiKeyPanel.edit" }));
    const routeFlash = screen.getByText("apiKeyPanel.gatewayFlash").parentElement!;
    const selects = within(routeFlash).getAllByRole("combobox");
    expect(selects[0]).toHaveValue("deepseek");
    const effortOptions = Array.from((selects[3] as HTMLSelectElement).options).map((o) => o.value);
    expect(effortOptions).toEqual(["", "low", "medium", "high"]);
    expect(effortOptions).not.toContain("max");

    await userEvent.selectOptions(selects[3], "medium");
    fireEvent.click(screen.getByRole("button", { name: "apiKeyPanel.save" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_gateway_profile",
        expect.objectContaining({
          profile: expect.objectContaining({
            routes: expect.objectContaining({
              "claude-sonnet-5": expect.objectContaining({
                provider: "deepseek",
                reasoning_effort: "medium",
              }),
            }),
          }),
        }),
      );
    });
  });
});
