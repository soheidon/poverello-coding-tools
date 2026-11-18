import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type { TranslationKey } from "../i18n";
import { getAgentOptErrorPresentation, unregisteredWorkspaceErrorMatchesPath } from "./agentOptErrorPresentation";
import AgentOptConfigEditor from "./AgentOptConfigEditor";
import type {
  AgentOptWorkspaceInspection,
  AgentOptWorkspaceState,
  NerfEngineOverview,
} from "../types";

interface AgentOptPanelProps {
  overview: NerfEngineOverview | null;
  busy: boolean;
  onRefreshOverview: () => Promise<void>;
  onWorkspacePrepared?: () => Promise<boolean>;
  onError: (msg: string | null) => void;
  onSuccess: (msg: string | null) => void;
  onOpenAgentOptSettings?: () => void;
}

const STATE_STYLE: Record<AgentOptWorkspaceState, { bg: string; color: string; border: string }> = {
  not_installed: { bg: "#f6f8fa", color: "#57606a", border: "#d0d7de" },
  not_initialized: { bg: "#ddf4ff", color: "#0969da", border: "#54aeff" },
  disabled: { bg: "#fff8c5", color: "#6e4a00", border: "#d4a72c" },
  enabled: { bg: "#dafbe1", color: "#1a7f37", border: "#4ac26b" },
  invalid_config: { bg: "#ffebe9", color: "#cf222e", border: "#ff8182" },
  unsupported_version: { bg: "#ffebe9", color: "#cf222e", border: "#ff8182" },
  error: { bg: "#ffebe9", color: "#cf222e", border: "#ff8182" },
};

function displayPath(path: string): string {
  if (path.startsWith("\\\\?\\UNC\\")) return `\\\\${path.slice(8)}`;
  if (path.startsWith("\\\\?\\")) return path.slice(4);
  return path;
}

