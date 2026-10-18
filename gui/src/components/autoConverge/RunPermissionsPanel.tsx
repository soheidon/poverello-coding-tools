import {
  PERM_COMMIT,
  PERM_PAID_LIVE_API,
  PERM_PUSH_BRANCH,
  PERM_PUSH_TAG,
  PERM_RELEASE_PUBLISH,
  PERM_STAGE,
  PERM_TAG,
  type AutoConvergeProfile,
  type DeliveryPhase,
  type ReleasePermissionIntent,
} from "../../types/autoConverge";

interface RunPermissionsPanelProps {
  profile: AutoConvergeProfile;
  runPermissions: number;
  onChangePermissions: (nextPermissions: number) => void;
  releaseIntent: ReleasePermissionIntent;
  onChangeReleaseIntent: (nextIntent: ReleasePermissionIntent) => void;
  deliveryPhase?: DeliveryPhase;
  disabled?: boolean;
}

export default function RunPermissionsPanel({
  profile,
  runPermissions,
  onChangePermissions,
  releaseIntent,
  onChangeReleaseIntent,
  deliveryPhase = "PhaseCTagAndRelease",
  disabled = false,
}: RunPermissionsPanelProps) {
  const caps = profile.capabilities;
  const isPhaseA = deliveryPhase === "PhaseALocalConvergenceMvp";

  const hasBit = (bit: number) => (runPermissions & bit) !== 0;

  const toggleBit = (bit: number, checked: boolean) => {
    let next = runPermissions;
    if (checked) {
      next |= bit;
      // Enforce prerequisite chains (§9.1):
      if (bit === PERM_COMMIT) next |= PERM_STAGE;
      if (bit === PERM_PUSH_BRANCH) next |= PERM_COMMIT | PERM_STAGE;
      if (bit === PERM_PUSH_TAG) next |= PERM_TAG;
      if (bit === PERM_RELEASE_PUBLISH) next |= PERM_PUSH_TAG | PERM_TAG;
    } else {
      next &= ~bit;
      // Cascade disable dependents (§9.1):
      if (bit === PERM_STAGE) next &= ~(PERM_COMMIT | PERM_PUSH_BRANCH);
      if (bit === PERM_COMMIT) next &= ~PERM_PUSH_BRANCH;
      if (bit === PERM_TAG) next &= ~(PERM_PUSH_TAG | PERM_RELEASE_PUBLISH);
      if (bit === PERM_PUSH_TAG) next &= ~PERM_RELEASE_PUBLISH;
    }
    onChangePermissions(next);
  };

  const tagOrReleaseRequested =
    hasBit(PERM_TAG) || hasBit(PERM_PUSH_TAG) || hasBit(PERM_RELEASE_PUBLISH);

  return (
    <div className="auto-converge-permissions-panel" data-testid="run-permissions-panel">
      <h4>Run Permissions (This Run Only)</h4>
      {isPhaseA && (
        <p className="phase-a-notice" data-testid="phase-a-deferred-notice">
          Phase A MVP Mode: Local convergence only. Git/release/paid-live-API mutations are deferred.
        </p>
      )}
      <div className="permissions-list">
        <label>
          <input
            type="checkbox"
            data-testid="perm-stage"
            checked={hasBit(PERM_STAGE)}
            disabled={disabled || isPhaseA || !caps.allow_stage}
            onChange={(e) => toggleBit(PERM_STAGE, e.target.checked)}
          />
          Stage approved files only
        </label>
        <label>
          <input
            type="checkbox"
            data-testid="perm-commit"
            checked={hasBit(PERM_COMMIT)}
            disabled={disabled || isPhaseA || !caps.allow_commit}
            onChange={(e) => toggleBit(PERM_COMMIT, e.target.checked)}
          />
          Git Commit
        </label>
        <label>
          <input
            type="checkbox"
            data-testid="perm-push-branch"
            checked={hasBit(PERM_PUSH_BRANCH)}
            disabled={disabled || isPhaseA || !caps.allow_push_branch}
            onChange={(e) => toggleBit(PERM_PUSH_BRANCH, e.target.checked)}
          />
          Git Push to upstream
        </label>
        <label>
          <input
            type="checkbox"
            data-testid="perm-tag"
            checked={hasBit(PERM_TAG)}
            disabled={
              disabled ||
              isPhaseA ||
              !caps.allow_tag ||
              !profile.release_readiness_review
            }
            onChange={(e) => toggleBit(PERM_TAG, e.target.checked)}
          />
          Git Tag
        </label>
        <label>
          <input
            type="checkbox"
            data-testid="perm-push-tag"
            checked={hasBit(PERM_PUSH_TAG)}
            disabled={
              disabled ||
              isPhaseA ||
              !caps.allow_push_tag ||
              !profile.release_readiness_review
            }
            onChange={(e) => toggleBit(PERM_PUSH_TAG, e.target.checked)}
          />
          Git Push Tag
        </label>
        <label>
          <input
            type="checkbox"
            data-testid="perm-release-publish"
            checked={hasBit(PERM_RELEASE_PUBLISH)}
            disabled={
              disabled ||
              isPhaseA ||
              !caps.allow_release_publish ||
              !profile.release_readiness_review
            }
            onChange={(e) => toggleBit(PERM_RELEASE_PUBLISH, e.target.checked)}
          />
          GitHub Release Publication
        </label>
        <label>
          <input
            type="checkbox"
            data-testid="perm-paid-live-api"
            checked={hasBit(PERM_PAID_LIVE_API)}
            disabled={disabled || isPhaseA || !caps.allow_paid_live_api}
            onChange={(e) => toggleBit(PERM_PAID_LIVE_API, e.target.checked)}
          />
          Live Paid API Execution
        </label>
      </div>

      {tagOrReleaseRequested && (
        <div className="release-intent-fields" data-testid="release-intent-section">
          <label>
            Version Tag:
            <input
              type="text"
              data-testid="release-version-tag-input"
              value={releaseIntent.requested_version_tag}
              disabled={disabled}
              placeholder="v0.24.6"
              onChange={(e) =>
                onChangeReleaseIntent({
                  ...releaseIntent,
                  requested_version_tag: e.target.value,
                })
              }
            />
          </label>
          <label>
            Release Repository:
            <input
              type="text"
              data-testid="release-repo-input"
              value={releaseIntent.release_repository}
              disabled={disabled}
              placeholder="soheidon/poverello"
              onChange={(e) =>
                onChangeReleaseIntent({
                  ...releaseIntent,
                  release_repository: e.target.value,
                })
              }
            />
          </label>
        </div>
      )}
    </div>
  );
}
