import React, { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type { ReviewerDiagnosticEvent } from "../types";

export interface ReviewerDiagnosticsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export default function ReviewerDiagnosticsModal({ isOpen, onClose }: ReviewerDiagnosticsModalProps) {
  const { t } = useTranslation();
  const [events, setEvents] = useState<ReviewerDiagnosticEvent[]>([]);
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copiedMessage, setCopiedMessage] = useState<string | null>(null);
  const [exportedText, setExportedText] = useState<string | null>(null);

  const fetchEvents = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const res = await invoke<ReviewerDiagnosticEvent[]>("get_reviewer_diagnostic_events", { limit: 50 });
      setEvents(res);
      if (res.length > 0 && !selectedRunId) {
        setSelectedRunId(res[0].run_id);
      }
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [selectedRunId]);

  useEffect(() => {
    if (isOpen) {
      void fetchEvents();
    }
  }, [isOpen, fetchEvents]);

  if (!isOpen) return null;

  const selectedEvent = events.find((e) => e.run_id === selectedRunId) || events[0] || null;

  const handleCopyDetails = async () => {
    if (!selectedEvent) return;
    try {
      await navigator.clipboard.writeText(JSON.stringify(selectedEvent, null, 2));
      setCopiedMessage(t("diagnostics.copiedSuccess") || "Copied to clipboard!");
      setTimeout(() => setCopiedMessage(null), 2500);
    } catch (e: unknown) {
      setError(`Clipboard error: ${String(e)}`);
    }
  };

  const handleExport = async () => {
    try {
      setLoading(true);
      const report = await invoke<string>("export_reviewer_diagnostics_report", {
        runId: selectedRunId,
      });
      setExportedText(report);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div
      className="modal-overlay"
      style={{
        position: "fixed",
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        backgroundColor: "rgba(0,0,0,0.5)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
      }}
      data-testid="reviewer-diagnostics-modal"
    >
      <div
        className="modal-container"
        style={{
          background: "var(--bg-primary, #ffffff)",
          color: "var(--text-primary, #1f2937)",
          borderRadius: 8,
          width: "90%",
          maxWidth: 900,
          maxHeight: "85vh",
          display: "flex",
          flexDirection: "column",
          boxShadow: "0 10px 25px rgba(0,0,0,0.2)",
          overflow: "hidden",
        }}
      >
        {/* Header */}
        <div
          style={{
            padding: "12px 16px",
            borderBottom: "1px solid var(--border-color, #e1e4e8)",
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            background: "var(--bg-secondary, #f6f8fa)",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span style={{ fontWeight: 700, fontSize: 14 }}>
              {t("diagnostics.title") || "External Reviewer Diagnostic Logs (§18)"}
            </span>
            {loading && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
          </div>
          <button
            type="button"
            className="btn"
            style={{ padding: "2px 8px", fontSize: 14 }}
            onClick={onClose}
            data-testid="btn-close-diagnostics-modal"
          >
            ✕
          </button>
        </div>

        {/* Content Body */}
        <div style={{ display: "flex", flex: 1, overflow: "hidden", minHeight: 380 }}>
          {/* Left pane: Event Run list */}
          <div
            style={{
              width: 320,
              borderRight: "1px solid var(--border-color, #e1e4e8)",
              overflowY: "auto",
              padding: 8,
              background: "var(--bg-secondary, #fafbfc)",
            }}
            data-testid="diagnostics-runs-list"
          >
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6 }}>
              <span style={{ fontSize: 11, fontWeight: 600, color: "#57606a" }}>
                {t("diagnostics.recentRuns") || "Recent Review Runs"} ({events.length})
              </span>
              <button
                type="button"
                className="btn"
                style={{ fontSize: 10, padding: "2px 6px" }}
                onClick={() => void fetchEvents()}
                disabled={loading}
                data-testid="btn-reload-diagnostics"
              >
                {t("codexSkillsRules.btnReload") || "Reload"}
              </button>
            </div>

            {events.length === 0 ? (
              <div style={{ padding: 16, fontSize: 12, color: "#8c959f", textAlign: "center" }}>
                {t("diagnostics.noEvents") || "No reviewer diagnostic events recorded yet."}
              </div>
            ) : (
              events.map((ev) => {
                const isSelected = selectedRunId === ev.run_id;
                const isUnavailable = ev.verdict === "UNAVAILABLE" || !ev.sandbox_verified;
                return (
                  <div
                    key={ev.run_id}
                    onClick={() => {
                      setSelectedRunId(ev.run_id);
                      setExportedText(null);
                    }}
                    style={{
                      padding: "8px 10px",
                      borderRadius: 4,
                      marginBottom: 4,
                      cursor: "pointer",
                      background: isSelected ? "#eaf4ff" : "#ffffff",
                      border: isSelected ? "1px solid #8dbbff" : "1px solid #e1e4e8",
                      fontSize: 11,
                    }}
                    data-testid={`diagnostic-item-${ev.run_id}`}
                  >
                    <div style={{ display: "flex", justifyContent: "space-between", fontWeight: 600 }}>
                      <span style={{ textTransform: "capitalize" }}>{ev.reviewer}</span>
                      <span
                        style={{
                          color: isUnavailable ? "#cf222e" : "#1a7f37",
                          fontSize: 10,
                          padding: "1px 4px",
                          borderRadius: 3,
                          background: isUnavailable ? "#ffebe9" : "#dafbe1",
                        }}
                      >
                        {ev.verdict || (ev.sandbox_verified ? "VERIFIED" : "UNAVAILABLE")}
                      </span>
                    </div>
                    <div style={{ color: "#57606a", marginTop: 2, fontSize: 10 }}>
                      {ev.timestamp.replace("T", " ").substring(0, 19)}
                    </div>
                    <div style={{ color: "#8c959f", fontSize: 9, marginTop: 1 }}>
                      ID: {ev.run_id.substring(0, 8)}...
                    </div>
                  </div>
                );
              })
            )}
          </div>

          {/* Right pane: Event Detail */}
          <div style={{ flex: 1, padding: 16, overflowY: "auto", display: "flex", flexDirection: "column", gap: 12 }}>
            {error && (
              <div
                style={{
                  padding: "6px 10px",
                  background: "#ffebe9",
                  border: "1px solid #ff8182",
                  borderRadius: 4,
                  fontSize: 11,
                  color: "#cf222e",
                }}
              >
                {error}
              </div>
            )}

            {selectedEvent ? (
              <>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
                  <div>
                    <h4 style={{ margin: "0 0 4px 0", fontSize: 13 }}>
                      {selectedEvent.reviewer.toUpperCase()} — {selectedEvent.model || "Default Model"}
                    </h4>
                    <span style={{ fontSize: 11, color: "#57606a" }}>
                      Run ID: <code>{selectedEvent.run_id}</code>
                    </span>
                  </div>
                  <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                    {copiedMessage && (
                      <span style={{ fontSize: 11, color: "#1a7f37", fontWeight: 500 }}>{copiedMessage}</span>
                    )}
                    <button
                      type="button"
                      className="btn"
                      style={{ fontSize: 11, padding: "3px 8px" }}
                      onClick={() => void handleCopyDetails()}
                      data-testid="btn-copy-diagnostic-detail"
                    >
                      {t("diagnostics.copyJson") || "Copy JSON"}
                    </button>
                    <button
                      type="button"
                      className="btn btn-primary"
                      style={{ fontSize: 11, padding: "3px 10px" }}
                      onClick={() => void handleExport()}
                      data-testid="btn-export-diagnostic-report"
                    >
                      {t("diagnostics.exportReport") || "Export Report"}
                    </button>
                  </div>
                </div>

                {/* Structured attributes grid */}
                <div
                  style={{
                    display: "grid",
                    gridTemplateColumns: "1fr 1fr",
                    gap: 8,
                    background: "#f6f8fa",
                    padding: 10,
                    borderRadius: 6,
                    fontSize: 11,
                  }}
                  data-testid="diagnostic-attributes-grid"
                >
                  <div>
                    <strong>Phase:</strong> {selectedEvent.execution_phase}
                  </div>
                  <div>
                    <strong>Verdict:</strong> {selectedEvent.verdict || "N/A"}
                  </div>
                  <div>
                    <strong>Sandbox Mechanism:</strong> {selectedEvent.sandbox_mechanism}
                  </div>
                  <div>
                    <strong>Sandbox Verified:</strong>{" "}
                    <span style={{ color: selectedEvent.sandbox_verified ? "#1a7f37" : "#cf222e", fontWeight: 600 }}>
                      {selectedEvent.sandbox_verified ? "YES" : "NO (Fail-Closed UNAVAILABLE)"}
                    </span>
                  </div>
                  <div>
                    <strong>External Dispatch Occurred:</strong>{" "}
                    {selectedEvent.external_dispatch_occurred ? "YES" : "NO"}
                  </div>
                  <div>
                    <strong>Process Spawned:</strong> {selectedEvent.process_spawned ? "YES" : "NO"}
                  </div>
                  <div>
                    <strong>Children Reaped:</strong> {selectedEvent.children_reaped ? "YES" : "NO"}
                  </div>
                  <div>
                    <strong>Error Category:</strong> {selectedEvent.error_category || "None"}
                  </div>
                  <div>
                    <strong>Snapshot Files:</strong> {selectedEvent.snapshot_files_count} ({selectedEvent.snapshot_total_bytes} bytes)
                  </div>
                  <div>
                    <strong>Duration:</strong> {selectedEvent.duration_ms} ms
                  </div>
                </div>

                {/* Exported preview if clicked */}
                {exportedText && (
                  <div style={{ marginTop: 8 }}>
                    <div style={{ fontSize: 11, fontWeight: 600, marginBottom: 4 }}>
                      {t("diagnostics.exportPreview") || "Redacted Export Preview:"}
                    </div>
                    <textarea
                      readOnly
                      value={exportedText}
                      style={{
                        width: "100%",
                        height: 140,
                        fontSize: 10,
                        fontFamily: "monospace",
                        background: "#ffffff",
                        border: "1px solid #d0d7de",
                        borderRadius: 4,
                        padding: 6,
                      }}
                      data-testid="diagnostic-export-textarea"
                    />
                  </div>
                )}
              </>
            ) : (
              <div style={{ padding: 24, textAlign: "center", color: "#8c959f", fontSize: 12 }}>
                {t("diagnostics.selectRun") || "Select a run to view details"}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
