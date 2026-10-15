import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type { TranslationKey } from "../i18n/lang/en";
import type { GatewayConfig, McpConfig, McpTargetConfig, AntigravityMcpInfo, AntigravityCommandsInfo, AntigravityCommandInstallOutcome, PovNerfCommandsInfo, PovNerfSkillId, PovNerfOperationResult } from "../types";
import { getMcpTargetKey } from "../types";
import { MODEL_CAPABILITIES, MODEL_CATALOG, getProviderModels, isKnownModel } from "../modelCapabilities";
import { getVisibleOpenRouterProfiles } from "../dashboardTiles";
import { getModelDisplayName } from "../config/modelDisplayNames";
import {
  getOpenRouterModelsForProfile,
  getOpenRouterModelDisplayName,
} from "../config/builtinOpenRouter";

interface McpSettingPanelProps {
  config: GatewayConfig | null;
  refreshConfig: () => Promise<void>;
}

const MCP_PROVIDER_ORDER = [
  "deepseek",
  "mimo",
  "zai",
  "minimax",
  "kimi",
  "kimi-code",
  "openrouter",
];

const providerLabelMap = new Map(MODEL_CATALOG.providers.map((p) => [p.id, p.label]));
const MCP_PROVIDERS = [
  ...MCP_PROVIDER_ORDER.filter((id) => providerLabelMap.has(id)).map((id) => ({
    id,
    name: providerLabelMap.get(id)!,
  })),
  ...MODEL_CATALOG.providers
    .filter((p) => !MCP_PROVIDER_ORDER.includes(p.id))
    .map((p) => ({ id: p.id, name: p.label })),
];

const COL_STYLE: React.CSSProperties = {
  padding: "6px 4px",
  fontSize: 12,
  color: "#1f2937",
  whiteSpace: "nowrap",
};

function formatEffortLabel(opt: string, t: ReturnType<typeof useTranslation>["t"]): string {
  switch (opt) {
    case "low":
      return t("apiKeyPanel.reasoningEffortLow");
    case "medium":
      return t("apiKeyPanel.reasoningEffortMedium");
    case "high":
      return t("apiKeyPanel.reasoningEffortHigh");
    case "xhigh":
      return t("openRouterModels.reasoningExtraHigh") || "Extra High";
    case "max":
      return t("apiKeyPanel.reasoningEffortMaxFixed");
    default:
      return opt;
  }
}

