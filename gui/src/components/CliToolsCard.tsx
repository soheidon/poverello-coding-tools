import React, { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type { CliToolInfo, CliAuthInfo } from "../types";

export default function CliToolsCard() {
  const { t } = useTranslation();

  const [tools, setTools] = useState<CliToolInfo[]>([]);
  const [authStatus, setAuthStatus] = useState<CliAuthInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [checkingUpdates, setCheckingUpdates] = useState(false);
  const [recheckingAuth, setRecheckingAuth] = useState(false);
  const [actionLoadingTool, setActionLoadingTool] = useState<string | null>(null);
  const [launchingAuth, setLaunchingAuth] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  // Confirmation modal state for install/update
  const [confirmTool, setConfirmTool] = useState<{
    name: string;
    display_name: string;
    package_name: string;
    is_preview: boolean;
    is_update: boolean;
    latest_version: string | null;
    check_id: string | null;
  } | null>(null);

  // Confirmation modal state for launching auth / console
  const [confirmAuthTool, setConfirmAuthTool] = useState<{
    name: string;
    display_name: string;
  } | null>(null);

  const fetchTools = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const res = await invoke<CliToolInfo[]>("get_cli_tools_status");
      setTools(res ?? []);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
    try {
      const auth = await invoke<CliAuthInfo[]>("get_cli_auth_status");
      setAuthStatus(auth ?? []);
    } catch {
      // ignore
    }
  }, []);

  const handleCheckUpdates = async () => {
    try {
      setCheckingUpdates(true);
      setError(null);
      const res = await invoke<CliToolInfo[]>("check_cli_tools_latest_version");
      setTools(res ?? []);
      const failedChecks = (res ?? []).filter((t) => t.latest_check_error);
      if (failedChecks.length > 0) {
        setError(
          `${failedChecks.map((t) => t.display_name).join(", ")}: ${t("cliTools.latestCheckFailed") || "最新版の確認に失敗しました"}`
        );
      } else {
        setSuccessMessage(t("antigravity.cliToolsCheckedSuccess") || "Latest versions checked successfully");
        setTimeout(() => setSuccessMessage(null), 3500);
      }
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setCheckingUpdates(false);
    }
  };

  const handleRecoverOutcomeUnknown = async (name: string) => {
    try {
      setActionLoadingTool(name);
      setError(null);
      const updated = await invoke<CliToolInfo>("recover_cli_tool_state", { name });
      await fetchTools();
      setSuccessMessage(`${updated.display_name}: 状態を再確認しました`);
      setTimeout(() => setSuccessMessage(null), 3500);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setActionLoadingTool(null);
    }
  };

  const handleRecheckAuth = async () => {
    if (recheckingAuth) return;
    try {
      setRecheckingAuth(true);
      setError(null);
      const auth = await invoke<CliAuthInfo[]>("get_cli_auth_status", { refresh: true });
      setAuthStatus(auth ?? []);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setRecheckingAuth(false);
    }
  };

  useEffect(() => {
    void fetchTools();
  }, [fetchTools]);

  const handleActionClick = (tool: CliToolInfo) => {
    setConfirmTool({
      name: tool.name,
      display_name: tool.display_name,
      package_name: tool.package_name,
      is_preview: tool.is_preview,
      is_update: tool.installed,
      latest_version: tool.latest_version,
      check_id: tool.check_id,
    });
  };

  const handleConfirmAction = async () => {
    if (!confirmTool) return;
    const { name, display_name, is_update, check_id } = confirmTool;
    setConfirmTool(null);
    try {
      setActionLoadingTool(name);
      setError(null);
      if (is_update) {
        if (!check_id) {
          throw new Error("Update requires a valid update check. Please check for updates first.");
        }
        await invoke<CliToolInfo>("update_cli_tool", { name, checkId: check_id });
      } else {
        await invoke<CliToolInfo>("install_cli_tool", { name });
      }
      await fetchTools();
      const msg = is_update
        ? `${display_name} updated successfully`
        : `${display_name} installed successfully`;
      setSuccessMessage(msg);
      setTimeout(() => setSuccessMessage(null), 3500);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setActionLoadingTool(null);
    }
  };

  const handleConfirmLaunchAuth = async () => {
    if (!confirmAuthTool) return;
    const { name, display_name } = confirmAuthTool;
    setConfirmAuthTool(null);
    try {
      setLaunchingAuth(name);
      setError(null);
      await invoke("launch_cli_auth", { toolName: name });
      setSuccessMessage(`${display_name} console opened`);
      setTimeout(() => setSuccessMessage(null), 3500);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLaunchingAuth(null);
    }
  };

  return (
    <div className="settings-tile" data-testid="cli-tools-card">
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
            {t("antigravity.cliToolsCardTitle")}
          </h3>
          {(loading || checkingUpdates) && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {successMessage && (
            <span style={{ fontSize: 12, color: "#1a7f37", fontWeight: 500 }} data-testid="cli-success-msg">
              ✓ {successMessage}
            </span>
          )}
          <button
            type="button"
            className="btn"
            style={{ fontSize: 11, padding: "4px 10px" }}
            onClick={handleCheckUpdates}
            disabled={loading || checkingUpdates || actionLoadingTool !== null}
            data-testid="btn-check-latest"
          >
            {checkingUpdates ? "..." : t("antigravity.cliToolCheckLatestBtn") || "Check for Updates"}
          </button>
          <button
            type="button"
            className="btn"
            style={{ fontSize: 11, padding: "4px 10px" }}
            onClick={fetchTools}
            disabled={loading || checkingUpdates || actionLoadingTool !== null}
          >
            {t("antigravity.btnRefresh") || "Refresh"}
          </button>
        </div>
      </div>

      <p style={{ margin: "0 0 12px 0", fontSize: 12, color: "#656d76" }}>
        {t("antigravity.cliToolsCardDesc")}
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
          data-testid="cli-tools-error"
        >
          {error}
        </div>
      )}

      {/* Tool items list */}
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        {(tools ?? []).map((tool) => {
          const isBusy = actionLoadingTool === tool.name || launchingAuth === tool.name;
          const isDisabled = isBusy || actionLoadingTool !== null || loading || checkingUpdates || !!tool.prerequisite_error || !!tool.outcome_unknown;
          const auth = authStatus.find((a) => a.name === tool.name);
          const toolStatus = tool.status ?? (tool.outcome_unknown ? "unknown" : tool.execution_error ? "execution_error" : tool.installed ? "installed" : "not_installed");

          return (
            <div
              key={tool.name}
              data-testid={`cli-tool-item-${tool.name}`}
              style={{
                background: "#fff",
                border: "1px solid #e1e4e8",
                borderRadius: 6,
                padding: "10px 14px",
                display: "flex",
                justifyContent: "space-between",
                alignItems: "flex-start",
                flexWrap: "wrap",
                gap: 12,
              }}
            >
              <div style={{ display: "flex", flexDirection: "column", gap: 4, flex: 1, minWidth: 260 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                  <span style={{ fontSize: 13, fontWeight: 700, color: "#1f2937" }}>
                    {tool.display_name}
                  </span>
                  {tool.name === "gemini" && (
                    <span
                      style={{
                        fontSize: 10,
                        fontWeight: 500,
                        background: "#f6f8fa",
                        color: "#57606a",
                        padding: "1px 6px",
                        borderRadius: 4,
                        border: "1px solid #d0d7de",
                      }}
                      data-testid="optional-tag-gemini"
                    >
                      {t("cliTools.optionalTag") || "Optional"}
                    </span>
                  )}
                  <div>
                    {toolStatus === "installed" ? (
                      <span
                        style={{
                          display: "inline-flex",
                          alignItems: "center",
                          gap: 3,
                          fontSize: 11,
                          color: "#1a7f37",
                          fontWeight: 600,
                        }}
                        data-testid={`install-status-${tool.name}`}
                      >
                        <span>✓</span> {t("antigravity.cliToolInstalled")} ({tool.installed_version ?? (t("cliTools.authUnknown") || "version unknown")})
                      </span>
                    ) : toolStatus === "execution_error" ? (
                      <span
                        style={{
                          display: "inline-flex",
                          alignItems: "center",
                          gap: 3,
                          fontSize: 11,
                          color: "#cf222e",
                          fontWeight: 600,
                        }}
                        data-testid={`install-status-${tool.name}`}
                      >
                        <span>✗</span> {t("cliTools.executionErrorBadge") || "実行エラー"}
                      </span>
                    ) : toolStatus === "unknown" ? (
                      <span
                        style={{
                          display: "inline-flex",
                          alignItems: "center",
                          gap: 3,
                          fontSize: 11,
                          color: "#9a6700",
                          fontWeight: 600,
                        }}
                        data-testid={`install-status-${tool.name}`}
                      >
                        <span>?</span> {t("cliTools.statusUnknownBadge") || "状態不明"}
                      </span>
                    ) : (
                      <span style={{ fontSize: 11, color: "#656d76", fontWeight: 500 }} data-testid={`install-status-${tool.name}`}>
                        {t("antigravity.cliToolNotInstalled")}
                      </span>
                    )}
                  </div>
                  {tool.latest_version && (
                    <div>
                      {tool.update_available ? (
                        <span
                          style={{
                            fontSize: 10,
                            fontWeight: 600,
                            background: "#ddf4ff",
                            color: "#0969da",
                            padding: "1px 6px",
                            borderRadius: 4,
                            border: "1px solid #54aeff",
                          }}
                          data-testid={`update-badge-${tool.name}`}
                        >
                          ↑ {t("antigravity.cliToolUpdateAvailable") || "Update available"}: {tool.latest_version}
                        </span>
                      ) : tool.installed ? (
                        <span
                          style={{
                            fontSize: 10,
                            color: "#1a7f37",
                            fontWeight: 500,
                          }}
                        >
                          {t("antigravity.cliToolUpToDate") || "Up to date"}
                        </span>
                      ) : null}
                    </div>
                  )}
                  {tool.latest_check_error && (
                    <span
                      style={{
                        fontSize: 10,
                        fontWeight: 500,
                        background: "#fff8c5",
                        color: "#9a6700",
                        padding: "1px 6px",
                        borderRadius: 4,
                        border: "1px solid #d4a72c",
                      }}
                      data-testid={`latest-check-error-${tool.name}`}
                    >
                      ⚠ {t("cliTools.latestCheckFailed") || "最新版の確認に失敗"}: {tool.latest_check_error}
                    </span>
                  )}
                </div>

                {/* Auth status block */}
                {auth && (
                  <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap", marginTop: 2 }}>
                    <span style={{ fontSize: 11, fontWeight: 600, color: "#57606a" }}>
                      {t("cliTools.authStatus")}:
                    </span>
                    <span
                      style={{
                        fontSize: 10,
                        padding: "1px 6px",
                        borderRadius: 4,
                        fontWeight: 600,
                        background:
                          auth.status === "authenticated"
                            ? "#dafbe1"
                            : auth.status === "not_authenticated"
                            ? "#ffebe9"
                            : "#f6f8fa",
                        color:
                          auth.status === "authenticated"
                            ? "#1a7f37"
                            : auth.status === "not_authenticated"
                            ? "#cf222e"
                            : "#57606a",
                        border: `1px solid ${
                          auth.status === "authenticated"
                            ? "#aceebb"
                            : auth.status === "not_authenticated"
                            ? "#ff8182"
                            : "#d0d7de"
                        }`,
                      }}
                      data-testid={`auth-badge-${tool.name}`}
                    >
                      {auth.status === "authenticated"
                        ? `✓ ${t("cliTools.authAuthenticated")}${auth.auth_type ? ` (${auth.auth_type})` : ""}`
                        : auth.status === "not_authenticated"
                        ? `✗ ${t("cliTools.authNotAuthenticated")}`
                        : `? ${t("cliTools.authUnknown")}`}
                    </span>
                    {auth.can_launch_login && (
                      <button
                        type="button"
                        className="btn"
                        style={{ fontSize: 10, padding: "2px 8px" }}
                        onClick={() => setConfirmAuthTool({ name: tool.name, display_name: tool.display_name })}
                        disabled={loading || actionLoadingTool !== null || launchingAuth !== null || recheckingAuth}
                        data-testid={`btn-launch-auth-${tool.name}`}
                      >
                        {t("cliTools.btnLaunchAuth")}
                      </button>
                    )}
                    {tool.name === "codex" && (
                      <button
                        type="button"
                        className="btn"
                        style={{ fontSize: 10, padding: "2px 8px" }}
                        onClick={handleRecheckAuth}
                        disabled={loading || actionLoadingTool !== null || launchingAuth !== null || recheckingAuth}
                        data-testid="btn-recheck-auth-codex"
                      >
                        {recheckingAuth ? "..." : (t("cliTools.btnRecheckAuth") || "再確認")}
                      </button>
                    )}
                  </div>
                )}
                {/* Codex-specific identity and home info */}
                {tool.name === "codex" && auth && (
                  <div style={{ display: "flex", flexDirection: "column", gap: 3, marginTop: 4 }}>
                    {auth.status === "authenticated" && (
                      <div style={{ fontSize: 11, color: "#24292f" }} data-testid="codex-account-identity">
                        <strong>{t("cliTools.accountIdentity")}:</strong>{" "}
                        {auth.account_identity ?? t("cliTools.identityUnknown")}
                      </div>
                    )}
                    {auth.codex_home && (
                      <div style={{ fontSize: 10, color: "#57606a", wordBreak: "break-all" }} data-testid="codex-effective-home">
                        <strong>{t("cliTools.effectiveCodexHome")}:</strong>{" "}
                        <code>{auth.codex_home}</code>
                      </div>
                    )}
                    {auth.last_checked_at && (
                      <div style={{ fontSize: 10, color: "#8c959f" }} data-testid="codex-last-checked">
                        {t("cliTools.lastCheckedAt")}: {new Date(auth.last_checked_at).toLocaleTimeString()}
                      </div>
                    )}
                  </div>
                )}
                {auth?.message && (
                  <div style={{ fontSize: 10, color: "#656d76", fontStyle: "italic" }} data-testid={`auth-message-${tool.name}`}>
                    {auth.message}
                  </div>
                )}

                {tool.name === "gemini" && (
                  <div
                    style={{
                      fontSize: 10,
                      color: "#9a6700",
                      background: "#fff8c5",
                      border: "1px solid #d4a72c",
                      borderRadius: 4,
                      padding: "4px 8px",
                      marginTop: 2,
                    }}
                    data-testid="gemini-sandbox-warning"
                  >
                    ⚠ {t("cliTools.geminiWindowsSandboxWarning")}
                  </div>
                )}

                {tool.prerequisite_error && (
                  <div
                    style={{ fontSize: 11, color: "#cf222e", fontWeight: 500 }}
                    data-testid={`prereq-error-${tool.name}`}
                  >
                    ⚠ {tool.prerequisite_error}
                  </div>
                )}

                {tool.execution_error && (
                  <div
                    style={{
                      fontSize: 11,
                      color: "#cf222e",
                      background: "#ffebe9",
                      border: "1px solid #ff8182",
                      borderRadius: 4,
                      padding: "4px 8px",
                      marginTop: 2,
                    }}
                    data-testid={`execution-error-${tool.name}`}
                  >
                    ⚠ {t("cliTools.executionError") || "実行エラー"}: {tool.execution_error}
                  </div>
                )}

                {tool.outcome_unknown && (
                  <div
                    style={{
                      fontSize: 11,
                      color: "#cf222e",
                      background: "#ffebe9",
                      border: "1px solid #ff8182",
                      borderRadius: 4,
                      padding: "6px 10px",
                      marginTop: 4,
                      display: "flex",
                      justifyContent: "space-between",
                      alignItems: "center",
                      gap: 8,
                      flexWrap: "wrap",
                    }}
                    data-testid={`outcome-unknown-box-${tool.name}`}
                  >
                    <div>
                      <strong>⚠ {t("cliTools.outcomeUnknownTitle") || "未確定状態 (OUTCOME_UNKNOWN)"}:</strong>{" "}
                      {tool.outcome_unknown}
                    </div>
                    <button
                      type="button"
                      className="btn"
                      style={{ fontSize: 10, padding: "2px 8px" }}
                      onClick={() => handleRecoverOutcomeUnknown(tool.name)}
                      disabled={loading || actionLoadingTool !== null}
                      data-testid={`btn-recover-outcome-unknown-${tool.name}`}
                    >
                      {t("cliTools.btnSafeRecheck") || "安全に再確認"}
                    </button>
                  </div>
                )}

                {tool.path && (
                  <div style={{ fontFamily: "var(--font-mono)", fontSize: 10, color: "#57606a", wordBreak: "break-all" }}>
                    {tool.path}
                  </div>
                )}
                <div style={{ fontSize: 10, color: "#8c959f" }}>
                  npm: <code>{tool.package_name}</code>
                </div>
              </div>

              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                {!tool.installed ? (
                  <button
                    type="button"
                    className="btn btn-primary"
                    style={{ fontSize: 11, padding: "4px 10px" }}
                    data-testid={`btn-install-${tool.name}`}
                    onClick={() => handleActionClick(tool)}
                    disabled={isDisabled}
                    title={tool.prerequisite_error ?? undefined}
                  >
                    {isBusy ? "..." : t("antigravity.cliToolInstallBtn")}
                  </button>
                ) : tool.update_available ? (
                  <button
                    type="button"
                    className="btn btn-primary"
                    style={{ fontSize: 11, padding: "4px 10px" }}
                    data-testid={`btn-update-${tool.name}`}
                    onClick={() => handleActionClick(tool)}
                    disabled={isDisabled}
                    title={tool.prerequisite_error ?? undefined}
                  >
                    {isBusy ? "..." : t("antigravity.cliToolUpdateBtn")}
                  </button>
                ) : null}
              </div>
            </div>
          );
        })}
      </div>

      {/* Confirmation Modal */}
      {confirmTool && (
        <div
          data-testid="cli-confirm-modal"
          style={{
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
          }}
        >
          <div
            style={{
              background: "#ffffff",
              borderRadius: 8,
              padding: "20px 24px",
              maxWidth: 440,
              width: "90%",
              boxShadow: "0 8px 24px rgba(0, 0, 0, 0.2)",
            }}
          >
            <h4 style={{ margin: "0 0 10px 0", fontSize: 15, fontWeight: 700, color: "#24292f" }}>
              {confirmTool.is_update
                ? t("antigravity.cliToolUpdateConfirmTitle") || `Update ${confirmTool.display_name}`
                : t("antigravity.cliToolInstallConfirmTitle") || `Install ${confirmTool.display_name}`}
            </h4>
            <p style={{ margin: "0 0 12px 0", fontSize: 13, color: "#57606a", lineHeight: 1.5 }}>
              {confirmTool.is_update
                ? t("antigravity.cliToolUpdateConfirm")
                : t("antigravity.cliToolInstallConfirm")}
            </p>
            <div style={{ marginBottom: 14, fontSize: 12, color: "#24292f" }}>
              <strong>Package:</strong>{" "}
              <code>
                {confirmTool.package_name}@{confirmTool.latest_version ?? "latest"}
              </code>
            </div>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 10 }}>
              <button
                type="button"
                className="btn"
                style={{ fontSize: 12, padding: "6px 12px" }}
                data-testid="cli-confirm-cancel"
                onClick={() => setConfirmTool(null)}
              >
                {t("antigravity.btnCancel") || "Cancel"}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                style={{ fontSize: 12, padding: "6px 14px" }}
                data-testid="cli-confirm-proceed"
                onClick={handleConfirmAction}
              >
                {t("antigravity.btnConfirm") || "Proceed"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Auth Launch Confirmation Modal */}
      {confirmAuthTool && (
        <div
          data-testid="cli-auth-modal"
          style={{
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
          }}
        >
          <div
            style={{
              background: "#ffffff",
              borderRadius: 8,
              padding: "20px 24px",
              maxWidth: 440,
              width: "90%",
              boxShadow: "0 8px 24px rgba(0, 0, 0, 0.2)",
            }}
          >
            <h4 style={{ margin: "0 0 10px 0", fontSize: 15, fontWeight: 700, color: "#24292f" }}>
              {t("cliTools.launchModalTitle")}: {confirmAuthTool.display_name}
            </h4>
            <p style={{ margin: "0 0 14px 0", fontSize: 13, color: "#57606a", lineHeight: 1.5 }}>
              {confirmAuthTool.name === "codex"
                ? t("cliTools.launchModalCodexDesc")
                : confirmAuthTool.name === "gemini"
                ? t("cliTools.launchModalGeminiDesc")
                : t("cliTools.launchModalDshDesc")}
            </p>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 10 }}>
              <button
                type="button"
                className="btn"
                style={{ fontSize: 12, padding: "6px 12px" }}
                data-testid="cli-auth-cancel"
                onClick={() => setConfirmAuthTool(null)}
              >
                {t("antigravity.btnCancel") || "Cancel"}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                style={{ fontSize: 12, padding: "6px 14px" }}
                data-testid="cli-auth-proceed"
                onClick={handleConfirmLaunchAuth}
              >
                {t("cliTools.launchModalProceed") || "Launch Console"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
