use super::finding_tracker::update_tracked_findings;
use super::git_runner::{
    compute_baseline_dirty_hash, compute_baseline_workspace_digest, compute_diff_digest,
    CommandRunner,
};
use super::guards::{
    compute_approval_auth_tag, extract_and_validate_allowlist_from_plan,
    has_valid_reusable_impl_approval, seal_candidate_release_authorization,
    validate_and_compute_run_start_permission_binding, validate_corrective_allowlist_expansion,
    validate_verdict_and_binding, verify_run_start_permission_binding,
    verify_touched_files_within_allowlist,
};
use super::release_reviewer::{validate_release_candidate_manifest, ReleaseCandidateManifest};
use super::types::{
    sha256_hex, ApprovalRecord, ApprovalTargetType, AutoConvergeRunConfig, AutoConvergeRunState,
    AutoConvergeState, AutoConvergeWorkspaceState, BaselineFileEntry, BaselineFileStatus,
    BlockedContext, BlockedKind, CodexReviewVerdict, CycleCounts, EscalationPolicy,
    EscalationTriggerReason, ExecutionLogEntry, PlanLineage, ReleaseAuthorizationState, ReviewMode,
    RoleModelConfig, RunDeltaEntry, SessionProvenance, UserConfirmationToken, VerdictDecision,
    VerificationEvidence, PERM_COMMIT, RELEASE_ACTION_MASK, TAG_OR_RELEASE_MASK,
};

pub fn default_role_configs() -> Vec<RoleModelConfig> {
    vec![
        RoleModelConfig {
            role: "Planner".to_string(),
            model: "gpt-6-luna".to_string(),
            reasoning_effort: "high".to_string(),
        },
        RoleModelConfig {
            role: "Plan Reviewer".to_string(),
            model: "gpt-6-luna".to_string(),
            reasoning_effort: "high".to_string(),
        },
        RoleModelConfig {
            role: "Implementer".to_string(),
            model: "gpt-6-luna".to_string(),
            reasoning_effort: "high".to_string(),
        },
        RoleModelConfig {
            role: "Implementation Reviewer".to_string(),
            model: "gpt-6-luna".to_string(),
            reasoning_effort: "high".to_string(),
        },
        RoleModelConfig {
            role: "Release Reviewer".to_string(),
            model: "gpt-6-luna".to_string(),
            reasoning_effort: "high".to_string(),
        },
        RoleModelConfig {
            role: "Escalation Planner".to_string(),
            model: "gpt-6-sol".to_string(),
            reasoning_effort: "high".to_string(),
        },
        RoleModelConfig {
            role: "Escalation Reviewer".to_string(),
            model: "gpt-6-sol".to_string(),
            reasoning_effort: "high".to_string(),
        },
    ]
}

/// Initializes a new `AutoConvergeRunState` at `RunStart` (§6.2 Row 1, §7.2, §9.2, §9.3).
pub fn initialize_run(
    config: AutoConvergeRunConfig,
    git_head_commit: &str,
    baseline_manifest: Vec<BaselineFileEntry>,
    runner: &dyn CommandRunner,
    now_iso: &str,
) -> Result<AutoConvergeRunState, String> {
    // §7.2 Trigger 1: ForcedRunStart under EscalationPolicy::Disabled is rejected fail-closed at RunStart
    if config.force_escalation && config.profile.escalation_policy == EscalationPolicy::Disabled {
        return Err(
            "RunStart rejected fail-closed: force_escalation == true is forbidden when profile.escalation_policy == Disabled"
                .to_string(),
        );
    }

    // Invariant 10 / §9.3: Pre-existing staged work in .git/index blocks Auto-Converge commit at RunStart
    if (config.run_permissions & PERM_COMMIT) != 0 && runner.has_preexisting_staged_work()? {
        return Err(
            "Invariant 10 violation: pre-existing staged work detected in .git/index while commit permission is requested; commit, unstage, or stash index changes first"
                .to_string(),
        );
    }

    let run_id = config
        .run_id
        .unwrap_or_else(|| format!("ac-{}", uuid::Uuid::new_v4()));

    // Validate dual-gate permissions, prerequisite chains, Phase A boundary, and seal Phase 1 RunStartPermissionBinding
    let (effective_permissions, run_start_permission_binding) =
        validate_and_compute_run_start_permission_binding(
            &run_id,
            &config.repo_remote_url,
            &config.target_branch,
            &config.profile,
            config.run_permissions,
            config.release_intent.as_ref(),
            config.delivery_phase,
        )?;

    let baseline_workspace_digest = compute_baseline_workspace_digest(&baseline_manifest);
    let baseline_dirty_hash = compute_baseline_dirty_hash(&baseline_manifest);
    let policy_profile_digest = config.profile.canonical_digest();

    let (review_mode, active_producer_model, active_reviewer_model, escalation_count) =
        if config.force_escalation {
            (
                ReviewMode::Escalated,
                "gpt-6-sol".to_string(),
                "gpt-6-sol".to_string(),
                1,
            )
        } else {
            (
                ReviewMode::Normal,
                "gpt-6-luna".to_string(),
                "gpt-6-luna".to_string(),
                0,
            )
        };

    let mut cycle_counts = CycleCounts::default();
    cycle_counts.escalation_count = escalation_count;

    let mut state = AutoConvergeRunState {
        schema_version: 1,
        state_sequence: 1,
        run_id: run_id.clone(),
        created_at: now_iso.to_string(),
        updated_at: now_iso.to_string(),
        delivery_phase: config.delivery_phase,
        workspace: AutoConvergeWorkspaceState {
            working_dir: config.working_dir,
            git_head_commit: git_head_commit.to_string(),
            branch_name: config.target_branch,
            repo_remote_url: config.repo_remote_url,
            baseline_dirty_hash,
            baseline_file_manifest: baseline_manifest,
            baseline_workspace_digest,
        },
        plan_lineage: PlanLineage::default(),
        runtime_allowlist: Vec::new(),
        allowlist_digest: None,
        current_diff_digest: None,
        current_evidence_manifest_digest: None,
        current_release_manifest_digest: None,
        allowlist_modified_since_last_impl_approval: false,
        profile: config.profile,
        policy_profile_digest,
        run_permissions: config.run_permissions,
        effective_permissions,
        release_authorization: ReleaseAuthorizationState {
            run_start_permission_binding,
            release_intent: config.release_intent,
            candidate_release_authorization: None,
            concrete_release_target: None,
            release_execution_permission_binding: None,
        },
        session_provenance: SessionProvenance {
            session_id: format!("{run_id}-planner-1"),
            session_role: "Planner".to_string(),
            producer_session_id: format!("{run_id}-planner-1"),
        },
        roles: default_role_configs(),
        current_state: AutoConvergeState::PlanCreate,
        is_terminal: false,
        is_paused: false,
        blocked_context: None,
        review_mode,
        active_producer_model,
        active_reviewer_model,
        active_reasoning_effort: "high".to_string(),
        cycle_counts,
        findings: Vec::new(),
        approvals: Vec::new(),
        evidence: Vec::new(),
        completed_release_actions: Vec::new(),
        execution_log: vec![ExecutionLogEntry {
            timestamp: now_iso.to_string(),
            state: AutoConvergeState::PlanCreate,
            actor: "Controller".to_string(),
            message: "Initialized Auto-Converge run and captured baseline_file_manifest"
                .to_string(),
        }],
        configured_agent_cli: config.agent_cli_path,
        engine_auth_tag: String::new(),
    };
    state.engine_auth_tag = super::guards::compute_engine_state_auth_tag(&state)?;
    Ok(state)
}

