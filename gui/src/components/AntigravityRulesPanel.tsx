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

export default function AntigravityRulesPanel() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<PovDirectStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

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
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchStatus();
  }, [fetchStatus]);

  // Compute live preview based on draft
  const previewLines = useMemo(() => {
    const lines = [
      `Automatic activation: ${
        draft.auto_activation
          ? "Enabled (model_decision)"
          : "Disabled (manual only: activated via @pov-direct mention; /pov-direct invokes canonical Skill only)"
      }`,
      `Web research: ${
        draft.web_research === "requested_only"
          ? "When requested only"
          : draft.web_research === "requested_or_necessary"
          ? "When requested or necessary"
          : "Unrestricted"
      }`,
      `Response verbosity: ${draft.response_verbosity}`,
      `Repository exploration: ${draft.repository_exploration}`,
      `Verification: ${draft.verification}`,
    ];
    return lines.join("\n");
  }, [draft]);

  const handleOpenFolder = async () => {
    try {
      await invoke("open_antigravity_rules_folder");
    } catch (e: unknown) {
      setError(String(e));
    }
  };

  const handleResetDefaults = () => {
    setDraft({
      auto_activation: true,
      web_research: "requested_or_necessary",
      response_verbosity: "concise",
      repository_exploration: "minimal",
      verification: "focused",
    });
  };

  const executeSave = async (operation: PovDirectApplyOperation = "apply") => {
    if (!status) return;
    try {
      setSaving(true);
      setError(null);
      setSuccessMessage(null);
      const req: PovDirectApplyRequest = {
        settings: draft,
        operation,
        expected_state: status.state,
        expected_content_hash: status.content_hash,
      };
      const res = await invoke<PovDirectStatus>("save_pov_direct_settings", {
        request: req,
      });
      setStatus(res);
      setDraft(res.settings);
      setShowConfirmModal(false);
      setShowMigrateModal(false);
      setSuccessMessage(t("antigravityRules.status.synced"));
      setTimeout(() => setSuccessMessage(null), 4000);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleSaveClick = () => {
    if (status?.state === "unmanaged_custom") {
      setShowConfirmModal(true);
    } else if (status?.state === "legacy_poverello") {
      setShowMigrateModal(true);
    } else {
      void executeSave("apply");
    }
  };

  // State badge styling and label
  const stateBadge = useMemo(() => {
    if (!status) return null;
    let label = "";
    let bg = "#6b7280";
    let color = "#ffffff";

    switch (status.state) {
      case "managed_synced":
        label = t("antigravityRules.status.synced");
        bg = "#10b981"; // green
        break;
      case "managed_drifted":
        label = t("antigravityRules.status.externalDrift");
        bg = "#f59e0b"; // amber
        break;
      case "legacy_poverello":
        label = t("antigravityRules.status.legacyFile");
        bg = "#3b82f6"; // blue
        break;
      case "unmanaged_custom":
        label = t("antigravityRules.status.unmanaged");
        bg = "#ef4444"; // red
        break;
      case "absent":
      default:
        label = t("antigravityRules.status.notApplied");
        bg = "#6b7280"; // gray
        break;
    }

    return (
      <span
        style={{
          display: "inline-block",
          padding: "2px 8px",
          borderRadius: "12px",
          fontSize: "11px",
          fontWeight: 600,
          backgroundColor: bg,
          color,
          marginLeft: "8px",
        }}
      >
        {label}
      </span>
    );
  }, [status, t]);

  const webResearchOptions: { value: WebResearchPolicy; label: string }[] = [
    {
      value: "requested_only",
      label: t("antigravityRules.webResearch.requestedOnly"),
    },
    {
      value: "requested_or_necessary",
      label: t("antigravityRules.webResearch.requestedOrNecessary"),
    },
    {
      value: "unrestricted",
      label: t("antigravityRules.webResearch.unrestricted"),
    },
  ];

  const verbosityOptions: { value: ResponseVerbosityPolicy; label: string }[] = [
    { value: "minimal", label: t("antigravityRules.verbosity.minimal") },
    { value: "concise", label: t("antigravityRules.verbosity.concise") },
    { value: "standard", label: t("antigravityRules.verbosity.standard") },
    { value: "detailed", label: t("antigravityRules.verbosity.detailed") },
  ];

  const explorationOptions: { value: ExplorationPolicy; label: string }[] = [
    { value: "minimal", label: t("antigravityRules.exploration.minimal") },
    { value: "standard", label: t("antigravityRules.exploration.standard") },
    { value: "detailed", label: t("antigravityRules.exploration.detailed") },
  ];

  const verificationOptions: { value: VerificationPolicy; label: string }[] = [
    { value: "focused", label: t("antigravityRules.verification.focused") },
    { value: "standard", label: t("antigravityRules.verification.standard") },
    { value: "broad", label: t("antigravityRules.verification.broad") },
  ];

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
          <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)", display: "inline-flex", alignItems: "center" }}>
            {t("antigravityRules.panelTitle")}
            {stateBadge}
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
        </div>
      </div>

      <p style={{ margin: "0 0 12px 0", fontSize: 12, color: "#656d76" }}>
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
          {t("antigravityRules.alerts.legacySidecar")}
        </div>
      )}

      {/* Setting 1: Auto Activate Toggle */}
      <div style={{ borderTop: "1px solid #e1e4e8", paddingTop: 12, marginBottom: 14 }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <div style={{ fontWeight: 600, fontSize: 13, color: "#24292f" }}>
              {t("antigravityRules.autoActivation.title")}
            </div>
            <div style={{ fontSize: 12, color: "#57606a", marginTop: 2 }}>
              {t("antigravityRules.autoActivation.desc")}
            </div>
          </div>
          <label style={{ display: "flex", alignItems: "center", cursor: "pointer" }}>
            <input
              type="checkbox"
              checked={draft.auto_activation}
              onChange={(e) => setDraft({ ...draft, auto_activation: e.target.checked })}
              style={{ width: 16, height: 16, cursor: "pointer" }}
            />
          </label>
        </div>
      </div>

      {/* Setting 2: Web Research Radio Group */}
      <div style={{ borderTop: "1px solid #e1e4e8", paddingTop: 12, marginBottom: 14 }}>
        <div style={{ fontWeight: 600, fontSize: 13, color: "#24292f", marginBottom: 6 }}>
          {t("antigravityRules.webResearch.title")}
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {webResearchOptions.map((opt) => (
            <label
              key={opt.value}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                fontSize: 13,
                color: "#24292f",
                cursor: "pointer",
              }}
            >
              <input
                type="radio"
                name="web_research"
                value={opt.value}
                checked={draft.web_research === opt.value}
                onChange={() => setDraft({ ...draft, web_research: opt.value })}
                style={{ cursor: "pointer" }}
              />
              <span style={{ fontWeight: 500 }}>{opt.label}</span>
            </label>
          ))}
        </div>
      </div>

      {/* Setting 3: Response Verbosity Pill Selector */}
      <div style={{ borderTop: "1px solid #e1e4e8", paddingTop: 12, marginBottom: 14 }}>
        <div style={{ fontWeight: 600, fontSize: 13, color: "#24292f", marginBottom: 6 }}>
          {t("antigravityRules.verbosity.title")}
        </div>
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
          {verbosityOptions.map((opt) => {
            const active = draft.response_verbosity === opt.value;
            return (
              <button
                key={opt.value}
                type="button"
                className={active ? "btn btn-primary" : "btn"}
                style={{ fontSize: 12, padding: "3px 12px", borderRadius: 16 }}
                onClick={() => setDraft({ ...draft, response_verbosity: opt.value })}
              >
                {opt.label}
              </button>
            );
          })}
        </div>
      </div>

      {/* Setting 4: Repository Exploration Pill Selector */}
      <div style={{ borderTop: "1px solid #e1e4e8", paddingTop: 12, marginBottom: 14 }}>
        <div style={{ fontWeight: 600, fontSize: 13, color: "#24292f", marginBottom: 6 }}>
          {t("antigravityRules.exploration.title")}
        </div>
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
          {explorationOptions.map((opt) => {
            const active = draft.repository_exploration === opt.value;
            return (
              <button
                key={opt.value}
                type="button"
                className={active ? "btn btn-primary" : "btn"}
                style={{ fontSize: 12, padding: "3px 12px", borderRadius: 16 }}
                onClick={() => setDraft({ ...draft, repository_exploration: opt.value })}
              >
                {opt.label}
              </button>
            );
          })}
        </div>
      </div>

      {/* Setting 5: Verification Scope Pill Selector */}
      <div style={{ borderTop: "1px solid #e1e4e8", paddingTop: 12, marginBottom: 14 }}>
        <div style={{ fontWeight: 600, fontSize: 13, color: "#24292f", marginBottom: 6 }}>
          {t("antigravityRules.verification.title")}
        </div>
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
          {verificationOptions.map((opt) => {
            const active = draft.verification === opt.value;
            return (
              <button
                key={opt.value}
                type="button"
                className={active ? "btn btn-primary" : "btn"}
                style={{ fontSize: 12, padding: "3px 12px", borderRadius: 16 }}
                onClick={() => setDraft({ ...draft, verification: opt.value })}
              >
                {opt.label}
              </button>
            );
          })}
        </div>
      </div>

      {/* Target Rule File Info */}
      <div style={{ borderTop: "1px solid #e1e4e8", paddingTop: 12, marginBottom: 14 }}>
        <div style={{ fontWeight: 600, fontSize: 13, color: "#24292f", marginBottom: 4 }}>
          {t("antigravityRules.ruleFile.title")}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <code
            style={{
              fontSize: 11,
              fontFamily: "var(--font-mono)",
              color: "#57606a",
              backgroundColor: "#f6f8fa",
              padding: "4px 8px",
              borderRadius: 4,
              border: "1px solid #d0d7de",
              flex: 1,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {status?.rule_path}
          </code>
          <span
            style={{
              fontSize: 11,
              fontWeight: 600,
              color: "#0969da",
              backgroundColor: "#ddf4ff",
              border: "1px solid #54aeff66",
              padding: "2px 6px",
              borderRadius: 4,
              whiteSpace: "nowrap",
            }}
          >
            {t("antigravityRules.ruleFile.managedBadge")}
          </span>
        </div>
      </div>

      {/* Live Settings Preview */}
      <div style={{ borderTop: "1px solid #e1e4e8", paddingTop: 12, marginBottom: 14 }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6 }}>
          <div style={{ fontWeight: 600, fontSize: 13, color: "#24292f" }}>
            {t("antigravityRules.ruleFile.previewTitle")}
          </div>
          {status && (
            <div
              style={{
                fontSize: 11,
                color: isOverByteLimit ? "#cf222e" : "#57606a",
                fontWeight: isOverByteLimit ? 600 : 400,
              }}
            >
              {t("antigravityRules.ruleFile.sizeLabel")}: {status.expanded_byte_size.toLocaleString()} /{" "}
              {status.byte_limit.toLocaleString()} bytes
            </div>
          )}
        </div>
        <pre
          style={{
            backgroundColor: "#f6f8fa",
            border: "1px solid #d0d7de",
            borderRadius: 6,
            padding: "8px 12px",
            fontSize: 12,
            color: "#24292f",
            fontFamily: "var(--font-mono)",
            margin: 0,
            whiteSpace: "pre-wrap",
          }}
        >
          {previewLines}
        </pre>
      </div>

      {/* Action Buttons */}
      <div style={{ display: "flex", justifyContent: "flex-end", alignItems: "center", borderTop: "1px solid #e1e4e8", paddingTop: 14 }}>
        <button
          type="button"
          className="btn btn-primary"
          disabled={saving || !status?.can_apply || isOverByteLimit}
          onClick={handleSaveClick}
          style={{ fontSize: 12, padding: "5px 16px" }}
        >
          {saving ? t("antigravityRules.saving") : t("antigravityRules.actions.apply")}
        </button>
      </div>

      {/* UnmanagedCustom Adopt Confirmation Modal */}
      {showConfirmModal && (
        <div
          style={{
            position: "fixed",
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            backgroundColor: "rgba(0, 0, 0, 0.5)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            zIndex: 1000,
          }}
        >
          <div
            style={{
              backgroundColor: "#ffffff",
              borderRadius: "8px",
              padding: "24px",
              maxWidth: "500px",
              width: "90%",
              boxShadow: "0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 10px 10px -5px rgba(0, 0, 0, 0.04)",
            }}
          >
            <h4 style={{ margin: "0 0 12px 0", fontSize: "16px", fontWeight: 700, color: "#1f2937" }}>
              {t("antigravityRules.modal.unmanagedWarning")}
            </h4>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: "10px", marginTop: "16px" }}>
              <button
                type="button"
                onClick={() => setShowConfirmModal(false)}
                disabled={saving}
                style={{
                  padding: "6px 14px",
                  borderRadius: "6px",
                  fontSize: "12px",
                  fontWeight: 500,
                  backgroundColor: "#ffffff",
                  color: "#374151",
                  border: "1px solid #d1d5db",
                  cursor: "pointer",
                }}
              >
                {t("antigravityRules.modal.cancel")}
              </button>
              <button
                type="button"
                onClick={() => executeSave("adopt_with_backup")}
                disabled={saving}
                style={{
                  padding: "6px 14px",
                  borderRadius: "6px",
                  fontSize: "12px",
                  fontWeight: 500,
                  backgroundColor: "#2563eb",
                  color: "#ffffff",
                  border: "none",
                  cursor: "pointer",
                }}
              >
                {t("antigravityRules.modal.adopt")}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* LegacyPoverello Migrate Confirmation Modal */}
      {showMigrateModal && (
        <div
          style={{
            position: "fixed",
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            backgroundColor: "rgba(0, 0, 0, 0.5)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            zIndex: 1000,
          }}
        >
          <div
            style={{
              backgroundColor: "#ffffff",
              borderRadius: "8px",
              padding: "24px",
              maxWidth: "500px",
              width: "90%",
              boxShadow: "0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 10px 10px -5px rgba(0, 0, 0, 0.04)",
            }}
          >
            <h4 style={{ margin: "0 0 12px 0", fontSize: "16px", fontWeight: 700, color: "#1f2937" }}>
              {t("antigravityRules.modal.migrateTitle")}
            </h4>
            <p style={{ fontSize: "13px", color: "#4b5563", margin: "0 0 16px 0", lineHeight: 1.5 }}>
              {t("antigravityRules.modal.migrateDesc")}
            </p>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: "10px", marginTop: "16px" }}>
              <button
                type="button"
                onClick={() => setShowMigrateModal(false)}
                disabled={saving}
                style={{
                  padding: "6px 14px",
                  borderRadius: "6px",
                  fontSize: "12px",
                  fontWeight: 500,
                  backgroundColor: "#ffffff",
                  color: "#374151",
                  border: "1px solid #d1d5db",
                  cursor: "pointer",
                }}
              >
                {t("antigravityRules.modal.cancel")}
              </button>
              <button
                type="button"
                onClick={() => executeSave("migrate_with_backup")}
                disabled={saving}
                style={{
                  padding: "6px 14px",
                  borderRadius: "6px",
                  fontSize: "12px",
                  fontWeight: 500,
                  backgroundColor: "#2563eb",
                  color: "#ffffff",
                  border: "none",
                  cursor: "pointer",
                }}
              >
                {t("antigravityRules.modal.migrate")}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
