import React, { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type { CodexReviewSettings, ReviewMode } from "../types";
import { getCodexReviewerModels, getDeepSeekReviewerModels } from "../modelCapabilities";
import PlanFileManagerCard from "./PlanFileManagerCard";

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
  const [reviewSaving, setReviewSaving] = useState(false);
  const [reviewError, setReviewError] = useState<string | null>(null);
  const [reviewSuccessMessage, setReviewSuccessMessage] = useState<string | null>(null);

  const isSavingRef = useRef(false);
  const pendingConfigRef = useRef<CodexReviewSettings | null>(null);

  const [showPriorityInfo, setShowPriorityInfo] = useState(false);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
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

  useEffect(() => {
    void fetchReviewSettings();
  }, [fetchReviewSettings]);

  const buildReviewers = useMemo<string[]>(() => {
    if (reviewSettings?.build_reviewers !== undefined && reviewSettings.build_reviewers !== null) {
      return reviewSettings.build_reviewers;
    }
    const mode = reviewSettings?.review_mode;
    if (mode === "codex_self_only") return ["codex"];
    if (mode === "external_only") {
      return reviewSettings?.reviewers?.deepseek ? ["deepseek"] : [];
    }
    if (mode === "codex_plus_external") {
      const list = ["codex"];
      if (reviewSettings?.reviewers?.deepseek) list.push("deepseek");
      return list;
    }
    if (mode === "none") return [];
    return ["codex"];
  }, [reviewSettings]);

  const isCodexActive = buildReviewers.includes("codex");
  const isDeepseekActive = buildReviewers.includes("deepseek");

  const deepseekModels = useMemo(() => getDeepSeekReviewerModels(), []);
  const codexModels = useMemo(() => getCodexReviewerModels(), []);

  const selectedCodexModel = reviewSettings?.codex?.model || "gpt-6-luna";
  const selectedCodexEffort = reviewSettings?.codex?.reasoning_effort || "medium";
  const currentCodexEntry = codexModels.find((m) => m.id === selectedCodexModel);

  const processSaveQueue = useCallback(async () => {
    if (isSavingRef.current) return;
    if (!pendingConfigRef.current) return;

    isSavingRef.current = true;
    setReviewSaving(true);
    setReviewError(null);

    while (pendingConfigRef.current) {
      const toSave = pendingConfigRef.current;
      pendingConfigRef.current = null;

      try {
        const res = await invoke<CodexReviewSettings>("save_codex_review_settings", {
          settings: {
            ...toSave,
            schema_version: 5,
          },
        });
        if (!pendingConfigRef.current) {
          setReviewSettings(res);
          setReviewSuccessMessage(t("codexSkillsRules.saveSuccess") || t("antigravity.reviewerSaveSuccess") || "設定を保存しました");
          setTimeout(() => setReviewSuccessMessage(null), 3000);
        }
      } catch (e: unknown) {
        setReviewError(String(e));
        if (!pendingConfigRef.current) {
          try {
            const reloaded = await invoke<CodexReviewSettings>("get_codex_review_settings");
            if (!pendingConfigRef.current) {
              setReviewSettings(reloaded);
            }
          } catch {
            // retain reviewError
          }
        }
        break;
      }
    }

    isSavingRef.current = false;
    setReviewSaving(false);
  }, [t]);

  const queueSaveReviewSettings = useCallback(
    (newSettings: CodexReviewSettings) => {
      setReviewSettings(newSettings);
      pendingConfigRef.current = newSettings;
      void processSaveQueue();
    },
    [processSaveQueue]
  );

  const handleConfirmMigration = useCallback(() => {
    if (!reviewSettings) return;
    const updated: CodexReviewSettings = {
      ...reviewSettings,
      unconfirmed_migration: false,
    };
    queueSaveReviewSettings(updated);
  }, [reviewSettings, queueSaveReviewSettings]);

  const handleToggleReviewer = (target: "codex" | "deepseek") => {
    if (!reviewSettings) return;
    const nextList = buildReviewers.includes(target)
      ? buildReviewers.filter((r) => r !== target)
      : [...buildReviewers, target];

    const hasCodex = nextList.includes("codex");
    const hasDeepseek = nextList.includes("deepseek");

    let newMode: ReviewMode;
    let newPriority = reviewSettings.priority_reviewer;

    if (hasCodex && hasDeepseek) {
      newMode = "codex_plus_external";
      if (newPriority !== "codex" && newPriority !== "deepseek") {
        newPriority = "codex";
      }
    } else if (hasCodex) {
      newMode = "codex_self_only";
      newPriority = "codex";
    } else if (hasDeepseek) {
      newMode = "external_only";
      newPriority = "deepseek";
    } else {
      newMode = "none";
      newPriority = null;
    }

    const updatedSettings: CodexReviewSettings = {
      ...reviewSettings,
      review_mode: newMode,
      build_reviewers: nextList,
      priority_reviewer: newPriority,
      reviewers: {
        ...reviewSettings.reviewers,
        codex: false,
        deepseek: hasDeepseek,
      },
    };

    queueSaveReviewSettings(updatedSettings);
  };

  const handleSelectPriority = (priority: string) => {
    if (!reviewSettings) return;
    const updated: CodexReviewSettings = {
      ...reviewSettings,
      priority_reviewer: priority,
    };
    queueSaveReviewSettings(updated);
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

    const updated: CodexReviewSettings = {
      ...reviewSettings,
      deepseek: {
        ...currentDeepSeek,
        model,
        reasoning_effort: newEffort,
      },
    };
    queueSaveReviewSettings(updated);
  };

  const handleDeepSeekEffortChange = (effort: string) => {
    if (!reviewSettings) return;
    const updated: CodexReviewSettings = {
      ...reviewSettings,
      deepseek: {
        ...(reviewSettings.deepseek ?? {}),
        reasoning_effort: effort,
      },
    };
    queueSaveReviewSettings(updated);
  };

  const handleActivePlanChanged = useCallback((planRelPath: string | null) => {
    if (!reviewSettings || reviewSettings.active_plan_path === planRelPath) return;
    const updated: CodexReviewSettings = { ...reviewSettings, active_plan_path: planRelPath };
    queueSaveReviewSettings(updated);
  }, [reviewSettings, queueSaveReviewSettings]);

  const handleRolesChanged = useCallback(
    ({
      plannerRole,
      reviserRole,
      planReviewers,
    }: {
      plannerRole?: string | null;
      reviserRole?: string | null;
      planReviewers?: string[] | null;
    }) => {
      if (!reviewSettings) return;
      if (plannerRole !== undefined || reviserRole !== undefined) return;
      const nextPlanReviewers = planReviewers !== undefined ? planReviewers : (reviewSettings.plan_reviewers ?? null);

      const updated: CodexReviewSettings = {
        ...reviewSettings,
        plan_reviewers: nextPlanReviewers,
      };
      queueSaveReviewSettings(updated);
    },
    [reviewSettings, queueSaveReviewSettings]
  );

  return (
    <div className="codex-autocode-panel poverello-panel" data-testid="codex-autocode-panel">
      {/* ── CARD: Plan File Manager + Codex Review Settings (Unified Block) ── */}
      <div className="settings-tile" data-testid="codex-review-settings-card">
        <PlanFileManagerCard
          embedded
          surface="codex"
          activePlanPath={reviewSettings?.active_plan_path}
          planReviewers={reviewSettings?.plan_reviewers}
          onActivePlanChanged={handleActivePlanChanged}
          onRolesChanged={handleRolesChanged}
        />

        <div
          style={{
            borderTop: "1px solid var(--color-border, #e5e7eb)",
            paddingTop: 12,
            marginTop: 12,
          }}
        >
          {/* Header row: title, codex model badge, description, success message, diagnostic logs, reload */}
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 10, flexWrap: "wrap", gap: 10 }}>
            <div style={{ display: "flex", alignItems: "baseline", gap: 10, flexWrap: "wrap", minWidth: 0 }}>
              <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
                {t("antigravity.reviewerPolicyTitle") || "実装レビューAIの選択"}
              </h3>

              {/* Codex Model & Effort (displayed when Codex is active) */}
              {isCodexActive && (
                <span style={{ fontSize: 12 }}>
                  <span style={{ color: "var(--color-text-secondary, #656d76)" }}>Codex: </span>
                  <span
                    data-testid="codex-model-badge"
                    style={{
                      fontWeight: 600,
                      color: "var(--color-primary, #0969da)",
                    }}
                  >
                    {currentCodexEntry?.displayName ?? selectedCodexModel} ({selectedCodexEffort})
                  </span>
                </span>
              )}

              <span style={{ fontSize: 12, color: "var(--color-text-secondary, #656d76)" }}>
                {t("antigravity.reviewerPolicyDesc") || "複数選択可能。選択されたAIがそれぞれレビューを実施"}
              </span>

              {reviewLoading && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
            </div>

            <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
              {reviewSaving && (
                <span style={{ fontSize: 11, color: "#656d76" }} data-testid="review-settings-saving-msg">
                  {t("codexSkillsRules.savingStatus") || "保存中..."}
                </span>
              )}
              {!reviewSaving && reviewSuccessMessage && (
                <span style={{ fontSize: 12, color: "#1a7f37", fontWeight: 500 }} data-testid="review-settings-success-msg">
                  ✓ {reviewSuccessMessage}
                </span>
              )}
              <button
                type="button"
                className="btn"
                style={{ fontSize: 11, padding: "4px 10px" }}
                onClick={() => {
                  void fetchReviewSettings();
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
                padding: "8px 12px",
                background: "#fff8c5",
                border: "1px solid #d4a72c",
                borderRadius: 6,
                fontSize: 12,
                color: "#573a08",
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                flexWrap: "wrap",
                gap: 8,
              }}
              data-testid="unconfirmed-migration-banner"
            >
              <div>
                <div style={{ fontWeight: 700, marginBottom: 2 }}>
                  ⚠ {t("codexSkillsRules.unconfirmedMigrationTitle") || "Review Settings Migration Confirmation Required"}
                </div>
                <div>
                  {t("codexSkillsRules.unconfirmedMigrationDesc") ||
                    "Reviewer configuration was updated during migration. Please review the external reviewers and click 'Confirm Migration' before launching automated external reviews."}
                </div>
              </div>
              <button
                type="button"
                className="btn btn-primary"
                style={{ fontSize: 11, padding: "4px 12px", whiteSpace: "nowrap" }}
                disabled={reviewSaving}
                onClick={handleConfirmMigration}
                data-testid="btn-confirm-migration"
              >
                {t("codexSkillsRules.btnConfirmMigration") || "移行内容を確認"}
              </button>
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

          {/* ── REVIEW MODE ROW (Multi-select toggles for Codex (Session) and DeepSeek Harness) ── */}
          <div style={{ marginBottom: 10 }}>
            <div
              className="pov-segment-group"
              role="group"
              aria-label={t("antigravity.reviewerPolicyTitle") || "実装レビューAIの選択"}
              style={{ flexWrap: "wrap", display: "inline-flex" }}
              data-testid="reviewer-policy-group"
            >
              <label
                className={`pov-segment-btn ${isCodexActive ? "active" : ""}`}
                style={{ cursor: "pointer", display: "inline-flex", alignItems: "center", gap: 4 }}
                data-testid="reviewer-toggle-codex"
              >
                <input
                  type="checkbox"
                  name="reviewerPolicyCodex"
                  data-testid="reviewer-policy-codex-only"
                  className="sr-only"
                  checked={isCodexActive}
                  disabled={reviewLoading}
                  onChange={() => handleToggleReviewer("codex")}
                />
                <span>{t("planManager.roleCodexSession") || "Codex (Session)"}</span>
              </label>

              <label
                className={`pov-segment-btn ${isDeepseekActive ? "active" : ""}`}
                style={{ cursor: "pointer", display: "inline-flex", alignItems: "center", gap: 4 }}
                data-testid="reviewer-toggle-deepseek"
              >
                <input
                  type="checkbox"
                  name="reviewerPolicyDeepseek"
                  data-testid="reviewer-policy-deepseek-only"
                  className="sr-only"
                  checked={isDeepseekActive}
                  disabled={reviewLoading}
                  onChange={() => handleToggleReviewer("deepseek")}
                />
                <span>DeepSeek Harness</span>
              </label>
            </div>
          </div>

          {/* DeepSeek Model Settings (Shown when DeepSeek is active) */}
          {isDeepseekActive && (() => {
            const currentDsModel = reviewSettings?.deepseek?.model || "deepseek-flash";
            const currentDsEffort = reviewSettings?.deepseek?.reasoning_effort || "high";
            const currentDsEntry = deepseekModels.find((m) => m.id === currentDsModel) || deepseekModels[0];
            const supportedEfforts = currentDsEntry?.supportedEfforts ?? ["low", "high", "max"];

            return (
              <div
                style={{
                  border: "1px solid #d0d7de",
                  borderRadius: 6,
                  padding: "8px 12px",
                  background: "#f6f8fa",
                  marginBottom: 10,
                  display: "flex",
                  alignItems: "center",
                  gap: 12,
                  flexWrap: "wrap",
                }}
                data-testid="deepseek-model-settings"
              >
                <span style={{ fontWeight: 600, fontSize: 12, color: "var(--text-primary, #1f2937)", whiteSpace: "nowrap" }}>
                  {t("codexSkillsRules.reviewerDeepseek") || "DeepSeek Harness"} {t("header.settings") || "Settings"}
                </span>

                <div style={{ display: "flex", alignItems: "center", gap: 6, flexShrink: 0 }}>
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

                <div style={{ display: "flex", alignItems: "center", gap: 6, flexShrink: 0 }}>
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
            );
          })()}

          {/* Priority Reviewer (Shown only when both Codex and DeepSeek are active) */}
          {isCodexActive && isDeepseekActive && (
            <div
              style={{
                marginBottom: 10,
                display: "flex",
                alignItems: "center",
                gap: 8,
                flexWrap: "wrap",
              }}
              data-testid="priority-selection-section"
            >
              <div style={{ display: "flex", alignItems: "center", gap: 6, flexShrink: 0 }}>
                <span style={{ fontSize: 12, fontWeight: 700, color: "#1f2937", whiteSpace: "nowrap" }}>
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
                    "In case of conflicting verdicts between reviewers, the priority reviewer determines the final outcome unless there is an unresolved blocking finding."
                  }
                />
              </div>
              <div style={{ display: "flex", flexWrap: "wrap", gap: 8, alignItems: "center" }}>
                <button
                  type="button"
                  role="radio"
                  aria-checked={reviewSettings?.priority_reviewer === "codex"}
                  className={`codex-review-priority-btn ${reviewSettings?.priority_reviewer === "codex" ? "active" : ""}`}
                  style={{ width: "auto", minWidth: 140, padding: "4px 12px", minHeight: 32 }}
                  disabled={reviewLoading}
                  onClick={() => handleSelectPriority("codex")}
                  data-testid="radio-priority-codex"
                >
                  <span>{t("planManager.roleCodexSession") || "Codex (Session)"}</span>
                </button>
                <button
                  type="button"
                  role="radio"
                  aria-checked={reviewSettings?.priority_reviewer === "deepseek"}
                  className={`codex-review-priority-btn ${reviewSettings?.priority_reviewer === "deepseek" ? "active" : ""}`}
                  style={{ width: "auto", minWidth: 140, padding: "4px 12px", minHeight: 32 }}
                  disabled={reviewLoading}
                  onClick={() => handleSelectPriority("deepseek")}
                  data-testid="radio-priority-deepseek"
                >
                  <span>DeepSeek Harness</span>
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
