import React, { useState, useEffect, useCallback, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type { PlanFileDetails, ActivePlanFileDetails, ReviewerPolicy, ProjectPlanSlot, SelectedPlanResult } from "../types";
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
    plannerRole?: string | null;
    reviserRole?: string | null;
    planReviewPolicy?: ReviewerPolicy;
    planReviewers?: string[] | null;
  }) => void;
  onPlanReviewPolicyChanged?: (policy: ReviewerPolicy) => void;
  onPlanReviewersChanged?: (reviewers: string[] | null) => void;
}

export function normalizeDisplayPath(rawPath?: string | null): string {
  if (!rawPath) return "";
  let p = rawPath.trim();
  // Strip file:/// or file:// prefix if present
  if (p.startsWith("file:///")) {
    p = decodeURI(p.slice(8));
  } else if (p.startsWith("file://")) {
    p = decodeURI(p.slice(7));
  }
  // Strip Windows extended-length prefix \\?\ or //?/
  if (p.startsWith("\\\\?\\UNC\\")) {
    p = "\\\\" + p.slice(8);
  } else if (p.startsWith("\\\\?\\")) {
    p = p.slice(4);
  } else if (p.startsWith("//?/UNC/")) {
    p = "//" + p.slice(8);
  } else if (p.startsWith("//?/")) {
    p = p.slice(4);
  } else if (p.startsWith("/?/")) {
    p = p.slice(3);
  }
  // If starts with /C:/ or /C:\, remove leading slash
  if (/^\/[a-zA-Z]:[/|\\]/.test(p)) {
    p = p.slice(1);
  }
  // If it's a Windows drive path (e.g. C:/...), normalize forward slashes to backslashes
  if (/^[a-zA-Z]:/.test(p)) {
    p = p.replace(/\//g, "\\");
  }
  return p;
}

export type KnownPlanStatus =
  | "PLANNED"
  | "IMPLEMENTED"
  | "APPROVED"
  | "REQUEST_CHANGES"
  | "BLOCKED"
  | "IN_PROGRESS";

export function resolveStatusCategory(rawStatus?: string | null): KnownPlanStatus | null {
  if (!rawStatus) return null;
  let clean = rawStatus.trim();
  if (clean.startsWith("[") && clean.endsWith("]")) {
    clean = clean.slice(1, -1).trim();
  }
  const first = clean.split(/\s*[-—]\s*/)[0].trim().toUpperCase();
  if (!first) return null;

  if (first.startsWith("APPROVED")) return "APPROVED";
  if (first.startsWith("IMPLEMENTED")) return "IMPLEMENTED";
  if (first.startsWith("PLANNED")) return "PLANNED";
  if (first.startsWith("REQUEST_CHANGES") || first.startsWith("CHANGES_REQUESTED")) return "REQUEST_CHANGES";
  if (first.startsWith("BLOCKED") || first.startsWith("HALTED")) return "BLOCKED";
  if (first.startsWith("IN_PROGRESS")) return "IN_PROGRESS";
  return null;
}

export function getLocalizedStatusLabel(
  rawStatus: string | null | undefined,
  t: (key: any, params?: any) => string
): string {
  const category = resolveStatusCategory(rawStatus);
  if (category === "PLANNED") return t("planManager.status.planned") || "計画済";
  if (category === "IMPLEMENTED") return t("planManager.status.implemented") || "実装済";
  if (category === "APPROVED") return t("planManager.status.approved") || "承認済";
  if (category === "REQUEST_CHANGES") return t("planManager.status.requestChanges") || "要修正";
  if (category === "BLOCKED") return t("planManager.status.blocked") || "中断";
  if (category === "IN_PROGRESS") return t("planManager.status.inProgress") || "進行中";
  return t("planManager.status.unknown") || t("planManager.selected") || "選択中";
}

// Preserve toFileUrl and getConciseStatus for backward compatibility
export function toFileUrl(filePath: string): string {
  if (!filePath) return "";
  if (filePath.startsWith("file:///")) return filePath;
  let normalized = filePath.replace(/\\/g, "/");
  if (/^[a-zA-Z]:/.test(normalized) || !normalized.startsWith("/")) {
    normalized = `/${normalized}`;
  }
  return `file://${encodeURI(normalized)}`;
}

export function getConciseStatus(status?: string | null): string | null {
  return resolveStatusCategory(status);
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
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [actionMessage, setActionMessage] = useState<string | null>(null);
  const [configuredRoot, setConfiguredRoot] = useState<string | null>(repoPath ?? null);
  const [locationLoaded, setLocationLoaded] = useState<boolean>(!!repoPath);

  // Surface-scoped project-plan slots state (1 or 2 slots)
  const [slots, setSlots] = useState<ProjectPlanSlot[]>([
    { slot_id: "primary", repo_path: null, plan_path: activePlanPath ?? null },
  ]);
  const [slotDetails, setSlotDetails] = useState<Record<string, PlanFileDetails | null>>({});
  const activeSlotIdsRef = useRef<Set<string>>(new Set(["primary"]));
  const slotsRef = useRef<ProjectPlanSlot[]>(slots);
  useEffect(() => {
    slotsRef.current = slots;
  }, [slots]);

  // Sequenced slot save queue to prevent out-of-order writes or recreation after remove
  const slotSaveQueueRef = useRef<Promise<void>>(Promise.resolve());
  const slotSaveSequenceRef = useRef(0);

  const persistSlots = (explicitSlots?: ProjectPlanSlot[]) => {
    const sequence = ++slotSaveSequenceRef.current;
    if (explicitSlots) {
      slotsRef.current = explicitSlots;
    }
    slotSaveQueueRef.current = slotSaveQueueRef.current.catch(() => undefined).then(async () => {
      if (sequence !== slotSaveSequenceRef.current) return;
      const currentSlots = slotsRef.current.filter((s) => activeSlotIdsRef.current.has(s.slot_id));
      try {
        await invoke("set_project_plan_slots", { surface, slots: currentSlots });
      } catch (e) {
        if (sequence === slotSaveSequenceRef.current) {
          setError(String(e));
        }
      }
    });
    return slotSaveQueueRef.current;
  };

  useEffect(() => {
    if (repoPath) {
      setConfiguredRoot(repoPath);
      setLocationLoaded(true);
      return;
    }
    let cancelled = false;
    void invoke<string | null>("get_plan_manager_location", { surface })
      .then((loc) => {
        if (!cancelled) {
          setConfiguredRoot(loc);
          setLocationLoaded(true);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setError(String(e));
          setLocationLoaded(true);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [repoPath, surface]);

  const normalizePathForCompare = (p: string | null | undefined): string => {
    if (!p) return "";
    return normalizeDisplayPath(p).replace(/\\/g, "/").replace(/\/+/g, "/").toLowerCase();
  };

  // Match active workspace (repoPath) to the corresponding slot
  const matchedSlot = useMemo(() => {
    if (!repoPath) {
      return slots[0] || null;
    }
    const normTarget = normalizePathForCompare(repoPath);
    const found = slots.find((s) => s.repo_path && normalizePathForCompare(s.repo_path) === normTarget);
    if (found) return found;
    // If primary slot has no repo configured yet, associate with it
    if (slots.length === 1 && !slots[0].repo_path) {
      return slots[0];
    }
    // Ambiguous / unmatched workspace root: do NOT guess another slot's plan!
    return null;
  }, [repoPath, slots]);

  const isUnmatchedWorkspace = !!repoPath && !matchedSlot;
  const activeRepo = matchedSlot ? (matchedSlot.repo_path || repoPath || null) : null;
  const operationalSlotId = matchedSlot?.slot_id || "primary";

  // Notify parent of null plan if workspace is unmatched
  useEffect(() => {
    if (isUnmatchedWorkspace) {
      onActivePlanChangedRef.current?.(null);
    }
  }, [isUnmatchedWorkspace]);

  // Load project-plan slots on mount or surface change
  useEffect(() => {
    let cancelled = false;
    void invoke<ProjectPlanSlot[]>("get_project_plan_slots", { surface })
      .then((fetchedSlots) => {
        if (cancelled) return;
        if (fetchedSlots && Array.isArray(fetchedSlots) && fetchedSlots.length > 0) {
          setSlots(fetchedSlots);
          slotsRef.current = fetchedSlots;
          activeSlotIdsRef.current = new Set(fetchedSlots.map((s) => s.slot_id));
        } else {
          const initial: ProjectPlanSlot[] = [
            {
              slot_id: "primary",
              repo_path: null,
              plan_path: activePlanPath || null,
            },
          ];
          setSlots(initial);
          slotsRef.current = initial;
          activeSlotIdsRef.current = new Set(["primary"]);
        }
      })
      .catch(() => {
        if (cancelled) return;
        const initial: ProjectPlanSlot[] = [
          {
            slot_id: "primary",
            repo_path: null,
            plan_path: activePlanPath || null,
          },
        ];
        setSlots(initial);
        slotsRef.current = initial;
        activeSlotIdsRef.current = new Set(["primary"]);
      });
    return () => {
      cancelled = true;
    };
  }, [surface]);

  // Keep role assignments scoped to this repository and Autocode surface.
  const [currentPlannerRole, setCurrentPlannerRole] = useState<string | null>(plannerRole || null);
  const [currentReviserRole, setCurrentReviserRole] = useState<string | null>(reviserRole || null);
  const roleAssignmentRef = useRef({ plannerRole: plannerRole || null, reviserRole: reviserRole || null });
  const roleSaveQueueRef = useRef<Promise<void>>(Promise.resolve());
  const roleSaveSequenceRef = useRef(0);
  const onRolesChangedRef = useRef(onRolesChanged);
  useEffect(() => { onRolesChangedRef.current = onRolesChanged; }, [onRolesChanged]);

  useEffect(() => {
    if (!locationLoaded || !activeRepo) return;
    let cancelled = false;
    void invoke<{ planner_id: string | null; reviser_id: string | null }>("get_plan_role_assignment", { surface, repoPath: activeRepo })
      .then((assignment) => {
        if (cancelled) return;
        setCurrentPlannerRole(assignment.planner_id);
        setCurrentReviserRole(assignment.reviser_id);
        roleAssignmentRef.current = { plannerRole: assignment.planner_id, reviserRole: assignment.reviser_id };
        onRolesChangedRef.current?.({
          plannerRole: assignment.planner_id,
          reviserRole: assignment.reviser_id,
        });
      })
      .catch((e) => { if (!cancelled) setError(String(e)); });
    return () => { cancelled = true; };
  }, [surface, activeRepo, locationLoaded]);

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

  const [currentPlanReviewPolicy, setCurrentPlanReviewPolicy] = useState<ReviewerPolicy>(
    planReviewPolicy || "antigravity"
  );
  const [currentPlanReviewers, setCurrentPlanReviewers] = useState<string[] | null>(
    planReviewers !== undefined ? planReviewers : null
  );

  useEffect(() => {
    if (planReviewPolicy) {
      setCurrentPlanReviewPolicy(planReviewPolicy);
    }
  }, [planReviewPolicy]);

  useEffect(() => {
    setCurrentPlanReviewers(planReviewers !== undefined ? planReviewers : null);
  }, [planReviewers]);

  const loadPlans = useCallback(async (folderOverride?: string | null, repoOverride?: string | null) => {
    const targetRepo = repoOverride ?? activeRepo;
    if (!targetRepo) return;
    try {
      setLoading(true);
      setError(null);
      const targetFolder = folderOverride !== undefined ? folderOverride : selectedFolderRef.current;
      const res = await invoke<PlanFileDetails>("list_available_plans", {
        repoPath: targetRepo,
        folderPath: targetFolder ?? null,
        surface,
        slotId: operationalSlotId,
      });
      setDetails(res);
      setSlotDetails((prev) => ({ ...prev, primary: res }));

      const loadedActive = res.active_plan_relative_path ?? null;
      if (loadedActive && !selectedFolderRef.current && folderOverride === undefined) {
        const derivedFolder = loadedActive.includes("/")
          ? loadedActive.substring(0, loadedActive.lastIndexOf("/"))
          : ".";
        setSelectedFolder(derivedFolder);
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
  }, [activeRepo, surface]);

  const prevRepoRef = useRef<string | null>(activeRepo);
  useEffect(() => {
    if (!locationLoaded) return;
    if (prevRepoRef.current !== activeRepo) {
      prevRepoRef.current = activeRepo;
      setSelectedFolder(null);
      selectedFolderRef.current = null;
      setDetails(null);
    }
    if (!activeRepo) {
      setDetails(null);
      setLoading(false);
      return;
    }
    void loadPlans();
  }, [locationLoaded, activeRepo, loadPlans]);

  // Load secondary slot details when secondary slot has a repo
  useEffect(() => {
    const secondarySlot = slots.find((s) => s.slot_id === "secondary");
    if (!secondarySlot?.repo_path) return;
    let cancelled = false;
    void invoke<PlanFileDetails>("list_available_plans", {
      repoPath: secondarySlot.repo_path,
      surface,
      slotId: "secondary",
    })
      .then((res) => {
        if (!cancelled && activeSlotIdsRef.current.has("secondary")) {
          setSlotDetails((prev) => ({ ...prev, secondary: res }));
        }
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [slots, surface]);

  const handleManualRefresh = async () => {
    setRefreshing(true);
    try {
      if (selectedFolder) {
        await loadPlans(selectedFolder);
      } else {
        await loadPlans();
      }
      const secondarySlot = slotsRef.current.find((s) => s.slot_id === "secondary");
      if (secondarySlot?.repo_path) {
        const res = await invoke<PlanFileDetails>("list_available_plans", {
          repoPath: secondarySlot.repo_path,
          surface,
          slotId: "secondary",
        });
        if (activeSlotIdsRef.current.has("secondary")) {
          setSlotDetails((prev) => ({ ...prev, secondary: res }));
        }
      }
    } finally {
      setRefreshing(false);
    }
  };

  const handleAddSlot = () => {
    if (slotsRef.current.length >= 2) return;
    const newSlot: ProjectPlanSlot = {
      slot_id: "secondary",
      repo_path: null,
      plan_path: null,
    };
    const nextSlots = [...slotsRef.current, newSlot];
    activeSlotIdsRef.current.add("secondary");
    setSlots(nextSlots);
    slotsRef.current = nextSlots;
    void persistSlots(nextSlots);
  };

  const handleRemoveSecondarySlot = () => {
    // Only remove association in settings, do NOT delete or modify underlying project/plan files!
    activeSlotIdsRef.current.delete("secondary");
    const nextSlots = slotsRef.current.filter((s) => s.slot_id !== "secondary");
    setSlots(nextSlots);
    slotsRef.current = nextSlots;
    setSlotDetails((prev) => {
      const copy = { ...prev };
      delete copy.secondary;
      return copy;
    });
    void persistSlots(nextSlots);
  };

  const handleSelectPlanFile = async (slotId: string) => {
    try {
      setError(null);
      const targetSlot = slotsRef.current.find((s) => s.slot_id === slotId);
      const slotRepo = targetSlot?.repo_path || null;

      const dialogArgs: Record<string, unknown> = {
        repoPath: slotRepo || undefined,
        surface,
        slotId,
        dialogTitle: t("planManager.selectPlanFile") || "プランファイルを選択",
      };

      const picked = await invoke<SelectedPlanResult | string | null>("select_plan_file_dialog", dialogArgs);
      if (!picked) return;
      if (!activeSlotIdsRef.current.has(slotId)) return; // Stale slot check

      // Validate returned slot_id and surface to prevent cross-row or cross-surface stale misassignment
      if (typeof picked !== "string") {
        if (picked.slot_id && picked.slot_id !== slotId) return;
        if (picked.surface && picked.surface !== surface) return;
      }

      const pickedRel = typeof picked === "string" ? picked : picked.relative_path;
      const pickedRepo = typeof picked === "string" ? slotRepo : picked.repo_path;

      if (!pickedRepo) {
        setError("Cannot determine project repository for selected plan.");
        return;
      }

      const res = await invoke<PlanFileDetails>("set_active_plan", {
        relativePath: pickedRel,
        repoPath: pickedRepo,
        surface,
        slotId,
      });

      if (!activeSlotIdsRef.current.has(slotId)) return; // Stale slot check

      const nextSlots = slotsRef.current.map((s) =>
        s.slot_id === slotId
          ? { ...s, repo_path: pickedRepo, plan_path: pickedRel }
          : s
      );
      setSlots(nextSlots);
      slotsRef.current = nextSlots;
      void persistSlots(nextSlots);

      setSlotDetails((prev) => ({ ...prev, [slotId]: res }));
      if (slotId === operationalSlotId) {
        const parentFolder = pickedRel.includes("/") ? pickedRel.substring(0, pickedRel.lastIndexOf("/")) : ".";
        setDetails(res);
        setSelectedFolder(parentFolder);
        selectedFolderRef.current = parentFolder;
        if (onActivePlanChangedRef.current) {
          onActivePlanChangedRef.current(pickedRel);
        }
        prevRepoRef.current = pickedRepo;
        await loadPlans(parentFolder, pickedRepo);
      }

      setActionMessage(`${t("planManager.planActivated") || "計画書を設定しました"}: ${pickedRel.split("/").pop() || pickedRel}`);
      setTimeout(() => setActionMessage(null), 3000);
    } catch (e: unknown) {
      setError(String(e));
    }
  };

  const handleOpenPlan = async (slotId: string) => {
    const operationalPath = getSlotOperationalPath(slotId);
    if (!operationalPath) return;
    try {
      await invoke("open_path", { path: operationalPath });
    } catch (e: unknown) {
      setError(String(e));
    }
  };

  const getSlotOperationalPath = (slotId: string): string => {
    const slot = slots.find((s) => s.slot_id === slotId);
    if (!slot) return "";
    const slotRepo = slot.repo_path || (slotId === "primary" ? activeRepo : null);
    const slotRel = slot.plan_path || (slotId === "primary" ? (details?.active_plan_relative_path || activePlanPath) : null);
    if (!slotRel) return "";

    const slotDet = slotDetails[slotId] || (slotId === "primary" ? details : null);
    const entry = slotDet?.plans?.find((p) => p.relative_path === slotRel);
    if (entry?.path) return entry.path;

    if (slotRepo && slotRel) {
      return slotRepo.includes("\\")
        ? `${slotRepo}\\${slotRel.replace(/\//g, "\\")}`
        : `${slotRepo}/${slotRel}`;
    }
    return slotRel;
  };

  const persistRoleAssignment = async (plannerRole: string | null, reviserRole: string | null) => {
    if (!activeRepo) return;
    setError(null);
    const target = { plannerRole, reviserRole };
    roleAssignmentRef.current = target;
    setCurrentPlannerRole(plannerRole);
    setCurrentReviserRole(reviserRole);
    const sequence = ++roleSaveSequenceRef.current;
    roleSaveQueueRef.current = roleSaveQueueRef.current.catch(() => undefined).then(async () => {
      try {
        await invoke("set_plan_role_assignment", {
          surface,
          repoPath: activeRepo,
          assignment: { planner_id: target.plannerRole, reviser_id: target.reviserRole },
        });
        if (sequence === roleSaveSequenceRef.current) setError(null);
      } catch (e) {
        if (sequence !== roleSaveSequenceRef.current) return;
        setError(String(e));
        try {
          const stored = await invoke<{ planner_id: string | null; reviser_id: string | null }>("get_plan_role_assignment", {
            surface,
            repoPath: activeRepo,
          });
          if (sequence === roleSaveSequenceRef.current) {
            roleAssignmentRef.current = { plannerRole: stored.planner_id, reviserRole: stored.reviser_id };
            setCurrentPlannerRole(stored.planner_id);
            setCurrentReviserRole(stored.reviser_id);
          }
        } catch (reloadError) {
          if (sequence === roleSaveSequenceRef.current) setError(`${String(e)}; failed to reload saved role assignment: ${String(reloadError)}`);
        }
      }
    });
    await roleSaveQueueRef.current;
  };

  const handlePlannerRoleChange = (role: string | null) => {
    void persistRoleAssignment(role, roleAssignmentRef.current.reviserRole);
    if (onRolesChanged) {
      onRolesChanged({ plannerRole: role });
    }
  };

  const handleReviserRoleChange = (role: string | null) => {
    void persistRoleAssignment(roleAssignmentRef.current.plannerRole, role);
    if (onRolesChanged) {
      onRolesChanged({ reviserRole: role });
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

  // Operational slot details for header chip
  const primaryRelPath = matchedSlot?.plan_path || (operationalSlotId === "primary" ? details?.active_plan_relative_path : null) || activePlanPath;
  const primaryDetails = slotDetails[operationalSlotId] || details;
  const primaryEntry = primaryDetails?.plans?.find((p) => p.relative_path === primaryRelPath);
  const primaryLocalizedStatusLabel = getLocalizedStatusLabel(primaryEntry?.status, t);

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
      {isUnmatchedWorkspace && (
        <div
          data-testid="unmatched-workspace-warning"
          style={{
            padding: "8px 12px",
            marginBottom: 8,
            backgroundColor: "#fff8c5",
            border: "1px solid #d4a72c",
            borderRadius: 6,
            fontSize: 12,
            color: "#573a08",
            display: "flex",
            alignItems: "center",
            gap: 6,
          }}
        >
          <span aria-hidden="true">⚠️</span>
          <span>
            {t("planManager.unmatchedWorkspaceWarning") ||
              `現在の作業フォルダ (${repoPath}) に対応するスロットがありません。プランファイルを選択して関連付けてください。`}
          </span>
        </div>
      )}

      {/* ── ROW 1: Heading, Concise Localized Status Chip & Refresh ── */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          gap: 8,
          marginBottom: 10,
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0 }}>
          <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--color-text-primary, #111827)", flexShrink: 0 }}>
            {t("planManager.title") || "プランファイル"}
          </h3>

          {primaryRelPath ? (
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
                whiteSpace: "nowrap",
              }}
              title={primaryLocalizedStatusLabel}
              aria-label={primaryLocalizedStatusLabel}
            >
              [{primaryLocalizedStatusLabel}]
            </span>
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
          style={{ fontSize: 11, padding: "2px 8px", flexShrink: 0, whiteSpace: "nowrap" }}
        >
          {refreshing ? t("planManager.loading") || "更新中..." : `⟳ ${t("planManager.refresh") || "再読込"}`}
        </button>
      </div>

      {/* ── PROJECT-PLAN SLOTS (Permanent Row 1, Optional Row 2) ── */}
      <div style={{ display: "flex", flexDirection: "column", gap: 10, marginBottom: 12 }}>
        {slots.map((slot) => {
          const isPrimary = slot.slot_id === "primary";
          const slotRepo = slot.repo_path || (isPrimary ? activeRepo : null);
          const slotRel = slot.plan_path || (isPrimary ? primaryRelPath : null);
          const slotDet = slotDetails[slot.slot_id] || (isPrimary ? details : null);
          const slotEntry = isPrimary
            ? primaryEntry
            : slotDet?.plans?.find(
                (p) =>
                  p.relative_path === slotRel ||
                  (slotRel && p.relative_path.replace(/\\/g, "/") === slotRel.replace(/\\/g, "/"))
              );
          const slotOperational = getSlotOperationalPath(slot.slot_id);
          const slotDisplay = normalizeDisplayPath(slotOperational);
          const slotRepoDisplay = normalizeDisplayPath(slotRepo);
          const slotOpenPlanAriaLabel = slotEntry?.title
            ? `${slotDisplay} — ${slotEntry.title}`
            : slotDisplay;
          const slotOpenPlanTitle = slotEntry?.title
            ? `${slotDisplay} — ${slotEntry.title} (${t("planManager.openPlanTooltip") || "エディタで開く"})`
            : `${slotDisplay} (${t("planManager.openPlanTooltip") || "エディタで開く"})`;
          const slotLabel = isPrimary
            ? (t("planManager.slotPrimary") || "プロジェクト 1")
            : (t("planManager.slotSecondary") || "プロジェクト 2");

          return (
            <div
              key={slot.slot_id}
              data-testid={`project-plan-slot-${slot.slot_id}`}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                minWidth: 0,
              }}
            >
              {/* Slot Label */}
              <span
                data-testid={`slot-label-${slot.slot_id}`}
                style={{
                  fontSize: 12,
                  fontWeight: 600,
                  color: "var(--color-text-secondary, #4b5563)",
                  whiteSpace: "nowrap",
                  flexShrink: 0,
                }}
              >
                {slotLabel}:
              </span>

              {/* Bordered Read-Only Static Path Field */}
              <div
                data-testid={isPrimary ? "active-plan-path-container" : `active-plan-path-container-${slot.slot_id}`}
                style={{
                  flex: "1 1 0%",
                  minWidth: 0,
                  display: "flex",
                  alignItems: "center",
                }}
              >
                {slotDisplay ? (
                  <div
                    data-testid={isPrimary ? "active-plan-path-display" : `active-plan-path-display-${slot.slot_id}`}
                    title={slotRepoDisplay ? `${slotRepoDisplay}/${slotDisplay}` : slotDisplay}
                    aria-label={slotOpenPlanAriaLabel}
                    style={{
                      flex: 1,
                      minWidth: 0,
                      maxWidth: "100%",
                      boxSizing: "border-box",
                      display: "block",
                      padding: "4px 10px",
                      fontSize: 12,
                      fontFamily: "monospace",
                      color: "var(--color-text-primary, #111827)",
                      backgroundColor: "#ffffff",
                      border: "1px solid #d0d7de",
                      borderRadius: 6,
                      textAlign: "left",
                      textDecoration: "none",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                      userSelect: "text",
                      cursor: "text",
                      lineHeight: "1.5",
                    }}
                  >
                    {slotDisplay}
                  </div>
                ) : (
                  <div
                    data-testid={isPrimary ? "empty-plan-path-field" : `empty-plan-path-field-${slot.slot_id}`}
                    title={slotRepoDisplay || undefined}
                    style={{
                      flex: 1,
                      minWidth: 0,
                      maxWidth: "100%",
                      boxSizing: "border-box",
                      padding: "4px 10px",
                      fontSize: 12,
                      color: "#8c959f",
                      backgroundColor: "#f6f8fa",
                      border: "1px solid #d0d7de",
                      borderRadius: 6,
                      fontStyle: "italic",
                      whiteSpace: "nowrap",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      lineHeight: "1.5",
                    }}
                  >
                    {t("planManager.emptyPlanPlaceholder") || "計画書が選択されていません"}
                  </div>
                )}
              </div>

              {/* Plan File Picker Button (no leading icon) */}
              <button
                type="button"
                className="btn btn-small"
                onClick={() => void handleSelectPlanFile(slot.slot_id)}
                data-testid={isPrimary ? "btn-select-plan-file" : `btn-select-plan-file-${slot.slot_id}`}
                style={{
                  display: "inline-flex",
                  alignItems: "center",
                  gap: 4,
                  flexShrink: 0,
                  padding: "4px 10px",
                  fontSize: 12,
                  whiteSpace: "nowrap",
                }}
                title={t("planManager.selectPlanFile") || "プランファイルを選択"}
              >
                <span>
                  {t("planManager.planFileButtonLabel") || "プランファイル選択"}
                </span>
              </button>

              {/* Dedicated Open Plan File Button */}
              <button
                type="button"
                className="btn btn-small"
                onClick={() => void handleOpenPlan(slot.slot_id)}
                disabled={!slotDisplay}
                data-testid={isPrimary ? "btn-open-plan" : `btn-open-plan-${slot.slot_id}`}
                style={{
                  display: "inline-flex",
                  alignItems: "center",
                  gap: 4,
                  flexShrink: 0,
                  padding: "4px 12px",
                  fontSize: 12,
                  whiteSpace: "nowrap",
                }}
                title={slotOpenPlanTitle}
                aria-label={slotOpenPlanAriaLabel}
              >
                <span>{t("planManager.btnOpenPlanFile") || "プランファイルを開く"}</span>
              </button>

              {/* Secondary slot removal button */}
              {!isPrimary && (
                <button
                  type="button"
                  className="btn btn-small"
                  onClick={() => handleRemoveSecondarySlot()}
                  data-testid="btn-remove-slot-secondary"
                  title={t("planManager.removeSlotTooltip") || "この行の関連付けのみを解除します（実ファイルは削除されません）"}
                  style={{
                    fontSize: 12,
                    padding: "4px 10px",
                    whiteSpace: "nowrap",
                    color: "#dc2626",
                    flexShrink: 0,
                  }}
                >
                  {t("planManager.removeSlot") || "削除"}
                </button>
              )}
            </div>
          );
        })}

        {/* Add Slot Button (Max 2 rows) */}
        {slots.length < 2 && (
          <div style={{ marginTop: 2 }}>
            <button
              type="button"
              className="btn btn-small"
              onClick={() => handleAddSlot()}
              data-testid="btn-add-slot"
              style={{ fontSize: 11, padding: "2px 8px", whiteSpace: "nowrap" }}
            >
              + {t("planManager.addSlot") || "プロジェクトを追加"}
            </button>
          </div>
        )}
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
              if (optVal === "antigravity") return current === "antigravity";
              if (optVal === "codex") return current === "codex" || current === "codex_session";
            }
            return current === optVal;
          };

          const roleOptions = surface === "codex"
            ? [
                { value: "", labelKey: "planManager.roleNone", defaultLabel: "Unselected" },
                { value: "codex_session", labelKey: "planManager.roleCodexSession", defaultLabel: "Codex (Session)" },
              ]
            : [
                { value: "", labelKey: "planManager.roleNone", defaultLabel: "Unselected" },
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
                    const selected = opt.value === "" ? !currentPlannerRole : isSelectedRole(currentPlannerRole || "", opt.value);
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
                          onChange={() => handlePlannerRoleChange(opt.value || null)}
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
                    <span>{t("planManager.planReviewInherit") || "実装レビュー設定"}</span>
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
                      <span>Antigravity</span>
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
                    <span>{surface === "codex" ? (t("planManager.roleCodexSession") || "Codex (Session)") : "Codex CLI"}</span>
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
                    <span>DeepSeek Harness</span>
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
                    const selected = opt.value === "" ? !currentReviserRole : isSelectedRole(currentReviserRole || "", opt.value);
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
                          onChange={() => handleReviserRoleChange(opt.value || null)}
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
