import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type { TranslationKey } from "../i18n";
import type { AgentOptWorkspaceInspection } from "../types";

type OptMode = "FAST" | "BALANCED" | "STRICT";
type PresetSelection = OptMode | "CUSTOM";
type ConfirmedPresetOverride = { mode: PresetSelection; baseline: string };

const MODE_PRESETS: Record<OptMode, {
  verification: Pick<EditableAgentOptConfig["verification"], "enabled" | "enforceOnStop" | "autoRunFastTestsOnStop" | "maxStopBlocksPerSession">;
}> = {
  FAST: { verification: { enabled: false, enforceOnStop: false, autoRunFastTestsOnStop: false, maxStopBlocksPerSession: "0" } },
  BALANCED: { verification: { enabled: true, enforceOnStop: true, autoRunFastTestsOnStop: true, maxStopBlocksPerSession: "2" } },
  STRICT: { verification: { enabled: true, enforceOnStop: true, autoRunFastTestsOnStop: false, maxStopBlocksPerSession: "2" } },
};

function matchesModePreset(config: EditableAgentOptConfig, mode: OptMode): boolean {
  const preset = MODE_PRESETS[mode].verification;
  return config.mode === mode
    && config.verification.enabled === preset.enabled
    && config.verification.enforceOnStop === preset.enforceOnStop
    && config.verification.autoRunFastTestsOnStop === preset.autoRunFastTestsOnStop
    && Number(config.verification.maxStopBlocksPerSession) === Number(preset.maxStopBlocksPerSession);
}

function inferPresetSelection(config: EditableAgentOptConfig): PresetSelection {
  return matchesModePreset(config, config.mode) ? config.mode : "CUSTOM";
}

interface EditableAgentOptConfig {
  mode: OptMode;
  context: {
    enabled: boolean;
    maxRankedFiles: string;
    maxRepoMapTokens: string;
    deduplicateReads: boolean;
    blockRedundantFullReadsAfter: string;
    minQuerySpecificityForInjection: boolean;
  };
  verification: {
    enabled: boolean;
    enforceOnStop: boolean;
    autoRunFastTestsOnStop: boolean;
    maxStopBlocksPerSession: string;
    requireTestsWhenCodeModified: boolean;
    exemptDocOnlyChanges: boolean;
  };
  failureDetection: {
    enabled: boolean;
    maxRepeatedCommandFailures: string;
    maxIdenticalErrors: string;
    maxRedundantReadsOfUnchangedFile: string;
    detectEditOscillation: boolean;
  };
  memory: {
    enabled: boolean;
    maxEntries: string;
    minConfidenceToInject: string;
    autoPruneContradictions: boolean;
  };
}

function asObj(v: unknown): Record<string, unknown> {
  return v && typeof v === "object" && !Array.isArray(v) ? (v as Record<string, unknown>) : {};
}

function readBool(obj: Record<string, unknown>, key: string, fallback: boolean): boolean {
  return typeof obj[key] === "boolean" ? (obj[key] as boolean) : fallback;
}

function readNumStr(obj: Record<string, unknown>, key: string, fallback: number): string {
  return typeof obj[key] === "number" && Number.isFinite(obj[key] as number)
    ? String(obj[key])
    : String(fallback);
}

