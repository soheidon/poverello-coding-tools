pub mod agent_runner;
pub mod finding_tracker;
pub mod git_runner;
pub mod guards;
pub mod isolation;
pub mod profile_store;
pub mod release_reviewer;
pub mod state_machine;
pub mod state_store;
pub mod types;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub use agent_runner::*;
pub use finding_tracker::*;
pub use git_runner::*;
pub use guards::*;
pub use isolation::*;
pub use profile_store::*;
pub use release_reviewer::*;
pub use state_machine::*;
pub use state_store::*;
pub use types::*;

/// Returns the engine-owned durable state path for `run_id`, stored in the OS user-data root
/// strictly outside agent-writable repository scopes (§10.1, §16.3).
pub fn engine_run_state_path(run_id: &str) -> Result<PathBuf, String> {
    let validated =
        state_store::validate_auto_converge_run_id(run_id).map_err(|error| error.to_string())?;
    state_store::get_engine_runs_dir(&validated)
        .map(|d| d.join("run_state.json"))
        .map_err(|e| format!("Failed to access engine runs directory: {e}"))
}

fn prepare_reviewer_snapshot_dir(
    temp_root: &Path,
    run_id: &str,
    cycle: u64,
) -> Result<PathBuf, String> {
    let validated_run_id =
        state_store::validate_auto_converge_run_id(run_id).map_err(|error| error.to_string())?;
    let snapshots_root = temp_root.join("poverello").join("snapshots");
    let snapshot_dir = snapshots_root
        .join(validated_run_id)
        .join(cycle.to_string());
    std::fs::create_dir_all(&snapshot_dir).map_err(|error| error.to_string())?;
    Ok(snapshot_dir)
}

#[tauri::command]
pub fn auto_converge_get_profiles() -> Result<Vec<AutoConvergeProfile>, String> {
    let store = ProfileStore::new(ProfileStore::default_custom_profiles_path());
    store.load_all_profiles()
}

#[tauri::command]
pub fn auto_converge_save_profile(profile: AutoConvergeProfile) -> Result<(), String> {
    let store = ProfileStore::new(ProfileStore::default_custom_profiles_path());
    store.save_custom_profile(profile)
}

#[tauri::command]
pub fn auto_converge_start_run(
    mut config: AutoConvergeRunConfig,
) -> Result<AutoConvergeRunState, String> {
    let requested_run_id = config
        .run_id
        .take()
        .unwrap_or_else(|| format!("ac-{}", uuid::Uuid::new_v4()));
    let run_id = state_store::validate_auto_converge_run_id(&requested_run_id)
        .map_err(|error| error.to_string())?;
    config.run_id = Some(run_id.clone());

    let ws_path = PathBuf::from(&config.working_dir);
    let manifest = capture_baseline_file_manifest(&ws_path, &BTreeMap::new())?;

    // Perform platform OS-boundary isolation preflight (§20.4 B, §23.1.1, §23.4.2)
    let snapshot_dir = prepare_reviewer_snapshot_dir(&std::env::temp_dir(), &run_id, 0)?;

    let git_runner = ProductionGitCommandRunner::new(ws_path.clone());
    let now = chrono::Utc::now().to_rfc3339();
    let configured_cli = config.agent_cli_path.clone();
    let mut state = initialize_run(config, "HEAD", manifest, &git_runner, &now)?;
    let state_path = engine_run_state_path(&state.run_id)?;

    if let Err(e) = PlatformIsolationGuard::verify_reviewer_os_boundary(&ws_path, &snapshot_dir) {
        halt_blocked(
            &mut state,
            BlockedKind::FatalInvariant,
            &format!("Platform isolation preflight failed: {e}"),
        );
        save_run_state_authenticated(&state_path, &mut state)?;
        return Ok(state);
    }

    save_run_state_authenticated(&state_path, &mut state)?;

    let cli_path = configured_cli
        .map(PathBuf::from)
        .or_else(|| std::env::var("POVERELLO_AGENT_CLI").ok().map(PathBuf::from));

    let agent_runner = match cli_path {
        Some(path) => {
            if !path.exists() {
                halt_blocked(
                    &mut state,
                    BlockedKind::FatalInvariant,
                    &format!("Configured agent CLI not found at: {}", path.display()),
                );
                save_run_state_authenticated(&state_path, &mut state)?;
                return Ok(state);
            }
            ProductionAgentRunner::with_cli(path)
        }
        None => {
            halt_blocked(
                &mut state,
                BlockedKind::FatalInvariant,
                "No configured agent CLI available: failing closed without spawning any process",
            );
            save_run_state_authenticated(&state_path, &mut state)?;
            return Ok(state);
        }
    };

    if let Err(e) =
        run_phase_a_to_completion(&mut state, &agent_runner, &git_runner, &state_path, 50)
    {
        halt_blocked(
            &mut state,
            BlockedKind::FatalInvariant,
            &format!("Phase A execution halted: {e}"),
        );
        save_run_state_authenticated(&state_path, &mut state)?;
    }
    Ok(state)
}

#[tauri::command]
pub fn auto_converge_get_run_state(run_id: String) -> Result<AutoConvergeRunState, String> {
    let state_path = engine_run_state_path(&run_id)?;
    load_and_verify_run_state_authenticated(&state_path, None)
}

#[tauri::command]
pub fn auto_converge_pause_run(run_id: String) -> Result<(), String> {
    let state_path = engine_run_state_path(&run_id)?;
    let mut state = load_and_verify_run_state_authenticated(&state_path, None)?;
    if state.is_terminal {
        return Err("Cannot pause a terminal Auto-Converge run".to_string());
    }
    state.is_paused = true;
    state.updated_at = chrono::Utc::now().to_rfc3339();
    save_run_state_authenticated(&state_path, &mut state)?;
    Ok(())
}

#[tauri::command]
pub fn auto_converge_resume_run(
    run_id: String,
    user_confirmation: Option<UserConfirmationToken>,
) -> Result<AutoConvergeRunState, String> {
    let state_path = engine_run_state_path(&run_id)?;
    let mut state = load_and_verify_run_state_authenticated(&state_path, None)?;
    let ws_path = PathBuf::from(&state.workspace.working_dir);

    // Verify isolation preflight upon resume (§20.4 B, §23.1.1, §23.4.2)
    let snapshot_dir =
        prepare_reviewer_snapshot_dir(&std::env::temp_dir(), &state.run_id, state.state_sequence)?;

    if let Err(e) = PlatformIsolationGuard::verify_reviewer_os_boundary(&ws_path, &snapshot_dir) {
        halt_blocked(
            &mut state,
            BlockedKind::FatalInvariant,
            &format!("Platform isolation preflight failed: {e}"),
        );
        save_run_state_authenticated(&state_path, &mut state)?;
        return Ok(state);
    }

    let current_manifest = capture_baseline_file_manifest(&ws_path, &BTreeMap::new())?;
    let git_runner = ProductionGitCommandRunner::new(ws_path);
    let now = chrono::Utc::now().to_rfc3339();
    let active_plan = state.plan_lineage.active_plan_digest.clone();
    let diff = state.current_diff_digest.clone();
    resume_run(
        &mut state,
        &current_manifest,
        active_plan.as_deref(),
        diff.as_deref(),
        &git_runner,
        user_confirmation,
        &now,
    )?;
    save_run_state_authenticated(&state_path, &mut state)?;

    let configured_cli = state.configured_agent_cli.clone();
    let cli_path = configured_cli
        .map(PathBuf::from)
        .or_else(|| std::env::var("POVERELLO_AGENT_CLI").ok().map(PathBuf::from));

    let agent_runner = match cli_path {
        Some(path) => {
            if !path.exists() {
                halt_blocked(
                    &mut state,
                    BlockedKind::FatalInvariant,
                    &format!("Configured agent CLI not found at: {}", path.display()),
                );
                save_run_state_authenticated(&state_path, &mut state)?;
                return Ok(state);
            }
            ProductionAgentRunner::with_cli(path)
        }
        None => {
            halt_blocked(
                &mut state,
                BlockedKind::FatalInvariant,
                "No configured agent CLI available: failing closed without spawning any process",
            );
            save_run_state_authenticated(&state_path, &mut state)?;
            return Ok(state);
        }
    };

    if let Err(e) =
        run_phase_a_to_completion(&mut state, &agent_runner, &git_runner, &state_path, 50)
    {
        halt_blocked(
            &mut state,
            BlockedKind::FatalInvariant,
            &format!("Phase A execution halted: {e}"),
        );
        save_run_state_authenticated(&state_path, &mut state)?;
    }
    Ok(state)
}