/// Submits an initial or revised plan during `PlanCreate` or `PlanRevise` (§6.2).
pub fn submit_draft_plan(
    run_state: &mut AutoConvergeRunState,
    producer_session_id: &str,
    plan_markdown: &str,
    case_insensitive_fs: bool,
    now_iso: &str,
) -> Result<(), String> {
    if !matches!(
        run_state.current_state,
        AutoConvergeState::PlanCreate | AutoConvergeState::PlanRevise
    ) {
        return Err(format!(
            "Cannot submit draft plan in state {:?}",
            run_state.current_state
        ));
    }
    if plan_markdown.trim().is_empty() {
        halt_blocked(
            run_state,
            BlockedKind::FatalInvariant,
            "Draft plan cannot be empty",
        );
        return Err("Draft plan cannot be empty".to_string());
    }

    let (allowlist, allowlist_digest) =
        match extract_and_validate_allowlist_from_plan(plan_markdown, case_insensitive_fs) {
            Ok(res) => res,
            Err(e) => {
                halt_blocked(run_state, BlockedKind::FatalInvariant, &e);
                return Err(e);
            }
        };

    // Invariant 10: Check that no file in initial allowlist was dirty at RunStart
    if let Err(e) = validate_corrective_allowlist_expansion(
        &[],
        &allowlist,
        &run_state.workspace.baseline_file_manifest,
        case_insensitive_fs,
    ) {
        halt_blocked(run_state, BlockedKind::UserConfirmationRequired, &e);
        return Err(e);
    }

    // Invariant 6(a): Plan creation/revision invalidates any prior plan/downstream approvals
    apply_stage_dependent_invalidation(run_state, InvalidationEvent::PlanModified);

    let plan_sha = sha256_hex(plan_markdown.as_bytes());
    run_state.plan_lineage.candidate_plan_digest = Some(plan_sha);
    run_state.runtime_allowlist = allowlist;
    run_state.allowlist_digest = Some(allowlist_digest);
    run_state.session_provenance = SessionProvenance {
        session_id: producer_session_id.to_string(),
        session_role: if run_state.review_mode == ReviewMode::Escalated {
            "Escalation Planner".to_string()
        } else {
            "Planner".to_string()
        },
        producer_session_id: producer_session_id.to_string(),
    };

    transition_to(
        run_state,
        AutoConvergeState::PlanReview,
        "Planner",
        "Draft plan saved and candidate_plan_digest recorded",
        now_iso,
    );
    Ok(())
}

/// Freezes the approved plan and verifies `baseline_workspace_digest` (`PlanApproved -> PlanFrozen -> Implement`, §6.2).
pub fn freeze_approved_plan_and_begin_implement(
    run_state: &mut AutoConvergeRunState,
    current_workspace_manifest: &[BaselineFileEntry],
    implementer_session_id: &str,
    now_iso: &str,
) -> Result<(), String> {
    if run_state.current_state != AutoConvergeState::PlanApproved {
        return Err(format!(
            "Cannot enter PlanFrozen from {:?}",
            run_state.current_state
        ));
    }

    let has_valid_plan_approval = run_state
        .approvals
        .iter()
        .any(|a| !a.invalidated && a.binding.target_type == ApprovalTargetType::Plan);
    if !has_valid_plan_approval
        || run_state.plan_lineage.root_plan_digest.is_none()
        || run_state.plan_lineage.active_plan_digest.is_none()
    {
        let msg = "Invariant 3 violation: cannot enter PlanFrozen/Implement without verified Plan APPROVE";
        halt_blocked(run_state, BlockedKind::FatalInvariant, msg);
        return Err(msg.to_string());
    }

    // Recompute baseline digest with §2.4 exclusion rules (.plan/ excluded) and compare against RunStart
    let current_ws_digest = compute_baseline_workspace_digest(current_workspace_manifest);
    if current_ws_digest != run_state.workspace.baseline_workspace_digest {
        let msg = format!(
            "PlanFrozen baseline verification failed: current workspace digest {current_ws_digest} != baseline {}",
            run_state.workspace.baseline_workspace_digest
        );
        halt_blocked(run_state, BlockedKind::FatalInvariant, &msg);
        return Err(msg);
    }

    transition_to(
        run_state,
        AutoConvergeState::PlanFrozen,
        "Controller",
        "Verified baseline_workspace_digest and locked runtime_allowlist",
        now_iso,
    );

    run_state.cycle_counts.impl_review_count = 0;
    run_state.session_provenance = SessionProvenance {
        session_id: implementer_session_id.to_string(),
        session_role: "Implementer".to_string(),
        producer_session_id: implementer_session_id.to_string(),
    };

    transition_to(
        run_state,
        AutoConvergeState::Implement,
        "Controller",
        "Entered Implement state with frozen runtime_allowlist",
        now_iso,
    );
    Ok(())
}

