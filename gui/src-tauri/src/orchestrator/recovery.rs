use super::types::{RunConfigurationSnapshot, WorkflowState};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::{Arc, Mutex};

pub const JOURNAL_SCHEMA_VERSION: u32 = 1;
pub const CHECKPOINT_SCHEMA_VERSION: u32 = 3;

/// Validates that a run ID conforms to canonical format and contains no path separators or traversal components.
pub fn validate_run_id(run_id: &str) -> Result<(), String> {
    let trimmed = run_id.trim();
    if trimmed.is_empty() {
        return Err("Run ID cannot be empty".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains("..") {
        return Err(format!(
            "Run ID '{}' contains invalid path separators or traversal components",
            run_id
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(format!(
            "Run ID '{}' contains invalid characters; expected alphanumeric, dash, underscore, or dot",
            run_id
        ));
    }
    Ok(())
}

/// Recovery status of an orchestrator run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunRecoveryStatus {
    Active,
    Interrupted,
    Complete,
    Failed,
    Cancelled,
}

/// Durable iteration counters needed to reconstruct or verify loop limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunIterationCounters {
    #[serde(default)]
    #[serde(alias = "plan_review_count")]
    pub plan_review_count: u32,
    #[serde(default)]
    #[serde(alias = "fix_count")]
    pub fix_count: u32,
    #[serde(default)]
    #[serde(alias = "code_review_count")]
    pub code_review_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "antigravity_dispatches")]
    pub antigravity_dispatches: Option<u32>,
    #[serde(default)]
    pub antigravity_task_dispatches: std::collections::HashMap<String, u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mailbox_epoch: Option<u64>,
}

/// Runtime-only continuation data reconstructed from a verified checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanGatedResumeContext {
    pub stage: WorkflowState,
    pub adopted_baseline: bool,
    pub plan_text: String,
    pub plan_review_count: u32,
    pub fix_count: u32,
    pub code_review_count: u32,
    pub antigravity_dispatches: u32,
    pub antigravity_task_dispatches: std::collections::HashMap<String, u32>,
    pub mailbox_epoch: u64,
}

/// Durable run journal persisted to secure application storage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunJournal {
    #[serde(alias = "schema_version")]
    pub schema_version: u32,
    #[serde(alias = "run_id")]
    pub run_id: String,
    #[serde(alias = "workflow_type")]
    pub workflow_type: String,
    #[serde(alias = "canonical_project_path")]
    pub canonical_project_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "task_prompt")]
    pub task_prompt: Option<String>,
    /// Last plan that crossed the PlanReview approval boundary. Older journals
    /// may omit it; a verified checkpoint stage input can provide the fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_plan: Option<String>,
    pub snapshot: RunConfigurationSnapshot,
    #[serde(alias = "current_state")]
    pub current_state: WorkflowState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "last_successful_state")]
    pub last_successful_state: Option<WorkflowState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "stage_entry_info")]
    pub stage_entry_info: Option<String>,
    #[serde(default)]
    #[serde(alias = "iteration_counters")]
    pub iteration_counters: RunIterationCounters,
    /// Durable MCP invocation audit trail. Missing on legacy journals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub direct_mcp_invocations: Vec<super::adapters::direct_mcp::DirectMcpAuditRecord>,
    /// Durable Planner-Reviewer convergence audit trail. Missing on legacy journals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plan_convergence_audit: Vec<super::convergence::PlanConvergenceAuditEntry>,
    /// Durable confirmation intent for a converged new primary plan. Absent on
    /// legacy journals and on runs that never reached that human gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_convergence_confirmation_intent:
        Option<super::convergence::PlanConvergenceConfirmationIntent>,
    #[serde(alias = "revision")]
    pub revision: u64,
    #[serde(default)]
    pub resume_generation: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "checkpoint_manifest_ref")]
    pub checkpoint_manifest_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "checkpoint_digest")]
    pub checkpoint_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_shelve_backup_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_shelve_backup_digest: Option<String>,
    pub status: RunRecoveryStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<super::adapters::NormalizedUsage>,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    #[serde(alias = "next_dispatch_ordinal")]
    pub next_dispatch_ordinal: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[serde(alias = "adapter_dispatches")]
    pub adapter_dispatches: Vec<AdapterDispatchRecord>,
    #[serde(alias = "created_at_unix")]
    pub created_at_unix: u64,
    #[serde(alias = "updated_at_unix")]
    pub updated_at_unix: u64,
}

fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}

/// Durable audit record for an individual adapter dispatch within a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterDispatchRecord {
    #[serde(alias = "dispatch_id")]
    pub dispatch_id: String,
    pub ordinal: u64,
    pub role: String,
    #[serde(alias = "profile_id")]
    pub profile_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "payload_digest")]
    pub payload_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<super::adapters::NormalizedUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(alias = "created_at_unix")]
    pub created_at_unix: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "completed_at_unix")]
    pub completed_at_unix: Option<u64>,
}

/// Typed summary response for listing interrupted, historical, and invalid runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecoverySummary {
    #[serde(alias = "run_id")]
    pub run_id: String,
    #[serde(alias = "workflow_type")]
    pub workflow_type: String,
    #[serde(alias = "project_path")]
    pub project_path: String,
    pub status: RunRecoveryStatus,
    #[serde(alias = "current_state")]
    pub current_state: WorkflowState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "last_successful_state")]
    pub last_successful_state: Option<WorkflowState>,
    pub revision: u64,
    #[serde(alias = "created_at_unix")]
    pub created_at_unix: u64,
    #[serde(alias = "updated_at_unix")]
    pub updated_at_unix: u64,
    #[serde(alias = "is_resumable")]
    pub is_resumable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "error")]
    pub error: Option<String>,
}

/// Type of tracked file change in a checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackedChangeType {
    Modified,
    Added,
    Deleted,
    Renamed,
}

/// Checkpoint record for an individual tracked file change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedChangeRecord {
    pub path: String,
    #[serde(alias = "change_type")]
    pub change_type: TrackedChangeType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "old_path")]
    pub old_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "content_hash")]
    pub content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
}

/// Checkpoint record for an untracked non-ignored file or symlink.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UntrackedCheckpointEntry {
    #[serde(alias = "rel_path")]
    pub rel_path: String,
    #[serde(alias = "file_type")]
    pub file_type: String,
    #[serde(alias = "content_hash")]
    pub content_hash: String,
    pub mode: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "symlink_target")]
    pub symlink_target: Option<String>,
}

/// Checkpoint state for a single repository (root or submodule).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCheckpoint {
    #[serde(alias = "canonical_root")]
    pub canonical_root: String,
    #[serde(alias = "head_oid")]
    pub head_oid: String,
    #[serde(alias = "refs_snapshot")]
    pub refs_snapshot: String,
    #[serde(alias = "status_inventory")]
    pub status_inventory: Vec<u8>,
    #[serde(default)]
    #[serde(alias = "tracked_changes")]
    pub tracked_changes: Vec<TrackedChangeRecord>,
    #[serde(default)]
    #[serde(alias = "untracked_files")]
    pub untracked_files: Vec<UntrackedCheckpointEntry>,
    #[serde(alias = "integrity_hash")]
    pub integrity_hash: String,
}

/// Submodule child record in the checkpoint hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleCheckpoint {
    #[serde(default)]
    pub node_key: String,
    #[serde(default)]
    pub immediate_parent_key: String,
    #[serde(alias = "immediate_parent_path")]
    pub immediate_parent_path: String,
    #[serde(alias = "rel_path")]
    pub rel_path: String,
    #[serde(alias = "gitlink_oid")]
    pub gitlink_oid: String,
    #[serde(alias = "head_oid")]
    pub head_oid: String,
    pub repository: RepositoryCheckpoint,
    #[serde(default)]
    pub payload_entries: Vec<CheckpointPayloadEntry>,
    #[serde(default)]
    pub index_blob_hash: String,
}

/// Durable manifest interface for workspace checkpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointManifest {
    #[serde(alias = "schema_version")]
    pub schema_version: u32,
    #[serde(alias = "checkpoint_id")]
    pub checkpoint_id: String,
    #[serde(alias = "run_id")]
    pub run_id: String,
    pub stage: WorkflowState,
    #[serde(default)]
    pub kind: CheckpointKind,
    #[serde(alias = "created_at_unix")]
    pub created_at_unix: u64,
    pub repository: RepositoryCheckpoint,
    #[serde(default)]
    pub payload_entries: Vec<CheckpointPayloadEntry>,
    #[serde(default)]
    pub index_blob_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage_input: Option<String>,
    #[serde(default)]
    pub submodules: Vec<SubmoduleCheckpoint>,
    #[serde(alias = "manifest_digest")]
    pub manifest_digest: String,
}

/// Semantic checkpoint boundary. Adopted state is deliberately not a completed stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointKind {
    EntryBaseline,
    #[default]
    StageCheckpoint,
    AdoptBaseline,
    ShelveBackup,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointPayloadEntry {
    pub path: String,
    /// file, symlink, symlink_file, symlink_dir, submodule, or missing.
    pub kind: String,
    pub mode: u32,
    pub size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symlink_target: Option<String>,
}

pub struct JournalManager {
    runs_dir: PathBuf,
    #[cfg(test)]
    pub permissions_test_hook: Mutex<Option<Arc<dyn Fn(&Path) -> Result<(), String> + Send + Sync>>>,
    #[cfg(test)]
    directory_permissions_test_hook: Mutex<Option<Arc<dyn Fn(&Path) -> Result<(), String> + Send + Sync>>>,
    #[cfg(test)]
    existing_metadata_test_hook: Mutex<Option<Arc<dyn Fn(&Path) -> Result<(), String> + Send + Sync>>>,
    #[cfg(test)]
    pub write_test_hook: Mutex<Option<Arc<dyn Fn(&Path) -> Result<(), String> + Send + Sync>>>,
    #[cfg(test)]
    pub cleanup_test_hook: Mutex<Option<Arc<dyn Fn(&Path) -> Result<(), String> + Send + Sync>>>,
    #[cfg(test)]
    pub reserve_test_hook: Mutex<Option<Arc<dyn Fn(&RunJournal) -> Result<(), String> + Send + Sync>>>,
    #[cfg(test)]
    pub record_test_hook: Mutex<Option<Arc<dyn Fn(&RunJournal) -> Result<(), String> + Send + Sync>>>,
    #[cfg(test)]
    pub dispatch_lock_test_hook: Mutex<Option<Arc<dyn Fn(&std::sync::Mutex<()>) -> Result<(), String> + Send + Sync>>>,
}

/// Replays durable reservations; a missing verdict never refunds a dispatched slot.
pub fn replay_convergence_review_count(journal: &RunJournal) -> Result<u32, String> {
    let mut count = 0u32;
    let mut candidates = std::collections::HashSet::new();
    let mut reserved = std::collections::HashSet::new();
    let mut verdicts = std::collections::HashSet::new();
    for entry in &journal.plan_convergence_audit {
        if entry.event_type == "candidate_created" {
            if entry.artifact_ref.is_none() || entry.artifact_digest.is_none() {
                return Err("PC_INVALID_REVIEW_HISTORY: candidate artifact missing".into());
            }
            let id = entry.candidate_id.as_ref().filter(|id| !id.is_empty())
                .ok_or("PC_INVALID_REVIEW_HISTORY: candidate ID missing")?;
            if entry.sequence != count + 1 || !candidates.insert((entry.sequence, id.clone())) {
                return Err("PC_INVALID_REVIEW_HISTORY: invalid candidate sequence".into());
            }
        }
        if entry.event_type == "review_attempt_reserved" {
            let id = entry.candidate_id.as_ref()
                .ok_or("PC_INVALID_REVIEW_HISTORY: reservation candidate missing")?;
            let key = (entry.sequence, id.clone());
            if entry.sequence != count + 1 || !candidates.contains(&key) || !reserved.insert(key) {
                return Err("PC_INVALID_REVIEW_HISTORY: invalid review reservation".into());
            }
            count = count.checked_add(1).ok_or("PC_INVALID_REVIEW_HISTORY: overflow")?;
            if count > journal.snapshot.iteration_limits.max_plan_review_iterations {
                return Err("PC_INVALID_REVIEW_HISTORY: review budget exceeded".into());
            }
        }
        if entry.event_type == "verdict_recorded" {
            let id = entry.candidate_id.as_ref()
                .ok_or("PC_INVALID_REVIEW_HISTORY: verdict candidate missing")?;
            if entry.artifact_ref.is_none() || entry.artifact_digest.is_none()
                || !reserved.contains(&(entry.sequence, id.clone()))
                || !verdicts.insert((entry.sequence, id.clone())) {
                return Err("PC_INVALID_REVIEW_HISTORY: verdict without reservation".into());
            }
        }
    }
    Ok(count)
}

fn validate_journal(journal: &RunJournal) -> Result<(), String> {
    validate_run_id(&journal.run_id)?;
    if journal.workflow_type == "plan_convergence"
        && replay_convergence_review_count(journal)? != journal.iteration_counters.plan_review_count
    {
        return Err("PC_INVALID_REVIEW_HISTORY: counter does not match reservations".into());
    }
    if let Some(intent) = &journal.plan_convergence_confirmation_intent {
        // An ambiguous intent identity must fail closed rather than authorize a
        // plan write against an unverifiable target.
        if journal.workflow_type != "plan_convergence"
            || intent.run_id != journal.run_id
            || intent.candidate_id.trim().is_empty()
            || intent.candidate_artifact_ref.trim().is_empty()
            || intent.target_path.trim().is_empty()
            || intent.request_revision == 0
            || intent.intent_record_revision != intent.request_revision.saturating_add(1)
            || intent.target_revision == 0
            || intent.idempotency_key.trim().is_empty()
            || intent.content_digest.len() != 64
            || !intent.content_digest.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
        {
            return Err("PC_INVALID_CONFIRMATION_INTENT: malformed confirmation intent".into());
        }
    }
    if journal.schema_version != JOURNAL_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported journal schema version {}; expected {}",
            journal.schema_version, JOURNAL_SCHEMA_VERSION
        ));
    }
    if journal.revision == 0 {
        return Err("Journal revision must be greater than zero".to_string());
    }
    if journal.updated_at_unix < journal.created_at_unix {
        return Err(format!(
            "Journal updated_at_unix {} precedes created_at_unix {}",
            journal.updated_at_unix, journal.created_at_unix
        ));
    }
    if journal.checkpoint_manifest_ref.is_some() != journal.checkpoint_digest.is_some() {
        return Err(
            "Checkpoint manifest reference and digest must either both be present or both be absent"
                .to_string(),
        );
    }

    let expected_terminal_state = match journal.status {
        RunRecoveryStatus::Complete => Some(WorkflowState::Complete),
        RunRecoveryStatus::Failed => Some(WorkflowState::Failed),
        RunRecoveryStatus::Cancelled => Some(WorkflowState::Cancelled),
        RunRecoveryStatus::Active | RunRecoveryStatus::Interrupted => None,
    };
    let is_terminal_state = matches!(
        journal.current_state,
        WorkflowState::Complete | WorkflowState::Failed | WorkflowState::Cancelled
    );
    match expected_terminal_state {
        Some(expected) if journal.current_state != expected => {
            return Err(format!(
                "Terminal journal status '{:?}' contradicts current state '{:?}'",
                journal.status, journal.current_state
            ));
        }
        None if is_terminal_state => {
            return Err(format!(
                "Non-terminal journal status '{:?}' contradicts terminal current state '{:?}'",
                journal.status, journal.current_state
            ));
        }
        _ => {}
    }

    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_ordinals = std::collections::HashSet::new();
    let mut max_ordinal = 0u64;

    for d in &journal.adapter_dispatches {
        if d.ordinal == 0 {
            return Err("PC_DISPATCH_INVALID_ORDINAL: dispatch ordinal must be non-zero".to_string());
        }
        let expected_id = format!("{}:{}", journal.run_id, d.ordinal);
        if d.dispatch_id != expected_id {
            return Err(format!(
                "PC_DISPATCH_ID_MISMATCH: dispatch_id '{}' does not match expected canonical format '{}'",
                d.dispatch_id, expected_id
            ));
        }
        if let Some(status_str) = d.status.as_deref() {
            if status_str.trim().is_empty() {
                return Err("PC_DISPATCH_INVALID_STATUS: empty or whitespace-only dispatch status".to_string());
            }
        }
        if !seen_ids.insert(&d.dispatch_id) || !seen_ordinals.insert(d.ordinal) {
            return Err(format!(
                "PC_DISPATCH_DUPLICATE: duplicate dispatch id '{}' or ordinal {}",
                d.dispatch_id, d.ordinal
            ));
        }
        if d.ordinal > max_ordinal {
            max_ordinal = d.ordinal;
        }
    }

    if !journal.adapter_dispatches.is_empty() && journal.next_dispatch_ordinal <= max_ordinal {
        return Err(format!(
            "PC_DISPATCH_INVALID_NEXT_ORDINAL: next_dispatch_ordinal {} must strictly exceed max ordinal {}",
            journal.next_dispatch_ordinal, max_ordinal
        ));
    }

    Ok(())
}

