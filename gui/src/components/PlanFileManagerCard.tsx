import React, { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type { PlanFileDetails, ReviewerPolicy } from "../types";
import { reviewersToPolicy } from "./ExternalReviewCard";

export interface PlanFileManagerCardProps {
  repoPath?: string | null;
  surface?: "codex" | "antigravity";
  activePlanPath?: string | null;
  plannerRole?: string | null;
  reviserRole?: string | null;
  planReviewPolicy?: ReviewerPolicy | null;
  planReviewers?: string[] | null;
  embedded?: boolean;
  onActivePlanChanged?: (planRelativePath: string | null) => void;
  onRolesChanged?: (roles: {
    plannerRole: string;
    reviserRole: string;
    planReviewPolicy?: ReviewerPolicy;
    planReviewers?: string[] | null;
  }) => void;
  onPlanReviewPolicyChanged?: (policy: ReviewerPolicy) => void;
  onPlanReviewersChanged?: (reviewers: string[] | null) => void;
}

export default function PlanFileManagerCard({
  repoPath,
  surface = "codex",
  activePlanPath,
  plannerRole,
  reviserRole,
  planReviewPolicy,
  planReviewers,
  embedded = false,
  onActivePlanChanged,
  onRolesChanged,
  onPlanReviewPolicyChanged,
  onPlanReviewersChanged,
}: PlanFileManagerCardProps) {
  const { t } = useTranslation();

  const [details, setDetails] = useState<PlanFileDetails | null>(null);
  const [selectedFolder, setSelectedFolder] = useState<string | null>(null);
  const [inputFileName, setInputFileName] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [actionMessage, setActionMessage] = useState<string | null>(null);

  // Keep callback and activePlanPath references in refs to avoid re-render loops
  const onActivePlanChangedRef = useRef(onActivePlanChanged);
  useEffect(() => {
    onActivePlanChangedRef.current = onActivePlanChanged;
  });

  const activePlanPathRef = useRef(activePlanPath);
  useEffect(() => {
    activePlanPathRef.current = activePlanPath;
  });

  const selectedFolderRef = useRef(selectedFolder);
  useEffect(() => {
    selectedFolderRef.current = selectedFolder;
  });

  // Local role state fallback
  const defaultRole = surface === "antigravity" ? "antigravity" : "codex_session";
  const [currentPlannerRole, setCurrentPlannerRole] = useState(
    plannerRole || defaultRole
  );
  const [currentReviserRole, setCurrentReviserRole] = useState(
    reviserRole || defaultRole
  );
  const [currentPlanReviewPolicy, setCurrentPlanReviewPolicy] = useState<ReviewerPolicy>(
    planReviewPolicy || "antigravity"
  );
  const [currentPlanReviewers, setCurrentPlanReviewers] = useState<string[] | null>(
    planReviewers !== undefined ? planReviewers : null
  );

  useEffect(() => {
    const next = plannerRole || defaultRole;
    setCurrentPlannerRole((prev) => (prev === next ? prev : next));
  }, [plannerRole, defaultRole]);

  useEffect(() => {
    const next = reviserRole || defaultRole;
    setCurrentReviserRole((prev) => (prev === next ? prev : next));
  }, [reviserRole, defaultRole]);

  useEffect(() => {
    if (planReviewPolicy) {
      setCurrentPlanReviewPolicy(planReviewPolicy);
    }
  }, [planReviewPolicy]);

  useEffect(() => {
    setCurrentPlanReviewers(planReviewers !== undefined ? planReviewers : null);
  }, [planReviewers]);

  const loadPlans = useCallback(async (folderOverride?: string | null) => {
    try {
      setLoading(true);
      setError(null);
      const targetFolder = folderOverride !== undefined ? folderOverride : selectedFolderRef.current;
      const res = await invoke<PlanFileDetails>("list_available_plans", {
        repoPath: repoPath ?? null,
        folderPath: targetFolder ?? null,
      });
      setDetails(res);

      const loadedActive = res.active_plan_relative_path ?? null;
      if (loadedActive && !selectedFolderRef.current && folderOverride === undefined) {
        const derivedFolder = loadedActive.includes("/")
          ? loadedActive.substring(0, loadedActive.lastIndexOf("/"))
          : ".";
        setSelectedFolder(derivedFolder);
        setInputFileName(loadedActive.split("/").pop() || "");
      } else if (res.selected_folder && !selectedFolderRef.current && folderOverride === undefined) {
        setSelectedFolder(res.selected_folder);
      }

      if (loadedActive !== activePlanPathRef.current && onActivePlanChangedRef.current) {
        onActivePlanChangedRef.current(loadedActive);
      }
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [repoPath]);

  useEffect(() => {
    void loadPlans();
  }, [loadPlans]);

  const handleManualRefresh = async () => {
    if (!selectedFolder) return;
    setRefreshing(true);
    try {
      await loadPlans(selectedFolder);
    } finally {
      setRefreshing(false);
    }
  };

  const handleSelectFolder = async () => {
    try {
      setError(null);
      const picked = await invoke<string | null>("select_plan_folder_dialog", {
        repoPath: repoPath ?? null,
      });
      if (picked) {
        setSelectedFolder(picked);
        setInputFileName("");
        await loadPlans(picked);
      }
    } catch (e: unknown) {
      setError(String(e));
    }
  };

  const handleConfirm = async () => {
    if (!selectedFolder || !inputFileName.trim() || confirming) return;
    let cleanName = inputFileName.trim();
    if (!cleanName.endsWith(".md")) {
      cleanName = `${cleanName}.md`;
    }

    if (cleanName.includes("/") || cleanName.includes("\\") || cleanName.includes(":") || cleanName.includes("..")) {
      setError(t("planManager.errorInvalidFileName") || "無効なファイル名です（スラッシュやコロンは使用できません）");
      return;
    }

    const targetRelPath = selectedFolder === "." ? cleanName : `${selectedFolder}/${cleanName}`;

    const existing = details?.plans?.find((p) => {
      const pName = p.relative_path.split("/").pop() || p.relative_path;
      return pName.toLowerCase() === cleanName.toLowerCase() || p.relative_path === targetRelPath;
    });

    try {
      setConfirming(true);
      setError(null);

      if (existing) {
        const res = await invoke<PlanFileDetails>("set_active_plan", {
          relativePath: existing.relative_path,
          repoPath: repoPath ?? null,
        });
        setDetails(res);
        setActionMessage(`${t("planManager.planActivated") || "計画書を設定しました"}: ${cleanName}`);
        if (onActivePlanChangedRef.current) {
          onActivePlanChangedRef.current(existing.relative_path);
        }
      } else {
        const res = await invoke<PlanFileDetails>("create_plan_file", {
          request: {
            name: cleanName,
            folder: selectedFolder,
            set_as_active: true,
          },
          repoPath: repoPath ?? null,
        });
        setDetails(res);
        setActionMessage(`${t("planManager.planCreated") || "新しい計画書を作成しました"}: ${cleanName}`);
        if (onActivePlanChangedRef.current) {
          onActivePlanChangedRef.current(targetRelPath);
        }
      }

      setTimeout(() => setActionMessage(null), 3000);
      await loadPlans(selectedFolder);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setConfirming(false);
    }
  };

  const handleOpenPlan = async () => {
    if (!activeEntry?.path) return;
    try {
      await invoke("open_path", { path: activeEntry.path });
    } catch (e: unknown) {
      setError(String(e));
    }
  };

  const handlePlannerRoleChange = (role: string) => {
    setCurrentPlannerRole(role);
    if (onRolesChanged) {
      onRolesChanged({
        plannerRole: role,
        reviserRole: currentReviserRole,
        planReviewPolicy: currentPlanReviewPolicy,
        planReviewers: currentPlanReviewers,
      });
    }
  };

  const handleReviserRoleChange = (role: string) => {
    setCurrentReviserRole(role);
    if (onRolesChanged) {
      onRolesChanged({
        plannerRole: currentPlannerRole,
        reviserRole: role,
        planReviewPolicy: currentPlanReviewPolicy,
        planReviewers: currentPlanReviewers,
      });
    }
  };

  const handleTogglePlanReviewer = (target: "antigravity" | "codex" | "deepseek") => {
    let nextList: string[];
    if (currentPlanReviewers === null) {
      nextList = [target];
    } else {
      nextList = currentPlanReviewers.includes(target)
        ? currentPlanReviewers.filter((r) => r !== target)
        : [...currentPlanReviewers, target];
    }
    setCurrentPlanReviewers(nextList);
    const derivedPolicy = reviewersToPolicy(nextList);
    setCurrentPlanReviewPolicy(derivedPolicy);
    if (onPlanReviewersChanged) {
      onPlanReviewersChanged(nextList);
    }
    if (onPlanReviewPolicyChanged) {
      onPlanReviewPolicyChanged(derivedPolicy);
    }
    if (onRolesChanged) {
      onRolesChanged({
        plannerRole: currentPlannerRole,
        reviserRole: currentReviserRole,
        planReviewPolicy: derivedPolicy,
        planReviewers: nextList,
      });
    }
  };

  const handleSelectInherit = () => {
    setCurrentPlanReviewers(null);
    if (onPlanReviewersChanged) {
      onPlanReviewersChanged(null);
    }
    if (onRolesChanged) {
      onRolesChanged({
        plannerRole: currentPlannerRole,
        reviserRole: currentReviserRole,
        planReviewers: null,
      });
    }
  };

  const handlePlanReviewPolicyChange = (policy: ReviewerPolicy) => {
    setCurrentPlanReviewPolicy(policy);
    if (onPlanReviewPolicyChanged) {
      onPlanReviewPolicyChanged(policy);
    }
    if (onRolesChanged) {
      onRolesChanged({
        plannerRole: currentPlannerRole,
        reviserRole: currentReviserRole,
        planReviewPolicy: policy,
        planReviewers: currentPlanReviewers,
      });
    }
  };

  const activeRelPath = details?.active_plan_relative_path || activePlanPath;
  const activeEntry = details?.plans.find((p) => p.relative_path === activeRelPath);

  return (
    <div
      className={embedded ? undefined : "card"}
      style={
        embedded
          ? undefined
          : {
              border: "1px solid var(--color-border, #e5e7eb)",
              borderRadius: 8,
              padding: "12px 14px",
              marginBottom: 14,
              backgroundColor: "var(--color-bg-card, #ffffff)",
            }
      }
      data-testid="plan-manager-card"
    >
      {/* Action / Error Messages */}
      {actionMessage && (
        <div style={{ padding: "6px 10px", marginBottom: 8, backgroundColor: "#ecfdf5", color: "#065f46", borderRadius: 6, fontSize: 12 }}>
          {actionMessage}
        </div>
      )}
      {error && (
        <div style={{ padding: "6px 10px", marginBottom: 8, backgroundColor: "#fef2f2", color: "#991b1b", borderRadius: 6, fontSize: 12 }}>
          {error}
        </div>
      )}

      {/* ── ROW 1: Heading, Active Status & Refresh ── */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          gap: 8,
          marginBottom: 8,
          flexWrap: "wrap",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0, flexWrap: "wrap" }}>
          <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--color-text-primary, #111827)" }}>
            {t("planManager.title") || "プランファイル"}
          </h3>

          {activeRelPath ? (
            <div style={{ display: "inline-flex", alignItems: "center", gap: 6, fontSize: 12, minWidth: 0 }}>
              <span style={{ fontSize: 13 }} aria-hidden="true">📄</span>
              <button
                type="button"
                onClick={() => void handleOpenPlan()}
                data-testid="btn-open-plan"
                style={{
                  background: "none",
                  border: "none",
                  padding: 0,
                  fontFamily: "monospace",
                  fontSize: 12,
                  fontWeight: 600,
                  color: "var(--color-primary, #0969da)",
                  cursor: "pointer",
                  textDecoration: "underline",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                  textAlign: "left",
                }}
                title={`${activeEntry?.path || activeRelPath} (${t("planManager.openPlanTooltip") || "エディタで開く"})`}
              >
                {activeRelPath}
              </button>
              <span
                className="badge badge-success"
                data-testid="active-plan-badge"
                style={{
                  fontSize: 11,
                  padding: "1px 6px",
                  borderRadius: 4,
                  fontWeight: 500,
                  backgroundColor: "#dafbe1",
                  color: "#1a7f37",
                  border: "1px solid #aceebb",
                  flexShrink: 0,
                }}
              >
                {activeEntry?.status ? `[${activeEntry.status}]` : t("planManager.selected") || "選択中"}
              </span>
              {activeEntry?.title && (
                <span
                  style={{
                    fontSize: 12,
                    color: "var(--color-text-secondary, #57606a)",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                    maxWidth: 220,
                  }}
                  title={activeEntry.title}
                >
                  <span aria-hidden="true" style={{ margin: "0 4px", color: "#8c959f" }}>—</span>
                  <span>{activeEntry.title}</span>
                </span>
              )}
            </div>
          ) : (
            <span
              data-testid="no-active-plan-warning"
              style={{ fontSize: 12, color: "#656d76", display: "inline-flex", alignItems: "center", gap: 4 }}
            >
              <span aria-hidden="true">⚠️</span>
              <span>{t("planManager.warningTitle") || "計画書が選択されていません"}</span>
            </span>
          )}
        </div>

        <button
          type="button"
          className="btn btn-small"
          onClick={() => void handleManualRefresh()}
          disabled={refreshing || !selectedFolder}
          data-testid="btn-refresh-plans"
          title={t("planManager.refresh") || "再読込"}
          style={{ fontSize: 11, padding: "2px 8px" }}
        >
          {refreshing ? t("planManager.loading") || "更新中..." : `⟳ ${t("planManager.refresh") || "再読込"}`}
        </button>
      </div>

      {/* ── ROW 2: Folder Selector, File Input & Confirm Button ── */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          marginBottom: 12,
        }}
      >
        {/* Folder Select Button */}
        <button
          type="button"
          className="btn btn-small"
          onClick={() => void handleSelectFolder()}
          data-testid="btn-select-folder"
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 4,
            flexShrink: 0,
            padding: "4px 10px",
            fontSize: 12,
          }}
          title={selectedFolder ? `${t("planManager.folderLabel") || "プランフォルダ"}: ${selectedFolder}` : t("planManager.btnSelectFolder") || "プランフォルダを選択"}
        >
          <span aria-hidden="true">📁</span>
          <span>
            {!selectedFolder || selectedFolder === ".plan"
              ? t("planManager.defaultPlanFolderLabel") || "プランフォルダ"
              : selectedFolder}
          </span>
        </button>

        {/* Unified File Name Input with Candidate Datalist */}
        <div style={{ flex: 1, position: "relative" }}>
          <input
            type="text"
            className="form-control"
            list="plan-file-candidates"
            value={inputFileName}
            onChange={(e) => setInputFileName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                void handleConfirm();
              }
            }}
            placeholder={
              !selectedFolder
                ? t("planManager.placeholderSelectFolderFirst") || "フォルダを選択してください"
                : t("planManager.placeholderFileName") || "計画ファイル名を入力または選択 (.md)"
            }
            disabled={!selectedFolder}
            data-testid="plan-file-input"
            style={{
              width: "100%",
              padding: "4px 10px",
              fontSize: 12,
              borderRadius: 6,
              border: "1px solid #d0d7de",
              backgroundColor: !selectedFolder ? "#f6f8fa" : "#ffffff",
              color: !selectedFolder ? "#8c959f" : "inherit",
            }}
          />
          <datalist id="plan-file-candidates" data-testid="plan-file-candidates">
            {details?.plans?.map((p) => {
              const name = p.relative_path.split("/").pop() || p.relative_path;
              return (
                <option key={p.relative_path} value={name}>
                  {p.title ? `${name} — ${p.title}` : name}
                </option>
              );
            })}
          </datalist>
        </div>

        {/* Confirm Button */}
        <button
          type="button"
          className="btn btn-small btn-primary"
          onClick={() => void handleConfirm()}
          disabled={!selectedFolder || !inputFileName.trim() || confirming}
          data-testid="btn-confirm-plan"
          style={{ flexShrink: 0, padding: "4px 12px", fontSize: 12 }}
        >
          {confirming ? t("planManager.confirming") || "処理中..." : t("planManager.btnConfirm") || "確定"}
        </button>
      </div>

      {/* Role Assignment Section */}
      <div
        style={{
          borderTop: "1px solid var(--color-border, #e5e7eb)",
          paddingTop: 12,
        }}
      >
        <div style={{ display: "flex", alignItems: "baseline", gap: 10, flexWrap: "wrap", marginBottom: 10 }}>
          <h4 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--color-text-primary, #111827)" }}>
            {t("planManager.rolesTitle") || "プラン作成AIの選択"}
          </h4>
          <span style={{ fontSize: 12, color: "var(--color-text-secondary, #6b7280)" }}>
            {t("planManager.rolesSubtitle") || "計画の作成・修正を担当するAIを選択"}
          </span>
        </div>

        {(() => {
          const isSelectedRole = (current: string, optVal: string) => {
            if (surface === "codex") {
              if (!optVal) return !current;
              if (optVal === "codex_session") return current === "codex_session" || current === "codex";
            }
            if (surface === "antigravity") {
              if (optVal === "antigravity") return !current || current === "antigravity";
              if (optVal === "codex") return current === "codex" || current === "codex_session";
            }
            return current === optVal;
          };

          const roleOptions = surface === "codex"
            ? [
                { value: "codex_session", labelKey: "planManager.roleCodexSession", defaultLabel: "Codex (Session)" },
              ]
            : [
                { value: "antigravity", labelKey: "planManager.roleAntigravity", defaultLabel: "Antigravity" },
                { value: "codex", labelKey: "planManager.roleCodex", defaultLabel: "Codex CLI" },
              ];

          const isInheritActive = currentPlanReviewers === null;
          const isAntigravityActive = !isInheritActive && currentPlanReviewers.includes("antigravity");
          const isCodexActive = !isInheritActive && currentPlanReviewers.includes("codex");
          const isDeepseekActive = !isInheritActive && currentPlanReviewers.includes("deepseek");

          return (
            <div
              style={{
                display: "flex",
                flexWrap: "wrap",
                gap: "12px 20px",
                alignItems: "flex-start",
              }}
            >
              {/* 1. 計画作成 (Planner Role) */}
              <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0, flexWrap: "wrap" }}>
                <label style={{ fontSize: 12, color: "var(--color-text-secondary, #4b5563)", fontWeight: 600, whiteSpace: "nowrap", flexShrink: 0 }}>
                  {t("planManager.plannerRoleLabel") || "計画作成"}
                </label>
                <div
                  className="pov-segment-group"
                  role="radiogroup"
                  aria-label={t("planManager.plannerRoleLabel") || "計画作成"}
                  data-testid="planner-role-group"
                  style={{ display: "inline-flex", flexWrap: "wrap", gap: 2, maxWidth: "100%" }}
                >
                  {roleOptions.map((opt) => {
                    const selected = isSelectedRole(currentPlannerRole, opt.value);
                    const testIdKey = opt.value ? opt.value.replace(/_/g, "-") : "none";
                    return (
                      <label
                        key={opt.value}
                        className={`pov-segment-btn ${selected ? "active" : ""}`}
                        style={{ cursor: "pointer", display: "inline-flex", alignItems: "center", whiteSpace: "nowrap" }}
                      >
                        <input
                          type="radio"
                          name="plannerRole"
                          value={opt.value}
                          checked={selected}
                          onChange={() => handlePlannerRoleChange(opt.value)}
                          className="sr-only"
                          data-testid={`planner-role-${testIdKey}`}
                        />
                        <span>{t(opt.labelKey as any) || opt.defaultLabel}</span>
                      </label>
                    );
                  })}
                </div>
              </div>

              {/* 2. 計画レビュー (Plan Reviewer Policy) */}
              <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0, flexWrap: "wrap" }}>
                <label style={{ fontSize: 12, color: "var(--color-text-secondary, #4b5563)", fontWeight: 600, whiteSpace: "nowrap", flexShrink: 0 }}>
                  {t("planManager.planReviewerRoleLabel") || "計画レビュー"}
                </label>
                <div
                  className="pov-segment-group"
                  role="group"
                  aria-label={t("planManager.planReviewerRoleLabel") || "計画レビュー"}
                  data-testid="plan-reviewer-group"
                  style={{ display: "inline-flex", flexWrap: "wrap", gap: 2, maxWidth: "100%" }}
                >
                  {/* 実装レビュー設定を使用 */}
                  <label
                    className={`pov-segment-btn ${isInheritActive ? "active" : ""}`}
                    style={{ cursor: "pointer", display: "inline-flex", alignItems: "center", whiteSpace: "nowrap" }}
                    data-testid="plan-reviewer-inherit-toggle"
                  >
                    <input
                      type="checkbox"
                      name="planReviewInherit"
                      className="sr-only"
                      checked={isInheritActive}
                      onChange={() => handleSelectInherit()}
                      data-testid="plan-reviewer-inherit"
                    />
                    <span>{isInheritActive ? "✓ " : ""}{t("planManager.planReviewInherit") || "実装レビュー設定を使用"}</span>
                  </label>

                  {/* Antigravity (Antigravity surface only) */}
                  {surface === "antigravity" && (
                    <label
                      className={`pov-segment-btn ${isAntigravityActive ? "active" : ""}`}
                      style={{ cursor: "pointer", display: "inline-flex", alignItems: "center", whiteSpace: "nowrap" }}
                      data-testid="plan-reviewer-antigravity-toggle"
                    >
                      <input
                        type="checkbox"
                        name="planReviewAntigravity"
                        className="sr-only"
                        checked={isAntigravityActive}
                        onChange={() => handleTogglePlanReviewer("antigravity")}
                        data-testid="plan-reviewer-antigravity"
                      />
                      <span>{isAntigravityActive ? "✓ " : ""}Antigravity</span>
                    </label>
                  )}

                  {/* Codex CLI (antigravity) / Codex (Session) (codex) */}
                  <label
                    className={`pov-segment-btn ${isCodexActive ? "active" : ""}`}
                    style={{ cursor: "pointer", display: "inline-flex", alignItems: "center", whiteSpace: "nowrap" }}
                    data-testid="plan-reviewer-codex-toggle"
                  >
                    <input
                      type="checkbox"
                      name="planReviewCodex"
                      className="sr-only"
                      checked={isCodexActive}
                      onChange={() => handleTogglePlanReviewer("codex")}
                      data-testid="plan-reviewer-codex-only"
                    />
                    <span>{isCodexActive ? "✓ " : ""}{surface === "codex" ? (t("planManager.roleCodexSession") || "Codex (Session)") : "Codex CLI"}</span>
                  </label>

                  {/* DeepSeek Harness */}
                  <label
                    className={`pov-segment-btn ${isDeepseekActive ? "active" : ""}`}
                    style={{ cursor: "pointer", display: "inline-flex", alignItems: "center", whiteSpace: "nowrap" }}
                    data-testid="plan-reviewer-deepseek-toggle"
                  >
                    <input
                      type="checkbox"
                      name="planReviewDeepseek"
                      className="sr-only"
                      checked={isDeepseekActive}
                      onChange={() => handleTogglePlanReviewer("deepseek")}
                      data-testid="plan-reviewer-deepseek-only"
                    />
                    <span>{isDeepseekActive ? "✓ " : ""}DeepSeek Harness</span>
                  </label>
                </div>
              </div>

              {/* 3. 計画修正 (Reviser Role) */}
              <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0, flexWrap: "wrap" }}>
                <label style={{ fontSize: 12, color: "var(--color-text-secondary, #4b5563)", fontWeight: 600, whiteSpace: "nowrap", flexShrink: 0 }}>
                  {t("planManager.reviserRoleLabel") || "計画修正"}
                </label>
                <div
                  className="pov-segment-group"
                  role="radiogroup"
                  aria-label={t("planManager.reviserRoleLabel") || "計画修正"}
                  data-testid="reviser-role-group"
                  style={{ display: "inline-flex", flexWrap: "wrap", gap: 2, maxWidth: "100%" }}
                >
                  {roleOptions.map((opt) => {
                    const selected = isSelectedRole(currentReviserRole, opt.value);
                    const testIdKey = opt.value ? opt.value.replace(/_/g, "-") : "none";
                    return (
                      <label
                        key={opt.value}
                        className={`pov-segment-btn ${selected ? "active" : ""}`}
                        style={{ cursor: "pointer", display: "inline-flex", alignItems: "center", whiteSpace: "nowrap" }}
                      >
                        <input
                          type="radio"
                          name="reviserRole"
                          value={opt.value}
                          checked={selected}
                          onChange={() => handleReviserRoleChange(opt.value)}
                          className="sr-only"
                          data-testid={`reviser-role-${testIdKey}`}
                        />
                        <span>{t(opt.labelKey as any) || opt.defaultLabel}</span>
                      </label>
                    );
                  })}
                </div>
              </div>
            </div>
          );
        })()}
      </div>
    </div>
  );
}