/// Completes an `Implement` or `CorrectiveImplement` step (§6.2, §6.3).
/// Enforces Hard Invariant 11 (`runtime_allowlist`), computes `diff_digest` via `RunDeltaManifest`
/// and `evidence_manifest_digest`, and evaluates `has_valid_reusable_impl_approval`.
pub fn complete_implementation_step(
    run_state: &mut AutoConvergeRunState,
    implementer_session_id: &str,
    touched_files: &[String],
    delta_manifest: &[RunDeltaEntry],
    evidence: Vec<VerificationEvidence>,
    case_insensitive_fs: bool,
    now_iso: &str,
) -> Result<(), String> {
    if !matches!(
        run_state.current_state,
        AutoConvergeState::Implement | AutoConvergeState::CorrectiveImplement
    ) {
        let msg = format!(
            "Invariant 3/4 violation: cannot execute implementation in state {:?}",
            run_state.current_state
        );
        halt_blocked(run_state, BlockedKind::FatalInvariant, &msg);
        return Err(msg);
    }

    // Invariant 11: Strict Runtime File Scope
    if let Err(e) = verify_touched_files_within_allowlist(
        touched_files,
        &run_state.runtime_allowlist,
        case_insensitive_fs,
    ) {
        halt_blocked(run_state, BlockedKind::FatalInvariant, &e);
        return Err(e);
    }

    // Invariant 6(c): Code modifications during Implement / CorrectiveImplement do NOT invalidate Plan Approval!
    if !touched_files.is_empty() {
        run_state.allowlist_modified_since_last_impl_approval = true;
    }

    let diff_digest = compute_diff_digest(delta_manifest);
    let evidence_json = serde_json::to_string(&evidence).unwrap_or_default();
    let evidence_digest = sha256_hex(evidence_json.as_bytes());

    run_state.current_diff_digest = Some(diff_digest);
    run_state.current_evidence_manifest_digest = Some(evidence_digest);
    run_state.evidence = evidence;
    run_state.session_provenance = SessionProvenance {
        session_id: implementer_session_id.to_string(),
        session_role: "Implementer".to_string(),
        producer_session_id: implementer_session_id.to_string(),
    };

    // §6.3 Authoritative Approval Reuse check
    if has_valid_reusable_impl_approval(run_state) {
        transition_to(
            run_state,
            AutoConvergeState::ImplementationApproved,
            "Controller",
            "Reused verified existing implementation_diff APPROVE binding (all 6 digests match)",
            now_iso,
        );
    } else {
        transition_to(
            run_state,
            AutoConvergeState::ImplementationReview,
            "Implementer",
            "Computed RunDeltaManifest diff_digest and evidence_manifest_digest; ready for ImplementationReview",
            now_iso,
        );
    }

    Ok(())
}

/// Submits a corrective plan during `CorrectivePlanCreate` or `CorrectivePlanRevise` (§6.2, §13.2).
pub fn submit_corrective_plan(
    run_state: &mut AutoConvergeRunState,
    producer_session_id: &str,
    corrective_plan_markdown: &str,
    case_insensitive_fs: bool,
    now_iso: &str,
) -> Result<(), String> {
    if !matches!(
        run_state.current_state,
        AutoConvergeState::CorrectivePlanCreate | AutoConvergeState::CorrectivePlanRevise
    ) {
        return Err(format!(
            "Cannot submit corrective plan in state {:?}",
            run_state.current_state
        ));
    }

    if corrective_plan_markdown.trim().is_empty() {
        halt_blocked(
            run_state,
            BlockedKind::FatalInvariant,
            "Corrective plan cannot be empty",
        );
        return Err("Corrective plan cannot be empty".to_string());
    }

    // Invariant 6(d): Clear Phase-2 release authorizations and invalidate implementation/release approvals
    apply_stage_dependent_invalidation(run_state, InvalidationEvent::EnteredCorrectiveLoop);

    let (candidate_allowlist, candidate_allowlist_digest) =
        match extract_and_validate_allowlist_from_plan(
            corrective_plan_markdown,
            case_insensitive_fs,
        ) {
            Ok(res) => res,
            Err(e) => {
                halt_blocked(run_state, BlockedKind::FatalInvariant, &e);
                return Err(e);
            }
        };

    // §13.2 / Invariant 10: Check every newly added path against baseline_file_manifest
    if let Err(e) = validate_corrective_allowlist_expansion(
        &run_state.runtime_allowlist,
        &candidate_allowlist,
        &run_state.workspace.baseline_file_manifest,
        case_insensitive_fs,
    ) {
        halt_blocked(run_state, BlockedKind::UserConfirmationRequired, &e);
        return Err(e);
    }

    let cand_sha = sha256_hex(corrective_plan_markdown.as_bytes());
    run_state.plan_lineage.candidate_plan_digest = Some(cand_sha);
    run_state.allowlist_digest = Some(candidate_allowlist_digest);
    if run_state.current_state == AutoConvergeState::CorrectivePlanCreate {
        run_state.cycle_counts.corrective_review_count = 0;
    }
    run_state.session_provenance = SessionProvenance {
        session_id: producer_session_id.to_string(),
        session_role: if run_state.review_mode == ReviewMode::Escalated {
            "Escalation Planner".to_string()
        } else {
            "Planner".to_string()
        },
        producer_session_id: producer_session_id.to_string(),
    };

    transition_to(
        run_state,
        AutoConvergeState::CorrectivePlanReview,
        "Planner",
        "Corrective plan drafted and candidate_plan_digest recorded",
        now_iso,
    );
    Ok(())
}