/// Process-wide mutual exclusion guarding run journal read-modify-write operations
/// for adapter dispatch reservations and records across all JournalManager instances.
static DISPATCH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn acquire_dispatch_lock<'a>(
    mutex: &'a std::sync::Mutex<()>,
) -> Result<std::sync::MutexGuard<'a, ()>, String> {
    mutex
        .lock()
        .map_err(|e| format!("PC_DISPATCH_LOCK_POISONED: dispatch mutex poisoned: {e}"))
}

impl JournalManager {
    pub fn new(runs_dir: PathBuf) -> Self {
        Self {
            runs_dir,
            #[cfg(test)]
            permissions_test_hook: Mutex::new(None),
            #[cfg(test)]
            directory_permissions_test_hook: Mutex::new(None),
            #[cfg(test)]
            existing_metadata_test_hook: Mutex::new(None),
            #[cfg(test)]
            write_test_hook: Mutex::new(None),
            #[cfg(test)]
            cleanup_test_hook: Mutex::new(None),
            #[cfg(test)]
            reserve_test_hook: Mutex::new(None),
            #[cfg(test)]
            record_test_hook: Mutex::new(None),
            #[cfg(test)]
            dispatch_lock_test_hook: Mutex::new(None),
        }
    }

    pub fn runs_dir(&self) -> &Path {
        &self.runs_dir
    }

    pub fn ensure_directory(&self) -> Result<(), String> {
        if !self.runs_dir.exists() {
            fs::create_dir_all(&self.runs_dir).map_err(|e| {
                format!(
                    "Failed to create runs directory '{}': {}",
                    self.runs_dir.display(),
                    e
                )
            })?;
        }
        #[cfg(test)]
        if let Some(ref hook) = *self.directory_permissions_test_hook.lock().unwrap() {
            hook(&self.runs_dir)?;
        }
        // Unconditionally apply and verify permissions on runs directory on every use
        apply_and_verify_permissions(&self.runs_dir, true)?;
        Ok(())
    }

    pub fn write_journal(&self, journal: &RunJournal) -> Result<(), String> {
        validate_journal(journal)?;

        self.ensure_directory()?;

        let target_path = self.runs_dir.join(format!("{}.json", journal.run_id));

        // Only an explicit NotFound result permits creating a new journal. Do not
        // let Path::exists() collapse metadata errors into an apparent absence.
        #[cfg(test)]
        if let Some(ref hook) = *self.existing_metadata_test_hook.lock().unwrap() {
            hook(&target_path)?;
        }
        let existing_metadata = match fs::symlink_metadata(&target_path) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(format!(
                    "Failed to inspect existing journal target '{}': {error}",
                    target_path.display()
                ));
            }
        };

        // Monotonic revision check & strict validation if an existing journal exists.
        if let Some(metadata) = existing_metadata {
            if !metadata.file_type().is_file() {
                return Err(format!(
                    "Existing journal target '{}' is not a regular file",
                    target_path.display()
                ));
            }
            let existing_bytes = fs::read(&target_path).map_err(|e| {
                format!(
                    "Failed to read existing journal at '{}': {}",
                    target_path.display(),
                    e
                )
            })?;
            let existing: RunJournal = serde_json::from_slice(&existing_bytes).map_err(|e| {
                format!(
                    "Failed to parse existing journal at '{}' (corrupt or invalid JSON): {}",
                    target_path.display(),
                    e
                )
            })?;
            if existing.schema_version != JOURNAL_SCHEMA_VERSION {
                return Err(format!(
                    "Existing journal at '{}' has unsupported schema version {}; expected {}",
                    target_path.display(),
                    existing.schema_version,
                    JOURNAL_SCHEMA_VERSION
                ));
            }
            if existing.run_id != journal.run_id {
                return Err(format!(
                    "Existing journal at '{}' has mismatched run ID '{}'; expected '{}'",
                    target_path.display(),
                    existing.run_id,
                    journal.run_id
                ));
            }
            validate_journal(&existing).map_err(|e| {
                format!(
                    "Existing journal at '{}' is semantically invalid: {e}",
                    target_path.display()
                )
            })?;
            if journal.revision < existing.revision {
                return Err(format!(
                    "Stale journal write rejected: incoming revision {} < existing revision {}",
                    journal.revision, existing.revision
                ));
            }
        }

        // 1. Create empty temporary file first
        let tmp_filename = format!("{}.tmp.{}", journal.run_id, uuid::Uuid::new_v4());
        let tmp_path = self.runs_dir.join(tmp_filename);

        fs::write(&tmp_path, b"").map_err(|e| {
            format!("Failed to create temporary journal file '{}': {}", tmp_path.display(), e)
        })?;

        // 2. Apply and verify permissions on temporary file BEFORE writing journal content
        #[cfg(test)]
        {
            if let Some(ref hook) = *self.permissions_test_hook.lock().unwrap() {
                if let Err(primary) = hook(&tmp_path) {
                    return Err(self.cleanup_error(&tmp_path, primary));
                }
            }
        }

        if let Err(e) = apply_and_verify_permissions(&tmp_path, false) {
            return Err(self.cleanup_error(
                &tmp_path,
                format!("Permission enforcement failure on temporary journal: {e}"),
            ));
        }

        // 3. Serialize journal content only after the temporary file is protected.
        let serialized = match serde_json::to_vec_pretty(journal) {
            Ok(bytes) => bytes,
            Err(e) => {
                return Err(self.cleanup_error(&tmp_path, format!("Failed to serialize journal: {e}")));
            }
        };

        // Test seam at the content-write operation, after serialization and permission checks.
        #[cfg(test)]
        {
            if let Some(ref hook) = *self.write_test_hook.lock().unwrap() {
                if let Err(primary) = hook(&tmp_path) {
                    return Err(self.cleanup_error(&tmp_path, primary));
                }
            }
        }

        if let Err(e) = fs::write(&tmp_path, &serialized) {
            return Err(self.cleanup_error(
                &tmp_path,
                format!("Failed to write temporary journal file content: {e}"),
            ));
        }

        // 4. Atomic replace / rename
        if let Err(e) = replace_file_atomically(&tmp_path, &target_path) {
            return Err(self.cleanup_error(
                &tmp_path,
                format!("Atomic replace failed for journal: {e}"),
            ));
        }

        // 5. Verify permissions on target file
        apply_and_verify_permissions(&target_path, false)?;

        Ok(())
    }

    fn cleanup_temp_file(&self, path: &Path) -> Result<(), String> {
        #[cfg(test)]
        if let Some(ref hook) = *self.cleanup_test_hook.lock().unwrap() {
            return hook(path);
        }
        fs::remove_file(path).map_err(|e| {
            format!("Failed to remove temporary journal '{}': {e}", path.display())
        })
    }

    fn cleanup_error(&self, path: &Path, primary: String) -> String {
        match self.cleanup_temp_file(path) {
            Ok(()) => primary,
            Err(cleanup) => format!("{primary}; additionally, {cleanup}"),
        }
    }

    pub fn read_journal(&self, run_id: &str) -> Result<RunJournal, String> {
        validate_run_id(run_id)?;
        self.ensure_directory()?;
        let path = self.runs_dir.join(format!("{}.json", run_id));
        if !path.exists() {
            return Err(format!("Journal for run '{}' not found", run_id));
        }
        let data = fs::read(&path).map_err(|e| format!("Failed to read journal: {}", e))?;
        let journal: RunJournal = serde_json::from_slice(&data).map_err(|e| {
            format!("Failed to deserialize journal for run '{}': {}", run_id, e)
        })?;
        if journal.run_id != run_id {
            return Err(format!(
                "Run ID mismatch in journal: expected '{}', got '{}'",
                run_id, journal.run_id
            ));
        }
        validate_journal(&journal)?;

        Ok(journal)
    }

    pub fn list_journals(&self) -> Result<Vec<RunRecoverySummary>, String> {
        self.ensure_directory()?;

        let mut summaries = Vec::new();
        let entries = fs::read_dir(&self.runs_dir)
            .map_err(|e| format!("Failed to read runs directory: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Error reading directory entry: {}", e))?;
            let path = entry.path();
            if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    if file_name.ends_with(".json") && !file_name.contains(".tmp.") {
                        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(file_name);
                        let mod_time = entry
                            .metadata()
                            .and_then(|m| m.modified())
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs())
                            .unwrap_or(0);

                        match fs::read(&path) {
                            Ok(bytes) => {
                                match serde_json::from_slice::<RunJournal>(&bytes) {
                                    Ok(journal) => {
                                        if validate_journal(&journal).is_ok() && journal.run_id == stem {
                                            let status = match journal.status {
                                                RunRecoveryStatus::Active => RunRecoveryStatus::Interrupted,
                                                other => other,
                                            };
                                            // In Phase A, resumability is false until payload restore is implemented
                                            summaries.push(RunRecoverySummary {
                                                run_id: journal.run_id,
                                                workflow_type: journal.workflow_type,
                                                project_path: journal.canonical_project_path,
                                                status,
                                                current_state: journal.current_state,
                                                last_successful_state: journal.last_successful_state,
                                                revision: journal.revision,
                                                created_at_unix: journal.created_at_unix,
                                                updated_at_unix: journal.updated_at_unix,
                                                is_resumable: false,
                                                error: None,
                                            });
                                        } else {
                                            // Invalid schema or run ID mismatch
                                            let err_msg = if journal.run_id != stem {
                                                format!("Run ID mismatch: filename '{}' != journal '{}'", stem, journal.run_id)
                                            } else if let Err(validation_error) = validate_journal(&journal) {
                                                validation_error
                                            } else if journal.schema_version != JOURNAL_SCHEMA_VERSION {
                                                format!("Unsupported schema version {}", journal.schema_version)
                                            } else {
                                                "Journal failed semantic validation".to_string()
                                            };
                                            summaries.push(RunRecoverySummary {
                                                run_id: stem.to_string(),
                                                workflow_type: "unknown".to_string(),
                                                project_path: String::new(),
                                                status: RunRecoveryStatus::Failed,
                                                current_state: WorkflowState::Failed,
                                                last_successful_state: None,
                                                revision: 0,
                                                created_at_unix: 0,
                                                updated_at_unix: mod_time,
                                                is_resumable: false,
                                                error: Some(err_msg),
                                            });
                                        }
                                    }
                                    Err(e) => {
                                        // Malformed JSON
                                        summaries.push(RunRecoverySummary {
                                            run_id: stem.to_string(),
                                            workflow_type: "unknown".to_string(),
                                            project_path: String::new(),
                                            status: RunRecoveryStatus::Failed,
                                            current_state: WorkflowState::Failed,
                                            last_successful_state: None,
                                            revision: 0,
                                            created_at_unix: 0,
                                            updated_at_unix: mod_time,
                                            is_resumable: false,
                                            error: Some(format!("Malformed journal JSON: {e}")),
                                        });
                                    }
                                }
                            }
                            Err(e) => {
                                summaries.push(RunRecoverySummary {
                                    run_id: stem.to_string(),
                                    workflow_type: "unknown".to_string(),
                                    project_path: String::new(),
                                    status: RunRecoveryStatus::Failed,
                                    current_state: WorkflowState::Failed,
                                    last_successful_state: None,
                                    revision: 0,
                                    created_at_unix: 0,
                                    updated_at_unix: mod_time,
                                    is_resumable: false,
                                    error: Some(format!("Unreadable journal file: {e}")),
                                });
                            }
                        }
                    }
                }
            }
        }

        summaries.sort_by(|a, b| b.updated_at_unix.cmp(&a.updated_at_unix));
        Ok(summaries)
    }

    fn acquire_dispatch_lock<'a>(
        &self,
        lock: &'a std::sync::Mutex<()>,
    ) -> Result<std::sync::MutexGuard<'a, ()>, String> {
        #[cfg(test)]
        {
            if let Some(ref hook) = *self.dispatch_lock_test_hook.lock().unwrap() {
                hook(lock)?;
            }
        }
        acquire_dispatch_lock(lock)
    }

    /// Private transaction path for reserving a dispatch ordinal with an explicit dispatch lock dependency.
    fn reserve_dispatch_ordinal_with_lock(
        &self,
        run_id: &str,
        lock: &std::sync::Mutex<()>,
    ) -> Result<u64, String> {
        let _guard = self.acquire_dispatch_lock(lock)?;
        let mut journal = self.read_journal(run_id)?;
        #[cfg(test)]
        {
            if let Some(ref hook) = *self.reserve_test_hook.lock().unwrap() {
                hook(&journal)?;
            }
        }
        let ordinal = journal.reserve_dispatch_ordinal()?;
        journal.revision = journal
            .revision
            .checked_add(1)
            .ok_or_else(|| "Run journal revision overflow".to_string())?;
        journal.updated_at_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.write_journal(&journal)?;
        Ok(ordinal)
    }

    /// Private transaction path for recording an adapter dispatch with an explicit dispatch lock dependency.
    fn record_adapter_dispatch_with_lock(
        &self,
        run_id: &str,
        record: AdapterDispatchRecord,
        lock: &std::sync::Mutex<()>,
    ) -> Result<bool, String> {
        let _guard = self.acquire_dispatch_lock(lock)?;
        let mut journal = self.read_journal(run_id)?;
        #[cfg(test)]
        {
            if let Some(ref hook) = *self.record_test_hook.lock().unwrap() {
                hook(&journal)?;
            }
        }
        let changed = journal.record_adapter_dispatch(record)?;
        if changed {
            journal.revision = journal
                .revision
                .checked_add(1)
                .ok_or_else(|| "Run journal revision overflow".to_string())?;
            journal.updated_at_unix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            self.write_journal(&journal)?;
        }
        Ok(changed)
    }

    /// Durably reserves the next monotonically increasing dispatch ordinal for the run.
    /// Writes the reservation to disk before the adapter executes so that crashed ordinals
    /// are never reused on reboot.
    pub fn reserve_dispatch_ordinal(&self, run_id: &str) -> Result<u64, String> {
        self.reserve_dispatch_ordinal_with_lock(run_id, &DISPATCH_LOCK)
    }

    /// Durably records an adapter dispatch in the run journal.
    /// Handles idempotency (same ID + same payload => Ok(false)) and rejects conflicts.
    pub fn record_adapter_dispatch(
        &self,
        run_id: &str,
        record: AdapterDispatchRecord,
    ) -> Result<bool, String> {
        self.record_adapter_dispatch_with_lock(run_id, record, &DISPATCH_LOCK)
    }
}

impl RunJournal {
    /// Allocates and reserves the next monotonically increasing dispatch ordinal.
    /// Guarantees that any ordinal previously reserved or present in existing dispatches
    /// is strictly exceeded. Rejects on u64 overflow.
    pub fn reserve_dispatch_ordinal(&mut self) -> Result<u64, String> {
        let max_dispatch = self.adapter_dispatches.iter().map(|d| d.ordinal).max().unwrap_or(0);
        let min_from_dispatches = if max_dispatch == 0 {
            1
        } else {
            max_dispatch.checked_add(1).ok_or_else(|| {
                "PC_DISPATCH_ORDINAL_OVERFLOW: dispatch ordinal exceeded u64::MAX".to_string()
            })?
        };
        let ordinal = if self.next_dispatch_ordinal == 0 {
            min_from_dispatches
        } else {
            self.next_dispatch_ordinal.max(min_from_dispatches)
        };
        self.next_dispatch_ordinal = ordinal.checked_add(1).ok_or_else(|| {
            "PC_DISPATCH_ORDINAL_OVERFLOW: next dispatch ordinal exceeded u64::MAX".to_string()
        })?;
        Ok(ordinal)
    }

