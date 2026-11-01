import React, { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type { CodexReviewSettings, CliToolInfo, CliAuthInfo } from "../types";
import { mapReviewerToCliId } from "../types";

export interface CodexAutocodePanelProps {
  onNavigateToCliTools?: () => void;
}

export default function CodexAutocodePanel({ onNavigateToCliTools }: CodexAutocodePanelProps = {}) {
  const { t } = useTranslation();

  const [reviewSettings, setReviewSettings] = useState<CodexReviewSettings | null>(null);
  const [reviewLoading, setReviewLoading] = useState(false);
  const [reviewError, setReviewError] = useState<string | null>(null);
  const [reviewSuccessMessage, setReviewSuccessMessage] = useState<string | null>(null);
  const [confirmZeroReviewers, setConfirmZeroReviewers] = useState(false);
  const [cliToolsStatus, setCliToolsStatus] = useState<CliToolInfo[]>([]);
  const [cliAuthStatus, setCliAuthStatus] = useState<CliAuthInfo[]>([]);
  const [cliStatusState, setCliStatusState] = useState<"loading" | "loaded" | "error">("loading");

  const fetchReviewSettings = useCallback(async () => {
    try {
      setReviewLoading(true);
      setReviewError(null);
      const res = await invoke<CodexReviewSettings>("get_codex_review_settings");
      setReviewSettings(res);
    } catch (e: unknown) {
      setReviewError(String(e));
    } finally {
      setReviewLoading(false);
    }
  }, []);

  const fetchCliStatus = useCallback(async () => {
    setCliStatusState("loading");
    let hasError = false;
    try {
      const res = await invoke<CliToolInfo[]>("get_cli_tools_status");
      setCliToolsStatus(res ?? []);
    } catch {
      hasError = true;
      setCliToolsStatus([]);
    }
    try {
      const auth = await invoke<CliAuthInfo[]>("get_cli_auth_status");
      setCliAuthStatus(auth ?? []);
    } catch {
      // ignore
    }
    setCliStatusState(hasError ? "error" : "loaded");
  }, []);

  useEffect(() => {
    void fetchReviewSettings();
    void fetchCliStatus();
  }, [fetchReviewSettings, fetchCliStatus]);

  const isGeminiEnabled = reviewSettings?.gemini_cli_enabled ?? false;

  const activeReviewersCount = reviewSettings
    ? (reviewSettings.reviewers.codex ? 1 : 0) +
      (reviewSettings.gemini_cli_enabled && reviewSettings.reviewers.gemini ? 1 : 0) +
      (reviewSettings.reviewers.deepseek ? 1 : 0)
    : 0;

  const isSaveValid =
    reviewSettings !== null &&
    (!reviewSettings.reviewers.gemini || reviewSettings.gemini_cli_enabled) &&
    (activeReviewersCount === 0 ||
      activeReviewersCount === 1 ||
      (activeReviewersCount >= 2 &&
        reviewSettings.priority_reviewer !== null &&
        reviewSettings.reviewers[reviewSettings.priority_reviewer as keyof typeof reviewSettings.reviewers]));

  const candidateReviewers = [
    { id: "codex" as const, nameKey: "codexSkillsRules.reviewerCodex", defaultName: "Codex CLI" },
    ...(isGeminiEnabled
      ? [{ id: "gemini" as const, nameKey: "codexSkillsRules.reviewerGemini", defaultName: "Gemini CLI" }]
      : []),
    { id: "deepseek" as const, nameKey: "codexSkillsRules.reviewerDeepseek", defaultName: "DeepSeek Harness" },
  ];

  const handleToggleReviewer = (reviewer: "codex" | "gemini" | "deepseek") => {
    if (!reviewSettings) return;
    if (reviewer === "gemini" && !reviewSettings.gemini_cli_enabled) return;

    const current = reviewSettings.reviewers[reviewer];
    const newReviewers = {
      ...reviewSettings.reviewers,
      [reviewer]: !current,
    };

    const count =
      (newReviewers.codex ? 1 : 0) +
      (reviewSettings.gemini_cli_enabled && newReviewers.gemini ? 1 : 0) +
      (newReviewers.deepseek ? 1 : 0);

    let newPriority = reviewSettings.priority_reviewer;
    if (count === 0) {
      newPriority = null;
    } else if (count === 1) {
      if (newReviewers.codex) newPriority = "codex";
      else if (reviewSettings.gemini_cli_enabled && newReviewers.gemini) newPriority = "gemini";
      else if (newReviewers.deepseek) newPriority = "deepseek";
    } else {
      if (newPriority && !newReviewers[newPriority as keyof typeof newReviewers]) {
        newPriority = null;
      }
    }

    setReviewSettings({
      ...reviewSettings,
      reviewers: newReviewers,
      priority_reviewer: newPriority,
    });
  };

  const handleSelectPriority = (priority: string) => {
    if (!reviewSettings) return;
    setReviewSettings({
      ...reviewSettings,
      priority_reviewer: priority,
    });
  };

  const handleSaveReviewSettingsClick = () => {
    if (!reviewSettings) return;
    if (activeReviewersCount === 0) {
      setConfirmZeroReviewers(true);
    } else {
      void executeSaveReviewSettings();
    }
  };

  const executeSaveReviewSettings = async () => {
    if (!reviewSettings) return;
    setConfirmZeroReviewers(false);
    try {
      setReviewLoading(true);
      setReviewError(null);
      const saved = await invoke<CodexReviewSettings>("save_codex_review_settings", { settings: reviewSettings });
      setReviewSettings(saved);
      setReviewSuccessMessage(t("codexSkillsRules.reviewSettingsSavedSuccess") || "Review settings saved successfully");
      setTimeout(() => setReviewSuccessMessage(null), 3500);
    } catch (e: unknown) {
      setReviewError(String(e));
    } finally {
      setReviewLoading(false);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }} data-testid="codex-autocode-panel">
      {/* ── CARD: Codex Review Settings ── */}
      <div className="settings-tile" data-testid="codex-review-settings-card">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6, flexWrap: "wrap", gap: 8 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
              {t("codexSkillsRules.reviewSettingsTitle") || "Codex Review Settings"}
            </h3>
            {reviewLoading && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
            {reviewSuccessMessage && (
              <span style={{ fontSize: 12, color: "#1a7f37", fontWeight: 500 }} data-testid="review-settings-success-msg">
                ✓ {reviewSuccessMessage}
              </span>
            )}
            {onNavigateToCliTools && (
              <button
                type="button"
                className="btn"
                style={{ fontSize: 11, padding: "4px 10px" }}
                onClick={onNavigateToCliTools}
                data-testid="btn-nav-cli-tools"
              >
                {t("codexSkillsRules.manageCliTools") || "Manage in CLI Tools"}
              </button>
            )}
            <button
              type="button"
              className="btn"
              style={{ fontSize: 11, padding: "4px 10px" }}
              onClick={() => {
                void fetchReviewSettings();
                void fetchCliStatus();
              }}
              disabled={reviewLoading}
              data-testid="btn-reload-review-settings"
            >
              {t("codexSkillsRules.btnReload") || "Reload"}
            </button>
          </div>
        </div>

        <p style={{ margin: "0 0 12px 0", fontSize: 12, color: "#656d76" }}>
          {t("codexSkillsRules.reviewSettingsDesc") || "Configure multi-CLI parallel review for Codex Workflow Skills (saved in CODEX_HOME/pov-review-settings.json)."}
        </p>

        {reviewError && (
          <div
            style={{
              margin: "0 0 12px 0",
              padding: "8px 12px",
              background: "#ffebe9",
              border: "1px solid #ff8182",
              borderRadius: 6,
              fontSize: 12,
              color: "#cf222e",
            }}
            data-testid="review-settings-error"
          >
            {reviewError}
          </div>
        )}

        <div style={{ display: "flex", flexDirection: "column", gap: 12, background: "#fff", border: "1px solid #e1e4e8", borderRadius: 6, padding: "12px 14px" }}>
          {/* Section 1: Reviewer Selection Grid (§12.20.2, §12.23, §12.27) */}
          <div className={`codex-review-grid ${isGeminiEnabled ? "cols-3" : "cols-2"}`} data-testid="reviewer-selection-grid">
            {candidateReviewers.map(({ id, nameKey, defaultName }) => {
              const isChecked = reviewSettings?.reviewers[id] ?? false;
              return (
                <button
                  key={id}
                  type="button"
                  role="checkbox"
                  aria-checked={isChecked}
                  className={`codex-review-toggle-btn ${isChecked ? "active" : ""}`}
                  onClick={() => handleToggleReviewer(id)}
                  disabled={reviewLoading}
                  data-testid={`checkbox-reviewer-${id}`}
                >
                  <span style={{ fontSize: 13, marginRight: 2 }}>{isChecked ? "✓" : "○"}</span>
                  <span>{t(nameKey as any) || defaultName}</span>
                </button>
              );
            })}
          </div>

          {/* Section 2: Installation Status Single Row (§12.21, §12.23) */}
          <div className="codex-review-status-row" data-testid="reviewer-status-row">
            {candidateReviewers.map(({ id, nameKey, defaultName }) => {
              const cliId = mapReviewerToCliId(id);
              const cliInfo = cliToolsStatus.find((c) => c.name === cliId);
              const isInstalled = cliInfo?.installed ?? false;

              return (
                <div key={id} className="codex-review-status-item" data-testid={`reviewer-status-item-${id}`}>
                  <span style={{ fontWeight: 600, color: "var(--text-primary, #1f2937)" }}>
                    {t(nameKey as any) || defaultName}:
                  </span>

                  <span data-testid={`reviewer-install-status-${id}`} style={{ fontWeight: 500 }}>
                    {cliStatusState === "loading" ? (
                      <span style={{ color: "#656d76" }}>… {t("codexSkillsRules.statusChecking") || "Checking..."}</span>
                    ) : cliStatusState === "error" ? (
                      <span style={{ color: "#cf222e" }}>? {t("codexSkillsRules.statusUnavailable") || "Status unavailable"}</span>
                    ) : isInstalled ? (
                      <span style={{ color: "#1a7f37" }}>
                        ✓ {cliInfo?.installed_version ?? (t("cliTools.authUnknown") || "version unknown")}
                      </span>
                    ) : (
                      <span style={{ color: "#656d76" }}>○ {t("antigravity.cliToolNotInstalled")}</span>
                    )}
                  </span>
                </div>
              );
            })}
          </div>

          {/* Section 3: Priority Reviewer Grid (§12.20.4, permanently visible) */}
          <div style={{ marginTop: 4, paddingTop: 10, borderTop: "1px solid #e1e4e8" }} data-testid="priority-selection-section">
            <div style={{ fontSize: 12, fontWeight: 700, color: "#1f2937", marginBottom: 4 }}>
              {t("codexSkillsRules.priorityLabel") || "Priority Reviewer"}
            </div>
            <p style={{ margin: "0 0 8px 0", fontSize: 11, color: "#656d76" }}>
              {t("codexSkillsRules.priorityHelp") || "In case of conflicting verdicts between reviewers, the priority reviewer takes precedence unless there is an unresolved blocking finding."}
            </p>
            <div className={`codex-review-grid ${isGeminiEnabled ? "cols-3" : "cols-2"}`}>
              {candidateReviewers.map(({ id, nameKey, defaultName }) => {
                const isChecked = reviewSettings?.reviewers[id] ?? false;
                const isPriority = reviewSettings?.priority_reviewer === id;
                const isDisabled = reviewLoading || !isChecked;

                return (
                  <button
                    key={id}
                    type="button"
                    role="radio"
                    aria-checked={isPriority}
                    className={`codex-review-priority-btn ${isPriority ? "active" : ""}`}
                    disabled={isDisabled}
                    onClick={() => handleSelectPriority(id)}
                    data-testid={`radio-priority-${id}`}
                  >
                    <span style={{ fontSize: 12, marginRight: 4 }}>{isPriority ? "★" : "☆"}</span>
                    <span>{t(nameKey as any) || defaultName}</span>
                  </button>
                );
              })}
            </div>
          </div>

          {/* Action Row */}
          <div style={{ display: "flex", justifyContent: "flex-end", alignItems: "center", gap: 10, marginTop: 4 }}>
            <button
              type="button"
              className="btn btn-primary"
              style={{ fontSize: 12, padding: "5px 16px" }}
              onClick={handleSaveReviewSettingsClick}
              disabled={reviewLoading || !isSaveValid}
              data-testid="btn-save-review-settings"
            >
              {t("codexSkillsRules.btnSaveReviewSettings") || "Save Settings"}
            </button>
          </div>
        </div>
      </div>

      {confirmZeroReviewers && (
        <div data-testid="modal-confirm-zero-reviewers" style={modalOverlayStyle}>
          <div style={modalContentStyle}>
            <h4 style={{ margin: "0 0 10px 0", color: "#cf222e" }}>
              {t("codexSkillsRules.zeroReviewersConfirmTitle") || "Skip External Reviews?"}
            </h4>
            <p style={{ fontSize: 13, color: "#57606a", lineHeight: 1.5 }}>
              {t("codexSkillsRules.zeroReviewersConfirmDesc") ||
                "No reviewers are selected. Workflow skills will proceed without any external validation. Are you sure you want to save this configuration?"}
            </p>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 16 }}>
              <button
                type="button"
                className="btn"
                onClick={() => setConfirmZeroReviewers(false)}
                data-testid="modal-zero-reviewers-cancel"
              >
                {t("antigravity.btnCancel") || "Cancel"}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => void executeSaveReviewSettings()}
                data-testid="modal-zero-reviewers-proceed"
              >
                {t("antigravity.btnConfirm") || "Proceed"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

const modalOverlayStyle: React.CSSProperties = {
  position: "fixed",
  top: 0,
  left: 0,
  right: 0,
  bottom: 0,
  backgroundColor: "rgba(0, 0, 0, 0.4)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  zIndex: 1000,
};

const modalContentStyle: React.CSSProperties = {
  background: "#ffffff",
  border: "1px solid #d0d7de",
  borderRadius: 8,
  padding: 20,
  maxWidth: 440,
  width: "90%",
  boxShadow: "0 8px 24px rgba(140, 149, 159, 0.2)",
};
