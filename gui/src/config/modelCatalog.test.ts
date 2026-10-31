import { describe, expect, it } from "vitest";
import { MODEL_CATALOG, MODEL_CAPABILITIES, getProviderModels } from "../modelCapabilities";
import { getModelDisplayName } from "./modelDisplayNames";
import { getOpenRouterModelDisplayName } from "./builtinOpenRouter";
import { getOrchestratorProfileDisplayName } from "./orchestratorProfileDisplayName";

describe("shared model catalog", () => {
  it("is the sole source for MCP and Orchestrator provider model choices", () => {
    expect(MODEL_CATALOG.providers.map(({ id }) => id)).toEqual([
      "deepseek", "mimo", "zai", "minimax", "kimi", "kimi-code", "anthropic", "openrouter",
    ]);
    for (const provider of MODEL_CATALOG.providers) {
        expect(getProviderModels(provider.id)).toEqual(provider.modelIds);
        for (const modelId of provider.modelIds) {
          expect(MODEL_CATALOG.models[modelId]?.providerId).toBe(provider.id);
          expect(MODEL_CAPABILITIES[modelId]).toBeDefined();
        }
    }
  });

  it("provides canonical model labels for direct and OpenRouter display", () => {
    expect(getModelDisplayName("deepseek-v4.1-flash", "deepseek")).toBe("DeepSeek V4.1 Flash");
    expect(getModelDisplayName("MiniMax-M3", "minimax")).toBe("MiniMax M3");
    expect(getModelDisplayName("kimi-k3", "kimi")).toBe("Kimi K3");
    expect(getOpenRouterModelDisplayName("openai/gpt-5.6-sol")).toBe("GPT-5.6 Sol");
    expect(MODEL_CATALOG.models["openai/gpt-5.6-sol"].providerId).toBe("openrouter");
  });

  it("keeps catalog data limited to model facts and provider model lists", () => {
    expect(Object.keys(MODEL_CATALOG)).toEqual([
      "schemaVersion", "providers", "models", "contextWindows",
    ]);
    expect(Object.keys(MODEL_CATALOG.providers[0])).toEqual(["id", "label", "modelIds"]);
    expect(MODEL_CATALOG.models["deepseek-flash"]).not.toHaveProperty("selectable");
    expect(MODEL_CATALOG.models["deepseek-flash"]).not.toHaveProperty("proxyCapabilities");
    expect(MODEL_CATALOG.models["deepseek-flash"]).not.toHaveProperty("quickSlots");
  });

  it("uses canonical configuration name for Profile display names", () => {
    expect(getOrchestratorProfileDisplayName({
      id: "kimi-profile", displayName: "Old name", adapter: "provider", providerId: "kimi",
      model: "kimi-k3", capabilities: [],
    })).toBe("kimi-k3");
  });

  it("keeps non-cloud runtime adapters outside the static catalog", () => {
    expect(MODEL_CATALOG.providers.some(({ id }) => id === "ollama" || id === "cli")).toBe(false);
  });

  it("exposes the direct GLM models in the catalog providers list", () => {
    expect(getProviderModels("zai")).toEqual([
      "glm-5.3",
      "glm-5.3-flash",
      "glm-5.3-flashx",
      "glm-5.2",
    ]);
    expect(MODEL_CATALOG.providers.some(({ id }) => id === "zai")).toBe(true);

    expect(MODEL_CATALOG.models["glm-5.3"].providerId).toBe("zai");
    expect(MODEL_CATALOG.models["glm-5.3-flash"].providerId).toBe("zai");
    expect(MODEL_CATALOG.models["glm-5.3-flashx"].providerId).toBe("zai");
    expect(MODEL_CATALOG.contextWindows["zai:glm-5.3-flash"].context_length).toBe(1_048_576);
    expect(MODEL_CATALOG.contextWindows["zai:glm-5.3-flashx"].context_length).toBe(1_048_576);
    // The two Flash codes accept image input on the direct API; video stays
    // disabled until the official block form is confirmed.
    expect(MODEL_CAPABILITIES["glm-5.3-flash"].supports_vision).toBe(true);
    expect(MODEL_CAPABILITIES["glm-5.3-flashx"].supports_video).toBe(false);
    expect(MODEL_CAPABILITIES["glm-5.3-flashx"].reasoningEffortOptions).toEqual([
      "low",
      "high",
      "max",
    ]);
    expect(getModelDisplayName("glm-5.3", "zai")).toBe("GLM-5.3");

    // GLM-5.2 is the pay-as-you-go text model whose thinking may be turned off
    // (the 5.3 series documents reasoning as impossible to disable), and the
    // Coding Plan routes its requests to GLM-5.3 instead of serving it.
    expect(MODEL_CATALOG.models["glm-5.2"].providerId).toBe("zai");
    expect(MODEL_CATALOG.contextWindows["zai:glm-5.2"].context_length).toBe(1_000_000);
    expect(MODEL_CAPABILITIES["glm-5.2"].supports_vision).toBe(false);
    expect(MODEL_CAPABILITIES["glm-5.2"].force_thinking).toBe(false);
    expect(MODEL_CAPABILITIES["glm-5.2"].thinkingModePolicy).toBe("toggleable");
    expect(MODEL_CAPABILITIES["glm-5.2"].reasoningEffortOptions).toEqual([
      "low",
      "high",
      "max",
    ]);
    expect(getModelDisplayName("glm-5.2", "zai")).toBe("GLM-5.2");

    // The direct ID and the OpenRouter ID stay distinct entries.
    expect(MODEL_CATALOG.models["z-ai/glm-5.3"].providerId).toBe("openrouter");
    expect(MODEL_CATALOG.contextWindows["zai:glm-5.3"].context_length).toBe(1_048_576);
    expect(MODEL_CATALOG.contextWindows["openrouter:z-ai/glm-5.3"].context_length).toBe(1_048_576);

    // Thinking is mandatory and the documented effort axis is low/high/max.
    const caps = MODEL_CAPABILITIES["glm-5.3"];
    expect(caps.thinkingModePolicy).toBe("thinking_only");
    expect(caps.force_thinking).toBe(true);
    expect(caps.supportsReasoningEffort).toBe(true);
    expect(caps.reasoningEffortOptions).toEqual(["low", "high", "max"]);
    expect(MODEL_CATALOG.models["glm-5.3"].capabilities.defaultReasoningEffort).toBe("max");
    expect(MODEL_CATALOG.models["glm-5.3"].capabilities.forcedReasoningEffort).toBeUndefined();
    expect(MODEL_CATALOG.models["glm-5.3-flash"].capabilities.defaultReasoningEffort).toBe("max");
    expect(MODEL_CATALOG.models["glm-5.3-flashx"].capabilities.defaultReasoningEffort).toBe("max");
  });

  it("exposes the direct Anthropic models in the catalog providers list", () => {
    expect(getProviderModels("anthropic")).toEqual([
      "claude-opus-5-5",
      "claude-sonnet-5-5",
      "claude-haiku-5-5",
    ]);
    expect(MODEL_CATALOG.providers.some(({ id }) => id === "anthropic")).toBe(true);

    expect(MODEL_CATALOG.models["claude-opus-5-5"].providerId).toBe("anthropic");
    expect(MODEL_CATALOG.models["claude-sonnet-5-5"].providerId).toBe("anthropic");
    expect(MODEL_CATALOG.models["claude-haiku-5-5"].providerId).toBe("anthropic");

    expect(getModelDisplayName("claude-opus-5-5", "anthropic")).toBe("Claude Opus 5.5");
    expect(getModelDisplayName("claude-sonnet-5-5", "anthropic")).toBe("Claude Sonnet 5.5");
    expect(getModelDisplayName("claude-haiku-5-5", "anthropic")).toBe("Claude Haiku 5.5");

    expect(MODEL_CAPABILITIES["claude-opus-5-5"].thinkingModePolicy).toBe("thinking_only");
    expect(MODEL_CAPABILITIES["claude-sonnet-5-5"].thinkingModePolicy).toBe("toggleable");
    expect(MODEL_CAPABILITIES["claude-haiku-5-5"].thinkingModePolicy).toBe("toggleable");
    expect(MODEL_CAPABILITIES["claude-opus-5-5"].reasoningEffortOptions).toEqual([
      "low",
      "medium",
      "high",
      "xhigh",
      "max",
    ]);
  });

  it("defines MiniMax-M3.1-Flash and Kimi models according to v0.25.3 specifications", () => {
    // MiniMax-M3.1-Flash
    const m31 = MODEL_CATALOG.models["MiniMax-M3.1-Flash"];
    expect(m31).toBeDefined();
    expect(m31.providerId).toBe("minimax");
    expect(MODEL_CATALOG.contextWindows["minimax:MiniMax-M3.1-Flash"].context_length).toBe(1_048_576);
    expect(m31.capabilities.thinkingPolicy).toBe("thinking_only");
    expect(m31.capabilities.forceThinking).toBe(true);
    expect(m31.capabilities.supportsReasoningEffort).toBe(true);
    expect(m31.capabilities.reasoningEffortOptions).toEqual(["low", "medium", "high", "xhigh", "max"]);
    expect(m31.capabilities.defaultReasoningEffort).toBe("max");
    expect(m31.capabilities.forcedReasoningEffort).toBeUndefined();

    // k3.1-pro (Kimi Code API)
    const k31Code = MODEL_CATALOG.models["k3.1-pro"];
    expect(k31Code).toBeDefined();
    expect(k31Code.providerId).toBe("kimi-code");
    expect(MODEL_CATALOG.contextWindows["kimi-code:k3.1-pro"].context_length).toBe(262_144);
    expect(k31Code.capabilities.thinkingPolicy).toBe("thinking_only");
    expect(k31Code.capabilities.forceThinking).toBe(true);
    expect(k31Code.capabilities.supportsReasoningEffort).toBe(true);
    expect(k31Code.capabilities.reasoningEffortOptions).toEqual(["low", "high", "max"]);
    expect(k31Code.capabilities.defaultReasoningEffort).toBe("high");
    expect(k31Code.capabilities.forcedReasoningEffort).toBeUndefined();

    // k3.1-pro-highspeed (Kimi Code API)
    const k31CodeHs = MODEL_CATALOG.models["k3.1-pro-highspeed"];
    expect(k31CodeHs).toBeDefined();
    expect(k31CodeHs.providerId).toBe("kimi-code");
    expect(MODEL_CATALOG.contextWindows["kimi-code:k3.1-pro-highspeed"].context_length).toBe(1_048_576);
    expect(k31CodeHs.capabilities.thinkingPolicy).toBe("thinking_only");
    expect(k31CodeHs.capabilities.forceThinking).toBe(true);
    expect(k31CodeHs.capabilities.supportsReasoningEffort).toBe(true);
    expect(k31CodeHs.capabilities.reasoningEffortOptions).toEqual(["low", "high", "max"]);
    expect(k31CodeHs.capabilities.defaultReasoningEffort).toBe("high");
    expect(k31CodeHs.capabilities.forcedReasoningEffort).toBeUndefined();

    // ID separation between kimi (Moonshot) and kimi-code
    const k31Moonshot = MODEL_CATALOG.models["kimi-k3.1-pro"];
    expect(k31Moonshot).toBeDefined();
    expect(k31Moonshot.providerId).toBe("kimi");
    expect(k31Moonshot.capabilities.thinkingPolicy).toBe("thinking_only");
    expect(k31Moonshot.capabilities.forceThinking).toBe(true);
    expect(k31Code.providerId).toBe("kimi-code");
    expect(k31Moonshot).not.toBe(k31Code);
  });
});