    /// Records or updates an adapter dispatch entry.
    ///
    /// Semantics:
    /// - If a dispatch with matching `dispatch_id` exists:
    ///   - Verifies immutable fields: ordinal, role, profile_id, model, payload_digest.
    ///   - Disallowed transitions: completed -> failed, completed -> reserved, failed -> completed, failed -> reserved.
    ///   - Allowed transitions: reserved -> completed, reserved -> failed.
    ///   - Idempotent retries (reserved -> reserved, completed -> completed with same usage, failed -> failed with same usage) return Ok(false).
    /// - If new `dispatch_id`:
    ///   - Rejects ordinal collision.
    ///   - Advances next_dispatch_ordinal on checked_add(1).
    ///   - Appends record and aggregates usage.
    pub fn record_adapter_dispatch(&mut self, record: AdapterDispatchRecord) -> Result<bool, String> {
        if record.ordinal == 0 {
            return Err("PC_DISPATCH_INVALID_ORDINAL: dispatch ordinal must be non-zero".to_string());
        }
        let expected_id = format!("{}:{}", self.run_id, record.ordinal);
        if record.dispatch_id != expected_id {
            return Err(format!(
                "PC_DISPATCH_ID_MISMATCH: dispatch_id '{}' does not match expected canonical format '{}'",
                record.dispatch_id, expected_id
            ));
        }
        let target_status = record.status.as_deref().unwrap_or("reserved");
        if !matches!(target_status, "reserved" | "completed" | "failed") {
            return Err(format!(
                "PC_DISPATCH_INVALID_STATUS: unrecognized dispatch status '{}'",
                target_status
            ));
        }

        if let Some(existing_idx) = self.adapter_dispatches.iter().position(|d| d.dispatch_id == record.dispatch_id) {
            let existing = &self.adapter_dispatches[existing_idx];

            if existing.ordinal != record.ordinal
                || existing.role != record.role
                || existing.profile_id != record.profile_id
                || existing.model != record.model
                || existing.payload_digest != record.payload_digest
            {
                return Err(format!(
                    "PC_DISPATCH_CONFLICT: dispatch id '{}' already exists with conflicting identity or payload",
                    record.dispatch_id
                ));
            }

            let curr_status = existing.status.as_deref().unwrap_or("reserved");

            match curr_status {
                "reserved" => match target_status {
                    "reserved" => Ok(false),
                    "completed" => {
                        self.adapter_dispatches[existing_idx].status = Some("completed".to_string());
                        self.adapter_dispatches[existing_idx].usage = record.usage;
                        self.adapter_dispatches[existing_idx].completed_at_unix = record.completed_at_unix.or_else(|| {
                            Some(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())
                        });
                        self.usage = self.aggregate_dispatches_usage();
                        Ok(true)
                    }
                    "failed" => {
                        self.adapter_dispatches[existing_idx].status = Some("failed".to_string());
                        self.adapter_dispatches[existing_idx].usage = record.usage;
                        self.adapter_dispatches[existing_idx].completed_at_unix = record.completed_at_unix.or_else(|| {
                            Some(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())
                        });
                        self.usage = self.aggregate_dispatches_usage();
                        Ok(true)
                    }
                    other => Err(format!(
                        "PC_DISPATCH_INVALID_TRANSITION: invalid target status '{}' from 'reserved'",
                        other
                    )),
                },
                "completed" => match target_status {
                    "completed" => {
                        if existing.usage == record.usage {
                            Ok(false)
                        } else {
                            Err(format!(
                                "PC_DISPATCH_CONFLICT: dispatch id '{}' already completed with different usage",
                                record.dispatch_id
                            ))
                        }
                    }
                    other => Err(format!(
                        "PC_DISPATCH_INVALID_TRANSITION: cannot transition from 'completed' to '{}'",
                        other
                    )),
                },
                "failed" => match target_status {
                    "failed" => {
                        if existing.usage == record.usage {
                            Ok(false)
                        } else {
                            Err(format!(
                                "PC_DISPATCH_CONFLICT: dispatch id '{}' already failed with different usage",
                                record.dispatch_id
                            ))
                        }
                    }
                    other => Err(format!(
                        "PC_DISPATCH_INVALID_TRANSITION: cannot transition from 'failed' to '{}'",
                        other
                    )),
                },
                other => Err(format!(
                    "PC_DISPATCH_INVALID_STATE: unexpected current dispatch status '{}'",
                    other
                )),
            }
        } else {
            if self.adapter_dispatches.iter().any(|d| d.ordinal == record.ordinal) {
                return Err(format!(
                    "PC_DISPATCH_ORDINAL_COLLISION: ordinal {} already assigned to another dispatch",
                    record.ordinal
                ));
            }

            if record.ordinal >= self.next_dispatch_ordinal {
                self.next_dispatch_ordinal = record.ordinal.checked_add(1).ok_or_else(|| {
                    "PC_DISPATCH_ORDINAL_OVERFLOW: next dispatch ordinal exceeded u64::MAX".to_string()
                })?;
            }

            self.adapter_dispatches.push(record);
            self.usage = self.aggregate_dispatches_usage();
            Ok(true)
        }
    }

    /// Aggregates token counts from all actual dispatches in this run using checked addition.
    /// Each field is known only if every dispatch has a known measurement for that field.
    /// Any unmeasured dispatch or missing field makes the field None.
    /// Checked addition overflow results in None (unknown) without saturating.
    /// Some(0) is strictly preserved.
    pub fn aggregate_dispatches_usage(&self) -> Option<super::adapters::NormalizedUsage> {
        if self.adapter_dispatches.is_empty() {
            return None;
        }

        let input_tokens = aggregate_single_field(&self.adapter_dispatches, |u| u.input_tokens);
        let output_tokens = aggregate_single_field(&self.adapter_dispatches, |u| u.output_tokens);
        let cache_read_tokens = aggregate_single_field(&self.adapter_dispatches, |u| u.cache_read_tokens);
        let cache_write_tokens = aggregate_single_field(&self.adapter_dispatches, |u| u.cache_write_tokens);
        let raw_total_tokens = aggregate_single_field(&self.adapter_dispatches, |u| u.raw_total_tokens);

        if input_tokens.is_none()
            && output_tokens.is_none()
            && cache_read_tokens.is_none()
            && cache_write_tokens.is_none()
            && raw_total_tokens.is_none()
        {
            None
        } else {
            Some(super::adapters::NormalizedUsage {
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cache_write_tokens,
                raw_total_tokens,
            })
        }
    }
}

fn aggregate_single_field<F>(dispatches: &[AdapterDispatchRecord], extract: F) -> Option<u64>
where
    F: Fn(&super::adapters::NormalizedUsage) -> Option<u64>,
{
    if dispatches.is_empty() {
        return None;
    }
    let mut sum: u64 = 0;
    for d in dispatches {
        if !matches!(d.status.as_deref(), Some("completed") | Some("failed")) {
            return None;
        }
        let usage = d.usage.as_ref()?;
        let val = extract(usage)?;
        sum = sum.checked_add(val)?;
    }
    Some(sum)
}

#[cfg(windows)]
pub fn apply_permissions(path: &Path, _is_dir: bool) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Security::Authorization::{
        SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, NO_MULTIPLE_TRUSTEE,
        SET_ACCESS, SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_USER,
    };
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenUser, ACL, DACL_SECURITY_INFORMATION,
        PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(hmem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    unsafe {
        // 1. Get current process token user SID
        let mut token_handle: HANDLE = INVALID_HANDLE_VALUE;
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle) == 0 {
            return Err(format!(
                "Failed to open current process token (err={})",
                GetLastError()
            ));
        }

        let mut token_info_len: u32 = 0;
        let _ = GetTokenInformation(
            token_handle,
            TokenUser,
            std::ptr::null_mut(),
            0,
            &mut token_info_len,
        );

        if token_info_len == 0 {
            CloseHandle(token_handle);
            return Err("Failed to query token user information length".to_string());
        }

        let mut token_buf = vec![0u8; token_info_len as usize];
        if GetTokenInformation(
            token_handle,
            TokenUser,
            token_buf.as_mut_ptr() as *mut _,
            token_info_len,
            &mut token_info_len,
        ) == 0
        {
            CloseHandle(token_handle);
            return Err(format!(
                "Failed to get token user information (err={})",
                GetLastError()
            ));
        }
        CloseHandle(token_handle);

        let token_user = &*(token_buf.as_ptr() as *const TOKEN_USER);
        let user_sid = token_user.User.Sid;

        const NO_INHERITANCE: u32 = 0;
        const SUB_CONTAINERS_AND_OBJECTS_INHERIT: u32 = 3; // CONTAINER_INHERIT_ACE (2) | OBJECT_INHERIT_ACE (1)

        // 2. Build explicit access structure granting access only to current user's SID
        let mut explicit_access: EXPLICIT_ACCESS_W = std::mem::zeroed();
        explicit_access.grfAccessPermissions = 0x1F01FF; // STANDARD_RIGHTS_ALL | SPECIFIC_RIGHTS_ALL (GENERIC_ALL equivalent)
        explicit_access.grfAccessMode = SET_ACCESS;
        explicit_access.grfInheritance = if _is_dir {
            SUB_CONTAINERS_AND_OBJECTS_INHERIT
        } else {
            NO_INHERITANCE
        };
        explicit_access.Trustee.pMultipleTrustee = std::ptr::null_mut();
        explicit_access.Trustee.MultipleTrusteeOperation = NO_MULTIPLE_TRUSTEE;
        explicit_access.Trustee.TrusteeForm = TRUSTEE_IS_SID;
        explicit_access.Trustee.TrusteeType = TRUSTEE_IS_USER;
        explicit_access.Trustee.ptstrName = user_sid as *mut u16;

        let mut new_acl: *mut ACL = std::ptr::null_mut();
        let set_entries_res = SetEntriesInAclW(
            1,
            &explicit_access,
            std::ptr::null_mut(),
            &mut new_acl,
        );
        if set_entries_res != 0 {
            return Err(format!(
                "Failed to build DACL with SetEntriesInAclW (code={})",
                set_entries_res
            ));
        }

        let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();

        // 3. Set protected DACL with inheritance disabled
        let set_res = SetNamedSecurityInfoW(
            wide_path.as_ptr() as *mut u16,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            new_acl,
            std::ptr::null_mut(),
        );

        LocalFree(new_acl as *mut std::ffi::c_void);

        if set_res != 0 {
            return Err(format!(
                "Failed to set protected DACL on '{}' (code={})",
                path.display(),
                set_res
            ));
        }

        Ok(())
    }
}

#[cfg(windows)]
pub fn verify_permissions(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        EqualSid, GetAce, GetAclInformation, GetSecurityDescriptorControl, GetTokenInformation,
        TokenUser, ACCESS_ALLOWED_ACE, ACL, ACL_SIZE_INFORMATION, AclSizeInformation,
        DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SE_DACL_PROTECTED, TOKEN_QUERY,
        TOKEN_USER,
    };
    const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(hmem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    unsafe {
        // 1. Get current process token user SID
        let mut token_handle: HANDLE = INVALID_HANDLE_VALUE;
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle) == 0 {
            return Err(format!(
                "Failed to open current process token (err={})",
                GetLastError()
            ));
        }

        let mut token_info_len: u32 = 0;
        let _ = GetTokenInformation(
            token_handle,
            TokenUser,
            std::ptr::null_mut(),
            0,
            &mut token_info_len,
        );

        if token_info_len == 0 {
            CloseHandle(token_handle);
            return Err("Failed to query token user information length".to_string());
        }

        let mut token_buf = vec![0u8; token_info_len as usize];
        if GetTokenInformation(
            token_handle,
            TokenUser,
            token_buf.as_mut_ptr() as *mut _,
            token_info_len,
            &mut token_info_len,
        ) == 0
        {
            CloseHandle(token_handle);
            return Err(format!(
                "Failed to get token user information (err={})",
                GetLastError()
            ));
        }
        CloseHandle(token_handle);

        let token_user = &*(token_buf.as_ptr() as *const TOKEN_USER);
        let user_sid = token_user.User.Sid;

        let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();

        // 2. Query security descriptor and DACL
        let mut sec_desc: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        let mut dacl: *mut ACL = std::ptr::null_mut();
        let get_res = GetNamedSecurityInfoW(
            wide_path.as_ptr() as *mut u16,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut dacl,
            std::ptr::null_mut(),
            &mut sec_desc,
        );

        if get_res != 0 || sec_desc.is_null() || dacl.is_null() {
            if !sec_desc.is_null() {
                LocalFree(sec_desc as *mut std::ffi::c_void);
            }
            return Err(format!(
                "Failed to verify DACL on '{}' (code={})",
                path.display(),
                get_res
            ));
        }

        // Verify DACL is protected (inheritance disabled)
        let mut control: u16 = 0;
        let mut revision: u32 = 0;
        if GetSecurityDescriptorControl(sec_desc, &mut control, &mut revision) == 0 {
            LocalFree(sec_desc as *mut std::ffi::c_void);
            return Err("Failed to query security descriptor control bits".to_string());
        }
        if (control & SE_DACL_PROTECTED) == 0 {
            LocalFree(sec_desc as *mut std::ffi::c_void);
            return Err(format!(
                "DACL on '{}' is not protected (inheritance not disabled)",
                path.display()
            ));
        }

        // Verify ACL size and iterate ACEs to verify only user_sid has access
        let mut acl_size: ACL_SIZE_INFORMATION = std::mem::zeroed();
        if GetAclInformation(
            dacl,
            &mut acl_size as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        ) == 0
        {
            LocalFree(sec_desc as *mut std::ffi::c_void);
            return Err("Failed to get ACL size information".to_string());
        }

        if acl_size.AceCount == 0 {
            LocalFree(sec_desc as *mut std::ffi::c_void);
            return Err(format!("DACL on '{}' contains zero ACEs", path.display()));
        }

        for i in 0..acl_size.AceCount {
            let mut p_ace: *mut std::ffi::c_void = std::ptr::null_mut();
            if GetAce(dacl, i, &mut p_ace) == 0 || p_ace.is_null() {
                LocalFree(sec_desc as *mut std::ffi::c_void);
                return Err(format!("Failed to retrieve ACE {i} from DACL"));
            }

            let ace = &*(p_ace as *const ACCESS_ALLOWED_ACE);
            if ace.Header.AceType != ACCESS_ALLOWED_ACE_TYPE {
                LocalFree(sec_desc as *mut std::ffi::c_void);
                return Err(format!(
                    "Unexpected ACE type {} in DACL on '{}'",
                    ace.Header.AceType,
                    path.display()
                ));
            }

            let ace_sid = &ace.SidStart as *const u32 as *mut std::ffi::c_void;
            if EqualSid(user_sid, ace_sid) == 0 {
                LocalFree(sec_desc as *mut std::ffi::c_void);
                return Err(format!(
                    "Security violation: Foreign principal SID found in DACL on '{}'",
                    path.display()
                ));
            }
        }

        LocalFree(sec_desc as *mut std::ffi::c_void);
        Ok(())
    }
}

#[cfg(windows)]
pub fn apply_and_verify_permissions(path: &Path, is_dir: bool) -> Result<(), String> {
    apply_permissions(path, is_dir)?;
    verify_permissions(path)?;
    Ok(())
}

#[cfg(not(windows))]
pub fn apply_and_verify_permissions(path: &Path, is_dir: bool) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let target_mode = if is_dir { 0o700 } else { 0o600 };
    fs::set_permissions(path, fs::Permissions::from_mode(target_mode))
        .map_err(|e| format!("Failed to set permissions on '{}': {}", path.display(), e))?;

    let meta = fs::metadata(path)
        .map_err(|e| format!("Failed to read metadata for '{}': {}", path.display(), e))?;
    let mode = meta.permissions().mode();
    if (mode & 0o077) != 0 {
        return Err(format!(
            "Permission verification failed on '{}': mode {:o} has group/other bits set",
            path.display(),
            mode
        ));
    }
    Ok(())
}

