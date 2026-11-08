import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type {
  ReviewerPolicyConfig,
  ReviewerPolicy,
  AntigravityPriorityReviewer,
  WorkflowSkillsPolicyStatus,
  CodexAccountInfo,
} from "../types";
import {
  getCodexReviewerModels,
  getDeepSeekReviewerModels,
} from "../modelCapabilities";
import PlanFileManagerCard from "./PlanFileManagerCard";
import ReviewerDiagnosticsModal from "./ReviewerDiagnosticsModal";

export function reviewersToPolicy(reviewers: string[]): ReviewerPolicy {
  const hasCodex = reviewers.includes("codex");
  const hasDeepseek = reviewers.includes("deepseek");
  const hasAntigravity = reviewers.includes("antigravity");

  if (hasCodex && hasDeepseek) {
    return "codex_plus_deepseek";
  }
  if (hasCodex) {
    return "codex_only";
  }
  if (hasDeepseek) {
    return "deepseek_only";
  }
  if (hasAntigravity) {
    return "antigravity";
  }
  return "none";
}

export function policyToReviewers(policy: ReviewerPolicy | undefined | null): string[] {
  switch (policy) {
    case "antigravity":
      return ["antigravity"];
    case "codex_only":
      return ["codex"];
    case "deepseek_only":
      return ["deepseek"];
    case "codex_plus_deepseek":
      return ["codex", "deepseek"];
    case "none":
    default:
      return [];
  }
}

