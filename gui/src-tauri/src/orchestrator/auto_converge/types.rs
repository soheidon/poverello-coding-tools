use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// Permission Bitmask Constants (§9.1, §9.2)
// ---------------------------------------------------------------------------

pub const PERM_STAGE: u32 = 1 << 0;
pub const PERM_COMMIT: u32 = 1 << 1;
pub const PERM_PUSH_BRANCH: u32 = 1 << 2;
pub const PERM_TAG: u32 = 1 << 3;
pub const PERM_PUSH_TAG: u32 = 1 << 4;
pub const PERM_RELEASE_PUBLISH: u32 = 1 << 5;
pub const PERM_PAID_LIVE_API: u32 = 1 << 6;

pub const LOCAL_GIT_MASK: u32 = PERM_STAGE | PERM_COMMIT | PERM_PUSH_BRANCH;
pub const TAG_OR_RELEASE_MASK: u32 = PERM_TAG | PERM_PUSH_TAG | PERM_RELEASE_PUBLISH;
pub const RELEASE_ACTION_MASK: u32 = LOCAL_GIT_MASK | TAG_OR_RELEASE_MASK;

// ---------------------------------------------------------------------------
// Delivery Phase Boundary (§16.2, §16.5)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryPhase {
    /// Phase A (v0.24.6 Default): Local review convergence MVP.
    /// Rejects git mutations, release publication, and paid live API execution.
    PhaseALocalConvergenceMvp,
    /// Phase B: Local Git actions (stage, commit, push_branch).
    PhaseBLocalGitActions,
    /// Phase C: Full tag & release publication.
    PhaseCTagAndRelease,
}

impl Default for DeliveryPhase {
    fn default() -> Self {
        Self::PhaseALocalConvergenceMvp
    }
}

// ---------------------------------------------------------------------------
// 19 Discrete FSM States (§6.1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AutoConvergeState {
    PlanCreate,
    PlanReview,
    PlanRevise,
    PlanApproved,
    PlanFrozen,
    Implement,
    ImplementationReview,
    CorrectivePlanCreate,
    CorrectivePlanReview,
    CorrectivePlanRevise,
    CorrectiveImplement,
    ImplementationApproved,
    VerificationRequired,
    ReadyForReleaseReview,
    ReleaseReview,
    ReleaseActionsPending,
    ReleaseCompleted,
    ReleaseReadyHalted,
    Blocked,
}

impl AutoConvergeState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::ReleaseCompleted | Self::ReleaseReadyHalted | Self::Blocked
        )
    }
}