fn replace_file_atomically(temp: &Path, target: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "Kernel32")]
        unsafe extern "system" {
            fn ReplaceFileW(
                replaced_file_name: *const u16,
                replacement_file_name: *const u16,
                backup_file_name: *const u16,
                replace_flags: u32,
                exclude: *mut std::ffi::c_void,
                reserved: *mut std::ffi::c_void,
            ) -> i32;
        }

        if !target.exists() {
            return fs::rename(temp, target);
        }

        let target_wide = target.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
        let temp_wide = temp.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
        let result = unsafe {
            ReplaceFileW(
                target_wide.as_ptr(),
                temp_wide.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if result == 0 {
            fs::rename(temp, target)
        } else {
            Ok(())
        }
    }
    #[cfg(not(windows))]
    {
        fs::rename(temp, target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::adapters::NormalizedUsage;
    use crate::orchestrator::types::{AgentRole, ExecutionAdapterType, LoopIterationLimits, OrchestratorProfile, ProfileCapability};
    use std::collections::HashMap;

    fn sample_snapshot() -> RunConfigurationSnapshot {
        let planner = OrchestratorProfile {
            id: "planner-1".to_string(),
            display_name: "Planner Profile".to_string(),
            adapter: ExecutionAdapterType::Provider,
            capabilities: vec![ProfileCapability::Reasoning],
            provider_id: Some("deepseek".to_string()),
            provider_profile_id: None,
            model: Some("deepseek-v4.1-flash".to_string()),
            thinking_mode: None,
            reasoning_effort: None,
            ollama_model: None,
            ollama_endpoint: None,
            executable: None,
            args: None,
            external_mcp_server: None,
            mcp_tool: None,
            context_window_tokens: Some(131_072),
        };
        let mut assignments = HashMap::new();
        assignments.insert(AgentRole::Planner, planner);

        RunConfigurationSnapshot {
            project_path: "C:\\dev\\project".to_string(),
            assignments,
            iteration_limits: LoopIterationLimits::default(),
            validation_gates: Vec::new(),
            budget_limits: HashMap::new(),
            created_at_unix: 1700000000,
            lean_antigravity_mode: true,
            plan_workspace: Default::default(),
            mcp_servers: Default::default(),
        }
    }

    fn sample_journal(run_id: &str, revision: u64, status: RunRecoveryStatus) -> RunJournal {
        let current_state = match status {
            RunRecoveryStatus::Complete => WorkflowState::Complete,
            RunRecoveryStatus::Failed => WorkflowState::Failed,
            RunRecoveryStatus::Cancelled => WorkflowState::Cancelled,
            RunRecoveryStatus::Active | RunRecoveryStatus::Interrupted => WorkflowState::PlanGeneration,
        };
        RunJournal {
            schema_version: JOURNAL_SCHEMA_VERSION,
            run_id: run_id.to_string(),
            workflow_type: "full_loop".to_string(),
            canonical_project_path: "C:\\dev\\project".to_string(),
            task_prompt: Some("Implement recovery".to_string()),
            approved_plan: Some("Approved recovery plan".to_string()),
            snapshot: sample_snapshot(),
            current_state,
            last_successful_state: Some(WorkflowState::BuildingContext),
            stage_entry_info: Some("Drafting initial plan".to_string()),
            iteration_counters: RunIterationCounters {
                plan_review_count: 1,
                fix_count: 0,
                code_review_count: 0,
                antigravity_dispatches: Some(1),
                antigravity_task_dispatches: std::collections::HashMap::new(),
                mailbox_epoch: Some(1),
            },
            direct_mcp_invocations: Vec::new(),
            plan_convergence_audit: Vec::new(),
            plan_convergence_confirmation_intent: None,
            usage: None,
            next_dispatch_ordinal: 0,
            adapter_dispatches: Vec::new(),
            revision,
            resume_generation: 0,
            checkpoint_manifest_ref: Some("chk-123".to_string()),
            checkpoint_digest: Some("sha256:abcd".to_string()),
            last_shelve_backup_id: None,
            last_shelve_backup_digest: None,
            status,
            created_at_unix: 1700000000,
            updated_at_unix: 1700000100,
        }
    }

    #[test]
    fn test_journal_round_trip_serialization() {
        let journal = sample_journal("run-test-1", 1, RunRecoveryStatus::Active);
        let serialized = serde_json::to_string_pretty(&journal).unwrap();
        let deserialized: RunJournal = serde_json::from_str(&serialized).unwrap();
        assert_eq!(journal, deserialized);
    }

    #[test]
    fn phase_a_journal_without_approved_plan_remains_readable() {
        let journal = sample_journal("phase-a-legacy", 1, RunRecoveryStatus::Interrupted);
        let mut value = serde_json::to_value(journal).unwrap();
        value.as_object_mut().unwrap().remove("approvedPlan");
        let deserialized: RunJournal = serde_json::from_value(value).unwrap();
        assert_eq!(deserialized.approved_plan, None);
    }

    #[test]
    fn test_checkpoint_manifest_round_trip() {
        let manifest = CheckpointManifest {
            schema_version: CHECKPOINT_SCHEMA_VERSION,
            checkpoint_id: "chk-001".to_string(),
            run_id: "run-test-1".to_string(),
            stage: WorkflowState::Implementation,
            kind: CheckpointKind::StageCheckpoint,
            created_at_unix: 1700000050,
            repository: RepositoryCheckpoint {
                canonical_root: "C:\\dev\\project".to_string(),
                head_oid: "a".repeat(40),
                refs_snapshot: "refs/heads/main aaaa".to_string(),
                status_inventory: b"M src/lib.rs\0".to_vec(),
                tracked_changes: vec![TrackedChangeRecord {
                    path: "src/lib.rs".to_string(),
                    change_type: TrackedChangeType::Modified,
                    old_path: None,
                    content_hash: Some("sha256:1111".to_string()),
                    mode: Some(0o644),
                }],
                untracked_files: vec![UntrackedCheckpointEntry {
                    rel_path: "untracked.txt".to_string(),
                    file_type: "file".to_string(),
                    content_hash: "sha256:2222".to_string(),
                    mode: 0o644,
                    symlink_target: None,
                }],
                integrity_hash: "sha256:repo_hash".to_string(),
            },
            payload_entries: vec![],
            index_blob_hash: "0".repeat(64),
            task_prompt: None,
            stage_input: None,
            submodules: vec![SubmoduleCheckpoint {
                node_key: "vendor/sub".to_string(),
                immediate_parent_key: ".".to_string(),
                immediate_parent_path: "C:\\dev\\project".to_string(),
                rel_path: "vendor/sub".to_string(),
                gitlink_oid: "b".repeat(40),
                head_oid: "b".repeat(40),
                repository: RepositoryCheckpoint {
                    canonical_root: "C:\\dev\\project\\vendor\\sub".to_string(),
                    head_oid: "b".repeat(40),
                    refs_snapshot: "refs/heads/main bbbb".to_string(),
                    status_inventory: vec![],
                    tracked_changes: vec![],
                    untracked_files: vec![],
                    integrity_hash: "sha256:sub_hash".to_string(),
                },
                payload_entries: vec![CheckpointPayloadEntry {
                    path: "sub_file.txt".to_string(),
                    kind: "file".to_string(),
                    mode: 0o644,
                    size: 10,
                    blob_hash: Some("sha256:subblob".to_string()),
                    symlink_target: None,
                }],
                index_blob_hash: "1".repeat(64),
            }],
            manifest_digest: "sha256:manifest_hash".to_string(),
        };

        let serialized = serde_json::to_string_pretty(&manifest).unwrap();
        let deserialized: CheckpointManifest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(manifest, deserialized);
    }

    #[test]
    fn test_legacy_v2_checkpoint_manifest_deserialization() {
        let legacy_v2_json = r#"{
            "schemaVersion": 2,
            "checkpointId": "chk-v2-legacy",
            "runId": "run-legacy-2",
            "stage": "implementation",
            "kind": "stage_checkpoint",
            "createdAtUnix": 1700000000,
            "repository": {
                "canonicalRoot": "C:\\dev\\project",
                "headOid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "refsSnapshot": "refs/heads/main aaaa",
                "statusInventory": [],
                "trackedChanges": [],
                "untrackedFiles": [],
                "integrityHash": "sha256:root_hash"
            },
            "payloadEntries": [],
            "indexBlobHash": "0000000000000000000000000000000000000000000000000000000000000000",
            "submodules": [
                {
                    "immediateParentPath": "C:\\dev\\project",
                    "relPath": "vendor/sub",
                    "gitlinkOid": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "headOid": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "repository": {
                        "canonicalRoot": "C:\\dev\\project\\vendor\\sub",
                        "headOid": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                        "refsSnapshot": "refs/heads/main bbbb",
                        "statusInventory": [],
                        "trackedChanges": [],
                        "untrackedFiles": [],
                        "integrityHash": "sha256:sub_hash"
                    }
                }
            ],
            "manifestDigest": "sha256:manifest_hash"
        }"#;

        let deserialized: CheckpointManifest = serde_json::from_str(legacy_v2_json).unwrap();
        assert_eq!(deserialized.schema_version, 2);
        assert_eq!(deserialized.submodules.len(), 1);
        assert_eq!(deserialized.submodules[0].node_key, "");
        assert_eq!(deserialized.submodules[0].payload_entries, vec![]);
        assert_eq!(deserialized.submodules[0].index_blob_hash, "");
    }

    #[test]
    fn test_journal_manager_atomic_write_and_read() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JournalManager::new(temp.path().join("runs"));

        let journal = sample_journal("run-write-1", 1, RunRecoveryStatus::Active);
        manager.write_journal(&journal).unwrap();

        let loaded = manager.read_journal("run-write-1").unwrap();
        assert_eq!(journal, loaded);
    }

    #[test]
    fn test_journal_manager_rejects_stale_revision() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JournalManager::new(temp.path().join("runs"));

        let j1 = sample_journal("run-rev-1", 5, RunRecoveryStatus::Active);
        manager.write_journal(&j1).unwrap();

        let mut j_stale = sample_journal("run-rev-1", 4, RunRecoveryStatus::Active);
        j_stale.current_state = WorkflowState::Validation;
        let res = manager.write_journal(&j_stale);

        assert!(res.is_err(), "Must reject stale revision");
        assert!(res.unwrap_err().contains("Stale journal write rejected"));

        let current = manager.read_journal("run-rev-1").unwrap();
        assert_eq!(current.revision, 5);
        assert_eq!(current.current_state, WorkflowState::PlanGeneration);
    }

    #[test]
    fn test_startup_classification_active_becomes_interrupted() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JournalManager::new(temp.path().join("runs"));

        let active_journal = sample_journal("run-active", 1, RunRecoveryStatus::Active);
        manager.write_journal(&active_journal).unwrap();

        let complete_journal = sample_journal("run-complete", 1, RunRecoveryStatus::Complete);
        manager.write_journal(&complete_journal).unwrap();

        let failed_journal = sample_journal("run-failed", 1, RunRecoveryStatus::Failed);
        manager.write_journal(&failed_journal).unwrap();

        let list = manager.list_journals().unwrap();
        assert_eq!(list.len(), 3);

        let active_summary = list.iter().find(|s| s.run_id == "run-active").unwrap();
        assert_eq!(active_summary.status, RunRecoveryStatus::Interrupted);
        assert!(!active_summary.is_resumable, "Phase A reports is_resumable=false");
        assert!(active_summary.error.is_none());

        let complete_summary = list.iter().find(|s| s.run_id == "run-complete").unwrap();
        assert_eq!(complete_summary.status, RunRecoveryStatus::Complete);
        assert!(!complete_summary.is_resumable);

        let failed_summary = list.iter().find(|s| s.run_id == "run-failed").unwrap();
        assert_eq!(failed_summary.status, RunRecoveryStatus::Failed);
        assert!(!failed_summary.is_resumable);
    }

    #[test]
    fn test_zero_content_bytes_written_on_permission_failure() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JournalManager::new(temp.path().join("runs"));
        *manager.permissions_test_hook.lock().unwrap() = Some(Arc::new(|_path| {
            Err("Injected permission setup failure".to_string())
        }));

        let journal = sample_journal("run-zero-bytes", 1, RunRecoveryStatus::Active);
        let res = manager.write_journal(&journal);

        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Injected permission setup failure"));

        let target_path = temp.path().join("runs").join("run-zero-bytes.json");
        assert!(!target_path.exists(), "Target journal must not exist on permission failure");

        // Verify no orphaned temporary files exist in runs directory
        let files: Vec<_> = fs::read_dir(temp.path().join("runs")).unwrap().collect();
        assert!(files.is_empty(), "No temporary file should remain on permission failure");
    }

    #[test]
    fn test_existing_runs_directory_permissions_rechecked() {
        let temp = tempfile::tempdir().unwrap();
        let runs_dir = temp.path().join("runs");
        fs::create_dir_all(&runs_dir).unwrap();

        let manager = JournalManager::new(runs_dir);
        // Ensure directory rechecks permissions on existing folder without error
        assert!(manager.ensure_directory().is_ok());
    }

    #[test]
    fn test_read_and_list_fail_before_exposing_journals_when_directory_protection_fails() {
        let temp = tempfile::tempdir().unwrap();
        let runs_dir = temp.path().join("runs");
        let manager = JournalManager::new(runs_dir.clone());
        let journal = sample_journal("protected-access", 1, RunRecoveryStatus::Active);
        manager.write_journal(&journal).unwrap();
        assert!(runs_dir.exists(), "test exercises an already-existing runs directory");

        let verification_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = Arc::clone(&verification_count);
        *manager.directory_permissions_test_hook.lock().unwrap() = Some(Arc::new(move |path| {
            assert!(path.exists());
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err("injected runs-directory protection verification failure".to_string())
        }));

        let read_error = manager.read_journal("protected-access").unwrap_err();
        assert!(read_error.contains("injected runs-directory protection verification failure"));
        let list_error = manager.list_journals().unwrap_err();
        assert!(list_error.contains("injected runs-directory protection verification failure"));
        assert_eq!(verification_count.load(std::sync::atomic::Ordering::SeqCst), 2);
        // The existing journal is preserved; neither API returns its contents after failed verification.
        assert!(runs_dir.join("protected-access.json").exists());
    }

    #[test]
    fn test_read_and_list_succeed_after_directory_protection_verification() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JournalManager::new(temp.path().join("runs"));
        let journal = sample_journal("protected-access-ok", 1, RunRecoveryStatus::Active);
        manager.write_journal(&journal).unwrap();

        let read_back = manager.read_journal("protected-access-ok").unwrap();
        assert_eq!(read_back, journal);
        let summaries = manager.list_journals().unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].run_id, "protected-access-ok");
    }

    #[test]
    fn test_validate_run_id_rejects_path_traversal() {
        assert!(validate_run_id("").is_err());
        assert!(validate_run_id("   ").is_err());
        assert!(validate_run_id("../escape").is_err());
        assert!(validate_run_id("dir/subdir").is_err());
        assert!(validate_run_id("dir\\subdir").is_err());
        assert!(validate_run_id("run;bad").is_err());
        assert!(validate_run_id("valid-run_id.123").is_ok());
        assert!(validate_run_id("4a7844eb-fc56-427a-8f12-c2053fe9a80e").is_ok());
    }

    #[test]
    fn test_corrupt_and_invalid_journals_enumerated_as_errors() {
        let temp = tempfile::tempdir().unwrap();
        let runs_dir = temp.path().join("runs");
        fs::create_dir_all(&runs_dir).unwrap();

        // 1. Corrupt JSON file
        fs::write(runs_dir.join("run-corrupt.json"), b"{ invalid json ").unwrap();

        // 2. Schema mismatch file
        let mut bad_schema = sample_journal("run-bad-schema", 1, RunRecoveryStatus::Active);
        bad_schema.schema_version = 999;
        fs::write(runs_dir.join("run-bad-schema.json"), serde_json::to_vec_pretty(&bad_schema).unwrap()).unwrap();

        let manager = JournalManager::new(runs_dir);
        let list = manager.list_journals().unwrap();
        assert_eq!(list.len(), 2);

        let corrupt = list.iter().find(|s| s.run_id == "run-corrupt").unwrap();
        assert_eq!(corrupt.status, RunRecoveryStatus::Failed);
        assert!(!corrupt.is_resumable);
        assert!(corrupt.error.as_ref().unwrap().contains("Malformed journal JSON"));

        let bad_sch = list.iter().find(|s| s.run_id == "run-bad-schema").unwrap();
        assert_eq!(bad_sch.status, RunRecoveryStatus::Failed);
        assert!(!bad_sch.is_resumable);
        assert!(bad_sch
            .error
            .as_ref()
            .unwrap()
            .contains("Unsupported journal schema version 999"));
    }

    #[test]
    fn test_write_journal_fails_closed_and_preserves_corrupt_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let runs_dir = temp.path().join("runs");
        let manager = JournalManager::new(runs_dir.clone());
        manager.ensure_directory().unwrap();

        let target_file = runs_dir.join("run-corrupt-target.json");
        let corrupt_content = b"{\"corrupt\": true, \"unclosed_json\": ";
        fs::write(&target_file, corrupt_content).unwrap();

        let journal = sample_journal("run-corrupt-target", 1, RunRecoveryStatus::Active);

        let res = manager.write_journal(&journal);
        assert!(res.is_err(), "write_journal must fail on corrupt existing file");
        let err_msg = res.unwrap_err();
        assert!(err_msg.contains("corrupt or invalid JSON"), "Error was: {err_msg}");

        // Assert file bytes remain strictly unchanged
        let current_bytes = fs::read(&target_file).unwrap();
        assert_eq!(current_bytes, corrupt_content, "Corrupt file must remain byte-for-byte untouched");
    }

    #[test]
    fn test_write_journal_fails_closed_on_existing_target_metadata_error() {
        let temp = tempfile::tempdir().unwrap();
        let runs_dir = temp.path().join("runs");
        let manager = JournalManager::new(runs_dir.clone());
        manager.ensure_directory().unwrap();

        let target = runs_dir.join("metadata-error.json");
        let original = b"existing journal bytes must remain untouched";
        fs::write(&target, original).unwrap();
        *manager.existing_metadata_test_hook.lock().unwrap() = Some(Arc::new(|path| {
            assert!(path.exists(), "injected metadata failure targets an existing file");
            Err("injected metadata inspection failure".to_string())
        }));

        let replacement = sample_journal("metadata-error", 2, RunRecoveryStatus::Active);
        let error = manager.write_journal(&replacement).unwrap_err();
        assert!(error.contains("injected metadata inspection failure"), "{error}");
        assert_eq!(fs::read(&target).unwrap(), original);
    }

    #[test]
    fn test_write_journal_fails_closed_on_unknown_schema_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let runs_dir = temp.path().join("runs");
        let manager = JournalManager::new(runs_dir.clone());
        manager.ensure_directory().unwrap();

        let target_file = runs_dir.join("run-unknown-schema.json");
        let mut bad_journal = sample_journal("run-unknown-schema", 1, RunRecoveryStatus::Active);
        bad_journal.schema_version = 42;
        let serialized = serde_json::to_vec_pretty(&bad_journal).unwrap();
        fs::write(&target_file, &serialized).unwrap();

        let valid_journal = sample_journal("run-unknown-schema", 2, RunRecoveryStatus::Active);

        let res = manager.write_journal(&valid_journal);
        assert!(res.is_err(), "write_journal must fail on unsupported schema existing file");
        assert!(res.unwrap_err().contains("unsupported schema version 42"));

        // Assert file bytes remain strictly untouched
        let current_bytes = fs::read(&target_file).unwrap();
        assert_eq!(current_bytes, serialized);
    }

    #[test]
    fn test_write_journal_fails_closed_on_run_id_mismatch_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let runs_dir = temp.path().join("runs");
        let manager = JournalManager::new(runs_dir.clone());
        manager.ensure_directory().unwrap();

        let target_file = runs_dir.join("run-target-id.json");
        let mismatch_journal = sample_journal("other-run-id", 1, RunRecoveryStatus::Active);
        let serialized = serde_json::to_vec_pretty(&mismatch_journal).unwrap();
        fs::write(&target_file, &serialized).unwrap();

        let valid_journal = sample_journal("run-target-id", 2, RunRecoveryStatus::Active);

        let res = manager.write_journal(&valid_journal);
        assert!(res.is_err(), "write_journal must fail on run ID mismatch");
        assert!(res.unwrap_err().contains("mismatched run ID"));

        // Assert file bytes remain untouched
        let current_bytes = fs::read(&target_file).unwrap();
        assert_eq!(current_bytes, serialized);
    }

    #[test]
    fn test_semantically_invalid_existing_journal_is_rejected_without_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let runs_dir = temp.path().join("runs");
        let manager = JournalManager::new(runs_dir.clone());
        manager.ensure_directory().unwrap();

        let target = runs_dir.join("invalid-semantic.json");
        let mut invalid = sample_journal("invalid-semantic", 1, RunRecoveryStatus::Active);
        invalid.checkpoint_digest = None;
        let original = serde_json::to_vec(&invalid).unwrap();
        fs::write(&target, &original).unwrap();

        let replacement = sample_journal("invalid-semantic", 2, RunRecoveryStatus::Active);
        let error = manager.write_journal(&replacement).unwrap_err();
        assert!(error.contains("Checkpoint manifest reference and digest"));
        assert_eq!(fs::read(&target).unwrap(), original);
        assert!(manager
            .read_journal("invalid-semantic")
            .unwrap_err()
            .contains("Checkpoint manifest reference and digest"));
        let summary = manager.list_journals().unwrap();
        assert_eq!(summary.len(), 1);
        assert!(summary[0]
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("Checkpoint manifest reference and digest"));
    }

    #[test]
    fn test_journal_semantic_validator_rejects_invalid_boundaries() {
        let mut journal = sample_journal("semantic-validation", 1, RunRecoveryStatus::Active);
        journal.revision = 0;
        assert!(validate_journal(&journal).unwrap_err().contains("revision"));

        journal = sample_journal("semantic-validation", 1, RunRecoveryStatus::Active);
        journal.updated_at_unix = journal.created_at_unix - 1;
        assert!(validate_journal(&journal).unwrap_err().contains("precedes"));

        journal = sample_journal("semantic-validation", 1, RunRecoveryStatus::Complete);
        journal.current_state = WorkflowState::PlanGeneration;
        assert!(validate_journal(&journal).unwrap_err().contains("contradicts"));

        journal = sample_journal("semantic-validation", 1, RunRecoveryStatus::Active);
        journal.current_state = WorkflowState::Failed;
        assert!(validate_journal(&journal).unwrap_err().contains("contradicts"));
    }

    #[test]
    fn test_temp_cleanup_failure_is_reported_with_primary_write_error() {
        for primary_failure in ["permission", "content"] {
            let temp = tempfile::tempdir().unwrap();
            let runs_dir = temp.path().join("runs");
            let manager = JournalManager::new(runs_dir.clone());
            if primary_failure == "permission" {
                *manager.permissions_test_hook.lock().unwrap() = Some(Arc::new(|_| {
                    Err("injected permission failure".to_string())
                }));
            } else {
                *manager.write_test_hook.lock().unwrap() = Some(Arc::new(|_| {
                    Err("injected content write failure".to_string())
                }));
            }
            *manager.cleanup_test_hook.lock().unwrap() = Some(Arc::new(|path| {
                Err(format!("injected temp cleanup failure for {}", path.display()))
            }));

            let journal = sample_journal("cleanup-failure", 1, RunRecoveryStatus::Active);
            let error = manager.write_journal(&journal).unwrap_err();
            assert!(error.contains(&format!("injected {primary_failure}")), "{error}");
            assert!(error.contains("injected temp cleanup failure"), "{error}");
            assert!(!runs_dir.join("cleanup-failure.json").exists());
            let leftovers: Vec<_> = fs::read_dir(&runs_dir).unwrap().collect();
            assert_eq!(leftovers.len(), 1, "injected cleanup failure should leave only the reported temp file");
        }
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_dacl_protected_current_user_accepted() {
        let temp = tempfile::tempdir().unwrap();
        let test_file = temp.path().join("valid_dacl.json");
        fs::write(&test_file, b"test").unwrap();

        assert!(apply_permissions(&test_file, false).is_ok());
        assert!(verify_permissions(&test_file).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_dacl_foreign_ace_rejected() {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::Security::Authorization::{
            SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, NO_MULTIPLE_TRUSTEE,
            SET_ACCESS, SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_USER,
        };
        use windows_sys::Win32::Security::{
            CreateWellKnownSid, GetTokenInformation, TokenUser, WinWorldSid, ACL,
            DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
            SECURITY_MAX_SID_SIZE, TOKEN_QUERY, TOKEN_USER,
        };
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        #[link(name = "kernel32")]
        extern "system" {
            fn LocalFree(hmem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        }

        let temp = tempfile::tempdir().unwrap();
        let test_file = temp.path().join("foreign_ace.json");
        fs::write(&test_file, b"test").unwrap();

        unsafe {
            // Get user SID
            let mut token_handle: HANDLE = INVALID_HANDLE_VALUE;
            assert_ne!(OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle), 0);
            let mut token_info_len: u32 = 0;
            let _ = GetTokenInformation(token_handle, TokenUser, std::ptr::null_mut(), 0, &mut token_info_len);
            let mut token_buf = vec![0u8; token_info_len as usize];
            assert_ne!(GetTokenInformation(token_handle, TokenUser, token_buf.as_mut_ptr() as *mut _, token_info_len, &mut token_info_len), 0);
            CloseHandle(token_handle);
            let token_user = &*(token_buf.as_ptr() as *const TOKEN_USER);
            let user_sid = token_user.User.Sid;

            // Create well-known Everyone SID (WinWorldSid)
            let mut world_sid = vec![0u8; SECURITY_MAX_SID_SIZE as usize];
            let mut world_sid_size: u32 = SECURITY_MAX_SID_SIZE;
            assert_ne!(
                CreateWellKnownSid(
                    WinWorldSid,
                    std::ptr::null_mut(),
                    world_sid.as_mut_ptr() as *mut _,
                    &mut world_sid_size
                ),
                0
            );

            // Build explicit access entries for user SID and Everyone
            let mut entries: [EXPLICIT_ACCESS_W; 2] = std::mem::zeroed();
            entries[0].grfAccessPermissions = 0x1F01FF;
            entries[0].grfAccessMode = SET_ACCESS;
            entries[0].grfInheritance = 0;
            entries[0].Trustee.pMultipleTrustee = std::ptr::null_mut();
            entries[0].Trustee.MultipleTrusteeOperation = NO_MULTIPLE_TRUSTEE;
            entries[0].Trustee.TrusteeForm = TRUSTEE_IS_SID;
            entries[0].Trustee.TrusteeType = TRUSTEE_IS_USER;
            entries[0].Trustee.ptstrName = user_sid as *mut u16;

            entries[1].grfAccessPermissions = 0x1F01FF;
            entries[1].grfAccessMode = SET_ACCESS;
            entries[1].grfInheritance = 0;
            entries[1].Trustee.pMultipleTrustee = std::ptr::null_mut();
            entries[1].Trustee.MultipleTrusteeOperation = NO_MULTIPLE_TRUSTEE;
            entries[1].Trustee.TrusteeForm = TRUSTEE_IS_SID;
            entries[1].Trustee.TrusteeType = TRUSTEE_IS_USER;
            entries[1].Trustee.ptstrName = world_sid.as_mut_ptr() as *mut u16;

            let mut new_acl: *mut ACL = std::ptr::null_mut();
            assert_eq!(SetEntriesInAclW(2, entries.as_ptr(), std::ptr::null_mut(), &mut new_acl), 0);

            let wide_path: Vec<u16> = test_file.as_os_str().encode_wide().chain(Some(0)).collect();
            assert_eq!(
                SetNamedSecurityInfoW(
                    wide_path.as_ptr() as *mut u16,
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    new_acl,
                    std::ptr::null_mut(),
                ),
                0
            );
            LocalFree(new_acl as *mut std::ffi::c_void);
        }

        let verify_res = verify_permissions(&test_file);
        assert!(verify_res.is_err(), "Must reject foreign principal ACE");
        let err = verify_res.unwrap_err();
        assert!(err.contains("Foreign principal SID found in DACL"), "Error was: {err}");
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_dacl_unprotected_inheritance_rejected() {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::Security::Authorization::{
            SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, NO_MULTIPLE_TRUSTEE,
            SET_ACCESS, SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_USER,
        };
        use windows_sys::Win32::Security::{
            GetTokenInformation, TokenUser, ACL, DACL_SECURITY_INFORMATION,
            UNPROTECTED_DACL_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER,
        };
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        #[link(name = "kernel32")]
        extern "system" {
            fn LocalFree(hmem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        }

        let temp = tempfile::tempdir().unwrap();
        let test_file = temp.path().join("unprotected_dacl.json");
        fs::write(&test_file, b"test").unwrap();

        unsafe {
            let mut token_handle: HANDLE = INVALID_HANDLE_VALUE;
            assert_ne!(OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle), 0);
            let mut token_info_len: u32 = 0;
            let _ = GetTokenInformation(token_handle, TokenUser, std::ptr::null_mut(), 0, &mut token_info_len);
            let mut token_buf = vec![0u8; token_info_len as usize];
            assert_ne!(GetTokenInformation(token_handle, TokenUser, token_buf.as_mut_ptr() as *mut _, token_info_len, &mut token_info_len), 0);
            CloseHandle(token_handle);
            let token_user = &*(token_buf.as_ptr() as *const TOKEN_USER);
            let user_sid = token_user.User.Sid;

            let mut explicit_access: EXPLICIT_ACCESS_W = std::mem::zeroed();
            explicit_access.grfAccessPermissions = 0x1F01FF;
            explicit_access.grfAccessMode = SET_ACCESS;
            explicit_access.grfInheritance = 0;
            explicit_access.Trustee.pMultipleTrustee = std::ptr::null_mut();
            explicit_access.Trustee.MultipleTrusteeOperation = NO_MULTIPLE_TRUSTEE;
            explicit_access.Trustee.TrusteeForm = TRUSTEE_IS_SID;
            explicit_access.Trustee.TrusteeType = TRUSTEE_IS_USER;
            explicit_access.Trustee.ptstrName = user_sid as *mut u16;

            let mut new_acl: *mut ACL = std::ptr::null_mut();
            assert_eq!(SetEntriesInAclW(1, &explicit_access, std::ptr::null_mut(), &mut new_acl), 0);

            let wide_path: Vec<u16> = test_file.as_os_str().encode_wide().chain(Some(0)).collect();
            assert_eq!(
                SetNamedSecurityInfoW(
                    wide_path.as_ptr() as *mut u16,
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION | UNPROTECTED_DACL_SECURITY_INFORMATION,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    new_acl,
                    std::ptr::null_mut(),
                ),
                0
            );
            LocalFree(new_acl as *mut std::ffi::c_void);
        }

        let verify_res = verify_permissions(&test_file);
        assert!(verify_res.is_err(), "Must reject unprotected/inheritable DACL");
        let err = verify_res.unwrap_err();
        assert!(err.contains("is not protected (inheritance not disabled)"), "Error was: {err}");
    }

    #[test]
    fn test_stage_completion_boundary_and_counters_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JournalManager::new(temp.path().join("runs"));

        let mut journal = sample_journal("run-stage-1", 1, RunRecoveryStatus::Active);
        journal.last_successful_state = None;
        journal.current_state = WorkflowState::BuildingContext;
        journal.iteration_counters = RunIterationCounters {
            plan_review_count: 2,
            fix_count: 3,
            code_review_count: 1,
            antigravity_dispatches: Some(4),
            antigravity_task_dispatches: std::collections::HashMap::new(),
            mailbox_epoch: Some(3),
        };

        manager.write_journal(&journal).unwrap();
        let loaded = manager.read_journal("run-stage-1").unwrap();
        assert_eq!(loaded.last_successful_state, None);
        assert_eq!(loaded.iteration_counters.plan_review_count, 2);
        assert_eq!(loaded.iteration_counters.fix_count, 3);
        assert_eq!(loaded.iteration_counters.code_review_count, 1);
        assert_eq!(loaded.iteration_counters.antigravity_dispatches, Some(4));

        // Advance successful boundary explicitly
        journal.revision = 2;
        journal.last_successful_state = Some(WorkflowState::PlanGeneration);
        journal.current_state = WorkflowState::Implementation;
        manager.write_journal(&journal).unwrap();

        let loaded2 = manager.read_journal("run-stage-1").unwrap();
        assert_eq!(loaded2.last_successful_state, Some(WorkflowState::PlanGeneration));
        assert_eq!(loaded2.current_state, WorkflowState::Implementation);

        // A failed stage does not clear or regress the last successful boundary
        journal.revision = 3;
        journal.current_state = WorkflowState::Failed;
        journal.status = RunRecoveryStatus::Failed;
        manager.write_journal(&journal).unwrap();

        let loaded3 = manager.read_journal("run-stage-1").unwrap();
        assert_eq!(loaded3.last_successful_state, Some(WorkflowState::PlanGeneration));
        assert_eq!(loaded3.status, RunRecoveryStatus::Failed);
    }

    #[test]
    fn test_old_run_journal_schema_v1_reads_cleanly() {
        // Legacy JSON lacking usage, next_dispatch_ordinal, and adapter_dispatches
        let legacy_json = serde_json::json!({
            "schemaVersion": 1,
            "runId": "run-legacy-1",
            "workflowType": "test",
            "canonicalProjectPath": "C:\\test",
            "taskPrompt": "test task",
            "snapshot": {
                "projectPath": "C:\\test",
                "assignments": {},
                "iterationLimits": {
                    "maxPlanReviewIterations": 3,
                    "maxFixIterations": 3,
                    "maxCodeReviewIterations": 3,
                    "maxAntigravityDispatches": 10
                },
                "validationGates": [],
                "budgetLimits": {},
                "createdAtUnix": 100,
                "leanAntigravityMode": false,
                "planWorkspace": {
                    "canonicalRoot": "C:\\test",
                    "planDir": "C:\\test\\.plan",
                    "archiveDir": "C:\\test\\.plan\\archive",
                    "primaryPlanId": "V0.23.0-r1.md",
                    "supplementalPlanIds": []
                },
                "mcpServers": {}
            },
            "currentState": "building_context",
            "iterationCounters": {
                "planReviewCount": 0,
                "fixCount": 0,
                "codeReviewCount": 0
            },
            "revision": 1,
            "resumeGeneration": 0,
            "status": "active",
            "createdAtUnix": 100,
            "updatedAtUnix": 100
        });

        let deserialized: RunJournal = serde_json::from_value(legacy_json).unwrap();
        assert_eq!(deserialized.schema_version, 1);
        assert_eq!(deserialized.usage, None);
        assert_eq!(deserialized.next_dispatch_ordinal, 0);
        assert!(deserialized.adapter_dispatches.is_empty());
    }

    #[test]
    fn test_usage_omitted_or_null_reads_as_none_and_none_omits_field() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JournalManager::new(temp.path().join("runs"));
        let mut journal = sample_journal("run-null-usage", 1, RunRecoveryStatus::Active);
        journal.usage = None;

        // Writing journal with usage == None omits the "usage" field
        manager.write_journal(&journal).unwrap();
        let target_path = temp.path().join("runs").join("run-null-usage.json");
        let content = fs::read_to_string(&target_path).unwrap();
        let parsed_val: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed_val.get("usage").is_none());

        // Explicit null in JSON deserializes to None
        let mut modified_val = parsed_val.clone();
        modified_val.as_object_mut().unwrap().insert("usage".to_string(), serde_json::Value::Null);
        let deserialized: RunJournal = serde_json::from_value(modified_val).unwrap();
        assert_eq!(deserialized.usage, None);
    }

    #[test]
    fn test_usage_some_zero_retained_distinct_from_none() {
        use crate::orchestrator::adapters::NormalizedUsage;

        let usage_zero = NormalizedUsage {
            input_tokens: Some(0),
            output_tokens: Some(0),
            raw_total_tokens: Some(0),
            cache_read_tokens: Some(0),
            cache_write_tokens: None,
        };

        let temp = tempfile::tempdir().unwrap();
        let manager = JournalManager::new(temp.path().join("runs"));
        let mut journal = sample_journal("run-zero-usage", 1, RunRecoveryStatus::Active);
        journal.usage = Some(usage_zero.clone());

        manager.write_journal(&journal).unwrap();
        let loaded = manager.read_journal("run-zero-usage").unwrap();

        assert!(loaded.usage.is_some());
        let loaded_usage = loaded.usage.unwrap();
        assert_eq!(loaded_usage.input_tokens, Some(0));
        assert_ne!(loaded_usage.input_tokens, None);
        assert_eq!(loaded_usage.output_tokens, Some(0));
        assert_eq!(loaded_usage.raw_total_tokens, Some(0));
        assert_eq!(loaded_usage.cache_read_tokens, Some(0));
        assert_eq!(loaded_usage.cache_write_tokens, None);
    }

    #[test]
    fn test_journal_roundtrip_with_usage() {
        use crate::orchestrator::adapters::NormalizedUsage;

        let usage = NormalizedUsage {
            input_tokens: Some(150),
            output_tokens: Some(42),
            raw_total_tokens: Some(192),
            cache_read_tokens: Some(50),
            cache_write_tokens: Some(25),
        };

        let temp = tempfile::tempdir().unwrap();
        let manager = JournalManager::new(temp.path().join("runs"));
        let mut journal = sample_journal("run-roundtrip-usage", 1, RunRecoveryStatus::Active);
        journal.usage = Some(usage.clone());
        journal.next_dispatch_ordinal = 3;
        journal.adapter_dispatches.push(AdapterDispatchRecord {
            dispatch_id: "run-roundtrip-usage:1".to_string(),
            ordinal: 1,
            role: "planner".to_string(),
            profile_id: "profile-1".to_string(),
            model: Some("model-a".to_string()),
            payload_digest: Some("hash1".to_string()),
            usage: Some(usage.clone()),
            status: Some("completed".to_string()),
            created_at_unix: 100,
            completed_at_unix: Some(105),
        });

        manager.write_journal(&journal).unwrap();
        let loaded = manager.read_journal("run-roundtrip-usage").unwrap();

        assert_eq!(loaded.schema_version, 1);
        assert_eq!(loaded.usage, Some(usage));
        assert_eq!(loaded.next_dispatch_ordinal, 3);
        assert_eq!(loaded.adapter_dispatches.len(), 1);
        assert_eq!(loaded.adapter_dispatches[0].dispatch_id, "run-roundtrip-usage:1");
    }

    #[test]
    fn test_ordinal_monotonic_increase_and_distinct_ids_on_repeated_role() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JournalManager::new(temp.path().join("runs"));
        let journal = sample_journal("run-monotonic", 1, RunRecoveryStatus::Active);
        manager.write_journal(&journal).unwrap();

        let ord1 = manager.reserve_dispatch_ordinal("run-monotonic").unwrap();
        let ord2 = manager.reserve_dispatch_ordinal("run-monotonic").unwrap();
        let ord3 = manager.reserve_dispatch_ordinal("run-monotonic").unwrap();

        assert_eq!(ord1, 1);
        assert_eq!(ord2, 2);
        assert_eq!(ord3, 3);

        let id1 = format!("run-monotonic:{ord1}");
        let id2 = format!("run-monotonic:{ord2}");
        let id3 = format!("run-monotonic:{ord3}");

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);

        // Repeated role dispatch
        let res1 = manager.record_adapter_dispatch("run-monotonic", AdapterDispatchRecord {
            dispatch_id: id1.clone(),
            ordinal: ord1,
            role: "planner".to_string(),
            profile_id: "prof-1".to_string(),
            model: None,
            payload_digest: Some("digest-1".to_string()),
            usage: None,
            status: Some("completed".to_string()),
            created_at_unix: 1,
            completed_at_unix: Some(2),
        });
        assert!(res1.is_ok());

        let res2 = manager.record_adapter_dispatch("run-monotonic", AdapterDispatchRecord {
            dispatch_id: id2.clone(),
            ordinal: ord2,
            role: "planner".to_string(), // repeated role
            profile_id: "prof-1".to_string(),
            model: None,
            payload_digest: Some("digest-2".to_string()),
            usage: None,
            status: Some("completed".to_string()),
            created_at_unix: 3,
            completed_at_unix: Some(4),
        });
        assert!(res2.is_ok());

        let loaded = manager.read_journal("run-monotonic").unwrap();
        assert_eq!(loaded.adapter_dispatches.len(), 2);
        assert_eq!(loaded.adapter_dispatches[0].role, "planner");
        assert_eq!(loaded.adapter_dispatches[1].role, "planner");
        assert_ne!(loaded.adapter_dispatches[0].dispatch_id, loaded.adapter_dispatches[1].dispatch_id);
    }

    #[test]
    fn test_crash_recovery_never_reuses_reserved_ordinal() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JournalManager::new(temp.path().join("runs"));
        let journal = sample_journal("run-crash", 1, RunRecoveryStatus::Active);
        manager.write_journal(&journal).unwrap();

        // Reserve ordinal 1, simulate crash without recording dispatch
        let ord1 = manager.reserve_dispatch_ordinal("run-crash").unwrap();
        assert_eq!(ord1, 1);

        // After crash/restart, next reserved ordinal must be strictly greater (2)
        let manager2 = JournalManager::new(temp.path().join("runs"));
        let ord2 = manager2.reserve_dispatch_ordinal("run-crash").unwrap();
        assert_eq!(ord2, 2);
        assert!(ord2 > ord1);
    }

    #[test]
    fn test_dispatch_persistence_retry_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JournalManager::new(temp.path().join("runs"));
        let journal = sample_journal("run-idempotent", 1, RunRecoveryStatus::Active);
        manager.write_journal(&journal).unwrap();

        let ord = manager.reserve_dispatch_ordinal("run-idempotent").unwrap();
        let record = AdapterDispatchRecord {
            dispatch_id: format!("run-idempotent:{ord}"),
            ordinal: ord,
            role: "planner".to_string(),
            profile_id: "prof-1".to_string(),
            model: Some("m1".to_string()),
            payload_digest: Some("hash-abc".to_string()),
            usage: None,
            status: Some("completed".to_string()),
            created_at_unix: 10,
            completed_at_unix: Some(15),
        };

        // First attempt inserts
        let changed1 = manager.record_adapter_dispatch("run-idempotent", record.clone()).unwrap();
        assert!(changed1);

        // Retry with same ID and same payload is an idempotent no-op
        let changed2 = manager.record_adapter_dispatch("run-idempotent", record.clone()).unwrap();
        assert!(!changed2);

        let loaded = manager.read_journal("run-idempotent").unwrap();
        assert_eq!(loaded.adapter_dispatches.len(), 1);
    }

    #[test]
    fn test_dispatch_conflict_different_payload_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JournalManager::new(temp.path().join("runs"));
        let journal = sample_journal("run-conflict", 1, RunRecoveryStatus::Active);
        manager.write_journal(&journal).unwrap();

        let ord = manager.reserve_dispatch_ordinal("run-conflict").unwrap();
        let record1 = AdapterDispatchRecord {
            dispatch_id: format!("run-conflict:{ord}"),
            ordinal: ord,
            role: "planner".to_string(),
            profile_id: "prof-1".to_string(),
            model: Some("m1".to_string()),
            payload_digest: Some("hash-original".to_string()),
            usage: None,
            status: Some("completed".to_string()),
            created_at_unix: 10,
            completed_at_unix: Some(15),
        };
        manager.record_adapter_dispatch("run-conflict", record1).unwrap();

        // Same dispatch ID but different payload_digest => Conflict error
        let record2 = AdapterDispatchRecord {
            dispatch_id: format!("run-conflict:{ord}"),
            ordinal: ord,
            role: "planner".to_string(),
            profile_id: "prof-1".to_string(),
            model: Some("m1".to_string()),
            payload_digest: Some("hash-tampered".to_string()),
            usage: None,
            status: Some("completed".to_string()),
            created_at_unix: 10,
            completed_at_unix: Some(15),
        };
        let err = manager.record_adapter_dispatch("run-conflict", record2);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("PC_DISPATCH_CONFLICT"));
    }

    #[test]
    fn test_concurrent_ordinal_reservations_deduplicated() {
        let temp = tempfile::tempdir().unwrap();
        let manager = Arc::new(JournalManager::new(temp.path().join("runs")));
        let journal = sample_journal("run-concurrent", 1, RunRecoveryStatus::Active);
        manager.write_journal(&journal).unwrap();

        let mut handles = Vec::new();
        for _ in 0..10 {
            let m = manager.clone();
            handles.push(std::thread::spawn(move || {
                m.reserve_dispatch_ordinal("run-concurrent").unwrap()
            }));
        }

        let mut ordinals: Vec<u64> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        ordinals.sort();

        // Must be strictly unique from 1 to 10
        let expected: Vec<u64> = (1..=10).collect();
        assert_eq!(ordinals, expected);
    }

    #[test]
    fn test_concurrent_reserve_and_record_dispatch_different_instances() {
        let temp = tempfile::tempdir().unwrap();
        let runs_dir = temp.path().join("runs");
        let setup_manager = JournalManager::new(runs_dir.clone());
        let mut journal = sample_journal("run-concurrent-cross", 1, RunRecoveryStatus::Active);

        // Pre-populate 5 dispatches in 'reserved' state (ordinals 1..=5)
        for ord in 1..=5 {
            journal.adapter_dispatches.push(AdapterDispatchRecord {
                dispatch_id: format!("run-concurrent-cross:{ord}"),
                ordinal: ord,
                role: "planner".to_string(),
                profile_id: "prof-1".to_string(),
                model: Some("m1".to_string()),
                payload_digest: Some(format!("digest-{ord}")),
                usage: None,
                status: Some("reserved".to_string()),
                created_at_unix: 10,
                completed_at_unix: None,
            });
        }
        journal.next_dispatch_ordinal = 6;
        journal.revision = 2;
        setup_manager.write_journal(&journal).unwrap();

        // 5 threads will record 'completed' for ordinals 1..=5,
        // and 5 threads will reserve new ordinals (expecting 6..=10).
        // Each thread uses its own distinct JournalManager instance.
        let num_threads = 10;
        let barrier = Arc::new(std::sync::Barrier::new(num_threads));

        let mut handles = Vec::new();

        // Spawn 5 updater threads for record_adapter_dispatch
        for ord in 1..=5 {
            let runs_dir_clone = runs_dir.clone();
            let b = barrier.clone();
            let record = AdapterDispatchRecord {
                dispatch_id: format!("run-concurrent-cross:{ord}"),
                ordinal: ord,
                role: "planner".to_string(),
                profile_id: "prof-1".to_string(),
                model: Some("m1".to_string()),
                payload_digest: Some(format!("digest-{ord}")),
                usage: Some(NormalizedUsage {
                    input_tokens: Some(100),
                    output_tokens: Some(50),
                    raw_total_tokens: Some(150),
                    cache_read_tokens: None,
                    cache_write_tokens: None,
                }),
                status: Some("completed".to_string()),
                created_at_unix: 10,
                completed_at_unix: Some(20),
            };
            handles.push(std::thread::spawn(move || {
                let m = JournalManager::new(runs_dir_clone);
                b.wait();
                let changed = m.record_adapter_dispatch("run-concurrent-cross", record).unwrap();
                assert!(changed);
                None
            }));
        }

        // Spawn 5 reserver threads for reserve_dispatch_ordinal
        for _ in 0..5 {
            let runs_dir_clone = runs_dir.clone();
            let b = barrier.clone();
            handles.push(std::thread::spawn(move || {
                let m = JournalManager::new(runs_dir_clone);
                b.wait();
                let ord = m.reserve_dispatch_ordinal("run-concurrent-cross").unwrap();
                Some(ord)
            }));
        }

        let mut reserved_ordinals = Vec::new();
        for h in handles {
            if let Some(ord) = h.join().unwrap() {
                reserved_ordinals.push(ord);
            }
        }

        reserved_ordinals.sort();
        assert_eq!(reserved_ordinals, vec![6, 7, 8, 9, 10]);

        // Verify the final state on disk using another fresh JournalManager instance
        let verify_manager = JournalManager::new(runs_dir);
        let final_journal = verify_manager.read_journal("run-concurrent-cross").unwrap();

        // Initial revision 2 + 5 completed transitions + 5 reservations = 12
        assert_eq!(final_journal.revision, 12);
        assert_eq!(final_journal.next_dispatch_ordinal, 11);
        assert_eq!(final_journal.adapter_dispatches.len(), 5);

        for ord in 1..=5 {
            let d = final_journal
                .adapter_dispatches
                .iter()
                .find(|d| d.ordinal == ord)
                .expect("dispatch exists");
            assert_eq!(d.status.as_deref(), Some("completed"));
            assert_eq!(d.usage.as_ref().and_then(|u| u.input_tokens), Some(100));
        }

        let total_usage = final_journal.usage.expect("cumulative usage must exist");
        assert_eq!(total_usage.input_tokens, Some(500));
        assert_eq!(total_usage.output_tokens, Some(250));
        assert_eq!(total_usage.raw_total_tokens, Some(750));
    }

    #[test]
    fn test_dispatch_lock_poison_returns_error_without_panic() {
        let test_mutex = Arc::new(std::sync::Mutex::new(()));
        let m = test_mutex.clone();

        // Poison test_mutex by panicking inside a thread while holding the lock
        let handle = std::thread::spawn(move || {
            let _guard = m.lock().unwrap();
            panic!("intentional test panic to poison test mutex");
        });
        let join_res = handle.join();
        assert!(join_res.is_err(), "thread should have panicked");

        // Verify acquire_dispatch_lock returns Err with PC_DISPATCH_LOCK_POISONED without panicking
        let result = acquire_dispatch_lock(&test_mutex);
        assert!(result.is_err(), "acquire_dispatch_lock must return Err on poisoned mutex");
        let err_msg = result.err().unwrap();
        assert!(
            err_msg.contains("PC_DISPATCH_LOCK_POISONED"),
            "Error message must indicate poisoned lock: {err_msg}"
        );

        // Also verify both transaction paths fail closed with PC_DISPATCH_LOCK_POISONED when passed a poisoned lock
        let temp = tempfile::tempdir().unwrap();
        let manager = JournalManager::new(temp.path().join("runs"));
        let res_reserve = manager.reserve_dispatch_ordinal_with_lock("run-poison", &test_mutex);
        assert!(res_reserve.is_err());
        assert!(res_reserve.unwrap_err().contains("PC_DISPATCH_LOCK_POISONED"));

        let dummy_record = AdapterDispatchRecord {
            dispatch_id: "run-poison:1".to_string(),
            ordinal: 1,
            role: "planner".to_string(),
            profile_id: "p".to_string(),
            model: None,
            payload_digest: None,
            usage: None,
            status: Some("completed".to_string()),
            created_at_unix: 1,
            completed_at_unix: None,
        };
        let res_record = manager.record_adapter_dispatch_with_lock("run-poison", dummy_record, &test_mutex);
        assert!(res_record.is_err());
        assert!(res_record.unwrap_err().contains("PC_DISPATCH_LOCK_POISONED"));
    }

    #[test]
    fn test_concurrent_reserve_and_record_synchronized_seam_verifies_mutual_exclusion() {
        let temp = tempfile::tempdir().unwrap();
        let runs_dir = temp.path().join("runs");
        let setup_manager = JournalManager::new(runs_dir.clone());
        let journal = sample_journal("run-sync-seam", 1, RunRecoveryStatus::Active);
        setup_manager.write_journal(&journal).unwrap();

        let (tx_reserver_in_cs, rx_reserver_in_cs) = std::sync::mpsc::channel();
        let (tx_reserver_continue, rx_reserver_continue) = std::sync::mpsc::channel();
        let rx_reserver_continue = Arc::new(Mutex::new(rx_reserver_continue));
        let (tx_wouldblock_observed, rx_wouldblock_observed) = std::sync::mpsc::channel();

        let test_mutex = Arc::new(std::sync::Mutex::new(()));
        let test_mutex_t1 = test_mutex.clone();
        let test_mutex_t2 = test_mutex.clone();

        // Thread 1: JournalManager instance with reserve_test_hook installed
        let m1 = JournalManager::new(runs_dir.clone());
        let rx_cont_hook = rx_reserver_continue.clone();
        *m1.reserve_test_hook.lock().unwrap() = Some(Arc::new(move |_journal| {
            // Signal that Thread 1 has acquired the dispatch lock and read the journal
            tx_reserver_in_cs.send(()).unwrap();
            // Wait for Thread 2 to attempt lock acquisition and verify WouldBlock.
            // Timeout is solely a hang watchdog (not used for pass/fail determination).
            let ok = rx_cont_hook.lock().unwrap().recv_timeout(std::time::Duration::from_secs(5));
            assert!(ok.is_ok(), "hang watchdog: reserver should have received continue signal");
            Ok(())
        }));

        // Thread 2: completely distinct JournalManager instance for record_adapter_dispatch
        let runs_dir_t2 = runs_dir.clone();
        let handle_recorder = std::thread::spawn(move || {
            // Wait for Thread 1 to enter critical section (hang watchdog)
            let received = rx_reserver_in_cs.recv_timeout(std::time::Duration::from_secs(5));
            assert!(received.is_ok(), "hang watchdog: recorder must receive signal that reserver entered critical section");

            let m2 = JournalManager::new(runs_dir_t2);
            let tx_wb = tx_wouldblock_observed.clone();
            let tx_cont = tx_reserver_continue.clone();
            *m2.dispatch_lock_test_hook.lock().unwrap() = Some(Arc::new(move |lock| {
                // This hook executes inside m2's actual record_adapter_dispatch execution path
                // right before acquire_dispatch_lock. We verify that attempting to acquire the
                // lock via try_lock yields WouldBlock because Thread 1 is holding the lock.
                let try_res = lock.try_lock();
                match try_res {
                    Err(std::sync::TryLockError::WouldBlock) => {
                        // Confirmed WouldBlock: lock is held by Thread 1
                        tx_wb.send(true).unwrap();
                        // Unblock Thread 1 so it can complete and release the lock
                        tx_cont.send(()).unwrap();
                        Ok(())
                    }
                    Ok(_guard) => {
                        tx_wb.send(false).unwrap();
                        tx_cont.send(()).unwrap();
                        panic!("Regression detected: dispatch lock try_lock succeeded while another dispatch operation was active! Expected WouldBlock.");
                    }
                    Err(std::sync::TryLockError::Poisoned(e)) => {
                        panic!("Unexpected poisoned lock: {e}");
                    }
                }
            }));

            let record = AdapterDispatchRecord {
                dispatch_id: "run-sync-seam:100".to_string(),
                ordinal: 100,
                role: "planner".to_string(),
                profile_id: "prof-1".to_string(),
                model: Some("m1".to_string()),
                payload_digest: Some("digest-100".to_string()),
                usage: Some(NormalizedUsage {
                    input_tokens: Some(50),
                    output_tokens: Some(25),
                    raw_total_tokens: Some(75),
                    cache_read_tokens: None,
                    cache_write_tokens: None,
                }),
                status: Some("completed".to_string()),
                created_at_unix: 10,
                completed_at_unix: Some(20),
            };
            m2.record_adapter_dispatch_with_lock("run-sync-seam", record, &test_mutex_t2)
        });

        let handle_reserver = std::thread::spawn(move || {
            m1.reserve_dispatch_ordinal_with_lock("run-sync-seam", &test_mutex_t1)
        });

        // Deterministically verify that the second acquisition attempt yielded WouldBlock
        let would_block = rx_wouldblock_observed.recv_timeout(std::time::Duration::from_secs(5));
        assert!(would_block.is_ok(), "hang watchdog: should receive WouldBlock notification");
        assert!(would_block.unwrap(), "Second acquisition attempt must be WouldBlock");

        // Thread joining and cleanup (スレッド回収)
        let res_reserver = handle_reserver.join().expect("reserver thread should not panic");
        let res_recorder = handle_recorder.join().expect("recorder thread should not panic");

        assert!(res_reserver.is_ok(), "reserve_dispatch_ordinal must succeed: {:?}", res_reserver.err());
        assert!(res_recorder.is_ok(), "record_adapter_dispatch must succeed: {:?}", res_recorder.err());

        // Verify final journal state on disk with a fresh JournalManager instance
        let verify_manager = JournalManager::new(runs_dir);
        let final_journal = verify_manager.read_journal("run-sync-seam").unwrap();

        // 1 initial + 1 reservation + 1 record = 3
        assert_eq!(final_journal.revision, 3);
        assert_eq!(final_journal.adapter_dispatches.len(), 1);
        assert_eq!(final_journal.adapter_dispatches[0].ordinal, 100);
        assert_eq!(final_journal.adapter_dispatches[0].status.as_deref(), Some("completed"));
        // Ordinal reservation allocated ordinal 1 (initial empty journal had 0, so next is 1)
        assert_eq!(res_reserver.unwrap(), 1);
        // Next ordinal after 100 is 101
        assert_eq!(final_journal.next_dispatch_ordinal, 101);
    }

    #[test]
    fn test_concurrent_record_and_reserve_synchronized_seam_verifies_mutual_exclusion() {
        let temp = tempfile::tempdir().unwrap();
        let runs_dir = temp.path().join("runs");
        let setup_manager = JournalManager::new(runs_dir.clone());
        let journal = sample_journal("run-sync-rev", 1, RunRecoveryStatus::Active);
        setup_manager.write_journal(&journal).unwrap();

        let (tx_recorder_in_cs, rx_recorder_in_cs) = std::sync::mpsc::channel();
        let (tx_recorder_continue, rx_recorder_continue) = std::sync::mpsc::channel();
        let rx_recorder_continue = Arc::new(Mutex::new(rx_recorder_continue));
        let (tx_wouldblock_observed, rx_wouldblock_observed) = std::sync::mpsc::channel();

        let test_mutex = Arc::new(std::sync::Mutex::new(()));
        let test_mutex_t1 = test_mutex.clone();
        let test_mutex_t2 = test_mutex.clone();

        // Thread 1: JournalManager instance with record_test_hook installed
        let m1 = JournalManager::new(runs_dir.clone());
        let rx_cont_hook = rx_recorder_continue.clone();
        *m1.record_test_hook.lock().unwrap() = Some(Arc::new(move |_journal| {
            // Signal that Thread 1 has acquired the dispatch lock and read the journal
            tx_recorder_in_cs.send(()).unwrap();
            // Wait for Thread 2 to attempt lock acquisition and verify WouldBlock.
            // Timeout is solely a hang watchdog (not used for pass/fail determination).
            let ok = rx_cont_hook.lock().unwrap().recv_timeout(std::time::Duration::from_secs(5));
            assert!(ok.is_ok(), "hang watchdog: recorder should have received continue signal");
            Ok(())
        }));

        // Thread 2: completely distinct JournalManager instance for reserve_dispatch_ordinal
        let runs_dir_t2 = runs_dir.clone();
        let handle_reserver = std::thread::spawn(move || {
            // Wait for Thread 1 to enter critical section (hang watchdog)
            let received = rx_recorder_in_cs.recv_timeout(std::time::Duration::from_secs(5));
            assert!(received.is_ok(), "hang watchdog: reserver must receive signal that recorder entered critical section");

            let m2 = JournalManager::new(runs_dir_t2);
            let tx_wb = tx_wouldblock_observed.clone();
            let tx_cont = tx_recorder_continue.clone();
            *m2.dispatch_lock_test_hook.lock().unwrap() = Some(Arc::new(move |lock| {
                // This hook executes inside m2's actual reserve_dispatch_ordinal execution path
                // right before acquire_dispatch_lock. We verify that attempting to acquire the
                // lock via try_lock yields WouldBlock because Thread 1 is holding the lock.
                let try_res = lock.try_lock();
                match try_res {
                    Err(std::sync::TryLockError::WouldBlock) => {
                        // Confirmed WouldBlock: lock is held by Thread 1
                        tx_wb.send(true).unwrap();
                        // Unblock Thread 1 so it can complete and release the lock
                        tx_cont.send(()).unwrap();
                        Ok(())
                    }
                    Ok(_guard) => {
                        tx_wb.send(false).unwrap();
                        tx_cont.send(()).unwrap();
                        panic!("Regression detected: dispatch lock try_lock succeeded while another dispatch operation was active! Expected WouldBlock.");
                    }
                    Err(std::sync::TryLockError::Poisoned(e)) => {
                        panic!("Unexpected poisoned lock: {e}");
                    }
                }
            }));

            m2.reserve_dispatch_ordinal_with_lock("run-sync-rev", &test_mutex_t2)
        });

        let handle_recorder = std::thread::spawn(move || {
            let record = AdapterDispatchRecord {
                dispatch_id: "run-sync-rev:50".to_string(),
                ordinal: 50,
                role: "planner".to_string(),
                profile_id: "prof-1".to_string(),
                model: Some("m1".to_string()),
                payload_digest: Some("digest-50".to_string()),
                usage: Some(NormalizedUsage {
                    input_tokens: Some(30),
                    output_tokens: Some(15),
                    raw_total_tokens: Some(45),
                    cache_read_tokens: None,
                    cache_write_tokens: None,
                }),
                status: Some("completed".to_string()),
                created_at_unix: 10,
                completed_at_unix: Some(20),
            };
            m1.record_adapter_dispatch_with_lock("run-sync-rev", record, &test_mutex_t1)
        });

        // Deterministically verify that the second acquisition attempt yielded WouldBlock
        let would_block = rx_wouldblock_observed.recv_timeout(std::time::Duration::from_secs(5));
        assert!(would_block.is_ok(), "hang watchdog: should receive WouldBlock notification");
        assert!(would_block.unwrap(), "Second acquisition attempt must be WouldBlock");

        // Thread joining and cleanup (スレッド回収)
        let res_recorder = handle_recorder.join().expect("recorder thread should not panic");
        let res_reserver = handle_reserver.join().expect("reserver thread should not panic");

        assert!(res_recorder.is_ok(), "record_adapter_dispatch must succeed: {:?}", res_recorder.err());
        assert!(res_reserver.is_ok(), "reserve_dispatch_ordinal must succeed: {:?}", res_reserver.err());

        // Verify final journal state on disk with a fresh JournalManager instance
        let verify_manager = JournalManager::new(runs_dir);
        let final_journal = verify_manager.read_journal("run-sync-rev").unwrap();

        // 1 initial + 1 record + 1 reservation = 3
        assert_eq!(final_journal.revision, 3);
        assert_eq!(final_journal.adapter_dispatches.len(), 1);
        assert_eq!(final_journal.adapter_dispatches[0].ordinal, 50);
        assert_eq!(final_journal.adapter_dispatches[0].status.as_deref(), Some("completed"));
        // Ordinal reservation happened after record with ordinal 50, so next is 51
        assert_eq!(res_reserver.unwrap(), 51);
        assert_eq!(final_journal.next_dispatch_ordinal, 52);
    }

    #[test]
    fn test_dispatch_state_transitions_and_disallowed_transitions() {
        let mut journal = sample_journal("run-trans", 1, RunRecoveryStatus::Active);
        let ord = journal.reserve_dispatch_ordinal().unwrap();
        let dispatch_id = format!("run-trans:{ord}");

        // 1. Initial reservation
        let reserved = AdapterDispatchRecord {
            dispatch_id: dispatch_id.clone(),
            ordinal: ord,
            role: "planner".to_string(),
            profile_id: "prof-1".to_string(),
            model: Some("m1".to_string()),
            payload_digest: Some("hash-1".to_string()),
            usage: None,
            status: Some("reserved".to_string()),
            created_at_unix: 10,
            completed_at_unix: None,
        };
        assert!(journal.record_adapter_dispatch(reserved.clone()).unwrap());

        // Idempotent retry of reserved
        assert!(!journal.record_adapter_dispatch(reserved.clone()).unwrap());

        // Identity mutability rejection (e.g. changing model or role)
        let mut tampered = reserved.clone();
        tampered.model = Some("m2".to_string());
        assert!(journal.record_adapter_dispatch(tampered).is_err());

        // 2. Transition reserved -> completed
        let completed = AdapterDispatchRecord {
            dispatch_id: dispatch_id.clone(),
            ordinal: ord,
            role: "planner".to_string(),
            profile_id: "prof-1".to_string(),
            model: Some("m1".to_string()),
            payload_digest: Some("hash-1".to_string()),
            usage: Some(NormalizedUsage {
                input_tokens: Some(10),
                output_tokens: Some(5),
                raw_total_tokens: Some(15),
                cache_read_tokens: Some(0),
                cache_write_tokens: None,
            }),
            status: Some("completed".to_string()),
            created_at_unix: 10,
            completed_at_unix: Some(20),
        };
        assert!(journal.record_adapter_dispatch(completed.clone()).unwrap());

        // Idempotent retry of completed
        assert!(!journal.record_adapter_dispatch(completed.clone()).unwrap());

        // Disallowed: completed -> failed
        let mut to_failed = completed.clone();
        to_failed.status = Some("failed".to_string());
        assert!(journal.record_adapter_dispatch(to_failed).is_err());

        // Disallowed: completed -> reserved
        assert!(journal.record_adapter_dispatch(reserved).is_err());

        // 3. Test failure transition on a fresh dispatch
        let ord2 = journal.reserve_dispatch_ordinal().unwrap();
        let dispatch_id2 = format!("run-trans:{ord2}");
        let reserved2 = AdapterDispatchRecord {
            dispatch_id: dispatch_id2.clone(),
            ordinal: ord2,
            role: "plan_reviewer".to_string(),
            profile_id: "prof-2".to_string(),
            model: Some("m2".to_string()),
            payload_digest: Some("hash-2".to_string()),
            usage: None,
            status: Some("reserved".to_string()),
            created_at_unix: 30,
            completed_at_unix: None,
        };
        assert!(journal.record_adapter_dispatch(reserved2.clone()).unwrap());

        // Transition reserved -> failed
        let failed = AdapterDispatchRecord {
            dispatch_id: dispatch_id2.clone(),
            ordinal: ord2,
            role: "plan_reviewer".to_string(),
            profile_id: "prof-2".to_string(),
            model: Some("m2".to_string()),
            payload_digest: Some("hash-2".to_string()),
            usage: None,
            status: Some("failed".to_string()),
            created_at_unix: 30,
            completed_at_unix: Some(40),
        };
        assert!(journal.record_adapter_dispatch(failed.clone()).unwrap());

        // Idempotent retry of failed
        assert!(!journal.record_adapter_dispatch(failed.clone()).unwrap());

        // Disallowed: failed -> completed
        let mut to_completed = failed.clone();
        to_completed.status = Some("completed".to_string());
        assert!(journal.record_adapter_dispatch(to_completed).is_err());
    }

    #[test]
    fn test_dispatch_ordinal_overflow_rejection() {
        let mut journal = sample_journal("run-overflow", 1, RunRecoveryStatus::Active);
        journal.next_dispatch_ordinal = u64::MAX;
        let err = journal.reserve_dispatch_ordinal();
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("PC_DISPATCH_ORDINAL_OVERFLOW"));

        let mut journal2 = sample_journal("run-overflow-2", 1, RunRecoveryStatus::Active);
        journal2.adapter_dispatches.push(AdapterDispatchRecord {
            dispatch_id: "run-overflow-2:max".to_string(),
            ordinal: u64::MAX,
            role: "planner".to_string(),
            profile_id: "prof".to_string(),
            model: None,
            payload_digest: None,
            usage: None,
            status: Some("completed".to_string()),
            created_at_unix: 1,
            completed_at_unix: None,
        });
        let err2 = journal2.reserve_dispatch_ordinal();
        assert!(err2.is_err());
        assert!(err2.unwrap_err().contains("PC_DISPATCH_ORDINAL_OVERFLOW"));
    }

    #[test]
    fn test_aggregate_dispatches_usage_comprehensive() {
        let mut journal = sample_journal("run-agg", 1, RunRecoveryStatus::Active);

        // 1. Both dispatches known => checked sum calculated
        let d1 = AdapterDispatchRecord {
            dispatch_id: "run-agg:1".to_string(),
            ordinal: 1,
            role: "planner".to_string(),
            profile_id: "prof".to_string(),
            model: None,
            payload_digest: None,
            usage: Some(NormalizedUsage {
                input_tokens: Some(100),
                output_tokens: Some(50),
                raw_total_tokens: Some(150),
                cache_read_tokens: Some(20),
                cache_write_tokens: Some(0),
            }),
            status: Some("completed".to_string()),
            created_at_unix: 1,
            completed_at_unix: Some(2),
        };
        let d2 = AdapterDispatchRecord {
            dispatch_id: "run-agg:2".to_string(),
            ordinal: 2,
            role: "plan_reviewer".to_string(),
            profile_id: "prof".to_string(),
            model: None,
            payload_digest: None,
            usage: Some(NormalizedUsage {
                input_tokens: Some(200),
                output_tokens: Some(80),
                raw_total_tokens: Some(280),
                cache_read_tokens: Some(40),
                cache_write_tokens: Some(10),
            }),
            status: Some("completed".to_string()),
            created_at_unix: 3,
            completed_at_unix: Some(4),
        };
        journal.record_adapter_dispatch(d1).unwrap();
        journal.record_adapter_dispatch(d2).unwrap();

        let agg = journal.usage.clone().expect("cumulative usage must exist");
        assert_eq!(agg.input_tokens, Some(300));
        assert_eq!(agg.output_tokens, Some(130));
        assert_eq!(agg.raw_total_tokens, Some(430));
        assert_eq!(agg.cache_read_tokens, Some(60));
        assert_eq!(agg.cache_write_tokens, Some(10)); // 0 + 10 = 10, Some(0) preserved

        // 2. Add dispatch with None field => aggregate field becomes None
        let d3 = AdapterDispatchRecord {
            dispatch_id: "run-agg:3".to_string(),
            ordinal: 3,
            role: "planner".to_string(),
            profile_id: "prof".to_string(),
            model: None,
            payload_digest: None,
            usage: Some(NormalizedUsage {
                input_tokens: Some(50),
                output_tokens: Some(20),
                raw_total_tokens: Some(70),
                cache_read_tokens: None, // unmeasured
                cache_write_tokens: Some(5),
            }),
            status: Some("completed".to_string()),
            created_at_unix: 5,
            completed_at_unix: Some(6),
        };
        journal.record_adapter_dispatch(d3).unwrap();
        let agg2 = journal.usage.clone().unwrap();
        assert_eq!(agg2.input_tokens, Some(350));
        assert_eq!(agg2.cache_read_tokens, None); // Because d3 had None
        assert_eq!(agg2.cache_write_tokens, Some(15)); // 0 + 10 + 5 = 15

        // Raw records in adapter_dispatches are preserved exactly
        assert_eq!(journal.adapter_dispatches.len(), 3);
        assert_eq!(journal.adapter_dispatches[0].usage.as_ref().unwrap().cache_read_tokens, Some(20));
        assert_eq!(journal.adapter_dispatches[1].usage.as_ref().unwrap().cache_read_tokens, Some(40));
        assert_eq!(journal.adapter_dispatches[2].usage.as_ref().unwrap().cache_read_tokens, None);

        // 3. Overflow yields None rather than saturating
        let d_overflow = AdapterDispatchRecord {
            dispatch_id: "run-agg:4".to_string(),
            ordinal: 4,
            role: "planner".to_string(),
            profile_id: "prof".to_string(),
            model: None,
            payload_digest: None,
            usage: Some(NormalizedUsage {
                input_tokens: Some(u64::MAX),
                output_tokens: Some(1),
                raw_total_tokens: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
            }),
            status: Some("completed".to_string()),
            created_at_unix: 7,
            completed_at_unix: Some(8),
        };
        journal.record_adapter_dispatch(d_overflow).unwrap();
        let agg3 = journal.usage.clone().unwrap();
        assert_eq!(agg3.input_tokens, None); // Overflowed 350 + u64::MAX -> None (not saturating!)
    }

    #[test]
    fn test_record_adapter_dispatch_validation_and_unmodified_on_failure() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JournalManager::new(temp.path().join("runs"));
        let journal = sample_journal("run-val-test", 1, RunRecoveryStatus::Active);
        manager.write_journal(&journal).unwrap();

        // 1. Accept valid records with various allowed statuses (None, reserved, completed, failed)
        let valid_rec1 = AdapterDispatchRecord {
            dispatch_id: "run-val-test:1".to_string(),
            ordinal: 1,
            role: "planner".to_string(),
            profile_id: "profile-1".to_string(),
            model: Some("model-a".to_string()),
            payload_digest: Some("hash1".to_string()),
            usage: None,
            status: None, // Defaults to "reserved"
            created_at_unix: 100,
            completed_at_unix: None,
        };
        let changed1 = manager.record_adapter_dispatch("run-val-test", valid_rec1).unwrap();
        assert!(changed1);

        // Update record 1 to "completed"
        let valid_rec1_comp = AdapterDispatchRecord {
            dispatch_id: "run-val-test:1".to_string(),
            ordinal: 1,
            role: "planner".to_string(),
            profile_id: "profile-1".to_string(),
            model: Some("model-a".to_string()),
            payload_digest: Some("hash1".to_string()),
            usage: None,
            status: Some("completed".to_string()),
            created_at_unix: 100,
            completed_at_unix: Some(105),
        };
        let changed2 = manager.record_adapter_dispatch("run-val-test", valid_rec1_comp).unwrap();
        assert!(changed2);

        // Capture durable state before invalid attempts
        let baseline = manager.read_journal("run-val-test").unwrap();
        let baseline_path = manager.runs_dir().join("run-val-test.json");
        let baseline_raw = std::fs::read(&baseline_path).unwrap();

        // 2. Reject ordinal 0
        let rec_ord_zero = AdapterDispatchRecord {
            dispatch_id: "run-val-test:0".to_string(),
            ordinal: 0,
            role: "planner".to_string(),
            profile_id: "profile-1".to_string(),
            model: None,
            payload_digest: None,
            usage: None,
            status: Some("reserved".to_string()),
            created_at_unix: 110,
            completed_at_unix: None,
        };
        let err_zero = manager.record_adapter_dispatch("run-val-test", rec_ord_zero);
        assert!(err_zero.is_err());
        assert!(err_zero.unwrap_err().contains("PC_DISPATCH_INVALID_ORDINAL"));

        // 3. Reject ID/ordinal mismatch
        let rec_mismatch = AdapterDispatchRecord {
            dispatch_id: "run-val-test:99".to_string(),
            ordinal: 2,
            role: "planner".to_string(),
            profile_id: "profile-1".to_string(),
            model: None,
            payload_digest: None,
            usage: None,
            status: Some("reserved".to_string()),
            created_at_unix: 110,
            completed_at_unix: None,
        };
        let err_mismatch = manager.record_adapter_dispatch("run-val-test", rec_mismatch);
        assert!(err_mismatch.is_err());
        assert!(err_mismatch.unwrap_err().contains("PC_DISPATCH_ID_MISMATCH"));

        // Reject ID with different run_id
        let rec_diff_run = AdapterDispatchRecord {
            dispatch_id: "other-run:2".to_string(),
            ordinal: 2,
            role: "planner".to_string(),
            profile_id: "profile-1".to_string(),
            model: None,
            payload_digest: None,
            usage: None,
            status: Some("reserved".to_string()),
            created_at_unix: 110,
            completed_at_unix: None,
        };
        let err_diff_run = manager.record_adapter_dispatch("run-val-test", rec_diff_run);
        assert!(err_diff_run.is_err());
        assert!(err_diff_run.unwrap_err().contains("PC_DISPATCH_ID_MISMATCH"));

        // 4. Reject unknown status and empty status
        let rec_unknown_status = AdapterDispatchRecord {
            dispatch_id: "run-val-test:2".to_string(),
            ordinal: 2,
            role: "planner".to_string(),
            profile_id: "profile-1".to_string(),
            model: None,
            payload_digest: None,
            usage: None,
            status: Some("in_progress".to_string()),
            created_at_unix: 110,
            completed_at_unix: None,
        };
        let err_status = manager.record_adapter_dispatch("run-val-test", rec_unknown_status);
        assert!(err_status.is_err());
        assert!(err_status.unwrap_err().contains("PC_DISPATCH_INVALID_STATUS"));

        let rec_empty_status = AdapterDispatchRecord {
            dispatch_id: "run-val-test:2".to_string(),
            ordinal: 2,
            role: "planner".to_string(),
            profile_id: "profile-1".to_string(),
            model: None,
            payload_digest: None,
            usage: None,
            status: Some("".to_string()),
            created_at_unix: 110,
            completed_at_unix: None,
        };
        let err_empty = manager.record_adapter_dispatch("run-val-test", rec_empty_status);
        assert!(err_empty.is_err());
        assert!(err_empty.unwrap_err().contains("PC_DISPATCH_INVALID_STATUS"));

        // 5. Reject duplicate ordinal collision on new dispatch
        let rec_dup_ordinal = AdapterDispatchRecord {
            dispatch_id: "run-val-test:2".to_string(),
            ordinal: 1, // Already exists with dispatch_id "run-val-test:1"
            role: "reviewer".to_string(),
            profile_id: "profile-2".to_string(),
            model: None,
            payload_digest: None,
            usage: None,
            status: Some("reserved".to_string()),
            created_at_unix: 110,
            completed_at_unix: None,
        };
        let err_dup_id_mismatch = manager.record_adapter_dispatch("run-val-test", rec_dup_ordinal);
        assert!(err_dup_id_mismatch.is_err());
        assert!(err_dup_id_mismatch.unwrap_err().contains("PC_DISPATCH_ID_MISMATCH"));

        // Conflict on existing dispatch
        let rec_conflict = AdapterDispatchRecord {
            dispatch_id: "run-val-test:1".to_string(),
            ordinal: 1,
            role: "conflicting-role".to_string(),
            profile_id: "profile-1".to_string(),
            model: Some("model-a".to_string()),
            payload_digest: Some("hash1".to_string()),
            usage: None,
            status: Some("completed".to_string()),
            created_at_unix: 100,
            completed_at_unix: Some(105),
        };
        let err_conflict = manager.record_adapter_dispatch("run-val-test", rec_conflict);
        assert!(err_conflict.is_err());
        assert!(err_conflict.unwrap_err().contains("PC_DISPATCH_CONFLICT"));

        // 6. Verify journal in-memory and on-disk remain completely unchanged after all rejections
        let current_journal = manager.read_journal("run-val-test").unwrap();
        assert_eq!(current_journal.revision, baseline.revision);
        assert_eq!(current_journal.updated_at_unix, baseline.updated_at_unix);
        assert_eq!(current_journal.next_dispatch_ordinal, baseline.next_dispatch_ordinal);
        assert_eq!(current_journal.adapter_dispatches.len(), baseline.adapter_dispatches.len());
        assert_eq!(current_journal.adapter_dispatches[0].status, baseline.adapter_dispatches[0].status);

        let current_raw = std::fs::read(&baseline_path).unwrap();
        assert_eq!(current_raw, baseline_raw, "Disk file must remain byte-for-byte identical after rejected records");

        // In-memory RunJournal direct call verification
        let mut mem_journal = baseline.clone();
        let rec_invalid_mem = AdapterDispatchRecord {
            dispatch_id: "run-val-test:0".to_string(),
            ordinal: 0,
            role: "planner".to_string(),
            profile_id: "p".to_string(),
            model: None,
            payload_digest: None,
            usage: None,
            status: Some("reserved".to_string()),
            created_at_unix: 1,
            completed_at_unix: None,
        };
        assert!(mem_journal.record_adapter_dispatch(rec_invalid_mem).is_err());
        assert_eq!(mem_journal.adapter_dispatches.len(), baseline.adapter_dispatches.len());
        assert_eq!(mem_journal.next_dispatch_ordinal, baseline.next_dispatch_ordinal);
    }

    #[test]
    fn test_validate_journal_rejects_inconsistent_dispatches() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JournalManager::new(temp.path().join("runs"));

        // Baseline valid journal
        let make_base = || {
            let mut j = sample_journal("run-validate-test", 1, RunRecoveryStatus::Active);
            j.next_dispatch_ordinal = 3;
            j.adapter_dispatches = vec![
                AdapterDispatchRecord {
                    dispatch_id: "run-validate-test:1".to_string(),
                    ordinal: 1,
                    role: "planner".to_string(),
                    profile_id: "p1".to_string(),
                    model: None,
                    payload_digest: None,
                    usage: None,
                    status: Some("completed".to_string()),
                    created_at_unix: 10,
                    completed_at_unix: Some(12),
                },
                AdapterDispatchRecord {
                    dispatch_id: "run-validate-test:2".to_string(),
                    ordinal: 2,
                    role: "plan_reviewer".to_string(),
                    profile_id: "p2".to_string(),
                    model: None,
                    payload_digest: None,
                    usage: None,
                    status: Some("reserved".to_string()),
                    created_at_unix: 15,
                    completed_at_unix: None,
                },
            ];
            j
        };

        // 1. Baseline is valid
        let valid_j = make_base();
        assert!(validate_journal(&valid_j).is_ok());

        // 2. Out-of-order enumeration is accepted (list order does not strictly require ascending)
        let mut out_of_order = make_base();
        out_of_order.adapter_dispatches.reverse(); // [ordinal 2, ordinal 1]
        assert!(validate_journal(&out_of_order).is_ok());

        // 3. Ordinal 0 rejected
        let mut zero_ord = make_base();
        zero_ord.adapter_dispatches[0].ordinal = 0;
        zero_ord.adapter_dispatches[0].dispatch_id = "run-validate-test:0".to_string();
        let err = validate_journal(&zero_ord);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("PC_DISPATCH_INVALID_ORDINAL"));

        // 4. ID / ordinal mismatch rejected
        let mut mismatch_id = make_base();
        mismatch_id.adapter_dispatches[0].dispatch_id = "run-validate-test:99".to_string();
        let err = validate_journal(&mismatch_id);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("PC_DISPATCH_ID_MISMATCH"));

        // 5. ID / run_id mismatch rejected
        let mut mismatch_run = make_base();
        mismatch_run.adapter_dispatches[0].dispatch_id = "other-run:1".to_string();
        let err = validate_journal(&mismatch_run);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("PC_DISPATCH_ID_MISMATCH"));

        // 6. Empty or whitespace-only status rejected
        let mut empty_status = make_base();
        empty_status.adapter_dispatches[0].status = Some("".to_string());
        let err_empty = validate_journal(&empty_status);
        assert!(err_empty.is_err());
        assert!(err_empty.unwrap_err().contains("PC_DISPATCH_INVALID_STATUS"));

        let mut ws_status = make_base();
        ws_status.adapter_dispatches[0].status = Some("   \t ".to_string());
        let err_ws = validate_journal(&ws_status);
        assert!(err_ws.is_err());
        assert!(err_ws.unwrap_err().contains("PC_DISPATCH_INVALID_STATUS"));

        // 7. Non-empty unknown status is accepted on validation/read as unresolved state
        let mut unknown_status_j = make_base();
        unknown_status_j.adapter_dispatches[0].status = Some("custom_unresolved".to_string());
        unknown_status_j.adapter_dispatches[0].usage = Some(crate::orchestrator::adapters::NormalizedUsage {
            input_tokens: Some(100),
            output_tokens: Some(50),
            ..Default::default()
        });
        assert!(validate_journal(&unknown_status_j).is_ok());

        // Unknown status dispatch usage is NOT aggregated as a confirmed value (yields None)
        assert!(unknown_status_j.aggregate_dispatches_usage().is_none());

        // State transition from unknown status is rejected
        let rec_trans = AdapterDispatchRecord {
            dispatch_id: "run-validate-test:1".to_string(),
            ordinal: 1,
            role: "planner".to_string(),
            profile_id: "p1".to_string(),
            model: None,
            payload_digest: None,
            usage: None,
            status: Some("completed".to_string()),
            created_at_unix: 10,
            completed_at_unix: Some(15),
        };
        let err_trans = unknown_status_j.record_adapter_dispatch(rec_trans);
        assert!(err_trans.is_err());
        assert!(err_trans.unwrap_err().contains("PC_DISPATCH_INVALID_STATE"));

        // 8. Duplicate dispatch_id / duplicate ordinal rejected
        let mut dup_j = make_base();
        dup_j.adapter_dispatches[1].ordinal = 1;
        dup_j.adapter_dispatches[1].dispatch_id = "run-validate-test:1".to_string();
        let err = validate_journal(&dup_j);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("PC_DISPATCH_DUPLICATE"));

        // 8. next_dispatch_ordinal <= max_ordinal rejected
        let mut bad_next = make_base();
        bad_next.next_dispatch_ordinal = 2; // max ordinal is 2 -> must be > 2
        let err = validate_journal(&bad_next);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("PC_DISPATCH_INVALID_NEXT_ORDINAL"));

        bad_next.next_dispatch_ordinal = 1;
        let err2 = validate_journal(&bad_next);
        assert!(err2.is_err());
        assert!(err2.unwrap_err().contains("PC_DISPATCH_INVALID_NEXT_ORDINAL"));

        // 9. Reading corrupted file from disk via manager is rejected
        std::fs::create_dir_all(manager.runs_dir()).unwrap();
        let corrupt_path = manager.runs_dir().join("run-corrupt.json");
        let mut corrupt_j = make_base();
        corrupt_j.run_id = "run-corrupt".to_string();
        corrupt_j.adapter_dispatches[0].dispatch_id = "run-corrupt:1".to_string();
        corrupt_j.adapter_dispatches[1].dispatch_id = "run-corrupt:2".to_string();
        corrupt_j.next_dispatch_ordinal = 1; // Corrupted next ordinal
        let corrupt_str = serde_json::to_string_pretty(&corrupt_j).unwrap();
        std::fs::write(&corrupt_path, corrupt_str).unwrap();

        let read_err = manager.read_journal("run-corrupt");
        assert!(read_err.is_err());
        assert!(read_err.unwrap_err().contains("PC_DISPATCH_INVALID_NEXT_ORDINAL"));
    }

    #[test]
    fn test_legacy_schema_v1_without_dispatches_reads_cleanly_via_manager() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JournalManager::new(temp.path().join("runs"));
        std::fs::create_dir_all(manager.runs_dir()).unwrap();

        let legacy_json = serde_json::json!({
            "schemaVersion": 1,
            "runId": "run-legacy-mgr",
            "workflowType": "test",
            "canonicalProjectPath": "C:\\test",
            "taskPrompt": "test task",
            "snapshot": {
                "projectPath": "C:\\test",
                "assignments": {},
                "iterationLimits": {
                    "maxPlanReviewIterations": 3,
                    "maxFixIterations": 3,
                    "maxCodeReviewIterations": 3,
                    "maxAntigravityDispatches": 10
                },
                "validationGates": [],
                "budgetLimits": {},
                "createdAtUnix": 100,
                "leanAntigravityMode": false,
                "planWorkspace": {
                    "canonicalRoot": "C:\\test",
                    "planDir": "C:\\test\\.plan",
                    "archiveDir": "C:\\test\\.plan\\archive",
                    "primaryPlanId": "V0.23.0-r1.md",
                    "supplementalPlanIds": []
                },
                "mcpServers": {}
            },
            "currentState": "building_context",
            "iterationCounters": {
                "planReviewCount": 0,
                "fixCount": 0,
                "codeReviewCount": 0
            },
            "revision": 1,
            "resumeGeneration": 0,
            "status": "active",
            "createdAtUnix": 100,
            "updatedAtUnix": 100
        });

        let file_path = manager.runs_dir().join("run-legacy-mgr.json");
        std::fs::write(&file_path, serde_json::to_string(&legacy_json).unwrap()).unwrap();

        let loaded = manager.read_journal("run-legacy-mgr").unwrap();
        assert_eq!(loaded.schema_version, 1);
        assert_eq!(loaded.run_id, "run-legacy-mgr");
        assert_eq!(loaded.next_dispatch_ordinal, 0);
        assert!(loaded.adapter_dispatches.is_empty());
        assert_eq!(loaded.usage, None);
    }
}
