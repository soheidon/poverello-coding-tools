import React, { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type {
  CodexSkillsStatus,
  CodexSkillItemInfo,
  CodexGlobalRulesStatus,
} from "../types";

export default function CodexSkillsRulesPanel() {
  const { t } = useTranslation();

  // ── 1. Codex Skills State ──
  const [skillsStatus, setSkillsStatus] = useState<CodexSkillsStatus | null>(null);
  const [skillsLoading, setSkillsLoading] = useState(false);
  const [skillsError, setSkillsError] = useState<string | null>(null);
  const [skillsSuccessMessage, setSkillsSuccessMessage] = useState<string | null>(null);

  // Modal states for Skills
  const [confirmInstallSkill, setConfirmInstallSkill] = useState<CodexSkillItemInfo | null>(null);
  const [confirmUpdateSkill, setConfirmUpdateSkill] = useState<CodexSkillItemInfo | null>(null);
  const [confirmRemoveSkill, setConfirmRemoveSkill] = useState<CodexSkillItemInfo | null>(null);
  const [confirmRestoreSkill, setConfirmRestoreSkill] = useState<CodexSkillItemInfo | null>(null);

  // ── 2. Codex Global Rules State ──
  const [globalRulesStatus, setGlobalRulesStatus] = useState<CodexGlobalRulesStatus | null>(null);
  const [rulesLoading, setRulesLoading] = useState(false);
  const [rulesError, setRulesError] = useState<string | null>(null);
  const [rulesSuccessMessage, setRulesSuccessMessage] = useState<string | null>(null);

  // Modal states for Rules
  const [confirmInstallRules, setConfirmInstallRules] = useState(false);
  const [confirmRemoveRules, setConfirmRemoveRules] = useState(false);

  // ── 3. Preview Modal State ──
  const [previewData, setPreviewData] = useState<{ title: string; content: string } | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);

  // ── Fetchers ──
  const fetchSkillsStatus = useCallback(async () => {
    try {
      setSkillsLoading(true);
      setSkillsError(null);
      const res = await invoke<CodexSkillsStatus>("get_codex_skills_status");
      setSkillsStatus(res);
    } catch (e: unknown) {
      setSkillsError(String(e));
    } finally {
      setSkillsLoading(false);
    }
  }, []);

  const fetchGlobalRulesStatus = useCallback(async () => {
    try {
      setRulesLoading(true);
      setRulesError(null);
      const res = await invoke<CodexGlobalRulesStatus>("get_codex_global_rules_status");
      setGlobalRulesStatus(res);
    } catch (e: unknown) {
      setRulesError(String(e));
    } finally {
      setRulesLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchSkillsStatus();
    void fetchGlobalRulesStatus();
  }, [fetchSkillsStatus, fetchGlobalRulesStatus]);

  // ── Handlers: Skills ──
  const handleOpenSkillsFolder = async () => {
    try {
      await invoke("open_codex_skills_folder");
    } catch (e: unknown) {
      setSkillsError(String(e));
    }
  };

  const handleInstallSkill = async (name: string) => {
    setConfirmInstallSkill(null);
    try {
      setSkillsLoading(true);
      setSkillsError(null);
      await invoke<CodexSkillItemInfo>("install_codex_skill", { name });
      await fetchSkillsStatus();
      setSkillsSuccessMessage(t("codexSkillsRules.installSuccess") || "Skill installed successfully");
      setTimeout(() => setSkillsSuccessMessage(null), 3500);
    } catch (e: unknown) {
      setSkillsError(String(e));
    } finally {
      setSkillsLoading(false);
    }
  };

  const handleUpdateSkill = async (name: string) => {
    setConfirmUpdateSkill(null);
    try {
      setSkillsLoading(true);
      setSkillsError(null);
      await invoke<CodexSkillItemInfo>("update_codex_skill", { name });
      await fetchSkillsStatus();
      setSkillsSuccessMessage(t("codexSkillsRules.updateSuccess") || "Skill updated successfully");
      setTimeout(() => setSkillsSuccessMessage(null), 3500);
    } catch (e: unknown) {
      setSkillsError(String(e));
    } finally {
      setSkillsLoading(false);
    }
  };

  const handleRemoveSkill = async (name: string) => {
    setConfirmRemoveSkill(null);
    try {
      setSkillsLoading(true);
      setSkillsError(null);
      await invoke<CodexSkillItemInfo>("remove_codex_skill", { name });
      await fetchSkillsStatus();
      setSkillsSuccessMessage(t("codexSkillsRules.removeSuccess") || "Skill removed (quarantined) successfully");
      setTimeout(() => setSkillsSuccessMessage(null), 3500);
    } catch (e: unknown) {
      setSkillsError(String(e));
    } finally {
      setSkillsLoading(false);
    }
  };

  const handleRestoreSkill = async (name: string) => {
    setConfirmRestoreSkill(null);
    try {
      setSkillsLoading(true);
      setSkillsError(null);
      await invoke<CodexSkillItemInfo>("restore_codex_skill_backup", { name });
      await fetchSkillsStatus();
      setSkillsSuccessMessage(t("codexSkillsRules.restoreSuccess") || "Skill restored from backup successfully");
      setTimeout(() => setSkillsSuccessMessage(null), 3500);
    } catch (e: unknown) {
      setSkillsError(String(e));
    } finally {
      setSkillsLoading(false);
    }
  };

  // ── Handlers: Global Rules ──
  const handleOpenCodexHome = async () => {
    try {
      await invoke("open_codex_home_folder");
    } catch (e: unknown) {
      setRulesError(String(e));
    }
  };

  const handleInstallOrUpdateRules = async () => {
    setConfirmInstallRules(false);
    try {
      setRulesLoading(true);
      setRulesError(null);
      await invoke<CodexGlobalRulesStatus>("install_or_update_codex_global_rules");
      await fetchGlobalRulesStatus();
      setRulesSuccessMessage(t("codexSkillsRules.rulesUpdatedSuccess") || "Codex Global Rules updated successfully");
      setTimeout(() => setRulesSuccessMessage(null), 3500);
    } catch (e: unknown) {
      setRulesError(String(e));
    } finally {
      setRulesLoading(false);
    }
  };

  const handleRemoveRules = async () => {
    setConfirmRemoveRules(false);
    try {
      setRulesLoading(true);
      setRulesError(null);
      await invoke<CodexGlobalRulesStatus>("remove_codex_global_rules");
      await fetchGlobalRulesStatus();
      setRulesSuccessMessage(t("codexSkillsRules.rulesRemovedSuccess") || "Codex managed rules block removed");
      setTimeout(() => setRulesSuccessMessage(null), 3500);
    } catch (e: unknown) {
      setRulesError(String(e));
    } finally {
      setRulesLoading(false);
    }
  };

  // ── Handlers: Preview ──
  const handlePreviewRules = async () => {
    try {
      setPreviewLoading(true);
      const content = await invoke<string>("get_codex_global_rules_template");
      setPreviewData({
        title: t("codexSkillsRules.previewRulesTitle") || "Codex Global Rules Template",
        content,
      });
    } catch (e: unknown) {
      setRulesError(String(e));
    } finally {
      setPreviewLoading(false);
    }
  };

  const handlePreviewSkill = async (skillName: string, displayName: string) => {
    try {
      setPreviewLoading(true);
      const content = await invoke<string>("get_codex_skill_template", { name: skillName });
      setPreviewData({
        title: `${displayName} (${skillName}/SKILL.md)`,
        content,
      });
    } catch (e: unknown) {
      setSkillsError(String(e));
    } finally {
      setPreviewLoading(false);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }} data-testid="codex-skills-rules-panel">
      {/* ── CARD 1: Codex Workflow Skills ── */}
      <div className="settings-tile" data-testid="codex-skills-card">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6, flexWrap: "wrap", gap: 8 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
              {t("codexSkillsRules.skillsTitle") || "Codex Workflow Skills"}
            </h3>
            {skillsLoading && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            {skillsSuccessMessage && (
              <span style={{ fontSize: 12, color: "#1a7f37", fontWeight: 500 }} data-testid="skills-success-msg">
                ✓ {skillsSuccessMessage}
              </span>
            )}
            <button
              type="button"
              className="btn"
              style={{ fontSize: 11, padding: "4px 10px" }}
              onClick={handleOpenSkillsFolder}
              data-testid="btn-open-skills-folder"
            >
              {t("codexSkillsRules.openFolderBtn") || "Open Skills Folder"}
            </button>
            <button
              type="button"
              className="btn"
              style={{ fontSize: 11, padding: "4px 10px" }}
              onClick={fetchSkillsStatus}
              disabled={skillsLoading}
              data-testid="btn-reload-skills"
            >
              {t("codexSkillsRules.btnReload") || "Reload"}
            </button>
          </div>
        </div>

        <p style={{ margin: "0 0 12px 0", fontSize: 12, color: "#656d76" }}>
          {t("codexSkillsRules.skillsDesc") || "Manage Poverello workflow skills in Codex user scope ($HOME/.agents/skills/). Files are installed locally; Codex live discovery is separate."}
        </p>

        {skillsStatus?.skills_dir && (
          <div style={{ fontFamily: "var(--font-mono)", fontSize: 10, color: "#57606a", marginBottom: 12 }}>
            {skillsStatus.skills_dir}
          </div>
        )}

        {skillsError && (
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
            data-testid="skills-error"
          >
            {skillsError}
          </div>
        )}

        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {(skillsStatus?.skills ?? []).map((skill) => {
            const isInstalled = skill.state === "Installed";
            const isUpdateAvailable = skill.state === "UpdateAvailable";
            const isConflict = skill.state === "ProtectedConflict";
            const isRecovery = skill.state === "RecoveryRequired";
            const isBlocked = skill.state === "Blocked";
            const isNotInstalled = skill.state === "NotInstalled";

            return (
              <div
                key={skill.name}
                data-testid={`skill-item-${skill.name}`}
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
                      {skill.display_name}
                    </span>

                    {/* Status Badge */}
                    {isInstalled && (
                      <span
                        style={{
                          fontSize: 10,
                          fontWeight: 600,
                          background: "#dafbe1",
                          color: "#1a7f37",
                          padding: "1px 6px",
                          borderRadius: 4,
                          border: "1px solid #82e596",
                        }}
                        data-testid={`skill-status-${skill.name}`}
                      >
                        ✓ {t("codexSkillsRules.stateInstalled") || "Installed (Files present, Codex discovery not verified)"}
                      </span>
                    )}
                    {isUpdateAvailable && (
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
                        data-testid={`skill-status-${skill.name}`}
                      >
                        ↑ {t("codexSkillsRules.stateUpdateAvailable") || "Update Available"}
                      </span>
                    )}
                    {isConflict && (
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
                        data-testid={`skill-status-${skill.name}`}
                      >
                        ⚠ {t("codexSkillsRules.stateConflict") || "Modified / Conflict"}
                      </span>
                    )}
                    {isRecovery && (
                      <span
                        style={{
                          fontSize: 10,
                          fontWeight: 600,
                          background: "#ffebe9",
                          color: "#cf222e",
                          padding: "1px 6px",
                          borderRadius: 4,
                          border: "1px solid #ff8182",
                        }}
                        data-testid={`skill-status-${skill.name}`}
                      >
                        ⚠ {t("codexSkillsRules.stateRecovery") || "Recovery Required"}
                      </span>
                    )}
                    {isBlocked && (
                      <span
                        style={{
                          fontSize: 10,
                          fontWeight: 600,
                          background: "#ffebe9",
                          color: "#cf222e",
                          padding: "1px 6px",
                          borderRadius: 4,
                          border: "1px solid #ff8182",
                        }}
                        data-testid={`skill-status-${skill.name}`}
                      >
                        ✕ {t("codexSkillsRules.stateBlocked") || "Blocked"}
                      </span>
                    )}
                    {isNotInstalled && (
                      <span style={{ fontSize: 11, color: "#656d76", fontWeight: 500 }} data-testid={`skill-status-${skill.name}`}>
                        {t("codexSkillsRules.stateNotInstalled") || "Not Installed"}
                      </span>
                    )}
                  </div>

                  <div style={{ fontSize: 11, color: "#57606a" }}>
                    {skill.description}
                  </div>

                  {skill.detail_message && (
                    <div style={{ fontSize: 10, color: isConflict || isBlocked ? "#cf222e" : "#57606a" }}>
                      {skill.detail_message}
                    </div>
                  )}
                </div>

                {/* Action Buttons */}
                <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                  <button
                    type="button"
                    className="btn"
                    style={{ fontSize: 11, padding: "4px 10px" }}
                    onClick={() => handlePreviewSkill(skill.name, skill.display_name)}
                    disabled={skillsLoading || previewLoading}
                    data-testid={`btn-preview-${skill.name}`}
                  >
                    {t("codexSkillsRules.btnPreview") || "Preview"}
                  </button>
                  {isNotInstalled && (
                    <button
                      type="button"
                      className="btn btn-primary"
                      style={{ fontSize: 11, padding: "4px 10px" }}
                      onClick={() => setConfirmInstallSkill(skill)}
                      disabled={skillsLoading}
                      data-testid={`btn-install-${skill.name}`}
                    >
                      {t("codexSkillsRules.btnInstall") || "Install"}
                    </button>
                  )}
                  {(isInstalled || isUpdateAvailable || isConflict) && !isBlocked && !isRecovery && (
                    <>
                      <button
                        type="button"
                        className="btn"
                        style={{ fontSize: 11, padding: "4px 10px" }}
                        onClick={() => setConfirmUpdateSkill(skill)}
                        disabled={skillsLoading || isConflict}
                        title={isConflict ? "Cannot update modified or conflicted skill directory" : undefined}
                        data-testid={`btn-update-${skill.name}`}
                      >
                        {t("codexSkillsRules.btnUpdate") || "Update"}
                      </button>
                      <button
                        type="button"
                        className="btn"
                        style={{ fontSize: 11, padding: "4px 10px", color: "#cf222e" }}
                        onClick={() => setConfirmRemoveSkill(skill)}
                        disabled={skillsLoading || isConflict}
                        title={isConflict ? "Cannot remove conflicted directory" : undefined}
                        data-testid={`btn-remove-${skill.name}`}
                      >
                        {t("codexSkillsRules.btnRemove") || "Remove"}
                      </button>
                    </>
                  )}
                  {isRecovery && (
                    <button
                      type="button"
                      className="btn btn-primary"
                      style={{ fontSize: 11, padding: "4px 10px" }}
                      onClick={() => setConfirmRestoreSkill(skill)}
                      disabled={skillsLoading}
                      data-testid={`btn-restore-${skill.name}`}
                    >
                      {t("codexSkillsRules.btnRestore") || "Restore Backup"}
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* ── CARD 2: Codex Global Rules (AGENTS.md) ── */}
      <div className="settings-tile" data-testid="codex-global-rules-card">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6, flexWrap: "wrap", gap: 8 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
              {t("codexSkillsRules.globalRulesTitle") || "Codex Global Rules (AGENTS.md)"}
            </h3>
            {rulesLoading && <span style={{ fontSize: 11, color: "#656d76" }}>...</span>}
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            {rulesSuccessMessage && (
              <span style={{ fontSize: 12, color: "#1a7f37", fontWeight: 500 }} data-testid="rules-success-msg">
                ✓ {rulesSuccessMessage}
              </span>
            )}
            <button
              type="button"
              className="btn"
              style={{ fontSize: 11, padding: "4px 10px" }}
              onClick={handlePreviewRules}
              disabled={previewLoading || rulesLoading}
              data-testid="btn-preview-rules"
            >
              {t("codexSkillsRules.btnPreviewRules") || "Preview Rules"}
            </button>
            <button
              type="button"
              className="btn"
              style={{ fontSize: 11, padding: "4px 10px" }}
              onClick={handleOpenCodexHome}
              data-testid="btn-open-codex-home"
            >
              {t("codexSkillsRules.openCodexHomeBtn") || "Open Codex Home"}
            </button>
            <button
              type="button"
              className="btn"
              style={{ fontSize: 11, padding: "4px 10px" }}
              onClick={fetchGlobalRulesStatus}
              disabled={rulesLoading}
              data-testid="btn-reload-rules"
            >
              {t("codexSkillsRules.btnReload") || "Reload"}
            </button>
          </div>
        </div>

        <p style={{ margin: "0 0 12px 0", fontSize: 12, color: "#656d76" }}>
          {t("codexSkillsRules.globalRulesDesc") || "Manage Poverello safety rules block in CODEX_HOME/AGENTS.md. Only the managed block is updated; existing user instructions are preserved."}
        </p>

        {globalRulesStatus?.agents_md_path && (
          <div style={{ fontFamily: "var(--font-mono)", fontSize: 10, color: "#57606a", marginBottom: 12 }}>
            {globalRulesStatus.agents_md_path}
          </div>
        )}

        {rulesError && (
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
            data-testid="rules-error"
          >
            {rulesError}
          </div>
        )}

        {globalRulesStatus?.is_shadowed && (
          <div
            style={{
              margin: "0 0 12px 0",
              padding: "8px 12px",
              background: "#fff8c5",
              border: "1px solid #d4a72c",
              borderRadius: 6,
              fontSize: 12,
              color: "#734f00",
            }}
            data-testid="rules-shadowed-warning"
          >
            <strong>Shadowed:</strong> AGENTS.override.md is present and takes precedence over AGENTS.md. Managed rules in AGENTS.md are currently inactive.
          </div>
        )}

        <div
          style={{
            background: "#fff",
            border: "1px solid #e1e4e8",
            borderRadius: 6,
            padding: "10px 14px",
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            flexWrap: "wrap",
            gap: 10,
          }}
        >
          <div>
            <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
              <span style={{ fontSize: 13, fontWeight: 700, color: "#1f2937" }}>
                Poverello Managed Rules Block
              </span>

              {globalRulesStatus?.state === "Installed" && (
                <span
                  style={{
                    fontSize: 10,
                    fontWeight: 600,
                    background: "#dafbe1",
                    color: "#1a7f37",
                    padding: "1px 6px",
                    borderRadius: 4,
                    border: "1px solid #82e596",
                  }}
                  data-testid="rules-status"
                >
                  ✓ {t("codexSkillsRules.rulesStateInstalled") || "Active"}
                </span>
              )}
              {globalRulesStatus?.state === "UpdateAvailable" && (
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
                  data-testid="rules-status"
                >
                  ↑ {t("codexSkillsRules.rulesStateUpdate") || "Update Available"}
                </span>
              )}
              {globalRulesStatus?.state === "Shadowed" && (
                <span
                  style={{
                    fontSize: 10,
                    fontWeight: 600,
                    background: "#fff8c5",
                    color: "#734f00",
                    padding: "1px 6px",
                    borderRadius: 4,
                    border: "1px solid #d4a72c",
                  }}
                  data-testid="rules-status"
                >
                  ⚠ {t("codexSkillsRules.rulesStateShadowed") || "Shadowed"}
                </span>
              )}
              {globalRulesStatus?.state === "Conflict" && (
                <span
                  style={{
                    fontSize: 10,
                    fontWeight: 600,
                    background: "#ffebe9",
                    color: "#cf222e",
                    padding: "1px 6px",
                    borderRadius: 4,
                    border: "1px solid #ff8182",
                  }}
                  data-testid="rules-status"
                >
                  ⚠ {t("codexSkillsRules.rulesStateConflict") || "Conflict"}
                </span>
              )}
              {globalRulesStatus?.state === "NotInstalled" && (
                <span style={{ fontSize: 11, color: "#656d76", fontWeight: 500 }} data-testid="rules-status">
                  {t("codexSkillsRules.rulesStateNotInstalled") || "Not Installed"}
                </span>
              )}
            </div>

            <div style={{ fontSize: 11, color: "#57606a" }}>
              Markers: <code>&lt;!-- BEGIN POVERELLO MANAGED RULES --&gt;</code> ... <code>&lt;!-- END POVERELLO MANAGED RULES --&gt;</code>
            </div>

            {globalRulesStatus?.detail_message && (
              <div style={{ fontSize: 10, color: "#57606a", marginTop: 4 }}>
                {globalRulesStatus.detail_message ===
                "AGENTS.md exists but contains no Poverello managed rules block. Existing content will be preserved on install."
                  ? t("codexSkillsRules.rulesUnmanagedNotice") || globalRulesStatus.detail_message
                  : globalRulesStatus.detail_message}
              </div>
            )}
          </div>

          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            {globalRulesStatus?.state === "NotInstalled" && (
              <button
                type="button"
                className="btn btn-primary"
                style={{ fontSize: 11, padding: "4px 10px" }}
                onClick={() => setConfirmInstallRules(true)}
                disabled={rulesLoading}
                data-testid="btn-install-rules"
              >
                {t("codexSkillsRules.btnInstallRules") || "Install Managed Rules"}
              </button>
            )}
            {(globalRulesStatus?.state === "Installed" ||
              globalRulesStatus?.state === "UpdateAvailable" ||
              globalRulesStatus?.state === "Shadowed") && (
              <>
                <button
                  type="button"
                  className="btn"
                  style={{ fontSize: 11, padding: "4px 10px" }}
                  onClick={() => setConfirmInstallRules(true)}
                  disabled={rulesLoading}
                  data-testid="btn-update-rules"
                >
                  {t("codexSkillsRules.btnUpdateRules") || "Update Rules"}
                </button>
                <button
                  type="button"
                  className="btn"
                  style={{ fontSize: 11, padding: "4px 10px", color: "#cf222e" }}
                  onClick={() => setConfirmRemoveRules(true)}
                  disabled={rulesLoading}
                  data-testid="btn-remove-rules"
                >
                  {t("codexSkillsRules.btnRemoveRules") || "Remove Block"}
                </button>
              </>
            )}
          </div>
        </div>
      </div>

      {/* ── Confirm Modals ── */}
      {confirmInstallSkill && (
        <div data-testid="modal-confirm-install-skill" style={modalOverlayStyle}>
          <div style={modalContentStyle}>
            <h4 style={{ margin: "0 0 10px 0" }}>Install {confirmInstallSkill.display_name}</h4>
            <p style={{ fontSize: 13, color: "#57606a", lineHeight: 1.5 }}>
              This will create the skill folder under <code>~/.agents/skills/{confirmInstallSkill.name}/</code> with standard templates.
            </p>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 16 }}>
              <button className="btn" onClick={() => setConfirmInstallSkill(null)}>
                {t("antigravity.btnCancel") || "Cancel"}
              </button>
              <button data-testid="modal-proceed-install-skill" className="btn btn-primary" onClick={() => handleInstallSkill(confirmInstallSkill.name)}>
                {t("antigravity.btnConfirm") || "Proceed"}
              </button>
            </div>
          </div>
        </div>
      )}

      {confirmUpdateSkill && (
        <div data-testid="modal-confirm-update-skill" style={modalOverlayStyle}>
          <div style={modalContentStyle}>
            <h4 style={{ margin: "0 0 10px 0" }}>Update {confirmUpdateSkill.display_name}</h4>
            <p style={{ fontSize: 13, color: "#57606a", lineHeight: 1.5 }}>
              Any custom modifications inside the skill template will be replaced with standard Poverello templates. A verified backup will be saved automatically before replacing.
            </p>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 16 }}>
              <button className="btn" onClick={() => setConfirmUpdateSkill(null)}>
                {t("antigravity.btnCancel") || "Cancel"}
              </button>
              <button data-testid="modal-proceed-update-skill" className="btn btn-primary" onClick={() => handleUpdateSkill(confirmUpdateSkill.name)}>
                {t("antigravity.btnConfirm") || "Proceed with Update"}
              </button>
            </div>
          </div>
        </div>
      )}

      {confirmRemoveSkill && (
        <div data-testid="modal-confirm-remove-skill" style={modalOverlayStyle}>
          <div style={modalContentStyle}>
            <h4 style={{ margin: "0 0 10px 0", color: "#cf222e" }}>Remove {confirmRemoveSkill.display_name}</h4>
            <p style={{ fontSize: 13, color: "#57606a", lineHeight: 1.5 }}>
              The skill folder will be renamed to a quarantine folder so no data is deleted. Are you sure you want to remove this skill?
            </p>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 16 }}>
              <button className="btn" onClick={() => setConfirmRemoveSkill(null)}>
                {t("antigravity.btnCancel") || "Cancel"}
              </button>
              <button data-testid="modal-proceed-remove-skill" className="btn" style={{ background: "#cf222e", color: "#fff" }} onClick={() => handleRemoveSkill(confirmRemoveSkill.name)}>
                {t("codexSkillsRules.btnRemove") || "Remove"}
              </button>
            </div>
          </div>
        </div>
      )}

      {confirmRestoreSkill && (
        <div data-testid="modal-confirm-restore-skill" style={modalOverlayStyle}>
          <div style={modalContentStyle}>
            <h4 style={{ margin: "0 0 10px 0" }}>Restore {confirmRestoreSkill.display_name}</h4>
            <p style={{ fontSize: 13, color: "#57606a", lineHeight: 1.5 }}>
              Restore this skill from its last transaction backup to resolve the recovery required state.
            </p>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 16 }}>
              <button className="btn" onClick={() => setConfirmRestoreSkill(null)}>
                {t("antigravity.btnCancel") || "Cancel"}
              </button>
              <button className="btn btn-primary" onClick={() => handleRestoreSkill(confirmRestoreSkill.name)}>
                {t("codexSkillsRules.btnRestore") || "Restore"}
              </button>
            </div>
          </div>
        </div>
      )}

      {confirmInstallRules && (
        <div data-testid="modal-confirm-rules" style={modalOverlayStyle}>
          <div style={modalContentStyle}>
            <h4 style={{ margin: "0 0 10px 0" }}>Install / Update Managed Rules</h4>
            <p style={{ fontSize: 13, color: "#57606a", lineHeight: 1.5 }}>
              This will write the standard Poverello safety rules block between <code>&lt;!-- BEGIN POVERELLO MANAGED RULES --&gt;</code> and <code>&lt;!-- END POVERELLO MANAGED RULES --&gt;</code> in <code>AGENTS.md</code>. All surrounding custom instructions will be preserved.
            </p>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 16 }}>
              <button className="btn" onClick={() => setConfirmInstallRules(false)}>
                {t("antigravity.btnCancel") || "Cancel"}
              </button>
              <button className="btn btn-primary" onClick={handleInstallOrUpdateRules}>
                {t("antigravity.btnConfirm") || "Proceed"}
              </button>
            </div>
          </div>
        </div>
      )}

      {confirmRemoveRules && (
        <div data-testid="modal-confirm-remove-rules" style={modalOverlayStyle}>
          <div style={modalContentStyle}>
            <h4 style={{ margin: "0 0 10px 0", color: "#cf222e" }}>Remove Managed Rules Block</h4>
            <p style={{ fontSize: 13, color: "#57606a", lineHeight: 1.5 }}>
              Only the Poverello managed rules block will be removed from <code>AGENTS.md</code>. All surrounding custom user text will be preserved.
            </p>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 16 }}>
              <button className="btn" onClick={() => setConfirmRemoveRules(false)}>
                {t("antigravity.btnCancel") || "Cancel"}
              </button>
              <button className="btn" style={{ background: "#cf222e", color: "#fff" }} onClick={handleRemoveRules}>
                {t("codexSkillsRules.btnRemoveRules") || "Remove Block"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ── Template Preview Modal ── */}
      {previewData && (
        <div data-testid="modal-template-preview" style={modalOverlayStyle}>
          <div style={{ ...modalContentStyle, maxWidth: 680, maxHeight: "80vh", display: "flex", flexDirection: "column" }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
              <h4 style={{ margin: 0, fontSize: 15, fontWeight: 700, color: "#24292f" }}>{previewData.title}</h4>
              <button
                type="button"
                className="btn"
                style={{ fontSize: 12, padding: "2px 8px" }}
                onClick={() => setPreviewData(null)}
                data-testid="modal-preview-close-x"
              >
                ✕
              </button>
            </div>
            <div
              style={{
                flex: 1,
                overflowY: "auto",
                background: "#f6f8fa",
                border: "1px solid #d0d7de",
                borderRadius: 6,
                padding: "12px",
                fontFamily: "var(--font-mono)",
                fontSize: 11,
                lineHeight: 1.5,
                whiteSpace: "pre-wrap",
                color: "#24292f",
                userSelect: "text",
              }}
              data-testid="modal-preview-content"
            >
              {previewData.content}
            </div>
            <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 14 }}>
              <button
                type="button"
                className="btn"
                style={{ fontSize: 12, padding: "5px 14px" }}
                onClick={() => setPreviewData(null)}
                data-testid="modal-preview-close"
              >
                {t("popup.close") || "Close"}
              </button>
            </div>
          </div>
        </div>
      )}
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
  borderRadius: 8,
  padding: "20px 24px",
  maxWidth: 460,
  width: "90%",
  boxShadow: "0 8px 24px rgba(0, 0, 0, 0.2)",
};
