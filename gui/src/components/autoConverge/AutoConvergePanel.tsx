import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../../i18n";
import ProfileSelector from "./ProfileSelector";
import RunPermissionsPanel from "./RunPermissionsPanel";
import ExecutionMonitor from "./ExecutionMonitor";
import AuditReportModal from "./AuditReportModal";
import type {
  AutoConvergeProfile,
  AutoConvergeRunConfig,
  AutoConvergeRunState,
  ReleasePermissionIntent,
} from "../../types/autoConverge";

export const BUILTIN_FALLBACK_PROFILES: AutoConvergeProfile[] = [
  {
    id: "fast-local",
    name: "Fast Local",
    is_builtin: true,
    plan_review_mandatory: true,
    implementation_review_mandatory: true,
    corrective_plan_review_mandatory: true,
    targeted_tests: true,
    full_test_suite: false,
    frontend_rust_build: false,
    lint_and_format_check: false,
    git_diff_check: true,
    manual_smoke_test: false,
    live_api_smoke: false,
    installer_smoke: false,
    artifact_verification: false,
    release_readiness_review: false,
    review_reuse_policy: "ValidBinding",
    escalation_policy: "Disabled",
    capabilities: {
      allow_stage: true,
      allow_commit: false,
      allow_push_branch: false,
      allow_tag: false,
      allow_push_tag: false,
      allow_release_publish: false,
      allow_paid_live_api: false,
    },
    convergence_limits: {
      max_plan_reviews: 2,
      max_impl_reviews: 2,
      max_corrective_reviews: 1,
      max_verification_retries: 2,
      max_release_reviews: 0,
      same_blocker_threshold: 2,
      max_escalations: 0,
      max_escalation_reviews: 0,
      max_transport_retries: 2,
    },
  },
  {
    id: "balanced",
    name: "Balanced (Default)",
    is_builtin: true,
    plan_review_mandatory: true,
    implementation_review_mandatory: true,
    corrective_plan_review_mandatory: true,
    targeted_tests: true,
    full_test_suite: true,
    frontend_rust_build: true,
    lint_and_format_check: true,
    git_diff_check: true,
    manual_smoke_test: false,
    live_api_smoke: false,
    installer_smoke: false,
    artifact_verification: false,
    release_readiness_review: false,
    review_reuse_policy: "ChangedOnly",
    escalation_policy: "BlockingOnly",
    capabilities: {
      allow_stage: true,
      allow_commit: true,
      allow_push_branch: false,
      allow_tag: false,
      allow_push_tag: false,
      allow_release_publish: false,
      allow_paid_live_api: false,
    },
    convergence_limits: {
      max_plan_reviews: 3,
      max_impl_reviews: 3,
      max_corrective_reviews: 2,
      max_verification_retries: 2,
      max_release_reviews: 1,
      same_blocker_threshold: 2,
      max_escalations: 1,
      max_escalation_reviews: 2,
      max_transport_retries: 2,
    },
  },
  {
    id: "strict-release",
    name: "Strict Release",
    is_builtin: true,
    plan_review_mandatory: true,
    implementation_review_mandatory: true,
    corrective_plan_review_mandatory: true,
    targeted_tests: true,
    full_test_suite: true,
    frontend_rust_build: true,
    lint_and_format_check: true,
    git_diff_check: true,
    manual_smoke_test: true,
    live_api_smoke: true,
    installer_smoke: true,
    artifact_verification: true,
    release_readiness_review: true,
    review_reuse_policy: "AlwaysReview",
    escalation_policy: "Enabled",
    capabilities: {
      allow_stage: true,
      allow_commit: true,
      allow_push_branch: true,
      allow_tag: true,
      allow_push_tag: true,
      allow_release_publish: true,
      allow_paid_live_api: true,
    },
    convergence_limits: {
      max_plan_reviews: 3,
      max_impl_reviews: 3,
      max_corrective_reviews: 2,
      max_verification_retries: 2,
      max_release_reviews: 1,
      same_blocker_threshold: 2,
      max_escalations: 1,
      max_escalation_reviews: 2,
      max_transport_retries: 2,
    },
  },
];