export default function ExternalReviewCard() {
  const { t } = useTranslation();

  const [reviewerPolicy, setReviewerPolicy] = useState<ReviewerPolicyConfig | null>(null);
  const [policyStatus, setPolicyStatus] = useState<WorkflowSkillsPolicyStatus | null>(null);
  const [codexAccount, setCodexAccount] = useState<CodexAccountInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedMessage, setSavedMessage] = useState(false);
  const [showDiagnosticsModal, setShowDiagnosticsModal] = useState(false);

  const isSavingRef = useRef(false);
  const pendingConfigRef = useRef<ReviewerPolicyConfig | null>(null);
  const latestSavedHashRef = useRef<string | null>(null);

  const codexModels = getCodexReviewerModels();
  const deepseekModels = getDeepSeekReviewerModels();

  const fetchPolicy = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const [pol, compliance, account] = await Promise.all([
        invoke<ReviewerPolicyConfig>("get_reviewer_policy"),
        invoke<WorkflowSkillsPolicyStatus>("get_workflow_skills_policy_status"),
        invoke<CodexAccountInfo>("get_codex_account_info").catch(() => null),
      ]);
      setReviewerPolicy(pol);
      latestSavedHashRef.current = pol?.content_hash ?? null;
      setPolicyStatus(compliance);
      if (account) {
        setCodexAccount(account);
      }
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchPolicy();
  }, [fetchPolicy]);

  const processSaveQueue = useCallback(async () => {
    if (isSavingRef.current) return;
    if (!pendingConfigRef.current) return;

    isSavingRef.current = true;
    setSaving(true);
    setError(null);

    while (pendingConfigRef.current) {
      const configToSave = pendingConfigRef.current;
      pendingConfigRef.current = null;

      try {
        const expectedHash = latestSavedHashRef.current ?? reviewerPolicy?.content_hash ?? null;
        const updated = await invoke<ReviewerPolicyConfig>("set_reviewer_policy", {
          config: {
            schema_version: 6,
            policy: configToSave.build_review_policy ?? configToSave.policy,
            plan_review_policy: configToSave.plan_review_policy ?? reviewerPolicy?.plan_review_policy ?? configToSave.policy,
            plan_priority_reviewer: configToSave.plan_priority_reviewer ?? reviewerPolicy?.plan_priority_reviewer ?? null,
            plan_reviewers: configToSave.plan_reviewers !== undefined ? configToSave.plan_reviewers : (reviewerPolicy?.plan_reviewers ?? null),
            build_review_policy: configToSave.build_review_policy ?? configToSave.policy,
            build_priority_reviewer: configToSave.build_priority_reviewer ?? configToSave.priority_reviewer ?? null,
            build_reviewers: configToSave.build_reviewers ?? (reviewerPolicy?.build_reviewers ?? null),
            priority_reviewer: configToSave.priority_reviewer ?? null,
            codex: configToSave.codex ?? reviewerPolicy?.codex ?? null,
            deepseek: configToSave.deepseek ?? reviewerPolicy?.deepseek ?? null,
            content_hash: expectedHash,
            active_plan_path: configToSave.active_plan_path ?? null,
          },
          expectedHash,
        });
        latestSavedHashRef.current = updated.content_hash ?? null;
        if (!pendingConfigRef.current) {
          setReviewerPolicy(updated);
          setSavedMessage(true);
          setTimeout(() => setSavedMessage(false), 3000);
        }
      } catch (e: unknown) {
        setError(String(e));
        if (!pendingConfigRef.current) {
          void fetchPolicy();
        }
        break;
      }
    }

    isSavingRef.current = false;
    setSaving(false);
  }, [fetchPolicy, reviewerPolicy]);

  const savePolicy = useCallback((configToSave: ReviewerPolicyConfig) => {
    if (!policyStatus?.compliant) return;
    if (configToSave.policy === "codex_plus_deepseek" && !configToSave.priority_reviewer) {
      setError(t("antigravity.priorityReviewerRequired"));
      return;
    }
    setReviewerPolicy(configToSave);
    pendingConfigRef.current = configToSave;
    void processSaveQueue();
  }, [policyStatus, t, processSaveQueue]);

  const activeReviewers = reviewerPolicy?.build_reviewers != null
    ? reviewerPolicy.build_reviewers
    : policyToReviewers(reviewerPolicy?.build_review_policy ?? reviewerPolicy?.policy);

  const isAntigravityActive = activeReviewers.includes("antigravity");
  const isCodexActive = activeReviewers.includes("codex");
  const isDeepseekActive = activeReviewers.includes("deepseek");

  const handleToggleReviewer = (target: "antigravity" | "codex" | "deepseek") => {
    if (!reviewerPolicy) return;
    const currentList = activeReviewers;
    const nextList = currentList.includes(target)
      ? currentList.filter((r) => r !== target)
      : [...currentList, target];

    const hasCodex = nextList.includes("codex");
    const hasDeepseek = nextList.includes("deepseek");

    let newCodex = reviewerPolicy.codex;
    let newDeepseek = reviewerPolicy.deepseek;
    let newPriority = reviewerPolicy.priority_reviewer;

    if (!newCodex) {
      newCodex = { model: "gpt-6.1-terra", reasoning_effort: "high" };
    }
    if (!newDeepseek) {
      newDeepseek = { model: "deepseek-flash", reasoning_effort: "high" };
    }

    if (hasCodex && hasDeepseek) {
      if (!newPriority) {
        newPriority = "codex_cli";
      }
    } else {
      newPriority = null;
    }

    const nextPolicy = reviewersToPolicy(nextList);
    const isPlanInheriting = reviewerPolicy.plan_reviewers == null;

    const updatedConfig: ReviewerPolicyConfig = {
      ...reviewerPolicy,
      schema_version: 6,
      policy: nextPolicy,
      build_review_policy: nextPolicy,
      build_reviewers: nextList,
      build_priority_reviewer: newPriority,
      priority_reviewer: newPriority,
      plan_review_policy: isPlanInheriting ? nextPolicy : reviewerPolicy.plan_review_policy,
      plan_priority_reviewer: isPlanInheriting ? newPriority : reviewerPolicy.plan_priority_reviewer,
      codex: newCodex,
      deepseek: newDeepseek,
    };

    setReviewerPolicy(updatedConfig);
    void savePolicy(updatedConfig);
  };

  const handleSelectPolicy = (newPolicy: ReviewerPolicy) => {
    if (!reviewerPolicy) return;
    const nextReviewers = policyToReviewers(newPolicy);
    const hasCodex = nextReviewers.includes("codex");
    const hasDeepseek = nextReviewers.includes("deepseek");

    let newCodex = reviewerPolicy.codex;
    let newDeepseek = reviewerPolicy.deepseek;
    let newPriority = reviewerPolicy.priority_reviewer;

    if (!newCodex) {
      newCodex = { model: "gpt-6.1-terra", reasoning_effort: "high" };
    }
    if (!newDeepseek) {
      newDeepseek = { model: "deepseek-flash", reasoning_effort: "high" };
    }

    if (hasCodex && hasDeepseek) {
      if (!newPriority) {
        newPriority = "codex_cli";
      }
    } else {
      newPriority = null;
    }

    const updatedConfig: ReviewerPolicyConfig = {
      ...reviewerPolicy,
      schema_version: 6,
      policy: newPolicy,
      build_review_policy: newPolicy,
      build_reviewers: nextReviewers,
      build_priority_reviewer: newPriority,
      priority_reviewer: newPriority,
      codex: newCodex,
      deepseek: newDeepseek,
    };
    setReviewerPolicy(updatedConfig);
    if (newPolicy === "codex_plus_deepseek" && !newPriority) {
      return;
    }
    void savePolicy(updatedConfig);
  };

  const handleSelectPriority = (priority: AntigravityPriorityReviewer) => {
    if (!reviewerPolicy) return;
    const updatedConfig: ReviewerPolicyConfig = {
      ...reviewerPolicy,
      priority_reviewer: priority,
    };
    setReviewerPolicy(updatedConfig);
    void savePolicy(updatedConfig);
  };

  const handleCodexModelChange = (modelId: string) => {
    if (!reviewerPolicy) return;
    const modelEntry = codexModels.find((m) => m.id === modelId);
    const defaultEffort = modelEntry?.defaultEffort || "medium";
    const supported = modelEntry?.supportedEfforts || ["medium"];
    const currentEffort = reviewerPolicy.codex?.reasoning_effort || "medium";
    const newEffort = supported.includes(currentEffort) ? currentEffort : defaultEffort;

    const newCodex = { model: modelId, reasoning_effort: newEffort };
    const updatedConfig: ReviewerPolicyConfig = {
      ...reviewerPolicy,
      codex: newCodex,
    };
    setReviewerPolicy(updatedConfig);
    void savePolicy(updatedConfig);
  };

  const handleCodexEffortChange = (effort: string) => {
    if (!reviewerPolicy) return;
    const currentModel = reviewerPolicy.codex?.model || "gpt-6.1-terra";
    const newCodex = { model: currentModel, reasoning_effort: effort };
    const updatedConfig: ReviewerPolicyConfig = {
      ...reviewerPolicy,
      codex: newCodex,
    };
    setReviewerPolicy(updatedConfig);
    void savePolicy(updatedConfig);
  };

  const handleDeepSeekModelChange = (modelId: string) => {
    if (!reviewerPolicy) return;
    const modelEntry = deepseekModels.find((m) => m.id === modelId);
    const defaultEffort = modelEntry?.defaultEffort || "high";
    const supported = modelEntry?.supportedEfforts || ["high"];
    const currentEffort = reviewerPolicy.deepseek?.reasoning_effort || "high";
    const newEffort = supported.includes(currentEffort) ? currentEffort : defaultEffort;

    const newDeepseek = { model: modelId, reasoning_effort: newEffort };
    const updatedConfig: ReviewerPolicyConfig = {
      ...reviewerPolicy,
      deepseek: newDeepseek,
    };
    setReviewerPolicy(updatedConfig);
    void savePolicy(updatedConfig);
  };

  const handleDeepSeekEffortChange = (effort: string) => {
    if (!reviewerPolicy) return;
    const currentModel = reviewerPolicy.deepseek?.model || "deepseek-flash";
    const newDeepseek = { model: currentModel, reasoning_effort: effort };
    const updatedConfig: ReviewerPolicyConfig = {
      ...reviewerPolicy,
      deepseek: newDeepseek,
    };
    setReviewerPolicy(updatedConfig);
    void savePolicy(updatedConfig);
  };

  const handleActivePlanChanged = useCallback((planRelPath: string | null) => {
    setReviewerPolicy((prev) => {
      if (!prev || prev.active_plan_path === planRelPath) return prev;
      return { ...prev, active_plan_path: planRelPath };
    });
  }, []);

  const handleRolesChanged = useCallback(
    ({
      plannerRole,
      reviserRole,
      planReviewPolicy,
      planReviewers,
    }: {
      plannerRole?: string | null;
      reviserRole?: string | null;
      planReviewPolicy?: ReviewerPolicy;
      planReviewers?: string[] | null;
    }) => {
      if (!reviewerPolicy) return;
      if (plannerRole !== undefined || reviserRole !== undefined) return;
      const nextPlanReviewers = planReviewers !== undefined ? planReviewers : (reviewerPolicy.plan_reviewers ?? null);

      let nextPlanPolicy: ReviewerPolicy;
      let nextPlanPriority = reviewerPolicy.plan_priority_reviewer;
      let newCodex = reviewerPolicy.codex ?? { model: "gpt-6.1-terra", reasoning_effort: "high" };
      let newDeepseek = reviewerPolicy.deepseek ?? { model: "deepseek-flash", reasoning_effort: "high" };

      if (nextPlanReviewers === null) {
        nextPlanPolicy = reviewerPolicy.build_review_policy ?? reviewerPolicy.policy ?? "none";
        nextPlanPriority = reviewerPolicy.build_priority_reviewer ?? reviewerPolicy.priority_reviewer ?? null;
      } else {
        nextPlanPolicy = planReviewPolicy !== undefined ? planReviewPolicy : reviewersToPolicy(nextPlanReviewers);
        const hasCodex = nextPlanReviewers.includes("codex");
        const hasDeepseek = nextPlanReviewers.includes("deepseek");
        if (hasCodex && hasDeepseek) {
          if (!nextPlanPriority) {
            nextPlanPriority = "codex_cli";
          }
        } else {
          nextPlanPriority = null;
        }
      }

      if (
        reviewerPolicy.plan_review_policy === nextPlanPolicy &&
        reviewerPolicy.plan_priority_reviewer === nextPlanPriority &&
        JSON.stringify(reviewerPolicy.plan_reviewers ?? null) === JSON.stringify(nextPlanReviewers)
      ) {
        return;
      }
      const updated: ReviewerPolicyConfig = {
        ...reviewerPolicy,
        plan_reviewers: nextPlanReviewers,
        plan_review_policy: nextPlanPolicy,
        plan_priority_reviewer: nextPlanPriority,
        codex: newCodex,
        deepseek: newDeepseek,
      };
      setReviewerPolicy(updated);
      void savePolicy(updated);
    },
    [reviewerPolicy, policyStatus]
  );

  const handlePlanReviewPolicyChanged = useCallback(
    (newPolicy: ReviewerPolicy) => {
      if (!reviewerPolicy) return;
      let newCodex = reviewerPolicy.codex;
      let newDeepseek = reviewerPolicy.deepseek;
      let newPriority = reviewerPolicy.plan_priority_reviewer ?? reviewerPolicy.priority_reviewer;

      if (newPolicy !== "none" && newPolicy !== "antigravity") {
        if ((newPolicy === "codex_only" || newPolicy === "codex_plus_deepseek") && !newCodex) {
          newCodex = { model: "gpt-6.1-terra", reasoning_effort: "high" };
        }
        if ((newPolicy === "deepseek_only" || newPolicy === "codex_plus_deepseek") && !newDeepseek) {
          newDeepseek = { model: "deepseek-flash", reasoning_effort: "high" };
        }
      }

      if (newPolicy !== "codex_plus_deepseek") {
        newPriority = null;
      } else if (!newPriority) {
        newPriority = "codex_cli";
      }

      const updated: ReviewerPolicyConfig = {
        ...reviewerPolicy,
        plan_review_policy: newPolicy,
        plan_priority_reviewer: newPriority,
        codex: newCodex,
        deepseek: newDeepseek,
      };
      setReviewerPolicy(updated);
      void savePolicy(updated);
    },
    [reviewerPolicy, policyStatus]
  );

  const selectedPolicy = reviewerPolicy?.build_review_policy ?? reviewerPolicy?.policy ?? "none";
  const selectedCodexModel = reviewerPolicy?.codex?.model ?? "gpt-6.1-terra";
  const selectedCodexEffort = reviewerPolicy?.codex?.reasoning_effort ?? "high";
  const selectedDeepSeekModel = reviewerPolicy?.deepseek?.model ?? "deepseek-flash";
  const selectedDeepSeekEffort = reviewerPolicy?.deepseek?.reasoning_effort ?? "high";

  const currentCodexEntry = codexModels.find((m) => m.id === selectedCodexModel);
  const currentDeepSeekEntry = deepseekModels.find((m) => m.id === selectedDeepSeekModel);

  return (
    <>
      <div className="settings-tile" data-testid="reviewer-policy-card">
        <PlanFileManagerCard
          embedded
          surface="antigravity"
          activePlanPath={reviewerPolicy?.active_plan_path}
          planReviewPolicy={reviewerPolicy?.plan_review_policy ?? reviewerPolicy?.policy}
          planReviewers={reviewerPolicy?.plan_reviewers ?? null}
          onActivePlanChanged={handleActivePlanChanged}
          onRolesChanged={handleRolesChanged}
          onPlanReviewPolicyChanged={handlePlanReviewPolicyChanged}
        />

        <div
          style={{
            borderTop: "1px solid var(--color-border, #e5e7eb)",
            paddingTop: 12,
            marginTop: 12,
          }}
        >
          {/* ── HEADER ROW: Title, Model Badge, Short Description & Diagnostics Button ── */}
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 10, flexWrap: "wrap", gap: 10 }}>
            <div style={{ display: "flex", alignItems: "baseline", gap: 10, flexWrap: "wrap", minWidth: 0 }}>
              <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
                {t("antigravity.reviewerPolicyTitle")}
              </h3>

              {/* Codex Model & Effort (Codex使用時のみ表示) */}
              {isCodexActive && (
                <span style={{ fontSize: 12 }}>
                  <span style={{ color: "var(--color-text-secondary, #656d76)" }}>
                    Codex CLI:{" "}
                  </span>
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
                {t("antigravity.reviewerPolicyDesc")}
              </span>

              {(loading || saving) && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
            </div>

            <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
              {savedMessage && (
                <span style={{ fontSize: 12, color: "#1a7f37", fontWeight: 500 }} data-testid="save-success-msg">
                  ✓ {t("antigravity.reviewerSaveSuccess")}
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
            </div>
          </div>

          {error && (
            <div style={{ margin: "0 0 10px 0", padding: "8px 12px", background: "#ffebe9", border: "1px solid #ff8182", borderRadius: 6, fontSize: 12, color: "#cf222e" }}>
              {error}
            </div>
          )}

          {policyStatus && !policyStatus.compliant && (
            <div style={{ margin: "0 0 10px 0", padding: "8px 12px", background: "#fff8c5", border: "1px solid #d4a72c", borderRadius: 6, fontSize: 12, color: "#6e4a00" }}>
              ⚠ {t("antigravity.reviewerPolicyUpdateRequiredNotice")}
            </div>
          )}

          {reviewerPolicy?.unconfirmed_migration && (
            <div
              data-testid="reviewer-migration-banner"
              style={{
                margin: "0 0 10px 0",
                padding: "8px 12px",
                background: "#fff8c5",
                border: "1px solid #d4a72c",
                borderRadius: 6,
                fontSize: 12,
                color: "#6e4a00",
              }}
            >
              ⚠ {t("antigravity.priorityReviewerMigrationNotice")}
            </div>
          )}

          {/* ── REVIEW MODE ROW (Multi-select toggles for Antigravity, Codex CLI, DeepSeek Harness) ── */}
          <div style={{ marginBottom: 10 }}>
            <div
              className="pov-segment-group"
              role="group"
              aria-label={t("antigravity.reviewerPolicyTitle")}
              style={{ flexWrap: "wrap", display: "inline-flex" }}
              data-testid="reviewer-policy-group"
            >
              <label
                className={`pov-segment-btn ${isAntigravityActive ? "active" : ""}`}
                style={{ cursor: policyStatus?.compliant ? "pointer" : "not-allowed", display: "inline-flex", alignItems: "center", gap: 4 }}
                data-testid="reviewer-toggle-antigravity"
              >
                <input
                  type="checkbox"
                  name="reviewerPolicyAntigravity"
                  data-testid="reviewer-policy-antigravity"
                  className="sr-only"
                  checked={isAntigravityActive}
                  disabled={!policyStatus?.compliant || loading || saving}
                  onChange={() => handleToggleReviewer("antigravity")}
                />
                <span>{isAntigravityActive ? "✓ Antigravity" : "Antigravity"}</span>
              </label>

              <label
                className={`pov-segment-btn ${isCodexActive ? "active" : ""}`}
                style={{ cursor: policyStatus?.compliant ? "pointer" : "not-allowed", display: "inline-flex", alignItems: "center", gap: 4 }}
                data-testid="reviewer-toggle-codex"
              >
                <input
                  type="checkbox"
                  name="reviewerPolicyCodex"
                  data-testid="reviewer-policy-codex-only"
                  className="sr-only"
                  checked={isCodexActive}
                  disabled={!policyStatus?.compliant || loading || saving}
                  onChange={() => handleToggleReviewer("codex")}
                />
                <span>{isCodexActive ? "✓ Codex CLI" : "Codex CLI"}</span>
              </label>

              <label
                className={`pov-segment-btn ${isDeepseekActive ? "active" : ""}`}
                style={{ cursor: policyStatus?.compliant ? "pointer" : "not-allowed", display: "inline-flex", alignItems: "center", gap: 4 }}
                data-testid="reviewer-toggle-deepseek"
              >
                <input
                  type="checkbox"
                  name="reviewerPolicyDeepseek"
                  data-testid="reviewer-policy-deepseek-only"
                  className="sr-only"
                  checked={isDeepseekActive}
                  disabled={!policyStatus?.compliant || loading || saving}
                  onChange={() => handleToggleReviewer("deepseek")}
                />
                <span>{isDeepseekActive ? "✓ DeepSeek Harness" : "DeepSeek Harness"}</span>
              </label>
            </div>
          </div>

          {/* ── PRIORITY REVIEWER SELECTOR (Only in Codex + DeepSeek) ── */}
          {isCodexActive && isDeepseekActive && (
            <div style={{ marginBottom: 10, display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
              <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary, #4b5563)" }}>
                {t("antigravity.priorityReviewerTitle")}:
              </span>
              <div className="pov-segment-group" role="radiogroup" aria-label={t("antigravity.priorityReviewerTitle")}>
                <label
                  className={`pov-segment-btn ${reviewerPolicy?.priority_reviewer === "codex_cli" ? "active" : ""}`}
                  style={{ cursor: policyStatus?.compliant ? "pointer" : "not-allowed", display: "inline-flex", alignItems: "center" }}
                >
                  <input
                    type="radio"
                    name="priorityReviewer"
                    value="codex_cli"
                    data-testid="priority-reviewer-codex-cli"
                    className="sr-only"
                    checked={reviewerPolicy?.priority_reviewer === "codex_cli"}
                    disabled={!policyStatus?.compliant || loading || saving}
                    onChange={() => handleSelectPriority("codex_cli")}
                  />
                  <span>{t("antigravity.priorityCodexCli")}</span>
                </label>
                <label
                  className={`pov-segment-btn ${reviewerPolicy?.priority_reviewer === "deepseek" ? "active" : ""}`}
                  style={{ cursor: policyStatus?.compliant ? "pointer" : "not-allowed", display: "inline-flex", alignItems: "center" }}
                >
                  <input
                    type="radio"
                    name="priorityReviewer"
                    value="deepseek"
                    data-testid="priority-reviewer-deepseek"
                    className="sr-only"
                    checked={reviewerPolicy?.priority_reviewer === "deepseek"}
                    disabled={!policyStatus?.compliant || loading || saving}
                    onChange={() => handleSelectPriority("deepseek")}
                  />
                  <span>{t("antigravity.priorityDeepSeek")}</span>
                </label>
              </div>
              {!reviewerPolicy?.priority_reviewer && (
                <span data-testid="priority-reviewer-summary" style={{ fontSize: 11, color: "var(--error, #cf222e)", fontWeight: 600 }}>
                  {t("antigravity.priorityReviewerNotSelected")}
                </span>
              )}
            </div>
          )}

          {/* ── DETAILED REVIEWER CONFIGURATION ── */}
          {(isCodexActive || isDeepseekActive) && (
            <div style={{ display: "flex", flexDirection: "column", gap: 10, marginTop: 6 }}>
          {/* Codex CLI Detailed Settings (Compact 1-row layout) */}
          {isCodexActive && (
            <div
              style={{
                border: "1px solid #d0d7de",
                borderRadius: 6,
                padding: "8px 12px",
                background: "#ffffff",
                display: "flex",
                alignItems: "center",
                gap: 12,
                flexWrap: "wrap",
              }}
              data-testid="codex-model-settings"
            >
              <span style={{ fontWeight: 600, fontSize: 13, color: "var(--text-primary, #24292f)", whiteSpace: "nowrap" }}>
                {t("antigravity.cliToolCodexTitle") || "Codex CLI"}
              </span>

              <div style={{ display: "flex", alignItems: "center", gap: 6, flexShrink: 0 }}>
                <span style={{ fontSize: 11, fontWeight: 600, color: "var(--text-secondary, #57606a)", whiteSpace: "nowrap" }}>
                  {t("antigravity.reviewerModel") || "モデル"}
                </span>
                <div className="pov-segment-group" role="radiogroup" aria-label={t("antigravity.reviewerModel") || "モデル"} data-testid="codex-model-group">
                  {codexModels.map((m) => (
                    <button
                      key={m.id}
                      type="button"
                      className={`pov-segment-btn ${selectedCodexModel === m.id ? "active" : ""}`}
                      data-testid={`codex-model-${m.id}`}
                      onClick={() => handleCodexModelChange(m.id)}
                      disabled={!policyStatus?.compliant || saving}
                    >
                      {m.displayName}
                    </button>
                  ))}
                </div>
              </div>

              <div style={{ display: "flex", alignItems: "center", gap: 6, flexShrink: 0 }}>
                <span style={{ fontSize: 11, fontWeight: 600, color: "var(--text-secondary, #57606a)", whiteSpace: "nowrap" }}>
                  {t("antigravity.reviewerReasoningEffort") || "推論強度"}
                </span>
                <div className="pov-segment-group" role="radiogroup" aria-label={t("antigravity.reviewerReasoningEffort") || "推論強度"} data-testid="codex-effort-group">
                  {(currentCodexEntry?.supportedEfforts ?? ["low", "medium", "high", "xhigh", "max"]).map((effort) => {
                    const effortKey = `antigravity.reviewerEffort${effort.charAt(0).toUpperCase() + effort.slice(1)}` as any;
                    const label = t(effortKey) || (effort === "none" ? "Off" : effort);
                    return (
                      <button
                        key={effort}
                        type="button"
                        className={`pov-segment-btn ${selectedCodexEffort === effort ? "active" : ""}`}
                        data-testid={`codex-effort-${effort}`}
                        onClick={() => handleCodexEffortChange(effort)}
                        disabled={!policyStatus?.compliant || saving}
                      >
                        {label}
                      </button>
                    );
                  })}
                </div>
              </div>
            </div>
          )}

          {/* DeepSeek Harness Detailed Settings (Compact 1-row layout) */}
          {isDeepseekActive && (
            <div
              style={{
                border: "1px solid #d0d7de",
                borderRadius: 6,
                padding: "8px 12px",
                background: "#ffffff",
                display: "flex",
                alignItems: "center",
                gap: 12,
                flexWrap: "wrap",
              }}
              data-testid="deepseek-model-settings"
            >
              <span style={{ fontWeight: 600, fontSize: 13, color: "var(--text-primary, #24292f)", whiteSpace: "nowrap" }}>
                {t("antigravity.cliToolDeepSeekTitle") || "DeepSeek Harness"}
              </span>

              <div style={{ display: "flex", alignItems: "center", gap: 6, flexShrink: 0 }}>
                <span style={{ fontSize: 11, fontWeight: 600, color: "var(--text-secondary, #57606a)", whiteSpace: "nowrap" }}>
                  {t("antigravity.reviewerModel") || "モデル"}
                </span>
                <div className="pov-segment-group" role="radiogroup" aria-label={t("antigravity.reviewerModel") || "モデル"} data-testid="deepseek-model-group">
                  {deepseekModels.map((m) => (
                    <button
                      key={m.id}
                      type="button"
                      className={`pov-segment-btn ${selectedDeepSeekModel === m.id ? "active" : ""}`}
                      data-testid={`deepseek-model-${m.id}`}
                      onClick={() => handleDeepSeekModelChange(m.id)}
                      disabled={!policyStatus?.compliant || saving}
                    >
                      {m.displayName}
                    </button>
                  ))}
                </div>
              </div>

              <div style={{ display: "flex", alignItems: "center", gap: 6, flexShrink: 0 }}>
                <span style={{ fontSize: 11, fontWeight: 600, color: "var(--text-secondary, #57606a)", whiteSpace: "nowrap" }}>
                  {t("antigravity.reviewerReasoningEffort") || "推論強度"}
                </span>
                <div className="pov-segment-group" role="radiogroup" aria-label={t("antigravity.reviewerReasoningEffort") || "推論強度"} data-testid="deepseek-effort-group">
                  {(currentDeepSeekEntry?.supportedEfforts ?? ["low", "high", "max"]).map((effort) => {
                    const effortKey = `antigravity.reviewerEffort${effort.charAt(0).toUpperCase() + effort.slice(1)}` as any;
                    const label = t(effortKey) || effort;
                    return (
                      <button
                        key={effort}
                        type="button"
                        className={`pov-segment-btn ${selectedDeepSeekEffort === effort ? "active" : ""}`}
                        data-testid={`deepseek-effort-${effort}`}
                        onClick={() => handleDeepSeekEffortChange(effort)}
                        disabled={!policyStatus?.compliant || saving}
                      >
                        {label}
                      </button>
                    );
                  })}
                </div>
              </div>
            </div>
          )}
        </div>
      )}
        </div>
      </div>

      <ReviewerDiagnosticsModal
        isOpen={showDiagnosticsModal}
        onClose={() => setShowDiagnosticsModal(false)}
      />
    </>
  );
}