function fromRawConfig(raw: Record<string, unknown> | null): EditableAgentOptConfig {
  const root = raw ?? {};
  const rawMode = root.mode === "BALANCED" || root.mode === "STRICT" ? root.mode : "FAST";
  const ctx = asObj(root.context);
  const ver = asObj(root.verification);
  const fail = asObj(root.failureDetection);
  const mem = asObj(root.memory);
  return {
    mode: rawMode,
    context: {
      enabled: readBool(ctx, "enabled", true),
      maxRankedFiles: readNumStr(ctx, "maxRankedFiles", 8),
      maxRepoMapTokens: readNumStr(ctx, "maxRepoMapTokens", 650),
      deduplicateReads: readBool(ctx, "deduplicateReads", true),
      blockRedundantFullReadsAfter: readNumStr(ctx, "blockRedundantFullReadsAfter", 2),
      minQuerySpecificityForInjection: readBool(ctx, "minQuerySpecificityForInjection", true),
    },
    verification: {
      enabled: readBool(ver, "enabled", rawMode !== "FAST"),
      enforceOnStop: readBool(ver, "enforceOnStop", rawMode !== "FAST"),
      autoRunFastTestsOnStop: readBool(ver, "autoRunFastTestsOnStop", rawMode === "BALANCED"),
      maxStopBlocksPerSession: readNumStr(ver, "maxStopBlocksPerSession", rawMode === "FAST" ? 0 : 2),
      requireTestsWhenCodeModified: readBool(ver, "requireTestsWhenCodeModified", true),
      exemptDocOnlyChanges: readBool(ver, "exemptDocOnlyChanges", true),
    },
    failureDetection: {
      enabled: readBool(fail, "enabled", true),
      maxRepeatedCommandFailures: readNumStr(fail, "maxRepeatedCommandFailures", 2),
      maxIdenticalErrors: readNumStr(fail, "maxIdenticalErrors", 2),
      maxRedundantReadsOfUnchangedFile: readNumStr(fail, "maxRedundantReadsOfUnchangedFile", 2),
      detectEditOscillation: readBool(fail, "detectEditOscillation", true),
    },
    memory: {
      enabled: readBool(mem, "enabled", true),
      maxEntries: readNumStr(mem, "maxEntries", 50),
      minConfidenceToInject: readNumStr(mem, "minConfidenceToInject", 0.7),
      autoPruneContradictions: readBool(mem, "autoPruneContradictions", true),
    },
  };
}

function parseIntField(raw: string, min: number): number | null {
  if (!/^\d+$/.test(raw.trim())) return null;
  const n = Number(raw.trim());
  return Number.isSafeInteger(n) && n >= min ? n : null;
}

function parseUnitFloat(raw: string): number | null {
  const trimmed = raw.trim();
  if (!trimmed) return null;
  const n = Number(trimmed);
  return Number.isFinite(n) && n >= 0 && n <= 1 ? n : null;
}

function persistedMatchesPatch(
  persisted: EditableAgentOptConfig,
  patch: {
    mode: OptMode;
    context: {
      enabled: boolean;
      maxRankedFiles: number;
      maxRepoMapTokens: number;
      deduplicateReads: boolean;
      blockRedundantFullReadsAfter: number;
      minQuerySpecificityForInjection: boolean;
    };
    verification: {
      enabled: boolean;
      enforceOnStop: boolean;
      autoRunFastTestsOnStop: boolean;
      maxStopBlocksPerSession: number;
      requireTestsWhenCodeModified: boolean;
      exemptDocOnlyChanges: boolean;
    };
    failureDetection: {
      enabled: boolean;
      maxRepeatedCommandFailures: number;
      maxIdenticalErrors: number;
      maxRedundantReadsOfUnchangedFile: number;
      detectEditOscillation: boolean;
    };
    memory: {
      enabled: boolean;
      maxEntries: number;
      minConfidenceToInject: number;
      autoPruneContradictions: boolean;
    };
  }
): boolean {
  return (
    persisted.mode === patch.mode &&
    persisted.context.enabled === patch.context.enabled &&
    Number(persisted.context.maxRankedFiles) === patch.context.maxRankedFiles &&
    Number(persisted.context.maxRepoMapTokens) === patch.context.maxRepoMapTokens &&
    persisted.context.deduplicateReads === patch.context.deduplicateReads &&
    Number(persisted.context.blockRedundantFullReadsAfter) === patch.context.blockRedundantFullReadsAfter &&
    persisted.context.minQuerySpecificityForInjection === patch.context.minQuerySpecificityForInjection &&
    persisted.verification.enabled === patch.verification.enabled &&
    persisted.verification.enforceOnStop === patch.verification.enforceOnStop &&
    persisted.verification.autoRunFastTestsOnStop === patch.verification.autoRunFastTestsOnStop &&
    Number(persisted.verification.maxStopBlocksPerSession) === patch.verification.maxStopBlocksPerSession &&
    persisted.verification.requireTestsWhenCodeModified === patch.verification.requireTestsWhenCodeModified &&
    persisted.verification.exemptDocOnlyChanges === patch.verification.exemptDocOnlyChanges &&
    persisted.failureDetection.enabled === patch.failureDetection.enabled &&
    Number(persisted.failureDetection.maxRepeatedCommandFailures) === patch.failureDetection.maxRepeatedCommandFailures &&
    Number(persisted.failureDetection.maxIdenticalErrors) === patch.failureDetection.maxIdenticalErrors &&
    Number(persisted.failureDetection.maxRedundantReadsOfUnchangedFile) === patch.failureDetection.maxRedundantReadsOfUnchangedFile &&
    persisted.failureDetection.detectEditOscillation === patch.failureDetection.detectEditOscillation &&
    persisted.memory.enabled === patch.memory.enabled &&
    Number(persisted.memory.maxEntries) === patch.memory.maxEntries &&
    Number(persisted.memory.minConfidenceToInject) === patch.memory.minConfidenceToInject &&
    persisted.memory.autoPruneContradictions === patch.memory.autoPruneContradictions
  );
}

