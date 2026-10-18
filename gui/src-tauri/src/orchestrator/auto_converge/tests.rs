use std::collections::BTreeMap;
use std::path::PathBuf;
use tempfile::TempDir;

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

    let dir = tempfile::TempDir::new().unwrap();
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

    let dir = tempfile::TempDir::new().unwrap();
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

    let dir = tempfile::TempDir::new().unwrap();
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
    let dir = tempfile::TempDir::new().unwrap();
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

    let dir = tempfile::TempDir::new().unwrap();
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

    let dir = tempfile::TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let snap = dir.path().join("snapshot");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&snap).unwrap();

    // Script 1: outputs malformed JSON missing required fields
    let bad_script = dir.path().join("bad_verdict.cmd");
    std::fs::write(&bad_script, b"@echo off\r\necho {\"random_key\": 123}\r\n").unwrap();

    let runner = ProductionAgentRunner::with_cli(bad_script);
    let req = RoleExecutionRequest {
        role: AutoConvergeRole::Reviewer,
        session_id: "rev-001".to_string(),
        workspace_root: ws.clone(),
        snapshot_dir: Some(snap.clone()),
        plan_path: ws.join(".plan").join("plan.md"),
        runtime_allowlist: vec!["src/lib.rs".to_string()],
        input_prompt: "Review plan".to_string(),
        timeout_seconds: 5,
    };

    let res = runner.execute_role_session(&req);
    assert!(
        res.is_err(),
        "Malformed verdict must be rejected by schema validation"
    );
    let err = res.err().unwrap();
    assert!(
        err.contains("schema validation for CodexReviewVerdict"),
        "Unexpected error: {err}"
    );

    // Script 2: outputs unclosed/truncated JSON
    let trunc_script = dir.path().join("trunc_verdict.cmd");
    std::fs::write(
        &trunc_script,
        b"@echo off\r\necho {\"verdict\": \"APPROVE\"\r\n",
    )
    .unwrap();
    let trunc_runner = ProductionAgentRunner::with_cli(trunc_script);
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

    let dir = tempfile::TempDir::new().unwrap();
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

    let dir = tempfile::TempDir::new().unwrap();
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

    let dir = tempfile::TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    let plan_dir = ws.join(".plan");
    let src_dir = ws.join("src");
    std::fs::create_dir_all(&plan_dir).unwrap();
    std::fs::create_dir_all(&src_dir).unwrap();

    let target_file = src_dir.join("lib.rs");
    std::fs::write(&target_file, b"pub fn initial() {}\n").unwrap();

    let mut config = default_test_config(fast_profile());
    config.working_dir = ws.to_string_lossy().to_string();
    let run_id = config.run_id.clone().unwrap();
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
                    let target = self.workspace.join("src").join("lib.rs");
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
}

// 52. (§24.4.3, §24.5) test_preflight_missing_plan_fails_closed_with_zero_launches
#[test]
fn test_preflight_missing_plan_fails_closed_with_zero_launches() {
    use super::agent_runner::MockAgentRunner;

    let dir = tempfile::TempDir::new().unwrap();
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
    let dir = tempfile::TempDir::new().unwrap();
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
    let dir = tempfile::TempDir::new().unwrap();
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

    let dir = tempfile::TempDir::new().unwrap();
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
    let dir = tempfile::TempDir::new().unwrap();
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

    let dir = tempfile::TempDir::new().unwrap();
    let ws = dir.path().join("workspace");
    std::fs::create_dir_all(&ws).unwrap();

    let run_id = format!("ac-run-57-{}", uuid::Uuid::new_v4());
    let mut cfg = default_test_config(fast_profile());
    cfg.run_id = Some(run_id);
    cfg.working_dir = ws.to_string_lossy().to_string();
    cfg.agent_cli_path = Some("C:\\fake\\nonexistent_cli.exe".to_string());

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
}

// 58. (§25.3.1, §25.3.2, §25.4) test_plan_review_snapshot_binding_positive_and_tamper_matrix
#[test]
fn test_plan_review_snapshot_binding_positive_and_tamper_matrix() {
    let _lock = super::isolation::TEST_ISOLATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::TempDir::new().unwrap();
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
    let dir = tempfile::TempDir::new().unwrap();
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
    let dir = tempfile::TempDir::new().unwrap();
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
        let dir = tempfile::TempDir::new().unwrap();
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
    let dir = tempfile::TempDir::new().unwrap();
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
    let dir = tempfile::TempDir::new().unwrap();
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

    let dir = tempfile::TempDir::new().unwrap();
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

    let dir = tempfile::TempDir::new().unwrap();
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