#[tauri::command]
pub fn auto_converge_abort_run(run_id: String) -> Result<(), String> {
    let state_path = engine_run_state_path(&run_id)?;
    let mut state = load_and_verify_run_state_authenticated(&state_path, None)?;
    halt_blocked(
        &mut state,
        BlockedKind::FatalInvariant,
        "Run aborted explicitly by user",
    );
    state.updated_at = chrono::Utc::now().to_rfc3339();
    save_run_state_authenticated(&state_path, &mut state)?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectedSnapshotFault {
    None,
    CreateDir,
    ReadPlan,
    WritePlan,
    WriteAllowlist,
    SerializeManifest,
    WriteManifest,
    ComputeDiff,
    WriteDiff,
    ReadDelta,
    ParseDelta,
    SerializeEvidence,
    WriteEvidence,
    SerializeFindings,
    WriteFindings,
    CopyFile,
    PreLaunchHash,
    PostLaunchHash,
}

#[cfg(test)]
thread_local! {
    static CURRENT_THREAD_FAULT: std::cell::Cell<InjectedSnapshotFault> = std::cell::Cell::new(InjectedSnapshotFault::None);
    static INJECTED_SYMLINK_PATH: std::cell::RefCell<Option<std::path::PathBuf>> = std::cell::RefCell::new(None);
}

#[cfg(test)]
pub struct ScopedSnapshotFaultGuard {
    prev_fault: InjectedSnapshotFault,
}

#[cfg(test)]
impl Drop for ScopedSnapshotFaultGuard {
    fn drop(&mut self) {
        CURRENT_THREAD_FAULT.with(|f| f.set(self.prev_fault));
    }
}

#[cfg(test)]
pub fn with_scoped_snapshot_fault(fault: InjectedSnapshotFault) -> ScopedSnapshotFaultGuard {
    let prev = CURRENT_THREAD_FAULT.with(|f| {
        let old = f.get();
        f.set(fault);
        old
    });
    ScopedSnapshotFaultGuard { prev_fault: prev }
}

#[cfg(test)]
pub struct ScopedInjectedSymlinkGuard;

#[cfg(test)]
impl Drop for ScopedInjectedSymlinkGuard {
    fn drop(&mut self) {
        INJECTED_SYMLINK_PATH.with(|p| *p.borrow_mut() = None);
    }
}

#[cfg(test)]
pub fn with_scoped_injected_symlink(path: std::path::PathBuf) -> ScopedInjectedSymlinkGuard {
    INJECTED_SYMLINK_PATH.with(|p| *p.borrow_mut() = Some(path));
    ScopedInjectedSymlinkGuard
}

#[cfg(test)]
pub fn is_injected_symlink(path: &Path) -> bool {
    INJECTED_SYMLINK_PATH.with(|p| {
        p.borrow()
            .as_ref()
            .map(|target| path.ends_with(target) || path == target)
            .unwrap_or(false)
    })
}

#[cfg(test)]
pub fn check_injected_fault(target: InjectedSnapshotFault) -> Result<(), String> {
    let active = CURRENT_THREAD_FAULT.with(|f| f.get());
    if active == target && target != InjectedSnapshotFault::None {
        Err(format!("Injected deterministic fault: {:?}", target))
    } else {
        Ok(())
    }
}

#[cfg(not(test))]
#[inline(always)]
pub fn check_injected_fault(_target: InjectedSnapshotFault) -> Result<(), String> {
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotEntryKind {
    File,
    Dir,
}

pub fn check_snapshot_entry_kind(
    is_symlink: bool,
    is_dir: bool,
    is_file: bool,
    path: &Path,
) -> Result<SnapshotEntryKind, String> {
    if is_symlink {
        return Err(format!(
            "Symlink forbidden in snapshot package: {}",
            path.display()
        ));
    }
    if is_dir {
        Ok(SnapshotEntryKind::Dir)
    } else if is_file {
        Ok(SnapshotEntryKind::File)
    } else {
        Err(format!(
            "Non-regular file in snapshot package: {}",
            path.display()
        ))
    }
}

/// Helper verifying that required snapshot package members exist and are non-empty.
pub fn verify_snapshot_package(snapshot_dir: &Path, required_files: &[&str]) -> Result<(), String> {
    for rel in required_files {
        let p = snapshot_dir.join(rel);
        if !p.exists() {
            return Err(format!("Required snapshot package member missing: {rel}"));
        }
        let meta =
            std::fs::metadata(&p).map_err(|e| format!("Failed to read metadata for {rel}: {e}"))?;
        if meta.len() == 0 {
            return Err(format!("Snapshot package member is empty: {rel}"));
        }
    }
    Ok(())
}

/// Validates snapshot contents against authenticated run state digests, baseline manifests, and stage bindings (§25.3, §26.3).
pub fn validate_snapshot_against_authenticated_state(
    snapshot_dir: &Path,
    state: &AutoConvergeRunState,
    stage: AutoConvergeState,
    ws_path: &Path,
    case_insensitive_fs: bool,
) -> Result<(), String> {
    if !snapshot_dir.exists() || !snapshot_dir.is_dir() {
        return Err("Snapshot directory does not exist or is not a directory".to_string());
    }

    // 1. Gather all relative paths recursively in snapshot_dir; reject symlinks, traversal, and non-regular files
    let mut files = BTreeSet::new();
    let mut dirs = vec![snapshot_dir.to_path_buf()];
    while let Some(d) = dirs.pop() {
        for entry in std::fs::read_dir(&d)
            .map_err(|e| format!("Failed to read directory {}: {e}", d.display()))?
        {
            let entry = entry.map_err(|e| format!("Dir entry error: {e}"))?;
            let file_type = entry
                .file_type()
                .map_err(|e| format!("Failed to get file type: {e}"))?;
            let is_symlink = file_type.is_symlink() || {
                #[cfg(test)]
                {
                    is_injected_symlink(&entry.path())
                }
                #[cfg(not(test))]
                {
                    false
                }
            };
            match check_snapshot_entry_kind(
                is_symlink,
                file_type.is_dir(),
                file_type.is_file(),
                &entry.path(),
            )? {
                SnapshotEntryKind::Dir => dirs.push(entry.path()),
                SnapshotEntryKind::File => {
                    let rel = entry
                        .path()
                        .strip_prefix(snapshot_dir)
                        .map_err(|e| format!("Failed to strip prefix: {e}"))?
                        .to_str()
                        .ok_or_else(|| "Invalid UTF-8 in snapshot path".to_string())?
                        .replace('\\', "/");
                    files.insert(rel);
                }
            }
        }
    }

    // 2. Determine fixed required files by stage
    let fixed_files = match stage {
        AutoConvergeState::PlanReview => vec![
            "candidate_plan.md",
            "allowlist.json",
            "review_manifest.json",
        ],
        AutoConvergeState::ImplementationReview => vec![
            "active_plan.md",
            "allowlist.json",
            "run_delta.json",
            "evidence.json",
            "review_manifest.json",
        ],
        AutoConvergeState::CorrectivePlanReview => vec![
            "candidate_corrective_plan.md",
            "unresolved_findings.json",
            "allowlist.json",
            "review_manifest.json",
        ],
        _ => {
            return Err(format!(
                "Unsupported stage for snapshot validation: {:?}",
                stage
            ))
        }
    };

    for fixed in &fixed_files {
        if !files.contains(*fixed) {
            return Err(format!("Required snapshot package member missing: {fixed}"));
        }
    }

    // 3. Validate plan file against authenticated digest
    match stage {
        AutoConvergeState::PlanReview => {
            let plan_path = snapshot_dir.join("candidate_plan.md");
            let bytes = std::fs::read(&plan_path)
                .map_err(|e| format!("Failed to read candidate_plan.md: {e}"))?;
            let digest = sha256_hex(&bytes);
            let expected = state
                .plan_lineage
                .candidate_plan_digest
                .as_deref()
                .ok_or_else(|| {
                    "candidate_plan_digest missing in authenticated state".to_string()
                })?;
            if digest != expected {
                return Err(format!(
                    "candidate_plan.md digest mismatch: computed {digest}, expected {expected}"
                ));
            }
        }
        AutoConvergeState::ImplementationReview => {
            let plan_path = snapshot_dir.join("active_plan.md");
            let bytes = std::fs::read(&plan_path)
                .map_err(|e| format!("Failed to read active_plan.md: {e}"))?;
            let digest = sha256_hex(&bytes);
            let expected = state
                .plan_lineage
                .active_plan_digest
                .as_deref()
                .ok_or_else(|| "active_plan_digest missing in authenticated state".to_string())?;
            if digest != expected {
                return Err(format!(
                    "active_plan.md digest mismatch: computed {digest}, expected {expected}"
                ));
            }
        }
        AutoConvergeState::CorrectivePlanReview => {
            let plan_path = snapshot_dir.join("candidate_corrective_plan.md");
            let bytes = std::fs::read(&plan_path)
                .map_err(|e| format!("Failed to read candidate_corrective_plan.md: {e}"))?;
            let digest = sha256_hex(&bytes);
            let expected = state
                .plan_lineage
                .candidate_plan_digest
                .as_deref()
                .ok_or_else(|| {
                    "candidate_plan_digest missing in authenticated state".to_string()
                })?;
            if digest != expected {
                return Err(format!("candidate_corrective_plan.md digest mismatch: computed {digest}, expected {expected}"));
            }
        }
        _ => {}
    }

    // 4. Validate allowlist.json
    let allowlist_path = snapshot_dir.join("allowlist.json");
    let allowlist_bytes = std::fs::read(&allowlist_path)
        .map_err(|e| format!("Failed to read allowlist.json: {e}"))?;
    let parsed_allowlist: Vec<String> = serde_json::from_slice(&allowlist_bytes)
        .map_err(|e| format!("Invalid JSON in allowlist.json: {e}"))?;
    if parsed_allowlist != state.runtime_allowlist {
        return Err(
            "Snapshot allowlist content does not match state runtime_allowlist".to_string(),
        );
    }
    let (_, computed_allowlist_digest) =
        canonicalize_and_hash_allowlist(&parsed_allowlist, case_insensitive_fs)?;
    let expected_allowlist_digest = state
        .allowlist_digest
        .as_deref()
        .ok_or_else(|| "allowlist_digest missing in authenticated state".to_string())?;
    if computed_allowlist_digest != expected_allowlist_digest {
        return Err(format!("Snapshot allowlist digest mismatch: computed {computed_allowlist_digest}, expected {expected_allowlist_digest}"));
    }

    // 5. Validate review_manifest.json binding fields
    let manifest_path = snapshot_dir.join("review_manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path)
        .map_err(|e| format!("Failed to read review_manifest.json: {e}"))?;
    let m: serde_json::Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| format!("Invalid JSON in review_manifest.json: {e}"))?;

    if m.get("run_id").and_then(|v| v.as_str()) != Some(&state.run_id) {
        return Err("review_manifest.json run_id mismatch".to_string());
    }
    if m.get("state_sequence").and_then(|v| v.as_u64()) != Some(state.state_sequence as u64) {
        return Err("review_manifest.json state_sequence mismatch".to_string());
    }
    if m.get("allowlist_digest").and_then(|v| v.as_str()) != state.allowlist_digest.as_deref() {
        return Err("review_manifest.json allowlist_digest mismatch".to_string());
    }
    if m.get("policy_profile_digest").and_then(|v| v.as_str()) != Some(&state.policy_profile_digest)
    {
        return Err("review_manifest.json policy_profile_digest mismatch".to_string());
    }

    match stage {
        AutoConvergeState::PlanReview => {
            if m.get("target_type").and_then(|v| v.as_str()) != Some("plan") {
                return Err("review_manifest.json target_type mismatch for PlanReview".to_string());
            }
            if m.get("candidate_plan_digest").and_then(|v| v.as_str())
                != state.plan_lineage.candidate_plan_digest.as_deref()
            {
                return Err("review_manifest.json candidate_plan_digest mismatch".to_string());
            }
            if m.get("baseline_workspace_digest").and_then(|v| v.as_str())
                != Some(&state.workspace.baseline_workspace_digest)
            {
                return Err("review_manifest.json baseline_workspace_digest mismatch".to_string());
            }
        }
        AutoConvergeState::ImplementationReview => {
            if m.get("target_type").and_then(|v| v.as_str()) != Some("implementation_diff") {
                return Err(
                    "review_manifest.json target_type mismatch for ImplementationReview"
                        .to_string(),
                );
            }
            if m.get("root_plan_digest").and_then(|v| v.as_str())
                != state.plan_lineage.root_plan_digest.as_deref()
            {
                return Err("review_manifest.json root_plan_digest mismatch".to_string());
            }
            if m.get("active_plan_digest").and_then(|v| v.as_str())
                != state.plan_lineage.active_plan_digest.as_deref()
            {
                return Err("review_manifest.json active_plan_digest mismatch".to_string());
            }
            if m.get("diff_digest").and_then(|v| v.as_str()) != state.current_diff_digest.as_deref()
            {
                return Err("review_manifest.json diff_digest mismatch".to_string());
            }
            if m.get("evidence_manifest_digest").and_then(|v| v.as_str())
                != state.current_evidence_manifest_digest.as_deref()
            {
                return Err("review_manifest.json evidence_manifest_digest mismatch".to_string());
            }
        }
        AutoConvergeState::CorrectivePlanReview => {
            if m.get("target_type").and_then(|v| v.as_str()) != Some("corrective_plan") {
                return Err(
                    "review_manifest.json target_type mismatch for CorrectivePlanReview"
                        .to_string(),
                );
            }
            if m.get("root_plan_digest").and_then(|v| v.as_str())
                != state.plan_lineage.root_plan_digest.as_deref()
            {
                return Err("review_manifest.json root_plan_digest mismatch".to_string());
            }
            if m.get("active_plan_digest").and_then(|v| v.as_str())
                != state.plan_lineage.active_plan_digest.as_deref()
            {
                return Err("review_manifest.json active_plan_digest mismatch".to_string());
            }
            if m.get("candidate_plan_digest").and_then(|v| v.as_str())
                != state.plan_lineage.candidate_plan_digest.as_deref()
            {
                return Err("review_manifest.json candidate_plan_digest mismatch".to_string());
            }
            if m.get("diff_digest").and_then(|v| v.as_str()) != state.current_diff_digest.as_deref()
            {
                return Err("review_manifest.json diff_digest mismatch".to_string());
            }
            if m.get("baseline_workspace_digest").and_then(|v| v.as_str())
                != Some(&state.workspace.baseline_workspace_digest)
            {
                return Err("review_manifest.json baseline_workspace_digest mismatch".to_string());
            }
        }
        _ => {}
    }

    let mut expected_package_files: BTreeSet<String> =
        fixed_files.iter().map(|s| s.to_string()).collect();

    let runtime_allowlist_set: BTreeSet<String> = state
        .runtime_allowlist
        .iter()
        .map(|p| p.replace('\\', "/").trim_start_matches("./").to_string())
        .collect();

    // 6. Stage-specific payload validation and file-level completeness/mode checks
    match stage {
        AutoConvergeState::PlanReview => {
            // Check baseline manifest integrity (no duplicates, no malformed paths)
            let mut seen_baseline = BTreeSet::new();
            for b in &state.workspace.baseline_file_manifest {
                let norm_b = b.path.replace('\\', "/");
                let norm_b = norm_b.trim_start_matches("./").to_string();
                if norm_b.is_empty() || norm_b.contains("..") || norm_b.starts_with('/') {
                    return Err(format!(
                        "Malformed path in baseline file manifest: {}",
                        b.path
                    ));
                }
                if !seen_baseline.insert(norm_b) {
                    return Err(format!("Duplicate baseline manifest entry: {}", b.path));
                }
            }

            for file in &state.runtime_allowlist {
                let norm_file = file.replace('\\', "/");
                let norm_file = norm_file.trim_start_matches("./").to_string();
                let snap_file = snapshot_dir.join(&norm_file);

                if let Some(b) = state.workspace.baseline_file_manifest.iter().find(|b| {
                    let bp = b.path.replace('\\', "/");
                    bp.trim_start_matches("./") == norm_file
                }) {
                    // Baseline-present: MUST exist as regular non-symlink file matching content digest and mode
                    expected_package_files.insert(norm_file.clone());
                    if !snap_file.exists() {
                        return Err(format!(
                            "Baseline-present allowlisted file missing from snapshot: {norm_file}"
                        ));
                    }
                    let meta = std::fs::symlink_metadata(&snap_file)
                        .map_err(|e| format!("Failed to read metadata for {norm_file}: {e}"))?;
                    if meta.file_type().is_symlink() {
                        return Err(format!(
                            "Symlink forbidden in snapshot package: {}",
                            snap_file.display()
                        ));
                    }
                    if !meta.is_file() {
                        return Err(format!("Non-regular file in snapshot package: {norm_file}"));
                    }
                    let actual_mode = if meta.permissions().readonly() {
                        "100444".to_string()
                    } else {
                        "100644".to_string()
                    };
                    if actual_mode != b.file_mode {
                        return Err(format!("Snapshot file mode mismatch for {norm_file}: computed {actual_mode}, expected baseline {}", b.file_mode));
                    }
                    let b_bytes = std::fs::read(&snap_file)
                        .map_err(|e| format!("Failed to read {norm_file}: {e}"))?;
                    let h = sha256_hex(&b_bytes);
                    if h != b.content_sha256 {
                        return Err(format!("Snapshot file content hash mismatch for {norm_file}: computed {h}, expected baseline {}", b.content_sha256));
                    }
                } else {
                    // Baseline-absent: must NOT exist in PlanReview snapshot
                    if snap_file.exists() {
                        return Err(format!("Baseline-absent file must not exist in PlanReview snapshot: {norm_file}"));
                    }
                }
            }
        }
        AutoConvergeState::ImplementationReview => {
            // Read and parse run_delta.json ONCE
            let delta_path = snapshot_dir.join("run_delta.json");
            check_injected_fault(InjectedSnapshotFault::ReadDelta)?;
            let delta_bytes = std::fs::read(&delta_path)
                .map_err(|e| format!("Failed to read run_delta.json: {e}"))?;
            check_injected_fault(InjectedSnapshotFault::ParseDelta)?;
            let delta: Vec<RunDeltaEntry> = serde_json::from_slice(&delta_bytes)
                .map_err(|e| format!("Invalid JSON in run_delta.json: {e}"))?;

            let computed_diff = compute_diff_digest(&delta);
            let expected_diff = state
                .current_diff_digest
                .as_deref()
                .ok_or_else(|| "current_diff_digest missing in authenticated state".to_string())?;
            if computed_diff != expected_diff {
                return Err(format!("run_delta.json diff digest mismatch: computed {computed_diff}, expected {expected_diff}"));
            }

            // Exactly one delta entry per runtime allowlist path; reject duplicate or extraneous entries
            let mut delta_paths = BTreeSet::new();
            for d in &delta {
                let norm_dp = d.path.replace('\\', "/");
                let norm_dp = norm_dp.trim_start_matches("./").to_string();
                if !delta_paths.insert(norm_dp.clone()) {
                    return Err(format!("Duplicate entry in run_delta.json: {norm_dp}"));
                }
                if !runtime_allowlist_set.contains(&norm_dp) {
                    return Err(format!(
                        "run_delta.json contains entry outside runtime_allowlist: {norm_dp}"
                    ));
                }
            }
            for p in &state.runtime_allowlist {
                let norm_p = p.replace('\\', "/");
                let norm_p = norm_p.trim_start_matches("./").to_string();
                if !delta_paths.contains(&norm_p) {
                    return Err(format!(
                        "Missing run_delta.json entry for allowlisted path: {norm_p}"
                    ));
                }
            }

            // Validate copied files and represented deletions
            for d in &delta {
                let norm_file = d.path.replace('\\', "/");
                let norm_file = norm_file.trim_start_matches("./").to_string();
                let snap_file = snapshot_dir.join(&norm_file);
                let is_deleted = d.current_sha256
                    == "0000000000000000000000000000000000000000000000000000000000000000";
                if is_deleted {
                    if d.current_mode != "000000" {
                        return Err(format!(
                            "Deleted file must have mode 000000 in delta: {norm_file}"
                        ));
                    }
                    if snap_file.exists() {
                        return Err(format!(
                            "Deleted file must not exist in snapshot: {norm_file}"
                        ));
                    }
                } else {
                    expected_package_files.insert(norm_file.clone());
                    if !snap_file.exists() {
                        return Err(format!(
                            "Allowlisted file missing from snapshot: {norm_file}"
                        ));
                    }
                    let meta = std::fs::symlink_metadata(&snap_file)
                        .map_err(|e| format!("Failed to read metadata for {norm_file}: {e}"))?;
                    if meta.file_type().is_symlink() {
                        return Err(format!(
                            "Symlink forbidden in snapshot package: {}",
                            snap_file.display()
                        ));
                    }
                    if !meta.is_file() {
                        return Err(format!("Non-regular file in snapshot package: {norm_file}"));
                    }
                    let actual_mode = if meta.permissions().readonly() {
                        "100444".to_string()
                    } else {
                        "100644".to_string()
                    };
                    if actual_mode != d.current_mode {
                        return Err(format!("Snapshot file mode mismatch for {norm_file}: computed {actual_mode}, expected delta {}", d.current_mode));
                    }
                    let b_bytes = std::fs::read(&snap_file)
                        .map_err(|e| format!("Failed to read {norm_file}: {e}"))?;
                    let h = sha256_hex(&b_bytes);
                    if h != d.current_sha256 {
                        return Err(format!("Snapshot file content hash mismatch for {norm_file}: computed {h}, expected delta {}", d.current_sha256));
                    }
                }
            }

            // Evidence manifest
            let evidence_path = snapshot_dir.join("evidence.json");
            let evidence_bytes = std::fs::read(&evidence_path)
                .map_err(|e| format!("Failed to read evidence.json: {e}"))?;
            let evidence: Vec<VerificationEvidence> = serde_json::from_slice(&evidence_bytes)
                .map_err(|e| format!("Invalid JSON in evidence.json: {e}"))?;
            let compact_evidence = serde_json::to_string(&evidence).map_err(|e| e.to_string())?;
            let computed_ev = sha256_hex(compact_evidence.as_bytes());
            let expected_ev = state
                .current_evidence_manifest_digest
                .as_deref()
                .ok_or_else(|| {
                    "current_evidence_manifest_digest missing in authenticated state".to_string()
                })?;
            if computed_ev != expected_ev {
                return Err(format!("evidence manifest digest mismatch: computed {computed_ev}, expected {expected_ev}"));
            }
        }
        AutoConvergeState::CorrectivePlanReview => {
            // Unresolved findings
            let findings_path = snapshot_dir.join("unresolved_findings.json");
            let findings_bytes = std::fs::read(&findings_path)
                .map_err(|e| format!("Failed to read unresolved_findings.json: {e}"))?;
            let findings: Vec<TrackedFinding> = serde_json::from_slice(&findings_bytes)
                .map_err(|e| format!("Invalid JSON in unresolved_findings.json: {e}"))?;
            let expected_findings: Vec<TrackedFinding> = state
                .findings
                .iter()
                .filter(|f| !f.resolved)
                .cloned()
                .collect();
            if findings != expected_findings {
                return Err("Snapshot unresolved findings mismatch: unresolved_findings.json does not match active unresolved findings in state".to_string());
            }

            // Compute delta ONCE and propagate errors fail-closed
            check_injected_fault(InjectedSnapshotFault::ComputeDiff)?;
            let delta = compute_run_delta_manifest(
                ws_path,
                &state.runtime_allowlist,
                &state.workspace.baseline_file_manifest,
            )
            .map_err(|e| format!("Failed to compute run delta for CorrectivePlanReview: {e}"))?;

            let computed_diff = compute_diff_digest(&delta);
            let expected_diff = state
                .current_diff_digest
                .as_deref()
                .ok_or_else(|| "current_diff_digest missing in authenticated state".to_string())?;
            if computed_diff != expected_diff {
                return Err(format!("diff digest mismatch in CorrectivePlanReview: computed {computed_diff}, expected {expected_diff}"));
            }

            // Exactly one delta entry per runtime allowlist path; reject duplicate or extraneous entries
            let mut delta_paths = BTreeSet::new();
            for d in &delta {
                let norm_dp = d.path.replace('\\', "/");
                let norm_dp = norm_dp.trim_start_matches("./").to_string();
                if !delta_paths.insert(norm_dp.clone()) {
                    return Err(format!("Duplicate entry in computed delta: {norm_dp}"));
                }
                if !runtime_allowlist_set.contains(&norm_dp) {
                    return Err(format!(
                        "Computed delta contains entry outside runtime_allowlist: {norm_dp}"
                    ));
                }
            }
            for p in &state.runtime_allowlist {
                let norm_p = p.replace('\\', "/");
                let norm_p = norm_p.trim_start_matches("./").to_string();
                if !delta_paths.contains(&norm_p) {
                    return Err(format!(
                        "Missing computed delta entry for allowlisted path: {norm_p}"
                    ));
                }
            }

            // Validate copied files and represented deletions
            for d in &delta {
                let norm_file = d.path.replace('\\', "/");
                let norm_file = norm_file.trim_start_matches("./").to_string();
                let snap_file = snapshot_dir.join(&norm_file);
                let is_deleted = d.current_sha256
                    == "0000000000000000000000000000000000000000000000000000000000000000";
                if is_deleted {
                    if d.current_mode != "000000" {
                        return Err(format!(
                            "Deleted file must have mode 000000 in delta: {norm_file}"
                        ));
                    }
                    if snap_file.exists() {
                        return Err(format!(
                            "Deleted file must not exist in snapshot: {norm_file}"
                        ));
                    }
                } else {
                    expected_package_files.insert(norm_file.clone());
                    if !snap_file.exists() {
                        return Err(format!(
                            "Allowlisted file missing from snapshot: {norm_file}"
                        ));
                    }
                    let meta = std::fs::symlink_metadata(&snap_file)
                        .map_err(|e| format!("Failed to read metadata for {norm_file}: {e}"))?;
                    if meta.file_type().is_symlink() {
                        return Err(format!(
                            "Symlink forbidden in snapshot package: {}",
                            snap_file.display()
                        ));
                    }
                    if !meta.is_file() {
                        return Err(format!("Non-regular file in snapshot package: {norm_file}"));
                    }
                    let actual_mode = if meta.permissions().readonly() {
                        "100444".to_string()
                    } else {
                        "100644".to_string()
                    };
                    if actual_mode != d.current_mode {
                        return Err(format!("Snapshot file mode mismatch for {norm_file}: computed {actual_mode}, expected delta {}", d.current_mode));
                    }
                    let b_bytes = std::fs::read(&snap_file)
                        .map_err(|e| format!("Failed to read {norm_file}: {e}"))?;
                    let h = sha256_hex(&b_bytes);
                    if h != d.current_sha256 {
                        return Err(format!("Snapshot file content hash mismatch for {norm_file}: computed {h}, expected delta {}", d.current_sha256));
                    }
                }
            }
        }
        _ => {}
    }

    // 7. Ensure no unexpected files exist in snapshot_dir
    for f in &files {
        if !expected_package_files.contains(f) {
            return Err(format!("Unexpected file in snapshot package: {f}"));
        }
    }

    Ok(())
}

/// Materializes a complete, immutable review package for PlanReview (§23.1.2, §23.4.3, §24.4.3, §25.3, §26.3).
pub fn materialize_plan_review_snapshot(
    state: &AutoConvergeRunState,
    ws_path: &Path,
    snapshot_dir: &Path,
) -> Result<PathBuf, String> {
    check_injected_fault(InjectedSnapshotFault::CreateDir)?;
    std::fs::create_dir_all(snapshot_dir).map_err(|e| e.to_string())?;

    // 1. Candidate plan markdown - never synthesize a placeholder plan
    let plan_source = ws_path.join(".plan").join(format!("{}.md", state.run_id));
    if !plan_source.exists() {
        return Err(format!(
            "Required candidate plan file missing: {}",
            plan_source.display()
        ));
    }
    check_injected_fault(InjectedSnapshotFault::ReadPlan)?;
    let plan_content = std::fs::read_to_string(&plan_source).map_err(|e| {
        format!(
            "Failed to read candidate plan from {}: {e}",
            plan_source.display()
        )
    })?;
    if plan_content.trim().is_empty() {
        return Err(format!(
            "Candidate plan file is empty: {}",
            plan_source.display()
        ));
    }

    let target_plan = snapshot_dir.join("candidate_plan.md");
    check_injected_fault(InjectedSnapshotFault::WritePlan)?;
    std::fs::write(&target_plan, &plan_content).map_err(|e| e.to_string())?;

    // 2. Frozen allowlist
    let allowlist_file = snapshot_dir.join("allowlist.json");
    let allowlist_json =
        serde_json::to_string_pretty(&state.runtime_allowlist).map_err(|e| e.to_string())?;
    check_injected_fault(InjectedSnapshotFault::WriteAllowlist)?;
    std::fs::write(&allowlist_file, &allowlist_json).map_err(|e| e.to_string())?;

    // 3. Stage binding manifest
    let manifest_file = snapshot_dir.join("review_manifest.json");
    let manifest_json = serde_json::json!({
        "target_type": "plan",
        "run_id": state.run_id,
        "state_sequence": state.state_sequence,
        "candidate_plan_digest": state.plan_lineage.candidate_plan_digest,
        "allowlist_digest": state.allowlist_digest,
        "policy_profile_digest": state.policy_profile_digest,
        "baseline_workspace_digest": state.workspace.baseline_workspace_digest,
    });
    check_injected_fault(InjectedSnapshotFault::SerializeManifest)?;
    let manifest_str = serde_json::to_string_pretty(&manifest_json).map_err(|e| e.to_string())?;
    check_injected_fault(InjectedSnapshotFault::WriteManifest)?;
    std::fs::write(&manifest_file, manifest_str).map_err(|e| e.to_string())?;

    // 4. Copy baseline-present allowlisted files from workspace into snapshot (fail closed on any error)
    for file in &state.runtime_allowlist {
        let norm_file = file.replace('\\', "/");
        let norm_file = norm_file.trim_start_matches("./");
        if let Some(_b) = state.workspace.baseline_file_manifest.iter().find(|b| {
            let bp = b.path.replace('\\', "/");
            bp.trim_start_matches("./") == norm_file
        }) {
            let src = ws_path.join(file);
            if !src.exists() || !src.is_file() {
                return Err(format!(
                    "Baseline-present allowlisted file missing from workspace: {file}"
                ));
            }
            let dst = snapshot_dir.join(file);
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "Failed to create snapshot directory {}: {e}",
                        parent.display()
                    )
                })?;
            }
            check_injected_fault(InjectedSnapshotFault::CopyFile)?;
            std::fs::copy(&src, &dst).map_err(|e| {
                format!(
                    "Failed to copy allowlisted file {} to snapshot: {e}",
                    src.display()
                )
            })?;
        }
    }

    Ok(target_plan)
}

