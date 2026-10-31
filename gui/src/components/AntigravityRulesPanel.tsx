import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type {
  PovDirectStatus,
  PovDirectSettings,
  WebResearchPolicy,
  ResponseVerbosityPolicy,
  ExplorationPolicy,
  VerificationPolicy,
  PovDirectApplyOperation,
  PovDirectApplyRequest,
} from "../types";

export function formatDisplayPath(path?: string | null): string {
  if (!path) return "N/A";
  return path.replace(/\\/g, "/");
}

export default function AntigravityRulesPanel() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<PovDirectStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [customExplicit, setCustomExplicit] = useState(false);

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

  const fetchStatus = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const res = await invoke<PovDirectStatus>("get_pov_direct_status");
      setStatus(res);
      setDraft(res.settings);
    } catch (e) {
      setError(typeof e === "string" ? e : (e as Error).message || "Failed to load status");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchStatus();
  }, [fetchStatus]);

  // Determine current operation mode
  const currentMode = useMemo((): "gemini_38_standard" | "gemini_37_lightweight" | "custom" => {
    if (customExplicit) {
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
    setCustomExplicit(false);
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
    <div className="settings-tile">
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6, flexWrap: "wrap", gap: 8 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
          <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
            {t("antigravityRules.panelTitle")}
          </h3>
          {saving && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {successMessage && (
            <span style={{ fontSize: 12, color: "#1a7f37", fontWeight: 500 }}>
              ✓ {successMessage}
            </span>
          )}
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
            onClick={handleResetDefaults}
          >
            {t("antigravityRules.actions.resetDefaults")}
          </button>
          <button
            type="button"
            className="btn btn-primary"
            style={{ fontSize: 11, padding: "4px 12px", fontWeight: 600 }}
            disabled={saving || (status ? !status.can_apply : false)}
            onClick={handleSaveClick}
          >
            {t("antigravityRules.actions.apply")}
          </button>
        </div>
      </div>

      <p style={{ margin: "0 0 14px 0", fontSize: 12, color: "#656d76" }}>
        {t("antigravityRules.panelSubtitle")}
      </p>

      {/* Global Alerts / Messages */}
      {error && (
        <div
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
          {error}
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

      {/* Operation Mode Preset Segment */}
      <div style={{ borderTop: "1px solid var(--border-color, #e1e4e8)", paddingTop: 12, marginBottom: 14 }}>
        <div style={{ fontWeight: 600, fontSize: 13, color: "var(--text-primary, #24292f)", marginBottom: 6 }}>
          {t("antigravityRules.mode.title")}
        </div>
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
        <div className="pov-segment-desc">
          {currentMode === "gemini_38_standard" && t("antigravityRules.mode.descStandard")}
          {currentMode === "gemini_37_lightweight" && t("antigravityRules.mode.descLightweight")}
          {currentMode === "custom" && t("antigravityRules.mode.descCustom")}
        </div>
      </div>

      {/* Axis 1: Auto Activation */}
      <div style={{ borderTop: "1px solid var(--border-color, #e1e4e8)", paddingTop: 12, marginBottom: 14 }}>
        <div style={{ fontWeight: 600, fontSize: 13, color: "var(--text-primary, #24292f)", marginBottom: 6 }}>
          {t("antigravityRules.autoActivation.title")}
        </div>
        <div className="pov-segment-group">
          <button
            type="button"
            className={`pov-segment-btn ${draft.auto_activation ? "active" : ""}`}
            onClick={() => updateDraft({ ...draft, auto_activation: true })}
          >
            {t("antigravityRules.autoActivation.enabled")}
          </button>
          <button
            type="button"
            className={`pov-segment-btn ${!draft.auto_activation ? "active" : ""}`}
            onClick={() => updateDraft({ ...draft, auto_activation: false })}
          >
            {t("antigravityRules.autoActivation.disabled")}
          </button>
        </div>
        <div className="pov-segment-desc">
          {draft.auto_activation
            ? t("antigravityRules.autoActivation.descEnabled")
            : t("antigravityRules.autoActivation.descDisabled")}
        </div>
      </div>

      {/* Axis 2: Web Research */}
      <div style={{ borderTop: "1px solid var(--border-color, #e1e4e8)", paddingTop: 12, marginBottom: 14 }}>
        <div style={{ fontWeight: 600, fontSize: 13, color: "var(--text-primary, #24292f)", marginBottom: 6 }}>
          {t("antigravityRules.webResearch.title")}
        </div>
        <div className="pov-segment-group">
          <button
            type="button"
            className={`pov-segment-btn ${draft.web_research === "unrestricted" ? "active" : ""}`}
            onClick={() => updateDraft({ ...draft, web_research: "unrestricted" })}
          >
            {t("antigravityRules.webResearch.unrestricted")}
          </button>
          <button
            type="button"
            className={`pov-segment-btn ${draft.web_research === "requested_or_necessary" ? "active" : ""}`}
            onClick={() => updateDraft({ ...draft, web_research: "requested_or_necessary" })}
          >
            {t("antigravityRules.webResearch.requestedOrNecessary")}
          </button>
          <button
            type="button"
            className={`pov-segment-btn ${draft.web_research === "requested_only" ? "active" : ""}`}
            onClick={() => updateDraft({ ...draft, web_research: "requested_only" })}
          >
            {t("antigravityRules.webResearch.requestedOnly")}
          </button>
        </div>
        <div className="pov-segment-desc">
          {draft.web_research === "unrestricted" && t("antigravityRules.webResearch.descUnrestricted")}
          {draft.web_research === "requested_or_necessary" && t("antigravityRules.webResearch.descRequestedOrNecessary")}
          {draft.web_research === "requested_only" && t("antigravityRules.webResearch.descRequestedOnly")}
        </div>
      </div>

      {/* Axis 3: Response Verbosity */}
      <div style={{ borderTop: "1px solid var(--border-color, #e1e4e8)", paddingTop: 12, marginBottom: 14 }}>
        <div style={{ fontWeight: 600, fontSize: 13, color: "var(--text-primary, #24292f)", marginBottom: 6 }}>
          {t("antigravityRules.verbosity.title")}
        </div>
        <div className="pov-segment-group">
          <button
            type="button"
            className={`pov-segment-btn ${draft.response_verbosity === "standard" ? "active" : ""}`}
            onClick={() => updateDraft({ ...draft, response_verbosity: "standard" })}
          >
            {t("antigravityRules.verbosity.standard")}
          </button>
          <button
            type="button"
            className={`pov-segment-btn ${draft.response_verbosity === "concise" ? "active" : ""}`}
            onClick={() => updateDraft({ ...draft, response_verbosity: "concise" })}
          >
            {t("antigravityRules.verbosity.concise")}
          </button>
          <button
            type="button"
            className={`pov-segment-btn ${draft.response_verbosity === "minimal" ? "active" : ""}`}
            onClick={() => updateDraft({ ...draft, response_verbosity: "minimal" })}
          >
            {t("antigravityRules.verbosity.minimal")}
          </button>
          <button
            type="button"
            className={`pov-segment-btn ${draft.response_verbosity === "detailed" ? "active" : ""}`}
            onClick={() => updateDraft({ ...draft, response_verbosity: "detailed" })}
          >
            {t("antigravityRules.verbosity.detailed")}
          </button>
        </div>
        <div className="pov-segment-desc">
          {draft.response_verbosity === "standard" && t("antigravityRules.verbosity.descStandard")}
          {draft.response_verbosity === "concise" && t("antigravityRules.verbosity.descConcise")}
          {draft.response_verbosity === "minimal" && t("antigravityRules.verbosity.descMinimal")}
          {draft.response_verbosity === "detailed" && t("antigravityRules.verbosity.descDetailed")}
        </div>
      </div>

      {/* Axis 4: Repository Exploration */}
      <div style={{ borderTop: "1px solid var(--border-color, #e1e4e8)", paddingTop: 12, marginBottom: 14 }}>
        <div style={{ fontWeight: 600, fontSize: 13, color: "var(--text-primary, #24292f)", marginBottom: 6 }}>
          {t("antigravityRules.exploration.title")}
        </div>
        <div className="pov-segment-group">
          <button
            type="button"
            className={`pov-segment-btn ${draft.repository_exploration === "standard" ? "active" : ""}`}
            onClick={() => updateDraft({ ...draft, repository_exploration: "standard" })}
          >
            {t("antigravityRules.exploration.standard")}
          </button>
          <button
            type="button"
            className={`pov-segment-btn ${draft.repository_exploration === "minimal" ? "active" : ""}`}
            onClick={() => updateDraft({ ...draft, repository_exploration: "minimal" })}
          >
            {t("antigravityRules.exploration.minimal")}
          </button>
          <button
            type="button"
            className={`pov-segment-btn ${draft.repository_exploration === "detailed" ? "active" : ""}`}
            onClick={() => updateDraft({ ...draft, repository_exploration: "detailed" })}
          >
            {t("antigravityRules.exploration.detailed")}
          </button>
        </div>
        <div className="pov-segment-desc">
          {draft.repository_exploration === "standard" && t("antigravityRules.exploration.descStandard")}
          {draft.repository_exploration === "minimal" && t("antigravityRules.exploration.descMinimal")}
          {draft.repository_exploration === "detailed" && t("antigravityRules.exploration.descDetailed")}
        </div>
      </div>

      {/* Axis 5: Verification */}
      <div style={{ borderTop: "1px solid var(--border-color, #e1e4e8)", paddingTop: 12, marginBottom: 14 }}>
        <div style={{ fontWeight: 600, fontSize: 13, color: "var(--text-primary, #24292f)", marginBottom: 6 }}>
          {t("antigravityRules.verification.title")}
        </div>
        <div className="pov-segment-group">
          <button
            type="button"
            className={`pov-segment-btn ${draft.verification === "standard" ? "active" : ""}`}
            onClick={() => updateDraft({ ...draft, verification: "standard" })}
          >
            {t("antigravityRules.verification.standard")}
          </button>
          <button
            type="button"
            className={`pov-segment-btn ${draft.verification === "focused" ? "active" : ""}`}
            onClick={() => updateDraft({ ...draft, verification: "focused" })}
          >
            {t("antigravityRules.verification.focused")}
          </button>
          <button
            type="button"
            className={`pov-segment-btn ${draft.verification === "broad" ? "active" : ""}`}
            onClick={() => updateDraft({ ...draft, verification: "broad" })}
          >
            {t("antigravityRules.verification.broad")}
          </button>
        </div>
        <div className="pov-segment-desc">
          {draft.verification === "standard" && t("antigravityRules.verification.descStandard")}
          {draft.verification === "focused" && t("antigravityRules.verification.descFocused")}
          {draft.verification === "broad" && t("antigravityRules.verification.descBroad")}
        </div>
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
