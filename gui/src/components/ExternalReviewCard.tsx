import { useState, useEffect, useCallback } from "react";
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
import openrouterPricingData from "../config/openrouterPricing.json";

interface OpenRouterPricingEntry {
  pricing?: {
    inputPerMillionUsd?: number;
    outputPerMillionUsd?: number;
  };
}

const openrouterPricing = openrouterPricingData as Record<string, OpenRouterPricingEntry>;

function formatPrice(input?: number, output?: number): string {
  if (input === undefined || output === undefined) return "";
  return `($${input.toFixed(2)} / $${output.toFixed(2)} / 1M)`;
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

  // Accordion open/close states (default: closed)
  const [codexAccordionOpen, setCodexAccordionOpen] = useState(false);
  const [deepseekAccordionOpen, setDeepseekAccordionOpen] = useState(false);

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

  const savePolicy = async (configToSave: ReviewerPolicyConfig) => {
    if (!policyStatus?.compliant) return;
    if (configToSave.policy === "codex_plus_deepseek" && !configToSave.priority_reviewer) {
      setError(t("antigravity.priorityReviewerRequired"));
      return;
    }
    try {
      setSaving(true);
      setError(null);
      const updated = await invoke<ReviewerPolicyConfig>("set_reviewer_policy", {
        config: {
          schema_version: 4,
          policy: configToSave.policy,
          priority_reviewer: configToSave.priority_reviewer ?? null,
          codex: configToSave.codex ?? null,
          deepseek: configToSave.deepseek ?? null,
          content_hash: reviewerPolicy?.content_hash ?? null,
        },
        expectedHash: reviewerPolicy?.content_hash ?? null,
      });
      setReviewerPolicy(updated);
      setSavedMessage(true);
      setTimeout(() => setSavedMessage(false), 3000);
    } catch (e: unknown) {
      setError(String(e));
      void fetchPolicy();
    } finally {
      setSaving(false);
    }
  };

  const handleSelectPolicy = (newPolicy: ReviewerPolicy) => {
    if (!reviewerPolicy) return;
    let newCodex = reviewerPolicy.codex;
    let newDeepseek = reviewerPolicy.deepseek;
    let newPriority = reviewerPolicy.priority_reviewer;

    if (newPolicy !== "none") {
      if (!newCodex) {
        newCodex = { model: "gpt-6.1-terra", reasoning_effort: "high" };
      }
      if (newPolicy === "codex_plus_deepseek" && !newDeepseek) {
        newDeepseek = { model: "deepseek-flash", reasoning_effort: "high" };
      }
    }

    if (newPolicy !== "codex_plus_deepseek") {
      newPriority = null;
    }

    const updatedConfig: ReviewerPolicyConfig = {
      ...reviewerPolicy,
      policy: newPolicy,
      priority_reviewer: newPriority,
      codex: newPolicy === "none" ? null : newCodex,
      deepseek: newPolicy === "codex_plus_deepseek" ? newDeepseek : null,
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

  const selectedPolicy = reviewerPolicy?.policy ?? "none";
  const selectedCodexModel = reviewerPolicy?.codex?.model ?? "gpt-6.1-terra";
  const selectedCodexEffort = reviewerPolicy?.codex?.reasoning_effort ?? "high";
  const selectedDeepSeekModel = reviewerPolicy?.deepseek?.model ?? "deepseek-flash";
  const selectedDeepSeekEffort = reviewerPolicy?.deepseek?.reasoning_effort ?? "high";

  const currentCodexEntry = codexModels.find((m) => m.id === selectedCodexModel);
  const currentDeepSeekEntry = deepseekModels.find((m) => m.id === selectedDeepSeekModel);

  return (
    <div className="settings-tile" data-testid="reviewer-policy-card">
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6, flexWrap: "wrap", gap: 8 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
            {t("antigravity.reviewerPolicyTitle")}
          </h3>
          {(loading || saving) && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
        </div>
        {savedMessage && (
          <span style={{ fontSize: 12, color: "#1a7f37", fontWeight: 500 }} data-testid="save-success-msg">
            ✓ {t("antigravity.reviewerSaveSuccess")}
          </span>
        )}
      </div>

      <p style={{ margin: "0 0 12px 0", fontSize: 12, color: "#656d76" }}>
        {t("antigravity.reviewerPolicyDesc")}
      </p>

      {error && (
        <div style={{ margin: "0 0 12px 0", padding: "8px 12px", background: "#ffebe9", border: "1px solid #ff8182", borderRadius: 6, fontSize: 12, color: "#cf222e" }}>
          {error}
        </div>
      )}

      {policyStatus && !policyStatus.compliant && (
        <div style={{ margin: "0 0 12px 0", padding: "8px 12px", background: "#fff8c5", border: "1px solid #d4a72c", borderRadius: 6, fontSize: 12, color: "#6e4a00" }}>
          ⚠ {t("antigravity.reviewerPolicyUpdateRequiredNotice")}
        </div>
      )}

      {reviewerPolicy?.unconfirmed_migration && (
        <div
          data-testid="reviewer-migration-banner"
          style={{
            margin: "0 0 12px 0",
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

      {/* Review Mode Single-Row Selector (§10.3) */}
      <div id="reviewer-options-box" className="pov-setting-row" style={{ marginTop: 4, marginBottom: 2 }}>
        <div className="pov-setting-label">
          {t("antigravity.reviewerPolicyTitle")}
        </div>
        <div className="pov-setting-control">
          <div className="pov-segment-group" role="radiogroup" aria-label={t("antigravity.reviewerPolicyTitle")}>
            <label
              className={`pov-segment-btn ${selectedPolicy === "none" ? "active" : ""}`}
              style={{ cursor: policyStatus?.compliant ? "pointer" : "not-allowed", display: "inline-flex", alignItems: "center" }}
            >
              <input
                type="radio"
                name="reviewerPolicy"
                value="none"
                data-testid="reviewer-policy-none"
                className="sr-only"
                checked={selectedPolicy === "none"}
                disabled={!policyStatus?.compliant || loading || saving}
                onChange={() => handleSelectPolicy("none")}
              />
              <span>{t("antigravity.reviewerPolicyDisabled")}</span>
            </label>
            <label
              className={`pov-segment-btn ${selectedPolicy === "codex_only" ? "active" : ""}`}
              style={{ cursor: policyStatus?.compliant ? "pointer" : "not-allowed", display: "inline-flex", alignItems: "center" }}
            >
              <input
                type="radio"
                name="reviewerPolicy"
                value="codex_only"
                data-testid="reviewer-policy-codex-only"
                className="sr-only"
                checked={selectedPolicy === "codex_only"}
                disabled={!policyStatus?.compliant || loading || saving}
                onChange={() => handleSelectPolicy("codex_only")}
              />
              <span>{t("antigravity.reviewerPolicyCodexOnly")}</span>
            </label>
            <label
              className={`pov-segment-btn ${selectedPolicy === "codex_plus_deepseek" ? "active" : ""}`}
              style={{ cursor: policyStatus?.compliant ? "pointer" : "not-allowed", display: "inline-flex", alignItems: "center" }}
            >
              <input
                type="radio"
                name="reviewerPolicy"
                value="codex_plus_deepseek"
                data-testid="reviewer-policy-codex-plus-deepseek"
                className="sr-only"
                checked={selectedPolicy === "codex_plus_deepseek"}
                disabled={!policyStatus?.compliant || loading || saving}
                onChange={() => handleSelectPolicy("codex_plus_deepseek")}
              />
              <span>{t("antigravity.reviewerPolicyCodexPlusDeepseek")}</span>
            </label>
          </div>
        </div>
      </div>
      <div style={{ fontSize: 11, color: "var(--text-secondary, #656d76)", marginBottom: 10 }}>
        {selectedPolicy === "none" && t("antigravity.reviewerPolicyDisabledDesc")}
        {selectedPolicy === "codex_only" && t("antigravity.reviewerPolicyCodexOnlyDesc")}
        {selectedPolicy === "codex_plus_deepseek" && t("antigravity.reviewerPolicyCodexPlusDeepseekDesc")}
      </div>

      {/* Priority Reviewer Single-Row Selector (§20.4) */}
      {selectedPolicy === "codex_plus_deepseek" && (
        <>
          <div id="priority-reviewer-box" className="pov-setting-row" style={{ marginTop: 8, marginBottom: 2 }}>
            <div className="pov-setting-label">
              {t("antigravity.priorityReviewerTitle")}
            </div>
            <div className="pov-setting-control">
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
            </div>
          </div>
          <div style={{ fontSize: 11, color: "var(--text-secondary, #656d76)", marginBottom: 10 }}>
            {t("antigravity.priorityReviewerDesc")}
          </div>
        </>
      )}

      {/* Effective Configuration Summary */}
      <div
        data-testid="reviewer-config-summary"
        style={{
          marginTop: 12,
          padding: "10px 14px",
          background: selectedPolicy === "none" ? "#f6f8fa" : "#f0f7ff",
          border: selectedPolicy === "none" ? "1px solid #d0d7de" : "1px solid #c8e1ff",
          borderRadius: 6,
          fontSize: 12,
        }}
      >
        <div style={{ fontWeight: 600, color: "#24292f", marginBottom: 4 }}>
          {t("antigravity.effectiveConfigTitle")}
        </div>
        {selectedPolicy === "none" ? (
          <div style={{ color: "#57606a" }}>
            {t("antigravity.effectiveReviewDisabled")}
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {/* Codex CLI summary */}
            <div style={{ display: "flex", flexWrap: "wrap", gap: 12, color: "#24292f" }}>
              <span>
                <strong>Codex CLI:</strong> {currentCodexEntry?.displayName ?? selectedCodexModel} ({selectedCodexEffort})
              </span>
              <span>
                <strong>{t("antigravity.accountIdentityLabel")}:</strong>{" "}
                {codexAccount?.account_identity ?? t("antigravity.accountIdentityUnavailable")}
              </span>
              <span>
                <strong>{t("antigravity.authMethodLabel")}:</strong>{" "}
                {codexAccount?.auth_method ?? "Unknown"}
              </span>
              <span>
                <strong>{t("antigravity.configProfileLabel")}:</strong>{" "}
                {codexAccount?.config_profile ?? t("antigravity.configProfileDefault")}
              </span>
              {codexAccount?.codex_home && (
                <span>
                  <strong>Codex Home:</strong> <code>{codexAccount.codex_home}</code>
                </span>
              )}
            </div>

            {/* DeepSeek Harness summary */}
            {selectedPolicy === "codex_plus_deepseek" && (
              <>
                <div style={{ display: "flex", flexWrap: "wrap", gap: 12, color: "#24292f", borderTop: "1px dashed #d0d7de", paddingTop: 4 }}>
                  <span>
                    <strong>DeepSeek Harness:</strong> {currentDeepSeekEntry?.displayName ?? selectedDeepSeekModel} ({selectedDeepSeekEffort})
                  </span>
                  <span style={{ color: "#57606a", fontSize: 11 }}>
                    {selectedDeepSeekModel === "deepseek-flash" ? "($0.15 / $0.60 / 1M)" : "($0.55 / $2.19 / 1M)"}
                  </span>
                </div>
                <div style={{ display: "flex", flexWrap: "wrap", gap: 12, color: "#24292f", borderTop: "1px dashed #d0d7de", paddingTop: 4 }}>
                  <span data-testid="priority-reviewer-summary">
                    <strong>{t("antigravity.priorityReviewerTitle")}:</strong>{" "}
                    {reviewerPolicy?.priority_reviewer === "codex_cli"
                      ? t("antigravity.priorityCodexCli")
                      : reviewerPolicy?.priority_reviewer === "deepseek"
                      ? t("antigravity.priorityDeepSeek")
                      : <span style={{ color: "var(--error, #cf222e)", fontWeight: 600 }}>{t("antigravity.priorityReviewerNotSelected")}</span>}
                  </span>
                </div>
              </>
            )}
          </div>
        )}
      </div>

      {/* Accordions for Direct Reviewer Configuration (When not 'none') */}
      {selectedPolicy !== "none" && (
        <div style={{ display: "flex", flexDirection: "column", gap: 12, marginTop: 16 }}>
          {/* Codex CLI Accordion */}
          <div style={{ border: "1px solid #d0d7de", borderRadius: 6, overflow: "hidden" }}>
            <button
              type="button"
              data-testid="codex-accordion-toggle"
              onClick={() => setCodexAccordionOpen((prev) => !prev)}
              style={{
                width: "100%",
                padding: "10px 14px",
                background: "#f6f8fa",
                border: "none",
                textAlign: "left",
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                cursor: "pointer",
                fontWeight: 600,
                fontSize: 13,
                color: "#24292f",
              }}
            >
              <span>{t("antigravity.cliToolCodexTitle")}</span>
              <span>{codexAccordionOpen ? "▲" : "▼"}</span>
            </button>

            {codexAccordionOpen && (
              <div style={{ padding: "14px", background: "#ffffff", borderTop: "1px solid #d0d7de" }}>
                <div style={{ display: "grid", gridTemplateColumns: "1fr", gap: 10 }}>
                  <div>
                    <label style={{ display: "block", fontSize: 11, fontWeight: 600, color: "#57606a", marginBottom: 4 }}>
                      {t("antigravity.reviewerModel")}
                    </label>
                    <select
                      className="settings-input"
                      data-testid="codex-model-select"
                      value={selectedCodexModel}
                      disabled={!policyStatus?.compliant || saving}
                      onChange={(e) => handleCodexModelChange(e.target.value)}
                      style={{ width: "100%", padding: "6px 8px", fontSize: 12 }}
                    >
                      {codexModels.map((m) => (
                        <option key={m.id} value={m.id}>
                          {m.displayName}
                        </option>
                      ))}
                    </select>
                  </div>

                  <div>
                    <label style={{ display: "block", fontSize: 11, fontWeight: 600, color: "#57606a", marginBottom: 4 }}>
                      {t("antigravity.reviewerReasoningEffort")}
                    </label>
                    <div style={{ display: "flex", gap: 4, flexWrap: "wrap", background: "#f6f8fa", padding: 4, borderRadius: 6, border: "1px solid #e1e4e8" }}>
                      {(currentCodexEntry?.supportedEfforts ?? ["low", "medium", "high", "xhigh", "max"]).map((effort) => {
                        const effortKey = `antigravity.reviewerEffort${effort.charAt(0).toUpperCase() + effort.slice(1)}` as any;
                        const label = t(effortKey) || effort;
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
              </div>
            )}
          </div>

          {/* DeepSeek Harness Accordion (Only in Codex + DeepSeek) */}
          {selectedPolicy === "codex_plus_deepseek" && (
            <div style={{ border: "1px solid #d0d7de", borderRadius: 6, overflow: "hidden" }}>
              <button
                type="button"
                data-testid="deepseek-accordion-toggle"
                onClick={() => setDeepseekAccordionOpen((prev) => !prev)}
                style={{
                  width: "100%",
                  padding: "10px 14px",
                  background: "#f6f8fa",
                  border: "none",
                  textAlign: "left",
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                  cursor: "pointer",
                  fontWeight: 600,
                  fontSize: 13,
                  color: "#24292f",
                }}
              >
                <span>{t("antigravity.cliToolDeepSeekTitle")}</span>
                <span>{deepseekAccordionOpen ? "▲" : "▼"}</span>
              </button>

              {deepseekAccordionOpen && (
                <div style={{ padding: "14px", background: "#ffffff", borderTop: "1px solid #d0d7de" }}>
                  <div style={{ display: "grid", gridTemplateColumns: "1fr", gap: 10 }}>
                    <div>
                      <label style={{ display: "block", fontSize: 11, fontWeight: 600, color: "#57606a", marginBottom: 4 }}>
                        {t("antigravity.reviewerModel")}
                      </label>
                      <select
                        className="settings-input"
                        data-testid="deepseek-model-select"
                        value={selectedDeepSeekModel}
                        disabled={!policyStatus?.compliant || saving}
                        onChange={(e) => handleDeepSeekModelChange(e.target.value)}
                        style={{ width: "100%", padding: "6px 8px", fontSize: 12 }}
                      >
                        {deepseekModels.map((m) => {
                          const priceStr = m.id === "deepseek-flash" ? "($0.15 / $0.60 / 1M)" : "($0.55 / $2.19 / 1M)";
                          return (
                            <option key={m.id} value={m.id}>
                              {m.displayName} {priceStr}
                            </option>
                          );
                        })}
                      </select>
                    </div>

                    <div>
                      <label style={{ display: "block", fontSize: 11, fontWeight: 600, color: "#57606a", marginBottom: 4 }}>
                        {t("antigravity.reviewerReasoningEffort")}
                      </label>
                      <div style={{ display: "flex", gap: 4, flexWrap: "wrap", background: "#f6f8fa", padding: 4, borderRadius: 6, border: "1px solid #e1e4e8" }}>
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
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