/// Materializes a complete, immutable review package for ImplementationReview (§23.1.2, §23.4.3, §24.4.3, §25.3, §26.3).
pub fn materialize_implementation_review_snapshot(
    state: &AutoConvergeRunState,
    ws_path: &Path,
    snapshot_dir: &Path,
) -> Result<PathBuf, String> {
    check_injected_fault(InjectedSnapshotFault::CreateDir)?;
    std::fs::create_dir_all(snapshot_dir).map_err(|e| e.to_string())?;

    // 1. Active plan markdown - never synthesize a placeholder plan
    let plan_source = ws_path.join(".plan").join(format!("{}.md", state.run_id));
    if !plan_source.exists() {
        return Err(format!(
            "Required active plan file missing: {}",
            plan_source.display()
        ));
    }
    check_injected_fault(InjectedSnapshotFault::ReadPlan)?;
    let plan_content = std::fs::read_to_string(&plan_source).map_err(|e| {
        format!(
            "Failed to read active plan from {}: {e}",
            plan_source.display()
        )
    })?;
    if plan_content.trim().is_empty() {
        return Err(format!(
            "Active plan file is empty: {}",
            plan_source.display()
        ));
    }
    let target_plan = snapshot_dir.join("active_plan.md");
    check_injected_fault(InjectedSnapshotFault::WritePlan)?;
    std::fs::write(&target_plan, &plan_content).map_err(|e| e.to_string())?;

    // 2. Frozen allowlist
    let allowlist_file = snapshot_dir.join("allowlist.json");
    let allowlist_json =
        serde_json::to_string_pretty(&state.runtime_allowlist).map_err(|e| e.to_string())?;
    check_injected_fault(InjectedSnapshotFault::WriteAllowlist)?;
    std::fs::write(&allowlist_file, &allowlist_json).map_err(|e| e.to_string())?;

    // 3. Run delta manifest - propagate calculation failure fail-closed
    let delta_file = snapshot_dir.join("run_delta.json");
    check_injected_fault(InjectedSnapshotFault::ComputeDiff)?;
    let delta = compute_run_delta_manifest(
        ws_path,
        &state.runtime_allowlist,
        &state.workspace.baseline_file_manifest,
    )
    .map_err(|e| format!("Failed to compute run delta manifest: {e}"))?;
    let delta_str = serde_json::to_string_pretty(&delta).map_err(|e| e.to_string())?;
    check_injected_fault(InjectedSnapshotFault::WriteDiff)?;
    std::fs::write(&delta_file, delta_str).map_err(|e| e.to_string())?;

    // 4. Evidence manifest
    let evidence_file = snapshot_dir.join("evidence.json");
    check_injected_fault(InjectedSnapshotFault::SerializeEvidence)?;
    let evidence_str = serde_json::to_string_pretty(&state.evidence).map_err(|e| e.to_string())?;
    check_injected_fault(InjectedSnapshotFault::WriteEvidence)?;
    std::fs::write(&evidence_file, evidence_str).map_err(|e| e.to_string())?;

    // 5. Stage binding manifest
    let manifest_file = snapshot_dir.join("review_manifest.json");
    let manifest_json = serde_json::json!({
        "target_type": "implementation_diff",
        "run_id": state.run_id,
        "state_sequence": state.state_sequence,
        "root_plan_digest": state.plan_lineage.root_plan_digest,
        "active_plan_digest": state.plan_lineage.active_plan_digest,
        "diff_digest": state.current_diff_digest,
        "evidence_manifest_digest": state.current_evidence_manifest_digest,
        "allowlist_digest": state.allowlist_digest,
        "policy_profile_digest": state.policy_profile_digest,
    });
    check_injected_fault(InjectedSnapshotFault::SerializeManifest)?;
    let manifest_str = serde_json::to_string_pretty(&manifest_json).map_err(|e| e.to_string())?;
    check_injected_fault(InjectedSnapshotFault::WriteManifest)?;
    std::fs::write(&manifest_file, manifest_str).map_err(|e| e.to_string())?;

    // 6. Copy modified/present allowlisted files into snapshot (fail closed on any error)
    for entry in &delta {
        if entry.current_sha256
            != "0000000000000000000000000000000000000000000000000000000000000000"
        {
            let src = ws_path.join(&entry.path);
            if !src.exists() || !src.is_file() {
                return Err(format!(
                    "Allowlisted file missing from workspace: {}",
                    entry.path
                ));
            }
            let dst = snapshot_dir.join(&entry.path);
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "Failed to create snapshot directory {}: {e}",
                        parent.display()
                    )
                })?;
            }
            check_injected_fault(InjectedSnapshotFault::CopyFile)?;
            std::fs::copy(&src, &dst).map_err(|e| {
                format!(
                    "Failed to copy allowlisted file {} to snapshot: {e}",
                    src.display()
                )
            })?;
        }
    }

    Ok(target_plan)
}

