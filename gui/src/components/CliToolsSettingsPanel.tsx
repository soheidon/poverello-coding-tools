import React, { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type { NodePrerequisiteStatus, CodexReviewSettings } from "../types";
import CliToolsCard from "./CliToolsCard";

export function NodePrerequisitesCard() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<NodePrerequisiteStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchStatus = useCallback(async (forceRefresh = false) => {
    try {
      setLoading(true);
      setError(null);
      const res = await invoke<NodePrerequisiteStatus>("get_node_prerequisites_status", { refresh: forceRefresh });
      setStatus(res);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const handleOpenDownload = async () => {
    try {
      await invoke("open_node_download_page");
    } catch (e: unknown) {
      setError(String(e));
    }
  };

  useEffect(() => {
    void fetchStatus(false);
  }, [fetchStatus]);

  return (
    <div className="settings-tile" data-testid="node-prerequisites-card">
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: 6,
          flexWrap: "wrap",
          gap: 8,
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
            {t("cliTools.nodePrerequisitesTitle")}
          </h3>
          {loading && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <button
            type="button"
            className="btn"
            style={{ fontSize: 11, padding: "4px 10px" }}
            onClick={handleOpenDownload}
            data-testid="btn-open-node-download"
          >
            {t("cliTools.openNodeDownload")} ↗
          </button>
          <button
            type="button"
            className="btn"
            style={{ fontSize: 11, padding: "4px 10px" }}
            onClick={() => fetchStatus(true)}
            disabled={loading}
            data-testid="btn-refresh-node-prereq"
          >
            {t("antigravity.btnRefresh") || "Refresh"}
          </button>
        </div>
      </div>

      <p style={{ margin: "0 0 12px 0", fontSize: 12, color: "#656d76" }}>
        {t("cliTools.nodePrerequisitesDesc")}
      </p>

      {error && (
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
          data-testid="node-prereq-error"
        >
          {error}
        </div>
      )}

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
        {/* Node.js card */}
        <div
          style={{
            background: "#fff",
            border: "1px solid #e1e4e8",
            borderRadius: 6,
            padding: "10px 14px",
            display: "flex",
            flexDirection: "column",
            gap: 4,
          }}
          data-testid="node-status-box"
        >
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span style={{ fontSize: 13, fontWeight: 700, color: "#1f2937" }}>
              {t("cliTools.nodeStatus")}
            </span>
            {status?.node_installed ? (
              <span style={{ fontSize: 11, color: "#1a7f37", fontWeight: 600 }} data-testid="node-version-badge">
                ✓ v{status.node_version}
              </span>
            ) : (
              <span style={{ fontSize: 11, color: "#cf222e", fontWeight: 600 }} data-testid="node-not-installed-badge">
                ✗ {t("antigravity.cliToolNotInstalled")}
              </span>
            )}
          </div>
          {status?.node_major_version !== null && status?.node_major_version !== undefined && status.node_major_version < 20 && (
            <div style={{ fontSize: 10, color: "#9a6700", fontWeight: 500 }} data-testid="gemini-node-min-warning">
              ⚠ {t("cliTools.geminiNodeRequirement")} (found: v{status.node_version})
            </div>
          )}
          {status?.node_path && (
            <div style={{ fontFamily: "var(--font-mono)", fontSize: 10, color: "#57606a", wordBreak: "break-all" }}>
              {status.node_path}
            </div>
          )}
        </div>

        {/* npm card */}
        <div
          style={{
            background: "#fff",
            border: "1px solid #e1e4e8",
            borderRadius: 6,
            padding: "10px 14px",
            display: "flex",
            flexDirection: "column",
            gap: 4,
          }}
          data-testid="npm-status-box"
        >
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span style={{ fontSize: 13, fontWeight: 700, color: "#1f2937" }}>
              {t("cliTools.npmStatus")}
            </span>
            {status?.npm_installed ? (
              <span style={{ fontSize: 11, color: "#1a7f37", fontWeight: 600 }} data-testid="npm-version-badge">
                ✓ v{status.npm_version}
              </span>
            ) : (
              <span style={{ fontSize: 11, color: "#cf222e", fontWeight: 600 }} data-testid="npm-not-installed-badge">
                ✗ {t("antigravity.cliToolNotInstalled")}
              </span>
            )}
          </div>
          {status?.npm_path && (
            <div style={{ fontFamily: "var(--font-mono)", fontSize: 10, color: "#57606a", wordBreak: "break-all" }}>
              {status.npm_path}
            </div>
          )}
          {status?.npm_global_prefix && (
            <div style={{ fontFamily: "var(--font-mono)", fontSize: 10, color: "#57606a", wordBreak: "break-all" }} data-testid="npm-global-prefix">
              Prefix: {status.npm_global_prefix}
            </div>
          )}
        </div>
      </div>

      <div style={{ marginTop: 10, fontSize: 11, color: "#656d76" }}>
        ℹ {t("cliTools.nodeRestartNotice")}
      </div>
    </div>
  );
}

export function GeminiReviewerOptionCard() {
  const { t } = useTranslation();
  const [reviewSettings, setReviewSettings] = useState<CodexReviewSettings | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);

  const [confirmDialog, setConfirmDialog] = useState<{
    isOpen: boolean;
    snapshot: CodexReviewSettings | null;
    selectedNewPriority: string | null;
  }>({
    isOpen: false,
    snapshot: null,
    selectedNewPriority: null,
  });

  const fetchSettings = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const res = await invoke<CodexReviewSettings>("get_codex_review_settings");
      setReviewSettings(res);
      return res;
    } catch (e: unknown) {
      setError(String(e));
      return null;
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchSettings();
  }, [fetchSettings]);

  const handleToggleOption = async () => {
    if (loading) return;
    setError(null);
    setSuccessMsg(null);

    const current = await fetchSettings();
    if (!current) return;

    if (!current.gemini_cli_enabled) {
      try {
        setLoading(true);
        const payload: CodexReviewSettings = {
          ...current,
          gemini_cli_enabled: true,
        };
        const saved = await invoke<CodexReviewSettings>("save_codex_review_settings", { settings: payload });
        setReviewSettings(saved);
        setSuccessMsg(t("cliTools.geminiOptionSaved"));
        setTimeout(() => setSuccessMsg(null), 3000);
      } catch (e: unknown) {
        setError(String(e));
        setReviewSettings(current);
      } finally {
        setLoading(false);
      }
    } else {
      const isSelected = current.reviewers.gemini;
      const isPriority = current.priority_reviewer === "gemini";

      if (!isSelected && !isPriority) {
        try {
          setLoading(true);
          const payload: CodexReviewSettings = {
            ...current,
            gemini_cli_enabled: false,
          };
          const saved = await invoke<CodexReviewSettings>("save_codex_review_settings", { settings: payload });
          setReviewSettings(saved);
          setSuccessMsg(t("cliTools.geminiOptionSaved"));
          setTimeout(() => setSuccessMsg(null), 3000);
        } catch (e: unknown) {
          setError(String(e));
          setReviewSettings(current);
        } finally {
          setLoading(false);
        }
      } else {
        setConfirmDialog({
          isOpen: true,
          snapshot: current,
          selectedNewPriority: null,
        });
      }
    }
  };

  const handleCancelDialog = () => {
    setConfirmDialog({ isOpen: false, snapshot: null, selectedNewPriority: null });
  };

  const handleConfirmDisable = async () => {
    if (!confirmDialog.snapshot) return;
    try {
      setLoading(true);
      setError(null);

      // Re-fetch latest canonical settings immediately before saving (§12.27.3)
      const latest = await invoke<CodexReviewSettings>("get_codex_review_settings");

      const snapshot = confirmDialog.snapshot;
      const relevantChanged =
        latest.reviewers.gemini !== snapshot.reviewers.gemini ||
        latest.priority_reviewer !== snapshot.priority_reviewer ||
        latest.reviewers.codex !== snapshot.reviewers.codex ||
        latest.reviewers.deepseek !== snapshot.reviewers.deepseek;

      if (relevantChanged) {
        setConfirmDialog({
          isOpen: true,
          snapshot: latest,
          selectedNewPriority: null,
        });
        setLoading(false);
        return;
      }

      const remaining: ("codex" | "deepseek")[] = [];
      if (latest.reviewers.codex) remaining.push("codex");
      if (latest.reviewers.deepseek) remaining.push("deepseek");

      let newPriority = latest.priority_reviewer;
      if (latest.priority_reviewer === "gemini") {
        if (remaining.length === 1) {
          newPriority = remaining[0];
        } else if (remaining.length >= 2) {
          if (!confirmDialog.selectedNewPriority) {
            setLoading(false);
            return;
          }
          newPriority = confirmDialog.selectedNewPriority;
        } else {
          newPriority = null;
        }
      }

      const payload: CodexReviewSettings = {
        ...latest,
        gemini_cli_enabled: false,
        reviewers: {
          ...latest.reviewers,
          gemini: false,
        },
        priority_reviewer: newPriority,
      };

      const saved = await invoke<CodexReviewSettings>("save_codex_review_settings", { settings: payload });
      setReviewSettings(saved);
      setConfirmDialog({ isOpen: false, snapshot: null, selectedNewPriority: null });
      setSuccessMsg(t("cliTools.geminiOptionSaved"));
      setTimeout(() => setSuccessMsg(null), 3000);
    } catch (e: unknown) {
      setError(String(e));
      setConfirmDialog({ isOpen: false, snapshot: null, selectedNewPriority: null });
      void fetchSettings();
    } finally {
      setLoading(false);
    }
  };

  const isGeminiEnabled = reviewSettings?.gemini_cli_enabled ?? false;

  const snapshot = confirmDialog.snapshot;
  const isPriorityGemini = snapshot?.priority_reviewer === "gemini";
  const remainingReviewers: ("codex" | "deepseek")[] = [];
  if (snapshot?.reviewers.codex) remainingReviewers.push("codex");
  if (snapshot?.reviewers.deepseek) remainingReviewers.push("deepseek");

  const isConfirmDisabled =
    loading || (isPriorityGemini && remainingReviewers.length >= 2 && !confirmDialog.selectedNewPriority);

  return (
    <div className="settings-tile" data-testid="gemini-reviewer-option-card">
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6, flexWrap: "wrap", gap: 8 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
            {t("cliTools.geminiOptionTileTitle")}
          </h3>
          {loading && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {successMsg && (
            <span style={{ fontSize: 11, color: "#1a7f37", fontWeight: 600 }} data-testid="gemini-option-success-msg">
              ✓ {successMsg}
            </span>
          )}
          <button
            type="button"
            className="btn"
            style={{ fontSize: 11, padding: "4px 10px" }}
            onClick={() => void fetchSettings()}
            disabled={loading}
            data-testid="btn-refresh-gemini-option"
          >
            {t("antigravity.btnRefresh") || "Refresh"}
          </button>
        </div>
      </div>

      <p style={{ margin: "0 0 12px 0", fontSize: 12, color: "#656d76" }}>
        {t("cliTools.geminiOptionDesc")}
      </p>

      {error && (
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
          data-testid="gemini-option-error"
        >
          {error}
        </div>
      )}

      <div
        style={{
          background: "#fff",
          border: "1px solid #e1e4e8",
          borderRadius: 6,
          padding: "10px 14px",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          flexWrap: "wrap",
          gap: 12,
        }}
      >
        <label
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 8,
            cursor: loading ? "not-allowed" : "pointer",
            fontSize: 13,
            fontWeight: 600,
            color: "var(--text-primary, #1f2937)",
          }}
        >
          <input
            type="checkbox"
            checked={isGeminiEnabled}
            onChange={handleToggleOption}
            disabled={loading}
            data-testid="toggle-gemini-option-settings"
          />
          <span>{t("cliTools.geminiOptionToggle")}</span>
        </label>
        <span
          style={{
            fontSize: 11,
            fontWeight: 600,
            color: isGeminiEnabled ? "#1a7f37" : "#656d76",
            background: isGeminiEnabled ? "#dafbe1" : "#f6f8fa",
            border: `1px solid ${isGeminiEnabled ? "#4ac26b" : "#d0d7de"}`,
            borderRadius: 4,
            padding: "2px 8px",
          }}
          data-testid="gemini-option-badge"
        >
          {isGeminiEnabled ? "✓ Active" : "Disabled"}
        </span>
      </div>

      {/* Confirmation Modal when disabling Gemini while selected or priority (§12.27.3) */}
      {confirmDialog.isOpen && (
        <div
          style={{
            position: "fixed",
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            background: "rgba(0,0,0,0.5)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            zIndex: 9999,
          }}
          data-testid="gemini-disable-confirm-modal"
        >
          <div
            style={{
              background: "#fff",
              borderRadius: 8,
              padding: "16px 20px",
              maxWidth: 480,
              width: "90%",
              boxShadow: "0 8px 24px rgba(0,0,0,0.15)",
              display: "flex",
              flexDirection: "column",
              gap: 12,
            }}
          >
            <h4 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
              {t("cliTools.geminiDisableConfirmTitle")}
            </h4>

            <p style={{ margin: 0, fontSize: 12, color: "#57606a" }}>
              {t("cliTools.geminiDisableConfirmDesc")}
            </p>

            {isPriorityGemini && (
              <div
                style={{
                  background: "#fff8c5",
                  border: "1px solid #d4a72c",
                  borderRadius: 6,
                  padding: "10px 12px",
                  fontSize: 12,
                  color: "#6a5300",
                }}
                data-testid="gemini-disable-priority-resolution"
              >
                {remainingReviewers.length === 1 && (
                  <div>
                    {t("cliTools.geminiDisableSinglePriorityNote")}
                    <strong>
                      {remainingReviewers[0] === "codex" ? "Codex CLI" : "DeepSeek Harness"}
                    </strong>
                  </div>
                )}

                {remainingReviewers.length >= 2 && (
                  <div style={{ display: "flex", flexDirection: "column", gap: 6 }} data-testid="radio-group-new-priority">
                    <span>{t("cliTools.geminiDisableSelectPriorityPrompt")}</span>
                    {remainingReviewers.map((id) => (
                      <label
                        key={id}
                        style={{ display: "inline-flex", alignItems: "center", gap: 6, cursor: "pointer", fontSize: 12 }}
                      >
                        <input
                          type="radio"
                          name="new-priority-reviewer"
                          value={id}
                          checked={confirmDialog.selectedNewPriority === id}
                          onChange={() =>
                            setConfirmDialog((prev) => ({
                              ...prev,
                              selectedNewPriority: id,
                            }))
                          }
                          data-testid={`radio-priority-${id}`}
                        />
                        <span>{id === "codex" ? "Codex CLI" : "DeepSeek Harness"}</span>
                      </label>
                    ))}
                  </div>
                )}

                {remainingReviewers.length === 0 && (
                  <div data-testid="zero-reviewers-warning">
                    ⚠ {t("cliTools.geminiDisableZeroReviewersWarning")}
                  </div>
                )}
              </div>
            )}

            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 4 }}>
              <button
                type="button"
                className="btn"
                onClick={handleCancelDialog}
                disabled={loading}
                data-testid="btn-cancel-disable-gemini"
              >
                {t("cliTools.btnCancel")}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => void handleConfirmDisable()}
                disabled={isConfirmDisabled}
                data-testid="btn-confirm-disable-gemini"
              >
                {t("cliTools.btnConfirmDisable")}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default function CliToolsSettingsPanel() {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }} data-testid="cli-tools-settings-panel">
      <NodePrerequisitesCard />
      <CliToolsCard />
      <GeminiReviewerOptionCard />
    </div>
  );
}