export default function AgentOptConfigEditor({
  slotId,
  useAgentOpt,
  inspection,
  disabled,
  onRefreshOverview,
}: {
  slotId: string;
  useAgentOpt: boolean;
  inspection: AgentOptWorkspaceInspection;
  disabled: boolean;
  onRefreshOverview: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const canonical = useMemo(
    () => fromRawConfig(inspection.config?.raw ?? null),
    [inspection.config?.raw]
  );
  const [expanded, setExpanded] = useState(false);
  const [draft, setDraft] = useState<EditableAgentOptConfig>(canonical);
  const [presetSelection, setPresetSelection] = useState<PresetSelection>(() => inferPresetSelection(canonical));
  const [confirmedPresetOverride, setConfirmedPresetOverride] = useState<ConfirmedPresetOverride | null>(null);
  const [saving, setSaving] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);
  const [localSuccess, setLocalSuccess] = useState<string | null>(null);
  const savedPresetSelection = confirmedPresetOverride?.mode ?? inferPresetSelection(canonical);

  const isDirty = useMemo(
    () => JSON.stringify(draft) !== JSON.stringify(canonical),
    [draft, canonical]
  );

  useEffect(() => {
    if (!isDirty) {
      setDraft(canonical);
      setPresetSelection(inferPresetSelection(canonical));
    }
  }, [canonical, isDirty]);

  useEffect(() => {
    if (confirmedPresetOverride && JSON.stringify(canonical) !== confirmedPresetOverride.baseline) {
      setConfirmedPresetOverride(null);
    }
  }, [canonical, confirmedPresetOverride]);

  const handleDiscard = () => {
    setDraft(canonical);
    setPresetSelection(inferPresetSelection(canonical));
    setLocalError(null);
    setLocalSuccess(null);
  };

  const handleSave = async () => {
    setLocalError(null);
    setLocalSuccess(null);

    const maxRankedFiles = parseIntField(draft.context.maxRankedFiles, 1);
    if (maxRankedFiles === null) {
      setLocalError(t("agentOpt.configEditor.invalidPositiveInt", { field: t("agentOpt.configEditor.field.maxRankedFiles") }));
      return;
    }
    const maxRepoMapTokens = parseIntField(draft.context.maxRepoMapTokens, 1);
    if (maxRepoMapTokens === null) {
      setLocalError(t("agentOpt.configEditor.invalidPositiveInt", { field: t("agentOpt.configEditor.field.maxRepoMapTokens") }));
      return;
    }
    const blockRedundantFullReadsAfter = parseIntField(draft.context.blockRedundantFullReadsAfter, 1);
    if (blockRedundantFullReadsAfter === null) {
      setLocalError(t("agentOpt.configEditor.invalidPositiveInt", { field: t("agentOpt.configEditor.field.blockRedundantFullReadsAfter") }));
      return;
    }
    const maxStopBlocksPerSession = parseIntField(draft.verification.maxStopBlocksPerSession, 0);
    if (maxStopBlocksPerSession === null) {
      setLocalError(t("agentOpt.configEditor.invalidNonNegInt", { field: t("agentOpt.configEditor.field.maxStopBlocksPerSession") }));
      return;
    }
    const maxRepeatedCommandFailures = parseIntField(draft.failureDetection.maxRepeatedCommandFailures, 1);
    if (maxRepeatedCommandFailures === null) {
      setLocalError(t("agentOpt.configEditor.invalidPositiveInt", { field: t("agentOpt.configEditor.field.maxRepeatedCommandFailures") }));
      return;
    }
    const maxIdenticalErrors = parseIntField(draft.failureDetection.maxIdenticalErrors, 1);
    if (maxIdenticalErrors === null) {
      setLocalError(t("agentOpt.configEditor.invalidPositiveInt", { field: t("agentOpt.configEditor.field.maxIdenticalErrors") }));
      return;
    }
    const maxRedundantReadsOfUnchangedFile = parseIntField(draft.failureDetection.maxRedundantReadsOfUnchangedFile, 1);
    if (maxRedundantReadsOfUnchangedFile === null) {
      setLocalError(t("agentOpt.configEditor.invalidPositiveInt", { field: t("agentOpt.configEditor.field.maxRedundantReadsOfUnchangedFile") }));
      return;
    }
    const maxEntries = parseIntField(draft.memory.maxEntries, 1);
    if (maxEntries === null) {
      setLocalError(t("agentOpt.configEditor.invalidPositiveInt", { field: t("agentOpt.configEditor.field.maxEntries") }));
      return;
    }
    const minConfidenceToInject = parseUnitFloat(draft.memory.minConfidenceToInject);
    if (minConfidenceToInject === null) {
      setLocalError(t("agentOpt.configEditor.invalidConfidence"));
      return;
    }

    const patch = {
      mode: draft.mode,
      context: {
        enabled: draft.context.enabled,
        maxRankedFiles,
        maxRepoMapTokens,
        deduplicateReads: draft.context.deduplicateReads,
        blockRedundantFullReadsAfter,
        minQuerySpecificityForInjection: draft.context.minQuerySpecificityForInjection,
      },
      verification: {
        enabled: draft.verification.enabled,
        enforceOnStop: draft.verification.enforceOnStop,
        autoRunFastTestsOnStop: draft.verification.autoRunFastTestsOnStop,
        maxStopBlocksPerSession,
        requireTestsWhenCodeModified: draft.verification.requireTestsWhenCodeModified,
        exemptDocOnlyChanges: draft.verification.exemptDocOnlyChanges,
      },
      failureDetection: {
        enabled: draft.failureDetection.enabled,
        maxRepeatedCommandFailures,
        maxIdenticalErrors,
        maxRedundantReadsOfUnchangedFile,
        detectEditOscillation: draft.failureDetection.detectEditOscillation,
      },
      memory: {
        enabled: draft.memory.enabled,
        maxEntries,
        minConfidenceToInject,
        autoPruneContradictions: draft.memory.autoPruneContradictions,
      },
    };

    setSaving(true);
    try {
      const updated = await invoke<AgentOptWorkspaceInspection>("update_agent_opt_config", {
        workspacePath: inspection.workspace_path,
        patch,
      });
      await onRefreshOverview();
      const persisted = fromRawConfig(updated?.config?.raw ?? null);
      if (!persistedMatchesPatch(persisted, patch)) {
        setLocalError(t("agentOpt.configEditor.saveMismatch"));
        return;
      }
      setDraft(persisted);
      setLocalSuccess(t("agentOpt.configEditor.saved"));
    } catch (e) {
      setLocalError(t("agentOpt.configEditor.saveFailed", { error: String(e) }));
    } finally {
      setSaving(false);
    }
  };

  const handleApplyPreset = async (mode: OptMode) => {
    if (isDirty || disabled || saving) return;
    setLocalError(null);
    setLocalSuccess(null);
    if (matchesModePreset(canonical, mode)) {
      setExpanded(false);
      return;
    }

    setSaving(true);
    let updated: AgentOptWorkspaceInspection;
    try {
      updated = await invoke<AgentOptWorkspaceInspection>("update_agent_opt_config", {
        workspacePath: inspection.workspace_path,
        patch: {
          mode,
          verification: {
            ...MODE_PRESETS[mode].verification,
            maxStopBlocksPerSession: Number(MODE_PRESETS[mode].verification.maxStopBlocksPerSession),
          },
        },
      });
    } catch (error) {
      setLocalError(t("agentOpt.configEditor.saveFailed", { error: String(error) }));
      try { await onRefreshOverview(); } catch { /* Keep the original command error visible. */ }
      setSaving(false);
      return;
    }
    if (!updated?.config?.raw_valid || !updated.config.schema_supported || !updated.config.raw) {
      setLocalError(t("agentOpt.configEditor.saveMismatch"));
      try { await onRefreshOverview(); } catch { /* Keep the missing/invalid read-back visible. */ }
      setSaving(false);
      return;
    }
    const persisted = fromRawConfig(updated.config.raw);
    if (!matchesModePreset(persisted, mode)) {
      setLocalError(t("agentOpt.configEditor.saveMismatch"));
      try { await onRefreshOverview(); } catch { /* Keep the read-back mismatch visible. */ }
      setSaving(false);
      return;
    }
    setDraft(persisted);
    setPresetSelection(mode);
    setConfirmedPresetOverride({ mode, baseline: JSON.stringify(canonical) });
    setExpanded(false);
    try {
      await onRefreshOverview();
      setLocalSuccess(t("agentOpt.configEditor.saved"));
    } catch (error) {
      setLocalError(t("agentOpt.configEditor.saveFailed", { error: String(error) }));
    } finally {
      setSaving(false);
    }
  };

  const renderBool = (
    testId: string,
    labelKey: TranslationKey,
    descKey: TranslationKey,
    checked: boolean,
    onChange: (next: boolean) => void
  ) => (
    <label style={{ display: "flex", flexDirection: "column", gap: 2, fontSize: 12 }}>
      <span style={{ display: "inline-flex", alignItems: "center", gap: 6, fontWeight: 500 }}>
        <input
          type="checkbox"
          data-testid={testId}
          checked={checked}
          disabled={disabled || saving}
          onChange={(e) => {
            setPresetSelection("CUSTOM");
            onChange(e.currentTarget.checked);
          }}
        />
        {t(labelKey)}
      </span>
      <span style={{ fontSize: 11, color: "#57606a", paddingLeft: 20 }}>{t(descKey)}</span>
    </label>
  );

  const renderNumber = (
    testId: string,
    labelKey: TranslationKey,
    descKey: TranslationKey,
    value: string,
    onChange: (next: string) => void,
    step = "1"
  ) => (
    <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8 }}>
        <label htmlFor={testId} style={{ fontSize: 12, fontWeight: 500 }}>{t(labelKey)}</label>
        <input
          id={testId}
          data-testid={testId}
          type="number"
          step={step}
          value={value}
          disabled={disabled || saving}
          onChange={(e) => {
            setPresetSelection("CUSTOM");
            onChange(e.currentTarget.value);
          }}
          style={{ width: 88, fontSize: 12, padding: "2px 6px" }}
        />
      </div>
      <span style={{ fontSize: 11, color: "#57606a" }}>{t(descKey)}</span>
    </div>
  );

  const hasEnabledMismatch =
    typeof inspection.config?.enabled === "boolean" &&
    inspection.config.enabled !== useAgentOpt;

  const handleReconcileEnabled = async () => {
    setLocalError(null);
    setLocalSuccess(null);
    setSaving(true);
    try {
      await invoke("apply_selected_antigravity_nerf_engine");
      await onRefreshOverview();
    } catch (e) {
      setLocalError(t("agentOpt.configEditor.saveFailed", { error: String(e) }));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div data-testid={`agentopt-config-editor-${slotId}`} style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <div data-testid={`agentopt-config-presets-${slotId}`} style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8, flexWrap: "wrap" }}>
        <div className="pov-segment-group">
          {(["FAST", "BALANCED", "STRICT"] as const).map((mode) => (
            <button
              key={mode}
              type="button"
              data-testid={`agentopt-config-mode-${slotId}-${mode}`}
              className={`pov-segment-btn ${savedPresetSelection === mode ? "active" : ""}`}
              disabled={disabled || saving || isDirty}
              onClick={() => void handleApplyPreset(mode)}
            >
              {mode}
            </button>
          ))}
          <button
            type="button"
            data-testid={`agentopt-config-mode-${slotId}-CUSTOM`}
            className={`pov-segment-btn ${savedPresetSelection === "CUSTOM" ? "active" : ""}`}
            disabled={disabled || saving}
            onClick={() => {
              setLocalError(null);
              setLocalSuccess(null);
              setPresetSelection("CUSTOM");
              setExpanded(true);
            }}
          >
            {t("agentOpt.configEditor.customPreset")}
          </button>
        </div>
        {expanded && <button type="button" className="btn" data-testid={`agentopt-config-close-${slotId}`} disabled={saving} onClick={() => setExpanded(false)}>{t("agentOpt.configEditor.toggleHide")}</button>}
      </div>
      <div data-testid={`agentopt-config-preset-description-${slotId}`} style={{ fontSize: 11, color: "#57606a" }}>
        {t(`agentOpt.configEditor.presetDesc.${presetSelection.toLowerCase()}` as TranslationKey)}
      </div>
      {localError && (
        <div role="alert" data-testid={`agentopt-config-error-${slotId}`} style={{ padding: "6px 8px", background: "#ffebe9", border: "1px solid #ff8182", borderRadius: 4, color: "#cf222e", fontSize: 12 }}>
          {localError}
        </div>
      )}
      {localSuccess && (
        <div data-testid={`agentopt-config-success-${slotId}`} style={{ padding: "6px 8px", background: "#dafbe1", border: "1px solid #4ac26b", borderRadius: 4, color: "#1a7f37", fontSize: 12 }}>
          {localSuccess}
        </div>
      )}
      {hasEnabledMismatch && (
        <div
          data-testid={`agentopt-enabled-mismatch-${slotId}`}
          style={{
            padding: "6px 8px",
            background: "#fff8c5",
            border: "1px solid #d4a72c",
            borderRadius: 4,
            color: "#6e4a00",
            fontSize: 12,
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            gap: 8,
            flexWrap: "wrap",
          }}
        >
          <span>{t("agentOpt.configEditor.enabledMismatch")}</span>
          <button
            type="button"
            className="btn"
            data-testid={`agentopt-reconcile-enabled-${slotId}`}
            disabled={disabled || saving}
            onClick={() => void handleReconcileEnabled()}
          >
            {t("agentOpt.configEditor.reconcileEnabled")}
          </button>
        </div>
      )}
      {expanded && isDirty && <span data-testid={`agentopt-config-unsaved-${slotId}`} style={{ color: "#9a6700", fontSize: 11, fontWeight: 600 }}>{t("agentOpt.configEditor.unsavedBadge")}</span>}

      {expanded && (
        <div
          data-testid={`agentopt-config-body-${slotId}`}
          style={{
            display: "flex",
            flexDirection: "column",
            gap: 10,
            padding: 10,
            background: "#f6f8fa",
            border: "1px solid #d0d7de",
            borderRadius: 6,
          }}
        >
          {/* Context */}
          <fieldset style={{ border: "1px solid #d0d7de", borderRadius: 4, padding: 8, margin: 0, display: "flex", flexDirection: "column", gap: 6 }}>
            <legend style={{ fontSize: 11, fontWeight: 600, padding: "0 4px" }}>{t("agentOpt.configEditor.groupContext")}</legend>
            {renderBool(`agentopt-cfg-${slotId}-context-enabled`, "agentOpt.configEditor.field.contextEnabled", "agentOpt.configEditor.fieldDesc.contextEnabled", draft.context.enabled, (v) => setDraft((c) => ({ ...c, context: { ...c.context, enabled: v } })))}
            {renderNumber(`agentopt-cfg-${slotId}-maxRankedFiles`, "agentOpt.configEditor.field.maxRankedFiles", "agentOpt.configEditor.fieldDesc.maxRankedFiles", draft.context.maxRankedFiles, (v) => setDraft((c) => ({ ...c, context: { ...c.context, maxRankedFiles: v } })))}
            {renderNumber(`agentopt-cfg-${slotId}-maxRepoMapTokens`, "agentOpt.configEditor.field.maxRepoMapTokens", "agentOpt.configEditor.fieldDesc.maxRepoMapTokens", draft.context.maxRepoMapTokens, (v) => setDraft((c) => ({ ...c, context: { ...c.context, maxRepoMapTokens: v } })))}
            {renderBool(`agentopt-cfg-${slotId}-deduplicateReads`, "agentOpt.configEditor.field.deduplicateReads", "agentOpt.configEditor.fieldDesc.deduplicateReads", draft.context.deduplicateReads, (v) => setDraft((c) => ({ ...c, context: { ...c.context, deduplicateReads: v } })))}
            {renderNumber(`agentopt-cfg-${slotId}-blockRedundantFullReadsAfter`, "agentOpt.configEditor.field.blockRedundantFullReadsAfter", "agentOpt.configEditor.fieldDesc.blockRedundantFullReadsAfter", draft.context.blockRedundantFullReadsAfter, (v) => setDraft((c) => ({ ...c, context: { ...c.context, blockRedundantFullReadsAfter: v } })))}
            {renderBool(`agentopt-cfg-${slotId}-minQuerySpecificity`, "agentOpt.configEditor.field.minQuerySpecificityForInjection", "agentOpt.configEditor.fieldDesc.minQuerySpecificityForInjection", draft.context.minQuerySpecificityForInjection, (v) => setDraft((c) => ({ ...c, context: { ...c.context, minQuerySpecificityForInjection: v } })))}
          </fieldset>

          {/* Verification */}
          <fieldset style={{ border: "1px solid #d0d7de", borderRadius: 4, padding: 8, margin: 0, display: "flex", flexDirection: "column", gap: 6 }}>
            <legend style={{ fontSize: 11, fontWeight: 600, padding: "0 4px" }}>{t("agentOpt.configEditor.groupVerification")}</legend>
            {renderBool(`agentopt-cfg-${slotId}-ver-enabled`, "agentOpt.configEditor.field.verificationEnabled", "agentOpt.configEditor.fieldDesc.verificationEnabled", draft.verification.enabled, (v) => setDraft((c) => ({ ...c, verification: { ...c.verification, enabled: v } })))}
            {renderBool(`agentopt-cfg-${slotId}-enforceOnStop`, "agentOpt.configEditor.field.enforceOnStop", "agentOpt.configEditor.fieldDesc.enforceOnStop", draft.verification.enforceOnStop, (v) => setDraft((c) => ({ ...c, verification: { ...c.verification, enforceOnStop: v } })))}
            {renderBool(`agentopt-cfg-${slotId}-autoRunFastTestsOnStop`, "agentOpt.configEditor.field.autoRunFastTestsOnStop", "agentOpt.configEditor.fieldDesc.autoRunFastTestsOnStop", draft.verification.autoRunFastTestsOnStop, (v) => setDraft((c) => ({ ...c, verification: { ...c.verification, autoRunFastTestsOnStop: v } })))}
            {renderNumber(`agentopt-cfg-${slotId}-maxStopBlocksPerSession`, "agentOpt.configEditor.field.maxStopBlocksPerSession", "agentOpt.configEditor.fieldDesc.maxStopBlocksPerSession", draft.verification.maxStopBlocksPerSession, (v) => setDraft((c) => ({ ...c, verification: { ...c.verification, maxStopBlocksPerSession: v } })))}
            {renderBool(`agentopt-cfg-${slotId}-requireTestsWhenCodeModified`, "agentOpt.configEditor.field.requireTestsWhenCodeModified", "agentOpt.configEditor.fieldDesc.requireTestsWhenCodeModified", draft.verification.requireTestsWhenCodeModified, (v) => setDraft((c) => ({ ...c, verification: { ...c.verification, requireTestsWhenCodeModified: v } })))}
            {renderBool(`agentopt-cfg-${slotId}-exemptDocOnlyChanges`, "agentOpt.configEditor.field.exemptDocOnlyChanges", "agentOpt.configEditor.fieldDesc.exemptDocOnlyChanges", draft.verification.exemptDocOnlyChanges, (v) => setDraft((c) => ({ ...c, verification: { ...c.verification, exemptDocOnlyChanges: v } })))}
          </fieldset>

          {/* Failure Detection */}
          <fieldset style={{ border: "1px solid #d0d7de", borderRadius: 4, padding: 8, margin: 0, display: "flex", flexDirection: "column", gap: 6 }}>
            <legend style={{ fontSize: 11, fontWeight: 600, padding: "0 4px" }}>{t("agentOpt.configEditor.groupFailure")}</legend>
            {renderBool(`agentopt-cfg-${slotId}-fail-enabled`, "agentOpt.configEditor.field.failureEnabled", "agentOpt.configEditor.fieldDesc.failureEnabled", draft.failureDetection.enabled, (v) => setDraft((c) => ({ ...c, failureDetection: { ...c.failureDetection, enabled: v } })))}
            {renderNumber(`agentopt-cfg-${slotId}-maxRepeatedCommandFailures`, "agentOpt.configEditor.field.maxRepeatedCommandFailures", "agentOpt.configEditor.fieldDesc.maxRepeatedCommandFailures", draft.failureDetection.maxRepeatedCommandFailures, (v) => setDraft((c) => ({ ...c, failureDetection: { ...c.failureDetection, maxRepeatedCommandFailures: v } })))}
            {renderNumber(`agentopt-cfg-${slotId}-maxIdenticalErrors`, "agentOpt.configEditor.field.maxIdenticalErrors", "agentOpt.configEditor.fieldDesc.maxIdenticalErrors", draft.failureDetection.maxIdenticalErrors, (v) => setDraft((c) => ({ ...c, failureDetection: { ...c.failureDetection, maxIdenticalErrors: v } })))}
            {renderNumber(`agentopt-cfg-${slotId}-maxRedundantReads`, "agentOpt.configEditor.field.maxRedundantReadsOfUnchangedFile", "agentOpt.configEditor.fieldDesc.maxRedundantReadsOfUnchangedFile", draft.failureDetection.maxRedundantReadsOfUnchangedFile, (v) => setDraft((c) => ({ ...c, failureDetection: { ...c.failureDetection, maxRedundantReadsOfUnchangedFile: v } })))}
            {renderBool(`agentopt-cfg-${slotId}-detectEditOscillation`, "agentOpt.configEditor.field.detectEditOscillation", "agentOpt.configEditor.fieldDesc.detectEditOscillation", draft.failureDetection.detectEditOscillation, (v) => setDraft((c) => ({ ...c, failureDetection: { ...c.failureDetection, detectEditOscillation: v } })))}
          </fieldset>

          {/* Memory */}
          <fieldset style={{ border: "1px solid #d0d7de", borderRadius: 4, padding: 8, margin: 0, display: "flex", flexDirection: "column", gap: 6 }}>
            <legend style={{ fontSize: 11, fontWeight: 600, padding: "0 4px" }}>{t("agentOpt.configEditor.groupMemory")}</legend>
            {renderBool(`agentopt-cfg-${slotId}-mem-enabled`, "agentOpt.configEditor.field.memoryEnabled", "agentOpt.configEditor.fieldDesc.memoryEnabled", draft.memory.enabled, (v) => setDraft((c) => ({ ...c, memory: { ...c.memory, enabled: v } })))}
            {renderNumber(`agentopt-cfg-${slotId}-maxEntries`, "agentOpt.configEditor.field.maxEntries", "agentOpt.configEditor.fieldDesc.maxEntries", draft.memory.maxEntries, (v) => setDraft((c) => ({ ...c, memory: { ...c.memory, maxEntries: v } })))}
            {renderNumber(`agentopt-cfg-${slotId}-minConfidenceToInject`, "agentOpt.configEditor.field.minConfidenceToInject", "agentOpt.configEditor.fieldDesc.minConfidenceToInject", draft.memory.minConfidenceToInject, (v) => setDraft((c) => ({ ...c, memory: { ...c.memory, minConfidenceToInject: v } })), "0.05")}
            {renderBool(`agentopt-cfg-${slotId}-autoPruneContradictions`, "agentOpt.configEditor.field.autoPruneContradictions", "agentOpt.configEditor.fieldDesc.autoPruneContradictions", draft.memory.autoPruneContradictions, (v) => setDraft((c) => ({ ...c, memory: { ...c.memory, autoPruneContradictions: v } })))}
          </fieldset>

          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <button
              type="button"
              className="btn"
              data-testid={`agentopt-config-cancel-${slotId}`}
              disabled={disabled || saving || !isDirty}
              onClick={handleDiscard}
            >
              {t("agentOpt.configEditor.cancel")}
            </button>
            <button
              type="button"
              className="btn btn-primary"
              data-testid={`agentopt-config-save-${slotId}`}
              disabled={disabled || saving || !isDirty}
              onClick={() => void handleSave()}
            >
              {t("agentOpt.configEditor.save")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