/// Materializes a complete, immutable review package for CorrectivePlanReview (§23.1.2, §23.4.3, §24.4.3, §25.3, §26.3).
pub fn materialize_corrective_plan_review_snapshot(
    state: &AutoConvergeRunState,
    ws_path: &Path,
    snapshot_dir: &Path,
) -> Result<PathBuf, String> {
    check_injected_fault(InjectedSnapshotFault::CreateDir)?;
    std::fs::create_dir_all(snapshot_dir).map_err(|e| e.to_string())?;

    // 1. Candidate corrective plan markdown - never synthesize a placeholder plan
    let plan_source = ws_path.join(".plan").join(format!("{}.md", state.run_id));
    if !plan_source.exists() {
        return Err(format!(
            "Required candidate corrective plan file missing: {}",
            plan_source.display()
        ));
    }
    check_injected_fault(InjectedSnapshotFault::ReadPlan)?;
    let plan_content = std::fs::read_to_string(&plan_source).map_err(|e| {
        format!(
            "Failed to read candidate corrective plan from {}: {e}",
            plan_source.display()
        )
    })?;
    if plan_content.trim().is_empty() {
        return Err(format!(
            "Candidate corrective plan file is empty: {}",
            plan_source.display()
        ));
    }
    let target_plan = snapshot_dir.join("candidate_corrective_plan.md");
    check_injected_fault(InjectedSnapshotFault::WritePlan)?;
    std::fs::write(&target_plan, &plan_content).map_err(|e| e.to_string())?;

    // 2. Unresolved findings
    let findings_file = snapshot_dir.join("unresolved_findings.json");
    let active_findings: Vec<_> = state.findings.iter().filter(|f| !f.resolved).collect();
    check_injected_fault(InjectedSnapshotFault::SerializeFindings)?;
    let findings_str = serde_json::to_string_pretty(&active_findings).map_err(|e| e.to_string())?;
    check_injected_fault(InjectedSnapshotFault::WriteFindings)?;
    std::fs::write(&findings_file, findings_str).map_err(|e| e.to_string())?;

    // 3. Frozen allowlist
    let allowlist_file = snapshot_dir.join("allowlist.json");
    let allowlist_json =
        serde_json::to_string_pretty(&state.runtime_allowlist).map_err(|e| e.to_string())?;
    check_injected_fault(InjectedSnapshotFault::WriteAllowlist)?;
    std::fs::write(&allowlist_file, &allowlist_json).map_err(|e| e.to_string())?;

    // 4. Stage binding manifest
    let manifest_file = snapshot_dir.join("review_manifest.json");
    let manifest_json = serde_json::json!({
        "target_type": "corrective_plan",
        "run_id": state.run_id,
        "state_sequence": state.state_sequence,
        "root_plan_digest": state.plan_lineage.root_plan_digest,
        "active_plan_digest": state.plan_lineage.active_plan_digest,
        "candidate_plan_digest": state.plan_lineage.candidate_plan_digest,
        "diff_digest": state.current_diff_digest,
        "allowlist_digest": state.allowlist_digest,
        "policy_profile_digest": state.policy_profile_digest,
        "baseline_workspace_digest": state.workspace.baseline_workspace_digest,
    });
    check_injected_fault(InjectedSnapshotFault::SerializeManifest)?;
    let manifest_str = serde_json::to_string_pretty(&manifest_json).map_err(|e| e.to_string())?;
    check_injected_fault(InjectedSnapshotFault::WriteManifest)?;
    std::fs::write(&manifest_file, manifest_str).map_err(|e| e.to_string())?;

    // 5. Compute current delta and copy present allowlisted files into snapshot (fail closed on any error)
    check_injected_fault(InjectedSnapshotFault::ComputeDiff)?;
    let delta = compute_run_delta_manifest(
        ws_path,
        &state.runtime_allowlist,
        &state.workspace.baseline_file_manifest,
    )
    .map_err(|e| format!("Failed to compute run delta for corrective snapshot: {e}"))?;
    for entry in &delta {
        if entry.current_sha256
            != "0000000000000000000000000000000000000000000000000000000000000000"
        {
            let src = ws_path.join(&entry.path);
            if !src.exists() || !src.is_file() {
                return Err(format!(
                    "Allowlisted file missing from workspace: {}",
                    entry.path
                ));
            }
            let dst = snapshot_dir.join(&entry.path);
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "Failed to create snapshot directory {}: {e}",
                        parent.display()
                    )
                })?;
            }
            check_injected_fault(InjectedSnapshotFault::CopyFile)?;
            std::fs::copy(&src, &dst).map_err(|e| {
                format!(
                    "Failed to copy allowlisted file {} to snapshot: {e}",
                    src.display()
                )
            })?;
        }
    }

    Ok(target_plan)
}