export default function AutoConvergePanel() {
  const { t } = useTranslation();
  const [profiles, setProfiles] = useState<AutoConvergeProfile[]>(BUILTIN_FALLBACK_PROFILES);
  const [selectedProfileId, setSelectedProfileId] = useState<string>("balanced");
  const [runPermissions, setRunPermissions] = useState<number>(0);
  const [releaseIntent, setReleaseIntent] = useState<ReleasePermissionIntent>({
    requested_version_tag: "",
    release_repository: "",
  });
  const [workingDir, setWorkingDir] = useState<string>(".");
  const [repoRemoteUrl, setRepoRemoteUrl] = useState<string>("origin");
  const [targetBranch, setTargetBranch] = useState<string>("main");
  const [forceEscalation, setForceEscalation] = useState<boolean>(false);
  const [runState, setRunState] = useState<AutoConvergeRunState | null>(null);
  const [errorBanner, setErrorBanner] = useState<string | null>(null);
  const [auditModalOpen, setAuditModalOpen] = useState<boolean>(false);

  const loadProfiles = useCallback(async () => {
    try {
      const loaded = await invoke<AutoConvergeProfile[]>("auto_converge_get_profiles");
      if (Array.isArray(loaded) && loaded.length > 0) {
        setProfiles(loaded);
      }
    } catch {
      // Fallback to built-in profiles in test/offline mode
    }
  }, []);

  useEffect(() => {
    void loadProfiles();
  }, [loadProfiles]);

  const isRunActive = useMemo(() => {
    if (!runState) return false;
    return !runState.is_terminal;
  }, [runState]);

  // §8.4: Active runs use sealed profile snapshot exclusively
  const activeProfile = useMemo(() => {
    if (runState && !runState.is_terminal) {
      return runState.profile;
    }
    return (
      profiles.find((p) => p.id === selectedProfileId) ??
      profiles[1] ??
      BUILTIN_FALLBACK_PROFILES[1]
    );
  }, [profiles, selectedProfileId, runState]);

  const handleSelectProfile = useCallback((profileId: string) => {
    setSelectedProfileId(profileId);
    // Reset per-run permissions when switching profiles (§8.2)
    setRunPermissions(0);
    setErrorBanner(null);
  }, []);

  const handleResetToPresets = useCallback(() => {
    setProfiles(BUILTIN_FALLBACK_PROFILES);
    setSelectedProfileId("balanced");
    setRunPermissions(0);
    setErrorBanner(null);
  }, []);

  const handleSaveCustomProfile = useCallback(async (customProfile: AutoConvergeProfile) => {
    setErrorBanner(null);
    if (
      !customProfile.plan_review_mandatory ||
      !customProfile.implementation_review_mandatory ||
      !customProfile.corrective_plan_review_mandatory
    ) {
      setErrorBanner("Hard Invariant Violation: Mandatory review gates cannot be disabled.");
      return;
    }

    setProfiles((prev) => [...prev.filter((p) => p.id !== customProfile.id), customProfile]);
    setSelectedProfileId(customProfile.id);

    try {
      const saved = await invoke<AutoConvergeProfile | undefined>("auto_converge_save_profile", {
        profile: customProfile,
      });
      if (saved && saved.id) {
        setProfiles((prev) => [...prev.filter((p) => p.id !== saved.id), saved]);
        setSelectedProfileId(saved.id);
      }
    } catch (err) {
      setErrorBanner(String(err));
    }
  }, []);

  const handleStartRun = useCallback(async () => {
    setErrorBanner(null);
    const config: AutoConvergeRunConfig = {
      run_id: null,
      working_dir: workingDir,
      repo_remote_url: repoRemoteUrl,
      target_branch: targetBranch,
      profile: activeProfile,
      run_permissions: runPermissions,
      release_intent:
        releaseIntent.requested_version_tag && releaseIntent.release_repository
          ? releaseIntent
          : null,
      force_escalation: forceEscalation,
      require_environment_verification: false,
      delivery_phase: "PhaseALocalConvergenceMvp",
    };

    try {
      const state = await invoke<AutoConvergeRunState>("auto_converge_start_run", {
        config,
      });
      setRunState(state);
    } catch (err) {
      setErrorBanner(String(err));
    }
  }, [
    workingDir,
    repoRemoteUrl,
    targetBranch,
    activeProfile,
    runPermissions,
    releaseIntent,
    forceEscalation,
  ]);

  const handlePauseRun = useCallback(async () => {
    if (!runState) return;
    setErrorBanner(null);
    try {
      const next = await invoke<AutoConvergeRunState>("auto_converge_pause_run", {
        workingDir,
        runId: runState.run_id,
      });
      setRunState(next);
    } catch (err) {
      setErrorBanner(String(err));
    }
  }, [runState, workingDir]);

  const handleResumeRun = useCallback(async () => {
    if (!runState) return;
    setErrorBanner(null);
    try {
      const next = await invoke<AutoConvergeRunState>("auto_converge_resume_run", {
        workingDir,
        runId: runState.run_id,
        confirmation: null,
      });
      setRunState(next);
    } catch (err) {
      setErrorBanner(String(err));
    }
  }, [runState, workingDir]);

  const handleAbortRun = useCallback(async () => {
    if (!runState) return;
    setErrorBanner(null);
    try {
      const next = await invoke<AutoConvergeRunState>("auto_converge_abort_run", {
        workingDir,
        runId: runState.run_id,
      });
      setRunState(next);
    } catch (err) {
      setErrorBanner(String(err));
    }
  }, [runState, workingDir]);

  return (
    <div className="auto-converge-panel" data-testid="auto-converge-panel">
      <p
        className="auto-converge-unavailable-banner"
        role="alert"
        data-testid="auto-converge-unavailable"
        style={{ color: "#b42318", background: "#fef3f2", border: "1px solid #fecdca", borderRadius: 6, padding: "10px 12px" }}
      >
        {t("autoConverge.unavailable")}
      </p>
      <div className="auto-converge-header">
        <div>
          <h2>Codex Auto-Converge / Review Orchestrator</h2>
          <p className="auto-converge-subtitle">
            Bounded, non-Rubber-Stamp Plan → Review → Implement → Test → Review convergence loop
          </p>
        </div>
        <div className="auto-converge-header-actions">
          {!isRunActive ? (
            <button
              type="button"
              className="btn btn-primary"
              onClick={handleStartRun}
              data-testid="start-run-btn"
              disabled
              title={t("autoConverge.unavailable")}
            >
              Start Auto-Converge
            </button>
          ) : (
            <>
              {!runState?.is_paused ? (
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={handlePauseRun}
                  data-testid="pause-run-btn"
                >
                  Pause
                </button>
              ) : (
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={handleResumeRun}
                  data-testid="resume-run-btn"
                >
                  Resume
                </button>
              )}
              <button
                type="button"
                className="btn btn-danger"
                onClick={handleAbortRun}
                data-testid="abort-run-btn"
              >
                Abort
              </button>
            </>
          )}
          <button
            type="button"
            className="btn btn-secondary"
            disabled={!runState}
            onClick={() => setAuditModalOpen(true)}
            data-testid="open-audit-modal-btn"
          >
            View Audit Report
          </button>
        </div>
      </div>

      {errorBanner && (
        <div className="auto-converge-error-banner" role="alert" data-testid="auto-converge-error">
          {errorBanner}
        </div>
      )}

      <div className="auto-converge-config-bar">
        <label>
          <span>Workspace Root:</span>
          <input
            type="text"
            value={workingDir}
            disabled={isRunActive}
            onChange={(e) => {
              setWorkingDir(e.target.value);
              setRunPermissions(0);
            }}
            data-testid="input-workspace-root"
          />
        </label>
        <label>
          <span>Remote URL:</span>
          <input
            type="text"
            value={repoRemoteUrl}
            disabled={isRunActive}
            onChange={(e) => {
              setRepoRemoteUrl(e.target.value);
              setRunPermissions(0);
            }}
            data-testid="input-remote-url"
          />
        </label>
        <label>
          <span>Target Branch:</span>
          <input
            type="text"
            value={targetBranch}
            disabled={isRunActive}
            onChange={(e) => {
              setTargetBranch(e.target.value);
              setRunPermissions(0);
            }}
            data-testid="input-target-branch"
          />
        </label>
        <label>
          <input
            type="checkbox"
            checked={forceEscalation}
            disabled={isRunActive || activeProfile.escalation_policy === "Disabled"}
            onChange={(e) => setForceEscalation(e.target.checked)}
            data-testid="force-escalation-checkbox"
          />
          Force Escalation (SoL 450B / High)
        </label>
      </div>

      <ProfileSelector
        profiles={profiles}
        selectedProfileId={activeProfile.id}
        disabled={isRunActive}
        onSelectProfile={handleSelectProfile}
        onSaveCustomProfile={handleSaveCustomProfile}
        onResetToPresets={handleResetToPresets}
      />

      <RunPermissionsPanel
        profile={activeProfile}
        runPermissions={runPermissions}
        onChangePermissions={setRunPermissions}
        releaseIntent={releaseIntent}
        onChangeReleaseIntent={setReleaseIntent}
        deliveryPhase={runState?.delivery_phase ?? "PhaseALocalConvergenceMvp"}
        disabled={isRunActive}
      />

      <ExecutionMonitor runState={runState} />

      <AuditReportModal
        open={auditModalOpen}
        runState={runState}
        onClose={() => setAuditModalOpen(false)}
      />
    </div>
  );
}
