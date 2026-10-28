export const PERM_STAGE = 1 << 0;
export const PERM_COMMIT = 1 << 1;
export const PERM_PUSH_BRANCH = 1 << 2;
export const PERM_TAG = 1 << 3;
export const PERM_PUSH_TAG = 1 << 4;
export const PERM_RELEASE_PUBLISH = 1 << 5;
export const PERM_PAID_LIVE_API = 1 << 6;

export const LOCAL_GIT_MASK = PERM_STAGE | PERM_COMMIT | PERM_PUSH_BRANCH;
export const TAG_OR_RELEASE_MASK = PERM_TAG | PERM_PUSH_TAG | PERM_RELEASE_PUBLISH;
export const RELEASE_ACTION_MASK = LOCAL_GIT_MASK | TAG_OR_RELEASE_MASK;

export type DeliveryPhase =
  | "PhaseALocalConvergenceMvp"
  | "PhaseBLocalGitActions"
  | "PhaseCTagAndRelease";

export type AutoConvergeState =
  | "PlanCreate"
  | "PlanReview"
  | "PlanRevise"
  | "PlanApproved"
  | "PlanFrozen"
  | "Implement"
  | "ImplementationReview"
  | "CorrectivePlanCreate"
  | "CorrectivePlanReview"
  | "CorrectivePlanRevise"
  | "CorrectiveImplement"
  | "ImplementationApproved"
  | "VerificationRequired"
  | "ReadyForReleaseReview"
  | "ReleaseReview"
  | "ReleaseActionsPending"
  | "ReleaseCompleted"
  | "ReleaseReadyHalted"
  | "Blocked";

export type BlockedKind =
  | "UserConfirmationRequired"
  | "FatalInvariant"
  | "BudgetExhausted";

export interface BlockedContext {
  kind: BlockedKind;
  pre_blocked_state: AutoConvergeState;
  reason: string;
}

export type EscalationPolicy = "Enabled" | "BlockingOnly" | "Disabled";
export type ReviewReusePolicy = "AlwaysReview" | "ChangedOnly" | "ValidBinding";
export type ReviewMode = "Normal" | "Escalated";

export interface ProfilePolicyCapabilities {
  allow_stage: boolean;
  allow_commit: boolean;
  allow_push_branch: boolean;
  allow_tag: boolean;
  allow_push_tag: boolean;
  allow_release_publish: boolean;
  allow_paid_live_api: boolean;
}

export interface ConvergenceLimits {
  max_plan_reviews: number;
  max_impl_reviews: number;
  max_corrective_reviews: number;
  max_verification_retries: number;
  max_release_reviews: number;
  same_blocker_threshold: number;
  max_escalations: number;
  max_escalation_reviews: number;
  max_transport_retries: number;
}

export interface AutoConvergeProfile {
  id: string;
  name: string;
  is_builtin: boolean;
  plan_review_mandatory: boolean;
  implementation_review_mandatory: boolean;
  corrective_plan_review_mandatory: boolean;
  targeted_tests: boolean;
  full_test_suite: boolean;
  frontend_rust_build: boolean;
  lint_and_format_check: boolean;
  git_diff_check: boolean;
  manual_smoke_test: boolean;
  live_api_smoke: boolean;
  installer_smoke: boolean;
  artifact_verification: boolean;
  release_readiness_review: boolean;
  review_reuse_policy: ReviewReusePolicy;
  escalation_policy: EscalationPolicy;
  capabilities: ProfilePolicyCapabilities;
  convergence_limits: ConvergenceLimits;
  pre_granted_run_permissions?: number;
}

export interface ReleasePermissionIntent {
  requested_version_tag: string;
  release_repository: string;
}

export interface ConcreteReleaseTarget {
  version_tag: string;
  candidate_diff_digest: string;
  release_manifest_digest: string;
  target_commit_sha: string;
  release_repository: string;
}

export interface ReleaseAuthorizationState {
  run_start_permission_binding: string;
  release_intent: ReleasePermissionIntent | null;
  candidate_release_authorization: string | null;
  concrete_release_target: ConcreteReleaseTarget | null;
  release_execution_permission_binding: string | null;
}

export interface PlanLineage {
  root_plan_digest: string | null;
  active_plan_digest: string | null;
  candidate_plan_digest: string | null;
  approved_plan_chain: string[];
  approved_plan_chain_digest: string | null;
}

export interface ReviewFindingKey {
  category:
    | "correctness"
    | "security"
    | "contract"
    | "performance"
    | "scope"
    | "maintainability";
  file: string;
  symbol: string;
  failure_code: string;
}

export interface ReviewFinding {
  finding_key: ReviewFindingKey;
  location: string;
  failure_condition: string;
  acceptance_condition: string;
  blocking: boolean;
}

export interface TrackedFinding {
  finding_id: string;
  finding: ReviewFinding;
  consecutive_cycles: number;
  resolved: boolean;
}

export interface CycleCounts {
  plan_review_count: number;
  impl_review_count: number;
  corrective_review_count: number;
  verification_retry_count: number;
  release_review_count: number;
  escalation_count: number;
  escalation_review_count: number;
  transport_retry_count: number;
}

export interface ExecutionLogEntry {
  timestamp: string;
  state: AutoConvergeState;
  actor: string;
  message: string;
}

export interface UserConfirmationToken {
  run_id: string;
  confirmed_reason: string;
  issued_at: string;
}

export interface AutoConvergeRunConfig {
  run_id?: string | null;
  working_dir: string;
  repo_remote_url: string;
  target_branch: string;
  profile: AutoConvergeProfile;
  run_permissions: number;
  release_intent: ReleasePermissionIntent | null;
  force_escalation: boolean;
  require_environment_verification: boolean;
  delivery_phase?: DeliveryPhase;
  reviewer_level?: "R0-D" | string;
  r0d_acknowledged?: boolean;
}

export interface AutoConvergeRunState {
  schema_version: number;
  state_sequence: number;
  run_id: string;
  created_at: string;
  updated_at: string;
  delivery_phase: DeliveryPhase;
  plan_lineage: PlanLineage;
  runtime_allowlist: string[];
  allowlist_digest: string | null;
  current_diff_digest: string | null;
  current_evidence_manifest_digest: string | null;
  current_release_manifest_digest: string | null;
  profile: AutoConvergeProfile;
  policy_profile_digest: string;
  run_permissions: number;
  effective_permissions: number;
  release_authorization: ReleaseAuthorizationState;
  current_state: AutoConvergeState;
  is_terminal: boolean;
  is_paused: boolean;
  blocked_context: BlockedContext | null;
  review_mode: ReviewMode;
  active_producer_model: string;
  active_reviewer_model: string;
  active_reasoning_effort: string;
  cycle_counts: CycleCounts;
  findings: TrackedFinding[];
  completed_release_actions: string[];
  execution_log: ExecutionLogEntry[];
}