/// Advances the Phase A state machine by one transition using the provided agent and command runners.
/// Every state transition verifies bounds and persists authenticated state before any subsequent launch.
fn run_phase_a_step(
    state: &mut AutoConvergeRunState,
    agent_runner: &dyn AgentCommandRunner,
    _git_runner: &dyn CommandRunner,
    state_path: &Path,
) -> Result<(), String> {
    if state.is_terminal || state.is_paused || state.current_state == AutoConvergeState::Blocked {
        return Ok(());
    }

    let ws_path = PathBuf::from(&state.workspace.working_dir);
    let now_iso = chrono::Utc::now().to_rfc3339();
    let case_insensitive_fs = cfg!(windows);

    match state.current_state {
        AutoConvergeState::PlanCreate | AutoConvergeState::PlanRevise => {
            let session_id = format!("{}-planner-{}", state.run_id, state.state_sequence + 1);
            let plan_path = ws_path.join(".plan").join(format!("{}.md", state.run_id));
            let req = RoleExecutionRequest {
                role: AutoConvergeRole::Planner,
                session_id: session_id.clone(),
                workspace_root: ws_path.clone(),
                snapshot_dir: None,
                plan_path: plan_path.clone(),
                runtime_allowlist: state.runtime_allowlist.clone(),
                input_prompt: "Create an implementation plan satisfying auto-converge requirements"
                    .to_string(),
                timeout_seconds: 600,
            };
            let resp = match agent_runner.execute_role_session(&req) {
                Ok(r) => r,
                Err(e) => {
                    halt_blocked(
                        state,
                        BlockedKind::FatalInvariant,
                        &format!("Planner execution failed: {e}"),
                    );
                    save_run_state_authenticated(state_path, state)?;
                    return Err(e);
                }
            };
            let plan_content = if plan_path.exists() {
                std::fs::read_to_string(&plan_path).unwrap_or(resp.stdout)
            } else {
                resp.stdout
            };
            if let Err(e) = submit_draft_plan(
                state,
                &session_id,
                &plan_content,
                case_insensitive_fs,
                &now_iso,
            ) {
                save_run_state_authenticated(state_path, state)?;
                return Err(e);
            }
            save_run_state_authenticated(state_path, state)?;
        }
        AutoConvergeState::PlanReview => {
            let session_id = format!(
                "{}-plan-reviewer-{}",
                state.run_id,
                state.state_sequence + 1
            );
            let snapshot_dir = std::env::temp_dir()
                .join("poverello")
                .join("snapshots")
                .join(&state.run_id)
                .join(format!("{}", state.state_sequence + 1));

            let snap_plan = match materialize_plan_review_snapshot(state, &ws_path, &snapshot_dir) {
                Ok(p) => p,
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&snapshot_dir);
                    halt_blocked(
                        state,
                        BlockedKind::FatalInvariant,
                        &format!("Failed to materialize plan review snapshot: {e}"),
                    );
                    save_run_state_authenticated(state_path, state)?;
                    return Err(e);
                }
            };
            if let Err(e) = validate_snapshot_against_authenticated_state(
                &snapshot_dir,
                state,
                AutoConvergeState::PlanReview,
                &ws_path,
                case_insensitive_fs,
            ) {
                let _ = std::fs::remove_dir_all(&snapshot_dir);
                halt_blocked(
                    state,
                    BlockedKind::FatalInvariant,
                    &format!("Plan review snapshot pre-launch verification failed: {e}"),
                );
                save_run_state_authenticated(state_path, state)?;
                return Err(e);
            }
            if let Err(e) = check_injected_fault(InjectedSnapshotFault::PreLaunchHash) {
                let _ = std::fs::remove_dir_all(&snapshot_dir);
                halt_blocked(
                    state,
                    BlockedKind::FatalInvariant,
                    &format!("Plan review pre-launch hash computation failed: {e}"),
                );
                save_run_state_authenticated(state_path, state)?;
                return Err(e);
            }
            let pre_hash = match compute_filesystem_manifest_hash(&snapshot_dir) {
                Ok(h) => h,
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&snapshot_dir);
                    halt_blocked(
                        state,
                        BlockedKind::FatalInvariant,
                        &format!("Plan review pre-launch hash computation failed: {e}"),
                    );
                    save_run_state_authenticated(state_path, state)?;
                    return Err(e.to_string());
                }
            };

            let req = RoleExecutionRequest {
                role: AutoConvergeRole::Reviewer,
                session_id: session_id.clone(),
                workspace_root: snapshot_dir.clone(),
                snapshot_dir: Some(snapshot_dir.clone()),
                plan_path: snap_plan,
                runtime_allowlist: state.runtime_allowlist.clone(),
                input_prompt: "Review candidate plan against requirements and scope".to_string(),
                timeout_seconds: 600,
            };
            let resp = match agent_runner.execute_role_session(&req) {
                Ok(r) => r,
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&snapshot_dir);
                    halt_blocked(
                        state,
                        BlockedKind::FatalInvariant,
                        &format!("Plan review execution failed: {e}"),
                    );
                    save_run_state_authenticated(state_path, state)?;
                    return Err(e);
                }
            };
            if let Err(e) = check_injected_fault(InjectedSnapshotFault::PostLaunchHash) {
                let _ = std::fs::remove_dir_all(&snapshot_dir);
                let msg = format!("Plan review post-launch hash calculation failed: {e}");
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }
            let post_hash = match compute_filesystem_manifest_hash(&snapshot_dir) {
                Ok(h) => h,
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&snapshot_dir);
                    let msg = format!("Plan review post-launch hash calculation failed: {e}");
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            };
            let _ = std::fs::remove_dir_all(&snapshot_dir);

            if post_hash != pre_hash {
                let msg = format!(
                    "Post-review snapshot integrity verification failed: snapshot modified during review (pre: {pre_hash}, post: {post_hash})"
                );
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            let raw_verdict = match resp.verdict_json {
                Some(v) => v,
                None => {
                    let msg = "Reviewer response did not contain structured verdict".to_string();
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            };

            if let Err(e) = submit_review_verdict(
                state,
                &session_id,
                &raw_verdict,
                &pre_hash,
                &post_hash,
                None,
                None,
                case_insensitive_fs,
                &now_iso,
            ) {
                save_run_state_authenticated(state_path, state)?;
                return Err(e);
            }
            save_run_state_authenticated(state_path, state)?;
        }
        AutoConvergeState::PlanApproved => {
            let session_id = format!("{}-implementer-{}", state.run_id, state.state_sequence + 1);
            let current_manifest = capture_baseline_file_manifest(&ws_path, &BTreeMap::new())?;
            if let Err(e) = freeze_approved_plan_and_begin_implement(
                state,
                &current_manifest,
                &session_id,
                &now_iso,
            ) {
                save_run_state_authenticated(state_path, state)?;
                return Err(e);
            }
            save_run_state_authenticated(state_path, state)?;
        }
        AutoConvergeState::Implement | AutoConvergeState::CorrectiveImplement => {
            let session_id = format!("{}-implementer-{}", state.run_id, state.state_sequence + 1);
            let req = RoleExecutionRequest {
                role: AutoConvergeRole::Implementer,
                session_id: session_id.clone(),
                workspace_root: ws_path.clone(),
                snapshot_dir: None,
                plan_path: ws_path.join(".plan").join(format!("{}.md", state.run_id)),
                runtime_allowlist: state.runtime_allowlist.clone(),
                input_prompt: "Implement approved changes strictly within runtime_allowlist"
                    .to_string(),
                timeout_seconds: 1200,
            };
            let resp = match agent_runner.execute_role_session(&req) {
                Ok(r) => r,
                Err(e) => {
                    halt_blocked(
                        state,
                        BlockedKind::FatalInvariant,
                        &format!("Implementer execution failed: {e}"),
                    );
                    save_run_state_authenticated(state_path, state)?;
                    return Err(e);
                }
            };
            let delta = compute_run_delta_manifest(
                &ws_path,
                &state.runtime_allowlist,
                &state.workspace.baseline_file_manifest,
            )?;
            let touched_files: Vec<String> = delta.iter().map(|d| d.path.clone()).collect();
            let evidence = vec![VerificationEvidence {
                command: "local-verification".to_string(),
                exit_code: resp.exit_code,
                stdout_summary: resp.stdout.chars().take(200).collect(),
                passed: resp.exit_code == 0,
            }];
            if let Err(e) = complete_implementation_step(
                state,
                &session_id,
                &touched_files,
                &delta,
                evidence,
                case_insensitive_fs,
                &now_iso,
            ) {
                save_run_state_authenticated(state_path, state)?;
                return Err(e);
            }
            save_run_state_authenticated(state_path, state)?;
        }
        AutoConvergeState::ImplementationReview => {
            let session_id = format!(
                "{}-impl-reviewer-{}",
                state.run_id,
                state.state_sequence + 1
            );
            let snapshot_dir = std::env::temp_dir()
                .join("poverello")
                .join("snapshots")
                .join(&state.run_id)
                .join(format!("{}", state.state_sequence + 1));

            let snap_plan =
                match materialize_implementation_review_snapshot(state, &ws_path, &snapshot_dir) {
                    Ok(p) => p,
                    Err(e) => {
                        let _ = std::fs::remove_dir_all(&snapshot_dir);
                        halt_blocked(
                            state,
                            BlockedKind::FatalInvariant,
                            &format!("Failed to materialize implementation review snapshot: {e}"),
                        );
                        save_run_state_authenticated(state_path, state)?;
                        return Err(e);
                    }
                };
            if let Err(e) = validate_snapshot_against_authenticated_state(
                &snapshot_dir,
                state,
                AutoConvergeState::ImplementationReview,
                &ws_path,
                case_insensitive_fs,
            ) {
                let _ = std::fs::remove_dir_all(&snapshot_dir);
                halt_blocked(
                    state,
                    BlockedKind::FatalInvariant,
                    &format!("Implementation review snapshot pre-launch verification failed: {e}"),
                );
                save_run_state_authenticated(state_path, state)?;
                return Err(e);
            }
            if let Err(e) = check_injected_fault(InjectedSnapshotFault::PreLaunchHash) {
                let _ = std::fs::remove_dir_all(&snapshot_dir);
                halt_blocked(
                    state,
                    BlockedKind::FatalInvariant,
                    &format!("Implementation review pre-launch hash computation failed: {e}"),
                );
                save_run_state_authenticated(state_path, state)?;
                return Err(e);
            }
            let pre_hash = match compute_filesystem_manifest_hash(&snapshot_dir) {
                Ok(h) => h,
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&snapshot_dir);
                    halt_blocked(
                        state,
                        BlockedKind::FatalInvariant,
                        &format!("Implementation review pre-launch hash computation failed: {e}"),
                    );
                    save_run_state_authenticated(state_path, state)?;
                    return Err(e.to_string());
                }
            };

            let req = RoleExecutionRequest {
                role: AutoConvergeRole::Reviewer,
                session_id: session_id.clone(),
                workspace_root: snapshot_dir.clone(),
                snapshot_dir: Some(snapshot_dir.clone()),
                plan_path: snap_plan,
                runtime_allowlist: state.runtime_allowlist.clone(),
                input_prompt: "Review implementation diff against approved plan and allowlist"
                    .to_string(),
                timeout_seconds: 600,
            };
            let resp = match agent_runner.execute_role_session(&req) {
                Ok(r) => r,
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&snapshot_dir);
                    halt_blocked(
                        state,
                        BlockedKind::FatalInvariant,
                        &format!("Implementation review execution failed: {e}"),
                    );
                    save_run_state_authenticated(state_path, state)?;
                    return Err(e);
                }
            };
            if let Err(e) = check_injected_fault(InjectedSnapshotFault::PostLaunchHash) {
                let _ = std::fs::remove_dir_all(&snapshot_dir);
                let msg = format!("Implementation review post-launch hash calculation failed: {e}");
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }
            let post_hash = match compute_filesystem_manifest_hash(&snapshot_dir) {
                Ok(h) => h,
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&snapshot_dir);
                    let msg =
                        format!("Implementation review post-launch hash calculation failed: {e}");
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            };
            let _ = std::fs::remove_dir_all(&snapshot_dir);

            if post_hash != pre_hash {
                let msg = format!(
                    "Post-review snapshot integrity verification failed: snapshot modified during review (pre: {pre_hash}, post: {post_hash})"
                );
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            let raw_verdict = match resp.verdict_json {
                Some(v) => v,
                None => {
                    let msg = "Reviewer response did not contain structured verdict".to_string();
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            };

            if let Err(e) = submit_review_verdict(
                state,
                &session_id,
                &raw_verdict,
                &pre_hash,
                &post_hash,
                None,
                None,
                case_insensitive_fs,
                &now_iso,
            ) {
                save_run_state_authenticated(state_path, state)?;
                return Err(e);
            }
            save_run_state_authenticated(state_path, state)?;
        }
        AutoConvergeState::CorrectivePlanCreate | AutoConvergeState::CorrectivePlanRevise => {
            let session_id = format!(
                "{}-corrective-planner-{}",
                state.run_id,
                state.state_sequence + 1
            );
            let plan_path = ws_path.join(".plan").join(format!("{}.md", state.run_id));
            let req = RoleExecutionRequest {
                role: AutoConvergeRole::Planner,
                session_id: session_id.clone(),
                workspace_root: ws_path.clone(),
                snapshot_dir: None,
                plan_path: plan_path.clone(),
                runtime_allowlist: state.runtime_allowlist.clone(),
                input_prompt: "Create a corrective plan addressing unresolved review findings"
                    .to_string(),
                timeout_seconds: 600,
            };
            let resp = match agent_runner.execute_role_session(&req) {
                Ok(r) => r,
                Err(e) => {
                    halt_blocked(
                        state,
                        BlockedKind::FatalInvariant,
                        &format!("Corrective planner execution failed: {e}"),
                    );
                    save_run_state_authenticated(state_path, state)?;
                    return Err(e);
                }
            };
            let plan_content = if plan_path.exists() {
                std::fs::read_to_string(&plan_path).unwrap_or(resp.stdout)
            } else {
                resp.stdout
            };
            if let Err(e) = submit_corrective_plan(
                state,
                &session_id,
                &plan_content,
                case_insensitive_fs,
                &now_iso,
            ) {
                save_run_state_authenticated(state_path, state)?;
                return Err(e);
            }
            save_run_state_authenticated(state_path, state)?;
        }
        AutoConvergeState::CorrectivePlanReview => {
            let session_id = format!(
                "{}-corrective-reviewer-{}",
                state.run_id,
                state.state_sequence + 1
            );
            let snapshot_dir = std::env::temp_dir()
                .join("poverello")
                .join("snapshots")
                .join(&state.run_id)
                .join(format!("{}", state.state_sequence + 1));

            let snap_plan =
                match materialize_corrective_plan_review_snapshot(state, &ws_path, &snapshot_dir) {
                    Ok(p) => p,
                    Err(e) => {
                        let _ = std::fs::remove_dir_all(&snapshot_dir);
                        halt_blocked(
                            state,
                            BlockedKind::FatalInvariant,
                            &format!("Failed to materialize corrective plan review snapshot: {e}"),
                        );
                        save_run_state_authenticated(state_path, state)?;
                        return Err(e);
                    }
                };
            if let Err(e) = validate_snapshot_against_authenticated_state(
                &snapshot_dir,
                state,
                AutoConvergeState::CorrectivePlanReview,
                &ws_path,
                case_insensitive_fs,
            ) {
                let _ = std::fs::remove_dir_all(&snapshot_dir);
                halt_blocked(
                    state,
                    BlockedKind::FatalInvariant,
                    &format!("Corrective plan review snapshot pre-launch verification failed: {e}"),
                );
                save_run_state_authenticated(state_path, state)?;
                return Err(e);
            }
            if let Err(e) = check_injected_fault(InjectedSnapshotFault::PreLaunchHash) {
                let _ = std::fs::remove_dir_all(&snapshot_dir);
                halt_blocked(
                    state,
                    BlockedKind::FatalInvariant,
                    &format!("Corrective plan review pre-launch hash computation failed: {e}"),
                );
                save_run_state_authenticated(state_path, state)?;
                return Err(e);
            }
            let pre_hash = match compute_filesystem_manifest_hash(&snapshot_dir) {
                Ok(h) => h,
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&snapshot_dir);
                    halt_blocked(
                        state,
                        BlockedKind::FatalInvariant,
                        &format!("Corrective plan review pre-launch hash computation failed: {e}"),
                    );
                    save_run_state_authenticated(state_path, state)?;
                    return Err(e.to_string());
                }
            };

            let req = RoleExecutionRequest {
                role: AutoConvergeRole::Reviewer,
                session_id: session_id.clone(),
                workspace_root: snapshot_dir.clone(),
                snapshot_dir: Some(snapshot_dir.clone()),
                plan_path: snap_plan,
                runtime_allowlist: state.runtime_allowlist.clone(),
                input_prompt: "Review candidate corrective plan against findings and scope"
                    .to_string(),
                timeout_seconds: 600,
            };
            let resp = match agent_runner.execute_role_session(&req) {
                Ok(r) => r,
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&snapshot_dir);
                    halt_blocked(
                        state,
                        BlockedKind::FatalInvariant,
                        &format!("Corrective plan review execution failed: {e}"),
                    );
                    save_run_state_authenticated(state_path, state)?;
                    return Err(e);
                }
            };
            if let Err(e) = check_injected_fault(InjectedSnapshotFault::PostLaunchHash) {
                let _ = std::fs::remove_dir_all(&snapshot_dir);
                let msg =
                    format!("Corrective plan review post-launch hash calculation failed: {e}");
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }
            let post_hash = match compute_filesystem_manifest_hash(&snapshot_dir) {
                Ok(h) => h,
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&snapshot_dir);
                    let msg =
                        format!("Corrective plan review post-launch hash calculation failed: {e}");
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            };
            let _ = std::fs::remove_dir_all(&snapshot_dir);

            if post_hash != pre_hash {
                let msg = format!(
                    "Post-review snapshot integrity verification failed: snapshot modified during review (pre: {pre_hash}, post: {post_hash})"
                );
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            let raw_verdict = match resp.verdict_json {
                Some(v) => v,
                None => {
                    let msg = "Reviewer response did not contain structured verdict".to_string();
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            };

            if let Err(e) = submit_review_verdict(
                state,
                &session_id,
                &raw_verdict,
                &pre_hash,
                &post_hash,
                None,
                None,
                case_insensitive_fs,
                &now_iso,
            ) {
                save_run_state_authenticated(state_path, state)?;
                return Err(e);
            }
            save_run_state_authenticated(state_path, state)?;
        }
        AutoConvergeState::ImplementationApproved => {
            state.is_terminal = true;
            state.updated_at = now_iso;
            save_run_state_authenticated(state_path, state)?;
        }
        _ => {}
    }

    Ok(())
}

/// Runs Phase A steps to completion or blocked/terminal state.
fn run_phase_a_to_completion(
    state: &mut AutoConvergeRunState,
    agent_runner: &dyn AgentCommandRunner,
    git_runner: &dyn CommandRunner,
    state_path: &Path,
    max_steps: usize,
) -> Result<(), String> {
    for _ in 0..max_steps {
        if state.is_terminal || state.is_paused || state.current_state == AutoConvergeState::Blocked
        {
            break;
        }
        run_phase_a_step(state, agent_runner, git_runner, state_path)?;
    }
    Ok(())
}
