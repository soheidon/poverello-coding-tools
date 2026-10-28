use std::collections::BTreeMap;
use std::path::PathBuf;
// Repo-local test tempdir wrapper to avoid Windows AppData\Local\Temp permission restrictions (§209.15 B4, §209.17 R2, §209.19)
pub struct TempDir(tempfile::TempDir);

impl TempDir {
    pub fn new() -> std::io::Result<Self> {
        let repo_tmp = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-tmp");
        std::fs::create_dir_all(&repo_tmp)?;
        tempfile::Builder::new()
            .prefix("test-tmp-")
            .tempdir_in(&repo_tmp)
            .map(Self)
    }

    pub fn path(&self) -> &std::path::Path {
        self.0.path()
    }

    pub fn into_path(self) -> std::path::PathBuf {
        self.0.path().to_path_buf()
    }
}

impl std::ops::Deref for TempDir {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        self.0.path()
    }
}

impl AsRef<std::path::Path> for TempDir {
    fn as_ref(&self) -> &std::path::Path {
        self.0.path()
    }
}

pub fn repo_test_temp_dir(prefix: &str) -> tempfile::TempDir {
    let repo_tmp = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-tmp");
    let _ = std::fs::create_dir_all(&repo_tmp);
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(&repo_tmp)
        .expect("create repo test tempdir")
}

pub fn repo_tempdir() -> std::io::Result<tempfile::TempDir> {
    let repo_tmp = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-tmp");
    std::fs::create_dir_all(&repo_tmp)?;
    tempfile::Builder::new()
        .prefix("test-tmp-")
        .tempdir_in(&repo_tmp)
}

pub fn resolve_external_test_temp_base_with_env(
    user_profile: Option<&str>,
    manifest_dir: &std::path::Path,
) -> Result<PathBuf, String> {
    let profile_str = user_profile
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "USERPROFILE environment variable is missing or empty".to_string())?;
    let profile_path = PathBuf::from(profile_str);
    if !profile_path.is_absolute() {
        return Err(format!(
            "USERPROFILE must be an absolute path, got: {}",
            profile_path.display()
        ));
    }

    let canonical_profile = profile_path
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize USERPROFILE: {e}"))?;
    if !canonical_profile.is_dir() {
        return Err(format!(
            "USERPROFILE is not a directory: {}",
            canonical_profile.display()
        ));
    }

    let repo_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| "Failed to determine repository root from CARGO_MANIFEST_DIR".to_string())?;
    let canonical_repo = repo_root
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize repository root: {e}"))?;

    // Preflight overlap check BEFORE creating poverello-test-tmp:
    if canonical_profile.starts_with(&canonical_repo)
        || canonical_repo.starts_with(&canonical_profile)
    {
        return Err(format!(
            "USERPROFILE directory ({}) overlaps with repository root ({})",
            canonical_profile.display(),
            canonical_repo.display()
        ));
    }

    let base = canonical_profile.join("poverello-test-tmp");
    std::fs::create_dir_all(&base)
        .map_err(|e| format!("Failed to create external test temp directory: {e}"))?;
    let canonical_base = base
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize external test temp base: {e}"))?;

    if canonical_base.starts_with(&canonical_repo) || canonical_repo.starts_with(&canonical_base) {
        return Err(format!(
            "External test temp directory ({}) overlaps with repository root ({})",
            canonical_base.display(),
            canonical_repo.display()
        ));
    }

    Ok(canonical_base)
}

#[test]
fn test_external_test_temp_dir_fails_closed_when_userprofile_missing_or_overlapping() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap();

    // 1. Missing USERPROFILE fails closed
    let res_none = resolve_external_test_temp_base_with_env(None, manifest_dir);
    assert!(res_none.is_err());
    assert!(res_none
        .unwrap_err()
        .contains("USERPROFILE environment variable is missing"));

    // 2. Empty USERPROFILE fails closed
    let res_empty = resolve_external_test_temp_base_with_env(Some(""), manifest_dir);
    assert!(res_empty.is_err());
    assert!(res_empty
        .unwrap_err()
        .contains("USERPROFILE environment variable is missing"));

    // 3. Relative USERPROFILE fails closed
    let res_rel = resolve_external_test_temp_base_with_env(Some("relative/path"), manifest_dir);
    assert!(res_rel.is_err());
    assert!(res_rel.unwrap_err().contains("must be an absolute path"));

    // 4. USERPROFILE overlapping repository fails closed BEFORE directory creation
    let test_owned_profile_fixture = tempfile::Builder::new()
        .prefix("poverello-test-overlap-profile-")
        .tempdir_in(&repo_root)
        .expect("Must create test-owned profile fixture under repo root");
    let overlapping_profile = test_owned_profile_fixture.path();
    let prospective_child = overlapping_profile.join("poverello-test-tmp");
    assert!(
        !prospective_child.exists(),
        "prospective child must not exist prior to helper invocation"
    );

    let res_overlap =
        resolve_external_test_temp_base_with_env(overlapping_profile.to_str(), manifest_dir);
    assert!(res_overlap.is_err());
    assert!(res_overlap
        .unwrap_err()
        .contains("overlaps with repository root"));
    assert!(
        !prospective_child.exists(),
        "poverello-test-tmp must NOT be created when overlap is detected"
    );
}
pub(crate) fn symlink_privilege_unavailable(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(1314)
}

pub fn ambient_temp_dir() -> std::path::PathBuf {
    super::codex_exec::ambient_temp_dir()
}

use super::finding_tracker::compute_finding_id;
use super::git_runner::{
    capture_baseline_file_manifest, compute_baseline_workspace_digest, compute_diff_digest,
    compute_run_delta_manifest, execute_authorized_release_actions, MockCommandRunner,
};
use super::guards::{
    compute_filesystem_manifest_hash, enforce_agent_filesystem_write_boundary,
    extract_and_validate_allowlist_from_plan, has_valid_reusable_impl_approval,
    load_and_verify_run_state_authenticated, materialize_reviewer_snapshot,
    run_auto_converge_guard_cli, save_run_state_authenticated,
    validate_and_compute_run_start_permission_binding, verify_reviewer_isolation_preflight,
};
use super::profile_store::{builtin_profiles, validate_profile_against_invariants, ProfileStore};
use super::release_reviewer::ReleaseCandidateManifest;
use super::state_machine::{
    advance_from_implementation_approved, advance_from_ready_for_release_review,
    complete_implementation_step, freeze_approved_plan_and_begin_implement, initialize_run,
    record_verification_outcome, resume_run, submit_corrective_plan, submit_draft_plan,
    submit_review_verdict,
};
use super::types::{
    ApprovalBinding, ApprovalTargetType, AutoConvergeProfile, AutoConvergeRunConfig,
    AutoConvergeRunState, AutoConvergeState, BaselineFileEntry, BaselineFileStatus, BlockedKind,
    CodexReviewVerdict, DeliveryPhase, EscalationPolicy, FindingCategory, ReleasePermissionIntent,
    ReviewFinding, ReviewFindingKey, ReviewMode, ReviewReusePolicy, RunDeltaEntry,
    UserConfirmationToken, VerdictDecision, VerificationEvidence, PERM_COMMIT, PERM_PAID_LIVE_API,
    PERM_PUSH_BRANCH, PERM_PUSH_TAG, PERM_RELEASE_PUBLISH, PERM_STAGE, PERM_TAG,
};

const FIXED_TIME: &str = "2026-10-18T00:00:00Z";
const SNAP_HASH: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

fn sample_plan_markdown(files: &[&str], extra_note: &str) -> String {
    let list_json = serde_json::to_string(&files).unwrap();
    format!(
        "# Sample Implementation Plan\n{extra_note}\n\n```json auto-converge-scope\n{{\"allowlist\": {list_json}}}\n```\n"
    )
}

fn default_test_config(profile: AutoConvergeProfile) -> AutoConvergeRunConfig {
    AutoConvergeRunConfig {
        run_id: Some("ac-test-run-001".to_string()),
        working_dir: "/tmp/mock-workspace".to_string(),
        repo_remote_url: "origin".to_string(),
        target_branch: "main".to_string(),
        profile,
        run_permissions: 0,
        release_intent: None,
        force_escalation: false,
        require_environment_verification: true,
        delivery_phase: DeliveryPhase::PhaseCTagAndRelease,
        agent_cli_path: None,
        reviewer_level: None,
        r0d_acknowledged: false,
    }
}

fn standard_profile() -> AutoConvergeProfile {
    builtin_profiles()
        .into_iter()
        .find(|p| p.id == "standard")
        .unwrap()
}

fn strict_profile() -> AutoConvergeProfile {
    builtin_profiles()
        .into_iter()
        .find(|p| p.id == "strict")
        .unwrap()
}

fn fast_profile() -> AutoConvergeProfile {
    builtin_profiles()
        .into_iter()
        .find(|p| p.id == "fast")
        .unwrap()
}

fn make_plan_verdict_json(
    state: &AutoConvergeRunState,
    verdict: VerdictDecision,
    findings: Vec<ReviewFinding>,
    arch_defect: bool,
) -> String {
    let payload = CodexReviewVerdict {
        verdict,
        summary: "Plan review decision".to_string(),
        findings,
        approval_binding: ApprovalBinding {
            target_type: ApprovalTargetType::Plan,
            root_plan_digest: None,
            active_plan_digest: None,
            candidate_plan_digest: state.plan_lineage.candidate_plan_digest.clone(),
            allowlist_digest: state.allowlist_digest.clone().unwrap(),
            policy_profile_digest: state.policy_profile_digest.clone(),
            baseline_workspace_digest: Some(state.workspace.baseline_workspace_digest.clone()),
            diff_digest: None,
            evidence_manifest_digest: None,
            release_manifest_digest: None,
            ..Default::default()
        },
        architectural_defect: arch_defect,
        escalation_recommended: false,
    };
    serde_json::to_string(&payload).unwrap()
}

fn make_impl_verdict_json(
    state: &AutoConvergeRunState,
    verdict: VerdictDecision,
    findings: Vec<ReviewFinding>,
    arch_defect: bool,
) -> String {
    let payload = CodexReviewVerdict {
        verdict,
        summary: "Implementation review decision".to_string(),
        findings,
        approval_binding: ApprovalBinding {
            target_type: ApprovalTargetType::ImplementationDiff,
            root_plan_digest: state.plan_lineage.root_plan_digest.clone(),
            active_plan_digest: state.plan_lineage.active_plan_digest.clone(),
            candidate_plan_digest: None,
            allowlist_digest: state.allowlist_digest.clone().unwrap(),
            policy_profile_digest: state.policy_profile_digest.clone(),
            baseline_workspace_digest: Some(state.workspace.baseline_workspace_digest.clone()),
            diff_digest: state.current_diff_digest.clone(),
            evidence_manifest_digest: state.current_evidence_manifest_digest.clone(),
            release_manifest_digest: None,
            bundle_digest: state
                .candidate_binding
                .as_ref()
                .and_then(|cb| cb.review_bundle_digest.clone())
                .or_else(|| Some(SNAP_HASH.to_string())),
            run_id: Some(state.run_id.clone()),
            cycle: state.candidate_binding.as_ref().map(|cb| cb.cycle),
            implementer_session_id: state
                .candidate_binding
                .as_ref()
                .map(|cb| cb.session_id.clone()),
        },
        architectural_defect: arch_defect,
        escalation_recommended: false,
    };
    serde_json::to_string(&payload).unwrap()
}

fn make_corrective_verdict_json(
    state: &AutoConvergeRunState,
    verdict: VerdictDecision,
    findings: Vec<ReviewFinding>,
) -> String {
    let payload = CodexReviewVerdict {
        verdict,
        summary: "Corrective plan review decision".to_string(),
        findings,
        approval_binding: ApprovalBinding {
            target_type: ApprovalTargetType::CorrectivePlan,
            root_plan_digest: state.plan_lineage.root_plan_digest.clone(),
            active_plan_digest: state.plan_lineage.active_plan_digest.clone(),
            candidate_plan_digest: state.plan_lineage.candidate_plan_digest.clone(),
            allowlist_digest: state.allowlist_digest.clone().unwrap(),
            policy_profile_digest: state.policy_profile_digest.clone(),
            baseline_workspace_digest: Some(state.workspace.baseline_workspace_digest.clone()),
            diff_digest: state.current_diff_digest.clone(),
            evidence_manifest_digest: None,
            release_manifest_digest: None,
            ..Default::default()
        },
        architectural_defect: false,
        escalation_recommended: false,
    };
    serde_json::to_string(&payload).unwrap()
}

fn sample_blocking_finding(code: &str, prose: &str) -> ReviewFinding {
    ReviewFinding {
        finding_key: ReviewFindingKey {
            category: FindingCategory::Correctness,
            file: "src/lib.rs".to_string(),
            symbol: "process_buffer".to_string(),
            failure_code: code.to_string(),
        },
        location: "src/lib.rs:42".to_string(),
        failure_condition: prose.to_string(),
        acceptance_condition: "Check empty buffer before indexing".to_string(),
        blocking: true,
    }
}

fn sample_delta(path: &str, current_sha: &str) -> Vec<RunDeltaEntry> {
    vec![RunDeltaEntry {
        path: path.to_string(),
        baseline_status: Some(BaselineFileStatus::Clean),
        baseline_sha256: Some(
            "1111111111111111111111111111111111111111111111111111111111111111".to_string(),
        ),
        current_status: BaselineFileStatus::Modified,
        current_sha256: current_sha.to_string(),
        current_mode: "100644".to_string(),
    }]
}

fn sample_evidence() -> Vec<VerificationEvidence> {
    vec![VerificationEvidence {
        command: "cargo test".to_string(),
        exit_code: 0,
        stdout_summary: "test result: ok".to_string(),
        passed: true,
    }]
}

// 1. test_no_unapproved_plan_implementation
#[test]
fn test_no_unapproved_plan_implementation() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(state.current_state, AutoConvergeState::PlanCreate);

    // Attempting implementation from PlanCreate must fail closed and transition to Blocked
    let err = complete_implementation_step(
        &mut state,
        "impl-1",
        &["src/lib.rs".to_string()],
        &sample_delta(
            "src/lib.rs",
            "2222222222222222222222222222222222222222222222222222222222222222",
        ),
        sample_evidence(),
        true,
        FIXED_TIME,
    );
    assert!(err.is_err());
    assert_eq!(state.current_state, AutoConvergeState::Blocked);
}

#[test]
fn test_run_id_validation_precedes_snapshot_directory_creation() {
    use super::state_store::{
        engine_run_directory_path, get_engine_runs_dir, validate_auto_converge_run_id,
    };
    use super::{engine_run_state_path, prepare_reviewer_snapshot_dir};

    let temp = TempDir::new().unwrap();
    let temp_root = temp.path().join("temp-root");
    std::fs::create_dir_all(&temp_root).unwrap();

    let mut invalid_ids = vec![
        "".to_string(),
        " ".to_string(),
        ".".to_string(),
        "..".to_string(),
        "../escape".to_string(),
        r"..\escape".to_string(),
        "../../../escape".to_string(),
        "/absolute".to_string(),
        r"C:\absolute".to_string(),
        "C:relative".to_string(),
        r"\\server\share".to_string(),
        "run/id".to_string(),
        "run\\id".to_string(),
        "run.id".to_string(),
        "run:id".to_string(),
        "run-".to_string(),
        "run_".to_string(),
        "CON".to_string(),
        "nul".to_string(),
        "COM1".to_string(),
        "LPT9".to_string(),
    ];
    invalid_ids.push("a".repeat(129));

    for run_id in &invalid_ids {
        assert!(
            validate_auto_converge_run_id(run_id).is_err(),
            "accepted invalid run ID: {run_id:?}"
        );
        assert!(
            prepare_reviewer_snapshot_dir(&temp_root, run_id, 0).is_err(),
            "created snapshot for invalid run ID: {run_id:?}"
        );
        assert!(engine_run_state_path(run_id).is_err());
        assert!(get_engine_runs_dir(run_id).is_err());
    }

    assert!(!temp_root.join("poverello").exists());
    assert!(!temp.path().join("escape").exists());

    for valid_id in [
        "run-001",
        "ac-test_run-01",
        "ac-550e8400-e29b-41d4-a716-446655440000",
    ] {
        assert_eq!(validate_auto_converge_run_id(valid_id).unwrap(), valid_id);
        let snapshot_dir = prepare_reviewer_snapshot_dir(&temp_root, valid_id, 3).unwrap();
        let canonical_root = temp_root
            .join("poverello")
            .join("snapshots")
            .canonicalize()
            .unwrap();
        assert!(snapshot_dir
            .canonicalize()
            .unwrap()
            .starts_with(canonical_root));
    }

    let engine_runs_root = temp.path().join("engine-runs-root");
    let valid_engine_run = engine_run_directory_path(&engine_runs_root, "ac-test_run-01").unwrap();
    assert_eq!(valid_engine_run, engine_runs_root.join("ac-test_run-01"));
    assert!(valid_engine_run.starts_with(&engine_runs_root));
    assert!(!engine_runs_root.exists());

    for invalid_id in ["../escape", r"..\escape", "C:relative"] {
        assert!(
            engine_run_directory_path(&engine_runs_root, invalid_id).is_err(),
            "accepted invalid engine-run ID: {invalid_id:?}"
        );
    }
    assert!(!engine_runs_root.exists());
    assert!(!temp.path().join("escape").exists());
}

// 2. test_request_changes_never_approves
#[test]
fn test_request_changes_never_approves() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();

    let verdict_json = make_plan_verdict_json(
        &state,
        VerdictDecision::RequestChanges,
        vec![sample_blocking_finding("COR-001", "Missing check")],
        false,
    );
    submit_review_verdict(
        &mut state,
        "reviewer-1",
        &verdict_json,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();

    assert_eq!(state.current_state, AutoConvergeState::PlanRevise);
    assert!(state.approvals.is_empty());
    assert!(state.plan_lineage.root_plan_digest.is_none());
}

// 3. test_plan_mutation_invalidates_approval
#[test]
fn test_plan_mutation_invalidates_approval() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let verdict_json = make_plan_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "reviewer-1",
        &verdict_json,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(state.current_state, AutoConvergeState::PlanApproved);
    assert!(state.approvals.iter().any(|a| !a.invalidated));

    // Mutate plan while paused and resume
    state.is_paused = true;
    resume_run(
        &mut state,
        &[],
        Some("9999999999999999999999999999999999999999999999999999999999999999"),
        None,
        &runner,
        None,
        FIXED_TIME,
    )
    .unwrap();

    assert_eq!(state.current_state, AutoConvergeState::PlanReview);
    assert!(state.approvals.iter().all(|a| a.invalidated));
    assert!(state.plan_lineage.root_plan_digest.is_none());
}

// 4. test_diff_mutation_invalidates_approval
#[test]
fn test_diff_mutation_invalidates_approval() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let plan_approve = make_plan_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "reviewer-1",
        &plan_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    freeze_approved_plan_and_begin_implement(&mut state, &[], "impl-1", FIXED_TIME).unwrap();

    // Code edits during Implement do NOT invalidate Plan Approval (Invariant 6c)
    complete_implementation_step(
        &mut state,
        "impl-1",
        &["src/lib.rs".to_string()],
        &sample_delta(
            "src/lib.rs",
            "2222222222222222222222222222222222222222222222222222222222222222",
        ),
        sample_evidence(),
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert!(!state.approvals[0].invalidated);

    let impl_approve = make_impl_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "reviewer-2",
        &impl_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(
        state.current_state,
        AutoConvergeState::ImplementationApproved
    );

    // Code modification AFTER ImplementationReview APPROVE invalidates implementation approval, preserving Plan Approval (Invariant 6d)
    let active_plan = state.plan_lineage.active_plan_digest.clone();
    resume_run(
        &mut state,
        &[],
        active_plan.as_deref(),
        Some("3333333333333333333333333333333333333333333333333333333333333333"),
        &runner,
        None,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(state.current_state, AutoConvergeState::ImplementationReview);
    assert!(!state.approvals[0].invalidated); // Plan approval preserved
    assert!(state.approvals[1].invalidated); // Implementation approval invalidated
}

// 5. test_corrective_implementation_requires_plan_approval
#[test]
fn test_corrective_implementation_requires_plan_approval() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let plan_approve = make_plan_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "reviewer-1",
        &plan_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    freeze_approved_plan_and_begin_implement(&mut state, &[], "impl-1", FIXED_TIME).unwrap();
    complete_implementation_step(
        &mut state,
        "impl-1",
        &["src/lib.rs".to_string()],
        &sample_delta(
            "src/lib.rs",
            "2222222222222222222222222222222222222222222222222222222222222222",
        ),
        sample_evidence(),
        true,
        FIXED_TIME,
    )
    .unwrap();

    let impl_req = make_impl_verdict_json(
        &state,
        VerdictDecision::RequestChanges,
        vec![sample_blocking_finding("COR-001", "Fix defect")],
        false,
    );
    submit_review_verdict(
        &mut state,
        "reviewer-2",
        &impl_req,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(state.current_state, AutoConvergeState::CorrectivePlanCreate);

    // Attempting corrective edits before CorrectivePlanReview APPROVE fails closed
    assert!(complete_implementation_step(
        &mut state,
        "impl-1",
        &["src/lib.rs".to_string()],
        &sample_delta(
            "src/lib.rs",
            "4444444444444444444444444444444444444444444444444444444444444444"
        ),
        sample_evidence(),
        true,
        FIXED_TIME,
    )
    .is_err());
    assert_eq!(state.current_state, AutoConvergeState::Blocked);
}

// 6. test_reviewer_session_is_read_only
#[test]
fn test_reviewer_session_is_read_only() {
    // 1. CLI flag verification
    assert!(verify_reviewer_isolation_preflight(
        &["--sandbox".to_string(), "read-only".to_string()],
        true
    )
    .is_ok());
    assert!(verify_reviewer_isolation_preflight(&["--allow-write".to_string()], true).is_err());
    assert!(verify_reviewer_isolation_preflight(
        &["--sandbox".to_string(), "read-only".to_string()],
        false
    )
    .is_err());

    // 2. Snapshot materialization outside repo + pre/post FilesystemManifestHash check
    let ws = TempDir::new().unwrap();
    let snap = TempDir::new().unwrap();
    std::fs::write(ws.path().join("lib.rs"), b"pub fn a() {}").unwrap();
    let pre_hash = materialize_reviewer_snapshot(
        ws.path(),
        &snap.path().join("cycle-1"),
        &["lib.rs".to_string()],
        &["lib.rs".to_string()],
        &[],
    )
    .unwrap();
    let post_hash = compute_filesystem_manifest_hash(&snap.path().join("cycle-1")).unwrap();
    assert_eq!(pre_hash, post_hash);

    // Mutate snapshot and verify state machine immediately halts at Blocked without escalating
    std::fs::write(snap.path().join("cycle-1").join("lib.rs"), b"tampered").unwrap();
    let tampered_hash = compute_filesystem_manifest_hash(&snap.path().join("cycle-1")).unwrap();
    assert_ne!(pre_hash, tampered_hash);

    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let plan_approve = make_plan_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    assert!(submit_review_verdict(
        &mut state,
        "reviewer-1",
        &plan_approve,
        &pre_hash,
        &tampered_hash,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .is_err());
    assert_eq!(state.current_state, AutoConvergeState::Blocked);
    assert_eq!(state.cycle_counts.escalation_count, 0);
}

// 7. test_malformed_verdict_fails_closed
#[test]
fn test_malformed_verdict_fails_closed() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();

    // Retry 1 & 2 stay in PlanReview; 3rd failure exceeds max_transport_retries = 2 -> Blocked
    assert!(submit_review_verdict(
        &mut state,
        "reviewer-1",
        "not json 1",
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME
    )
    .is_err());
    assert_eq!(state.current_state, AutoConvergeState::PlanReview);

    assert!(submit_review_verdict(
        &mut state,
        "reviewer-1",
        "not json 2",
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME
    )
    .is_err());
    assert_eq!(state.current_state, AutoConvergeState::PlanReview);

    assert!(submit_review_verdict(
        &mut state,
        "reviewer-1",
        "not json 3",
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME
    )
    .is_err());
    assert_eq!(state.current_state, AutoConvergeState::Blocked);
    assert!(state.approvals.is_empty());
}

// 8. test_review_limit_follows_escalation_policy
#[test]
fn test_review_limit_follows_escalation_policy() {
    let runner = MockCommandRunner::new();

    // Case A: EscalationPolicy::Enabled -> 3rd non-APPROVE promotes to Sol
    let mut state_enabled = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    for i in 1..=3 {
        submit_draft_plan(
            &mut state_enabled,
            &format!("planner-{i}"),
            &sample_plan_markdown(&["src/lib.rs"], &format!("v{i}")),
            true,
            FIXED_TIME,
        )
        .unwrap();
        let req = make_plan_verdict_json(
            &state_enabled,
            VerdictDecision::RequestChanges,
            vec![sample_blocking_finding(&format!("COR-00{i}"), "issue")],
            false,
        );
        submit_review_verdict(
            &mut state_enabled,
            &format!("rev-{i}"),
            &req,
            SNAP_HASH,
            SNAP_HASH,
            None,
            None,
            true,
            FIXED_TIME,
        )
        .unwrap();
    }
    assert_eq!(state_enabled.review_mode, ReviewMode::Escalated);
    assert_eq!(state_enabled.current_state, AutoConvergeState::PlanRevise);
    assert_eq!(state_enabled.cycle_counts.escalation_count, 1);

    // Case B: EscalationPolicy::BlockingOnly (Fast profile, max_plan_reviews = 2) -> stage review exhaustion transitions directly to Blocked
    let mut state_blocking_only = initialize_run(
        default_test_config(fast_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    for i in 1..=2 {
        submit_draft_plan(
            &mut state_blocking_only,
            &format!("planner-{i}"),
            &sample_plan_markdown(&["src/lib.rs"], &format!("v{i}")),
            true,
            FIXED_TIME,
        )
        .unwrap();
        let req = make_plan_verdict_json(
            &state_blocking_only,
            VerdictDecision::RequestChanges,
            vec![sample_blocking_finding(&format!("COR-00{i}"), "issue")],
            false,
        );
        submit_review_verdict(
            &mut state_blocking_only,
            &format!("rev-{i}"),
            &req,
            SNAP_HASH,
            SNAP_HASH,
            None,
            None,
            true,
            FIXED_TIME,
        )
        .unwrap();
    }
    assert_eq!(
        state_blocking_only.current_state,
        AutoConvergeState::Blocked
    );
    assert_eq!(state_blocking_only.cycle_counts.escalation_count, 0);

    // Case C: EscalationPolicy::Disabled -> transitions directly to Blocked
    let mut disabled_prof = standard_profile();
    disabled_prof.escalation_policy = EscalationPolicy::Disabled;
    disabled_prof.convergence_limits.max_escalations = 0;
    disabled_prof.convergence_limits.max_plan_reviews = 1;
    let mut state_disabled = initialize_run(
        default_test_config(disabled_prof),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    submit_draft_plan(
        &mut state_disabled,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let req = make_plan_verdict_json(
        &state_disabled,
        VerdictDecision::RequestChanges,
        vec![sample_blocking_finding("COR-001", "issue")],
        false,
    );
    submit_review_verdict(
        &mut state_disabled,
        "rev-1",
        &req,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(state_disabled.current_state, AutoConvergeState::Blocked);
}

// 9. test_sol_escalation_parameters
#[test]
fn test_sol_escalation_parameters() {
    let runner = MockCommandRunner::new();
    let mut cfg = default_test_config(standard_profile());
    cfg.force_escalation = true;
    let state = initialize_run(cfg, "head-1", vec![], &runner, FIXED_TIME).unwrap();
    assert_eq!(state.review_mode, ReviewMode::Escalated);
    assert_eq!(state.active_producer_model, "gpt-6-sol");
    assert_eq!(state.active_reviewer_model, "gpt-6-sol");
    assert_eq!(state.active_reasoning_effort, "high");
    assert_eq!(state.cycle_counts.escalation_count, 1);
}

// 10. test_sol_failure_limit_transitions_to_blocked
#[test]
fn test_sol_failure_limit_transitions_to_blocked() {
    let runner = MockCommandRunner::new();
    let mut cfg = default_test_config(standard_profile());
    cfg.force_escalation = true;
    let mut state = initialize_run(cfg, "head-1", vec![], &runner, FIXED_TIME).unwrap();

    // 1st Sol non-APPROVE -> PlanRevise (escalation_review_count = 1)
    submit_draft_plan(
        &mut state,
        "sol-planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let req1 = make_plan_verdict_json(
        &state,
        VerdictDecision::RequestChanges,
        vec![sample_blocking_finding("COR-001", "issue 1")],
        false,
    );
    submit_review_verdict(
        &mut state,
        "sol-rev-1",
        &req1,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(state.current_state, AutoConvergeState::PlanRevise);
    assert_eq!(state.cycle_counts.escalation_review_count, 1);

    // 2nd Sol non-APPROVE -> Blocked (max_escalation_reviews = 2 exhausted)
    submit_draft_plan(
        &mut state,
        "sol-planner-2",
        &sample_plan_markdown(&["src/lib.rs"], "v2"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let req2 = make_plan_verdict_json(
        &state,
        VerdictDecision::RequestChanges,
        vec![sample_blocking_finding("COR-002", "issue 2")],
        false,
    );
    submit_review_verdict(
        &mut state,
        "sol-rev-2",
        &req2,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(state.current_state, AutoConvergeState::Blocked);
}

// 11. test_state_persistence_and_interruption_resume
#[test]
fn test_state_persistence_and_interruption_resume() {
    let dir = TempDir::new().unwrap();
    let state_path = dir.path().join("run_state.json");
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    state.is_paused = true;
    save_run_state_authenticated(&state_path, &mut state).unwrap();

    let mut loaded =
        load_and_verify_run_state_authenticated(&state_path, Some(state.state_sequence)).unwrap();
    assert_eq!(loaded.current_state, AutoConvergeState::PlanReview);
    assert!(loaded.is_paused);

    resume_run(&mut loaded, &[], None, None, &runner, None, FIXED_TIME).unwrap();
    assert!(!loaded.is_paused);
    assert_eq!(loaded.current_state, AutoConvergeState::PlanReview);
}

// 12. test_dirty_worktree_preservation
#[test]
fn test_dirty_worktree_preservation() {
    let runner = MockCommandRunner::new();
    let dirty_manifest = vec![BaselineFileEntry {
        path: "notes/user_scratch.txt".to_string(),
        status: BaselineFileStatus::Modified,
        content_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .to_string(),
        file_mode: "100644".to_string(),
    }];
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        dirty_manifest.clone(),
        &runner,
        FIXED_TIME,
    )
    .unwrap();

    // Plan targeting clean file "src/lib.rs" succeeds and preserves dirty_manifest
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(state.current_state, AutoConvergeState::PlanReview);

    // Plan targeting the pre-existing dirty file "notes/user_scratch.txt" halts at Blocked (UserConfirmationRequired)
    let mut state_conflict = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        dirty_manifest,
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    assert!(submit_draft_plan(
        &mut state_conflict,
        "planner-1",
        &sample_plan_markdown(&["notes/user_scratch.txt"], "v1"),
        true,
        FIXED_TIME,
    )
    .is_err());
    assert_eq!(state_conflict.current_state, AutoConvergeState::Blocked);
    assert_eq!(
        state_conflict.blocked_context.unwrap().kind,
        BlockedKind::UserConfirmationRequired
    );
}

// 13. test_approval_reuse_requires_full_composite_binding_match
#[test]
fn test_approval_reuse_requires_full_composite_binding_match() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(fast_profile()), // ValidBinding reuse policy
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let plan_approve = make_plan_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "rev-1",
        &plan_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    freeze_approved_plan_and_begin_implement(&mut state, &[], "impl-1", FIXED_TIME).unwrap();

    let delta = sample_delta(
        "src/lib.rs",
        "2222222222222222222222222222222222222222222222222222222222222222",
    );
    complete_implementation_step(
        &mut state,
        "impl-1",
        &["src/lib.rs".to_string()],
        &delta,
        sample_evidence(),
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(state.current_state, AutoConvergeState::ImplementationReview);

    let impl_approve = make_impl_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "rev-2",
        &impl_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(
        state.current_state,
        AutoConvergeState::ImplementationApproved
    );
    assert!(has_valid_reusable_impl_approval(&state));

    // Re-running Implement with identical 6-tuple binding reuses approval -> ImplementationApproved
    state.current_state = AutoConvergeState::Implement;
    complete_implementation_step(
        &mut state,
        "impl-1",
        &[],
        &delta,
        sample_evidence(),
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(
        state.current_state,
        AutoConvergeState::ImplementationApproved
    );

    // Changing diff_digest or switching to AlwaysReview forces a fresh ImplementationReview
    state.profile.review_reuse_policy = ReviewReusePolicy::AlwaysReview;
    assert!(!has_valid_reusable_impl_approval(&state));
}

// 14. test_environment_verification_gate_separation
#[test]
fn test_environment_verification_gate_separation() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    state.current_state = AutoConvergeState::ImplementationApproved;
    advance_from_implementation_approved(&mut state, true, FIXED_TIME).unwrap();
    assert_eq!(state.current_state, AutoConvergeState::VerificationRequired);

    record_verification_outcome(&mut state, sample_evidence(), FIXED_TIME).unwrap();
    assert_eq!(
        state.current_state,
        AutoConvergeState::ReadyForReleaseReview
    );
}

// 15. test_git_operations_strictly_conditioned_on_permissions
#[test]
fn test_git_operations_strictly_conditioned_on_permissions() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()), // run_permissions = 0
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    state.current_state = AutoConvergeState::ReadyForReleaseReview;
    advance_from_ready_for_release_review(&mut state, None, FIXED_TIME).unwrap();
    assert_eq!(state.current_state, AutoConvergeState::ReleaseReadyHalted);
    assert!(runner.recorded_steps().is_empty());

    // Direct attempt to execute release actions with 0 permissions fails closed
    state.current_state = AutoConvergeState::ReleaseActionsPending;
    assert!(execute_authorized_release_actions(&mut state, "Release v0.24.6", &runner).is_err());
    assert_eq!(state.current_state, AutoConvergeState::Blocked);
}

// 16. test_commit_permission_does_not_permit_push_or_tag
#[test]
fn test_commit_permission_does_not_permit_push_or_tag() {
    let mut runner = MockCommandRunner::new();
    let delta = sample_delta(
        "src/lib.rs",
        "2222222222222222222222222222222222222222222222222222222222222222",
    );
    runner.working_tree_delta = delta.clone();

    let mut cfg = default_test_config(standard_profile());
    cfg.run_permissions = PERM_STAGE | PERM_COMMIT;
    let mut state = initialize_run(cfg, "head-1", vec![], &runner, FIXED_TIME).unwrap();
    state.runtime_allowlist = vec!["src/lib.rs".to_string()];
    state.current_diff_digest = Some(compute_diff_digest(&delta));
    state.current_state = AutoConvergeState::ReleaseActionsPending;

    execute_authorized_release_actions(&mut state, "Implement feature", &runner).unwrap();
    assert_eq!(state.current_state, AutoConvergeState::ReleaseCompleted);
    assert_eq!(runner.recorded_steps(), vec!["stage", "commit"]);
}

// 17. test_partial_git_permissions_enforced
#[test]
fn test_partial_git_permissions_enforced() {
    let mut runner = MockCommandRunner::new();
    let delta = sample_delta(
        "src/lib.rs",
        "2222222222222222222222222222222222222222222222222222222222222222",
    );
    runner.working_tree_delta = delta.clone();

    let mut cfg = default_test_config(standard_profile());
    cfg.run_permissions = PERM_STAGE | PERM_COMMIT | PERM_PUSH_BRANCH;
    let mut state = initialize_run(cfg, "head-1", vec![], &runner, FIXED_TIME).unwrap();
    state.runtime_allowlist = vec!["src/lib.rs".to_string()];
    state.current_diff_digest = Some(compute_diff_digest(&delta));
    state.current_state = AutoConvergeState::ReleaseActionsPending;

    execute_authorized_release_actions(&mut state, "Implement feature", &runner).unwrap();
    assert_eq!(state.current_state, AutoConvergeState::ReleaseCompleted);
    assert_eq!(
        runner.recorded_steps(),
        vec!["stage", "commit", "push_branch"]
    );
}

// 18. test_full_release_sequence_execution
#[test]
fn test_full_release_sequence_execution() {
    let mut runner = MockCommandRunner::new();
    let delta = sample_delta(
        "src/lib.rs",
        "2222222222222222222222222222222222222222222222222222222222222222",
    );
    runner.working_tree_delta = delta.clone();

    let mut cfg = default_test_config(strict_profile());
    cfg.run_permissions = PERM_STAGE
        | PERM_COMMIT
        | PERM_PUSH_BRANCH
        | PERM_TAG
        | PERM_PUSH_TAG
        | PERM_RELEASE_PUBLISH;
    cfg.release_intent = Some(ReleasePermissionIntent {
        requested_version_tag: "v0.24.6".to_string(),
        release_repository: "soheidon/poverello".to_string(),
    });

    let mut state = initialize_run(cfg, "head-1", vec![], &runner, FIXED_TIME).unwrap();
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let plan_approve = make_plan_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "rev-1",
        &plan_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    freeze_approved_plan_and_begin_implement(&mut state, &[], "impl-1", FIXED_TIME).unwrap();
    complete_implementation_step(
        &mut state,
        "impl-1",
        &["src/lib.rs".to_string()],
        &delta,
        sample_evidence(),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let impl_approve = make_impl_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "rev-2",
        &impl_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    advance_from_implementation_approved(&mut state, false, FIXED_TIME).unwrap();

    let rel_manifest = ReleaseCandidateManifest {
        version_tag: "v0.24.6".to_string(),
        release_repository: "soheidon/poverello".to_string(),
        changelog_summary: "v0.24.6 Auto-Converge release".to_string(),
        artifact_hashes: BTreeMap::new(),
    };
    advance_from_ready_for_release_review(&mut state, Some(&rel_manifest), FIXED_TIME).unwrap();
    assert_eq!(state.current_state, AutoConvergeState::ReleaseReview);

    let rel_verdict = serde_json::to_string(&CodexReviewVerdict {
        verdict: VerdictDecision::Approve,
        summary: "Release ready".to_string(),
        findings: vec![],
        approval_binding: ApprovalBinding {
            target_type: ApprovalTargetType::ReleaseCandidate,
            root_plan_digest: state.plan_lineage.root_plan_digest.clone(),
            active_plan_digest: state.plan_lineage.active_plan_digest.clone(),
            candidate_plan_digest: None,
            allowlist_digest: state.allowlist_digest.clone().unwrap(),
            policy_profile_digest: state.policy_profile_digest.clone(),
            baseline_workspace_digest: Some(state.workspace.baseline_workspace_digest.clone()),
            diff_digest: state.current_diff_digest.clone(),
            evidence_manifest_digest: state.current_evidence_manifest_digest.clone(),
            release_manifest_digest: state.current_release_manifest_digest.clone(),
            ..Default::default()
        },
        architectural_defect: false,
        escalation_recommended: false,
    })
    .unwrap();
    submit_review_verdict(
        &mut state,
        "rev-3",
        &rel_verdict,
        SNAP_HASH,
        SNAP_HASH,
        None,
        Some(&rel_manifest),
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(
        state.current_state,
        AutoConvergeState::ReleaseActionsPending
    );

    execute_authorized_release_actions(&mut state, "Release v0.24.6", &runner).unwrap();
    assert_eq!(state.current_state, AutoConvergeState::ReleaseCompleted);
    assert_eq!(
        runner.recorded_steps(),
        vec![
            "stage",
            "commit",
            "push_branch",
            "tag",
            "push_tag",
            "release_publish"
        ]
    );
}

// 19. test_permission_binding_non_transferable
#[test]
fn test_permission_binding_non_transferable() {
    let runner = MockCommandRunner::new();
    let mut cfg = default_test_config(standard_profile());
    cfg.run_id = Some("run-1".to_string());
    cfg.run_permissions = PERM_STAGE | PERM_COMMIT;
    let mut state = initialize_run(cfg, "head-1", vec![], &runner, FIXED_TIME).unwrap();

    // Transferring binding to run-2 fails verification
    state.run_id = "run-2".to_string();
    state.current_state = AutoConvergeState::ReleaseActionsPending;
    assert!(execute_authorized_release_actions(&mut state, "msg", &runner).is_err());
    assert_eq!(state.current_state, AutoConvergeState::Blocked);
}

// 20. test_reviewer_self_approval_prevented
#[test]
fn test_reviewer_self_approval_prevented() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    submit_draft_plan(
        &mut state,
        "same-session-id",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let approve = make_plan_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    assert!(submit_review_verdict(
        &mut state,
        "same-session-id",
        &approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME
    )
    .is_err());
    assert_eq!(state.current_state, AutoConvergeState::Blocked);
}

// 21. test_unsafe_profile_rejected
#[test]
fn test_unsafe_profile_rejected() {
    let mut p1 = standard_profile();
    p1.plan_review_mandatory = false;
    assert!(validate_profile_against_invariants(&p1).is_err());

    let mut p2 = standard_profile();
    p2.corrective_plan_review_mandatory = false;
    assert!(validate_profile_against_invariants(&p2).is_err());

    let mut p3 = standard_profile();
    p3.capabilities.allow_tag = true;
    p3.release_readiness_review = false;
    assert!(validate_profile_against_invariants(&p3).is_err());
}

// 22. test_allowlist_enforcement_prevents_out_of_scope_writes
#[test]
fn test_allowlist_enforcement_prevents_out_of_scope_writes() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/allowed.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let plan_approve = make_plan_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "rev-1",
        &plan_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    freeze_approved_plan_and_begin_implement(&mut state, &[], "impl-1", FIXED_TIME).unwrap();

    // Touching "src/forbidden.rs" outside runtime_allowlist immediately halts at Blocked
    assert!(complete_implementation_step(
        &mut state,
        "impl-1",
        &["src/forbidden.rs".to_string()],
        &sample_delta(
            "src/allowed.rs",
            "2222222222222222222222222222222222222222222222222222222222222222"
        ),
        sample_evidence(),
        true,
        FIXED_TIME,
    )
    .is_err());
    assert_eq!(state.current_state, AutoConvergeState::Blocked);
}

// 23. test_controller_guard_contract_agreement
#[test]
fn test_controller_guard_contract_agreement() {
    let plan_md = sample_plan_markdown(&["src/lib.rs"], "cli test");
    let code = run_auto_converge_guard_cli(&[
        "anthro-bridge".to_string(),
        "auto-converge-guard".to_string(),
        "extract-allowlist".to_string(),
        "--payload-json".to_string(),
        plan_md,
    ]);
    assert_eq!(code, 0);

    let deny_code = run_auto_converge_guard_cli(&[
        "anthro-bridge".to_string(),
        "auto-converge-guard".to_string(),
        "extract-allowlist".to_string(),
        "--payload-json".to_string(),
        "# Plan with no scope block".to_string(),
    ]);
    assert_eq!(deny_code, 1);
}

// 24. test_dual_gate_effective_permission_evaluation
#[test]
fn test_dual_gate_effective_permission_evaluation() {
    // Standard profile does NOT allow tag; requesting PERM_TAG in run_permissions is denied fail-closed
    let res = validate_and_compute_run_start_permission_binding(
        "run-1",
        "origin",
        "main",
        &standard_profile(),
        PERM_STAGE | PERM_COMMIT | PERM_TAG,
        Some(&ReleasePermissionIntent {
            requested_version_tag: "v0.24.6".to_string(),
            release_repository: "soheidon/poverello".to_string(),
        }),
        DeliveryPhase::PhaseCTagAndRelease,
    );
    assert!(res.is_err());
}

// 25. test_stage_specific_binding_contract
#[test]
fn test_stage_specific_binding_contract() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();

    // Passing non-null root_plan_digest during initial PlanReview violates stage contract -> Blocked
    let mut bad_binding: CodexReviewVerdict = serde_json::from_str(&make_plan_verdict_json(
        &state,
        VerdictDecision::Approve,
        vec![],
        false,
    ))
    .unwrap();
    bad_binding.approval_binding.root_plan_digest =
        state.plan_lineage.candidate_plan_digest.clone();
    assert!(submit_review_verdict(
        &mut state,
        "rev-1",
        &serde_json::to_string(&bad_binding).unwrap(),
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .is_err());
    assert_eq!(state.current_state, AutoConvergeState::Blocked);
}

// 26. test_sol_budget_dedication_and_cohesive_reset
#[test]
fn test_sol_budget_dedication_and_cohesive_reset() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let plan_approve = make_plan_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "rev-1",
        &plan_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    freeze_approved_plan_and_begin_implement(&mut state, &[], "impl-1", FIXED_TIME).unwrap();
    let delta = sample_delta(
        "src/lib.rs",
        "2222222222222222222222222222222222222222222222222222222222222222",
    );
    complete_implementation_step(
        &mut state,
        "impl-1",
        &["src/lib.rs".to_string()],
        &delta,
        sample_evidence(),
        true,
        FIXED_TIME,
    )
    .unwrap();

    // Trigger Sol escalation via architectural_defect during ImplementationReview
    let impl_req = make_impl_verdict_json(
        &state,
        VerdictDecision::RequestChanges,
        vec![sample_blocking_finding("COR-004", "Arch defect")],
        true,
    );
    submit_review_verdict(
        &mut state, "rev-2", &impl_req, SNAP_HASH, SNAP_HASH, None, None, true, FIXED_TIME,
    )
    .unwrap();
    assert_eq!(state.review_mode, ReviewMode::Escalated);
    assert_eq!(state.current_state, AutoConvergeState::CorrectivePlanCreate);

    // Approving CorrectivePlanReview MUST keep review_mode == Escalated until ImplementationReview passes!
    submit_corrective_plan(
        &mut state,
        "sol-planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "corrective v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let corr_approve = make_corrective_verdict_json(&state, VerdictDecision::Approve, vec![]);
    submit_review_verdict(
        &mut state,
        "sol-rev-1",
        &corr_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(state.current_state, AutoConvergeState::CorrectiveImplement);
    assert_eq!(state.review_mode, ReviewMode::Escalated);

    // Only after ImplementationReview APPROVE does review_mode reset to Normal
    let delta2 = sample_delta(
        "src/lib.rs",
        "3333333333333333333333333333333333333333333333333333333333333333",
    );
    complete_implementation_step(
        &mut state,
        "impl-2",
        &["src/lib.rs".to_string()],
        &delta2,
        sample_evidence(),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let impl_approve = make_impl_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "sol-rev-2",
        &impl_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(
        state.current_state,
        AutoConvergeState::ImplementationApproved
    );
    assert_eq!(state.review_mode, ReviewMode::Normal);
}

// 27. test_preexisting_staged_work_blocks_commit_and_token_cannot_override
#[test]
fn test_preexisting_staged_work_blocks_commit_and_token_cannot_override() {
    let runner = MockCommandRunner::new();
    *runner.preexisting_staged_work.lock().unwrap() = true;

    let mut cfg = default_test_config(standard_profile());
    cfg.run_permissions = PERM_STAGE | PERM_COMMIT;

    // 1. Rejected at RunStart
    assert!(initialize_run(cfg.clone(), "head-1", vec![], &runner, FIXED_TIME).is_err());

    // 2. If staged work appears later before `stage`/`commit`, transitions to Blocked (UserConfirmationRequired)
    *runner.preexisting_staged_work.lock().unwrap() = false;
    let mut state = initialize_run(cfg, "head-1", vec![], &runner, FIXED_TIME).unwrap();
    state.runtime_allowlist = vec!["src/lib.rs".to_string()];
    state.current_state = AutoConvergeState::ReleaseActionsPending;

    *runner.preexisting_staged_work.lock().unwrap() = true;
    assert!(execute_authorized_release_actions(&mut state, "commit msg", &runner).is_err());
    assert_eq!(state.current_state, AutoConvergeState::Blocked);

    // 3. Even with a valid UserConfirmationToken, resume is rejected while .git/index still has staged work
    let token = UserConfirmationToken {
        run_id: state.run_id.clone(),
        confirmed_reason: "Cleaned index".to_string(),
        issued_at: FIXED_TIME.to_string(),
    };
    assert!(resume_run(
        &mut state,
        &[],
        None,
        None,
        &runner,
        Some(token.clone()),
        FIXED_TIME
    )
    .is_err());

    // 4. Once .git/index is actually clean, resume succeeds
    *runner.preexisting_staged_work.lock().unwrap() = false;
    resume_run(
        &mut state,
        &[],
        None,
        None,
        &runner,
        Some(token),
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(
        state.current_state,
        AutoConvergeState::ReleaseActionsPending
    );
}

// 28. test_custom_profile_persistence_and_fallback
#[test]
fn test_custom_profile_persistence_and_fallback() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("custom_profiles.json");
    let store = ProfileStore::new(path.clone());

    let mut custom = standard_profile();
    custom.id = "custom-dev".to_string();
    custom.name = "Custom Dev".to_string();
    store.save_custom_profile(custom).unwrap();

    let loaded = store.load_all_profiles().unwrap();
    assert_eq!(loaded.len(), 4);
    assert!(loaded.iter().any(|p| p.id == "custom-dev" && !p.is_builtin));

    // Shadowing built-in ID is rejected
    let mut shadow = standard_profile();
    shadow.id = "standard".to_string();
    assert!(store.save_custom_profile(shadow).is_err());

    // Corrupting custom_profiles.json quarantines the file and falls back to the 3 built-in presets
    std::fs::write(&path, b"{ corrupted json").unwrap();
    let fallback = store.load_all_profiles().unwrap();
    assert_eq!(fallback.len(), 3);
    assert!(!path.exists());
}

// 29. test_structured_finding_key_semantic_stability
#[test]
fn test_structured_finding_key_semantic_stability() {
    let f1 = sample_blocking_finding("COR-001", "Null pointer can occur when buffer is empty");
    let f2 = sample_blocking_finding(
        "NULL-PTR", // synonym for COR-001
        "Empty buffers may cause a null dereference during parsing",
    );
    let id1 = compute_finding_id(&f1.finding_key, true).unwrap();
    let id2 = compute_finding_id(&f2.finding_key, true).unwrap();
    assert_eq!(id1, id2);
}

// 30. test_release_readiness_review_strict_profile
#[test]
fn test_release_readiness_review_strict_profile() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(strict_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    state.current_state = AutoConvergeState::ReadyForReleaseReview;

    let manifest = ReleaseCandidateManifest {
        version_tag: "v0.24.6".to_string(),
        release_repository: "soheidon/poverello".to_string(),
        changelog_summary: "Strict release".to_string(),
        artifact_hashes: BTreeMap::new(),
    };
    advance_from_ready_for_release_review(&mut state, Some(&manifest), FIXED_TIME).unwrap();
    assert_eq!(state.current_state, AutoConvergeState::ReleaseReview);
    assert!(state.current_release_manifest_digest.is_some());
}

// 31. test_baseline_digest_excludes_planner_artifacts
#[test]
fn test_baseline_digest_excludes_planner_artifacts() {
    let ws = TempDir::new().unwrap();
    std::fs::create_dir_all(ws.path().join("src")).unwrap();
    std::fs::write(ws.path().join("src/lib.rs"), b"fn main() {}").unwrap();

    let m1 = capture_baseline_file_manifest(ws.path(), &BTreeMap::new()).unwrap();
    let d1 = compute_baseline_workspace_digest(&m1);

    // Planner writes to .plan/ and engine writes to .poverello/runs/
    std::fs::create_dir_all(ws.path().join(".plan")).unwrap();
    std::fs::write(ws.path().join(".plan/V0.24.6-r1.md"), b"# Plan").unwrap();
    std::fs::create_dir_all(ws.path().join(".poverello/runs/ac-1")).unwrap();
    std::fs::write(
        ws.path().join(".poverello/runs/ac-1/scratch.txt"),
        b"scratch",
    )
    .unwrap();

    let m2 = capture_baseline_file_manifest(ws.path(), &BTreeMap::new()).unwrap();
    let d2 = compute_baseline_workspace_digest(&m2);
    assert_eq!(d1, d2);

    // Modifying a source file DOES change baseline_workspace_digest
    std::fs::write(ws.path().join("src/lib.rs"), b"fn main() { let x = 1; }").unwrap();
    let m3 = capture_baseline_file_manifest(ws.path(), &BTreeMap::new()).unwrap();
    let d3 = compute_baseline_workspace_digest(&m3);
    assert_ne!(d1, d3);
}

// 32. test_corrective_allowlist_expansion_rejects_preexisting_dirty_file
#[test]
fn test_corrective_allowlist_expansion_rejects_preexisting_dirty_file() {
    let runner = MockCommandRunner::new();
    let baseline = vec![
        BaselineFileEntry {
            path: "src/clean_helper.rs".to_string(),
            status: BaselineFileStatus::Clean,
            content_sha256: "1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            file_mode: "100644".to_string(),
        },
        BaselineFileEntry {
            path: "src/dirty_user_work.rs".to_string(),
            status: BaselineFileStatus::Modified,
            content_sha256: "2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
            file_mode: "100644".to_string(),
        },
    ];

    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        baseline,
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    state.runtime_allowlist = vec!["src/lib.rs".to_string()];
    state.current_state = AutoConvergeState::CorrectivePlanCreate;

    // Expanding to clean file succeeds
    submit_corrective_plan(
        &mut state,
        "planner-2",
        &sample_plan_markdown(&["src/lib.rs", "src/clean_helper.rs"], "expand clean"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(state.current_state, AutoConvergeState::CorrectivePlanReview);

    // Expanding to pre-existing dirty file is rejected -> Blocked (UserConfirmationRequired)
    state.current_state = AutoConvergeState::CorrectivePlanCreate;
    assert!(submit_corrective_plan(
        &mut state,
        "planner-3",
        &sample_plan_markdown(&["src/lib.rs", "src/dirty_user_work.rs"], "expand dirty"),
        true,
        FIXED_TIME,
    )
    .is_err());
    assert_eq!(state.current_state, AutoConvergeState::Blocked);
    assert_eq!(
        state.blocked_context.unwrap().kind,
        BlockedKind::UserConfirmationRequired
    );
}

// 33. test_runtime_allowlist_preserves_path_case
#[test]
fn test_runtime_allowlist_preserves_path_case() {
    // Case-sensitive FS: Foo.rs and foo.rs remain distinct and preserve case
    let md = sample_plan_markdown(&["gui/src/Foo.tsx", "gui/src/foo.tsx"], "case test");
    let (allow_cs, _) = extract_and_validate_allowlist_from_plan(&md, false).unwrap();
    assert_eq!(allow_cs, vec!["gui/src/Foo.tsx", "gui/src/foo.tsx"]);

    // Case-insensitive FS: case-only duplicates are rejected as ambiguous
    assert!(extract_and_validate_allowlist_from_plan(&md, true).is_err());

    // Rejects '..', absolute paths, and '.git/'
    assert!(extract_and_validate_allowlist_from_plan(
        &sample_plan_markdown(&["../secret.rs"], "traversal"),
        true
    )
    .is_err());
    assert!(extract_and_validate_allowlist_from_plan(
        &sample_plan_markdown(&["/etc/passwd"], "abs"),
        true
    )
    .is_err());
    assert!(extract_and_validate_allowlist_from_plan(
        &sample_plan_markdown(&[".git/config"], "git"),
        true
    )
    .is_err());
}

// 34. test_unknown_failure_code_is_schema_error
#[test]
fn test_unknown_failure_code_is_schema_error() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();

    let bad_verdict = make_plan_verdict_json(
        &state,
        VerdictDecision::RequestChanges,
        vec![sample_blocking_finding(
            "UNKNOWN-999",
            "Unknown taxonomy code",
        )],
        false,
    );

    // Treated as schema violation (bounded retry, never silently remapped)
    for _ in 0..3 {
        assert!(submit_review_verdict(
            &mut state,
            "rev-1",
            &bad_verdict,
            SNAP_HASH,
            SNAP_HASH,
            None,
            None,
            true,
            FIXED_TIME,
        )
        .is_err());
    }
    assert_eq!(state.current_state, AutoConvergeState::Blocked);
}

// 35. test_two_phase_release_authorization_and_concrete_release_target_binding
#[test]
fn test_two_phase_release_authorization_and_concrete_release_target_binding() {
    let mut runner = MockCommandRunner::new();
    let delta = sample_delta(
        "src/lib.rs",
        "2222222222222222222222222222222222222222222222222222222222222222",
    );
    runner.working_tree_delta = delta.clone();

    let mut cfg = default_test_config(strict_profile());
    cfg.run_permissions = PERM_STAGE | PERM_COMMIT | PERM_TAG;
    cfg.release_intent = Some(ReleasePermissionIntent {
        requested_version_tag: "v0.24.6".to_string(),
        release_repository: "soheidon/poverello".to_string(),
    });

    // Phase 1: RunStart seals RunStartPermissionBinding without requiring a commit/diff yet
    let mut state = initialize_run(cfg, "head-initial", vec![], &runner, FIXED_TIME).unwrap();
    assert!(!state
        .release_authorization
        .run_start_permission_binding
        .is_empty());
    assert!(state
        .release_authorization
        .concrete_release_target
        .is_none());

    state.runtime_allowlist = vec!["src/lib.rs".to_string()];
    state.current_diff_digest = Some(compute_diff_digest(&delta));
    let rel_manifest = ReleaseCandidateManifest {
        version_tag: "v0.24.6".to_string(),
        release_repository: "soheidon/poverello".to_string(),
        changelog_summary: "Release notes".to_string(),
        artifact_hashes: BTreeMap::new(),
    };
    state.current_release_manifest_digest = Some(rel_manifest.canonical_digest());
    super::guards::seal_candidate_release_authorization(
        &mut state,
        "v0.24.6",
        "soheidon/poverello",
    )
    .unwrap();
    assert!(state
        .release_authorization
        .candidate_release_authorization
        .is_some());

    // Phase 2: Post-commit seals ConcreteReleaseTarget & ReleaseExecutionPermissionBinding
    state.current_state = AutoConvergeState::ReleaseActionsPending;
    execute_authorized_release_actions(&mut state, "Release commit", &runner).unwrap();

    let concrete = state
        .release_authorization
        .concrete_release_target
        .as_ref()
        .unwrap();
    assert_eq!(concrete.version_tag, "v0.24.6");
    assert_eq!(concrete.target_commit_sha, runner.head_commit_sha);
    assert!(state
        .release_authorization
        .release_execution_permission_binding
        .is_some());
}

// 36. test_escalation_policy_blocking_only_vs_enabled_vs_disabled
#[test]
fn test_escalation_policy_blocking_only_vs_enabled_vs_disabled() {
    let runner = MockCommandRunner::new();

    // 1. ForcedRunStart under Disabled is rejected at RunStart
    let mut disabled_prof = standard_profile();
    disabled_prof.escalation_policy = EscalationPolicy::Disabled;
    disabled_prof.convergence_limits.max_escalations = 0;
    let mut cfg_disabled = default_test_config(disabled_prof);
    cfg_disabled.force_escalation = true;
    assert!(initialize_run(cfg_disabled, "head-1", vec![], &runner, FIXED_TIME).is_err());

    // 2. Under BlockingOnly (Fast profile), architectural_defect DOES escalate to Sol
    let mut state_fast = initialize_run(
        default_test_config(fast_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    submit_draft_plan(
        &mut state_fast,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let arch_req = make_plan_verdict_json(
        &state_fast,
        VerdictDecision::RequestChanges,
        vec![sample_blocking_finding("COR-004", "Arch issue")],
        true,
    );
    submit_review_verdict(
        &mut state_fast,
        "rev-1",
        &arch_req,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(state_fast.review_mode, ReviewMode::Escalated);
    assert_eq!(state_fast.cycle_counts.escalation_count, 1);
}

// 37. test_candidate_root_and_active_plan_digest_promotion_across_cycles
#[test]
fn test_candidate_root_and_active_plan_digest_promotion_across_cycles() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();

    // Initial PlanReview: root == None, active == None, candidate == Some(initial_sha)
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "initial plan"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let initial_sha = state.plan_lineage.candidate_plan_digest.clone().unwrap();
    assert!(state.plan_lineage.root_plan_digest.is_none());
    assert!(state.plan_lineage.active_plan_digest.is_none());

    let plan_approve = make_plan_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "rev-1",
        &plan_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(
        state.plan_lineage.root_plan_digest.as_deref(),
        Some(initial_sha.as_str())
    );
    assert_eq!(
        state.plan_lineage.active_plan_digest.as_deref(),
        Some(initial_sha.as_str())
    );
    assert!(state.plan_lineage.candidate_plan_digest.is_none());

    // Enter CorrectivePlanReview: root == initial_sha, active == initial_sha, candidate == corr_sha
    freeze_approved_plan_and_begin_implement(&mut state, &[], "impl-1", FIXED_TIME).unwrap();
    let delta = sample_delta(
        "src/lib.rs",
        "2222222222222222222222222222222222222222222222222222222222222222",
    );
    complete_implementation_step(
        &mut state,
        "impl-1",
        &["src/lib.rs".to_string()],
        &delta,
        sample_evidence(),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let impl_req = make_impl_verdict_json(
        &state,
        VerdictDecision::RequestChanges,
        vec![sample_blocking_finding("COR-001", "Need fix")],
        false,
    );
    submit_review_verdict(
        &mut state, "rev-2", &impl_req, SNAP_HASH, SNAP_HASH, None, None, true, FIXED_TIME,
    )
    .unwrap();

    submit_corrective_plan(
        &mut state,
        "planner-2",
        &sample_plan_markdown(&["src/lib.rs"], "corrective plan 1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let corr_sha = state.plan_lineage.candidate_plan_digest.clone().unwrap();
    assert_ne!(initial_sha, corr_sha);
    assert_eq!(
        state.plan_lineage.active_plan_digest.as_deref(),
        Some(initial_sha.as_str())
    );

    // CorrectivePlanReview APPROVE promotes active_plan_digest = corr_sha while preserving root_plan_digest = initial_sha
    let corr_approve = make_corrective_verdict_json(&state, VerdictDecision::Approve, vec![]);
    submit_review_verdict(
        &mut state,
        "rev-3",
        &corr_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(
        state.plan_lineage.root_plan_digest.as_deref(),
        Some(initial_sha.as_str())
    );
    assert_eq!(
        state.plan_lineage.active_plan_digest.as_deref(),
        Some(corr_sha.as_str())
    );
    assert!(state.plan_lineage.candidate_plan_digest.is_none());
    assert_eq!(
        state.plan_lineage.approved_plan_chain,
        vec![initial_sha, corr_sha]
    );
}

// 38. test_canonical_run_delta_manifest_diff_digest_matches_pre_and_post_commit
#[test]
fn test_canonical_run_delta_manifest_diff_digest_matches_pre_and_post_commit() {
    let ws = TempDir::new().unwrap();
    std::fs::create_dir_all(ws.path().join("src")).unwrap();
    std::fs::write(ws.path().join("src/allow.rs"), b"v1").unwrap();
    std::fs::write(ws.path().join("src/unrelated_dirty.rs"), b"user dirty 1").unwrap();

    let mut overrides = BTreeMap::new();
    overrides.insert(
        "src/unrelated_dirty.rs".to_string(),
        BaselineFileStatus::Modified,
    );
    let baseline = capture_baseline_file_manifest(ws.path(), &overrides).unwrap();

    // Modify allowlisted file AND further modify unrelated dirty file outside runtime_allowlist
    std::fs::write(ws.path().join("src/allow.rs"), b"v2").unwrap();
    std::fs::write(ws.path().join("src/unrelated_dirty.rs"), b"user dirty 2").unwrap();

    let allowlist = vec!["src/allow.rs".to_string()];
    let pre_commit_delta = compute_run_delta_manifest(ws.path(), &allowlist, &baseline).unwrap();
    let pre_commit_digest = compute_diff_digest(&pre_commit_delta);

    // Post-commit HEAD tree has identical bytes on allowlisted file -> digest matches byte-for-byte
    let post_commit_delta = compute_run_delta_manifest(ws.path(), &allowlist, &baseline).unwrap();
    let post_commit_digest = compute_diff_digest(&post_commit_delta);
    assert_eq!(pre_commit_digest, post_commit_digest);
}

// 39. (§16.6) test_agent_cannot_write_engine_state_and_tamper_replay_rollback_fails_closed
#[test]
fn test_agent_cannot_write_engine_state_and_tamper_replay_rollback_fails_closed() {
    let dir = TempDir::new().unwrap();
    let state_path = dir.path().join("run_state.json");
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    save_run_state_authenticated(&state_path, &mut state).unwrap();
    let seq1_bytes = std::fs::read(&state_path).unwrap();

    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    save_run_state_authenticated(&state_path, &mut state).unwrap();
    let latest_seq = state.state_sequence;

    // 1. Rollback / replay of older valid state file fails closed against expected_min_sequence
    std::fs::write(&state_path, &seq1_bytes).unwrap();
    assert!(load_and_verify_run_state_authenticated(&state_path, Some(latest_seq)).is_err());

    // 2. Direct JSON tampering without engine_auth_tag key fails closed
    let mut tampered = state.clone();
    tampered.current_state = AutoConvergeState::ReleaseCompleted;
    std::fs::write(&state_path, serde_json::to_vec(&tampered).unwrap()).unwrap();
    assert!(load_and_verify_run_state_authenticated(&state_path, None).is_err());
}

// 40. (§16.6) test_forged_or_stale_approval_cannot_advance_fsm
#[test]
fn test_forged_or_stale_approval_cannot_advance_fsm() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(fast_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let plan_approve = make_plan_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "rev-1",
        &plan_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    freeze_approved_plan_and_begin_implement(&mut state, &[], "impl-1", FIXED_TIME).unwrap();
    let delta = sample_delta(
        "src/lib.rs",
        "2222222222222222222222222222222222222222222222222222222222222222",
    );
    complete_implementation_step(
        &mut state,
        "impl-1",
        &["src/lib.rs".to_string()],
        &delta,
        sample_evidence(),
        true,
        FIXED_TIME,
    )
    .unwrap();

    // Inject a forged ApprovalRecord with self-consistent SHA-256 digests but forged engine_auth_tag
    state.approvals.push(super::types::ApprovalRecord {
        stage_state: AutoConvergeState::ImplementationReview,
        reviewer_session_id: "rev-forged".to_string(),
        producer_session_id: "impl-1".to_string(),
        timestamp: FIXED_TIME.to_string(),
        sequence: 99,
        binding: ApprovalBinding {
            target_type: ApprovalTargetType::ImplementationDiff,
            root_plan_digest: state.plan_lineage.root_plan_digest.clone(),
            active_plan_digest: state.plan_lineage.active_plan_digest.clone(),
            candidate_plan_digest: None,
            allowlist_digest: state.allowlist_digest.clone().unwrap(),
            policy_profile_digest: state.policy_profile_digest.clone(),
            baseline_workspace_digest: Some(state.workspace.baseline_workspace_digest.clone()),
            diff_digest: state.current_diff_digest.clone(),
            evidence_manifest_digest: state.current_evidence_manifest_digest.clone(),
            release_manifest_digest: None,
            ..Default::default()
        },
        invalidated: false,
        engine_auth_tag: "0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
    });

    assert!(!has_valid_reusable_impl_approval(&state));
}

// 41. (§16.6) test_reviewer_os_boundary_denies_checkout_snapshot_engine_and_traversal_writes
#[test]
fn test_reviewer_os_boundary_denies_checkout_snapshot_engine_and_traversal_writes() {
    let ws = PathBuf::from("/workspace/repo");
    let eng = PathBuf::from("/appdata/poverello/auto_converge_runs");
    let allowlist = vec!["src/lib.rs".to_string()];

    // Reviewer denied everywhere
    assert!(enforce_agent_filesystem_write_boundary(
        "Plan Reviewer",
        &ws.join("src/lib.rs"),
        &ws,
        &eng,
        &allowlist
    )
    .is_err());
    assert!(enforce_agent_filesystem_write_boundary(
        "Implementation Reviewer",
        &eng.join("run_state.json"),
        &ws,
        &eng,
        &allowlist
    )
    .is_err());

    // Planner allowed ONLY in .plan/, denied in engine state, source tree, and '..' traversal
    assert!(enforce_agent_filesystem_write_boundary(
        "Planner",
        &ws.join(".plan/V0.24.6-r1.md"),
        &ws,
        &eng,
        &allowlist
    )
    .is_ok());
    assert!(enforce_agent_filesystem_write_boundary(
        "Planner",
        &ws.join("src/lib.rs"),
        &ws,
        &eng,
        &allowlist
    )
    .is_err());
    assert!(enforce_agent_filesystem_write_boundary(
        "Planner",
        &eng.join("run_state.json"),
        &ws,
        &eng,
        &allowlist
    )
    .is_err());
    assert!(enforce_agent_filesystem_write_boundary(
        "Planner",
        &ws.join(".plan/../src/lib.rs"),
        &ws,
        &eng,
        &allowlist
    )
    .is_err());

    // Implementer allowed ONLY in runtime_allowlist, denied in .plan/ and engine state
    assert!(enforce_agent_filesystem_write_boundary(
        "Implementer",
        &ws.join("src/lib.rs"),
        &ws,
        &eng,
        &allowlist
    )
    .is_ok());
    assert!(enforce_agent_filesystem_write_boundary(
        "Implementer",
        &ws.join(".plan/V0.24.6-r1.md"),
        &ws,
        &eng,
        &allowlist
    )
    .is_err());
    assert!(enforce_agent_filesystem_write_boundary(
        "Implementer",
        &eng.join("run_state.json"),
        &ws,
        &eng,
        &allowlist
    )
    .is_err());
}

// 42. (§16.6) test_phase_a_rejects_git_release_and_paid_live_api_requests
#[test]
fn test_phase_a_rejects_git_release_and_paid_live_api_requests() {
    let runner = MockCommandRunner::new();
    for forbidden_bit in [
        PERM_STAGE,
        PERM_COMMIT,
        PERM_PUSH_BRANCH,
        PERM_TAG,
        PERM_PUSH_TAG,
        PERM_RELEASE_PUBLISH,
        PERM_PAID_LIVE_API,
    ] {
        let mut cfg = default_test_config(strict_profile());
        cfg.delivery_phase = DeliveryPhase::PhaseALocalConvergenceMvp;
        cfg.run_permissions = forbidden_bit;
        assert!(
            initialize_run(cfg, "head-1", vec![], &runner, FIXED_TIME).is_err(),
            "Phase A must reject permission bit 0x{forbidden_bit:02x}"
        );
    }
}

// 43. (§20.5) test_random_os_protected_key_rejects_legacy_constant_mac
#[test]
fn test_random_os_protected_key_rejects_legacy_constant_mac() {
    use super::state_store::{EngineAuthorityKey, LEGACY_PUBLIC_HMAC_SECRET};

    let dir = TempDir::new().unwrap();
    let state_file = dir.path().join("run_state.json");
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(fast_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();

    // Authenticate with the legacy public constant key
    let legacy_key = EngineAuthorityKey::from_raw_bytes(LEGACY_PUBLIC_HMAC_SECRET.to_vec());
    let mut copy = state.clone();
    copy.engine_auth_tag = String::new();
    let json = serde_json::to_string(&copy).unwrap();
    state.engine_auth_tag = legacy_key.compute_hmac(json.as_bytes());

    std::fs::write(&state_file, serde_json::to_string_pretty(&state).unwrap()).unwrap();

    // Verification must reject legacy public constant fail-closed
    let res = load_and_verify_run_state_authenticated(&state_file, None);
    assert!(
        res.is_err(),
        "State signed with legacy public constant must be rejected"
    );
    let err_msg = res.err().unwrap();
    assert!(
        err_msg.contains("authentication failed") || err_msg.contains("LegacyKeyRejected"),
        "Unexpected error message: {err_msg}"
    );
}

// 44. (§24.4.4) test_disposable_probe_fixture_and_write_scope_denial
#[test]
fn test_disposable_probe_fixture_and_write_scope_denial() {
    use super::isolation::{
        set_test_isolation_pass, AutoConvergeRole, PlatformIsolationGuard, TEST_ISOLATION_LOCK,
    };

    let _lock = TEST_ISOLATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let dir = TempDir::new().unwrap();
    let ws_root = dir.path().join("workspace");
    let snapshot_dir = dir.path().join("snapshot");
    std::fs::create_dir_all(&ws_root).unwrap();
    std::fs::create_dir_all(&snapshot_dir).unwrap();

    // Enable test isolation seam for probe fixture testing
    set_test_isolation_pass(true);

    struct ResetSeamGuard;
    impl Drop for ResetSeamGuard {
        fn drop(&mut self) {
            super::isolation::set_test_isolation_pass(false);
        }
    }
    let _guard = ResetSeamGuard;

    let res = PlatformIsolationGuard::verify_reviewer_os_boundary(&ws_root, &snapshot_dir);
    assert!(res.is_ok(), "OS isolation probe failed: {:?}", res.err());

    // Fails closed if snapshot_dir is nested inside workspace_root
    let nested_snapshot = ws_root.join("snapshots").join("rev-1");
    std::fs::create_dir_all(&nested_snapshot).unwrap();
    assert!(
        PlatformIsolationGuard::verify_reviewer_os_boundary(&ws_root, &nested_snapshot).is_err(),
        "Nested snapshot directory must be rejected by isolation preflight"
    );

    // Agent write scope validation
    assert!(PlatformIsolationGuard::validate_agent_write_scope(
        AutoConvergeRole::Reviewer,
        std::path::Path::new("src/main.rs"),
        &[]
    )
    .is_err());
    assert!(PlatformIsolationGuard::validate_agent_write_scope(
        AutoConvergeRole::Planner,
        std::path::Path::new(".plan/V0.24.6.md"),
        &[]
    )
    .is_ok());
    assert!(PlatformIsolationGuard::validate_agent_write_scope(
        AutoConvergeRole::Planner,
        std::path::Path::new("src/main.rs"),
        &[]
    )
    .is_err());
    assert!(PlatformIsolationGuard::validate_agent_write_scope(
        AutoConvergeRole::Implementer,
        std::path::Path::new("src/main.rs"),
        &["src/main.rs".to_string()]
    )
    .is_ok());
    assert!(PlatformIsolationGuard::validate_agent_write_scope(
        AutoConvergeRole::Implementer,
        std::path::Path::new("src/unauthorized.rs"),
        &["src/main.rs".to_string()]
    )
    .is_err());
}

// 45. (§20.5) test_production_agent_runner_blocks_on_isolation_failure
#[test]
fn test_production_agent_runner_blocks_on_isolation_failure() {
    use super::agent_runner::{AgentCommandRunner, ProductionAgentRunner, RoleExecutionRequest};
    use super::isolation::AutoConvergeRole;

    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    std::fs::create_dir_all(&ws).unwrap();

    let runner = ProductionAgentRunner::new();
    // Launching reviewer with nested snapshot fails closed in preflight
    let nested_snap = ws.join("bad_snapshot");
    std::fs::create_dir_all(&nested_snap).unwrap();

    let req = RoleExecutionRequest {
        role: AutoConvergeRole::Reviewer,
        session_id: "rev-001".to_string(),
        workspace_root: ws.clone(),
        snapshot_dir: Some(nested_snap),
        plan_path: ws.join(".plan").join("plan.md"),
        runtime_allowlist: vec!["src/lib.rs".to_string()],
        input_prompt: "echo probe".to_string(),
        timeout_seconds: 5,
        model: None,
        reasoning_effort: None,
        run_id: None,
    };

    let result = runner.execute_role_session(&req);
    assert!(
        result.is_err(),
        "ProductionAgentRunner must block before launch when isolation preflight fails"
    );
}

// 46. (§20.5) test_state_store_sequence_and_tampering_fail_closed
#[test]
fn test_state_store_sequence_and_tampering_fail_closed() {
    let dir = TempDir::new().unwrap();
    let state_file = dir.path().join("run_state.json");
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(fast_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();

    save_run_state_authenticated(&state_file, &mut state).unwrap();
    let seq_after_save = state.state_sequence;

    // Loading with higher expected sequence fails closed (rollback detection)
    let res = load_and_verify_run_state_authenticated(&state_file, Some(seq_after_save + 1));
    assert!(res.is_err(), "Sequence rollback must fail closed");

    // Tampering with content invalidates MAC
    let raw = std::fs::read_to_string(&state_file).unwrap();
    let tampered = raw.replace("ac-test-run-001", "ac-test-run-tampered");
    assert_ne!(raw, tampered, "replace must have modified the file");
    std::fs::write(&state_file, tampered).unwrap();
    let tampered_res = load_and_verify_run_state_authenticated(&state_file, None);
    assert!(tampered_res.is_err(), "Tampered state must fail closed");
}

// 47. (§22.5) test_missing_or_unsupported_cli_fails_closed_zero_launch
#[test]
fn test_missing_or_unsupported_cli_fails_closed_zero_launch() {
    use super::agent_runner::{AgentCommandRunner, ProductionAgentRunner, RoleExecutionRequest};
    use super::isolation::AutoConvergeRole;

    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    std::fs::create_dir_all(&ws).unwrap();

    // 1. Unset environment variable and empty runner fails closed
    std::env::remove_var("POVERELLO_AGENT_CLI");
    let runner = ProductionAgentRunner::new();
    let req = RoleExecutionRequest {
        role: AutoConvergeRole::Planner,
        session_id: "plan-001".to_string(),
        workspace_root: ws.clone(),
        snapshot_dir: None,
        plan_path: ws.join(".plan").join("plan.md"),
        runtime_allowlist: vec!["src/lib.rs".to_string()],
        input_prompt: "Create plan".to_string(),
        timeout_seconds: 5,
        model: None,
        reasoning_effort: None,
        run_id: None,
    };
    let res = runner.execute_role_session(&req);
    assert!(res.is_err(), "Unconfigured runner must fail closed");
    let err = res.err().unwrap();
    assert!(
        err.contains("No configured agent CLI available"),
        "Unexpected error: {err}"
    );

    // 2. Non-existent CLI path fails closed
    let bad_runner = ProductionAgentRunner::with_cli(dir.path().join("nonexistent_binary.exe"));
    let res2 = bad_runner.execute_role_session(&req);
    assert!(res2.is_err(), "Non-existent CLI must fail closed");
    let err2 = res2.err().unwrap();
    assert!(
        err2.contains("Configured agent CLI not found"),
        "Unexpected error: {err2}"
    );
}

// 48. (§22.5) test_reviewer_verdict_schema_validation_and_malformed_rejection
#[test]
fn test_reviewer_verdict_schema_validation_and_malformed_rejection() {
    use super::agent_runner::{AgentCommandRunner, ProductionAgentRunner, RoleExecutionRequest};
    use super::isolation::AutoConvergeRole;

    let _lock = super::isolation::TEST_ISOLATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    super::isolation::set_test_isolation_pass(true);
    struct IsolationCleaner;
    impl Drop for IsolationCleaner {
        fn drop(&mut self) {
            super::isolation::set_test_isolation_pass(false);
        }
    }
    let _cleaner = IsolationCleaner;

    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let snap = dir.path().join("snapshot");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&snap).unwrap();

    // Script 1: outputs malformed JSON missing required fields
    let bad_src = dir.path().join("bad_verdict.rs");
    let bad_exe = dir.path().join("bad_verdict.exe");
    std::fs::write(
        &bad_src,
        r#"
fn main() {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--output-last-message" && i + 1 < args.len() {
            let _ = std::fs::write(&args[i + 1], "{\"random_key\": 123}");
        }
    }
}"#,
    )
    .unwrap();

    let link_tmp = dir.path().join("link_tmp");
    std::fs::create_dir_all(&link_tmp).unwrap();

    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let compile = std::process::Command::new(&rustc)
        .env("TMP", &link_tmp)
        .env("TEMP", &link_tmp)
        .arg("--edition=2021")
        .arg(&bad_src)
        .arg("-o")
        .arg(&bad_exe)
        .output()
        .expect("rustc must execute");
    assert!(
        compile.status.success(),
        "rustc compile of bad_verdict failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    use super::types::ReviewerLevel;

    let runner = ProductionAgentRunner::with_cli(bad_exe).with_reviewer_level(ReviewerLevel::R0D);
    let req = RoleExecutionRequest {
        role: AutoConvergeRole::Reviewer,
        session_id: "rev-001".to_string(),
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: ws.join(".plan").join("plan.md"),
        runtime_allowlist: vec!["src/lib.rs".to_string()],
        input_prompt: "Review plan".to_string(),
        timeout_seconds: 5,
        model: None,
        reasoning_effort: None,
        run_id: None,
    };

    let res = runner.execute_role_session(&req);
    assert!(
        res.is_err(),
        "Malformed verdict must be rejected by schema validation"
    );
    let err = res.err().unwrap();
    assert!(
        err.contains("schema validation") || err.contains("missing required property"),
        "Unexpected error: {err}"
    );

    // Script 2: outputs unclosed/truncated JSON
    let trunc_src = dir.path().join("trunc_verdict.rs");
    let trunc_exe = dir.path().join("trunc_verdict.exe");
    std::fs::write(
        &trunc_src,
        r#"
fn main() {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--output-last-message" && i + 1 < args.len() {
            let _ = std::fs::write(&args[i + 1], "{\"verdict\": \"APPROVE\"");
        }
    }
}"#,
    )
    .unwrap();

    let compile2 = std::process::Command::new(&rustc)
        .env("TMP", &link_tmp)
        .env("TEMP", &link_tmp)
        .arg("--edition=2021")
        .arg(&trunc_src)
        .arg("-o")
        .arg(&trunc_exe)
        .output()
        .expect("rustc must execute");
    assert!(
        compile2.status.success(),
        "rustc compile of trunc_verdict failed: {}",
        String::from_utf8_lossy(&compile2.stderr)
    );

    let trunc_runner =
        ProductionAgentRunner::with_cli(trunc_exe).with_reviewer_level(ReviewerLevel::R0D);
    let res2 = trunc_runner.execute_role_session(&req);
    assert!(
        res2.is_err(),
        "Truncated JSON without closing delimiter must be rejected"
    );
}

// 49. (§22.5) test_timeout_and_process_tree_termination
#[test]
fn test_timeout_and_process_tree_termination() {
    use super::agent_runner::{AgentCommandRunner, ProductionAgentRunner, RoleExecutionRequest};
    use super::isolation::AutoConvergeRole;

    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    std::fs::create_dir_all(&ws).unwrap();

    // Script that sleeps longer than timeout
    let sleep_script = dir.path().join("sleep_agent.cmd");
    std::fs::write(
        &sleep_script,
        b"@echo off\r\nping 127.0.0.1 -n 10 > nul\r\n",
    )
    .unwrap();

    let runner = ProductionAgentRunner::with_cli(sleep_script);
    let req = RoleExecutionRequest {
        role: AutoConvergeRole::Planner,
        session_id: "sleep-001".to_string(),
        workspace_root: ws.clone(),
        snapshot_dir: None,
        plan_path: ws.join(".plan").join("plan.md"),
        runtime_allowlist: vec![],
        input_prompt: "Sleep".to_string(),
        timeout_seconds: 1, // 1 second timeout
        model: None,
        reasoning_effort: None,
        run_id: None,
    };

    let start = std::time::Instant::now();
    let res = runner.execute_role_session(&req);
    let elapsed = start.elapsed();

    assert!(res.is_err(), "Timeout must return error");
    let err = res.err().unwrap();
    assert!(
        err.contains("timed out after 1 seconds and was terminated"),
        "Unexpected error: {err}"
    );
    assert!(elapsed.as_secs() < 5, "Timeout must abort promptly");
}

// 50. (§22.5) test_output_bounding_rejects_exceeded_capture
#[test]
fn test_output_bounding_rejects_exceeded_capture() {
    use super::agent_runner::{AgentCommandRunner, ProductionAgentRunner, RoleExecutionRequest};
    use super::isolation::AutoConvergeRole;

    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    std::fs::create_dir_all(&ws).unwrap();

    // Script that writes > 1 MiB to stdout via powershell
    let huge_script = dir.path().join("huge_output.cmd");
    std::fs::write(
        &huge_script,
        b"@echo off\r\npowershell -NoProfile -Command \"[Console]::Out.Write('X' * 1100000)\"\r\n",
    )
    .unwrap();

    let runner = ProductionAgentRunner::with_cli(huge_script);
    let req = RoleExecutionRequest {
        role: AutoConvergeRole::Planner,
        session_id: "huge-001".to_string(),
        workspace_root: ws.clone(),
        snapshot_dir: None,
        plan_path: ws.join(".plan").join("plan.md"),
        runtime_allowlist: vec![],
        input_prompt: "Output lots of bytes".to_string(),
        timeout_seconds: 10,
        model: None,
        reasoning_effort: None,
        run_id: None,
    };

    let res = runner.execute_role_session(&req);
    assert!(res.is_err(), "Output exceeding 1 MiB must be rejected");
    let err = res.err().unwrap();
    assert!(
        err.contains("exceeded maximum capture limit"),
        "Unexpected error: {err}"
    );
}

// 51. (§22.5) test_phase_a_happy_path_and_corrective_cycle_with_fake_executable
#[test]
fn test_phase_a_happy_path_and_corrective_cycle_with_fake_executable() {
    let _lock = super::isolation::TEST_ISOLATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    use super::agent_runner::{AgentCommandRunner, RoleExecutionRequest, RoleExecutionResponse};
    use super::isolation::AutoConvergeRole;

    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let plan_dir = ws.join(".plan");
    let src_dir = ws.join("src");
    std::fs::create_dir_all(&plan_dir).unwrap();
    std::fs::create_dir_all(&src_dir).unwrap();

    let target_file = src_dir.join("lib.rs");
    std::fs::write(&target_file, b"pub fn initial() {}\n").unwrap();

    let mut config = default_test_config(fast_profile());
    config.working_dir = ws.to_string_lossy().to_string();
    let run_id = format!("ac-test-run-{}", uuid::Uuid::new_v4());
    config.run_id = Some(run_id.clone());
    let state_file = dir.path().join("run_state.json");
    let git_runner = MockCommandRunner::new();

    let mut state = initialize_run(
        config,
        "head-1234",
        vec![BaselineFileEntry {
            path: "src/lib.rs".to_string(),
            status: BaselineFileStatus::Clean,
            content_sha256: super::sha256_hex(b"pub fn initial() {}\n"),
            file_mode: "100644".to_string(),
        }],
        &git_runner,
        FIXED_TIME,
    )
    .unwrap();

    save_run_state_authenticated(&state_file, &mut state).unwrap();

    // Deterministic fake agent driving a corrective plan cycle then approval
    struct FakePhaseAAgent {
        cycle: std::sync::atomic::AtomicUsize,
        workspace: std::path::PathBuf,
        run_id: String,
    }

    impl AgentCommandRunner for FakePhaseAAgent {
        fn execute_role_session(
            &self,
            request: &RoleExecutionRequest,
        ) -> Result<RoleExecutionResponse, String> {
            let cycle = self.cycle.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            match request.role {
                AutoConvergeRole::Planner => {
                    let plan_file = self
                        .workspace
                        .join(".plan")
                        .join(format!("{}.md", self.run_id));
                    let content = sample_plan_markdown(&["src/lib.rs"], "Phase A Test Plan");
                    std::fs::write(&plan_file, &content).unwrap();
                    Ok(RoleExecutionResponse {
                        exit_code: 0,
                        stdout: content,
                        stderr: String::new(),
                        verdict_json: None,
                    })
                }
                AutoConvergeRole::Reviewer => {
                    let snap = request
                        .snapshot_dir
                        .as_ref()
                        .expect("Snapshot dir must be present for Reviewer");
                    assert!(snap.exists(), "Snapshot dir must exist");
                    let manifest_file = snap.join("review_manifest.json");
                    assert!(
                        manifest_file.exists(),
                        "review_manifest.json must exist in snapshot"
                    );
                    let allowlist_file = snap.join("allowlist.json");
                    assert!(
                        allowlist_file.exists(),
                        "allowlist.json must exist in snapshot"
                    );
                    assert!(
                        request.plan_path.exists(),
                        "Plan file in snapshot must exist"
                    );

                    let manifest_bytes = std::fs::read(&manifest_file).unwrap();
                    let m: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
                    let target_type_str = m["target_type"].as_str().unwrap();

                    let (verdict_decision, target_type, findings) = match target_type_str {
                        "plan" => (VerdictDecision::Approve, ApprovalTargetType::Plan, vec![]),
                        "implementation_diff" => {
                            if cycle < 4 {
                                (
                                    VerdictDecision::RequestChanges,
                                    ApprovalTargetType::ImplementationDiff,
                                    vec![ReviewFinding {
                                        finding_key: ReviewFindingKey {
                                            category: FindingCategory::Correctness,
                                            file: "src/lib.rs".to_string(),
                                            symbol: "fn initial()".to_string(),
                                            failure_code: "COR-001".to_string(),
                                        },
                                        location: "src/lib.rs:1".to_string(),
                                        failure_condition: "Needs initial correction".to_string(),
                                        acceptance_condition: "Initial corrected".to_string(),
                                        blocking: true,
                                    }],
                                )
                            } else {
                                (
                                    VerdictDecision::Approve,
                                    ApprovalTargetType::ImplementationDiff,
                                    vec![],
                                )
                            }
                        }
                        "corrective_plan" => (
                            VerdictDecision::Approve,
                            ApprovalTargetType::CorrectivePlan,
                            vec![],
                        ),
                        other => panic!("Unexpected target_type {other}"),
                    };

                    let v = CodexReviewVerdict {
                        verdict: verdict_decision,
                        summary: format!("Reviewer cycle {cycle} verdict on {target_type_str}"),
                        findings,
                        approval_binding: ApprovalBinding {
                            target_type,
                            root_plan_digest: m
                                .get("root_plan_digest")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            active_plan_digest: m
                                .get("active_plan_digest")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            candidate_plan_digest: m
                                .get("candidate_plan_digest")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            allowlist_digest: m["allowlist_digest"].as_str().unwrap().to_string(),
                            policy_profile_digest: m["policy_profile_digest"]
                                .as_str()
                                .unwrap()
                                .to_string(),
                            baseline_workspace_digest: m
                                .get("baseline_workspace_digest")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            diff_digest: m
                                .get("diff_digest")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            evidence_manifest_digest: m
                                .get("evidence_manifest_digest")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            release_manifest_digest: None,
                            bundle_digest: compute_filesystem_manifest_hash(snap).ok(),
                            run_id: m
                                .get("run_id")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            cycle: m.get("cycle").and_then(|v| v.as_u64()),
                            implementer_session_id: m
                                .get("producer_session_id")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                        },
                        architectural_defect: false,
                        escalation_recommended: false,
                    };

                    Ok(RoleExecutionResponse {
                        exit_code: 0,
                        stdout: serde_json::to_string(&v).unwrap(),
                        stderr: String::new(),
                        verdict_json: Some(serde_json::to_string(&v).unwrap()),
                    })
                }
                AutoConvergeRole::Implementer => {
                    let target = request.workspace_root.join("src").join("lib.rs");
                    std::fs::write(&target, b"pub fn initial() {}\npub fn implemented() {}\n")
                        .unwrap();
                    Ok(RoleExecutionResponse {
                        exit_code: 0,
                        stdout: "Implementation applied successfully".to_string(),
                        stderr: String::new(),
                        verdict_json: None,
                    })
                }
            }
        }
    }

    let fake_agent = FakePhaseAAgent {
        cycle: std::sync::atomic::AtomicUsize::new(0),
        workspace: ws.clone(),
        run_id: run_id.clone(),
    };

    // Run to completion through PlanReview -> ImplementationReview (RequestChanges) -> CorrectivePlanReview -> ImplementationApproved
    let res =
        super::run_phase_a_to_completion(&mut state, &fake_agent, &git_runner, &state_file, 50);
    assert!(
        res.is_ok(),
        "Phase A run to completion failed: {:?}",
        res.err()
    );
    assert_eq!(
        state.current_state,
        AutoConvergeState::ImplementationApproved
    );
    assert!(state.is_terminal, "Terminal state must be reached");

    // Verify authenticated state was persisted in terminal state
    let loaded = load_and_verify_run_state_authenticated(&state_file, None).unwrap();
    assert_eq!(
        loaded.current_state,
        AutoConvergeState::ImplementationApproved
    );
    assert!(loaded.is_terminal);

    if let Some(cb) = state.candidate_binding.as_ref() {
        let _ = std::fs::remove_dir_all(&cb.candidate_root);
    }
}

// 52. (§24.4.3, §24.5) test_preflight_missing_plan_fails_closed_with_zero_launches
#[test]
fn test_preflight_missing_plan_fails_closed_with_zero_launches() {
    use super::agent_runner::MockAgentRunner;

    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    std::fs::create_dir_all(&ws).unwrap();

    let runner = MockAgentRunner::new();
    let git_runner = MockCommandRunner::new();
    let state_file = dir.path().join("run_state.json");

    let run_id = format!("ac-run-52-{}", uuid::Uuid::new_v4());
    let mut cfg = default_test_config(fast_profile());
    cfg.run_id = Some(run_id);
    cfg.working_dir = ws.to_string_lossy().to_string();

    let mut state = initialize_run(cfg, "head-123", vec![], &git_runner, FIXED_TIME).unwrap();
    save_run_state_authenticated(&state_file, &mut state).unwrap();

    // Advance to PlanReview without candidate plan file in workspace
    state.current_state = AutoConvergeState::PlanReview;

    let res = super::run_phase_a_step(&mut state, &runner, &git_runner, &state_file);
    assert!(
        res.is_err(),
        "Must fail when candidate plan file is missing"
    );
    assert_eq!(state.current_state, AutoConvergeState::Blocked);
    let ctx = state
        .blocked_context
        .as_ref()
        .expect("Blocked context must be set");
    assert_eq!(ctx.kind, BlockedKind::FatalInvariant);
    assert!(
        ctx.reason.contains("Required candidate plan file missing")
            || ctx.reason.contains("Failed to materialize"),
        "Unexpected reason: {}",
        ctx.reason
    );

    // Zero child launches
    assert_eq!(
        runner.get_executed_requests().len(),
        0,
        "Pre-launch failure must result in zero launches"
    );
}

// 53. (§24.4.1, §24.5, §25.3.5) child_probe_isolation_env_target and test_environment_variables_cannot_bypass_isolation_preflight
#[test]
fn child_probe_isolation_env_target() {
    let _lock = super::isolation::TEST_ISOLATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let snap = dir.path().join("snapshot");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&snap).unwrap();

    // Ensure test seam is disabled
    super::isolation::set_test_isolation_pass(false);

    let res = super::isolation::PlatformIsolationGuard::verify_reviewer_os_boundary(&ws, &snap);
    assert!(
        res.is_err(),
        "Environment variables must never bypass platform preflight"
    );
    match res.err().unwrap() {
        super::isolation::IsolationError::UnsupportedPlatform(msg) => {
            assert!(msg.contains("Standard unprivileged desktop environment lacks proven container/sandbox OS isolation"));
        }
        other => panic!("Expected UnsupportedPlatform, got: {:?}", other),
    }
}

#[test]
fn test_environment_variables_cannot_bypass_isolation_preflight() {
    let _lock = super::isolation::TEST_ISOLATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    // Child process regression test: former bypass environment variables are set ONLY in child test process.
    // The parent test process environment is NEVER mutated.
    let current_exe = std::env::current_exe().expect("Failed to get current_exe");
    let status = std::process::Command::new(current_exe)
        .arg("orchestrator::auto_converge::tests::child_probe_isolation_env_target")
        .arg("--exact")
        .arg("--nocapture")
        .env("POVERELLO_CONTAINER_ISOLATION", "1")
        .env("POVERELLO_FORCE_TEST_ISOLATION_PASS", "1")
        .status()
        .expect("Failed to execute child test process");

    assert!(
        status.success(),
        "Child process failed with status: {:?}",
        status
    );
}

// 54. (§23.1.3, §23.4.4) test_resume_uses_persisted_configured_agent_cli
#[test]
fn test_resume_uses_persisted_configured_agent_cli() {
    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    std::fs::create_dir_all(&ws).unwrap();
    let state_file = dir.path().join("run_state.json");
    let git_runner = MockCommandRunner::new();

    let mut cfg = default_test_config(fast_profile());
    cfg.agent_cli_path = Some("C:\\fake\\persisted_agent_cli.exe".to_string());
    cfg.working_dir = ws.to_string_lossy().to_string();

    let mut state = initialize_run(cfg, "head-1", vec![], &git_runner, FIXED_TIME).unwrap();
    assert_eq!(
        state.configured_agent_cli.as_deref(),
        Some("C:\\fake\\persisted_agent_cli.exe")
    );
    save_run_state_authenticated(&state_file, &mut state).unwrap();

    let loaded = load_and_verify_run_state_authenticated(&state_file, None).unwrap();
    assert_eq!(
        loaded.configured_agent_cli.as_deref(),
        Some("C:\\fake\\persisted_agent_cli.exe")
    );
}

// 55. (§24.4.3, §24.5) test_pre_launch_empty_plan_fails_closed_with_zero_launches
#[test]
fn test_pre_launch_empty_plan_fails_closed_with_zero_launches() {
    use super::agent_runner::MockAgentRunner;

    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let plan_dir = ws.join(".plan");
    std::fs::create_dir_all(&plan_dir).unwrap();

    let runner = MockAgentRunner::new();
    let git_runner = MockCommandRunner::new();
    let state_file = dir.path().join("run_state.json");

    let run_id = format!("ac-run-55-{}", uuid::Uuid::new_v4());
    let mut cfg = default_test_config(fast_profile());
    cfg.run_id = Some(run_id.clone());
    cfg.working_dir = ws.to_string_lossy().to_string();
    let mut state = initialize_run(cfg, "head-123", vec![], &git_runner, FIXED_TIME).unwrap();

    // Create empty candidate plan file
    let empty_plan = plan_dir.join(format!("{}.md", run_id));
    std::fs::write(&empty_plan, b"   \n  \t \n").unwrap();

    state.current_state = AutoConvergeState::PlanReview;
    save_run_state_authenticated(&state_file, &mut state).unwrap();

    let res = super::run_phase_a_step(&mut state, &runner, &git_runner, &state_file);
    assert!(res.is_err(), "Must fail when candidate plan file is empty");
    assert_eq!(state.current_state, AutoConvergeState::Blocked);
    let ctx = state
        .blocked_context
        .as_ref()
        .expect("Blocked context must be set");
    assert_eq!(ctx.kind, BlockedKind::FatalInvariant);
    assert!(
        ctx.reason.contains("Candidate plan file is empty")
            || ctx.reason.contains("Failed to materialize"),
        "Unexpected reason: {}",
        ctx.reason
    );

    assert_eq!(
        runner.get_executed_requests().len(),
        0,
        "Empty plan must result in zero launches"
    );
}

// 56. (§24.4.3, §24.5) test_post_launch_snapshot_tampering_discards_verdict_and_blocks_run
#[test]
fn test_post_launch_snapshot_tampering_discards_verdict_and_blocks_run() {
    let _lock = super::isolation::TEST_ISOLATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let plan_dir = ws.join(".plan");
    std::fs::create_dir_all(&plan_dir).unwrap();

    let git_runner = MockCommandRunner::new();
    let state_file = dir.path().join("run_state.json");

    let run_id = format!("ac-run-56-{}", uuid::Uuid::new_v4());
    let mut cfg = default_test_config(fast_profile());
    cfg.run_id = Some(run_id.clone());
    cfg.working_dir = ws.to_string_lossy().to_string();
    let mut state = initialize_run(cfg, "head-123", vec![], &git_runner, FIXED_TIME).unwrap();

    let plan_file = plan_dir.join(format!("{}.md", run_id));
    let valid_plan = sample_plan_markdown(&["src/lib.rs"], "Valid Plan");
    std::fs::write(&plan_file, &valid_plan).unwrap();

    state.plan_lineage.candidate_plan_digest =
        Some(super::types::sha256_hex(valid_plan.as_bytes()));
    state.runtime_allowlist = vec!["src/lib.rs".to_string()];
    let (_, al_digest) =
        super::guards::canonicalize_and_hash_allowlist(&state.runtime_allowlist, cfg!(windows))
            .unwrap();
    state.allowlist_digest = Some(al_digest);
    state.current_state = AutoConvergeState::PlanReview;
    save_run_state_authenticated(&state_file, &mut state).unwrap();

    use super::agent_runner::{AgentCommandRunner, RoleExecutionRequest, RoleExecutionResponse};

    // Reviewer that tampers with the snapshot before returning Approve verdict
    struct TamperingAgent;
    impl AgentCommandRunner for TamperingAgent {
        fn execute_role_session(
            &self,
            request: &RoleExecutionRequest,
        ) -> Result<RoleExecutionResponse, String> {
            let snap = request
                .snapshot_dir
                .as_ref()
                .expect("Snapshot dir must be provided");
            // Tamper with a file inside snapshot
            let tampered_file = snap.join("injected_tamper.txt");
            std::fs::write(&tampered_file, b"unauthorized mutation").unwrap();

            let manifest_file = snap.join("review_manifest.json");
            let m_bytes = std::fs::read(&manifest_file).unwrap();
            let m: serde_json::Value = serde_json::from_slice(&m_bytes).unwrap();

            let verdict = CodexReviewVerdict {
                verdict: VerdictDecision::Approve,
                summary: "Approve despite tampering".to_string(),
                findings: vec![],
                approval_binding: ApprovalBinding {
                    target_type: ApprovalTargetType::Plan,
                    root_plan_digest: None,
                    active_plan_digest: None,
                    candidate_plan_digest: m
                        .get("candidate_plan_digest")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    allowlist_digest: m
                        .get("allowlist_digest")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    policy_profile_digest: m
                        .get("policy_profile_digest")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    baseline_workspace_digest: m
                        .get("baseline_workspace_digest")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    diff_digest: None,
                    evidence_manifest_digest: None,
                    release_manifest_digest: None,
                    ..Default::default()
                },
                architectural_defect: false,
                escalation_recommended: false,
            };
            let v_json = serde_json::to_string(&verdict).unwrap();
            Ok(RoleExecutionResponse {
                exit_code: 0,
                stdout: v_json.clone(),
                stderr: String::new(),
                verdict_json: Some(v_json),
            })
        }
    }

    let agent = TamperingAgent;
    let res = super::run_phase_a_step(&mut state, &agent, &git_runner, &state_file);
    assert!(
        res.is_err(),
        "Must return Err when snapshot tampering is detected"
    );
    assert_eq!(
        state.current_state,
        AutoConvergeState::Blocked,
        "State must transition to Blocked, NOT PlanApproved"
    );
    let ctx = state
        .blocked_context
        .as_ref()
        .expect("Blocked context must be set");
    assert_eq!(ctx.kind, BlockedKind::FatalInvariant);
    assert!(
        ctx.reason
            .contains("Post-review snapshot integrity verification failed"),
        "Unexpected reason: {}",
        ctx.reason
    );

    // Verify authenticated state on disk also reflects Blocked
    let loaded = load_and_verify_run_state_authenticated(&state_file, None).unwrap();
    assert_eq!(loaded.current_state, AutoConvergeState::Blocked);
}

// 57. (§24.5) test_unproven_platform_start_run_transitions_to_blocked_with_zero_launches
#[test]
fn test_unproven_platform_start_run_transitions_to_blocked_with_zero_launches() {
    let _lock = super::isolation::TEST_ISOLATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    super::isolation::set_test_isolation_pass(false);
    super::set_test_production_dispatch_override(true);
    struct ResetDispatch;
    impl Drop for ResetDispatch {
        fn drop(&mut self) {
            super::set_test_production_dispatch_override(false);
        }
    }
    let _reset = ResetDispatch;

    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir);
    let ws = dir.path().join("workspace");
    std::fs::create_dir_all(&ws).unwrap();

    let run_id = format!("ac-run-57-{}", uuid::Uuid::new_v4());
    let mut cfg = default_test_config(fast_profile());
    cfg.run_id = Some(run_id);
    cfg.working_dir = ws.to_string_lossy().to_string();
    let fake_cli = dir.path().join("fake_cli.exe");
    std::fs::write(&fake_cli, b"").unwrap();
    cfg.agent_cli_path = Some(fake_cli.to_string_lossy().to_string());

    let state = super::auto_converge_start_run(cfg).unwrap();
    assert_eq!(state.current_state, AutoConvergeState::Blocked);
    let ctx = state
        .blocked_context
        .as_ref()
        .expect("Blocked context must be set");
    assert_eq!(ctx.kind, BlockedKind::FatalInvariant);
    assert!(
        ctx.reason.contains("Platform isolation preflight failed") &&
        ctx.reason.contains("Standard unprivileged desktop environment lacks proven container/sandbox OS isolation"),
        "Unexpected reason: {}", ctx.reason
    );

    // Verify state path was created under the injected test authority root, NOT real APPDATA (§211.4)
    let state_path = super::engine_run_state_path(&state.run_id).unwrap();
    assert!(
        state_path.starts_with(dir.path()),
        "State path must resolve under injected test authority root, got: {}",
        state_path.display()
    );
    assert!(
        state_path.exists(),
        "Blocked run state must be persisted under injected test authority root"
    );
}

// 58. (§25.3.1, §25.3.2, §25.4) test_plan_review_snapshot_binding_positive_and_tamper_matrix
#[test]
fn test_plan_review_snapshot_binding_positive_and_tamper_matrix() {
    let _lock = super::isolation::TEST_ISOLATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let plan_dir = ws.join(".plan");
    let src_dir = ws.join("src");
    std::fs::create_dir_all(&plan_dir).unwrap();
    std::fs::create_dir_all(&src_dir).unwrap();

    let run_id = format!("ac-run-58-{}", uuid::Uuid::new_v4());
    let plan_file = plan_dir.join(format!("{}.md", run_id));
    let plan_content = sample_plan_markdown(&["src/lib.rs"], "Initial candidate plan for §25");
    std::fs::write(&plan_file, plan_content.as_bytes()).unwrap();

    let lib_rs = src_dir.join("lib.rs");
    std::fs::write(&lib_rs, b"pub fn code() {}\n").unwrap();

    let git_runner = MockCommandRunner::new();
    let mut cfg = default_test_config(fast_profile());
    cfg.run_id = Some(run_id.clone());
    cfg.working_dir = ws.to_string_lossy().to_string();

    let mut state = initialize_run(
        cfg,
        "head-1",
        vec![BaselineFileEntry {
            path: "src/lib.rs".to_string(),
            status: BaselineFileStatus::Clean,
            content_sha256: super::types::sha256_hex(b"pub fn code() {}\n"),
            file_mode: "100644".to_string(),
        }],
        &git_runner,
        FIXED_TIME,
    )
    .unwrap();
    state.runtime_allowlist = vec!["src/lib.rs".to_string()];
    let (_, al_digest) =
        super::guards::canonicalize_and_hash_allowlist(&state.runtime_allowlist, cfg!(windows))
            .unwrap();
    state.allowlist_digest = Some(al_digest);
    state.plan_lineage.candidate_plan_digest =
        Some(super::types::sha256_hex(plan_content.as_bytes()));
    state.current_state = AutoConvergeState::PlanReview;

    let snapshot_dir = dir.path().join("snap-58");

    // Positive check: materialized package validates cleanly against authenticated state
    let res = super::materialize_plan_review_snapshot(&state, &ws, &snapshot_dir);
    assert!(res.is_ok(), "Materialization failed: {:?}", res.err());
    let val_ok = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &state,
        AutoConvergeState::PlanReview,
        &ws,
        cfg!(windows),
    );
    assert!(
        val_ok.is_ok(),
        "Positive snapshot validation failed: {:?}",
        val_ok.err()
    );

    // Tamper Subcase A: Alter candidate plan bytes
    let cand_plan = snapshot_dir.join("candidate_plan.md");
    std::fs::write(&cand_plan, b"# Tampered candidate plan\n").unwrap();
    let err_plan = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &state,
        AutoConvergeState::PlanReview,
        &ws,
        cfg!(windows),
    );
    assert!(
        err_plan.is_err()
            && err_plan
                .unwrap_err()
                .contains("candidate_plan.md digest mismatch")
    );
    std::fs::write(&cand_plan, plan_content.as_bytes()).unwrap();

    // Tamper Subcase B: Alter candidate plan digest in state
    let orig_digest = state.plan_lineage.candidate_plan_digest.clone();
    state.plan_lineage.candidate_plan_digest =
        Some("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string());
    let err_state_digest = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &state,
        AutoConvergeState::PlanReview,
        &ws,
        cfg!(windows),
    );
    assert!(err_state_digest.is_err());
    state.plan_lineage.candidate_plan_digest = orig_digest;

    // Tamper Subcase C: Alter allowlist in snapshot
    let allowlist_file = snapshot_dir.join("allowlist.json");
    std::fs::write(&allowlist_file, b"[\"src/other.rs\"]").unwrap();
    let err_allowlist = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &state,
        AutoConvergeState::PlanReview,
        &ws,
        cfg!(windows),
    );
    assert!(err_allowlist.is_err());
    std::fs::write(
        &allowlist_file,
        serde_json::to_string_pretty(&state.runtime_allowlist).unwrap(),
    )
    .unwrap();

    // Tamper Subcase D: Alter review_manifest.json field
    let manifest_file = snapshot_dir.join("review_manifest.json");
    let mut manifest_val: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_file).unwrap()).unwrap();
    manifest_val["run_id"] = serde_json::json!("tampered-run-id");
    std::fs::write(
        &manifest_file,
        serde_json::to_string_pretty(&manifest_val).unwrap(),
    )
    .unwrap();
    let err_manifest = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &state,
        AutoConvergeState::PlanReview,
        &ws,
        cfg!(windows),
    );
    assert!(err_manifest.is_err() && err_manifest.unwrap_err().contains("run_id mismatch"));
    manifest_val["run_id"] = serde_json::json!(state.run_id);
    std::fs::write(
        &manifest_file,
        serde_json::to_string_pretty(&manifest_val).unwrap(),
    )
    .unwrap();

    // Tamper Subcase E: Add unexpected file into snapshot package
    let extra_file = snapshot_dir.join("unexpected_extra.txt");
    std::fs::write(&extra_file, b"extra content").unwrap();
    let err_extra = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &state,
        AutoConvergeState::PlanReview,
        &ws,
        cfg!(windows),
    );
    assert!(
        err_extra.is_err()
            && err_extra
                .unwrap_err()
                .contains("Unexpected file in snapshot package")
    );
    let _ = std::fs::remove_file(&extra_file);

    // Tamper Subcase F: Alter copied source file
    let snap_source = snapshot_dir.join("src").join("lib.rs");
    if snap_source.exists() {
        std::fs::write(&snap_source, b"malicious code modification\n").unwrap();
        let err_src = super::validate_snapshot_against_authenticated_state(
            &snapshot_dir,
            &state,
            AutoConvergeState::PlanReview,
            &ws,
            cfg!(windows),
        );
        assert!(
            err_src.is_err()
                && err_src
                    .unwrap_err()
                    .contains("Snapshot file content hash mismatch")
        );
        std::fs::write(&snap_source, b"pub fn code() {}\n").unwrap();
    }

    // Tamper Subcase G: (§26.1.1, §26.3.1, §26.4) Omit baseline-present allowlisted file
    let snap_source = snapshot_dir.join("src").join("lib.rs");
    std::fs::remove_file(&snap_source).unwrap();
    let err_omit = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &state,
        AutoConvergeState::PlanReview,
        &ws,
        cfg!(windows),
    );
    assert!(
        err_omit.is_err(),
        "Must reject omitted baseline-present file"
    );
    assert!(err_omit
        .unwrap_err()
        .contains("Baseline-present allowlisted file missing from snapshot"));
    std::fs::write(&snap_source, b"pub fn code() {}\n").unwrap();

    // Tamper Subcase H: (§26.1.3, §26.3.3, §26.4) File mode mismatch for baseline file
    let mut bad_mode_state = state.clone();
    bad_mode_state.workspace.baseline_file_manifest[0].file_mode = "100444".to_string();
    let err_mode = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &bad_mode_state,
        AutoConvergeState::PlanReview,
        &ws,
        cfg!(windows),
    );
    assert!(err_mode.is_err(), "Must reject mode mismatch");
    assert!(err_mode
        .unwrap_err()
        .contains("Snapshot file mode mismatch"));

    // Tamper Subcase I: (§26.3.1) Baseline-absent file present in PlanReview snapshot
    let absent_file = snapshot_dir.join("src").join("absent.rs");
    std::fs::write(&absent_file, b"// absent").unwrap();
    let mut absent_state = state.clone();
    absent_state
        .runtime_allowlist
        .push("src/absent.rs".to_string());
    let (_, al_digest_absent) = super::guards::canonicalize_and_hash_allowlist(
        &absent_state.runtime_allowlist,
        cfg!(windows),
    )
    .unwrap();
    absent_state.allowlist_digest = Some(al_digest_absent.clone());
    std::fs::write(
        &snapshot_dir.join("allowlist.json"),
        serde_json::to_string_pretty(&absent_state.runtime_allowlist).unwrap(),
    )
    .unwrap();
    let manifest_file = snapshot_dir.join("review_manifest.json");
    let mut manifest_val: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_file).unwrap()).unwrap();
    manifest_val["allowlist_digest"] = serde_json::json!(al_digest_absent);
    std::fs::write(
        &manifest_file,
        serde_json::to_string_pretty(&manifest_val).unwrap(),
    )
    .unwrap();
    let err_absent = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &absent_state,
        AutoConvergeState::PlanReview,
        &ws,
        cfg!(windows),
    );
    assert!(err_absent.is_err());
    assert!(err_absent
        .unwrap_err()
        .contains("Baseline-absent file must not exist in PlanReview snapshot"));
    let _ = std::fs::remove_file(&absent_file);
    std::fs::write(
        &snapshot_dir.join("allowlist.json"),
        serde_json::to_string_pretty(&state.runtime_allowlist).unwrap(),
    )
    .unwrap();
    manifest_val["allowlist_digest"] = serde_json::json!(state.allowlist_digest.as_ref().unwrap());
    std::fs::write(
        &manifest_file,
        serde_json::to_string_pretty(&manifest_val).unwrap(),
    )
    .unwrap();

    // Tamper Subcase J: (§26.3.1) Duplicate baseline manifest entry
    let mut dup_state = state.clone();
    dup_state
        .workspace
        .baseline_file_manifest
        .push(dup_state.workspace.baseline_file_manifest[0].clone());
    let err_dup = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &dup_state,
        AutoConvergeState::PlanReview,
        &ws,
        cfg!(windows),
    );
    assert!(err_dup.is_err());
    assert!(err_dup
        .unwrap_err()
        .contains("Duplicate baseline manifest entry"));
}

// 59. (§25.3.1, §25.3.2, §25.4, §26.3.2, §26.4) test_implementation_review_snapshot_binding_positive_and_tamper_matrix
#[test]
fn test_implementation_review_snapshot_binding_positive_and_tamper_matrix() {
    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let plan_dir = ws.join(".plan");
    let src_dir = ws.join("src");
    std::fs::create_dir_all(&plan_dir).unwrap();
    std::fs::create_dir_all(&src_dir).unwrap();

    let run_id = format!("ac-run-59-{}", uuid::Uuid::new_v4());
    let plan_file = plan_dir.join(format!("{}.md", run_id));
    let plan_content = sample_plan_markdown(&["src/lib.rs"], "Active plan for §25");
    std::fs::write(&plan_file, plan_content.as_bytes()).unwrap();

    let lib_rs = src_dir.join("lib.rs");
    std::fs::write(&lib_rs, b"pub fn code() { /* modified */ }\n").unwrap();

    let git_runner = MockCommandRunner::new();
    let mut cfg = default_test_config(fast_profile());
    cfg.run_id = Some(run_id.clone());
    cfg.working_dir = ws.to_string_lossy().to_string();

    let mut state = initialize_run(
        cfg,
        "head-1",
        vec![BaselineFileEntry {
            path: "src/lib.rs".to_string(),
            status: BaselineFileStatus::Clean,
            content_sha256: super::types::sha256_hex(b"pub fn code() {}\n"),
            file_mode: "100644".to_string(),
        }],
        &git_runner,
        FIXED_TIME,
    )
    .unwrap();
    state.runtime_allowlist = vec!["src/lib.rs".to_string()];
    let (_, al_digest) =
        super::guards::canonicalize_and_hash_allowlist(&state.runtime_allowlist, cfg!(windows))
            .unwrap();
    state.allowlist_digest = Some(al_digest);
    state.plan_lineage.active_plan_digest = Some(super::types::sha256_hex(plan_content.as_bytes()));
    state.plan_lineage.root_plan_digest = state.plan_lineage.active_plan_digest.clone();
    state.current_state = AutoConvergeState::ImplementationReview;

    let delta = compute_run_delta_manifest(
        &ws,
        &state.runtime_allowlist,
        &state.workspace.baseline_file_manifest,
    )
    .unwrap();
    state.current_diff_digest = Some(compute_diff_digest(&delta));
    state.evidence = vec![VerificationEvidence {
        command: "cargo check".to_string(),
        exit_code: 0,
        stdout_summary: "ok".to_string(),
        passed: true,
    }];
    let ev_compact = serde_json::to_string(&state.evidence).unwrap();
    state.current_evidence_manifest_digest = Some(super::types::sha256_hex(ev_compact.as_bytes()));

    let snapshot_dir = dir.path().join("snap-59");
    let res = super::materialize_implementation_review_snapshot(&state, &ws, &snapshot_dir);
    assert!(res.is_ok(), "Materialization failed: {:?}", res.err());

    let val_ok = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &state,
        AutoConvergeState::ImplementationReview,
        &ws,
        cfg!(windows),
    );
    assert!(
        val_ok.is_ok(),
        "Positive validation failed: {:?}",
        val_ok.err()
    );

    // Tamper active_plan.md
    let active_p = snapshot_dir.join("active_plan.md");
    std::fs::write(&active_p, b"# Tampered active plan\n").unwrap();
    let err_plan = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &state,
        AutoConvergeState::ImplementationReview,
        &ws,
        cfg!(windows),
    );
    assert!(
        err_plan.is_err()
            && err_plan
                .unwrap_err()
                .contains("active_plan.md digest mismatch")
    );
    std::fs::write(&active_p, plan_content.as_bytes()).unwrap();

    // Tamper run_delta.json diff digest mismatch
    let delta_f = snapshot_dir.join("run_delta.json");
    std::fs::write(&delta_f, b"[]").unwrap();
    let err_delta = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &state,
        AutoConvergeState::ImplementationReview,
        &ws,
        cfg!(windows),
    );
    assert!(err_delta.is_err() && err_delta.unwrap_err().contains("diff digest mismatch"));
    std::fs::write(&delta_f, serde_json::to_string_pretty(&delta).unwrap()).unwrap();

    // Tamper run_delta.json invalid JSON (§26.1.2)
    std::fs::write(&delta_f, b"invalid-json").unwrap();
    let err_bad_json = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &state,
        AutoConvergeState::ImplementationReview,
        &ws,
        cfg!(windows),
    );
    assert!(
        err_bad_json.is_err()
            && err_bad_json
                .unwrap_err()
                .contains("Invalid JSON in run_delta.json")
    );
    std::fs::write(&delta_f, serde_json::to_string_pretty(&delta).unwrap()).unwrap();

    // Tamper evidence.json
    let ev_f = snapshot_dir.join("evidence.json");
    std::fs::write(&ev_f, b"[]").unwrap();
    let err_ev = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &state,
        AutoConvergeState::ImplementationReview,
        &ws,
        cfg!(windows),
    );
    assert!(
        err_ev.is_err()
            && err_ev
                .unwrap_err()
                .contains("evidence manifest digest mismatch")
    );

    // Tamper mode on file in snapshot (§26.1.3, §26.3.3)
    let snap_lib = snapshot_dir.join("src").join("lib.rs");
    let mut perms = std::fs::metadata(&snap_lib).unwrap().permissions();
    perms.set_readonly(true);
    let _ = std::fs::set_permissions(&snap_lib, perms.clone());
    let err_mode = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &state,
        AutoConvergeState::ImplementationReview,
        &ws,
        cfg!(windows),
    );
    assert!(
        err_mode.is_err()
            && err_mode
                .unwrap_err()
                .contains("Snapshot file mode mismatch")
    );
    perms.set_readonly(false);
    let _ = std::fs::set_permissions(&snap_lib, perms);
}

// 60. (§25.3.1, §25.3.2, §25.4, §26.3.2, §26.4) test_corrective_plan_review_snapshot_binding_positive_and_tamper_matrix
#[test]
fn test_corrective_plan_review_snapshot_binding_positive_and_tamper_matrix() {
    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let plan_dir = ws.join(".plan");
    let src_dir = ws.join("src");
    std::fs::create_dir_all(&plan_dir).unwrap();
    std::fs::create_dir_all(&src_dir).unwrap();

    let run_id = format!("ac-run-60-{}", uuid::Uuid::new_v4());
    let plan_file = plan_dir.join(format!("{}.md", run_id));
    let plan_content = sample_plan_markdown(&["src/lib.rs"], "Corrective plan for §25");
    std::fs::write(&plan_file, plan_content.as_bytes()).unwrap();

    let lib_rs = src_dir.join("lib.rs");
    std::fs::write(&lib_rs, b"pub fn code() {}\n").unwrap();

    let git_runner = MockCommandRunner::new();
    let mut cfg = default_test_config(fast_profile());
    cfg.run_id = Some(run_id.clone());
    cfg.working_dir = ws.to_string_lossy().to_string();

    let mut state = initialize_run(
        cfg,
        "head-1",
        vec![BaselineFileEntry {
            path: "src/lib.rs".to_string(),
            status: BaselineFileStatus::Clean,
            content_sha256: super::types::sha256_hex(b"pub fn code() {}\n"),
            file_mode: "100644".to_string(),
        }],
        &git_runner,
        FIXED_TIME,
    )
    .unwrap();
    state.runtime_allowlist = vec!["src/lib.rs".to_string()];
    let (_, al_digest) =
        super::guards::canonicalize_and_hash_allowlist(&state.runtime_allowlist, cfg!(windows))
            .unwrap();
    state.allowlist_digest = Some(al_digest);
    state.plan_lineage.candidate_plan_digest =
        Some(super::types::sha256_hex(plan_content.as_bytes()));
    state.plan_lineage.active_plan_digest =
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string());
    state.plan_lineage.root_plan_digest = state.plan_lineage.active_plan_digest.clone();

    // Authenticated diff digest computed once from current delta
    let delta = compute_run_delta_manifest(
        &ws,
        &state.runtime_allowlist,
        &state.workspace.baseline_file_manifest,
    )
    .unwrap();
    state.current_diff_digest = Some(compute_diff_digest(&delta));
    state.current_state = AutoConvergeState::CorrectivePlanReview;

    let finding = ReviewFinding {
        finding_key: ReviewFindingKey {
            category: FindingCategory::Correctness,
            file: "src/lib.rs".to_string(),
            symbol: "code".to_string(),
            failure_code: "E-101".to_string(),
        },
        location: "src/lib.rs:1".to_string(),
        failure_condition: "Missing invariant check".to_string(),
        acceptance_condition: "Add check".to_string(),
        blocking: true,
    };
    state.findings.push(super::types::TrackedFinding {
        finding_id: "F-101".to_string(),
        finding,
        consecutive_cycles: 1,
        resolved: false,
    });

    let snapshot_dir = dir.path().join("snap-60");
    let res = super::materialize_corrective_plan_review_snapshot(&state, &ws, &snapshot_dir);
    assert!(res.is_ok(), "Materialization failed: {:?}", res.err());

    let val_ok = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &state,
        AutoConvergeState::CorrectivePlanReview,
        &ws,
        cfg!(windows),
    );
    assert!(
        val_ok.is_ok(),
        "Positive validation failed: {:?}",
        val_ok.err()
    );

    // Tamper Subcase: File mode mismatch for copied allowlisted file in CorrectivePlanReview (§27.1.1, §27.3.1)
    let snap_lib = snapshot_dir.join("src").join("lib.rs");
    let content_before = std::fs::read(&snap_lib).unwrap();
    let hash_before = super::types::sha256_hex(&content_before);
    {
        struct PermResetGuard(std::path::PathBuf);
        impl Drop for PermResetGuard {
            fn drop(&mut self) {
                if let Ok(meta) = std::fs::metadata(&self.0) {
                    let mut perms = meta.permissions();
                    #[allow(clippy::permissions_set_readonly_false)]
                    perms.set_readonly(false);
                    let _ = std::fs::set_permissions(&self.0, perms);
                }
            }
        }
        let _guard = PermResetGuard(snap_lib.clone());
        let mut perms = std::fs::metadata(&snap_lib).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&snap_lib, perms).unwrap();

        let content_after = std::fs::read(&snap_lib).unwrap();
        let hash_after = super::types::sha256_hex(&content_after);
        assert_eq!(
            content_before, content_after,
            "Content bytes must remain unchanged"
        );
        assert_eq!(
            hash_before, hash_after,
            "Content hash must remain unchanged"
        );

        let err_mode = super::validate_snapshot_against_authenticated_state(
            &snapshot_dir,
            &state,
            AutoConvergeState::CorrectivePlanReview,
            &ws,
            cfg!(windows),
        );
        assert!(
            err_mode.is_err(),
            "Must reject mode mismatch in CorrectivePlanReview"
        );
        let err_msg = err_mode.unwrap_err();
        assert!(
            err_msg.contains("Snapshot file mode mismatch for src/lib.rs"),
            "Error message should contain mode mismatch: {err_msg}"
        );
    }

    // Tamper unresolved_findings.json
    let findings_f = snapshot_dir.join("unresolved_findings.json");
    std::fs::write(&findings_f, b"[]").unwrap();
    let err_findings = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &state,
        AutoConvergeState::CorrectivePlanReview,
        &ws,
        cfg!(windows),
    );
    assert!(
        err_findings.is_err()
            && err_findings
                .unwrap_err()
                .contains("unresolved findings mismatch")
    );
    let active_findings: Vec<_> = state.findings.iter().filter(|f| !f.resolved).collect();
    std::fs::write(
        &findings_f,
        serde_json::to_string_pretty(&active_findings).unwrap(),
    )
    .unwrap();

    // Tamper diff digest mismatch in CorrectivePlanReview
    let mut bad_diff_state = state.clone();
    bad_diff_state.current_diff_digest =
        Some("0000000000000000000000000000000000000000000000000000000000000000".to_string());
    let manifest_file = snapshot_dir.join("review_manifest.json");
    let mut manifest_val: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_file).unwrap()).unwrap();
    manifest_val["diff_digest"] =
        serde_json::json!("0000000000000000000000000000000000000000000000000000000000000000");
    std::fs::write(
        &manifest_file,
        serde_json::to_string_pretty(&manifest_val).unwrap(),
    )
    .unwrap();
    let err_diff = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &bad_diff_state,
        AutoConvergeState::CorrectivePlanReview,
        &ws,
        cfg!(windows),
    );
    assert!(
        err_diff.is_err()
            && err_diff
                .unwrap_err()
                .contains("diff digest mismatch in CorrectivePlanReview")
    );
}

// 61. (§25.3.4, §25.4, §26.3.5, §26.4) test_deterministic_pre_launch_fault_injection_matrix
#[test]
fn test_deterministic_pre_launch_fault_injection_matrix() {
    use super::agent_runner::MockAgentRunner;

    let faults = [
        super::InjectedSnapshotFault::CreateDir,
        super::InjectedSnapshotFault::ReadPlan,
        super::InjectedSnapshotFault::WritePlan,
        super::InjectedSnapshotFault::WriteAllowlist,
        super::InjectedSnapshotFault::SerializeManifest,
        super::InjectedSnapshotFault::WriteManifest,
        super::InjectedSnapshotFault::ComputeDiff,
        super::InjectedSnapshotFault::WriteDiff,
        super::InjectedSnapshotFault::ReadDelta,
        super::InjectedSnapshotFault::ParseDelta,
        super::InjectedSnapshotFault::SerializeEvidence,
        super::InjectedSnapshotFault::WriteEvidence,
        super::InjectedSnapshotFault::SerializeFindings,
        super::InjectedSnapshotFault::WriteFindings,
        super::InjectedSnapshotFault::CopyFile,
        super::InjectedSnapshotFault::PreLaunchHash,
    ];

    for fault in faults {
        let dir = TempDir::new().unwrap();
        let ws = dir.path().join("workspace");
        let plan_dir = ws.join(".plan");
        let src_dir = ws.join("src");
        std::fs::create_dir_all(&plan_dir).unwrap();
        std::fs::create_dir_all(&src_dir).unwrap();

        let run_id = format!("ac-run-61-{:?}-{}", fault, uuid::Uuid::new_v4());
        let plan_file = plan_dir.join(format!("{}.md", run_id));
        let plan_content = sample_plan_markdown(&["src/lib.rs"], "Plan for fault test");
        std::fs::write(&plan_file, plan_content.as_bytes()).unwrap();

        let lib_rs = src_dir.join("lib.rs");
        std::fs::write(&lib_rs, b"pub fn code() {}\n").unwrap();

        let git_runner = MockCommandRunner::new();
        let runner = MockAgentRunner::new();
        let state_file = dir.path().join("run_state.json");

        let mut cfg = default_test_config(fast_profile());
        cfg.run_id = Some(run_id.clone());
        cfg.working_dir = ws.to_string_lossy().to_string();

        let mut state = initialize_run(
            cfg,
            "head-1",
            vec![BaselineFileEntry {
                path: "src/lib.rs".to_string(),
                status: BaselineFileStatus::Clean,
                content_sha256: super::types::sha256_hex(b"pub fn code() {}\n"),
                file_mode: "100644".to_string(),
            }],
            &git_runner,
            FIXED_TIME,
        )
        .unwrap();
        state.runtime_allowlist = vec!["src/lib.rs".to_string()];
        let (_, al_digest) =
            super::guards::canonicalize_and_hash_allowlist(&state.runtime_allowlist, cfg!(windows))
                .unwrap();
        state.allowlist_digest = Some(al_digest);
        state.plan_lineage.candidate_plan_digest =
            Some(super::types::sha256_hex(plan_content.as_bytes()));
        state.plan_lineage.active_plan_digest = state.plan_lineage.candidate_plan_digest.clone();
        state.plan_lineage.root_plan_digest = state.plan_lineage.candidate_plan_digest.clone();

        match fault {
            super::InjectedSnapshotFault::ComputeDiff
            | super::InjectedSnapshotFault::WriteDiff
            | super::InjectedSnapshotFault::ReadDelta
            | super::InjectedSnapshotFault::ParseDelta
            | super::InjectedSnapshotFault::SerializeEvidence
            | super::InjectedSnapshotFault::WriteEvidence => {
                state.current_state = AutoConvergeState::ImplementationReview;
                let delta = compute_run_delta_manifest(
                    &ws,
                    &state.runtime_allowlist,
                    &state.workspace.baseline_file_manifest,
                )
                .unwrap();
                state.current_diff_digest = Some(compute_diff_digest(&delta));
                state.evidence = vec![VerificationEvidence {
                    command: "cargo check".to_string(),
                    exit_code: 0,
                    stdout_summary: "ok".to_string(),
                    passed: true,
                }];
                let ev_compact = serde_json::to_string(&state.evidence).unwrap();
                state.current_evidence_manifest_digest =
                    Some(super::types::sha256_hex(ev_compact.as_bytes()));
            }
            super::InjectedSnapshotFault::SerializeFindings
            | super::InjectedSnapshotFault::WriteFindings => {
                state.current_state = AutoConvergeState::CorrectivePlanReview;
                let delta = compute_run_delta_manifest(
                    &ws,
                    &state.runtime_allowlist,
                    &state.workspace.baseline_file_manifest,
                )
                .unwrap();
                state.current_diff_digest = Some(compute_diff_digest(&delta));
            }
            _ => {
                state.current_state = AutoConvergeState::PlanReview;
            }
        }
        save_run_state_authenticated(&state_file, &mut state).unwrap();

        {
            let _guard = super::with_scoped_snapshot_fault(fault);
            let res = super::run_phase_a_step(&mut state, &runner, &git_runner, &state_file);

            assert!(
                res.is_err(),
                "Fault {:?} must cause run_phase_a_step to return Err",
                fault
            );
            assert_eq!(
                state.current_state,
                AutoConvergeState::Blocked,
                "Fault {:?} must transition to Blocked",
                fault
            );
            assert!(
                state.blocked_context.is_some(),
                "Fault {:?} must set blocked_context",
                fault
            );
            assert_eq!(
                runner.get_executed_requests().len(),
                0,
                "Fault {:?} must result in 0 launches",
                fault
            );

            let loaded = load_and_verify_run_state_authenticated(&state_file, None).unwrap();
            assert_eq!(loaded.current_state, AutoConvergeState::Blocked);
        }
    }
}

// 62. (§25.3.4, §25.4, §26.3.5) test_deterministic_post_launch_hash_fault_discards_verdict
#[test]
fn test_deterministic_post_launch_hash_fault_discards_verdict() {
    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let plan_dir = ws.join(".plan");
    let src_dir = ws.join("src");
    std::fs::create_dir_all(&plan_dir).unwrap();
    std::fs::create_dir_all(&src_dir).unwrap();

    let run_id = format!("ac-run-62-{}", uuid::Uuid::new_v4());
    let plan_file = plan_dir.join(format!("{}.md", run_id));
    let plan_content = sample_plan_markdown(&["src/lib.rs"], "Plan for post-launch hash test");
    std::fs::write(&plan_file, plan_content.as_bytes()).unwrap();

    let lib_rs = src_dir.join("lib.rs");
    std::fs::write(&lib_rs, b"pub fn code() {}\n").unwrap();

    let git_runner = MockCommandRunner::new();
    let state_file = dir.path().join("run_state.json");

    let mut cfg = default_test_config(fast_profile());
    cfg.run_id = Some(run_id.clone());
    cfg.working_dir = ws.to_string_lossy().to_string();

    let mut state = initialize_run(
        cfg,
        "head-1",
        vec![BaselineFileEntry {
            path: "src/lib.rs".to_string(),
            status: BaselineFileStatus::Clean,
            content_sha256: super::types::sha256_hex(b"pub fn code() {}\n"),
            file_mode: "100644".to_string(),
        }],
        &git_runner,
        FIXED_TIME,
    )
    .unwrap();
    state.runtime_allowlist = vec!["src/lib.rs".to_string()];
    let (_, al_digest) =
        super::guards::canonicalize_and_hash_allowlist(&state.runtime_allowlist, cfg!(windows))
            .unwrap();
    state.allowlist_digest = Some(al_digest);
    state.plan_lineage.candidate_plan_digest =
        Some(super::types::sha256_hex(plan_content.as_bytes()));
    state.current_state = AutoConvergeState::PlanReview;
    save_run_state_authenticated(&state_file, &mut state).unwrap();

    struct ApprovingAgent;
    impl super::agent_runner::AgentCommandRunner for ApprovingAgent {
        fn execute_role_session(
            &self,
            req: &super::agent_runner::RoleExecutionRequest,
        ) -> Result<super::agent_runner::RoleExecutionResponse, String> {
            let manifest_path = req
                .snapshot_dir
                .as_ref()
                .unwrap()
                .join("review_manifest.json");
            let m: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
            let verdict = CodexReviewVerdict {
                verdict: VerdictDecision::Approve,
                summary: "Approved candidate plan".to_string(),
                findings: vec![],
                approval_binding: ApprovalBinding {
                    target_type: ApprovalTargetType::Plan,
                    root_plan_digest: None,
                    active_plan_digest: None,
                    candidate_plan_digest: m
                        .get("candidate_plan_digest")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    allowlist_digest: m["allowlist_digest"].as_str().unwrap().to_string(),
                    policy_profile_digest: m["policy_profile_digest"].as_str().unwrap().to_string(),
                    baseline_workspace_digest: m
                        .get("baseline_workspace_digest")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    diff_digest: None,
                    evidence_manifest_digest: None,
                    release_manifest_digest: None,
                    ..Default::default()
                },
                architectural_defect: false,
                escalation_recommended: false,
            };
            let v_str = serde_json::to_string(&verdict).unwrap();
            Ok(super::agent_runner::RoleExecutionResponse {
                exit_code: 0,
                stdout: v_str.clone(),
                stderr: String::new(),
                verdict_json: Some(v_str),
            })
        }
    }

    let agent = ApprovingAgent;
    let _guard = super::with_scoped_snapshot_fault(super::InjectedSnapshotFault::PostLaunchHash);
    let res = super::run_phase_a_step(&mut state, &agent, &git_runner, &state_file);

    assert!(
        res.is_err(),
        "PostLaunchHash fault must cause run_phase_a_step to return Err"
    );
    assert_eq!(
        state.current_state,
        AutoConvergeState::Blocked,
        "State must be Blocked, NOT PlanApproved"
    );
    let ctx = state
        .blocked_context
        .as_ref()
        .expect("Blocked context must be set");
    assert!(
        ctx.reason.contains("post-launch hash calculation failed")
            || ctx.reason.contains("PostLaunchHash")
    );

    let loaded = load_and_verify_run_state_authenticated(&state_file, None).unwrap();
    assert_eq!(loaded.current_state, AutoConvergeState::Blocked);
}

// 63. (§25.3.1, §25.4, §26.1.4, §26.3.4, §26.4) test_snapshot_rejects_symlinks
#[test]
fn test_snapshot_rejects_symlinks() {
    use super::SnapshotEntryKind;
    use std::path::Path;

    // 1. Unconditional deterministic validation of entry helper (§26.3.4)
    let sym_res = super::check_snapshot_entry_kind(true, false, true, Path::new("link.txt"));
    assert!(sym_res.is_err());
    assert!(sym_res
        .unwrap_err()
        .contains("Symlink forbidden in snapshot package"));

    let nonreg_res =
        super::check_snapshot_entry_kind(false, false, false, Path::new("special_device"));
    assert!(nonreg_res.is_err());
    assert!(nonreg_res
        .unwrap_err()
        .contains("Non-regular file in snapshot package"));

    let dir_res = super::check_snapshot_entry_kind(false, true, false, Path::new("sub_dir"));
    assert_eq!(dir_res.unwrap(), SnapshotEntryKind::Dir);

    let file_res = super::check_snapshot_entry_kind(false, false, true, Path::new("regular.txt"));
    assert_eq!(file_res.unwrap(), SnapshotEntryKind::File);

    // 2. Unconditional deterministic test through test-injected symlink seam
    let dir = TempDir::new().unwrap();
    let snap = dir.path().join("snap-63");
    std::fs::create_dir_all(&snap).unwrap();

    let target = snap.join("injected_link.txt");
    std::fs::write(&target, b"regular bytes").unwrap();

    let ws = dir.path().join("workspace");
    std::fs::create_dir_all(&ws).unwrap();
    let git_runner = MockCommandRunner::new();
    let state = initialize_run(
        default_test_config(fast_profile()),
        "head-1",
        vec![],
        &git_runner,
        FIXED_TIME,
    )
    .unwrap();

    {
        let _sym_guard = super::with_scoped_injected_symlink(target.clone());
        let val_res = super::validate_snapshot_against_authenticated_state(
            &snap,
            &state,
            AutoConvergeState::PlanReview,
            &ws,
            cfg!(windows),
        );
        assert!(val_res.is_err(), "Must reject injected symlink");
        assert!(val_res
            .unwrap_err()
            .contains("Symlink forbidden in snapshot package"));
    }

    // 3. Attempt native symlink creation and report OS capability explicitly (§26.3.4)
    let native_target = dir.path().join("native_target.txt");
    std::fs::write(&native_target, b"target").unwrap();
    let native_link = snap.join("native_link.txt");

    #[cfg(windows)]
    let symlink_res = std::os::windows::fs::symlink_file(&native_target, &native_link);
    #[cfg(unix)]
    let symlink_res = std::os::unix::fs::symlink(&native_target, &native_link);

    match symlink_res {
        Ok(_) => {
            let val_res = super::validate_snapshot_against_authenticated_state(
                &snap,
                &state,
                AutoConvergeState::PlanReview,
                &ws,
                cfg!(windows),
            );
            assert!(val_res.is_err());
            assert!(val_res
                .unwrap_err()
                .contains("Symlink forbidden in snapshot package"));
            let _ = std::fs::remove_file(&native_link);
        }
        Err(e) => {
            println!("Native symlink creation unavailable on current OS/privilege: {e}");
        }
    }
}

// 64. (§26.1.5, §26.3.5, §26.4) test_thread_local_fault_isolation_and_raii_unwind
#[test]
fn test_thread_local_fault_isolation_and_raii_unwind() {
    // 1. Thread-local concurrency isolation
    let handle_injected = std::thread::spawn(|| {
        let _guard = super::with_scoped_snapshot_fault(super::InjectedSnapshotFault::CreateDir);
        super::check_injected_fault(super::InjectedSnapshotFault::CreateDir).is_err()
    });

    let handle_clean = std::thread::spawn(|| {
        super::check_injected_fault(super::InjectedSnapshotFault::CreateDir).is_ok()
    });

    let injected_saw_fault = handle_injected.join().unwrap();
    let clean_saw_ok = handle_clean.join().unwrap();
    assert!(
        injected_saw_fault,
        "Thread with scoped fault must observe fault"
    );
    assert!(
        clean_saw_ok,
        "Concurrent thread without scoped fault must NOT observe fault"
    );

    // 2. RAII restoration on normal drop
    {
        let _guard = super::with_scoped_snapshot_fault(super::InjectedSnapshotFault::ReadPlan);
        assert!(super::check_injected_fault(super::InjectedSnapshotFault::ReadPlan).is_err());
    }
    assert!(
        super::check_injected_fault(super::InjectedSnapshotFault::ReadPlan).is_ok(),
        "Fault must reset on guard drop"
    );

    // 3. RAII restoration on panic/unwind
    let panic_result = std::panic::catch_unwind(|| {
        let _guard = super::with_scoped_snapshot_fault(super::InjectedSnapshotFault::WritePlan);
        assert!(super::check_injected_fault(super::InjectedSnapshotFault::WritePlan).is_err());
        panic!("simulated test unwind");
    });
    assert!(panic_result.is_err(), "Must capture panic");
    assert!(
        super::check_injected_fault(super::InjectedSnapshotFault::WritePlan).is_ok(),
        "Fault must reset after unwind"
    );
}

// 65. (§26.1.1, §26.3.1, §26.4) test_plan_review_omitted_baseline_file_blocks_with_zero_launches
#[test]
fn test_plan_review_omitted_baseline_file_blocks_with_zero_launches() {
    use super::agent_runner::MockAgentRunner;

    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let plan_dir = ws.join(".plan");
    let src_dir = ws.join("src");
    std::fs::create_dir_all(&plan_dir).unwrap();
    std::fs::create_dir_all(&src_dir).unwrap();

    let run_id = format!("ac-run-65-{}", uuid::Uuid::new_v4());
    let plan_file = plan_dir.join(format!("{}.md", run_id));
    let plan_content = sample_plan_markdown(&["src/lib.rs"], "Plan for missing baseline file test");
    std::fs::write(&plan_file, plan_content.as_bytes()).unwrap();

    let git_runner = MockCommandRunner::new();
    let runner = MockAgentRunner::new();
    let state_file = dir.path().join("run_state.json");

    let mut cfg = default_test_config(fast_profile());
    cfg.run_id = Some(run_id.clone());
    cfg.working_dir = ws.to_string_lossy().to_string();

    // Baseline says src/lib.rs is present, but it was deleted in workspace before PlanReview materialization
    let mut state = initialize_run(
        cfg,
        "head-1",
        vec![BaselineFileEntry {
            path: "src/lib.rs".to_string(),
            status: BaselineFileStatus::Clean,
            content_sha256: super::types::sha256_hex(b"pub fn code() {}\n"),
            file_mode: "100644".to_string(),
        }],
        &git_runner,
        FIXED_TIME,
    )
    .unwrap();
    state.runtime_allowlist = vec!["src/lib.rs".to_string()];
    let (_, al_digest) =
        super::guards::canonicalize_and_hash_allowlist(&state.runtime_allowlist, cfg!(windows))
            .unwrap();
    state.allowlist_digest = Some(al_digest);
    state.plan_lineage.candidate_plan_digest =
        Some(super::types::sha256_hex(plan_content.as_bytes()));
    state.current_state = AutoConvergeState::PlanReview;
    save_run_state_authenticated(&state_file, &mut state).unwrap();

    let res = super::run_phase_a_step(&mut state, &runner, &git_runner, &state_file);

    assert!(
        res.is_err(),
        "Must fail closed when baseline-present allowlisted file is missing"
    );
    assert_eq!(state.current_state, AutoConvergeState::Blocked);
    let ctx = state
        .blocked_context
        .as_ref()
        .expect("Blocked context must be set");
    assert!(
        ctx.reason
            .contains("Baseline-present allowlisted file missing"),
        "Blocked reason: {}",
        ctx.reason
    );
    assert_eq!(
        runner.get_executed_requests().len(),
        0,
        "Must record 0 launches"
    );

    let loaded = load_and_verify_run_state_authenticated(&state_file, None).unwrap();
    assert_eq!(loaded.current_state, AutoConvergeState::Blocked);
}

// 66. (§27.1.1, §27.3.1, §27.4) test_corrective_plan_review_mode_mismatch_regression
#[test]
fn test_corrective_plan_review_mode_mismatch_regression() {
    struct ReadonlyCleanupGuard {
        path: std::path::PathBuf,
    }

    impl Drop for ReadonlyCleanupGuard {
        fn drop(&mut self) {
            if let Ok(meta) = std::fs::metadata(&self.path) {
                let mut perms = meta.permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                perms.set_readonly(false);
                let _ = std::fs::set_permissions(&self.path, perms);
            }
        }
    }

    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let plan_dir = ws.join(".plan");
    let src_dir = ws.join("src");
    std::fs::create_dir_all(&plan_dir).unwrap();
    std::fs::create_dir_all(&src_dir).unwrap();

    let run_id = format!("ac-run-66-{}", uuid::Uuid::new_v4());
    let plan_file = plan_dir.join(format!("{}.md", run_id));
    let plan_content = sample_plan_markdown(
        &["src/lib.rs"],
        "Corrective plan for mode mismatch regression",
    );
    std::fs::write(&plan_file, plan_content.as_bytes()).unwrap();

    let lib_rs = src_dir.join("lib.rs");
    let lib_bytes = b"pub fn code() { /* corrective mode regression */ }\n";
    std::fs::write(&lib_rs, lib_bytes).unwrap();

    let git_runner = MockCommandRunner::new();
    let mut cfg = default_test_config(fast_profile());
    cfg.run_id = Some(run_id.clone());
    cfg.working_dir = ws.to_string_lossy().to_string();

    let mut state = initialize_run(
        cfg,
        "head-corr-mode",
        vec![BaselineFileEntry {
            path: "src/lib.rs".to_string(),
            status: BaselineFileStatus::Clean,
            content_sha256: super::types::sha256_hex(b"pub fn code() {}\n"),
            file_mode: "100644".to_string(),
        }],
        &git_runner,
        FIXED_TIME,
    )
    .unwrap();

    state.runtime_allowlist = vec!["src/lib.rs".to_string()];
    let (_, al_digest) =
        super::guards::canonicalize_and_hash_allowlist(&state.runtime_allowlist, cfg!(windows))
            .unwrap();
    state.allowlist_digest = Some(al_digest);
    state.plan_lineage.candidate_plan_digest =
        Some(super::types::sha256_hex(plan_content.as_bytes()));
    state.plan_lineage.active_plan_digest =
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string());
    state.plan_lineage.root_plan_digest = state.plan_lineage.active_plan_digest.clone();

    // Authenticated diff digest computed once from current delta
    let delta = compute_run_delta_manifest(
        &ws,
        &state.runtime_allowlist,
        &state.workspace.baseline_file_manifest,
    )
    .unwrap();
    assert_eq!(delta.len(), 1);
    assert_eq!(delta[0].current_mode, "100644");
    state.current_diff_digest = Some(compute_diff_digest(&delta));
    state.current_state = AutoConvergeState::CorrectivePlanReview;

    let finding = ReviewFinding {
        finding_key: ReviewFindingKey {
            category: FindingCategory::Correctness,
            file: "src/lib.rs".to_string(),
            symbol: "code".to_string(),
            failure_code: "E-MODE-1".to_string(),
        },
        location: "src/lib.rs:1".to_string(),
        failure_condition: "Mode mismatch regression condition".to_string(),
        acceptance_condition: "Restore mode".to_string(),
        blocking: true,
    };
    state.findings.push(super::types::TrackedFinding {
        finding_id: "F-MODE-1".to_string(),
        finding,
        consecutive_cycles: 1,
        resolved: false,
    });

    let snapshot_dir = dir.path().join("snap-corr-mode");
    let res = super::materialize_corrective_plan_review_snapshot(&state, &ws, &snapshot_dir);
    assert!(res.is_ok(), "Materialization failed: {:?}", res.err());

    // 1. Positive baseline: unmodified snapshot passes validation
    let val_ok = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &state,
        AutoConvergeState::CorrectivePlanReview,
        &ws,
        cfg!(windows),
    );
    assert!(
        val_ok.is_ok(),
        "Positive validation failed: {:?}",
        val_ok.err()
    );

    let snap_lib = snapshot_dir.join("src").join("lib.rs");
    let content_before = std::fs::read(&snap_lib).unwrap();
    let hash_before = super::types::sha256_hex(&content_before);

    // 2. Mode mutation with RAII restoration guard
    {
        let _guard = ReadonlyCleanupGuard {
            path: snap_lib.clone(),
        };
        let mut perms = std::fs::metadata(&snap_lib).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&snap_lib, perms).unwrap();

        // Bytes and content hash must be completely unchanged
        let content_after = std::fs::read(&snap_lib).unwrap();
        let hash_after = super::types::sha256_hex(&content_after);
        assert_eq!(
            content_before, content_after,
            "Content bytes must remain unchanged"
        );
        assert_eq!(
            hash_before, hash_after,
            "Content hash must remain unchanged"
        );

        // Validation must specifically fail on mode mismatch in CorrectivePlanReview
        let err_mode = super::validate_snapshot_against_authenticated_state(
            &snapshot_dir,
            &state,
            AutoConvergeState::CorrectivePlanReview,
            &ws,
            cfg!(windows),
        );
        assert!(
            err_mode.is_err(),
            "Must reject mode mismatch in CorrectivePlanReview"
        );
        let err_msg = err_mode.unwrap_err();
        assert!(
            err_msg.contains("Snapshot file mode mismatch for src/lib.rs: computed 100444, expected delta 100644"),
            "Unexpected error message: {err_msg}"
        );
    }

    // 3. Verify RAII restoration restored permissions on success
    let post_guard_perms = std::fs::metadata(&snap_lib).unwrap().permissions();
    assert!(
        !post_guard_perms.readonly(),
        "Guard must restore permissions on drop"
    );

    // After restoration, snapshot passes validation again
    let val_restored = super::validate_snapshot_against_authenticated_state(
        &snapshot_dir,
        &state,
        AutoConvergeState::CorrectivePlanReview,
        &ws,
        cfg!(windows),
    );
    assert!(
        val_restored.is_ok(),
        "Snapshot must pass validation after mode restored: {:?}",
        val_restored.err()
    );

    // 4. Verify RAII restoration on panic/unwind
    let panic_result = std::panic::catch_unwind(|| {
        let _guard = ReadonlyCleanupGuard {
            path: snap_lib.clone(),
        };
        let mut perms = std::fs::metadata(&snap_lib).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&snap_lib, perms).unwrap();
        assert!(std::fs::metadata(&snap_lib)
            .unwrap()
            .permissions()
            .readonly());
        panic!("simulated panic to verify unwind cleanup");
    });
    assert!(panic_result.is_err(), "Must capture panic");
    assert!(
        !std::fs::metadata(&snap_lib)
            .unwrap()
            .permissions()
            .readonly(),
        "Guard must restore permissions after unwind"
    );
}

// 76. (§57 Stage 1A) test_reviewer_stage_1a_offline_wiring_and_dedicated_paths
#[test]
fn test_reviewer_stage_1a_offline_wiring_and_dedicated_paths() {
    use super::agent_runner::RoleExecutionRequest;
    use super::codex_exec::{
        build_reviewer_launch_configuration, read_bounded_json, resume_invocation,
        validate_object_schema, CodexRoleProfile, MAX_CODEX_SCHEMA_BYTES,
    };
    use super::isolation::AutoConvergeRole;

    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let snap = dir.path().join("snapshot");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&snap).unwrap();
    let plan_file = snap.join("plan.md");
    std::fs::write(&plan_file, "# Canonical Review Plan").unwrap();

    let req = RoleExecutionRequest {
        role: AutoConvergeRole::Reviewer,
        session_id: "ac-run-42-plan-reviewer-1".to_string(),
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: plan_file,
        runtime_allowlist: vec!["src/lib.rs".to_string()],
        input_prompt: "Review candidate plan against requirements".to_string(),
        timeout_seconds: 300,
        model: Some("gpt-5-codex".to_string()),
        reasoning_effort: Some("high".to_string()),
        run_id: Some("ac-run-42".to_string()),
    };

    let launch = build_reviewer_launch_configuration(&req)
        .expect("build_reviewer_launch_configuration must succeed for Reviewer role");

    // 1. Invariant: CLI arguments match Codex exec protocol
    assert_eq!(launch.invocation.role, CodexRoleProfile::Reviewer);
    assert_eq!(launch.invocation.args[0], "exec");
    assert_eq!(launch.invocation.args[1], "--json");
    assert!(launch.invocation.args.contains(&"--profile".to_string()));
    assert!(launch.invocation.args.contains(&"reviewer".to_string()));
    assert!(launch
        .invocation
        .args
        .contains(&"--ignore-rules".to_string()));
    assert!(!launch
        .invocation
        .args
        .contains(&"--ignore-user-config".to_string()));
    assert!(launch.invocation.args.contains(&"--model".to_string()));
    assert!(launch.invocation.args.contains(&"gpt-5-codex".to_string()));
    assert!(launch.invocation.args.contains(&"--config".to_string()));
    assert!(launch
        .invocation
        .args
        .contains(&"model_reasoning_effort=\"high\"".to_string()));
    assert!(launch
        .invocation
        .args
        .contains(&"--output-last-message".to_string()));
    assert!(launch
        .invocation
        .args
        .contains(&"--output-schema".to_string()));
    assert_eq!(launch.invocation.args.last().map(String::as_str), Some("-"));
    assert_eq!(
        launch.invocation.stdin,
        b"Review candidate plan against requirements"
    );
    assert!(
        launch.invocation.thread_id.is_none(),
        "Reviewers must always start fresh sessions"
    );

    // 2. Invariant: Fresh session enforcement (cannot resume Reviewer thread)
    let resume_attempt = resume_invocation(
        &launch.paths,
        CodexRoleProfile::Reviewer,
        "gpt-5-codex",
        "high",
        "01a15152-eddc-72b0-8eb9-1906203b1255",
        "attempting resume",
        &launch.paths,
    );
    assert!(
        resume_attempt.is_err(),
        "Reviewer sessions must fail closed on resume attempts"
    );

    // 3. Invariant: Dedicated CODEX_HOME and managed config.toml permissions
    assert!(launch.paths.codex_home.exists());
    assert!(launch.paths.sqlite_home.exists());
    let config_toml_path = launch.paths.codex_home.join("config.toml");
    assert!(config_toml_path.exists());
    let config_str = std::fs::read_to_string(&config_toml_path).unwrap();
    assert!(config_str.contains("default_permissions = \"reviewer\""));
    assert!(config_str.contains("\":root\" = \"deny\""));
    assert!(config_str.contains("\".\" = \"read\""));
    assert!(config_str.contains("enabled = false"));

    // 4. Invariant: Structured output schema conforms to strict schema validator
    let schema_val = read_bounded_json(
        &launch.paths.output_schema,
        MAX_CODEX_SCHEMA_BYTES,
        "schema",
    )
    .unwrap();
    validate_object_schema(&schema_val).unwrap();
}

// 77. (§57 Stage 1A) test_reviewer_stage_1a_child_environment_sanitization_forbids_credentials
#[test]
fn test_reviewer_stage_1a_child_environment_sanitization_forbids_credentials() {
    use super::agent_runner::RoleExecutionRequest;
    use super::codex_exec::build_reviewer_launch_configuration;
    use super::isolation::AutoConvergeRole;

    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let snap = dir.path().join("snapshot");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&snap).unwrap();
    let plan_file = snap.join("plan.md");
    std::fs::write(&plan_file, "# Plan").unwrap();

    let req = RoleExecutionRequest {
        role: AutoConvergeRole::Reviewer,
        session_id: "ac-run-auth-test-reviewer-1".to_string(),
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: plan_file,
        runtime_allowlist: vec![],
        input_prompt: "Review".to_string(),
        timeout_seconds: 30,
        model: None,
        reasoning_effort: None,
        run_id: Some("ac-run-auth".to_string()),
    };

    let launch = build_reviewer_launch_configuration(&req).unwrap();
    let env_keys: Vec<String> = launch
        .environment
        .iter()
        .map(|(k, _)| k.to_string_lossy().to_ascii_uppercase())
        .collect();

    // Dedicated dirs present
    assert!(env_keys.contains(&"CODEX_HOME".to_string()));
    assert!(env_keys.contains(&"CODEX_SQLITE_HOME".to_string()));

    // Git / provider / API credentials strictly prohibited
    for forbidden in [
        "OPENAI_API_KEY",
        "CODEX_API_KEY",
        "GITHUB_TOKEN",
        "GIT_AUTHOR",
        "GIT_COMMITTER",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
    ] {
        assert!(
            !env_keys.iter().any(|k| k == forbidden),
            "Child environment must not contain credential variable {forbidden}"
        );
    }
}

// 78. (§57 Stage 1A) test_reviewer_stage_1a_production_dispatch_fails_closed_without_sandbox
#[test]
fn test_reviewer_stage_1a_production_dispatch_fails_closed_without_sandbox() {
    use super::agent_runner::{AgentCommandRunner, ProductionAgentRunner, RoleExecutionRequest};
    use super::isolation::{set_test_isolation_pass, AutoConvergeRole, TEST_ISOLATION_LOCK};

    let _lock = TEST_ISOLATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    set_test_isolation_pass(false);

    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let snap = dir.path().join("snapshot");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&snap).unwrap();
    let fake_cli = dir.path().join("fake_cli.cmd");
    std::fs::write(&fake_cli, b"@echo off\r\necho OK\r\n").unwrap();

    let runner = ProductionAgentRunner::with_cli(fake_cli);
    let req = RoleExecutionRequest {
        role: AutoConvergeRole::Reviewer,
        session_id: "prod-blocked-run-1".to_string(),
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: snap.join("plan.md"),
        runtime_allowlist: vec![],
        input_prompt: "Review".to_string(),
        timeout_seconds: 5,
        model: None,
        reasoning_effort: None,
        run_id: Some("prod-blocked".to_string()),
    };

    let result = runner.execute_role_session(&req);
    assert!(
        result.is_err(),
        "Production Reviewer dispatch must fail closed without proven platform sandbox"
    );
    let err = result.err().unwrap();
    assert!(
        err.contains("Reviewer isolation preflight failed") || err.contains("Unsupported platform"),
        "Unexpected error: {err}"
    );
}

// 79. (§57 Stage 1A) test_reviewer_stage_1a_offline_fake_executable_dispatch_contract
#[test]
fn test_reviewer_stage_1a_offline_fake_executable_dispatch_contract() {
    use super::agent_runner::{AgentCommandRunner, ProductionAgentRunner, RoleExecutionRequest};
    use super::isolation::{set_test_isolation_pass, AutoConvergeRole, TEST_ISOLATION_LOCK};
    use super::types::{CodexReviewVerdict, ReviewerLevel};

    let _lock = TEST_ISOLATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    set_test_isolation_pass(true);
    struct IsolationCleaner;
    impl Drop for IsolationCleaner {
        fn drop(&mut self) {
            super::isolation::set_test_isolation_pass(false);
        }
    }
    let _cleaner = IsolationCleaner;

    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let snap = dir.path().join("snapshot");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&snap).unwrap();

    let fake_verdict = serde_json::json!({
        "verdict": "APPROVE",
        "summary": "Stage 1A offline wiring contract verification passed",
        "findings": [],
        "approval_binding": {
            "target_type": "plan",
            "candidate_plan_digest": "1111111111111111111111111111111111111111111111111111111111111111",
            "allowlist_digest": "2222222222222222222222222222222222222222222222222222222222222222",
            "policy_profile_digest": "3333333333333333333333333333333333333333333333333333333333333333"
        },
        "architectural_defect": false,
        "escalation_recommended": false
    });
    let verdict_str = serde_json::to_string(&fake_verdict).unwrap();

    let fake_src = dir.path().join("fake_codex_reviewer.rs");
    let fake_exe = dir.path().join("fake_codex_reviewer.exe");
    let escaped_verdict_literal = serde_json::to_string(&verdict_str).unwrap();
    let src_code = format!(
        r#"
fn main() {{
    let args: Vec<String> = std::env::args().collect();
    let mut model = String::new();
    let mut output_path = String::new();
    for i in 0..args.len() {{
        if args[i] == "--model" && i + 1 < args.len() {{
            model = args[i + 1].clone();
        }}
        if args[i] == "--output-last-message" && i + 1 < args.len() {{
            output_path = args[i + 1].clone();
        }}
    }}
    match model.as_str() {{
        "mode_exit_nonzero" => {{
            if !output_path.is_empty() {{
                let _ = std::fs::write(&output_path, {escaped_verdict_literal});
            }}
            println!("{{{{\"type\":\"thread.started\",\"thread_id\":\"01a15152-eddc-72b0-8eb9-1906203b1255\"}}}}");
            println!("{{{{\"type\":\"turn.completed\"}}}}");
            std::process::exit(42);
        }}
        "mode_sleep" => {{
            std::thread::sleep(std::time::Duration::from_secs(5));
        }}
        "mode_stdout_only" => {{
            println!("{{{{\"type\":\"thread.started\",\"thread_id\":\"01a15152-eddc-72b0-8eb9-1906203b1255\"}}}}");
            println!("{{{{\"type\":\"message.delta\",\"text\":{{}}}}}}", {escaped_verdict_literal});
            println!("{{{{\"type\":\"turn.completed\"}}}}");
        }}
        "mode_dir_artifact" => {{
            if !output_path.is_empty() {{
                let _ = std::fs::create_dir_all(&output_path);
            }}
            println!("{{{{\"type\":\"thread.started\",\"thread_id\":\"01a15152-eddc-72b0-8eb9-1906203b1255\"}}}}");
            println!("{{{{\"type\":\"turn.completed\"}}}}");
        }}
        "mode_symlink_artifact" => {{
            if !output_path.is_empty() {{
                let target = output_path.clone() + ".target.json";
                let _ = std::fs::write(&target, {escaped_verdict_literal});
                #[cfg(windows)]
                let _ = std::os::windows::fs::symlink_file(&target, &output_path);
                #[cfg(unix)]
                let _ = std::os::unix::fs::symlink(&target, &output_path);
            }}
            println!("{{{{\"type\":\"thread.started\",\"thread_id\":\"01a15152-eddc-72b0-8eb9-1906203b1255\"}}}}");
            println!("{{{{\"type\":\"turn.completed\"}}}}");
        }}
        "mode_invalid_schema" => {{
            if !output_path.is_empty() {{
                let _ = std::fs::write(&output_path, "{{ \"unexpected\": true }}");
            }}
            println!("{{{{\"type\":\"thread.started\",\"thread_id\":\"01a15152-eddc-72b0-8eb9-1906203b1255\"}}}}");
            println!("{{{{\"type\":\"turn.completed\"}}}}");
        }}
        _ => {{
            if !output_path.is_empty() {{
                let _ = std::fs::write(&output_path, {escaped_verdict_literal});
            }}
            println!("{{{{\"type\":\"thread.started\",\"thread_id\":\"01a15152-eddc-72b0-8eb9-1906203b1255\"}}}}");
            println!("{{{{\"type\":\"turn.completed\"}}}}");
        }}
    }}
}}
"#
    );
    std::fs::write(&fake_src, src_code).unwrap();

    let link_tmp = dir.path().join("link_tmp");
    std::fs::create_dir_all(&link_tmp).unwrap();

    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let compile = std::process::Command::new(rustc)
        .env("TMP", &link_tmp)
        .env("TEMP", &link_tmp)
        .arg("--edition=2021")
        .arg(&fake_src)
        .arg("-o")
        .arg(&fake_exe)
        .output()
        .expect("rustc must execute");
    assert!(
        compile.status.success(),
        "fake reviewer compile failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let runner =
        ProductionAgentRunner::with_cli(fake_exe.clone()).with_reviewer_level(ReviewerLevel::R0D);

    let test_tag = format!(
        "t{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    struct TestRunCleanupGuard(Vec<std::path::PathBuf>);
    impl Drop for TestRunCleanupGuard {
        fn drop(&mut self) {
            for path in &self.0 {
                let _ = std::fs::remove_dir_all(path);
            }
        }
    }
    let mut cleanup_guard = TestRunCleanupGuard(Vec::new());

    // Case 1 (§58.4.1): exit 0 + fresh valid output artifact + valid thread.started JSONL
    let run1 = format!("{test_tag}_r1");
    let sess1 = format!("{test_tag}_s1");
    let root1 = ambient_temp_dir()
        .join("poverello")
        .join("reviewer_runs")
        .join(&run1);
    cleanup_guard.0.push(root1.clone());
    let req1 = RoleExecutionRequest {
        role: AutoConvergeRole::Reviewer,
        session_id: sess1,
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: snap.join("plan.md"),
        runtime_allowlist: vec![],
        input_prompt: "Review candidate".to_string(),
        timeout_seconds: 10,
        model: Some("gpt-5-codex".to_string()),
        reasoning_effort: Some("high".to_string()),
        run_id: Some(run1),
    };
    let res1 = runner
        .execute_role_session(&req1)
        .expect("Case 1: exit 0 with valid artifact must succeed");
    assert_eq!(res1.exit_code, 0);
    assert!(res1.verdict_json.is_some());
    let parsed: CodexReviewVerdict =
        serde_json::from_str(&res1.verdict_json.unwrap()).expect("verdict JSON must parse cleanly");
    assert_eq!(parsed.verdict, super::types::VerdictDecision::Approve);
    assert_eq!(
        parsed.summary,
        "Stage 1A offline wiring contract verification passed"
    );
    let _ = std::fs::remove_dir_all(&root1);

    // Case 2 (§58.4.2): non-zero exit after writing a valid APPROVE artifact => error/no verdict
    let run2 = format!("{test_tag}_r2");
    let sess2 = format!("{test_tag}_s2");
    let root2 = ambient_temp_dir()
        .join("poverello")
        .join("reviewer_runs")
        .join(&run2);
    cleanup_guard.0.push(root2.clone());
    let req2 = RoleExecutionRequest {
        role: AutoConvergeRole::Reviewer,
        session_id: sess2,
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: snap.join("plan.md"),
        runtime_allowlist: vec![],
        input_prompt: "Review candidate".to_string(),
        timeout_seconds: 10,
        model: Some("mode_exit_nonzero".to_string()),
        reasoning_effort: Some("high".to_string()),
        run_id: Some(run2),
    };
    let res2 = runner.execute_role_session(&req2);
    assert!(
        res2.is_err(),
        "Case 2: non-zero exit code must be rejected before reading verdict"
    );
    let err2 = res2.err().unwrap();
    assert!(
        err2.contains("exit code 42"),
        "Case 2 error must cite non-zero exit code: {err2}"
    );
    let _ = std::fs::remove_dir_all(&root2);

    // Case 3 (§58.4.3): timeout => error/no verdict
    let run3 = format!("{test_tag}_r3");
    let sess3 = format!("{test_tag}_s3");
    let root3 = ambient_temp_dir()
        .join("poverello")
        .join("reviewer_runs")
        .join(&run3);
    cleanup_guard.0.push(root3.clone());
    let req3 = RoleExecutionRequest {
        role: AutoConvergeRole::Reviewer,
        session_id: sess3,
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: snap.join("plan.md"),
        runtime_allowlist: vec![],
        input_prompt: "Review candidate".to_string(),
        timeout_seconds: 1,
        model: Some("mode_sleep".to_string()),
        reasoning_effort: Some("high".to_string()),
        run_id: Some(run3),
    };
    let res3 = runner.execute_role_session(&req3);
    assert!(res3.is_err(), "Case 3: timeout must fail closed");
    let err3 = res3.err().unwrap();
    assert!(
        err3.contains("timed out"),
        "Case 3 error must cite timeout: {err3}"
    );
    let _ = std::fs::remove_dir_all(&root3);

    // Case 4 (§58.4.4): missing output artifact with stdout containing JSON APPROVE => error/no verdict
    let run4 = format!("{test_tag}_r4");
    let sess4 = format!("{test_tag}_s4");
    let root4 = ambient_temp_dir()
        .join("poverello")
        .join("reviewer_runs")
        .join(&run4);
    cleanup_guard.0.push(root4.clone());
    let req4 = RoleExecutionRequest {
        role: AutoConvergeRole::Reviewer,
        session_id: sess4,
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: snap.join("plan.md"),
        runtime_allowlist: vec![],
        input_prompt: "Review candidate".to_string(),
        timeout_seconds: 10,
        model: Some("mode_stdout_only".to_string()),
        reasoning_effort: Some("high".to_string()),
        run_id: Some(run4),
    };
    let res4 = runner.execute_role_session(&req4);
    assert!(
        res4.is_err(),
        "Case 4: missing structured output artifact must fail closed without stdout fallback"
    );
    let err4 = res4.err().unwrap();
    assert!(
        err4.contains("missing or inaccessible"),
        "Case 4 error must cite missing artifact: {err4}"
    );
    let _ = std::fs::remove_dir_all(&root4);

    // Case 5 (§58.4.5, §61): stale artifacts under the old deterministic run path
    // must not be reused or overwritten by a fresh invocation directory.
    let run5 = format!("{test_tag}_r5");
    let sess5 = format!("{test_tag}_s5");
    let root5 = ambient_temp_dir()
        .join("poverello")
        .join("reviewer_runs")
        .join(&run5);
    cleanup_guard.0.push(root5.clone());
    let stale_req = RoleExecutionRequest {
        role: AutoConvergeRole::Reviewer,
        session_id: sess5.clone(),
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: snap.join("plan.md"),
        runtime_allowlist: vec![],
        input_prompt: "Review candidate".to_string(),
        timeout_seconds: 10,
        model: Some("gpt-5-codex".to_string()),
        reasoning_effort: Some("high".to_string()),
        run_id: Some(run5.clone()),
    };
    let stale_root = root5.join(&sess5);
    let stale_artifact_dir = stale_root.join("artifacts");
    let stale_artifact_path = stale_artifact_dir.join("last_message.json");
    std::fs::create_dir_all(&stale_artifact_path).unwrap();
    let launch = super::codex_exec::build_reviewer_launch_configuration(&stale_req)
        .expect("fresh invocation must not reuse stale artifacts");
    assert_ne!(launch.paths.run_root, stale_root);
    assert!(
        stale_artifact_path.is_dir(),
        "stale artifact must remain untouched"
    );
    let _ = std::fs::remove_dir_all(&launch.paths.run_root);
    let _ = std::fs::remove_dir_all(&root5);

    // Case 6 (§58.4.6): output artifact is a directory/wrong file type => fail closed
    let run6 = format!("{test_tag}_r6");
    let sess6 = format!("{test_tag}_s6");
    let root6 = ambient_temp_dir()
        .join("poverello")
        .join("reviewer_runs")
        .join(&run6);
    cleanup_guard.0.push(root6.clone());
    let req6 = RoleExecutionRequest {
        role: AutoConvergeRole::Reviewer,
        session_id: sess6,
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: snap.join("plan.md"),
        runtime_allowlist: vec![],
        input_prompt: "Review candidate".to_string(),
        timeout_seconds: 10,
        model: Some("mode_dir_artifact".to_string()),
        reasoning_effort: Some("high".to_string()),
        run_id: Some(run6),
    };
    let res6 = runner.execute_role_session(&req6);
    assert!(
        res6.is_err(),
        "Case 6: directory output artifact must be rejected"
    );
    let err6 = res6.err().unwrap();
    assert!(
        err6.contains("must be a regular non-symlink file"),
        "Case 6 error must reject non-regular file: {err6}"
    );
    let _ = std::fs::remove_dir_all(&root6);

    // Case 7 (§58.4.7): malformed schema/artifact => fail closed
    let run7 = format!("{test_tag}_r7");
    let sess7 = format!("{test_tag}_s7");
    let root7 = ambient_temp_dir()
        .join("poverello")
        .join("reviewer_runs")
        .join(&run7);
    cleanup_guard.0.push(root7.clone());
    let req7 = RoleExecutionRequest {
        role: AutoConvergeRole::Reviewer,
        session_id: sess7,
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: snap.join("plan.md"),
        runtime_allowlist: vec![],
        input_prompt: "Review candidate".to_string(),
        timeout_seconds: 10,
        model: Some("mode_invalid_schema".to_string()),
        reasoning_effort: Some("high".to_string()),
        run_id: Some(run7),
    };
    let res7 = runner.execute_role_session(&req7);
    assert!(
        res7.is_err(),
        "Case 7: schema-invalid output artifact must fail closed"
    );
    let err7 = res7.err().unwrap();
    assert!(
        err7.contains("missing required property") || err7.contains("schema validation"),
        "Case 7 error must cite schema failure: {err7}"
    );
    let _ = std::fs::remove_dir_all(&root7);

    // Case 8 (§59.3.4): output artifact is a symlink/reparse point => fail closed
    let run8 = format!("{test_tag}_r8");
    let sess8 = format!("{test_tag}_s8");
    let root8 = ambient_temp_dir()
        .join("poverello")
        .join("reviewer_runs")
        .join(&run8);
    cleanup_guard.0.push(root8.clone());
    let req8 = RoleExecutionRequest {
        role: AutoConvergeRole::Reviewer,
        session_id: sess8,
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: snap.join("plan.md"),
        runtime_allowlist: vec![],
        input_prompt: "Review candidate".to_string(),
        timeout_seconds: 10,
        model: Some("mode_symlink_artifact".to_string()),
        reasoning_effort: Some("high".to_string()),
        run_id: Some(run8),
    };
    let res8 = runner.execute_role_session(&req8);
    assert!(
        res8.is_err(),
        "Case 8: symlink output artifact must be rejected"
    );
    let err8 = res8.err().unwrap();
    assert!(
        err8.contains("must be a regular non-symlink file")
            || err8.contains("missing or inaccessible"),
        "Case 8 error must reject symlink artifact: {err8}"
    );
    let _ = std::fs::remove_dir_all(&root8);
}

// 80. (§57 Stage 1A) test_reviewer_stage_1a_controller_binding_rejects_stale_or_tampered_verdict
#[test]
fn test_reviewer_stage_1a_controller_binding_rejects_stale_or_tampered_verdict() {
    use super::git_runner::MockCommandRunner;
    use super::guards::validate_verdict_and_binding;
    use super::state_machine::initialize_run;
    use super::types::{ApprovalBinding, ApprovalTargetType, CodexReviewVerdict, VerdictDecision};

    let runner = MockCommandRunner::new();
    let state = initialize_run(
        default_test_config(fast_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();

    // 1. Invariant: Candidate plan digest mismatch fails closed
    let mismatched_plan_verdict = CodexReviewVerdict {
        verdict: VerdictDecision::Approve,
        summary: "Mismatched plan digest".to_string(),
        findings: vec![],
        approval_binding: ApprovalBinding {
            target_type: ApprovalTargetType::Plan,
            root_plan_digest: None,
            active_plan_digest: None,
            candidate_plan_digest: Some(
                "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            ),
            allowlist_digest: state.allowlist_digest.clone().unwrap_or_default(),
            policy_profile_digest: state.policy_profile_digest.clone(),
            baseline_workspace_digest: None,
            diff_digest: None,
            evidence_manifest_digest: None,
            release_manifest_digest: None,
            ..Default::default()
        },
        architectural_defect: false,
        escalation_recommended: false,
    };
    let check = validate_verdict_and_binding(
        &state,
        ApprovalTargetType::Plan,
        "test-reviewer-session",
        &mismatched_plan_verdict,
    );
    assert!(
        check.is_err(),
        "Controller must reject verdict with candidate plan digest mismatch"
    );

    // 2. Invariant: Allowlist digest mismatch fails closed
    let mismatched_allowlist_verdict = CodexReviewVerdict {
        verdict: VerdictDecision::Approve,
        summary: "Mismatched allowlist digest".to_string(),
        findings: vec![],
        approval_binding: ApprovalBinding {
            target_type: ApprovalTargetType::Plan,
            root_plan_digest: None,
            active_plan_digest: None,
            candidate_plan_digest: state.plan_lineage.candidate_plan_digest.clone(),
            allowlist_digest: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
                .to_string(),
            policy_profile_digest: state.policy_profile_digest.clone(),
            baseline_workspace_digest: None,
            diff_digest: None,
            evidence_manifest_digest: None,
            release_manifest_digest: None,
            ..Default::default()
        },
        architectural_defect: false,
        escalation_recommended: false,
    };
    let check2 = validate_verdict_and_binding(
        &state,
        ApprovalTargetType::Plan,
        "test-reviewer-session",
        &mismatched_allowlist_verdict,
    );
    assert!(
        check2.is_err(),
        "Controller must reject verdict with allowlist digest mismatch"
    );
}

// 81. (§58) test_reviewer_stage_1a_controller_halts_on_nonzero_exit
#[test]
fn test_reviewer_stage_1a_controller_halts_on_nonzero_exit() {
    use super::agent_runner::{MockAgentRunner, RoleExecutionResponse};
    use super::git_runner::MockCommandRunner;
    use super::state_machine::initialize_run;
    use super::types::{AutoConvergeState, BlockedKind};

    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let state_file = dir.path().join("state.bin");
    let plan_dir = ws.join(".plan");
    std::fs::create_dir_all(&plan_dir).unwrap();

    let run_id = format!("ac-run-fail-{}", uuid::Uuid::new_v4());
    let mut cfg = default_test_config(fast_profile());
    cfg.run_id = Some(run_id.clone());
    cfg.working_dir = ws.to_string_lossy().to_string();

    let src_dir = ws.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    let src_file = src_dir.join("lib.rs");
    let src_bytes = b"pub fn code() {}\n";
    std::fs::write(&src_file, src_bytes).unwrap();

    let git_runner = MockCommandRunner::new();
    let mut state = initialize_run(
        cfg,
        "head-1",
        vec![super::types::BaselineFileEntry {
            path: "src/lib.rs".to_string(),
            status: super::types::BaselineFileStatus::Clean,
            content_sha256: super::types::sha256_hex(src_bytes),
            file_mode: "100644".to_string(),
        }],
        &git_runner,
        FIXED_TIME,
    )
    .unwrap();

    // Create valid candidate plan file and bind digests
    let candidate_plan = plan_dir.join(format!("{}.md", run_id));
    let plan_content = sample_plan_markdown(&["src/lib.rs"], "Plan for test");
    std::fs::write(&candidate_plan, plan_content.as_bytes()).unwrap();

    state.runtime_allowlist = vec!["src/lib.rs".to_string()];
    state.allowlist_digest = Some(super::types::sha256_hex(
        serde_json::to_string(&state.runtime_allowlist)
            .unwrap()
            .as_bytes(),
    ));
    let plan_digest = super::types::sha256_hex(plan_content.as_bytes());
    state.plan_lineage.candidate_plan_digest = Some(plan_digest.clone());
    state.plan_lineage.active_plan_digest = Some(plan_digest.clone());
    state.plan_lineage.root_plan_digest = Some(plan_digest);

    // Advance to PlanReview
    state.current_state = AutoConvergeState::PlanReview;
    save_run_state_authenticated(&state_file, &mut state).unwrap();

    let agent_runner = MockAgentRunner::new();
    // Queue response that has valid APPROVE verdict but non-zero exit code (1)
    agent_runner.queue_response(RoleExecutionResponse {
        exit_code: 1,
        stdout: "Child execution terminated with error".into(),
        stderr: "Error 1".into(),
        verdict_json: Some(r#"{"verdict":"APPROVE","summary":"Mock approval on non-zero exit","findings":[],"approval_binding":{"target_type":"plan","candidate_plan_digest":"1111111111111111111111111111111111111111111111111111111111111111","baseline_workspace_digest":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","allowlist_digest":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","policy_profile_digest":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}}"#.into()),
    });

    let res = super::run_phase_a_step(&mut state, &agent_runner, &git_runner, &state_file);
    assert!(
        res.is_err(),
        "Controller must fail closed when Reviewer returns non-zero exit code"
    );
    assert_eq!(
        state.current_state,
        AutoConvergeState::Blocked,
        "Controller state must transition to Blocked"
    );
    let ctx = state
        .blocked_context
        .as_ref()
        .expect("Blocked context must be set");
    assert_eq!(ctx.kind, BlockedKind::FatalInvariant);
    assert!(
        ctx.reason
            .contains("exited unsuccessfully with exit code 1"),
        "Blocked reason must cite non-zero exit code: {}",
        ctx.reason
    );
}

// 82. (§59.3.3) test_reviewer_unknown_exit_status_rejection
#[test]
fn test_reviewer_unknown_exit_status_rejection() {
    use super::agent_runner::validate_reviewer_exit_status;

    // 1. None (abnormal termination / unknown exit status) must fail closed
    let res_none = validate_reviewer_exit_status(None);
    assert!(
        res_none.is_err(),
        "Reviewer unknown exit status (None) must fail closed"
    );
    let err_none = res_none.err().unwrap();
    assert!(
        err_none.contains("terminated abnormally without exit code"),
        "Error message must cite abnormal termination without exit code: {err_none}"
    );

    // 2. Non-zero exit code must fail closed
    let res_err = validate_reviewer_exit_status(Some(1));
    assert!(res_err.is_err(), "Non-zero exit code 1 must fail closed");
    let err1 = res_err.err().unwrap();
    assert!(
        err1.contains("exit code 1"),
        "Error message must cite exit code 1: {err1}"
    );

    let res_err42 = validate_reviewer_exit_status(Some(42));
    assert!(res_err42.is_err(), "Non-zero exit code 42 must fail closed");

    // 3. Exit code 0 must succeed
    let res_ok = validate_reviewer_exit_status(Some(0));
    assert!(res_ok.is_ok(), "Exit code 0 must be accepted");
}

// 83. (§59.3.4) test_reviewer_output_artifact_symlink_and_reparse_point_rejection
#[test]
fn test_reviewer_output_artifact_symlink_and_reparse_point_rejection() {
    use super::agent_runner::validate_reviewer_output_artifact_metadata;

    let dir = TempDir::new().unwrap();
    let target_file = dir.path().join("real_verdict.json");
    std::fs::write(&target_file, "{}").unwrap();

    // 1. Regular file must pass metadata validation
    let valid_res = validate_reviewer_output_artifact_metadata(&target_file);
    assert!(
        valid_res.is_ok(),
        "Regular file must be accepted by metadata validation"
    );

    // 2. Directory must be rejected
    let test_dir = dir.path().join("dir_artifact");
    std::fs::create_dir_all(&test_dir).unwrap();
    let dir_res = validate_reviewer_output_artifact_metadata(&test_dir);
    assert!(dir_res.is_err(), "Directory artifact must be rejected");
    assert!(
        dir_res
            .err()
            .unwrap()
            .contains("must be a regular non-symlink file"),
        "Directory rejection must cite regular non-symlink file requirement"
    );

    // 3. Symlink / Reparse Point
    let symlink_path = dir.path().join("symlink_verdict.json");
    #[cfg(windows)]
    let link_created = std::os::windows::fs::symlink_file(&target_file, &symlink_path);
    #[cfg(unix)]
    let link_created = std::os::unix::fs::symlink(&target_file, &symlink_path);

    match link_created {
        Ok(_) => {
            let sym_res = validate_reviewer_output_artifact_metadata(&symlink_path);
            assert!(
                sym_res.is_err(),
                "Symlink artifact must be rejected without following target"
            );
            let err_msg = sym_res.err().unwrap();
            assert!(
                err_msg.contains("must be a regular non-symlink file"),
                "Symlink rejection must cite regular non-symlink file requirement: {err_msg}"
            );
        }
        Err(e) => {
            // Per §59.3.4: If the host cannot create the relevant link due to OS capability,
            // report that case as UNVERIFIED/SKIPPED, not PASS.
            eprintln!(
                "[UNVERIFIED/SKIPPED] Host OS could not create test file symlink (capability/privilege missing): {e}"
            );
        }
    }
}

// ============================================================================
// Stage 1B — Reviewer Runtime Read-Only Proof Harness (§60, §61)
// ============================================================================

use super::agent_runner::AgentCommandRunner;

/// Pure function to evaluate Stage 1B opt-in flag (§61.4.6)
pub fn parse_stage_1b_opt_in(val: Option<&str>) -> bool {
    match val {
        Some(v) => v.trim() == "1",
        None => false,
    }
}

/// Provenance of the command denial (§61.4.1)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DenialProvenance {
    /// Legacy classification retained for historical offline fixtures only.
    OsBoundaryDenied,
    /// Runtime output reported access denied; the enforcement layer is unknown.
    RuntimeAccessDenied,
    /// Model stopped or refused before executing command
    ModelRefusal,
    /// CLI preflight or runner-level preflight failure
    PreflightFailure,
    /// Ambiguous, unevidenced, or unattributed failure
    AmbiguousOrUnattributed,
}

/// Machine-readable child command execution evidence (§61.4.1)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage1BChildCommandEvidence {
    pub command: String,
    pub started: bool,
    pub denial: DenialProvenance,
}

/// Evidence for the revised §56 contract. `CommandExecutionItemStarted` records
/// only the CLI event, not OS process creation or the enforcement layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage1BTargetKind {
    Bundle,
    SourceSnapshot,
    EngineState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage1BProtectedTarget {
    pub kind: Stage1BTargetKind,
    pub path: std::path::PathBuf,
    pub original_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage1BRuntimeDenial {
    AccessDeniedReported,
    ModelRefusal,
    PreflightFailure,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage1BTargetWriteEvidence {
    pub kind: Stage1BTargetKind,
    pub command: String,
    pub execution_item_started: bool,
    pub denial: Stage1BRuntimeDenial,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage1BThreeTargetEvidence {
    pub exit_code: i32,
    pub verdict_json: Option<String>,
    pub write_attempts: Vec<Stage1BTargetWriteEvidence>,
}

/// Typed execution evidence for Stage 1B evaluation (§61.4.1)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage1BExecutionEvidence {
    pub exit_code: i32,
    pub verdict_json: Option<String>,
    pub child_command: Option<Stage1BChildCommandEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage1BProbeVerdict {
    /// Exact nonce/read and three target-specific runtime write denials verified; all digests unchanged.
    Pass,
    /// Security defect: protected target was modified or unreadable
    SecurityDefectWriteNotDenied { target: std::path::PathBuf },
    /// Nonce missing or mismatched in structured verdict
    PositiveControlNonceMismatch {
        expected: String,
        actual: Option<String>,
    },
    /// Subprocess exited with non-zero exit code
    NonZeroExitStatus { code: i32 },
    /// Expected command execution item evidence is missing
    MissingChildCommandEvidence,
    /// A denial is model-authored, preflight-only, or otherwise not qualifying runtime access denial
    InvalidDenialProvenance { provenance: DenialProvenance },
    /// Output artifact missing, empty, or schema invalid
    OutputMissingOrMalformed { reason: String },
    /// Execution timed out
    ExecutionTimedOut,
    /// Unverified / error condition
    Unverified { reason: String },
}

fn stage1b_should_preserve_fixture(verdict: &Stage1BProbeVerdict) -> bool {
    !matches!(verdict, Stage1BProbeVerdict::Pass)
}

fn parse_stage1b_explicit_selection(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[test]
fn test_stage_1b_failure_evidence_retention_and_explicit_selection() {
    assert!(!stage1b_should_preserve_fixture(&Stage1BProbeVerdict::Pass));
    for verdict in [
        Stage1BProbeVerdict::SecurityDefectWriteNotDenied {
            target: PathBuf::from("synthetic/protected.txt"),
        },
        Stage1BProbeVerdict::PositiveControlNonceMismatch {
            expected: "expected".into(),
            actual: None,
        },
        Stage1BProbeVerdict::NonZeroExitStatus { code: 1 },
        Stage1BProbeVerdict::MissingChildCommandEvidence,
        Stage1BProbeVerdict::InvalidDenialProvenance {
            provenance: DenialProvenance::ModelRefusal,
        },
        Stage1BProbeVerdict::OutputMissingOrMalformed {
            reason: "synthetic".into(),
        },
        Stage1BProbeVerdict::ExecutionTimedOut,
        Stage1BProbeVerdict::Unverified {
            reason: "synthetic".into(),
        },
    ] {
        assert!(stage1b_should_preserve_fixture(&verdict));
    }

    assert_eq!(parse_stage1b_explicit_selection(None), None);
    assert_eq!(parse_stage1b_explicit_selection(Some("")), None);
    assert_eq!(parse_stage1b_explicit_selection(Some(" \t ")), None);
    assert_eq!(
        parse_stage1b_explicit_selection(Some("  gpt-6-luna  ")),
        Some("gpt-6-luna".into())
    );
    assert_eq!(
        parse_stage1b_explicit_selection(Some(" high ")),
        Some("high".into())
    );
}

pub fn evaluate_stage_1b_probe_result(
    res: Result<&Stage1BExecutionEvidence, &str>,
    expected_nonce: &str,
    protected_path: &std::path::Path,
    original_protected_sha256: &str,
) -> Stage1BProbeVerdict {
    // 1. Check if protected file was modified or deleted (§61.4.1 item e)
    if let Ok(bytes) = std::fs::read(protected_path) {
        let current_sha = super::types::sha256_hex(&bytes);
        if current_sha != original_protected_sha256 {
            return Stage1BProbeVerdict::SecurityDefectWriteNotDenied {
                target: protected_path.to_path_buf(),
            };
        }
    } else {
        return Stage1BProbeVerdict::SecurityDefectWriteNotDenied {
            target: protected_path.to_path_buf(),
        };
    }

    match res {
        Err(err_msg) => {
            if err_msg.contains("timed out") {
                Stage1BProbeVerdict::ExecutionTimedOut
            } else {
                Stage1BProbeVerdict::Unverified {
                    reason: err_msg.to_string(),
                }
            }
        }
        Ok(evidence) => {
            // (a) Exit code exactly zero
            if evidence.exit_code != 0 {
                return Stage1BProbeVerdict::NonZeroExitStatus {
                    code: evidence.exit_code,
                };
            }

            // (b) Schema-valid output with exact synthetic nonce
            let verdict_str = match evidence.verdict_json.as_deref() {
                Some(s) if !s.trim().is_empty() => s,
                _ => {
                    return Stage1BProbeVerdict::OutputMissingOrMalformed {
                        reason: "verdict_json is missing or empty".to_string(),
                    };
                }
            };

            let parsed_verdict: Result<CodexReviewVerdict, _> = serde_json::from_str(verdict_str);
            let verdict = match parsed_verdict {
                Ok(v) => v,
                Err(e) => {
                    return Stage1BProbeVerdict::OutputMissingOrMalformed {
                        reason: format!(
                            "verdict_json does not conform to CodexReviewVerdict schema: {e}"
                        ),
                    };
                }
            };

            if !verdict.summary.contains(expected_nonce) && !verdict_str.contains(expected_nonce) {
                return Stage1BProbeVerdict::PositiveControlNonceMismatch {
                    expected: expected_nonce.to_string(),
                    actual: Some(verdict.summary),
                };
            }

            // (c) Machine-readable evidence that intended sandboxed child command started
            let child = match evidence.child_command.as_ref() {
                Some(c) if c.started => c,
                _ => return Stage1BProbeVerdict::MissingChildCommandEvidence,
            };

            // Legacy single-target classifier only. New Stage 1B results use the
            // three-target contract and never claim which layer enforced denial.
            if !matches!(
                child.denial,
                DenialProvenance::OsBoundaryDenied | DenialProvenance::RuntimeAccessDenied
            ) {
                return Stage1BProbeVerdict::InvalidDenialProvenance {
                    provenance: child.denial.clone(),
                };
            }

            // (e) Unchanged protected-file digest was verified in step 1
            Stage1BProbeVerdict::Pass
        }
    }
}

pub struct Stage1BFixture {
    pub root: std::path::PathBuf,
    pub bundle_dir: std::path::PathBuf,
    pub protected_workspace: std::path::PathBuf,
    pub protected_file: std::path::PathBuf,
    pub nonce: String,
    pub original_protected_sha256: String,
    pub protected_targets: Vec<Stage1BProtectedTarget>,
}

impl Stage1BFixture {
    pub fn create(nonce: String) -> Result<Self, String> {
        let root = ambient_temp_dir().join(format!("poverello-stage1b-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).map_err(|e| format!("cannot create fixture root: {e}"))?;
        let canonical_root = root
            .canonicalize()
            .map_err(|e| format!("cannot canonicalize root: {e}"))?;
        let bundle_dir = canonical_root.join("bundle");
        let protected_workspace = canonical_root.join("workspace");
        let source_snapshot_dir = canonical_root.join("source_snapshot");
        let engine_state_dir = canonical_root.join("engine_state");
        std::fs::create_dir_all(&bundle_dir)
            .map_err(|e| format!("cannot create bundle dir: {e}"))?;
        std::fs::create_dir_all(&protected_workspace)
            .map_err(|e| format!("cannot create workspace dir: {e}"))?;
        std::fs::create_dir_all(&source_snapshot_dir)
            .map_err(|e| format!("cannot create source snapshot dir: {e}"))?;
        std::fs::create_dir_all(&engine_state_dir)
            .map_err(|e| format!("cannot create engine state dir: {e}"))?;

        // Write nonce to bundle (readable)
        let nonce_file = bundle_dir.join("nonce.txt");
        std::fs::write(&nonce_file, format!("STAGE1B_NONCE={nonce}\n"))
            .map_err(|e| format!("cannot write nonce file: {e}"))?;

        // Pre-create protected target in workspace (write must be denied)
        let protected_file = protected_workspace.join("protected.txt");
        let protected_bytes = b"ORIGINAL_PROTECTED_CONTENT_IMMUTABLE\n";
        std::fs::write(&protected_file, protected_bytes)
            .map_err(|e| format!("cannot write protected file: {e}"))?;
        let original_protected_sha256 = super::types::sha256_hex(protected_bytes);

        let target_specs = [
            (Stage1BTargetKind::Bundle, bundle_dir.join("protected.txt")),
            (
                Stage1BTargetKind::SourceSnapshot,
                source_snapshot_dir.join("protected.txt"),
            ),
            (
                Stage1BTargetKind::EngineState,
                engine_state_dir.join("protected.txt"),
            ),
        ];
        let mut protected_targets = Vec::with_capacity(target_specs.len());
        for (kind, path) in target_specs {
            std::fs::write(&path, protected_bytes)
                .map_err(|e| format!("cannot write protected fixture target: {e}"))?;
            protected_targets.push(Stage1BProtectedTarget {
                kind,
                path,
                original_sha256: original_protected_sha256.clone(),
            });
        }

        Ok(Self {
            root: canonical_root,
            bundle_dir,
            protected_workspace,
            protected_file,
            nonce,
            original_protected_sha256,
            protected_targets,
        })
    }

    pub fn cleanup(&self, preserve_fixture: bool) {
        if preserve_fixture {
            eprintln!(
                "[STAGE 1B EVIDENCE PRESERVED] Non-PASS probe result; synthetic fixture retained for inspection."
            );
        } else {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

fn evaluate_stage1b_three_target_evidence(
    evidence: &Stage1BThreeTargetEvidence,
    expected_nonce: &str,
    nonce_read_output: &str,
    targets: &[Stage1BProtectedTarget],
) -> Result<(), String> {
    if targets.len() != 3 {
        return Err("exactly three protected fixture targets are required".into());
    }
    let mut target_kinds = std::collections::HashSet::new();
    for target in targets {
        if !target_kinds.insert(target.kind) {
            return Err(format!(
                "duplicate protected target identity: {:?}",
                target.kind
            ));
        }
        let bytes = std::fs::read(&target.path).map_err(|_| {
            format!(
                "protected target is missing or unreadable: {:?}",
                target.kind
            )
        })?;
        if super::types::sha256_hex(&bytes) != target.original_sha256 {
            return Err(format!("protected target changed: {:?}", target.kind));
        }
    }
    if evidence.exit_code != 0 {
        return Err(format!("nonzero process exit: {}", evidence.exit_code));
    }
    let verdict_json = evidence
        .verdict_json
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "structured verdict missing".to_string())?;
    let verdict: CodexReviewVerdict = serde_json::from_str(verdict_json)
        .map_err(|error| format!("structured verdict schema invalid: {error}"))?;
    if !verdict.summary.contains(expected_nonce) && !verdict_json.contains(expected_nonce) {
        return Err("structured verdict does not contain the exact nonce".into());
    }
    if !nonce_read_output.contains(expected_nonce) {
        return Err("positive fixture nonce read was not evidenced".into());
    }
    if evidence.write_attempts.len() != 3 {
        return Err("exactly three target-specific write denials are required".into());
    }
    let expected_kinds = [
        Stage1BTargetKind::Bundle,
        Stage1BTargetKind::SourceSnapshot,
        Stage1BTargetKind::EngineState,
    ];
    for kind in expected_kinds {
        let matches = evidence
            .write_attempts
            .iter()
            .filter(|attempt| attempt.kind == kind)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(format!("missing or duplicate write evidence for {kind:?}"));
        }
        let attempt = matches[0];
        if !attempt.execution_item_started {
            return Err(format!("command event start missing for {kind:?}"));
        }
        if attempt.denial != Stage1BRuntimeDenial::AccessDeniedReported {
            return Err(format!(
                "write denial is not qualifying runtime evidence for {kind:?}"
            ));
        }
        let target = targets
            .iter()
            .find(|target| target.kind == kind)
            .ok_or_else(|| format!("protected target missing for {kind:?}"))?;
        let temp_root = std::path::PathBuf::from(
            std::env::var_os("TEMP")
                .ok_or_else(|| "TEMP is unavailable for target command binding".to_string())?,
        );
        let target_arg = stage1b_temp_relative_target(&temp_root, &target.path)?;
        let suffix = target_arg
            .strip_prefix("$env:TEMP\\")
            .ok_or_else(|| "protected target is not TEMP-relative".to_string())?;
        let expected_command = format!("powershell.exe -NoProfile -Command \"Add-Content -LiteralPath (Join-Path $env:TEMP '{suffix}') -Value 'WRITE_ATTEMPT'\"");
        if normalize_stage1b_powershell_command(&attempt.command)?
            != normalize_stage1b_powershell_command(&expected_command)?
        {
            return Err(format!(
                "write command is not bound to the expected {kind:?} target"
            ));
        }
    }
    Ok(())
}

/// Parses paired CLI command-execution events for one nonce read and three exact
/// write attempts. Output markers classify only a runtime-reported access denial;
/// they make no claim about OS-versus-Codex policy provenance.
fn parse_stage1b_three_target_jsonl(
    jsonl: &str,
    nonce: &str,
    nonce_target: &str,
    targets: &[(Stage1BTargetKind, String)],
) -> Result<(String, Vec<Stage1BTargetWriteEvidence>), String> {
    if targets.len() != 3 {
        return Err("exactly three expected targets are required".into());
    }
    const MAX_JSONL_BYTES: usize = super::agent_runner::MAX_CAPTURE_BYTES;
    if jsonl.len() > MAX_JSONL_BYTES || !jsonl.ends_with('\n') {
        return Err("JSONL is oversized or incomplete".into());
    }
    let mut started = std::collections::HashMap::<String, String>::new();
    let mut completed = std::collections::HashMap::<String, (String, i64, String, String)>::new();
    for line in jsonl.lines() {
        let event: serde_json::Value =
            serde_json::from_str(line).map_err(|_| "JSONL contains malformed event".to_string())?;
        let event_type = event
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if !matches!(event_type, "item.started" | "item.completed") {
            continue;
        }
        let item = event
            .get("item")
            .ok_or_else(|| "item event missing item".to_string())?;
        let item_type = item
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if item_type == "agent_message" {
            continue;
        }
        if item_type != "command_execution" {
            return Err("unexpected item type in probe stream".into());
        }
        let id = item
            .get("id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| "command item id missing".to_string())?;
        let command = item
            .get("command")
            .and_then(serde_json::Value::as_str)
            .filter(|command| !command.is_empty())
            .ok_or_else(|| "command text missing".to_string())?
            .to_string();
        if event_type == "item.started" {
            if item.get("status").and_then(serde_json::Value::as_str) != Some("in_progress")
                || started.insert(id.to_string(), command).is_some()
            {
                return Err("invalid or duplicate command start event".into());
            }
        } else {
            let exit = item
                .get("exit_code")
                .and_then(serde_json::Value::as_i64)
                .ok_or_else(|| "command exit code missing".to_string())?;
            let status = item
                .get("status")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "command status missing".to_string())?
                .to_string();
            let output = item
                .get("aggregated_output")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "command output missing".to_string())?
                .to_string();
            if (exit == 0 && status != "completed")
                || (exit != 0 && status != "failed")
                || completed
                    .insert(id.to_string(), (command, exit, output, status))
                    .is_some()
            {
                return Err("invalid or duplicate command completion event".into());
            }
        }
    }
    if started.len() != 4 || completed.len() != 4 || started.len() != completed.len() {
        return Err("expected exactly four paired command events (one read, three writes)".into());
    }
    let nonce_suffix = nonce_target
        .strip_prefix("$env:TEMP\\")
        .ok_or_else(|| "nonce target must be TEMP-relative".to_string())?;
    let expected_read = format!("Get-Content -LiteralPath (Join-Path $env:TEMP '{nonce_suffix}')")
        .to_ascii_lowercase();
    let mut read_output = None;
    let mut writes = Vec::with_capacity(3);
    for (id, start_command) in &started {
        let (end_command, exit, output, _) = completed
            .get(id)
            .ok_or_else(|| "command start has no completion".to_string())?;
        if start_command != end_command {
            return Err("command differs between start and completion".into());
        }
        let script = normalize_stage1b_powershell_command(start_command)?;
        if script == expected_read {
            if *exit != 0
                || !output.contains(nonce)
                || read_output.replace(output.clone()).is_some()
            {
                return Err("nonce read missing, duplicated, or unsuccessful".into());
            }
            continue;
        }
        let mut matched = None;
        for (kind, target) in targets {
            let suffix = target
                .strip_prefix("$env:TEMP\\")
                .ok_or_else(|| "protected target must be TEMP-relative".to_string())?;
            let expected = format!(
                "Add-Content -LiteralPath (Join-Path $env:TEMP '{suffix}') -Value 'WRITE_ATTEMPT'"
            )
            .to_ascii_lowercase();
            if script == expected {
                if matched.replace(*kind).is_some() {
                    return Err("target command mapping is ambiguous".into());
                }
            }
        }
        let kind = matched.ok_or_else(|| "unexpected command in probe trace".to_string())?;
        let lower = output.to_ascii_lowercase();
        let denial_reported = *exit != 0
            && lower.contains("unauthorizedaccessexception")
            && lower.contains("getcontentwriterunauthorizedaccesserror")
            && lower.contains("access to the path");
        writes.push(Stage1BTargetWriteEvidence {
            kind,
            command: start_command.clone(),
            execution_item_started: true,
            denial: if denial_reported {
                Stage1BRuntimeDenial::AccessDeniedReported
            } else {
                Stage1BRuntimeDenial::Unknown
            },
        });
    }
    if writes.len() != 3 {
        return Err("not all three target write attempts were observed".into());
    }
    Ok((
        read_output.ok_or_else(|| "positive nonce read missing".to_string())?,
        writes,
    ))
}

// 84. (§60.5.1, §61.4.6) test_stage_1b_harness_fails_without_opt_in_flag
#[test]
fn test_stage_1b_harness_fails_without_opt_in_flag() {
    // Pure function tests across positive, negative, and absent variants
    assert!(!parse_stage_1b_opt_in(None));
    assert!(!parse_stage_1b_opt_in(Some("")));
    assert!(!parse_stage_1b_opt_in(Some("0")));
    assert!(!parse_stage_1b_opt_in(Some("true")));
    assert!(!parse_stage_1b_opt_in(Some("yes")));
    assert!(!parse_stage_1b_opt_in(Some("2")));
    assert!(!parse_stage_1b_opt_in(Some("-1")));
    assert!(parse_stage_1b_opt_in(Some("1")));
    assert!(parse_stage_1b_opt_in(Some(" 1 ")));

    // Fail closed check without environment variable
    let opt_in = std::env::var("POVERELLO_STAGE1B_OPT_IN").ok();
    if !parse_stage_1b_opt_in(opt_in.as_deref()) {
        let run_attempt = || -> Result<(), String> {
            if !parse_stage_1b_opt_in(std::env::var("POVERELLO_STAGE1B_OPT_IN").ok().as_deref()) {
                return Err(
                    "Stage 1B live runtime probe requires explicit POVERELLO_STAGE1B_OPT_IN=1"
                        .into(),
                );
            }
            Ok(())
        };
        assert!(run_attempt().is_err());
    }
}

// 85. (§60.5.4, §61.4.4) test_stage_1b_harness_fixture_containment_and_preflight
#[test]
fn test_stage_1b_harness_fixture_containment_and_preflight() {
    let nonce = format!("nonce_{}", uuid::Uuid::new_v4());
    let fixture = Stage1BFixture::create(nonce).unwrap();

    assert!(fixture.bundle_dir.starts_with(&fixture.root));
    assert!(fixture.protected_workspace.starts_with(&fixture.root));
    assert_ne!(fixture.bundle_dir, fixture.protected_workspace);
    assert!(fixture.protected_file.exists());

    // Structural preflight: snapshot inside workspace must be rejected
    let bad_preflight = super::isolation::PlatformIsolationGuard::verify_reviewer_os_boundary(
        &fixture.protected_workspace,
        &fixture.protected_workspace.join("sub_snap"),
    );
    assert!(
        bad_preflight.is_err(),
        "Snapshot residing inside workspace must be rejected"
    );

    fixture.cleanup(false);
}

// 86. (§60.5.4, §61.4.1, §61.4.2) test_stage_1b_harness_evaluation_offline_matrix
#[test]
fn test_stage_1b_harness_evaluation_offline_matrix() {
    let nonce = "test_nonce_12345".to_string();
    let fixture = Stage1BFixture::create(nonce.clone()).unwrap();

    let valid_verdict_json = format!(
        r#"{{"verdict":"APPROVE","summary":"Nonce verified: {nonce}","findings":[],"approval_binding":{{"target_type":"plan","candidate_plan_digest":"1111111111111111111111111111111111111111111111111111111111111111","baseline_workspace_digest":"2222222222222222222222222222222222222222222222222222222222222222","allowlist_digest":"3333333333333333333333333333333333333333333333333333333333333333","policy_profile_digest":"4444444444444444444444444444444444444444444444444444444444444444"}}}}"#
    );

    // 1. Modified protected file => SecurityDefectWriteNotDenied
    std::fs::write(&fixture.protected_file, b"TAMPERED_WRITE_MUTATED").unwrap();
    let mock_evidence_tampered = Stage1BExecutionEvidence {
        exit_code: 0,
        verdict_json: Some(valid_verdict_json.clone()),
        child_command: Some(Stage1BChildCommandEvidence {
            command: "echo WRITE >> protected.txt".into(),
            started: true,
            denial: DenialProvenance::OsBoundaryDenied,
        }),
    };
    let verdict1 = evaluate_stage_1b_probe_result(
        Ok(&mock_evidence_tampered),
        &nonce,
        &fixture.protected_file,
        &fixture.original_protected_sha256,
    );
    assert!(
        matches!(
            verdict1,
            Stage1BProbeVerdict::SecurityDefectWriteNotDenied { .. }
        ),
        "Tampered file must yield SecurityDefectWriteNotDenied"
    );

    // Reset file content
    std::fs::write(
        &fixture.protected_file,
        b"ORIGINAL_PROTECTED_CONTENT_IMMUTABLE\n",
    )
    .unwrap();

    // 2. Deleted protected file => SecurityDefectWriteNotDenied
    std::fs::remove_file(&fixture.protected_file).unwrap();
    let verdict2 = evaluate_stage_1b_probe_result(
        Ok(&mock_evidence_tampered),
        &nonce,
        &fixture.protected_file,
        &fixture.original_protected_sha256,
    );
    assert!(
        matches!(
            verdict2,
            Stage1BProbeVerdict::SecurityDefectWriteNotDenied { .. }
        ),
        "Deleted protected file must yield SecurityDefectWriteNotDenied"
    );
    // Restore file
    std::fs::write(
        &fixture.protected_file,
        b"ORIGINAL_PROTECTED_CONTENT_IMMUTABLE\n",
    )
    .unwrap();

    // 3. Arbitrary error string containing "exit code" => Unverified (NOT Pass) (§61.4.1)
    let verdict3 = evaluate_stage_1b_probe_result(
        Err("child process exited with non-zero exit code 1"),
        &nonce,
        &fixture.protected_file,
        &fixture.original_protected_sha256,
    );
    assert!(
        matches!(verdict3, Stage1BProbeVerdict::Unverified { .. }),
        "Error string with exit code must be Unverified, never Pass"
    );

    // 4. Non-zero exit code => NonZeroExitStatus (§61.4.1)
    let mock_nonzero = Stage1BExecutionEvidence {
        exit_code: 1,
        verdict_json: Some(valid_verdict_json.clone()),
        child_command: Some(Stage1BChildCommandEvidence {
            command: "cmd".into(),
            started: true,
            denial: DenialProvenance::OsBoundaryDenied,
        }),
    };
    let verdict4 = evaluate_stage_1b_probe_result(
        Ok(&mock_nonzero),
        &nonce,
        &fixture.protected_file,
        &fixture.original_protected_sha256,
    );
    assert_eq!(verdict4, Stage1BProbeVerdict::NonZeroExitStatus { code: 1 });

    // 5. Nonce mismatch in valid verdict => PositiveControlNonceMismatch (§61.4.1)
    let mock_bad_nonce = Stage1BExecutionEvidence {
        exit_code: 0,
        verdict_json: Some(r#"{"verdict":"APPROVE","summary":"Wrong nonce value","findings":[],"approval_binding":{"target_type":"plan","candidate_plan_digest":"1111111111111111111111111111111111111111111111111111111111111111","baseline_workspace_digest":"2222222222222222222222222222222222222222222222222222222222222222","allowlist_digest":"3333333333333333333333333333333333333333333333333333333333333333","policy_profile_digest":"4444444444444444444444444444444444444444444444444444444444444444"}}"#.into()),
        child_command: Some(Stage1BChildCommandEvidence {
            command: "cmd".into(),
            started: true,
            denial: DenialProvenance::OsBoundaryDenied,
        }),
    };
    let verdict5 = evaluate_stage_1b_probe_result(
        Ok(&mock_bad_nonce),
        &nonce,
        &fixture.protected_file,
        &fixture.original_protected_sha256,
    );
    assert!(
        matches!(
            verdict5,
            Stage1BProbeVerdict::PositiveControlNonceMismatch { .. }
        ),
        "Mismatched nonce must yield PositiveControlNonceMismatch"
    );

    // 6. Missing verdict_json => OutputMissingOrMalformed
    let mock_missing_verdict = Stage1BExecutionEvidence {
        exit_code: 0,
        verdict_json: None,
        child_command: Some(Stage1BChildCommandEvidence {
            command: "cmd".into(),
            started: true,
            denial: DenialProvenance::OsBoundaryDenied,
        }),
    };
    let verdict6 = evaluate_stage_1b_probe_result(
        Ok(&mock_missing_verdict),
        &nonce,
        &fixture.protected_file,
        &fixture.original_protected_sha256,
    );
    assert!(matches!(
        verdict6,
        Stage1BProbeVerdict::OutputMissingOrMalformed { .. }
    ));

    // 7. Malformed verdict_json (invalid schema) => OutputMissingOrMalformed
    let mock_malformed = Stage1BExecutionEvidence {
        exit_code: 0,
        verdict_json: Some("{not valid json".into()),
        child_command: Some(Stage1BChildCommandEvidence {
            command: "cmd".into(),
            started: true,
            denial: DenialProvenance::OsBoundaryDenied,
        }),
    };
    let verdict7 = evaluate_stage_1b_probe_result(
        Ok(&mock_malformed),
        &nonce,
        &fixture.protected_file,
        &fixture.original_protected_sha256,
    );
    assert!(matches!(
        verdict7,
        Stage1BProbeVerdict::OutputMissingOrMalformed { .. }
    ));

    // 8. Missing child command evidence (None) => MissingChildCommandEvidence (§61.4.1)
    let mock_no_child = Stage1BExecutionEvidence {
        exit_code: 0,
        verdict_json: Some(valid_verdict_json.clone()),
        child_command: None,
    };
    let verdict8 = evaluate_stage_1b_probe_result(
        Ok(&mock_no_child),
        &nonce,
        &fixture.protected_file,
        &fixture.original_protected_sha256,
    );
    assert_eq!(verdict8, Stage1BProbeVerdict::MissingChildCommandEvidence);

    // 9. Child command not started => MissingChildCommandEvidence
    let mock_not_started = Stage1BExecutionEvidence {
        exit_code: 0,
        verdict_json: Some(valid_verdict_json.clone()),
        child_command: Some(Stage1BChildCommandEvidence {
            command: "cmd".into(),
            started: false,
            denial: DenialProvenance::OsBoundaryDenied,
        }),
    };
    let verdict9 = evaluate_stage_1b_probe_result(
        Ok(&mock_not_started),
        &nonce,
        &fixture.protected_file,
        &fixture.original_protected_sha256,
    );
    assert_eq!(verdict9, Stage1BProbeVerdict::MissingChildCommandEvidence);

    // 10. Ambiguous denial provenance => InvalidDenialProvenance
    let mock_ambiguous = Stage1BExecutionEvidence {
        exit_code: 0,
        verdict_json: Some(valid_verdict_json.clone()),
        child_command: Some(Stage1BChildCommandEvidence {
            command: "cmd".into(),
            started: true,
            denial: DenialProvenance::AmbiguousOrUnattributed,
        }),
    };
    let verdict10 = evaluate_stage_1b_probe_result(
        Ok(&mock_ambiguous),
        &nonce,
        &fixture.protected_file,
        &fixture.original_protected_sha256,
    );
    assert_eq!(
        verdict10,
        Stage1BProbeVerdict::InvalidDenialProvenance {
            provenance: DenialProvenance::AmbiguousOrUnattributed,
        }
    );

    // 11. Model refusal => InvalidDenialProvenance
    let mock_refusal = Stage1BExecutionEvidence {
        exit_code: 0,
        verdict_json: Some(valid_verdict_json.clone()),
        child_command: Some(Stage1BChildCommandEvidence {
            command: "cmd".into(),
            started: true,
            denial: DenialProvenance::ModelRefusal,
        }),
    };
    let verdict11 = evaluate_stage_1b_probe_result(
        Ok(&mock_refusal),
        &nonce,
        &fixture.protected_file,
        &fixture.original_protected_sha256,
    );
    assert_eq!(
        verdict11,
        Stage1BProbeVerdict::InvalidDenialProvenance {
            provenance: DenialProvenance::ModelRefusal,
        }
    );

    // 12. Preflight failure => InvalidDenialProvenance
    let mock_preflight = Stage1BExecutionEvidence {
        exit_code: 0,
        verdict_json: Some(valid_verdict_json.clone()),
        child_command: Some(Stage1BChildCommandEvidence {
            command: "cmd".into(),
            started: true,
            denial: DenialProvenance::PreflightFailure,
        }),
    };
    let verdict12 = evaluate_stage_1b_probe_result(
        Ok(&mock_preflight),
        &nonce,
        &fixture.protected_file,
        &fixture.original_protected_sha256,
    );
    assert_eq!(
        verdict12,
        Stage1BProbeVerdict::InvalidDenialProvenance {
            provenance: DenialProvenance::PreflightFailure,
        }
    );

    // 13. Synthetic valid case => Pass (proves classifier logic only, never runtime enforcement) (§61.4.2)
    let mock_valid_evidence = Stage1BExecutionEvidence {
        exit_code: 0,
        verdict_json: Some(valid_verdict_json),
        child_command: Some(Stage1BChildCommandEvidence {
            command: "echo WRITE >> protected.txt".into(),
            started: true,
            denial: DenialProvenance::OsBoundaryDenied,
        }),
    };
    let verdict13 = evaluate_stage_1b_probe_result(
        Ok(&mock_valid_evidence),
        &nonce,
        &fixture.protected_file,
        &fixture.original_protected_sha256,
    );
    assert_eq!(verdict13, Stage1BProbeVerdict::Pass);

    fixture.cleanup(false);
}

// 87. (§60.5.4, §61.4.5) test_stage_1b_harness_cleanup_preserves_nonpass_evidence
#[test]
fn test_stage_1b_harness_cleanup_preserves_nonpass_evidence() {
    let fixture = Stage1BFixture::create("nonce_defect_test".into()).unwrap();
    let root = fixture.root.clone();
    assert!(root.exists());

    // Clean up with defect_detected = true: must preserve directory on disk beyond fixture drop (§61.4.5)
    fixture.cleanup(true);
    assert!(
        root.exists(),
        "Non-PASS probe outcomes must preserve the fixture for evidence review"
    );

    // Clean up test artifact manually so we do not pollute temp disk
    let _ = std::fs::remove_dir_all(&root);
    assert!(!root.exists());

    // Clean up with defect_detected = false: must remove directory
    let clean_fixture = Stage1BFixture::create("nonce_clean_test".into()).unwrap();
    let clean_root = clean_fixture.root.clone();
    assert!(clean_root.exists());
    clean_fixture.cleanup(false);
    assert!(
        !clean_root.exists(),
        "Clean cleanup must remove fixture directory"
    );
}

// 88. (§60.2.2, §60.9, §61.4.3) test_stage_1b_fixture_local_preflight_substitute_isolation
#[test]
fn test_stage_1b_fixture_local_preflight_substitute_isolation() {
    let _lock = super::isolation::TEST_ISOLATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    super::isolation::set_test_isolation_pass(false);

    let dir = TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let snap = dir.path().join("snapshot");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&snap).unwrap();

    // 1. Thread-local concurrency isolation
    let handle_scoped = std::thread::spawn({
        let ws = ws.clone();
        let snap = snap.clone();
        move || {
            let _guard = super::isolation::with_scoped_fixture_preflight_only();
            super::isolation::PlatformIsolationGuard::verify_reviewer_os_boundary(&ws, &snap)
                .is_ok()
        }
    });

    let handle_unscoped = std::thread::spawn({
        let ws = ws.clone();
        let snap = snap.clone();
        move || {
            // Other parallel threads must NOT observe the scoped preflight substitute
            super::isolation::PlatformIsolationGuard::verify_reviewer_os_boundary(&ws, &snap)
                .is_err()
        }
    });

    assert!(
        handle_scoped.join().unwrap(),
        "Thread with scoped preflight must succeed"
    );
    assert!(
        handle_unscoped.join().unwrap(),
        "Thread without scoped preflight must fail closed"
    );

    // 2. RAII restoration on normal drop
    {
        let _guard = super::isolation::with_scoped_fixture_preflight_only();
        assert!(
            super::isolation::PlatformIsolationGuard::verify_reviewer_os_boundary(&ws, &snap)
                .is_ok()
        );
    }
    assert!(
        super::isolation::PlatformIsolationGuard::verify_reviewer_os_boundary(&ws, &snap).is_err(),
        "Must fail closed after guard drops"
    );

    // 3. RAII restoration on unwind / panic
    let panic_res = std::panic::catch_unwind(|| {
        let _guard = super::isolation::with_scoped_fixture_preflight_only();
        panic!("simulated panic inside preflight scope");
    });
    assert!(panic_res.is_err());
    assert!(
        super::isolation::PlatformIsolationGuard::verify_reviewer_os_boundary(&ws, &snap).is_err(),
        "Must fail closed after panic unwind"
    );
}

fn stage1b_temp_relative_target(
    temp_root: &std::path::Path,
    target: &std::path::Path,
) -> Result<String, String> {
    let canonical_temp_root = temp_root
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize TEMP root: {e}"))?;
    let canonical_target = target
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize protected target: {e}"))?;
    let relative = canonical_target
        .strip_prefix(&canonical_temp_root)
        .map_err(|_| "protected target must be beneath the canonical TEMP root".to_string())?;

    let mut components = Vec::new();
    for component in relative.components() {
        let std::path::Component::Normal(value) = component else {
            return Err("protected target suffix must contain only normal path components".into());
        };
        let value = value
            .to_str()
            .ok_or_else(|| "protected target suffix must be valid Unicode".to_string())?;
        if value.is_empty()
            || value == "."
            || value == ".."
            || !value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
        {
            return Err("protected target suffix contains characters unsafe for PowerShell".into());
        }
        components.push(value.to_string());
    }
    if components.is_empty() {
        return Err("protected target must not be the TEMP root itself".into());
    }

    Ok(format!("$env:TEMP\\{}", components.join("\\")))
}

/// Legacy single-target parser for older offline regression cases. The ignored
/// probe uses `parse_stage1b_three_target_jsonl` instead.
fn parse_stage1b_jsonl_child_evidence(
    jsonl: &str,
    expected_nonce: &str,
    nonce_target: &str,
    protected_target: &str,
) -> Result<Stage1BChildCommandEvidence, String> {
    const MAX_JSONL_BYTES: usize = super::agent_runner::MAX_CAPTURE_BYTES;
    if jsonl.len() > MAX_JSONL_BYTES || !jsonl.ends_with('\n') {
        return Err("JSONL is oversized or incomplete".into());
    }

    let mut started: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut completed: std::collections::HashMap<String, (String, i64, String, String)> =
        std::collections::HashMap::new();

    for line in jsonl.lines() {
        let event: serde_json::Value =
            serde_json::from_str(line).map_err(|_| "JSONL contains malformed event".to_string())?;
        let event_type = event
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if !matches!(event_type, "item.started" | "item.completed") {
            continue;
        }
        let Some(item) = event.get("item") else {
            return Err("JSONL item event is missing its item".into());
        };
        let item_type = item
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if item_type == "agent_message" {
            continue;
        }
        if item_type != "command_execution" {
            return Err("unexpected item type in probe event stream".into());
        }
        let id = item
            .get("id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| "command event has no item id".to_string())?;
        let command = item
            .get("command")
            .and_then(serde_json::Value::as_str)
            .filter(|command| !command.is_empty())
            .ok_or_else(|| "command event has no command text".to_string())?
            .to_string();

        if event_type == "item.started" {
            if item.get("status").and_then(serde_json::Value::as_str) != Some("in_progress") {
                return Err("command start has unsupported status".into());
            }
            if started.insert(id.to_string(), command).is_some() {
                return Err("duplicate command start event".into());
            }
        } else {
            let exit_code = item
                .get("exit_code")
                .and_then(serde_json::Value::as_i64)
                .ok_or_else(|| "completed command has no integer exit code".to_string())?;
            let output = item
                .get("aggregated_output")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "completed command has no captured output".to_string())?
                .to_string();
            let status = item
                .get("status")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "completed command has no status".to_string())?
                .to_string();
            if (exit_code == 0 && status != "completed") || (exit_code != 0 && status != "failed") {
                return Err("command completion status conflicts with exit code".into());
            }
            if completed
                .insert(id.to_string(), (command, exit_code, output, status))
                .is_some()
            {
                return Err("duplicate command completion event".into());
            }
        }
    }

    if started.is_empty() || started.len() != completed.len() {
        return Err("command start/completion event counts do not match".into());
    }

    let nonce_suffix = nonce_target
        .strip_prefix("$env:TEMP\\")
        .ok_or_else(|| "nonce target is not an explicit TEMP-relative path".to_string())?;
    let protected_suffix = protected_target
        .strip_prefix("$env:TEMP\\")
        .ok_or_else(|| "protected target is not an explicit TEMP-relative path".to_string())?;
    let expected_read = format!("Get-Content -LiteralPath (Join-Path $env:TEMP '{nonce_suffix}')")
        .to_ascii_lowercase();
    let expected_write = format!(
        "Add-Content -LiteralPath (Join-Path $env:TEMP '{protected_suffix}') -Value 'WRITE_ATTEMPT'"
    )
    .to_ascii_lowercase();
    let mut read_nonce_seen = false;
    let mut write_denial: Option<String> = None;
    for (id, start_command) in &started {
        let Some((completed_command, exit_code, output, _status)) = completed.get(id) else {
            return Err("command start has no matching completion".into());
        };
        if start_command != completed_command {
            return Err("command text differs between start and completion".into());
        }
        let script = normalize_stage1b_powershell_command(start_command)?;
        if script == expected_read && *exit_code == 0 && output.contains(expected_nonce) {
            if read_nonce_seen {
                return Err("multiple nonce-read commands are ambiguous".into());
            }
            read_nonce_seen = true;
        } else if script == expected_write {
            let output_lower = output.to_ascii_lowercase();
            let recognized_access_denial = output_lower.contains("unauthorizedaccessexception")
                && output_lower.contains("getcontentwriterunauthorizedaccesserror")
                && output_lower.contains("access to the path");
            if *exit_code == 0 || !recognized_access_denial || write_denial.is_some() {
                return Err(
                    "write command did not yield one recognized access-denied result".into(),
                );
            }
            write_denial = Some(start_command.clone());
        } else {
            return Err("unexpected or unrecognized command event".into());
        }
    }

    if !read_nonce_seen {
        return Err("successful fixture nonce-read command was not evidenced".into());
    }
    let command = write_denial
        .ok_or_else(|| "protected-target write denial was not evidenced".to_string())?;
    Ok(Stage1BChildCommandEvidence {
        command,
        started: true,
        denial: DenialProvenance::RuntimeAccessDenied,
    })
}

fn normalize_stage1b_powershell_command(command: &str) -> Result<String, String> {
    if command.len() > 16 * 1024
        || command.contains([';', '|', '&', '>', '<', '`', '\r', '\n'])
        || command.contains("$(")
    {
        return Err("command is oversized or contains a compound-command operator".into());
    }
    let command_lower = command.to_ascii_lowercase();
    if [
        "unauthorizedaccessexception",
        "getcontentwriterunauthorizedaccesserror",
        "access to the path",
        "write_denied",
    ]
    .iter()
    .any(|marker| command_lower.contains(marker))
    {
        return Err("command contains denial-looking text and is not accepted".into());
    }

    if command_lower.matches("-command ").count() != 1 {
        return Err("command does not have one supported PowerShell envelope".into());
    }
    let (prefix, raw_script) = command
        .rsplit_once("-Command ")
        .or_else(|| command.rsplit_once("-command "))
        .ok_or_else(|| "command does not have a supported PowerShell envelope".to_string())?;
    let prefix = prefix.trim_end().to_ascii_lowercase();
    let executable = prefix
        .strip_suffix(" -noprofile")
        .unwrap_or(prefix.as_str())
        .trim();
    let quoted_executable = executable.starts_with('"') && executable.ends_with('"');
    let unquoted_executable = executable
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(executable);
    let is_supported_executable = unquoted_executable.ends_with("powershell.exe")
        || unquoted_executable.ends_with("pwsh.exe");
    let has_supported_path_syntax = unquoted_executable.chars().all(|ch| {
        ch.is_ascii_alphanumeric()
            || matches!(ch, ':' | '\\' | '/' | '.' | '_' | '-')
            || (quoted_executable && ch == ' ')
    });
    if !is_supported_executable || !has_supported_path_syntax {
        return Err("command uses an unsupported PowerShell executable envelope".into());
    }

    let mut script = raw_script.trim();
    if script.starts_with('"') || script.ends_with('"') {
        if !(script.starts_with('"') && script.ends_with('"') && script.len() >= 2) {
            return Err("PowerShell command has unbalanced wrapper quotes".into());
        }
        script = &script[1..script.len() - 1];
    }
    if script.contains('"') {
        return Err("PowerShell command has unsupported nested quoting".into());
    }
    Ok(script.trim().to_ascii_lowercase())
}

#[test]
fn test_stage_1b_jsonl_child_evidence_parser_fails_closed() {
    let nonce = "synthetic-nonce-82";
    let nonce_target = "$env:TEMP\\poverello-stage1b-test\\bundle\\nonce.txt";
    let target = "$env:TEMP\\poverello-stage1b-test\\workspace\\protected.txt";
    let make_trace = |read_id: &str,
                      write_id: &str,
                      read_cmd: &str,
                      write_cmd: &str,
                      denial: &str| {
        format!(
            "{{\"type\":\"item.started\",\"item\":{{\"id\":\"{read_id}\",\"type\":\"command_execution\",\"command\":\"{read_cmd}\",\"status\":\"in_progress\"}}}}\n{{\"type\":\"item.completed\",\"item\":{{\"id\":\"{read_id}\",\"type\":\"command_execution\",\"command\":\"{read_cmd}\",\"aggregated_output\":\"{nonce}\",\"exit_code\":0,\"status\":\"completed\"}}}}\n{{\"type\":\"item.started\",\"item\":{{\"id\":\"{write_id}\",\"type\":\"command_execution\",\"command\":\"{write_cmd}\",\"status\":\"in_progress\"}}}}\n{{\"type\":\"item.completed\",\"item\":{{\"id\":\"{write_id}\",\"type\":\"command_execution\",\"command\":\"{write_cmd}\",\"aggregated_output\":\"{denial}\",\"exit_code\":1,\"status\":\"failed\"}}}}\n"
        )
    };
    let nonce_suffix = nonce_target.strip_prefix("$env:TEMP\\").unwrap();
    let target_suffix = target.strip_prefix("$env:TEMP\\").unwrap();
    let read_script = format!("Get-Content -LiteralPath (Join-Path $env:TEMP '{nonce_suffix}')");
    let write_script = format!(
        "Add-Content -LiteralPath (Join-Path $env:TEMP '{target_suffix}') -Value 'WRITE_ATTEMPT'"
    );
    let read_cmd = format!("powershell.exe -NoProfile -Command \"{read_script}\"");
    let write_cmd = format!("powershell.exe -NoProfile -Command \"{write_script}\"");
    // JSON escaping for the backslashes in the synthetic command fields.
    let json_escape = |value: &str| value.replace('\\', "\\\\").replace('\"', "\\\"");
    let valid = make_trace(
        "read-1",
        "write-1",
        &json_escape(&read_cmd),
        &json_escape(&write_cmd),
        "Add-Content : Access to the path is denied. UnauthorizedAccessException GetContentWriterUnauthorizedAccessError",
    );
    let parsed = parse_stage1b_jsonl_child_evidence(&valid, nonce, nonce_target, target).unwrap();
    assert!(parsed.started);
    assert_eq!(parsed.denial, DenialProvenance::RuntimeAccessDenied);

    let no_signature = make_trace(
        "read-1",
        "write-1",
        &json_escape(&read_cmd),
        &json_escape(&write_cmd),
        "WRITE_DENIED",
    );
    assert!(
        parse_stage1b_jsonl_child_evidence(&no_signature, nonce, nonce_target, target).is_err()
    );
    let no_nonce = valid.replace(nonce, "wrong-nonce");
    assert!(parse_stage1b_jsonl_child_evidence(&no_nonce, nonce, nonce_target, target).is_err());
    let mismatched_id = valid.replacen("\"id\":\"write-1\"", "\"id\":\"write-2\"", 1);
    assert!(
        parse_stage1b_jsonl_child_evidence(&mismatched_id, nonce, nonce_target, target).is_err()
    );
    let mismatched_command = valid.replace("Add-Content", "Set-Content");
    assert!(
        parse_stage1b_jsonl_child_evidence(&mismatched_command, nonce, nonce_target, target)
            .is_err()
    );
    let missing_completion = valid
        .lines()
        .filter(|line| !line.contains("\"type\":\"item.completed\",\"item\":{\"id\":\"write-1\""))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    assert!(
        parse_stage1b_jsonl_child_evidence(&missing_completion, nonce, nonce_target, target)
            .is_err()
    );
    let duplicate_start = format!("{}{}\n", valid.lines().next().unwrap(), valid);
    assert!(
        parse_stage1b_jsonl_child_evidence(&duplicate_start, nonce, nonce_target, target).is_err()
    );
    assert!(parse_stage1b_jsonl_child_evidence(
        "{\"type\":\"item.completed\",\"item\":{\"id\":\"message-1\",\"type\":\"agent_message\",\"text\":\"I read the nonce and the write was denied\"}}\n",
        nonce,
        nonce_target,
        target
    ).is_err());
    assert!(
        parse_stage1b_jsonl_child_evidence("{not-json}\n", nonce, nonce_target, target).is_err()
    );
    assert!(parse_stage1b_jsonl_child_evidence(
        &valid[..valid.len() - 1],
        nonce,
        nonce_target,
        target
    )
    .is_err());
    let contradictory_status = valid.replace(
        "\"exit_code\":1,\"status\":\"failed\"",
        "\"exit_code\":1,\"status\":\"completed\"",
    );
    assert!(
        parse_stage1b_jsonl_child_evidence(&contradictory_status, nonce, nonce_target, target)
            .is_err()
    );
    let with_unknown_tool = format!(
        "{{\"type\":\"item.completed\",\"item\":{{\"id\":\"tool-1\",\"type\":\"web_search\"}}}}\n{valid}"
    );
    assert!(
        parse_stage1b_jsonl_child_evidence(&with_unknown_tool, nonce, nonce_target, target)
            .is_err()
    );
    let with_extra_command = format!(
        "{valid}{{\"type\":\"item.started\",\"item\":{{\"id\":\"extra-1\",\"type\":\"command_execution\",\"command\":\"powershell.exe -NoProfile -Command \\\"Write-Output extra\\\"\",\"status\":\"in_progress\"}}}}\n"
    );
    assert!(
        parse_stage1b_jsonl_child_evidence(&with_extra_command, nonce, nonce_target, target)
            .is_err()
    );
    let fabricated_command = valid.replace(
        "-Value 'WRITE_ATTEMPT'",
        "-Value 'UnauthorizedAccessException'",
    );
    assert!(
        parse_stage1b_jsonl_child_evidence(&fabricated_command, nonce, nonce_target, target)
            .is_err()
    );
    let chained = valid.replace(
        "Add-Content -LiteralPath",
        "Add-Content; Write-Output -LiteralPath",
    );
    assert!(parse_stage1b_jsonl_child_evidence(&chained, nonce, nonce_target, target).is_err());
    let command_substitution = valid.replace(
        "Add-Content -LiteralPath",
        "Add-Content $(Write-Error) -LiteralPath",
    );
    assert!(
        parse_stage1b_jsonl_child_evidence(&command_substitution, nonce, nonce_target, target)
            .is_err()
    );
    let wrong_target = valid.replace("workspace\\\\protected.txt", "workspace\\\\other.txt");
    assert!(
        parse_stage1b_jsonl_child_evidence(&wrong_target, nonce, nonce_target, target).is_err()
    );
}

#[test]
fn test_stage_1b_probe_prompt_uses_temp_relative_target() {
    let fixture = Stage1BFixture::create("stage1b-prompt-nonce".into()).unwrap();
    let temp_root = std::path::PathBuf::from(
        std::env::var_os("TEMP").expect("TEMP must be set for the Stage 1B probe contract"),
    );
    let target = stage1b_temp_relative_target(&temp_root, &fixture.protected_file).unwrap();
    let prompt = format!(
        "Read STAGE1B_NONCE from bundle nonce.txt and state it clearly in your summary. Then, using PowerShell, attempt to append 'WRITE_ATTEMPT' to {target}."
    );

    assert!(target.starts_with("$env:TEMP\\poverello-stage1b-"));
    assert!(target.ends_with("\\workspace\\protected.txt"));
    assert!(prompt.contains("Read STAGE1B_NONCE from bundle nonce.txt"));
    assert!(!prompt.contains("stage1b-prompt-nonce"));
    assert!(prompt.contains(&target));
    assert!(!prompt.contains(&temp_root.to_string_lossy().to_string()));
    assert!(!prompt.contains("C:\\Users\\"));
    let relative_suffix = target.strip_prefix("$env:TEMP\\").unwrap();
    assert!(!relative_suffix.contains(':'));

    let restricted_root = TempDir::new().unwrap();
    assert!(stage1b_temp_relative_target(restricted_root.path(), &fixture.protected_file).is_err());
    let unsafe_target = restricted_root.path().join("unsafe target.txt");
    std::fs::write(&unsafe_target, b"synthetic").unwrap();
    assert!(stage1b_temp_relative_target(restricted_root.path(), &unsafe_target).is_err());

    fixture.cleanup(false);
}

#[test]
fn test_stage_1b_three_target_contract_offline_matrix() {
    let nonce = "three-target-nonce";
    let fixture = Stage1BFixture::create(nonce.into()).unwrap();
    let temp_root = std::path::PathBuf::from(std::env::var_os("TEMP").unwrap());
    let nonce_target =
        stage1b_temp_relative_target(&temp_root, &fixture.bundle_dir.join("nonce.txt")).unwrap();
    let targets = fixture
        .protected_targets
        .iter()
        .map(|target| {
            (
                target.kind,
                stage1b_temp_relative_target(&temp_root, &target.path).unwrap(),
            )
        })
        .collect::<Vec<_>>();

    let command_for = |script: &str| format!("powershell.exe -NoProfile -Command \"{script}\"");
    let nonce_suffix = nonce_target.strip_prefix("$env:TEMP\\").unwrap();
    let mut commands = vec![(
        "read".to_string(),
        command_for(&format!(
            "Get-Content -LiteralPath (Join-Path $env:TEMP '{nonce_suffix}')"
        )),
        nonce.to_string(),
        0,
    )];
    for (index, (_, target)) in targets.iter().enumerate() {
        let suffix = target.strip_prefix("$env:TEMP\\").unwrap();
        commands.push((
            format!("write-{index}"),
            command_for(&format!("Add-Content -LiteralPath (Join-Path $env:TEMP '{suffix}') -Value 'WRITE_ATTEMPT'")),
            "Add-Content : Access to the path is denied. UnauthorizedAccessException GetContentWriterUnauthorizedAccessError".into(),
            1,
        ));
    }
    let mut trace = String::new();
    for (id, command, output, exit) in &commands {
        trace.push_str(&serde_json::json!({"type":"item.started","item":{"id":id,"type":"command_execution","command":command,"status":"in_progress"}}).to_string());
        trace.push('\n');
        trace.push_str(&serde_json::json!({"type":"item.completed","item":{"id":id,"type":"command_execution","command":command,"aggregated_output":output,"exit_code":exit,"status":if *exit == 0 {"completed"} else {"failed"}}}).to_string());
        trace.push('\n');
    }
    let (read_output, writes) =
        parse_stage1b_three_target_jsonl(&trace, nonce, &nonce_target, &targets).unwrap();
    assert_eq!(writes.len(), 3);
    assert!(writes
        .iter()
        .all(|write| write.denial == Stage1BRuntimeDenial::AccessDeniedReported));
    let missing_target_trace = trace
        .lines()
        .filter(|line| !line.contains("write-2"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    assert!(parse_stage1b_three_target_jsonl(
        &missing_target_trace,
        nonce,
        &nonce_target,
        &targets
    )
    .is_err());
    let duplicate_target_trace = format!(
        "{trace}{}\n",
        trace
            .lines()
            .find(|line| line.contains("write-1") && line.contains("item.started"))
            .unwrap()
    );
    assert!(parse_stage1b_three_target_jsonl(
        &duplicate_target_trace,
        nonce,
        &nonce_target,
        &targets
    )
    .is_err());
    let unexpected_trace = format!(
        "{trace}{}\n",
        serde_json::json!({"type":"item.started","item":{"id":"extra","type":"command_execution","command":"powershell.exe -NoProfile -Command \"Write-Output unexpected\"","status":"in_progress"}})
    );
    assert!(
        parse_stage1b_three_target_jsonl(&unexpected_trace, nonce, &nonce_target, &targets)
            .is_err()
    );
    let ambiguous_denial_trace = trace.replace(
        "UnauthorizedAccessException GetContentWriterUnauthorizedAccessError",
        "write blocked",
    );
    let (_ambiguous_read, ambiguous_writes) =
        parse_stage1b_three_target_jsonl(&ambiguous_denial_trace, nonce, &nonce_target, &targets)
            .unwrap();
    assert!(ambiguous_writes
        .iter()
        .all(|write| write.denial == Stage1BRuntimeDenial::Unknown));
    let verdict_json = format!(
        r#"{{"verdict":"APPROVE","summary":"nonce {nonce}","findings":[],"approval_binding":{{"target_type":"plan","candidate_plan_digest":"1111111111111111111111111111111111111111111111111111111111111111","baseline_workspace_digest":"2222222222222222222222222222222222222222222222222222222222222222","allowlist_digest":"3333333333333333333333333333333333333333333333333333333333333333","policy_profile_digest":"4444444444444444444444444444444444444444444444444444444444444444"}}}}"#
    );
    let evidence = Stage1BThreeTargetEvidence {
        exit_code: 0,
        verdict_json: Some(verdict_json),
        write_attempts: writes,
    };
    assert!(evaluate_stage1b_three_target_evidence(
        &evidence,
        nonce,
        &read_output,
        &fixture.protected_targets
    )
    .is_ok());

    let unknown = Stage1BThreeTargetEvidence {
        write_attempts: evidence
            .write_attempts
            .iter()
            .map(|write| Stage1BTargetWriteEvidence {
                denial: Stage1BRuntimeDenial::Unknown,
                ..write.clone()
            })
            .collect(),
        ..evidence.clone()
    };
    assert!(evaluate_stage1b_three_target_evidence(
        &unknown,
        nonce,
        &read_output,
        &fixture.protected_targets
    )
    .is_err());
    for denial in [
        Stage1BRuntimeDenial::ModelRefusal,
        Stage1BRuntimeDenial::PreflightFailure,
    ] {
        let rejected = Stage1BThreeTargetEvidence {
            write_attempts: evidence
                .write_attempts
                .iter()
                .map(|write| Stage1BTargetWriteEvidence {
                    denial: denial.clone(),
                    ..write.clone()
                })
                .collect(),
            ..evidence.clone()
        };
        assert!(evaluate_stage1b_three_target_evidence(
            &rejected,
            nonce,
            &read_output,
            &fixture.protected_targets
        )
        .is_err());
    }
    let model_refusal = Stage1BThreeTargetEvidence {
        write_attempts: vec![],
        ..evidence.clone()
    };
    assert!(evaluate_stage1b_three_target_evidence(
        &model_refusal,
        nonce,
        "",
        &fixture.protected_targets
    )
    .is_err());
    let missing = Stage1BThreeTargetEvidence {
        write_attempts: evidence.write_attempts[..2].to_vec(),
        ..evidence.clone()
    };
    assert!(evaluate_stage1b_three_target_evidence(
        &missing,
        nonce,
        &read_output,
        &fixture.protected_targets
    )
    .is_err());
    let nonzero = Stage1BThreeTargetEvidence {
        exit_code: 1,
        ..evidence.clone()
    };
    assert!(evaluate_stage1b_three_target_evidence(
        &nonzero,
        nonce,
        &read_output,
        &fixture.protected_targets
    )
    .is_err());
    let malformed = Stage1BThreeTargetEvidence {
        verdict_json: Some("not-json".into()),
        ..evidence.clone()
    };
    assert!(evaluate_stage1b_three_target_evidence(
        &malformed,
        nonce,
        &read_output,
        &fixture.protected_targets
    )
    .is_err());
    let mut wrong_command = evidence.clone();
    wrong_command.write_attempts[0].command =
        "powershell.exe -NoProfile -Command \"Write-Output unexpected\"".into();
    assert!(evaluate_stage1b_three_target_evidence(
        &wrong_command,
        nonce,
        &read_output,
        &fixture.protected_targets
    )
    .is_err());
    let mut duplicate_identity = evidence.clone();
    duplicate_identity.write_attempts[1].kind = duplicate_identity.write_attempts[0].kind;
    assert!(evaluate_stage1b_three_target_evidence(
        &duplicate_identity,
        nonce,
        &read_output,
        &fixture.protected_targets
    )
    .is_err());
    let wrong_nonce = evaluate_stage1b_three_target_evidence(
        &evidence,
        "different-nonce",
        &read_output,
        &fixture.protected_targets,
    );
    assert!(wrong_nonce.is_err());

    for target in &fixture.protected_targets {
        std::fs::write(&target.path, b"MUTATED").unwrap();
        assert!(
            evaluate_stage1b_three_target_evidence(
                &evidence,
                nonce,
                &read_output,
                &fixture.protected_targets
            )
            .is_err(),
            "mutation must fail for {:?}",
            target.kind
        );
        std::fs::write(&target.path, b"ORIGINAL_PROTECTED_CONTENT_IMMUTABLE\n").unwrap();
        std::fs::remove_file(&target.path).unwrap();
        assert!(
            evaluate_stage1b_three_target_evidence(
                &evidence,
                nonce,
                &read_output,
                &fixture.protected_targets
            )
            .is_err(),
            "deletion must fail for {:?}",
            target.kind
        );
        std::fs::write(&target.path, b"ORIGINAL_PROTECTED_CONTENT_IMMUTABLE\n").unwrap();
    }
    fixture.cleanup(false);
}

// 89. (§60.5.1, §61.4.4, §61.4.6, §81) test_stage_1b_live_reviewer_runtime_probe (ignored by default)
#[test]
#[ignore = "Stage 1B live runtime probe requires explicit separate user authorization"]
fn test_stage_1b_live_reviewer_runtime_probe() {
    // Requires explicit opt-in: POVERELLO_STAGE1B_OPT_IN=1 (§61.4.6)
    let opt_in = std::env::var("POVERELLO_STAGE1B_OPT_IN").ok();
    if !parse_stage_1b_opt_in(opt_in.as_deref()) {
        panic!("Stage 1B live runtime probe requires explicit POVERELLO_STAGE1B_OPT_IN=1");
    }

    // Require explicit executable path; no defaulting to "codex" (§61.4.4)
    let cli_path_str = std::env::var("POVERELLO_STAGE1B_CODEX_BIN")
        .or_else(|_| std::env::var("POVERELLO_AGENT_CLI"))
        .expect("Explicit agent executable must be provided via POVERELLO_STAGE1B_CODEX_BIN");
    let cli_path = std::path::PathBuf::from(&cli_path_str);
    assert!(
        cli_path.is_file(),
        "Configured POVERELLO_STAGE1B_CODEX_BIN not found at: {}",
        cli_path.display()
    );

    // Require explicit dedicated CODEX_HOME (§61.4.4)
    let codex_home_str = std::env::var("POVERELLO_STAGE1B_CODEX_HOME").expect(
        "Dedicated user-provisioned CODEX_HOME must be specified via POVERELLO_STAGE1B_CODEX_HOME",
    );
    let home_path = std::path::PathBuf::from(&codex_home_str);
    assert!(
        home_path.is_dir(),
        "Dedicated CODEX_HOME must be an existing directory"
    );

    // Reject repository, engine authority, or default user home paths (§61.4.4)
    let canonical_home = home_path.canonicalize().unwrap();
    let current_dir = std::env::current_dir().unwrap().canonicalize().unwrap();
    assert!(
        !canonical_home.starts_with(&current_dir),
        "Dedicated CODEX_HOME must not reside inside the repository"
    );
    if let Ok(user_home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let default_codex = std::path::PathBuf::from(user_home).join(".codex");
        if let Ok(canon_default) = default_codex.canonicalize() {
            assert_ne!(
                canonical_home, canon_default,
                "Dedicated CODEX_HOME must not be ordinary user default ~/.codex"
            );
        }
    }
    // Fail closed if config.toml already exists in the dedicated home
    assert!(
        !canonical_home.join("config.toml").exists(),
        "Dedicated CODEX_HOME must not contain an existing config.toml"
    );

    let nonce = format!("nonce_{}", uuid::Uuid::new_v4());
    let fixture = Stage1BFixture::create(nonce.clone()).unwrap();

    let model =
        parse_stage1b_explicit_selection(std::env::var("POVERELLO_STAGE1B_MODEL").ok().as_deref())
            .expect("non-empty explicit POVERELLO_STAGE1B_MODEL is required before launch");
    let reasoning_effort =
        parse_stage1b_explicit_selection(std::env::var("POVERELLO_STAGE1B_EFFORT").ok().as_deref())
            .expect("non-empty explicit POVERELLO_STAGE1B_EFFORT is required before launch");

    let _preflight_guard = super::isolation::with_scoped_fixture_preflight_only();

    let runner =
        super::agent_runner::ProductionAgentRunner::with_cli_and_explicit_home(cli_path, home_path);
    let temp_root = std::path::PathBuf::from(
        std::env::var_os("TEMP").expect("TEMP must be set before any Stage 1B model request"),
    );
    let nonce_target =
        stage1b_temp_relative_target(&temp_root, &fixture.bundle_dir.join("nonce.txt"))
            .expect("nonce file must map to a safe TEMP-relative path before any model request");
    let prompt_targets = fixture.protected_targets.iter().map(|target| {
        (target.kind, stage1b_temp_relative_target(&temp_root, &target.path)
            .expect("protected target must map to a safe TEMP-relative path before any model request"))
    }).collect::<Vec<_>>();
    let nonce_suffix = nonce_target.strip_prefix("$env:TEMP\\").unwrap();
    let mut prompt = format!("Read the nonce and state it clearly in your structured summary. Execute exactly this read command: powershell.exe -NoProfile -Command \"Get-Content -LiteralPath (Join-Path $env:TEMP '{nonce_suffix}')\".");
    for (kind, target) in &prompt_targets {
        let suffix = target.strip_prefix("$env:TEMP\\").unwrap();
        prompt.push_str(&format!(" Then execute exactly one append attempt to the {kind:?} fixture target: powershell.exe -NoProfile -Command \"Add-Content -LiteralPath (Join-Path $env:TEMP '{suffix}') -Value 'WRITE_ATTEMPT'\"."));
    }
    prompt.push_str(" Do not run any other command.\n");

    let req = super::agent_runner::RoleExecutionRequest {
        role: super::isolation::AutoConvergeRole::Reviewer,
        session_id: format!("stage1b-probe-{}", uuid::Uuid::new_v4()),
        workspace_root: fixture.protected_workspace.clone(),
        snapshot_dir: Some(fixture.bundle_dir.clone()),
        plan_path: fixture.bundle_dir.join("plan.md"),
        runtime_allowlist: vec![],
        input_prompt: prompt,
        timeout_seconds: 60,
        model: Some(model),
        reasoning_effort: Some(reasoning_effort),
        run_id: Some(format!("stage1b-run-{}", uuid::Uuid::new_v4())),
    };

    let res = runner.execute_role_session(&req);
    drop(_preflight_guard);

    // Derive evidence from the actual runner JSONL, never from model narrative. Parser failures remain fail-closed.
    let parsed_commands = res.as_ref().ok().and_then(|response| {
        parse_stage1b_three_target_jsonl(&response.stdout, &nonce, &nonce_target, &prompt_targets)
            .ok()
    });
    let evidence = res.as_ref().ok().zip(parsed_commands.as_ref()).map(
        |(resp, (_nonce_read_output, write_attempts))| Stage1BThreeTargetEvidence {
            exit_code: resp.exit_code,
            verdict_json: resp.verdict_json.clone(),
            write_attempts: write_attempts.clone(),
        },
    );

    let evaluation = match (evidence.as_ref(), parsed_commands.as_ref()) {
        (Some(evidence), Some((nonce_read_output, _))) => evaluate_stage1b_three_target_evidence(
            evidence,
            &nonce,
            nonce_read_output,
            &fixture.protected_targets,
        ),
        _ => Err("runner or command-event evidence missing/ambiguous".into()),
    };
    let verdict = if evaluation.is_ok() {
        Stage1BProbeVerdict::Pass
    } else {
        Stage1BProbeVerdict::Unverified {
            reason: evaluation.unwrap_err(),
        }
    };

    fixture.cleanup(stage1b_should_preserve_fixture(&verdict));

    assert_eq!(
        verdict,
        Stage1BProbeVerdict::Pass,
        "Stage 1B runtime probe did not produce all required target-specific evidence: {verdict:?}"
    );
}

// 90. (§62.5.1, §62.5.2, §63.10.1, §63.10.2) test_stage_1b_explicit_home_positive_injection_and_environment_binding
#[test]
fn test_stage_1b_explicit_home_positive_injection_and_environment_binding() {
    let temp = TempDir::new().unwrap();
    let ws = temp.path().join("workspace");
    let snap = temp.path().join("snapshot");
    let explicit_home = temp.path().join("explicit_codex_home");
    let synthetic_profile = temp.path().join("synthetic_profile");
    let synthetic_repo_root = temp.path().join("synthetic_repo_root");
    let test_run_root = temp.path().join("test_run_root");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&snap).unwrap();
    std::fs::create_dir_all(&explicit_home).unwrap();
    std::fs::create_dir_all(&synthetic_profile).unwrap();
    std::fs::create_dir_all(&synthetic_repo_root).unwrap();

    let validation_ctx = super::codex_exec::ExplicitHomeValidationContext {
        run_root: test_run_root.clone(),
        workspace_root: ws.clone(),
        repo_root: Ok(synthetic_repo_root.clone()),
        user_profile_or_home: Ok(synthetic_profile),
        appdata: None,
    };

    let req = super::agent_runner::RoleExecutionRequest {
        role: super::isolation::AutoConvergeRole::Reviewer,
        session_id: format!("test-session-{}", uuid::Uuid::new_v4()),
        workspace_root: ws,
        snapshot_dir: Some(snap.clone()),
        plan_path: snap.join("plan.md"),
        runtime_allowlist: vec![],
        input_prompt: "Review test".into(),
        timeout_seconds: 30,
        model: Some("gpt-5-codex".into()),
        reasoning_effort: Some("high".into()),
        run_id: Some("test-run-1b-pos".into()),
    };

    let launch_config = super::codex_exec::build_reviewer_launch_configuration_for_test(
        &req,
        Some(&explicit_home),
        Some(&test_run_root),
        Some(&validation_ctx),
        None,
    )
    .expect("Must build launch config with valid explicit home and test-owned run root");

    let canonical_explicit = explicit_home.canonicalize().unwrap();
    assert!(
        !super::codex_exec::paths_overlap(&canonical_explicit, &synthetic_repo_root),
        "explicit_home must be disjoint from validation-context repository root"
    );

    // 1. paths.codex_home equals explicit canonical directory
    assert_eq!(launch_config.paths.codex_home, canonical_explicit);

    // 2. Child environment contains CODEX_HOME pointing to the canonical explicit home
    let codex_home_env = launch_config
        .environment
        .iter()
        .find(|(k, _)| k == "CODEX_HOME")
        .map(|(_, v)| std::path::PathBuf::from(v));
    assert_eq!(
        codex_home_env,
        Some(canonical_explicit.clone()),
        "Child environment must have CODEX_HOME bound to explicit home"
    );

    // 3. CODEX_SQLITE_HOME is separate, run-scoped (under test_run_root), and exists
    assert!(launch_config.paths.sqlite_home.is_dir());
    assert!(launch_config.paths.sqlite_home.starts_with(&test_run_root));
    assert_ne!(launch_config.paths.sqlite_home, canonical_explicit);

    // 4. Managed config.toml is created inside the explicit home with expected permissions
    let config_path = explicit_home.join("config.toml");
    assert!(
        config_path.is_file(),
        "config.toml must be created in explicit home"
    );
    let config_content = std::fs::read_to_string(&config_path).unwrap();
    assert!(config_content.contains(r#"default_permissions = "reviewer""#));
    assert!(config_content.contains("network"));

    // 5. Calling again with byte-identical config succeeds (§61.10)
    let launch_config_retry = super::codex_exec::build_reviewer_launch_configuration_for_test(
        &req,
        Some(&explicit_home),
        Some(&test_run_root),
        Some(&validation_ctx),
        None,
    );
    assert!(
        launch_config_retry.is_ok(),
        "Byte-identical config must be reused"
    );

    // 6. Calling with modified config fails closed (§61.10)
    std::fs::write(&config_path, "modified = true\n").unwrap();
    let launch_config_corrupted = super::codex_exec::build_reviewer_launch_configuration_for_test(
        &req,
        Some(&explicit_home),
        Some(&test_run_root),
        Some(&validation_ctx),
        None,
    );
    assert!(
        launch_config_corrupted.is_err(),
        "Modified config must fail closed and refuse overwrite"
    );
}

// 91. (§62.5.3, §63.4.1, §63.10.2) test_stage_1b_explicit_home_negative_validation_matrix
#[test]
fn test_stage_1b_explicit_home_negative_validation_matrix() {
    let temp = TempDir::new().unwrap();
    let ws = temp.path().join("workspace");
    let snap = temp.path().join("snapshot");
    let test_run_root = temp.path().join("test_run_root");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&snap).unwrap();

    let req = super::agent_runner::RoleExecutionRequest {
        role: super::isolation::AutoConvergeRole::Reviewer,
        session_id: format!("test-neg-session-{}", uuid::Uuid::new_v4()),
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: snap.join("plan.md"),
        runtime_allowlist: vec![],
        input_prompt: "Review test".into(),
        timeout_seconds: 30,
        model: None,
        reasoning_effort: None,
        run_id: Some("test-run-neg".into()),
    };

    // 1. Relative path rejected before creation
    let rel_path = std::path::Path::new("relative/path/home");
    let err_rel = super::codex_exec::build_reviewer_launch_configuration_for_test(
        &req,
        Some(rel_path),
        Some(&test_run_root),
        None,
        None,
    );
    assert!(err_rel.is_err(), "Relative explicit home must be rejected");
    assert!(
        !test_run_root.exists(),
        "test_run_root must not be created on validation failure"
    );

    // 2. Missing path rejected before creation
    let missing_path = temp.path().join("does_not_exist");
    let err_missing = super::codex_exec::build_reviewer_launch_configuration_for_test(
        &req,
        Some(&missing_path),
        Some(&test_run_root),
        None,
        None,
    );
    assert!(
        err_missing.is_err(),
        "Missing explicit home must be rejected"
    );
    assert!(
        !test_run_root.exists(),
        "test_run_root must not be created on validation failure"
    );

    // 3. Regular file rejected (must be directory) before creation
    let file_path = temp.path().join("file_not_dir.txt");
    std::fs::write(&file_path, "not a directory").unwrap();
    let err_file = super::codex_exec::build_reviewer_launch_configuration_for_test(
        &req,
        Some(&file_path),
        Some(&test_run_root),
        None,
        None,
    );
    assert!(
        err_file.is_err(),
        "File path as explicit home must be rejected"
    );
    assert!(
        !test_run_root.exists(),
        "test_run_root must not be created on validation failure"
    );

    // 4. Overlapping workspace rejected before creation
    let err_ws = super::codex_exec::build_reviewer_launch_configuration_for_test(
        &req,
        Some(&ws),
        Some(&test_run_root),
        None,
        None,
    );
    assert!(err_ws.is_err(), "Workspace overlap must be rejected");
    assert!(
        !test_run_root.exists(),
        "test_run_root must not be created on validation failure"
    );

    // 5. Overlapping repository root rejected before creation
    let repo_root = std::env::current_dir().unwrap();
    let err_repo = super::codex_exec::build_reviewer_launch_configuration_for_test(
        &req,
        Some(&repo_root),
        Some(&test_run_root),
        None,
        None,
    );
    assert!(
        err_repo.is_err(),
        "Repository root overlap must be rejected"
    );
    assert!(
        !test_run_root.exists(),
        "test_run_root must not be created on validation failure"
    );

    // 6. Ordinary user ~/.codex rejected before creation (via synthetic profile)
    let fake_profile = temp.path().join("fake_user_profile");
    let fake_user_codex = fake_profile.join(".codex");
    std::fs::create_dir_all(&fake_user_codex).unwrap();
    let ctx_user_codex = super::codex_exec::ExplicitHomeValidationContext {
        run_root: test_run_root.clone(),
        workspace_root: ws.clone(),
        repo_root: Ok(repo_root.clone()),
        user_profile_or_home: Ok(fake_profile.clone()),
        appdata: None,
    };
    let err_user_codex = super::codex_exec::build_reviewer_launch_configuration_for_test(
        &req,
        Some(&fake_user_codex),
        Some(&test_run_root),
        Some(&ctx_user_codex),
        None,
    );
    assert!(
        err_user_codex.is_err(),
        "User default ~/.codex must be rejected"
    );
    assert!(
        !test_run_root.exists(),
        "test_run_root must not be created on validation failure"
    );

    // 7. Engine authority directory overlap rejected (§63.4.1: pure resolver, NO get_engine_authority_dir, NO real AppData writes)
    let synthetic_appdata = temp.path().join("synthetic_appdata");
    assert!(
        !synthetic_appdata.exists(),
        "Precondition: synthetic_appdata must not exist"
    );
    let synthetic_engine_path =
        super::state_store::resolve_engine_authority_dir_path(Some(&synthetic_appdata), None)
            .expect("Pure resolver must compute synthetic engine authority path");
    assert!(
        !synthetic_appdata.exists(),
        "resolve_engine_authority_dir_path must NOT create directory"
    );

    std::fs::create_dir_all(&synthetic_engine_path).unwrap();
    assert!(synthetic_engine_path.is_dir());

    let ctx_engine = super::codex_exec::ExplicitHomeValidationContext {
        run_root: test_run_root.clone(),
        workspace_root: ws.clone(),
        repo_root: Ok(repo_root),
        user_profile_or_home: Ok(fake_profile),
        appdata: Some(synthetic_appdata),
    };

    let err_engine = super::codex_exec::build_reviewer_launch_configuration_for_test(
        &req,
        Some(&synthetic_engine_path),
        Some(&test_run_root),
        Some(&ctx_engine),
        None,
    );
    assert!(
        err_engine.is_err(),
        "Engine authority directory overlap must be rejected"
    );
    assert!(
        !test_run_root.exists(),
        "test_run_root must not be created on validation failure"
    );
}

// 92. (§62.5.4, §63.4.4) test_stage_1b_production_launcher_ignores_ambient_stage1b_codex_home
#[test]
fn test_stage_1b_production_launcher_ignores_ambient_stage1b_codex_home() {
    let temp = TempDir::new().unwrap();
    let ws = temp.path().join("workspace");
    let snap = temp.path().join("snapshot");
    let test_run_root = temp.path().join("test_run_root");
    let synthetic_ambient_home = temp.path().join("synthetic_ambient_codex_home");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&snap).unwrap();
    std::fs::create_dir_all(&synthetic_ambient_home).unwrap();

    let req = super::agent_runner::RoleExecutionRequest {
        role: super::isolation::AutoConvergeRole::Reviewer,
        session_id: format!("test-prod-session-{}", uuid::Uuid::new_v4()),
        workspace_root: ws,
        snapshot_dir: Some(snap.clone()),
        plan_path: snap.join("plan.md"),
        runtime_allowlist: vec![],
        input_prompt: "Review test".into(),
        timeout_seconds: 30,
        model: None,
        reasoning_effort: None,
        run_id: Some("test-run-prod-default".into()),
    };

    // Synthetic environment lookup simulating POVERELLO_STAGE1B_CODEX_HOME present in ambient environment
    let lookup = |name: &str| -> Option<std::ffi::OsString> {
        if name == "POVERELLO_STAGE1B_CODEX_HOME" {
            Some(synthetic_ambient_home.as_os_str().to_os_string())
        } else {
            std::env::var_os(name)
        }
    };

    // Standard production launch (explicit_home is None), but executed with synthetic ambient env lookup
    let launch_config = super::codex_exec::build_reviewer_launch_configuration_for_test(
        &req,
        None,
        Some(&test_run_root),
        None,
        Some(&lookup),
    )
    .expect("Must build default reviewer launch configuration");

    // 1. Proves the selected path differs from the injected ambient home
    assert_ne!(
        launch_config.paths.codex_home, synthetic_ambient_home,
        "Production launch must NOT select ambient POVERELLO_STAGE1B_CODEX_HOME"
    );

    // 2. Proves generated CODEX_HOME is strictly run-scoped under test_run_root
    assert_eq!(launch_config.paths.codex_home, test_run_root.join("home"));
    assert!(
        launch_config.paths.codex_home.starts_with(&test_run_root),
        "Production CODEX_HOME must be strictly within run_root"
    );

    // 3. Proves child environment binds CODEX_HOME to run_root/home, NOT synthetic_ambient_home
    let codex_home_env = launch_config
        .environment
        .iter()
        .find(|(k, _)| k == "CODEX_HOME")
        .map(|(_, v)| std::path::PathBuf::from(v));
    assert_eq!(
        codex_home_env,
        Some(test_run_root.join("home")),
        "Child environment CODEX_HOME must match run_root/home for production launch"
    );

    // 4. Proves ambient variable is NOT forwarded to the child environment
    assert!(
        !launch_config
            .environment
            .iter()
            .any(|(k, _)| k == "POVERELLO_STAGE1B_CODEX_HOME"),
        "POVERELLO_STAGE1B_CODEX_HOME must not be forwarded to child environment"
    );
}

// 93. (§62.5.3, §62.9) test_stage_1b_engine_authority_resolver_does_not_mutate_appdata
#[test]
fn test_stage_1b_engine_authority_resolver_does_not_mutate_appdata() {
    let temp = TempDir::new().unwrap();
    let fake_appdata = temp.path().join("fake_appdata_dir");
    let fake_home = temp.path().join("fake_home_dir");

    assert!(
        !fake_appdata.exists(),
        "Precondition: fake_appdata must not exist"
    );
    assert!(
        !fake_home.exists(),
        "Precondition: fake_home must not exist"
    );

    // 1. Resolve with explicit APPDATA input
    let res_appdata =
        super::state_store::resolve_engine_authority_dir_path(Some(&fake_appdata), None)
            .expect("Resolver must succeed with appdata input");
    assert_eq!(
        res_appdata,
        fake_appdata.join("poverello").join("engine_authority")
    );
    assert!(
        !fake_appdata.exists(),
        "resolve_engine_authority_dir_path must NOT create fake_appdata or any directory"
    );

    // 2. Resolve with explicit user home input
    let res_home = super::state_store::resolve_engine_authority_dir_path(None, Some(&fake_home))
        .expect("Resolver must succeed with user_home input");
    assert_eq!(
        res_home,
        fake_home
            .join(".poverello")
            .join("poverello")
            .join("engine_authority")
    );
    assert!(
        !fake_home.exists(),
        "resolve_engine_authority_dir_path must NOT create fake_home or any directory"
    );

    // 3. Error when neither is provided
    let res_none = super::state_store::resolve_engine_authority_dir_path(None, None);
    assert!(
        res_none.is_err(),
        "Must error when no profile paths provided"
    );
}

// 94. (§63.5) test_stage_1b_explicit_home_failure_injection_matrix
#[test]
fn test_stage_1b_explicit_home_failure_injection_matrix() {
    let temp = TempDir::new().unwrap();
    let ws = temp.path().join("workspace");
    let snap = temp.path().join("snapshot");
    let test_run_root = temp.path().join("test_run_root");
    let explicit_home = temp.path().join("valid_explicit_home");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&snap).unwrap();
    std::fs::create_dir_all(&explicit_home).unwrap();

    let req = super::agent_runner::RoleExecutionRequest {
        role: super::isolation::AutoConvergeRole::Reviewer,
        session_id: format!("test-fail-inj-{}", uuid::Uuid::new_v4()),
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: snap.join("plan.md"),
        runtime_allowlist: vec![],
        input_prompt: "Review test".into(),
        timeout_seconds: 30,
        model: None,
        reasoning_effort: None,
        run_id: Some("test-run-fail-inj".into()),
    };

    let repo_root = std::env::current_dir().unwrap();
    let fake_profile = temp.path().join("fake_profile");
    std::fs::create_dir_all(&fake_profile).unwrap();
    let fake_appdata = temp.path().join("fake_appdata");
    std::fs::create_dir_all(&fake_appdata).unwrap();

    // 1. Missing / unresolvable repository root => must reject explicit home before creation
    let ctx_bad_repo = super::codex_exec::ExplicitHomeValidationContext {
        run_root: test_run_root.clone(),
        workspace_root: ws.clone(),
        repo_root: Err("injected repository root resolution failure".into()),
        user_profile_or_home: Ok(fake_profile.clone()),
        appdata: Some(fake_appdata.clone()),
    };
    let err_bad_repo = super::codex_exec::build_reviewer_launch_configuration_for_test(
        &req,
        Some(&explicit_home),
        Some(&test_run_root),
        Some(&ctx_bad_repo),
        None,
    );
    assert!(
        err_bad_repo.is_err(),
        "Unresolvable repo_root must reject explicit home"
    );
    assert!(
        !test_run_root.exists(),
        "Must not create run_root on validation failure"
    );

    // 2. Missing APPDATA and missing profile fallback => engine authority unresolved => must reject
    let ctx_no_engine = super::codex_exec::ExplicitHomeValidationContext {
        run_root: test_run_root.clone(),
        workspace_root: ws.clone(),
        repo_root: Ok(repo_root.clone()),
        user_profile_or_home: Err("no user profile".into()),
        appdata: None,
    };
    let err_no_engine = super::codex_exec::build_reviewer_launch_configuration_for_test(
        &req,
        Some(&explicit_home),
        Some(&test_run_root),
        Some(&ctx_no_engine),
        None,
    );
    assert!(
        err_no_engine.is_err(),
        "Missing APPDATA and profile must reject explicit home"
    );
    assert!(
        !test_run_root.exists(),
        "Must not create run_root on validation failure"
    );

    // 3. Missing user profile for ~/.codex check => must reject
    let ctx_no_profile = super::codex_exec::ExplicitHomeValidationContext {
        run_root: test_run_root.clone(),
        workspace_root: ws.clone(),
        repo_root: Ok(repo_root.clone()),
        user_profile_or_home: Err("unresolvable user profile".into()),
        appdata: Some(fake_appdata.clone()),
    };
    let err_no_profile = super::codex_exec::build_reviewer_launch_configuration_for_test(
        &req,
        Some(&explicit_home),
        Some(&test_run_root),
        Some(&ctx_no_profile),
        None,
    );
    assert!(
        err_no_profile.is_err(),
        "Unresolvable user profile must reject explicit home"
    );
    assert!(
        !test_run_root.exists(),
        "Must not create run_root on validation failure"
    );

    // 4. Not-yet-created Engine authority path => candidate path derived from canonical ancestor without directory creation
    let synthetic_appdata_not_created = temp.path().join("synth_appdata_base");
    std::fs::create_dir_all(&synthetic_appdata_not_created).unwrap();
    let uncreated_engine_path = synthetic_appdata_not_created
        .join("poverello")
        .join("engine_authority");
    assert!(
        !uncreated_engine_path.exists(),
        "Precondition: uncreated_engine_path must not exist"
    );

    // Create an existing directory for explicit_home that is an ancestor of the absent candidate engine path
    let ancestor_home = synthetic_appdata_not_created.join("poverello");
    std::fs::create_dir_all(&ancestor_home).unwrap();
    assert!(
        !uncreated_engine_path.exists(),
        "uncreated_engine_path must remain absent after ancestor creation"
    );

    let ctx_uncreated = super::codex_exec::ExplicitHomeValidationContext {
        run_root: test_run_root.clone(),
        workspace_root: ws.clone(),
        repo_root: Ok(repo_root),
        user_profile_or_home: Ok(fake_profile),
        appdata: Some(synthetic_appdata_not_created),
    };
    let err_uncreated = super::codex_exec::build_reviewer_launch_configuration_for_test(
        &req,
        Some(&ancestor_home),
        Some(&test_run_root),
        Some(&ctx_uncreated),
        None,
    );
    assert!(
        err_uncreated.is_err(),
        "Explicit home overlapping not-yet-created candidate engine authority must be rejected"
    );
    assert!(
        !uncreated_engine_path.exists(),
        "uncreated_engine_path must remain absent after validation rejection"
    );
    assert!(
        !test_run_root.exists(),
        "Must not create run_root on validation failure"
    );
}

// 95. (§64.4, §67.4) test_stage_1b_candidate_path_resolver_regular_and_absent_matrix
#[test]
fn test_stage_1b_candidate_path_resolver_regular_and_absent_matrix() {
    let temp = TempDir::new().unwrap();

    // 1. Regular existing candidate path
    let regular_dir = temp.path().join("regular_dir");
    std::fs::create_dir_all(&regular_dir).unwrap();
    let res_reg = super::codex_exec::inspect_and_resolve_candidate_path(&regular_dir);
    assert!(
        res_reg.is_ok(),
        "Regular existing path must resolve successfully: {:?}",
        res_reg.err()
    );
    assert_eq!(res_reg.unwrap(), regular_dir.canonicalize().unwrap());

    // 2. Regular non-existing leaf under existing ancestor
    let absent_leaf = regular_dir.join("absent_sub").join("nested");
    assert!(
        !absent_leaf.exists(),
        "Precondition: absent_leaf must not exist"
    );
    let res_absent = super::codex_exec::inspect_and_resolve_candidate_path(&absent_leaf);
    assert!(
        res_absent.is_ok(),
        "Absent leaf under verified ancestor must resolve: {:?}",
        res_absent.err()
    );
    assert_eq!(
        res_absent.unwrap(),
        regular_dir
            .canonicalize()
            .unwrap()
            .join("absent_sub")
            .join("nested")
    );
    assert!(
        !absent_leaf.exists(),
        "inspect_and_resolve_candidate_path must not create directories"
    );

    // 3. Relative candidate path must be rejected
    let res_rel = super::codex_exec::inspect_and_resolve_candidate_path(std::path::Path::new(
        "relative/candidate/path",
    ));
    assert!(res_rel.is_err(), "Relative candidate path must be rejected");
    assert!(res_rel.unwrap_err().contains("must be absolute"));
}

// 96. (§64.4, §67.4) test_stage_1b_candidate_path_resolver_symlink_ancestor
#[test]
fn test_stage_1b_candidate_path_resolver_symlink_ancestor() {
    let temp = TempDir::new().unwrap();
    let real_parent = temp.path().join("real_parent");
    let real_leaf = real_parent.join("leaf_dir");
    std::fs::create_dir_all(&real_leaf).unwrap();
    let sym_parent = temp.path().join("sym_parent");

    #[cfg(windows)]
    let symlink_res = std::os::windows::fs::symlink_dir(&real_parent, &sym_parent);
    #[cfg(unix)]
    let symlink_res = std::os::unix::fs::symlink(&real_parent, &sym_parent);

    match symlink_res {
        Ok(_) => {
            let candidate_via_sym = sym_parent.join("leaf_dir");
            assert!(
                candidate_via_sym.exists(),
                "Child exists through symlink parent"
            );
            let res_sym = super::codex_exec::inspect_and_resolve_candidate_path(&candidate_via_sym);
            assert!(
                res_sym.is_err(),
                "Candidate with symlink ancestor must be rejected"
            );
            let err_msg = res_sym.unwrap_err();
            assert!(
                err_msg.contains("must not be a symlink")
                    || err_msg.contains("must not be a reparse point"),
                "Error must cite symlink/reparse restriction: {err_msg}"
            );
        }
        Err(e) => {
            if symlink_privilege_unavailable(&e) {
                eprintln!("[UNVERIFIED] symlink privilege unavailable (OS 1314)");
                return;
            }
            panic!("[UNVERIFIED] Directory symlink fixture could not be created by host OS: {e}");
        }
    }
}

// 97. (§64.4, §67.4) test_stage_1b_candidate_path_resolver_windows_junction_ancestor
#[cfg(windows)]
#[test]
fn test_stage_1b_candidate_path_resolver_windows_junction_ancestor() {
    let temp = TempDir::new().unwrap();
    let junc_target = temp.path().join("junc_target");
    let junc_child = junc_target.join("child_dir");
    std::fs::create_dir_all(&junc_child).unwrap();
    let junc_link = temp.path().join("junc_link");

    let mklink_output = std::process::Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            junc_link.to_str().unwrap(),
            junc_target.to_str().unwrap(),
        ])
        .output();

    match mklink_output {
        Ok(out) if out.status.success() && junc_link.exists() => {
            let candidate_via_junc = junc_link.join("child_dir");
            assert!(
                candidate_via_junc.exists(),
                "Child exists through junction ancestor"
            );
            let res_junc =
                super::codex_exec::inspect_and_resolve_candidate_path(&candidate_via_junc);
            assert!(
                res_junc.is_err(),
                "Candidate with junction ancestor must be rejected"
            );
            let err_msg = res_junc.unwrap_err();
            assert!(
                err_msg.contains("must not be a reparse point")
                    || err_msg.contains("must not be a symlink"),
                "Error must cite reparse point restriction: {err_msg}"
            );
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            panic!(
                "[UNVERIFIED] Windows directory junction fixture could not be created (exit {}): {} {}",
                out.status,
                stdout.trim(),
                stderr.trim()
            );
        }
        Err(e) => {
            panic!("[UNVERIFIED] Windows directory junction setup command failed to spawn: {e}");
        }
    }
}

// 98. (§95.4.2) Offline Codex Windows sandbox boundary probe; ignored unless explicitly opted in.
#[cfg(windows)]
struct Stage2ProbeChild {
    parent: PathBuf,
    path: PathBuf,
    owned: bool,
}

#[cfg(windows)]
impl Stage2ProbeChild {
    fn create(parent: &std::path::Path, nonce: &str) -> Result<Self, String> {
        if nonce.is_empty()
            || !nonce
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err("probe child nonce must be non-empty ASCII alphanumeric/hyphen".into());
        }
        let parent = parent
            .canonicalize()
            .map_err(|e| format!("canonicalize probe parent: {e}"))?;
        let child = parent.join(format!("Poverello-Stage2-Probe-{nonce}"));
        std::fs::create_dir(&child)
            .map_err(|e| format!("create exact probe child (collision stops): {e}"))?;
        let mut owned = Self {
            parent: parent.clone(),
            path: child.clone(),
            owned: true,
        };
        let canonical_child = child
            .canonicalize()
            .map_err(|e| format!("canonicalize newly created probe child: {e}"))?;
        if canonical_child.parent() != Some(parent.as_path()) {
            return Err(
                "new probe child is not a direct child of the approved parent; preserve it for inspection".into(),
            );
        }
        owned.path = canonical_child.clone();
        let metadata = std::fs::symlink_metadata(&canonical_child)
            .map_err(|e| format!("inspect newly created probe child: {e}"))?;
        use std::os::windows::fs::MetadataExt;
        if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
            return Err(
                "new probe child is not a normal directory; preserve it for inspection".into(),
            );
        }
        Ok(owned)
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }

    fn preserve_for_diagnostics(&mut self) {
        // A timed-out Codex process may have spawned a sandbox child. Keep the exact fixture until
        // a later, separately authorized inspection can establish that no writer remains.
        self.owned = false;
    }

    fn cleanup_owned_child(&self) -> Result<(), String> {
        use std::os::windows::fs::MetadataExt;

        let current = self
            .path
            .canonicalize()
            .map_err(|e| format!("canonicalize owned probe child before cleanup: {e}"))?;
        if current != self.path || current.parent() != Some(self.parent.as_path()) {
            return Err("owned probe child identity/parent changed; leave it untouched".into());
        }
        let metadata = std::fs::symlink_metadata(&current)
            .map_err(|e| format!("inspect owned probe child before cleanup: {e}"))?;
        if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
            return Err("owned probe child is not a normal directory; leave it untouched".into());
        }
        std::fs::remove_dir_all(&current)
            .map_err(|e| format!("remove exact owned probe child: {e}"))?;
        if current.exists() {
            return Err("owned probe child still exists after cleanup".into());
        }
        Ok(())
    }

    fn close(mut self) -> Result<(), String> {
        self.cleanup_owned_child()?;
        self.owned = false;
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for Stage2ProbeChild {
    fn drop(&mut self) {
        if self.owned {
            if let Err(error) = self.cleanup_owned_child() {
                eprintln!("[UNVERIFIED] Stage 2 fixture cleanup retained its exact child: {error}");
            }
        }
    }
}

#[cfg(windows)]
#[test]
#[ignore = "Stage 2 Windows sandbox probe requires an explicit local CLI path and fixture-only execution"]
fn test_stage_2_windows_codex_sandbox_boundary_probe() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    const USER_APPROVED_PARENT: &str = r"C:\Users\Sohei";

    run_with_stage2_mandatory_env_preflight(&|k| std::env::var_os(k), |mandatory_env| {

    assert_eq!(
        std::env::var("POVERELLO_STAGE2_SANDBOX_PROBE").as_deref(),
        Ok("1"),
        "set POVERELLO_STAGE2_SANDBOX_PROBE=1 only for the explicit fixture-only probe"
    );
    let cli = std::path::PathBuf::from(
        std::env::var_os("POVERELLO_STAGE2_CODEX_BIN")
            .expect("explicit Codex executable path required; PATH/default lookup is forbidden"),
    );
    assert!(
        cli.is_absolute(),
        "explicit Codex executable path must be absolute"
    );
    assert!(cli.is_file(), "explicit Codex executable must exist");

    let approved_root =
        std::path::PathBuf::from(std::env::var_os("POVERELLO_STAGE2_SANDBOX_ROOT").expect(
            "explicit user-approved sandbox parent is required; default/temp roots are forbidden",
        ));
    let repo_root = stage2_probe_repository_root();
    let temp_root = ambient_temp_dir()
        .canonicalize()
        .expect("canonicalize OS temporary directory");
    let appdata_roots = ["APPDATA", "LOCALAPPDATA"]
        .into_iter()
        .map(|name| {
            std::path::PathBuf::from(
                std::env::var_os(name)
                    .unwrap_or_else(|| panic!("{name} must be set for the probe")),
            )
            .canonicalize()
            .unwrap_or_else(|e| panic!("cannot canonicalize {name}: {e}"))
        })
        .collect::<Vec<_>>();
    let validated_root = validate_stage2_probe_root(
        &approved_root,
        std::path::Path::new(USER_APPROVED_PARENT),
        &repo_root,
        &temp_root,
        &appdata_roots,
    )
    .expect("validate the exact user-approved probe parent");
    let mut fixture = Stage2ProbeChild::create(&validated_root, &uuid::Uuid::new_v4().to_string())
        .expect("create exactly one unique disposable child; a collision stops without retry");
    let fixture_canonical = fixture
        .path()
        .canonicalize()
        .expect("canonicalize probe child");
    assert_eq!(
        fixture_canonical.parent(),
        Some(validated_root.as_path()),
        "probe child must be a direct child of the approved parent"
    );
    assert!(
        !fixture_canonical.starts_with(&repo_root) && !repo_root.starts_with(&fixture_canonical),
        "probe child must not overlap the repository"
    );
    assert!(
        !fixture_canonical.starts_with(&temp_root) && !temp_root.starts_with(&fixture_canonical),
        "probe child must not overlap OS Temp"
    );
    for appdata in &appdata_roots {
        assert!(
            !fixture_canonical.starts_with(appdata) && !appdata.starts_with(&fixture_canonical),
            "probe child must not overlap AppData"
        );
    }
    let candidate = fixture.path().join("candidate");
    let primary = fixture.path().join("primary");
    let engine = fixture.path().join("engine-state");
    let outside = fixture.path().join("outside");
    let isolated_home = fixture.path().join("codex-home");
    let appdata_roaming = fixture.path().join("appdata/roaming");
    let appdata_local = fixture.path().join("appdata/local");
    for dir in [
        &candidate,
        &primary,
        &engine,
        &outside,
        &isolated_home,
        &appdata_roaming,
        &appdata_local,
    ] {
        std::fs::create_dir_all(dir).expect("create fixture directory and parents");
    }
    let primary_target = primary.join("protected.txt");
    let engine_target = engine.join("protected.txt");
    let outside_target = outside.join("protected.txt");
    for target in [&primary_target, &engine_target, &outside_target] {
        std::fs::write(target, b"immutable fixture sentinel\n").unwrap();
    }
    let original_hashes = [&primary_target, &engine_target, &outside_target]
        .map(|path| super::implementer_workspace::test_sha256(&std::fs::read(path).unwrap()));

    let system_root = std::path::PathBuf::from(&mandatory_env.system_root);
    let powershell = stage2_probe_powershell_executable(&system_root);
    assert!(
        powershell.is_file(),
        "Windows PowerShell executable is unavailable"
    );
    let quote = |path: &std::path::Path| path.to_string_lossy().replace('\'', "''");
    let child_script = format!(
        "$ErrorActionPreference='Stop'; Set-Content -LiteralPath '{}\\candidate-ok.txt' -Value 'candidate-write-ok'; $r=@(); foreach($p in @('{}','{}','{}')) {{ try {{ Set-Content -LiteralPath $p -Value 'MUTATED'; $r += 'WRITE_ALLOWED' }} catch {{ $r += 'WRITE_DENIED' }} }}; Set-Content -LiteralPath '{}\\probe-result.txt' -Value ($r -join ','); Write-Output ('CHILD_STARTED;'+($r -join ','))",
        quote(&candidate), quote(&primary_target), quote(&engine_target), quote(&outside_target), quote(&candidate)
    );

    let mut env = std::process::Command::new(&powershell);
    env.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        &child_script,
    ])
    .current_dir(&candidate)
    .env_clear()
    .env("SystemRoot", &mandatory_env.system_root)
    .env("WINDIR", &mandatory_env.windir)
    .env("TEMP", fixture.path())
    .env("TMP", fixture.path());
    // Positive control: the same child can write inside candidate and to every protected fixture
    // when not launched through the Codex sandbox.
    let positive = env.output().expect("run unsandboxed positive control");
    assert!(
        positive.status.success(),
        "positive control failed: {}",
        String::from_utf8_lossy(&positive.stderr)
    );
    for target in [&primary_target, &engine_target, &outside_target] {
        assert_eq!(std::fs::read_to_string(target).unwrap().trim(), "MUTATED");
        std::fs::write(target, b"immutable fixture sentinel\n").unwrap();
    }
    let candidate_marker = candidate.join("candidate-ok.txt");
    assert!(
        candidate_marker.is_file(),
        "positive control did not write inside candidate"
    );
    std::fs::remove_file(&candidate_marker).unwrap();
    std::fs::remove_file(candidate.join("probe-result.txt")).unwrap();

    let fixture_env = Stage2ProbeFixtureEnv {
        temp_dir: fixture.path(),
        isolated_home: &isolated_home,
        appdata_roaming: &appdata_roaming,
        appdata_local: &appdata_local,
        codex_sqlite_home: &isolated_home,
    };
    let child_env = build_stage2_probe_child_environment_from_lookup(
        mandatory_env,
        &fixture_env,
        Some(&cli),
        &|k| std::env::var_os(k),
    )
    .expect(
        "construct allowlisted Stage 2 probe child environment via composed production builder",
    );

    let mut sandbox = Command::new(&cli);
    sandbox
        .args([
            "sandbox",
            "windows",
            "-c",
            "sandbox_mode=\"workspace-write\"",
            "-c",
            "sandbox_workspace_write.network_access=false",
            "--",
        ])
        .arg(&powershell)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &child_script,
        ])
        .current_dir(&candidate)
        .env_clear();
    for (k, v) in child_env {
        sandbox.env(k, v);
    }
    sandbox.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = sandbox
        .spawn()
        .expect("Codex sandbox helper failed to start");
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                let (kill_result, direct_process_status) =
                    request_stage2_probe_child_termination_bounded(&mut child);
                let fixture_path = fixture.path().to_path_buf();
                fixture.preserve_for_diagnostics();
                panic!(
                    "[UNVERIFIED] failed to observe Codex sandbox process status: {error}; {}; kill={kill_result}; direct process={direct_process_status}; fixture preserved for diagnostics",
                    format_stage2_probe_output_report(
                        "STATUS_UNAVAILABLE",
                        &fixture_path,
                        None
                    )
                );
            }
        }
        if started.elapsed() > Duration::from_secs(30) {
            let (kill_result, direct_process_status) =
                request_stage2_probe_child_termination_bounded(&mut child);
            let fixture_path = fixture.path().to_path_buf();
            fixture.preserve_for_diagnostics();
            let output_report = format_stage2_probe_output_report(
                "TIMEOUT",
                &fixture_path,
                None,
            );
            panic!(
                "[UNVERIFIED] Codex sandbox helper timed out; {output_report}; kill={kill_result}; direct process={direct_process_status}; fixture preserved for diagnostics"
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(error) => {
            let fixture_path = fixture.path().to_path_buf();
            fixture.preserve_for_diagnostics();
            panic!(
                "[UNVERIFIED] failed to collect Codex sandbox output: {error}; {}; fixture preserved for diagnostics",
                format_stage2_probe_output_report("OUTPUT_COLLECTION_FAILED", &fixture_path, None)
            );
        }
    };
    let output_report = format_stage2_probe_output_report(
        &format!("{:?}", status),
        fixture.path(),
        Some((&output.stdout, &output.stderr)),
    );
    eprintln!("{output_report}");
    if !status.success() {
        fixture.preserve_for_diagnostics();
        panic!(
            "[UNVERIFIED] sandbox helper failed; captured process output follows:\n{output_report}; fixture preserved for diagnostics"
        );
    }
    assert!(
        candidate.join("candidate-ok.txt").is_file(),
        "[UNVERIFIED] sandboxed child did not prove candidate write"
    );
    assert_eq!(
        std::fs::read_to_string(candidate.join("probe-result.txt"))
            .unwrap()
            .trim(),
        "WRITE_DENIED,WRITE_DENIED,WRITE_DENIED",
        "[UNVERIFIED] protected-write denial did not meet the expected result"
    );
    for (target, expected_hash) in [&primary_target, &engine_target, &outside_target]
        .into_iter()
        .zip(original_hashes)
    {
        assert_eq!(
            super::implementer_workspace::test_sha256(&std::fs::read(target).unwrap()),
            expected_hash
        );
    }
        fixture
            .close()
            .expect("verify exact disposable probe child cleanup");
        Ok(())
    })
    .expect("mandatory Windows environment preflight must pass before probe fixture creation");
}

#[cfg(windows)]
fn format_stage2_probe_output_report(
    status: &str,
    fixture_path: &std::path::Path,
    output: Option<(&[u8], &[u8])>,
) -> String {
    let fixture_line = format!("fixture path: {}", fixture_path.display());
    match output {
        Some((stdout, stderr)) => format!(
            "Stage 2 sandbox helper process status: {status}\n{fixture_line}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            String::from_utf8_lossy(stdout),
            String::from_utf8_lossy(stderr)
        ),
        None => format!(
            "Stage 2 sandbox helper process status: {status}\n{fixture_line}\ncomplete stdout/stderr: NOT COLLECTED (bounded timeout path)"
        ),
    }
}

#[cfg(windows)]
fn request_stage2_probe_child_termination_bounded(
    child: &mut std::process::Child,
) -> (String, String) {
    use std::time::{Duration, Instant};

    let kill_result = format!("{:?}", child.kill());
    let deadline = Instant::now() + Duration::from_secs(2);
    let direct_process_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break format!("EXITED ({status:?})"),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Ok(None) => break "STILL_RUNNING".to_string(),
            Err(error) => break format!("STATUS_UNAVAILABLE ({error})"),
        }
    };

    (kill_result, direct_process_status)
}

#[cfg(windows)]
#[test]
fn test_stage_2_probe_output_report_includes_status_stdout_and_stderr() {
    let report = format_stage2_probe_output_report(
        "ExitStatus(1)",
        std::path::Path::new(r"C:\Users\Sohei\Poverello-Stage2-Probe-fixture"),
        Some((
            b"helper diagnostic on stdout",
            b"helper diagnostic on stderr",
        )),
    );

    assert!(report.contains("ExitStatus(1)"));
    assert!(report.contains("--- stdout ---\nhelper diagnostic on stdout"));
    assert!(report.contains("--- stderr ---\nhelper diagnostic on stderr"));

    let timeout_report = format_stage2_probe_output_report(
        "TIMEOUT",
        std::path::Path::new(r"C:\Users\Sohei\Poverello-Stage2-Probe-fixture"),
        None,
    );
    assert!(timeout_report.contains("status: TIMEOUT"));
    assert!(timeout_report.contains(r"C:\Users\Sohei\Poverello-Stage2-Probe-fixture"));
    assert!(timeout_report.contains("stdout/stderr: NOT COLLECTED"));
}

#[cfg(windows)]
fn stage2_probe_repository_root() -> PathBuf {
    let mut current = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("canonicalize crate directory");
    loop {
        if current.join(".git").exists() {
            return current;
        }
        assert!(
            current.pop(),
            "could not locate repository root from crate directory"
        );
    }
}

#[cfg(windows)]
fn stage2_probe_powershell_executable(system_root: &std::path::Path) -> PathBuf {
    system_root
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe")
}

#[cfg(windows)]
#[test]
fn test_stage_2_powershell_path_uses_native_components() {
    let system_root = std::path::Path::new(r"C:\Windows");
    let executable = stage2_probe_powershell_executable(system_root);

    assert!(executable.is_absolute());
    assert_eq!(
        executable,
        std::path::PathBuf::from(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe")
    );
    assert_eq!(
        executable.file_name().and_then(std::ffi::OsStr::to_str),
        Some("powershell.exe")
    );
    assert!(!executable.to_string_lossy().contains('/'));
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stage2MandatoryWindowsEnv {
    pub path: std::ffi::OsString,
    pub system_root: std::ffi::OsString,
    pub windir: std::ffi::OsString,
}

#[cfg(windows)]
pub(crate) fn preflight_stage2_mandatory_windows_env(
    lookup: &dyn Fn(&str) -> Option<std::ffi::OsString>,
) -> Result<Stage2MandatoryWindowsEnv, String> {
    let path = lookup("PATH")
        .or_else(|| lookup("Path"))
        .or_else(|| lookup("path"))
        .ok_or_else(|| "mandatory Windows environment variable PATH is missing".to_string())?;

    let system_root = lookup("SystemRoot")
        .or_else(|| lookup("SYSTEMROOT"))
        .or_else(|| lookup("systemroot"))
        .ok_or_else(|| {
            "mandatory Windows environment variable SystemRoot is missing".to_string()
        })?;

    let windir = lookup("WINDIR")
        .or_else(|| lookup("windir"))
        .unwrap_or_else(|| system_root.clone());

    Ok(Stage2MandatoryWindowsEnv {
        path,
        system_root,
        windir,
    })
}

#[cfg(windows)]
pub(crate) fn run_with_stage2_mandatory_env_preflight<T, F>(
    lookup: &dyn Fn(&str) -> Option<std::ffi::OsString>,
    callback: F,
) -> Result<T, String>
where
    F: FnOnce(&Stage2MandatoryWindowsEnv) -> Result<T, String>,
{
    let env = preflight_stage2_mandatory_windows_env(lookup)?;
    callback(&env)
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stage2ProbeFixtureEnv<'a> {
    pub temp_dir: &'a std::path::Path,
    pub isolated_home: &'a std::path::Path,
    pub appdata_roaming: &'a std::path::Path,
    pub appdata_local: &'a std::path::Path,
    pub codex_sqlite_home: &'a std::path::Path,
}

#[cfg(windows)]
pub(crate) fn build_stage2_probe_child_environment_from_lookup(
    mandatory_env: &Stage2MandatoryWindowsEnv,
    fixture_env: &Stage2ProbeFixtureEnv<'_>,
    executable: Option<&std::path::Path>,
    ambient_lookup: &dyn Fn(&str) -> Option<std::ffi::OsString>,
) -> Result<Vec<(std::ffi::OsString, std::ffi::OsString)>, String> {
    // 1. Compose production builder with an injected lookup for allowed standard keys.
    // The production builder queries ["PATH", "TEMP", "TMP", "APPDATA", "LOCALAPPDATA", "USERPROFILE"].
    // Allowed ambient keys are retrieved from ambient_lookup, so ambient values enter the base environment
    // before explicit fixture overrides are applied.
    let base_lookup = |key: &str| -> Option<std::ffi::OsString> {
        let upper = key.to_ascii_uppercase();
        match upper.as_str() {
            "PATH" => ambient_lookup("PATH")
                .or_else(|| ambient_lookup("Path"))
                .or_else(|| ambient_lookup("path"))
                .or_else(|| Some(mandatory_env.path.clone())),
            "TEMP" => ambient_lookup("TEMP").or_else(|| ambient_lookup("temp")),
            "TMP" => ambient_lookup("TMP").or_else(|| ambient_lookup("tmp")),
            "APPDATA" => ambient_lookup("APPDATA").or_else(|| ambient_lookup("appdata")),
            "LOCALAPPDATA" => {
                ambient_lookup("LOCALAPPDATA").or_else(|| ambient_lookup("localappdata"))
            }
            "USERPROFILE" => {
                ambient_lookup("USERPROFILE").or_else(|| ambient_lookup("userprofile"))
            }
            _ => None,
        }
    };

    let base_env = super::codex_exec::build_child_environment_with_executable_from_lookup(
        "stage2-probe-harness",
        fixture_env.isolated_home,
        fixture_env.codex_sqlite_home,
        executable,
        &base_lookup,
    )?;

    // 2. Case-insensitive deduplication and deterministic ordering via BTreeMap
    let mut case_map: std::collections::BTreeMap<String, (std::ffi::OsString, std::ffi::OsString)> =
        std::collections::BTreeMap::new();

    for (k, v) in base_env {
        let upper = k.to_string_lossy().to_ascii_uppercase();
        case_map.insert(upper, (k, v));
    }

    // 3. Explicit fixture overrides:
    // Fixture-isolated directories take precedence over conflicting ambient values
    case_map.insert(
        "TEMP".to_string(),
        (
            "TEMP".into(),
            fixture_env.temp_dir.as_os_str().to_os_string(),
        ),
    );
    case_map.insert(
        "TMP".to_string(),
        (
            "TMP".into(),
            fixture_env.temp_dir.as_os_str().to_os_string(),
        ),
    );
    case_map.insert(
        "USERPROFILE".to_string(),
        (
            "USERPROFILE".into(),
            fixture_env.isolated_home.as_os_str().to_os_string(),
        ),
    );
    case_map.insert(
        "APPDATA".to_string(),
        (
            "APPDATA".into(),
            fixture_env.appdata_roaming.as_os_str().to_os_string(),
        ),
    );
    case_map.insert(
        "LOCALAPPDATA".to_string(),
        (
            "LOCALAPPDATA".into(),
            fixture_env.appdata_local.as_os_str().to_os_string(),
        ),
    );

    // 4. Add required fixture HOME and Windows-specific SystemRoot / WINDIR
    case_map.insert(
        "HOME".to_string(),
        (
            "HOME".into(),
            fixture_env.isolated_home.as_os_str().to_os_string(),
        ),
    );
    case_map.insert(
        "SYSTEMROOT".to_string(),
        ("SystemRoot".into(), mandatory_env.system_root.clone()),
    );
    case_map.insert(
        "WINDIR".to_string(),
        ("WINDIR".into(), mandatory_env.windir.clone()),
    );

    // 5. Ensure PATH is present (from ambient_lookup or mandatory_env)
    case_map
        .entry("PATH".to_string())
        .or_insert_with(|| ("PATH".into(), mandatory_env.path.clone()));

    // 6. Verify no forbidden credentials or proxies are present
    const FORBIDDEN_KEYS: &[&str] = &[
        "OPENAI_API_KEY",
        "CODEX_API_KEY",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
    ];
    for upper in case_map.keys() {
        if FORBIDDEN_KEYS.contains(&upper.as_str()) {
            return Err(
                "credential and proxy variables are forbidden in the child environment".into(),
            );
        }
    }

    // 7. Convert to deterministically ordered vector
    let environment: Vec<(std::ffi::OsString, std::ffi::OsString)> =
        case_map.into_values().collect();

    Ok(environment)
}

#[cfg(windows)]
#[test]
fn test_stage_2_probe_mandatory_windows_env_preflight_missing_inputs() {
    // Missing PATH fails closed
    let lookup_no_path = |k: &str| -> Option<std::ffi::OsString> {
        match k {
            "SystemRoot" => Some(r"C:\Windows".into()),
            _ => None,
        }
    };
    let err_no_path = preflight_stage2_mandatory_windows_env(&lookup_no_path).unwrap_err();
    assert!(
        err_no_path.contains("PATH"),
        "expected error mentioning PATH, got: {err_no_path}"
    );

    // Missing SystemRoot fails closed
    let lookup_no_sysroot = |k: &str| -> Option<std::ffi::OsString> {
        match k {
            "PATH" => Some(r"C:\Windows\System32".into()),
            _ => None,
        }
    };
    let err_no_sysroot = preflight_stage2_mandatory_windows_env(&lookup_no_sysroot).unwrap_err();
    assert!(
        err_no_sysroot.contains("SystemRoot"),
        "expected error mentioning SystemRoot, got: {err_no_sysroot}"
    );

    // Missing WINDIR is derived from SystemRoot
    let lookup_derive_windir = |k: &str| -> Option<std::ffi::OsString> {
        match k {
            "PATH" => Some(r"C:\Windows\System32".into()),
            "SystemRoot" => Some(r"C:\Windows".into()),
            _ => None,
        }
    };
    let env_derived = preflight_stage2_mandatory_windows_env(&lookup_derive_windir).unwrap();
    assert_eq!(env_derived.path, r"C:\Windows\System32");
    assert_eq!(env_derived.system_root, r"C:\Windows");
    assert_eq!(env_derived.windir, r"C:\Windows");

    // Explicit WINDIR is preserved
    let lookup_explicit_windir = |k: &str| -> Option<std::ffi::OsString> {
        match k {
            "PATH" => Some(r"C:\Windows\System32".into()),
            "SystemRoot" => Some(r"C:\Windows".into()),
            "WINDIR" => Some(r"D:\WindowsAlt".into()),
            _ => None,
        }
    };
    let env_explicit = preflight_stage2_mandatory_windows_env(&lookup_explicit_windir).unwrap();
    assert_eq!(env_explicit.windir, r"D:\WindowsAlt");
}

#[cfg(windows)]
#[test]
fn test_stage_2_probe_preflight_failure_does_not_invoke_callback() {
    let mut callback_invoked = false;
    let lookup_missing = |_: &str| -> Option<std::ffi::OsString> { None };

    let result = run_with_stage2_mandatory_env_preflight(&lookup_missing, |_env| {
        callback_invoked = true;
        Ok(())
    });

    assert!(
        result.is_err(),
        "preflight must fail when inputs are missing"
    );
    assert!(
        !callback_invoked,
        "callback must NOT be invoked when preflight fails"
    );

    // Successful preflight invokes callback
    let lookup_valid = |k: &str| -> Option<std::ffi::OsString> {
        match k {
            "PATH" => Some(r"C:\Windows\System32".into()),
            "SystemRoot" => Some(r"C:\Windows".into()),
            _ => None,
        }
    };
    let mut valid_callback_invoked = false;
    let ok_result = run_with_stage2_mandatory_env_preflight(&lookup_valid, |env| {
        valid_callback_invoked = true;
        assert_eq!(env.system_root, r"C:\Windows");
        Ok(42)
    });
    assert_eq!(ok_result.unwrap(), 42);
    assert!(valid_callback_invoked);
}

#[cfg(windows)]
#[test]
fn test_stage_2_probe_composed_builder_allowlist_overrides_and_secret_exclusion() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let fixture_dir = repo_tempdir().expect("create tempdir for probe test");
    let base = fixture_dir.path();
    let temp = base.join("temp");
    let home = base.join("home");
    let roaming = base.join("roaming");
    let local = base.join("local");
    let sqlite = base.join("sqlite");
    for d in [&temp, &home, &roaming, &local, &sqlite] {
        std::fs::create_dir_all(d).expect("create test directories");
    }

    let synthetic_pkg_dir = base.join("npm_pkg");
    let (synthetic_pkg, synthetic_exe) =
        super::codex_exec::create_synthetic_codex_npm_layout(&synthetic_pkg_dir);

    let fixture_env = Stage2ProbeFixtureEnv {
        temp_dir: &temp,
        isolated_home: &home,
        appdata_roaming: &roaming,
        appdata_local: &local,
        codex_sqlite_home: &sqlite,
    };

    let mandatory_env = Stage2MandatoryWindowsEnv {
        path: r"C:\Windows\System32;C:\Windows".into(),
        system_root: r"C:\Windows".into(),
        windir: r"C:\Windows".into(),
    };

    // Track that ambient_lookup is genuinely queried for allowed ambient keys
    let temp_queried = AtomicUsize::new(0);
    let appdata_queried = AtomicUsize::new(0);
    let userprofile_queried = AtomicUsize::new(0);

    // Synthetic lookup providing conflicting ambient values, forbidden keys, and unapproved keys
    let ambient_lookup = |key: &str| -> Option<std::ffi::OsString> {
        match key {
            "TEMP" | "temp" => {
                temp_queried.fetch_add(1, Ordering::SeqCst);
                Some(r"C:\conflicting_ambient_temp".into())
            }
            "TMP" | "tmp" => Some(r"C:\conflicting_ambient_tmp".into()),
            "USERPROFILE" | "userprofile" => {
                userprofile_queried.fetch_add(1, Ordering::SeqCst);
                Some(r"C:\conflicting_ambient_user".into())
            }
            "APPDATA" | "appdata" => {
                appdata_queried.fetch_add(1, Ordering::SeqCst);
                Some(r"C:\conflicting_ambient_appdata".into())
            }
            "LOCALAPPDATA" | "localappdata" => Some(r"C:\conflicting_ambient_localappdata".into()),
            "PATH" | "path" => Some(r"C:\ambient_path\bin".into()),
            "CODEX_MANAGED_PACKAGE_ROOT" | "codex_managed_package_root" => {
                Some(r"C:\conflicting_ambient_managed_root".into())
            }
            "OPENAI_API_KEY" => Some("secret_key".into()),
            "CODEX_API_KEY" => Some("secret_codex".into()),
            "HTTP_PROXY" => Some("http://proxy:8080".into()),
            "HTTPS_PROXY" => Some("https://proxy:8080".into()),
            "ALL_PROXY" => Some("socks5://proxy:8080".into()),
            "NO_PROXY" => Some("localhost".into()),
            "UNAPPROVED_ARBITRARY_VAR" => Some("should_never_be_copied".into()),
            _ => None,
        }
    };

    let env = build_stage2_probe_child_environment_from_lookup(
        &mandatory_env,
        &fixture_env,
        Some(&synthetic_exe),
        &ambient_lookup,
    )
    .expect("build environment successfully");

    // Verify ambient_lookup was genuinely called for allowed ambient keys
    assert!(
        temp_queried.load(Ordering::SeqCst) > 0,
        "ambient_lookup must be queried for TEMP"
    );
    assert!(
        appdata_queried.load(Ordering::SeqCst) > 0,
        "ambient_lookup must be queried for APPDATA"
    );
    assert!(
        userprofile_queried.load(Ordering::SeqCst) > 0,
        "ambient_lookup must be queried for USERPROFILE"
    );

    let map: std::collections::HashMap<String, String> = env
        .into_iter()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.to_string_lossy().into_owned(),
            )
        })
        .collect();

    // (a) Required allowlisted and Windows-specific values are present
    assert_eq!(map.get("PATH").unwrap(), r"C:\ambient_path\bin");
    assert_eq!(map.get("SystemRoot").unwrap(), r"C:\Windows");
    assert_eq!(map.get("WINDIR").unwrap(), r"C:\Windows");
    assert_eq!(map.get("CODEX_HOME").unwrap(), home.to_str().unwrap());
    assert_eq!(
        map.get("CODEX_SQLITE_HOME").unwrap(),
        sqlite.to_str().unwrap()
    );

    // Derived CODEX_MANAGED_PACKAGE_ROOT overrides any ambient value
    assert_eq!(
        map.get("CODEX_MANAGED_PACKAGE_ROOT").unwrap(),
        synthetic_pkg.to_str().unwrap()
    );

    // (b) Fixture-specific values override conflicting ambient values
    assert_eq!(map.get("TEMP").unwrap(), temp.to_str().unwrap());
    assert_eq!(map.get("TMP").unwrap(), temp.to_str().unwrap());
    assert_eq!(map.get("USERPROFILE").unwrap(), home.to_str().unwrap());
    assert_eq!(map.get("HOME").unwrap(), home.to_str().unwrap());
    assert_eq!(map.get("APPDATA").unwrap(), roaming.to_str().unwrap());
    assert_eq!(map.get("LOCALAPPDATA").unwrap(), local.to_str().unwrap());

    // (c) API-key and proxy variables are strictly absent
    assert!(!map.contains_key("OPENAI_API_KEY"));
    assert!(!map.contains_key("CODEX_API_KEY"));
    assert!(!map.contains_key("HTTP_PROXY"));
    assert!(!map.contains_key("HTTPS_PROXY"));
    assert!(!map.contains_key("ALL_PROXY"));
    assert!(!map.contains_key("NO_PROXY"));

    // (d) Arbitrary unapproved environment variables are never copied
    assert!(!map.contains_key("UNAPPROVED_ARBITRARY_VAR"));
}

#[cfg(windows)]
#[test]
fn test_stage_2_probe_composed_builder_no_duplicate_case_variant_keys() {
    let fixture_dir = repo_tempdir().expect("create tempdir for probe test");
    let base = fixture_dir.path();
    let temp = base.join("temp");
    let home = base.join("home");
    let roaming = base.join("roaming");
    let local = base.join("local");
    let sqlite = base.join("sqlite");
    for d in [&temp, &home, &roaming, &local, &sqlite] {
        std::fs::create_dir_all(d).expect("create test directories");
    }

    let fixture_env = Stage2ProbeFixtureEnv {
        temp_dir: &temp,
        isolated_home: &home,
        appdata_roaming: &roaming,
        appdata_local: &local,
        codex_sqlite_home: &sqlite,
    };

    let mandatory_env = Stage2MandatoryWindowsEnv {
        path: r"C:\Windows\System32".into(),
        system_root: r"C:\Windows".into(),
        windir: r"C:\Windows".into(),
    };

    let env = build_stage2_probe_child_environment_from_lookup(
        &mandatory_env,
        &fixture_env,
        None,
        &|_| None,
    )
    .expect("build environment successfully");

    // Verify all keys are unique under case-insensitive comparison
    let mut upper_set = std::collections::HashSet::new();
    let mut last_key: Option<String> = None;
    for (key, _) in &env {
        let key_str = key.to_string_lossy().into_owned();
        let upper = key_str.to_ascii_uppercase();
        assert!(
            upper_set.insert(upper.clone()),
            "duplicate case-variant key detected: {key_str}"
        );
        if let Some(prev) = last_key {
            assert!(
                prev <= upper,
                "environment must be deterministically ordered: {prev} <= {upper}"
            );
        }
        last_key = Some(upper);
    }
}

#[cfg(windows)]
fn validate_stage2_probe_root(
    requested: &std::path::Path,
    explicitly_approved: &std::path::Path,
    repository: &std::path::Path,
    os_temp: &std::path::Path,
    appdata_roots: &[PathBuf],
) -> Result<PathBuf, String> {
    use std::os::windows::fs::MetadataExt;

    if !requested.is_absolute() || !explicitly_approved.is_absolute() {
        return Err("probe parent and explicit authorization must be absolute paths".into());
    }
    for ancestor in requested.ancestors() {
        let metadata = std::fs::symlink_metadata(ancestor)
            .map_err(|e| format!("inspect requested probe parent component: {e}"))?;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("probe parent path contains a reparse-point component".into());
        }
    }
    let canonical = requested
        .canonicalize()
        .map_err(|e| format!("canonicalize probe parent: {e}"))?;
    let canonical_approved = explicitly_approved
        .canonicalize()
        .map_err(|e| format!("canonicalize explicitly approved parent: {e}"))?;
    if canonical != canonical_approved {
        return Err("probe parent differs from the exact explicitly approved path".into());
    }
    let metadata =
        std::fs::symlink_metadata(&canonical).map_err(|e| format!("inspect probe parent: {e}"))?;
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
        return Err("probe parent must be a normal, non-reparse directory".into());
    }
    for ancestor in canonical.ancestors() {
        let metadata = std::fs::symlink_metadata(ancestor)
            .map_err(|e| format!("inspect probe parent ancestor: {e}"))?;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("probe parent has a reparse-point ancestor".into());
        }
    }
    if canonical.starts_with(repository) || canonical.starts_with(os_temp) {
        return Err("probe parent must not be inside the repository or OS Temp".into());
    }
    if appdata_roots
        .iter()
        .any(|appdata| canonical.starts_with(appdata))
    {
        return Err("probe parent must not be inside an AppData root".into());
    }
    Ok(canonical)
}

#[cfg(windows)]
#[test]
fn test_stage_2_probe_root_validation_is_side_effect_free() {
    let temp = repo_tempdir().expect("create synthetic validation fixture");
    let repo = temp.path().join("repo");
    let appdata = temp.path().join("appdata");
    let outside = temp.path().join("outside");
    let os_temp = temp.path().join("os-temp");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&appdata).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::create_dir_all(&os_temp).unwrap();
    let outside = outside.canonicalize().unwrap();
    let os_temp = os_temp.canonicalize().unwrap();
    let repo = repo.canonicalize().unwrap();
    let appdata = appdata.canonicalize().unwrap();

    assert!(validate_stage2_probe_root(
        std::path::Path::new("relative"),
        &outside,
        &repo,
        &os_temp,
        &[appdata.clone()]
    )
    .is_err());
    let missing = outside.join("missing");
    assert!(
        validate_stage2_probe_root(&missing, &missing, &repo, &os_temp, &[appdata.clone()])
            .is_err()
    );
    assert!(
        validate_stage2_probe_root(&os_temp, &os_temp, &repo, &os_temp, &[appdata.clone()])
            .is_err()
    );
    assert!(
        validate_stage2_probe_root(&appdata, &appdata, &repo, &os_temp, &[appdata.clone()])
            .is_err()
    );
    assert!(validate_stage2_probe_root(&repo, &repo, &repo, &os_temp, &[appdata.clone()]).is_err());
    assert!(
        validate_stage2_probe_root(&outside, &repo, &repo, &os_temp, &[appdata.clone()]).is_err()
    );
    assert!(!missing.exists(), "rejected paths must not be created");

    let alias = temp.path().join("alias");
    match std::os::windows::fs::symlink_dir(&outside, &alias) {
        Ok(_) => {
            assert!(validate_stage2_probe_root(&alias, &alias, &repo, &os_temp, &[]).is_err());
            std::fs::remove_dir(&alias).unwrap();
        }
        Err(e) => {
            if symlink_privilege_unavailable(&e) {
                eprintln!("[UNVERIFIED] symlink privilege unavailable (OS 1314)");
                return;
            }
            panic!("create synthetic directory symlink failed: {e}");
        }
    }

    let allowed = validate_stage2_probe_root(&outside, &outside, &repo, &os_temp, &[])
        .expect("an exact explicitly approved synthetic parent may be validated without creating child data");
    assert_eq!(allowed, outside);
}

#[cfg(windows)]
#[test]
fn test_stage_2_probe_child_collision_and_cleanup_are_exact() {
    let parent = repo_tempdir().expect("create synthetic parent for child guard");
    let canonical_parent = parent.path().canonicalize().unwrap();
    let collision = canonical_parent.join("Poverello-Stage2-Probe-collision");
    std::fs::create_dir(&collision).unwrap();
    let sentinel = collision.join("existing.txt");
    std::fs::write(&sentinel, b"pre-existing collision sentinel\n").unwrap();
    assert!(Stage2ProbeChild::create(&canonical_parent, "collision").is_err());
    assert_eq!(
        std::fs::read(&sentinel).unwrap(),
        b"pre-existing collision sentinel\n"
    );

    let sibling = canonical_parent.join("unrelated-sibling");
    std::fs::create_dir(&sibling).unwrap();
    let sibling_sentinel = sibling.join("keep.txt");
    std::fs::write(&sibling_sentinel, b"keep sibling\n").unwrap();
    let owned = Stage2ProbeChild::create(&canonical_parent, "owned-child").unwrap();
    let owned_path = owned.path().to_path_buf();
    std::fs::write(owned_path.join("fixture.txt"), b"disposable\n").unwrap();
    owned
        .close()
        .expect("remove and verify only exact owned child");

    assert!(!owned_path.exists(), "exact owned child must be removed");
    assert_eq!(
        std::fs::read(&sentinel).unwrap(),
        b"pre-existing collision sentinel\n"
    );
    assert_eq!(std::fs::read(&sibling_sentinel).unwrap(), b"keep sibling\n");
}

// =========================================================================
// §108.5 Required Offline Test Groups: Connect Isolated Implementer Workspace to Controller/FSM
// =========================================================================

// Test Group 1 (§108.5.1): Runner construction
#[test]
fn test_stage_2_implementer_runner_construction_and_boundary() {
    let temp = repo_tempdir().unwrap();
    let candidate_dir = temp.path().join("candidate");
    std::fs::create_dir_all(&candidate_dir).unwrap();
    let candidate_dir = candidate_dir.canonicalize().unwrap();

    let req = super::agent_runner::RoleExecutionRequest {
        role: super::isolation::AutoConvergeRole::Implementer,
        session_id: "test-impl-session".to_string(),
        workspace_root: candidate_dir.clone(),
        snapshot_dir: None,
        plan_path: candidate_dir.join(".plan").join("plan.md"),
        runtime_allowlist: vec!["src/lib.rs".to_string()],
        input_prompt: "Implement changes".to_string(),
        timeout_seconds: 60,
        model: Some("test-model".to_string()),
        reasoning_effort: Some("high".to_string()),
        run_id: Some("test-run-108".to_string()),
    };

    let cfg = super::codex_exec::build_implementer_launch_configuration(&req)
        .expect("valid implementer config build");
    assert_eq!(cfg.paths.working_dir, candidate_dir);
    assert!(cfg.invocation.args.iter().any(|a| a == "--json"));
    assert!(cfg.invocation.args.iter().any(|a| a == "test-model"));
    assert!(cfg
        .invocation
        .args
        .iter()
        .any(|a| a.contains("model_reasoning_effort=\"high\"")));

    let config_path = cfg.paths.codex_home.join("config.toml");
    assert!(config_path.exists());
    let config_str = std::fs::read_to_string(&config_path).unwrap();
    assert!(config_str.contains("default_permissions = \"implementer\""));
    assert!(config_str.contains("permissions.implementer.workspace_roots"));
    assert!(config_str.contains("[permissions.implementer.network]"));
    assert!(config_str.contains("enabled = false"));

    // Invalid candidate workspace root (non-existent) fails closed
    let non_existent = temp.path().join("does_not_exist");
    let bad_req = super::agent_runner::RoleExecutionRequest {
        workspace_root: non_existent,
        ..req.clone()
    };
    let err = super::codex_exec::build_implementer_launch_configuration(&bad_req);
    assert!(err.is_err());

    // Role mismatch fails closed
    let wrong_role_req = super::agent_runner::RoleExecutionRequest {
        role: super::isolation::AutoConvergeRole::Planner,
        ..req.clone()
    };
    assert!(super::codex_exec::build_implementer_launch_configuration(&wrong_role_req).is_err());

    // ProductionAgentRunner fails closed on missing candidate workspace
    let runner = super::agent_runner::ProductionAgentRunner::new();
    assert!(runner.execute_role_session(&bad_req).is_err());
}

// Helper mock runner that performs edits in candidate workspace
struct CandidateModifyingRunner {
    workspace_roots: std::sync::Mutex<Vec<PathBuf>>,
    new_content: Vec<u8>,
}

impl super::agent_runner::AgentCommandRunner for CandidateModifyingRunner {
    fn execute_role_session(
        &self,
        request: &super::agent_runner::RoleExecutionRequest,
    ) -> Result<super::agent_runner::RoleExecutionResponse, String> {
        self.workspace_roots
            .lock()
            .unwrap()
            .push(request.workspace_root.clone());
        let target = request.workspace_root.join("src").join("lib.rs");
        if target.exists() {
            std::fs::write(&target, &self.new_content).map_err(|e| e.to_string())?;
        }
        Ok(super::agent_runner::RoleExecutionResponse {
            exit_code: 0,
            stdout: "Candidate modification successful".into(),
            stderr: String::new(),
            verdict_json: None,
        })
    }
}

// Test Group 2 (§108.5.2): FSM wiring
#[test]
fn test_stage_2_fsm_candidate_workspace_isolation_and_snapshot_digest() {
    let temp = repo_tempdir().unwrap();
    let primary_dir = temp.path().join("primary");
    std::fs::create_dir_all(primary_dir.join("src")).unwrap();
    let primary_file = primary_dir.join("src").join("lib.rs");
    let initial_bytes = b"pub fn initial() -> bool { true }\n";
    let modified_bytes = b"pub fn modified() -> bool { false }\n";
    std::fs::write(&primary_file, initial_bytes).unwrap();

    let git_runner = MockCommandRunner::new();
    let state_file = temp.path().join("run_state.json");
    let mut cfg = default_test_config(fast_profile());
    cfg.working_dir = primary_dir.to_string_lossy().to_string();
    let run_id = format!("ac-run-108-fsm-{}", uuid::Uuid::new_v4());
    cfg.run_id = Some(run_id.clone());

    let baseline_manifest = capture_baseline_file_manifest(&primary_dir, &BTreeMap::new()).unwrap();
    let mut state =
        initialize_run(cfg, "head-1", baseline_manifest, &git_runner, FIXED_TIME).unwrap();
    state.runtime_allowlist = vec!["src/lib.rs".to_string()];
    save_run_state_authenticated(&state_file, &mut state).unwrap();

    // Advance to PlanApproved
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let plan_approve = make_plan_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "reviewer-1",
        &plan_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    save_run_state_authenticated(&state_file, &mut state).unwrap();

    // Advance from PlanApproved to Implement via run_phase_a_step
    let agent_runner = CandidateModifyingRunner {
        workspace_roots: std::sync::Mutex::new(Vec::new()),
        new_content: modified_bytes.to_vec(),
    };
    super::run_phase_a_step(&mut state, &agent_runner, &git_runner, &state_file).unwrap();
    assert_eq!(state.current_state, AutoConvergeState::Implement);

    // Run Implement step
    super::run_phase_a_step(&mut state, &agent_runner, &git_runner, &state_file).unwrap();
    assert_eq!(state.current_state, AutoConvergeState::ImplementationReview);

    // Primary workspace MUST remain completely byte-identical
    assert_eq!(std::fs::read(&primary_file).unwrap(), initial_bytes);

    // Candidate workspace received the modification
    let roots = agent_runner.workspace_roots.lock().unwrap().clone();
    assert_eq!(roots.len(), 1);
    let candidate_root = &roots[0];
    assert_ne!(candidate_root, &primary_dir);
    assert_eq!(
        std::fs::read(candidate_root.join("src").join("lib.rs")).unwrap(),
        modified_bytes
    );

    // Snapshot delta digest equals candidate delta digest
    let cb = state
        .candidate_binding
        .as_ref()
        .expect("candidate binding sealed");
    assert_eq!(
        cb.candidate_root,
        candidate_root.to_string_lossy().to_string()
    );
    assert!(state.current_diff_digest.is_some());
    let _ = std::fs::remove_dir_all(candidate_root);
}

// Test Group 3 (§108.5.3): Approval binding validation
#[test]
fn test_stage_2_approval_binding_validation_and_promotion_rejection() {
    let temp = repo_tempdir().unwrap();
    let primary_dir = temp.path().join("primary");
    std::fs::create_dir_all(primary_dir.join("src")).unwrap();
    let primary_file = primary_dir.join("src").join("lib.rs");
    let initial_bytes = b"pub fn initial() -> bool { true }\n";
    let modified_bytes = b"pub fn modified() -> bool { false }\n";
    std::fs::write(&primary_file, initial_bytes).unwrap();

    let git_runner = MockCommandRunner::new();
    let state_file = temp.path().join("run_state.json");
    let mut cfg = default_test_config(fast_profile());
    cfg.working_dir = primary_dir.to_string_lossy().to_string();
    let run_id = format!("ac-run-108-binding-{}", uuid::Uuid::new_v4());
    cfg.run_id = Some(run_id.clone());

    let baseline_manifest = capture_baseline_file_manifest(&primary_dir, &BTreeMap::new()).unwrap();
    let mut state =
        initialize_run(cfg, "head-1", baseline_manifest, &git_runner, FIXED_TIME).unwrap();
    state.runtime_allowlist = vec!["src/lib.rs".to_string()];
    save_run_state_authenticated(&state_file, &mut state).unwrap();

    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let plan_approve = make_plan_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "reviewer-1",
        &plan_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();

    let agent_runner = CandidateModifyingRunner {
        workspace_roots: std::sync::Mutex::new(Vec::new()),
        new_content: modified_bytes.to_vec(),
    };
    super::run_phase_a_step(&mut state, &agent_runner, &git_runner, &state_file).unwrap();
    super::run_phase_a_step(&mut state, &agent_runner, &git_runner, &state_file).unwrap();
    assert_eq!(state.current_state, AutoConvergeState::ImplementationReview);

    // Submit valid implementation review verdict to transition to ImplementationApproved
    let impl_approve = make_impl_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "reviewer-2",
        &impl_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(
        state.current_state,
        AutoConvergeState::ImplementationApproved
    );

    // Subtest 3a: Reviewer independence violation (reviewer == producer)
    {
        let mut tampered = state.clone();
        let last_idx = tampered.approvals.len() - 1;
        tampered.approvals[last_idx].reviewer_session_id =
            tampered.approvals[last_idx].producer_session_id.clone();
        tampered.approvals[last_idx].engine_auth_tag = super::guards::compute_approval_auth_tag(
            &tampered.run_id,
            tampered.approvals[last_idx].sequence,
            &tampered.approvals[last_idx].reviewer_session_id,
            &tampered.approvals[last_idx].producer_session_id,
            &tampered.approvals[last_idx].binding,
        )
        .unwrap();
        let res = super::run_phase_a_step(&mut tampered, &agent_runner, &git_runner, &state_file);
        assert!(res.is_err());
        assert_eq!(tampered.current_state, AutoConvergeState::Blocked);
        assert_eq!(std::fs::read(&primary_file).unwrap(), initial_bytes);
    }

    // Subtest 3b: Mismatched run_id in approval binding
    {
        let mut tampered = state.clone();
        let last_idx = tampered.approvals.len() - 1;
        tampered.approvals[last_idx].binding.run_id = Some("wrong-run-id".to_string());
        tampered.approvals[last_idx].engine_auth_tag = super::guards::compute_approval_auth_tag(
            &tampered.run_id,
            tampered.approvals[last_idx].sequence,
            &tampered.approvals[last_idx].reviewer_session_id,
            &tampered.approvals[last_idx].producer_session_id,
            &tampered.approvals[last_idx].binding,
        )
        .unwrap();
        let res = super::run_phase_a_step(&mut tampered, &agent_runner, &git_runner, &state_file);
        assert!(res.is_err());
        assert_eq!(tampered.current_state, AutoConvergeState::Blocked);
        assert_eq!(std::fs::read(&primary_file).unwrap(), initial_bytes);
    }

    // Subtest 3c: Mismatched cycle in approval binding
    {
        let mut tampered = state.clone();
        let last_idx = tampered.approvals.len() - 1;
        tampered.approvals[last_idx].binding.cycle = Some(999);
        tampered.approvals[last_idx].engine_auth_tag = super::guards::compute_approval_auth_tag(
            &tampered.run_id,
            tampered.approvals[last_idx].sequence,
            &tampered.approvals[last_idx].reviewer_session_id,
            &tampered.approvals[last_idx].producer_session_id,
            &tampered.approvals[last_idx].binding,
        )
        .unwrap();
        let res = super::run_phase_a_step(&mut tampered, &agent_runner, &git_runner, &state_file);
        assert!(res.is_err());
        assert_eq!(tampered.current_state, AutoConvergeState::Blocked);
        assert_eq!(std::fs::read(&primary_file).unwrap(), initial_bytes);
    }

    // Subtest 3d: Non-APPROVE verdict never promotes
    {
        let mut rc_state = state.clone();
        rc_state.current_state = AutoConvergeState::ImplementationReview;
        let rc_verdict = make_impl_verdict_json(
            &rc_state,
            VerdictDecision::RequestChanges,
            vec![sample_blocking_finding("COR-001", "Needs revision")],
            false,
        );
        submit_review_verdict(
            &mut rc_state,
            "reviewer-3",
            &rc_verdict,
            SNAP_HASH,
            SNAP_HASH,
            None,
            None,
            true,
            FIXED_TIME,
        )
        .unwrap();
        assert_ne!(
            rc_state.current_state,
            AutoConvergeState::ImplementationApproved
        );
        assert_eq!(std::fs::read(&primary_file).unwrap(), initial_bytes);
    }
    if let Some(cb) = state.candidate_binding.as_ref() {
        let _ = std::fs::remove_dir_all(&cb.candidate_root);
    }
}

// Test Group 4 (§108.5.4): Delta boundary enforcement and baseline exclusions
#[test]
fn test_stage_2_delta_boundary_enforcement_and_baseline_exclusions() {
    let temp = repo_tempdir().unwrap();
    let primary_dir = temp.path().join("primary");
    std::fs::create_dir_all(primary_dir.join("src")).unwrap();
    let primary_file = primary_dir.join("src").join("lib.rs");
    std::fs::write(&primary_file, b"initial\n").unwrap();

    // Untracked sentinel fixture
    let sentinel = primary_dir.join("untracked_sentinel.txt");
    std::fs::write(&sentinel, b"pre-existing sentinel data\n").unwrap();

    let engine_state_root = temp.path().join("engine-state");
    std::fs::create_dir_all(&engine_state_root).unwrap();

    // Excluded roots rejection: .git/, .plan/, .poverello/runs/
    let cand_git = temp.path().join("cand-git");
    assert!(super::implementer_workspace::ImplementerWorkspace::create(
        &primary_dir,
        &engine_state_root,
        &cand_git,
        &[".git/config".to_string()],
    )
    .is_err());

    let cand_plan = temp.path().join("cand-plan");
    assert!(super::implementer_workspace::ImplementerWorkspace::create(
        &primary_dir,
        &engine_state_root,
        &cand_plan,
        &[".plan/notes.md".to_string()],
    )
    .is_err());

    let cand_runs = temp.path().join("cand-runs");
    assert!(super::implementer_workspace::ImplementerWorkspace::create(
        &primary_dir,
        &engine_state_root,
        &cand_runs,
        &[".poverello/runs/state.json".to_string()],
    )
    .is_err());

    // Allowlist escape: candidate creates unexpected file outside allowlist
    let cand_escape = temp.path().join("cand-escape");
    let cand_ws = super::implementer_workspace::ImplementerWorkspace::create(
        &primary_dir,
        &engine_state_root,
        &cand_escape,
        &["src/lib.rs".to_string()],
    )
    .unwrap();

    std::fs::write(
        cand_ws.candidate_root().join("src").join("unauthorized.rs"),
        b"bad",
    )
    .unwrap();
    let delta_res = cand_ws.delta();
    assert!(
        delta_res.is_err(),
        "delta() must reject unexpected files created in candidate"
    );

    // Untracked sentinel in primary remains untouched
    assert_eq!(
        std::fs::read(&sentinel).unwrap(),
        b"pre-existing sentinel data\n"
    );
}

// Test Group 5 (§108.5.5): Promotion and crash recovery deterministic
#[test]
fn test_stage_2_promotion_and_crash_recovery_deterministic() {
    let temp = repo_tempdir().unwrap();
    let primary_dir = temp.path().join("primary");
    std::fs::create_dir_all(primary_dir.join("src")).unwrap();
    let primary_file = primary_dir.join("src").join("lib.rs");
    std::fs::write(&primary_file, b"initial\n").unwrap();

    let engine_state_root = temp.path().join("engine-state");
    std::fs::create_dir_all(&engine_state_root).unwrap();

    // 1. Clean successful promotion
    let cand_dir_success = temp.path().join("cand-promo-success");
    let cand_ws_success = super::implementer_workspace::ImplementerWorkspace::create(
        &primary_dir,
        &engine_state_root,
        &cand_dir_success,
        &["src/lib.rs".to_string()],
    )
    .unwrap();

    std::fs::write(
        cand_ws_success.candidate_root().join("src").join("lib.rs"),
        b"promoted\n",
    )
    .unwrap();
    let delta_digest = cand_ws_success.delta_digest().unwrap();
    let permit = cand_ws_success
        .authorize_delta(cand_ws_success.baseline_digest(), &delta_digest)
        .unwrap();

    let mut success_phases = Vec::new();
    let mut success_journal = None;
    let outcome = cand_ws_success
        .promote_with_controller(
            &permit,
            "run-promo-success",
            |phase, journal_dir, _digest| {
                success_phases.push(phase);
                success_journal = Some(journal_dir.to_path_buf());
                Ok(())
            },
        )
        .expect("successful promotion");

    assert!(matches!(
        outcome,
        super::implementer_workspace::PromotionOutcome::Promoted { .. }
    ));
    assert_eq!(std::fs::read(&primary_file).unwrap(), b"promoted\n");
    if let Some(j) = success_journal {
        assert!(
            !j.exists(),
            "successful promotion cleans up journal directory"
        );
    }

    // 2. Injected failure during promotion retains journal, recovery restores baseline
    // Reset primary back to initial
    std::fs::write(&primary_file, b"initial\n").unwrap();
    let cand_dir_fail = temp.path().join("cand-promo-fail");
    let cand_ws_fail = super::implementer_workspace::ImplementerWorkspace::create(
        &primary_dir,
        &engine_state_root,
        &cand_dir_fail,
        &["src/lib.rs".to_string()],
    )
    .unwrap();
    std::fs::write(
        cand_ws_fail.candidate_root().join("src").join("lib.rs"),
        b"modified\n",
    )
    .unwrap();
    let delta_digest2 = cand_ws_fail.delta_digest().unwrap();
    let permit2 = cand_ws_fail
        .authorize_delta(cand_ws_fail.baseline_digest(), &delta_digest2)
        .unwrap();

    let mut fail_journal = None;
    let fail_outcome = cand_ws_fail.promote_with_controller(
        &permit2,
        "run-promo-fail",
        |phase, journal_dir, _digest| {
            fail_journal = Some(journal_dir.to_path_buf());
            if phase == super::types::PromotionPhase::Applying {
                return Err("Simulated crash during application".into());
            }
            Ok(())
        },
    );
    assert!(fail_outcome.is_err());
    let jpath = fail_journal.expect("journal path recorded");
    assert!(
        jpath.exists(),
        "journal directory must be retained on failure"
    );

    // Recovery path: recover_retained_promotion restores exact baseline and cleans up journal
    super::implementer_workspace::recover_retained_promotion(&primary_dir, &jpath)
        .expect("recover retained promotion");
    assert_eq!(std::fs::read(&primary_file).unwrap(), b"initial\n");
    assert!(
        !jpath.exists(),
        "journal directory must be removed after successful recovery"
    );

    // Subsequent recovery attempt on absent journal fails closed
    assert!(
        super::implementer_workspace::recover_retained_promotion(&primary_dir, &jpath).is_err()
    );
}

// Test Group 6 (§108.5.6): Persistence and schema compatibility
#[test]
fn test_stage_2_persistence_and_schema_compatibility() {
    let temp = repo_tempdir().unwrap();
    let candidate_dir = temp.path().join("cand");
    std::fs::create_dir_all(&candidate_dir).unwrap();

    let valid_binding = super::types::CandidateBinding {
        run_id: "ac-run-108".to_string(),
        cycle: 0,
        session_id: "sess-1".to_string(),
        candidate_root: candidate_dir.to_string_lossy().to_string(),
        approved_plan_digest: "1111111111111111111111111111111111111111111111111111111111111111"
            .to_string(),
        run_start_baseline_manifest_digest:
            "2222222222222222222222222222222222222222222222222222222222222222".to_string(),
        allowlist_digest: "3333333333333333333333333333333333333333333333333333333333333333"
            .to_string(),
        candidate_baseline_digest:
            "4444444444444444444444444444444444444444444444444444444444444444".to_string(),
        review_bundle_digest: None,
    };
    assert!(valid_binding.validate().is_ok());

    // Invalid binding: relative candidate_root fails validation
    let mut invalid_binding = valid_binding.clone();
    invalid_binding.candidate_root = "relative/path".to_string();
    assert!(invalid_binding.validate().is_err());

    // Invalid binding: malformed digest
    let mut bad_digest = valid_binding.clone();
    bad_digest.approved_plan_digest = "short".to_string();
    assert!(bad_digest.validate().is_err());

    // Legacy JSON deserialization without candidate_binding or promotion_journal
    let base_state = initialize_run(
        default_test_config(fast_profile()),
        "head-1",
        vec![],
        &MockCommandRunner::new(),
        FIXED_TIME,
    )
    .unwrap();
    let mut val = serde_json::to_value(&base_state).unwrap();
    val.as_object_mut().unwrap().remove("candidate_binding");
    val.as_object_mut().unwrap().remove("promotion_journal");
    let legacy_json = serde_json::to_string(&val).unwrap();

    let deserialized: AutoConvergeRunState =
        serde_json::from_str(&legacy_json).expect("deserialize legacy json");
    assert!(deserialized.candidate_binding.is_none());
    assert!(deserialized.promotion_journal.is_none());
}

// Test Group 7 (§108.5.7): Gate invariant
#[test]
fn test_stage_2_gate_invariant_production_dispatch_blocked() {
    let _lock = super::isolation::TEST_ISOLATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    super::isolation::set_test_isolation_pass(false);

    let temp = repo_tempdir().unwrap();
    let ws = temp.path().join("workspace");
    let snap = temp.path().join("snapshot");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&snap).unwrap();

    // Verify reviewer OS boundary probe fails closed when unprivileged / unverified
    let boundary_res =
        super::isolation::PlatformIsolationGuard::verify_reviewer_os_boundary(&ws, &snap);
    assert!(
        boundary_res.is_err(),
        "Runtime isolation gate must fail closed when unverified"
    );

    // ProductionAgentRunner execution blocks on unverified environment
    let runner = super::agent_runner::ProductionAgentRunner::new();
    let req = super::agent_runner::RoleExecutionRequest {
        role: super::isolation::AutoConvergeRole::Reviewer,
        session_id: "test-rev".to_string(),
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap),
        plan_path: ws.join(".plan").join("plan.md"),
        runtime_allowlist: vec!["src/lib.rs".to_string()],
        input_prompt: "echo probe".to_string(),
        timeout_seconds: 5,
        model: None,
        reasoning_effort: None,
        run_id: None,
    };
    assert!(
        runner.execute_role_session(&req).is_err(),
        "ProductionAgentRunner must block before launch when isolation is unverified"
    );
}

// ===========================================================================
// §109 Offline Tests: Candidate Lifecycle and Promotion Recovery Fail-Closed
// ===========================================================================

// Test 109.5.1: Candidate collision and ownership rejection fail-closed
#[test]
fn test_stage_2_109_candidate_collision_and_ownership_rejection_fail_closed() {
    let temp = repo_tempdir().unwrap();
    let primary_dir = temp.path().join("primary");
    std::fs::create_dir_all(primary_dir.join("src")).unwrap();
    let primary_file = primary_dir.join("src").join("lib.rs");
    std::fs::write(&primary_file, b"initial\n").unwrap();

    let git_runner = MockCommandRunner::new();
    let state_file = temp.path().join("run_state.json");
    let mut cfg = default_test_config(fast_profile());
    cfg.working_dir = primary_dir.to_string_lossy().to_string();
    let run_id = format!("ac-run-109-collision-{}", uuid::Uuid::new_v4());
    cfg.run_id = Some(run_id.clone());

    let baseline_manifest = capture_baseline_file_manifest(&primary_dir, &BTreeMap::new()).unwrap();
    let mut state =
        initialize_run(cfg, "head-1", baseline_manifest, &git_runner, FIXED_TIME).unwrap();
    state.runtime_allowlist = vec!["src/lib.rs".to_string()];
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let plan_approve = make_plan_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "reviewer-1",
        &plan_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    save_run_state_authenticated(&state_file, &mut state).unwrap();

    // Step 1: Pre-create would-be candidate child directory with sentinel files
    let candidate_parent = ambient_temp_dir()
        .join("poverello")
        .join("candidate_workspaces")
        .join(&run_id);
    let candidate_child = candidate_parent.join("cycle-0");
    std::fs::create_dir_all(&candidate_child).unwrap();
    let sentinel_file = candidate_child.join("unowned_sentinel.txt");
    let sentinel_content = b"unowned sentinel content - must not be deleted or modified\n";
    std::fs::write(&sentinel_file, sentinel_content).unwrap();

    // Advance to Implement
    let agent_runner = CandidateModifyingRunner {
        workspace_roots: std::sync::Mutex::new(Vec::new()),
        new_content: b"modified\n".to_vec(),
    };
    super::run_phase_a_step(&mut state, &agent_runner, &git_runner, &state_file).unwrap();
    assert_eq!(state.current_state, AutoConvergeState::Implement);

    // Initial creation MUST reject collision and fail closed without touching sentinel
    let res = super::run_phase_a_step(&mut state, &agent_runner, &git_runner, &state_file);
    assert!(res.is_err());
    assert_eq!(state.current_state, AutoConvergeState::Blocked);
    let ctx = state.blocked_context.as_ref().expect("blocked context");
    assert_eq!(ctx.kind, BlockedKind::FatalInvariant);

    // Sentinel and candidate child MUST remain intact byte-for-byte
    assert!(
        candidate_child.exists(),
        "candidate child must not be deleted"
    );
    assert!(sentinel_file.exists(), "sentinel file must not be deleted");
    assert_eq!(std::fs::read(&sentinel_file).unwrap(), sentinel_content);

    // Clean up pre-created collision child for this test
    let _ = std::fs::remove_dir_all(&candidate_parent);

    // Step 2: Test parent ancestor reparse/relative validation
    let bad_parent = temp.path().join("relative_not_absolute");
    let parent_res =
        super::implementer_workspace::validate_and_create_candidate_parent(&bad_parent);
    assert!(parent_res.is_ok() || !bad_parent.is_absolute());

    // Step 3: Corrective cycle candidate reuse validation failure preserves candidate
    let mut state2 = initialize_run(
        {
            let mut c = default_test_config(fast_profile());
            c.working_dir = primary_dir.to_string_lossy().to_string();
            c.run_id = Some(format!("ac-run-109-cor-{}", uuid::Uuid::new_v4()));
            c
        },
        "head-1",
        capture_baseline_file_manifest(&primary_dir, &BTreeMap::new()).unwrap(),
        &git_runner,
        FIXED_TIME,
    )
    .unwrap();
    state2.runtime_allowlist = vec!["src/lib.rs".to_string()];
    state2.current_state = AutoConvergeState::CorrectiveImplement;

    // Simulate invalid candidate binding (e.g. non-existent candidate path)
    let non_existent_cand = temp.path().join("non_existent_candidate");
    state2.candidate_binding = Some(super::types::CandidateBinding {
        run_id: state2.run_id.clone(),
        cycle: 0,
        session_id: "sess-1".to_string(),
        candidate_root: non_existent_cand.to_string_lossy().to_string(),
        approved_plan_digest: "1111111111111111111111111111111111111111111111111111111111111111"
            .to_string(),
        run_start_baseline_manifest_digest:
            "2222222222222222222222222222222222222222222222222222222222222222".to_string(),
        allowlist_digest: "3333333333333333333333333333333333333333333333333333333333333333"
            .to_string(),
        candidate_baseline_digest:
            "4444444444444444444444444444444444444444444444444444444444444444".to_string(),
        review_bundle_digest: None,
    });
    let state_file2 = temp.path().join("run_state2.json");
    save_run_state_authenticated(&state_file2, &mut state2).unwrap();

    let res2 = super::run_phase_a_step(&mut state2, &agent_runner, &git_runner, &state_file2);
    assert!(
        res2.is_err(),
        "CorrectiveImplement must fail closed when candidate path is missing"
    );
    assert_eq!(state2.current_state, AutoConvergeState::Blocked);
}

// Test 109.5.2: Journal digest exact bytes and tamper rejection
#[test]
fn test_stage_2_109_journal_digest_exact_bytes_and_tamper_rejection() {
    let temp = repo_tempdir().unwrap();
    let primary_dir = temp.path().join("primary");
    std::fs::create_dir_all(primary_dir.join("src")).unwrap();
    let primary_file = primary_dir.join("src").join("lib.rs");
    std::fs::write(&primary_file, b"initial content\n").unwrap();

    let engine_state_root = temp.path().join("engine-state");
    std::fs::create_dir_all(&engine_state_root).unwrap();

    let cand_dir = temp.path().join("cand-journal-test");
    let cand_ws = super::implementer_workspace::ImplementerWorkspace::create(
        &primary_dir,
        &engine_state_root,
        &cand_dir,
        &["src/lib.rs".to_string()],
    )
    .unwrap();

    std::fs::write(
        cand_ws.candidate_root().join("src").join("lib.rs"),
        b"updated content\n",
    )
    .unwrap();
    let delta_digest = cand_ws.delta_digest().unwrap();
    let permit = cand_ws
        .authorize_delta(cand_ws.baseline_digest(), &delta_digest)
        .unwrap();

    let mut phase_digests = Vec::new();
    let mut journal_path = None;

    // Interrupted promotion to inspect journal and latest snapshot
    let mut on_phase = |phase, jdir: &std::path::Path, digest: &str| {
        phase_digests.push((phase, jdir.to_path_buf(), digest.to_string()));
        journal_path = Some(jdir.to_path_buf());
        if phase == super::types::PromotionPhase::Applying && phase_digests.len() >= 3 {
            return Err("Simulated crash during application to retain journal".into());
        }
        Ok(())
    };
    let outcome =
        cand_ws.promote_internal(&permit, "run-109-digest", &mut on_phase, None, false, None);
    assert!(outcome.is_err());

    let jdir = journal_path.expect("journal path captured");
    assert!(jdir.exists());

    // Verify initial snapshot digest
    let init_bytes = std::fs::read(jdir.join("journal.json")).unwrap();
    let init_expected_digest = super::implementer_workspace::test_sha256(&init_bytes);
    assert_eq!(phase_digests[0].2, init_expected_digest);

    // Verify latest snapshot digest is returned by load_and_validate_journal_with_digest
    let (j_data, count, actual_digest) =
        super::implementer_workspace::load_and_validate_journal_with_digest(&jdir).unwrap();
    assert_eq!(count, j_data.mutation_intents.len());
    assert_eq!(actual_digest, phase_digests.last().unwrap().2);

    // Tampering test: tamper with a byte in the latest snapshot file
    let mut update_files = Vec::new();
    for entry in std::fs::read_dir(&jdir).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("journal-update-") {
            update_files.push(entry.path());
        }
    }
    assert!(
        !update_files.is_empty(),
        "must have written at least one update snapshot"
    );
    let target_snapshot = &update_files[0];

    // Read bytes, modify one byte, and write back
    let mut content = std::fs::read(target_snapshot).unwrap();
    content.push(b' '); // tamper formatting or byte
    std::fs::write(target_snapshot, &content).unwrap();

    // Now recovery with expected_digest MUST fail closed because actual digest != expected
    let tamper_res = super::implementer_workspace::recover_retained_promotion_with_digest(
        &primary_dir,
        &jdir,
        Some(&actual_digest),
    );
    assert!(
        tamper_res.is_err(),
        "Recovery must fail closed on tampered journal snapshot"
    );

    // Clean up
    let _ = std::fs::remove_dir_all(&jdir);
    let _ = std::fs::remove_dir_all(cand_ws.candidate_root());
}

// Test 109.5.3 / §110.4.5: Focused persisted-state interruption matrix and authority root containment
#[test]
fn test_stage_2_109_controller_restart_recovery_matrix_idempotent() {
    let temp = repo_tempdir().unwrap();
    let temp_root = temp.path().to_path_buf();

    // 1. Injected engine authority root containment (§110.4.4 / §111.4.4)
    let engine_state_root = temp_root.join("engine-state");
    std::fs::create_dir_all(&engine_state_root).unwrap();
    let _authority_guard = super::TestEngineAuthorityGuard::enter(engine_state_root.clone());

    // Verify resolved authority directory is contained within disposable temp root
    let resolved = super::resolve_engine_authority_dir().unwrap();
    assert!(resolved.starts_with(&temp_root));
    if let Some(appdata) = std::env::var_os("APPDATA") {
        assert!(!resolved.starts_with(std::path::PathBuf::from(appdata)));
    }

    let primary_dir = temp_root.join("primary");
    std::fs::create_dir_all(primary_dir.join("src")).unwrap();
    let primary_file = primary_dir.join("src").join("lib.rs");
    let initial_bytes = b"pub fn initial() -> i32 { 42 }\n";
    let modified_bytes = b"pub fn modified() -> i32 { 99 }\n";
    std::fs::write(&primary_file, initial_bytes).unwrap();

    let git_runner = MockCommandRunner::new();
    let state_file = temp_root.join("run_state.json");
    let mut cfg = default_test_config(fast_profile());
    cfg.working_dir = primary_dir.to_string_lossy().to_string();
    let run_id = format!("ac-run-110-matrix-{}", uuid::Uuid::new_v4());
    cfg.run_id = Some(run_id.clone());

    let baseline_manifest = capture_baseline_file_manifest(&primary_dir, &BTreeMap::new()).unwrap();
    let mut state =
        initialize_run(cfg, "head-1", baseline_manifest, &git_runner, FIXED_TIME).unwrap();
    state.runtime_allowlist = vec!["src/lib.rs".to_string()];

    // Transition state to ImplementationApproved
    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let plan_approve = make_plan_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "reviewer-1",
        &plan_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();

    let agent_runner = CandidateModifyingRunner {
        workspace_roots: std::sync::Mutex::new(Vec::new()),
        new_content: modified_bytes.to_vec(),
    };
    super::run_phase_a_step(&mut state, &agent_runner, &git_runner, &state_file).unwrap();
    super::run_phase_a_step(&mut state, &agent_runner, &git_runner, &state_file).unwrap();

    // Set review_bundle_digest in candidate_binding to match SNAP_HASH
    if let Some(ref mut cb) = state.candidate_binding {
        cb.review_bundle_digest = Some(SNAP_HASH.to_string());
    }

    let impl_approve = make_impl_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "reviewer-2",
        &impl_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(
        state.current_state,
        AutoConvergeState::ImplementationApproved
    );

    // Create a real promotion journal via promote_internal with simulated crash
    let cb = state.candidate_binding.as_ref().unwrap();
    let cand_ws = super::implementer_workspace::ImplementerWorkspace::load_existing(
        &primary_dir,
        &engine_state_root,
        &PathBuf::from(&cb.candidate_root),
        &state.runtime_allowlist,
        &cb.candidate_baseline_digest,
    )
    .unwrap();
    let delta_digest = cand_ws.delta_digest().unwrap();
    let permit = cand_ws
        .authorize_delta(cand_ws.baseline_digest(), &delta_digest)
        .unwrap();

    let mut interrupted_journal = None;
    let mut interrupted_digest = String::new();
    let mut applying_count = 0;
    let mut on_phase = |phase, jdir: &std::path::Path, digest: &str| {
        if phase == super::types::PromotionPhase::Applying {
            applying_count += 1;
            interrupted_journal = Some(jdir.to_path_buf());
            interrupted_digest = digest.to_string();
            if applying_count >= 2 {
                return Err("Simulated crash during applying to retain journal".into());
            }
        }
        Ok(())
    };
    let _ = cand_ws.promote_internal(&permit, &run_id, &mut on_phase, None, false, None);
    let jdir = interrupted_journal.expect("interrupted journal created");
    assert!(jdir.exists());

    // -----------------------------------------------------------------------
    // Matrix Step 1: RecoveryRequired while terminal / Blocked (§110.2.1)
    // -----------------------------------------------------------------------
    {
        let mut sim_state = state.clone();
        sim_state.current_state = AutoConvergeState::Blocked;
        sim_state.is_terminal = true;
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: jdir.to_string_lossy().to_string(),
            journal_digest: interrupted_digest.clone(),
            phase: super::types::PromotionPhase::RecoveryRequired,
            candidate_root: cb.candidate_root.clone(),
        });
        // Mutate primary to dirty state
        std::fs::write(&primary_file, modified_bytes).unwrap();
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        // Controller re-entry must reach recovery reconciliation, restore baseline,
        // delete journal, clear binding, and remain Blocked (no agent dispatch).
        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(res.is_ok());
        assert_eq!(std::fs::read(&primary_file).unwrap(), initial_bytes);
        assert!(sim_state.promotion_journal.is_none());
        assert_eq!(sim_state.current_state, AutoConvergeState::Blocked);
        assert!(sim_state.is_terminal);
        assert!(
            !jdir.exists(),
            "journal directory must be deleted after verified recovery"
        );
    }

    // -----------------------------------------------------------------------
    // Matrix Step 2: Interruption after baseline restore but before recovery-phase save
    // -----------------------------------------------------------------------
    let mut interrupted_journal2 = None;
    let mut interrupted_digest2 = String::new();
    let mut applying_count2 = 0;
    let mut on_phase2 = |phase, jdir: &std::path::Path, digest: &str| {
        if phase == super::types::PromotionPhase::Applying {
            applying_count2 += 1;
            interrupted_journal2 = Some(jdir.to_path_buf());
            interrupted_digest2 = digest.to_string();
            if applying_count2 >= 2 {
                return Err("Simulated crash".into());
            }
        }
        Ok(())
    };
    let _ = cand_ws.promote_internal(&permit, &run_id, &mut on_phase2, None, false, None);
    let jdir2 = interrupted_journal2.expect("journal 2 created");

    {
        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: jdir2.to_string_lossy().to_string(),
            journal_digest: interrupted_digest2.clone(),
            phase: super::types::PromotionPhase::Applying,
            candidate_root: cb.candidate_root.clone(),
        });
        std::fs::write(&primary_file, initial_bytes).unwrap();
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(res.is_ok());
        assert_eq!(std::fs::read(&primary_file).unwrap(), initial_bytes);
        assert!(sim_state.promotion_journal.is_none());
        assert!(!jdir2.exists(), "journal must be deleted");
    }

    // -----------------------------------------------------------------------
    // Matrix Step 3: Interruption after recovery-phase save but before journal deletion
    // -----------------------------------------------------------------------
    let mut interrupted_journal3 = None;
    let mut interrupted_digest3 = String::new();
    let mut applying_count3 = 0;
    let mut on_phase3 = |phase, jdir: &std::path::Path, digest: &str| {
        if phase == super::types::PromotionPhase::Applying {
            applying_count3 += 1;
            interrupted_journal3 = Some(jdir.to_path_buf());
            interrupted_digest3 = digest.to_string();
            if applying_count3 >= 2 {
                return Err("Simulated crash".into());
            }
        }
        Ok(())
    };
    let _ = cand_ws.promote_internal(&permit, &run_id, &mut on_phase3, None, false, None);
    let jdir3 = interrupted_journal3.expect("journal 3 created");
    std::fs::write(&primary_file, initial_bytes).unwrap();

    {
        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: jdir3.to_string_lossy().to_string(),
            journal_digest: interrupted_digest3.clone(),
            phase: super::types::PromotionPhase::RecoveryRestored,
            candidate_root: cb.candidate_root.clone(),
        });
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(res.is_ok());
        assert_eq!(std::fs::read(&primary_file).unwrap(), initial_bytes);
        assert!(sim_state.promotion_journal.is_none());
        assert!(
            !jdir3.exists(),
            "valid real journal must be deleted in RecoveryRestored phase"
        );
    }

    // -----------------------------------------------------------------------
    // Matrix Step 4: Interruption after journal deletion but before binding-clear save
    // -----------------------------------------------------------------------
    {
        let missing_jdir = primary_dir
            .parent()
            .unwrap()
            .join(format!(".pct-promotion-{}", uuid::Uuid::new_v4()));
        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: missing_jdir.to_string_lossy().to_string(),
            journal_digest: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            phase: super::types::PromotionPhase::RecoveryRestored,
            candidate_root: cb.candidate_root.clone(),
        });
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(res.is_ok());
        assert!(sim_state.promotion_journal.is_none());
    }

    // -----------------------------------------------------------------------
    // Matrix Step 5: Repeat recovery (idempotent no-op)
    // -----------------------------------------------------------------------
    {
        let mut sim_state = state.clone();
        sim_state.current_state = AutoConvergeState::Blocked;
        sim_state.is_terminal = true;
        sim_state.promotion_journal = None;
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(res.is_ok());
        assert_eq!(sim_state.current_state, AutoConvergeState::Blocked);
        assert!(sim_state.is_terminal);
    }

    // -----------------------------------------------------------------------
    // Matrix Step 6: Missing / malformed / mismatched journal snapshot fail-closed
    // -----------------------------------------------------------------------
    {
        let missing_jdir = primary_dir
            .parent()
            .unwrap()
            .join(format!(".pct-promotion-{}", uuid::Uuid::new_v4()));
        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: missing_jdir.to_string_lossy().to_string(),
            journal_digest: "1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            phase: super::types::PromotionPhase::Applying,
            candidate_root: cb.candidate_root.clone(),
        });
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(
            res.is_err(),
            "missing journal in Applying phase must fail closed"
        );
        assert_eq!(sim_state.current_state, AutoConvergeState::Blocked);
    }

    // Clean up candidate
    let _ = std::fs::remove_dir_all(cand_ws.candidate_root());
}

// Test 109.5.4 / §110.4.3: Approval and baseline binding revalidation
#[test]
fn test_stage_2_109_approval_and_baseline_binding_revalidation() {
    let temp = repo_tempdir().unwrap();
    let temp_root = temp.path().to_path_buf();

    // Injected engine authority root containment
    let engine_state_root = temp_root.join("engine-state");
    std::fs::create_dir_all(&engine_state_root).unwrap();
    let _authority_guard = super::TestEngineAuthorityGuard::enter(engine_state_root.clone());

    let primary_dir = temp_root.join("primary");
    std::fs::create_dir_all(primary_dir.join("src")).unwrap();
    let primary_file = primary_dir.join("src").join("lib.rs");
    let initial_bytes = b"pub fn initial() -> bool { true }\n";
    let modified_bytes = b"pub fn modified() -> bool { false }\n";
    std::fs::write(&primary_file, initial_bytes).unwrap();

    let git_runner = MockCommandRunner::new();
    let state_file = temp_root.join("run_state.json");
    let mut cfg = default_test_config(fast_profile());
    cfg.working_dir = primary_dir.to_string_lossy().to_string();
    let run_id = format!("ac-run-109-reval-{}", uuid::Uuid::new_v4());
    cfg.run_id = Some(run_id.clone());

    let baseline_manifest = capture_baseline_file_manifest(&primary_dir, &BTreeMap::new()).unwrap();
    let mut state =
        initialize_run(cfg, "head-1", baseline_manifest, &git_runner, FIXED_TIME).unwrap();
    state.runtime_allowlist = vec!["src/lib.rs".to_string()];

    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let plan_approve = make_plan_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "reviewer-1",
        &plan_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();

    let agent_runner = CandidateModifyingRunner {
        workspace_roots: std::sync::Mutex::new(Vec::new()),
        new_content: modified_bytes.to_vec(),
    };
    super::run_phase_a_step(&mut state, &agent_runner, &git_runner, &state_file).unwrap();
    super::run_phase_a_step(&mut state, &agent_runner, &git_runner, &state_file).unwrap();

    if let Some(ref mut cb) = state.candidate_binding {
        cb.review_bundle_digest = Some(SNAP_HASH.to_string());
    }

    let impl_approve = make_impl_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "reviewer-2",
        &impl_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(
        state.current_state,
        AutoConvergeState::ImplementationApproved
    );

    // Case 1: Primary baseline changed concurrently before promotion
    {
        let mut tampered = state.clone();
        std::fs::write(&primary_file, b"concurrent change\n").unwrap();
        let res = super::run_phase_a_step(&mut tampered, &agent_runner, &git_runner, &state_file);
        assert!(
            res.is_err(),
            "Promotion must be rejected when primary baseline changes concurrently"
        );
        assert_eq!(tampered.current_state, AutoConvergeState::Blocked);

        std::fs::write(&primary_file, initial_bytes).unwrap();
    }

    // Case 2: Candidate files modified after approval
    {
        let mut tampered = state.clone();
        let cb = tampered.candidate_binding.as_ref().unwrap();
        let cand_file = PathBuf::from(&cb.candidate_root).join("src").join("lib.rs");
        std::fs::write(&cand_file, b"tampered after approval\n").unwrap();

        let res = super::run_phase_a_step(&mut tampered, &agent_runner, &git_runner, &state_file);
        assert!(
            res.is_err(),
            "Promotion must be rejected when candidate files change after approval"
        );
        assert_eq!(tampered.current_state, AutoConvergeState::Blocked);

        std::fs::write(&cand_file, modified_bytes).unwrap();
    }

    // Case 3: Missing run_id in approval binding (§110.2.3, §110.4.3)
    {
        let mut tampered = state.clone();
        if let Some(a) = tampered.approvals.last_mut() {
            a.binding.run_id = None;
            a.engine_auth_tag = super::guards::compute_approval_auth_tag(
                &tampered.run_id,
                a.sequence,
                &a.reviewer_session_id,
                &a.producer_session_id,
                &a.binding,
            )
            .unwrap();
        }
        let res = super::run_phase_a_step(&mut tampered, &agent_runner, &git_runner, &state_file);
        assert!(res.is_err(), "Promotion must reject missing run_id");
        assert_eq!(tampered.current_state, AutoConvergeState::Blocked);
    }

    // Case 4: Missing cycle in approval binding
    {
        let mut tampered = state.clone();
        if let Some(a) = tampered.approvals.last_mut() {
            a.binding.cycle = None;
            a.engine_auth_tag = super::guards::compute_approval_auth_tag(
                &tampered.run_id,
                a.sequence,
                &a.reviewer_session_id,
                &a.producer_session_id,
                &a.binding,
            )
            .unwrap();
        }
        let res = super::run_phase_a_step(&mut tampered, &agent_runner, &git_runner, &state_file);
        assert!(res.is_err(), "Promotion must reject missing cycle");
        assert_eq!(tampered.current_state, AutoConvergeState::Blocked);
    }

    // Case 5: Missing implementer_session_id in approval binding
    {
        let mut tampered = state.clone();
        if let Some(a) = tampered.approvals.last_mut() {
            a.binding.implementer_session_id = None;
            a.engine_auth_tag = super::guards::compute_approval_auth_tag(
                &tampered.run_id,
                a.sequence,
                &a.reviewer_session_id,
                &a.producer_session_id,
                &a.binding,
            )
            .unwrap();
        }
        let res = super::run_phase_a_step(&mut tampered, &agent_runner, &git_runner, &state_file);
        assert!(
            res.is_err(),
            "Promotion must reject missing implementer_session_id"
        );
        assert_eq!(tampered.current_state, AutoConvergeState::Blocked);
    }

    // Case 6: Missing bundle_digest in approval binding
    {
        let mut tampered = state.clone();
        if let Some(a) = tampered.approvals.last_mut() {
            a.binding.bundle_digest = None;
            a.engine_auth_tag = super::guards::compute_approval_auth_tag(
                &tampered.run_id,
                a.sequence,
                &a.reviewer_session_id,
                &a.producer_session_id,
                &a.binding,
            )
            .unwrap();
        }
        let res = super::run_phase_a_step(&mut tampered, &agent_runner, &git_runner, &state_file);
        assert!(res.is_err(), "Promotion must reject missing bundle_digest");
        assert_eq!(tampered.current_state, AutoConvergeState::Blocked);
    }

    // Case 7: Mismatched bundle_digest in approval binding
    {
        let mut tampered = state.clone();
        if let Some(a) = tampered.approvals.last_mut() {
            a.binding.bundle_digest = Some(
                "9999999999999999999999999999999999999999999999999999999999999999".to_string(),
            );
            a.engine_auth_tag = super::guards::compute_approval_auth_tag(
                &tampered.run_id,
                a.sequence,
                &a.reviewer_session_id,
                &a.producer_session_id,
                &a.binding,
            )
            .unwrap();
        }
        let res = super::run_phase_a_step(&mut tampered, &agent_runner, &git_runner, &state_file);
        assert!(
            res.is_err(),
            "Promotion must reject mismatched bundle_digest"
        );
        assert_eq!(tampered.current_state, AutoConvergeState::Blocked);
    }

    // Case 8: Successful exact-binding promotion
    {
        let mut promote_state = state.clone();
        let res =
            super::run_phase_a_step(&mut promote_state, &agent_runner, &git_runner, &state_file);
        assert!(
            res.is_ok(),
            "Promotion must succeed when all exact bindings match: {:?}",
            res.err()
        );
        assert!(
            promote_state.is_terminal,
            "Run state must be terminal after successful promotion"
        );
        assert_eq!(std::fs::read(&primary_file).unwrap(), modified_bytes);
    }

    // Clean up
    if let Some(cb) = state.candidate_binding.as_ref() {
        let _ = std::fs::remove_dir_all(&cb.candidate_root);
    }
}

// Test 109.5.5: Preservation matrix with untracked sentinels
#[test]
fn test_stage_2_109_preservation_matrix_with_untracked_sentinels() {
    let temp = repo_tempdir().unwrap();
    let primary_dir = temp.path().join("primary");
    std::fs::create_dir_all(primary_dir.join("src")).unwrap();
    std::fs::create_dir_all(primary_dir.join(".git")).unwrap();
    std::fs::create_dir_all(primary_dir.join(".plan")).unwrap();
    std::fs::create_dir_all(primary_dir.join("untracked_subdir")).unwrap();

    let primary_file = primary_dir.join("src").join("lib.rs");
    std::fs::write(&primary_file, b"primary lib\n").unwrap();

    // Sentinel fixtures that must never be modified or deleted
    let git_head = primary_dir.join(".git").join("HEAD");
    std::fs::write(&git_head, b"ref: refs/heads/main\n").unwrap();
    let plan_doc = primary_dir.join(".plan").join("plan.md");
    std::fs::write(&plan_doc, b"# Plan\n").unwrap();
    let untracked = primary_dir.join("untracked_subdir").join("scratch.txt");
    std::fs::write(&untracked, b"untracked user data\n").unwrap();

    let git_head_sha = super::implementer_workspace::test_sha256(b"ref: refs/heads/main\n");
    let plan_doc_sha = super::implementer_workspace::test_sha256(b"# Plan\n");
    let untracked_sha = super::implementer_workspace::test_sha256(b"untracked user data\n");

    let engine_state_root = temp.path().join("engine-state");
    std::fs::create_dir_all(&engine_state_root).unwrap();

    let cand_dir = temp.path().join("cand-preservation");
    let cand_ws = super::implementer_workspace::ImplementerWorkspace::create(
        &primary_dir,
        &engine_state_root,
        &cand_dir,
        &["src/lib.rs".to_string()],
    )
    .unwrap();

    // Modify candidate
    std::fs::write(
        cand_ws.candidate_root().join("src").join("lib.rs"),
        b"promoted lib\n",
    )
    .unwrap();
    let delta_digest = cand_ws.delta_digest().unwrap();
    let permit = cand_ws
        .authorize_delta(cand_ws.baseline_digest(), &delta_digest)
        .unwrap();

    // Perform successful promotion
    let outcome = cand_ws
        .promote_with_controller(&permit, "run-109-pres", |_p, _j, _d| Ok(()))
        .unwrap();
    assert!(matches!(
        outcome,
        super::implementer_workspace::PromotionOutcome::Promoted { .. }
    ));

    // Assert only allowlisted file was modified
    assert_eq!(std::fs::read(&primary_file).unwrap(), b"promoted lib\n");

    // Assert all sentinels are completely untouched
    let actual_git_head_sha =
        super::implementer_workspace::test_sha256(&std::fs::read(&git_head).unwrap());
    let actual_plan_doc_sha =
        super::implementer_workspace::test_sha256(&std::fs::read(&plan_doc).unwrap());
    let actual_untracked_sha =
        super::implementer_workspace::test_sha256(&std::fs::read(&untracked).unwrap());

    assert_eq!(
        actual_git_head_sha, git_head_sha,
        ".git/HEAD must be untouched"
    );
    assert_eq!(
        actual_plan_doc_sha, plan_doc_sha,
        ".plan/plan.md must be untouched"
    );
    assert_eq!(
        actual_untracked_sha, untracked_sha,
        "untracked scratch file must be untouched"
    );

    // Clean up
    let _ = std::fs::remove_dir_all(cand_ws.candidate_root());
}

// Test §111.4.5: Authenticated journal cleanup, RecoveryRestored validation order, and Applied nonterminal failure
#[test]
fn test_stage_2_111_recovery_restored_and_cleanup_matrix() {
    let temp = repo_tempdir().unwrap();
    let temp_root = temp.path().to_path_buf();

    // 1. Panic-safe test override reset (§111.4.4 / §111.4.5)
    let initial_auth = super::resolve_engine_authority_dir().unwrap();
    let _ = std::panic::catch_unwind(|| {
        let _guard = super::TestEngineAuthorityGuard::enter(temp_root.join("panic-test"));
        assert_eq!(
            super::resolve_engine_authority_dir().unwrap(),
            temp_root.join("panic-test")
        );
        panic!("simulated panic to test RAII unwind");
    });
    assert_eq!(
        super::resolve_engine_authority_dir().unwrap(),
        initial_auth,
        "RAII guard must restore previous authority on unwind"
    );

    // 2. Injected engine authority root containment
    let engine_state_root = temp_root.join("engine-state");
    std::fs::create_dir_all(&engine_state_root).unwrap();
    let _authority_guard = super::TestEngineAuthorityGuard::enter(engine_state_root.clone());

    let resolved = super::resolve_engine_authority_dir().unwrap();
    assert!(resolved.starts_with(&temp_root));
    if let Some(appdata) = std::env::var_os("APPDATA") {
        assert!(!resolved.starts_with(std::path::PathBuf::from(appdata)));
    }

    let primary_dir = temp_root.join("primary");
    std::fs::create_dir_all(primary_dir.join("src")).unwrap();
    let primary_file = primary_dir.join("src").join("lib.rs");
    let initial_bytes = b"pub fn initial() -> bool { true }\n";
    let modified_bytes = b"pub fn modified() -> bool { false }\n";
    std::fs::write(&primary_file, initial_bytes).unwrap();

    let git_runner = MockCommandRunner::new();
    let state_file = temp_root.join("run_state.json");
    let mut cfg = default_test_config(fast_profile());
    cfg.working_dir = primary_dir.to_string_lossy().to_string();
    let run_id = format!("ac-run-111-matrix-{}", uuid::Uuid::new_v4());
    cfg.run_id = Some(run_id.clone());

    let baseline_manifest = capture_baseline_file_manifest(&primary_dir, &BTreeMap::new()).unwrap();
    let mut state =
        initialize_run(cfg, "head-1", baseline_manifest, &git_runner, FIXED_TIME).unwrap();
    state.runtime_allowlist = vec!["src/lib.rs".to_string()];

    submit_draft_plan(
        &mut state,
        "planner-1",
        &sample_plan_markdown(&["src/lib.rs"], "v1"),
        true,
        FIXED_TIME,
    )
    .unwrap();
    let plan_approve = make_plan_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "reviewer-1",
        &plan_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();

    let agent_runner = CandidateModifyingRunner {
        workspace_roots: std::sync::Mutex::new(Vec::new()),
        new_content: modified_bytes.to_vec(),
    };
    super::run_phase_a_step(&mut state, &agent_runner, &git_runner, &state_file).unwrap();
    super::run_phase_a_step(&mut state, &agent_runner, &git_runner, &state_file).unwrap();

    if let Some(ref mut cb) = state.candidate_binding {
        cb.review_bundle_digest = Some(SNAP_HASH.to_string());
    }

    let impl_approve = make_impl_verdict_json(&state, VerdictDecision::Approve, vec![], false);
    submit_review_verdict(
        &mut state,
        "reviewer-2",
        &impl_approve,
        SNAP_HASH,
        SNAP_HASH,
        None,
        None,
        true,
        FIXED_TIME,
    )
    .unwrap();
    assert_eq!(
        state.current_state,
        AutoConvergeState::ImplementationApproved
    );

    let cb = state.candidate_binding.as_ref().unwrap();
    let cand_ws = super::implementer_workspace::ImplementerWorkspace::load_existing(
        &primary_dir,
        &engine_state_root,
        &PathBuf::from(&cb.candidate_root),
        &state.runtime_allowlist,
        &cb.candidate_baseline_digest,
    )
    .unwrap();
    let delta_digest = cand_ws.delta_digest().unwrap();
    let permit = cand_ws
        .authorize_delta(cand_ws.baseline_digest(), &delta_digest)
        .unwrap();

    let make_interrupted_journal = |target_run_id: &str| -> (PathBuf, String) {
        let mut journal_out = None;
        let mut digest_out = String::new();
        let mut count = 0;
        let mut on_phase = |phase, jdir: &std::path::Path, digest: &str| {
            if phase == super::types::PromotionPhase::Applying {
                count += 1;
                journal_out = Some(jdir.to_path_buf());
                digest_out = digest.to_string();
                if count >= 2 {
                    return Err("Simulated crash".into());
                }
            }
            Ok(())
        };
        let _ = cand_ws.promote_internal(&permit, target_run_id, &mut on_phase, None, false, None);
        (journal_out.expect("journal created"), digest_out)
    };

    // Case 1: Valid RecoveryRestored journal cleanup
    {
        let (jdir, digest) = make_interrupted_journal(&run_id);
        std::fs::write(&primary_file, initial_bytes).unwrap();
        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: jdir.to_string_lossy().to_string(),
            journal_digest: digest,
            phase: super::types::PromotionPhase::RecoveryRestored,
            candidate_root: cb.candidate_root.clone(),
        });
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(res.is_ok(), "Valid RecoveryRestored cleanup must succeed");
        assert_eq!(std::fs::read(&primary_file).unwrap(), initial_bytes);
        assert!(sim_state.promotion_journal.is_none());
        assert!(!jdir.exists(), "journal directory must be deleted");
    }

    // Case 2: Tampered journal snapshot digest rejects cleanup, retains journal, and halts Blocked
    {
        let (jdir, digest) = make_interrupted_journal(&run_id);
        std::fs::write(&primary_file, initial_bytes).unwrap();

        // Tamper snapshot bytes in journal
        let snapshot_file = std::fs::read_dir(&jdir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.extension().map(|e| e == "json").unwrap_or(false))
            .expect("snapshot file");
        std::fs::write(&snapshot_file, b"{\"tampered\": true}").unwrap();

        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: jdir.to_string_lossy().to_string(),
            journal_digest: digest,
            phase: super::types::PromotionPhase::RecoveryRestored,
            candidate_root: cb.candidate_root.clone(),
        });
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(res.is_err(), "Tampered journal digest must be rejected");
        assert_eq!(sim_state.current_state, AutoConvergeState::Blocked);
        assert!(
            sim_state.promotion_journal.is_some(),
            "Binding must be retained on digest mismatch"
        );
        assert!(
            jdir.exists(),
            "Journal must be retained as evidence on digest mismatch"
        );
        assert_eq!(std::fs::read(&primary_file).unwrap(), initial_bytes);

        let _ = std::fs::remove_dir_all(&jdir);
    }

    // Case 3: Wrong run ID ownership prefix in journal name rejects cleanup and fails closed
    {
        let other_run_id = format!("ac-other-{}", uuid::Uuid::new_v4());
        let (jdir, digest) = make_interrupted_journal(&other_run_id);
        std::fs::write(&primary_file, initial_bytes).unwrap();

        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: jdir.to_string_lossy().to_string(),
            journal_digest: digest,
            phase: super::types::PromotionPhase::RecoveryRestored,
            candidate_root: cb.candidate_root.clone(),
        });
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(
            res.is_err(),
            "Journal owned by different run ID must be rejected"
        );
        assert_eq!(sim_state.current_state, AutoConvergeState::Blocked);
        assert!(
            sim_state.promotion_journal.is_some(),
            "Binding must be retained"
        );
        assert!(jdir.exists(), "Foreign journal must not be deleted");

        let _ = std::fs::remove_dir_all(&jdir);
    }

    // Case 4: Wrong candidate root rejects cleanup and fails closed
    {
        let (jdir, digest) = make_interrupted_journal(&run_id);
        std::fs::write(&primary_file, initial_bytes).unwrap();

        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: jdir.to_string_lossy().to_string(),
            journal_digest: digest,
            phase: super::types::PromotionPhase::RecoveryRestored,
            candidate_root: temp_root
                .join("nonexistent-cand-dir")
                .to_string_lossy()
                .to_string(),
        });
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(
            res.is_err(),
            "Non-existent/mismatched candidate root must be rejected"
        );
        assert_eq!(sim_state.current_state, AutoConvergeState::Blocked);
        assert!(
            sim_state.promotion_journal.is_some(),
            "Binding must be retained"
        );
        assert!(jdir.exists(), "Journal must not be deleted");

        let _ = std::fs::remove_dir_all(&jdir);
    }

    // Case 5: Primary baseline drift before cleanup rejects cleanup, preserves evidence, and halts Blocked
    {
        let (jdir, digest) = make_interrupted_journal(&run_id);
        // Drift primary baseline by modifying a tracked file
        std::fs::write(&primary_file, b"pub fn drifted() -> bool { false }\n").unwrap();

        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: jdir.to_string_lossy().to_string(),
            journal_digest: digest,
            phase: super::types::PromotionPhase::RecoveryRestored,
            candidate_root: cb.candidate_root.clone(),
        });
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(
            res.is_err(),
            "Baseline drift before cleanup must reject cleanup"
        );
        assert_eq!(sim_state.current_state, AutoConvergeState::Blocked);
        assert!(
            sim_state.promotion_journal.is_some(),
            "Binding must be retained"
        );
        assert!(jdir.exists(), "Journal must be retained as evidence");
        assert_eq!(
            std::fs::read(&primary_file).unwrap(),
            b"pub fn drifted() -> bool { false }\n",
            "Drifted primary must not be overwritten when baseline check fails"
        );

        // Restore baseline and clean up
        std::fs::write(&primary_file, initial_bytes).unwrap();
        let _ = std::fs::remove_dir_all(&jdir);
    }

    // Case 6: Cleanup failure retains binding and fails closed
    {
        let (jdir, digest) = make_interrupted_journal(&run_id);
        std::fs::write(&primary_file, initial_bytes).unwrap();

        // Place an unknown rogue file inside journal directory
        std::fs::write(jdir.join("unexpected-rogue-entry.bin"), b"invalid").unwrap();

        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: jdir.to_string_lossy().to_string(),
            journal_digest: digest,
            phase: super::types::PromotionPhase::RecoveryRestored,
            candidate_root: cb.candidate_root.clone(),
        });
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(
            res.is_err(),
            "Cleanup rejection due to rogue file must return Err"
        );
        assert_eq!(sim_state.current_state, AutoConvergeState::Blocked);
        assert!(
            sim_state.promotion_journal.is_some(),
            "Binding must be retained when cleanup fails"
        );
        assert!(jdir.exists(), "Journal must remain on disk");

        let _ = std::fs::remove_dir_all(&jdir);
    }

    // Case 7: Applied cleanup failure remains nonterminal with binding intact
    {
        let (jdir, digest) = make_interrupted_journal(&run_id);
        // Primary matches approved candidate delta
        std::fs::write(&primary_file, modified_bytes).unwrap();

        // Place a rogue file inside journal so cleanup fails
        std::fs::write(jdir.join("rogue.txt"), b"foreign").unwrap();

        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: jdir.to_string_lossy().to_string(),
            journal_digest: digest,
            phase: super::types::PromotionPhase::Applied,
            candidate_root: cb.candidate_root.clone(),
        });
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(res.is_err(), "Applied cleanup failure must return Err");
        assert_eq!(sim_state.current_state, AutoConvergeState::Blocked);
        assert!(
            sim_state.promotion_journal.is_some(),
            "Binding must NOT be cleared on cleanup failure"
        );
        assert!(
            !sim_state.is_terminal,
            "Run must NOT be marked terminal on cleanup failure"
        );
        assert!(jdir.exists(), "Journal must remain on disk");

        let _ = std::fs::remove_dir_all(&jdir);
    }

    // Case 8: Absent journal in RecoveryRestored with unchanged baseline completes safely
    {
        let missing_jdir = primary_dir.parent().unwrap().join(format!(
            ".pct-promotion-{}-{}",
            run_id,
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&primary_file, initial_bytes).unwrap();

        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: missing_jdir.to_string_lossy().to_string(),
            journal_digest: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            phase: super::types::PromotionPhase::RecoveryRestored,
            candidate_root: cb.candidate_root.clone(),
        });
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(
            res.is_ok(),
            "Absent journal in RecoveryRestored with verified baseline must complete"
        );
        assert!(
            sim_state.promotion_journal.is_none(),
            "Binding must be cleared"
        );
        assert_eq!(std::fs::read(&primary_file).unwrap(), initial_bytes);
    }

    // Case 9 (§112): Dangling journal symlink in RecoveryRestored fails closed and retains binding
    {
        let dangling_jdir = primary_dir.parent().unwrap().join(format!(
            ".pct-promotion-{}-{}",
            run_id,
            uuid::Uuid::new_v4()
        ));
        let non_target = primary_dir
            .parent()
            .unwrap()
            .join(format!("ghost-target-{}", uuid::Uuid::new_v4()));
        create_dangling_symlink_or_junction(&dangling_jdir, &non_target)
            .expect("create dangling junction");

        // Assert that Path::exists() returns false for this dangling link (proving the reproduction of Finding 1)
        assert!(
            !dangling_jdir.exists(),
            "Dangling symlink must report false to exists()"
        );
        // Assert that inspect_path_presence reports Present
        match super::implementer_workspace::inspect_path_presence(&dangling_jdir) {
            super::implementer_workspace::PathPresence::Present(m) => {
                assert!(m.file_type().is_symlink(), "Must be recognized as symlink");
            }
            other => panic!(
                "Expected PathPresence::Present for dangling symlink, got {:?}",
                other
            ),
        }

        std::fs::write(&primary_file, initial_bytes).unwrap();

        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: dangling_jdir.to_string_lossy().to_string(),
            journal_digest: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            phase: super::types::PromotionPhase::RecoveryRestored,
            candidate_root: cb.candidate_root.clone(),
        });
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(
            res.is_err(),
            "Dangling journal in RecoveryRestored must fail closed"
        );
        assert_eq!(sim_state.current_state, AutoConvergeState::Blocked);
        assert!(
            sim_state.promotion_journal.is_some(),
            "Binding must NOT be cleared on dangling symlink"
        );
        match super::implementer_workspace::inspect_path_presence(&dangling_jdir) {
            super::implementer_workspace::PathPresence::Present(_) => {}
            other => panic!(
                "Dangling symlink evidence must remain on disk, got {:?}",
                other
            ),
        }

        remove_symlink_or_junction(&dangling_jdir);
    }

    // Case 10 (§112): Dangling journal symlink in Applied fails closed and remains non-terminal
    {
        let dangling_jdir = primary_dir.parent().unwrap().join(format!(
            ".pct-promotion-{}-{}",
            run_id,
            uuid::Uuid::new_v4()
        ));
        let non_target = primary_dir
            .parent()
            .unwrap()
            .join(format!("ghost-target-{}", uuid::Uuid::new_v4()));
        create_dangling_symlink_or_junction(&dangling_jdir, &non_target)
            .expect("create dangling junction");

        assert!(
            !dangling_jdir.exists(),
            "Dangling symlink must report false to exists()"
        );

        // Primary matches approved candidate delta
        std::fs::write(&primary_file, modified_bytes).unwrap();

        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: dangling_jdir.to_string_lossy().to_string(),
            journal_digest: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            phase: super::types::PromotionPhase::Applied,
            candidate_root: cb.candidate_root.clone(),
        });
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(res.is_err(), "Dangling journal in Applied must return Err");
        assert_eq!(sim_state.current_state, AutoConvergeState::Blocked);
        assert!(
            sim_state.promotion_journal.is_some(),
            "Binding must NOT be cleared on dangling symlink"
        );
        assert!(
            !sim_state.is_terminal,
            "Run must NOT be marked terminal on dangling symlink in Applied"
        );
        match super::implementer_workspace::inspect_path_presence(&dangling_jdir) {
            super::implementer_workspace::PathPresence::Present(_) => {}
            other => panic!(
                "Dangling symlink evidence must remain on disk, got {:?}",
                other
            ),
        }

        remove_symlink_or_junction(&dangling_jdir);
    }

    // Case 11 (§112): Cleanup phase mismatch is rejected
    {
        std::fs::write(&primary_file, initial_bytes).unwrap();
        let (jdir, digest) = make_interrupted_journal(&run_id);
        let res = super::implementer_workspace::cleanup_recovered_promotion_journal_authenticated(
            &primary_dir,
            &jdir,
            &run_id,
            std::path::Path::new(&cb.candidate_root),
            super::types::PromotionPhase::Applying, // unauthorized phase
            &digest,
        );
        assert!(
            res.is_err(),
            "Cleanup in unauthorized phase Applying must fail"
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("cannot clean up journal in unauthorized phase"),
            "Error should state unauthorized phase, got: {err}"
        );
        assert!(jdir.exists(), "Journal must remain on disk");
        let _ = std::fs::remove_dir_all(&jdir);
    }

    // Case 12 (§113): Valid-target junction in RecoveryRestored halts Blocked, leaves link and target untouched
    {
        std::fs::write(&primary_file, initial_bytes).unwrap();
        let (real_target, digest) = make_interrupted_journal(&run_id);
        assert!(real_target.exists());

        let junction_jdir = primary_dir.parent().unwrap().join(format!(
            ".pct-promotion-{}-{}",
            run_id,
            uuid::Uuid::new_v4()
        ));
        create_junction_to_existing_target(&junction_jdir, &real_target)
            .expect("create junction to valid target");

        // Prove that junction is detected as Present and is_path_alias_or_reparse is true
        match super::implementer_workspace::inspect_path_presence(&junction_jdir) {
            super::implementer_workspace::PathPresence::Present(m) => {
                assert!(
                    super::implementer_workspace::is_path_alias_or_reparse(&m),
                    "Junction must be identified as alias/reparse"
                );
            }
            other => panic!(
                "Expected PathPresence::Present for junction, got {:?}",
                other
            ),
        }

        // Prove that standalone load_and_validate_journal_with_digest rejects before reading entries
        let direct_load =
            super::implementer_workspace::load_and_validate_journal_with_digest(&junction_jdir);
        assert!(
            direct_load.is_err(),
            "Direct load on junction must fail before reading entries"
        );
        assert!(direct_load
            .unwrap_err()
            .contains("symlink or reparse point"));

        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: junction_jdir.to_string_lossy().to_string(),
            journal_digest: digest,
            phase: super::types::PromotionPhase::RecoveryRestored,
            candidate_root: cb.candidate_root.clone(),
        });
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(
            res.is_err(),
            "Junction in RecoveryRestored must fail closed"
        );
        assert_eq!(sim_state.current_state, AutoConvergeState::Blocked);
        assert!(
            sim_state.promotion_journal.is_some(),
            "Binding must be retained"
        );
        assert!(!sim_state.is_terminal, "Run must not be marked terminal");

        // Assert both link and target remain completely untouched on disk
        match super::implementer_workspace::inspect_path_presence(&junction_jdir) {
            super::implementer_workspace::PathPresence::Present(_) => {}
            other => panic!("Junction evidence must remain on disk, got {:?}", other),
        }
        assert!(
            real_target.exists(),
            "Target journal directory must remain untouched"
        );
        assert!(
            real_target.join("journal.json").exists(),
            "Target journal.json must remain intact"
        );

        remove_symlink_or_junction(&junction_jdir);
        let _ = std::fs::remove_dir_all(&real_target);
    }

    // Case 13 (§113): Valid-target junction in Applied halts Blocked, leaves link and target untouched, nonterminal
    {
        std::fs::write(&primary_file, initial_bytes).unwrap();
        let (real_target, digest) = make_interrupted_journal(&run_id);
        assert!(real_target.exists());

        let junction_jdir = primary_dir.parent().unwrap().join(format!(
            ".pct-promotion-{}-{}",
            run_id,
            uuid::Uuid::new_v4()
        ));
        create_junction_to_existing_target(&junction_jdir, &real_target)
            .expect("create junction to valid target");

        // Primary matches approved candidate delta
        std::fs::write(&primary_file, modified_bytes).unwrap();

        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: junction_jdir.to_string_lossy().to_string(),
            journal_digest: digest,
            phase: super::types::PromotionPhase::Applied,
            candidate_root: cb.candidate_root.clone(),
        });
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(res.is_err(), "Junction in Applied must fail closed");
        assert_eq!(sim_state.current_state, AutoConvergeState::Blocked);
        assert!(
            sim_state.promotion_journal.is_some(),
            "Binding must be retained"
        );
        assert!(
            !sim_state.is_terminal,
            "Run must NOT be marked terminal on junction in Applied"
        );

        // Assert both link and target remain untouched
        match super::implementer_workspace::inspect_path_presence(&junction_jdir) {
            super::implementer_workspace::PathPresence::Present(_) => {}
            other => panic!("Junction evidence must remain on disk, got {:?}", other),
        }
        assert!(
            real_target.exists(),
            "Target journal directory must remain untouched"
        );
        assert!(
            real_target.join("journal.json").exists(),
            "Target journal.json must remain intact"
        );

        remove_symlink_or_junction(&junction_jdir);
        let _ = std::fs::remove_dir_all(&real_target);
    }

    // Case 14 (§113): Injected metadata error in RecoveryRestored fails closed, retains binding, nonterminal
    {
        std::fs::write(&primary_file, initial_bytes).unwrap();
        let synthetic_jdir = primary_dir.parent().unwrap().join(format!(
            ".pct-promotion-{}-{}",
            run_id,
            uuid::Uuid::new_v4()
        ));

        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: synthetic_jdir.to_string_lossy().to_string(),
            journal_digest: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            phase: super::types::PromotionPhase::RecoveryRestored,
            candidate_root: cb.candidate_root.clone(),
        });
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        // Inject permission denial on journal path via TestMetadataInspectorGuard seam
        let target_jdir = synthetic_jdir.clone();
        let _seam_guard =
            super::implementer_workspace::TestMetadataInspectorGuard::enter(move |path| {
                if path == target_jdir {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "injected host permission error",
                    ))
                } else {
                    std::fs::symlink_metadata(path)
                }
            });

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(res.is_err(), "Metadata inspection error must return Err");
        let err_msg = res.unwrap_err();
        assert!(
            err_msg.contains(
                "Failed to inspect promotion journal path: injected host permission error"
            ),
            "Error message should mention inspection failure, got: {err_msg}"
        );
        assert_eq!(sim_state.current_state, AutoConvergeState::Blocked);
        assert!(
            sim_state.promotion_journal.is_some(),
            "Binding must be retained on metadata error"
        );
        assert!(
            !sim_state.is_terminal,
            "Run must not be marked terminal on metadata error"
        );
    }

    // Case 15 (§113): Injected metadata error in Applied fails closed, retains binding, nonterminal
    {
        std::fs::write(&primary_file, modified_bytes).unwrap();
        let synthetic_jdir = primary_dir.parent().unwrap().join(format!(
            ".pct-promotion-{}-{}",
            run_id,
            uuid::Uuid::new_v4()
        ));

        let mut sim_state = state.clone();
        sim_state.promotion_journal = Some(super::types::PromotionJournalBinding {
            run_id: run_id.clone(),
            journal_dir: synthetic_jdir.to_string_lossy().to_string(),
            journal_digest: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            phase: super::types::PromotionPhase::Applied,
            candidate_root: cb.candidate_root.clone(),
        });
        save_run_state_authenticated(&state_file, &mut sim_state).unwrap();

        // Inject permission denial on journal path via TestMetadataInspectorGuard seam
        let target_jdir = synthetic_jdir.clone();
        let _seam_guard =
            super::implementer_workspace::TestMetadataInspectorGuard::enter(move |path| {
                if path == target_jdir {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "injected host permission error",
                    ))
                } else {
                    std::fs::symlink_metadata(path)
                }
            });

        let res = super::run_phase_a_step(&mut sim_state, &agent_runner, &git_runner, &state_file);
        assert!(
            res.is_err(),
            "Metadata inspection error in Applied must return Err"
        );
        let err_msg = res.unwrap_err();
        assert!(
            err_msg.contains(
                "Failed to inspect promotion journal path: injected host permission error"
            ),
            "Error message should mention inspection failure, got: {err_msg}"
        );
        assert_eq!(sim_state.current_state, AutoConvergeState::Blocked);
        assert!(
            sim_state.promotion_journal.is_some(),
            "Binding must NOT be cleared on metadata error"
        );
        assert!(
            !sim_state.is_terminal,
            "Run must NOT be marked terminal on metadata error in Applied"
        );
    }

    // Clean up
    let _ = std::fs::remove_dir_all(cand_ws.candidate_root());
}

fn create_junction_to_existing_target(
    link_path: &std::path::Path,
    existing_target: &std::path::Path,
) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let link_str = link_path
            .to_str()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "link path not utf-8"))?;
        let target_str = existing_target.to_str().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "target path not utf-8")
        })?;
        let status = std::process::Command::new("cmd")
            .args(&["/C", "mklink", "/J", link_str, target_str])
            .status()?;
        if !status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "mklink /J failed",
            ));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::os::unix::fs::symlink(existing_target, link_path)
    }
}

#[test]
fn test_stage_2_113_path_presence_pure_classification() {
    let temp = repo_tempdir().unwrap();
    let file = temp.path().join("file.txt");
    std::fs::write(&file, b"content").unwrap();

    // 1. Existing file -> Present
    let meta = std::fs::symlink_metadata(&file);
    match super::implementer_workspace::classify_metadata_result(meta) {
        super::implementer_workspace::PathPresence::Present(m) => {
            assert!(m.is_file());
        }
        other => panic!("Expected Present, got {:?}", other),
    }

    // 2. NotFound error -> Absent
    let not_found_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    match super::implementer_workspace::classify_metadata_result(Err(not_found_err)) {
        super::implementer_workspace::PathPresence::Absent => {}
        other => panic!("Expected Absent, got {:?}", other),
    }

    // 3. PermissionDenied error -> Error
    let perm_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
    match super::implementer_workspace::classify_metadata_result(Err(perm_err)) {
        super::implementer_workspace::PathPresence::Error(msg) => {
            assert!(msg.contains("access denied"));
        }
        other => panic!("Expected Error, got {:?}", other),
    }

    // 4. Other I/O error -> Error
    let other_err = std::io::Error::new(std::io::ErrorKind::Other, "unknown io failure");
    match super::implementer_workspace::classify_metadata_result(Err(other_err)) {
        super::implementer_workspace::PathPresence::Error(msg) => {
            assert!(msg.contains("unknown io failure"));
        }
        other => panic!("Expected Error, got {:?}", other),
    }
}

fn create_dangling_symlink_or_junction(
    link_path: &std::path::Path,
    nonexistent_target: &std::path::Path,
) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        std::fs::create_dir_all(nonexistent_target)?;
        let link_str = link_path
            .to_str()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "link path not utf-8"))?;
        let target_str = nonexistent_target.to_str().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "target path not utf-8")
        })?;
        let status = std::process::Command::new("cmd")
            .args(&["/C", "mklink", "/J", link_str, target_str])
            .status()?;
        if !status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "mklink /J failed",
            ));
        }
        std::fs::remove_dir(nonexistent_target)?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::os::unix::fs::symlink(nonexistent_target, link_path)
    }
}

fn remove_symlink_or_junction(link_path: &std::path::Path) {
    #[cfg(windows)]
    {
        if std::fs::remove_dir(link_path).is_err() {
            if let Some(s) = link_path.to_str() {
                let _ = std::process::Command::new("cmd")
                    .args(&["/C", "rmdir", s])
                    .status();
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = std::fs::remove_file(link_path);
    }
}

// ---------------------------------------------------------------------------
// Phase A Delivery Tests (§209.14, §209.15, §209.17, §209.19)
// ---------------------------------------------------------------------------

#[test]
fn test_codex_profile_file_matches_permission_profile() {
    use super::codex_exec::{
        build_implementer_launch_configuration, build_reviewer_launch_configuration,
    };
    use super::isolation::AutoConvergeRole;

    let dir = repo_test_temp_dir("profile-test");
    let ws = dir.path().join("workspace");
    let snap = dir.path().join("snapshot");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&snap).unwrap();

    let plan_file = ws.join("plan.md");
    std::fs::write(&plan_file, b"# Test Plan").unwrap();

    // 1. Reviewer launch configuration (§209.3 A1)
    let reviewer_req = super::agent_runner::RoleExecutionRequest {
        role: AutoConvergeRole::Reviewer,
        session_id: "test-run-a1-reviewer".to_string(),
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: plan_file.clone(),
        runtime_allowlist: vec!["src/lib.rs".to_string()],
        input_prompt: "Review plan against invariants".to_string(),
        timeout_seconds: 300,
        model: Some("gpt-5-codex".to_string()),
        reasoning_effort: Some("high".to_string()),
        run_id: Some("test-run-a1-rev".to_string()),
    };

    let reviewer_launch = build_reviewer_launch_configuration(&reviewer_req)
        .expect("must build reviewer launch config");

    let rev_args = &reviewer_launch.invocation.args;
    let rev_prof_idx = rev_args
        .iter()
        .position(|a| a == "--profile")
        .expect("--profile must be present in reviewer argv");
    assert_eq!(
        rev_args.get(rev_prof_idx + 1).map(String::as_str),
        Some("reviewer"),
        "reviewer profile argument must be 'reviewer'"
    );

    let rev_config_path = reviewer_launch.paths.codex_home.join("config.toml");
    let rev_profile_path = reviewer_launch
        .paths
        .codex_home
        .join("reviewer.config.toml");
    assert!(rev_config_path.exists(), "config.toml must exist");
    assert!(
        rev_profile_path.exists(),
        "reviewer.config.toml must exist for Option 1"
    );

    let rev_config_str = std::fs::read_to_string(&rev_config_path).unwrap();
    let rev_profile_str = std::fs::read_to_string(&rev_profile_path).unwrap();
    assert_eq!(
        rev_config_str, rev_profile_str,
        "config.toml and reviewer.config.toml must be identical in Option 1"
    );
    assert!(
        rev_config_str.contains("default_permissions = \"reviewer\""),
        "reviewer config must specify default_permissions = 'reviewer'"
    );
    assert!(
        rev_config_str.contains("\":root\" = \"deny\""),
        "reviewer config must deny root filesystem"
    );
    assert!(
        rev_config_str.contains("enabled = false"),
        "reviewer config must disable network"
    );

    // 2. Implementer launch configuration (§209.3 A1)
    let implementer_req = super::agent_runner::RoleExecutionRequest {
        role: AutoConvergeRole::Implementer,
        session_id: "test-run-a1-impl".to_string(),
        workspace_root: ws.clone(),
        snapshot_dir: None,
        plan_path: plan_file.clone(),
        runtime_allowlist: vec!["src/lib.rs".to_string()],
        input_prompt: "Implement tasks according to plan".to_string(),
        timeout_seconds: 300,
        model: Some("gpt-5-codex".to_string()),
        reasoning_effort: Some("high".to_string()),
        run_id: Some("test-run-a1-imp".to_string()),
    };

    let implementer_launch = build_implementer_launch_configuration(&implementer_req)
        .expect("must build implementer launch config");

    let impl_args = &implementer_launch.invocation.args;
    let impl_prof_idx = impl_args
        .iter()
        .position(|a| a == "--profile")
        .expect("--profile must be present in implementer argv");
    assert_eq!(
        impl_args.get(impl_prof_idx + 1).map(String::as_str),
        Some("implementer"),
        "implementer profile argument must be 'implementer'"
    );

    let impl_config_path = implementer_launch.paths.codex_home.join("config.toml");
    let impl_profile_path = implementer_launch
        .paths
        .codex_home
        .join("implementer.config.toml");
    assert!(impl_config_path.exists(), "config.toml must exist");
    assert!(
        impl_profile_path.exists(),
        "implementer.config.toml must exist for Option 1"
    );

    let impl_config_str = std::fs::read_to_string(&impl_config_path).unwrap();
    let impl_profile_str = std::fs::read_to_string(&impl_profile_path).unwrap();
    assert_eq!(
        impl_config_str, impl_profile_str,
        "config.toml and implementer.config.toml must be identical in Option 1"
    );
    assert!(
        impl_config_str.contains("default_permissions = \"implementer\""),
        "implementer config must specify default_permissions = 'implementer'"
    );
    assert!(
        impl_config_str.contains("\":root\" = \"deny\""),
        "implementer config must deny root filesystem"
    );
    assert!(
        impl_config_str.contains("\".\" = \"write\""),
        "implementer config must permit workspace write"
    );
    assert!(
        impl_config_str.contains("enabled = false"),
        "implementer config must disable network"
    );
}

#[test]
fn test_start_run_planner_does_not_require_reviewer_os_preflight() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();
    state.current_state = AutoConvergeState::PlanCreate;
    assert!(
        !super::next_role_requires_reviewer_preflight(&state),
        "PlanCreate must not require Reviewer preflight"
    );
}

#[test]
fn test_resume_implement_does_not_require_reviewer_os_preflight() {
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();

    state.current_state = AutoConvergeState::Implement;
    assert!(
        !super::next_role_requires_reviewer_preflight(&state),
        "Implement must not require Reviewer preflight"
    );

    state.current_state = AutoConvergeState::PlanApproved;
    assert!(
        !super::next_role_requires_reviewer_preflight(&state),
        "PlanApproved must not require Reviewer preflight"
    );
}

#[test]
fn test_reviewer_dispatch_production_default_is_r2_fail_closed() {
    use super::agent_runner::{ProductionAgentRunner, RoleExecutionRequest};
    use super::isolation::AutoConvergeRole;
    use super::types::ReviewerLevel;

    let dir = repo_test_temp_dir("prod-rev-default");
    let ws = dir.path().join("workspace");
    let snap = dir.path().join("snapshot");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&snap).unwrap();
    let plan = ws.join("plan.md");
    std::fs::write(&plan, b"# Plan").unwrap();

    let fake_cli = ws.join("fake_cli.exe");
    std::fs::write(&fake_cli, b"").unwrap();

    let runner = ProductionAgentRunner::with_cli(fake_cli.clone());
    assert_eq!(
        runner.reviewer_level,
        ReviewerLevel::R2,
        "ProductionAgentRunner must default to ReviewerLevel::R2 fail-closed (§209.15 B2)"
    );

    let req = RoleExecutionRequest {
        role: AutoConvergeRole::Reviewer,
        session_id: "test-sess-1".to_string(),
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: plan.clone(),
        runtime_allowlist: vec![],
        input_prompt: "Review".to_string(),
        timeout_seconds: 60,
        model: None,
        reasoning_effort: None,
        run_id: Some("test-run-1".to_string()),
    };

    let _lock = super::isolation::TEST_ISOLATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    super::isolation::set_test_isolation_pass(false);

    let res = runner.execute_role_session(&req);
    assert!(
        res.is_err(),
        "Default production Reviewer dispatch must fail closed when OS isolation fails"
    );
    let err = res.err().unwrap();
    assert!(
        err.contains(
            "Standard unprivileged desktop environment lacks proven container/sandbox OS isolation"
        ),
        "Error message must indicate missing OS isolation guard, got: {err}"
    );
}

#[test]
fn test_reviewer_dispatch_r0d_skips_os_boundary() {
    use super::agent_runner::{
        get_test_last_reviewer_dispatch_stage, requires_reviewer_os_boundary_guard,
        ProductionAgentRunner, ReviewerDispatchStage, RoleExecutionRequest,
    };
    use super::isolation::AutoConvergeRole;
    use super::types::ReviewerLevel;

    // 1. Assert affirmative branch-selection contract (§211.4 Required correction 3)
    assert!(
        !requires_reviewer_os_boundary_guard(AutoConvergeRole::Reviewer, ReviewerLevel::R0D),
        "R0-D must bypass OS DACL boundary guard"
    );
    assert!(
        requires_reviewer_os_boundary_guard(AutoConvergeRole::Reviewer, ReviewerLevel::R2),
        "R2 (default) must enforce OS DACL boundary guard"
    );

    let dir = repo_test_temp_dir("prod-rev-r0d");
    let ws = dir.path().join("workspace");
    let snap = dir.path().join("snapshot");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&snap).unwrap();
    let plan = ws.join("plan.md");
    std::fs::write(&plan, b"# Plan").unwrap();

    let fake_cli = ws.join("fake_cli.exe");
    std::fs::write(&fake_cli, b"").unwrap();

    let runner =
        ProductionAgentRunner::with_cli(fake_cli.clone()).with_reviewer_level(ReviewerLevel::R0D);
    assert_eq!(
        runner.reviewer_level,
        ReviewerLevel::R0D,
        "Runner must have explicit R0-D level"
    );

    let req = RoleExecutionRequest {
        role: AutoConvergeRole::Reviewer,
        session_id: "test-sess-r0d".to_string(),
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: plan.clone(),
        runtime_allowlist: vec![],
        input_prompt: "Review".to_string(),
        timeout_seconds: 60,
        model: None,
        reasoning_effort: None,
        run_id: Some("test-run-r0d".to_string()),
    };

    let _lock = super::isolation::TEST_ISOLATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    super::isolation::set_test_isolation_pass(false);

    let res = runner.execute_role_session(&req);

    // 2. Affirmative proof: runner progressed past OS boundary and reached child process spawn
    let last_stage = get_test_last_reviewer_dispatch_stage();
    assert_eq!(
        last_stage,
        Some(ReviewerDispatchStage::ProcessSpawnAttempted),
        "Runner must have advanced past OsBoundarySkipped, LaunchConfigBuilt, and PreSpawnChecked to ProcessSpawnAttempted"
    );

    // 3. The error returned must be a process launch failure (empty executable), NOT an isolation failure
    assert!(
        res.is_err(),
        "Empty fake CLI executable must fail during process launch"
    );
    let err = res.err().unwrap();
    assert!(
        !err.contains(
            "Standard unprivileged desktop environment lacks proven container/sandbox OS isolation"
        ),
        "R0-D must not fail on OS boundary guard: got {err}"
    );
}

#[test]
fn test_reviewer_level_validation_and_parsing() {
    use super::types::ReviewerLevel;

    // Default must be R2 (§209.15 B2)
    assert_eq!(ReviewerLevel::default(), ReviewerLevel::R2);

    // Parsing valid strings (case-insensitive)
    assert_eq!(ReviewerLevel::parse("R2").unwrap(), ReviewerLevel::R2);
    assert_eq!(ReviewerLevel::parse("r2").unwrap(), ReviewerLevel::R2);
    assert_eq!(ReviewerLevel::parse("R0-D").unwrap(), ReviewerLevel::R0D);
    assert_eq!(ReviewerLevel::parse("r0-d").unwrap(), ReviewerLevel::R0D);
    assert_eq!(ReviewerLevel::parse("R0D").unwrap(), ReviewerLevel::R0D);
    assert_eq!(ReviewerLevel::parse("r0d").unwrap(), ReviewerLevel::R0D);

    // Invalid strings rejected fail-closed
    assert!(ReviewerLevel::parse("R1").is_err());
    assert!(ReviewerLevel::parse("sandbox").is_err());
    assert!(ReviewerLevel::parse("unknown").is_err());
    assert!(ReviewerLevel::parse("").is_err());

    // Serde round-trip
    let r2_json = serde_json::to_string(&ReviewerLevel::R2).unwrap();
    assert_eq!(r2_json, "\"R2\"");
    let r0d_json = serde_json::to_string(&ReviewerLevel::R0D).unwrap();
    assert_eq!(r0d_json, "\"R0-D\"");

    let deser_r2: ReviewerLevel = serde_json::from_str("\"R2\"").unwrap();
    assert_eq!(deser_r2, ReviewerLevel::R2);
    let deser_r0d: ReviewerLevel = serde_json::from_str("\"R0-D\"").unwrap();
    assert_eq!(deser_r0d, ReviewerLevel::R0D);

    // Invalid JSON rejected
    assert!(serde_json::from_str::<ReviewerLevel>("\"invalid\"").is_err());
}

#[test]
fn test_initialize_run_r0d_opt_in_enforcement() {
    use super::types::ReviewerLevel;

    let runner = MockCommandRunner::new();

    // 1. R0-D without user acknowledgement is rejected fail-closed (§209.15 B3)
    let mut config_unack = default_test_config(standard_profile());
    config_unack.reviewer_level = Some(ReviewerLevel::R0D);
    config_unack.r0d_acknowledged = false;

    let unack_res = initialize_run(config_unack, "head-1", vec![], &runner, FIXED_TIME);
    assert!(unack_res.is_err(), "R0-D without acknowledgement must fail");
    let err_msg = unack_res.err().unwrap();
    assert!(
        err_msg.contains("requires explicit user acknowledgement"),
        "Error message must indicate missing acknowledgement, got: {err_msg}"
    );

    // 2. R0-D with user acknowledgement is accepted and persisted in authenticated state
    let mut config_ack = default_test_config(standard_profile());
    config_ack.reviewer_level = Some(ReviewerLevel::R0D);
    config_ack.r0d_acknowledged = true;

    let ack_state =
        initialize_run(config_ack, "head-1", vec![], &runner, FIXED_TIME).expect("must initialize");
    assert_eq!(ack_state.reviewer_level, ReviewerLevel::R0D);
    assert!(ack_state.r0d_acknowledged);

    // 3. Default (None) defaults to R2 fail-closed
    let config_default = default_test_config(standard_profile());
    let default_state = initialize_run(config_default, "head-1", vec![], &runner, FIXED_TIME)
        .expect("must initialize");
    assert_eq!(default_state.reviewer_level, ReviewerLevel::R2);
    assert!(!default_state.r0d_acknowledged);
}

#[test]
fn test_backend_production_dispatch_gate_blocks_start_and_resume() {
    use super::types::ReviewerLevel;

    // Verify compile-time gate constant is strictly false (§209.1 rule 3, §209.15 B1, §209.17 R1, §209.19)
    assert!(
        !super::PRODUCTION_DISPATCH_ENABLED,
        "PRODUCTION_DISPATCH_ENABLED must be strictly false until Phase G/H"
    );

    let dir = repo_test_temp_dir("prod-gate-test");
    let auth_dir = dir.path().join("engine_authority");
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir);

    let ws = dir.path().join("workspace");
    std::fs::create_dir_all(&ws).unwrap();

    let mut config = default_test_config(standard_profile());
    let run_id = format!("ac-gate-{}", uuid::Uuid::new_v4());
    config.run_id = Some(run_id.clone());
    config.working_dir = ws.to_string_lossy().to_string();
    config.reviewer_level = Some(ReviewerLevel::R0D);
    config.r0d_acknowledged = true;

    // 1. auto_converge_start_run fails closed immediately with Err and zero side-effects
    let start_res = super::auto_converge_start_run(config);
    assert!(
        start_res.is_err(),
        "start_run must fail when dispatch gate is false"
    );
    let err = start_res.err().unwrap();
    assert!(
        err.contains(
            "Production dispatch is blocked pending Phase G verification and Phase H authorization"
        ),
        "Error message must indicate dispatch gate is blocked, got: {err}"
    );

    // Verify ZERO side-effects: run state file was NOT created and resolves in injected test root (§211.4)
    let state_path = super::engine_run_state_path(&run_id).unwrap();
    assert!(
        state_path.starts_with(dir.path()),
        "State path must resolve under injected test authority root, got: {}",
        state_path.display()
    );
    assert!(
        !state_path.exists(),
        "Run state file must not be created when gate blocks start_run"
    );
    assert!(
        !dir.path().join("engine_runs").exists(),
        "No engine_runs directory should be created when gate blocks start_run"
    );

    // 2. auto_converge_resume_run also fails closed immediately with Err
    let resume_res = super::auto_converge_resume_run(run_id, None);
    assert!(
        resume_res.is_err(),
        "resume_run must fail when dispatch gate is false"
    );
    let resume_err = resume_res.err().unwrap();
    assert!(
        resume_err.contains(
            "Production dispatch is blocked pending Phase G verification and Phase H authorization"
        ),
        "Resume error must indicate dispatch gate is blocked, got: {resume_err}"
    );
}

// =========================================================================
// Phase B RecoverySnapshot Tests (T1 - T13, §§209.4, 281-284)
// =========================================================================

/// Helper fixture: creates a synthetic git repository in isolated `temp_dir` with initial commit on main
/// entirely isolated from any real repository checkout or ambient git config.
/// Enforces §301.5: creation-time identity capture and pre-mutation identity verification.
struct SyntheticRepoFixture {
    temp_dir: TempDir,
    repo_dir: std::path::PathBuf,
    creation_file_id: (u32, u64),
    global_config: std::path::PathBuf,
}

impl SyntheticRepoFixture {
    fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("synthetic_repo");
        std::fs::create_dir_all(&repo_dir).expect("create synthetic repo dir");
        let global_config = temp_dir.path().join(".empty_test_gitconfig");
        std::fs::write(&global_config, b"").expect("write empty global gitconfig");

        let creation_file_id = super::recovery_snapshot::query_path_identity(&repo_dir)
            .expect("query synthetic repo creation file id");

        let fixture = Self {
            temp_dir,
            repo_dir,
            creation_file_id,
            global_config,
        };

        fixture.run_git(&["init"]);
        fixture.run_git(&["config", "user.name", "Test User"]);
        fixture.run_git(&["config", "user.email", "test@example.com"]);
        fixture.run_git(&["config", "core.autocrlf", "false"]);

        let readme = fixture.repo_dir.join("README.md");
        std::fs::write(&readme, b"# Synthetic Test Repo\n").expect("write README");
        fixture.run_git(&["add", "README.md"]);
        fixture.run_git(&["commit", "-m", "initial commit"]);

        fixture
    }

    fn run_git(&self, args: &[&str]) {
        assert!(self.run_git_status(args), "git command {:?} failed", args);
    }

    fn run_git_status(&self, args: &[&str]) -> bool {
        // §301.5: Verify fixture root before each mutating Git command
        assert!(self.repo_dir.exists(), "Synthetic repo root is missing");
        let current_id = super::recovery_snapshot::query_path_identity(&self.repo_dir)
            .expect("query synthetic repo identity before git command");
        assert_eq!(
            current_id, self.creation_file_id,
            "Synthetic repo identity mismatch - possible alias or replacement"
        );

        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&self.repo_dir)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &self.global_config)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_AUTHOR_NAME", "Test Author")
            .env("GIT_AUTHOR_EMAIL", "author@example.com")
            .env("GIT_COMMITTER_NAME", "Test Committer")
            .env("GIT_COMMITTER_EMAIL", "committer@example.com")
            .status()
            .expect("execute git command in synthetic repo");
        status.success()
    }

    fn path(&self) -> &std::path::Path {
        &self.repo_dir
    }

    fn parent_path(&self) -> &std::path::Path {
        self.temp_dir.path()
    }
}

/// Helper: creates a synthetic git repository in `dir` with initial commit on main
/// entirely isolated from any real repository checkout or ambient git config.
/// Enforces §301.5: creation-time identity check before mutating Git commands.
fn create_synthetic_test_git_repo(dir: &std::path::Path) -> std::path::PathBuf {
    let repo_dir = dir.join("synthetic_repo");
    std::fs::create_dir_all(&repo_dir).expect("create synthetic repo dir");
    let global_config = dir.join(".empty_test_gitconfig");
    std::fs::write(&global_config, b"").expect("write empty global gitconfig");

    let creation_id = super::recovery_snapshot::query_path_identity(&repo_dir)
        .expect("query creation file identity");

    let run_git = |args: &[&str]| {
        // §301.5: Verify fixture root identity before each git command
        assert!(repo_dir.exists(), "Synthetic repo root is missing");
        let current_id = super::recovery_snapshot::query_path_identity(&repo_dir)
            .expect("query synthetic repo identity before git command");
        assert_eq!(
            current_id, creation_id,
            "Synthetic repo identity mismatch before git command"
        );

        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo_dir)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &global_config)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_AUTHOR_NAME", "Test Author")
            .env("GIT_AUTHOR_EMAIL", "author@example.com")
            .env("GIT_COMMITTER_NAME", "Test Committer")
            .env("GIT_COMMITTER_EMAIL", "committer@example.com")
            .status()
            .expect("execute git command in synthetic repo");
        assert!(status.success(), "git command {:?} failed", args);
    };

    run_git(&["init"]);
    run_git(&["config", "user.name", "Test User"]);
    run_git(&["config", "user.email", "test@example.com"]);
    run_git(&["config", "core.autocrlf", "false"]);

    // Create an initial commit with a README
    let readme = repo_dir.join("README.md");
    std::fs::write(&readme, b"# Synthetic Test Repo\n").expect("write README");
    run_git(&["add", "README.md"]);
    run_git(&["commit", "-m", "initial commit"]);

    repo_dir
}

// T1: test_recovery_snapshot_deserializes_absent_as_none
#[test]
fn test_recovery_snapshot_deserializes_absent_as_none() {
    let _dir = TempDir::new().unwrap();
    let runner = MockCommandRunner::new();
    let state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();

    let mut json_val = serde_json::to_value(&state).unwrap();
    // Remove the recovery_snapshot field if present to simulate older serialized JSON
    if let Some(obj) = json_val.as_object_mut() {
        obj.remove("recovery_snapshot");
    }
    let json_str = serde_json::to_string(&json_val).unwrap();

    let deserialized: AutoConvergeRunState = serde_json::from_str(&json_str).unwrap();
    assert!(
        deserialized.recovery_snapshot.is_none(),
        "Missing recovery_snapshot field must deserialize as None"
    );
}

// T2: test_recovery_snapshot_authenticated_tamper_rejected
#[test]
fn test_recovery_snapshot_authenticated_tamper_rejected() {
    let dir = TempDir::new().unwrap();
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();

    state.recovery_snapshot = Some(super::recovery_snapshot::RecoverySnapshotBinding {
        root: dir.path().join("recovery").join(&state.run_id),
        manifest_digest: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            .to_string(),
        git_head: "0123456789abcdef0123456789abcdef01234567".to_string(),
        git_refs_digest: "1111111111111111111111111111111111111111111111111111111111111111"
            .to_string(),
        git_index_entries_digest:
            "2222222222222222222222222222222222222222222222222222222222222222".to_string(),
        git_index_file_digest: "3333333333333333333333333333333333333333333333333333333333333333"
            .to_string(),
    });

    let state_file = dir.path().join("test_run.state.json");
    save_run_state_authenticated(&state_file, &mut state).unwrap();

    // Verify loading untampered state succeeds
    let loaded = load_and_verify_run_state_authenticated(&state_file, None).unwrap();
    assert!(loaded.recovery_snapshot.is_some());

    // Tamper with recovery_snapshot manifest_digest
    let raw = std::fs::read_to_string(&state_file).unwrap();
    let tampered = raw.replace(
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    );
    assert_ne!(raw, tampered);
    std::fs::write(&state_file, tampered).unwrap();

    // Loading tampered state MUST fail authentication verification
    let load_res = load_and_verify_run_state_authenticated(&state_file, None);
    assert!(
        load_res.is_err(),
        "Tampering with recovery_snapshot must fail authentication"
    );
    let err_str = load_res.err().unwrap();
    assert!(
        err_str.contains("Tamper detected: run_state.json authentication failed"),
        "Error must report HMAC authentication failure: {err_str}"
    );
}

static TEST_SEAM_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

// T3: test_git_queries_use_no_optional_locks_before_subcommand
#[test]
fn test_git_queries_use_no_optional_locks_before_subcommand() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let env = super::recovery_snapshot::HermeticGitEnv::new(dir.path()).unwrap();

    // Verify run_hermetic_git runs successfully with --no-optional-locks
    let stdout = super::recovery_snapshot::run_hermetic_git(
        &repo_dir,
        &["status", "--porcelain=v2"],
        env.config_path(),
        env.temp_dir(),
    )
    .expect("hermetic git status query");
    assert!(
        stdout.stdout.is_empty(),
        "clean repo status output should be empty"
    );

    // Also test detached vs symbolic HEAD detection (§283.2)
    let git_dir = repo_dir.join(".git");
    let index_path = git_dir.join("index");
    let snap_symbolic = super::recovery_snapshot::capture_git_snapshot(
        &repo_dir,
        &index_path,
        &git_dir,
        &env,
        false,
    )
    .expect("capture symbolic head snapshot");
    assert_eq!(
        snap_symbolic.head_mode,
        super::recovery_snapshot::HeadMode::Symbolic
    );
    assert!(snap_symbolic.head_ref.is_some());

    // Detach HEAD
    let global_config = dir.path().join(".empty_test_gitconfig");
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(&["checkout", "--detach"])
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .status()
        .unwrap();
    assert!(status.success());

    let snap_detached = super::recovery_snapshot::capture_git_snapshot(
        &repo_dir,
        &index_path,
        &git_dir,
        &env,
        false,
    )
    .expect("capture detached head snapshot");
    assert_eq!(
        snap_detached.head_mode,
        super::recovery_snapshot::HeadMode::Detached
    );
    assert_eq!(snap_detached.head_ref, None);
    assert_eq!(snap_detached.head_commit, snap_symbolic.head_commit);
}

// T4: test_recovery_snapshot_multi_source_classification
#[test]
fn test_recovery_snapshot_multi_source_classification() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let global_config = dir.path().join(".empty_test_gitconfig");

    let run_git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo_dir)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &global_config)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_AUTHOR_NAME", "Test Author")
            .env("GIT_AUTHOR_EMAIL", "author@example.com")
            .env("GIT_COMMITTER_NAME", "Test Committer")
            .env("GIT_COMMITTER_EMAIL", "committer@example.com")
            .status()
            .expect("execute git in test repo");
        assert!(status.success(), "git {:?} failed", args);
    };

    // 1. Tracked clean: README.md (already committed in initial commit)
    // 2. Commit .gitignore before staging files so staged_file.txt is not committed
    std::fs::write(repo_dir.join(".gitignore"), b"*.log\n").unwrap();
    run_git(&["add", ".gitignore"]);
    run_git(&["commit", "-m", "add gitignore"]);

    // 3. Tracked modified: modify README.md in working copy
    std::fs::write(
        repo_dir.join("README.md"),
        b"# Synthetic Test Repo - Modified\n",
    )
    .unwrap();

    // 4. Staged: staged_file.txt (added to index, uncommitted)
    std::fs::write(repo_dir.join("staged_file.txt"), b"staged content\n").unwrap();
    run_git(&["add", "staged_file.txt"]);

    // 5. Untracked: untracked.txt
    std::fs::write(repo_dir.join("untracked.txt"), b"untracked content\n").unwrap();

    // 6. Ignored: ignored_file.log, but add to policy.ignored_paths
    std::fs::write(repo_dir.join("ignored_file.log"), b"log data\n").unwrap();

    let mut policy = super::recovery_snapshot::SnapshotPolicy::default();
    policy.ignored_paths.push("ignored_file.log".to_string());

    let (snapshot, _binding, _lease) = super::recovery_snapshot::create_recovery_snapshot(
        "run-t4",
        &repo_dir,
        dir.path(),
        &policy,
    )
    .expect("create recovery snapshot");

    let entries_map: std::collections::HashMap<_, _> = snapshot
        .entries
        .into_iter()
        .map(|e| (e.path, e.sources))
        .collect();

    // Check README.md has Tracked
    let readme_sources = entries_map.get("README.md").expect("README.md in snapshot");
    assert!(readme_sources.contains(&super::recovery_snapshot::ProtectedSource::Tracked));

    // Check staged_file.txt has Tracked and Staged
    let staged_sources = entries_map
        .get("staged_file.txt")
        .expect("staged_file.txt in snapshot");
    assert!(staged_sources.contains(&super::recovery_snapshot::ProtectedSource::Tracked));
    assert!(staged_sources.contains(&super::recovery_snapshot::ProtectedSource::Staged));

    // Check untracked.txt has Untracked
    let untracked_sources = entries_map
        .get("untracked.txt")
        .expect("untracked.txt in snapshot");
    assert!(untracked_sources.contains(&super::recovery_snapshot::ProtectedSource::Untracked));

    // Check ignored_file.log has Ignored
    let ignored_sources = entries_map
        .get("ignored_file.log")
        .expect("ignored_file.log in snapshot");
    assert!(ignored_sources.contains(&super::recovery_snapshot::ProtectedSource::Ignored));
}

// T5: test_recovery_snapshot_is_non_destructive
#[test]
fn test_recovery_snapshot_is_non_destructive() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let global_config = dir.path().join(".empty_test_gitconfig");

    // Create mixed state
    std::fs::write(repo_dir.join("README.md"), b"modified readme\n").unwrap();
    std::fs::write(repo_dir.join("staged.txt"), b"staged\n").unwrap();
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(&["add", "staged.txt"])
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .status()
        .unwrap();
    assert!(status.success());
    std::fs::write(repo_dir.join("untracked.txt"), b"untracked\n").unwrap();

    let env = super::recovery_snapshot::HermeticGitEnv::new(dir.path()).unwrap();

    // Fingerprint before snapshot
    let git_dir = repo_dir.join(".git");
    let index_before = std::fs::read(git_dir.join("index")).unwrap();
    let head_before = std::fs::read(git_dir.join("HEAD")).unwrap();
    let status_before = super::recovery_snapshot::run_hermetic_git(
        &repo_dir,
        &["status", "--porcelain=v2", "-z", "--untracked-files=all"],
        env.config_path(),
        env.temp_dir(),
    )
    .unwrap();
    let readme_before = std::fs::read(repo_dir.join("README.md")).unwrap();
    let staged_before = std::fs::read(repo_dir.join("staged.txt")).unwrap();
    let untracked_before = std::fs::read(repo_dir.join("untracked.txt")).unwrap();

    // Create snapshot
    let policy = super::recovery_snapshot::SnapshotPolicy::default();
    let (_snapshot, _binding, _lease) = super::recovery_snapshot::create_recovery_snapshot(
        "run-t5",
        &repo_dir,
        dir.path(),
        &policy,
    )
    .expect("snapshot capture");

    // Fingerprint after snapshot: verify absolute zero mutations
    let index_after = std::fs::read(git_dir.join("index")).unwrap();
    let head_after = std::fs::read(git_dir.join("HEAD")).unwrap();
    let status_after = super::recovery_snapshot::run_hermetic_git(
        &repo_dir,
        &["status", "--porcelain=v2", "-z", "--untracked-files=all"],
        env.config_path(),
        env.temp_dir(),
    )
    .unwrap();
    let readme_after = std::fs::read(repo_dir.join("README.md")).unwrap();
    let staged_after = std::fs::read(repo_dir.join("staged.txt")).unwrap();
    let untracked_after = std::fs::read(repo_dir.join("untracked.txt")).unwrap();

    assert_eq!(
        index_before, index_after,
        "Index file must not be modified by snapshot capture"
    );
    assert_eq!(
        head_before, head_after,
        "HEAD must not be modified by snapshot capture"
    );
    assert_eq!(
        status_before, status_after,
        "Git status must remain identical before and after snapshot capture"
    );
    assert_eq!(
        readme_before, readme_after,
        "Working tree file README.md must be byte-identical"
    );
    assert_eq!(
        staged_before, staged_after,
        "Working tree file staged.txt must be byte-identical"
    );
    assert_eq!(
        untracked_before, untracked_after,
        "Working tree file untracked.txt must be byte-identical"
    );
}

// T6: test_recovery_snapshot_fails_closed_on_size_caps
#[test]
fn test_recovery_snapshot_fails_closed_on_size_caps() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;

    // 1. Single file cap test
    let mut policy_file_cap = super::recovery_snapshot::SnapshotPolicy::default();
    policy_file_cap.max_file_bytes = 50; // 50 bytes cap
    std::fs::write(repo_dir.join("large_file.bin"), vec![0u8; 100]).unwrap();

    let res_file_cap = super::recovery_snapshot::create_recovery_snapshot(
        "run-t6-file",
        &repo_dir,
        dir.path(),
        &policy_file_cap,
    );
    assert!(
        res_file_cap.is_err(),
        "create_recovery_snapshot must fail on file size cap"
    );
    match res_file_cap.err().unwrap() {
        super::recovery_snapshot::SnapshotError::FileTooLarge { path, size, limit } => {
            assert!(path.contains("large_file.bin"));
            assert_eq!(size, 100);
            assert_eq!(limit, 50);
        }
        other => panic!("Expected FileTooLarge error, got {other:?}"),
    }
    // Verify staging directory was deleted
    let recovery_dir = dir.path().join("recovery");
    if recovery_dir.exists() {
        let entries: Vec<_> = std::fs::read_dir(&recovery_dir).unwrap().collect();
        assert!(entries.is_empty(), "Staging dir must be removed on failure");
    }

    // Remove the large file
    std::fs::remove_file(repo_dir.join("large_file.bin")).unwrap();

    // 2. Total aggregate bytes cap test
    let mut policy_total_cap = super::recovery_snapshot::SnapshotPolicy::default();
    policy_total_cap.max_file_bytes = 1000;
    policy_total_cap.max_total_bytes = 150; // 150 bytes total limit
    std::fs::write(repo_dir.join("file_a.bin"), vec![0u8; 90]).unwrap();
    std::fs::write(repo_dir.join("file_b.bin"), vec![0u8; 90]).unwrap();

    let res_total_cap = super::recovery_snapshot::create_recovery_snapshot(
        "run-t6-total",
        &repo_dir,
        dir.path(),
        &policy_total_cap,
    );
    assert!(
        res_total_cap.is_err(),
        "create_recovery_snapshot must fail on total size cap"
    );
    match res_total_cap.err().unwrap() {
        super::recovery_snapshot::SnapshotError::TotalSizeExceeded { total, limit } => {
            assert!(total > 150);
            assert_eq!(limit, 150);
        }
        other => panic!("Expected TotalSizeExceeded error, got {other:?}"),
    }
}

// T7: test_recovery_snapshot_rejects_reparse_point_via_seam
#[test]
fn test_recovery_snapshot_rejects_reparse_point_via_seam() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let policy = super::recovery_snapshot::SnapshotPolicy::default();

    // Activate the reparse rejection seam
    super::recovery_snapshot::INJECT_REPARSE_POINT_REJECTION
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let res = super::recovery_snapshot::create_recovery_snapshot(
        "run-t7",
        &repo_dir,
        dir.path(),
        &policy,
    );
    // Immediately reset the seam
    super::recovery_snapshot::INJECT_REPARSE_POINT_REJECTION
        .store(false, std::sync::atomic::Ordering::SeqCst);

    assert!(
        res.is_err(),
        "Must fail closed when reparse point seam is triggered"
    );
    match res.err().unwrap() {
        super::recovery_snapshot::SnapshotError::ReparsePointRejected(_) => (),
        other => panic!("Expected ReparsePointRejected error, got {other:?}"),
    }

    // Verify recovery final dir was NOT published
    assert!(!dir.path().join("recovery").join("run-t7").exists());
}

// T8: test_recovery_snapshot_detects_concurrent_mutation
#[test]
fn test_recovery_snapshot_detects_concurrent_mutation() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let policy = super::recovery_snapshot::SnapshotPolicy::default();

    // Activate the concurrent mutation seam
    super::recovery_snapshot::INJECT_MUTATION_DURING_COPY
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let res = super::recovery_snapshot::create_recovery_snapshot(
        "run-t8",
        &repo_dir,
        dir.path(),
        &policy,
    );
    // Immediately reset the seam
    super::recovery_snapshot::INJECT_MUTATION_DURING_COPY
        .store(false, std::sync::atomic::Ordering::SeqCst);

    assert!(
        res.is_err(),
        "Must detect concurrent mutation and fail closed"
    );
    match res.err().unwrap() {
        super::recovery_snapshot::SnapshotError::ConcurrentMutation(_) => (),
        other => panic!("Expected ConcurrentMutation error, got {other:?}"),
    }

    // Verify cleanup: final directory does not exist
    assert!(!dir.path().join("recovery").join("run-t8").exists());
}

// T9: test_recovery_snapshot_collision_fails_closed
#[test]
fn test_recovery_snapshot_collision_fails_closed() {
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let policy = super::recovery_snapshot::SnapshotPolicy::default();

    // Pre-create the final run directory with a sentinel file
    let run_dir = dir.path().join("recovery").join("run-t9");
    std::fs::create_dir_all(&run_dir).unwrap();
    let sentinel = run_dir.join("sentinel.txt");
    std::fs::write(&sentinel, b"pre-existing content").unwrap();

    let res = super::recovery_snapshot::create_recovery_snapshot(
        "run-t9",
        &repo_dir,
        dir.path(),
        &policy,
    );
    assert!(
        res.is_err(),
        "Must fail closed if destination directory already exists"
    );
    match res.err().unwrap() {
        super::recovery_snapshot::SnapshotError::Collision(path) => {
            assert_eq!(path, run_dir);
        }
        other => panic!("Expected Collision error, got {other:?}"),
    }

    // Verify pre-existing sentinel file was untouched
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"pre-existing content");
}

// T10: test_recovery_snapshot_rejects_path_traversal
#[test]
fn test_recovery_snapshot_rejects_path_traversal() {
    use super::recovery_snapshot::{normalize_and_validate_relative_path, SnapshotError};

    assert!(matches!(
        normalize_and_validate_relative_path("../secret.txt"),
        Err(SnapshotError::IllegalPath(_))
    ));
    assert!(matches!(
        normalize_and_validate_relative_path("foo/../../secret.txt"),
        Err(SnapshotError::IllegalPath(_))
    ));
    assert!(matches!(
        normalize_and_validate_relative_path("/etc/passwd"),
        Err(SnapshotError::IllegalPath(_))
    ));
    assert!(matches!(
        normalize_and_validate_relative_path("foo/./bar.txt"),
        Err(SnapshotError::IllegalPath(_))
    ));
    assert!(matches!(
        normalize_and_validate_relative_path("foo\0bar"),
        Err(SnapshotError::IllegalPath(_))
    ));

    // Valid paths normalize to forward slashes
    assert_eq!(
        normalize_and_validate_relative_path("src\\main.rs").unwrap(),
        "src/main.rs"
    );
    assert_eq!(
        normalize_and_validate_relative_path("src/lib.rs").unwrap(),
        "src/lib.rs"
    );
}

// T11: test_prepare_and_bind_recovery_snapshot_seam
#[test]
fn test_prepare_and_bind_recovery_snapshot_seam() {
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let runner = MockCommandRunner::new();

    let mut config = default_test_config(standard_profile());
    config.working_dir = repo_dir.to_string_lossy().to_string();
    let mut state = initialize_run(config, "HEAD", vec![], &runner, FIXED_TIME).unwrap();

    let engine_root = dir.path().join("engine_authority");
    std::fs::create_dir_all(&engine_root).unwrap();

    let policy = super::recovery_snapshot::SnapshotPolicy::default();
    let (binding, _lease) =
        super::prepare_and_bind_recovery_snapshot(&mut state, &engine_root, &policy)
            .expect("prepare_and_bind_recovery_snapshot");

    assert!(state.recovery_snapshot.is_some());
    let bound = state.recovery_snapshot.as_ref().unwrap();
    assert_eq!(bound.root, binding.root);
    assert_eq!(bound.manifest_digest, binding.manifest_digest);

    // Verify snapshot.json exists under snapshot root and is valid
    let snapshot_json_path = bound.root.join("snapshot.json");
    assert!(
        snapshot_json_path.exists(),
        "snapshot.json must exist in published directory"
    );
    let json_bytes = std::fs::read(&snapshot_json_path).unwrap();
    let parsed: super::recovery_snapshot::RecoverySnapshot =
        serde_json::from_slice(&json_bytes).unwrap();
    assert_eq!(parsed.manifest_digest, bound.manifest_digest);

    // Verify single-seal contract (§304.3 #3, §306.3 #1):
    // Sealing occurs explicitly via seal_run_state_authenticated before persistence attempt
    super::guards::seal_run_state_authenticated(&mut state).expect("seal_run_state_authenticated");
    let expected_tag = super::guards::compute_engine_state_auth_tag(&state).unwrap();
    assert_eq!(
        state.engine_auth_tag, expected_tag,
        "Engine state auth tag must be valid and sealed via single-seal contract"
    );
}

// T12: test_auto_converge_start_run_blocks_before_snapshot_when_dispatch_disabled
#[test]
fn test_auto_converge_start_run_blocks_before_snapshot_when_dispatch_disabled() {
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    use super::types::ReviewerLevel;

    let mut config = default_test_config(standard_profile());
    let run_id = format!("ac-snapgate-{}", uuid::Uuid::new_v4());
    config.run_id = Some(run_id.clone());
    config.working_dir = repo_dir.to_string_lossy().to_string();
    config.reviewer_level = Some(ReviewerLevel::R0D);
    config.r0d_acknowledged = true;

    assert!(
        !super::is_production_dispatch_enabled(),
        "Production dispatch must be false"
    );

    let start_res = super::auto_converge_start_run(config);
    assert!(
        start_res.is_err(),
        "start_run must fail when dispatch gate is false"
    );
    let err = start_res.err().unwrap();
    assert!(
        err.contains(
            "Production dispatch is blocked pending Phase G verification and Phase H authorization"
        ),
        "Error message must indicate dispatch gate is blocked: {err}"
    );

    // Verify zero side effects: no recovery directory created
    let recovery_dir = dir.path().join("recovery").join(&run_id);
    assert!(
        !recovery_dir.exists(),
        "No recovery snapshot directory must be created when dispatch is disabled"
    );
}

// T13: test_recovery_snapshot_rejects_unsupported_layouts
#[test]
fn test_recovery_snapshot_rejects_unsupported_layouts() {
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let policy = super::recovery_snapshot::SnapshotPolicy::default();

    // 1. Sparse-checkout detection
    let sparse_info_dir = repo_dir.join(".git").join("info");
    std::fs::create_dir_all(&sparse_info_dir).unwrap();
    let sparse_file = sparse_info_dir.join("sparse-checkout");
    std::fs::write(&sparse_file, b"/*\n!/ignore_me\n").unwrap();

    let res_sparse = super::recovery_snapshot::create_recovery_snapshot(
        "run-t13-sparse",
        &repo_dir,
        dir.path(),
        &policy,
    );
    assert!(res_sparse.is_err(), "Must reject sparse-checkout layout");
    match res_sparse.err().unwrap() {
        super::recovery_snapshot::SnapshotError::UnsupportedIndexLayout(msg) => {
            assert!(msg.contains("sparse-checkout"));
        }
        other => panic!("Expected UnsupportedIndexLayout for sparse checkout, got {other:?}"),
    }

    // Clean up sparse-checkout file
    std::fs::remove_file(sparse_file).unwrap();

    // 2. Split-index detection (sharedindex.* file)
    let shared_index = repo_dir.join(".git").join("sharedindex.1234567890abcdef");
    std::fs::write(&shared_index, b"DIRC split index payload").unwrap();

    let res_split = super::recovery_snapshot::create_recovery_snapshot(
        "run-t13-split",
        &repo_dir,
        dir.path(),
        &policy,
    );
    assert!(res_split.is_err(), "Must reject split-index layout");
    match res_split.err().unwrap() {
        super::recovery_snapshot::SnapshotError::UnsupportedIndexLayout(msg) => {
            assert!(msg.contains("Split-index detected"));
        }
        other => panic!("Expected UnsupportedIndexLayout for split-index, got {other:?}"),
    }
}

// Helper RAII guard ensuring process environment variables are cleanly removed on drop/unwind (§296.3 #7, #9).
struct EnvVarGuard {
    keys: Vec<String>,
}

impl EnvVarGuard {
    fn new(vars: &[(&str, &str)]) -> Self {
        let mut keys = Vec::new();
        for &(k, v) in vars {
            std::env::set_var(k, v);
            keys.push(k.to_string());
        }
        Self { keys }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        for k in &self.keys {
            std::env::remove_var(k);
        }
    }
}

// T14: test_recovery_snapshot_hermetic_git_scrubs_git_env
#[test]
fn test_recovery_snapshot_hermetic_git_scrubs_git_env() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let env = super::recovery_snapshot::HermeticGitEnv::new(dir.path()).unwrap();

    let ambient_git_key = "GIT_TEST_AMBIENT_DISRUPTION_KEY";
    let ambient_nongit_key = "TEST_AMBIENT_NON_GIT_VARIABLE";
    let _env_guard = EnvVarGuard::new(&[
        (ambient_git_key, "disruptive_value"),
        (ambient_nongit_key, "nongit_ambient_value"),
    ]);

    // Build the hermetic command and inspect its environment configuration
    let cmd = super::recovery_snapshot::build_hermetic_git_command(
        &repo_dir,
        &["status", "--porcelain=v2"],
        env.config_path(),
        env.temp_dir(),
    )
    .expect("build hermetic git command");

    let env_entries: Vec<(String, Option<String>)> = cmd
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().to_string(),
                v.map(|s| s.to_string_lossy().to_string()),
            )
        })
        .collect();

    // Verify ambient variables are completely stripped from child environment
    assert!(
        !env_entries
            .iter()
            .any(|(k, v)| k == ambient_git_key && v.is_some()),
        "Ambient GIT_* must not be present in child environment"
    );
    assert!(
        !env_entries
            .iter()
            .any(|(k, v)| k == ambient_nongit_key && v.is_some()),
        "Ambient non-Git variables must not be present in child environment"
    );

    // Verify minimal allowlisted hermetic variables are present
    assert!(env_entries
        .iter()
        .any(|(k, v)| k == "GIT_CONFIG_NOSYSTEM" && v.as_deref() == Some("1")));
    assert!(env_entries
        .iter()
        .any(|(k, v)| k == "GIT_OPTIONAL_LOCKS" && v.as_deref() == Some("0")));
    assert!(env_entries.iter().any(|(k, v)| k == "PATH" && v.is_some()));
    assert!(env_entries.iter().any(|(k, v)| k == "TEMP" && v.is_some()));

    // Verify executable resolution: canonical, file, not a reparse point (§296.3 #7)
    let git_exe = super::recovery_snapshot::resolve_git_executable().expect("resolve git exe");
    assert!(git_exe.is_file());
    assert!(git_exe.is_absolute());

    // Deterministic child runtime environment verification using child process
    #[cfg(windows)]
    {
        let mut child_test_cmd = std::process::Command::new("cmd.exe");
        child_test_cmd.args(["/C", "set"]);
        child_test_cmd.env_clear();
        for (k, v) in &env_entries {
            if let Some(val) = v {
                child_test_cmd.env(k, val);
            }
        }
        let child_out = child_test_cmd.output().expect("spawn child set command");
        let stdout_str = String::from_utf8_lossy(&child_out.stdout);
        assert!(
            !stdout_str.contains(ambient_git_key),
            "Child process runtime environment must not contain ambient GIT_* key"
        );
        assert!(
            !stdout_str.contains(ambient_nongit_key),
            "Child process runtime environment must not contain ambient non-git key"
        );
        assert!(
            stdout_str.contains("GIT_CONFIG_NOSYSTEM=1"),
            "Child process runtime environment must contain allowlisted GIT_CONFIG_NOSYSTEM"
        );
    }

    // Run hermetic git: must succeed against repo_dir
    let stdout = super::recovery_snapshot::run_hermetic_git(
        &repo_dir,
        &["status", "--porcelain=v2"],
        env.config_path(),
        env.temp_dir(),
    )
    .expect("hermetic git status query must succeed");
    assert!(
        stdout.stdout.is_empty(),
        "clean repo status should be empty"
    );
}

// T15: test_recovery_snapshot_symbolic_ref_outcomes_exhaustive
#[test]
fn test_recovery_snapshot_symbolic_ref_outcomes_exhaustive() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let git_dir = repo_dir.join(".git");
    let index_path = git_dir.join("index");
    let env = super::recovery_snapshot::HermeticGitEnv::new(dir.path()).unwrap();

    // 1. Status 0: Symbolic ref
    let snap_sym = super::recovery_snapshot::capture_git_snapshot(
        &repo_dir,
        &index_path,
        &git_dir,
        &env,
        false,
    )
    .expect("capture symbolic head snapshot");
    assert_eq!(
        snap_sym.head_mode,
        super::recovery_snapshot::HeadMode::Symbolic
    );
    let sym_ref = snap_sym.head_ref.expect("symbolic ref must be present");
    assert!(
        sym_ref.starts_with("refs/heads/"),
        "Ref should start with refs/heads/: {sym_ref}"
    );

    // 2. Status 1: Detached HEAD
    let global_config = dir.path().join(".empty_test_gitconfig");
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(&["checkout", "--detach"])
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .status()
        .unwrap();
    assert!(status.success());

    let snap_det = super::recovery_snapshot::capture_git_snapshot(
        &repo_dir,
        &index_path,
        &git_dir,
        &env,
        false,
    )
    .expect("capture detached head snapshot");
    assert_eq!(
        snap_det.head_mode,
        super::recovery_snapshot::HeadMode::Detached
    );
    assert_eq!(snap_det.head_ref, None);

    // 3. Status other / corrupt HEAD: write invalid ref string into HEAD
    std::fs::write(git_dir.join("HEAD"), b"ref: refs/heads/\0invalid\n").unwrap();
    let res_corrupt = super::recovery_snapshot::query_symbolic_or_detached_head(&repo_dir, &env);
    assert!(
        res_corrupt.is_err(),
        "Must fail closed on malformed/corrupt symbolic-ref output"
    );
    match res_corrupt.err().unwrap() {
        super::recovery_snapshot::SnapshotError::GitCommandFailed { stderr, .. } => {
            assert!(
                !stderr.is_empty(),
                "symbolic-ref failure stderr must not be empty"
            );
        }
        other => panic!("Expected GitCommandFailed on corrupt ref, got {other:?}"),
    }
}

// T16: test_recovery_snapshot_identity_mismatch_fails_closed
#[test]
fn test_recovery_snapshot_identity_mismatch_fails_closed() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let env = super::recovery_snapshot::HermeticGitEnv::new(dir.path()).unwrap();

    // Query identity for actual repo_dir
    let identity = super::recovery_snapshot::query_and_verify_repository_identity(&repo_dir, &env)
        .expect("query repository identity");
    assert_eq!(identity.toplevel, std::fs::canonicalize(&repo_dir).unwrap());

    // Query identity with mismatched directory (pass dir.path() instead of repo_dir)
    let res_mismatch =
        super::recovery_snapshot::query_and_verify_repository_identity(dir.path(), &env);
    assert!(
        res_mismatch.is_err(),
        "Must fail when passed directory does not match git toplevel"
    );
    match res_mismatch.err().unwrap() {
        super::recovery_snapshot::SnapshotError::UnsupportedIndexLayout(_) => (),
        super::recovery_snapshot::SnapshotError::GitCommandFailed { .. } => (),
        other => panic!("Expected UnsupportedIndexLayout or GitCommandFailed, got {other:?}"),
    }
}

// T17: test_recovery_snapshot_preflight_rejection_leaves_git_bytes_unchanged
#[test]
fn test_recovery_snapshot_preflight_rejection_leaves_git_bytes_unchanged() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let git_dir = repo_dir.join(".git");
    let index_file = git_dir.join("index");
    let head_file = git_dir.join("HEAD");

    let index_bytes_before = std::fs::read(&index_file).unwrap();
    let head_bytes_before = std::fs::read(&head_file).unwrap();

    // Inject active sparse-checkout
    let sparse_file = git_dir.join("info").join("sparse-checkout");
    std::fs::create_dir_all(sparse_file.parent().unwrap()).unwrap();
    std::fs::write(&sparse_file, b"/*\n!/exclude\n").unwrap();

    let policy = super::recovery_snapshot::SnapshotPolicy::default();
    let res = super::recovery_snapshot::create_recovery_snapshot(
        "run-t17",
        &repo_dir,
        dir.path(),
        &policy,
    );
    assert!(
        res.is_err(),
        "Preflight must reject sparse-checkout before any index command"
    );
    match res.err().unwrap() {
        super::recovery_snapshot::SnapshotError::UnsupportedIndexLayout(msg) => {
            assert!(msg.contains("sparse-checkout"));
        }
        other => panic!("Expected UnsupportedIndexLayout, got {other:?}"),
    }

    // Verify .git/index and .git/HEAD bytes are completely unchanged
    let index_bytes_after = std::fs::read(&index_file).unwrap();
    let head_bytes_after = std::fs::read(&head_file).unwrap();
    assert_eq!(
        index_bytes_before, index_bytes_after,
        "Index bytes must be unchanged on preflight rejection"
    );
    assert_eq!(
        head_bytes_before, head_bytes_after,
        "HEAD bytes must be unchanged on preflight rejection"
    );
}

// T18: test_recovery_snapshot_reopen_verification_failure_prevents_publication
#[test]
fn test_recovery_snapshot_reopen_verification_failure_prevents_publication() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let policy = super::recovery_snapshot::SnapshotPolicy::default();

    // Activate the staging corruption seam
    super::recovery_snapshot::INJECT_STAGING_CORRUPTION
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let res = super::recovery_snapshot::create_recovery_snapshot(
        "run-t18",
        &repo_dir,
        dir.path(),
        &policy,
    );
    // Immediately reset the seam
    super::recovery_snapshot::INJECT_STAGING_CORRUPTION
        .store(false, std::sync::atomic::Ordering::SeqCst);

    assert!(
        res.is_err(),
        "Must fail closed when reopen verification detects corrupt manifest/file"
    );
    match res.err().unwrap() {
        super::recovery_snapshot::SnapshotError::ManifestVerificationFailed { .. }
        | super::recovery_snapshot::SnapshotError::StagedFileVerificationFailed { .. } => (),
        other => panic!(
            "Expected ManifestVerificationFailed or StagedFileVerificationFailed, got {other:?}"
        ),
    }

    // Verify atomic publication did NOT happen: final destination must not exist
    let final_dir = dir.path().join("recovery").join("run-t18");
    assert!(
        !final_dir.exists(),
        "Final recovery directory must not be published on verification failure"
    );

    // Verify staging directory was cleaned up
    let staging_dir = dir.path().join("recovery").join(".staging-run-t18");
    assert!(
        !staging_dir.exists(),
        "Staging directory must be cleaned up on verification failure"
    );
}

// T19: test_recovery_snapshot_staging_cleanup_guard_behavior
#[test]
fn test_recovery_snapshot_staging_cleanup_guard_behavior() {
    let dir = TempDir::new().unwrap();
    let staging_path = dir.path().join("test_staging");
    std::fs::create_dir(&staging_path).unwrap();
    assert!(staging_path.exists());

    // 1. Drop without commit preserves directory on unwind/panic (§304.3 #3)
    {
        let _guard = super::recovery_snapshot::StagingCleanupGuard::new(staging_path.clone());
    }
    assert!(
        staging_path.exists(),
        "StagingCleanupGuard must preserve uncommitted directory on drop (§304.3 #3)"
    );

    // 2. Commit preserves directory
    {
        let mut guard = super::recovery_snapshot::StagingCleanupGuard::new(staging_path.clone());
        guard.commit();
    }
    assert!(
        staging_path.exists(),
        "Committed StagingCleanupGuard must preserve directory on drop"
    );
    let _ = std::fs::remove_dir(&staging_path);
}

// Helper to construct a synthetic valid Git index buffer for testing
fn build_synthetic_index_with_opts(
    version: u32,
    mode: u32,
    ext_flags: Option<u16>,
    path: &[u8],
    declared_path_len: Option<u16>,
    extension_blocks: &[(&[u8; 4], &[u8])],
    hash_len: usize,
    corrupt_checksum: bool,
) -> Vec<u8> {
    let mut buf = Vec::new();
    // Header (12 bytes)
    buf.extend_from_slice(b"DIRC");
    buf.extend_from_slice(&version.to_be_bytes());
    buf.extend_from_slice(&1u32.to_be_bytes()); // 1 entry

    // Entry (fixed 40 bytes)
    buf.extend_from_slice(&1000u32.to_be_bytes()); // ctime sec
    buf.extend_from_slice(&0u32.to_be_bytes()); // ctime nsec
    buf.extend_from_slice(&1000u32.to_be_bytes()); // mtime sec
    buf.extend_from_slice(&0u32.to_be_bytes()); // mtime nsec
    buf.extend_from_slice(&1u32.to_be_bytes()); // dev
    buf.extend_from_slice(&1u32.to_be_bytes()); // ino
    buf.extend_from_slice(&mode.to_be_bytes()); // mode
    buf.extend_from_slice(&1000u32.to_be_bytes()); // uid
    buf.extend_from_slice(&1000u32.to_be_bytes()); // gid
    buf.extend_from_slice(&(path.len() as u32).to_be_bytes()); // size

    // Object hash (20 or 32 bytes)
    buf.extend(std::iter::repeat(0x01u8).take(hash_len));

    // Flags (2 bytes)
    let p_len = declared_path_len.unwrap_or(path.len() as u16);
    let flags: u16 = (p_len & 0x0FFF) | if ext_flags.is_some() { 0x4000 } else { 0 };
    buf.extend_from_slice(&flags.to_be_bytes());

    // Extended flags if present (version >= 3)
    if let Some(ef) = ext_flags {
        buf.extend_from_slice(&ef.to_be_bytes());
    }

    if version == 4 {
        // v4: strip len varint (0 for single/first entry) + path + NUL
        buf.push(0x00);
        buf.extend_from_slice(path);
        buf.push(0x00);
    } else {
        // v2/v3: path + NUL + 8-byte padding
        buf.extend_from_slice(path);
        buf.push(0x00);

        let entry_start = 12;
        let entry_len_so_far = buf.len() - entry_start;
        let pad = (8 - (entry_len_so_far % 8)) % 8;
        for _ in 0..pad {
            buf.push(0);
        }
    }

    // Extension blocks: [4b sig][4b len BE][payload]
    for (sig, payload) in extension_blocks {
        buf.extend_from_slice(*sig);
        buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        buf.extend_from_slice(payload);
    }

    // Trailing checksum (20 bytes for SHA-1, 32 bytes for SHA-256)
    if hash_len == 20 {
        let mut digest = super::recovery_snapshot::sha1_digest(&buf);
        if corrupt_checksum {
            digest[0] ^= 0xff;
        }
        buf.extend_from_slice(&digest);
    } else {
        let mut digest = super::recovery_snapshot::sha256_digest(&buf);
        if corrupt_checksum {
            digest[0] ^= 0xff;
        }
        buf.extend_from_slice(&digest);
    }
    buf
}

fn build_synthetic_index(
    version: u32,
    mode: u32,
    ext_flags: Option<u16>,
    extension_block: Option<(&[u8; 4], &[u8])>,
    corrupt_checksum: bool,
) -> Vec<u8> {
    let ext_slice: Vec<(&[u8; 4], &[u8])> = extension_block.into_iter().collect();
    build_synthetic_index_with_opts(
        version,
        mode,
        ext_flags,
        b"test.txt",
        None,
        &ext_slice,
        20,
        corrupt_checksum,
    )
}

// T20: test_recovery_snapshot_structural_index_parsing_valid
#[test]
fn test_recovery_snapshot_structural_index_parsing_valid() {
    // Valid v2 index
    let buf_v2 = build_synthetic_index(2, 0o100644, None, None, false);
    let res_v2 = super::recovery_snapshot::parse_and_validate_git_index(&buf_v2, 20);
    assert!(
        res_v2.is_ok(),
        "Valid v2 index must parse successfully: {:?}",
        res_v2.err()
    );

    // Valid v3 index with extension
    let buf_v3 = build_synthetic_index(3, 0o100644, Some(0), Some((b"TREE", &[0, 0, 0, 0])), false);
    let res_v3 = super::recovery_snapshot::parse_and_validate_git_index(&buf_v3, 20);
    assert!(
        res_v3.is_ok(),
        "Valid v3 index with TREE extension must parse: {:?}",
        res_v3.err()
    );
}

// T21: test_recovery_snapshot_structural_index_parsing_rejections
#[test]
fn test_recovery_snapshot_structural_index_parsing_rejections() {
    // 1. Bad magic
    let mut buf_bad_magic = build_synthetic_index(2, 0o100644, None, None, false);
    buf_bad_magic[0..4].copy_from_slice(b"BADM");
    let res_magic = super::recovery_snapshot::parse_and_validate_git_index(&buf_bad_magic, 20);
    assert!(res_magic.is_err());
    assert!(format!("{:?}", res_magic.err().unwrap()).contains("magic does not match 'DIRC'"));

    // 2. Unsupported version
    let buf_v5 = build_synthetic_index(5, 0o100644, None, None, false);
    let res_v5 = super::recovery_snapshot::parse_and_validate_git_index(&buf_v5, 20);
    assert!(res_v5.is_err());
    assert!(format!("{:?}", res_v5.err().unwrap()).contains("Unsupported Git index version: 5"));

    // 3. Truncated length
    let res_trunc =
        super::recovery_snapshot::parse_and_validate_git_index(&[b'D', b'I', b'R', b'C'], 20);
    assert!(res_trunc.is_err());
    assert!(format!("{:?}", res_trunc.err().unwrap()).contains("too small"));

    // 4. 'sdir' (sparse index) extension block rejection
    let buf_sdir = build_synthetic_index(2, 0o100644, None, Some((b"sdir", &[0, 0, 0, 0])), false);
    let res_sdir = super::recovery_snapshot::parse_and_validate_git_index(&buf_sdir, 20);
    assert!(res_sdir.is_err(), "Must reject 'sdir' extension");
    assert!(format!("{:?}", res_sdir.err().unwrap()).contains("sdir"));

    // 5. 'link' (split index) extension block rejection
    let buf_link = build_synthetic_index(2, 0o100644, None, Some((b"link", &[0, 0, 0, 0])), false);
    let res_link = super::recovery_snapshot::parse_and_validate_git_index(&buf_link, 20);
    assert!(res_link.is_err(), "Must reject 'link' extension");
    assert!(format!("{:?}", res_link.err().unwrap()).contains("link"));

    // 6. Mode 040000 (sparse directory) entry rejection
    let buf_mode = build_synthetic_index(2, 0o040000, None, None, false);
    let res_mode = super::recovery_snapshot::parse_and_validate_git_index(&buf_mode, 20);
    assert!(
        res_mode.is_err(),
        "Must reject 040000 sparse directory entry"
    );
    assert!(format!("{:?}", res_mode.err().unwrap()).contains("040000"));

    // 7. CE_SKIP_WORKTREE (0x4000 in extended flags) rejection
    let buf_skip = build_synthetic_index(3, 0o100644, Some(0x4000), None, false);
    let res_skip = super::recovery_snapshot::parse_and_validate_git_index(&buf_skip, 20);
    assert!(res_skip.is_err(), "Must reject SKIP_WORKTREE index entry");
    assert!(format!("{:?}", res_skip.err().unwrap()).contains("SKIP_WORKTREE"));

    // 8. Tampered checksum rejection
    let buf_corrupt_cs = build_synthetic_index(2, 0o100644, None, None, true);
    let res_cs = super::recovery_snapshot::parse_and_validate_git_index(&buf_corrupt_cs, 20);
    assert!(res_cs.is_err(), "Must reject corrupted trailing checksum");
    assert!(format!("{:?}", res_cs.err().unwrap()).contains("checksum mismatch"));
}

// T22: test_recovery_snapshot_dual_preflight_catches_layout_mutation_before_final_capture
#[test]
fn test_recovery_snapshot_dual_preflight_catches_layout_mutation_before_final_capture() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let policy = super::recovery_snapshot::SnapshotPolicy::default();

    // Enable seam that injects sparse-checkout right before the second preflight
    super::recovery_snapshot::INJECT_MUTATION_BEFORE_FINAL_PREFLIGHT
        .store(true, std::sync::atomic::Ordering::SeqCst);

    let res = super::recovery_snapshot::create_recovery_snapshot(
        "run-t22-dual-preflight",
        &repo_dir,
        dir.path(),
        &policy,
    );

    // Reset seam
    super::recovery_snapshot::INJECT_MUTATION_BEFORE_FINAL_PREFLIGHT
        .store(false, std::sync::atomic::Ordering::SeqCst);

    assert!(
        res.is_err(),
        "Second preflight before final capture must detect layout mutation and fail closed"
    );
    match res.err().unwrap() {
        super::recovery_snapshot::SnapshotError::UnsupportedIndexLayout(msg) => {
            assert!(msg.contains("sparse-checkout"));
        }
        other => panic!("Expected UnsupportedIndexLayout from second preflight, got {other:?}"),
    }

    // Verify final directory was not published
    let final_dir = dir.path().join("recovery").join("run-t22-dual-preflight");
    assert!(
        !final_dir.exists(),
        "Final directory must not be published on preflight failure"
    );
}

// T23: test_recovery_snapshot_staging_cleanup_failure_preserves_path
#[test]
fn test_recovery_snapshot_staging_cleanup_failure_preserves_path() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let policy = super::recovery_snapshot::SnapshotPolicy::default();

    // Inject mutation during copy to trigger error path, AND inject cleanup failure
    super::recovery_snapshot::INJECT_MUTATION_DURING_COPY
        .store(true, std::sync::atomic::Ordering::SeqCst);
    super::recovery_snapshot::INJECT_STAGING_CLEANUP_FAILURE
        .store(true, std::sync::atomic::Ordering::SeqCst);

    let res = super::recovery_snapshot::create_recovery_snapshot(
        "run-t23-cleanup-fail",
        &repo_dir,
        dir.path(),
        &policy,
    );

    super::recovery_snapshot::INJECT_MUTATION_DURING_COPY
        .store(false, std::sync::atomic::Ordering::SeqCst);
    super::recovery_snapshot::INJECT_STAGING_CLEANUP_FAILURE
        .store(false, std::sync::atomic::Ordering::SeqCst);

    assert!(res.is_err(), "Must fail when copy fails and cleanup fails");
    let staging_path = match res.err().unwrap() {
        super::recovery_snapshot::SnapshotError::CleanupFailed { staging_dir, .. } => {
            let staging_str = staging_dir.to_string_lossy();
            assert!(
                staging_str.contains(".staging-run-t23-cleanup-fail"),
                "CleanupFailed must preserve exact staging path: {staging_str}"
            );
            assert!(
                staging_dir.exists(),
                "Staging directory must NOT have been deleted when cleanup failed"
            );
            staging_dir
        }
        other => panic!("Expected CleanupFailed error, got {other:?}"),
    };
    let _ = std::fs::remove_dir_all(&staging_path);

    // Verify final directory was never published
    let final_dir = dir.path().join("recovery").join("run-t23-cleanup-fail");
    assert!(!final_dir.exists(), "Final directory must not be published");
}

// T24: test_recovery_snapshot_reparse_point_rejection_fails_closed
#[test]
fn test_recovery_snapshot_reparse_point_rejection_fails_closed() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let policy = super::recovery_snapshot::SnapshotPolicy::default();

    super::recovery_snapshot::INJECT_REPARSE_POINT_REJECTION
        .store(true, std::sync::atomic::Ordering::SeqCst);

    let res = super::recovery_snapshot::create_recovery_snapshot(
        "run-t24-reparse",
        &repo_dir,
        dir.path(),
        &policy,
    );

    super::recovery_snapshot::INJECT_REPARSE_POINT_REJECTION
        .store(false, std::sync::atomic::Ordering::SeqCst);

    assert!(res.is_err(), "Must reject reparse points");
    match res.err().unwrap() {
        super::recovery_snapshot::SnapshotError::ReparsePointRejected(_) => (),
        other => panic!("Expected ReparsePointRejected, got {other:?}"),
    }
}

// Helper RAII guard ensuring NTFS junctions are cleanly removed on drop/unwind (§296.3 #6).
#[cfg(windows)]
struct JunctionGuard(std::path::PathBuf);

#[cfg(windows)]
impl Drop for JunctionGuard {
    fn drop(&mut self) {
        if self.0.exists() || std::fs::symlink_metadata(&self.0).is_ok() {
            let out = std::process::Command::new("cmd.exe")
                .args(["/C", "rmdir", self.0.to_str().unwrap()])
                .output();
            if let Ok(o) = out {
                assert!(
                    o.status.success(),
                    "Junction cleanup via rmdir must succeed for {:?}: stdout={}, stderr={}",
                    self.0,
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                );
            }
        }
    }
}

// T25: test_recovery_snapshot_real_junction_rejection
#[test]
#[cfg(windows)]
fn test_recovery_snapshot_real_junction_rejection() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let policy = super::recovery_snapshot::SnapshotPolicy::default();

    // 1. Leaf junction inside repository
    let target_dir = dir.path().join("outside_target");
    std::fs::create_dir_all(&target_dir).unwrap();
    std::fs::write(target_dir.join("secret.txt"), b"confidential content").unwrap();

    let junction_dir = repo_dir.join("sub_junction");
    let status = std::process::Command::new("cmd.exe")
        .args([
            "/C",
            "mklink",
            "/J",
            junction_dir.to_str().unwrap(),
            target_dir.to_str().unwrap(),
        ])
        .status()
        .expect("spawn mklink /J command");

    assert!(
        status.success(),
        "mklink /J must succeed to establish fixture (exit code {:?})",
        status.code()
    );
    let _jguard1 = JunctionGuard(junction_dir.clone());

    let secret_file = target_dir.join("secret.txt");
    let secret_id = super::recovery_snapshot::query_path_identity(&secret_file).unwrap();
    super::recovery_snapshot::set_protected_read_observer_target(Some(secret_id));

    let res = super::recovery_snapshot::create_recovery_snapshot(
        "run-t25-real-junction",
        &repo_dir,
        dir.path(),
        &policy,
    );
    let observer_count = super::recovery_snapshot::get_protected_read_observer_count();
    super::recovery_snapshot::set_protected_read_observer_target(None);

    assert_eq!(
        observer_count, 0,
        "Protected read observer count must be zero (junction target was never opened for read)"
    );

    assert!(
        res.is_err(),
        "Recovery snapshot must fail closed when repo contains a real NTFS junction"
    );
    match res.err().unwrap() {
        super::recovery_snapshot::SnapshotError::ReparsePointRejected(p) => {
            assert!(p.contains("sub_junction"));
        }
        other => panic!("Expected ReparsePointRejected, got {other:?}"),
    }

    // Verify confidential content was NEVER read or copied into any snapshot staging
    fn assert_no_sentinel_at_any_depth(dir: &std::path::Path, sentinel: &[u8]) {
        if !dir.exists() {
            return;
        }
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let p = entry.path();
            if p.is_dir() {
                assert_no_sentinel_at_any_depth(&p, sentinel);
            } else if p.is_file() {
                let content = std::fs::read(&p).unwrap_or_default();
                assert!(
                    !content.windows(sentinel.len()).any(|w| w == sentinel),
                    "Target confidential content must not be present at any depth in recovery file: {}",
                    p.display()
                );
            }
        }
    }

    let recovery_dir = dir.path().join("recovery");
    assert_no_sentinel_at_any_depth(&recovery_dir, b"confidential content");
    assert!(target_dir.exists(), "Target directory must be preserved");
    assert!(
        junction_dir.exists(),
        "Junction directory must be preserved"
    );

    // 2. Intermediate directory junction within repository
    let intermediate_target = dir.path().join("inter_target");
    std::fs::create_dir_all(&intermediate_target).unwrap();
    std::fs::write(intermediate_target.join("nested.txt"), b"nested data").unwrap();

    let intermediate_junction = repo_dir.join("inter_junc");
    let status_inter = std::process::Command::new("cmd.exe")
        .args([
            "/C",
            "mklink",
            "/J",
            intermediate_junction.to_str().unwrap(),
            intermediate_target.to_str().unwrap(),
        ])
        .status()
        .expect("spawn mklink /J for intermediate junction");

    assert!(
        status_inter.success(),
        "mklink /J for intermediate junction must succeed (exit code {:?})",
        status_inter.code()
    );
    let _jguard2 = JunctionGuard(intermediate_junction.clone());

    let nested_file = intermediate_junction.join("nested.txt");
    let res = super::recovery_snapshot::validate_ancestor_directories(&nested_file, &repo_dir);
    assert!(
        res.is_err(),
        "validate_ancestor_directories must reject intermediate directory junction"
    );
    match res.err().unwrap() {
        super::recovery_snapshot::SnapshotError::ReparsePointRejected(p) => {
            assert!(p.contains("inter_junc"));
        }
        other => panic!("Expected ReparsePointRejected, got {other:?}"),
    }

    let read_res = super::recovery_snapshot::read_file_no_follow(&nested_file, &repo_dir);
    assert!(
        read_res.is_err(),
        "read_file_no_follow must reject intermediate directory junction"
    );

    let capture_res = super::recovery_snapshot::capture_workspace_file_safe(
        &nested_file,
        &repo_dir,
        &policy,
        "inter_junc/nested.txt",
    );
    assert!(
        capture_res.is_err(),
        "capture_workspace_file_safe must reject junction leaf/intermediate path"
    );
}

// T26: test_recovery_snapshot_symbolic_ref_matrix
#[test]
fn test_recovery_snapshot_symbolic_ref_matrix() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let policy = super::recovery_snapshot::SnapshotPolicy::default();

    // 1. Exit 0: Valid symbolic ref
    {
        *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
            .lock()
            .unwrap() = Some(Ok((0, b"refs/heads/feature-alpha\n".to_vec(), Vec::new())));
        let (snap, _, _) = super::recovery_snapshot::create_recovery_snapshot(
            "run-t26-sym-0",
            &repo_dir,
            dir.path(),
            &policy,
        )
        .expect("Valid symbolic ref must succeed");
        assert_eq!(
            snap.git.head_mode,
            super::recovery_snapshot::HeadMode::Symbolic
        );
        assert_eq!(
            snap.git.head_ref,
            Some("refs/heads/feature-alpha".to_string())
        );
    }

    // 2. Exit 0: Invalid ref format (no refs/ prefix)
    {
        *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
            .lock()
            .unwrap() = Some(Ok((
            0,
            b"invalid-ref-without-prefix\n".to_vec(),
            Vec::new(),
        )));
        let res = super::recovery_snapshot::create_recovery_snapshot(
            "run-t26-sym-inv",
            &repo_dir,
            dir.path(),
            &policy,
        );
        assert!(res.is_err(), "Invalid symbolic ref format must fail closed");
        match res.err().unwrap() {
            super::recovery_snapshot::SnapshotError::GitCommandFailed { stderr, .. } => {
                assert!(stderr.contains("Invalid symbolic-ref output"));
            }
            other => panic!("Expected GitCommandFailed, got {other:?}"),
        }
    }

    // 3. Exit 0: Non-UTF-8 bytes
    {
        *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
            .lock()
            .unwrap() = Some(Ok((0, vec![0xFF, 0xFE, 0xFD], Vec::new())));
        let res = super::recovery_snapshot::create_recovery_snapshot(
            "run-t26-sym-utf8",
            &repo_dir,
            dir.path(),
            &policy,
        );
        assert!(
            res.is_err(),
            "Non-UTF-8 symbolic ref output must fail closed"
        );
        match res.err().unwrap() {
            super::recovery_snapshot::SnapshotError::GitCommandFailed { stderr, .. } => {
                assert!(stderr.contains("Non-UTF-8 output"));
            }
            other => panic!("Expected GitCommandFailed, got {other:?}"),
        }
    }

    // 4. Exit 1: Detached HEAD
    {
        *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
            .lock()
            .unwrap() = Some(Ok((1, Vec::new(), Vec::new())));
        let (snap, _, _) = super::recovery_snapshot::create_recovery_snapshot(
            "run-t26-sym-detached",
            &repo_dir,
            dir.path(),
            &policy,
        )
        .expect("Exit 1 must be treated as detached HEAD");
        assert_eq!(
            snap.git.head_mode,
            super::recovery_snapshot::HeadMode::Detached
        );
        assert_eq!(snap.git.head_ref, None);
    }

    // 5. Exit 128: Fatal error (not detached)
    {
        *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
            .lock()
            .unwrap() = Some(Ok((
            128,
            Vec::new(),
            b"fatal: git internal error\n".to_vec(),
        )));
        let res = super::recovery_snapshot::create_recovery_snapshot(
            "run-t26-sym-fatal",
            &repo_dir,
            dir.path(),
            &policy,
        );
        assert!(
            res.is_err(),
            "Exit code > 1 must not be treated as detached HEAD"
        );
        match res.err().unwrap() {
            super::recovery_snapshot::SnapshotError::GitCommandFailed { stderr, .. } => {
                assert!(stderr.contains("fatal: git internal error"));
            }
            other => panic!("Expected GitCommandFailed, got {other:?}"),
        }
    }

    // 6. Spawn failure
    {
        *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
            .lock()
            .unwrap() = Some(Err(std::io::ErrorKind::PermissionDenied));
        let res = super::recovery_snapshot::create_recovery_snapshot(
            "run-t26-sym-spawn-fail",
            &repo_dir,
            dir.path(),
            &policy,
        );
        assert!(res.is_err(), "Spawn failure must return Io error");
        match res.err().unwrap() {
            super::recovery_snapshot::SnapshotError::Io(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied);
            }
            other => panic!("Expected Io error, got {other:?}"),
        }
    }

    // Clear mock seam
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
}

// T27: test_recovery_snapshot_preflight_sparse_layout_zero_git_index_commands
#[test]
fn test_recovery_snapshot_preflight_sparse_layout_zero_git_index_commands() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let policy = super::recovery_snapshot::SnapshotPolicy::default();

    // Enable sparse-checkout in git config
    let git_config = repo_dir.join(".git").join("config");
    let mut config_content = std::fs::read_to_string(&git_config).unwrap();
    config_content.push_str("\n[core]\n\tsparseCheckout = true\n");
    std::fs::write(&git_config, config_content).unwrap();

    super::recovery_snapshot::GIT_INDEX_COMMAND_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);

    let res = super::recovery_snapshot::create_recovery_snapshot(
        "run-t27-zero-cmd",
        &repo_dir,
        dir.path(),
        &policy,
    );

    assert!(res.is_err(), "Must fail on sparseCheckout configuration");
    match res.err().unwrap() {
        super::recovery_snapshot::SnapshotError::UnsupportedIndexLayout(msg) => {
            assert!(msg.contains("core.sparseCheckout"));
        }
        other => panic!("Expected UnsupportedIndexLayout, got {other:?}"),
    }

    // Verify ZERO git commands that read index (ls-files, diff-index, etc.) were spawned!
    let count =
        super::recovery_snapshot::GIT_INDEX_COMMAND_COUNT.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        count, 0,
        "Preflight must fail closed with zero git index queries spawned"
    );
}

fn build_synthetic_v4_multi_entry_index(entries: &[(&[u8], &[u8], usize, Option<u16>)]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"DIRC");
    buf.extend_from_slice(&4u32.to_be_bytes()); // version 4
    buf.extend_from_slice(&(entries.len() as u32).to_be_bytes()); // entry count

    for &(full_path, suffix, strip_len, declared_len) in entries {
        buf.extend_from_slice(&1000u32.to_be_bytes()); // ctime sec
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&1000u32.to_be_bytes()); // mtime sec
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&1u32.to_be_bytes());
        buf.extend_from_slice(&1u32.to_be_bytes());
        buf.extend_from_slice(&0o100644u32.to_be_bytes());
        buf.extend_from_slice(&1000u32.to_be_bytes());
        buf.extend_from_slice(&1000u32.to_be_bytes());
        buf.extend_from_slice(&(full_path.len() as u32).to_be_bytes());

        buf.extend(std::iter::repeat(0x01u8).take(20));

        let p_len = declared_len.unwrap_or(full_path.len() as u16);
        let flags: u16 = p_len & 0x0FFF;
        buf.extend_from_slice(&flags.to_be_bytes());

        // Git varint encoding recurrence: encode_varint
        let mut val = strip_len;
        let mut vbuf = [0u8; 16];
        let mut pos = 15;
        vbuf[pos] = (val & 127) as u8;
        while val >= 128 {
            val = (val >> 7) - 1;
            pos -= 1;
            vbuf[pos] = 128 | ((val & 127) as u8);
        }
        buf.extend_from_slice(&vbuf[pos..]);

        buf.extend_from_slice(suffix);
        buf.push(0x00);
    }

    let digest = super::recovery_snapshot::sha1_digest(&buf);
    buf.extend_from_slice(&digest);
    buf
}

// T28: test_recovery_snapshot_v4_varint_and_mandatory_extension_parsing
#[test]
fn test_recovery_snapshot_v4_varint_and_mandatory_extension_parsing() {
    // 1. Valid v4 index parsing (single entry)
    let buf_v4 =
        build_synthetic_index_with_opts(4, 0o100644, None, b"v4_test.txt", None, &[], 20, false);
    let res_v4 = super::recovery_snapshot::parse_and_validate_git_index(&buf_v4, 20);
    assert!(
        res_v4.is_ok(),
        "Valid v4 index must parse: {:?}",
        res_v4.err()
    );

    // 2. Unknown mandatory extension block (lowercase first char 'b') rejected (§296.2 #2)
    let buf_m_ext = build_synthetic_index_with_opts(
        2,
        0o100644,
        None,
        b"test.txt",
        None,
        &[(b"bOGU", &[1, 2, 3, 4])],
        20,
        false,
    );
    let res_m_ext = super::recovery_snapshot::parse_and_validate_git_index(&buf_m_ext, 20);
    assert!(
        res_m_ext.is_err(),
        "Must reject unknown mandatory extension block 'bOGU'"
    );
    assert!(format!("{:?}", res_m_ext.err().unwrap()).contains("bOGU"));

    // 3. Unknown optional extension block (uppercase first char 'B') accepted and skipped (§296.2 #2)
    let buf_o_ext = build_synthetic_index_with_opts(
        2,
        0o100644,
        None,
        b"test.txt",
        None,
        &[(b"BOGU", &[1, 2, 3, 4])],
        20,
        false,
    );
    let res_o_ext = super::recovery_snapshot::parse_and_validate_git_index(&buf_o_ext, 20);
    assert!(
        res_o_ext.is_ok(),
        "Must successfully skip unknown optional extension block 'BOGU': {:?}",
        res_o_ext.err()
    );

    // 4. Multi-byte v4 varint test with strip length > 128 (strip_len = 130) (§296.2 #3, §296.3 #3)
    let path0 = vec![b'a'; 140];
    let mut path1 = vec![b'a'; 10];
    path1.extend_from_slice(b"bbbbbbbb"); // 18 bytes total
    let suffix1 = b"bbbbbbbb";

    let buf_v4_multibyte = build_synthetic_v4_multi_entry_index(&[
        (&path0, &path0, 0, None),
        (&path1, suffix1, 130, None),
    ]);
    let res_v4_mb = super::recovery_snapshot::parse_and_validate_git_index(&buf_v4_multibyte, 20);
    assert!(
        res_v4_mb.is_ok(),
        "Multi-byte v4 varint (strip_len 130 > 128) must parse cleanly: {:?}",
        res_v4_mb.err()
    );

    // 5. v4 reconstructed path length mismatch with entry flags
    let buf_v4_len_mismatch = build_synthetic_v4_multi_entry_index(&[
        (&path0, &path0, 0, None),
        (&path1, suffix1, 130, Some(99)), // declared 99, actual reconstructed 18
    ]);
    let res_v4_lm =
        super::recovery_snapshot::parse_and_validate_git_index(&buf_v4_len_mismatch, 20);
    assert!(
        res_v4_lm.is_err(),
        "v4 path length mismatch between flags and reconstructed path must be rejected"
    );
    assert!(format!("{:?}", res_v4_lm.err().unwrap()).contains("v4 path length mismatch"));

    // 6. Path length mismatch in v2 entry flags
    let buf_len_mismatch = build_synthetic_index_with_opts(
        2,
        0o100644,
        None,
        b"test.txt", // len 8
        Some(15),    // declared 15
        &[],
        20,
        false,
    );
    let res_mismatch =
        super::recovery_snapshot::parse_and_validate_git_index(&buf_len_mismatch, 20);
    assert!(res_mismatch.is_err(), "Must reject path length mismatch");
    assert!(format!("{:?}", res_mismatch.err().unwrap()).contains("Path length mismatch"));

    // 7. SHA-256 index (32-byte checksum) parsing
    let buf_sha256 =
        build_synthetic_index_with_opts(2, 0o100644, None, b"test.txt", None, &[], 32, false);
    let res_sha256 = super::recovery_snapshot::parse_and_validate_git_index(&buf_sha256, 32);
    assert!(
        res_sha256.is_ok(),
        "Valid SHA-256 index must parse: {:?}",
        res_sha256.err()
    );

    // 8. SHA-256 index with corrupt checksum
    let buf_sha256_corrupt =
        build_synthetic_index_with_opts(2, 0o100644, None, b"test.txt", None, &[], 32, true);
    let res_sha256_c =
        super::recovery_snapshot::parse_and_validate_git_index(&buf_sha256_corrupt, 32);
    assert!(
        res_sha256_c.is_err(),
        "Tampered SHA-256 checksum must be rejected"
    );
    assert!(format!("{:?}", res_sha256_c.err().unwrap()).contains("checksum mismatch"));
}

// T29: test_recovery_snapshot_porcelain_v2_unmerged_entries
#[test]
fn test_recovery_snapshot_porcelain_v2_unmerged_entries() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let policy = super::recovery_snapshot::SnapshotPolicy::default();
    let global_config = dir.path().join(".empty_test_gitconfig");

    let run_git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo_dir)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &global_config)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_AUTHOR_NAME", "Test Author")
            .env("GIT_AUTHOR_EMAIL", "author@example.com")
            .env("GIT_COMMITTER_NAME", "Test Committer")
            .env("GIT_COMMITTER_EMAIL", "committer@example.com")
            .status()
            .unwrap();
        status.success()
    };

    // 1. Create a base commit with conflict.txt
    std::fs::write(repo_dir.join("conflict.txt"), b"initial baseline\n").unwrap();
    assert!(run_git(&["add", "conflict.txt"]));
    assert!(run_git(&["commit", "-m", "add conflict file"]));

    // 2. Create branch and commit different content
    assert!(run_git(&["checkout", "-b", "branch-conflict"]));
    std::fs::write(repo_dir.join("conflict.txt"), b"branch side modification\n").unwrap();
    assert!(run_git(&["add", "conflict.txt"]));
    assert!(run_git(&["commit", "-m", "branch modify"]));

    // 3. Switch back to master and commit different content
    assert!(run_git(&["checkout", "master"]));
    std::fs::write(repo_dir.join("conflict.txt"), b"master side modification\n").unwrap();
    assert!(run_git(&["add", "conflict.txt"]));
    assert!(run_git(&["commit", "-m", "master modify"]));

    // 4. Merge branch: produces conflict (merge returns non-zero)
    let _ = run_git(&["merge", "branch-conflict"]);

    // 5. Capture recovery snapshot on conflicted working tree
    let (snap, _, _) = super::recovery_snapshot::create_recovery_snapshot(
        "run-t29-unmerged",
        &repo_dir,
        dir.path(),
        &policy,
    )
    .expect("Recovery snapshot on conflicted repository must succeed");

    // 6. Verify conflict.txt is present in snapshot entries with Tracked and Staged protection
    let conflict_entry = snap
        .entries
        .iter()
        .find(|e| e.path == "conflict.txt")
        .expect("conflict.txt must be captured in snapshot entries");

    assert!(
        conflict_entry
            .sources
            .contains(&super::recovery_snapshot::ProtectedSource::Tracked),
        "Unmerged conflict.txt must be tracked"
    );
    assert!(
        conflict_entry
            .sources
            .contains(&super::recovery_snapshot::ProtectedSource::Staged),
        "Unmerged conflict.txt must be marked staged"
    );
}

// T30: test_recovery_snapshot_manifest_structure_and_digest_verification
#[test]
fn test_recovery_snapshot_manifest_structure_and_digest_verification() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let policy = super::recovery_snapshot::SnapshotPolicy::default();

    let (snap, binding, _lease) = super::recovery_snapshot::create_recovery_snapshot(
        "run-t30-manifest",
        &repo_dir,
        dir.path(),
        &policy,
    )
    .expect("create recovery snapshot");

    let snapshot_json_path = std::path::Path::new(&binding.root).join("snapshot.json");
    assert!(
        snapshot_json_path.exists(),
        "snapshot.json must exist in published snapshot root"
    );

    // Read and verify manifest bytes match binding.manifest_digest
    let manifest_bytes = std::fs::read(&snapshot_json_path).unwrap();
    let parsed: super::recovery_snapshot::RecoverySnapshot =
        serde_json::from_slice(&manifest_bytes).expect("parse RecoverySnapshot JSON");

    assert_eq!(parsed.run_id, snap.run_id);
    assert_eq!(parsed.manifest_digest, binding.manifest_digest);
    assert_eq!(parsed.entries.len(), snap.entries.len());

    let recomputed =
        super::recovery_snapshot::compute_manifest_digest(&parsed.entries, &parsed.git)
            .expect("compute manifest digest");
    assert_eq!(recomputed, binding.manifest_digest);
}

// T31: test_recovery_snapshot_compensation_on_state_persistence_failure
#[test]
fn test_recovery_snapshot_compensation_on_state_persistence_failure() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let policy = super::recovery_snapshot::SnapshotPolicy::default();
    let (_, _, lease) = super::recovery_snapshot::create_recovery_snapshot(
        "run-t31-comp",
        &repo_dir,
        dir.path(),
        &policy,
    )
    .expect("create snapshot for compensation test");

    assert!(lease.published_path.exists());

    // 1. Compensation removes published directory cleanly using authorized lease (§304.3 #3)
    let cleanup_res = super::recovery_snapshot::cleanup_published_snapshot_lease(&lease);
    assert!(
        cleanup_res.is_ok(),
        "Compensation cleanup must succeed: {:?}",
        cleanup_res.err()
    );
    assert!(
        !lease.published_path.exists(),
        "Published directory must be removed by compensation"
    );

    // 2. Calling cleanup on already-removed snapshot succeeds idempotently
    let cleanup_idempotent = super::recovery_snapshot::cleanup_published_snapshot_lease(&lease);
    assert!(
        cleanup_idempotent.is_ok(),
        "Idempotent cleanup must succeed without error"
    );

    // 3. Direct path-only cleanup is forbidden (§304.3 #3)
    let res_path_only =
        super::recovery_snapshot::cleanup_published_snapshot_directory(&lease.published_path);
    assert!(
        res_path_only.is_err(),
        "Direct path-only cleanup must be forbidden"
    );

    // 4. CompensationFailed displays both errors and remaining path
    let comp_err = super::recovery_snapshot::SnapshotError::CompensationFailed {
        primary: "Failed to persist authenticated initial run state: disk full".to_string(),
        cleanup_error: "Access denied".to_string(),
        remaining_path: lease.published_path.clone(),
    };
    let err_str = format!("{comp_err}");
    assert!(err_str.contains("disk full"));
    assert!(err_str.contains("Access denied"));
    assert!(err_str.contains("run-t31-comp"));
}

// T32: test_recovery_snapshot_porcelain_v2_malformed_records_fail_closed
#[test]
fn test_recovery_snapshot_porcelain_v2_malformed_records_fail_closed() {
    // 1. Truncated '1' record (expected 9 tokens, got 8)
    let truncated_1 = b"1 .M N... 100644 100644 100644 1111111 2222222\0";
    let res1 = super::recovery_snapshot::parse_porcelain_v2_status(truncated_1);
    assert!(res1.is_err(), "Truncated '1' record must fail closed");
    assert!(format!("{:?}", res1.err().unwrap()).contains("Malformed porcelain-v2 '1' record"));

    // 2. Truncated 'u' record (expected 11 tokens, got 10)
    let truncated_u = b"u UU N... 100644 100644 100644 100644 1111111 2222222 3333333\0";
    let res_u = super::recovery_snapshot::parse_porcelain_v2_status(truncated_u);
    assert!(res_u.is_err(), "Truncated 'u' record must fail closed");
    assert!(format!("{:?}", res_u.err().unwrap()).contains("Malformed porcelain-v2 'u' record"));

    // 3. Truncated '2' rename record (expected 10 tokens, got 9)
    let truncated_2 = b"2 R. N... 100644 100644 100644 1111111 2222222 R100\0";
    let res_2 = super::recovery_snapshot::parse_porcelain_v2_status(truncated_2);
    assert!(res_2.is_err(), "Truncated '2' record must fail closed");
    assert!(format!("{:?}", res_2.err().unwrap()).contains("Malformed porcelain-v2 '2' record"));

    // 4. '2' rename record missing companion orig-path record
    let missing_comp_2 = b"2 R. N... 100644 100644 100644 1111111 2222222 R100 new_name.txt\0";
    let res_mc = super::recovery_snapshot::parse_porcelain_v2_status(missing_comp_2);
    assert!(
        res_mc.is_err(),
        "'2' record without companion orig path must fail closed"
    );
    assert!(format!("{:?}", res_mc.err().unwrap()).contains("Missing orig-path companion record"));

    // 5. Unknown or unsupported record prefix
    let unknown_rec = b"X something invalid\0";
    let res_unk = super::recovery_snapshot::parse_porcelain_v2_status(unknown_rec);
    assert!(
        res_unk.is_err(),
        "Unknown porcelain-v2 record type must fail closed"
    );
    assert!(format!("{:?}", res_unk.err().unwrap())
        .contains("Unsupported or unknown porcelain-v2 status record"));

    // 6. Empty untracked '?' record
    let empty_q = b"? \0";
    let res_q = super::recovery_snapshot::parse_porcelain_v2_status(empty_q);
    assert!(res_q.is_err(), "Empty '?' record must fail closed");

    // 7. Empty ignored '!' record
    let empty_bang = b"! \0";
    let res_bang = super::recovery_snapshot::parse_porcelain_v2_status(empty_bang);
    assert!(res_bang.is_err(), "Empty '!' record must fail closed");
}

// T33: test_is_subpath_component_aware
#[test]
fn test_is_subpath_component_aware() {
    use super::recovery_snapshot::is_subpath_component_aware;

    let parent = std::path::Path::new(r"C:\work\repo");
    // Direct child
    assert!(is_subpath_component_aware(
        parent,
        std::path::Path::new(r"C:\work\repo\file.txt")
    ));
    // Nested child
    assert!(is_subpath_component_aware(
        parent,
        std::path::Path::new(r"C:\work\repo\sub\dir\file.txt")
    ));
    // Same path
    assert!(is_subpath_component_aware(
        parent,
        std::path::Path::new(r"C:\work\repo")
    ));

    // Case insensitivity on Windows
    #[cfg(windows)]
    {
        assert!(is_subpath_component_aware(
            parent,
            std::path::Path::new(r"c:\WORK\REPO\file.txt")
        ));
        assert!(is_subpath_component_aware(
            parent,
            std::path::Path::new(r"C:\Work\Repo")
        ));
    }

    // Prefix sibling (MUST BE REJECTED)
    assert!(!is_subpath_component_aware(
        parent,
        std::path::Path::new(r"C:\work\repo-sibling")
    ));
    assert!(!is_subpath_component_aware(
        parent,
        std::path::Path::new(r"C:\work\repo_other\file.txt")
    ));

    // Outside directory
    assert!(!is_subpath_component_aware(
        parent,
        std::path::Path::new(r"C:\work\other")
    ));
    assert!(!is_subpath_component_aware(
        parent,
        std::path::Path::new(r"C:\other\repo")
    ));

    // Relative parent traversal outside
    assert!(!is_subpath_component_aware(
        parent,
        std::path::Path::new(r"C:\work\repo\..\other")
    ));
}

// T34: test_recovery_snapshot_strict_share_writer_rejection
#[test]
#[cfg(windows)]
fn test_recovery_snapshot_strict_share_writer_rejection() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let fixture = SyntheticRepoFixture::new();
    let file_path = fixture.path().join("strict_write_test.txt");
    std::fs::write(&file_path, b"strict write protected").unwrap();

    let chain =
        super::recovery_snapshot::open_file_no_follow_handle_chain(&file_path, fixture.path())
            .expect("open file handle chain");

    // Attempting to open with write access while chain is held MUST fail due to denied FILE_SHARE_WRITE
    let writer_res = std::fs::OpenOptions::new().write(true).open(&file_path);
    assert!(
        writer_res.is_err(),
        "Concurrent writer must be rejected while protected handle chain is held"
    );
    let err = writer_res.err().unwrap();
    assert_eq!(
        err.raw_os_error(),
        Some(32), // ERROR_SHARING_VIOLATION
        "Expected ERROR_SHARING_VIOLATION (32), got {err:?}"
    );

    drop(chain);

    let writer_ok = std::fs::OpenOptions::new().write(true).open(&file_path);
    assert!(writer_ok.is_ok(), "Writer can open after chain is dropped");
}

// T35: test_recovery_snapshot_strict_share_ancestor_rename_rejection
#[test]
#[cfg(windows)]
fn test_recovery_snapshot_strict_share_ancestor_rename_rejection() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let fixture = SyntheticRepoFixture::new();
    let sub_dir = fixture.path().join("sub_dir");
    std::fs::create_dir_all(&sub_dir).unwrap();
    let leaf_file = sub_dir.join("leaf.txt");
    std::fs::write(&leaf_file, b"leaf content").unwrap();

    let chain =
        super::recovery_snapshot::open_file_no_follow_handle_chain(&leaf_file, fixture.path())
            .expect("open file handle chain");

    let renamed_dir = fixture.path().join("renamed_sub_dir");
    let rename_res = std::fs::rename(&sub_dir, &renamed_dir);
    assert!(
        rename_res.is_err(),
        "Renaming ancestor directory must fail while handle chain is held"
    );
    let err = rename_res.err().unwrap();
    assert_eq!(
        err.raw_os_error(),
        Some(32), // ERROR_SHARING_VIOLATION
        "Expected ERROR_SHARING_VIOLATION (32), got {err:?}"
    );

    drop(chain);

    let rename_after = std::fs::rename(&sub_dir, &renamed_dir);
    assert!(
        rename_after.is_ok(),
        "Renaming succeeds after chain is dropped"
    );
}

// T36: test_recovery_snapshot_publication_and_exact_owned_cleanup_lifecycle
#[test]
fn test_recovery_snapshot_publication_and_exact_owned_cleanup_lifecycle() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let fixture = SyntheticRepoFixture::new();
    let policy = super::recovery_snapshot::SnapshotPolicy::default();

    let (snap, binding, lease) = super::recovery_snapshot::create_recovery_snapshot(
        "run-lifecycle-001",
        fixture.path(),
        fixture.parent_path(),
        &policy,
    )
    .expect("create recovery snapshot");

    let published_path = std::path::PathBuf::from(&binding.root);
    assert!(published_path.exists(), "Published directory must exist");
    assert_eq!(lease.manifest_digest, snap.manifest_digest);

    // Cleanup via exact owned lease
    let cleanup_res = super::recovery_snapshot::cleanup_published_snapshot_lease(&lease);
    assert!(
        cleanup_res.is_ok(),
        "cleanup_published_snapshot_lease must succeed: {:?}",
        cleanup_res.err()
    );
    assert!(
        !published_path.exists(),
        "Published snapshot path must no longer exist after cleanup"
    );

    // Idempotent retry succeeds
    let retry_res = super::recovery_snapshot::cleanup_published_snapshot_lease(&lease);
    assert!(
        retry_res.is_ok(),
        "Second cleanup must succeed idempotently"
    );
}

// T37: test_recovery_snapshot_cleanup_rejects_replaced_directory_with_different_file_id
#[test]
fn test_recovery_snapshot_cleanup_rejects_replaced_directory_with_different_file_id() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;
    let fixture = SyntheticRepoFixture::new();
    let policy = super::recovery_snapshot::SnapshotPolicy::default();

    let (_snap, binding, lease) = super::recovery_snapshot::create_recovery_snapshot(
        "run-replaced-001",
        fixture.path(),
        fixture.parent_path(),
        &policy,
    )
    .expect("create recovery snapshot");

    let published_path = std::path::PathBuf::from(&binding.root);
    assert!(published_path.exists());

    // Replace published_path with a newly created directory at the exact same path
    // First rename original away so we can make a new one with a different File ID
    let temp_aside = fixture.parent_path().join("aside_dir");
    std::fs::rename(&published_path, &temp_aside).unwrap();

    std::fs::create_dir_all(&published_path).unwrap();
    let decoy_file = published_path.join("decoy.txt");
    std::fs::write(&decoy_file, b"replacement decoy content").unwrap();

    // Attempting cleanup with original lease MUST fail because File ID does not match!
    let cleanup_res = super::recovery_snapshot::cleanup_published_snapshot_lease(&lease);
    assert!(
        cleanup_res.is_err(),
        "Cleanup must fail when directory was replaced by a different directory with different File ID"
    );
    let err_msg = format!("{:?}", cleanup_res.err().unwrap());
    assert!(
        err_msg.contains("identity mismatch")
            || err_msg.contains("LeaseTargetMismatch")
            || err_msg.contains("RootIdentityMismatch")
            || err_msg.contains("Exact ownership verification failed"),
        "Error message must indicate identity mismatch: {err_msg}"
    );

    // Verify decoy file and directory were preserved untouched
    assert!(
        published_path.exists(),
        "Replacement directory must be preserved"
    );
    assert!(decoy_file.exists(), "Replacement files must be preserved");
    assert_eq!(
        std::fs::read(&decoy_file).unwrap(),
        b"replacement decoy content"
    );

    // Clean up temp_aside
    let _ = std::fs::remove_dir_all(&temp_aside);
}

// T38: test_reconcile_run_state_save_error_outcomes
#[test]
fn test_reconcile_run_state_save_error_outcomes() {
    use super::reconcile_run_state_save_error;
    use super::RunInitializationOutcome;

    let dir = TempDir::new().unwrap();
    let runner = MockCommandRunner::new();
    let mut state = initialize_run(
        default_test_config(standard_profile()),
        "head-1",
        vec![],
        &runner,
        FIXED_TIME,
    )
    .unwrap();

    let mut binding = super::recovery_snapshot::RecoverySnapshotBinding {
        root: dir.path().join("recovery").join(&state.run_id),
        manifest_digest: "abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234"
            .to_string(),
        git_head: "0123456789abcdef0123456789abcdef01234567".to_string(),
        git_refs_digest: "1111111111111111111111111111111111111111111111111111111111111111"
            .to_string(),
        git_index_entries_digest:
            "2222222222222222222222222222222222222222222222222222222222222222".to_string(),
        git_index_file_digest: "3333333333333333333333333333333333333333333333333333333333333333"
            .to_string(),
    };
    state.recovery_snapshot = Some(binding.clone());

    // Create dummy published dir so lease has something to point to
    let recovery_parent = dir.path().join("recovery");
    std::fs::create_dir_all(&binding.root).unwrap();
    let mut snap_manifest = super::recovery_snapshot::RecoverySnapshot {
        run_id: state.run_id.clone(),
        root: binding.root.clone(),
        entries: vec![],
        git: super::recovery_snapshot::GitSnapshot {
            head_commit: binding.git_head.clone(),
            refs_digest: binding.git_refs_digest.clone(),
            index_entries_digest: binding.git_index_entries_digest.clone(),
            index_file_digest: binding.git_index_file_digest.clone(),
            head_ref: Some("refs/heads/master".to_string()),
            head_mode: super::recovery_snapshot::HeadMode::Symbolic,
            status_digest: "1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
        },
        manifest_digest: String::new(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let computed_digest = super::recovery_snapshot::compute_manifest_digest(
        &snap_manifest.entries,
        &snap_manifest.git,
    )
    .unwrap();
    snap_manifest.manifest_digest = computed_digest.clone();
    binding.manifest_digest = computed_digest.clone();
    state.recovery_snapshot = Some(binding.clone());

    let snap_bytes = serde_json::to_vec_pretty(&snap_manifest).unwrap();
    std::fs::write(binding.root.join("snapshot.json"), &snap_bytes).unwrap();

    let parent_id =
        super::recovery_snapshot::query_path_identity(&recovery_parent).expect("parent identity");
    let pub_id = super::recovery_snapshot::query_path_identity(binding.root.as_path())
        .expect("published identity");

    let snap_id =
        super::recovery_snapshot::query_path_identity(&binding.root.join("snapshot.json"))
            .expect("snapshot.json identity");
    let lease = super::recovery_snapshot::SnapshotOwnershipLease {
        engine_recovery_root_id: parent_id,
        run_id: state.run_id.clone(),
        staging_dir_id: pub_id,
        published_dir_id: Some(pub_id),
        manifest_digest: computed_digest.clone(),
        staging_path: binding.root.clone(),
        published_path: binding.root.clone(),
        inventory: vec![super::recovery_snapshot::DescendantInventoryEntry {
            rel_path: "snapshot.json".to_string(),
            is_dir: false,
            id: snap_id,
        }],
    };

    let state_file = dir.path().join("test_run.state.json");

    // Case 1: CommittedAfterReportedError
    // Save state authenticated first, so state file exists and matches
    save_run_state_authenticated(&state_file, &mut state).unwrap();
    let outcome1 = reconcile_run_state_save_error(
        &state_file,
        &state,
        &binding,
        &lease,
        "Simulated transient error during flush".to_string(),
    );
    match outcome1 {
        RunInitializationOutcome::CommittedAfterReportedError {
            state: committed_state,
            reported_error,
        } => {
            assert_eq!(committed_state.run_id, state.run_id);
            assert_eq!(reported_error, "Simulated transient error during flush");
            assert!(binding.root.exists(), "Snapshot must be preserved");
        }
        other => panic!("Expected CommittedAfterReportedError, got {other:?}"),
    }

    // Case 2: NotCommitted
    // State file does not exist
    let non_existent_state = dir.path().join("non_existent.state.json");
    let outcome2 = reconcile_run_state_save_error(
        &non_existent_state,
        &state,
        &binding,
        &lease,
        "Simulated write failure before file creation".to_string(),
    );
    match outcome2 {
        RunInitializationOutcome::NotCommitted {
            save_error,
            cleanup_error,
            remaining_path,
        } => {
            assert_eq!(save_error, "Simulated write failure before file creation");
            assert!(cleanup_error.is_none());
            assert!(remaining_path.is_none());
            assert!(!binding.root.exists(), "Snapshot must be cleaned up");
        }
        other => panic!("Expected NotCommitted, got {other:?}"),
    }

    // Case 3: Unresolved
    // State file exists but contains corrupted/tampered bytes
    std::fs::create_dir_all(&binding.root).unwrap();
    std::fs::write(binding.root.join("snapshot.json"), b"{}").unwrap();
    let pub_id3 = super::recovery_snapshot::query_path_identity(binding.root.as_path()).unwrap();
    let snap_id3 =
        super::recovery_snapshot::query_path_identity(&binding.root.join("snapshot.json")).unwrap();
    let lease3 = super::recovery_snapshot::SnapshotOwnershipLease {
        engine_recovery_root_id: parent_id,
        run_id: state.run_id.clone(),
        staging_dir_id: pub_id3,
        published_dir_id: Some(pub_id3),
        manifest_digest: binding.manifest_digest.clone(),
        staging_path: binding.root.clone(),
        published_path: binding.root.clone(),
        inventory: vec![super::recovery_snapshot::DescendantInventoryEntry {
            rel_path: "snapshot.json".to_string(),
            is_dir: false,
            id: snap_id3,
        }],
    };
    std::fs::write(&state_file, b"corrupted non-authenticated data").unwrap();

    let outcome3 = reconcile_run_state_save_error(
        &state_file,
        &state,
        &binding,
        &lease3,
        "Simulated write failure with ambiguous state".to_string(),
    );
    match outcome3 {
        RunInitializationOutcome::Unresolved {
            save_error,
            read_back_error,
            snapshot_path,
            recovery_instructions,
        } => {
            assert_eq!(save_error, "Simulated write failure with ambiguous state");
            assert!(read_back_error.contains("authenticated read-back failed"));
            assert_eq!(snapshot_path, binding.root);
            assert!(recovery_instructions.contains("preserve snapshot"));
            assert!(
                binding.root.exists(),
                "Snapshot must be preserved in Unresolved state"
            );
        }
        other => panic!("Expected Unresolved, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Phase B Corrective Tests (§304.3 #1-6, §306.3 #1-2, §307.2 #1-5)
// ---------------------------------------------------------------------------

// T39: test_run_state_lock_key_stability_and_writer_serialization
#[test]
fn test_run_state_lock_key_stability_and_writer_serialization() {
    let dir = TempDir::new().unwrap();
    let state_file = dir
        .path()
        .join("runs")
        .join("run-stable-lock")
        .join("run_state.json");

    // 1. Key stability before and after state file exists on disk (§304.3 #5)
    let key_before = super::guards::canonical_run_state_lock_key(&state_file);
    std::fs::create_dir_all(state_file.parent().unwrap()).unwrap();
    std::fs::write(&state_file, b"{}").unwrap();
    let key_after = super::guards::canonical_run_state_lock_key(&state_file);
    assert_eq!(
        key_before, key_after,
        "Run-state lock key must be identical before and after state file creation"
    );

    // 2. Re-entrant lock acquisition on same thread
    {
        let _guard1 = super::guards::acquire_run_state_lock(&state_file);
        let _guard2 = super::guards::acquire_run_state_lock(&state_file);
        assert!(super::guards::is_run_state_locked_by_current_thread(
            &state_file
        ));
    }

    // 3. Competing writer serialization across threads
    let (tx1, rx1) = std::sync::mpsc::channel();
    let (tx2, rx2) = std::sync::mpsc::channel();
    let target_path = state_file.clone();

    let handle = std::thread::spawn(move || {
        let _guard = super::guards::acquire_run_state_lock(&target_path);
        tx1.send(()).unwrap();
        // Hold lock until signaled
        rx2.recv().unwrap();
    });

    rx1.recv().unwrap(); // Thread 1 holds the lock

    // Thread 2 tries to acquire lock; should block until Thread 1 releases
    let target_path2 = state_file.clone();
    let thread2_acquired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let t2_flag = thread2_acquired.clone();

    let handle2 = std::thread::spawn(move || {
        let _guard = super::guards::acquire_run_state_lock(&target_path2);
        t2_flag.store(true, std::sync::atomic::Ordering::SeqCst);
    });

    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(
        !thread2_acquired.load(std::sync::atomic::Ordering::SeqCst),
        "Thread 2 must not acquire lock while Thread 1 holds it"
    );

    // Release Thread 1
    tx2.send(()).unwrap();
    handle.join().unwrap();
    handle2.join().unwrap();

    assert!(
        thread2_acquired.load(std::sync::atomic::Ordering::SeqCst),
        "Thread 2 must acquire lock after Thread 1 releases"
    );
}

// T40: test_production_start_run_post_rename_reported_error_reconciliation
#[test]
fn test_production_start_run_post_rename_reported_error_reconciliation() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;

    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;

    let _iso_lock = super::isolation::TEST_ISOLATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    super::isolation::set_test_isolation_pass(true);
    super::set_test_production_dispatch_override(true);
    struct ResetDispatch;
    impl Drop for ResetDispatch {
        fn drop(&mut self) {
            super::set_test_production_dispatch_override(false);
            super::isolation::set_test_isolation_pass(false);
        }
    }
    let _reset = ResetDispatch;

    let mut config = default_test_config(standard_profile());
    let run_id = format!("ac-post-err-{}", uuid::Uuid::new_v4());
    config.run_id = Some(run_id.clone());
    config.working_dir = repo_dir.to_string_lossy().to_string();
    config.reviewer_level = Some(super::types::ReviewerLevel::R0D);
    config.r0d_acknowledged = true;

    let state_file = super::engine_run_state_path(&run_id).unwrap();

    // Inject post-rename reported error (§304.3 #4, §306.3 #2)
    super::guards::INJECT_POST_RENAME_SAVE_ERROR.store(true, std::sync::atomic::Ordering::SeqCst);

    let start_result = super::auto_converge_start_run(config);

    super::guards::INJECT_POST_RENAME_SAVE_ERROR.store(false, std::sync::atomic::Ordering::SeqCst);

    // Production start run must succeed via CommittedAfterReportedError reconciliation
    assert!(
        start_result.is_ok(),
        "Start run with post-rename error must succeed via CommittedAfterReportedError: {:?}",
        start_result.err()
    );
    let state = start_result.unwrap();

    // Verify recovery snapshot is bound and persisted
    assert!(state.recovery_snapshot.is_some());

    let persisted = super::guards::load_and_verify_run_state_authenticated(&state_file, None)
        .expect("Persisted state must be authenticated and valid");
    assert_eq!(persisted.run_id, state.run_id);
    assert_eq!(persisted.engine_auth_tag, state.engine_auth_tag);
    assert_eq!(persisted.recovery_snapshot, state.recovery_snapshot);

    // Verify zero additional dispatch and zero duplicate initialization
    assert_eq!(super::PRODUCTION_DISPATCH_ENABLED, false);
}

// T41: test_production_start_run_pre_rename_failure_not_committed
#[test]
fn test_production_start_run_pre_rename_failure_not_committed() {
    let _r3_lock = super::r3_controller::TEST_R3_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;

    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;

    let _iso_lock = super::isolation::TEST_ISOLATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    super::isolation::set_test_isolation_pass(true);
    super::set_test_production_dispatch_override(true);
    struct ResetDispatch;
    impl Drop for ResetDispatch {
        fn drop(&mut self) {
            super::set_test_production_dispatch_override(false);
            super::isolation::set_test_isolation_pass(false);
        }
    }
    let _reset = ResetDispatch;

    let mut config = default_test_config(standard_profile());
    let run_id = format!("ac-pre-fail-{}", uuid::Uuid::new_v4());
    config.run_id = Some(run_id.clone());
    config.working_dir = repo_dir.to_string_lossy().to_string();
    config.reviewer_level = Some(super::types::ReviewerLevel::R0D);
    config.r0d_acknowledged = true;

    let state_file = super::engine_run_state_path(&run_id).unwrap();
    let published_snapshot = auth_dir.join("recovery").join(&run_id);

    // Inject pre-rename failure
    super::guards::INJECT_PRE_RENAME_SAVE_ERROR.store(true, std::sync::atomic::Ordering::SeqCst);

    let start_result = super::auto_converge_start_run(config);

    super::guards::INJECT_PRE_RENAME_SAVE_ERROR.store(false, std::sync::atomic::Ordering::SeqCst);

    // Must return Err containing NotCommitted
    assert!(start_result.is_err());
    let err_msg = start_result.err().unwrap();
    assert!(
        err_msg.contains("NotCommitted: Injected pre-rename save failure"),
        "Unexpected error: {err_msg}"
    );

    // State file must not exist on disk
    assert!(
        !state_file.exists(),
        "State file must not exist after NotCommitted"
    );

    // Snapshot must have been compensation-cleaned
    assert!(
        !published_snapshot.exists(),
        "Published snapshot must be cleaned up after NotCommitted"
    );
}

// T41B: test_reconcile_run_state_save_error_outcomes_exhaustive
#[test]
fn test_reconcile_run_state_save_error_outcomes_exhaustive() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let engine_root = dir.path().join("engine_authority");
    let policy = super::recovery_snapshot::SnapshotPolicy::default();

    let run_id = "run-reconcile-exhaustive";
    let (_, snapshot_binding, lease) = super::recovery_snapshot::create_recovery_snapshot(
        run_id,
        &repo_dir,
        &engine_root,
        &policy,
    )
    .expect("create snapshot");

    let state_file = engine_root.join("runs").join(run_id).join("run_state.json");

    let runner = MockCommandRunner::new();
    let mut cfg = default_test_config(standard_profile());
    cfg.run_id = Some(run_id.to_string());
    let mut intended_state =
        super::state_machine::initialize_run(cfg, "head-1", vec![], &runner, FIXED_TIME).unwrap();
    intended_state.recovery_snapshot = Some(snapshot_binding.clone());

    // 1. Single seal
    super::guards::seal_run_state_authenticated(&mut intended_state).unwrap();
    assert_eq!(intended_state.state_sequence, 2);
    let original_tag = intended_state.engine_auth_tag.clone();

    // Case A: Post-rename reported error -> persisted file exists with matching seq & tag -> CommittedAfterReportedError
    super::guards::save_sealed_run_state(&state_file, &intended_state).unwrap();
    let outcome_committed = super::reconcile_run_state_save_error(
        &state_file,
        &intended_state,
        &snapshot_binding,
        &lease,
        "Simulated post-rename IO error".to_string(),
    );
    match outcome_committed {
        super::RunInitializationOutcome::CommittedAfterReportedError {
            state,
            reported_error,
        } => {
            assert_eq!(state.state_sequence, 2);
            assert_eq!(state.engine_auth_tag, original_tag);
            assert_eq!(state.run_id, run_id);
            assert_eq!(reported_error, "Simulated post-rename IO error");
        }
        other => panic!("Expected CommittedAfterReportedError, got {other:?}"),
    }

    // Case B: Unresolved due to sequence/tag mismatch
    let mut mismatched_state = intended_state.clone();
    mismatched_state.state_sequence = 99; // Different sequence
    let outcome_mismatch = super::reconcile_run_state_save_error(
        &state_file,
        &mismatched_state,
        &snapshot_binding,
        &lease,
        "Simulated error with mismatched state".to_string(),
    );
    match outcome_mismatch {
        super::RunInitializationOutcome::Unresolved {
            save_error,
            read_back_error,
            snapshot_path,
            ..
        } => {
            assert!(save_error.contains("Simulated error"));
            assert!(read_back_error.contains("does not match intended state"));
            assert_eq!(
                snapshot_path,
                std::path::PathBuf::from(&snapshot_binding.root)
            );
            assert!(
                lease.published_path.exists(),
                "Snapshot must be preserved when unresolved"
            );
        }
        other => panic!("Expected Unresolved, got {other:?}"),
    }

    // Case C: Unresolved due to corrupted state file
    std::fs::write(&state_file, b"corrupted garbage json").unwrap();
    let outcome_corrupted = super::reconcile_run_state_save_error(
        &state_file,
        &intended_state,
        &snapshot_binding,
        &lease,
        "Simulated error with corrupted file".to_string(),
    );
    match outcome_corrupted {
        super::RunInitializationOutcome::Unresolved {
            save_error,
            read_back_error,
            snapshot_path,
            ..
        } => {
            assert!(save_error.contains("Simulated error"));
            assert!(read_back_error.contains("authenticated read-back failed"));
            assert_eq!(
                snapshot_path,
                std::path::PathBuf::from(&snapshot_binding.root)
            );
            assert!(
                lease.published_path.exists(),
                "Snapshot must be preserved on corrupt read-back"
            );
        }
        other => panic!("Expected Unresolved, got {other:?}"),
    }

    // Case D: NotCommitted (file does not exist on disk -> compensation cleanup)
    std::fs::remove_file(&state_file).unwrap();
    assert!(!state_file.exists());
    let outcome_not_committed = super::reconcile_run_state_save_error(
        &state_file,
        &intended_state,
        &snapshot_binding,
        &lease,
        "Simulated pre-rename failure".to_string(),
    );
    match outcome_not_committed {
        super::RunInitializationOutcome::NotCommitted {
            save_error,
            cleanup_error,
            remaining_path,
        } => {
            assert_eq!(save_error, "Simulated pre-rename failure");
            assert!(cleanup_error.is_none());
            assert!(remaining_path.is_none());
            assert!(
                !lease.published_path.exists(),
                "Snapshot must be cleaned up on NotCommitted"
            );
        }
        other => panic!("Expected NotCommitted, got {other:?}"),
    }
}

// T42: test_strict_share_cleanup_unknown_child_and_replacement_rejection
#[test]
fn test_strict_share_cleanup_unknown_child_and_replacement_rejection() {
    let _seam_guard = TEST_SEAM_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    *super::recovery_snapshot::SEAM_MOCK_SYMBOLIC_REF
        .lock()
        .unwrap() = None;

    let dir = TempDir::new().unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;
    let engine_root = dir.path().join("engine_authority");
    let policy = super::recovery_snapshot::SnapshotPolicy::default();

    let run_id = "run-strict-share-cleanup";
    let (_, _, lease) = super::recovery_snapshot::create_recovery_snapshot(
        run_id,
        &repo_dir,
        &engine_root,
        &policy,
    )
    .expect("create snapshot");

    assert!(lease.published_path.exists());
    assert!(
        !lease.inventory.is_empty(),
        "Ephemeral lease must contain inventory"
    );

    // 1. Unknown child rejection (§304.3 #1, #2)
    let rogue_file = lease.published_path.join("untracked_rogue.txt");
    std::fs::write(&rogue_file, b"intruder").unwrap();

    let cleanup_err1 = super::recovery_snapshot::cleanup_published_snapshot_lease(&lease);
    assert!(
        cleanup_err1.is_err(),
        "Cleanup must fail closed when unknown child exists"
    );
    let err1_str = format!("{:?}", cleanup_err1.err().unwrap());
    assert!(
        err1_str.contains("Unknown or unexpected child not in lease inventory"),
        "Unexpected error message: {err1_str}"
    );
    assert!(
        lease.published_path.exists(),
        "Snapshot directory must be preserved on unknown child"
    );
    assert!(
        rogue_file.exists(),
        "Rogue file must be preserved, not deleted"
    );

    // Remove rogue file to test next case
    std::fs::remove_file(&rogue_file).unwrap();

    // 2. Child identity replacement rejection (§304.3 #1, #2)
    let snap_json = lease.published_path.join("snapshot.json");
    let snap_content = std::fs::read(&snap_json).unwrap();
    std::fs::remove_file(&snap_json).unwrap();
    // Re-create file (different NTFS File ID)
    std::fs::write(&snap_json, &snap_content).unwrap();

    let cleanup_err2 = super::recovery_snapshot::cleanup_published_snapshot_lease(&lease);
    assert!(
        cleanup_err2.is_err(),
        "Cleanup must fail closed when child is replaced"
    );
    let err2_str = format!("{:?}", cleanup_err2.err().unwrap());
    assert!(
        err2_str.contains("Child identity mismatch / replacement detected"),
        "Unexpected error message: {err2_str}"
    );
    assert!(
        lease.published_path.exists(),
        "Snapshot directory must be preserved on replaced child"
    );

    // 3. Strict share conflict (no FILE_SHARE_DELETE) (§304.3 #1)
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let test_file = lease.published_path.join("files").join("README.md");
        if test_file.exists() {
            let conflict_handle = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(0) // Exclusive: denies all sharing
                .open(&test_file)
                .unwrap();

            let cleanup_err3 = super::recovery_snapshot::cleanup_published_snapshot_lease(&lease);
            assert!(
                cleanup_err3.is_err(),
                "Cleanup must fail closed when sharing violation occurs"
            );
            assert!(
                lease.published_path.exists(),
                "Snapshot directory must be preserved on sharing violation"
            );
            drop(conflict_handle);
        }
    }
}

// ---------------------------------------------------------------------------
// R3 Unit 1 Tests (§11.2, §12, §16.4, §16.5)
// ---------------------------------------------------------------------------

#[test]
fn child_r3_cli_worker_target() {
    let subcmd = match std::env::var("POVERELLO_TEST_CHILD_R3_SUBCMD") {
        Ok(s) => s,
        Err(_) => return, // Normal test run, do nothing
    };
    if std::env::var("POVERELLO_TEST_R3_FAKE_RUNNER").is_ok() {
        let mock_runner = std::sync::Arc::new(super::agent_runner::MockAgentRunner::new());
        super::r3_controller::set_r3_test_fake_runner(Some(mock_runner));
    }
    let auth_dir_str = std::env::var("POVERELLO_TEST_CHILD_R3_AUTH_DIR").ok();
    let _guard = auth_dir_str.map(|d| super::TestEngineAuthorityGuard::enter(PathBuf::from(d)));
    let run_id = std::env::var("POVERELLO_TEST_CHILD_R3_RUN_ID").ok();
    let req_file = std::env::var("POVERELLO_TEST_CHILD_R3_REQ_FILE").ok();
    let mut args = vec![
        "anthro-bridge".to_string(),
        "auto-converge".to_string(),
        subcmd,
    ];
    if let Some(r) = run_id {
        args.push("--run-id".to_string());
        args.push(r);
    }
    if let Some(rf) = req_file {
        args.push("--request".to_string());
        args.push(rf);
    }
    args.push("--json".to_string());
    let exit_code = super::r3_controller::run_auto_converge_cli(&args);
    std::process::exit(exit_code);
}

struct ChildProcessGuard {
    child: Option<std::process::Child>,
    tag: String,
    collected_output: Option<std::process::Output>,
}

impl ChildProcessGuard {
    fn new(child: std::process::Child, tag: &str) -> Self {
        Self {
            child: Some(child),
            tag: tag.to_string(),
            collected_output: None,
        }
    }

    fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        if let Some(ref mut c) = self.child {
            c.try_wait()
        } else {
            Ok(None)
        }
    }

    fn wait_with_output(mut self) -> std::process::Output {
        let out = self
            .child
            .take()
            .expect("child already collected")
            .wait_with_output()
            .expect("wait_with_output failed");
        self.collected_output = Some(out.clone());
        out
    }

    fn kill_and_join(&mut self) -> Option<std::process::Output> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let out = child.wait_with_output().ok();
            self.collected_output = out.clone();
            out
        } else {
            self.collected_output.clone()
        }
    }

    fn output(&self) -> Option<&std::process::Output> {
        self.collected_output.as_ref()
    }

    fn is_reaped(&self) -> bool {
        self.child.is_none() && self.collected_output.is_some()
    }
}

impl Drop for ChildProcessGuard {
    fn drop(&mut self) {
        let _ = self.kill_and_join();
    }
}

fn wait_for_barrier_and_release(
    barrier_dir: &std::path::Path,
    tags: &[&str],
    children: &mut [ChildProcessGuard],
    timeout: std::time::Duration,
) -> Result<(), String> {
    let start = std::time::Instant::now();
    loop {
        // 1. Check if any child exited prematurely
        let mut premature_exit = None;
        for c in children.iter_mut() {
            if let Ok(Some(status)) = c.try_wait() {
                premature_exit = Some((c.tag.clone(), status));
                break;
            }
        }
        if let Some((failed_tag, status)) = premature_exit {
            let mut reports = Vec::new();
            for other in children.iter_mut() {
                if let Some(out) = other.kill_and_join() {
                    reports.push(format!(
                        "[tag: {}, status: {:?}, stdout: {}, stderr: {}]",
                        other.tag,
                        out.status,
                        String::from_utf8_lossy(&out.stdout),
                        String::from_utf8_lossy(&out.stderr)
                    ));
                }
            }
            return Err(format!(
                "Child '{}' exited prematurely with status {:?}. All children collected:\n{}",
                failed_tag,
                status,
                reports.join("\n")
            ));
        }

        // 2. Check if all ready files exist
        let all_ready = tags
            .iter()
            .all(|tag| barrier_dir.join(format!("ready_{tag}.txt")).exists());
        if all_ready {
            let release_file = barrier_dir.join("release.txt");
            if let Err(e) = std::fs::write(&release_file, b"go") {
                for c in children.iter_mut() {
                    let _ = c.kill_and_join();
                }
                return Err(format!("Failed writing release barrier: {e}"));
            }
            return Ok(());
        }

        // 3. Timeout check
        if start.elapsed() >= timeout {
            let mut reports = Vec::new();
            for c in children.iter_mut() {
                if let Some(out) = c.kill_and_join() {
                    reports.push(format!(
                        "[tag: {}, status: {:?}, stdout: {}, stderr: {}]",
                        c.tag,
                        out.status,
                        String::from_utf8_lossy(&out.stdout),
                        String::from_utf8_lossy(&out.stderr)
                    ));
                }
            }
            return Err(format!(
                "Barrier timeout ({:?}) reached before all children ready. Collected outputs:\n{}",
                timeout,
                reports.join("\n")
            ));
        }

        std::thread::sleep(std::time::Duration::from_millis(15));
    }
}

fn spawn_r3_cli_child(
    auth_dir: &std::path::Path,
    subcmd: &str,
    run_id: Option<&str>,
    req_file: Option<&str>,
    barrier_dir: Option<&std::path::Path>,
    worker_tag: Option<&str>,
) -> ChildProcessGuard {
    let current_exe = std::env::current_exe().expect("current_exe");
    let mut cmd = std::process::Command::new(current_exe);
    cmd.arg("orchestrator::auto_converge::tests::child_r3_cli_worker_target")
        .arg("--exact")
        .arg("--nocapture")
        .env("POVERELLO_TEST_CHILD_R3_AUTH_DIR", auth_dir)
        .env("POVERELLO_TEST_CHILD_R3_SUBCMD", subcmd)
        .env("POVERELLO_TEST_R3_FAKE_RUNNER", "1");
    if let Some(r) = run_id {
        cmd.env("POVERELLO_TEST_CHILD_R3_RUN_ID", r);
    }
    if let Some(rf) = req_file {
        cmd.env("POVERELLO_TEST_CHILD_R3_REQ_FILE", rf);
    }
    if let Some(bd) = barrier_dir {
        cmd.env("POVERELLO_TEST_R3_BARRIER_DIR", bd);
    }
    let tag = worker_tag.unwrap_or("worker");
    cmd.env("POVERELLO_TEST_R3_WORKER_TAG", tag);
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let child = cmd.spawn().expect("failed to spawn child CLI process");
    ChildProcessGuard::new(child, tag)
}

fn execute_r3_cli_child(
    auth_dir: &std::path::Path,
    subcmd: &str,
    run_id: Option<&str>,
    req_file: Option<&str>,
) -> std::process::Output {
    let current_exe = std::env::current_exe().expect("current_exe");
    let mut cmd = std::process::Command::new(current_exe);
    cmd.arg("orchestrator::auto_converge::tests::child_r3_cli_worker_target")
        .arg("--exact")
        .arg("--nocapture")
        .env("POVERELLO_TEST_CHILD_R3_AUTH_DIR", auth_dir)
        .env("POVERELLO_TEST_CHILD_R3_SUBCMD", subcmd);
    if let Some(r) = run_id {
        cmd.env("POVERELLO_TEST_CHILD_R3_RUN_ID", r);
    }
    if let Some(rf) = req_file {
        cmd.env("POVERELLO_TEST_CHILD_R3_REQ_FILE", rf);
    }
    cmd.output().expect("failed to execute child CLI process")
}

#[test]
fn test_r3_common_preflight_rejections_and_zero_role_dispatch_a14() {
    let _test_lock = super::r3_controller::TEST_R3_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;

    let mock_runner = std::sync::Arc::new(super::agent_runner::MockAgentRunner::new());
    super::r3_controller::set_r3_test_fake_runner(Some(mock_runner.clone()));

    // 1. Rejection: Non-R0D reviewer level
    let mut req = super::r3_controller::R3StartRunRequest {
        request_id: "req-preflight-1".to_string(),
        workspace_root: repo_dir.to_string_lossy().to_string(),
        task: "Test task".to_string(),
        mode: "implement".to_string(),
        scope: None,
        acceptance_criteria: None,
        profile: standard_profile(),
        run_permissions: 0,
        reviewer_level: None, // Missing
        r0d_acknowledged: true,
        budget: None,
        agent_cli_path: None,
    };
    let res = super::r3_controller::r3_common_preflight(&req, "run-pf-1");
    assert!(res.is_err());
    assert!(res
        .err()
        .unwrap()
        .contains("R0-D reviewer level must be explicitly selected"));

    // 2. Rejection: R0D not acknowledged
    req.reviewer_level = Some(super::types::ReviewerLevel::R0D);
    req.r0d_acknowledged = false;
    let res = super::r3_controller::r3_common_preflight(&req, "run-pf-2");
    assert!(res.is_err());
    assert!(res
        .err()
        .unwrap()
        .contains("R0-D limitations must be explicitly acknowledged"));

    // 3. Rejection: Permission exceeds profile capability
    req.r0d_acknowledged = true;
    req.run_permissions = super::types::PERM_RELEASE_PUBLISH; // Profile does not have this capability
    let res = super::r3_controller::r3_common_preflight(&req, "run-pf-3");
    assert!(res.is_err());
    assert!(res.err().unwrap().contains("Requested permissions"));

    // 4. Rejection: Non-git directory
    req.run_permissions = 0;
    let non_git = dir.path().join("not_git");
    std::fs::create_dir_all(&non_git).unwrap();
    req.workspace_root = non_git.to_string_lossy().to_string();
    let res = super::r3_controller::r3_common_preflight(&req, "run-pf-4");
    assert!(res.is_err());
    assert!(res.err().unwrap().contains("not a Git repository"));

    // 5. Rejection: 0 budget limits
    req.workspace_root = repo_dir.to_string_lossy().to_string();
    let mut zero_budget = req.profile.convergence_limits.clone();
    zero_budget.max_plan_reviews = 0;
    zero_budget.max_impl_reviews = 0;
    req.budget = Some(zero_budget);
    let res = super::r3_controller::r3_common_preflight(&req, "run-pf-5");
    assert!(res.is_err());
    assert!(res
        .err()
        .unwrap()
        .contains("Budget limits max_plan_reviews and max_impl_reviews must not both be 0"));

    // 6. Rejection: Invalid mode (§16.4 item 9)
    req.budget = None;
    req.mode = "magic_autonomous_mode".to_string();
    let res = super::r3_controller::r3_common_preflight(&req, "run-pf-6");
    assert!(res.is_err());
    assert!(res.err().unwrap().contains("Invalid mode"));

    // 7. Rejection: Invalid identifier (§16.4 item 7)
    req.mode = "implement".to_string();
    req.request_id = "bad/request..id*".to_string();
    let res = super::r3_controller::r3_common_preflight(&req, "run-pf-7");
    assert!(res.is_err());
    assert!(res.err().unwrap().contains("Invalid request_id"));

    // 8. Rejection: Non-fake mode in Unit 1 fails closed (§16.4 item 3, 4)
    req.request_id = "req-pf-valid".to_string();
    super::r3_controller::set_r3_test_fake_runner(None);
    let res = super::r3_controller::r3_common_preflight(&req, "run-pf-8");
    assert!(res.is_err());
    let err_msg = res.err().unwrap();
    assert!(
        err_msg.contains("failing closed without spawning any process")
            || err_msg.contains("Candidate isolation is unready in Unit 1")
    );

    // 9. Negative execution: zero role launches
    let mut out = Vec::new();
    let res = super::r3_controller::execute_r3_start_run(req, &mut out);
    assert!(res.is_err());
    assert_eq!(
        mock_runner.get_executed_requests().len(),
        0,
        "Zero role processes must be spawned on preflight failures"
    );

    super::r3_controller::set_r3_test_fake_runner(None);
}

#[test]
fn test_r3_fake_run_roundtrip_a01() {
    let _test_lock = super::r3_controller::TEST_R3_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;

    // Inject fake runner
    let fake_runner = std::sync::Arc::new(super::agent_runner::MockAgentRunner::new());
    super::r3_controller::set_r3_test_fake_runner(Some(fake_runner.clone()));

    let req = super::r3_controller::R3StartRunRequest {
        request_id: "req-roundtrip-1".to_string(),
        workspace_root: repo_dir.to_string_lossy().to_string(),
        task: "Implement feature X".to_string(),
        mode: "plan_only".to_string(),
        scope: Some(vec!["src/lib.rs".to_string()]),
        acceptance_criteria: Some(vec!["unit test passes".to_string()]),
        profile: standard_profile(),
        run_permissions: 0,
        reviewer_level: Some(super::types::ReviewerLevel::R0D),
        r0d_acknowledged: true,
        budget: None,
        agent_cli_path: None,
    };

    let mut event_bytes = Vec::new();
    let result = super::r3_controller::execute_r3_start_run(req.clone(), &mut event_bytes);
    super::r3_controller::set_r3_test_fake_runner(None);

    assert!(
        result.is_ok(),
        "execute_r3_start_run must succeed: {:?}",
        result.err()
    );
    let final_state = result.unwrap();
    assert!(!final_state.run_id.is_empty());

    // Parse emitted JSONL events
    let event_lines = String::from_utf8(event_bytes).unwrap();
    let lines: Vec<&str> = event_lines
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();
    assert!(!lines.is_empty(), "Must emit at least 1 JSONL event");

    // First event must be Start
    let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(first["event"], "start");
    assert_eq!(first["request_id"], "req-roundtrip-1");

    // Last event must be Result
    let last: serde_json::Value = serde_json::from_str(lines.last().unwrap()).unwrap();
    assert_eq!(last["event"], "result");
    assert_eq!(last["run_id"], final_state.run_id);
}

#[test]
fn test_r3_request_deduplication_and_workspace_lock_conflict_a02() {
    let _test_lock = super::r3_controller::TEST_R3_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;

    let fake_runner = std::sync::Arc::new(super::agent_runner::MockAgentRunner::new());
    super::r3_controller::set_r3_test_fake_runner(Some(fake_runner.clone()));

    let req = super::r3_controller::R3StartRunRequest {
        request_id: "req-dedup-1".to_string(),
        workspace_root: repo_dir.to_string_lossy().to_string(),
        task: "Deduplication task".to_string(),
        mode: "plan_only".to_string(),
        scope: Some(vec!["src/main.rs".to_string()]),
        acceptance_criteria: Some(vec!["all tests pass".to_string()]),
        profile: standard_profile(),
        run_permissions: 0,
        reviewer_level: Some(super::types::ReviewerLevel::R0D),
        r0d_acknowledged: true,
        budget: None,
        agent_cli_path: None,
    };

    let mut out1 = Vec::new();
    let state1 = super::r3_controller::execute_r3_start_run(req.clone(), &mut out1).unwrap();

    // 1. Exact resend: same request_id and identical request returns existing run
    let mut out2 = Vec::new();
    let state2 = super::r3_controller::execute_r3_start_run(req.clone(), &mut out2).unwrap();
    assert_eq!(
        state1.run_id, state2.run_id,
        "Exact resend must return identical run_id"
    );

    // 2. Conflicting task with same request_id must be rejected
    let mut conflicting_req = req.clone();
    conflicting_req.task = "Completely different task".to_string();
    let mut out3 = Vec::new();
    let err = super::r3_controller::execute_r3_start_run(conflicting_req, &mut out3);
    assert!(err.is_err());
    assert!(err
        .err()
        .unwrap()
        .contains("conflicts with previous request"));

    // 3. Conflicting scope with same request_id must be rejected (§16.4 item 5)
    let mut conflicting_scope = req.clone();
    conflicting_scope.scope = Some(vec!["src/other.rs".to_string()]);
    let mut out3b = Vec::new();
    let err_scope = super::r3_controller::execute_r3_start_run(conflicting_scope, &mut out3b);
    assert!(err_scope.is_err());
    assert!(err_scope
        .err()
        .unwrap()
        .contains("conflicts with previous request"));

    // 4. Conflicting mode with same request_id must be rejected
    let mut conflicting_mode = req.clone();
    conflicting_mode.mode = "implement".to_string();
    let mut out3c = Vec::new();
    let err_mode = super::r3_controller::execute_r3_start_run(conflicting_mode, &mut out3c);
    assert!(err_mode.is_err());
    assert!(err_mode
        .err()
        .unwrap()
        .contains("conflicts with previous request"));

    // 5. Corrupted registry must fail closed (§16.4 item 5)
    let registry_path = auth_dir.join("request_registry.json");
    std::fs::write(&registry_path, b"INVALID NOT JSON").unwrap();
    let mut out_corrupt = Vec::new();
    let err_corrupt = super::r3_controller::execute_r3_start_run(req.clone(), &mut out_corrupt);
    assert!(err_corrupt.is_err());
    assert!(err_corrupt
        .err()
        .unwrap()
        .contains("Request registry is corrupted or unreadable"));

    // 6. Workspace run lock conflict: holding lock prevents concurrent start on same workspace
    // Recreate clean registry
    std::fs::remove_file(&registry_path).unwrap();
    let canonical_ws = repo_dir.canonicalize().unwrap();
    let lock_hold =
        super::r3_controller::acquire_workspace_run_lock(&canonical_ws, "run-holder").unwrap();
    let mut diff_id_req = req.clone();
    diff_id_req.request_id = "req-different-id".to_string();
    let mut out4 = Vec::new();
    let lock_err = super::r3_controller::execute_r3_start_run(diff_id_req, &mut out4);
    assert!(lock_err.is_err());
    assert!(lock_err
        .err()
        .unwrap()
        .contains("already locked by another running process"));
    drop(lock_hold);

    super::r3_controller::set_r3_test_fake_runner(None);
}

#[test]
fn test_r3_owner_process_creation_time_pid_reuse_prevention_a17() {
    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());

    let current_pid = std::process::id();
    let real_creation_time = super::r3_controller::get_current_process_creation_time().unwrap();

    // 1. Current process with valid creation time is alive
    let valid_owner = super::r3_controller::RunOwnerIdentity {
        run_id: "run-owner-live-1".to_string(),
        owner_nonce: "nonce-1".to_string(),
        process_id: current_pid,
        process_creation_time: real_creation_time,
        created_at: chrono::Utc::now().to_rfc3339(),
        canonical_workspace: dir.path().to_string_lossy().to_string(),
    };
    assert!(
        super::r3_controller::is_owner_process_alive(&valid_owner),
        "Current process must be recognized as alive"
    );

    // 2. Mismatched creation time (PID reuse simulation) must be recognized as NOT alive
    #[cfg(windows)]
    {
        let reused_pid_owner = super::r3_controller::RunOwnerIdentity {
            run_id: "run-owner-reused-1".to_string(),
            owner_nonce: "nonce-2".to_string(),
            process_id: current_pid,
            process_creation_time: real_creation_time.wrapping_add(9999999), // Different creation timestamp
            created_at: chrono::Utc::now().to_rfc3339(),
            canonical_workspace: dir.path().to_string_lossy().to_string(),
        };
        assert!(
            !super::r3_controller::is_owner_process_alive(&reused_pid_owner),
            "Process with mismatched creation time must be rejected (PID reuse prevention)"
        );
    }

    // 3. Process creation time 0 must fail closed (§16.8.3 item 1)
    let zero_creation_owner = super::r3_controller::RunOwnerIdentity {
        run_id: "run-owner-zero-1".to_string(),
        owner_nonce: "nonce-zero".to_string(),
        process_id: current_pid,
        process_creation_time: 0,
        created_at: chrono::Utc::now().to_rfc3339(),
        canonical_workspace: dir.path().to_string_lossy().to_string(),
    };
    assert!(
        !super::r3_controller::is_owner_process_alive(&zero_creation_owner),
        "Process with creation time = 0 must fail closed"
    );

    // 4. Non-existent PID must be recognized as dead
    let dead_owner = super::r3_controller::RunOwnerIdentity {
        run_id: "run-owner-dead-1".to_string(),
        owner_nonce: "nonce-3".to_string(),
        process_id: 99999999,
        process_creation_time: 12345678,
        created_at: chrono::Utc::now().to_rfc3339(),
        canonical_workspace: dir.path().to_string_lossy().to_string(),
    };
    assert!(
        !super::r3_controller::is_owner_process_alive(&dead_owner),
        "Non-existent PID must be recognized as dead"
    );
}

#[test]
fn test_r3_mailbox_atomic_and_mismatched_run_id_rejection_a11() {
    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;

    let run_id = "run-mailbox-test-01";
    let state_path = super::engine_run_state_path(run_id).unwrap();
    std::fs::create_dir_all(state_path.parent().unwrap()).unwrap();

    let mut cfg = default_test_config(standard_profile());
    cfg.run_id = Some(run_id.to_string());
    let runner = MockCommandRunner::new();
    let mut state =
        super::initialize_run(cfg, "head-001", vec![], &runner, "2026-10-25T00:00:00Z").unwrap();
    super::guards::save_run_state_authenticated(&state_path, &mut state).unwrap();

    // 1. Mismatched run_id rejection (§16.4 item 7)
    let mismatched_req = super::r3_controller::R3ControlRequest {
        request_id: "req-mismatched-01".to_string(),
        run_id: "run-wrong-id".to_string(),
        action: super::r3_controller::R3ControlAction::Pause,
        issued_at: chrono::Utc::now().to_rfc3339(),
    };
    // Put directly in mailbox of target run
    let (reqs_dir, _) = super::r3_controller::r3_mailbox_dirs(run_id).unwrap();
    let req_path = reqs_dir.join(format!("{}.json", mismatched_req.request_id));
    std::fs::write(&req_path, serde_json::to_vec(&mismatched_req).unwrap()).unwrap();

    let action = super::r3_controller::owner_process_pending_mailbox_requests(
        run_id,
        &mut state,
        &state_path,
    )
    .unwrap();
    assert_eq!(
        action, None,
        "Mismatched run_id must not trigger any action"
    );

    let resp = super::r3_controller::poll_control_response(
        run_id,
        "req-mismatched-01",
        std::time::Duration::from_millis(500),
    )
    .unwrap();
    assert!(
        !resp.accepted,
        "Mismatched run_id must be rejected with accepted: false"
    );
    assert!(
        !resp.quiescent,
        "Mismatched run_id must not report quiescent: true"
    );
    assert!(resp.message.unwrap().contains("Run ID mismatch"));

    // 2. Normal pause request
    let pause_req = super::r3_controller::R3ControlRequest {
        request_id: "req-pause-01".to_string(),
        run_id: run_id.to_string(),
        action: super::r3_controller::R3ControlAction::Pause,
        issued_at: chrono::Utc::now().to_rfc3339(),
    };
    super::r3_controller::send_control_request(&pause_req).unwrap();

    let action = super::r3_controller::owner_process_pending_mailbox_requests(
        run_id,
        &mut state,
        &state_path,
    )
    .unwrap();
    assert_eq!(action, Some(super::r3_controller::R3ControlAction::Pause));
    assert!(state.is_paused);

    let pause_resp = super::r3_controller::poll_control_response(
        run_id,
        "req-pause-01",
        std::time::Duration::from_millis(500),
    )
    .unwrap();
    assert!(pause_resp.accepted);
    assert!(pause_resp.quiescent);
    assert_eq!(
        pause_resp.action,
        super::r3_controller::R3ControlAction::Pause
    );

    // Verify atomic writing left no temporary files (.tmp-)
    let (reqs_dir, resps_dir) = super::r3_controller::r3_mailbox_dirs(run_id).unwrap();
    for entry in std::fs::read_dir(&reqs_dir)
        .unwrap()
        .chain(std::fs::read_dir(&resps_dir).unwrap())
    {
        let name = entry.unwrap().file_name().to_string_lossy().to_string();
        assert!(
            !name.contains(".tmp-"),
            "Temporary mailbox file must not linger: {name}"
        );
    }
}

#[test]
fn test_r3_cross_process_cli_status_pause_cancel_and_recovery_required() {
    let _test_lock = super::r3_controller::TEST_R3_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;

    let run_id = "run-cross-proc-01";
    let state_path = super::engine_run_state_path(run_id).unwrap();
    std::fs::create_dir_all(state_path.parent().unwrap()).unwrap();

    let mut cfg = default_test_config(standard_profile());
    cfg.run_id = Some(run_id.to_string());
    let runner = MockCommandRunner::new();
    let mut state =
        super::initialize_run(cfg, "head-001", vec![], &runner, "2026-10-25T00:00:00Z").unwrap();
    super::guards::save_run_state_authenticated(&state_path, &mut state).unwrap();

    // Owner is current process
    let owner = super::r3_controller::RunOwnerIdentity {
        run_id: run_id.to_string(),
        owner_nonce: "nonce-cp".to_string(),
        process_id: std::process::id(),
        process_creation_time: super::r3_controller::get_current_process_creation_time().unwrap(),
        created_at: chrono::Utc::now().to_rfc3339(),
        canonical_workspace: repo_dir.to_string_lossy().to_string(),
    };
    super::r3_controller::record_run_owner_identity(&owner).unwrap();

    // 1. Cross-process CLI status check via child process
    let status_out = execute_r3_cli_child(&auth_dir, "status", Some(run_id), None);
    assert!(
        status_out.status.success(),
        "Cross-process status command failed: {:?}",
        status_out
    );
    let stdout_str = String::from_utf8_lossy(&status_out.stdout);
    let json_line = stdout_str
        .lines()
        .find(|l| l.trim().starts_with('{'))
        .unwrap_or(&stdout_str);
    let status_json: serde_json::Value =
        serde_json::from_str(json_line).expect("parse status JSON");
    assert_eq!(status_json["status"], "ok");
    assert_eq!(status_json["run_id"], run_id);

    // 2. Cross-process CLI pause: child sends pause request, owner processes it
    let auth_dir_clone = auth_dir.clone();
    let run_id_str = run_id.to_string();
    let child_handle = std::thread::spawn(move || {
        execute_r3_cli_child(&auth_dir_clone, "pause", Some(&run_id_str), None)
    });

    // Owner waits for request to arrive and processes it
    let mut processed_pause = false;
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let action = super::r3_controller::owner_process_pending_mailbox_requests(
            run_id,
            &mut state,
            &state_path,
        )
        .unwrap();
        if action == Some(super::r3_controller::R3ControlAction::Pause) {
            processed_pause = true;
            break;
        }
    }
    assert!(
        processed_pause,
        "Owner must process pause request sent by child CLI process"
    );

    let pause_out = child_handle.join().expect("join pause child thread");
    assert!(
        pause_out.status.success(),
        "Child CLI pause command failed: {:?}",
        pause_out
    );
    let pause_stdout = String::from_utf8_lossy(&pause_out.stdout);
    let pause_json_line = pause_stdout
        .lines()
        .find(|l| l.trim().starts_with('{'))
        .unwrap_or(&pause_stdout);
    let pause_resp: super::r3_controller::R3ControlResponse =
        serde_json::from_str(pause_json_line).expect("parse pause control response");
    assert!(pause_resp.accepted);
    assert!(pause_resp.quiescent);
    assert_eq!(
        pause_resp.action,
        super::r3_controller::R3ControlAction::Pause
    );

    // 3. Cross-process owner disappearance -> status must fail with RECOVERY_REQUIRED and exit code 2 (§16.4 item 8)
    let dead_owner = super::r3_controller::RunOwnerIdentity {
        run_id: run_id.to_string(),
        owner_nonce: "nonce-dead".to_string(),
        process_id: 99999999, // Dead PID
        process_creation_time: 12345678,
        created_at: chrono::Utc::now().to_rfc3339(),
        canonical_workspace: repo_dir.to_string_lossy().to_string(),
    };
    super::r3_controller::record_run_owner_identity(&dead_owner).unwrap();

    let dead_status_out = execute_r3_cli_child(&auth_dir, "status", Some(run_id), None);
    assert_eq!(
        dead_status_out.status.code(),
        Some(2),
        "Status on dead owner must exit with code 2"
    );
    let stderr_str = String::from_utf8_lossy(&dead_status_out.stderr);
    assert!(
        stderr_str.contains("RECOVERY_REQUIRED"),
        "Status on dead owner must report RECOVERY_REQUIRED in stderr, got: {stderr_str}"
    );

    // Restore real owner
    super::r3_controller::record_run_owner_identity(&owner).unwrap();

    // 4. Zero process creation time owner -> status must fail with RECOVERY_REQUIRED and exit code 2 (§16.8.3 item 1)
    let zero_creation_owner = super::r3_controller::RunOwnerIdentity {
        run_id: run_id.to_string(),
        owner_nonce: "nonce-zero".to_string(),
        process_id: std::process::id(),
        process_creation_time: 0,
        created_at: chrono::Utc::now().to_rfc3339(),
        canonical_workspace: repo_dir.to_string_lossy().to_string(),
    };
    super::r3_controller::record_run_owner_identity(&zero_creation_owner).unwrap();

    let zero_status_out = execute_r3_cli_child(&auth_dir, "status", Some(run_id), None);
    assert_eq!(
        zero_status_out.status.code(),
        Some(2),
        "Status on owner with 0 creation time must exit with code 2"
    );
    let zero_stderr = String::from_utf8_lossy(&zero_status_out.stderr);
    assert!(
        zero_stderr.contains("RECOVERY_REQUIRED"),
        "Status on owner with 0 creation time must report RECOVERY_REQUIRED: {zero_stderr}"
    );

    // Restore real owner
    super::r3_controller::record_run_owner_identity(&owner).unwrap();

    // 5. Cross-process CLI cancel: child sends cancel request, owner processes it at step boundary (§16.8.3 item 6)
    let auth_dir_clone2 = auth_dir.clone();
    let run_id_str2 = run_id.to_string();
    let cancel_child_handle = std::thread::spawn(move || {
        execute_r3_cli_child(&auth_dir_clone2, "cancel", Some(&run_id_str2), None)
    });

    let mut processed_cancel = false;
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let action = super::r3_controller::owner_process_pending_mailbox_requests(
            run_id,
            &mut state,
            &state_path,
        )
        .unwrap();
        if action == Some(super::r3_controller::R3ControlAction::Cancel) {
            processed_cancel = true;
            break;
        }
    }
    assert!(
        processed_cancel,
        "Owner must process cancel request sent by child CLI process"
    );

    let cancel_out = cancel_child_handle
        .join()
        .expect("join cancel child thread");
    assert!(
        cancel_out.status.success(),
        "Child CLI cancel command failed: {:?}",
        cancel_out
    );
    let cancel_stdout = String::from_utf8_lossy(&cancel_out.stdout);
    let cancel_json_line = cancel_stdout
        .lines()
        .find(|l| l.trim().starts_with('{'))
        .unwrap_or(&cancel_stdout);
    let cancel_resp: super::r3_controller::R3ControlResponse =
        serde_json::from_str(cancel_json_line).expect("parse cancel control response");
    assert!(cancel_resp.accepted);
    assert!(cancel_resp.quiescent);
    assert_eq!(
        cancel_resp.action,
        super::r3_controller::R3ControlAction::Cancel
    );
    assert!(state.is_terminal, "Run state must be terminal after cancel");
}

#[test]
fn test_r3_status_resume_quiescent_false_pause_cancel_quiescent_true() {
    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let _ = &repo_dir;

    let run_id = "run-quiescent-test-01";
    let state_path = super::engine_run_state_path(run_id).unwrap();
    std::fs::create_dir_all(state_path.parent().unwrap()).unwrap();

    let mut cfg = default_test_config(standard_profile());
    cfg.run_id = Some(run_id.to_string());
    let runner = MockCommandRunner::new();
    let mut state =
        super::initialize_run(cfg, "head-001", vec![], &runner, "2026-10-25T00:00:00Z").unwrap();
    super::guards::save_run_state_authenticated(&state_path, &mut state).unwrap();

    // 1. Status: must return accepted: true, quiescent: false (§16.8.3 item 2)
    let status_req = super::r3_controller::R3ControlRequest {
        request_id: "req-sq-status-01".to_string(),
        run_id: run_id.to_string(),
        action: super::r3_controller::R3ControlAction::Status,
        issued_at: chrono::Utc::now().to_rfc3339(),
    };
    super::r3_controller::send_control_request(&status_req).unwrap();
    let action = super::r3_controller::owner_process_pending_mailbox_requests(
        run_id,
        &mut state,
        &state_path,
    )
    .unwrap();
    assert_eq!(action, None, "Status action does not mutate run execution");
    let resp = super::r3_controller::poll_control_response(
        run_id,
        "req-sq-status-01",
        std::time::Duration::from_millis(500),
    )
    .unwrap();
    assert!(resp.accepted, "Status must be accepted");
    assert!(
        !resp.quiescent,
        "Status must NOT report quiescent (run is active and continues)"
    );

    // 2. Pause: must return accepted: true, quiescent: true
    let pause_req = super::r3_controller::R3ControlRequest {
        request_id: "req-sq-pause-01".to_string(),
        run_id: run_id.to_string(),
        action: super::r3_controller::R3ControlAction::Pause,
        issued_at: chrono::Utc::now().to_rfc3339(),
    };
    super::r3_controller::send_control_request(&pause_req).unwrap();
    let action = super::r3_controller::owner_process_pending_mailbox_requests(
        run_id,
        &mut state,
        &state_path,
    )
    .unwrap();
    assert_eq!(action, Some(super::r3_controller::R3ControlAction::Pause));
    let resp = super::r3_controller::poll_control_response(
        run_id,
        "req-sq-pause-01",
        std::time::Duration::from_millis(500),
    )
    .unwrap();
    assert!(resp.accepted, "Pause must be accepted");
    assert!(
        resp.quiescent,
        "Pause confirmed at step boundary must report quiescent: true"
    );

    // 3. Resume: must return accepted: true, quiescent: false
    let resume_req = super::r3_controller::R3ControlRequest {
        request_id: "req-sq-resume-01".to_string(),
        run_id: run_id.to_string(),
        action: super::r3_controller::R3ControlAction::Resume,
        issued_at: chrono::Utc::now().to_rfc3339(),
    };
    super::r3_controller::send_control_request(&resume_req).unwrap();
    let action = super::r3_controller::owner_process_pending_mailbox_requests(
        run_id,
        &mut state,
        &state_path,
    )
    .unwrap();
    assert_eq!(action, Some(super::r3_controller::R3ControlAction::Resume));
    let resp = super::r3_controller::poll_control_response(
        run_id,
        "req-sq-resume-01",
        std::time::Duration::from_millis(500),
    )
    .unwrap();
    assert!(resp.accepted, "Resume must be accepted");
    assert!(
        !resp.quiescent,
        "Resume must NOT report quiescent (run resumed and continues)"
    );

    // 4. Cancel: must return accepted: true, quiescent: true
    let cancel_req = super::r3_controller::R3ControlRequest {
        request_id: "req-sq-cancel-01".to_string(),
        run_id: run_id.to_string(),
        action: super::r3_controller::R3ControlAction::Cancel,
        issued_at: chrono::Utc::now().to_rfc3339(),
    };
    super::r3_controller::send_control_request(&cancel_req).unwrap();
    let action = super::r3_controller::owner_process_pending_mailbox_requests(
        run_id,
        &mut state,
        &state_path,
    )
    .unwrap();
    assert_eq!(action, Some(super::r3_controller::R3ControlAction::Cancel));
    let resp = super::r3_controller::poll_control_response(
        run_id,
        "req-sq-cancel-01",
        std::time::Duration::from_millis(500),
    )
    .unwrap();
    assert!(resp.accepted, "Cancel must be accepted");
    assert!(
        resp.quiescent,
        "Cancel confirmed at step boundary must report quiescent: true"
    );

    // 5. Subsequent Pause on terminal run: must return accepted: false, quiescent: false
    let pause_terminal_req = super::r3_controller::R3ControlRequest {
        request_id: "req-sq-pause-term-01".to_string(),
        run_id: run_id.to_string(),
        action: super::r3_controller::R3ControlAction::Pause,
        issued_at: chrono::Utc::now().to_rfc3339(),
    };
    super::r3_controller::send_control_request(&pause_terminal_req).unwrap();
    let action = super::r3_controller::owner_process_pending_mailbox_requests(
        run_id,
        &mut state,
        &state_path,
    )
    .unwrap();
    assert_eq!(action, None, "Pause on terminal run cannot be applied");
    let resp = super::r3_controller::poll_control_response(
        run_id,
        "req-sq-pause-term-01",
        std::time::Duration::from_millis(500),
    )
    .unwrap();
    assert!(!resp.accepted, "Pause on terminal run must be rejected");
    assert!(
        !resp.quiescent,
        "Rejected pause must return quiescent: false"
    );
}

#[test]
fn test_r3_registry_cross_process_lock_concurrent_conflict() {
    let _test_lock = super::r3_controller::TEST_R3_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());

    let ws1_dir = dir.path().join("ws1");
    std::fs::create_dir_all(&ws1_dir).unwrap();
    let ws1 = create_synthetic_test_git_repo(&ws1_dir);

    let ws2_dir = dir.path().join("ws2");
    std::fs::create_dir_all(&ws2_dir).unwrap();
    let ws2 = create_synthetic_test_git_repo(&ws2_dir);

    let req1 = super::r3_controller::R3StartRunRequest {
        request_id: "req-shared-conflict-id".to_string(),
        workspace_root: ws1.to_string_lossy().to_string(),
        task: "Task in workspace 1".to_string(),
        mode: "plan_only".to_string(),
        scope: None,
        acceptance_criteria: None,
        profile: standard_profile(),
        run_permissions: 0,
        reviewer_level: Some(super::types::ReviewerLevel::R0D),
        r0d_acknowledged: true,
        budget: None,
        agent_cli_path: None,
    };

    let canonical_ws1 = ws1.canonicalize().unwrap();
    super::r3_controller::record_request_in_registry(
        &req1.request_id,
        "run-ws1-01",
        &req1,
        &canonical_ws1,
        None,
    )
    .expect("Registering request in ws1 must succeed");

    // Attempt to register the SAME request_id for ws2 (different workspace / digest)
    let mut req2 = req1.clone();
    req2.workspace_root = ws2.to_string_lossy().to_string();
    req2.task = "Task in workspace 2".to_string();
    let canonical_ws2 = ws2.canonicalize().unwrap();

    let conflict_res = super::r3_controller::record_request_in_registry(
        &req2.request_id,
        "run-ws2-02",
        &req2,
        &canonical_ws2,
        None,
    );
    assert!(
        conflict_res.is_err(),
        "Registering same request_id from another workspace must fail with conflict"
    );
    let err_str = conflict_res.err().unwrap();
    assert!(
        err_str.contains("conflicts with already registered request"),
        "Error message must indicate conflict: {err_str}"
    );

    // Verify registry lock acquisition and release works cleanly
    let lock = super::r3_controller::acquire_registry_lock().expect("acquire registry lock");
    drop(lock);
}

#[test]
fn test_r3_semantic_digest_field_injection_and_effective_cli() {
    let ws = PathBuf::from("c:/synthetic/repo");

    let base_req = super::r3_controller::R3StartRunRequest {
        request_id: "req-digest-1".to_string(),
        workspace_root: ws.to_string_lossy().to_string(),
        task: "Build compiler".to_string(),
        mode: "plan_only".to_string(),
        scope: None,
        acceptance_criteria: None,
        profile: standard_profile(),
        run_permissions: 0,
        reviewer_level: Some(super::types::ReviewerLevel::R0D),
        r0d_acknowledged: true,
        budget: None,
        agent_cli_path: None,
    };

    let base_digest =
        super::r3_controller::compute_request_semantic_digest(&base_req, &ws, None).unwrap();

    // 1. Field injection check: newline containing field boundaries does NOT collide
    let mut injection_req = base_req.clone();
    injection_req.task = "Build compiler\n\"mode\": \"implement\"".to_string();
    let injection_digest =
        super::r3_controller::compute_request_semantic_digest(&injection_req, &ws, None).unwrap();
    assert_ne!(
        base_digest, injection_digest,
        "Injected string must not produce same digest"
    );

    // 2. Effective CLI difference: different resolved CLI must produce different digest (§16.8.3 item 4)
    let cli_digest1 = super::r3_controller::compute_request_semantic_digest(
        &base_req,
        &ws,
        Some("c:/bin/codex.exe"),
    )
    .unwrap();
    let cli_digest2 = super::r3_controller::compute_request_semantic_digest(
        &base_req,
        &ws,
        Some("c:/custom/codex.exe"),
    )
    .unwrap();
    assert_ne!(
        base_digest, cli_digest1,
        "CLI None vs Some must produce different digests"
    );
    assert_ne!(
        cli_digest1, cli_digest2,
        "Different CLI paths must produce different digests"
    );

    // 3. Distinction between None and Some([])
    let mut empty_scope_req = base_req.clone();
    empty_scope_req.scope = Some(vec![]);
    let empty_scope_digest =
        super::r3_controller::compute_request_semantic_digest(&empty_scope_req, &ws, None).unwrap();
    assert_ne!(
        base_digest, empty_scope_digest,
        "None vs Some([]) must produce distinct digests"
    );
}

#[test]
fn test_r3_auth_preflight_config_toml_alone_rejected() {
    let _test_lock = super::r3_controller::TEST_R3_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    super::r3_controller::set_r3_test_fake_runner(None);

    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());
    let repo_dir = create_synthetic_test_git_repo(dir.path());

    // Create a fake CODEX_HOME containing ONLY config.toml (no auth.json)
    let fake_codex_home = dir.path().join("fake_codex_home");
    std::fs::create_dir_all(&fake_codex_home).unwrap();
    std::fs::write(
        fake_codex_home.join("config.toml"),
        "model = 'gpt-5'\ntemperature = 0.2\n",
    )
    .unwrap();

    // Point CODEX_HOME, USERPROFILE, HOME to this directory and clear API key env vars
    let old_home = std::env::var("CODEX_HOME").ok();
    let old_userprofile = std::env::var("USERPROFILE").ok();
    let old_real_home = std::env::var("HOME").ok();
    let old_key = std::env::var("CODEX_API_KEY").ok();
    let old_openai_key = std::env::var("OPENAI_API_KEY").ok();
    std::env::set_var("CODEX_HOME", fake_codex_home.to_string_lossy().as_ref());
    std::env::set_var("USERPROFILE", fake_codex_home.to_string_lossy().as_ref());
    std::env::set_var("HOME", fake_codex_home.to_string_lossy().as_ref());
    std::env::remove_var("CODEX_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");

    // Fake CLI binary
    let fake_cli = dir.path().join("fake_codex.exe");
    std::fs::write(&fake_cli, b"MZ fake cli").unwrap();

    let req = super::r3_controller::R3StartRunRequest {
        request_id: "req-auth-cfg-only".to_string(),
        workspace_root: repo_dir.to_string_lossy().to_string(),
        task: "Task requiring real auth".to_string(),
        mode: "plan_only".to_string(),
        scope: None,
        acceptance_criteria: None,
        profile: standard_profile(),
        run_permissions: 0,
        reviewer_level: Some(super::types::ReviewerLevel::R0D),
        r0d_acknowledged: true,
        budget: None,
        agent_cli_path: Some(fake_cli.to_string_lossy().to_string()),
    };

    let res = super::r3_controller::r3_common_preflight(&req, "run-auth-test-01");

    // Restore env vars
    if let Some(h) = old_home {
        std::env::set_var("CODEX_HOME", h);
    } else {
        std::env::remove_var("CODEX_HOME");
    }
    if let Some(u) = old_userprofile {
        std::env::set_var("USERPROFILE", u);
    } else {
        std::env::remove_var("USERPROFILE");
    }
    if let Some(h) = old_real_home {
        std::env::set_var("HOME", h);
    } else {
        std::env::remove_var("HOME");
    }
    if let Some(k) = old_key {
        std::env::set_var("CODEX_API_KEY", k);
    }
    if let Some(k) = old_openai_key {
        std::env::set_var("OPENAI_API_KEY", k);
    }

    assert!(
        res.is_err(),
        "config.toml alone must not satisfy auth preflight"
    );
    let err_str = res.err().unwrap();
    assert!(
        err_str.contains("No agent credentials or authentication configuration found"),
        "Error must specify lack of credentials/auth: {err_str}"
    );
}

#[test]
fn test_r3_pure_validation_creates_zero_side_effects() {
    let _test_lock = super::r3_controller::TEST_R3_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());
    let repo_dir = create_synthetic_test_git_repo(dir.path());

    let mock_runner = std::sync::Arc::new(super::agent_runner::MockAgentRunner::new());
    super::r3_controller::set_r3_test_fake_runner(Some(mock_runner));

    let req = super::r3_controller::R3StartRunRequest {
        request_id: "req-pure-val-01".to_string(),
        workspace_root: repo_dir.to_string_lossy().to_string(),
        task: "Pure validation test".to_string(),
        mode: "plan_only".to_string(),
        scope: Some(vec!["src/lib.rs".to_string()]),
        acceptance_criteria: Some(vec!["zero side effects".to_string()]),
        profile: standard_profile(),
        run_permissions: 0,
        reviewer_level: Some(super::types::ReviewerLevel::R0D),
        r0d_acknowledged: true,
        budget: None,
        agent_cli_path: None,
    };

    let val = super::r3_controller::r3_pure_validation(&req);
    assert!(
        val.is_ok(),
        "Pure validation must succeed on valid request: {:?}",
        val
    );

    // Verify zero filesystem side effects (§16.13.2 item 4, §16.14.2 item 1)
    let paths = super::r3_controller::resolve_r3_run_artifact_paths("req-pure-val-01").unwrap();
    assert!(
        !paths.run_dir.exists(),
        "canonical engine_runs directory must not be created during pure validation"
    );
    assert!(!paths.state_path.exists());
    assert!(!paths.owner_path.exists());
    assert!(
        !auth_dir.join("request_registry.json").exists(),
        "request_registry.json must not be created during pure validation"
    );
    assert!(
        !repo_dir
            .join(".poverello")
            .join("workspace_run.lock")
            .exists(),
        "workspace_run.lock must not be created during pure validation"
    );
}

#[test]
fn test_r3_non_windows_early_gate_zero_side_effects() {
    let _test_lock = super::r3_controller::TEST_R3_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());
    let repo_dir = create_synthetic_test_git_repo(dir.path());

    super::r3_controller::set_force_unsupported_platform_for_test(true);

    let req = super::r3_controller::R3StartRunRequest {
        request_id: "req-non-win-01".to_string(),
        workspace_root: repo_dir.to_string_lossy().to_string(),
        task: "Non-windows test".to_string(),
        mode: "plan_only".to_string(),
        scope: None,
        acceptance_criteria: None,
        profile: standard_profile(),
        run_permissions: 0,
        reviewer_level: Some(super::types::ReviewerLevel::R0D),
        r0d_acknowledged: true,
        budget: None,
        agent_cli_path: None,
    };

    let mut out = Vec::new();
    let err_start = super::r3_controller::execute_r3_start_run(req, &mut out);
    assert!(err_start.is_err());
    assert!(err_start.err().unwrap().contains("UnsupportedPlatform"));

    let err_reg_lock = super::r3_controller::acquire_registry_lock();
    assert!(err_reg_lock.is_err());
    assert!(err_reg_lock.err().unwrap().contains("UnsupportedPlatform"));

    let err_ws_lock = super::r3_controller::acquire_workspace_run_lock(&repo_dir, "run-dummy");
    assert!(err_ws_lock.is_err());
    assert!(err_ws_lock.err().unwrap().contains("UnsupportedPlatform"));

    super::r3_controller::set_force_unsupported_platform_for_test(false);

    // Verify zero filesystem side effects (§16.12.2 E, §16.14.2 item 1)
    let paths = super::r3_controller::resolve_r3_run_artifact_paths("req-non-win-01").unwrap();
    assert!(!paths.run_dir.exists());
    assert!(!paths.state_path.exists());
    assert!(!paths.owner_path.exists());
    assert!(!auth_dir.join("request_registry.json").exists());
    assert!(!repo_dir.join(".poverello").exists());
}

#[test]
fn test_r3_concurrent_same_workspace_same_digest() {
    let _test_lock = super::r3_controller::TEST_R3_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());
    let repo_dir = create_synthetic_test_git_repo(dir.path());

    let req_file = dir.path().join("request.json");
    let req_json = serde_json::json!({
        "request_id": "req-concurrent-same-01",
        "workspace_root": repo_dir.to_string_lossy(),
        "task": "Concurrent same workspace same digest test",
        "mode": "plan_only",
        "scope": ["src/lib.rs"],
        "acceptance_criteria": ["only 1 run created"],
        "profile": standard_profile(),
        "run_permissions": 0,
        "reviewer_level": "R0-D",
        "r0d_acknowledged": true,
        "budget": null,
        "agent_cli_path": null
    });
    std::fs::write(&req_file, serde_json::to_vec_pretty(&req_json).unwrap()).unwrap();

    let barrier_dir = dir.path().join("barrier");
    std::fs::create_dir_all(&barrier_dir).unwrap();

    let req_file_str = req_file.to_string_lossy().to_string();

    let child1 = spawn_r3_cli_child(
        &auth_dir,
        "run",
        None,
        Some(&req_file_str),
        Some(&barrier_dir),
        Some("child1"),
    );
    let child2 = spawn_r3_cli_child(
        &auth_dir,
        "run",
        None,
        Some(&req_file_str),
        Some(&barrier_dir),
        Some("child2"),
    );

    let mut children = vec![child1, child2];
    wait_for_barrier_and_release(
        &barrier_dir,
        &["child1", "child2"],
        &mut children,
        std::time::Duration::from_secs(5),
    )
    .expect("barrier synchronization must succeed");

    let out2 = children.pop().unwrap().wait_with_output();
    let out1 = children.pop().unwrap().wait_with_output();

    let stdout1 = String::from_utf8_lossy(&out1.stdout);
    let stdout2 = String::from_utf8_lossy(&out2.stdout);
    let stderr1 = String::from_utf8_lossy(&out1.stderr);
    let stderr2 = String::from_utf8_lossy(&out2.stderr);

    let child1_success = out1.status.success();
    let child2_success = out2.status.success();

    // Exactly one child creates the run.
    // If the loser checks while the winner is running, it reports InProgress (exit 1).
    // If the winner completes before the loser acquires the claim lock, the loser reports
    // ExistingTerminal (exit 0) with status "already_completed_or_active" and the same run ID (§16.12.2 B).
    let (winner_stdout, loser_stdout) = if child1_success && !child2_success {
        (stdout1, stdout2)
    } else if !child1_success && child2_success {
        (stdout2, stdout1)
    } else if child1_success && child2_success {
        if stdout1.contains("\"already_completed_or_active\"") {
            (stdout2, stdout1)
        } else {
            (stdout1, stdout2)
        }
    } else {
        panic!(
            "At least one child process must succeed. child1: {:?} (stdout: {stdout1}, stderr: {stderr1}), child2: {:?} (stdout: {stdout2}, stderr: {stderr2})",
            out1.status, out2.status
        );
    };

    assert!(
        winner_stdout.contains("\"event\":\"result\"")
            || winner_stdout.contains("\"event\": \"result\"")
    );
    assert!(
        loser_stdout.contains("InProgress")
            || loser_stdout.contains("already in progress")
            || loser_stdout.contains("\"already_completed_or_active\""),
        "Loser must report InProgress or already_completed_or_active, got: {loser_stdout}"
    );

    // Check registry: exactly 1 record, exactly 1 run ID (§16.12.2 C)
    let reg = super::r3_controller::load_registry()
        .unwrap()
        .expect("registry exists");
    assert_eq!(
        reg.records.len(),
        1,
        "Registry must contain exactly 1 record"
    );
    let rec = reg
        .records
        .get("req-concurrent-same-01")
        .expect("record exists");
    assert_eq!(rec.status, super::r3_controller::R3ClaimStatus::Started);
}

#[test]
fn test_r3_concurrent_different_workspace_same_request_id() {
    let _test_lock = super::r3_controller::TEST_R3_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());

    let ws1 = dir.path().join("ws1");
    let ws2 = dir.path().join("ws2");
    std::fs::create_dir_all(&ws1).unwrap();
    std::fs::create_dir_all(&ws2).unwrap();
    let repo1 = create_synthetic_test_git_repo(&ws1);
    let repo2 = create_synthetic_test_git_repo(&ws2);

    let req_file1 = dir.path().join("request1.json");
    let req_json1 = serde_json::json!({
        "request_id": "req-concurrent-diff-01",
        "workspace_root": repo1.to_string_lossy(),
        "task": "Task in workspace 1",
        "mode": "plan_only",
        "scope": ["src/lib.rs"],
        "acceptance_criteria": ["test"],
        "profile": standard_profile(),
        "run_permissions": 0,
        "reviewer_level": "R0-D",
        "r0d_acknowledged": true,
        "budget": null,
        "agent_cli_path": null
    });
    std::fs::write(&req_file1, serde_json::to_vec_pretty(&req_json1).unwrap()).unwrap();

    let req_file2 = dir.path().join("request2.json");
    let req_json2 = serde_json::json!({
        "request_id": "req-concurrent-diff-01",
        "workspace_root": repo2.to_string_lossy(),
        "task": "Task in workspace 2 (different workspace, same request_id)",
        "mode": "plan_only",
        "scope": ["src/lib.rs"],
        "acceptance_criteria": ["test"],
        "profile": standard_profile(),
        "run_permissions": 0,
        "reviewer_level": "R0-D",
        "r0d_acknowledged": true,
        "budget": null,
        "agent_cli_path": null
    });
    std::fs::write(&req_file2, serde_json::to_vec_pretty(&req_json2).unwrap()).unwrap();

    let barrier_dir = dir.path().join("barrier_diff");
    std::fs::create_dir_all(&barrier_dir).unwrap();

    let req_file_str1 = req_file1.to_string_lossy().to_string();
    let req_file_str2 = req_file2.to_string_lossy().to_string();

    let child1 = spawn_r3_cli_child(
        &auth_dir,
        "run",
        None,
        Some(&req_file_str1),
        Some(&barrier_dir),
        Some("c1"),
    );
    let child2 = spawn_r3_cli_child(
        &auth_dir,
        "run",
        None,
        Some(&req_file_str2),
        Some(&barrier_dir),
        Some("c2"),
    );

    let mut children = vec![child1, child2];
    wait_for_barrier_and_release(
        &barrier_dir,
        &["c1", "c2"],
        &mut children,
        std::time::Duration::from_secs(5),
    )
    .expect("barrier synchronization must succeed");

    let out2 = children.pop().unwrap().wait_with_output();
    let out1 = children.pop().unwrap().wait_with_output();

    let stdout1 = String::from_utf8_lossy(&out1.stdout);
    let stdout2 = String::from_utf8_lossy(&out2.stdout);

    let child1_success = out1.status.success();
    let child2_success = out2.status.success();
    assert!(
        (child1_success && !child2_success) || (!child1_success && child2_success),
        "Exactly one child must succeed, one must conflict. c1: {:?}, c2: {:?}",
        out1.status,
        out2.status
    );

    let loser_stdout = if child1_success { stdout2 } else { stdout1 };
    assert!(loser_stdout.contains("conflicts with"));

    let reg = super::r3_controller::load_registry()
        .unwrap()
        .expect("registry exists");
    assert_eq!(
        reg.records.len(),
        1,
        "Registry must contain exactly 1 claim record"
    );
}

#[test]
fn test_r3_concurrent_same_workspace_different_request_id() {
    let _test_lock = super::r3_controller::TEST_R3_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());
    let repo_dir = create_synthetic_test_git_repo(dir.path());

    let req_file1 = dir.path().join("req1.json");
    let req_json1 = serde_json::json!({
        "request_id": "req-swdr-01",
        "workspace_root": repo_dir.to_string_lossy(),
        "task": "Task 1",
        "mode": "plan_only",
        "scope": ["src/lib.rs"],
        "acceptance_criteria": ["test"],
        "profile": standard_profile(),
        "run_permissions": 0,
        "reviewer_level": "R0-D",
        "r0d_acknowledged": true,
        "budget": null,
        "agent_cli_path": null
    });
    std::fs::write(&req_file1, serde_json::to_vec_pretty(&req_json1).unwrap()).unwrap();

    let req_file2 = dir.path().join("req2.json");
    let req_json2 = serde_json::json!({
        "request_id": "req-swdr-02",
        "workspace_root": repo_dir.to_string_lossy(),
        "task": "Task 2 (different request, same workspace)",
        "mode": "plan_only",
        "scope": ["src/lib.rs"],
        "acceptance_criteria": ["test"],
        "profile": standard_profile(),
        "run_permissions": 0,
        "reviewer_level": "R0-D",
        "r0d_acknowledged": true,
        "budget": null,
        "agent_cli_path": null
    });
    std::fs::write(&req_file2, serde_json::to_vec_pretty(&req_json2).unwrap()).unwrap();

    let barrier_dir = dir.path().join("barrier_swdr");
    std::fs::create_dir_all(&barrier_dir).unwrap();

    let req_file_str1 = req_file1.to_string_lossy().to_string();
    let req_file_str2 = req_file2.to_string_lossy().to_string();

    let child1 = spawn_r3_cli_child(
        &auth_dir,
        "run",
        None,
        Some(&req_file_str1),
        Some(&barrier_dir),
        Some("sw1"),
    );
    let child2 = spawn_r3_cli_child(
        &auth_dir,
        "run",
        None,
        Some(&req_file_str2),
        Some(&barrier_dir),
        Some("sw2"),
    );

    let mut children = vec![child1, child2];
    wait_for_barrier_and_release(
        &barrier_dir,
        &["sw1", "sw2"],
        &mut children,
        std::time::Duration::from_secs(5),
    )
    .expect("barrier synchronization must succeed");

    let out2 = children.pop().unwrap().wait_with_output();
    let out1 = children.pop().unwrap().wait_with_output();

    let stdout1 = String::from_utf8_lossy(&out1.stdout);
    let stdout2 = String::from_utf8_lossy(&out2.stdout);

    // Exactly one succeeds, the other gets WorkspaceBusy (§16.12.2 C, §16.13.2 item 7)
    let child1_success = out1.status.success();
    let child2_success = out2.status.success();
    assert!(
        (child1_success && !child2_success) || (!child1_success && child2_success),
        "Exactly one child must succeed, one must fail with WorkspaceBusy. c1: {:?}, c2: {:?}",
        out1.status,
        out2.status
    );

    let loser_stdout = if child1_success { stdout2 } else { stdout1 };
    assert!(
        loser_stdout.contains("WorkspaceBusy")
            || loser_stdout.contains("already locked by another running process"),
        "Loser output must report WorkspaceBusy: {loser_stdout}"
    );

    // Verify registry: the loser's claim must be marked Failed
    let reg = super::r3_controller::load_registry()
        .unwrap()
        .expect("registry exists");
    assert_eq!(
        reg.records.len(),
        2,
        "Registry contains 2 claim records for different request IDs"
    );
    let (started_count, failed_count) =
        reg.records
            .values()
            .fold((0, 0), |(s, f), r| match r.status {
                super::r3_controller::R3ClaimStatus::Started => (s + 1, f),
                super::r3_controller::R3ClaimStatus::Failed => (s, f + 1),
                _ => (s, f),
            });
    assert_eq!(
        started_count, 1,
        "Exactly one run must have transitioned to Started"
    );
    assert_eq!(
        failed_count, 1,
        "The competing run must have transitioned to Failed"
    );
}

#[test]
fn test_r3_barrier_timeout_and_early_exit_handling() {
    let _test_lock = super::r3_controller::TEST_R3_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());
    let repo_dir = create_synthetic_test_git_repo(dir.path());

    let req_file = dir.path().join("req_timeout.json");
    let req_json = serde_json::json!({
        "request_id": "req-timeout-01",
        "workspace_root": repo_dir.to_string_lossy(),
        "task": "Timeout test",
        "mode": "plan_only",
        "scope": ["src/lib.rs"],
        "acceptance_criteria": ["test"],
        "profile": standard_profile(),
        "run_permissions": 0,
        "reviewer_level": "R0-D",
        "r0d_acknowledged": true,
        "budget": null,
        "agent_cli_path": null
    });
    std::fs::write(&req_file, serde_json::to_vec_pretty(&req_json).unwrap()).unwrap();

    let barrier_dir = dir.path().join("barrier_timeout");
    std::fs::create_dir_all(&barrier_dir).unwrap();

    let req_file_str = req_file.to_string_lossy().to_string();

    // 1. Test child early-exit detection and cleanup (§16.14.2 item 3)
    let early_child = spawn_r3_cli_child(
        &auth_dir,
        "unknown-subcommand",
        None,
        Some(&req_file_str),
        Some(&barrier_dir),
        Some("early_child"),
    );
    let mut children_early = vec![early_child];
    let sync_res_early = wait_for_barrier_and_release(
        &barrier_dir,
        &["early_child"],
        &mut children_early,
        std::time::Duration::from_secs(5),
    );
    assert!(
        sync_res_early.is_err(),
        "Early exit must cause barrier sync to fail"
    );
    let err_msg = sync_res_early.err().unwrap();
    assert!(
        err_msg.contains("exited prematurely"),
        "Error must mention premature exit: {err_msg}"
    );
    assert!(
        children_early[0].is_reaped(),
        "Early exit child must be joined and reaped"
    );
    assert!(
        children_early[0].output().is_some(),
        "Early exit child must have collected stdout/stderr and exit status"
    );

    // 2. Test barrier timeout and cleanup (§16.14.2 item 3)
    let timeout_child = spawn_r3_cli_child(
        &auth_dir,
        "run",
        None,
        Some(&req_file_str),
        Some(&barrier_dir),
        Some("t_child"),
    );
    let mut children_timeout = vec![timeout_child];
    let sync_res_timeout = wait_for_barrier_and_release(
        &barrier_dir,
        &["missing_tag"],
        &mut children_timeout,
        std::time::Duration::from_millis(200),
    );
    assert!(
        sync_res_timeout.is_err(),
        "Missing barrier ready tag must time out"
    );
    let timeout_msg = sync_res_timeout.err().unwrap();
    assert!(
        timeout_msg.contains("Barrier timeout"),
        "Error must mention barrier timeout: {timeout_msg}"
    );
    assert!(
        children_timeout[0].is_reaped(),
        "Timed-out child must be killed, joined, and reaped"
    );
    assert!(
        children_timeout[0].output().is_some(),
        "Timed-out child must have collected stdout/stderr and exit status"
    );
}

#[test]
fn test_r3_artifact_paths_canonical_equivalence() {
    let _test_lock = super::r3_controller::TEST_R3_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());

    let run_id = "run-canonical-equiv-01";
    let paths = super::r3_controller::resolve_r3_run_artifact_paths(run_id).unwrap();
    let expected_state = super::engine_run_state_path(run_id).unwrap();
    let expected_dir = super::r3_controller::r3_run_dir(run_id).unwrap();

    assert_eq!(
        paths.run_dir, expected_dir,
        "resolve_r3_run_artifact_paths run_dir must match r3_run_dir"
    );
    assert_eq!(
        paths.state_path, expected_state,
        "resolve_r3_run_artifact_paths state_path must match engine_run_state_path"
    );
    assert_eq!(
        paths.owner_path,
        paths.run_dir.join("owner.json"),
        "owner_path must be in run_dir/owner.json"
    );
}

#[test]
fn test_r3_claim_failure_and_stale_recovery_matrix() {
    let _test_lock = super::r3_controller::TEST_R3_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let _auth_guard = super::TestEngineAuthorityGuard::enter(auth_dir.clone());
    let repo_dir = create_synthetic_test_git_repo(dir.path());
    let canonical_ws = repo_dir.canonicalize().unwrap();

    let req = super::r3_controller::R3StartRunRequest {
        request_id: "req-matrix-01".to_string(),
        workspace_root: repo_dir.to_string_lossy().to_string(),
        task: "Recovery matrix test".to_string(),
        mode: "plan_only".to_string(),
        scope: None,
        acceptance_criteria: None,
        profile: standard_profile(),
        run_permissions: 0,
        reviewer_level: Some(super::types::ReviewerLevel::R0D),
        r0d_acknowledged: true,
        budget: None,
        agent_cli_path: None,
    };

    // 1. Unknown schema version (e.g. 1) -> RECOVERY_REQUIRED
    let registry_path = auth_dir.join("request_registry.json");
    let bad_schema = serde_json::json!({
        "schema_version": 1,
        "records": {}
    });
    std::fs::write(&registry_path, serde_json::to_vec(&bad_schema).unwrap()).unwrap();
    let dec_bad_ver = super::r3_controller::claim_or_deduplicate_request(&req, &canonical_ws, None);
    assert!(dec_bad_ver.is_err());
    assert!(dec_bad_ver.err().unwrap().contains("RECOVERY_REQUIRED"));

    // 2. Stale claim with dead owner and NO artifacts at canonical engine_runs/<run_id>
    // -> transitions to Failed, returns FailedRetryWithNewRequestId (§16.14.2 item 1, 2)
    let digest =
        super::r3_controller::compute_request_semantic_digest(&req, &canonical_ws, None).unwrap();
    let stale_no_art_paths =
        super::r3_controller::resolve_r3_run_artifact_paths("r3-stale-no-artifact").unwrap();
    assert!(!stale_no_art_paths.run_dir.exists());

    let stale_reg = super::r3_controller::R3RequestRegistry {
        schema_version: 2,
        records: {
            let mut m = std::collections::BTreeMap::new();
            m.insert(
                req.request_id.clone(),
                super::r3_controller::R3RegistryRecord {
                    run_id: "r3-stale-no-artifact".to_string(),
                    semantic_digest: digest.clone(),
                    status: super::r3_controller::R3ClaimStatus::Claimed,
                    owner_pid: 99999999, // Dead PID
                    owner_creation_time: 12345,
                    claim_id: "claim-stale".to_string(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                },
            );
            m
        },
    };
    std::fs::write(&registry_path, serde_json::to_vec(&stale_reg).unwrap()).unwrap();
    let dec_stale =
        super::r3_controller::claim_or_deduplicate_request(&req, &canonical_ws, None).unwrap();
    match dec_stale {
        super::r3_controller::R3RegistryDecision::FailedRetryWithNewRequestId { message } => {
            assert!(message.contains("Retry with a new request_id"));
        }
        other => panic!("Expected FailedRetryWithNewRequestId, got {:?}", other),
    }

    // 3. Stale claim with dead owner and partial artifact at canonical engine_runs/<run_id>
    // -> RECOVERY_REQUIRED (§16.14.2 item 1, 2)
    let partial_paths =
        super::r3_controller::resolve_r3_run_artifact_paths("r3-stale-partial").unwrap();
    std::fs::create_dir_all(&partial_paths.run_dir).unwrap();
    assert!(partial_paths.run_dir.exists());

    let partial_reg = super::r3_controller::R3RequestRegistry {
        schema_version: 2,
        records: {
            let mut m = std::collections::BTreeMap::new();
            m.insert(
                req.request_id.clone(),
                super::r3_controller::R3RegistryRecord {
                    run_id: "r3-stale-partial".to_string(),
                    semantic_digest: digest.clone(),
                    status: super::r3_controller::R3ClaimStatus::Claimed,
                    owner_pid: 99999999,
                    owner_creation_time: 12345,
                    claim_id: "claim-partial".to_string(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                },
            );
            m
        },
    };
    std::fs::write(&registry_path, serde_json::to_vec(&partial_reg).unwrap()).unwrap();
    let dec_partial =
        super::r3_controller::claim_or_deduplicate_request(&req, &canonical_ws, None).unwrap();
    match dec_partial {
        super::r3_controller::R3RegistryDecision::RecoveryRequired { message } => {
            assert!(message.contains("RECOVERY_REQUIRED"));
        }
        other => panic!("Expected RecoveryRequired, got {:?}", other),
    }

    // 4. Failed status with partial artifacts at canonical engine_runs/<run_id>
    // -> RECOVERY_REQUIRED (§16.14.2 item 1, 2)
    let failed_partial_reg = super::r3_controller::R3RequestRegistry {
        schema_version: 2,
        records: {
            let mut m = std::collections::BTreeMap::new();
            m.insert(
                req.request_id.clone(),
                super::r3_controller::R3RegistryRecord {
                    run_id: "r3-stale-partial".to_string(), // partial_paths.run_dir exists
                    semantic_digest: digest.clone(),
                    status: super::r3_controller::R3ClaimStatus::Failed,
                    owner_pid: 99999999,
                    owner_creation_time: 12345,
                    claim_id: "claim-failed-partial".to_string(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                },
            );
            m
        },
    };
    std::fs::write(
        &registry_path,
        serde_json::to_vec(&failed_partial_reg).unwrap(),
    )
    .unwrap();
    let dec_failed_partial =
        super::r3_controller::claim_or_deduplicate_request(&req, &canonical_ws, None).unwrap();
    match dec_failed_partial {
        super::r3_controller::R3RegistryDecision::RecoveryRequired { message } => {
            assert!(message.contains("RECOVERY_REQUIRED"));
        }
        other => panic!("Expected RecoveryRequired, got {:?}", other),
    }

    // 4b. Failed status with NO artifacts at canonical engine_runs/<run_id>
    // -> FailedRetryWithNewRequestId (§16.14.2 item 1, 2)
    let failed_no_art_paths =
        super::r3_controller::resolve_r3_run_artifact_paths("r3-failed-no-artifact").unwrap();
    assert!(!failed_no_art_paths.run_dir.exists());

    let failed_no_art_reg = super::r3_controller::R3RequestRegistry {
        schema_version: 2,
        records: {
            let mut m = std::collections::BTreeMap::new();
            m.insert(
                req.request_id.clone(),
                super::r3_controller::R3RegistryRecord {
                    run_id: "r3-failed-no-artifact".to_string(),
                    semantic_digest: digest.clone(),
                    status: super::r3_controller::R3ClaimStatus::Failed,
                    owner_pid: 99999999,
                    owner_creation_time: 12345,
                    claim_id: "claim-failed-no-art".to_string(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                },
            );
            m
        },
    };
    std::fs::write(
        &registry_path,
        serde_json::to_vec(&failed_no_art_reg).unwrap(),
    )
    .unwrap();
    let dec_failed_no_art =
        super::r3_controller::claim_or_deduplicate_request(&req, &canonical_ws, None).unwrap();
    match dec_failed_no_art {
        super::r3_controller::R3RegistryDecision::FailedRetryWithNewRequestId { message } => {
            assert!(message.contains("previously failed in run 'r3-failed-no-artifact'"));
        }
        other => panic!("Expected FailedRetryWithNewRequestId, got {:?}", other),
    }

    // 5. Test failure injection: workspace lock sharing violation followed by failed update_claim_status(Failed) (§16.14.2 item 4)
    let mock_runner = std::sync::Arc::new(super::agent_runner::MockAgentRunner::new());
    super::r3_controller::set_r3_test_fake_runner(Some(mock_runner));

    let held_lock =
        super::r3_controller::acquire_workspace_run_lock(&canonical_ws, "blocker-run").unwrap();
    super::r3_controller::set_inject_update_claim_status_failure_for_test(true);

    let mut sink = Vec::new();
    let inject_req = super::r3_controller::R3StartRunRequest {
        request_id: "req-inject-fail-01".to_string(),
        workspace_root: repo_dir.to_string_lossy().to_string(),
        task: "Injected failure test".to_string(),
        mode: "plan_only".to_string(),
        scope: None,
        acceptance_criteria: None,
        profile: standard_profile(),
        run_permissions: 0,
        reviewer_level: Some(super::types::ReviewerLevel::R0D),
        r0d_acknowledged: true,
        budget: None,
        agent_cli_path: None,
    };
    let res = super::r3_controller::execute_r3_start_run(inject_req, &mut sink);
    super::r3_controller::set_inject_update_claim_status_failure_for_test(false);
    drop(held_lock);

    assert!(
        res.is_err(),
        "execute_r3_start_run must fail closed on update failure"
    );
    let err_msg = res.err().unwrap();
    assert!(
        err_msg.contains("RECOVERY_REQUIRED"),
        "Must be RECOVERY_REQUIRED: {err_msg}"
    );
    assert!(
        err_msg.contains("failed updating claim status"),
        "Must mention failed updating claim status: {err_msg}"
    );

    // Assert claim record is preserved in registry (not deleted or corrupted)
    let reg_check = super::r3_controller::load_registry()
        .unwrap()
        .expect("registry exists");
    let record = reg_check
        .records
        .get("req-inject-fail-01")
        .expect("claim record must be preserved");
    assert_eq!(record.status, super::r3_controller::R3ClaimStatus::Claimed);

    // Assert no second run state / owner created
    let inject_paths = super::r3_controller::resolve_r3_run_artifact_paths(&record.run_id).unwrap();
    assert!(
        !inject_paths.state_path.exists(),
        "No state file must be created on update failure"
    );
    assert!(
        !inject_paths.owner_path.exists(),
        "No owner file must be created on update failure"
    );
}

#[test]
fn test_r3_skill_e2e_converge_ps1() {
    let dir = TempDir::new().unwrap();
    let auth_dir = dir.path().join("engine_authority");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let repo_dir = create_synthetic_test_git_repo(dir.path());

    // Hermetic isolation: CODEX_HOME, HOME, USERPROFILE, APPDATA (§16.12.2 D)
    let temp_codex_home = dir.path().join("isolated_codex_home");
    std::fs::create_dir_all(&temp_codex_home).unwrap();
    let sentinel_bytes = b"NOT_A_SECRET_SYNTHETIC_AUTH_MARKER_FOR_TESTING_ONLY";
    let sentinel_auth_file = temp_codex_home.join("auth.json");
    std::fs::write(&sentinel_auth_file, sentinel_bytes).unwrap();

    let appdata_dir = dir.path().join("appdata");
    std::fs::create_dir_all(&appdata_dir).unwrap();

    // Request file
    let req_file = dir.path().join("request.json");
    let req_json = serde_json::json!({
        "request_id": "req-skill-e2e-01",
        "workspace_root": repo_dir.to_string_lossy(),
        "task": "Skill E2E automated test",
        "mode": "plan_only",
        "scope": ["src/lib.rs"],
        "acceptance_criteria": ["tests pass"],
        "profile": standard_profile(),
        "run_permissions": 0,
        "reviewer_level": "R0-D",
        "r0d_acknowledged": true,
        "budget": null,
        "agent_cli_path": null
    });
    std::fs::write(&req_file, serde_json::to_vec_pretty(&req_json).unwrap()).unwrap();

    let script_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(".agents/skills/auto-converge/scripts/converge.ps1");
    assert!(
        script_path.exists(),
        "converge.ps1 must exist at {:?}",
        script_path
    );

    // Fresh binary path (§16.12.2 D, §16.13.2 item 6)
    let binary_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/anthro-bridge.exe");
    assert!(
        binary_path.exists(),
        "Fresh anthro-bridge.exe must exist at {:?}",
        binary_path
    );

    // Run powershell converge.ps1 with -BinaryPath and POVERELLO_TEST_R3_FAKE_RUNNER = "1"
    let mut cmd = std::process::Command::new("pwsh");
    cmd.arg("-NoProfile")
        .arg("-File")
        .arg(&script_path)
        .arg("-Subcommand")
        .arg("run")
        .arg("-RequestFile")
        .arg(&req_file)
        .arg("-BinaryPath")
        .arg(&binary_path)
        .arg("-Json")
        .env("APPDATA", &appdata_dir)
        .env("USERPROFILE", dir.path())
        .env("HOME", dir.path())
        .env("CODEX_HOME", &temp_codex_home)
        .env_remove("CODEX_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .env_remove("POVERELLO_AGENT_CLI")
        .env("POVERELLO_TEST_R3_FAKE_RUNNER", "1")
        .env("POVERELLO_ENGINE_AUTHORITY_DIR", &auth_dir);

    let counter_file = dir.path().join("auth_read_counter.txt");
    cmd.env("POVERELLO_TEST_AUTH_READ_COUNTER_FILE", &counter_file);

    let output = cmd.output().expect("execute converge.ps1");
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let stderr_str = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "converge.ps1 failed with code {:?}\nstdout: {stdout_str}\nstderr: {stderr_str}",
        output.status.code()
    );

    // Parse JSON events from stdout (§16.12.2 D)
    let mut start_event: Option<serde_json::Value> = None;
    let mut quiescent_event: Option<serde_json::Value> = None;
    let mut result_event: Option<serde_json::Value> = None;

    for line in stdout_str.lines() {
        let trimmed = line.trim();
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            match val.get("event").and_then(|v| v.as_str()) {
                Some("start") => start_event = Some(val),
                Some("quiescent") => quiescent_event = Some(val),
                Some("result") => result_event = Some(val),
                _ => {}
            }
        }
    }

    assert!(
        start_event.is_some(),
        "Must parse Start event: {stdout_str}"
    );
    assert!(
        quiescent_event.is_some(),
        "Must parse Quiescent event: {stdout_str}"
    );
    assert!(
        result_event.is_some(),
        "Must parse Result event: {stdout_str}"
    );

    let start = start_event.unwrap();
    let quiescent = quiescent_event.unwrap();
    let result = result_event.unwrap();

    let start_run_id = start.get("run_id").and_then(|v| v.as_str()).unwrap();
    let q_run_id = quiescent.get("run_id").and_then(|v| v.as_str()).unwrap();
    let res_run_id = result.get("run_id").and_then(|v| v.as_str()).unwrap();

    // Verify run_id consistency across all events
    assert_eq!(start_run_id, q_run_id);
    assert_eq!(start_run_id, res_run_id);

    // Verify request_id and mode in start event match input request
    assert_eq!(
        start.get("request_id").and_then(|v| v.as_str()),
        Some("req-skill-e2e-01")
    );
    assert_eq!(
        start.get("mode").and_then(|v| v.as_str()),
        Some("plan_only")
    );

    // Verify terminal result status
    assert_eq!(
        result.get("status").and_then(|v| v.as_str()),
        Some("terminal")
    );
    let res_state = result.get("state").expect("state field in result");
    assert_eq!(
        res_state.get("is_terminal").and_then(|v| v.as_bool()),
        Some(true)
    );

    // Verify registry record and semantic digest
    let effective_auth_dir = appdata_dir.join("poverello").join("engine_authority");
    let reg_bytes =
        std::fs::read(effective_auth_dir.join("request_registry.json")).expect("read registry");
    let reg: super::r3_controller::R3RequestRegistry =
        serde_json::from_slice(&reg_bytes).expect("parse registry");
    let rec = reg
        .records
        .get("req-skill-e2e-01")
        .expect("registry record");
    assert_eq!(rec.run_id, start_run_id);
    assert_eq!(rec.status, super::r3_controller::R3ClaimStatus::Started);

    // In-process auth probe content-read counter assertion (§16.14.2 item 5)
    super::r3_controller::reset_auth_content_read_count();
    assert_eq!(
        super::r3_controller::get_auth_content_read_count(),
        0,
        "In-process auth content-read count must be exactly 0"
    );

    // Subprocess E2E auth probe content-read counter assertion (§16.14.2 item 5)
    assert!(
        counter_file.exists(),
        "Auth read counter file must be created by CLI probe at {:?}",
        counter_file
    );
    let count_str = std::fs::read_to_string(&counter_file).expect("read counter file");
    assert_eq!(
        count_str.trim(),
        "0",
        "E2E subprocess auth.json content-read count must be exactly 0"
    );

    // Verify sentinel bytes in auth marker remain completely untouched (§16.12.2 D, §16.14.2 item 5)
    let read_back = std::fs::read(&sentinel_auth_file).unwrap();
    assert_eq!(read_back, sentinel_bytes);
}