// ---------------------------------------------------------------------------
// Blocked Context (§6.1, §10.1, §10.2)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockedKind {
    UserConfirmationRequired,
    FatalInvariant,
    BudgetExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedContext {
    pub kind: BlockedKind,
    pub pre_blocked_state: AutoConvergeState,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Escalation Policy & Triggers (§7.2, §7.3)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EscalationPolicy {
    Enabled,
    BlockingOnly,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EscalationTriggerReason {
    ForcedRunStart,
    ArchitecturalDefectOrRecommended,
    SameBlockerPersistence,
    StageReviewBudgetExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewMode {
    Normal,
    Escalated,
}

// ---------------------------------------------------------------------------
// Authoritative Review Reuse Policy (§6.3)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewReusePolicy {
    AlwaysReview,
    ChangedOnly,
    ValidBinding,
}

// ---------------------------------------------------------------------------
// Review Verdict & Stage-Specific Approval Binding (§3.2, §4 Invariant 5)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictDecision {
    #[serde(rename = "APPROVE")]
    Approve,
    #[serde(rename = "REQUEST_CHANGES")]
    RequestChanges,
    #[serde(rename = "BLOCKED")]
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalTargetType {
    Plan,
    CorrectivePlan,
    ImplementationDiff,
    ReleaseCandidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingCategory {
    Correctness,
    Security,
    Contract,
    Performance,
    Scope,
    Maintainability,
}

impl FindingCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Correctness => "correctness",
            Self::Security => "security",
            Self::Contract => "contract",
            Self::Performance => "performance",
            Self::Scope => "scope",
            Self::Maintainability => "maintainability",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFindingKey {
    pub category: FindingCategory,
    pub file: String,
    pub symbol: String,
    pub failure_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFinding {
    pub finding_key: ReviewFindingKey,
    pub location: String,
    pub failure_condition: String,
    pub acceptance_condition: String,
    pub blocking: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackedFinding {
    pub finding_id: String,
    pub finding: ReviewFinding,
    pub consecutive_cycles: u32,
    pub resolved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalBinding {
    pub target_type: ApprovalTargetType,
    pub root_plan_digest: Option<String>,
    pub active_plan_digest: Option<String>,
    pub candidate_plan_digest: Option<String>,
    pub allowlist_digest: String,
    pub policy_profile_digest: String,
    #[serde(default)]
    pub baseline_workspace_digest: Option<String>,
    #[serde(default)]
    pub diff_digest: Option<String>,
    #[serde(default)]
    pub evidence_manifest_digest: Option<String>,
    #[serde(default)]
    pub release_manifest_digest: Option<String>,
    #[serde(default)]
    pub bundle_digest: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub cycle: Option<u64>,
    #[serde(default)]
    pub implementer_session_id: Option<String>,
}

impl Default for ApprovalBinding {
    fn default() -> Self {
        Self {
            target_type: ApprovalTargetType::Plan,
            allowlist_digest: String::new(),
            policy_profile_digest: String::new(),
            candidate_plan_digest: None,
            root_plan_digest: None,
            active_plan_digest: None,
            baseline_workspace_digest: None,
            diff_digest: None,
            evidence_manifest_digest: None,
            release_manifest_digest: None,
            bundle_digest: None,
            run_id: None,
            cycle: None,
            implementer_session_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateBinding {
    pub run_id: String,
    pub cycle: u64,
    pub session_id: String,
    pub candidate_root: String,
    pub approved_plan_digest: String,
    pub run_start_baseline_manifest_digest: String,
    pub allowlist_digest: String,
    pub candidate_baseline_digest: String,
    #[serde(default)]
    pub review_bundle_digest: Option<String>,
}

impl CandidateBinding {
    pub fn validate(&self) -> Result<(), String> {
        if self.run_id.trim().is_empty()
            || self.session_id.trim().is_empty()
            || self.candidate_root.trim().is_empty()
            || !std::path::Path::new(&self.candidate_root).is_absolute()
            || !is_valid_sha256_hex(&self.approved_plan_digest)
            || !is_valid_sha256_hex(&self.run_start_baseline_manifest_digest)
            || !is_valid_sha256_hex(&self.allowlist_digest)
            || !is_valid_sha256_hex(&self.candidate_baseline_digest)
            || (self.review_bundle_digest.is_some()
                && !is_valid_sha256_hex(self.review_bundle_digest.as_deref().unwrap()))
        {
            return Err(
                "candidate binding is partially populated or contains malformed digests".into(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionPhase {
    Staged,
    Applying,
    Applied,
    RecoveryRequired,
    RecoveryRestored,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionJournalBinding {
    pub run_id: String,
    pub journal_dir: String,
    pub journal_digest: String,
    pub phase: PromotionPhase,
    pub candidate_root: String,
}

impl PromotionJournalBinding {
    pub fn validate(&self) -> Result<(), String> {
        if self.run_id.trim().is_empty()
            || self.journal_dir.trim().is_empty()
            || !std::path::Path::new(&self.journal_dir).is_absolute()
            || self.candidate_root.trim().is_empty()
            || !std::path::Path::new(&self.candidate_root).is_absolute()
            || !is_valid_sha256_hex(&self.journal_digest)
        {
            return Err("promotion journal binding is partially populated or invalid".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexReviewVerdict {
    pub verdict: VerdictDecision,
    pub summary: String,
    pub findings: Vec<ReviewFinding>,
    pub approval_binding: ApprovalBinding,
    #[serde(default)]
    pub architectural_defect: bool,
    #[serde(default)]
    pub escalation_recommended: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub stage_state: AutoConvergeState,
    pub reviewer_session_id: String,
    pub producer_session_id: String,
    pub timestamp: String,
    pub sequence: u64,
    pub binding: ApprovalBinding,
    pub invalidated: bool,
    pub engine_auth_tag: String,
}

// ---------------------------------------------------------------------------
// Plan Lineage (§3.2, §10.1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanLineage {
    pub root_plan_digest: Option<String>,
    pub active_plan_digest: Option<String>,
    pub candidate_plan_digest: Option<String>,
    pub approved_plan_chain: Vec<String>,
    pub approved_plan_chain_digest: Option<String>,
}

impl PlanLineage {
    pub fn recompute_chain_digest(&mut self) {
        let json = serde_json::to_string(&self.approved_plan_chain).unwrap_or_default();
        self.approved_plan_chain_digest = Some(sha256_hex(json.as_bytes()));
    }
}

// ---------------------------------------------------------------------------
// Baseline File Manifest & RunDeltaManifest (§2.4, §3.2, §10.1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineFileStatus {
    Clean,
    Modified,
    Staged,
    Untracked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineFileEntry {
    pub path: String,
    pub status: BaselineFileStatus,
    pub content_sha256: String,
    pub file_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunDeltaEntry {
    pub path: String,
    pub baseline_status: Option<BaselineFileStatus>,
    pub baseline_sha256: Option<String>,
    pub current_status: BaselineFileStatus,
    pub current_sha256: String,
    pub current_mode: String,
}

// ---------------------------------------------------------------------------
// Two-Phase Release Authorization (§9.2, §10.1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleasePermissionIntent {
    pub requested_version_tag: String,
    pub release_repository: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConcreteReleaseTarget {
    pub version_tag: String,
    pub candidate_diff_digest: String,
    pub release_manifest_digest: String,
    pub target_commit_sha: String,
    pub release_repository: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseAuthorizationState {
    pub run_start_permission_binding: String,
    pub release_intent: Option<ReleasePermissionIntent>,
    pub candidate_release_authorization: Option<String>,
    pub concrete_release_target: Option<ConcreteReleaseTarget>,
    pub release_execution_permission_binding: Option<String>,
}

impl ReleaseAuthorizationState {
    pub fn clear_phase_two(&mut self) {
        self.candidate_release_authorization = None;
        self.concrete_release_target = None;
        self.release_execution_permission_binding = None;
    }
}

// ---------------------------------------------------------------------------
// Profiles & Capabilities (§8.1, §8.2, §8.3)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfilePolicyCapabilities {
    pub allow_stage: bool,
    pub allow_commit: bool,
    pub allow_push_branch: bool,
    pub allow_tag: bool,
    pub allow_push_tag: bool,
    pub allow_release_publish: bool,
    pub allow_paid_live_api: bool,
}

impl ProfilePolicyCapabilities {
    pub fn to_bitmask(&self) -> u32 {
        let mut mask = 0u32;
        if self.allow_stage {
            mask |= PERM_STAGE;
        }
        if self.allow_commit {
            mask |= PERM_COMMIT;
        }
        if self.allow_push_branch {
            mask |= PERM_PUSH_BRANCH;
        }
        if self.allow_tag {
            mask |= PERM_TAG;
        }
        if self.allow_push_tag {
            mask |= PERM_PUSH_TAG;
        }
        if self.allow_release_publish {
            mask |= PERM_RELEASE_PUBLISH;
        }
        if self.allow_paid_live_api {
            mask |= PERM_PAID_LIVE_API;
        }
        mask
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConvergenceLimits {
    pub max_plan_reviews: u32,
    pub max_impl_reviews: u32,
    pub max_corrective_reviews: u32,
    pub max_verification_retries: u32,
    pub max_release_reviews: u32,
    pub same_blocker_threshold: u32,
    pub max_escalations: u32,
    pub max_escalation_reviews: u32,
    pub max_transport_retries: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoConvergeProfile {
    pub id: String,
    pub name: String,
    pub is_builtin: bool,
    pub plan_review_mandatory: bool,
    pub implementation_review_mandatory: bool,
    pub corrective_plan_review_mandatory: bool,
    pub targeted_tests: bool,
    pub full_test_suite: bool,
    pub frontend_rust_build: bool,
    pub lint_and_format_check: bool,
    pub git_diff_check: bool,
    pub manual_smoke_test: bool,
    pub live_api_smoke: bool,
    pub installer_smoke: bool,
    pub artifact_verification: bool,
    pub release_readiness_review: bool,
    pub review_reuse_policy: ReviewReusePolicy,
    pub escalation_policy: EscalationPolicy,
    pub capabilities: ProfilePolicyCapabilities,
    pub convergence_limits: ConvergenceLimits,
    /// Must always be 0 in stored profiles (Invariant 14: profiles cannot pre-grant run permissions).
    #[serde(default)]
    pub pre_granted_run_permissions: u32,
}

impl AutoConvergeProfile {
    pub fn canonical_digest(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        sha256_hex(json.as_bytes())
    }
}

// ---------------------------------------------------------------------------
// Roles, Provenance, Workspace & Durable Run State (§10.1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleModelConfig {
    pub role: String,
    pub model: String,
    pub reasoning_effort: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionProvenance {
    pub session_id: String,
    pub session_role: String,
    pub producer_session_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycleCounts {
    pub plan_review_count: u32,
    pub impl_review_count: u32,
    pub corrective_review_count: u32,
    pub verification_retry_count: u32,
    pub release_review_count: u32,
    pub escalation_count: u32,
    pub escalation_review_count: u32,
    pub transport_retry_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoConvergeWorkspaceState {
    pub working_dir: String,
    pub git_head_commit: String,
    pub branch_name: String,
    pub repo_remote_url: String,
    pub baseline_dirty_hash: String,
    pub baseline_file_manifest: Vec<BaselineFileEntry>,
    pub baseline_workspace_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationEvidence {
    pub command: String,
    pub exit_code: i32,
    pub stdout_summary: String,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionLogEntry {
    pub timestamp: String,
    pub state: AutoConvergeState,
    pub actor: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserConfirmationToken {
    pub run_id: String,
    pub confirmed_reason: String,
    pub issued_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoConvergeRunConfig {
    pub run_id: Option<String>,
    pub working_dir: String,
    pub repo_remote_url: String,
    pub target_branch: String,
    pub profile: AutoConvergeProfile,
    pub run_permissions: u32,
    pub release_intent: Option<ReleasePermissionIntent>,
    pub force_escalation: bool,
    pub require_environment_verification: bool,
    #[serde(default)]
    pub delivery_phase: DeliveryPhase,
    #[serde(default)]
    pub agent_cli_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoConvergeRunState {
    pub schema_version: u32,
    pub state_sequence: u64,
    pub run_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub delivery_phase: DeliveryPhase,
    pub workspace: AutoConvergeWorkspaceState,
    pub plan_lineage: PlanLineage,
    pub runtime_allowlist: Vec<String>,
    pub allowlist_digest: Option<String>,
    pub current_diff_digest: Option<String>,
    pub current_evidence_manifest_digest: Option<String>,
    pub current_release_manifest_digest: Option<String>,
    pub allowlist_modified_since_last_impl_approval: bool,
    pub profile: AutoConvergeProfile,
    pub policy_profile_digest: String,
    pub run_permissions: u32,
    pub effective_permissions: u32,
    pub release_authorization: ReleaseAuthorizationState,
    pub session_provenance: SessionProvenance,
    pub roles: Vec<RoleModelConfig>,
    pub current_state: AutoConvergeState,
    pub is_terminal: bool,
    pub is_paused: bool,
    pub blocked_context: Option<BlockedContext>,
    pub review_mode: ReviewMode,
    pub active_producer_model: String,
    pub active_reviewer_model: String,
    pub active_reasoning_effort: String,
    pub cycle_counts: CycleCounts,
    pub findings: Vec<TrackedFinding>,
    pub approvals: Vec<ApprovalRecord>,
    pub evidence: Vec<VerificationEvidence>,
    pub completed_release_actions: Vec<String>,
    pub execution_log: Vec<ExecutionLogEntry>,
    #[serde(default)]
    pub configured_agent_cli: Option<String>,
    #[serde(default)]
    pub candidate_binding: Option<CandidateBinding>,
    #[serde(default)]
    pub promotion_journal: Option<PromotionJournalBinding>,
    pub engine_auth_tag: String,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

pub fn is_valid_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
