pub mod agent_runner;
pub mod codex_exec;
pub mod finding_tracker;
pub mod git_runner;
pub mod guards;
mod implementer_workspace;
pub mod isolation;
pub mod profile_store;
pub mod r3_controller;
pub mod recovery_snapshot;
pub mod release_reviewer;
pub mod state_machine;
pub mod state_store;
pub mod types;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod codex_exec_poc;

#[cfg(test)]
mod isolation_poc;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub use agent_runner::*;
pub use finding_tracker::*;
pub use git_runner::*;
pub use guards::*;
pub use isolation::*;
pub use profile_store::*;
pub use r3_controller::*;
pub use recovery_snapshot::*;
pub use release_reviewer::*;
pub use state_machine::*;
pub use state_store::*;
pub use types::*;

#[cfg(test)]
pub use state_store::{
    get_test_engine_authority_dir, set_test_engine_authority_dir, TestEngineAuthorityGuard,
};

pub fn resolve_engine_authority_dir() -> Result<PathBuf, String> {
    state_store::get_engine_authority_dir().map_err(|e| e.to_string())
}

/// Returns the engine-owned durable state path for `run_id`, stored in the OS user-data root
/// strictly outside agent-writable repository scopes (§10.1, §16.3).
pub fn engine_run_state_path(run_id: &str) -> Result<PathBuf, String> {
    let validated =
        state_store::validate_auto_converge_run_id(run_id).map_err(|error| error.to_string())?;
    let authority_dir = resolve_engine_authority_dir()?;
    let base = authority_dir
        .parent()
        .unwrap_or(Path::new("."))
        .join("engine_runs");
    let dir = state_store::engine_run_directory_path(&base, &validated)
        .map_err(|e| format!("Failed to access engine runs directory: {e}"))?;
    Ok(dir.join("run_state.json"))
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

/// Global production dispatch gate (§209.1 rule 3, §209.15 B1, §209.17 R1, R3).
/// Production dispatch remains strictly BLOCKED until Phase G verification and Phase H authorization.
/// This gate operates independently of the GUI and fails closed before any process, temp directory,
/// or run state is created.
pub const PRODUCTION_DISPATCH_ENABLED: bool = false;

#[cfg(test)]
thread_local! {
    static TEST_PRODUCTION_DISPATCH_OVERRIDE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn set_test_production_dispatch_override(enabled: bool) {
    TEST_PRODUCTION_DISPATCH_OVERRIDE.with(|cell| cell.set(enabled));
}

#[cfg(test)]
pub(crate) fn is_production_dispatch_enabled() -> bool {
    PRODUCTION_DISPATCH_ENABLED || TEST_PRODUCTION_DISPATCH_OVERRIDE.with(|cell| cell.get())
}

#[cfg(not(test))]
pub(crate) fn is_production_dispatch_enabled() -> bool {
    PRODUCTION_DISPATCH_ENABLED
}

/// Pure helper determining whether the state's upcoming role requires Reviewer preflight (§209.3 A2).
/// Only review states require Reviewer preflight; planner and implementer states do not.
pub(crate) fn next_role_requires_reviewer_preflight(state: &AutoConvergeRunState) -> bool {
    matches!(
        state.current_state,
        AutoConvergeState::PlanReview
            | AutoConvergeState::ImplementationReview
            | AutoConvergeState::CorrectivePlanReview
            | AutoConvergeState::ReleaseReview
    )
}

#[derive(Debug, PartialEq, Eq)]
pub enum RunInitializationOutcome {
    Committed(AutoConvergeRunState),
    CommittedAfterReportedError {
        state: AutoConvergeRunState,
        reported_error: String,
    },
    NotCommitted {
        save_error: String,
        cleanup_error: Option<String>,
        remaining_path: Option<PathBuf>,
    },
    Unresolved {
        save_error: String,
        read_back_error: String,
        snapshot_path: PathBuf,
        recovery_instructions: String,
    },
}

pub fn reconcile_run_state_save_error(
    state_path: &Path,
    intended_state: &AutoConvergeRunState,
    snapshot_binding: &recovery_snapshot::RecoverySnapshotBinding,
    ownership_lease: &recovery_snapshot::SnapshotOwnershipLease,
    save_error: String,
) -> RunInitializationOutcome {
    let read_back_result = guards::load_and_verify_run_state_authenticated(state_path, None);
    match read_back_result {
        Ok(persisted_state) => {
            if persisted_state.run_id == intended_state.run_id
                && persisted_state.state_sequence == intended_state.state_sequence
                && persisted_state.engine_auth_tag == intended_state.engine_auth_tag
                && persisted_state.recovery_snapshot == intended_state.recovery_snapshot
            {
                RunInitializationOutcome::CommittedAfterReportedError {
                    state: persisted_state,
                    reported_error: save_error,
                }
            } else {
                RunInitializationOutcome::Unresolved {
                    save_error,
                    read_back_error: format!(
                        "Persisted state exists but does not match intended state (run_id: {}, seq: {})",
                        persisted_state.run_id, persisted_state.state_sequence
                    ),
                    snapshot_path: PathBuf::from(&snapshot_binding.root),
                    recovery_instructions: "Inspect run state and snapshot manually; do not delete snapshot".to_string(),
                }
            }
        }
        Err(read_err) => {
            if !state_path.exists() {
                let cleanup_res =
                    recovery_snapshot::cleanup_published_snapshot_lease(ownership_lease);
                match cleanup_res {
                    Ok(()) => RunInitializationOutcome::NotCommitted {
                        save_error,
                        cleanup_error: None,
                        remaining_path: None,
                    },
                    Err(c_err) => RunInitializationOutcome::NotCommitted {
                        save_error,
                        cleanup_error: Some(c_err.to_string()),
                        remaining_path: Some(snapshot_binding.root.clone()),
                    },
                }
            } else {
                RunInitializationOutcome::Unresolved {
                    save_error,
                    read_back_error: format!(
                        "State file exists but authenticated read-back failed: {read_err}"
                    ),
                    snapshot_path: PathBuf::from(&snapshot_binding.root),
                    recovery_instructions:
                        "Inspect state file and preserve snapshot for manual recovery".to_string(),
                }
            }
        }
    }
}

pub(crate) fn prepare_and_bind_recovery_snapshot(
    state: &mut AutoConvergeRunState,
    engine_root: &Path,
    policy: &recovery_snapshot::SnapshotPolicy,
) -> Result<
    (
        recovery_snapshot::RecoverySnapshotBinding,
        recovery_snapshot::SnapshotOwnershipLease,
    ),
    recovery_snapshot::SnapshotError,
> {
    let ws_path = PathBuf::from(&state.workspace.working_dir);
    let (_snapshot, binding, lease) =
        recovery_snapshot::create_recovery_snapshot(&state.run_id, &ws_path, engine_root, policy)?;
    state.recovery_snapshot = Some(binding.clone());
    Ok((binding, lease))
}

#[tauri::command]
pub fn auto_converge_start_run(
    mut config: AutoConvergeRunConfig,
) -> Result<AutoConvergeRunState, String> {
    // B1/R1: Explicit backend production dispatch gate (§209.1 rule 3, §209.15 B1, §209.17 R1).
    // Fails closed immediately before any hash calculation, temp creation, state initialization, or process creation.
    if !is_production_dispatch_enabled() {
        return Err(
            "Production dispatch is blocked pending Phase G verification and Phase H authorization (PRODUCTION_DISPATCH_ENABLED = false)"
                .to_string(),
        );
    }

    let requested_run_id = config
        .run_id
        .take()
        .unwrap_or_else(|| format!("ac-{}", uuid::Uuid::new_v4()));
    let run_id = state_store::validate_auto_converge_run_id(&requested_run_id)
        .map_err(|error| error.to_string())?;
    config.run_id = Some(run_id.clone());

    let ws_path = PathBuf::from(&config.working_dir);
    let manifest = capture_baseline_file_manifest(&ws_path, &BTreeMap::new())?;

    // Perform platform OS-boundary isolation preflight (§20.4 B, §23.1.1, §23.4.2, §209.3 A2)
    let snapshot_dir = prepare_reviewer_snapshot_dir(&codex_exec::ambient_temp_dir(), &run_id, 0)?;

    let git_runner = ProductionGitCommandRunner::new(ws_path.clone());
    let now = chrono::Utc::now().to_rfc3339();
    let configured_cli = config.agent_cli_path.clone();
    let mut state = initialize_run(config, "HEAD", manifest, &git_runner, &now)?;
    let state_path = engine_run_state_path(&state.run_id)?;

    if agent_runner::requires_reviewer_os_boundary_guard(
        AutoConvergeRole::Reviewer,
        state.reviewer_level,
    ) {
        if let Err(e) = PlatformIsolationGuard::verify_reviewer_os_boundary(&ws_path, &snapshot_dir)
        {
            halt_blocked(
                &mut state,
                BlockedKind::FatalInvariant,
                &format!("Platform isolation preflight failed: {e}"),
            );
            save_run_state_authenticated(&state_path, &mut state)?;
            return Ok(state);
        }
    }

    let _run_state_lock = guards::acquire_run_state_lock(&state_path);

    let engine_root = resolve_engine_authority_dir()?;
    let (snapshot_binding, ownership_lease) = prepare_and_bind_recovery_snapshot(
        &mut state,
        &engine_root,
        &recovery_snapshot::SnapshotPolicy::default(),
    )
    .map_err(|e| format!("RecoverySnapshot capture failed: {e}"))?;

    // Single seal contract (§304.3 #4, §306.3 #1):
    // Increment sequence once and compute auth tag once.
    // Use the identical sealed state for persistence attempt and read-back reconciliation.
    guards::seal_run_state_authenticated(&mut state)
        .map_err(|e| format!("Failed to seal initial run state: {e}"))?;
    let intended_state = state.clone();

    if let Err(e) = guards::save_sealed_run_state(&state_path, &intended_state) {
        let outcome = reconcile_run_state_save_error(
            &state_path,
            &intended_state,
            &snapshot_binding,
            &ownership_lease,
            e,
        );
        match outcome {
            RunInitializationOutcome::CommittedAfterReportedError {
                state: committed_state,
                ..
            } => {
                state = committed_state;
            }
            RunInitializationOutcome::NotCommitted {
                save_error,
                cleanup_error,
                remaining_path,
            } => {
                state.recovery_snapshot = None;
                let msg = if let Some(c_err) = cleanup_error {
                    format!("NotCommitted: {save_error}; snapshot compensation cleanup failed at {remaining_path:?}: {c_err}")
                } else {
                    format!("NotCommitted: {save_error}")
                };
                return Err(msg);
            }
            RunInitializationOutcome::Unresolved {
                save_error,
                read_back_error,
                snapshot_path,
                recovery_instructions,
            } => {
                return Err(format!(
                    "Unresolved: {save_error}; authenticated read-back was ambiguous ({read_back_error}). Snapshot preserved at {}. Instructions: {recovery_instructions}",
                    snapshot_path.display()
                ));
            }
            RunInitializationOutcome::Committed(_) => unreachable!(),
        }
    }

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
            ProductionAgentRunner::with_cli(path).with_reviewer_level(state.reviewer_level)
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
    // B1/R1: Explicit backend production dispatch gate on resume (§209.1 rule 3, §209.15 B1, §209.17 R1).
    // Fails closed immediately before state resumption, git operations, or process creation.
    if !is_production_dispatch_enabled() {
        return Err(
            "Production dispatch is blocked pending Phase G verification and Phase H authorization (PRODUCTION_DISPATCH_ENABLED = false)"
                .to_string(),
        );
    }

    let state_path = engine_run_state_path(&run_id)?;
    let mut state = load_and_verify_run_state_authenticated(&state_path, None)?;
    let ws_path = PathBuf::from(&state.workspace.working_dir);

    // Verify isolation preflight upon resume if the upcoming state is a Reviewer role (§20.4 B, §23.1.1, §23.4.2, §209.3 A2)
    let snapshot_dir = prepare_reviewer_snapshot_dir(
        &codex_exec::ambient_temp_dir(),
        &state.run_id,
        state.state_sequence,
    )?;

    if agent_runner::requires_reviewer_os_boundary_guard(
        AutoConvergeRole::Reviewer,
        state.reviewer_level,
    ) && next_role_requires_reviewer_preflight(&state)
    {
        if let Err(e) = PlatformIsolationGuard::verify_reviewer_os_boundary(&ws_path, &snapshot_dir)
        {
            halt_blocked(
                &mut state,
                BlockedKind::FatalInvariant,
                &format!("Platform isolation preflight failed: {e}"),
            );
            save_run_state_authenticated(&state_path, &mut state)?;
            return Ok(state);
        }
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
            ProductionAgentRunner::with_cli(path).with_reviewer_level(state.reviewer_level)
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
            let source_dir = if let Some(ref cb) = state.candidate_binding {
                Path::new(&cb.candidate_root)
            } else {
                ws_path
            };
            let delta = compute_run_delta_manifest(
                source_dir,
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
    let source_root = if let Some(ref cb) = state.candidate_binding {
        PathBuf::from(&cb.candidate_root)
    } else {
        ws_path.to_path_buf()
    };
    let delta = compute_run_delta_manifest(
        &source_root,
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
        "cycle": state.candidate_binding.as_ref().map(|cb| cb.cycle),
        "producer_session_id": state.session_provenance.producer_session_id.clone(),
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
            let src = source_root.join(&entry.path);
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
    let source_dir = if let Some(ref cb) = state.candidate_binding {
        std::path::Path::new(&cb.candidate_root)
    } else {
        ws_path
    };
    let delta = compute_run_delta_manifest(
        source_dir,
        &state.runtime_allowlist,
        &state.workspace.baseline_file_manifest,
    )
    .map_err(|e| format!("Failed to compute run delta for corrective snapshot: {e}"))?;
    for entry in &delta {
        if entry.current_sha256
            != "0000000000000000000000000000000000000000000000000000000000000000"
        {
            let src = source_dir.join(&entry.path);
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

fn reconcile_interrupted_promotion_on_resume(
    state: &mut AutoConvergeRunState,
    state_path: &Path,
    ws_path: &Path,
    now_iso: &str,
) -> Result<(), String> {
    let pj = match state.promotion_journal.as_ref() {
        Some(pj) => pj.clone(),
        None => return Ok(()),
    };

    if let Err(e) = pj.validate() {
        let msg = format!("Invalid promotion journal binding on resume: {e}");
        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
        save_run_state_authenticated(state_path, state)?;
        return Err(msg);
    }

    let journal_path = PathBuf::from(&pj.journal_dir);
    let cand_path = PathBuf::from(&pj.candidate_root);

    // Verify pj.run_id matches state.run_id
    if pj.run_id != state.run_id {
        let msg = format!(
            "Promotion journal run_id mismatch: binding has {}, state has {}",
            pj.run_id, state.run_id
        );
        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
        save_run_state_authenticated(state_path, state)?;
        return Err(msg);
    }

    // Verify candidate_root matches cb.candidate_root if present
    if let Some(ref cb) = state.candidate_binding {
        if pj.candidate_root != cb.candidate_root {
            let msg = format!(
                "Promotion journal candidate_root mismatch: binding has {}, candidate_binding has {}",
                pj.candidate_root, cb.candidate_root
            );
            halt_blocked(state, BlockedKind::FatalInvariant, &msg);
            save_run_state_authenticated(state_path, state)?;
            return Err(msg);
        }
    }

    match pj.phase {
        PromotionPhase::Staged | PromotionPhase::Applying | PromotionPhase::RecoveryRequired => {
            match implementer_workspace::inspect_path_presence(&journal_path) {
                implementer_workspace::PathPresence::Present(meta) => {
                    if implementer_workspace::is_path_alias_or_reparse(&meta) {
                        let msg = format!(
                            "Promotion journal path is a symlink or reparse point: {}",
                            journal_path.display()
                        );
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        state.is_terminal = false;
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                    if !meta.is_dir() {
                        let msg = format!(
                            "Promotion journal path is not a directory: {}",
                            journal_path.display()
                        );
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        state.is_terminal = false;
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                }
                implementer_workspace::PathPresence::Absent => {
                    let msg = format!(
                        "Interrupted promotion journal directory does not exist at {}",
                        journal_path.display()
                    );
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
                implementer_workspace::PathPresence::Error(e) => {
                    let msg = format!("Failed to inspect promotion journal path: {e}");
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    state.is_terminal = false;
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            }

            // Verify journal snapshot digest matches authenticated binding before attempting recovery
            let (_journal_data, _count, actual_digest) =
                match implementer_workspace::load_and_validate_journal_with_digest(&journal_path) {
                    Ok(res) => res,
                    Err(e) => {
                        let msg = format!("Failed to load promotion journal for recovery: {e}");
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        state.is_terminal = false;
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                };

            if actual_digest != pj.journal_digest {
                let msg = format!(
                    "Promotion journal snapshot digest mismatch: expected {}, got {}",
                    pj.journal_digest, actual_digest
                );
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                state.is_terminal = false;
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            // Restore primary to recorded baseline WITHOUT deleting the journal yet
            if let Err(e) = implementer_workspace::recover_retained_promotion_restore_with_digest(
                ws_path,
                &journal_path,
                None,
                Some(&pj.journal_digest),
            ) {
                let msg = format!("Promotion journal recovery restore failed: {e}");
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                state.is_terminal = false;
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            // Re-capture and compare the exact in-scope primary manifest after recovery
            let current_primary_manifest =
                match capture_baseline_file_manifest(ws_path, &BTreeMap::new()) {
                    Ok(m) => m,
                    Err(e) => {
                        let msg = format!("Failed to capture primary manifest after recovery: {e}");
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        state.is_terminal = false;
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                };
            let current_primary_digest =
                compute_baseline_workspace_digest(&current_primary_manifest);
            if current_primary_digest != state.workspace.baseline_workspace_digest
                || current_primary_manifest != state.workspace.baseline_file_manifest
            {
                let msg = "Recovered primary workspace does not match recorded baseline manifest"
                    .to_string();
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                state.is_terminal = false;
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            // Persist intermediate RecoveryRestored phase while journal still exists on disk!
            if let Some(ref mut binding) = state.promotion_journal {
                binding.phase = PromotionPhase::RecoveryRestored;
            }
            state.updated_at = now_iso.to_string();
            save_run_state_authenticated(state_path, state)?;

            // Delete validated owned journal
            match implementer_workspace::inspect_path_presence(&journal_path) {
                implementer_workspace::PathPresence::Present(meta) => {
                    if implementer_workspace::is_path_alias_or_reparse(&meta) {
                        let msg = format!(
                            "Promotion journal path is a symlink or reparse point: {}",
                            journal_path.display()
                        );
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        state.is_terminal = false;
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                    if let Err(e) =
                        implementer_workspace::cleanup_recovered_promotion_journal_authenticated(
                            ws_path,
                            &journal_path,
                            &state.run_id,
                            &cand_path,
                            PromotionPhase::RecoveryRestored,
                            &pj.journal_digest,
                        )
                    {
                        let msg = format!("Failed to clean up recovered promotion journal: {e}");
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        state.is_terminal = false;
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                }
                implementer_workspace::PathPresence::Absent => {}
                implementer_workspace::PathPresence::Error(e) => {
                    let msg = format!("Failed to inspect promotion journal path: {e}");
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    state.is_terminal = false;
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            }

            // Clear promotion journal binding after successful cleanup
            state.promotion_journal = None;
            state.updated_at = now_iso.to_string();
            if state.current_state != AutoConvergeState::Blocked {
                halt_blocked(
                    state,
                    BlockedKind::FatalInvariant,
                    "Interrupted promotion was rolled back to baseline on resume",
                );
            }
            save_run_state_authenticated(state_path, state)?;
        }
        PromotionPhase::RecoveryRestored => {
            // §111.4.2 RecoveryRestored order:
            // First validate binding and journal (when present), then recapture/compare primary baseline manifest.
            // Only after both checks succeed may cleanup delete the exact journal.
            match implementer_workspace::inspect_path_presence(&journal_path) {
                implementer_workspace::PathPresence::Present(meta) => {
                    if implementer_workspace::is_path_alias_or_reparse(&meta) {
                        let msg = format!(
                            "Promotion journal path is a symlink or reparse point: {}",
                            journal_path.display()
                        );
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        state.is_terminal = false;
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                    if !meta.is_dir() {
                        let msg = format!(
                            "Promotion journal path is not a directory: {}",
                            journal_path.display()
                        );
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        state.is_terminal = false;
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                    let (_journal_data, _count, actual_digest) =
                        match implementer_workspace::load_and_validate_journal_with_digest(
                            &journal_path,
                        ) {
                            Ok(res) => res,
                            Err(e) => {
                                let msg = format!(
                                    "Failed to validate journal in RecoveryRestored phase: {e}"
                                );
                                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                                state.is_terminal = false;
                                save_run_state_authenticated(state_path, state)?;
                                return Err(msg);
                            }
                        };
                    if actual_digest != pj.journal_digest {
                        let msg = format!(
                            "Journal digest mismatch in RecoveryRestored phase: expected {}, got {}",
                            pj.journal_digest, actual_digest
                        );
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        state.is_terminal = false;
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                }
                implementer_workspace::PathPresence::Absent => {
                    // Genuinely absent: cleanup had already completed before crash
                }
                implementer_workspace::PathPresence::Error(e) => {
                    let msg = format!("Failed to inspect promotion journal path: {e}");
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    state.is_terminal = false;
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            }

            // Verify primary workspace matches recorded baseline BEFORE journal cleanup
            let current_primary_manifest =
                match capture_baseline_file_manifest(ws_path, &BTreeMap::new()) {
                    Ok(m) => m,
                    Err(e) => {
                        let msg = format!("Failed to capture primary manifest after recovery: {e}");
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        state.is_terminal = false;
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                };
            let current_primary_digest =
                compute_baseline_workspace_digest(&current_primary_manifest);
            if current_primary_digest != state.workspace.baseline_workspace_digest
                || current_primary_manifest != state.workspace.baseline_file_manifest
            {
                let msg = "Primary workspace does not match recorded baseline manifest before journal cleanup".to_string();
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                state.is_terminal = false;
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            // Delete journal only after verified baseline and journal checks
            match implementer_workspace::inspect_path_presence(&journal_path) {
                implementer_workspace::PathPresence::Present(meta) => {
                    if implementer_workspace::is_path_alias_or_reparse(&meta) {
                        let msg = format!(
                            "Promotion journal path is a symlink or reparse point: {}",
                            journal_path.display()
                        );
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        state.is_terminal = false;
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                    if let Err(e) =
                        implementer_workspace::cleanup_recovered_promotion_journal_authenticated(
                            ws_path,
                            &journal_path,
                            &state.run_id,
                            &cand_path,
                            PromotionPhase::RecoveryRestored,
                            &pj.journal_digest,
                        )
                    {
                        let msg = format!("Failed to clean up recovered promotion journal: {e}");
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        state.is_terminal = false;
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                }
                implementer_workspace::PathPresence::Absent => {}
                implementer_workspace::PathPresence::Error(e) => {
                    let msg = format!("Failed to inspect promotion journal path: {e}");
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    state.is_terminal = false;
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            }

            state.promotion_journal = None;
            state.updated_at = now_iso.to_string();
            if state.current_state != AutoConvergeState::Blocked {
                halt_blocked(
                    state,
                    BlockedKind::FatalInvariant,
                    "Interrupted promotion was rolled back to baseline on resume",
                );
            }
            save_run_state_authenticated(state_path, state)?;
        }
        PromotionPhase::Applied => {
            // For Applied, verify recorded final primary state and approval binding before marking terminal
            let approval = state.approvals.iter().rev().find(|a| {
                !a.invalidated && a.binding.target_type == ApprovalTargetType::ImplementationDiff
            });
            let approval = match approval {
                Some(a) => a,
                None => {
                    let msg = "Applied promotion has no valid ImplementationDiff approval record"
                        .to_string();
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            };

            // Authenticate approval record
            let expected_tag = match super::guards::compute_approval_auth_tag(
                &state.run_id,
                approval.sequence,
                &approval.reviewer_session_id,
                &approval.producer_session_id,
                &approval.binding,
            ) {
                Ok(tag) => tag,
                Err(e) => {
                    let msg = format!("Approval record authentication failed: {e}");
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            };
            if approval.engine_auth_tag != expected_tag {
                let msg =
                    "Approval record authentication failed on resume: invalid engine_auth_tag"
                        .to_string();
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            // Verify reviewer independence
            if approval.reviewer_session_id.trim().is_empty()
                || approval.reviewer_session_id == approval.producer_session_id
            {
                let msg = "Approval record on resume violates Reviewer independence".to_string();
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            // Verify candidate binding
            let cb = match state.candidate_binding.as_ref() {
                Some(cb) => cb,
                None => {
                    let msg = "Missing candidate_binding in run_state on resume".to_string();
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            };
            if let Err(e) = cb.validate() {
                let msg = format!("Invalid candidate_binding on resume: {e}");
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            // Validate all binding fields strictly (no optional bypass)
            let b = &approval.binding;
            if b.target_type != ApprovalTargetType::ImplementationDiff
                || b.allowlist_digest != state.allowlist_digest.as_deref().unwrap_or_default()
                || b.policy_profile_digest != state.policy_profile_digest
                || b.root_plan_digest != state.plan_lineage.root_plan_digest
                || b.active_plan_digest != state.plan_lineage.active_plan_digest
                || b.diff_digest != state.current_diff_digest
                || b.evidence_manifest_digest != state.current_evidence_manifest_digest
                || b.run_id.as_deref() != Some(&state.run_id)
                || b.cycle != Some(cb.cycle)
                || b.implementer_session_id.as_deref() != Some(&cb.session_id)
                || b.bundle_digest.is_none()
                || b.bundle_digest.as_deref() != cb.review_bundle_digest.as_deref()
                || !is_valid_sha256_hex(b.bundle_digest.as_deref().unwrap())
            {
                let msg = "Approval binding does not match run state on resume".to_string();
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            // Verify primary workspace has the approved candidate delta applied
            let cand_path = PathBuf::from(&cb.candidate_root);
            let delta = match compute_run_delta_manifest(
                &cand_path,
                &state.runtime_allowlist,
                &state.workspace.baseline_file_manifest,
            ) {
                Ok(d) => d,
                Err(e) => {
                    let msg = format!("Failed to compute candidate delta on resume: {e}");
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            };

            // Check that primary matches every entry in delta
            for entry in &delta {
                let target = ws_path.join(&entry.path);
                match implementer_workspace::inspect_path_presence(&target) {
                    implementer_workspace::PathPresence::Present(meta) => {
                        if implementer_workspace::is_path_alias_or_reparse(&meta) {
                            let msg = format!(
                                "Applied state verification failed: {} is a symlink or reparse point",
                                entry.path
                            );
                            halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                            save_run_state_authenticated(state_path, state)?;
                            return Err(msg);
                        }
                    }
                    implementer_workspace::PathPresence::Absent => {
                        let msg = format!(
                            "Applied state verification failed: {} is missing from primary",
                            entry.path
                        );
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                    implementer_workspace::PathPresence::Error(e) => {
                        let msg = format!(
                            "Applied state verification failed: cannot inspect {}: {e}",
                            entry.path
                        );
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                }
                let target_bytes = match std::fs::read(&target) {
                    Ok(b) => b,
                    Err(e) => {
                        let msg = format!(
                            "Failed to read primary file {} for applied state verification: {e}",
                            entry.path
                        );
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                };
                let target_sha = sha256_hex(&target_bytes);
                if target_sha != entry.current_sha256 {
                    let msg = format!(
                        "Applied state verification failed: digest mismatch for {}",
                        entry.path
                    );
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            }

            // §111.4.3 / §112.4 Applied order:
            // Validate any remaining journal against authenticated run ID/path/digest, and attempt cleanup.
            // If cleanup fails, do not clear promotion_journal or mark terminal; persist a retryable/blocked state with the binding intact.
            // If journal is absent, only permit completion when the persisted phase and verified final-state contract establish that cleanup had already completed.
            match implementer_workspace::inspect_path_presence(&journal_path) {
                implementer_workspace::PathPresence::Present(meta) => {
                    if implementer_workspace::is_path_alias_or_reparse(&meta) {
                        let msg = format!(
                            "Promotion journal path is a symlink or reparse point: {}",
                            journal_path.display()
                        );
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        state.is_terminal = false;
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                    if let Err(e) =
                        implementer_workspace::cleanup_recovered_promotion_journal_authenticated(
                            ws_path,
                            &journal_path,
                            &state.run_id,
                            &cand_path,
                            PromotionPhase::Applied,
                            &pj.journal_digest,
                        )
                    {
                        let msg = format!("Failed to clean up applied promotion journal: {e}");
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        state.is_terminal = false; // §111.4.3: do not mark terminal on cleanup failure
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                }
                implementer_workspace::PathPresence::Absent => {}
                implementer_workspace::PathPresence::Error(e) => {
                    let msg = format!("Failed to inspect promotion journal path: {e}");
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    state.is_terminal = false;
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            }

            // Primary verified to have approved changes applied and journal cleaned up!
            state.promotion_journal = None;
            state.is_terminal = true;
            state.updated_at = now_iso.to_string();
            save_run_state_authenticated(state_path, state)?;
        }
    }

    Ok(())
}

/// Advances the Phase A state machine by one transition using the provided agent and command runners.
/// Every state transition verifies bounds and persists authenticated state before any subsequent launch.
fn run_phase_a_step(
    state: &mut AutoConvergeRunState,
    agent_runner: &dyn AgentCommandRunner,
    _git_runner: &dyn CommandRunner,
    state_path: &Path,
) -> Result<(), String> {
    let ws_path = PathBuf::from(&state.workspace.working_dir);
    let now_iso = chrono::Utc::now().to_rfc3339();
    let case_insensitive_fs = cfg!(windows);

    // 1. Reconciliation of interrupted promotion on process resume / re-entry
    // Always runs BEFORE short-circuiting on is_terminal, is_paused, or Blocked!
    if state.promotion_journal.is_some() {
        reconcile_interrupted_promotion_on_resume(state, state_path, &ws_path, &now_iso)?;
    }

    // 2. Short-circuit if terminal, paused, or Blocked: no agent dispatch occurs.
    if state.is_terminal || state.is_paused || state.current_state == AutoConvergeState::Blocked {
        return Ok(());
    }

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
                model: Some(state.active_producer_model.clone()),
                reasoning_effort: Some(state.active_reasoning_effort.clone()),
                run_id: Some(state.run_id.clone()),
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
            let snapshot_dir = codex_exec::ambient_temp_dir()
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
                model: Some(state.active_reviewer_model.clone()),
                reasoning_effort: Some(state.active_reasoning_effort.clone()),
                run_id: Some(state.run_id.clone()),
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
            if resp.exit_code != 0 {
                let _ = std::fs::remove_dir_all(&snapshot_dir);
                let msg = format!(
                    "Plan review process exited unsuccessfully with exit code {}",
                    resp.exit_code
                );
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }
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
            let engine_state_root = resolve_engine_authority_dir()?;
            let candidate_parent_path = codex_exec::ambient_temp_dir()
                .join("poverello")
                .join("candidate_workspaces")
                .join(&state.run_id);
            let candidate_parent = match implementer_workspace::validate_and_create_candidate_parent(
                &candidate_parent_path,
            ) {
                Ok(p) => p,
                Err(e) => {
                    let msg = format!("Failed to validate candidate parent directory: {e}");
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            };
            let candidate_dir =
                candidate_parent.join(format!("cycle-{}", state.cycle_counts.impl_review_count));

            let candidate_ws = if state.current_state == AutoConvergeState::CorrectiveImplement {
                // Corrective cycles may reuse candidate ONLY after authenticated binding and tree validation
                let cb = match state.candidate_binding.as_ref() {
                    Some(b) => b,
                    None => {
                        let msg =
                            "Corrective cycle missing required candidate_binding in run_state"
                                .to_string();
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                };
                if let Err(e) = cb.validate() {
                    let msg = format!("Invalid candidate_binding for corrective reuse: {e}");
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
                let cand_path = PathBuf::from(&cb.candidate_root);
                let cand_presence = implementer_workspace::inspect_path_presence(&cand_path);
                let cand_valid = match cand_presence {
                    implementer_workspace::PathPresence::Present(ref m) => {
                        !implementer_workspace::is_path_alias_or_reparse(m)
                    }
                    _ => false,
                };
                if cb.run_id != state.run_id
                    || Some(&cb.approved_plan_digest)
                        != state.plan_lineage.active_plan_digest.as_ref()
                    || Some(&cb.allowlist_digest) != state.allowlist_digest.as_ref()
                    || !cand_valid
                {
                    let msg = "Candidate binding mismatch or candidate root missing for corrective cycle; refusing to delete or overwrite".to_string();
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
                match implementer_workspace::ImplementerWorkspace::load_existing(
                    &ws_path,
                    &engine_state_root,
                    &cand_path,
                    &state.runtime_allowlist,
                    &cb.candidate_baseline_digest,
                ) {
                    Ok(ws) => ws,
                    Err(e) => {
                        let msg = format!("Failed to validate existing candidate tree for corrective cycle: {e}; retaining candidate");
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                }
            } else {
                // Initial Implement step: candidate must NOT already exist; reject collision without deleting
                match implementer_workspace::inspect_path_presence(&candidate_dir) {
                    implementer_workspace::PathPresence::Absent => {}
                    _ => {
                        let msg = format!(
                            "Candidate directory already exists at {}; refusing to overwrite or delete unowned directory",
                            candidate_dir.display()
                        );
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                }
                match implementer_workspace::ImplementerWorkspace::create(
                    &ws_path,
                    &engine_state_root,
                    &candidate_dir,
                    &state.runtime_allowlist,
                ) {
                    Ok(ws) => ws,
                    Err(e) => {
                        let msg = format!("Failed to create candidate workspace: {e}");
                        halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                        save_run_state_authenticated(state_path, state)?;
                        return Err(msg);
                    }
                }
            };

            let manifest_bytes = serde_json::to_vec(&state.workspace.baseline_file_manifest)
                .map_err(|e| format!("serialize baseline manifest: {e}"))?;
            let run_start_baseline_manifest_digest = sha256_hex(&manifest_bytes);
            let candidate_binding = CandidateBinding {
                run_id: state.run_id.clone(),
                cycle: state.cycle_counts.impl_review_count as u64,
                session_id: session_id.clone(),
                candidate_root: candidate_ws.candidate_root().to_string_lossy().to_string(),
                approved_plan_digest: state
                    .plan_lineage
                    .active_plan_digest
                    .clone()
                    .unwrap_or_default(),
                run_start_baseline_manifest_digest,
                allowlist_digest: state.allowlist_digest.clone().unwrap_or_default(),
                candidate_baseline_digest: candidate_ws.baseline_digest().to_string(),
                review_bundle_digest: None,
            };
            candidate_binding
                .validate()
                .map_err(|e| format!("Invalid candidate binding: {e}"))?;
            state.candidate_binding = Some(candidate_binding);
            save_run_state_authenticated(state_path, state)?;

            let candidate_root = candidate_ws.candidate_root().to_path_buf();
            let req = RoleExecutionRequest {
                role: AutoConvergeRole::Implementer,
                session_id: session_id.clone(),
                workspace_root: candidate_root.clone(),
                snapshot_dir: None,
                plan_path: ws_path.join(".plan").join(format!("{}.md", state.run_id)),
                runtime_allowlist: state.runtime_allowlist.clone(),
                input_prompt: "Implement approved changes strictly within runtime_allowlist"
                    .to_string(),
                timeout_seconds: 1200,
                model: Some(state.active_producer_model.clone()),
                reasoning_effort: Some(state.active_reasoning_effort.clone()),
                run_id: Some(state.run_id.clone()),
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
            if resp.exit_code != 0 {
                let msg = format!(
                    "Implementer execution returned nonzero exit code: {}",
                    resp.exit_code
                );
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            // Verify candidate delta and boundary via workspace.delta()
            let _candidate_delta = match candidate_ws.delta() {
                Ok(d) => d,
                Err(e) => {
                    let msg = format!("Candidate workspace boundary violation: {e}");
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            };

            let delta = compute_run_delta_manifest(
                &candidate_root,
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
            let snapshot_dir = codex_exec::ambient_temp_dir()
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

            // Bind exact review bundle digest to candidate binding in authenticated state
            if let Some(ref mut cb) = state.candidate_binding {
                cb.review_bundle_digest = Some(pre_hash.clone());
            }
            save_run_state_authenticated(state_path, state)?;

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
                model: Some(state.active_reviewer_model.clone()),
                reasoning_effort: Some(state.active_reasoning_effort.clone()),
                run_id: Some(state.run_id.clone()),
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
            if resp.exit_code != 0 {
                let _ = std::fs::remove_dir_all(&snapshot_dir);
                let msg = format!(
                    "Implementation review process exited unsuccessfully with exit code {}",
                    resp.exit_code
                );
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }
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
                model: Some(state.active_producer_model.clone()),
                reasoning_effort: Some(state.active_reasoning_effort.clone()),
                run_id: Some(state.run_id.clone()),
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
            let snapshot_dir = codex_exec::ambient_temp_dir()
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
                model: Some(state.active_reviewer_model.clone()),
                reasoning_effort: Some(state.active_reasoning_effort.clone()),
                run_id: Some(state.run_id.clone()),
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
            if resp.exit_code != 0 {
                let _ = std::fs::remove_dir_all(&snapshot_dir);
                let msg = format!(
                    "Corrective plan review process exited unsuccessfully with exit code {}",
                    resp.exit_code
                );
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }
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
            // Any pre-existing promotion journal must have been reconciled at the start of run_phase_a_step
            if let Some(ref pj) = state.promotion_journal {
                let msg = format!("Unreconciled promotion journal in phase {:?}; run cannot proceed without reconciliation", pj.phase);
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            // Verify approval record
            let approval = state
                .approvals
                .iter()
                .rev()
                .find(|a| {
                    !a.invalidated
                        && a.binding.target_type == ApprovalTargetType::ImplementationDiff
                })
                .ok_or_else(|| "No valid ImplementationDiff approval record found".to_string())?;

            // Authenticate approval record
            let expected_tag = super::guards::compute_approval_auth_tag(
                &state.run_id,
                approval.sequence,
                &approval.reviewer_session_id,
                &approval.producer_session_id,
                &approval.binding,
            )?;
            if approval.engine_auth_tag != expected_tag {
                let msg =
                    "Approval record authentication failed: invalid engine_auth_tag".to_string();
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            // Verify reviewer independence
            if approval.reviewer_session_id.trim().is_empty()
                || approval.reviewer_session_id == approval.producer_session_id
            {
                let msg = "Approval record violates Reviewer independence".to_string();
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            // Verify candidate binding
            let cb = state
                .candidate_binding
                .as_ref()
                .ok_or_else(|| "Missing candidate_binding in run_state".to_string())?;
            if let Err(e) = cb.validate() {
                let msg = format!("Invalid candidate_binding: {e}");
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            // Validate all binding fields strictly (no optional bypass)
            let b = &approval.binding;
            if b.target_type != ApprovalTargetType::ImplementationDiff
                || b.allowlist_digest != state.allowlist_digest.as_deref().unwrap_or_default()
                || b.policy_profile_digest != state.policy_profile_digest
                || b.root_plan_digest != state.plan_lineage.root_plan_digest
                || b.active_plan_digest != state.plan_lineage.active_plan_digest
                || b.diff_digest != state.current_diff_digest
                || b.evidence_manifest_digest != state.current_evidence_manifest_digest
                || b.run_id.as_deref() != Some(&state.run_id)
                || b.cycle != Some(cb.cycle)
                || b.implementer_session_id.as_deref() != Some(&cb.session_id)
                || b.bundle_digest.is_none()
                || b.bundle_digest.as_deref() != cb.review_bundle_digest.as_deref()
                || !is_valid_sha256_hex(b.bundle_digest.as_deref().unwrap())
            {
                let msg = "Approval binding does not match current run state".to_string();
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            // Re-verify primary baseline manifest immediately before promotion
            let current_primary_manifest =
                capture_baseline_file_manifest(&ws_path, &BTreeMap::new())?;
            let current_primary_digest =
                compute_baseline_workspace_digest(&current_primary_manifest);
            if current_primary_digest != state.workspace.baseline_workspace_digest
                || current_primary_manifest != state.workspace.baseline_file_manifest
            {
                let msg =
                    "Primary workspace baseline changed concurrently before promotion".to_string();
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }

            // Load candidate workspace
            let engine_state_root = resolve_engine_authority_dir()?;
            let candidate_path = PathBuf::from(&cb.candidate_root);
            let workspace = implementer_workspace::ImplementerWorkspace::load_existing(
                &ws_path,
                &engine_state_root,
                &candidate_path,
                &state.runtime_allowlist,
                &cb.candidate_baseline_digest,
            )?;

            // Verify candidate delta matches approved diff digest
            let current_run_delta = compute_run_delta_manifest(
                &candidate_path,
                &state.runtime_allowlist,
                &state.workspace.baseline_file_manifest,
            )?;
            let current_diff = compute_diff_digest(&current_run_delta);
            if Some(&current_diff) != state.current_diff_digest.as_ref() {
                let msg = "Promotion rejected: candidate delta changed after approval (diff digest mismatch)".to_string();
                halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                save_run_state_authenticated(state_path, state)?;
                return Err(msg);
            }
            let current_delta_digest = workspace.delta_digest()?;
            let permit = match workspace
                .authorize_delta(workspace.baseline_digest(), &current_delta_digest)
            {
                Ok(p) => p,
                Err(e) => {
                    let msg = format!("Failed to authorize delta for promotion: {e}");
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            };

            // Execute promotion with phase callback
            let run_id = state.run_id.clone();
            let cand_root_str = cb.candidate_root.clone();
            let state_path_buf = state_path.to_path_buf();

            let outcome = workspace.promote_with_controller(
                &permit,
                &run_id,
                |phase, journal_dir, journal_digest| {
                    state.promotion_journal = Some(PromotionJournalBinding {
                        run_id: run_id.clone(),
                        journal_dir: journal_dir.to_string_lossy().to_string(),
                        journal_digest: journal_digest.to_string(),
                        phase,
                        candidate_root: cand_root_str.clone(),
                    });
                    save_run_state_authenticated(&state_path_buf, state)?;
                    Ok(())
                },
            );

            match outcome {
                Ok(implementer_workspace::PromotionOutcome::Promoted { changed_files: _ }) => {
                    state.promotion_journal = None;
                    state.is_terminal = true;
                    state.updated_at = now_iso;
                    save_run_state_authenticated(state_path, state)?;
                }
                Ok(implementer_workspace::PromotionOutcome::RecoveryRequired {
                    journal,
                    reason,
                }) => {
                    let digest =
                        implementer_workspace::load_and_validate_journal_with_digest(&journal)
                            .map(|(_, _, d)| d)
                            .unwrap_or_else(|_| {
                                state
                                    .promotion_journal
                                    .as_ref()
                                    .map(|p| p.journal_digest.clone())
                                    .unwrap_or_default()
                            });
                    state.promotion_journal = Some(PromotionJournalBinding {
                        run_id: run_id.clone(),
                        journal_dir: journal.to_string_lossy().to_string(),
                        journal_digest: digest,
                        phase: PromotionPhase::RecoveryRequired,
                        candidate_root: cand_root_str,
                    });
                    let msg = format!("Promotion requires recovery: {reason}");
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
                Ok(implementer_workspace::PromotionOutcome::RejectedBaselineChanged) => {
                    let msg =
                        "Promotion rejected: primary baseline changed concurrently".to_string();
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
                Ok(implementer_workspace::PromotionOutcome::RejectedCandidateChanged) => {
                    let msg = "Promotion rejected: candidate files changed after authorization"
                        .to_string();
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
                Err(e) => {
                    let msg = format!("Promotion execution failed: {e}");
                    halt_blocked(state, BlockedKind::FatalInvariant, &msg);
                    save_run_state_authenticated(state_path, state)?;
                    return Err(msg);
                }
            }
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
