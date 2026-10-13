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
      <div style={{ padding: "24px", color: "#6b7280" }}>
        {t("antigravityRules.loading")}
      </div>
    );
  }

  const isOverByteLimit = status ? status.expanded_byte_size > status.byte_limit : false;

  return (
    <div style={{ padding: "24px", maxWidth: "900px", margin: "0 auto" }}>
      {/* Header */}
      <div style={{ marginBottom: "20px" }}>
        <h2 style={{ fontSize: "20px", fontWeight: 700, margin: 0, color: "#111827" }}>
          {t("antigravityRules.panelTitle")}
        </h2>
        <div style={{ fontSize: "13px", color: "#6b7280", marginTop: "4px" }}>
          {t("antigravityRules.panelSubtitle")}
        </div>
      </div>

      {/* Global Alerts / Messages */}
      {error && (
        <div
          style={{
            padding: "10px 14px",
            marginBottom: "16px",
            backgroundColor: "#fef2f2",
            border: "1px solid #f87171",
            borderRadius: "6px",
            color: "#b91c1c",
            fontSize: "13px",
          }}
        >
          {error}
        </div>
      )}

      {successMessage && (
        <div
          style={{
            padding: "10px 14px",
            marginBottom: "16px",
            backgroundColor: "#f0fdf4",
            border: "1px solid #86efac",
            borderRadius: "6px",
            color: "#166534",
            fontSize: "13px",
          }}
        >
          {successMessage}
        </div>
      )}

      {status?.block_reason && (
        <div
          style={{
            padding: "10px 14px",
            marginBottom: "16px",
            backgroundColor: "#fffbeb",
            border: "1px solid #fcd34d",
            borderRadius: "6px",
            color: "#92400e",
            fontSize: "13px",
          }}
        >
          {status.block_reason}
        </div>
      )}

      {status?.legacy_sidecar_present && (
        <div
          style={{
            padding: "10px 14px",
            marginBottom: "16px",
            backgroundColor: "#fffbeb",
            border: "1px solid #fcd34d",
            borderRadius: "6px",
            color: "#92400e",
            fontSize: "13px",
          }}
        >
          {t("antigravityRules.alerts.legacySidecar")}
        </div>
      )}

      {/* Main Settings Card */}
      <div
        style={{
          backgroundColor: "#ffffff",
          border: "1px solid #e5e7eb",
          borderRadius: "8px",
          padding: "20px",
          boxShadow: "0 1px 3px rgba(0,0,0,0.05)",
          marginBottom: "20px",
        }}
      >
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "16px" }}>
          <div>
            <h3 style={{ fontSize: "16px", fontWeight: 600, margin: 0, color: "#111827", display: "inline-flex", alignItems: "center" }}>
              {t("antigravityRules.panelTitle")}
              {stateBadge}
            </h3>
            <div style={{ fontSize: "12px", color: "#6b7280", marginTop: "4px" }}>
              {t("antigravityRules.panelSubtitle")}
            </div>
          </div>
        </div>

        {/* Setting 1: Auto Activate Toggle */}
        <div style={{ borderTop: "1px solid #f3f4f6", paddingTop: "14px", marginBottom: "16px" }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <div>
              <div style={{ fontWeight: 600, fontSize: "13px", color: "#374151" }}>
                {t("antigravityRules.autoActivation.title")}
              </div>
              <div style={{ fontSize: "12px", color: "#6b7280", marginTop: "2px" }}>
                {t("antigravityRules.autoActivation.desc")}
              </div>
            </div>
            <label style={{ display: "flex", alignItems: "center", cursor: "pointer" }}>
              <input
                type="checkbox"
                checked={draft.auto_activation}
                onChange={(e) => setDraft({ ...draft, auto_activation: e.target.checked })}
                style={{ width: "16px", height: "16px", cursor: "pointer" }}
              />
            </label>
          </div>
        </div>

        {/* Setting 2: Web Research Radio Group */}
        <div style={{ borderTop: "1px solid #f3f4f6", paddingTop: "14px", marginBottom: "16px" }}>
          <div style={{ fontWeight: 600, fontSize: "13px", color: "#374151", marginBottom: "8px" }}>
            {t("antigravityRules.webResearch.title")}
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            {webResearchOptions.map((opt) => (
              <label
                key={opt.value}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "8px",
                  fontSize: "13px",
                  color: "#374151",
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
        <div style={{ borderTop: "1px solid #f3f4f6", paddingTop: "14px", marginBottom: "16px" }}>
          <div style={{ fontWeight: 600, fontSize: "13px", color: "#374151", marginBottom: "8px" }}>
            {t("antigravityRules.verbosity.title")}
          </div>
          <div style={{ display: "flex", gap: "6px" }}>
            {verbosityOptions.map((opt) => {
              const active = draft.response_verbosity === opt.value;
              return (
                <button
                  key={opt.value}
                  type="button"
                  onClick={() => setDraft({ ...draft, response_verbosity: opt.value })}
                  style={{
                    padding: "4px 12px",
                    borderRadius: "16px",
                    fontSize: "12px",
                    fontWeight: active ? 600 : 400,
                    backgroundColor: active ? "#2563eb" : "#f3f4f6",
                    color: active ? "#ffffff" : "#374151",
                    border: "none",
                    cursor: "pointer",
                  }}
                >
                  {opt.label}
                </button>
              );
            })}
          </div>
        </div>

        {/* Setting 4: Repository Exploration Pill Selector */}
        <div style={{ borderTop: "1px solid #f3f4f6", paddingTop: "14px", marginBottom: "16px" }}>
          <div style={{ fontWeight: 600, fontSize: "13px", color: "#374151", marginBottom: "8px" }}>
            {t("antigravityRules.exploration.title")}
          </div>
          <div style={{ display: "flex", gap: "6px" }}>
            {explorationOptions.map((opt) => {
              const active = draft.repository_exploration === opt.value;
              return (
                <button
                  key={opt.value}
                  type="button"
                  onClick={() => setDraft({ ...draft, repository_exploration: opt.value })}
                  style={{
                    padding: "4px 12px",
                    borderRadius: "16px",
                    fontSize: "12px",
                    fontWeight: active ? 600 : 400,
                    backgroundColor: active ? "#2563eb" : "#f3f4f6",
                    color: active ? "#ffffff" : "#374151",
                    border: "none",
                    cursor: "pointer",
                  }}
                >
                  {opt.label}
                </button>
              );
            })}
          </div>
        </div>

        {/* Setting 5: Verification Scope Pill Selector */}
        <div style={{ borderTop: "1px solid #f3f4f6", paddingTop: "14px", marginBottom: "16px" }}>
          <div style={{ fontWeight: 600, fontSize: "13px", color: "#374151", marginBottom: "8px" }}>
            {t("antigravityRules.verification.title")}
          </div>
          <div style={{ display: "flex", gap: "6px" }}>
            {verificationOptions.map((opt) => {
              const active = draft.verification === opt.value;
              return (
                <button
                  key={opt.value}
                  type="button"
                  onClick={() => setDraft({ ...draft, verification: opt.value })}
                  style={{
                    padding: "4px 12px",
                    borderRadius: "16px",
                    fontSize: "12px",
                    fontWeight: active ? 600 : 400,
                    backgroundColor: active ? "#2563eb" : "#f3f4f6",
                    color: active ? "#ffffff" : "#374151",
                    border: "none",
                    cursor: "pointer",
                  }}
                >
                  {opt.label}
                </button>
              );
            })}
          </div>
        </div>

        {/* Target Rule File Info */}
        <div style={{ borderTop: "1px solid #f3f4f6", paddingTop: "14px", marginBottom: "16px" }}>
          <div style={{ fontWeight: 600, fontSize: "13px", color: "#374151", marginBottom: "4px" }}>
            {t("antigravityRules.ruleFile.title")}
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <code
              style={{
                fontSize: "12px",
                color: "#4b5563",
                backgroundColor: "#f9fafb",
                padding: "4px 8px",
                borderRadius: "4px",
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
                fontSize: "11px",
                fontWeight: 600,
                color: "#2563eb",
                backgroundColor: "#eff6ff",
                border: "1px solid #bfdbfe",
                padding: "2px 6px",
                borderRadius: "4px",
                whiteSpace: "nowrap",
              }}
            >
              {t("antigravityRules.ruleFile.managedBadge")}
            </span>
          </div>
        </div>

        {/* Live Settings Preview */}
        <div style={{ borderTop: "1px solid #f3f4f6", paddingTop: "14px", marginBottom: "16px" }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "6px" }}>
            <div style={{ fontWeight: 600, fontSize: "13px", color: "#374151" }}>
              {t("antigravityRules.ruleFile.previewTitle")}
            </div>
            {status && (
              <div
                style={{
                  fontSize: "11px",
                  color: isOverByteLimit ? "#ef4444" : "#6b7280",
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
              backgroundColor: "#f8fafc",
              border: "1px solid #e2e8f0",
              borderRadius: "6px",
              padding: "10px 14px",
              fontSize: "12px",
              color: "#334155",
              fontFamily: "monospace",
              margin: 0,
              whiteSpace: "pre-wrap",
            }}
          >
            {previewLines}
          </pre>
        </div>

        {/* Action Buttons */}
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", borderTop: "1px solid #f3f4f6", paddingTop: "16px" }}>
          <div style={{ display: "flex", gap: "8px" }}>
            <button
              type="button"
              onClick={handleOpenFolder}
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
              {t("antigravityRules.actions.openFolder")}
            </button>
            <button
              type="button"
              onClick={handleResetDefaults}
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
              {t("antigravityRules.actions.resetDefaults")}
            </button>
          </div>

          <button
            type="button"
            disabled={saving || !status?.can_apply || isOverByteLimit}
            onClick={handleSaveClick}
            style={{
              padding: "6px 18px",
              borderRadius: "6px",
              fontSize: "13px",
              fontWeight: 600,
              backgroundColor: saving || !status?.can_apply || isOverByteLimit ? "#9ca3af" : "#2563eb",
              color: "#ffffff",
              border: "none",
              cursor: saving || !status?.can_apply || isOverByteLimit ? "not-allowed" : "pointer",
            }}
          >
            {saving ? t("antigravityRules.saving") : t("antigravityRules.actions.apply")}
          </button>
        </div>
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
