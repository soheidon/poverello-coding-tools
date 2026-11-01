import React, { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type {
  AntigravityCommandsInfo,
  AntigravityCommandItemInfo,
  GlobalRuleItemInfo,
  PovNerfCommandsInfo,
  PovNerfSkillId,
  PovNerfOperationResult,
} from "../types";

const WORKFLOW_COMMAND_NAMES = ["pov-plan-only", "pov-build-review", "pov-full-cycle"];

export default function AntigravitySkillsRulesPanel() {
  const { t } = useTranslation();

  // ── 1. Workflow Skills State ──
  const [commandsInfo, setCommandsInfo] = useState<AntigravityCommandsInfo | null>(null);
  const [commandsLoading, setCommandsLoading] = useState(false);
  const [commandsError, setCommandsError] = useState<string | null>(null);
  const [commandsSavedMessage, setCommandsSavedMessage] = useState(false);
  const [confirmRepairSkill, setConfirmRepairSkill] = useState<string | null>(null);

  // ── 2. Task Presets (Nerf) State ──
  const [nerfInfo, setNerfInfo] = useState<PovNerfCommandsInfo | null>(null);
  const [nerfStatusLoading, setNerfStatusLoading] = useState(false);
  const [nerfLoadingSkill, setNerfLoadingSkill] = useState<PovNerfSkillId | null>(null);
  const [nerfOpError, setNerfOpError] = useState<string | null>(null);
  const [nerfStatusError, setNerfStatusError] = useState<string | null>(null);

  // ── 4. Global Rules State ──
  const [globalRules, setGlobalRules] = useState<GlobalRuleItemInfo[]>([]);
  const [globalRulesLoading, setGlobalRulesLoading] = useState(false);
  const [globalRulesError, setGlobalRulesError] = useState<string | null>(null);
  const [globalRulesSavedMessage, setGlobalRulesSavedMessage] = useState(false);

  // ── Fetchers ──
  const fetchCommands = useCallback(async () => {
    try {
      setCommandsLoading(true);
      setCommandsError(null);
      const res = await invoke<AntigravityCommandsInfo>("get_antigravity_commands_status");
      setCommandsInfo(res);
    } catch (e: unknown) {
      setCommandsError(String(e));
    } finally {
      setCommandsLoading(false);
    }
  }, []);

  const fetchNerfStatus = useCallback(async () => {
    try {
      setNerfStatusLoading(true);
      setNerfStatusError(null);
      const res = await invoke<PovNerfCommandsInfo>("get_pov_nerf_commands_status");
      setNerfInfo(res);
    } catch (e: unknown) {
      setNerfStatusError(String(e));
    } finally {
      setNerfStatusLoading(false);
    }
  }, []);

  const fetchGlobalRules = useCallback(async () => {
    try {
      setGlobalRulesLoading(true);
      setGlobalRulesError(null);
      const res = await invoke<GlobalRuleItemInfo[]>("get_antigravity_global_rules_status");
      setGlobalRules(res);
    } catch (e: unknown) {
      setGlobalRulesError(String(e));
    } finally {
      setGlobalRulesLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchCommands();
    void fetchNerfStatus();
    void fetchGlobalRules();
  }, [fetchCommands, fetchNerfStatus, fetchGlobalRules]);

  // ── Workflow Commands Handlers ──
  const handleOpenSkillsFolder = async () => {
    try {
      await invoke("open_antigravity_skills_folder");
    } catch (e: unknown) {
      setCommandsError(String(e));
    }
  };

  const handleInstallCommand = async (name: string) => {
    try {
      setCommandsLoading(true);
      setCommandsError(null);
      const res = await invoke<AntigravityCommandsInfo>("install_antigravity_command", { name });
      setCommandsInfo(res);
      setCommandsSavedMessage(true);
      setTimeout(() => setCommandsSavedMessage(false), 3000);
    } catch (e: unknown) {
      setCommandsError(String(e));
    } finally {
      setCommandsLoading(false);
    }
  };

  const handleUpdateCommand = (name: string) => {
    setConfirmRepairSkill(name);
  };

  const handleProceedRepair = async () => {
    if (!confirmRepairSkill) return;
    const name = confirmRepairSkill;
    setConfirmRepairSkill(null);
    try {
      setCommandsLoading(true);
      setCommandsError(null);
      const res = await invoke<AntigravityCommandsInfo>("update_antigravity_command", { name });
      setCommandsInfo(res);
      setCommandsSavedMessage(true);
      setTimeout(() => setCommandsSavedMessage(false), 3000);
    } catch (e: unknown) {
      setCommandsError(String(e));
    } finally {
      setCommandsLoading(false);
    }
  };

  const handleRemoveCommand = async (name: string) => {
    try {
      setCommandsLoading(true);
      setCommandsError(null);
      const res = await invoke<AntigravityCommandsInfo>("remove_antigravity_command", { name });
      setCommandsInfo(res);
      setCommandsSavedMessage(true);
      setTimeout(() => setCommandsSavedMessage(false), 3000);
    } catch (e: unknown) {
      setCommandsError(String(e));
    } finally {
      setCommandsLoading(false);
    }
  };

  const handleInstallAllWorkflow = async () => {
    try {
      setCommandsLoading(true);
      setCommandsError(null);
      // Install each workflow command in order
      for (const name of WORKFLOW_COMMAND_NAMES) {
        await invoke<AntigravityCommandsInfo>("install_antigravity_command", { name });
      }
      const res = await invoke<AntigravityCommandsInfo>("get_antigravity_commands_status");
      setCommandsInfo(res);
      setCommandsSavedMessage(true);
      setTimeout(() => setCommandsSavedMessage(false), 3000);
    } catch (e: unknown) {
      setCommandsError(String(e));
    } finally {
      setCommandsLoading(false);
    }
  };

  // ── Nerf Handlers ──
  const handleNerfAction = async (skillId: PovNerfSkillId, action: "install" | "update" | "remove") => {
    try {
      setNerfLoadingSkill(skillId);
      setNerfOpError(null);
      const cmd =
        action === "install"
          ? "install_pov_nerf_command"
          : action === "update"
          ? "update_pov_nerf_command"
          : "remove_pov_nerf_command";
      await invoke<PovNerfOperationResult>(cmd, { skillId });
      await fetchNerfStatus();
    } catch (e: unknown) {
      setNerfOpError(String(e));
    } finally {
      setNerfLoadingSkill(null);
    }
  };

  // ── Global Rules Handlers ──
  const handleOpenRulesFolder = async () => {
    try {
      await invoke("open_antigravity_rules_folder");
    } catch (e: unknown) {
      setGlobalRulesError(String(e));
    }
  };

  const handleInstallGlobalRule = async (id: string) => {
    try {
      setGlobalRulesLoading(true);
      setGlobalRulesError(null);
      await invoke<GlobalRuleItemInfo>("install_antigravity_global_rule", { id });
      await fetchGlobalRules();
      setGlobalRulesSavedMessage(true);
      setTimeout(() => setGlobalRulesSavedMessage(false), 3000);
    } catch (e: unknown) {
      setGlobalRulesError(String(e));
    } finally {
      setGlobalRulesLoading(false);
    }
  };

  const handleRemoveGlobalRule = async (id: string) => {
    try {
      setGlobalRulesLoading(true);
      setGlobalRulesError(null);
      await invoke<GlobalRuleItemInfo>("remove_antigravity_global_rule", { id });
      await fetchGlobalRules();
      setGlobalRulesSavedMessage(true);
      setTimeout(() => setGlobalRulesSavedMessage(false), 3000);
    } catch (e: unknown) {
      setGlobalRulesError(String(e));
    } finally {
      setGlobalRulesLoading(false);
    }
  };

  const workflowSkills = (commandsInfo?.commands ?? []).filter((item) =>
    WORKFLOW_COMMAND_NAMES.includes(item.name)
  );

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      {/* ── CARD 1: Workflow Commands ── */}
      <div className="settings-tile">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6, flexWrap: "wrap", gap: 8 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
            <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
              {t("antigravity.workflowCommandsSectionHeader")}
            </h3>
            {commandsLoading && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            {commandsSavedMessage && (
              <span style={{ fontSize: 12, color: "#1a7f37", fontWeight: 500 }}>
                ✓ {t("antigravity.savedMessage")}
              </span>
            )}
            <button
              className="btn"
              style={{ fontSize: 11, padding: "4px 10px" }}
              onClick={handleOpenSkillsFolder}
              disabled={commandsLoading}
            >
              {t("antigravity.btnOpenSkillsFolder")}
            </button>
            {workflowSkills.some((item) => item.status !== "installed") && (
              <button
                className="btn btn-primary"
                style={{ fontSize: 11, padding: "4px 10px" }}
                onClick={handleInstallAllWorkflow}
                disabled={commandsLoading}
              >
                {t("antigravity.btnInstallAll")}
              </button>
            )}
          </div>
        </div>

        <p style={{ margin: "0 0 12px 0", fontSize: 12, color: "#656d76" }}>
          {t("antigravity.workflowCommandsDesc")}
        </p>

        {commandsError && (
          <div style={{ margin: "0 0 12px 0", padding: "8px 12px", background: "#ffebe9", border: "1px solid #ff8182", borderRadius: 6, fontSize: 12, color: "#cf222e" }}>
            {commandsError}
          </div>
        )}

        {/* Workflow Skills list */}
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {workflowSkills.map((item) => {
            const cmdName = item.name;
            const slashCmd = item.slash_command;
            const status = item.status;

            return (
              <div
                key={cmdName}
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
                <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <span style={{ fontFamily: "var(--font-mono)", fontSize: 12, fontWeight: 700, color: "#1f2937" }}>
                      {slashCmd}
                    </span>
                    <div>
                      {status === "invalid" ? (
                        <span style={{ fontSize: 11, color: "#cf222e", fontWeight: 600 }}>
                          ⚠ {t("antigravity.commandStatusInvalid")}
                        </span>
                      ) : status === "outdated" ? (
                        <span style={{ fontSize: 11, color: "#9a6700", fontWeight: 600 }}>
                          ⚠ {t("antigravity.commandStatusOutdated")}
                        </span>
                      ) : status === "installed" ? (
                        <span style={{ display: "inline-flex", alignItems: "center", gap: 3, fontSize: 11, color: "#1a7f37", fontWeight: 600 }}>
                          <span>✓</span> {t("antigravity.commandStatusInstalled")}
                        </span>
                      ) : (
                        <span style={{ fontSize: 11, color: "#656d76", fontWeight: 500 }}>
                          {t("antigravity.commandStatusNotInstalled")}
                        </span>
                      )}
                    </div>
                  </div>
                  <div style={{ fontSize: 11, color: "#57606a" }}>
                    {t("antigravity.commandGenericDesc")}
                  </div>
                  {status === "invalid" && item.error && (
                    <div style={{ fontSize: 11, color: "#cf222e" }}>{item.error}</div>
                  )}
                </div>

                <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                  {status === "not_installed" && (
                    <button
                      className="btn btn-primary"
                      style={{ fontSize: 11, padding: "4px 10px" }}
                      onClick={() => void handleInstallCommand(cmdName)}
                      disabled={commandsLoading}
                    >
                      {t("antigravity.commandBtnInstall")}
                    </button>
                  )}
                  {(status === "outdated" || status === "invalid") && (
                    <button
                      className="btn btn-primary"
                      style={{ fontSize: 11, padding: "4px 10px" }}
                      onClick={() => handleUpdateCommand(cmdName)}
                      disabled={commandsLoading}
                      data-testid={`btn-update-${cmdName}`}
                    >
                      {t("antigravity.commandBtnUpdate")}
                    </button>
                  )}
                  {status === "installed" && (
                    <button
                      className="btn btn-danger"
                      style={{ fontSize: 11, padding: "4px 10px" }}
                      onClick={() => void handleRemoveCommand(cmdName)}
                      disabled={commandsLoading}
                    >
                      {t("antigravity.commandBtnRemove")}
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* ── CARD 2: Task Presets (Nerf) ── */}
      <div className="settings-tile">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6, flexWrap: "wrap", gap: 8 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
              {t("antigravity.taskPresetsHeader")}
            </h3>
            {(nerfLoadingSkill !== null || nerfStatusLoading) && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
          </div>
        </div>

        <p style={{ margin: "0 0 12px 0", fontSize: 12, color: "#656d76" }}>
          {t("antigravity.nerfDesc")}
        </p>

        {nerfInfo?.legacy_single_skill?.present && (
          <div style={{ margin: "0 0 12px 0", padding: "8px 12px", backgroundColor: "#fff8c5", color: "#6e4a00", borderRadius: 6, fontSize: 12, border: "1px solid #d4a72c" }}>
            <div>⚠ {nerfInfo.legacy_single_skill.message}</div>
            <div style={{ fontFamily: "var(--font-mono)", fontSize: 11, marginTop: 4, color: "#57606a" }}>
              {nerfInfo.legacy_single_skill.path}
            </div>
          </div>
        )}

        {nerfOpError && (
          <div style={{ margin: "0 0 12px 0", padding: "8px 12px", background: "#ffebe9", border: "1px solid #ff8182", borderRadius: 6, fontSize: 12, color: "#cf222e" }}>
            {nerfOpError}
          </div>
        )}
        {nerfStatusError && (
          <div style={{ margin: "0 0 12px 0", padding: "8px 12px", background: "#ffebe9", border: "1px solid #ff8182", borderRadius: 6, fontSize: 12, color: "#cf222e" }}>
            {nerfStatusError}
          </div>
        )}

        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {(nerfInfo?.skills ?? []).map((skill) => {
            const isLoading = nerfLoadingSkill === skill.skill_id;
            return (
              <div
                key={skill.skill_id}
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
                <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <span style={{ fontFamily: "var(--font-mono)", fontSize: 12, fontWeight: 700, color: "#1f2937" }}>
                      {skill.slash_command}
                    </span>
                    <div>
                      {skill.state === "installed" ? (
                        <span style={{ display: "inline-flex", alignItems: "center", gap: 3, fontSize: 11, color: "#1a7f37", fontWeight: 600 }}>
                          <span>✓</span> {t("antigravity.commandStatusInstalled")}
                        </span>
                      ) : skill.state === "outdated" ? (
                        <span style={{ fontSize: 11, color: "#9a6700", fontWeight: 600 }}>
                          ⚠ {t("antigravity.commandStatusOutdated")}
                        </span>
                      ) : skill.state === "modified" || skill.state === "invalid" ? (
                        <span style={{ fontSize: 11, color: "#cf222e", fontWeight: 600 }}>
                          ⚠ {t("antigravity.nerfStatusModified")}
                        </span>
                      ) : (
                        <span style={{ fontSize: 11, color: "#656d76", fontWeight: 500 }}>
                          {t("antigravity.commandStatusNotInstalled")}
                        </span>
                      )}
                    </div>
                  </div>
                  <div style={{ fontSize: 11, color: "#57606a" }}>
                    {skill.skill_id === "balanced"
                      ? t("antigravity.nerfBalancedDesc")
                      : t("antigravity.nerfLightweightDesc")}
                  </div>
                  {skill.diagnostic_reason && (
                    <div style={{ fontSize: 11, color: "#cf222e" }}>{skill.diagnostic_reason}</div>
                  )}
                </div>

                <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                  {skill.permitted_actions?.includes("install") && (
                    <button
                      className="btn btn-primary"
                      style={{ fontSize: 11, padding: "4px 10px" }}
                      onClick={() => void handleNerfAction(skill.skill_id, "install")}
                      disabled={isLoading || nerfStatusLoading}
                    >
                      {t("antigravity.commandBtnInstall")}
                    </button>
                  )}
                  {skill.permitted_actions?.includes("update") && (
                    <button
                      className="btn btn-primary"
                      style={{ fontSize: 11, padding: "4px 10px" }}
                      onClick={() => void handleNerfAction(skill.skill_id, "update")}
                      disabled={isLoading || nerfStatusLoading}
                    >
                      {t("antigravity.commandBtnUpdate")}
                    </button>
                  )}
                  {skill.permitted_actions?.includes("remove") && (
                    <button
                      className="btn btn-danger"
                      style={{ fontSize: 11, padding: "4px 10px" }}
                      onClick={() => void handleNerfAction(skill.skill_id, "remove")}
                      disabled={isLoading || nerfStatusLoading}
                    >
                      {t("antigravity.commandBtnRemove")}
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* ── CARD 5: Global Rules (Fixed Templates) ── */}
      <div className="settings-tile">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6, flexWrap: "wrap", gap: 8 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
              {t("antigravity.globalRulesFixedHeader")}
            </h3>
            {globalRulesLoading && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            {globalRulesSavedMessage && (
              <span style={{ fontSize: 12, color: "#1a7f37", fontWeight: 500 }}>
                ✓ {t("antigravity.savedMessage")}
              </span>
            )}
            <button
              className="btn"
              style={{ fontSize: 11, padding: "4px 10px" }}
              onClick={handleOpenRulesFolder}
              disabled={globalRulesLoading}
            >
              {t("antigravity.btnOpenRulesFolder")}
            </button>
          </div>
        </div>

        <p style={{ margin: "0 0 12px 0", fontSize: 12, color: "#656d76" }}>
          {t("antigravity.globalRulesFixedDesc")}
        </p>

        {globalRulesError && (
          <div style={{ margin: "0 0 12px 0", padding: "8px 12px", background: "#ffebe9", border: "1px solid #ff8182", borderRadius: 6, fontSize: 12, color: "#cf222e" }}>
            {globalRulesError}
          </div>
        )}

        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {globalRules.map((rule) => {
            const isInstalled = rule.status === "installed";
            const isModified = rule.status === "modified_or_protected";
            const isNotInstalled = rule.status === "not_installed";

            return (
              <div
                key={rule.id}
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
                <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <span style={{ fontFamily: "var(--font-mono)", fontSize: 12, fontWeight: 700, color: "#1f2937" }}>
                      {rule.file_name}
                    </span>
                    <div>
                      {isInstalled ? (
                        <span style={{ display: "inline-flex", alignItems: "center", gap: 3, fontSize: 11, color: "#1a7f37", fontWeight: 600 }}>
                          <span>✓</span> {t("antigravity.globalRulesStatusInstalled")}
                        </span>
                      ) : isModified ? (
                        <span style={{ fontSize: 11, color: "#cf222e", fontWeight: 600 }}>
                          ⚠ {t("antigravity.globalRulesStatusModified")}
                        </span>
                      ) : (
                        <span style={{ fontSize: 11, color: "#656d76", fontWeight: 500 }}>
                          {t("antigravity.globalRulesStatusNotInstalled")}
                        </span>
                      )}
                    </div>
                  </div>
                  <div style={{ fontFamily: "var(--font-mono)", fontSize: 10, color: "#57606a" }}>
                    {rule.path}
                  </div>
                  {isModified && rule.error && (
                    <div style={{ fontSize: 11, color: "#cf222e" }}>{rule.error}</div>
                  )}
                </div>

                <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                  {isNotInstalled && (
                    <button
                      className="btn btn-primary"
                      style={{ fontSize: 11, padding: "4px 10px" }}
                      onClick={() => void handleInstallGlobalRule(rule.id)}
                      disabled={globalRulesLoading}
                    >
                      {t("antigravity.globalRulesBtnInstall")}
                    </button>
                  )}
                  {isInstalled && (
                    <button
                      className="btn btn-danger"
                      style={{ fontSize: 11, padding: "4px 10px" }}
                      onClick={() => void handleRemoveGlobalRule(rule.id)}
                      disabled={globalRulesLoading || !rule.can_remove}
                      title={!rule.can_remove ? t("antigravity.globalRulesRemovalUnsupported") : undefined}
                    >
                      {t("antigravity.globalRulesBtnRemove")}
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* Skill Repair Confirmation Modal */}
      {confirmRepairSkill && (
        <div
          data-testid="skill-repair-modal"
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
              {t("antigravity.skillRepairTitle") || `Update ${confirmRepairSkill}`}
            </h4>
            <p style={{ margin: "0 0 14px 0", fontSize: 13, color: "#57606a", lineHeight: 1.5 }}>
              {t("antigravity.skillRepairConfirm")}
            </p>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 10 }}>
              <button
                type="button"
                className="btn"
                style={{ fontSize: 12, padding: "6px 12px" }}
                data-testid="skill-repair-cancel"
                onClick={() => setConfirmRepairSkill(null)}
              >
                {t("antigravity.btnCancel") || "Cancel"}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                style={{ fontSize: 12, padding: "6px 14px" }}
                data-testid="skill-repair-proceed"
                onClick={handleProceedRepair}
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
