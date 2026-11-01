import React, { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type { CliToolInfo } from "../types";

export default function CliToolsCard() {
  const { t } = useTranslation();

  const [tools, setTools] = useState<CliToolInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [actionLoadingTool, setActionLoadingTool] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  // Confirmation modal state
  const [confirmTool, setConfirmTool] = useState<{
    name: string;
    display_name: string;
    package_name: string;
    is_preview: boolean;
    is_update: boolean;
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
  }, []);

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
    });
  };

  const handleConfirmAction = async () => {
    if (!confirmTool) return;
    const { name, display_name, is_update } = confirmTool;
    setConfirmTool(null);
    try {
      setActionLoadingTool(name);
      setError(null);
      await invoke<CliToolInfo>("install_cli_tool", { name });
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
          {loading && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {successMessage && (
            <span style={{ fontSize: 12, color: "#1a7f37", fontWeight: 500 }} data-testid="cli-success-msg">
              ✓ {successMessage}
            </span>
          )}
          <button
            className="btn"
            style={{ fontSize: 11, padding: "4px 10px" }}
            onClick={fetchTools}
            disabled={loading || actionLoadingTool !== null}
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
          const isBusy = actionLoadingTool === tool.name;

          return (
            <div
              key={tool.name}
              data-testid={`cli-tool-item-${tool.name}`}
              style={{
                background: "#fff",
                border: "1px solid #e1e4e8",
                borderRadius: 6,
                padding: "8px 12px",
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                flexWrap: "wrap",
                gap: 8,
              }}
            >
              <div style={{ display: "flex", flexDirection: "column", gap: 3 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                  <span style={{ fontSize: 13, fontWeight: 700, color: "#1f2937" }}>
                    {tool.display_name}
                  </span>
                  {tool.is_preview && (
                    <span
                      style={{
                        fontSize: 10,
                        fontWeight: 600,
                        background: "#fff8c5",
                        color: "#9a6700",
                        padding: "1px 6px",
                        borderRadius: 4,
                        border: "1px solid #d4a72c",
                      }}
                      data-testid="preview-badge"
                    >
                      {t("antigravity.cliToolPreviewNotice")}
                    </span>
                  )}
                  <div>
                    {tool.installed ? (
                      <span
                        style={{
                          display: "inline-flex",
                          alignItems: "center",
                          gap: 3,
                          fontSize: 11,
                          color: "#1a7f37",
                          fontWeight: 600,
                        }}
                      >
                        <span>✓</span> {t("antigravity.cliToolInstalled")} ({tool.installed_version ?? "unknown"})
                      </span>
                    ) : (
                      <span style={{ fontSize: 11, color: "#656d76", fontWeight: 500 }}>
                        {t("antigravity.cliToolNotInstalled")}
                      </span>
                    )}
                  </div>
                </div>

                {tool.path && (
                  <div style={{ fontFamily: "var(--font-mono)", fontSize: 10, color: "#57606a" }}>
                    {tool.path}
                  </div>
                )}
                <div style={{ fontSize: 10, color: "#8c959f" }}>
                  npm: <code>{tool.package_name}</code>
                </div>
              </div>

              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <button
                  type="button"
                  className="btn btn-primary"
                  style={{ fontSize: 11, padding: "4px 10px" }}
                  data-testid={`btn-install-${tool.name}`}
                  onClick={() => handleActionClick(tool)}
                  disabled={isBusy || actionLoadingTool !== null || loading}
                >
                  {isBusy
                    ? "..."
                    : tool.installed
                    ? t("antigravity.cliToolUpdateBtn")
                    : t("antigravity.cliToolInstallBtn")}
                </button>
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
              <strong>Package:</strong> <code>{confirmTool.package_name}@latest</code>
              {confirmTool.is_preview && (
                <div style={{ marginTop: 6, color: "#9a6700", fontWeight: 500 }}>
                  ⚠ {t("antigravity.cliToolPreviewNotice")}
                </div>
              )}
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
    </div>
  );
}