export default function McpSettingPanel({ config, refreshConfig }: McpSettingPanelProps) {
  const { t } = useTranslation();
  const [mcpConfig, setMcpConfig] = useState<McpConfig>({});
  const [expandedProviders, setExpandedProviders] = useState<Set<string>>(new Set());
  const [selectedOrProfileId, setSelectedOrProfileId] = useState<string>("");
  const [saving, setSaving] = useState(false);
  const [savedMessage, setSavedMessage] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // ── Antigravity Integration State ──
  const [agInfo, setAgInfo] = useState<AntigravityMcpInfo | null>(null);
  const [selectedExePath, setSelectedExePath] = useState<string>("");
  const [agLoading, setAgLoading] = useState(false);
  const [agSavedMessage, setAgSavedMessage] = useState(false);
  const [agError, setAgError] = useState<string | null>(null);

  // ── Antigravity Commands State (Global Skills) ──
  const [commandsInfo, setCommandsInfo] = useState<AntigravityCommandsInfo | null>(null);
  const [commandsLoading, setCommandsLoading] = useState(false);
  const [commandsSavedMessage, setCommandsSavedMessage] = useState(false);
  const [commandsError, setCommandsError] = useState<string | null>(null);

  // ── Antigravity Nerf Commands (/pov-nerf-balanced & /pov-nerf-lightweight) State ──
  const [nerfInfo, setNerfInfo] = useState<PovNerfCommandsInfo | null>(null);
  const [nerfStatusLoading, setNerfStatusLoading] = useState<boolean>(false);
  const [nerfLoadingSkill, setNerfLoadingSkill] = useState<string | null>(null);
  const [nerfSavedMessage, setNerfSavedMessage] = useState<string | null>(null);
  const [nerfStatusError, setNerfStatusError] = useState<string | null>(null);
  const [nerfOpError, setNerfOpError] = useState<string | null>(null);
  const nerfRefreshReqIdRef = useRef<number>(0);
  const nerfStatusLoadingRef = useRef<boolean>(false);
  const nerfLoadingSkillRef = useRef<string | null>(null);

  const refreshAgStatus = useCallback(async () => {
    try {
      const info = await invoke<AntigravityMcpInfo>("get_antigravity_mcp_status");
      setAgInfo(info);
      setSelectedExePath(info.registered_command ?? info.legacy_command ?? "");
      setAgError(null);
    } catch (e: unknown) {
      console.error("Failed to load Antigravity MCP status:", e);
      setAgError(String(e));
    }
  }, []);

  const refreshAgCommandsStatus = useCallback(async () => {
    try {
      const info = await invoke<AntigravityCommandsInfo>("get_antigravity_commands_status");
      setCommandsInfo(info);
      setCommandsError(null);
    } catch (e: unknown) {
      console.error("Failed to load Antigravity Commands status:", e);
      setCommandsError(String(e));
    }
  }, []);

  const refreshMcp = useCallback(async () => {
    try {
      const cfg = await invoke<McpConfig>("get_mcp_config");
      setMcpConfig(cfg);
    } catch (e: unknown) {
      console.error("Failed to load MCP config:", e);
      setError(String(e));
    }
  }, []);

  const refreshNerfStatus = useCallback(async () => {
    const reqId = ++nerfRefreshReqIdRef.current;
    nerfStatusLoadingRef.current = true;
    setNerfStatusLoading(true);
    try {
      const res = await invoke<PovNerfCommandsInfo>("get_pov_nerf_commands_status");
      if (reqId !== nerfRefreshReqIdRef.current) {
        return;
      }
      if (res && Array.isArray(res.skills) && res.skills.length > 0) {
        setNerfInfo(res);
        setNerfStatusError(null);
      } else {
        setNerfInfo(null);
      }
    } catch (e: unknown) {
      if (reqId !== nerfRefreshReqIdRef.current) {
        return;
      }
      console.error("Failed to load /pov-nerf commands status:", e);
      setNerfInfo(null);
      setNerfStatusError(String(e));
    } finally {
      if (reqId === nerfRefreshReqIdRef.current) {
        nerfStatusLoadingRef.current = false;
        setNerfStatusLoading(false);
      }
    }
  }, []);

  const handleInstallNerf = useCallback(async (skillId: PovNerfSkillId) => {
    if (nerfLoadingSkillRef.current !== null || nerfStatusLoadingRef.current) {
      return;
    }
    nerfLoadingSkillRef.current = skillId;
    setNerfLoadingSkill(skillId);
    setNerfOpError(null);
    setNerfSavedMessage(null);
    try {
      const res = await invoke<PovNerfOperationResult>("install_pov_nerf_command", { skillId });
      await refreshNerfStatus();
      if (res.outcome === "success") {
        setNerfOpError(null);
        setNerfSavedMessage(skillId);
        setTimeout(() => setNerfSavedMessage(null), 2500);
      } else {
        setNerfOpError(res.message ?? "Operation refused");
      }
    } catch (e: unknown) {
      console.error(`Failed to install ${skillId}:`, e);
      setNerfOpError(String(e));
      await refreshNerfStatus();
    } finally {
      nerfLoadingSkillRef.current = null;
      setNerfLoadingSkill(null);
    }
  }, [refreshNerfStatus]);

  const handleUpdateNerf = useCallback(async (skillId: PovNerfSkillId) => {
    if (nerfLoadingSkillRef.current !== null || nerfStatusLoadingRef.current) {
      return;
    }
    nerfLoadingSkillRef.current = skillId;
    setNerfLoadingSkill(skillId);
    setNerfOpError(null);
    setNerfSavedMessage(null);
    try {
      const res = await invoke<PovNerfOperationResult>("update_pov_nerf_command", { skillId });
      await refreshNerfStatus();
      if (res.outcome === "success") {
        setNerfOpError(null);
        setNerfSavedMessage(skillId);
        setTimeout(() => setNerfSavedMessage(null), 2500);
      } else {
        setNerfOpError(res.message ?? "Operation refused");
      }
    } catch (e: unknown) {
      console.error(`Failed to update ${skillId}:`, e);
      setNerfOpError(String(e));
      await refreshNerfStatus();
    } finally {
      nerfLoadingSkillRef.current = null;
      setNerfLoadingSkill(null);
    }
  }, [refreshNerfStatus]);

  const handleRemoveNerf = useCallback(async (skillId: PovNerfSkillId) => {
    if (nerfLoadingSkillRef.current !== null || nerfStatusLoadingRef.current) {
      return;
    }
    nerfLoadingSkillRef.current = skillId;
    setNerfLoadingSkill(skillId);
    setNerfOpError(null);
    setNerfSavedMessage(null);
    try {
      const res = await invoke<PovNerfOperationResult>("remove_pov_nerf_command", { skillId });
      await refreshNerfStatus();
      if (res.outcome === "success") {
        setNerfOpError(null);
        setNerfSavedMessage(skillId);
        setTimeout(() => setNerfSavedMessage(null), 2500);
      } else {
        setNerfOpError(res.message ?? "Operation refused");
      }
    } catch (e: unknown) {
      console.error(`Failed to remove ${skillId}:`, e);
      setNerfOpError(String(e));
      await refreshNerfStatus();
    } finally {
      nerfLoadingSkillRef.current = null;
      setNerfLoadingSkill(null);
    }
  }, [refreshNerfStatus]);

  useEffect(() => {
    void refreshMcp();
    void refreshAgStatus();
    void refreshAgCommandsStatus();
    void refreshNerfStatus();
  }, [refreshMcp, refreshAgStatus, refreshAgCommandsStatus, refreshNerfStatus]);

  const handleOpenAgFolder = useCallback(async () => {
    try {
      await invoke("open_antigravity_mcp_config_folder");
    } catch (e: unknown) {
      console.error("Failed to open Antigravity config folder:", e);
      setAgError(String(e));
    }
  }, []);

  const handleSelectExe = useCallback(async () => {
    try {
      const chosen = await invoke<string | null>("select_executable_dialog");
      if (chosen && chosen.trim().length > 0) {
        setSelectedExePath(chosen.trim());
        setAgError(null);
      }
    } catch (e: unknown) {
      console.error("Failed to select executable:", e);
      setAgError(String(e));
    }
  }, []);

  const handleConfigureAg = useCallback(async () => {
    if (!selectedExePath.trim()) {
      return;
    }
    setAgLoading(true);
    setAgError(null);
    setAgSavedMessage(false);
    try {
      const updated = await invoke<AntigravityMcpInfo>("configure_antigravity_mcp", {
        exePath: selectedExePath.trim(),
      });
      setAgInfo(updated);
      setSelectedExePath(updated.registered_command ?? "");
      setAgSavedMessage(true);
      setTimeout(() => setAgSavedMessage(false), 2500);
    } catch (e: unknown) {
      console.error("Failed to configure Antigravity MCP:", e);
      setAgError(String(e));
    } finally {
      setAgLoading(false);
    }
  }, [selectedExePath]);

  const handleRemoveAg = useCallback(async () => {
    setAgLoading(true);
    setAgError(null);
    setAgSavedMessage(false);
    try {
      const updated = await invoke<AntigravityMcpInfo>("remove_antigravity_mcp");
      setAgInfo(updated);
      setSelectedExePath(updated.registered_command ?? "");
      setAgSavedMessage(true);
      setTimeout(() => setAgSavedMessage(false), 2500);
    } catch (e: unknown) {
      console.error("Failed to remove Antigravity MCP configuration:", e);
      setAgError(String(e));
    } finally {
      setAgLoading(false);
    }
  }, []);

  const handleOpenSkillsFolder = useCallback(async () => {
    try {
      await invoke("open_antigravity_skills_folder");
    } catch (e: unknown) {
      console.error("Failed to open Antigravity skills folder:", e);
      setCommandsError(String(e));
    }
  }, []);

  const handleInstallCommand = useCallback(async (commandName: string) => {
    setCommandsLoading(true);
    setCommandsError(null);
    setCommandsSavedMessage(false);
    try {
      const updated = await invoke<AntigravityCommandsInfo>("install_antigravity_command", { name: commandName });
      setCommandsInfo(updated);
      setCommandsSavedMessage(true);
      setTimeout(() => setCommandsSavedMessage(false), 2500);
    } catch (e: unknown) {
      console.error(`Failed to install ${commandName} command:`, e);
      setCommandsError(String(e));
    } finally {
      setCommandsLoading(false);
    }
  }, []);

  const handleRemoveCommand = useCallback(async (commandName: string) => {
    setCommandsLoading(true);
    setCommandsError(null);
    setCommandsSavedMessage(false);
    try {
      const updated = await invoke<AntigravityCommandsInfo>("remove_antigravity_command", { name: commandName });
      setCommandsInfo(updated);
      setCommandsSavedMessage(true);
      setTimeout(() => setCommandsSavedMessage(false), 2500);
    } catch (e: unknown) {
      console.error(`Failed to remove ${commandName} command:`, e);
      setCommandsError(String(e));
    } finally {
      setCommandsLoading(false);
    }
  }, []);

  const handleInstallAllCommands = useCallback(async () => {
    setCommandsLoading(true);
    setCommandsError(null);
    setCommandsSavedMessage(false);
    try {
      const updated = await invoke<AntigravityCommandsInfo>("install_all_antigravity_commands");
      setCommandsInfo(updated);
      const outcomes = updated.install_all_results ?? [];
      const completed = outcomes.length === 3 && outcomes.every(
        (result) => result.outcome === "installed" || result.outcome === "already_current",
      );
      setCommandsSavedMessage(completed);
      if (completed) setTimeout(() => setCommandsSavedMessage(false), 2500);
    } catch (e: unknown) {
      console.error("Failed to install all Antigravity commands:", e);
      setCommandsError(String(e));
    } finally {
      setCommandsLoading(false);
    }
  }, []);

  const installOutcomeLabelKeys = {
    installed: "antigravity.commandInstallOutcomeInstalled",
    already_current: "antigravity.commandInstallOutcomeAlreadyCurrent",
    refused: "antigravity.commandInstallOutcomeRefused",
    failed: "antigravity.commandInstallOutcomeFailed",
    not_attempted: "antigravity.commandInstallOutcomeNotAttempted",
  } as const satisfies Record<AntigravityCommandInstallOutcome, TranslationKey>;

  // Active top-level target key
  const activeProviderId = mcpConfig.provider ?? config?.active_provider ?? "deepseek";
  const activeProfileId =
    activeProviderId === "openrouter"
      ? mcpConfig.profile_id ?? config?.active_openrouter_profile_id ?? ""
      : null;
  const activeTargetKey = getMcpTargetKey(activeProviderId, activeProfileId);

  // OpenRouter visible profiles
  const orProfiles = useMemo(() => {
    const orProvider = config?.providers?.["openrouter"];
    if (!orProvider?.profiles) return [];
    return getVisibleOpenRouterProfiles(orProvider.profiles) ?? orProvider.profiles;
  }, [config]);

  // Set initial selected OpenRouter profile if empty
  useEffect(() => {
    if (!selectedOrProfileId && orProfiles.length > 0) {
      const initial = activeProfileId || orProfiles[0].id;
      setSelectedOrProfileId(initial);
    }
  }, [orProfiles, activeProfileId, selectedOrProfileId]);

  /**
   * Helper: Resolve effective setting for a targetKey following user's resolution order:
   * 1. targets[targetKey]
   * 2. If targetKey == activeTargetKey, existing top-level mcpConfig
   * 3. Gateway config defaults
   */
  const resolveTargetSetting = useCallback(
    (targetKey: string, providerId: string, profileId?: string | null): {
      model: string;
      thinking_mode: string;
      reasoning_effort: string;
    } => {
      const saved = mcpConfig.targets?.[targetKey];
      const isActive = targetKey === activeTargetKey;

      // 1. Saved targets entry
      let model = saved?.model;
      let thinking_mode = saved?.thinking_mode;
      let reasoning_effort = saved?.reasoning_effort;

      // 2. Active top-level mirror fallback
      if (isActive) {
        model = model || mcpConfig.model;
        thinking_mode = thinking_mode || mcpConfig.thinking_mode;
        reasoning_effort = reasoning_effort || mcpConfig.reasoning_effort;
      }

      // 3. Provider/Profile defaults from config
      if (providerId === "openrouter") {
        const prof = orProfiles.find((p) => p.id === (profileId || selectedOrProfileId)) ?? orProfiles[0];
        const defaultOpus = prof?.models?.["claude-opus-5"];
        model = model || defaultOpus?.upstream_model || "deepseek/deepseek-r1";
        thinking_mode = thinking_mode || defaultOpus?.thinking_mode || "thinking";
        reasoning_effort = reasoning_effort || defaultOpus?.reasoning_effort;
      } else {
        const p = config?.providers?.[providerId];
        const defaultOpus = p?.models?.["claude-opus-5"];
        model = model || defaultOpus?.upstream_model || p?.default_model || (providerId === "zai" ? "glm-5.3" : "deepseek-v4-pro");
        thinking_mode = thinking_mode || defaultOpus?.thinking_mode || "thinking";
        reasoning_effort = reasoning_effort || defaultOpus?.reasoning_effort;
      }

      const caps = MODEL_CAPABILITIES[model];
      const policy = caps?.thinkingModePolicy ?? "toggleable";
      const supportsEffort = caps?.supportsReasoningEffort || !!caps?.forcedReasoningEffort;

      if (providerId === "zai") {
        if (policy === "thinking_only") {
          thinking_mode = "thinking";
        } else if (policy === "toggleable") {
          thinking_mode = thinking_mode === "normal" ? "normal" : "thinking";
        }
        if (supportsEffort && !reasoning_effort) {
          reasoning_effort = "max";
        } else if (!supportsEffort) {
          reasoning_effort = "";
        }
      } else if (policy === "thinking_only") {
        thinking_mode = "thinking_only";
        reasoning_effort = "";
      } else if (!supportsEffort) {
        reasoning_effort = "";
      } else if (!reasoning_effort) {
        reasoning_effort = providerId === "kimi" ? "max" : "high";
      }

      return {
        model: model || "",
        thinking_mode: thinking_mode || "thinking",
        reasoning_effort: reasoning_effort || "",
      };
    },
    [mcpConfig, activeTargetKey, orProfiles, selectedOrProfileId, config]
  );

  // Save changes to targets[targetKey] and mirror to top-level if targetKey is active
  const handleSaveTarget = async (
    targetKey: string,
    updatedTarget: McpTargetConfig
  ) => {
    setSaving(true);
    setError(null);
    try {
      const nextTargets = {
        ...(mcpConfig.targets || {}),
        [targetKey]: {
          ...(mcpConfig.targets?.[targetKey] || {}),
          ...updatedTarget,
        },
      };

      const nextMcp: McpConfig = {
        ...mcpConfig,
        targets: nextTargets,
      };

      // If editing currently active target, mirror to top-level
      if (targetKey === activeTargetKey) {
        if (updatedTarget.model !== undefined) nextMcp.model = updatedTarget.model;
        if (updatedTarget.thinking_mode !== undefined) nextMcp.thinking_mode = updatedTarget.thinking_mode;
        if (updatedTarget.reasoning_effort !== undefined) nextMcp.reasoning_effort = updatedTarget.reasoning_effort;
      }

      await invoke("update_mcp_config", { mcp: nextMcp });
      setMcpConfig(nextMcp);
      await refreshConfig();
      setSavedMessage(true);
      setTimeout(() => setSavedMessage(false), 2000);
    } catch (err) {
      console.error("Save MCP config failed:", err);
      setError(String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <>
      <div className="settings-tile" style={{ marginBottom: 16 }}>
        <h3>{t("mcp.destinationDetailedHeader")}</h3>

        {/* Provider rows enclosed in a rounded bordered card */}
        <div
          style={{
          display: "flex",
          flexDirection: "column",
          border: "1px solid #e5e7eb",
          borderRadius: 6,
          overflow: "hidden",
        }}
      >
        {MCP_PROVIDERS.map((provider, idx) => {
          const isExpanded = expandedProviders.has(provider.id);
          const providerId = provider.id;
          const isLast = idx === MCP_PROVIDERS.length - 1;

          const toggleExpand = () => {
            setExpandedProviders((prev) => {
              const next = new Set(prev);
              if (next.has(provider.id)) {
                next.delete(provider.id);
              } else {
                next.add(provider.id);
              }
              return next;
            });
          };

          // For summary calculation
          const targetKey =
            providerId === "openrouter"
              ? getMcpTargetKey("openrouter", selectedOrProfileId || orProfiles[0]?.id)
              : getMcpTargetKey(providerId);

          const setting = resolveTargetSetting(
            targetKey,
            providerId,
            providerId === "openrouter" ? selectedOrProfileId || orProfiles[0]?.id : undefined
          );

          // Get available models
          const availableModels: string[] = [];
          if (providerId === "openrouter") {
            const prof = orProfiles.find((p) => p.id === (selectedOrProfileId || orProfiles[0]?.id));
            const profileModels = getOpenRouterModelsForProfile(prof);
            const list = [...profileModels];
            if (setting.model && !profileModels.includes(setting.model)) {
              list.push(setting.model);
            }
            availableModels.push(...list);
          } else {
            const standardModels = getProviderModels(providerId);
            const list = [...standardModels];
            // If the current saved setting.model is not in standardModels (e.g. a legacy saved model),
            // include it as a selected-only compatibility option so existing saved configs are preserved.
            if (setting.model && !standardModels.includes(setting.model)) {
              list.push(setting.model);
            }
            availableModels.push(...list);
          }

          const caps = MODEL_CAPABILITIES[setting.model];
          const policy = caps?.thinkingModePolicy ?? "toggleable";
          const supportsReasoningEffort = caps?.supportsReasoningEffort || !!caps?.forcedReasoningEffort;
          const forcedOptions = caps?.forcedThinkingOptions;

          // Summary string for header
          const modelDisplay =
            providerId === "openrouter"
              ? getOpenRouterModelDisplayName(setting.model)
              : getModelDisplayName(setting.model, providerId);
          const summaryText =
            (setting.thinking_mode === "thinking_only" || policy === "thinking_only") && (!supportsReasoningEffort || !setting.reasoning_effort)
              ? `${modelDisplay} (${t("apiKeyPanel.thinkingOnly")})`
              : setting.thinking_mode === "thinking" || (policy === "thinking_only" && supportsReasoningEffort)
                ? (supportsReasoningEffort && setting.reasoning_effort
                    ? `${modelDisplay} (${setting.reasoning_effort})`
                    : `${modelDisplay} (${t("apiKeyPanel.thinkingModeOn")})`)
                : `${modelDisplay} (${t("apiKeyPanel.normalMode")})`;

          return (
            <div key={provider.id} style={{ borderBottom: isLast ? "none" : "1px solid #e5e7eb" }}>
              {/* Clickable header row */}
              <div
                role="button"
                tabIndex={0}
                aria-expanded={isExpanded}
                onClick={toggleExpand}
                style={{
                  display: "flex",
                  alignItems: "center",
                  background: "#ffffff",
                  cursor: "pointer",
                  transition: "background 0.1s",
                  padding: 0,
                }}
                onMouseEnter={(e) => {
                  (e.currentTarget as HTMLElement).style.background = "#f8f9fa";
                }}
                onMouseLeave={(e) => {
                  (e.currentTarget as HTMLElement).style.background = "#ffffff";
                }}
              >
                <div style={{ ...COL_STYLE, fontSize: 14, color: "#6b7280", userSelect: "none", padding: "6px 4px 6px 8px", minWidth: 28 }}>
                  {isExpanded ? "▾" : "▸"}
                </div>

                <div style={{ ...COL_STYLE, fontWeight: 600, minWidth: 130, fontSize: 13, padding: "6px 4px" }}>
                  {provider.name}
                </div>

                <div style={{ ...COL_STYLE, fontFamily: "var(--font-mono)", fontSize: 11, color: "var(--text-muted)", flex: 1, padding: "6px 8px" }}>
                  {summaryText}
                </div>
              </div>

              {/* Expandable edit area */}
              {isExpanded && (
                <div
                  style={{
                    background: "#fafafa",
                    borderTop: "1px solid #e5e7eb",
                    padding: "12px 16px 14px 38px",
                    display: "flex",
                    flexDirection: "column",
                    gap: 10,
                  }}
                  onClick={(e) => e.stopPropagation()}
                >
                  {/* OpenRouter Profile Selector */}
                  {providerId === "openrouter" && (
                    <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                      <span style={{ fontSize: 11, fontWeight: 600, color: "#1f2937", minWidth: 90 }}>
                        {t("mcp.profileLabel")}:
                      </span>
                      <select
                        style={{
                          padding: "4px 8px",
                          fontSize: 11,
                          fontFamily: "var(--font-mono)",
                          background: "#fff",
                          color: "#1f2937",
                          border: "1px solid #d0d7de",
                          borderRadius: 4,
                          outline: "none",
                          minWidth: 220,
                        }}
                        value={selectedOrProfileId || orProfiles[0]?.id}
                        onChange={(e) => setSelectedOrProfileId(e.target.value)}
                        disabled={saving}
                      >
                        {orProfiles.map((prof) => (
                          <option key={prof.id} value={prof.id}>
                            {prof.display_name || prof.id}
                          </option>
                        ))}
                      </select>
                    </div>
                  )}

                  {/* Model, Thinking Mode, and Reasoning Effort in a single row */}
                  <div style={{ display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
                    {/* Model Selector */}
                    <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                      <span style={{ fontSize: 11, fontWeight: 600, color: "#1f2937", whiteSpace: "nowrap" }}>
                        {t("mcp.modelLabel")}:
                      </span>
                      <select
                        style={{
                          padding: "4px 8px",
                          fontSize: 11,
                          fontFamily: "var(--font-mono)",
                          background: "#fff",
                          color: "#1f2937",
                          border: "1px solid #d0d7de",
                          borderRadius: 4,
                          outline: "none",
                          minWidth: 180,
                        }}
                        value={setting.model}
                        onChange={(e) => {
                          const newModel = e.target.value;
                          const newCaps = MODEL_CAPABILITIES[newModel];
                          const newPolicy = newCaps?.thinkingModePolicy ?? "toggleable";
                          const newSupportsEffort = newCaps?.supportsReasoningEffort || !!newCaps?.forcedReasoningEffort;

                          const update: McpTargetConfig = { model: newModel };
                          if (providerId === "zai") {
                            update.thinking_mode = newPolicy === "toggleable" && setting.thinking_mode === "normal" ? "normal" : "thinking";
                            update.reasoning_effort = newSupportsEffort ? (setting.reasoning_effort || "max") : "";
                          } else if (newPolicy === "thinking_only") {
                            update.thinking_mode = "thinking_only";
                            update.reasoning_effort = "";
                          } else if (!newSupportsEffort) {
                            update.reasoning_effort = "";
                          }
                          void handleSaveTarget(targetKey, update);
                        }}
                        disabled={saving}
                      >
                        {availableModels.map((m) => {
                          const prof = providerId === "openrouter"
                            ? orProfiles.find((p) => p.id === (selectedOrProfileId || orProfiles[0]?.id))
                            : undefined;
                          const isLegacySaved = providerId === "openrouter"
                            ? !getOpenRouterModelsForProfile(prof).includes(m)
                            : !getProviderModels(providerId).includes(m);
                          const name = providerId === "openrouter"
                            ? getOpenRouterModelDisplayName(m)
                            : getModelDisplayName(m, providerId);
                          const label = isLegacySaved
                            ? `${name} (Legacy / saved)`
                            : name;
                          return (
                            <option key={m} value={m}>
                              {label}
                            </option>
                          );
                        })}
                      </select>
                    </div>

                    {/* Thinking Mode */}
                    {policy === "thinking_only" ? (
                      <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                        <span style={{ fontSize: 11, fontWeight: 600, color: "#1f2937", whiteSpace: "nowrap" }}>
                          {t("mcp.thinkingLabel")}:
                        </span>
                        <span style={{ fontSize: 11, color: "var(--text-muted)", fontStyle: "italic" }}>
                          {t("apiKeyPanel.thinkingOnly")}
                        </span>
                      </div>
                    ) : policy !== "none" ? (
                      <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                        <span style={{ fontSize: 11, fontWeight: 600, color: "#1f2937", whiteSpace: "nowrap" }}>
                          {t("mcp.thinkingLabel")}:
                        </span>
                        <select
                          style={{
                            padding: "4px 8px",
                            fontSize: 11,
                            fontFamily: "var(--font-mono)",
                            background: "#fff",
                            color: "#1f2937",
                            border: "1px solid #d0d7de",
                            borderRadius: 4,
                            outline: "none",
                            minWidth: 100,
                          }}
                          value={setting.thinking_mode}
                          onChange={(e) => {
                            void handleSaveTarget(targetKey, { thinking_mode: e.target.value });
                          }}
                          disabled={saving || policy === "forced"}
                        >
                          <option value="thinking">{t("apiKeyPanel.thinkingModeOn")}</option>
                          <option value="normal">{t("apiKeyPanel.normalMode")}</option>
                        </select>
                      </div>
                    ) : null}

                    {/* Reasoning Effort */}
                    {((setting.thinking_mode === "thinking" && supportsReasoningEffort && policy !== "thinking_only") ||
                      (providerId === "zai" && supportsReasoningEffort && (policy !== "toggleable" || setting.thinking_mode === "thinking"))) && (
                      <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                        <span style={{ fontSize: 11, fontWeight: 600, color: "#1f2937", whiteSpace: "nowrap" }}>
                          {t("mcp.effortLabel")}:
                        </span>
                        <select
                          style={{
                            padding: "4px 8px",
                            fontSize: 11,
                            fontFamily: "var(--font-mono)",
                            background: "#fff",
                            color: "#1f2937",
                            border: "1px solid #d0d7de",
                            borderRadius: 4,
                            outline: "none",
                            minWidth: 80,
                          }}
                          value={setting.reasoning_effort}
                          onChange={(e) => {
                            void handleSaveTarget(targetKey, { reasoning_effort: e.target.value });
                          }}
                          disabled={saving}
                        >
                          {forcedOptions ? (
                            forcedOptions.map((opt) => (
                              <option key={opt} value={opt}>
                                {formatEffortLabel(opt, t)}
                              </option>
                            ))
                          ) : caps?.reasoningEffortOptions ? (
                            caps.reasoningEffortOptions.map((opt) => (
                              <option key={opt} value={opt}>
                                {formatEffortLabel(opt, t)}
                              </option>
                            ))
                          ) : (
                            <>
                              <option value="low">{t("apiKeyPanel.reasoningEffortLow")}</option>
                              <option value="medium">{t("apiKeyPanel.reasoningEffortMedium")}</option>
                              <option value="high">{t("apiKeyPanel.reasoningEffortHigh")}</option>
                              <option value="max">{t("apiKeyPanel.reasoningEffortMaxFixed")}</option>
                            </>
                          )}
                        </select>
                      </div>
                    )}
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>

    {/* ── Antigravity Integration Section ── */}
    <div className="settings-tile">
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6 }}>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            flexWrap: "wrap",
          }}
        >
          <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
            {t("antigravity.header")}
          </h3>
            <div>
              {agInfo?.status === "invalid" ? (
                <span style={{ fontSize: 12, color: "#cf222e", fontWeight: 600 }}>
                  ⚠ {t("antigravity.statusInvalid")}
                </span>
              ) : selectedExePath.length > 0 && selectedExePath !== (agInfo?.registered_command ?? "") ? (
                <span style={{ fontSize: 12, color: "#9a6700", fontWeight: 600 }}>
                  ⚠ {t("antigravity.statusOutdated")}
                </span>
              ) : agInfo?.status === "configured" && agInfo.registered_command ? (
                <span style={{ display: "inline-flex", alignItems: "center", gap: 4, fontSize: 12, color: "#1a7f37", fontWeight: 600 }}>
                  <span>✓</span> {t("antigravity.statusConfigured")}
                </span>
              ) : (
                <span style={{ fontSize: 12, color: "#656d76", fontWeight: 500 }}>
                  {t("antigravity.statusNotConfigured")}
                </span>
              )}
              {agLoading && <span style={{ fontSize: 12, color: "#656d76", marginLeft: 6 }}>...</span>}
            </div>
          </div>
          {agSavedMessage && (
            <span style={{ fontSize: 12, color: "#1a7f37", fontWeight: 500 }}>
              ✓ {t("antigravity.savedMessage")}
            </span>
          )}
        </div>

        <p style={{ margin: "0 0 12px 0", fontSize: 12, color: "#656d76" }}>
          {t("antigravity.desc")}
        </p>

            {agError && (
              <div style={{ margin: "0 0 12px 0", padding: "8px 12px", background: "#ffebe9", border: "1px solid #ff8182", borderRadius: 6, fontSize: 12, color: "#cf222e" }}>
                {agError}
              </div>
            )}

            <div style={{ background: "#f6f8fa", border: "1px solid #d0d7de", borderRadius: 6, padding: "12px 14px", marginBottom: 14 }}>
              {/* Block 1 Header: MCP Server Registration */}
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <span style={{ fontSize: 12, fontWeight: 700, color: "#24292f" }}>
                    {t("antigravity.mcpSectionHeader")}
                  </span>
                  <div>
                    {agInfo?.status === "invalid" ? (
                      <span style={{ fontSize: 11, color: "#cf222e", fontWeight: 600 }}>
                        ⚠ {t("antigravity.statusInvalid")}
                      </span>
                    ) : selectedExePath.length > 0 && selectedExePath !== (agInfo?.registered_command ?? "") ? (
                      <span style={{ fontSize: 11, color: "#9a6700", fontWeight: 600 }}>
                        ⚠ {t("antigravity.statusOutdated")}
                      </span>
                    ) : agInfo?.status === "configured" && agInfo.registered_command ? (
                      <span style={{ display: "inline-flex", alignItems: "center", gap: 3, fontSize: 11, color: "#1a7f37", fontWeight: 600 }}>
                        <span>✓</span> {t("antigravity.statusConfigured")}
                      </span>
                    ) : (
                      <span style={{ fontSize: 11, color: "#656d76", fontWeight: 500 }}>
                        {t("antigravity.statusNotConfigured")}
                      </span>
                    )}
                  </div>
                </div>
              </div>

              {/* Row 1: Antigravity Config File & Actions */}
              <div style={{ marginBottom: 14 }}>
                <div style={{ fontSize: 11, fontWeight: 600, color: "#57606a", marginBottom: 4 }}>
                  {t("antigravity.configPathLabel")}:
                </div>
                <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
                  <input
                    type="text"
                    readOnly
                    value={agInfo?.config_path || "~\\.gemini\\config\\mcp_config.json"}
                    style={{
                      flex: "1 1 240px",
                      fontFamily: "var(--font-mono)",
                      fontSize: 11,
                      padding: "5px 8px",
                      background: "#fff",
                      color: "#24292f",
                      border: "1px solid #d0d7de",
                      borderRadius: 4,
                      outline: "none",
                      boxSizing: "border-box",
                    }}
                  />
                  <button
                    className="btn"
                    style={{ fontSize: 12, padding: "5px 12px" }}
                    onClick={handleOpenAgFolder}
                    disabled={agLoading}
                  >
                    {t("antigravity.btnOpenFolder")}
                  </button>
                  <button
                    className="btn btn-primary"
                    style={{ fontSize: 12, padding: "5px 12px" }}
                    onClick={handleConfigureAg}
                    disabled={
                      agLoading ||
                      agInfo?.status === "invalid" ||
                      !selectedExePath.trim() ||
                      selectedExePath === (agInfo?.registered_command ?? "")
                    }
                  >
                    {t("antigravity.btnUpdate")}
                  </button>
                  {agInfo?.status === "configured" && (
                    <button
                      className="btn btn-danger"
                      style={{ fontSize: 12, padding: "5px 12px" }}
                      onClick={handleRemoveAg}
                      disabled={agLoading}
                    >
                      {t("antigravity.btnRemove")}
                    </button>
                  )}
                </div>
              </div>

              {/* Row 2: Selected / Target Poverello Executable */}
              <div>
                <div style={{ fontSize: 11, fontWeight: 600, color: "#57606a", marginBottom: 4 }}>
                  {t("antigravity.targetExeLabel")}:
                </div>
                <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
                  <input
                    type="text"
                    readOnly
                    value={selectedExePath}
                    placeholder={t("antigravity.placeholderNotSelected")}
                    style={{
                      flex: "1 1 240px",
                      fontFamily: "var(--font-mono)",
                      fontSize: 11,
                      padding: "5px 8px",
                      background: "#fff",
                      color: selectedExePath ? "#24292f" : "#656d76",
                      border: "1px solid #d0d7de",
                      borderRadius: 4,
                      outline: "none",
                      boxSizing: "border-box",
                    }}
                  />
                  <button
                    className="btn"
                    style={{ fontSize: 12, padding: "5px 12px" }}
                    onClick={handleSelectExe}
                    disabled={agLoading}
                  >
                    {t("antigravity.btnChangeExe")}
                  </button>
                </div>
              </div>

              {agInfo?.legacy_status === "configured" && (
                <div style={{ marginTop: 10, fontSize: 11, color: "#57606a", background: "#ffffff", border: "1px solid #d0d7de", borderRadius: 4, padding: "6px 10px" }}>
                  ℹ {t("antigravity.mcpLegacyPreserved")}
                </div>
              )}
            </div>

            {/* Block 2: Antigravity Commands (/pov-plan, /pov-revise, /pov-review) */}
            <div style={{ background: "#f6f8fa", border: "1px solid #d0d7de", borderRadius: 6, padding: "12px 14px", marginBottom: 12 }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6, flexWrap: "wrap", gap: 8 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                  <span style={{ fontSize: 12, fontWeight: 700, color: "#24292f" }}>
                    {t("antigravity.commandsSectionHeader")}
                  </span>
                  {commandsLoading && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  {commandsSavedMessage && (
                    <span style={{ fontSize: 12, color: "#1a7f37", fontWeight: 500 }}>
                      ✓ {t("antigravity.savedMessage")}
                    </span>
                  )}
                  <button
                    className="btn"
                    style={{ fontSize: 11, padding: "4px 10px" }}
                    onClick={handleOpenSkillsFolder}
                    disabled={commandsLoading}
                  >
                    {t("antigravity.btnOpenSkillsFolder")}
                  </button>
                  {((commandsInfo?.plan_command.status !== "installed") ||
                    (commandsInfo?.revise_command.status !== "installed") ||
                    (commandsInfo?.review_command.status !== "installed")) && (
                    <button
                      className="btn btn-primary"
                      style={{ fontSize: 11, padding: "4px 10px" }}
                      onClick={handleInstallAllCommands}
                      disabled={commandsLoading}
                    >
                      {t("antigravity.btnInstallAll")}
                    </button>
                  )}
                </div>
              </div>

              <p style={{ margin: "0 0 10px 0", fontSize: 12, color: "#656d76" }}>
                {t("antigravity.commandsDesc")}
              </p>

              {agInfo?.status !== "configured" && (
                <div style={{ margin: "0 0 10px 0", padding: "6px 10px", background: "#fff8c5", border: "1px solid #d4a72c", borderRadius: 4, fontSize: 11, color: "#6e4a00" }}>
                  {t("antigravity.commandsMcpWarning")}
                </div>
              )}

              {commandsInfo &&
                ((commandsInfo.legacy_plan_command?.status && commandsInfo.legacy_plan_command.status !== "not_installed") ||
                  (commandsInfo.legacy_revise_command?.status && commandsInfo.legacy_revise_command.status !== "not_installed") ||
                  (commandsInfo.legacy_review_command?.status && commandsInfo.legacy_review_command.status !== "not_installed")) && (
                  <div style={{ margin: "0 0 10px 0", padding: "8px 12px", background: "#f6f8fa", border: "1px solid #d0d7de", borderRadius: 6, fontSize: 12 }}>
                    <div style={{ fontWeight: 600, color: "#24292f", marginBottom: 2 }}>
                      ℹ {t("antigravity.legacyNoticeTitle")}
                    </div>
                    <div style={{ color: "#57606a", fontSize: 11 }}>
                      {t("antigravity.legacyNoticeDesc")}
                    </div>
                  </div>
              )}

              {commandsError && (
                <div style={{ margin: "0 0 10px 0", padding: "6px 10px", background: "#ffebe9", border: "1px solid #ff8182", borderRadius: 4, fontSize: 11, color: "#cf222e" }}>
                  {commandsError}
                </div>
              )}

              {commandsInfo?.install_all_results && (
                <div
                  role="status"
                  aria-live="polite"
                  style={{ margin: "0 0 10px 0", padding: "8px 10px", background: "#fff8c5", border: "1px solid #d4a72c", borderRadius: 4, fontSize: 11, color: "#6e4a00" }}
                >
                  {commandsInfo.install_all_results.map((result) => (
                    <div key={result.name}>
                      <strong>{result.name}</strong>: {t(installOutcomeLabelKeys[result.outcome])}
                      {result.message ? ` — ${result.message}` : ""}
                    </div>
                  ))}
                </div>
              )}

              {/* Commands List Cards */}
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                {[
                  {
                    item: commandsInfo?.plan_command,
                    titleKey: "antigravity.commandPlanTitle" as const,
                    descKey: "antigravity.commandPlanDesc" as const,
                    fallbackName: "pov-plan",
                    fallbackCmd: "/pov-plan",
                  },
                  {
                    item: commandsInfo?.revise_command,
                    titleKey: "antigravity.commandReviseTitle" as const,
                    descKey: "antigravity.commandReviseDesc" as const,
                    fallbackName: "pov-revise",
                    fallbackCmd: "/pov-revise",
                  },
                  {
                    item: commandsInfo?.review_command,
                    titleKey: "antigravity.commandReviewTitle" as const,
                    descKey: "antigravity.commandReviewDesc" as const,
                    fallbackName: "pov-review",
                    fallbackCmd: "/pov-review",
                  },
                ].map(({ item, titleKey, descKey, fallbackName, fallbackCmd }) => {
                  const cmdName = item?.name ?? fallbackName;
                  const slashCmd = item?.slash_command ?? fallbackCmd;
                  const status = item?.status ?? "not_installed";

                  return (
                    <div
                      key={cmdName}
                      style={{
                        background: "#fff",
                        border: "1px solid #e1e4e8",
                        borderRadius: 6,
                        padding: "8px 12px",
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "center",
                        flexWrap: "wrap",
                        gap: 8,
                      }}
                    >
                      <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
                        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                          <span style={{ fontFamily: "var(--font-mono)", fontSize: 12, fontWeight: 700, color: "#1f2937" }}>
                            {t(titleKey)}
                          </span>
                          <div>
                            {status === "invalid" ? (
                              <span style={{ fontSize: 11, color: "#cf222e", fontWeight: 600 }}>
                                ⚠ {t("antigravity.commandStatusInvalid")}
                              </span>
                            ) : status === "outdated" ? (
                              <span style={{ fontSize: 11, color: "#9a6700", fontWeight: 600 }}>
                                ⚠ {t("antigravity.commandStatusOutdated")}
                              </span>
                            ) : status === "installed" ? (
                              <span style={{ display: "inline-flex", alignItems: "center", gap: 3, fontSize: 11, color: "#1a7f37", fontWeight: 600 }}>
                                <span>✓</span> {t("antigravity.commandStatusInstalled")}
                              </span>
                            ) : (
                              <span style={{ fontSize: 11, color: "#656d76", fontWeight: 500 }}>
                                {t("antigravity.commandStatusNotInstalled")}
                              </span>
                            )}
                          </div>
                        </div>
                        <div style={{ fontSize: 11, color: "#57606a" }}>
                          {t(descKey)}
                        </div>
                        {item?.status === "invalid" && item.error && (
                          <div style={{ fontSize: 11, color: "#cf222e" }}>{item.error}</div>
                        )}
                      </div>

                      <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                        {status === "not_installed" && (
                          <button
                            className="btn btn-primary"
                            style={{ fontSize: 11, padding: "4px 10px" }}
                            onClick={() => void handleInstallCommand(cmdName)}
                            disabled={commandsLoading}
                          >
                            {t("antigravity.commandBtnInstall")}
                          </button>
                        )}
                        {status === "outdated" && (
                          <button
                            className="btn btn-primary"
                            style={{ fontSize: 11, padding: "4px 10px" }}
                            onClick={() => void handleInstallCommand(cmdName)}
                            disabled={commandsLoading}
                          >
                            {t("antigravity.commandBtnUpdate")}
                          </button>
                        )}
                        {(status === "installed" || status === "outdated") && (
                          <button
                            className="btn btn-danger"
                            style={{ fontSize: 11, padding: "4px 10px" }}
                            onClick={() => void handleRemoveCommand(cmdName)}
                            disabled={commandsLoading}
                          >
                            {t("antigravity.commandBtnRemove")}
                          </button>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>

            {/* Antigravity Nerf Commands (/pov-nerf-balanced & /pov-nerf-lightweight) */}
            <div
              style={{
                marginTop: 20,
                padding: "16px 20px",
                backgroundColor: "#ffffff",
                border: "1px solid #d0d7de",
                borderRadius: 8,
                display: "flex",
                flexDirection: "column",
                gap: 14,
              }}
            >
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", flexWrap: "wrap", gap: 8 }}>
                <div>
                  <div style={{ fontSize: 14, fontWeight: 700, color: "#1f2937", display: "flex", alignItems: "center", gap: 8 }}>
                    <span>{t("antigravity.nerfSectionHeader")}</span>
                    {(nerfLoadingSkill !== null || nerfStatusLoading) && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
                  </div>
                  <div style={{ fontSize: 12, color: "#57606a", marginTop: 4 }}>
                    {t("antigravity.nerfDesc")}
                  </div>
                </div>
              </div>

              {/* Legacy /pov-nerf detected notice */}
              {nerfInfo?.legacy_single_skill?.present && (
                <div style={{ padding: "8px 12px", backgroundColor: "#fff8c5", color: "#6e4a00", borderRadius: 6, fontSize: 12, border: "1px solid #d4a72c" }}>
                  <div>⚠ {nerfInfo.legacy_single_skill.message}</div>
                  <div style={{ fontFamily: "var(--font-mono)", fontSize: 11, marginTop: 4, color: "#57606a" }}>
                    {nerfInfo.legacy_single_skill.path}
                  </div>
                </div>
              )}

              {/* Operation error banner (Install / Update / Remove failure) */}
              {nerfOpError && (
                <div
                  style={{
                    padding: "8px 12px",
                    backgroundColor: "#ffebe9",
                    color: "#cf222e",
                    borderRadius: 6,
                    fontSize: 12,
                    border: "1px solid #ffc1ba",
                  }}
                >
                  <div>⚠ {nerfOpError}</div>
                </div>
              )}

              {/* Status fetch error banner with localized retry control */}
              {nerfStatusError && (
                <div
                  style={{
                    padding: "8px 12px",
                    backgroundColor: "#ffebe9",
                    color: "#cf222e",
                    borderRadius: 6,
                    fontSize: 12,
                    border: "1px solid #ffc1ba",
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    gap: 8,
                    flexWrap: "wrap",
                  }}
                >
                  <div style={{ flex: "1 1 auto" }}>⚠ {nerfStatusError}</div>
                  <button
                    className="btn btn-secondary"
                    style={{ fontSize: 11, padding: "3px 8px", flexShrink: 0 }}
                    onClick={() => void refreshNerfStatus()}
                    disabled={nerfLoadingSkill !== null}
                  >
                    {t("antigravity.nerfRetryStatus")}
                  </button>
                </div>
              )}

              {/* 2 Skill Cards */}
              {!nerfInfo || !Array.isArray(nerfInfo.skills) || nerfInfo.skills.length === 0 ? (
                <div style={{ fontSize: 12, color: "#656d76", padding: "12px 0" }}>
                  {t("antigravity.nerfDesc")}
                </div>
              ) : (
                <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                  {nerfInfo.skills.map((skill) => {
                    const id = skill.skill_id;
                    const isActionLoading = nerfLoadingSkill === id;
                    const isAnyLoading = nerfLoadingSkill !== null || nerfStatusLoading;
                    const isSaved = nerfSavedMessage === id;
                    const titleKey = id === "balanced" ? "antigravity.nerfBalancedTitle" : "antigravity.nerfLightweightTitle";
                    const descKey = id === "balanced" ? "antigravity.nerfBalancedDesc" : "antigravity.nerfLightweightDesc";

                    return (
                      <div
                        key={id}
                        style={{
                          background: "#fff",
                          border: "1px solid #e1e4e8",
                          borderRadius: 6,
                          padding: "10px 14px",
                          display: "flex",
                          justifyContent: "space-between",
                          alignItems: "center",
                          flexWrap: "wrap",
                          gap: 10,
                        }}
                      >
                        <div style={{ display: "flex", flexDirection: "column", gap: 3, flex: "1 1 320px" }}>
                          <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                            <span style={{ fontFamily: "var(--font-mono)", fontSize: 13, fontWeight: 700, color: "#1f2937" }}>
                              {t(titleKey)}
                            </span>
                            <div>
                              {skill.state === "invalid" ? (
                                <span style={{ fontSize: 11, color: "#cf222e", fontWeight: 600 }}>
                                  ⚠ {t("antigravity.commandStatusInvalid")}
                                </span>
                              ) : skill.state === "modified" ? (
                                <span style={{ fontSize: 11, color: "#9a6700", fontWeight: 600 }}>
                                  ⚠ {t("antigravity.nerfStatusModified")}
                                </span>
                              ) : skill.state === "outdated" ? (
                                <span style={{ fontSize: 11, color: "#9a6700", fontWeight: 600 }}>
                                  ⚠ {t("antigravity.commandStatusOutdated")}
                                </span>
                              ) : skill.state === "installed" ? (
                                <span style={{ display: "inline-flex", alignItems: "center", gap: 3, fontSize: 11, color: "#1a7f37", fontWeight: 600 }}>
                                  <span>✓</span> {t("antigravity.commandStatusInstalled")}
                                </span>
                              ) : (
                                <span style={{ fontSize: 11, color: "#656d76", fontWeight: 500 }}>
                                  {t("antigravity.commandStatusNotInstalled")}
                                </span>
                              )}
                            </div>
                            {isSaved && (
                              <span style={{ fontSize: 11, color: "#1a7f37", fontWeight: 600 }}>
                                ✓ {t("antigravity.savedMessage")}
                              </span>
                            )}
                          </div>
                          <div style={{ fontSize: 11, color: "#57606a" }}>
                            {t(descKey)}
                          </div>
                          <div style={{ fontFamily: "var(--font-mono)", fontSize: 10, color: "#8c959f", wordBreak: "break-all" }}>
                            {skill.target_path}
                          </div>
                          {skill.diagnostic_reason && (
                            <div style={{ fontSize: 11, color: skill.state === "invalid" ? "#cf222e" : "#9a6700", marginTop: 2 }}>
                              {skill.diagnostic_reason}
                            </div>
                          )}
                        </div>

                        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                          {skill.permitted_actions.includes("install") && (
                            <button
                              className="btn btn-primary"
                              style={{ fontSize: 11, padding: "4px 10px" }}
                              onClick={() => void handleInstallNerf(id)}
                              disabled={isAnyLoading}
                            >
                              {isActionLoading ? "..." : t("antigravity.commandBtnInstall")}
                            </button>
                          )}
                          {skill.permitted_actions.includes("update") && (
                            <button
                              className="btn btn-primary"
                              style={{ fontSize: 11, padding: "4px 10px" }}
                              onClick={() => void handleUpdateNerf(id)}
                              disabled={isAnyLoading}
                            >
                              {isActionLoading ? "..." : t("antigravity.commandBtnUpdate")}
                            </button>
                          )}
                          {skill.permitted_actions.includes("remove") && (
                            <button
                              className="btn btn-danger"
                              style={{ fontSize: 11, padding: "4px 10px" }}
                              onClick={() => void handleRemoveNerf(id)}
                              disabled={isAnyLoading}
                            >
                              {isActionLoading ? "..." : t("antigravity.commandBtnRemove")}
                            </button>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
      </div>
    </>
  );
}