/// Processes a Reviewer output across `PlanReview`, `ImplementationReview`, `CorrectivePlanReview`, or `ReleaseReview` (§6.2).
pub fn submit_review_verdict(
    run_state: &mut AutoConvergeRunState,
    reviewer_session_id: &str,
    raw_verdict_json: &str,
    pre_snapshot_hash: &str,
    post_snapshot_hash: &str,
    approved_corrective_allowlist: Option<Vec<String>>,
    release_manifest: Option<&ReleaseCandidateManifest>,
    case_insensitive_fs: bool,
    now_iso: &str,
) -> Result<(), String> {
    let expected_target = match run_state.current_state {
        AutoConvergeState::PlanReview => ApprovalTargetType::Plan,
        AutoConvergeState::ImplementationReview => ApprovalTargetType::ImplementationDiff,
        AutoConvergeState::CorrectivePlanReview => ApprovalTargetType::CorrectivePlan,
        AutoConvergeState::ReleaseReview => ApprovalTargetType::ReleaseCandidate,
        other => {
            return Err(format!(
                "Cannot submit review verdict in non-review state {other:?}"
            ))
        }
    };

    // Hard Invariant 2: 3-tier Reviewer Read-Only Isolation check (NEVER escalates; immediate Blocked)
    if pre_snapshot_hash.is_empty()
        || post_snapshot_hash.is_empty()
        || pre_snapshot_hash != post_snapshot_hash
    {
        let msg = format!(
            "Invariant 2 violation: Reviewer snapshot FilesystemManifestHash discrepancy ({pre_snapshot_hash} != {post_snapshot_hash})"
        );
        halt_blocked(run_state, BlockedKind::FatalInvariant, &msg);
        return Err(msg);
    }

    // Parse verdict JSON (Invariant 8: bounded retries on malformed JSON)
    let verdict: CodexReviewVerdict = match serde_json::from_str(raw_verdict_json) {
        Ok(v) => v,
        Err(e) => {
            return handle_malformed_or_schema_retry(
                run_state,
                &format!("Malformed verdict JSON: {e}"),
            );
        }
    };

    // Validate findings taxonomy first (unknown failure_code is a schema violation counted against max_transport_retries, Test #34)
    for finding in &verdict.findings {
        if let Err(schema_err) =
            super::finding_tracker::normalize_failure_code(&finding.finding_key.failure_code)
        {
            return handle_malformed_or_schema_retry(run_state, &schema_err);
        }
    }

    // Validate Reviewer Independence (Invariant 1) and Stage-Specific Composite Binding (Invariant 5)
    if let Err(err) =
        validate_verdict_and_binding(run_state, expected_target, reviewer_session_id, &verdict)
    {
        halt_blocked(run_state, BlockedKind::FatalInvariant, &err);
        return Err(err);
    }

    // Valid verdict received: reset transport retry counter
    run_state.cycle_counts.transport_retry_count = 0;

    match verdict.verdict {
        VerdictDecision::Blocked => {
            halt_blocked(
                run_state,
                BlockedKind::FatalInvariant,
                &format!("Reviewer returned explicit BLOCKED: {}", verdict.summary),
            );
            Ok(())
        }
        VerdictDecision::Approve => {
            let seq = run_state.state_sequence + 1;
            let auth_tag = compute_approval_auth_tag(
                &run_state.run_id,
                seq,
                reviewer_session_id,
                &run_state.session_provenance.producer_session_id,
                &verdict.approval_binding,
            )?;
            run_state.approvals.push(ApprovalRecord {
                stage_state: run_state.current_state,
                reviewer_session_id: reviewer_session_id.to_string(),
                producer_session_id: run_state.session_provenance.producer_session_id.clone(),
                timestamp: now_iso.to_string(),
                sequence: seq,
                binding: verdict.approval_binding.clone(),
                invalidated: false,
                engine_auth_tag: auth_tag,
            });

            match run_state.current_state {
                AutoConvergeState::PlanReview => {
                    let approved_sha = run_state
                        .plan_lineage
                        .candidate_plan_digest
                        .take()
                        .ok_or_else(|| {
                            "Missing candidate_plan_digest on PlanReview APPROVE".to_string()
                        })?;
                    run_state.plan_lineage.root_plan_digest = Some(approved_sha.clone());
                    run_state.plan_lineage.active_plan_digest = Some(approved_sha.clone());
                    run_state.plan_lineage.approved_plan_chain = vec![approved_sha];
                    run_state.plan_lineage.recompute_chain_digest();

                    if run_state.review_mode == ReviewMode::Escalated {
                        reset_review_mode_to_normal(run_state);
                    }
                    transition_to(
                        run_state,
                        AutoConvergeState::PlanApproved,
                        "Plan Reviewer",
                        "PlanReview APPROVE verified; promoted root_plan_digest and active_plan_digest",
                        now_iso,
                    );
                }
                AutoConvergeState::ImplementationReview => {
                    run_state.allowlist_modified_since_last_impl_approval = false;
                    // §7.3 Cohesive Sol reset: reset to Normal ONLY after ImplementationReview APPROVE
                    if run_state.review_mode == ReviewMode::Escalated {
                        reset_review_mode_to_normal(run_state);
                    }
                    transition_to(
                        run_state,
                        AutoConvergeState::ImplementationApproved,
                        "Implementation Reviewer",
                        "ImplementationReview APPROVE verified",
                        now_iso,
                    );
                }
                AutoConvergeState::CorrectivePlanReview => {
                    let approved_corr_sha = run_state
                        .plan_lineage
                        .candidate_plan_digest
                        .take()
                        .ok_or_else(|| {
                            "Missing candidate_plan_digest on CorrectivePlanReview APPROVE"
                                .to_string()
                        })?;
                    run_state.plan_lineage.active_plan_digest = Some(approved_corr_sha.clone());
                    run_state
                        .plan_lineage
                        .approved_plan_chain
                        .push(approved_corr_sha);
                    run_state.plan_lineage.recompute_chain_digest();

                    if let Some(new_allowlist) = approved_corrective_allowlist {
                        validate_corrective_allowlist_expansion(
                            &run_state.runtime_allowlist,
                            &new_allowlist,
                            &run_state.workspace.baseline_file_manifest,
                            case_insensitive_fs,
                        )?;
                        let (canon, digest) = super::guards::canonicalize_and_hash_allowlist(
                            &new_allowlist,
                            case_insensitive_fs,
                        )?;
                        if Some(digest.as_str()) != run_state.allowlist_digest.as_deref() {
                            let msg = "Corrective allowlist digest does not match approved allowlist_digest";
                            halt_blocked(run_state, BlockedKind::FatalInvariant, msg);
                            return Err(msg.to_string());
                        }
                        run_state.runtime_allowlist = canon;
                    }

                    // Note (§6.2 / §7.3): If review_mode == Escalated, keep Escalated until ImplementationReview passes!
                    transition_to(
                        run_state,
                        AutoConvergeState::CorrectiveImplement,
                        "Plan Reviewer",
                        "CorrectivePlanReview APPROVE verified; promoted active_plan_digest",
                        now_iso,
                    );
                }
                AutoConvergeState::ReleaseReview => {
                    if (run_state.effective_permissions & TAG_OR_RELEASE_MASK) != 0 {
                        let manifest = release_manifest.ok_or_else(|| {
                            "Missing ReleaseCandidateManifest on ReleaseReview APPROVE".to_string()
                        })?;
                        if let Err(e) = seal_candidate_release_authorization(
                            run_state,
                            &manifest.version_tag,
                            &manifest.release_repository,
                        ) {
                            halt_blocked(run_state, BlockedKind::FatalInvariant, &e);
                            return Err(e);
                        }
                    }

                    if (run_state.effective_permissions & RELEASE_ACTION_MASK) != 0 {
                        transition_to(
                            run_state,
                            AutoConvergeState::ReleaseActionsPending,
                            "Release Reviewer",
                            "ReleaseReview APPROVE verified; proceeding to ReleaseActionsPending",
                            now_iso,
                        );
                    } else {
                        run_state.is_terminal = true;
                        transition_to(
                            run_state,
                            AutoConvergeState::ReleaseReadyHalted,
                            "Release Reviewer",
                            "ReleaseReview APPROVE verified; halting cleanly with zero release mutations",
                            now_iso,
                        );
                    }
                }
                _ => unreachable!(),
            }
            Ok(())
        }
        VerdictDecision::RequestChanges => {
            // Invariant 7: REQUEST_CHANGES is never treated as APPROVE
            let threshold = run_state.profile.convergence_limits.same_blocker_threshold;
            let (updated_findings, same_blocker_triggered) = update_tracked_findings(
                &run_state.findings,
                &verdict.findings,
                threshold,
                case_insensitive_fs,
            )?;
            run_state.findings = updated_findings;

            let immediate_trigger =
                if verdict.architectural_defect || verdict.escalation_recommended {
                    Some(EscalationTriggerReason::ArchitecturalDefectOrRecommended)
                } else if same_blocker_triggered {
                    Some(EscalationTriggerReason::SameBlockerPersistence)
                } else {
                    None
                };

            if run_state.current_state == AutoConvergeState::ReleaseReview {
                run_state.cycle_counts.release_review_count += 1;
                if run_state.cycle_counts.release_review_count
                    >= run_state.profile.convergence_limits.max_release_reviews
                {
                    halt_blocked(
                        run_state,
                        BlockedKind::BudgetExhausted,
                        "ReleaseReview budget exhausted",
                    );
                } else {
                    apply_stage_dependent_invalidation(
                        run_state,
                        InvalidationEvent::EnteredCorrectiveLoop,
                    );
                    transition_to(
                        run_state,
                        AutoConvergeState::CorrectivePlanCreate,
                        "Release Reviewer",
                        "ReleaseReview requested changes; entering CorrectivePlanCreate",
                        now_iso,
                    );
                }
                return Ok(());
            }

            // Handle Escalated (Sol) budget vs Normal (Luna) budget (§6.2, §7.3)
            if run_state.review_mode == ReviewMode::Escalated {
                run_state.cycle_counts.escalation_review_count += 1;
                if run_state.cycle_counts.escalation_review_count
                    >= run_state.profile.convergence_limits.max_escalation_reviews
                {
                    halt_blocked(
                        run_state,
                        BlockedKind::BudgetExhausted,
                        "Sol escalation review budget exhausted (2 consecutive Sol non-APPROVE)",
                    );
                    return Ok(());
                }
                let next_state = match run_state.current_state {
                    AutoConvergeState::PlanReview => AutoConvergeState::PlanRevise,
                    AutoConvergeState::ImplementationReview => {
                        AutoConvergeState::CorrectivePlanCreate
                    }
                    AutoConvergeState::CorrectivePlanReview => {
                        AutoConvergeState::CorrectivePlanRevise
                    }
                    _ => unreachable!(),
                };
                transition_to(
                    run_state,
                    next_state,
                    "Escalation Reviewer",
                    "Sol REQUEST_CHANGES within Sol review budget",
                    now_iso,
                );
                return Ok(());
            }

            // Normal (Luna) mode: increment stage counter & check triggers
            let (new_count, max_stage_reviews, normal_next_state) = match run_state.current_state {
                AutoConvergeState::PlanReview => {
                    run_state.cycle_counts.plan_review_count += 1;
                    (
                        run_state.cycle_counts.plan_review_count,
                        run_state.profile.convergence_limits.max_plan_reviews,
                        AutoConvergeState::PlanRevise,
                    )
                }
                AutoConvergeState::ImplementationReview => {
                    run_state.cycle_counts.impl_review_count += 1;
                    (
                        run_state.cycle_counts.impl_review_count,
                        run_state.profile.convergence_limits.max_impl_reviews,
                        AutoConvergeState::CorrectivePlanCreate,
                    )
                }
                AutoConvergeState::CorrectivePlanReview => {
                    run_state.cycle_counts.corrective_review_count += 1;
                    (
                        run_state.cycle_counts.corrective_review_count,
                        run_state.profile.convergence_limits.max_corrective_reviews,
                        AutoConvergeState::CorrectivePlanRevise,
                    )
                }
                _ => unreachable!(),
            };

            if let Some(reason) = immediate_trigger {
                enter_escalation(run_state, reason, now_iso);
            } else if new_count >= max_stage_reviews {
                enter_escalation(
                    run_state,
                    EscalationTriggerReason::StageReviewBudgetExhausted,
                    now_iso,
                );
            } else {
                transition_to(
                    run_state,
                    normal_next_state,
                    "Reviewer",
                    "REQUEST_CHANGES within normal stage budget",
                    now_iso,
                );
            }
            Ok(())
        }
    }
}

