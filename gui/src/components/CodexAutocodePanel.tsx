import React, { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type { CodexReviewSettings, CliToolInfo, CliAuthInfo, ReviewMode } from "../types";
import { mapReviewerToCliId } from "../types";
import { getDeepSeekReviewerModels } from "../modelCapabilities";
import ReviewerDiagnosticsModal from "./ReviewerDiagnosticsModal";

export interface CodexAutocodePanelProps {
  onNavigateToCliTools?: () => void;
}

function InfoButton({
  label,
  isOpen,
  onToggle,
  testId,
  popoverId,
  popoverContent,
}: {
  label: string;
  isOpen: boolean;
  onToggle: () => void;
  testId: string;
  popoverId: string;
  popoverContent: React.ReactNode;
}) {
  return (
    <div style={{ position: "relative", display: "inline-flex", alignItems: "center" }}>
      <button
        type="button"
        aria-label={label}
        aria-expanded={isOpen}
        aria-describedby={isOpen ? popoverId : undefined}
        onClick={onToggle}
        data-testid={testId}
        style={{
          background: "none",
          border: "none",
          padding: "2px 4px",
          cursor: "pointer",
          fontSize: 13,
          color: "#57606a",
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          lineHeight: 1,
          borderRadius: 4,
        }}
      >
        ⓘ
      </button>
      {isOpen && (
        <div
          id={popoverId}
          role="tooltip"
          data-testid={`${testId}-popover`}
          style={{
            position: "absolute",
            top: "100%",
            left: 0,
            marginTop: 4,
            width: 290,
            maxWidth: "calc(100vw - 40px)",
            padding: "8px 12px",
            background: "#ffffff",
            border: "1px solid #d0d7de",
            borderRadius: 6,
            boxShadow: "0 4px 12px rgba(140, 149, 159, 0.2)",
            fontSize: 11,
            lineHeight: 1.45,
            color: "#24292f",
            zIndex: 100,
          }}
        >
          {popoverContent}
        </div>
      )}
    </div>
  );
}

export default function CodexAutocodePanel({ onNavigateToCliTools: _unused }: CodexAutocodePanelProps = {}) {
  const { t } = useTranslation();

  const [reviewSettings, setReviewSettings] = useState<CodexReviewSettings | null>(null);
  const [reviewLoading, setReviewLoading] = useState(false);
  const [reviewError, setReviewError] = useState<string | null>(null);
  const [reviewSuccessMessage, setReviewSuccessMessage] = useState<string | null>(null);
  const [confirmZeroReviewers, setConfirmZeroReviewers] = useState(false);
  const [cliToolsStatus, setCliToolsStatus] = useState<CliToolInfo[]>([]);
  const [cliAuthStatus, setCliAuthStatus] = useState<CliAuthInfo[]>([]);
  const [cliStatusState, setCliStatusState] = useState<"loading" | "loaded" | "error">("loading");

  const [showSettingsInfo, setShowSettingsInfo] = useState(false);
  const [showPriorityInfo, setShowPriorityInfo] = useState(false);
  const [showDiagnosticsModal, setShowDiagnosticsModal] = useState(false);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setShowSettingsInfo(false);
        setShowPriorityInfo(false);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

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

  const currentReviewMode: ReviewMode = reviewSettings?.review_mode ?? "codex_self_only";
  const isExternalMode =
    currentReviewMode === "external_only" || currentReviewMode === "codex_plus_external";

  const activeReviewersCount = reviewSettings
    ? (reviewSettings.gemini_cli_enabled && reviewSettings.reviewers.gemini ? 1 : 0) +
      (reviewSettings.reviewers.deepseek ? 1 : 0)
    : 0;

  const isSaveValid =
    reviewSettings !== null &&
    (!reviewSettings.reviewers.gemini || reviewSettings.gemini_cli_enabled) &&
    (currentReviewMode === "codex_self_only" ||
      currentReviewMode === "none" ||
      (currentReviewMode === "external_only" &&
        activeReviewersCount > 0 &&
        reviewSettings.priority_reviewer !== null &&
        reviewSettings.priority_reviewer !== "codex" &&
        Boolean(reviewSettings.reviewers[reviewSettings.priority_reviewer as "gemini" | "deepseek"])) ||
      (currentReviewMode === "codex_plus_external" &&
        activeReviewersCount > 0 &&
        reviewSettings.priority_reviewer !== null &&
        (reviewSettings.priority_reviewer === "codex" ||
          Boolean(reviewSettings.reviewers[reviewSettings.priority_reviewer as "gemini" | "deepseek"]))));

  const candidateReviewers = [
    ...(isGeminiEnabled
      ? [{ id: "gemini" as const, nameKey: "codexSkillsRules.reviewerGemini", defaultName: "Gemini CLI" }]
      : []),
    { id: "deepseek" as const, nameKey: "codexSkillsRules.reviewerDeepseek", defaultName: "DeepSeek Harness" },
  ];

  const reviewModes: { id: ReviewMode; labelKey: string; descKey: string; defaultLabel: string; defaultDesc: string }[] = [
    {
      id: "codex_self_only",
      labelKey: "codexSkillsRules.modeCodexSelfOnly",
      descKey: "codexSkillsRules.descCodexSelfOnly",
      defaultLabel: "Codex Self-Review",
      defaultDesc: "Codex verifies plans and implementations within its own session. No external reviewer process is launched (default, safe).",
    },
    {
      id: "external_only",
      labelKey: "codexSkillsRules.modeExternalOnly",
      descKey: "codexSkillsRules.descExternalOnly",
      defaultLabel: "External Independent Review",
      defaultDesc: "Dispatches external reviewers (DeepSeek / Gemini) for independent review via the dispatcher.",
    },
    {
      id: "codex_plus_external",
      labelKey: "codexSkillsRules.modeCodexPlusExternal",
      descKey: "codexSkillsRules.descCodexPlusExternal",
      defaultLabel: "Codex Self-Review + External Review",
      defaultDesc: "Codex conducts in-session self-review, followed by external independent review upon passing.",
    },
    {
      id: "none",
      labelKey: "codexSkillsRules.modeNone",
      descKey: "codexSkillsRules.descNone",
      defaultLabel: "No Review",
      defaultDesc: "Skips the review step entirely (recorded as SKIPPED_BY_USER).",
    },
  ];

  const handleSelectReviewMode = (mode: ReviewMode) => {
    if (!reviewSettings) return;
    let newPriority = reviewSettings.priority_reviewer;
    if (mode === "codex_self_only") {
      newPriority = "codex";
    } else if (mode === "none") {
      newPriority = null;
    } else if (mode === "codex_plus_external") {
      if (
        newPriority !== "codex" &&
        (!newPriority || !reviewSettings.reviewers[newPriority as "gemini" | "deepseek"])
      ) {
        newPriority = "codex";
      }
    } else if (mode === "external_only") {
      if (!newPriority || newPriority === "codex" || !reviewSettings.reviewers[newPriority as "gemini" | "deepseek"]) {
        if (reviewSettings.reviewers.deepseek) {
          newPriority = "deepseek";
        } else if (reviewSettings.gemini_cli_enabled && reviewSettings.reviewers.gemini) {
          newPriority = "gemini";
        } else {
          newPriority = null;
        }
      }
    }
    setReviewSettings({
      ...reviewSettings,
      review_mode: mode,
      priority_reviewer: newPriority,
    });
  };

  const handleToggleReviewer = (reviewer: "gemini" | "deepseek") => {
    if (!reviewSettings) return;
    if (reviewer === "gemini" && !reviewSettings.gemini_cli_enabled) return;

    const current = reviewSettings.reviewers[reviewer];
    const newReviewers = {
      ...reviewSettings.reviewers,
      codex: false,
      [reviewer]: !current,
    };

    const count =
      (reviewSettings.gemini_cli_enabled && newReviewers.gemini ? 1 : 0) +
      (newReviewers.deepseek ? 1 : 0);

    let newPriority = reviewSettings.priority_reviewer;
    if (currentReviewMode === "codex_plus_external") {
      if (current && newPriority === reviewer) {
        newPriority = "codex";
      } else if (!newPriority) {
        newPriority = "codex";
      }
    } else if (currentReviewMode === "external_only") {
      if (count === 0) {
        newPriority = null;
      } else if (count === 1) {
        if (reviewSettings.gemini_cli_enabled && newReviewers.gemini) newPriority = "gemini";
        else if (newReviewers.deepseek) newPriority = "deepseek";
      } else {
        if (!newPriority || newPriority === "codex" || !newReviewers[newPriority as "gemini" | "deepseek"]) {
          newPriority = reviewSettings.gemini_cli_enabled && newReviewers.gemini ? "gemini" : "deepseek";
        }
      }
    } else if (currentReviewMode === "codex_self_only") {
      newPriority = "codex";
    } else {
      newPriority = null;
    }

    let deepseekCfg = reviewSettings.deepseek;
    if (reviewer === "deepseek" && !current && !deepseekCfg) {
      deepseekCfg = { model: "deepseek-flash", reasoning_effort: "high" };
    }
    let geminiCfg = reviewSettings.gemini;
    if (reviewer === "gemini" && !current && !geminiCfg) {
      geminiCfg = { model: "gemini-2.5-pro" };
    }

    setReviewSettings({
      ...reviewSettings,
      reviewers: newReviewers,
      priority_reviewer: newPriority,
      deepseek: deepseekCfg,
      gemini: geminiCfg,
    });
  };

  const priorityCandidates = useMemo(() => {
    if (currentReviewMode === "codex_plus_external") {
      const list: { id: string; label: string }[] = [
        {
          id: "codex",
          label: t("codexSkillsRules.modeCodexSelfOnly") || "Codex Self-Review",
        },
      ];
      if (reviewSettings?.reviewers.deepseek) {
        list.push({
          id: "deepseek",
          label: t("codexSkillsRules.reviewerDeepseek") || "DeepSeek Harness",
        });
      }
      if (reviewSettings?.gemini_cli_enabled && reviewSettings?.reviewers.gemini) {
        list.push({
          id: "gemini",
          label: t("codexSkillsRules.reviewerGemini") || "Gemini CLI",
        });
      }
      return list;
    }
    if (currentReviewMode === "external_only") {
      const list: { id: string; label: string }[] = [];
      if (reviewSettings?.reviewers.deepseek) {
        list.push({
          id: "deepseek",
          label: t("codexSkillsRules.reviewerDeepseek") || "DeepSeek Harness",
        });
      }
      if (reviewSettings?.gemini_cli_enabled && reviewSettings?.reviewers.gemini) {
        list.push({
          id: "gemini",
          label: t("codexSkillsRules.reviewerGemini") || "Gemini CLI",
        });
      }
      return list;
    }
    return [];
  }, [currentReviewMode, reviewSettings, t]);

  const priorityOrderDescription = useMemo(() => {
    if (!isExternalMode || priorityCandidates.length <= 1) {
      return null;
    }
    return t("codexSkillsRules.priorityArbitrationDescription");
  }, [isExternalMode, priorityCandidates.length, t]);

  const deepseekModels = useMemo(() => getDeepSeekReviewerModels(), []);

  const handleSelectPriority = (priority: string) => {
    if (!reviewSettings) return;
    setReviewSettings({
      ...reviewSettings,
      priority_reviewer: priority,
    });
  };

  const handleDeepSeekModelChange = (model: string) => {
    if (!reviewSettings) return;
    const currentDeepSeek = reviewSettings.deepseek ?? {};
    const selectedModel = deepseekModels.find((m) => m.id === model);
    const currentEffort = currentDeepSeek.reasoning_effort;
    const supportedEfforts = selectedModel?.supportedEfforts ?? ["low", "high", "max"];
    const newEffort = currentEffort && supportedEfforts.includes(currentEffort)
      ? currentEffort
      : (selectedModel?.defaultEffort ?? "high");

    setReviewSettings({
      ...reviewSettings,
      deepseek: {
        ...currentDeepSeek,
        model,
        reasoning_effort: newEffort,
      },
    });
  };

  const handleDeepSeekEffortChange = (effort: string) => {
    if (!reviewSettings) return;
    setReviewSettings({
      ...reviewSettings,
      deepseek: {
        ...(reviewSettings.deepseek ?? {}),
        reasoning_effort: effort,
      },
    });
  };

  const handleGeminiModelChange = (model: string) => {
    if (!reviewSettings) return;
    setReviewSettings({
      ...reviewSettings,
      gemini: {
        ...(reviewSettings.gemini ?? {}),
        model,
      },
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
      const payload: CodexReviewSettings = {
        ...reviewSettings,
        unconfirmed_migration: false,
      };
      const saved = await invoke<CodexReviewSettings>("save_codex_review_settings", { settings: payload });
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
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
              {t("codexSkillsRules.reviewSettingsTitle") || "Codex Review Settings"}
            </h3>
            <InfoButton
              label={t("codexSkillsRules.infoIconAriaReviewSettings") || "Show Review Settings explanation"}
              isOpen={showSettingsInfo}
              onToggle={() => setShowSettingsInfo((prev) => !prev)}
              testId="btn-info-review-settings"
              popoverId="popover-review-settings-info"
              popoverContent={
                t("codexSkillsRules.reviewSettingsDesc") ||
                "Configure multi-CLI parallel review for Codex Workflow Skills (saved in CODEX_HOME/pov-review-settings.json)."
              }
            />
            {reviewLoading && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
            {reviewSuccessMessage && (
              <span style={{ fontSize: 12, color: "#1a7f37", fontWeight: 500 }} data-testid="review-settings-success-msg">
                ✓ {reviewSuccessMessage}
              </span>
            )}
            <button
              type="button"
              className="btn"
              style={{ fontSize: 11, padding: "4px 10px" }}
              onClick={() => setShowDiagnosticsModal(true)}
              data-testid="btn-open-diagnostics-modal"
            >
              {t("diagnostics.buttonLabel") || "Diagnostic Logs"}
            </button>
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

        {reviewSettings?.unconfirmed_migration && (
          <div
            style={{
              margin: "0 0 10px 0",
              padding: "10px 14px",
              background: "#fff8c5",
              border: "1px solid #d4a72c",
              borderRadius: 6,
              fontSize: 12,
              color: "#573a08",
            }}
            data-testid="unconfirmed-migration-banner"
          >
            <div style={{ fontWeight: 700, marginBottom: 2 }}>
              ⚠ {t("codexSkillsRules.unconfirmedMigrationTitle") || "Review Settings Migration Confirmation Required"}
            </div>
            <div>
              {t("codexSkillsRules.unconfirmedMigrationDesc") ||
                "Reviewer configuration was updated during migration. Please review the external reviewers and click 'Save Settings' before launching automated external reviews."}
            </div>
          </div>
        )}

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

        <div style={{ display: "flex", flexDirection: "column", gap: 8, background: "#fff", border: "1px solid #e1e4e8", borderRadius: 6, padding: "10px 14px" }}>
          {/* Section 1: Installation & Working Agent Status Row (§5, §12.21, §12.23) */}
          <div className="codex-review-status-row" style={{ marginBottom: 4 }} data-testid="reviewer-status-row">
            <div className="codex-review-status-item" data-testid="working-agent-item">
              <span style={{ fontWeight: 600, color: "var(--text-primary, #1f2937)" }}>
                {t("codexSkillsRules.workingAgentLabel") || "Working Agent"}:
              </span>
              <span style={{ fontWeight: 500, color: "var(--text-primary, #1f2937)" }}>
                {t("codexSkillsRules.workingAgentCodex") || "Codex"}
              </span>
            </div>

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
                      <span style={{ color: "#656d76" }}>{t("codexSkillsRules.statusChecking") || "Checking..."}</span>
                    ) : cliStatusState === "error" ? (
                      <span style={{ color: "#cf222e" }}>{t("codexSkillsRules.statusUnavailable") || "Status unavailable"}</span>
                    ) : isInstalled ? (
                      <span style={{ color: "#1a7f37" }}>
                        {cliInfo?.installed_version ?? (t("cliTools.authUnknown") || "version unknown")}
                      </span>
                    ) : (
                      <span style={{ color: "#656d76" }}>{t("antigravity.cliToolNotInstalled")}</span>
                    )}
                  </span>
                </div>
              );
            })}
          </div>

          {/* Section 2: Review Mode Selection (4 distinct modes) */}
          <div style={{ marginTop: 4, paddingTop: 6, borderTop: "1px solid #e1e4e8" }} data-testid="review-mode-section">
            <div style={{ fontSize: 12, fontWeight: 700, color: "#1f2937", marginBottom: 6 }}>
              {t("codexSkillsRules.reviewModeLabel") || "Review Mode"}
            </div>
            <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }} data-testid="review-mode-selector">
              {reviewModes.map(({ id, labelKey, defaultLabel }) => {
                const isSelected = currentReviewMode === id;
                return (
                  <button
                    key={id}
                    type="button"
                    role="radio"
                    aria-checked={isSelected}
                    className={`codex-review-toggle-btn ${isSelected ? "active" : ""}`}
                    style={{ width: "auto", minWidth: 140, padding: "6px 14px", minHeight: 34 }}
                    onClick={() => handleSelectReviewMode(id)}
                    disabled={reviewLoading}
                    data-testid={`radio-review-mode-${id}`}
                  >
                    <span>{t(labelKey as any) || defaultLabel}</span>
                  </button>
                );
              })}
            </div>
            <div
              style={{
                marginTop: 6,
                padding: "6px 10px",
                background: "#f6f8fa",
                borderRadius: 4,
                fontSize: 12,
                color: "#57606a",
                lineHeight: 1.4,
              }}
              data-testid="review-mode-description"
            >
              ℹ {t((reviewModes.find((m) => m.id === currentReviewMode)?.descKey ?? "") as any) ||
                reviewModes.find((m) => m.id === currentReviewMode)?.defaultDesc}
            </div>
          </div>

          {/* Section 3: External Reviewer Selection */}
          <div style={{ marginTop: 6, paddingTop: 6, borderTop: "1px solid #e1e4e8" }}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
              <span style={{ fontSize: 12, fontWeight: 700, color: isExternalMode ? "#1f2937" : "#8c959f" }}>
                {t("codexSkillsRules.externalReviewersLabel") || "External Reviewers"}
              </span>
              {!isExternalMode && (
                <span style={{ fontSize: 11, color: "#8c959f", fontStyle: "italic" }}>
                  (Inactive in {currentReviewMode === "codex_self_only" ? "Codex Self-Review" : "No Review"} mode)
                </span>
              )}
            </div>

            {isExternalMode && activeReviewersCount === 0 && (
              <div
                style={{
                  marginBottom: 8,
                  padding: "6px 10px",
                  background: "#fff8c5",
                  border: "1px solid #d4a72c",
                  borderRadius: 4,
                  fontSize: 11,
                  color: "#573a08",
                }}
                data-testid="zero-external-reviewers-warning"
              >
                ⚠ {t("codexSkillsRules.externalRequiredWarning") || "At least one external reviewer must be selected when using an external review mode."}
              </div>
            )}

            <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }} data-testid="reviewer-selection-grid">
              {candidateReviewers.map(({ id, nameKey, defaultName }) => {
                const isChecked = reviewSettings?.reviewers[id] ?? false;
                return (
                  <button
                    key={id}
                    type="button"
                    role="checkbox"
                    aria-checked={isChecked}
                    className={`codex-review-toggle-btn ${isChecked && isExternalMode ? "active" : ""}`}
                    style={{
                      width: "auto",
                      minWidth: 140,
                      padding: "6px 14px",
                      minHeight: 34,
                      opacity: isExternalMode ? 1 : 0.6,
                    }}
                    onClick={() => handleToggleReviewer(id)}
                    disabled={reviewLoading || !isExternalMode}
                    data-testid={`checkbox-reviewer-${id}`}
                  >
                    <span>{t(nameKey as any) || defaultName}</span>
                  </button>
                );
              })}
            </div>
          </div>

          {/* Section 4: Model Settings for Selected External Reviewers */}
          {isExternalMode && reviewSettings?.reviewers.deepseek && (() => {
            const currentDsModel = reviewSettings.deepseek?.model || "deepseek-flash";
            const currentDsEffort = reviewSettings.deepseek?.reasoning_effort || "high";
            const currentDsEntry = deepseekModels.find((m) => m.id === currentDsModel) || deepseekModels[0];
            const supportedEfforts = currentDsEntry?.supportedEfforts ?? ["low", "high", "max"];

            return (
              <div
                style={{
                  border: "1px solid #d0d7de",
                  borderRadius: 6,
                  padding: "8px 12px",
                  background: "#f6f8fa",
                  marginTop: 4,
                }}
                data-testid="deepseek-model-settings"
              >
                <div style={{ fontWeight: 600, fontSize: 12, marginBottom: 6, color: "var(--text-primary, #1f2937)" }}>
                  {t("codexSkillsRules.reviewerDeepseek") || "DeepSeek Harness"} {t("header.settings") || "Settings"}
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: 16, flexWrap: "nowrap" }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8, flexShrink: 0 }}>
                    <label style={{ fontSize: 11, fontWeight: 600, color: "#57606a", whiteSpace: "nowrap" }}>
                      {t("antigravity.reviewerModel") || "Model"}:
                    </label>
                    <div className="pov-segment-group" data-testid="codex-deepseek-model-group">
                      {deepseekModels.map((m) => (
                        <button
                          key={m.id}
                          type="button"
                          className={`pov-segment-btn ${currentDsModel === m.id ? "active" : ""}`}
                          data-testid={`codex-deepseek-model-${m.id}`}
                          onClick={() => handleDeepSeekModelChange(m.id)}
                          disabled={reviewLoading}
                        >
                          {m.displayName}
                        </button>
                      ))}
                    </div>
                  </div>

                  <div style={{ display: "flex", alignItems: "center", gap: 8, flexShrink: 0 }}>
                    <label style={{ fontSize: 11, fontWeight: 600, color: "#57606a", whiteSpace: "nowrap" }}>
                      {t("antigravity.reviewerReasoningEffort") || "Reasoning Effort"}:
                    </label>
                    <div className="pov-segment-group" data-testid="codex-deepseek-effort-group">
                      {supportedEfforts.map((effort) => {
                        const effortKey = `antigravity.reviewerEffort${effort.charAt(0).toUpperCase() + effort.slice(1)}` as any;
                        const label = t(effortKey) || effort;
                        return (
                          <button
                            key={effort}
                            type="button"
                            className={`pov-segment-btn ${currentDsEffort === effort ? "active" : ""}`}
                            data-testid={`codex-deepseek-effort-${effort}`}
                            onClick={() => handleDeepSeekEffortChange(effort)}
                            disabled={reviewLoading}
                          >
                            {label}
                          </button>
                        );
                      })}
                    </div>
                  </div>
                </div>
              </div>
            );
          })()}

          {isExternalMode && isGeminiEnabled && reviewSettings?.reviewers.gemini && (
            <div
              style={{
                border: "1px solid #d0d7de",
                borderRadius: 6,
                padding: "8px 12px",
                background: "#f6f8fa",
                marginTop: 4,
              }}
              data-testid="gemini-model-settings"
            >
              <div style={{ fontWeight: 600, fontSize: 12, marginBottom: 6, color: "var(--text-primary, #1f2937)" }}>
                {t("codexSkillsRules.reviewerGemini") || "Gemini CLI"} {t("header.settings") || "Settings"}
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                <label style={{ fontSize: 11, fontWeight: 600, color: "#57606a", whiteSpace: "nowrap" }}>
                  {t("antigravity.reviewerModel") || "Model"}:
                </label>
                <div className="pov-segment-group" data-testid="gemini-model-group">
                  {[
                    { id: "gemini-2.5-pro", label: "Gemini 2.5 Pro" },
                    { id: "gemini-2.5-flash", label: "Gemini 2.5 Flash" },
                  ].map((m) => (
                    <button
                      key={m.id}
                      type="button"
                      className={`pov-segment-btn ${(reviewSettings.gemini?.model || "gemini-2.5-pro") === m.id ? "active" : ""}`}
                      data-testid={`gemini-model-${m.id}`}
                      onClick={() => handleGeminiModelChange(m.id)}
                      disabled={reviewLoading}
                    >
                      {m.label}
                    </button>
                  ))}
                </div>
              </div>
            </div>
          )}

          {/* Section 5: Priority Reviewer (if external mode) & Save Section */}
          <div style={{ marginTop: 2, paddingTop: 6, borderTop: "1px solid #e1e4e8" }} data-testid="priority-selection-section">
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", flexWrap: "wrap", gap: 10 }}>
              {isExternalMode ? (
                <div style={{ display: "flex", flexDirection: "column", gap: 4, flex: "1 1 300px" }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                    <span style={{ fontSize: 12, fontWeight: 700, color: "#1f2937" }}>
                      {t("codexSkillsRules.priorityLabel") || "Priority Reviewer"}
                    </span>
                    <InfoButton
                      label={t("codexSkillsRules.infoIconAriaPriority") || "Show Priority Reviewer decision rule"}
                      isOpen={showPriorityInfo}
                      onToggle={() => setShowPriorityInfo((prev) => !prev)}
                      testId="btn-info-priority"
                      popoverId="popover-priority-info"
                      popoverContent={
                        t("codexSkillsRules.priorityHelpPopover") ||
                        "In case of conflicting verdicts between reviewers, the priority reviewer takes precedence unless there is an unresolved blocking finding. If there are unresolved blocking findings, the priority reviewer alone will not approve."
                      }
                    />
                  </div>
                  <div style={{ display: "flex", flexWrap: "wrap", gap: 8, alignItems: "center" }}>
                    {priorityCandidates.map(({ id, label }) => {
                      const isPriority = reviewSettings?.priority_reviewer === id;

                      return (
                        <button
                          key={id}
                          type="button"
                          role="radio"
                          aria-checked={isPriority}
                          className={`codex-review-priority-btn ${isPriority ? "active" : ""}`}
                          style={{ width: "auto", minWidth: 140, padding: "4px 12px", minHeight: 32 }}
                          disabled={reviewLoading}
                          onClick={() => handleSelectPriority(id)}
                          data-testid={`radio-priority-${id}`}
                        >
                          <span>{label}</span>
                        </button>
                      );
                    })}
                  </div>
                  {priorityOrderDescription && (
                    <div
                      style={{
                        marginTop: 4,
                        fontSize: 12,
                        color: "#57606a",
                        lineHeight: 1.4,
                      }}
                      data-testid="priority-order-description"
                    >
                      {priorityOrderDescription}
                    </div>
                  )}
                </div>
              ) : (
                <div style={{ fontSize: 12, color: "#57606a" }}>
                  {/* Non-external mode indicator */}
                </div>
              )}

              <button
                type="button"
                className="btn btn-primary"
                style={{ fontSize: 12, padding: "5px 16px", marginLeft: "auto", alignSelf: "center" }}
                onClick={handleSaveReviewSettingsClick}
                disabled={reviewLoading || !isSaveValid}
                data-testid="btn-save-review-settings"
              >
                {t("codexSkillsRules.btnSaveReviewSettings") || "Save Settings"}
              </button>
            </div>
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

      <ReviewerDiagnosticsModal
        isOpen={showDiagnosticsModal}
        onClose={() => setShowDiagnosticsModal(false)}
      />
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
