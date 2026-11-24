import React, { useState, useEffect, useCallback, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type {
  PlanFileDetails,
  ActivePlanFileDetails,
  ReviewerPolicy,
  ProjectPlanSlot,
  SelectedPlanResult,
  SurfacePlanSlotsView,
  PinnedWorkflowTarget,
  ActivePlanContext,
  ProjectRegistryView,
  WorkflowRoleAssignmentView,
} from "../types";
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
  onActivePlanContextChanged?: (context: ActivePlanContext | null) => void;
  onRolesChanged?: (roles: {
    plannerRole?: string | null;
    reviserRole?: string | null;
    planReviewPolicy?: ReviewerPolicy;
    planReviewers?: string[] | null;
  }) => void;
  onPlanReviewPolicyChanged?: (policy: ReviewerPolicy) => void;
  onPlanReviewersChanged?: (reviewers: string[] | null) => void;
  onPinnedWorkflowTargetChanged?: (target: PinnedWorkflowTarget | null) => void;
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

function projectPlanPathForSurface(
  project: ProjectRegistryView["projects"][number],
  surface: "codex" | "antigravity",
): string | null {
  const paths = project.plan_paths;
  if (paths && Object.prototype.hasOwnProperty.call(paths, surface)) {
    return paths[surface] ?? null;
  }
  return project.plan_path ?? null;
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
  planReviewPolicy,
  planReviewers,
  embedded = false,
  onActivePlanChanged,
  onActivePlanContextChanged,
  onRolesChanged,
  onPlanReviewPolicyChanged,
  onPlanReviewersChanged,
  onPinnedWorkflowTargetChanged,
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
  const [selectedPlanRepoRoot, setSelectedPlanRepoRoot] = useState<string | null>(null);
  const [selectedPlanSlotId, setSelectedPlanSlotId] = useState<string | null>(null);
  const [selectedPlanContextResolved, setSelectedPlanContextResolved] = useState(false);
  const [selectedPlanContextLoaded, setSelectedPlanContextLoaded] = useState<boolean>(!!repoPath);
  const [slotsLoaded, setSlotsLoaded] = useState<boolean>(false);
  const [registryView, setRegistryView] = useState<ProjectRegistryView | null>(null);
  const [registryLoaded, setRegistryLoaded] = useState(false);
  const registryViewRef = useRef<ProjectRegistryView | null>(null);
  const isManagementView = !repoPath && (!registryLoaded || !!registryView);
  const userExplicitSlotRef = useRef<boolean>(false);

  // Legacy per-repository slot state. Registry-backed management rows are unbounded.
  const [slots, setSlots] = useState<ProjectPlanSlot[]>([
    { slot_id: "primary", repo_path: repoPath ?? null, plan_path: repoPath ? (activePlanPath ?? null) : null },
  ]);
  const [slotDetails, setSlotDetails] = useState<Record<string, PlanFileDetails | null>>({});
  const [canonicalSlotView, setCanonicalSlotView] = useState<SurfacePlanSlotsView | null>(null);
  const [selectedActiveSlotId, setSelectedActiveSlotId] = useState<string | null>(null);
  const [mirrorWarning, setMirrorWarning] = useState<string | null>(null);
  const [pinnedWorkflowTarget, setPinnedWorkflowTarget] = useState<PinnedWorkflowTarget | null>(null);
  const [pendingRemoveSlotConfirm, setPendingRemoveSlotConfirm] = useState<{
    slotId: string;
    fallbackSlotId: string;
    fallbackPlanPath: string | null;
  } | null>(null);
  const activeSlotIdsRef = useRef<Set<string>>(new Set(["primary"]));
  const slotsRef = useRef<ProjectPlanSlot[]>(slots);

  const syncSlotsFromRegistry = (reg: ProjectRegistryView) => {
    registryViewRef.current = reg;
    const synced: ProjectPlanSlot[] = reg.projects
      .filter((p) => !!projectPlanPathForSurface(p, surface))
      .map((p) => ({
        slot_id: p.project_id,
        repo_path: normalizeDisplayPath(p.repo_root),
        plan_path: projectPlanPathForSurface(p, surface),
      }));
    setSlots(synced);
    slotsRef.current = synced;
    activeSlotIdsRef.current = new Set(synced.map((s) => s.slot_id));
  };

  const broadcastRegistryUpdate = (reg: ProjectRegistryView) => {
    registryViewRef.current = reg;
    setRegistryView(reg);
    syncSlotsFromRegistry(reg);
    if (reg.mirror_sync_error) {
      setError(`Registry saved, but repo .active-plan.json mirror sync failed (retry/resync before starting workflow): ${reg.mirror_sync_error}`);
    }
    window.dispatchEvent(new CustomEvent<ProjectRegistryView>("poverello:project-registry-updated", { detail: reg }));
  };

  useEffect(() => {
    let cancelled = false;
    void invoke<ProjectRegistryView | undefined>("get_project_registry")
      .then((reg) => {
        if (cancelled) return;
        if (reg && typeof reg === "object" && Array.isArray(reg.projects)) {
          setRegistryView(reg);
          syncSlotsFromRegistry(reg);
          if (reg.mirror_sync_error) {
            setError(`Registry saved, but repo .active-plan.json mirror sync failed (retry/resync before starting workflow): ${reg.mirror_sync_error}`);
          }
        }
        setRegistryLoaded(true);
      })
      .catch(() => {
        if (!cancelled) setRegistryLoaded(true);
      });
    const onRegistryUpdated = (event: Event) => {
      const custom = event as CustomEvent<ProjectRegistryView>;
      const reg = custom.detail;
      if (!reg || typeof reg !== "object" || !Array.isArray(reg.projects)) return;
      setRegistryView(reg);
      syncSlotsFromRegistry(reg);
    };
    window.addEventListener("poverello:project-registry-updated", onRegistryUpdated);
    return () => {
      cancelled = true;
      window.removeEventListener("poverello:project-registry-updated", onRegistryUpdated);
    };
  }, [surface]);

  const handleResolveMigrationConflict = async (
    conflictId: string,
    chosenPlanPath: string | null
  ) => {
    try {
      setError(null);
      const updated = await invoke<ProjectRegistryView>("resolve_project_migration_conflict", {
        conflictId,
        chosenPlanPath,
      });
      broadcastRegistryUpdate(updated);
    } catch (e) {
      setError(String(e));
    }
  };

  const replaceProjectPlanAssociation = async (args: {
    sourceProjectId: string;
    targetRepo: string;
    planPath: string;
  }) => {
    try {
      setError(null);
      const updated = await invoke<ProjectRegistryView>("replace_project_plan_association", {
        sourceProjectId: args.sourceProjectId,
        targetRepoPath: args.targetRepo,
        targetPlanPath: args.planPath,
        surface,
      });
      if (!updated || !Array.isArray(updated.projects)) throw new Error(t("planManager.planSelectionFailed") || "The registry update could not be verified.");
      const target = updated.projects.find((project) => normalizePathForCompare(project.repo_root) === normalizePathForCompare(args.targetRepo));
      const source = updated.projects.find((project) => project.project_id === args.sourceProjectId);
      if (
        !target ||
        projectPlanPathForSurface(target, surface)?.replace(/\\/g, "/") !== args.planPath ||
        (source && projectPlanPathForSurface(source, surface) !== null)
      ) {
        throw new Error(t("planManager.planFileAssociationMismatch") || "The replacement plan association does not match the selected file.");
      }
      broadcastRegistryUpdate(updated);
      setActionMessage(`${t("planManager.planAssociationSwitched") || "Plan association switched"}: ${args.planPath}`);
      setTimeout(() => setActionMessage(null), 3000);
    } catch (e) {
      setError(`${t("planManager.planSelectionFailed") || "Could not save the plan selection. Existing project entries were kept; verify the file is inside a repository and try again."} (${String(e)})`);
      try {
        const refreshed = await invoke<ProjectRegistryView>("get_project_registry");
        if (refreshed && Array.isArray(refreshed.projects)) broadcastRegistryUpdate(refreshed);
      } catch { /* Keep the last verified registry view. */ }
    }
  };
  useEffect(() => {
    slotsRef.current = slots;
  }, [slots]);

  // Sequenced slot save queue to prevent out-of-order writes or recreation after remove
  const slotSaveQueueRef = useRef<Promise<void>>(Promise.resolve());
  const slotSaveSequenceRef = useRef(0);

  const persistSlots = (
    explicitSlots?: ProjectPlanSlot[],
    activeOverride?: { activeSlotId?: string | null; activeRepoPath?: string | null },
  ) => {
    const sequence = ++slotSaveSequenceRef.current;
    if (explicitSlots) {
      slotsRef.current = explicitSlots;
    }
    slotSaveQueueRef.current = slotSaveQueueRef.current.catch(() => undefined).then(async () => {
      if (sequence !== slotSaveSequenceRef.current) return;
      const currentSlots = slotsRef.current.filter((s) => activeSlotIdsRef.current.has(s.slot_id));
      try {
        const payload: Record<string, unknown> = { surface, slots: currentSlots };
        if (activeOverride?.activeSlotId) payload.activeSlotId = activeOverride.activeSlotId;
        if (activeOverride?.activeRepoPath) payload.activeRepoPath = activeOverride.activeRepoPath;
        await invoke("set_project_plan_slots", payload);
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

  useEffect(() => {
    if (repoPath) {
      setSelectedPlanContextLoaded(true);
      setSelectedPlanRepoRoot(normalizeDisplayPath(repoPath));
      return;
    }
    if (!registryLoaded) {
      setSelectedPlanContextLoaded(false);
      return;
    }
    if (registryView) {
      // Registry management is not a running-task context. Do not ask for or
      // infer a global active project when project records are authoritative.
      setSelectedPlanContextLoaded(true);
      setSelectedPlanContextResolved(false);
      setSelectedPlanRepoRoot(null);
      setSelectedPlanSlotId(null);
      return;
    }
    let cancelled = false;
    setSelectedPlanContextLoaded(false);
    void invoke<ActivePlanContext>("get_selected_project_plan_context", { surface })
      .then((context) => {
        if (cancelled) return;
        const resolved = !!context?.resolved && !!context.repoRoot;
        setSelectedPlanContextResolved(resolved);
        setSelectedPlanRepoRoot(resolved ? context.repoRoot : null);
        setSelectedPlanSlotId(resolved ? (context.slotId ?? null) : null);
        setSelectedPlanContextLoaded(true);
      })
      .catch(() => {
        if (cancelled) return;
        setSelectedPlanContextResolved(false);
        setSelectedPlanRepoRoot(null);
        setSelectedPlanSlotId(null);
        setSelectedPlanContextLoaded(true);
      });
    return () => { cancelled = true; };
  }, [repoPath, registryLoaded, registryView, surface]);

  // Match active workspace (repoPath) to the corresponding slot
  const matchedSlot = useMemo(() => {
    if (!repoPath) {
      if (!selectedPlanContextLoaded || !slotsLoaded) return null;
      if (registryView) return null;
      if (selectedPlanRepoRoot) {
        const repoMatches = slots.filter(
          (s) => s.repo_path && normalizePathForCompare(s.repo_path) === normalizePathForCompare(selectedPlanRepoRoot)
        );
        if (repoMatches.length > 0) {
          const exact = selectedPlanSlotId ? repoMatches.find((s) => s.slot_id === selectedPlanSlotId) : null;
          return exact || repoMatches[0];
        }
      }
      if (selectedActiveSlotId) {
        const activeMatch = slots.find((s) => s.slot_id === selectedActiveSlotId);
        if (activeMatch) return activeMatch;
      }
      if (slots.length === 1) return slots[0] || null;
      return null;
    }
    const normTarget = normalizePathForCompare(repoPath);
    const matches = slots.filter((s) => s.repo_path && normalizePathForCompare(s.repo_path) === normTarget);
    if (matches.length > 0) {
      if (selectedActiveSlotId) {
        const activeMatch = matches.find((s) => s.slot_id === selectedActiveSlotId);
        if (activeMatch) return activeMatch;
      }
      return matches[0];
    }
    if (selectedPlanRepoRoot) {
      const planRepoMatch = slots.find(
        (s) => s.repo_path && normalizePathForCompare(s.repo_path) === normalizePathForCompare(selectedPlanRepoRoot)
      );
      if (planRepoMatch) return planRepoMatch;
    }
    // If primary slot has no repo configured yet, associate with it
    if (slots.length === 1 && !slots[0].repo_path) {
      return slots[0];
    }
    // Ambiguous / unmatched workspace root: do NOT guess another slot's plan!
    return null;
  }, [repoPath, slots, slotsLoaded, selectedPlanContextLoaded, selectedActiveSlotId, selectedPlanRepoRoot, selectedPlanSlotId]);

  const isUnmatchedWorkspace = !!repoPath && !matchedSlot;
  const isUnresolvedProjectContext =
    !repoPath && !registryView && selectedPlanContextLoaded && slotsLoaded && !matchedSlot && slots.length > 1;
  const activeRepo = matchedSlot ? (matchedSlot.repo_path || repoPath || null) : null;
  const canonicalActiveSlotForRepo =
    canonicalSlotView &&
    activeRepo &&
    normalizePathForCompare(canonicalSlotView.repo_root) === normalizePathForCompare(activeRepo)
      ? canonicalSlotView.active_slot_id ?? null
      : null;
  const operationalSlotId =
    canonicalActiveSlotForRepo || selectedActiveSlotId || matchedSlot?.slot_id || "primary";

  // Notify parent of null plan if workspace is unmatched
  useEffect(() => {
    if (isUnmatchedWorkspace) {
      onActivePlanChangedRef.current?.(null);
    }
  }, [isUnmatchedWorkspace]);

  // Load project-plan slots on mount or surface change
  useEffect(() => {
    let cancelled = false;
    setSlotsLoaded(false);
    if (!repoPath && !registryLoaded) {
      return () => { cancelled = true; };
    }
    if (registryView) {
      syncSlotsFromRegistry(registryView);
      setSlotsLoaded(true);
      return () => { cancelled = true; };
    }
    void invoke<ProjectPlanSlot[]>("get_project_plan_slots", { surface })
      .then((fetchedSlots) => {
        if (cancelled) return;
        if (registryViewRef.current) {
          syncSlotsFromRegistry(registryViewRef.current);
          setSlotsLoaded(true);
          return;
        }
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
        setSlotsLoaded(true);
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
        setSlotsLoaded(true);
      });
    return () => {
      cancelled = true;
    };
  }, [surface, registryView, registryLoaded, repoPath]);

  // Keep one shared role assignment per Autocode surface across all repositories.
  const [currentPlannerRole, setCurrentPlannerRole] = useState<string | null>(null);
  const [currentReviserRole, setCurrentReviserRole] = useState<string | null>(null);
  const [legacyRoleConflict, setLegacyRoleConflict] = useState<string[]>([]);
  const roleAssignmentRef = useRef({ plannerRole: null as string | null, reviserRole: null as string | null });
  const lastConfirmedRoleRef = useRef({ plannerRole: null as string | null, reviserRole: null as string | null });
  const roleSaveQueueRef = useRef<Promise<void>>(Promise.resolve());
  const roleSaveSequenceRef = useRef(0);
  const onRolesChangedRef = useRef(onRolesChanged);
  useEffect(() => { onRolesChangedRef.current = onRolesChanged; }, [onRolesChanged]);

  useEffect(() => {
    let cancelled = false;
    const cleared = { plannerRole: null, reviserRole: null };
    roleSaveSequenceRef.current += 1;
    roleAssignmentRef.current = cleared;
    lastConfirmedRoleRef.current = cleared;
    setCurrentPlannerRole(null);
    setCurrentReviserRole(null);
    setLegacyRoleConflict([]);
    void invoke<WorkflowRoleAssignmentView>("get_workflow_role_assignment", { surface })
      .then((view) => {
        if (cancelled) return;
        const assignment = view.assignment;
        const confirmed = { plannerRole: assignment?.planner_id ?? null, reviserRole: assignment?.reviser_id ?? null };
        lastConfirmedRoleRef.current = confirmed;
        roleAssignmentRef.current = confirmed;
        setCurrentPlannerRole(confirmed.plannerRole);
        setCurrentReviserRole(confirmed.reviserRole);
        setLegacyRoleConflict(view.legacy_conflict ? view.conflict_sources : []);
        onRolesChangedRef.current?.(confirmed);
      })
      .catch((e) => {
        if (cancelled) return;
        setLegacyRoleConflict([]);
        const cleared = { plannerRole: null, reviserRole: null };
        roleAssignmentRef.current = cleared;
        lastConfirmedRoleRef.current = cleared;
        setCurrentPlannerRole(null);
        setCurrentReviserRole(null);
        onRolesChangedRef.current?.(cleared);
        setError(String(e));
      });
    return () => { cancelled = true; };
  }, [surface]);

  // Keep callback and activePlanPath references in refs to avoid re-render loops
  const onActivePlanChangedRef = useRef(onActivePlanChanged);
  useEffect(() => {
    onActivePlanChangedRef.current = onActivePlanChanged;
  });
  const onActivePlanContextChangedRef = useRef(onActivePlanContextChanged);
  useEffect(() => {
    onActivePlanContextChangedRef.current = onActivePlanContextChanged;
  });

  const reportActivePlanContext = useCallback((
    repoRoot: string | null,
    slotId: string | null,
    planPath: string | null,
    explicitSelection = false,
  ) => {
    if (!selectedPlanContextResolved && !explicitSelection) return;
    if (!repoRoot || !slotId || !planPath) {
      onActivePlanContextChangedRef.current?.(null);
      return;
    }
    if (explicitSelection) setSelectedPlanContextResolved(true);
    setSelectedPlanRepoRoot(repoRoot);
    onActivePlanContextChangedRef.current?.({
      surface,
      repoRoot,
      slotId: slotId === "primary" || slotId === "secondary" ? slotId : null,
      planPath,
      resolved: true,
    });
  }, [selectedPlanContextResolved, surface]);

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

  // Load canonical repo-local slot state for the active repository and surface
  useEffect(() => {
    if (!locationLoaded || !activeRepo) return;
    let cancelled = false;
    void invoke<SurfacePlanSlotsView | undefined>("get_surface_plan_slots", {
      repoPath: activeRepo,
      surface,
    })
      .then((view) => {
        if (cancelled || !view || typeof view !== "object" || !("slots" in view)) return;
        setCanonicalSlotView(view);
        if (view.active_slot_id === "primary" || view.active_slot_id === "secondary") {
          setSelectedActiveSlotId(view.active_slot_id);
          const matchesSelectedContext =
            !!repoPath ||
            userExplicitSlotRef.current ||
            (selectedPlanContextResolved &&
              (!selectedPlanRepoRoot ||
                normalizePathForCompare(activeRepo) === normalizePathForCompare(selectedPlanRepoRoot)) &&
              (!selectedPlanSlotId || view.active_slot_id === selectedPlanSlotId)) ||
            slots.length <= 1;
          if (matchesSelectedContext) {
            reportActivePlanContext(activeRepo, view.active_slot_id, view.active_plan_path ?? null);
            if (
              view.active_plan_path &&
              view.active_plan_path !== activePlanPathRef.current &&
              onActivePlanChangedRef.current
            ) {
              onActivePlanChangedRef.current(view.active_plan_path);
            }
          }
        } else if (view.is_unresolved) {
          setSelectedActiveSlotId(null);
        }
      })
      .catch(() => {
        // Canonical slot view unavailable; fall back to existing slot state without failing UI load
      });
    return () => {
      cancelled = true;
    };
  }, [
    locationLoaded,
    activeRepo,
    surface,
    repoPath,
    selectedPlanContextResolved,
    selectedPlanRepoRoot,
    selectedPlanSlotId,
    slots.length,
    reportActivePlanContext,
  ]);

  const loadPlans = useCallback(async (folderOverride?: string | null, repoOverride?: string | null, slotOverride?: string | null) => {
    const targetRepo = repoOverride ?? activeRepo;
    const targetSlotId = slotOverride ?? operationalSlotId;
    if (!targetRepo) return;
    try {
      setLoading(true);
      setError(null);
      const targetFolder = folderOverride !== undefined ? folderOverride : selectedFolderRef.current;
      const res = await invoke<PlanFileDetails>("list_available_plans", {
        repoPath: targetRepo,
        folderPath: targetFolder ?? null,
        surface,
        slotId: targetSlotId,
      });
      setDetails(res);
      setSlotDetails((prev) => ({ ...prev, [targetSlotId]: res }));

      const loadedActive = res.active_plan_relative_path ?? null;
      if (loadedActive && !selectedFolderRef.current && folderOverride === undefined) {
        const derivedFolder = loadedActive.includes("/")
          ? loadedActive.substring(0, loadedActive.lastIndexOf("/"))
          : ".";
        setSelectedFolder(derivedFolder);
      } else if (res.selected_folder && !selectedFolderRef.current && folderOverride === undefined) {
        setSelectedFolder(res.selected_folder);
      }

      const canSyncParentActivePlan =
        !!repoPath ||
        userExplicitSlotRef.current ||
        (selectedPlanContextResolved &&
          (!selectedPlanRepoRoot ||
            normalizePathForCompare(targetRepo) === normalizePathForCompare(selectedPlanRepoRoot)) &&
          (!selectedPlanSlotId || targetSlotId === selectedPlanSlotId)) ||
        slots.length <= 1;
      if (canSyncParentActivePlan) {
        if (loadedActive !== activePlanPathRef.current && onActivePlanChangedRef.current) {
          onActivePlanChangedRef.current(loadedActive);
        }
        reportActivePlanContext(targetRepo, targetSlotId, loadedActive);
      }
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [activeRepo, surface, operationalSlotId, reportActivePlanContext]);

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

  const executeRemoveSecondarySlot = async (fallbackSlotId?: string | null) => {
    const secondarySlot = slotsRef.current.find((s) => s.slot_id === "secondary");
    const secondaryRepo = secondarySlot?.repo_path || activeRepo;
    if (fallbackSlotId && secondaryRepo) {
      try {
        await invoke("clear_active_plan_slot", {
          repoPath: secondaryRepo,
          surface,
          slotId: "secondary",
          fallbackSlotId,
        });
      } catch (e: unknown) {
        setError(String(e));
        return;
      }
    }
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
    if (fallbackSlotId) {
      setSelectedActiveSlotId(fallbackSlotId);
      setCanonicalSlotView((prev) => {
        if (!prev) return prev;
        const nextMap = { ...prev.slots };
        delete nextMap.secondary;
        return {
          ...prev,
          active_slot_id: fallbackSlotId,
          active_plan_path: nextMap[fallbackSlotId] ?? null,
          slots: nextMap,
          is_unresolved: false,
        };
      });
    }
    setPendingRemoveSlotConfirm(null);
    void persistSlots(nextSlots);
  };

  const handleRemoveSecondarySlot = () => {
    const secondarySlot = slotsRef.current.find((s) => s.slot_id === "secondary");
    const primarySlot = slotsRef.current.find((s) => s.slot_id === "primary");
    const isSecondaryActive =
      operationalSlotId === "secondary" &&
      Boolean(secondarySlot?.plan_path || canonicalSlotView?.slots?.secondary);
    const primaryFallbackPath = primarySlot?.plan_path || canonicalSlotView?.slots?.primary || null;
    if (isSecondaryActive && primaryFallbackPath) {
      setPendingRemoveSlotConfirm({
        slotId: "secondary",
        fallbackSlotId: "primary",
        fallbackPlanPath: primaryFallbackPath,
      });
      return;
    }
    void executeRemoveSecondarySlot(null);
  };

  const handleSelectPlanFile = async (slotId: string) => {
    const isRegistrySelection = registryViewRef.current?.projects.some((p) => p.project_id === slotId) ?? false;
    try {
      setError(null);
      const targetSlot = slotsRef.current.find((s) => s.slot_id === slotId);
      const slotRepo = targetSlot?.repo_path || null;
      const registryProject = registryViewRef.current?.projects.find((p) => p.project_id === slotId) ?? null;

      if (registryProject) {
        const picked = await invoke<SelectedPlanResult | string | null>("select_plan_file_dialog", {
          repoPath: registryProject.repo_root,
          surface,
          dialogTitle: t("planManager.selectPlanFile") || "プランファイルを選択",
        });
        if (!picked) return;
        if (typeof picked === "string") {
          throw new Error(t("planManager.planFileAssociationMismatch") || "The selected plan file could not be verified for this repository.");
        }
        const pickedRel = picked.relative_path?.trim().replace(/\\/g, "/");
        const pickedRepo = picked.repo_path?.trim();
        if (picked.surface && picked.surface !== surface) {
          throw new Error(t("planManager.planFileAssociationMismatch") || "The selected plan file does not match this surface.");
        }
        const relativeSegments = (pickedRel || "").split("/");
        if (
          !pickedRepo ||
          !pickedRel ||
          pickedRel.startsWith("/") ||
          /^[a-zA-Z]:/.test(pickedRel) ||
          !pickedRel.toLowerCase().endsWith(".md") ||
          relativeSegments.some((segment) => segment === ".." || segment === "")
        ) {
          throw new Error(t("planManager.planFileAssociationMismatch") || "Choose a Markdown plan file inside a repository.");
        }

        const latestRegistry = registryViewRef.current;
        if (!latestRegistry || !Array.isArray(latestRegistry.projects)) {
          throw new Error(t("planManager.planFileAssociationMismatch") || "The project registry is unavailable. Reload and try again.");
        }
        const sourceProject = latestRegistry.projects.find((project) => project.project_id === slotId);
        if (!sourceProject) {
          throw new Error(t("planManager.planSelectionFailed") || "The project row changed. Refresh and try again.");
        }
        const pickedRepoKey = normalizePathForCompare(pickedRepo);
        const sourceRepoKey = normalizePathForCompare(sourceProject.repo_root);
        if (sourceRepoKey !== pickedRepoKey) {
          await replaceProjectPlanAssociation({
            sourceProjectId: sourceProject.project_id,
            targetRepo: normalizeDisplayPath(pickedRepo),
            planPath: pickedRel,
          });
          return;
        }
        const updated = await invoke<ProjectRegistryView>("set_project_plan_path", {
          projectId: sourceProject.project_id,
          planPath: pickedRel,
          surface,
        });
        if (!updated || !Array.isArray(updated.projects)) {
          throw new Error(t("planManager.planFileAssociationMismatch") || "The saved plan association could not be verified. Reload and try again.");
        }
        const saved = updated.projects.find((project) => normalizePathForCompare(project.repo_root) === pickedRepoKey);
        if (!saved || projectPlanPathForSurface(saved, surface)?.replace(/\\/g, "/") !== pickedRel) {
          throw new Error(t("planManager.planFileAssociationMismatch") || "The saved plan association does not match the selected file. Reload and try again.");
        }
        broadcastRegistryUpdate(updated);
        if (repoPath && normalizePathForCompare(repoPath) === normalizePathForCompare(saved.repo_root)) {
          onActivePlanChangedRef.current?.(pickedRel);
        }
        const successText = t("planManager.planPathSaved") || "プランファイルの関連付けを保存しました";
        setActionMessage(`${successText}: ${pickedRel}`);
        setTimeout(() => setActionMessage(null), 3000);
        return;
      }

      const dialogArgs: Record<string, unknown> = {
        repoPath: slotRepo || undefined,
        surface,
        slotId,
        dialogTitle: t("planManager.selectPlanFile") || "プランファイルを選択",
      };

      const picked = await invoke<SelectedPlanResult | string | null>("select_plan_file_dialog", dialogArgs);
      if (!picked) return;
      if (!activeSlotIdsRef.current.has(slotId)) return; // Stale slot check

      // Legacy compatibility path only; registry-managed rows use stable project IDs above.
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

      // Legacy compatibility path only.
      try {
        const pin = await invoke<PinnedWorkflowTarget>("pin_workflow_target", {
          repoPath: pickedRepo,
          surface,
          slotId,
        });
        setPinnedWorkflowTarget(pin);
        onPinnedWorkflowTargetChanged?.(pin);
      } catch (pinError: unknown) {
        setPinnedWorkflowTarget(null);
        onPinnedWorkflowTargetChanged?.(null);
        setError(String(pinError));
      }

      const nextSlots = slotsRef.current.map((s) =>
        s.slot_id === slotId
          ? { ...s, repo_path: pickedRepo, plan_path: pickedRel }
          : s
      );
      setSlots(nextSlots);
      slotsRef.current = nextSlots;
      userExplicitSlotRef.current = true;
      setSelectedPlanSlotId(slotId);
      setSelectedActiveSlotId(slotId);
      void persistSlots(nextSlots, { activeSlotId: slotId, activeRepoPath: pickedRepo });
      if (registryView) {
        void invoke<ProjectRegistryView>("register_project_in_registry", {
          repoPath: pickedRepo,
          planPath: pickedRel,
          surface,
        })
          .then((reg) => {
            if (reg && typeof reg === "object" && Array.isArray(reg.projects)) {
              broadcastRegistryUpdate(reg);
            }
          })
          .catch(() => undefined);
      }
      setMirrorWarning(null);
      setCanonicalSlotView((prev) => ({
        repo_root: pickedRepo,
        surface,
        active_slot_id: slotId,
        active_plan_path: pickedRel,
        slots: {
          ...(prev && normalizePathForCompare(prev.repo_root) === normalizePathForCompare(pickedRepo)
            ? prev.slots
            : {}),
          [slotId]: pickedRel,
        },
        is_unresolved: false,
      }));
      reportActivePlanContext(pickedRepo, slotId, pickedRel, true);
      setSlotDetails((prev) => ({ ...prev, [slotId]: res }));
      if (!repoPath || normalizePathForCompare(pickedRepo) === normalizePathForCompare(repoPath)) {
        const parentFolder = pickedRel.includes("/") ? pickedRel.substring(0, pickedRel.lastIndexOf("/")) : ".";
        setDetails(res);
        setSelectedFolder(parentFolder);
        selectedFolderRef.current = parentFolder;
        if (onActivePlanChangedRef.current) {
          onActivePlanChangedRef.current(pickedRel);
        }
        prevRepoRef.current = pickedRepo;
        await loadPlans(parentFolder, pickedRepo, slotId);
      }

      setActionMessage(`${t("planManager.planActivated") || "計画書を設定しました"}: ${pickedRel.split("/").pop() || pickedRel}`);
      setTimeout(() => setActionMessage(null), 3000);
    } catch (e: unknown) {
      if (isRegistrySelection) {
        try {
          const latest = await invoke<ProjectRegistryView>("get_project_registry");
          if (latest && Array.isArray(latest.projects)) broadcastRegistryUpdate(latest);
        } catch {
          // Keep the last known registry view if authoritative reload also fails.
        }
        const help = t("planManager.planSelectionFailed") || "Could not save the plan selection. Existing entries were kept; verify the plan is inside a repository and try again.";
        setError(help);
      } else {
        setError(String(e));
      }
    }
  };

  const handleAddProjectPlan = async () => {
    try {
      setError(null);
      const picked = await invoke<SelectedPlanResult | string | null>("select_plan_file_dialog", {
        surface,
        dialogTitle: t("planManager.addProjectPlan") || "プランファイルを追加",
      });
      if (!picked) return;
      if (typeof picked === "string") throw new Error(t("planManager.planFileAssociationMismatch") || "The selected plan file could not be verified.");
      const pickedRepo = picked.repo_path?.trim();
      const pickedRel = picked.relative_path?.trim().replace(/\\/g, "/");
      const segments = (pickedRel || "").split("/");
      if (
        (picked.surface && picked.surface !== surface) ||
        !pickedRepo || !pickedRel || pickedRel.startsWith("/") || /^[a-zA-Z]:/.test(pickedRel) ||
        !pickedRel.toLowerCase().endsWith(".md") || segments.some((part) => !part || part === "." || part === "..")
      ) {
        throw new Error(t("planManager.planFileAssociationMismatch") || "Choose a Markdown plan file inside a repository.");
      }
      const before = registryViewRef.current;
      if (!before || !Array.isArray(before.projects)) throw new Error(t("planManager.planSelectionFailed") || "The project registry is unavailable. Refresh and try again.");
      const repoKey = normalizePathForCompare(pickedRepo);
      const wasRegistered = before.projects.some((project) => normalizePathForCompare(project.repo_root) === repoKey);
      const updated = await invoke<ProjectRegistryView>("register_project_in_registry", { repoPath: pickedRepo, planPath: pickedRel, surface });
      const saved = updated?.projects?.find((project) => normalizePathForCompare(project.repo_root) === repoKey);
      if (!saved || projectPlanPathForSurface(saved, surface)?.replace(/\\/g, "/") !== pickedRel) throw new Error(t("planManager.planFileAssociationMismatch") || "The saved plan association could not be verified.");
      broadcastRegistryUpdate(updated);
      const successText = wasRegistered
        ? (t("planManager.planPathSaved") || "Plan association saved")
        : (t("planManager.planRepositoryAdded") || "Repository registered and plan saved");
      setActionMessage(`${successText}: ${pickedRel}`);
      setTimeout(() => setActionMessage(null), 3000);
    } catch (e) {
      setError(`${t("planManager.planSelectionFailed") || "Could not save the plan selection. Existing project entries were kept; verify the file is inside a repository and try again."} (${String(e)})`);
    }
  };

  const handleRemoveProjectAssociation = async (projectId: string) => {
    try {
      setError(null);
      const updated = await invoke<ProjectRegistryView>("unregister_project_from_registry", { projectId, surface });
      if (!updated || !Array.isArray(updated.projects)) throw new Error(t("planManager.planSelectionFailed") || "The registry update could not be verified.");
      const remaining = updated.projects.find((project) => project.project_id === projectId);
      if (remaining && projectPlanPathForSurface(remaining, surface) !== null) {
        throw new Error(t("planManager.planSelectionFailed") || "The project association could not be removed.");
      }
      broadcastRegistryUpdate(updated);
      setActionMessage(t("planManager.planAssociationRemoved") || "Plan association removed");
      setTimeout(() => setActionMessage(null), 3000);
    } catch (e) {
      setError(`${t("planManager.planSelectionFailed") || "Could not save the plan selection. Existing project entries were kept; verify the file is inside a repository and try again."} (${String(e)})`);
      try {
        const refreshed = await invoke<ProjectRegistryView>("get_project_registry");
        if (refreshed && Array.isArray(refreshed.projects)) broadcastRegistryUpdate(refreshed);
      } catch { /* Keep the last verified registry view. */ }
    }
  };

  const handleResyncProjectMirror = async (projectId: string) => {
    try {
      setError(null);
      const updated = await invoke<ProjectRegistryView>("resync_repo_active_plan_mirror", {
        projectId,
        surface,
      });
      if (!updated || !Array.isArray(updated.projects)) {
        throw new Error(t("planManager.planSelectionFailed") || "The registry update could not be verified.");
      }
      broadcastRegistryUpdate(updated);
      if (updated.mirror_sync_error) return;
      setActionMessage("Plan selection mirror resynchronized");
      setTimeout(() => setActionMessage(null), 3000);
    } catch (e) {
      setError(`Could not resynchronize the plan selection mirror. (${String(e)})`);
    }
  };

  const mirrorSyncFailedFor = (projectId: string): boolean => {
    const failure = registryView?.mirror_sync_error;
    if (!failure) return false;
    const targetPrefix = `${projectId} (${surface}):`;
    return failure.split(";").some((entry) => entry.trim().startsWith(targetPrefix));
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
    const isOperational = slotId === operationalSlotId;
    const slotRepo = slot.repo_path || (isOperational ? activeRepo : null);
    const canonicalSlotRel =
      canonicalSlotView &&
      slotRepo &&
      normalizePathForCompare(canonicalSlotView.repo_root) === normalizePathForCompare(slotRepo)
        ? canonicalSlotView.slots[slotId]
        : null;
    const slotRel =
      canonicalSlotRel ||
      slot.plan_path ||
      (isOperational ? (details?.active_plan_relative_path || activePlanPath) : null);
    if (!slotRel) return "";

    const slotDet = slotDetails[slotId] || (isOperational ? details : null);
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
    setError(null);
    setLegacyRoleConflict([]);
    const target = { plannerRole, reviserRole };
    roleAssignmentRef.current = target;
    setCurrentPlannerRole(plannerRole);
    setCurrentReviserRole(reviserRole);
    const sequence = ++roleSaveSequenceRef.current;
    roleSaveQueueRef.current = roleSaveQueueRef.current.catch(() => undefined).then(async () => {
      try {
        await invoke("set_workflow_role_assignment", {
          surface,
          assignment: { planner_id: target.plannerRole, reviser_id: target.reviserRole },
        });
        lastConfirmedRoleRef.current = target;
        if (sequence === roleSaveSequenceRef.current) setError(null);
      } catch (e) {
        if (sequence !== roleSaveSequenceRef.current) return;
        setError(String(e));
        try {
          const stored = await invoke<WorkflowRoleAssignmentView>("get_workflow_role_assignment", { surface });
          if (sequence === roleSaveSequenceRef.current) {
            const restored = { plannerRole: stored.assignment?.planner_id ?? null, reviserRole: stored.assignment?.reviser_id ?? null };
            setLegacyRoleConflict(stored.legacy_conflict ? stored.conflict_sources : []);
            lastConfirmedRoleRef.current = restored;
            roleAssignmentRef.current = restored;
            setCurrentPlannerRole(restored.plannerRole);
            setCurrentReviserRole(restored.reviserRole);
            onRolesChangedRef.current?.(restored);
          }
        } catch (reloadError) {
          if (sequence === roleSaveSequenceRef.current) {
            const fallback = lastConfirmedRoleRef.current;
            roleAssignmentRef.current = fallback;
            setCurrentPlannerRole(fallback.plannerRole);
            setCurrentReviserRole(fallback.reviserRole);
            onRolesChangedRef.current?.(fallback);
            setError(`${String(e)}; failed to reload saved role assignment: ${String(reloadError)}`);
          }
        }
      }
    });
    await roleSaveQueueRef.current;
  };

  const handlePlannerRoleChange = (role: string | null) => {
    void persistRoleAssignment(role, roleAssignmentRef.current.reviserRole);
    if (activeRepo && onRolesChanged) {
      onRolesChanged({ plannerRole: role });
    }
  };

  const handleReviserRoleChange = (role: string | null) => {
    void persistRoleAssignment(roleAssignmentRef.current.plannerRole, role);
    if (activeRepo && onRolesChanged) {
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
  const primaryRelPath =
    (canonicalActiveSlotForRepo ? canonicalSlotView?.active_plan_path : null) ||
    matchedSlot?.plan_path ||
    details?.active_plan_relative_path ||
    activePlanPath;
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
      data-pinned-plan={pinnedWorkflowTarget?.plan_path ?? undefined}
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
      {mirrorWarning && (
        <div
          data-testid="stale-mirror-warning"
          style={{ padding: "6px 10px", marginBottom: 8, backgroundColor: "#fff8c5", color: "#573a08", border: "1px solid #d4a72c", borderRadius: 6, fontSize: 12 }}
        >
          {mirrorWarning}
        </div>
      )}
      {canonicalSlotView?.is_unresolved && (
        <div
          data-testid="unresolved-slot-warning"
          style={{ padding: "6px 10px", marginBottom: 8, backgroundColor: "#fff8c5", color: "#573a08", border: "1px solid #d4a72c", borderRadius: 6, fontSize: 12 }}
        >
          アクティブスロットが未確定です。プロジェクト 1 またはプロジェクト 2 のいずれかを選択してください。
        </div>
      )}
      {pendingRemoveSlotConfirm && (
        <div
          data-testid="active-slot-removal-confirm"
          style={{
            padding: "8px 12px",
            marginBottom: 8,
            backgroundColor: "#fef2f2",
            border: "1px solid #fca5a5",
            borderRadius: 6,
            fontSize: 12,
            color: "#7f1d1d",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            gap: 8,
            flexWrap: "wrap",
          }}
        >
          <span>
            現在アクティブなスロット ({pendingRemoveSlotConfirm.slotId}) を削除すると、{pendingRemoveSlotConfirm.fallbackSlotId} ({pendingRemoveSlotConfirm.fallbackPlanPath}) へ切り替わります。
          </span>
          <div style={{ display: "inline-flex", gap: 6 }}>
            <button
              type="button"
              className="btn btn-small"
              data-testid="btn-confirm-remove-active-slot"
              onClick={() => void executeRemoveSecondarySlot(pendingRemoveSlotConfirm.fallbackSlotId)}
              style={{ fontSize: 11, padding: "2px 8px", color: "#dc2626" }}
            >
              確認して削除
            </button>
            <button
              type="button"
              className="btn btn-small"
              data-testid="btn-cancel-remove-active-slot"
              onClick={() => setPendingRemoveSlotConfirm(null)}
              style={{ fontSize: 11, padding: "2px 8px" }}
            >
              キャンセル
            </button>
          </div>
        </div>
      )}
      {registryView && registryView.pending_conflicts.length > 0 && (
        <div
          data-testid="migration-conflict-banner"
          style={{
            marginBottom: 10,
            padding: "8px 10px",
            borderRadius: 6,
            backgroundColor: "#ffebe9",
            border: "1px solid #ff8182",
            color: "#82071e",
            fontSize: 12,
            display: "flex",
            flexDirection: "column",
            gap: 6,
          }}
        >
          {registryView.pending_conflicts.map((conflict) => (
            <div key={conflict.conflict_id} style={{ display: "flex", flexDirection: "column", gap: 4 }}>
              <strong>{conflict.description}</strong>
              <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                {conflict.options.map((opt) => (
                  <button
                    key={`${conflict.conflict_id}:${opt.source}:${opt.label}`}
                    type="button"
                    className="btn btn-small"
                    data-testid={`btn-resolve-conflict-${conflict.conflict_id}`}
                    onClick={() =>
                      void handleResolveMigrationConflict(
                        conflict.conflict_id,
                        opt.plan_path ?? null
                      )
                    }
                  >
                    {opt.label}
                  </button>
                ))}
              </div>
            </div>
          ))}
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

          {isManagementView ? null : primaryRelPath ? (
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
          const isPrimary = !registryView && slot.slot_id === "primary";
          const isOperational = !registryView && slot.slot_id === operationalSlotId;
          const slotRepo = slot.repo_path || (isOperational ? activeRepo : null);
          const slotRel = slot.plan_path || (isOperational ? primaryRelPath : null);
          const slotDet = slotDetails[slot.slot_id] || (isOperational ? details : null);
          const slotEntry = isOperational
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

          return (
            <div
              key={slot.slot_id}
              data-testid={registryView ? `project-plan-row-${slot.slot_id}` : `project-plan-slot-${slot.slot_id}`}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                minWidth: 0,
              }}
            >

              {/* Bordered Read-Only Static Path Field */}
              <div
                data-testid={registryView ? `plan-path-container-${slot.slot_id}` : isPrimary ? "active-plan-path-container" : `active-plan-path-container-${slot.slot_id}`}
                style={{
                  flex: "1 1 0%",
                  minWidth: 0,
                  display: "flex",
                  alignItems: "center",
                }}
              >
                {slotDisplay ? (
                  <div
                    role="textbox"
                    aria-readonly="true"
                    tabIndex={0}
                    data-testid={registryView ? `plan-path-display-${slot.slot_id}` : isPrimary ? "active-plan-path-display" : `active-plan-path-display-${slot.slot_id}`}
                    title={slotRepoDisplay ? `${slotRepoDisplay}/${slotDisplay}` : slotDisplay}
                    aria-label={slotOpenPlanAriaLabel}
                    style={{
                      flex: 1,
                      minWidth: 0,
                      boxSizing: "border-box",
                      padding: "5px 10px",
                      fontSize: 12,
                      fontFamily: "monospace",
                      color: "var(--color-text-primary, #111827)",
                      backgroundColor: "#ffffff",
                      border: "1px solid #d0d7de",
                      borderRadius: 6,
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                      userSelect: "text",
                    }}
                  >
                    {slotDisplay}
                  </div>
                ) : (
                  <div
                    role="textbox"
                    aria-readonly="true"
                    data-testid={registryView ? `empty-plan-path-${slot.slot_id}` : isPrimary ? "empty-plan-path-field" : `empty-plan-path-field-${slot.slot_id}`}
                    title={slotRepoDisplay || undefined}
                    style={{
                      flex: 1,
                      minWidth: 0,
                      boxSizing: "border-box",
                      padding: "5px 10px",
                      fontSize: 12,
                      color: "#8c959f",
                      backgroundColor: "#f6f8fa",
                      border: "1px solid #d0d7de",
                      borderRadius: 6,
                      whiteSpace: "nowrap",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
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
                data-testid={registryView ? `btn-select-plan-file-${slot.slot_id}` : isPrimary ? "btn-select-plan-file" : `btn-select-plan-file-${slot.slot_id}`}
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
                data-testid={registryView ? `btn-open-plan-${slot.slot_id}` : isPrimary ? "btn-open-plan" : `btn-open-plan-${slot.slot_id}`}
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

              {/* Registry association removal; repository and plan files remain untouched. */}
              {registryView ? (
                <>
                  {mirrorSyncFailedFor(slot.slot_id) && (
                    <button
                      type="button"
                      className="btn btn-small"
                      onClick={() => void handleResyncProjectMirror(slot.slot_id)}
                      data-testid={`btn-resync-plan-mirror-${slot.slot_id}`}
                      title="Resynchronize this repository's selected plan mirror"
                      style={{ fontSize: 12, padding: "4px 10px", whiteSpace: "nowrap", flexShrink: 0 }}
                    >
                      {`⟳ ${t("planManager.refresh") || "Resync"}`}
                    </button>
                  )}
                  <button
                    type="button"
                    className="btn btn-small"
                    onClick={() => void handleRemoveProjectAssociation(slot.slot_id)}
                    data-testid={`btn-remove-project-${slot.slot_id}`}
                    title={t("planManager.removeSlotTooltip") || "Remove the Poverello association only; files are not deleted"}
                    style={{ fontSize: 12, padding: "4px 10px", whiteSpace: "nowrap", color: "#dc2626", flexShrink: 0 }}
                  >
                    {t("planManager.removeSlot") || "Remove"}
                  </button>
                </>
              ) : !isPrimary && (
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

        {registryView && (
          <div style={{ marginTop: 2 }}>
            <button
              type="button"
              className="btn btn-small"
              onClick={() => void handleAddProjectPlan()}
              data-testid="btn-add-project-plan"
              style={{ fontSize: 11, padding: "2px 8px", whiteSpace: "nowrap" }}
            >
              + {t("planManager.addProjectPlan") || "プランファイルを追加"}
            </button>
          </div>
        )}

        {/* Add Slot Button (Max 2 rows) */}
        {!registryView && slots.length < 2 && (
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
              {legacyRoleConflict.length > 0 && (
                <div role="alert" data-testid="shared-role-legacy-conflict" style={{ width: "100%", color: "var(--color-danger, #b42318)", fontSize: 12 }}>
                  {t("planManager.sharedRoleConflict") || "Registered projects have different saved role assignments. Choose the shared roles below to resolve the conflict."}
                  <ul>{legacyRoleConflict.map((source) => <li key={source}>{source}</li>)}</ul>
                </div>
              )}
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
