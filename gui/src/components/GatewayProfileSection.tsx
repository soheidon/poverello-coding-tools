import React, { useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type {
  GatewayConfig,
  GatewayProfile,
  GatewayProfileRoute,
  CommandResponse,
} from "../types";

const COL_STYLE: React.CSSProperties = {
  padding: "6px 10px",
  fontSize: 12,
  color: "#1f2937",
  whiteSpace: "nowrap",
};

const CANONICAL_ROUTES = [
  { key: "claude-opus-5", labelKey: "apiKeyPanel.gatewayPro" },
  { key: "claude-sonnet-5", labelKey: "apiKeyPanel.gatewayFlash" },
  { key: "claude-haiku-4-5", labelKey: "apiKeyPanel.gatewayHaiku" },
] as const;

const ELIGIBLE_PROVIDERS = [
  { id: "deepseek", label: "DeepSeek" },
  { id: "anthropic", label: "Anthropic Direct" },
  { id: "zai", label: "Z.ai GLM" },
  { id: "openrouter", label: "OpenRouter" },
] as const;

const ELIGIBLE_MODELS: Record<string, { id: string; label: string }[]> = {
  anthropic: [
    { id: "claude-opus-5-5", label: "claude-opus-5-5" },
    { id: "claude-sonnet-5-5", label: "claude-sonnet-5-5" },
    { id: "claude-haiku-5-5", label: "claude-haiku-5-5" },
  ],
  zai: [
    { id: "glm-5.2", label: "glm-5.2" },
    { id: "glm-5.3", label: "glm-5.3" },
    { id: "glm-5.3-flash", label: "glm-5.3-flash" },
    { id: "glm-5.3-flashx", label: "glm-5.3-flashx" },
  ],
  deepseek: [
    { id: "deepseek-v4-pro", label: "deepseek-v4-pro" },
    { id: "deepseek-v4-flash", label: "deepseek-v4-flash" },
    { id: "deepseek-chat", label: "deepseek-chat" },
    { id: "deepseek-reasoner", label: "deepseek-reasoner" },
  ],
};

interface GatewayProfileSectionProps {
  config: GatewayConfig;
  gatewayRunning: boolean;
  refreshConfig: () => Promise<void>;
  restartGateway: () => Promise<void>;
}

export default function GatewayProfileSection({
  config,
  gatewayRunning,
  refreshConfig,
  restartGateway,
}: GatewayProfileSectionProps) {
  const { t } = useTranslation();
  const [actionError, setActionError] = useState<string | null>(null);
  const [editingProfileId, setEditingProfileId] = useState<string | null>(null);
  const [profileDraft, setProfileDraft] = useState<GatewayProfile | null>(null);
  const [saveSuccessId, setSaveSuccessId] = useState<string | null>(null);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameText, setRenameText] = useState("");

  const profiles = config.gateway_profiles ?? [];
  const activeProfileId = config.active_gateway_profile_id ?? null;
  const isGatewayProfileActive = config.active_provider === "gateway_profile";

  const handleStartEdit = (p: GatewayProfile) => {
    setActionError(null);
    setSaveSuccessId(null);
    if (editingProfileId === p.id) {
      setEditingProfileId(null);
      setProfileDraft(null);
    } else {
      setEditingProfileId(p.id);
      // Deep clone to draft, repairing legacy GLM combinations that the
      // backend no longer accepts before the UI presents the editable state.
      const draft: GatewayProfile = JSON.parse(JSON.stringify(p));
      for (const route of Object.values(draft.routes)) {
        if (
          route.provider === "zai" &&
          route.model !== "glm-5.2" &&
          route.thinking_mode === "normal"
        ) {
          route.thinking_mode = "thinking";
        }
      }
      setProfileDraft(draft);
    }
  };

  const handleCreateProfile = async () => {
    setActionError(null);
    const newId = typeof crypto !== "undefined" && crypto.randomUUID
      ? crypto.randomUUID()
      : `gp-${Date.now()}`;
    const nextNum = profiles.length + 1;
    const newProfile: GatewayProfile = {
      id: newId,
      display_name: `Gateway Profile ${nextNum}`,
      hidden: false,
      routes: {
        "claude-opus-5": {
          provider: "deepseek",
          model: "deepseek-v4-pro",
          thinking_mode: "thinking",
          reasoning_effort: "medium",
        },
        "claude-sonnet-5": {
          provider: "deepseek",
          model: "deepseek-v4-flash",
          thinking_mode: "normal",
          reasoning_effort: null,
        },
        "claude-haiku-4-5": {
          provider: "deepseek",
          model: "deepseek-chat",
          thinking_mode: "normal",
          reasoning_effort: null,
        },
      },
    };

    try {
      const res = await invoke<CommandResponse<void>>("save_gateway_profile", {
        profile: newProfile,
      });
      await refreshConfig();
      if (gatewayRunning && res.restartGateway) {
        await restartGateway();
      }
      setEditingProfileId(newId);
      setProfileDraft(JSON.parse(JSON.stringify(newProfile)));
    } catch (e) {
      setActionError(`Create profile failed: ${String(e)}`);
    }
  };

  const handleSelectProfile = async (profileId: string) => {
    setActionError(null);
    try {
      const res = await invoke<CommandResponse<void>>("select_gateway_profile", {
        profileId,
      });
      await refreshConfig();
      if (gatewayRunning && res.restartGateway) {
        await restartGateway();
      }
    } catch (e) {
      setActionError(`Select profile failed: ${String(e)}`);
    }
  };

  const handleDuplicateProfile = async (profileId: string) => {
    setActionError(null);
    try {
      const res = await invoke<CommandResponse<void>>("duplicate_gateway_profile", {
        profileId,
      });
      await refreshConfig();
      if (gatewayRunning && res.restartGateway) {
        await restartGateway();
      }
    } catch (e) {
      setActionError(`Duplicate profile failed: ${String(e)}`);
    }
  };

  const handleDeleteProfile = async (p: GatewayProfile) => {
    setActionError(null);
    const isActive = isGatewayProfileActive && p.id === activeProfileId;
    const msg = isActive
      ? "This profile is currently active. Deleting it will fall back to another profile or default provider. Continue?"
      : "Delete this gateway profile?";
    if (!confirm(msg)) return;

    try {
      const res = await invoke<CommandResponse<void>>("delete_gateway_profile", {
        profileId: p.id,
      });
      if (editingProfileId === p.id) {
        setEditingProfileId(null);
        setProfileDraft(null);
      }
      await refreshConfig();
      if (gatewayRunning && res.restartGateway) {
        await restartGateway();
      }
    } catch (e) {
      setActionError(`Delete profile failed: ${String(e)}`);
    }
  };

  const handleToggleHidden = async (p: GatewayProfile, hidden: boolean) => {
    setActionError(null);
    try {
      await invoke("set_gateway_profile_hidden", {
        profileId: p.id,
        hidden,
      });
      await refreshConfig();
    } catch (e) {
      setActionError(`Visibility update failed: ${String(e)}`);
    }
  };

  const handleSaveDraft = async () => {
    if (!profileDraft) return;
    setActionError(null);
    setSaveSuccessId(null);

    // Validation checks
    for (const { key } of CANONICAL_ROUTES) {
      const r = profileDraft.routes[key];
      if (!r || !r.provider) {
        setActionError(`Route ${key} must have a provider specified`);
        return;
      }
      if (r.provider === "openrouter") {
        if (!r.profile_id) {
          setActionError(`Route ${key} requires an OpenRouter profile selection`);
          return;
        }
      } else {
        if (!r.model) {
          setActionError(`Route ${key} requires an upstream model`);
          return;
        }
        const eligibleModels = ELIGIBLE_MODELS[r.provider]?.map((m) => m.id) ?? [];
        if (!eligibleModels.includes(r.model)) {
          setActionError(`Route ${key} has invalid model ${r.model} for provider ${r.provider}`);
          return;
        }
      }

      if (r.provider === "zai") {
        if (r.thinking_mode === "thinking_only") {
          setActionError(`GLM route ${key} does not support thinking_only mode`);
          return;
        }
        if (r.thinking_mode === "normal" && r.model !== "glm-5.2") {
          setActionError(`GLM route ${key} only supports normal mode for glm-5.2`);
          return;
        }
        if (r.thinking_mode && !["normal", "thinking"].includes(r.thinking_mode)) {
          setActionError(`GLM route ${key} has unsupported thinking mode ${r.thinking_mode}`);
          return;
        }
        if (r.reasoning_effort && !["low", "high", "max"].includes(r.reasoning_effort)) {
          setActionError(`GLM route ${key} has unsupported reasoning effort ${r.reasoning_effort}`);
          return;
        }
      } else if (r.provider === "deepseek") {
        if (r.thinking_mode && !["normal", "thinking", "thinking_only"].includes(r.thinking_mode)) {
          setActionError(`DeepSeek route ${key} has unsupported thinking mode ${r.thinking_mode}`);
          return;
        }
        if (r.reasoning_effort && !["low", "medium", "high"].includes(r.reasoning_effort)) {
          setActionError(`DeepSeek route ${key} has unsupported reasoning effort ${r.reasoning_effort}`);
          return;
        }
      } else if (r.provider === "anthropic") {
        if (r.thinking_mode && !["normal", "thinking"].includes(r.thinking_mode)) {
          setActionError(`Anthropic route ${key} has unsupported thinking mode ${r.thinking_mode}`);
          return;
        }
        if (r.reasoning_effort) {
          setActionError(`Anthropic route ${key} does not support reasoning effort`);
          return;
        }
      }
    }

    try {
      const res = await invoke<CommandResponse<void>>("save_gateway_profile", {
        profile: profileDraft,
      });
      await refreshConfig();
      if (gatewayRunning && res.restartGateway) {
        await restartGateway();
      }
      setSaveSuccessId(profileDraft.id);
      setTimeout(() => setSaveSuccessId(null), 3000);
    } catch (e) {
      setActionError(`Save profile failed: ${String(e)}`);
    }
  };

  const handleDraftRouteChange = (
    routeKey: string,
    updater: (prev: GatewayProfileRoute) => GatewayProfileRoute,
  ) => {
    if (!profileDraft) return;
    const prevRoute = profileDraft.routes[routeKey] || {
      provider: "deepseek",
      model: "deepseek-v4-pro",
    };
    const updated = updater(prevRoute);
    setProfileDraft({
      ...profileDraft,
      routes: {
        ...profileDraft.routes,
        [routeKey]: updated,
      },
    });
  };

  const openRouterProfiles = config.providers?.openrouter?.profiles ?? [];

  return (
    <div className="settings-tile" style={{ marginTop: 8 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8 }}>
        <div>
          <div style={{ fontWeight: 600, fontSize: 14, margin: 0, color: "#111827" }}>
            Multi-Provider Gateway Profiles
          </div>
          <div style={{ fontSize: 11, color: "#6b7280", marginTop: 2 }}>
            Assign independent providers and models across the 3 canonical Claude routes.
          </div>
        </div>
        <button
          type="button"
          className="btn btn-secondary btn-small"
          onClick={handleCreateProfile}
          style={{ fontSize: 11, padding: "3px 10px", display: "flex", alignItems: "center", gap: 4 }}
        >
          <span>+</span> Add Profile
        </button>
      </div>

      {actionError && (
        <div style={{ padding: "6px 10px", background: "#fef2f2", color: "#dc2626", borderRadius: 4, fontSize: 11, marginBottom: 8 }}>
          {actionError}
        </div>
      )}

      {profiles.length === 0 ? (
        <div style={{ padding: 16, background: "#f9fafb", borderRadius: 6, textAlign: "center", color: "#6b7280", fontSize: 12 }}>
          No multi-provider profiles configured. Click "+ Add Profile" to create one.
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", border: "1px solid #e5e7eb", borderRadius: 6, overflow: "hidden" }}>
          {profiles.map((p) => {
            const isActive = isGatewayProfileActive && p.id === activeProfileId;
            const isEditing = editingProfileId === p.id;
            const draft = isEditing && profileDraft?.id === p.id ? profileDraft : p;

            return (
              <div key={p.id} style={{ borderBottom: "1px solid #e5e7eb", background: "#fff" }}>
                {/* Header Row */}
                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    padding: "8px 12px",
                    background: isActive ? "#f0fdf4" : "#ffffff",
                    gap: 8,
                  }}
                >
                  <label style={{ display: "flex", alignItems: "center", gap: 6, cursor: "pointer", fontSize: 11, color: "#4b5563" }}>
                    <input
                      type="checkbox"
                      checked={!p.hidden}
                      onChange={(e) => handleToggleHidden(p, !e.target.checked)}
                    />
                    <span>{t("openRouterProfile.showOnDashboard")}</span>
                  </label>

                  <div style={{ fontWeight: 600, fontSize: 13, color: "#111827", flex: 1, minWidth: 140 }}>
                    {renamingId === p.id ? (
                      <input
                        style={{
                          padding: "2px 6px",
                          fontSize: 12,
                          border: "1px solid #d0d7de",
                          borderRadius: 4,
                          outline: "none",
                        }}
                        value={renameText}
                        onChange={(e) => setRenameText(e.target.value)}
                        onKeyDown={async (e) => {
                          if (e.key === "Enter") {
                            const trimmed = renameText.trim();
                            if (trimmed && trimmed !== p.display_name) {
                              const updated = { ...p, display_name: trimmed };
                              try {
                                await invoke("save_gateway_profile", { profile: updated });
                                await refreshConfig();
                              } catch (err) {
                                setActionError(String(err));
                              }
                            }
                            setRenamingId(null);
                          } else if (e.key === "Escape") {
                            setRenamingId(null);
                          }
                        }}
                        onBlur={async () => {
                          const trimmed = renameText.trim();
                          if (trimmed && trimmed !== p.display_name) {
                            const updated = { ...p, display_name: trimmed };
                            try {
                              await invoke("save_gateway_profile", { profile: updated });
                              await refreshConfig();
                            } catch (err) {
                              setActionError(String(err));
                            }
                          }
                          setRenamingId(null);
                        }}
                        autoFocus
                      />
                    ) : (
                      <span
                        onDoubleClick={() => {
                          setRenamingId(p.id);
                          setRenameText(p.display_name);
                        }}
                        title="Double-click to rename"
                        style={{ cursor: "pointer" }}
                      >
                        {p.display_name}
                      </span>
                    )}
                  </div>

                  {isActive ? (
                    <span style={{ fontSize: 10, fontWeight: 700, color: "#166534", background: "#dcfce7", padding: "2px 8px", borderRadius: 4 }}>
                      {t("apiKeyPanel.badgeActive")}
                    </span>
                  ) : (
                    <button
                      type="button"
                      className="btn btn-secondary btn-small"
                      onClick={() => handleSelectProfile(p.id)}
                      style={{ fontSize: 10, padding: "2px 8px" }}
                    >
                      {t("apiKeyPanel.setActive")}
                    </button>
                  )}

                  <button
                    type="button"
                    className="btn btn-secondary btn-small"
                    onClick={() => handleStartEdit(p)}
                    style={{ fontSize: 10, padding: "2px 8px" }}
                  >
                    {isEditing ? t("apiKeyPanel.collapse") : t("apiKeyPanel.edit")}
                  </button>

                  <button
                    type="button"
                    className="btn btn-secondary btn-small"
                    onClick={() => handleDuplicateProfile(p.id)}
                    style={{ fontSize: 10, padding: "2px 8px" }}
                    title="Duplicate Profile"
                  >
                    Duplicate
                  </button>

                  <button
                    type="button"
                    className="btn btn-small"
                    onClick={() => handleDeleteProfile(p)}
                    style={{
                      fontSize: 10,
                      padding: "2px 8px",
                      background: "#fff",
                      color: "#dc2626",
                      borderColor: "#fca5a5",
                    }}
                    title="Delete Profile"
                  >
                    Delete
                  </button>
                </div>

                {/* Edit Area */}
                {isEditing && profileDraft && (
                  <div style={{ padding: "12px 16px", background: "#f9fafb", borderTop: "1px solid #e5e7eb" }}>
                    <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
                      {CANONICAL_ROUTES.map(({ key, labelKey }) => {
                        const route = draft.routes[key] || {
                          provider: "deepseek",
                          model: "deepseek-v4-pro",
                        };

                        return (
                          <div
                            key={key}
                            style={{
                              display: "flex",
                              alignItems: "center",
                              gap: 8,
                              background: "#ffffff",
                              padding: "8px 12px",
                              borderRadius: 6,
                              border: "1px solid #e5e7eb",
                            }}
                          >
                            <div style={{ minWidth: 90, fontWeight: 600, fontSize: 12, color: "#374151" }}>
                              {t(labelKey)}
                            </div>

                            {/* Provider Select */}
                            <select
                              value={route.provider}
                              onChange={(e) => {
                                const newProv = e.target.value;
                                handleDraftRouteChange(key, (prev) => {
                                  if (newProv === "openrouter") {
                                    const defaultOrId = openRouterProfiles[0]?.id ?? null;
                                    return {
                                      provider: "openrouter",
                                      model: null,
                                      profile_id: defaultOrId,
                                      thinking_mode: null,
                                      reasoning_effort: null,
                                    };
                                  }
                                  const defaultModel = ELIGIBLE_MODELS[newProv]?.[0]?.id ?? "";
                                  let nextThinkingMode = prev.thinking_mode || "normal";
                                  if (newProv === "zai") {
                                    if (nextThinkingMode === "thinking_only") {
                                      nextThinkingMode = "thinking";
                                    } else if (nextThinkingMode === "normal" && defaultModel !== "glm-5.2") {
                                      nextThinkingMode = "thinking";
                                    }
                                  } else if (newProv === "anthropic") {
                                    if (nextThinkingMode === "thinking_only") {
                                      nextThinkingMode = "thinking";
                                    }
                                  }

                                  let nextReasoningEffort: string | null = null;
                                  if (newProv === "deepseek") {
                                    if (prev.reasoning_effort && ["low", "medium", "high"].includes(prev.reasoning_effort)) {
                                      nextReasoningEffort = prev.reasoning_effort;
                                    }
                                  } else if (newProv === "zai") {
                                    if (prev.reasoning_effort && ["low", "high", "max"].includes(prev.reasoning_effort)) {
                                      nextReasoningEffort = prev.reasoning_effort;
                                    }
                                  }

                                  return {
                                    provider: newProv,
                                    model: defaultModel,
                                    profile_id: null,
                                    thinking_mode: nextThinkingMode,
                                    reasoning_effort: nextReasoningEffort,
                                  };
                                });
                              }}
                              style={{
                                fontSize: 11,
                                padding: "3px 6px",
                                borderRadius: 4,
                                border: "1px solid #d1d5db",
                              }}
                            >
                              {ELIGIBLE_PROVIDERS.map((prov) => (
                                <option key={prov.id} value={prov.id}>
                                  {prov.label}
                                </option>
                              ))}
                            </select>

                            {/* Route Model / Profile Controls */}
                            {route.provider === "openrouter" ? (
                              <div style={{ display: "flex", alignItems: "center", gap: 6, flex: 1 }}>
                                <span style={{ fontSize: 11, color: "#6b7280" }}>Profile:</span>
                                <select
                                  value={route.profile_id || ""}
                                  onChange={(e) => {
                                    const orId = e.target.value;
                                    handleDraftRouteChange(key, (prev) => ({
                                      ...prev,
                                      profile_id: orId,
                                    }));
                                  }}
                                  style={{
                                    fontSize: 11,
                                    padding: "3px 6px",
                                    borderRadius: 4,
                                    border: "1px solid #d1d5db",
                                    flex: 1,
                                    maxWidth: 240,
                                  }}
                                >
                                  {openRouterProfiles.map((orp) => {
                                    const m = orp.models?.[key]?.upstream_model ?? "—";
                                    return (
                                      <option key={orp.id} value={orp.id}>
                                        {orp.display_name} ({m})
                                      </option>
                                    );
                                  })}
                                </select>
                              </div>
                            ) : (
                              <div style={{ display: "flex", alignItems: "center", gap: 8, flex: 1 }}>
                                {/* Direct Model Select */}
                                <select
                                  value={route.model || ""}
                                  onChange={(e) => {
                                    const m = e.target.value;
                                    handleDraftRouteChange(key, (prev) => {
                                      let nextThinkingMode = prev.thinking_mode;
                                      if (prev.provider === "zai" && m !== "glm-5.2" && (prev.thinking_mode === "normal" || !prev.thinking_mode)) {
                                        nextThinkingMode = "thinking";
                                      }
                                      return {
                                        ...prev,
                                        model: m,
                                        thinking_mode: nextThinkingMode,
                                      };
                                    });
                                  }}
                                  style={{
                                    fontSize: 11,
                                    padding: "3px 6px",
                                    borderRadius: 4,
                                    border: "1px solid #d1d5db",
                                    fontFamily: "var(--font-mono)",
                                  }}
                                >
                                  {(ELIGIBLE_MODELS[route.provider] || []).map((m) => (
                                    <option key={m.id} value={m.id}>
                                      {m.label}
                                    </option>
                                  ))}
                                </select>

                                {/* Thinking Mode */}
                                <span style={{ fontSize: 11, color: "#6b7280" }}>Mode:</span>
                                <select
                                  value={
                                    route.provider === "zai" && route.model !== "glm-5.2"
                                      ? "thinking"
                                      : (route.thinking_mode || "normal")
                                  }
                                  onChange={(e) => {
                                    const tm = e.target.value;
                                    handleDraftRouteChange(key, (prev) => ({
                                      ...prev,
                                      thinking_mode: tm,
                                    }));
                                  }}
                                  style={{
                                    fontSize: 11,
                                    padding: "3px 6px",
                                    borderRadius: 4,
                                    border: "1px solid #d1d5db",
                                  }}
                                >
                                  {(route.provider !== "zai" || route.model === "glm-5.2") && (
                                    <option value="normal">{t("apiKeyPanel.normalMode")}</option>
                                  )}
                                  <option value="thinking">{t("apiKeyPanel.thinkingModeOn")}</option>
                                  {route.provider === "deepseek" && (
                                    <option value="thinking_only">{t("apiKeyPanel.thinkingOnly")}</option>
                                  )}
                                </select>

                                {/* Reasoning Effort (DeepSeek and GLM) */}
                                {(route.provider === "deepseek" || route.provider === "zai") && (
                                  <>
                                    <span style={{ fontSize: 11, color: "#6b7280" }}>Effort:</span>
                                    <select
                                      value={route.reasoning_effort || ""}
                                      onChange={(e) => {
                                        const re = e.target.value || null;
                                        handleDraftRouteChange(key, (prev) => ({
                                          ...prev,
                                          reasoning_effort: re,
                                        }));
                                      }}
                                      style={{
                                        fontSize: 11,
                                        padding: "3px 6px",
                                        borderRadius: 4,
                                        border: "1px solid #d1d5db",
                                      }}
                                    >
                                      <option value="">{t("apiKeyPanel.reasoningEffortUnset")}</option>
                                      <option value="low">{t("apiKeyPanel.reasoningEffortLow")}</option>
                                      {route.provider === "deepseek" && (
                                        <option value="medium">{t("apiKeyPanel.reasoningEffortMedium")}</option>
                                      )}
                                      <option value="high">{t("apiKeyPanel.reasoningEffortHigh")}</option>
                                      {route.provider === "zai" && (
                                        <option value="max">{t("apiKeyPanel.reasoningEffortMaxFixed")}</option>
                                      )}
                                    </select>
                                  </>
                                )}
                              </div>
                            )}
                          </div>
                        );
                      })}

                      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 4 }}>
                        {saveSuccessId === p.id && (
                          <span style={{ fontSize: 11, color: "#166534", alignSelf: "center", fontWeight: 600 }}>
                            {t("apiKeyPanel.modelSaved")}
                          </span>
                        )}
                        <button
                          type="button"
                          className="btn btn-primary btn-small"
                          onClick={handleSaveDraft}
                          style={{ fontSize: 11, padding: "4px 12px" }}
                        >
                          {t("apiKeyPanel.save")}
                        </button>
                      </div>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