/// Canonical `enter_escalation()` operation (§7.3).
pub fn enter_escalation(
    run_state: &mut AutoConvergeRunState,
    trigger_reason: EscalationTriggerReason,
    now_iso: &str,
) {
    let policy_allows = match (run_state.profile.escalation_policy, trigger_reason) {
        (EscalationPolicy::Enabled, _) => true,
        (
            EscalationPolicy::BlockingOnly,
            EscalationTriggerReason::ArchitecturalDefectOrRecommended,
        ) => true,
        (EscalationPolicy::BlockingOnly, EscalationTriggerReason::SameBlockerPersistence) => true,
        (EscalationPolicy::BlockingOnly, EscalationTriggerReason::ForcedRunStart) => true,
        (EscalationPolicy::BlockingOnly, EscalationTriggerReason::StageReviewBudgetExhausted) => {
            false
        }
        (EscalationPolicy::Disabled, _) => false,
    };

    if !policy_allows
        || run_state.cycle_counts.escalation_count
            >= run_state.profile.convergence_limits.max_escalations
    {
        halt_blocked(
            run_state,
            BlockedKind::BudgetExhausted,
            &format!(
                "Escalation rejected or exhausted (policy={:?}, trigger={:?}, escalation_count={}/{})",
                run_state.profile.escalation_policy,
                trigger_reason,
                run_state.cycle_counts.escalation_count,
                run_state.profile.convergence_limits.max_escalations
            ),
        );
        return;
    }

    run_state.cycle_counts.escalation_count += 1;
    run_state.review_mode = ReviewMode::Escalated;
    run_state.cycle_counts.escalation_review_count = 0;
    run_state.active_producer_model = "gpt-6-sol".to_string();
    run_state.active_reviewer_model = "gpt-6-sol".to_string();
    run_state.active_reasoning_effort = "high".to_string();

    let next_state = match run_state.current_state {
        AutoConvergeState::PlanReview => AutoConvergeState::PlanRevise,
        AutoConvergeState::ImplementationReview => AutoConvergeState::CorrectivePlanCreate,
        AutoConvergeState::CorrectivePlanReview => AutoConvergeState::CorrectivePlanRevise,
        other => other,
    };

    transition_to(
        run_state,
        next_state,
        "Controller",
        &format!("Promoted session to GPT-6 Sol (High) via enter_escalation({trigger_reason:?})"),
        now_iso,
    );
}

