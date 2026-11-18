import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import AgentOptPanel from "./AgentOptPanel";
import { getAgentOptErrorPresentation, unregisteredWorkspaceErrorMatchesPath } from "./agentOptErrorPresentation";
import type {
  PovDirectStatus,
  PovDirectSettings,
  WebResearchPolicy,
  ResponseVerbosityPolicy,
  ExplorationPolicy,
  VerificationPolicy,
  PovDirectApplyOperation,
  PovDirectApplyRequest,
  NerfEngine,
  NerfEngineOverview,
  EngineTransitionResult,
  ActivePlanContext,
} from "../types";

export function formatDisplayPath(path?: string | null): string {
  if (!path) return "N/A";
  return path.replace(/\\/g, "/");
}

export default function AntigravityRulesPanel({
  activePlanContext,
  onOpenAgentOptSettings,
}: {
  activePlanContext?: ActivePlanContext | null;
  onOpenAgentOptSettings?: () => void;
}) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<PovDirectStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [customExplicit, setCustomExplicit] = useState(false);
  const [draftDirty, setDraftDirty] = useState(false);
  const [engineOverview, setEngineOverview] = useState<NerfEngineOverview | null>(null);
  const [engineBusy, setEngineBusy] = useState(false);
  const [viewTab, setViewTab] = useState<NerfEngine>("poverello_rules");
  const lastPlanContextKey = useRef<string | null>(null);

  // Form draft settings
  const [draft, setDraft] = useState<PovDirectSettings>({
    auto_activation: true,
    web_research: "requested_or_necessary",
    response_verbosity: "concise",
    repository_exploration: "minimal",
    verification: "focused",
  });

  // Modal for unmanaged custom overwrite confirmation
  const [showConfirmModal, setShowConfirmModal] = useState(false);
  // Modal for legacy poverello migration confirmation
  const [showMigrateModal, setShowMigrateModal] = useState(false);

  const fetchEngineOverview = useCallback(async (syncTab = true) => {
    try {
      const res = await invoke<NerfEngineOverview | undefined>("get_antigravity_nerf_engine_overview");
      if (res) {
        setEngineOverview(res);
        if (syncTab) setViewTab(res.persisted_engine);
        if (res.pov_direct_status) setStatus(res.pov_direct_status);
      }
    } catch (e) {
      setError(typeof e === "string" ? e : (e as Error).message || "Failed to load engine overview");
    }
  }, []);
  const fetchStatus = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const res = await invoke<PovDirectStatus>("get_pov_direct_status");
      setStatus(res);
      if (res?.settings && !draftDirty) {
        setDraft(res.settings);
      }
      await fetchEngineOverview();
    } catch (e) {
      setError(typeof e === "string" ? e : (e as Error).message || "Failed to load status");
    } finally {
      setLoading(false);
    }
  }, [draftDirty, fetchEngineOverview]);

  useEffect(() => {
    void fetchStatus();
  }, [fetchStatus]);

  useEffect(() => {
    if (
      !activePlanContext?.resolved ||
      !activePlanContext.repoRoot ||
      !activePlanContext.planPath ||
      !engineOverview
    ) return;
    const key = [
      activePlanContext.surface,
      activePlanContext.repoRoot.toLowerCase(),
    ].join("|");
    if (lastPlanContextKey.current === key) return;
    lastPlanContextKey.current = key;
    if (engineOverview.persisted_engine !== "agentopt") return;
    let cancelled = false;
    setEngineBusy(true);
    setError(null);
    void invoke<EngineTransitionResult>("apply_selected_antigravity_nerf_engine")
      .then((res) => {
        if (cancelled || !res?.overview) return;
        setEngineOverview(res.overview);
        if (res.overview.pov_direct_status) setStatus(res.overview.pov_direct_status);
        if (res.overview.mismatch?.has_mismatch) {
          setError(res.overview.mismatch.reason || res.message);
          setSuccessMessage(null);
        } else {
          setSuccessMessage(res.message);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setError(typeof e === "string" ? e : (e as Error).message || "Failed to follow the selected Antigravity plan");
          setSuccessMessage(null);
          void fetchEngineOverview(false);
        }
      })
      .finally(() => {
        if (!cancelled) setEngineBusy(false);
      });
    return () => { cancelled = true; };
  }, [activePlanContext, engineOverview?.persisted_engine, engineOverview?.workspace_mode, fetchEngineOverview]);

  const activeEngine: NerfEngine = engineOverview?.persisted_engine ?? "poverello_rules";
  const presentedError = error ? getAgentOptErrorPresentation(error, t) : null;
  const targetInspectionExplainsError = viewTab === "agentopt"
    && !!error
    && !!engineOverview?.workspace_targets.some(({ inspection }) => unregisteredWorkspaceErrorMatchesPath(error, inspection.workspace_path)
      && !inspection.registered
      && (inspection.unregistered_files_detected || inspection.state === "not_initialized"));
  const presentedMismatchReason = engineOverview?.mismatch.reason
    ? getAgentOptErrorPresentation(engineOverview.mismatch.reason, t)
    : null;

  const handleSwitchEngine = async (targetEngine: NerfEngine) => {
    if (engineBusy) return;
    if (targetEngine === activeEngine && viewTab === targetEngine) return;
    setEngineBusy(true);
    setError(null);
    setSuccessMessage(null);
    try {
      const res = await invoke<EngineTransitionResult>("switch_antigravity_nerf_engine", {
        targetEngine,
      });
      if (res?.overview) {
        setEngineOverview(res.overview);
        setViewTab(res.overview.persisted_engine);
        if (res.overview.pov_direct_status) {
          setStatus(res.overview.pov_direct_status);
        }
        if (res.overview.mismatch?.has_mismatch) {
          setError(res.overview.mismatch.reason || res.message);
        } else {
          setSuccessMessage(res.message);
        }
      }
    } catch (e) {
      setError(typeof e === "string" ? e : (e as Error).message || "Failed to switch engine");
      if (targetEngine === "agentopt") {
        // Keep persisted engine unchanged; the CLI installer lives in Settings.
        setViewTab("agentopt");
        await fetchEngineOverview(false);
        const message = typeof e === "string" ? e : (e as Error).message;
        setError(message?.includes("Cannot switch to AgentOpt: select and prepare a target workspace first.")
          ? t("agentOpt.unavailableWorkspace")
          : message || t("agentOpt.unavailableWorkspace"));
      } else {
        setError(typeof e === "string" ? e : (e as Error).message || t("agentOpt.switchFailed"));
        await fetchEngineOverview();
      }
    } finally {
      setEngineBusy(false);
    }
  };

  const handleEngineTabKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    const tabs = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"]'));
    const currentIndex = tabs.indexOf(document.activeElement as HTMLButtonElement);
    if (currentIndex < 0 || tabs.length === 0) return;
    event.preventDefault();
    const nextIndex = event.key === "Home"
      ? 0
      : event.key === "End"
        ? tabs.length - 1
        : (currentIndex + (event.key === "ArrowRight" ? 1 : -1) + tabs.length) % tabs.length;
    const nextTab = tabs[nextIndex];
    nextTab.focus();
    nextTab.click();
  };

  const handleApplySelectedEngine = async () => {
    if (engineBusy) return;
    setEngineBusy(true);
    setError(null);
    setSuccessMessage(null);
    try {
      const res = await invoke<EngineTransitionResult>("apply_selected_antigravity_nerf_engine", {
      });
      if (res?.overview) {
        setEngineOverview(res.overview);
        setViewTab(res.overview.persisted_engine);
        if (res.overview.pov_direct_status) {
          setStatus(res.overview.pov_direct_status);
        }
        if (res.overview.mismatch?.has_mismatch) {
          setError(res.overview.mismatch.reason || res.message);
        } else {
          setSuccessMessage(res.message);
        }
      }
    } catch (e) {
      setError(typeof e === "string" ? e : (e as Error).message || "Failed to apply selected engine");
      await fetchEngineOverview();
    } finally {
      setEngineBusy(false);
    }
  };

  const handleWorkspacePrepared = async (): Promise<boolean> => {
    if (engineOverview?.persisted_engine !== "agentopt") {
      return true;
    }
    setEngineBusy(true);
    setError(null);
    setSuccessMessage(null);
    try {
      const res = await invoke<EngineTransitionResult>("apply_selected_antigravity_nerf_engine");
      if (res?.overview) {
        setEngineOverview(res.overview);
        if (res.overview.pov_direct_status) setStatus(res.overview.pov_direct_status);
        if (res.overview.mismatch?.has_mismatch) {
          setError(res.overview.mismatch.reason || res.message);
          return false;
        } else {
          setSuccessMessage(res.message);
        }
      }
      return true;
    } catch (e) {
      setError(typeof e === "string" ? e : (e as Error).message || "Failed to activate the prepared AgentOpt workspace");
      await fetchEngineOverview(false);
      return false;
    } finally {
      setEngineBusy(false);
    }
  };

  // Determine current operation mode
  const currentMode = useMemo((): "gemini_38_standard" | "gemini_37_lightweight" | "custom" => {
    if (!draft || customExplicit) {
      return "custom";
    }
    if (
      !draft.auto_activation &&
      draft.web_research === "unrestricted" &&
      draft.response_verbosity === "standard" &&
      draft.repository_exploration === "standard" &&
      draft.verification === "standard"
    ) {
      return "gemini_38_standard";
    }
    if (
      draft.auto_activation &&
      draft.web_research === "requested_or_necessary" &&
      draft.response_verbosity === "concise" &&
      draft.repository_exploration === "minimal" &&
      draft.verification === "focused"
    ) {
      return "gemini_37_lightweight";
    }
    return "custom";
  }, [draft, customExplicit]);

  const handleApplyPreset = (mode: "gemini_38_standard" | "gemini_37_lightweight" | "custom") => {
    if (mode === "gemini_38_standard") {
      setCustomExplicit(false);
      updateDraft({
        auto_activation: false,
        web_research: "unrestricted",
        response_verbosity: "standard",
        repository_exploration: "standard",
        verification: "standard",
      });
    } else if (mode === "gemini_37_lightweight") {
      setCustomExplicit(false);
      updateDraft({
        auto_activation: true,
        web_research: "requested_or_necessary",
        response_verbosity: "concise",
        repository_exploration: "minimal",
        verification: "focused",
      });
    } else {
      setCustomExplicit(true);
    }
  };

  const updateDraft = (newDraft: PovDirectSettings) => {
    setDraftDirty(true);
    setDraft(newDraft);
  };

  const executeApply = async (operation: PovDirectApplyOperation, confirmedOverwrite = false) => {
    if (!status) return;
    try {
      setSaving(true);
      setError(null);
      setSuccessMessage(null);

      const request: PovDirectApplyRequest = {
        operation,
        expected_state: status.state,
        expected_content_hash: status.content_hash,
        settings: draft,
      };

      const updated = await invoke<PovDirectStatus>("save_pov_direct_settings", { request });
      setStatus(updated);
      setDraft(updated.settings);
      setDraftDirty(false);
      setSuccessMessage("Settings saved and applied successfully.");
      setTimeout(() => setSuccessMessage(null), 3500);
    } catch (e) {
      setError(typeof e === "string" ? e : (e as Error).message || "Failed to save settings");
    } finally {
      setSaving(false);
      setShowConfirmModal(false);
      setShowMigrateModal(false);
    }
  };

  const handleSaveClick = () => {
    if (!status) return;
    if (status.state === "unmanaged_custom") {
      setShowConfirmModal(true);
    } else if (status.state === "legacy_poverello") {
      setShowMigrateModal(true);
    } else {
      void executeApply("apply");
    }
  };

  const handleResetDefaults = () => {
    setCustomExplicit(false);
    setDraft({
      auto_activation: false,
      web_research: "unrestricted",
      response_verbosity: "standard",
      repository_exploration: "standard",
      verification: "standard",
    });
  };

  const handleOpenFolder = async () => {
    try {
      await invoke("open_antigravity_rules_folder");
    } catch (e) {
      setError(typeof e === "string" ? e : (e as Error).message || "Failed to open rules folder");
    }
  };

  const stateBadgeText = useMemo(() => {
    if (!status) return t("antigravityRules.status.unknown");
    switch (status.state) {
      case "managed_synced":
        return t("antigravityRules.status.managedSynced");
      case "managed_drifted":
        return t("antigravityRules.status.managedDrifted");
      case "legacy_poverello":
        return t("antigravityRules.status.legacyPoverello");
      case "unmanaged_custom":
        return t("antigravityRules.status.unmanagedCustom");
      case "absent":
        return t("antigravityRules.status.absent");
      default:
        return t("antigravityRules.status.unknown");
    }
  }, [status, t]);

  if (loading) {
    return (
      <div className="settings-tile" style={{ color: "#656d76", fontSize: 12 }}>
        {t("antigravityRules.loading")}
      </div>
    );
  }

  const isOverByteLimit = status ? status.expanded_byte_size > status.byte_limit : false;

  return (
    <div className="settings-tile" data-testid="antigravity-rules-panel">
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 14, flexWrap: "wrap", gap: 8 }}>
        <div data-testid="antigravity-rules-heading" style={{ display: "flex", alignItems: "baseline", gap: 8, flexWrap: "wrap" }}>
          <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
            {t("antigravityRules.panelTitle")}
          </h3>
          <span style={{ fontSize: 12, color: "#656d76" }}>
            {t("antigravityRules.panelSubtitle")}
          </span>
          {saving && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {successMessage && (
            <span style={{ fontSize: 12, color: "#1a7f37", fontWeight: 500 }}>
              ✓ {successMessage}
            </span>
          )}
        </div>
      </div>

      {/* Engine pages */}
      <div
        data-testid="nerf-engine-selector"
        className="pov-engine-tabs"
        role="tablist"
        aria-label={t("agentOpt.engine")}
        onKeyDown={handleEngineTabKeyDown}
      >
        <button
          type="button"
          data-testid="engine-btn-poverello_rules"
          id="engine-tab-poverello-rules"
          role="tab"
          aria-selected={viewTab === "poverello_rules"}
          aria-controls="engine-tabpanel"
          tabIndex={viewTab === "poverello_rules" ? 0 : -1}
          className={`pov-engine-tab ${viewTab === "poverello_rules" ? "active" : ""}`}
          disabled={engineBusy}
          onClick={() => void handleSwitchEngine("poverello_rules")}
        >
          {t("agentOpt.enginePoverello")}
        </button>
        <button
          type="button"
          data-testid="engine-btn-agentopt"
          id="engine-tab-agentopt"
          role="tab"
          aria-selected={viewTab === "agentopt"}
          aria-controls="engine-tabpanel"
          tabIndex={viewTab === "agentopt" ? 0 : -1}
          className={`pov-engine-tab ${viewTab === "agentopt" ? "active" : ""}`}
          disabled={engineBusy}
          onClick={() => void handleSwitchEngine("agentopt")}
        >
          AgentOpt
        </button>
      </div>

      {/* Mismatch Recovery Banner */}
      {engineOverview?.mismatch.has_mismatch && (
        <div
          data-testid="engine-mismatch-banner"
          style={{
            padding: "10px 12px",
            marginBottom: 12,
            backgroundColor: "#fff8c5",
            border: "1px solid #d4a72c",
            borderRadius: 6,
            color: "#6e4a00",
            fontSize: 12,
            display: "flex",
            flexDirection: "column",
            gap: 6,
          }}
        >
          <div style={{ fontWeight: 600 }}>
            {t("agentOpt.mismatchTitle")}
          </div>
          <div>
            {t("agentOpt.mismatchSelected")} <code>{engineOverview.persisted_engine}</code> / {t("agentOpt.mismatchRuleState")}{" "}
            <code>{engineOverview.mismatch.observed_rule_state}</code> / {t("agentOpt.mismatchEnabledWorkspaces")}{" "}
            <code>
              {engineOverview.mismatch.enabled_workspaces.length > 0
                ? engineOverview.mismatch.enabled_workspaces.join(", ")
                : t("agentOpt.none")}
            </code>
          </div>
          {presentedMismatchReason && (
            <div>
              {presentedMismatchReason.message}
              {presentedMismatchReason.guidance && <div style={{ marginTop: 5 }}>{presentedMismatchReason.guidance}</div>}
            </div>
          )}
          <div>
            <button
              type="button"
              data-testid="apply-selected-engine-btn"
              className="btn btn-primary"
              disabled={engineBusy}
              onClick={() => void handleApplySelectedEngine()}
            >
              {t("agentOpt.applySelectedEngine")}
            </button>
          </div>
        </div>
      )}

      {/* Global Alerts / Messages */}
      {error && !targetInspectionExplainsError && (
        <div
          role="alert"
          style={{
            padding: "8px 12px",
            marginBottom: 12,
            backgroundColor: "#ffebe9",
            border: "1px solid #ff8182",
            borderRadius: 6,
            color: "#cf222e",
            fontSize: 12,
          }}
        >
          {presentedError?.guidance ? (
            <>
              <div>{presentedError.message}</div>
              <div style={{ marginTop: 5 }}>{presentedError.guidance}</div>
            </>
          ) : presentedError?.message}
        </div>
      )}

      {status?.block_reason && (
        <div
          style={{
            padding: "8px 12px",
            marginBottom: 12,
            backgroundColor: "#fff8c5",
            border: "1px solid #d4a72c",
            borderRadius: 6,
            color: "#6e4a00",
            fontSize: 12,
          }}
        >
          {status.block_reason}
        </div>
      )}

      {status?.legacy_sidecar_present && (
        <div
          style={{
            padding: "8px 12px",
            marginBottom: 12,
            backgroundColor: "#fff8c5",
            border: "1px solid #d4a72c",
            borderRadius: 6,
            color: "#6e4a00",
            fontSize: 12,
          }}
        >
          {t("antigravityRules.legacySidecarNotice")}
        </div>
      )}

      <div
        id="engine-tabpanel"
        data-testid="engine-tabpanel"
        role="tabpanel"
        tabIndex={0}
        aria-labelledby={viewTab === "agentopt" ? "engine-tab-agentopt" : "engine-tab-poverello-rules"}
      >
      {viewTab === "agentopt" ? (
        <AgentOptPanel
          overview={engineOverview}
          busy={engineBusy}
          onRefreshOverview={() => fetchEngineOverview(false)}
          onWorkspacePrepared={handleWorkspacePrepared}
          onError={setError}
          onSuccess={setSuccessMessage}
          onOpenAgentOptSettings={onOpenAgentOptSettings}
        />
      ) : (
      <>
      <div
        data-testid="poverello-rules-actions"
        style={{ display: "flex", justifyContent: "flex-end", alignItems: "center", gap: 8, marginBottom: 12 }}
      >
        <button
          type="button"
          className="btn"
          style={{ fontSize: 11, padding: "4px 10px" }}
          onClick={handleOpenFolder}
        >
          {t("antigravityRules.actions.openFolder")}
        </button>
        <button
          type="button"
          className="btn"
          style={{ fontSize: 11, padding: "4px 10px" }}
          disabled={activeEngine !== "poverello_rules"}
          onClick={handleResetDefaults}
        >
          {t("antigravityRules.actions.resetDefaults")}
        </button>
        <button
          type="button"
          className="btn btn-primary"
          style={{ fontSize: 11, padding: "4px 12px", fontWeight: 600 }}
          disabled={activeEngine !== "poverello_rules" || saving || (status ? !status.can_apply : false)}
          onClick={handleSaveClick}
        >
          {t("antigravityRules.actions.apply")}
        </button>
      </div>
      {/* 6 Settings Rows (§10.3) */}
      <div className="pov-settings-grid">
        {/* Row 0: Mode Preset */}
        <div id="axis-0" className="pov-setting-row">
          <div className="pov-setting-label">
            {t("antigravityRules.mode.title")}
          </div>
          <div className="pov-setting-control">
            <div className="pov-segment-group">
              <button
                type="button"
                className={`pov-segment-btn ${currentMode === "gemini_38_standard" ? "active" : ""}`}
                onClick={() => handleApplyPreset("gemini_38_standard")}
              >
                {t("antigravityRules.mode.presetStandard")}
              </button>
              <button
                type="button"
                className={`pov-segment-btn ${currentMode === "gemini_37_lightweight" ? "active" : ""}`}
                onClick={() => handleApplyPreset("gemini_37_lightweight")}
              >
                {t("antigravityRules.mode.presetLightweight")}
              </button>
              <button
                type="button"
                className={`pov-segment-btn ${currentMode === "custom" ? "active" : ""}`}
                onClick={() => handleApplyPreset("custom")}
              >
                {t("antigravityRules.mode.presetCustom")}
              </button>
            </div>
          </div>
        </div>
        <div className="pov-segment-desc" style={{ marginTop: -2, marginBottom: 4 }}>
          {currentMode === "gemini_38_standard" && t("antigravityRules.mode.descStandard")}
          {currentMode === "gemini_37_lightweight" && t("antigravityRules.mode.descLightweight")}
          {currentMode === "custom" && t("antigravityRules.mode.descCustom")}
        </div>

        {/* Rows 1-5: Individual settings shown only when Custom preset is selected */}
        {currentMode === "custom" && (
          <>
        {/* Row 1: Auto Activation */}
        <div id="axis-1" className="pov-setting-row">
          <div className="pov-setting-label">
            {t("antigravityRules.autoActivation.title")}
          </div>
          <div className="pov-setting-control">
            <div className="pov-segment-group">
              <button
                type="button"
                className={`pov-segment-btn ${draft.auto_activation ? "active" : ""}`}
                title={t("antigravityRules.autoActivation.descEnabled")}
                onClick={() => updateDraft({ ...draft, auto_activation: true })}
              >
                {t("antigravityRules.autoActivation.enabled")}
              </button>
              <button
                type="button"
                className={`pov-segment-btn ${!draft.auto_activation ? "active" : ""}`}
                title={t("antigravityRules.autoActivation.descDisabled")}
                onClick={() => updateDraft({ ...draft, auto_activation: false })}
              >
                {t("antigravityRules.autoActivation.disabled")}
              </button>
            </div>
          </div>
        </div>

        {/* Row 2: Web Research */}
        <div id="axis-2" className="pov-setting-row">
          <div className="pov-setting-label">
            {t("antigravityRules.webResearch.title")}
          </div>
          <div className="pov-setting-control">
            <div className="pov-segment-group">
              <button
                type="button"
                className={`pov-segment-btn ${draft.web_research === "unrestricted" ? "active" : ""}`}
                title={t("antigravityRules.webResearch.descUnrestricted")}
                onClick={() => updateDraft({ ...draft, web_research: "unrestricted" })}
              >
                {t("antigravityRules.webResearch.unrestricted")}
              </button>
              <button
                type="button"
                className={`pov-segment-btn ${draft.web_research === "requested_or_necessary" ? "active" : ""}`}
                title={t("antigravityRules.webResearch.descRequestedOrNecessary")}
                onClick={() => updateDraft({ ...draft, web_research: "requested_or_necessary" })}
              >
                {t("antigravityRules.webResearch.requestedOrNecessary")}
              </button>
              <button
                type="button"
                className={`pov-segment-btn ${draft.web_research === "requested_only" ? "active" : ""}`}
                title={t("antigravityRules.webResearch.descRequestedOnly")}
                onClick={() => updateDraft({ ...draft, web_research: "requested_only" })}
              >
                {t("antigravityRules.webResearch.requestedOnly")}
              </button>
            </div>
          </div>
        </div>

        {/* Row 3: Response Verbosity */}
        <div id="axis-3" className="pov-setting-row">
          <div className="pov-setting-label">
            {t("antigravityRules.verbosity.title")}
          </div>
          <div className="pov-setting-control">
            <div className="pov-segment-group">
              <button
                type="button"
                className={`pov-segment-btn ${draft.response_verbosity === "standard" ? "active" : ""}`}
                title={t("antigravityRules.verbosity.descStandard")}
                onClick={() => updateDraft({ ...draft, response_verbosity: "standard" })}
              >
                {t("antigravityRules.verbosity.standard")}
              </button>
              <button
                type="button"
                className={`pov-segment-btn ${draft.response_verbosity === "concise" ? "active" : ""}`}
                title={t("antigravityRules.verbosity.descConcise")}
                onClick={() => updateDraft({ ...draft, response_verbosity: "concise" })}
              >
                {t("antigravityRules.verbosity.concise")}
              </button>
              <button
                type="button"
                className={`pov-segment-btn ${draft.response_verbosity === "minimal" ? "active" : ""}`}
                title={t("antigravityRules.verbosity.descMinimal")}
                onClick={() => updateDraft({ ...draft, response_verbosity: "minimal" })}
              >
                {t("antigravityRules.verbosity.minimal")}
              </button>
              <button
                type="button"
                className={`pov-segment-btn ${draft.response_verbosity === "detailed" ? "active" : ""}`}
                title={t("antigravityRules.verbosity.descDetailed")}
                onClick={() => updateDraft({ ...draft, response_verbosity: "detailed" })}
              >
                {t("antigravityRules.verbosity.detailed")}
              </button>
            </div>
          </div>
        </div>

        {/* Row 4: Repository Exploration */}
        <div id="axis-4" className="pov-setting-row">
          <div className="pov-setting-label">
            {t("antigravityRules.exploration.title")}
          </div>
          <div className="pov-setting-control">
            <div className="pov-segment-group">
              <button
                type="button"
                className={`pov-segment-btn ${draft.repository_exploration === "standard" ? "active" : ""}`}
                title={t("antigravityRules.exploration.descStandard")}
                onClick={() => updateDraft({ ...draft, repository_exploration: "standard" })}
              >
                {t("antigravityRules.exploration.standard")}
              </button>
              <button
                type="button"
                className={`pov-segment-btn ${draft.repository_exploration === "minimal" ? "active" : ""}`}
                title={t("antigravityRules.exploration.descMinimal")}
                onClick={() => updateDraft({ ...draft, repository_exploration: "minimal" })}
              >
                {t("antigravityRules.exploration.minimal")}
              </button>
              <button
                type="button"
                className={`pov-segment-btn ${draft.repository_exploration === "detailed" ? "active" : ""}`}
                title={t("antigravityRules.exploration.descDetailed")}
                onClick={() => updateDraft({ ...draft, repository_exploration: "detailed" })}
              >
                {t("antigravityRules.exploration.detailed")}
              </button>
            </div>
          </div>
        </div>

        {/* Row 5: Verification */}
        <div id="axis-5" className="pov-setting-row">
          <div className="pov-setting-label">
            {t("antigravityRules.verification.title")}
          </div>
          <div className="pov-setting-control">
            <div className="pov-segment-group">
              <button
                type="button"
                className={`pov-segment-btn ${draft.verification === "standard" ? "active" : ""}`}
                title={t("antigravityRules.verification.descStandard")}
                onClick={() => updateDraft({ ...draft, verification: "standard" })}
              >
                {t("antigravityRules.verification.standard")}
              </button>
              <button
                type="button"
                className={`pov-segment-btn ${draft.verification === "focused" ? "active" : ""}`}
                title={t("antigravityRules.verification.descFocused")}
                onClick={() => updateDraft({ ...draft, verification: "focused" })}
              >
                {t("antigravityRules.verification.focused")}
              </button>
              <button
                type="button"
                className={`pov-segment-btn ${draft.verification === "broad" ? "active" : ""}`}
                title={t("antigravityRules.verification.descBroad")}
                onClick={() => updateDraft({ ...draft, verification: "broad" })}
              >
                {t("antigravityRules.verification.broad")}
              </button>
            </div>
          </div>
        </div>
          </>
        )}
      </div>

      {/* Advanced Details Accordion */}
      <div style={{ borderTop: "1px solid var(--border-color, #e1e4e8)", paddingTop: 12, marginTop: 14 }}>
        <button
          type="button"
          style={{
            background: "none",
            border: "none",
            color: "var(--text-primary, #24292f)",
            fontSize: 12,
            fontWeight: 600,
            cursor: "pointer",
            display: "flex",
            alignItems: "center",
            gap: 6,
            padding: 0,
            marginBottom: showAdvanced ? 10 : 0,
          }}
          onClick={() => setShowAdvanced(!showAdvanced)}
        >
          <span>{showAdvanced ? "▼" : "▶"}</span>
          <span>{t("antigravityRules.advanced.title")}</span>
        </button>

        {showAdvanced && (
          <div style={{ background: "#f6f8fa", border: "1px solid #d0d7de", borderRadius: 6, padding: "10px 12px", marginTop: 8 }}>
            <div style={{ fontSize: 12, fontWeight: 600, color: "#24292f", marginBottom: 6 }}>
              {stateBadgeText}
            </div>

            <div style={{ fontSize: 11, color: "#57606a", marginBottom: 4 }}>
              <strong>{t("antigravityRules.advanced.rulePath")}</strong> <code>{formatDisplayPath(status?.rule_path)}</code>
            </div>

            <div style={{ fontSize: 11, color: isOverByteLimit ? "#cf222e" : "#57606a", marginBottom: 8 }}>
              <strong>{t("antigravityRules.advanced.ruleSize")}</strong> {status ? `${status.expanded_byte_size.toLocaleString()} / ${status.byte_limit.toLocaleString()} bytes` : "N/A"}
              {isOverByteLimit && ` ${t("antigravityRules.advanced.budgetWarning")}`}
            </div>

            {status?.preview_text && (
              <div>
                <div style={{ fontSize: 11, fontWeight: 600, color: "#24292f", marginBottom: 4 }}>
                  {t("antigravityRules.advanced.rulePreview")}
                </div>
                <pre
                  style={{
                    backgroundColor: "#ffffff",
                    border: "1px solid #d0d7de",
                    borderRadius: 4,
                    padding: 8,
                    fontSize: 11,
                    maxHeight: 180,
                    overflowY: "auto",
                    whiteSpace: "pre-wrap",
                    margin: 0,
                  }}
                >
                  {status.preview_text}
                </pre>
              </div>
            )}
          </div>
        )}
      </div>
      </>
      )}
      </div>

      {/* Confirmation Modal for Unmanaged Custom */}
      {showConfirmModal && (
        <div
          style={{
            position: "fixed",
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            backgroundColor: "rgba(0,0,0,0.5)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            zIndex: 1000,
          }}
        >
          <div
            style={{
              backgroundColor: "#ffffff",
              padding: 20,
              borderRadius: 8,
              maxWidth: 440,
              boxShadow: "0 4px 12px rgba(0,0,0,0.15)",
            }}
          >
            <h4 style={{ margin: "0 0 10px 0", fontSize: 14 }}>{t("antigravityRules.modal.overwriteTitle")}</h4>
            <p style={{ margin: "0 0 16px 0", fontSize: 12, color: "#57606a" }}>
              {t("antigravityRules.modal.overwriteDesc")}
            </p>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
              <button type="button" className="btn" onClick={() => setShowConfirmModal(false)}>
                {t("antigravityRules.modal.cancel")}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => void executeApply("apply", true)}
              >
                {t("antigravityRules.modal.btnOverwriteApply")}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Migration Modal for Legacy Poverello */}
      {showMigrateModal && (
        <div
          style={{
            position: "fixed",
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            backgroundColor: "rgba(0,0,0,0.5)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            zIndex: 1000,
          }}
        >
          <div
            style={{
              backgroundColor: "#ffffff",
              padding: 20,
              borderRadius: 8,
              maxWidth: 440,
              boxShadow: "0 4px 12px rgba(0,0,0,0.15)",
            }}
          >
            <h4 style={{ margin: "0 0 10px 0", fontSize: 14 }}>{t("antigravityRules.modal.migrateTitle")}</h4>
            <p style={{ margin: "0 0 16px 0", fontSize: 12, color: "#57606a" }}>
              {t("antigravityRules.modal.migrateDesc")}
            </p>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
              <button type="button" className="btn" onClick={() => setShowMigrateModal(false)}>
                {t("antigravityRules.modal.cancel")}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => void executeApply("apply", true)}
              >
                {t("antigravityRules.modal.btnMigrateApply")}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
