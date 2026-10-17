// Compatibility facade over the shared model catalog. Add static model facts
// only in src/shared/model_catalog.json; Rust and both MCP/Orchestrator UIs
// consume that same data.

export type ThinkingModePolicy = "toggleable" | "thinking_only" | "forced" | "unknown" | "none" | "optional";

export interface ModelCapabilities {
  supports_vision: boolean;
  supports_video: boolean;
  supports_image_url: boolean;
  supports_image_base64: boolean;
  supports_video_url: boolean;
  supports_video_base64: boolean;
  force_thinking: boolean;
  thinking: string; // "default" | "disabled"
  thinkingModePolicy: ThinkingModePolicy;
  supportsReasoningEffort: boolean;
  suppressThinkingParameter?: boolean; // K3: do not send thinking parameter upstream
  forcedReasoningEffort?: "max";       // K3: max is the only allowed effort
  forcedThinkingOptions?: ThinkingOption[]; // OpenRouter: explicit options for "forced" models
  /** Allowed reasoning-effort values while Thinking mode is enabled. */
  reasoningEffortOptions?: ReasoningEffortOption[];
  reasoning?: ModelReasoningCapabilities;
}

import modelCatalog from "./shared/model_catalog.json";

export type ThinkingOption = "max" | "on" | "off" | "minimal" | "low" | "medium" | "high" | "xhigh";

// Values that may be sent as `reasoning_effort` upstream. Deliberately excludes
// "on"/"off" (thinking toggles, not effort levels) so they cannot leak into the
// reasoning_effort request field. "none" is supported by certain OpenAI models.
export type ReasoningEffortOption = "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max";

export interface ModelReasoningCapabilities {
  supportedEfforts: ReasoningEffortOption[];
  defaultEffort: ReasoningEffortOption;
}

export interface CatalogModel {
  providerId: string;
  displayName: string;
  vendor?: string;
  capabilities: {
    supportsVision: boolean;
    supportsVideo: boolean;
    supportsImageUrl: boolean;
    supportsImageBase64: boolean;
    supportsVideoUrl: boolean;
    supportsVideoBase64: boolean;
    forceThinking: boolean;
    thinkingDefault: string;
    thinkingPolicy: ThinkingModePolicy;
    supportsReasoningEffort: boolean;
    suppressThinkingParameter?: boolean;
    forcedReasoningEffort?: "max";
    forcedThinkingOptions?: ThinkingOption[];
    reasoningEffortOptions?: ReasoningEffortOption[];
    reasoning?: ModelReasoningCapabilities;
  };
}

export interface CatalogProvider {
  id: string;
  label: string;
  modelIds: string[];
}

export const MODEL_CATALOG = modelCatalog as {
  schemaVersion: number;
  providers: CatalogProvider[];
  models: Record<string, CatalogModel>;
  contextWindows: Record<string, { context_length: number; source: string; verified_at?: string }>;
};

function toUiCapabilities(capabilities: CatalogModel["capabilities"]): ModelCapabilities {
  return {
    supports_vision: capabilities.supportsVision,
    supports_video: capabilities.supportsVideo,
    supports_image_url: capabilities.supportsImageUrl,
    supports_image_base64: capabilities.supportsImageBase64,
    supports_video_url: capabilities.supportsVideoUrl,
    supports_video_base64: capabilities.supportsVideoBase64,
    force_thinking: capabilities.forceThinking,
    thinking: capabilities.thinkingDefault,
    thinkingModePolicy: capabilities.thinkingPolicy,
    supportsReasoningEffort: capabilities.supportsReasoningEffort,
    suppressThinkingParameter: capabilities.suppressThinkingParameter,
    forcedReasoningEffort: capabilities.forcedReasoningEffort,
    forcedThinkingOptions: capabilities.forcedThinkingOptions,
    reasoningEffortOptions: capabilities.reasoning?.supportedEfforts ?? capabilities.reasoningEffortOptions,
    reasoning: capabilities.reasoning,
  };
}

export const MODEL_CAPABILITIES: Record<string, ModelCapabilities> = Object.fromEntries(
  Object.entries(MODEL_CATALOG.models)
    .map(([id, model]) => [id, toUiCapabilities(model.capabilities)]),
);

export const PROVIDER_MODELS: Record<string, string[]> = Object.fromEntries(
  MODEL_CATALOG.providers.map(({ id, modelIds }) => [id, modelIds]),
);

export const OPENAI_REASONING_MODEL_IDS = new Set([
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
]);

export function isOpenAiReasoningModel(model: string): boolean {
  return OPENAI_REASONING_MODEL_IDS.has(model);
}

export const CUSTOM_MODEL_SENTINEL = "__custom__";

export const CUSTOM_MODEL_DEFAULTS: ModelCapabilities = {
  supports_vision: false,
  supports_video: false,
  supports_image_url: false,
  supports_image_base64: false,
  supports_video_url: false,
  supports_video_base64: false,
  force_thinking: false,
  thinking: "default",
  thinkingModePolicy: "unknown",
  supportsReasoningEffort: false,
};

export function isKnownModel(upstreamModel: string): boolean {
  return upstreamModel in MODEL_CAPABILITIES;
}

// Selectable models for providers. `PROVIDER_MODELS` is derived directly from
// the shared catalog's `providers` array (which includes `zai`).
// `DIRECT_PROVIDER_MODELS` is retained for fallback compatibility.
const DIRECT_PROVIDER_MODELS: Record<string, string[]> = {
  zai: ["glm-5.3", "glm-5.3-flash", "glm-5.3-flashx", "glm-5.2"],
};

// For the ApiKeyPanel and other panels: get the selectable models for a provider
export function getProviderModels(providerId: string): string[] {
  return PROVIDER_MODELS[providerId] ?? DIRECT_PROVIDER_MODELS[providerId] ?? [];
}