export default function AgentOptPanel({
  overview,
  busy,
  onRefreshOverview,
  onWorkspacePrepared,
  onError,
  onSuccess,
  onOpenAgentOptSettings,
}: AgentOptPanelProps) {
  const { t } = useTranslation();
  const [actionBusy, setActionBusy] = useState(false);
  const [initWorkspace, setInitWorkspace] = useState<string | null>(null);
  const [manualMenuSlotId, setManualMenuSlotId] = useState<string | null>(null);
  const [initMode, setInitMode] = useState<"FAST" | "BALANCED" | "STRICT">("BALANCED");
  const isBusy = busy || actionBusy;
  const cli = overview?.cli;
  const targets = overview?.workspace_targets ?? [];
  const saveManualWorkspace = async (slotId: string, path: string | null) => {
    setActionBusy(true);
    onError(null);
    onSuccess(null);
    try {
      await invoke("set_agent_opt_project_manual_workspace", { slotId, manualWorkspace: path?.trim() || null });
      await onRefreshOverview();
    } catch (e) {
      onError(String(e));
    } finally {
      setActionBusy(false);
    }
  };

  const browseWorkspace = async (slotId: string) => {
    setManualMenuSlotId(null);
    try {
      const folder = await invoke<string | null>("select_project_folder_dialog");
      if (folder) {
        await saveManualWorkspace(slotId, folder);
      }
    } catch (e) {
      onError(String(e));
    }
  };

  const setProjectAgentOpt = async (slotId: string, enabled: boolean) => {
    setActionBusy(true);
    onError(null);
    onSuccess(null);
    try {
      const result = await invoke<NerfEngineOverview>("set_agent_opt_project_preference", { slotId, useAgentOpt: enabled });
      await onRefreshOverview();
      if (result.mismatch?.has_mismatch) {
        onError(result.mismatch.reason || t("agentOpt.inconsistentStateGuidance"));
        return;
      }
      onSuccess(t(enabled ? "agentOpt.projectEnabled" : "agentOpt.projectDisabled"));
    } catch (error) {
      onError(String(error));
      try { await onRefreshOverview(); } catch { /* Keep the original action error visible. */ }
    } finally {
      setActionBusy(false);
    }
  };

  const withWorkspaceAction = async (
    workspace: string,
    action: () => Promise<unknown>,
    successKey: TranslationKey,
  ) => {
    setActionBusy(true);
    onError(null);
    onSuccess(null);
    try {
      await action();
      await onRefreshOverview();
      const reconciled = await onWorkspacePrepared?.();
      if (reconciled === false) return;
      onSuccess(t(successKey));
    } catch (e) {
      onError(String(e));
    } finally {
      setActionBusy(false);
    }
    void workspace;
  };

  const inspectTarget = (slotId: string, useAgentOpt: boolean, inspection: AgentOptWorkspaceInspection) => {
    const detail = inspection.detail === "AgentOpt CLI is not installed"
      ? t("agentOpt.detailCliNotInstalled")
      : inspection.detail === "AgentOpt is not initialized for Antigravity in this workspace"
        ? t("agentOpt.detailNotInitialized")
        : inspection.detail;
    const needsDetails = Boolean(detail)
      || inspection.unregistered_files_detected
      || inspection.state === "not_initialized"
      || inspection.state === "not_installed"
      || inspection.state === "invalid_config"
      || inspection.state === "unsupported_version"
      || inspection.state === "error";
    return (
      <div data-testid="agentopt-workspace-inspection" style={{ display: "flex", flexDirection: "column", gap: needsDetails ? 8 : 0 }}>
        {detail && <div data-testid="agentopt-workspace-detail" style={{ padding: "6px 8px", border: `1px solid ${inspection.state === "error" || inspection.state === "invalid_config" || inspection.state === "unsupported_version" ? "#ff8182" : "#d0d7de"}`, borderRadius: 5, background: inspection.state === "error" || inspection.state === "invalid_config" || inspection.state === "unsupported_version" ? "#ffebe9" : "#f6f8fa", color: inspection.state === "error" || inspection.state === "invalid_config" || inspection.state === "unsupported_version" ? "#cf222e" : "#57606a", fontSize: 12 }}>{inspection.state === "error" ? t("agentOpt.inconsistentStateGuidance") : detail}</div>}
        {inspection.unregistered_files_detected && !inspection.registered && (
          <div data-testid="agentopt-adopt-notice" style={{ padding: "8px 10px", background: "#fff8c5", border: "1px solid #d4a72c", borderRadius: 6, fontSize: 12, display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8 }}>
            <span>{t("agentOpt.unregisteredFilesNotice")}</span>
            <button type="button" className="btn" disabled={isBusy} onClick={() => void withWorkspaceAction(inspection.workspace_path, () => invoke("adopt_agent_opt_workspace", { workspacePath: inspection.workspace_path }), "agentOpt.workspaceRegistered")}>{t("agentOpt.registerWorkspace")}</button>
          </div>
        )}
        {inspection.state === "not_initialized" && !inspection.unregistered_files_detected && (
          <div>
            {initWorkspace !== inspection.workspace_path ? (
              <button type="button" className="btn" disabled={isBusy || !cli?.installed || !cli?.supported_version} onClick={() => setInitWorkspace(inspection.workspace_path)}>{t("agentOpt.initializeWorkspace")}</button>
            ) : (
              <div data-testid="agentopt-init-confirm" style={{ padding: 10, background: "#f6f8fa", border: "1px solid #d0d7de", borderRadius: 6, fontSize: 12 }}>
                <div style={{ fontWeight: 600, marginBottom: 4 }}>{t("agentOpt.confirmInitTitle")}</div>
                <ul style={{ margin: "0 0 8px 16px", padding: 0, color: "#57606a", fontSize: 11 }}>
                  <li>{t("agentOpt.initTarget", { path: displayPath(inspection.workspace_path) })}</li>
                  <li>{t("agentOpt.initPlatform")}</li>
                  <li>{t("agentOpt.initCli", { version: cli?.pinned_version ?? "0.1.0", node: cli?.node_version ?? "unknown" })}</li>
                  <li>{t("agentOpt.initFiles")}</li>
                </ul>
                <div style={{ display: "flex", gap: 7, alignItems: "center", marginBottom: 8 }}>
                  <span>{t("agentOpt.initialMode")}</span>
                  {(["FAST", "BALANCED", "STRICT"] as const).map((mode) => <button key={mode} type="button" className={`btn ${initMode === mode ? "btn-primary" : ""}`} onClick={() => setInitMode(mode)}>{mode}</button>)}
                </div>
                <button type="button" className="btn btn-primary" disabled={isBusy} onClick={() => void withWorkspaceAction(inspection.workspace_path, () => invoke("init_agent_opt_workspace", { workspacePath: inspection.workspace_path, mode: initMode }), "agentOpt.workspaceInitialized")}>{t("agentOpt.confirmInitialize")}</button>{" "}
                <button type="button" className="btn" disabled={isBusy} onClick={() => setInitWorkspace(null)}>{t("agentOpt.cancel")}</button>
              </div>
            )}
          </div>
        )}
        {inspection.config && (
          <div style={{ borderTop: "1px solid #eaeef2", paddingTop: 9 }}>
            {!inspection.registered || !inspection.config.raw_valid || !inspection.config.schema_supported ? (
              <div data-testid="agentopt-config-readonly-error" style={{ padding: "8px 10px", background: "#ffebe9", border: "1px solid #ff8182", borderRadius: 6, color: "#cf222e", fontSize: 12 }}>
                {t("agentOpt.configReadOnly", { error: inspection.config.error ?? t("agentOpt.invalidConfigSchema") })}
              </div>
            ) : (
              <AgentOptConfigEditor
                key={`${slotId}:${inspection.workspace_path}`}
                slotId={slotId}
                useAgentOpt={useAgentOpt}
                inspection={inspection}
                disabled={isBusy}
                onRefreshOverview={onRefreshOverview}
              />
            )}
          </div>
        )}
      </div>
    );
  };

  return (
    <div data-testid="agentopt-panel" style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      {cli && !cli.installed && <div data-testid="agentopt-install-settings-notice" style={{ padding: "10px 12px", border: "1px solid #d0d7de", borderRadius: 6, background: "#fff", display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8, flexWrap: "wrap" }}><span style={{ fontSize: 12, color: "#57606a" }}>{t("agentOpt.installFromSettingsNotice")}</span><button type="button" className="btn" disabled={isBusy || !onOpenAgentOptSettings} onClick={onOpenAgentOptSettings}>{t("agentOpt.openInstallSettings")}</button></div>
        {cli.error && <div style={{ fontSize: 11, color: "#cf222e" }}>{cli.error}</div>}
      </div>}
      {cli?.installed && cli.error && <div data-testid="agentopt-cli-error-notice" style={{ padding: "10px 12px", border: "1px solid #ff8182", borderRadius: 6, background: "#ffebe9", display: "flex", justifyContent: "space-between", gap: 8 }}><span style={{ fontSize: 12, color: "#cf222e" }}>{cli.error}</span>{onOpenAgentOptSettings && <button type="button" className="btn" disabled={isBusy} onClick={onOpenAgentOptSettings}>{t("agentOpt.openInstallSettings")}</button>}</div>}

      {overview?.target_resolution_errors?.filter((error) => !targets.some(({ inspection }) =>
        unregisteredWorkspaceErrorMatchesPath(error, inspection.workspace_path)
        && !inspection.registered
        && (inspection.unregistered_files_detected || inspection.state === "not_initialized")))?.map((error) => {
        const presented = getAgentOptErrorPresentation(error, t);
        return (
          <div key={error} role="alert" style={{ padding: 9, color: "#cf222e", background: "#ffebe9", border: "1px solid #ff8182", borderRadius: 6, fontSize: 12 }}>
            <div>{presented.message}</div>
            {presented.guidance && <div style={{ marginTop: 5 }}>{presented.guidance}</div>}
          </div>
        );
      })}
      <div data-testid="agentopt-target-section" style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        {targets.map((target) => {
          const ready = cli?.installed === true
            && cli.supported_version === true
            && target.inspection.registered
            && target.inspection.config?.raw_valid === true
            && target.inspection.config?.schema_supported === true
            && target.inspection.hooks_enabled != null;
          const runtimeActive = target.inspection.config?.enabled === true || target.inspection.hooks_enabled === true;
          const disableAction = runtimeActive;
          const projectName = t("agentOpt.projectTitle", { number: target.project_slot_id === "primary" ? 1 : 2 });
          const canEnable = overview?.persisted_engine === "agentopt" && ready;
          const savedStatus = !target.inspection.registered || !cli?.installed || !cli.supported_version
            ? "not_initialized"
            : target.inspection.state === "enabled"
              ? "enabled"
              : target.inspection.state === "disabled"
                ? "disabled"
                : "not_initialized";
          const statusStyle = target.inspection.state === "invalid_config" || target.inspection.state === "unsupported_version" || target.inspection.state === "error"
            ? STATE_STYLE[target.inspection.state]
            : STATE_STYLE[savedStatus === "not_initialized" ? "not_initialized" : savedStatus];
          return <section key={`${target.project_slot_id}:${target.repository_path}`} data-testid={`agentopt-project-${target.project_slot_id}`} style={{ padding: "8px 10px", border: "1px solid #d0d7de", borderRadius: 6, background: "#fff", display: "flex", flexDirection: "column", gap: 6 }}>
            <strong style={{ fontSize: 11 }}>{projectName}</strong>
            <div data-testid={`agentopt-project-controls-${target.project_slot_id}`} style={{ display: "flex", alignItems: "center", gap: 7, minWidth: 0, flexWrap: "wrap" }}>
              <input
                type="text"
                readOnly
                aria-label={`${projectName}: ${displayPath(target.workspace_path)}`}
                data-testid={`agentopt-project-directory-${target.project_slot_id}`}
                value={displayPath(target.workspace_path)}
                title={displayPath(target.workspace_path)}
                style={{ flex: "1 1 260px", minWidth: 120, boxSizing: "border-box", fontSize: 11, padding: "5px 8px", color: "#57606a", background: "#f6f8fa", border: "1px solid #d0d7de", borderRadius: 6 }}
              />
              <div style={{ position: "relative", flex: "0 0 auto" }}>
                <button type="button" className="btn" data-testid={`agentopt-manual-select-${target.project_slot_id}`} aria-expanded={manualMenuSlotId === target.project_slot_id} disabled={isBusy} onClick={() => target.manual_workspace
                  ? setManualMenuSlotId((current) => current === target.project_slot_id ? null : target.project_slot_id)
                  : void browseWorkspace(target.project_slot_id)}>{t("agentOpt.manualSelect")}</button>
                {manualMenuSlotId === target.project_slot_id && <div data-testid={`agentopt-manual-menu-${target.project_slot_id}`} style={{ position: "absolute", zIndex: 5, top: "calc(100% + 4px)", left: 0, minWidth: 190, padding: 5, display: "flex", flexDirection: "column", gap: 4, background: "#fff", border: "1px solid #d0d7de", borderRadius: 6, boxShadow: "0 4px 12px rgba(0,0,0,.12)" }}>
                  <button type="button" className="btn" disabled={isBusy} onClick={() => void browseWorkspace(target.project_slot_id)}>{t("agentOpt.browse")}</button>
                  {target.manual_workspace && <button type="button" className="btn" disabled={isBusy} onClick={() => { setManualMenuSlotId(null); void saveManualWorkspace(target.project_slot_id, null); }}>{t("agentOpt.followProjectFolder")}</button>}
                </div>}
              </div>
              <button
                type="button"
                className={`btn ${disableAction ? "" : "btn-primary"}`}
                data-testid={`agentopt-project-action-${target.project_slot_id}`}
                disabled={isBusy || (!disableAction && !canEnable)}
                onClick={() => void setProjectAgentOpt(target.project_slot_id, !disableAction)}
              >
                {t(disableAction ? "agentOpt.disableProject" : "agentOpt.enableProject")}
              </button>
              <span data-testid={`agentopt-workspace-state-${target.project_slot_id}`} data-state={savedStatus} style={{ fontSize: 11, fontWeight: 600, padding: "2px 8px", borderRadius: 12, background: statusStyle.bg, color: statusStyle.color, border: `1px solid ${statusStyle.border}`, whiteSpace: "nowrap" }}>
                {t(`agentOpt.state.${savedStatus}` as TranslationKey)}
              </span>
            </div>
            {overview?.persisted_engine === "poverello_rules" && !disableAction && <div data-testid={`agentopt-engine-required-${target.project_slot_id}`} style={{ fontSize: 11, color: "#57606a" }}>{t("agentOpt.requiresAgentOptEngine")}</div>}
            {inspectTarget(target.project_slot_id, target.use_agent_opt, target.inspection)}
          </section>;
        })}
      </div>
    </div>
  );
}