/// Advances from `ImplementationApproved` to `VerificationRequired` or `ReadyForReleaseReview` (§6.2).
pub fn advance_from_implementation_approved(
    run_state: &mut AutoConvergeRunState,
    require_environment_verification: bool,
    now_iso: &str,
) -> Result<(), String> {
    if run_state.current_state != AutoConvergeState::ImplementationApproved {
        return Err(format!(
            "Expected ImplementationApproved, found {:?}",
            run_state.current_state
        ));
    }

    if require_environment_verification {
        transition_to(
            run_state,
            AutoConvergeState::VerificationRequired,
            "Controller",
            "Environment verification checks required before ReadyForReleaseReview",
            now_iso,
        );
    } else {
        transition_to(
            run_state,
            AutoConvergeState::ReadyForReleaseReview,
            "Controller",
            "No extra verification required; advancing to ReadyForReleaseReview",
            now_iso,
        );
    }
    Ok(())
}

/// Records the outcome of `VerificationRequired` checks (§6.2).
pub fn record_verification_outcome(
    run_state: &mut AutoConvergeRunState,
    evidence: Vec<VerificationEvidence>,
    now_iso: &str,
) -> Result<(), String> {
    if run_state.current_state != AutoConvergeState::VerificationRequired {
        return Err(format!(
            "Expected VerificationRequired, found {:?}",
            run_state.current_state
        ));
    }

    let all_passed = !evidence.is_empty() && evidence.iter().all(|e| e.passed && e.exit_code == 0);
    let evidence_json = serde_json::to_string(&evidence).unwrap_or_default();
    run_state.current_evidence_manifest_digest = Some(sha256_hex(evidence_json.as_bytes()));
    run_state.evidence = evidence;

    if all_passed {
        transition_to(
            run_state,
            AutoConvergeState::ReadyForReleaseReview,
            "Controller",
            "All verification gates passed",
            now_iso,
        );
    } else {
        run_state.cycle_counts.verification_retry_count += 1;
        if run_state.cycle_counts.verification_retry_count
            >= run_state
                .profile
                .convergence_limits
                .max_verification_retries
        {
            halt_blocked(
                run_state,
                BlockedKind::BudgetExhausted,
                "Verification retry budget exhausted",
            );
        } else {
            apply_stage_dependent_invalidation(run_state, InvalidationEvent::EnteredCorrectiveLoop);
            transition_to(
                run_state,
                AutoConvergeState::CorrectivePlanCreate,
                "Controller",
                "Verification gate failed; entering CorrectivePlanCreate",
                now_iso,
            );
        }
    }
    Ok(())
}

