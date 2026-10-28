use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeSet, HashMap};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::{Condvar, Mutex};
use std::thread::ThreadId;

use super::finding_tracker::normalize_failure_code;
use super::profile_store::validate_profile_against_invariants;
use super::types::{
    is_valid_sha256_hex, sha256_hex, ApprovalBinding, ApprovalTargetType, AutoConvergeProfile,
    AutoConvergeRunState, BaselineFileEntry, BaselineFileStatus, CodexReviewVerdict,
    ConcreteReleaseTarget, DeliveryPhase, ReleasePermissionIntent, ReviewReusePolicy,
    VerdictDecision, LOCAL_GIT_MASK, PERM_COMMIT, PERM_PAID_LIVE_API, PERM_PUSH_BRANCH,
    PERM_PUSH_TAG, PERM_RELEASE_PUBLISH, PERM_STAGE, PERM_TAG, RELEASE_ACTION_MASK,
    TAG_OR_RELEASE_MASK,
};

use super::state_store::EngineAuthorityKey;

pub fn engine_key() -> Result<EngineAuthorityKey, String> {
    EngineAuthorityKey::load_or_create()
        .map_err(|e| format!("Failed to access engine authority key: {e}"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AllowlistScopeBlock {
    allowlist: Vec<String>,
}

// ---------------------------------------------------------------------------
// 1. Runtime Run Allowlist Extraction & Validation (§13.2)
// ---------------------------------------------------------------------------

/// Extracts and validates the `json auto-converge-scope` block from a plan markdown document.
/// Preserves real filesystem casing while rejecting `..`, absolute paths, `.git/`, `.poverello/`,
/// and case-only duplicates on case-insensitive filesystems (Test #33).
/// Returns `(canonical_allowlist, allowlist_digest)`.
pub fn extract_and_validate_allowlist_from_plan(
    plan_markdown: &str,
    case_insensitive_fs: bool,
) -> Result<(Vec<String>, String), String> {
    let marker = "```json auto-converge-scope";
    let start_idx = plan_markdown.find(marker).ok_or_else(|| {
        "Plan rejected fail-closed: missing required ```json auto-converge-scope block".to_string()
    })?;
    let after_marker = &plan_markdown[start_idx + marker.len()..];
    let fence_end = after_marker.find("```").ok_or_else(|| {
        "Plan rejected fail-closed: unclosed ```json auto-converge-scope block".to_string()
    })?;
    let json_body = after_marker[..fence_end].trim();
    let parsed: AllowlistScopeBlock = serde_json::from_str(json_body).map_err(|e| {
        format!("Plan rejected fail-closed: invalid JSON in auto-converge-scope block: {e}")
    })?;

    canonicalize_and_hash_allowlist(&parsed.allowlist, case_insensitive_fs)
}

/// Canonicalizes a list of repository-relative paths and computes `allowlist_digest = SHA256(canonical_json)`.
pub fn canonicalize_and_hash_allowlist(
    raw_paths: &[String],
    case_insensitive_fs: bool,
) -> Result<(Vec<String>, String), String> {
    if raw_paths.is_empty() {
        return Err("Allowlist cannot be empty".to_string());
    }

    let mut canonical = Vec::with_capacity(raw_paths.len());
    for raw in raw_paths {
        let norm = normalize_and_validate_repo_rel_path(raw)?;
        canonical.push(norm);
    }

    canonical.sort();
    canonical.dedup();

    if case_insensitive_fs {
        let mut seen_lower = BTreeSet::new();
        for p in &canonical {
            let lower = p.to_ascii_lowercase();
            if !seen_lower.insert(lower) {
                return Err(format!(
                    "Ambiguous case-only duplicate in runtime_allowlist on case-insensitive filesystem: '{p}'"
                ));
            }
        }
    }

    let json = serde_json::to_string(&canonical).unwrap_or_default();
    let digest = sha256_hex(json.as_bytes());
    Ok((canonical, digest))
}

pub fn normalize_and_validate_repo_rel_path(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Allowlist path cannot be empty".to_string());
    }
    if trimmed.starts_with('/')
        || trimmed.starts_with('\\')
        || (trimmed.len() >= 2 && trimmed.as_bytes()[1] == b':')
    {
        return Err(format!(
            "Allowlist path rejected: absolute path is forbidden ('{trimmed}')"
        ));
    }

    let forward = trimmed.replace('\\', "/");
    let mut parts = Vec::new();
    for segment in forward.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            return Err(format!(
                "Allowlist path rejected: parent traversal '..' is forbidden ('{trimmed}')"
            ));
        }
        parts.push(segment);
    }

    if parts.is_empty() {
        return Err(format!(
            "Allowlist path rejected: resolves to empty ('{trimmed}')"
        ));
    }

    let normalized = parts.join("/");
    let lower = normalized.to_ascii_lowercase();
    if lower == ".git"
        || lower.starts_with(".git/")
        || lower == ".poverello"
        || lower.starts_with(".poverello/")
    {
        return Err(format!(
            "Allowlist path rejected: protected repository/engine directory ('{normalized}')"
        ));
    }

    Ok(normalized)
}

/// Validates corrective allowlist expansion (§13.2, Invariant 10, Test #32).
/// Every newly added path beyond `existing_allowlist` must have been `Clean` at `RunStart`
/// according to `baseline_file_manifest`.
pub fn validate_corrective_allowlist_expansion(
    existing_allowlist: &[String],
    candidate_allowlist: &[String],
    baseline_manifest: &[BaselineFileEntry],
    case_insensitive_fs: bool,
) -> Result<(), String> {
    let existing_keys: BTreeSet<String> = existing_allowlist
        .iter()
        .map(|p| {
            if case_insensitive_fs {
                p.to_ascii_lowercase()
            } else {
                p.clone()
            }
        })
        .collect();

    for candidate in candidate_allowlist {
        let key = if case_insensitive_fs {
            candidate.to_ascii_lowercase()
        } else {
            candidate.clone()
        };
        if existing_keys.contains(&key) {
            continue;
        }

        // Newly added path in corrective plan: inspect baseline_file_manifest
        let base_match = baseline_manifest.iter().find(|entry| {
            if case_insensitive_fs {
                entry.path.eq_ignore_ascii_case(candidate)
            } else {
                &entry.path == candidate
            }
        });

        if let Some(entry) = base_match {
            if entry.status != BaselineFileStatus::Clean {
                return Err(format!(
                    "Invariant 10 violation: corrective allowlist expansion cannot add pre-existing dirty/untracked file '{}' (RunStart status: {:?})",
                    candidate, entry.status
                ));
            }
        }
    }

    Ok(())
}