/// Evaluates `ReadyForReleaseReview` transition rules (§6.2 Rows for `ReadyForReleaseReview`).
pub fn advance_from_ready_for_release_review(
    run_state: &mut AutoConvergeRunState,
    release_manifest: Option<&ReleaseCandidateManifest>,
    now_iso: &str,
) -> Result<(), String> {
    if run_state.current_state != AutoConvergeState::ReadyForReleaseReview {
        return Err(format!(
            "Expected ReadyForReleaseReview, found {:?}",
            run_state.current_state
        ));
    }

    if run_state.profile.release_readiness_review {
        let manifest = release_manifest.ok_or_else(|| {
            "ReleaseCandidateManifest is required when entering ReleaseReview".to_string()
        })?;
        let digest = validate_release_candidate_manifest(
            manifest,
            run_state.release_authorization.release_intent.as_ref(),
        )?;
        run_state.current_release_manifest_digest = Some(digest);
        transition_to(
            run_state,
            AutoConvergeState::ReleaseReview,
            "Controller",
            "Entering ReleaseReview with validated ReleaseCandidateManifest",
            now_iso,
        );
        return Ok(());
    }

    // If release_readiness_review == false, tag/release actions are strictly forbidden (§6.2, §8.2, §9.2)
    if (run_state.effective_permissions & TAG_OR_RELEASE_MASK) != 0 {
        let msg = "Tag/release actions cannot proceed with release_readiness_review == false";
        halt_blocked(run_state, BlockedKind::FatalInvariant, msg);
        return Err(msg.to_string());
    }

    if (run_state.effective_permissions & RELEASE_ACTION_MASK) != 0 {
        verify_run_start_permission_binding(run_state)?;
        transition_to(
            run_state,
            AutoConvergeState::ReleaseActionsPending,
            "Controller",
            "Proceeding to local/branch ReleaseActionsPending under verified RunStartPermissionBinding",
            now_iso,
        );
    } else {
        run_state.is_terminal = true;
        transition_to(
            run_state,
            AutoConvergeState::ReleaseReadyHalted,
            "Controller",
            "Reached ReleaseReadyHalted (terminal audit freeze; no release mutations authorized)",
            now_iso,
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Stage-Dependent Binding Invalidation (Hard Invariant 6) & Resumption (§10.2)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidationEvent {
    PlanModified,
    PostApprovalCodeModified,
    EnteredCorrectiveLoop,
}

pub fn apply_stage_dependent_invalidation(
    run_state: &mut AutoConvergeRunState,
    event: InvalidationEvent,
) {
    match event {
        InvalidationEvent::PlanModified => {
            for app in &mut run_state.approvals {
                app.invalidated = true;
            }
            run_state.plan_lineage.root_plan_digest = None;
            run_state.plan_lineage.active_plan_digest = None;
            run_state.plan_lineage.approved_plan_chain.clear();
            run_state.plan_lineage.approved_plan_chain_digest = None;
            run_state.release_authorization.clear_phase_two();
        }
        InvalidationEvent::PostApprovalCodeModified | InvalidationEvent::EnteredCorrectiveLoop => {
            for app in &mut run_state.approvals {
                if matches!(
                    app.binding.target_type,
                    ApprovalTargetType::ImplementationDiff | ApprovalTargetType::ReleaseCandidate
                ) {
                    app.invalidated = true;
                }
            }
            run_state.allowlist_modified_since_last_impl_approval = true;
            run_state.release_authorization.clear_phase_two();
        }
    }
}

/// Resumes a paused or `UserConfirmationRequired` blocked run after performing all §10.2 safety checks.
pub fn resume_run(
    run_state: &mut AutoConvergeRunState,
    current_baseline_manifest: &[BaselineFileEntry],
    current_active_plan_digest: Option<&str>,
    current_diff_digest: Option<&str>,
    runner: &dyn CommandRunner,
    user_confirmation: Option<UserConfirmationToken>,
    now_iso: &str,
) -> Result<(), String> {
    verify_run_start_permission_binding(run_state)?;

    // 1. Check BlockedContext (§6.1, §10.2)
    if run_state.current_state == AutoConvergeState::Blocked {
        let ctx = run_state
            .blocked_context
            .clone()
            .ok_or_else(|| "Cannot resume Blocked run without blocked_context".to_string())?;
        match ctx.kind {
            BlockedKind::FatalInvariant | BlockedKind::BudgetExhausted => {
                return Err(format!(
                    "Cannot resume permanently blocked run (kind={:?}: {})",
                    ctx.kind, ctx.reason
                ));
            }
            BlockedKind::UserConfirmationRequired => {
                let token = user_confirmation.as_ref().ok_or_else(|| {
                    "Resuming from UserConfirmationRequired requires an explicit UserConfirmationToken"
                        .to_string()
                })?;
                if token.run_id != run_state.run_id {
                    return Err("UserConfirmationToken run_id mismatch".to_string());
                }
                // Invariant 10: A token alone NEVER overrides pre-existing staged work in .git/index!
                if (run_state.effective_permissions & PERM_COMMIT) != 0
                    && runner.has_preexisting_staged_work()?
                {
                    return Err(
                        "Invariant 10: UserConfirmationToken cannot override pre-existing staged work; .git/index must be clean before resuming"
                            .to_string(),
                    );
                }
                run_state.current_state = ctx.pre_blocked_state;
                run_state.is_terminal = false;
                run_state.blocked_context = None;
            }
        }
    }

    // 2. Check unrelated dirty baseline drift (Invariant 10 / §10.2)
    let non_allowlisted_dirty: Vec<BaselineFileEntry> = current_baseline_manifest
        .iter()
        .filter(|e| {
            !run_state.runtime_allowlist.contains(&e.path) && e.status != BaselineFileStatus::Clean
        })
        .cloned()
        .collect();
    let expected_non_allowlisted_dirty: Vec<BaselineFileEntry> = run_state
        .workspace
        .baseline_file_manifest
        .iter()
        .filter(|e| {
            !run_state.runtime_allowlist.contains(&e.path) && e.status != BaselineFileStatus::Clean
        })
        .cloned()
        .collect();
    if compute_baseline_dirty_hash(&non_allowlisted_dirty)
        != compute_baseline_dirty_hash(&expected_non_allowlisted_dirty)
        && user_confirmation.is_none()
    {
        halt_blocked(
            run_state,
            BlockedKind::UserConfirmationRequired,
            "Unrelated working tree dirty set drifted while paused",
        );
        return Err("Working tree dirty set drifted while paused".to_string());
    }

    // 3. Stage-dependent invalidation if plan or code changed while paused (§10.2 Step 4)
    if let (Some(curr_plan), Some(expected_plan)) = (
        current_active_plan_digest,
        run_state.plan_lineage.active_plan_digest.as_deref(),
    ) {
        if curr_plan != expected_plan {
            apply_stage_dependent_invalidation(run_state, InvalidationEvent::PlanModified);
            run_state.plan_lineage.candidate_plan_digest = Some(curr_plan.to_string());
            run_state.is_paused = false;
            transition_to(
                run_state,
                AutoConvergeState::PlanReview,
                "Controller",
                "Plan modified while paused; invalidated downstream approvals and returned to PlanReview",
                now_iso,
            );
            return Ok(());
        }
    }

    if let (Some(curr_diff), Some(expected_diff)) = (
        current_diff_digest,
        run_state.current_diff_digest.as_deref(),
    ) {
        if curr_diff != expected_diff {
            apply_stage_dependent_invalidation(
                run_state,
                InvalidationEvent::PostApprovalCodeModified,
            );
            run_state.current_diff_digest = Some(curr_diff.to_string());
            run_state.is_paused = false;
            transition_to(
                run_state,
                AutoConvergeState::ImplementationReview,
                "Controller",
                "Code diff modified after approval; invalidated implementation approval and returned to ImplementationReview",
                now_iso,
            );
            return Ok(());
        }
    }

    run_state.is_paused = false;
    run_state.updated_at = now_iso.to_string();
    Ok(())
}

fn handle_malformed_or_schema_retry(
    run_state: &mut AutoConvergeRunState,
    reason: &str,
) -> Result<(), String> {
    run_state.cycle_counts.transport_retry_count += 1;
    if run_state.cycle_counts.transport_retry_count
        > run_state.profile.convergence_limits.max_transport_retries
    {
        halt_blocked(
            run_state,
            BlockedKind::FatalInvariant,
            &format!("Invariant 8: Exhausted max_transport_retries on malformed/schema-invalid output ({reason})"),
        );
    }
    Err(reason.to_string())
}

fn reset_review_mode_to_normal(run_state: &mut AutoConvergeRunState) {
    run_state.review_mode = ReviewMode::Normal;
    run_state.cycle_counts.escalation_review_count = 0;
    run_state.active_producer_model = "gpt-6-luna".to_string();
    run_state.active_reviewer_model = "gpt-6-luna".to_string();
    run_state.active_reasoning_effort = "high".to_string();
}

pub fn halt_blocked(run_state: &mut AutoConvergeRunState, kind: BlockedKind, reason: &str) {
    let pre = run_state.current_state;
    run_state.blocked_context = Some(BlockedContext {
        kind,
        pre_blocked_state: pre,
        reason: reason.to_string(),
    });
    run_state.current_state = AutoConvergeState::Blocked;
    run_state.is_terminal = true;
}

fn transition_to(
    run_state: &mut AutoConvergeRunState,
    next: AutoConvergeState,
    actor: &str,
    message: &str,
    now_iso: &str,
) {
    run_state.current_state = next;
    run_state.is_terminal = next.is_terminal();
    run_state.updated_at = now_iso.to_string();
    run_state.execution_log.push(ExecutionLogEntry {
        timestamp: now_iso.to_string(),
        state: next,
        actor: actor.to_string(),
        message: message.to_string(),
    });
}