/// Enforces Hard Invariant 11: verifies all touched files are strictly within `runtime_allowlist` (Test #22).
pub fn verify_touched_files_within_allowlist(
    touched_files: &[String],
    runtime_allowlist: &[String],
    case_insensitive_fs: bool,
) -> Result<(), String> {
    let allow_keys: BTreeSet<String> = runtime_allowlist
        .iter()
        .map(|p| {
            if case_insensitive_fs {
                p.to_ascii_lowercase()
            } else {
                p.clone()
            }
        })
        .collect();

    for touched in touched_files {
        let norm = normalize_and_validate_repo_rel_path(touched)?;
        let key = if case_insensitive_fs {
            norm.to_ascii_lowercase()
        } else {
            norm.clone()
        };
        if !allow_keys.contains(&key) {
            return Err(format!(
                "Invariant 11 violation: Implementer modified out-of-scope file '{norm}' not in frozen runtime_allowlist"
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. 3-Tier Reviewer Read-Only Isolation & Platform Preflight (§2.2, §16.4)
// ---------------------------------------------------------------------------

/// Verifies Reviewer CLI invocation options and platform sandbox isolation preflight (§2.2, §16.4).
/// Fails closed if `--sandbox read-only` is missing, if writable flags are present, or if the
/// platform isolation probe fails.
pub fn verify_reviewer_isolation_preflight(
    cli_args: &[String],
    platform_probe_passed: bool,
) -> Result<(), String> {
    if !platform_probe_passed {
        return Err(
            "Reviewer isolation preflight failed: OS sandbox probe did not confirm write denial"
                .to_string(),
        );
    }

    let mut has_readonly_sandbox = false;
    for window in cli_args.windows(2) {
        if window[0] == "--sandbox" && window[1] == "read-only" {
            has_readonly_sandbox = true;
        }
    }
    if !has_readonly_sandbox && !cli_args.iter().any(|a| a == "--sandbox=read-only") {
        return Err(
            "Invariant 2 violation: Reviewer session must be launched with '--sandbox read-only'"
                .to_string(),
        );
    }

    for arg in cli_args {
        let lower = arg.to_ascii_lowercase();
        if lower.contains("workspace-write")
            || lower.contains("danger-full-access")
            || lower == "--allow-write"
        {
            return Err(format!(
                "Invariant 2 violation: forbidden write-enabling flag '{arg}' in Reviewer session"
            ));
        }
    }

    Ok(())
}

/// Computes `FilesystemManifestHash` over all files in a directory (§2.2):
/// `SHA256(sum_{f in sorted_files} rel_path(f) + ":" + file_mode(f) + ":" + SHA256(content(f)))`
pub fn compute_filesystem_manifest_hash(root: &Path) -> Result<String, String> {
    if !root.exists() {
        return Err(format!(
            "Snapshot directory does not exist: {}",
            root.display()
        ));
    }
    let mut records = Vec::new();
    collect_snapshot_manifest_records(root, root, &mut records)?;
    records.sort();
    let joined = records.join("\n");
    Ok(sha256_hex(joined.as_bytes()))
}

fn collect_snapshot_manifest_records(
    root: &Path,
    current: &Path,
    records: &mut Vec<String>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(current)
        .map_err(|e| format!("Failed reading snapshot dir {}: {e}", current.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Snapshot dir entry error: {e}"))?;
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path)
            .map_err(|e| format!("Snapshot metadata error at {}: {e}", path.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!(
                "Security violation: symlink detected inside reviewer snapshot at {}",
                path.display()
            ));
        }
        if meta.is_dir() {
            collect_snapshot_manifest_records(root, &path, records)?;
        } else if meta.is_file() {
            let rel = path
                .strip_prefix(root)
                .map_err(|e| format!("Strip prefix error: {e}"))?
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = std::fs::read(&path)
                .map_err(|e| format!("Failed reading snapshot file {}: {e}", path.display()))?;
            let content_hash = sha256_hex(&bytes);
            let mode = if meta.permissions().readonly() {
                "100444"
            } else {
                "100644"
            };
            records.push(format!("{rel}:{mode}:{content_hash}"));
        }
    }
    Ok(())
}

/// Materializes an isolated Reviewer snapshot in an external OS temporary directory outside the repo (§2.2).
/// Copies only enumerable files: tracked files + `runtime_allowlist` + `.plan/` + evidence manifest paths;
/// arbitrary untracked files outside that scope are never copied.
pub fn materialize_reviewer_snapshot(
    workspace_root: &Path,
    snapshot_dir: &Path,
    tracked_files: &[String],
    runtime_allowlist: &[String],
    untracked_files_in_worktree: &[String],
) -> Result<String, String> {
    let canon_ws = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    if snapshot_dir.starts_with(&canon_ws) || snapshot_dir.starts_with(workspace_root) {
        return Err(
            "Invariant 2/11 violation: Reviewer snapshot must be materialized outside the repository"
                .to_string(),
        );
    }

    if snapshot_dir.exists() {
        std::fs::remove_dir_all(snapshot_dir)
            .map_err(|e| format!("Failed cleaning snapshot dir: {e}"))?;
    }
    std::fs::create_dir_all(snapshot_dir)
        .map_err(|e| format!("Failed creating snapshot dir: {e}"))?;

    let untracked_set: BTreeSet<&str> = untracked_files_in_worktree
        .iter()
        .map(|s| s.as_str())
        .collect();
    let allow_set: BTreeSet<&str> = runtime_allowlist.iter().map(|s| s.as_str()).collect();
    let tracked_set: BTreeSet<&str> = tracked_files.iter().map(|s| s.as_str()).collect();

    copy_enumerable_workspace_to_snapshot(
        workspace_root,
        workspace_root,
        snapshot_dir,
        &tracked_set,
        &allow_set,
        &untracked_set,
    )?;

    compute_filesystem_manifest_hash(snapshot_dir)
}

fn copy_enumerable_workspace_to_snapshot(
    ws_root: &Path,
    current: &Path,
    snap_root: &Path,
    tracked_set: &BTreeSet<&str>,
    allow_set: &BTreeSet<&str>,
    untracked_set: &BTreeSet<&str>,
) -> Result<(), String> {
    for entry in std::fs::read_dir(current)
        .map_err(|e| format!("Failed reading workspace {}: {e}", current.display()))?
    {
        let entry = entry.map_err(|e| format!("Dir entry error: {e}"))?;
        let path = entry.path();
        let rel = path
            .strip_prefix(ws_root)
            .map_err(|e| format!("Strip prefix error: {e}"))?
            .to_string_lossy()
            .replace('\\', "/");

        if rel == ".git"
            || rel.starts_with(".git/")
            || rel == ".poverello"
            || rel.starts_with(".poverello/")
        {
            continue;
        }

        let meta = std::fs::symlink_metadata(&path)
            .map_err(|e| format!("Metadata error at {}: {e}", path.display()))?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            copy_enumerable_workspace_to_snapshot(
                ws_root,
                &path,
                snap_root,
                tracked_set,
                allow_set,
                untracked_set,
            )?;
        } else if meta.is_file() {
            let is_plan = rel.starts_with(".plan/");
            let is_allowlisted = allow_set.contains(rel.as_str());
            let is_tracked = tracked_set.is_empty() || tracked_set.contains(rel.as_str());
            let is_arbitrary_untracked =
                untracked_set.contains(rel.as_str()) && !is_allowlisted && !is_plan;
            if is_arbitrary_untracked || (!is_tracked && !is_allowlisted && !is_plan) {
                continue;
            }

            let dest = snap_root.join(&rel);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed creating snapshot subdir: {e}"))?;
            }
            std::fs::copy(&path, &dest)
                .map_err(|e| format!("Failed copying {rel} to snapshot: {e}"))?;
        }
    }
    Ok(())
}

/// Platform write-boundary enforcement probe (§16.4, §16.6).
/// Verifies whether a candidate write by `role` (`"Reviewer"`, `"Planner"`, `"Implementer"`) to
/// `target_path` is permitted by the OS/sandbox boundary. Rejects symlinks, `..` traversal,
/// engine-state directory writes, and out-of-scope writes.
pub fn enforce_agent_filesystem_write_boundary(
    role: &str,
    target_path: &Path,
    workspace_root: &Path,
    engine_state_dir: &Path,
    runtime_allowlist: &[String],
) -> Result<(), String> {
    // 1. Reject any path with ParentDir (`..`) components
    for comp in target_path.components() {
        if matches!(comp, Component::ParentDir) {
            return Err(format!(
                "OS boundary denial: path traversal '..' forbidden for role '{role}' ({})",
                target_path.display()
            ));
        }
    }

    // 2. Reject symlinks / reparse points on target or existing ancestors
    let mut cursor = Some(target_path);
    while let Some(p) = cursor {
        if let Ok(meta) = std::fs::symlink_metadata(p) {
            if meta.file_type().is_symlink() {
                return Err(format!(
                    "OS boundary denial: symlink write forbidden for role '{role}' ({})",
                    p.display()
                ));
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
                if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                    return Err(format!(
                        "OS boundary denial: reparse point write forbidden for role '{role}' ({})",
                        p.display()
                    ));
                }
            }
        }
        cursor = p.parent();
    }

    // 3. Engine-owned durable state directory is strictly off-limits to ALL agent subprocesses (§16.3)
    if target_path.starts_with(engine_state_dir) {
        return Err(format!(
            "OS boundary denial: agent role '{role}' cannot write engine-owned state at {}",
            target_path.display()
        ));
    }

    // 4. Reviewer roles are universally READ-ONLY across all paths (§2.2, Invariant 2)
    let role_lower = role.to_ascii_lowercase();
    if role_lower.contains("reviewer") {
        return Err(format!(
            "OS boundary denial: Reviewer role '{role}' is strictly read-only; denied write to {}",
            target_path.display()
        ));
    }

    // 5. Must be inside workspace_root
    let rel = target_path
        .strip_prefix(workspace_root)
        .map_err(|_| {
            format!(
                "OS boundary denial: role '{role}' attempted write outside workspace root ({})",
                target_path.display()
            )
        })?
        .to_string_lossy()
        .replace('\\', "/");

    if rel == ".poverello"
        || rel.starts_with(".poverello/")
        || rel == ".git"
        || rel.starts_with(".git/")
    {
        return Err(format!(
            "OS boundary denial: role '{role}' cannot write protected directory '{rel}'"
        ));
    }

    // 6. Planner roles can ONLY write under `.plan/` (§2.3, §16.3)
    if role_lower.contains("planner") {
        if rel == ".plan" || rel.starts_with(".plan/") {
            return Ok(());
        }
        return Err(format!(
            "OS boundary denial: Planner role '{role}' can only write under .plan/, denied '{rel}'"
        ));
    }

    // 7. Implementer roles can ONLY write files in `runtime_allowlist` (§2.3, Invariant 11)
    if role_lower.contains("implementer") {
        verify_touched_files_within_allowlist(&[rel], runtime_allowlist, cfg!(windows))?;
        return Ok(());
    }

    Err(format!("OS boundary denial: unknown agent role '{role}'"))
}

// ---------------------------------------------------------------------------
// 3. Stage-Specific Composite Approval Binding & Review Reuse (§3.2, §4, §6.3)
// ---------------------------------------------------------------------------

/// Validates a `CodexReviewVerdict` against Hard Invariants 1, 5, 7, 8 and stage-specific
/// binding rules (`plan`, `corrective_plan`, `implementation_diff`, `release_candidate`).
pub fn validate_verdict_and_binding(
    run_state: &AutoConvergeRunState,
    expected_target_type: ApprovalTargetType,
    reviewer_session_id: &str,
    verdict: &CodexReviewVerdict,
) -> Result<(), String> {
    // Hard Invariant 1: Reviewer Independence
    let reviewer_trimmed = reviewer_session_id.trim();
    let producer_trimmed = run_state.session_provenance.producer_session_id.trim();
    if reviewer_trimmed.is_empty() || producer_trimmed.is_empty() {
        return Err("Invariant 1 violation: session IDs cannot be empty".to_string());
    }
    if reviewer_trimmed == producer_trimmed {
        return Err(format!(
            "Invariant 1 violation: Reviewer self-approval prevented (reviewer_session_id '{reviewer_trimmed}' == producer_session_id)"
        ));
    }

    // Validate all findings have valid taxonomy failure_code and non-empty fields (Invariant 8 / §5)
    for f in &verdict.findings {
        normalize_failure_code(&f.finding_key.failure_code)?;
        if f.finding_key.file.trim().is_empty()
            || f.location.trim().is_empty()
            || f.failure_condition.trim().is_empty()
            || f.acceptance_condition.trim().is_empty()
        {
            return Err("Schema violation: finding contains empty required fields".to_string());
        }
    }

    let b = &verdict.approval_binding;
    if b.target_type != expected_target_type {
        return Err(format!(
            "Invariant 5 violation: approval_binding.target_type {:?} != expected {:?}",
            b.target_type, expected_target_type
        ));
    }

    if !is_valid_sha256_hex(&b.allowlist_digest) {
        return Err("Invariant 5 violation: invalid allowlist_digest hex".to_string());
    }
    if !is_valid_sha256_hex(&b.policy_profile_digest) {
        return Err("Invariant 5 violation: invalid policy_profile_digest hex".to_string());
    }
    if b.policy_profile_digest != run_state.policy_profile_digest {
        return Err(format!(
            "Invariant 5 violation: policy_profile_digest mismatch ({} != {})",
            b.policy_profile_digest, run_state.policy_profile_digest
        ));
    }

    let expected_allowlist = run_state
        .allowlist_digest
        .as_deref()
        .ok_or_else(|| "Missing expected allowlist_digest in run_state".to_string())?;
    if b.allowlist_digest != expected_allowlist {
        return Err(format!(
            "Invariant 5 violation: allowlist_digest mismatch ({} != {})",
            b.allowlist_digest, expected_allowlist
        ));
    }

    match expected_target_type {
        ApprovalTargetType::Plan => {
            if b.root_plan_digest.is_some() || b.active_plan_digest.is_some() {
                return Err(
                    "Invariant 5 violation: target_type 'plan' requires root_plan_digest == null and active_plan_digest == null prior to promotion"
                        .to_string(),
                );
            }
            let cand = b.candidate_plan_digest.as_deref().ok_or_else(|| {
                "Invariant 5 violation: target_type 'plan' requires non-null candidate_plan_digest"
                    .to_string()
            })?;
            if !is_valid_sha256_hex(cand) {
                return Err("Invalid candidate_plan_digest SHA-256 hex".to_string());
            }
            let expected_cand = run_state
                .plan_lineage
                .candidate_plan_digest
                .as_deref()
                .ok_or_else(|| "Missing candidate_plan_digest in run_state".to_string())?;
            if cand != expected_cand {
                return Err(format!(
                    "Invariant 5 violation: candidate_plan_digest mismatch ({cand} != {expected_cand})"
                ));
            }
            let base_ws = b.baseline_workspace_digest.as_deref().ok_or_else(|| {
                "Invariant 5 violation: target_type 'plan' requires non-null baseline_workspace_digest"
                    .to_string()
            })?;
            if base_ws != run_state.workspace.baseline_workspace_digest {
                return Err(format!(
                    "Invariant 5 violation: baseline_workspace_digest mismatch ({base_ws} != {})",
                    run_state.workspace.baseline_workspace_digest
                ));
            }
        }
        ApprovalTargetType::CorrectivePlan => {
            let root = b.root_plan_digest.as_deref().ok_or_else(|| {
                "Invariant 5 violation: target_type 'corrective_plan' requires non-null root_plan_digest"
                    .to_string()
            })?;
            let active = b.active_plan_digest.as_deref().ok_or_else(|| {
                "Invariant 5 violation: target_type 'corrective_plan' requires non-null active_plan_digest"
                    .to_string()
            })?;
            let cand = b.candidate_plan_digest.as_deref().ok_or_else(|| {
                "Invariant 5 violation: target_type 'corrective_plan' requires non-null candidate_plan_digest"
                    .to_string()
            })?;
            let base_ws = b.baseline_workspace_digest.as_deref().ok_or_else(|| {
                "Invariant 5 violation: target_type 'corrective_plan' requires non-null baseline_workspace_digest"
                    .to_string()
            })?;
            let diff = b.diff_digest.as_deref().ok_or_else(|| {
                "Invariant 5 violation: target_type 'corrective_plan' requires non-null diff_digest"
                    .to_string()
            })?;

            if Some(root) != run_state.plan_lineage.root_plan_digest.as_deref() {
                return Err(
                    "Invariant 5 violation: root_plan_digest mismatch in corrective_plan"
                        .to_string(),
                );
            }
            if Some(active) != run_state.plan_lineage.active_plan_digest.as_deref() {
                return Err("Invariant 5 violation: active_plan_digest mismatch against existing approved plan in corrective_plan".to_string());
            }
            if Some(cand) != run_state.plan_lineage.candidate_plan_digest.as_deref() {
                return Err(
                    "Invariant 5 violation: candidate_plan_digest mismatch in corrective_plan"
                        .to_string(),
                );
            }
            if base_ws != run_state.workspace.baseline_workspace_digest {
                return Err(
                    "Invariant 5 violation: baseline_workspace_digest mismatch in corrective_plan"
                        .to_string(),
                );
            }
            if Some(diff) != run_state.current_diff_digest.as_deref() {
                return Err(
                    "Invariant 5 violation: diff_digest mismatch in corrective_plan".to_string(),
                );
            }
        }
        ApprovalTargetType::ImplementationDiff => {
            if b.candidate_plan_digest.is_some() {
                return Err(
                    "Invariant 5 violation: target_type 'implementation_diff' requires candidate_plan_digest == null"
                        .to_string(),
                );
            }
            let root = b.root_plan_digest.as_deref().ok_or_else(|| {
                "Invariant 5 violation: target_type 'implementation_diff' requires non-null root_plan_digest"
                    .to_string()
            })?;
            let active = b.active_plan_digest.as_deref().ok_or_else(|| {
                "Invariant 5 violation: target_type 'implementation_diff' requires non-null active_plan_digest"
                    .to_string()
            })?;
            let diff = b.diff_digest.as_deref().ok_or_else(|| {
                "Invariant 5 violation: target_type 'implementation_diff' requires non-null diff_digest"
                    .to_string()
            })?;
            let ev = b.evidence_manifest_digest.as_deref().ok_or_else(|| {
                "Invariant 5 violation: target_type 'implementation_diff' requires non-null evidence_manifest_digest"
                    .to_string()
            })?;

            if Some(root) != run_state.plan_lineage.root_plan_digest.as_deref() {
                return Err(
                    "Invariant 5 violation: root_plan_digest mismatch in implementation_diff"
                        .to_string(),
                );
            }
            if Some(active) != run_state.plan_lineage.active_plan_digest.as_deref() {
                return Err(
                    "Invariant 5 violation: active_plan_digest mismatch in implementation_diff"
                        .to_string(),
                );
            }
            if Some(diff) != run_state.current_diff_digest.as_deref() {
                return Err(
                    "Invariant 5 violation: diff_digest mismatch in implementation_diff"
                        .to_string(),
                );
            }
            if Some(ev) != run_state.current_evidence_manifest_digest.as_deref() {
                return Err("Invariant 5 violation: evidence_manifest_digest mismatch in implementation_diff".to_string());
            }
        }
        ApprovalTargetType::ReleaseCandidate => {
            if b.candidate_plan_digest.is_some() {
                return Err(
                    "Invariant 5 violation: target_type 'release_candidate' requires candidate_plan_digest == null"
                        .to_string(),
                );
            }
            let root = b.root_plan_digest.as_deref().ok_or_else(|| {
                "Invariant 5 violation: target_type 'release_candidate' requires non-null root_plan_digest"
                    .to_string()
            })?;
            let active = b.active_plan_digest.as_deref().ok_or_else(|| {
                "Invariant 5 violation: target_type 'release_candidate' requires non-null active_plan_digest"
                    .to_string()
            })?;
            let diff = b.diff_digest.as_deref().ok_or_else(|| {
                "Invariant 5 violation: target_type 'release_candidate' requires non-null diff_digest"
                    .to_string()
            })?;
            let ev = b.evidence_manifest_digest.as_deref().ok_or_else(|| {
                "Invariant 5 violation: target_type 'release_candidate' requires non-null evidence_manifest_digest"
                    .to_string()
            })?;
            let rel = b.release_manifest_digest.as_deref().ok_or_else(|| {
                "Invariant 5 violation: target_type 'release_candidate' requires non-null release_manifest_digest"
                    .to_string()
            })?;

            if Some(root) != run_state.plan_lineage.root_plan_digest.as_deref() {
                return Err(
                    "Invariant 5 violation: root_plan_digest mismatch in release_candidate"
                        .to_string(),
                );
            }
            if Some(active) != run_state.plan_lineage.active_plan_digest.as_deref() {
                return Err(
                    "Invariant 5 violation: active_plan_digest mismatch in release_candidate"
                        .to_string(),
                );
            }
            if Some(diff) != run_state.current_diff_digest.as_deref() {
                return Err(
                    "Invariant 5 violation: diff_digest mismatch in release_candidate".to_string(),
                );
            }
            if Some(ev) != run_state.current_evidence_manifest_digest.as_deref() {
                return Err(
                    "Invariant 5 violation: evidence_manifest_digest mismatch in release_candidate"
                        .to_string(),
                );
            }
            if Some(rel) != run_state.current_release_manifest_digest.as_deref() {
                return Err(
                    "Invariant 5 violation: release_manifest_digest mismatch in release_candidate"
                        .to_string(),
                );
            }
        }
    }

    Ok(())
}

/// Evaluates `has_valid_reusable_impl_approval(run_state)` per §6.3 and Hard Invariant 3 (Test #13).
pub fn has_valid_reusable_impl_approval(run_state: &AutoConvergeRunState) -> bool {
    match run_state.profile.review_reuse_policy {
        ReviewReusePolicy::AlwaysReview => return false,
        ReviewReusePolicy::ChangedOnly => {
            if run_state.allowlist_modified_since_last_impl_approval {
                return false;
            }
        }
        ReviewReusePolicy::ValidBinding => {}
    }

    let root = match run_state.plan_lineage.root_plan_digest.as_deref() {
        Some(r) if is_valid_sha256_hex(r) => r,
        _ => return false,
    };
    let active = match run_state.plan_lineage.active_plan_digest.as_deref() {
        Some(a) if is_valid_sha256_hex(a) => a,
        _ => return false,
    };
    let allowlist = match run_state.allowlist_digest.as_deref() {
        Some(al) if is_valid_sha256_hex(al) => al,
        _ => return false,
    };
    let diff = match run_state.current_diff_digest.as_deref() {
        Some(d) if is_valid_sha256_hex(d) => d,
        _ => return false,
    };
    let ev = match run_state.current_evidence_manifest_digest.as_deref() {
        Some(e) if is_valid_sha256_hex(e) => e,
        _ => return false,
    };
    let profile_digest = &run_state.policy_profile_digest;

    run_state.approvals.iter().rev().any(|record| {
        if record.invalidated
            || record.binding.target_type != ApprovalTargetType::ImplementationDiff
        {
            return false;
        }
        let expected_tag = match compute_approval_auth_tag(
            &run_state.run_id,
            record.sequence,
            &record.reviewer_session_id,
            &record.producer_session_id,
            &record.binding,
        ) {
            Ok(tag) => tag,
            Err(_) => return false,
        };
        if record.engine_auth_tag != expected_tag {
            return false;
        }
        record.binding.root_plan_digest.as_deref() == Some(root)
            && record.binding.active_plan_digest.as_deref() == Some(active)
            && record.binding.candidate_plan_digest.is_none()
            && record.binding.allowlist_digest == allowlist
            && record.binding.diff_digest.as_deref() == Some(diff)
            && record.binding.evidence_manifest_digest.as_deref() == Some(ev)
            && &record.binding.policy_profile_digest == profile_digest
    })
}

// ---------------------------------------------------------------------------
// 4. Dual-Gate Permissions & Two-Phase Release Authorization (§9.1, §9.2, §16.5)
// ---------------------------------------------------------------------------

/// Validates requested run permissions against:
/// 1. DeliveryPhase gate (§16.2 / §16.5: Phase A rejects any Git/release/paid-live-API bits)
/// 2. Dual-Gate Profile Capability bitmask (§9.1: any bit not enabled in profile is rejected)
/// 3. Mechanical Prerequisite Chains (`commit -> stage`, `push_branch -> commit`, `push_tag -> tag`, `release_publish -> push_tag`)
/// 4. Release Readiness Review requirement when any tag/release bit is enabled (§8.2 Rule 5)
/// 5. Phase 1 `ReleasePermissionIntent` when any tag/release bit is enabled (§9.2)
pub fn validate_and_compute_run_start_permission_binding(
    run_id: &str,
    repo_remote_url: &str,
    target_branch: &str,
    profile: &AutoConvergeProfile,
    requested_permissions: u32,
    release_intent: Option<&ReleasePermissionIntent>,
    delivery_phase: DeliveryPhase,
) -> Result<(u32, String), String> {
    validate_profile_against_invariants(profile)?;

    // §16.5 Phase A boundary enforcement
    match delivery_phase {
        DeliveryPhase::PhaseALocalConvergenceMvp => {
            if (requested_permissions & (RELEASE_ACTION_MASK | PERM_PAID_LIVE_API)) != 0 {
                return Err(
                    "Phase A MVP boundary violation: stage, commit, push, tag, release, and paid live API permissions are rejected in Phase A"
                        .to_string(),
                );
            }
        }
        DeliveryPhase::PhaseBLocalGitActions => {
            if (requested_permissions & (TAG_OR_RELEASE_MASK | PERM_PAID_LIVE_API)) != 0 {
                return Err(
                    "Phase B boundary violation: tag, push_tag, release_publish, and paid live API permissions are rejected in Phase B"
                        .to_string(),
                );
            }
        }
        DeliveryPhase::PhaseCTagAndRelease => {}
    }

    // Dual-Gate check (§9.1): every requested permission bit must be permitted by Profile Capabilities
    let profile_mask = profile.capabilities.to_bitmask();
    let effective = profile_mask & requested_permissions;
    if effective != requested_permissions {
        return Err(format!(
            "Dual-gate permission denial: requested_permissions (0x{requested_permissions:02x}) exceed profile capabilities (0x{profile_mask:02x})"
        ));
    }

    // Prerequisite chains (§9.1)
    if (effective & PERM_COMMIT) != 0 && (effective & PERM_STAGE) == 0 {
        return Err("Permission chain violation: commit requires stage".to_string());
    }
    if (effective & PERM_PUSH_BRANCH) != 0 && (effective & PERM_COMMIT) == 0 {
        return Err("Permission chain violation: push_branch requires commit".to_string());
    }
    if (effective & PERM_PUSH_TAG) != 0 && (effective & PERM_TAG) == 0 {
        return Err("Permission chain violation: push_tag requires tag".to_string());
    }
    if (effective & PERM_RELEASE_PUBLISH) != 0 && (effective & PERM_PUSH_TAG) == 0 {
        return Err("Permission chain violation: release_publish requires push_tag".to_string());
    }

    // Tag/release requires release_readiness_review + Phase 1 ReleasePermissionIntent (§9.2)
    let canonical_intent = if (effective & TAG_OR_RELEASE_MASK) != 0 {
        if !profile.release_readiness_review {
            return Err(
                "Tag/release permissions unconditionally require profile.release_readiness_review == true"
                    .to_string(),
            );
        }
        let intent = release_intent.ok_or_else(|| {
            "Phase 1 ReleasePermissionIntent (requested_version_tag, release_repository) is required at RunStart when tag/release permissions are requested".to_string()
        })?;
        if intent.requested_version_tag.trim().is_empty()
            || intent.release_repository.trim().is_empty()
        {
            return Err(
                "ReleasePermissionIntent fields (requested_version_tag, release_repository) cannot be empty"
                    .to_string(),
            );
        }
        serde_json::to_string(intent).unwrap_or_default()
    } else {
        "none".to_string()
    };

    let binding = compute_run_start_permission_binding_raw(
        run_id,
        repo_remote_url,
        target_branch,
        effective,
        &canonical_intent,
    )?;
    Ok((effective, binding))
}

pub fn compute_run_start_permission_binding_raw(
    run_id: &str,
    repo_remote_url: &str,
    target_branch: &str,
    effective_permissions: u32,
    canonical_release_intent: &str,
) -> Result<String, String> {
    let payload = format!(
        "{run_id}|{repo_remote_url}|{target_branch}|{effective_permissions}|{canonical_release_intent}"
    );
    let key = engine_key()?;
    Ok(key.compute_hmac(payload.as_bytes()))
}

pub fn verify_run_start_permission_binding(run_state: &AutoConvergeRunState) -> Result<(), String> {
    let eff = run_state.effective_permissions;
    let canonical_intent = if (eff & TAG_OR_RELEASE_MASK) != 0 {
        let intent = run_state
            .release_authorization
            .release_intent
            .as_ref()
            .ok_or_else(|| "Missing ReleasePermissionIntent in run_state".to_string())?;
        serde_json::to_string(intent).unwrap_or_default()
    } else {
        "none".to_string()
    };

    let payload = format!(
        "{}|{}|{}|{}|{}",
        run_state.run_id,
        run_state.workspace.repo_remote_url,
        run_state.workspace.branch_name,
        eff,
        canonical_intent
    );

    let key = engine_key()?;
    key.verify_hmac(
        payload.as_bytes(),
        &run_state.release_authorization.run_start_permission_binding,
    )
    .map_err(|e| format!("Permission binding verification failed: {e}"))
}

/// Seals Phase 2 `CandidateReleaseAuthorization` upon `ReleaseReview` `APPROVE` (§9.2).
pub fn seal_candidate_release_authorization(
    run_state: &mut AutoConvergeRunState,
    reviewed_version_tag: &str,
    reviewed_release_repo: &str,
) -> Result<String, String> {
    verify_run_start_permission_binding(run_state)?;
    let intent = run_state
        .release_authorization
        .release_intent
        .as_ref()
        .ok_or_else(|| {
            "Missing ReleasePermissionIntent for CandidateReleaseAuthorization".to_string()
        })?;

    if reviewed_version_tag != intent.requested_version_tag {
        return Err(format!(
            "Release target mismatch: reviewed version_tag '{reviewed_version_tag}' != requested '{}'",
            intent.requested_version_tag
        ));
    }
    if reviewed_release_repo != intent.release_repository {
        return Err(format!(
            "Release target mismatch: reviewed release_repository '{reviewed_release_repo}' != requested '{}'",
            intent.release_repository
        ));
    }

    let diff_digest = run_state
        .current_diff_digest
        .as_deref()
        .ok_or_else(|| "Missing current_diff_digest at ReleaseReview APPROVE".to_string())?;
    let release_manifest_digest = run_state
        .current_release_manifest_digest
        .as_deref()
        .ok_or_else(|| {
            "Missing current_release_manifest_digest at ReleaseReview APPROVE".to_string()
        })?;

    let payload = format!(
        "{}|{}|{}|{}|{}",
        run_state.release_authorization.run_start_permission_binding,
        diff_digest,
        release_manifest_digest,
        reviewed_version_tag,
        reviewed_release_repo
    );
    let key = engine_key()?;
    let auth = key.compute_hmac(payload.as_bytes());
    run_state
        .release_authorization
        .candidate_release_authorization = Some(auth.clone());
    Ok(auth)
}

/// Seals Phase 2 `ConcreteReleaseTarget` and `ReleaseExecutionPermissionBinding` immediately after
/// `commit` verifies that `HEAD`'s `RunDeltaManifest` matches `candidate_diff_digest` (§9.2, §9.3).
pub fn seal_concrete_release_target_and_execution_binding(
    run_state: &mut AutoConvergeRunState,
    target_commit_sha: &str,
) -> Result<(), String> {
    verify_run_start_permission_binding(run_state)?;
    let candidate_auth = run_state
        .release_authorization
        .candidate_release_authorization
        .as_deref()
        .ok_or_else(|| {
            "Missing CandidateReleaseAuthorization: cannot seal ConcreteReleaseTarget without ReleaseReview APPROVE".to_string()
        })?
        .to_string();

    let intent = run_state
        .release_authorization
        .release_intent
        .as_ref()
        .ok_or_else(|| "Missing ReleasePermissionIntent".to_string())?
        .clone();
    let candidate_diff_digest = run_state
        .current_diff_digest
        .clone()
        .ok_or_else(|| "Missing current_diff_digest".to_string())?;
    let release_manifest_digest = run_state
        .current_release_manifest_digest
        .clone()
        .ok_or_else(|| "Missing current_release_manifest_digest".to_string())?;

    if target_commit_sha.trim().is_empty() {
        return Err("target_commit_sha cannot be empty".to_string());
    }

    let concrete = ConcreteReleaseTarget {
        version_tag: intent.requested_version_tag,
        candidate_diff_digest,
        release_manifest_digest,
        target_commit_sha: target_commit_sha.to_string(),
        release_repository: intent.release_repository,
    };

    let concrete_json = serde_json::to_string(&concrete).unwrap_or_default();
    let payload = format!("{candidate_auth}|{concrete_json}");
    let key = engine_key()?;
    let exec_binding = key.compute_hmac(payload.as_bytes());

    run_state.release_authorization.concrete_release_target = Some(concrete);
    run_state
        .release_authorization
        .release_execution_permission_binding = Some(exec_binding);
    Ok(())
}

/// Verifies `ConcreteReleaseTarget` and `ReleaseExecutionPermissionBinding` before `tag`, `push_tag`, or `release_publish` (§9.2).
pub fn verify_release_execution_binding(run_state: &AutoConvergeRunState) -> Result<(), String> {
    verify_run_start_permission_binding(run_state)?;
    let intent = run_state
        .release_authorization
        .release_intent
        .as_ref()
        .ok_or_else(|| "Missing ReleasePermissionIntent".to_string())?;
    let candidate_auth = run_state
        .release_authorization
        .candidate_release_authorization
        .as_deref()
        .ok_or_else(|| "Missing CandidateReleaseAuthorization".to_string())?;
    let concrete = run_state
        .release_authorization
        .concrete_release_target
        .as_ref()
        .ok_or_else(|| "Missing ConcreteReleaseTarget".to_string())?;
    let exec_binding = run_state
        .release_authorization
        .release_execution_permission_binding
        .as_deref()
        .ok_or_else(|| "Missing ReleaseExecutionPermissionBinding".to_string())?;

    if concrete.version_tag != intent.requested_version_tag {
        return Err(
            "ConcreteReleaseTarget.version_tag does not match ReleasePermissionIntent".to_string(),
        );
    }
    if concrete.release_repository != intent.release_repository {
        return Err(
            "ConcreteReleaseTarget.release_repository does not match ReleasePermissionIntent"
                .to_string(),
        );
    }
    if Some(concrete.candidate_diff_digest.as_str()) != run_state.current_diff_digest.as_deref() {
        return Err(
            "ConcreteReleaseTarget.candidate_diff_digest does not match current_diff_digest"
                .to_string(),
        );
    }
    if Some(concrete.release_manifest_digest.as_str())
        != run_state.current_release_manifest_digest.as_deref()
    {
        return Err("ConcreteReleaseTarget.release_manifest_digest does not match current_release_manifest_digest".to_string());
    }
    if concrete.target_commit_sha != run_state.workspace.git_head_commit {
        return Err(
            "ConcreteReleaseTarget.target_commit_sha does not match committed HEAD SHA".to_string(),
        );
    }

    let expected_cand_payload = format!(
        "{}|{}|{}|{}|{}",
        run_state.release_authorization.run_start_permission_binding,
        concrete.candidate_diff_digest,
        concrete.release_manifest_digest,
        concrete.version_tag,
        concrete.release_repository
    );
    let key = engine_key()?;
    key.verify_hmac(expected_cand_payload.as_bytes(), candidate_auth)
        .map_err(|e| {
            format!("CandidateReleaseAuthorization cryptographic verification failed: {e}")
        })?;

    let concrete_json = serde_json::to_string(concrete).unwrap_or_default();
    let expected_exec_payload = format!("{candidate_auth}|{concrete_json}");
    key.verify_hmac(expected_exec_payload.as_bytes(), exec_binding)
        .map_err(|e| {
            format!("ReleaseExecutionPermissionBinding cryptographic verification failed: {e}")
        })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// 5. Engine-Owned State Authentication, Persistence & Replay Protection (§10, §16.3)
// ---------------------------------------------------------------------------

pub fn keyed_sha256_hex(secret: &[u8], payload: &[u8]) -> String {
    let mut buf = Vec::with_capacity(secret.len() + 1 + payload.len());
    buf.extend_from_slice(secret);
    buf.push(b':');
    buf.extend_from_slice(payload);
    sha256_hex(&buf)
}

pub fn compute_approval_auth_tag(
    run_id: &str,
    sequence: u64,
    reviewer_session_id: &str,
    producer_session_id: &str,
    binding: &ApprovalBinding,
) -> Result<String, String> {
    let binding_json = serde_json::to_string(binding).unwrap_or_default();
    let payload =
        format!("{run_id}|{sequence}|{reviewer_session_id}|{producer_session_id}|{binding_json}");
    let key = engine_key()?;
    Ok(key.compute_hmac(payload.as_bytes()))
}

pub fn compute_engine_state_auth_tag(state: &AutoConvergeRunState) -> Result<String, String> {
    let mut copy = state.clone();
    copy.engine_auth_tag = String::new();
    let json = serde_json::to_string(&copy).unwrap_or_default();
    let key = engine_key()?;
    Ok(key.compute_hmac(json.as_bytes()))
}

struct RunStateLockEntry {
    holder: ThreadId,
    count: usize,
}

static RUN_STATE_LOCKS: std::sync::LazyLock<Mutex<HashMap<PathBuf, RunStateLockEntry>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
static RUN_STATE_CONDVAR: std::sync::LazyLock<Condvar> =
    std::sync::LazyLock::new(|| Condvar::new());

pub struct RunStateLockGuard {
    path: PathBuf,
}

impl Drop for RunStateLockGuard {
    fn drop(&mut self) {
        let mut map = RUN_STATE_LOCKS.lock().unwrap();
        if let Some(entry) = map.get_mut(&self.path) {
            if entry.count > 1 {
                entry.count -= 1;
            } else {
                map.remove(&self.path);
                RUN_STATE_CONDVAR.notify_all();
            }
        }
    }
}

// Test failure injection seams for run-state save (§304.3 #4, §306.3 #2)
pub static INJECT_PRE_RENAME_SAVE_ERROR: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
pub static INJECT_POST_RENAME_SAVE_ERROR: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Derives a stable, canonical run-state lock key (§304.3 #5, §307.2 #4).
/// Keys from the canonical existing parent directory plus the fixed state filename,
/// ensuring the key is identical whether `run_state.json` exists on disk or not.
pub fn canonical_run_state_lock_key(state_path: &Path) -> PathBuf {
    let filename = state_path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("run_state.json"));
    if let Some(parent) = state_path.parent() {
        if parent.as_os_str().is_empty() {
            if let Ok(canon_cur) = Path::new(".").canonicalize() {
                return canon_cur.join(filename);
            }
        } else if let Ok(canon_parent) = parent.canonicalize() {
            return canon_parent.join(filename);
        } else {
            // Walk up ancestors to find deepest existing directory
            let mut rel_parts = Vec::new();
            let mut cur = parent;
            while !cur.as_os_str().is_empty() && !cur.exists() {
                if let Some(name) = cur.file_name() {
                    rel_parts.push(name);
                    match cur.parent() {
                        Some(p) if p != cur => cur = p,
                        _ => break,
                    }
                } else {
                    break;
                }
            }
            if let Ok(canon_base) = cur.canonicalize() {
                let mut full = canon_base;
                for part in rel_parts.into_iter().rev() {
                    full.push(part);
                }
                return full.join(filename);
            }
        }
    }
    state_path.to_path_buf()
}

/// Acquires a re-entrant, serialized per-path run-state lock (§301.2, §301.4 #1, §304.3 #5).
/// Guarantees that only one thread can mutate or reconcile a given run-state path at a time.
pub fn acquire_run_state_lock(state_path: &Path) -> RunStateLockGuard {
    let norm_path = canonical_run_state_lock_key(state_path);
    let current_thread = std::thread::current().id();
    let mut map = RUN_STATE_LOCKS.lock().unwrap();
    loop {
        if let Some(entry) = map.get_mut(&norm_path) {
            if entry.holder == current_thread {
                entry.count += 1;
                return RunStateLockGuard { path: norm_path };
            }
            map = RUN_STATE_CONDVAR.wait(map).unwrap();
        } else {
            map.insert(
                norm_path.clone(),
                RunStateLockEntry {
                    holder: current_thread,
                    count: 1,
                },
            );
            return RunStateLockGuard { path: norm_path };
        }
    }
}

/// Checks if the run state lock for `state_path` is currently held by the calling thread.
pub fn is_run_state_locked_by_current_thread(state_path: &Path) -> bool {
    let norm_path = canonical_run_state_lock_key(state_path);
    let current_thread = std::thread::current().id();
    let map = RUN_STATE_LOCKS.lock().unwrap();
    map.get(&norm_path)
        .map_or(false, |e| e.holder == current_thread)
}

/// Prepares and seals `AutoConvergeRunState` for persistence (§301.4 #1, §304.3 #3, §306.3 #1).
/// Increments `state_sequence` once, re-computes `engine_auth_tag` once, and verifies approval tags.
pub fn seal_run_state_authenticated(state: &mut AutoConvergeRunState) -> Result<(), String> {
    state.state_sequence += 1;
    state.engine_auth_tag = compute_engine_state_auth_tag(state)?;

    for approval in &state.approvals {
        let expected_tag = compute_approval_auth_tag(
            &state.run_id,
            approval.sequence,
            &approval.reviewer_session_id,
            &approval.producer_session_id,
            &approval.binding,
        )?;
        if approval.engine_auth_tag != expected_tag {
            return Err(
                "Tamper detected: approval record carries unauthenticated engine_auth_tag"
                    .to_string(),
            );
        }
    }
    Ok(())
}

/// Atomically persists an already-sealed `AutoConvergeRunState` to disk (§304.3 #4, §306.3 #1).
/// Does NOT increment sequence or recompute auth tag.
pub fn save_sealed_run_state(
    state_path: &Path,
    sealed_state: &AutoConvergeRunState,
) -> Result<(), String> {
    let _lock_guard = acquire_run_state_lock(state_path);

    if let Some(parent) = state_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed creating run state directory: {e}"))?;
    }

    let bytes = serde_json::to_vec_pretty(sealed_state)
        .map_err(|e| format!("Failed serializing run_state.json: {e}"))?;
    let tmp_path = state_path.with_extension("json.tmp");
    {
        let mut f = std::fs::File::create(&tmp_path)
            .map_err(|e| format!("Failed creating temp run_state file: {e}"))?;
        f.write_all(&bytes)
            .map_err(|e| format!("Failed writing temp run_state file: {e}"))?;
        f.sync_all()
            .map_err(|e| format!("Failed flushing temp run_state file: {e}"))?;
    }

    if INJECT_PRE_RENAME_SAVE_ERROR.swap(false, std::sync::atomic::Ordering::SeqCst) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err("Injected pre-rename save failure".to_string());
    }

    std::fs::rename(&tmp_path, state_path)
        .map_err(|e| format!("Failed atomic rename of run_state.json: {e}"))?;

    if INJECT_POST_RENAME_SAVE_ERROR.swap(false, std::sync::atomic::Ordering::SeqCst) {
        return Err(
            "Injected post-rename reported error (simulated IO/network failure after rename)"
                .to_string(),
        );
    }

    Ok(())
}

/// Persists `AutoConvergeRunState` atomically (`run_state.json.tmp` -> `sync_all()` -> rename)
/// while incrementing `state_sequence` and sealing `engine_auth_tag` (§10.1, §16.3, §301.4 #1).
pub fn save_run_state_authenticated(
    state_path: &Path,
    state: &mut AutoConvergeRunState,
) -> Result<(), String> {
    let _lock_guard = acquire_run_state_lock(state_path);
    seal_run_state_authenticated(state)?;
    save_sealed_run_state(state_path, state)
}

/// Loads `run_state.json` and verifies schema version, `engine_auth_tag`, all approval tags,
/// `policy_profile_digest`, `RunStartPermissionBinding`, and monotonic `expected_min_sequence`
/// to prevent tampering, forgery, replay, or rollback (§10.2, §16.3).
pub fn load_and_verify_run_state_authenticated(
    state_path: &Path,
    expected_min_sequence: Option<u64>,
) -> Result<AutoConvergeRunState, String> {
    let raw = std::fs::read_to_string(state_path).map_err(|e| {
        format!(
            "Failed reading run_state.json at {}: {e}",
            state_path.display()
        )
    })?;
    let state: AutoConvergeRunState = serde_json::from_str(&raw)
        .map_err(|e| format!("Malformed or schema-invalid run_state.json: {e}"))?;

    if state.schema_version != 1 {
        return Err(format!(
            "Unsupported run_state.json schema_version: {}",
            state.schema_version
        ));
    }

    let key = engine_key()?;
    let mut copy = state.clone();
    copy.engine_auth_tag = String::new();
    let json = serde_json::to_string(&copy).unwrap_or_default();

    key.verify_hmac(json.as_bytes(), &state.engine_auth_tag)
        .map_err(|e| format!("Tamper detected: run_state.json authentication failed ({e})"))?;

    if let Some(min_seq) = expected_min_sequence {
        if state.state_sequence < min_seq {
            return Err(format!(
                "Rollback / replay detected: state_sequence {} < expected minimum {}",
                state.state_sequence, min_seq
            ));
        }
    }

    if state.profile.canonical_digest() != state.policy_profile_digest {
        return Err(
            "Tamper detected: sealed profile snapshot does not match policy_profile_digest"
                .to_string(),
        );
    }

    verify_run_start_permission_binding(&state)?;

    for approval in &state.approvals {
        let expected_app_tag = compute_approval_auth_tag(
            &state.run_id,
            approval.sequence,
            &approval.reviewer_session_id,
            &approval.producer_session_id,
            &approval.binding,
        )?;
        if approval.engine_auth_tag != expected_app_tag {
            return Err(
                "Tamper detected: forged or unauthenticated approval record in run_state.json"
                    .to_string(),
            );
        }
    }

    Ok(state)
}

// ---------------------------------------------------------------------------
// 6. CLI Subcommand Dispatcher (`anthro-bridge auto-converge-guard`) (§2.1)
// ---------------------------------------------------------------------------

pub fn is_auto_converge_guard_mode(args: &[String]) -> bool {
    args.get(1).map(|s| s.as_str()) == Some("auto-converge-guard")
}

pub fn run_auto_converge_guard_cli(args: &[String]) -> i32 {
    match execute_guard_cli(args) {
        Ok(val) => {
            println!("{}", serde_json::to_string(&val).unwrap_or_default());
            0
        }
        Err(err) => {
            println!(
                "{}",
                serde_json::to_string(&json!({ "status": "denied", "error": err }))
                    .unwrap_or_default()
            );
            1
        }
    }
}

fn execute_guard_cli(args: &[String]) -> Result<serde_json::Value, String> {
    let subcommand = args
        .get(2)
        .map(|s| s.as_str())
        .ok_or_else(|| "Missing auto-converge-guard subcommand".to_string())?;

    let mut state_file: Option<String> = None;
    let mut payload_json: Option<String> = None;
    let mut payload_file: Option<String> = None;
    let mut action_bit: Option<String> = None;

    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--state-file" => {
                state_file = args.get(i + 1).cloned();
                i += 2;
            }
            "--payload-json" => {
                payload_json = args.get(i + 1).cloned();
                i += 2;
            }
            "--payload-file" => {
                payload_file = args.get(i + 1).cloned();
                i += 2;
            }
            "--action" => {
                action_bit = args.get(i + 1).cloned();
                i += 2;
            }
            other => return Err(format!("Unknown CLI flag: {other}")),
        }
    }

    let read_payload = || -> Result<String, String> {
        if let Some(ref p) = payload_json {
            return Ok(p.clone());
        }
        if let Some(ref path) = payload_file {
            return std::fs::read_to_string(path)
                .map_err(|e| format!("Failed reading payload file {path}: {e}"));
        }
        Err("Missing --payload-json or --payload-file".to_string())
    };

    match subcommand {
        "validate-profile" => {
            let body = read_payload()?;
            let profile: AutoConvergeProfile =
                serde_json::from_str(&body).map_err(|e| format!("Invalid profile JSON: {e}"))?;
            validate_profile_against_invariants(&profile)?;
            Ok(json!({
                "status": "ok",
                "policy_profile_digest": profile.canonical_digest()
            }))
        }
        "extract-allowlist" => {
            let body = read_payload()?;
            let (allowlist, digest) =
                extract_and_validate_allowlist_from_plan(&body, cfg!(windows))?;
            Ok(json!({
                "status": "ok",
                "allowlist": allowlist,
                "allowlist_digest": digest
            }))
        }
        "status" => {
            let path = state_file.ok_or_else(|| "Missing --state-file".to_string())?;
            let state = load_and_verify_run_state_authenticated(Path::new(&path), None)?;
            Ok(json!({
                "status": "ok",
                "run_id": state.run_id,
                "current_state": state.current_state,
                "is_terminal": state.is_terminal,
                "review_mode": state.review_mode
            }))
        }
        "check-permission" => {
            let path = state_file.ok_or_else(|| "Missing --state-file".to_string())?;
            let action = action_bit.ok_or_else(|| "Missing --action".to_string())?;
            let state = load_and_verify_run_state_authenticated(Path::new(&path), None)?;
            verify_run_start_permission_binding(&state)?;
            let required_bit = match action.as_str() {
                "stage" => PERM_STAGE,
                "commit" => PERM_COMMIT,
                "push_branch" => PERM_PUSH_BRANCH,
                "tag" => PERM_TAG,
                "push_tag" => PERM_PUSH_TAG,
                "release_publish" => PERM_RELEASE_PUBLISH,
                "paid_live_api" => PERM_PAID_LIVE_API,
                _ => return Err(format!("Unknown action '{action}'")),
            };
            if (state.effective_permissions & required_bit) == 0 {
                return Err(format!(
                    "Action '{action}' is not authorized by effective_permissions"
                ));
            }
            if (required_bit & TAG_OR_RELEASE_MASK) != 0 {
                verify_release_execution_binding(&state)?;
            }
            if (required_bit & LOCAL_GIT_MASK) != 0 && state.effective_permissions == 0 {
                return Err("No local git permissions granted".to_string());
            }
            Ok(json!({ "status": "ok", "action": action, "allowed": true }))
        }
        "verify-verdict" => {
            let path = state_file.ok_or_else(|| "Missing --state-file".to_string())?;
            let body = read_payload()?;
            let state = load_and_verify_run_state_authenticated(Path::new(&path), None)?;
            let verdict: CodexReviewVerdict =
                serde_json::from_str(&body).map_err(|e| format!("Malformed verdict JSON: {e}"))?;
            let reviewer_id = action_bit.unwrap_or_else(|| "cli-reviewer-session".to_string());
            validate_verdict_and_binding(
                &state,
                verdict.approval_binding.target_type,
                &reviewer_id,
                &verdict,
            )?;
            Ok(json!({
                "status": "ok",
                "verdict": verdict.verdict == VerdictDecision::Approve
            }))
        }
        other => Err(format!("Unknown auto-converge-guard subcommand: {other}")),
    }
}
