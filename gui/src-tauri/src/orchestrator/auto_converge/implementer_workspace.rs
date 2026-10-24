//! Offline prototype for an isolated Implementer workspace and guarded delta promotion.
//!
//! This module is deliberately not wired into the production FSM. Its promotion permit
//! prevents accidental API misuse; it is not a substitute for a production approval binding.

use super::git_runner::is_excluded_from_baseline;
use super::types::PromotionPhase;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateFileState {
    pub path: String,
    pub sha256: Option<String>,
    pub readonly: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateDeltaEntry {
    pub path: String,
    pub before_sha256: Option<String>,
    pub after_sha256: Option<String>,
    pub after_readonly: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ImplementerWorkspace {
    primary_root: PathBuf,
    candidate_root: PathBuf,
    allowlist: Vec<String>,
    baseline: Vec<CandidateFileState>,
    baseline_digest: String,
}

/// An opaque authorization token for the exact candidate delta observed by this prototype.
/// It is intentionally not serializable or constructible outside this module.
#[derive(Debug)]
pub struct PromotionPermit {
    baseline_digest: String,
    delta_digest: String,
    entries: Vec<CandidateDeltaEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromotionOutcome {
    Promoted { changed_files: usize },
    RejectedBaselineChanged,
    RejectedCandidateChanged,
    RecoveryRequired { journal: PathBuf, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub path: String,
    pub existed_before: bool,
    pub before_sha256: Option<String>,
    pub before_readonly: Option<bool>,
    pub after_sha256: Option<String>,
    pub after_readonly: Option<bool>,
    pub created_parent_dirs: Vec<String>,
    pub backup_name: String,
    pub staged_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionJournal {
    pub version: u32,
    pub entries: Vec<JournalEntry>,
    pub mutation_intents: Vec<String>,
}

impl ImplementerWorkspace {
    /// Create a fresh candidate from only the explicit allowlist. Missing allowlisted files
    /// are represented in the baseline and may be created by the candidate.
    pub fn create(
        primary_root: &Path,
        engine_state_root: &Path,
        candidate_root: &Path,
        allowlist: &[String],
    ) -> Result<Self, String> {
        let primary = canonical_existing_dir(primary_root, "primary root")?;
        let engine = canonical_existing_dir(engine_state_root, "engine-state root")?;
        let candidate_parent = candidate_root
            .parent()
            .ok_or_else(|| "candidate root must have a parent".to_string())?;
        let parent = canonical_existing_dir(candidate_parent, "candidate parent")?;
        let candidate = parent.join(
            candidate_root
                .file_name()
                .ok_or_else(|| "candidate root must have a final component".to_string())?,
        );
        let candidate_presence = inspect_path_presence(&candidate);
        if !matches!(candidate_presence, PathPresence::Absent) {
            return Err("candidate root must not already exist".into());
        }
        if paths_overlap(&candidate, &primary) || paths_overlap(&candidate, &engine) {
            return Err(
                "candidate root must be disjoint from primary and engine-state roots".into(),
            );
        }

        let normalized_allowlist = normalize_allowlist(allowlist)?;
        fs::create_dir(&candidate).map_err(|e| format!("create candidate root: {e}"))?;
        let mut baseline = Vec::with_capacity(normalized_allowlist.len());
        for rel in &normalized_allowlist {
            let source = checked_path(&primary, rel, false)?;
            let state = read_file_state(&source, rel)?;
            if let Some(bytes) = state.1.as_ref() {
                let destination = checked_path(&candidate, rel, true)?;
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("create candidate parent for {rel}: {e}"))?;
                }
                write_new_file(&destination, bytes)?;
                if state.2.unwrap_or(false) {
                    let mut permissions = fs::metadata(&destination)
                        .map_err(|e| format!("candidate metadata for {rel}: {e}"))?
                        .permissions();
                    permissions.set_readonly(true);
                    fs::set_permissions(&destination, permissions)
                        .map_err(|e| format!("preserve readonly mode for {rel}: {e}"))?;
                }
            }
            baseline.push(state.0);
        }
        let baseline_digest = digest_json(&baseline)?;
        Ok(Self {
            primary_root: primary,
            candidate_root: candidate,
            allowlist: normalized_allowlist,
            baseline,
            baseline_digest,
        })
    }

    /// Re-open an existing candidate workspace for corrective cycles, ensuring
    /// the tree has no aliases, candidate files match the allowlist, and the
    /// primary baseline matches the recorded baseline digest.
    pub fn load_existing(
        primary_root: &Path,
        engine_state_root: &Path,
        candidate_root: &Path,
        allowlist: &[String],
        expected_baseline_digest: &str,
    ) -> Result<Self, String> {
        let primary = canonical_existing_dir(primary_root, "primary root")?;
        let engine = canonical_existing_dir(engine_state_root, "engine-state root")?;
        let candidate = canonical_existing_dir(candidate_root, "candidate root")?;
        if paths_overlap(&candidate, &primary) || paths_overlap(&candidate, &engine) {
            return Err(
                "candidate root must be disjoint from primary and engine-state roots".into(),
            );
        }
        let normalized_allowlist = normalize_allowlist(allowlist)?;
        validate_tree_has_no_aliases(&candidate)?;
        validate_candidate_file_set(&candidate, &normalized_allowlist)?;

        let mut baseline = Vec::with_capacity(normalized_allowlist.len());
        for rel in &normalized_allowlist {
            let source = checked_path(&primary, rel, false)?;
            let state = read_file_state(&source, rel)?;
            baseline.push(state.0);
        }
        let baseline_digest = digest_json(&baseline)?;
        if baseline_digest != expected_baseline_digest {
            return Err("existing candidate baseline does not match recorded baseline digest".into());
        }
        Ok(Self {
            primary_root: primary,
            candidate_root: candidate,
            allowlist: normalized_allowlist,
            baseline,
            baseline_digest,
        })
    }

    pub fn candidate_root(&self) -> &Path {
        &self.candidate_root
    }

    pub fn baseline_digest(&self) -> &str {
        &self.baseline_digest
    }

    /// Compute changes from the frozen baseline. Any unexpected file beneath the candidate
    /// root, including an alias/reparse point, is rejected rather than silently omitted.
    pub fn delta(&self) -> Result<Vec<CandidateDeltaEntry>, String> {
        validate_tree_has_no_aliases(&self.candidate_root)?;
        validate_candidate_file_set(&self.candidate_root, &self.allowlist)?;
        let baseline: BTreeMap<&str, &CandidateFileState> = self
            .baseline
            .iter()
            .map(|item| (item.path.as_str(), item))
            .collect();
        let mut delta = Vec::new();
        for rel in &self.allowlist {
            let path = checked_path(&self.candidate_root, rel, false)?;
            let (current, _, readonly) = read_file_state(&path, rel)?;
            let before = baseline
                .get(rel.as_str())
                .copied()
                .expect("allowlist baseline");
            if before.sha256 != current.sha256 || before.readonly != readonly {
                delta.push(CandidateDeltaEntry {
                    path: rel.clone(),
                    before_sha256: before.sha256.clone(),
                    after_sha256: current.sha256,
                    after_readonly: readonly,
                });
            }
        }
        Ok(delta)
    }

    /// Digest of the currently observed canonical delta.
    pub fn delta_digest(&self) -> Result<String, String> {
        digest_json(&self.delta()?)
    }

    /// Bind a permit only when the supplied approval digests match this exact candidate and
    /// baseline. The caller-supplied strings are not a production approval authority; the
    /// prototype is intentionally not connected to the controller/FSM.
    pub fn authorize_delta(
        &self,
        approved_baseline_digest: &str,
        approved_delta_digest: &str,
    ) -> Result<PromotionPermit, String> {
        let entries = self.delta()?;
        let delta_digest = digest_json(&entries)?;
        if approved_baseline_digest != self.baseline_digest || approved_delta_digest != delta_digest
        {
            return Err(
                "approval digest does not match the current baseline and candidate delta".into(),
            );
        }
        Ok(PromotionPermit {
            baseline_digest: self.baseline_digest.clone(),
            delta_digest,
            entries,
        })
    }

    /// Promote only on a disposable fixture. Production FSM wiring is intentionally absent.
    pub fn promote(&self, permit: &PromotionPermit) -> Result<PromotionOutcome, String> {
        self.promote_with_failure(permit, None, false, None)
    }

    /// Promote an authorized candidate delta to the primary workspace, binding each
    /// promotion phase to the Controller/FSM state before mutations begin.
    pub fn promote_with_controller<F>(
        &self,
        permit: &PromotionPermit,
        run_id: &str,
        mut on_phase: F,
    ) -> Result<PromotionOutcome, String>
    where
        F: FnMut(PromotionPhase, &Path, &str) -> Result<(), String>,
    {
        self.promote_internal(permit, run_id, &mut on_phase, None, false, None)
    }

    pub fn promote_with_failure(
        &self,
        permit: &PromotionPermit,
        fail_after_apply: Option<usize>,
        fail_rollback: bool,
        fail_journal_update: Option<usize>,
    ) -> Result<PromotionOutcome, String> {
        self.promote_internal(
            permit,
            "default",
            &mut |_, _, _| Ok(()),
            fail_after_apply,
            fail_rollback,
            fail_journal_update,
        )
    }

    pub(crate) fn promote_internal(
        &self,
        permit: &PromotionPermit,
        run_id: &str,
        on_phase: &mut dyn FnMut(PromotionPhase, &Path, &str) -> Result<(), String>,
        fail_after_apply: Option<usize>,
        fail_rollback: bool,
        fail_journal_update: Option<usize>,
    ) -> Result<PromotionOutcome, String> {
        if permit.baseline_digest != self.baseline_digest {
            return Ok(PromotionOutcome::RejectedCandidateChanged);
        }
        if digest_json(&permit.entries)? != permit.delta_digest || self.delta()? != permit.entries {
            return Ok(PromotionOutcome::RejectedCandidateChanged);
        }
        if capture_states(&self.primary_root, &self.allowlist)? != self.baseline {
            return Ok(PromotionOutcome::RejectedBaselineChanged);
        }
        if permit.entries.is_empty() {
            return Ok(PromotionOutcome::Promoted { changed_files: 0 });
        }

        let parent = self
            .primary_root
            .parent()
            .ok_or("primary root has no parent")?;
        let journal_dir = parent.join(format!(".pct-promotion-{}-{}", run_id, Uuid::new_v4()));
        if journal_dir.exists() {
            return Err("promotion journal directory collision; refusing to proceed".into());
        }
        fs::create_dir(&journal_dir).map_err(|e| format!("create promotion journal: {e}"))?;
        let mut journal = PromotionJournal {
            version: 2,
            entries: Vec::new(),
            mutation_intents: Vec::new(),
        };

        for (index, delta) in permit.entries.iter().enumerate() {
            let primary = checked_path(&self.primary_root, &delta.path, true)?;
            let candidate = checked_path(&self.candidate_root, &delta.path, false)?;
            let before = self
                .baseline
                .iter()
                .find(|state| state.path == delta.path)
                .ok_or("promotion path is not in baseline")?;
            let backup_name = format!("backup-{index}");
            let staged_name = format!("staged-{index}");
            if primary.exists() {
                let bytes =
                    fs::read(&primary).map_err(|e| format!("read promotion backup: {e}"))?;
                write_new_file(&journal_dir.join(&backup_name), &bytes)?;
            }
            if let Some(after_hash) = delta.after_sha256.as_ref() {
                let bytes =
                    fs::read(&candidate).map_err(|e| format!("read candidate output: {e}"))?;
                if sha256_hex(&bytes) != *after_hash {
                    let _ = fs::remove_dir_all(&journal_dir);
                    return Ok(PromotionOutcome::RejectedCandidateChanged);
                }
                write_new_file(&journal_dir.join(&staged_name), &bytes)?;
            }
            journal.entries.push(JournalEntry {
                path: delta.path.clone(),
                existed_before: primary.exists(),
                before_sha256: before.sha256.clone(),
                before_readonly: primary
                    .exists()
                    .then(|| fs::metadata(&primary).map(|m| m.permissions().readonly()))
                    .transpose()
                    .map_err(|e| format!("read original permissions: {e}"))?,
                after_sha256: delta.after_sha256.clone(),
                after_readonly: delta.after_readonly,
                created_parent_dirs: missing_parent_directories(&self.primary_root, &delta.path)?,
                backup_name,
                staged_name,
            });
        }
        let (_initial_path, initial_digest) = persist_journal(&journal_dir, &journal)?;
        let mut latest_journal_digest = initial_digest;
        on_phase(PromotionPhase::Staged, &journal_dir, &latest_journal_digest)?;
        on_phase(PromotionPhase::Applying, &journal_dir, &latest_journal_digest)?;

        for (index, entry) in journal.entries.iter().enumerate() {
            if fail_after_apply == Some(index) {
                let reason = "injected promotion failure".to_string();
                if rollback(&self.primary_root, &journal_dir, &journal, fail_rollback).is_err() {
                    let _ = on_phase(PromotionPhase::RecoveryRequired, &journal_dir, &latest_journal_digest);
                    return Ok(PromotionOutcome::RecoveryRequired {
                        journal: journal_dir,
                        reason,
                    });
                }
                fs::remove_dir_all(&journal_dir)
                    .map_err(|e| format!("remove rolled-back journal: {e}"))?;
                return Err(reason);
            }
            // Persist intent before touching the primary fixture. A crash during the next
            // operation therefore leaves enough evidence for recovery to restore this path.
            journal.mutation_intents.push(entry.path.clone());
            let (journal_result, new_digest) = if fail_journal_update == Some(index) {
                (Err("injected journal update failure".to_string()), None)
            } else {
                match persist_journal(&journal_dir, &journal) {
                    Ok((_path, digest)) => (Ok(()), Some(digest)),
                    Err(e) => (Err(e), None),
                }
            };
            if let Err(error) = journal_result {
                if rollback(&self.primary_root, &journal_dir, &journal, fail_rollback).is_err() {
                    let _ = on_phase(PromotionPhase::RecoveryRequired, &journal_dir, &latest_journal_digest);
                    return Ok(PromotionOutcome::RecoveryRequired {
                        journal: journal_dir,
                        reason: error,
                    });
                }
                fs::remove_dir_all(&journal_dir)
                    .map_err(|e| format!("remove journal after pre-apply failure: {e}"))?;
                return Err(error);
            }
            if let Some(digest) = new_digest {
                latest_journal_digest = digest;
                on_phase(PromotionPhase::Applying, &journal_dir, &latest_journal_digest)?;
            }

            let apply_result = (|| -> Result<(), String> {
                let target = checked_path(&self.primary_root, &entry.path, true)?;
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("create promotion parent: {e}"))?;
                }
                if target.exists() {
                    remove_target_file(&target)?;
                }
                let delta = &permit.entries[index];
                if delta.after_sha256.is_some() {
                    fs::rename(journal_dir.join(&entry.staged_name), &target)
                        .map_err(|e| format!("install staged file: {e}"))?;
                    if delta.after_readonly.unwrap_or(false) {
                        let mut permissions = fs::metadata(&target)
                            .map_err(|e| e.to_string())?
                            .permissions();
                        permissions.set_readonly(true);
                        fs::set_permissions(&target, permissions).map_err(|e| e.to_string())?;
                    }
                }
                Ok(())
            })();
            if let Err(error) = apply_result {
                if rollback(&self.primary_root, &journal_dir, &journal, false).is_err() {
                    let _ = on_phase(PromotionPhase::RecoveryRequired, &journal_dir, &latest_journal_digest);
                    return Ok(PromotionOutcome::RecoveryRequired {
                        journal: journal_dir,
                        reason: error,
                    });
                }
                fs::remove_dir_all(&journal_dir)
                    .map_err(|e| format!("remove rolled-back journal: {e}"))?;
                return Err(error);
            }
        }
        if fail_after_apply == Some(journal.entries.len()) {
            let reason = "injected promotion failure after final apply".to_string();
            if rollback(&self.primary_root, &journal_dir, &journal, fail_rollback).is_err() {
                let _ = on_phase(PromotionPhase::RecoveryRequired, &journal_dir, &latest_journal_digest);
                return Ok(PromotionOutcome::RecoveryRequired {
                    journal: journal_dir,
                    reason,
                });
            }
            fs::remove_dir_all(&journal_dir)
                .map_err(|e| format!("remove journal after final-apply rollback: {e}"))?;
            return Err(reason);
        }

        on_phase(PromotionPhase::Applied, &journal_dir, &latest_journal_digest)?;
        let name = journal_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let expected_prefix = format!(".pct-promotion-{}-", run_id);
        if journal_dir.parent() == Some(parent) && name.starts_with(&expected_prefix) {
            fs::remove_dir_all(&journal_dir).map_err(|e| format!("remove completed journal: {e}"))?;
        } else {
            return Err("refusing to remove journal: unexpected path or ownership mismatch".into());
        }
        Ok(PromotionOutcome::Promoted {
            changed_files: permit.entries.len(),
        })
    }
}

fn normalize_allowlist(paths: &[String]) -> Result<Vec<String>, String> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for raw in paths {
        if raw.contains('\\') || raw.contains(':') || raw.contains('\0') {
            return Err(format!("unsafe allowlist path: {raw}"));
        }
        let path = Path::new(raw);
        if path.is_absolute()
            || path
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(format!(
                "allowlist path must be normalized and relative: {raw}"
            ));
        }
        let canonical = path
            .to_str()
            .ok_or("allowlist path is not UTF-8")?
            .to_string();
        if canonical.is_empty() || !seen.insert(canonical.to_lowercase()) {
            return Err(format!("empty or case-colliding allowlist path: {raw}"));
        }
        if is_excluded_from_baseline(&canonical) {
            return Err(format!(
                "allowlist path is under reserved/excluded baseline roots (.git/, .plan/, .poverello/runs/): {raw}"
            ));
        }
        normalized.push(canonical);
    }
    normalized.sort();
    Ok(normalized)
}

#[derive(Debug, Clone)]
pub enum PathPresence {
    Absent,
    Present(fs::Metadata),
    Error(String),
}

pub fn classify_metadata_result(res: std::io::Result<fs::Metadata>) -> PathPresence {
    match res {
        Ok(meta) => PathPresence::Present(meta),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => PathPresence::Absent,
        Err(err) => PathPresence::Error(err.to_string()),
    }
}

pub fn is_reparse(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        return meta.file_attributes() & 0x400 != 0;
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        false
    }
}

pub fn is_path_alias_or_reparse(meta: &fs::Metadata) -> bool {
    meta.file_type().is_symlink() || is_reparse(meta)
}

#[cfg(test)]
type MetadataInspectorFn = Box<dyn Fn(&Path) -> std::io::Result<fs::Metadata> + Send>;

#[cfg(test)]
thread_local! {
    static TEST_METADATA_INSPECTOR: std::cell::RefCell<Option<MetadataInspectorFn>> = std::cell::RefCell::new(None);
}

#[cfg(test)]
pub struct TestMetadataInspectorGuard {
    _private: (),
}

#[cfg(test)]
impl TestMetadataInspectorGuard {
    pub fn enter<F>(f: F) -> Self
    where
        F: Fn(&Path) -> std::io::Result<fs::Metadata> + Send + 'static,
    {
        TEST_METADATA_INSPECTOR.with(|cell| {
            *cell.borrow_mut() = Some(Box::new(f));
        });
        Self { _private: () }
    }
}

#[cfg(test)]
impl Drop for TestMetadataInspectorGuard {
    fn drop(&mut self) {
        TEST_METADATA_INSPECTOR.with(|cell| {
            *cell.borrow_mut() = None;
        });
    }
}

pub fn inspect_path_presence(path: &Path) -> PathPresence {
    #[cfg(test)]
    {
        let custom = TEST_METADATA_INSPECTOR.with(|cell| {
            if let Some(ref f) = *cell.borrow() {
                Some(f(path))
            } else {
                None
            }
        });
        if let Some(res) = custom {
            return classify_metadata_result(res);
        }
    }

    classify_metadata_result(fs::symlink_metadata(path))
}

fn canonical_existing_dir(path: &Path, label: &str) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute"));
    }
    let meta = fs::symlink_metadata(path).map_err(|e| format!("inspect {label}: {e}"))?;
    if is_path_alias_or_reparse(&meta) {
        return Err(format!("{label} is a symlink or reparse point"));
    }
    reject_alias_components(path)?;
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("canonicalize {label}: {e}"))?;
    if !canonical.is_dir() {
        return Err(format!("{label} is not a directory"));
    }
    Ok(canonical)
}

fn reject_alias_components(path: &Path) -> Result<(), String> {
    match inspect_path_presence(path) {
        PathPresence::Absent => return Err(format!("required path does not exist: {}", path.display())),
        PathPresence::Error(e) => return Err(format!("inspect path {}: {e}", path.display())),
        PathPresence::Present(meta) => {
            if is_path_alias_or_reparse(&meta) {
                return Err(format!("symlink or reparse component is forbidden: {}", path.display()));
            }
        }
    }
    let mut cursor = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                cursor.push(prefix.as_os_str());
                continue;
            }
            Component::RootDir => {
                cursor.push(component.as_os_str());
                continue;
            }
            Component::Normal(_) => cursor.push(component.as_os_str()),
            Component::CurDir | Component::ParentDir => {
                return Err("non-normal canonical path component".into())
            }
        }
        let meta =
            fs::symlink_metadata(&cursor).map_err(|e| format!("inspect path component: {e}"))?;
        if meta.file_type().is_symlink() || is_reparse(&meta) {
            return Err(format!(
                "symlink/reparse component is forbidden: {}",
                cursor.display()
            ));
        }
    }
    path.canonicalize()
        .map_err(|e| format!("resolve path: {e}"))?;
    Ok(())
}

fn checked_path(root: &Path, rel: &str, allow_missing: bool) -> Result<PathBuf, String> {
    let normalized = normalize_allowlist(&[rel.to_string()])?;
    let mut path = root.to_path_buf();
    let mut missing_prefix = false;
    for component in Path::new(&normalized[0]).components() {
        let Component::Normal(part) = component else {
            return Err("non-normal path component".into());
        };
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() || is_reparse(&meta) => {
                return Err(format!(
                    "symlink/reparse path is forbidden: {}",
                    path.display()
                ));
            }
            Ok(_meta) if missing_prefix => {
                return Err(format!(
                    "unexpected existing component after missing prefix: {}",
                    path.display()
                ))
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => missing_prefix = true,
            Err(e) => return Err(format!("inspect {}: {e}", path.display())),
        }
    }
    if missing_prefix && !allow_missing {
        return Ok(path);
    }
    Ok(path)
}

fn read_file_state(
    path: &Path,
    rel: &str,
) -> Result<(CandidateFileState, Option<Vec<u8>>, Option<bool>), String> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() || is_reparse(&meta) || !meta.is_file() {
                return Err(format!("allowlisted target is not a regular file: {rel}"));
            }
            let bytes = fs::read(path).map_err(|e| format!("read {rel}: {e}"))?;
            let readonly = meta.permissions().readonly();
            Ok((
                CandidateFileState {
                    path: rel.into(),
                    sha256: Some(sha256_hex(&bytes)),
                    readonly: Some(readonly),
                },
                Some(bytes),
                Some(readonly),
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((
            CandidateFileState {
                path: rel.into(),
                sha256: None,
                readonly: None,
            },
            None,
            None,
        )),
        Err(e) => Err(format!("inspect {rel}: {e}")),
    }
}

fn capture_states(root: &Path, allowlist: &[String]) -> Result<Vec<CandidateFileState>, String> {
    allowlist
        .iter()
        .map(|rel| read_file_state(&checked_path(root, rel, false)?, rel).map(|v| v.0))
        .collect()
}

fn missing_parent_directories(root: &Path, rel: &str) -> Result<Vec<String>, String> {
    let parent = Path::new(rel).parent().unwrap_or_else(|| Path::new(""));
    let mut relative = PathBuf::new();
    let mut absolute = root.to_path_buf();
    let mut missing = Vec::new();
    for component in parent.components() {
        let Component::Normal(part) = component else {
            return Err("non-normal promotion parent component".into());
        };
        relative.push(part);
        absolute.push(part);
        match fs::symlink_metadata(&absolute) {
            Ok(meta) if meta.file_type().is_symlink() || is_reparse(&meta) => {
                return Err(format!(
                    "promotion parent is a symlink/reparse point: {}",
                    absolute.display()
                ));
            }
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => {
                return Err(format!(
                    "promotion parent is not a directory: {}",
                    absolute.display()
                ))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(relative.to_string_lossy().replace('\\', "/"));
            }
            Err(error) => return Err(format!("inspect promotion parent: {error}")),
        }
    }
    Ok(missing)
}

fn validate_tree_has_no_aliases(root: &Path) -> Result<(), String> {
    let root_meta =
        fs::symlink_metadata(root).map_err(|e| format!("inspect candidate root: {e}"))?;
    if root_meta.file_type().is_symlink() || is_reparse(&root_meta) || !root_meta.is_dir() {
        return Err("candidate root must be a real, non-alias directory".into());
    }
    fn visit(path: &Path) -> Result<(), String> {
        for entry in fs::read_dir(path).map_err(|e| format!("read candidate tree: {e}"))? {
            let entry = entry.map_err(|e| e.to_string())?;
            let meta = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
            if meta.file_type().is_symlink() || is_reparse(&meta) {
                return Err(format!(
                    "candidate tree contains symlink/reparse path: {}",
                    entry.path().display()
                ));
            }
            if meta.is_dir() {
                visit(&entry.path())?;
            } else if !meta.is_file() {
                return Err(format!(
                    "candidate tree contains special file: {}",
                    entry.path().display()
                ));
            }
        }
        Ok(())
    }
    visit(root)
}

fn validate_candidate_file_set(root: &Path, allowlist: &[String]) -> Result<(), String> {
    fn collect(
        root: &Path,
        dir: &Path,
        files: &mut BTreeSet<String>,
        folded_files: &mut BTreeSet<String>,
    ) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| format!("read candidate tree: {e}"))? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if meta.is_dir() {
                collect(root, &path, files, folded_files)?;
            } else if meta.is_file() {
                let rel = path
                    .strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                if !files.insert(rel.clone()) || !folded_files.insert(rel.to_lowercase()) {
                    return Err("candidate tree contains case-colliding paths".into());
                }
            }
        }
        Ok(())
    }
    let expected: BTreeSet<String> = allowlist.iter().cloned().collect();
    let mut actual = BTreeSet::new();
    let mut folded_actual = BTreeSet::new();
    collect(root, root, &mut actual, &mut folded_actual)?;
    if actual != expected.intersection(&actual).cloned().collect() {
        return Err("candidate file set contains non-exact or non-allowlisted paths".into());
    }
    Ok(())
}


fn digest_json<T: Serialize>(value: &T) -> Result<String, String> {
    let bytes =
        serde_json::to_vec(value).map_err(|e| format!("serialize canonical evidence: {e}"))?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
pub(super) fn test_sha256(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("create {}: {e}", path.display()))?;
    use std::io::Write;
    file.write_all(bytes)
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    file.sync_all()
        .map_err(|e| format!("sync {}: {e}", path.display()))
}

fn remove_target_file(path: &Path) -> Result<(), String> {
    let mut permissions = fs::metadata(path)
        .map_err(|e| format!("inspect target before replacement: {e}"))?
        .permissions();
    if permissions.readonly() {
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)
            .map_err(|e| format!("clear readonly target before replacement: {e}"))?;
    }
    fs::remove_file(path).map_err(|e| format!("remove target {}: {e}", path.display()))
}

fn persist_journal(dir: &Path, journal: &PromotionJournal) -> Result<(PathBuf, String), String> {
    let path = if journal.mutation_intents.is_empty() {
        dir.join("journal.json")
    } else {
        dir.join(format!(
            "journal-update-{}-{}.json",
            journal.mutation_intents.len(),
            Uuid::new_v4()
        ))
    };
    let bytes = serde_json::to_vec_pretty(journal).map_err(|e| e.to_string())?;
    write_new_file(&path, &bytes)?;
    let digest = sha256_hex(&bytes);
    Ok((path, digest))
}

/// Offline recovery for retained promotion journals.
pub fn recover_retained_promotion(primary_root: &Path, journal_dir: &Path) -> Result<(), String> {
    recover_retained_promotion_with_digest(primary_root, journal_dir, None)
}

pub fn recover_retained_promotion_with_digest(
    primary_root: &Path,
    journal_dir: &Path,
    expected_digest: Option<&str>,
) -> Result<(), String> {
    recover_retained_promotion_ext(primary_root, journal_dir, None, expected_digest)
}

pub fn recover_retained_promotion_with_failure(
    primary_root: &Path,
    journal_dir: &Path,
    fail_after_restores: Option<usize>,
) -> Result<(), String> {
    recover_retained_promotion_ext(primary_root, journal_dir, fail_after_restores, None)
}

pub fn recover_retained_promotion_restore_with_digest(
    primary_root: &Path,
    journal_dir: &Path,
    fail_after_restores: Option<usize>,
    expected_digest: Option<&str>,
) -> Result<(), String> {
    let root = canonical_existing_dir(primary_root, "recovery primary root")?;
    let journal = canonical_existing_dir(journal_dir, "recovery journal")?;
    let root_parent = root.parent().ok_or("primary root has no parent")?;
    if journal.parent() != Some(root_parent) {
        return Err("recovery journal must be a sibling of the primary root".into());
    }
    let name = journal
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("journal directory name is not UTF-8")?;
    let id = name
        .strip_prefix(".pct-promotion-")
        .ok_or("unexpected journal directory name")?;
    let segments = id.split('-').collect::<Vec<_>>();
    let uuid_str = if segments.len() >= 5 {
        segments[segments.len() - 5..].join("-")
    } else {
        id.to_string()
    };
    Uuid::parse_str(&uuid_str).map_err(|_| "invalid promotion journal identifier")?;

    let (journal_data, latest_count, actual_digest) = load_and_validate_journal_with_digest(&journal)?;
    if let Some(expected) = expected_digest {
        if actual_digest != expected {
            return Err(format!(
                "journal snapshot digest mismatch: expected {}, got {}",
                expected, actual_digest
            ));
        }
    }
    let intents = &journal_data.mutation_intents;

    // Validate every target and required backup before the first mutation.
    for (index, entry) in journal_data.entries.iter().enumerate() {
        let target = checked_path(&root, &entry.path, true)?;
        let (state, _, _) = read_file_state(&target, &entry.path)?;
        if intents.iter().any(|path| path == &entry.path) {
            let before_matches =
                state.sha256 == entry.before_sha256 && state.readonly == entry.before_readonly;
            let after_matches =
                state.sha256 == entry.after_sha256 && state.readonly == entry.after_readonly;
            let absent = state.sha256.is_none() && state.readonly.is_none();
            if !before_matches && !after_matches && !absent {
                return Err(format!(
                    "target differs from both recorded transaction states: {}",
                    entry.path
                ));
            }
        } else if state.sha256 != entry.before_sha256 || state.readonly != entry.before_readonly {
            return Err(format!(
                "non-intended target changed before recovery: {}",
                entry.path
            ));
        }

        let backup = journal.join(&entry.backup_name);
        if entry.existed_before {
            let bytes = read_regular_file_no_alias(&backup)?;
            if Some(sha256_hex(&bytes)) != entry.before_sha256 {
                return Err(format!("backup digest mismatch for {}", entry.path));
            }
        } else if backup.exists() {
            return Err(format!("unexpected backup for new path: {}", entry.path));
        }

        let staged = journal.join(&entry.staged_name);
        if entry.after_sha256.is_some() && staged.exists() {
            let bytes = read_regular_file_no_alias(&staged)?;
            if Some(sha256_hex(&bytes)) != entry.after_sha256 {
                return Err(format!("staged digest mismatch for {}", entry.path));
            }
        } else if entry.after_sha256.is_some()
            && !staged.exists()
            && !(intents.iter().any(|path| path == &entry.path)
                && state.sha256 == entry.after_sha256
                && state.readonly == entry.after_readonly)
            && !(intents.iter().any(|path| path == &entry.path)
                && state.sha256 == entry.before_sha256
                && state.readonly == entry.before_readonly)
        {
            return Err(format!("staged output missing for {}", entry.path));
        } else if entry.after_sha256.is_none() && staged.exists() {
            return Err(format!(
                "unexpected staged output for deletion: {}",
                entry.path
            ));
        }

        let recovery_staged = journal.join(format!("recovery-staged-{index}"));
        if recovery_staged.exists() {
            if !entry.existed_before {
                return Err(format!(
                    "unexpected recovery-stage payload for new path: {}",
                    entry.path
                ));
            }
            let bytes = read_regular_file_no_alias(&recovery_staged)?;
            if Some(sha256_hex(&bytes)) != entry.before_sha256 {
                return Err(format!("recovery-stage digest mismatch for {}", entry.path));
            }
        }

        for created in &entry.created_parent_dirs {
            if !Path::new(&entry.path).starts_with(Path::new(created))
                || normalize_allowlist(&[created.clone()])?.first() != Some(created)
            {
                return Err(format!("invalid created-parent record for {}", entry.path));
            }
        }
        if entry.backup_name != format!("backup-{index}")
            || entry.staged_name != format!("staged-{index}")
        {
            return Err("unexpected backup/staged filename in journal".into());
        }
    }

    for (restore_index, path) in intents.iter().rev().enumerate() {
        let entry = journal_data
            .entries
            .iter()
            .find(|entry| &entry.path == path)
            .ok_or("validated journal intent has no entry")?;
        let target = checked_path(&root, &entry.path, true)?;
        let (state, _, _) = read_file_state(&target, &entry.path)?;
        if state.sha256 != entry.before_sha256 || state.readonly != entry.before_readonly {
            if entry.existed_before {
                let index = entry_index(&journal_data, entry)?;
                let recovery_stage = journal.join(format!("recovery-staged-{index}"));
                if !recovery_stage.exists() {
                    let bytes = read_regular_file_no_alias(&journal.join(&entry.backup_name))?;
                    write_new_file(&recovery_stage, &bytes)?;
                }
                let staged_bytes = read_regular_file_no_alias(&recovery_stage)?;
                if Some(sha256_hex(&staged_bytes)) != entry.before_sha256 {
                    return Err(format!("recovery-stage digest mismatch for {}", entry.path));
                }
                let mut permissions = fs::metadata(&recovery_stage)
                    .map_err(|error| error.to_string())?
                    .permissions();
                permissions.set_readonly(entry.before_readonly.unwrap_or(false));
                fs::set_permissions(&recovery_stage, permissions)
                    .map_err(|error| error.to_string())?;
                if target.exists() {
                    remove_target_file(&target)?;
                }
                fs::rename(&recovery_stage, &target)
                    .map_err(|error| format!("install recovery-stage file: {error}"))?;
            } else if state.sha256.is_some() {
                remove_target_file(&target)?;
            }
        }
        if fail_after_restores == Some(restore_index + 1) {
            return Err("injected interruption after completed recovery entry".into());
        }
    }

    let intended = intents.iter().cloned().collect::<Vec<_>>();
    if capture_states(&root, &intended)?
        != journal_data
            .entries
            .iter()
            .filter(|entry| intents.contains(&entry.path))
            .map(|entry| CandidateFileState {
                path: entry.path.clone(),
                sha256: entry.before_sha256.clone(),
                readonly: entry.before_readonly,
            })
            .collect::<Vec<_>>()
    {
        return Err("recovered primary state does not match recorded baseline".into());
    }
    let _ = latest_count;
    Ok(())
}

pub fn cleanup_recovered_promotion_journal(
    primary_root: &Path,
    journal_dir: &Path,
) -> Result<(), String> {
    let root = canonical_existing_dir(primary_root, "recovery primary root")?;
    let journal = canonical_existing_dir(journal_dir, "recovery journal")?;
    let root_parent = root.parent().ok_or("primary root has no parent")?;
    if journal.parent() != Some(root_parent) {
        return Err("recovery journal must be a sibling of the primary root".into());
    }
    let name = journal
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("journal directory name is not UTF-8")?;
    let id = name
        .strip_prefix(".pct-promotion-")
        .ok_or("unexpected journal directory name")?;
    let segments = id.split('-').collect::<Vec<_>>();
    let uuid_str = if segments.len() >= 5 {
        segments[segments.len() - 5..].join("-")
    } else {
        id.to_string()
    };
    Uuid::parse_str(&uuid_str).map_err(|_| "invalid promotion journal identifier")?;

    fs::remove_dir_all(&journal).map_err(|error| format!("remove recovered journal: {error}"))
}

pub fn cleanup_recovered_promotion_journal_authenticated(
    primary_root: &Path,
    journal_dir: &Path,
    expected_run_id: &str,
    expected_candidate_root: &Path,
    expected_phase: PromotionPhase,
    expected_digest: &str,
) -> Result<(), String> {
    // 1. Enforce that expected_phase is an authorized cleanup phase (§112.4.3)
    match expected_phase {
        PromotionPhase::RecoveryRestored | PromotionPhase::Applied => {}
        other => {
            return Err(format!(
                "cannot clean up journal in unauthorized phase {:?}; cleanup is only authorized in RecoveryRestored or Applied",
                other
            ));
        }
    }

    // 2. Inspect journal_dir presence first with symlink_metadata
    let journal_meta = fs::symlink_metadata(journal_dir)
        .map_err(|e| format!("read journal symlink metadata: {e}"))?;
    if is_path_alias_or_reparse(&journal_meta) {
        return Err("recovery journal directory must not be a symlink or reparse point".into());
    }
    if !journal_meta.is_dir() {
        return Err("recovery journal path is not a directory".into());
    }

    let root = canonical_existing_dir(primary_root, "recovery primary root")?;
    let journal = canonical_existing_dir(journal_dir, "recovery journal")?;

    // Reject aliases, symlinks, and reparse points
    let meta = fs::symlink_metadata(&journal).map_err(|e| format!("read journal symlink metadata: {e}"))?;
    if is_path_alias_or_reparse(&meta) {
        return Err("recovery journal directory must not be a symlink or reparse point".into());
    }

    // Must be a sibling of the primary root
    let root_parent = root.parent().ok_or("primary root has no parent")?;
    if journal.parent() != Some(root_parent) {
        return Err("recovery journal must be a sibling of the primary root".into());
    }

    // Directory name must be owned by expected_run_id: .pct-promotion-{expected_run_id}-{uuid}
    let name = journal
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("journal directory name is not UTF-8")?;
    let expected_prefix = format!(".pct-promotion-{}-", expected_run_id);
    let uuid_str = name
        .strip_prefix(&expected_prefix)
        .ok_or_else(|| format!("journal directory name {name:?} does not start with expected prefix {expected_prefix:?}"))?;
    Uuid::parse_str(uuid_str).map_err(|_| "invalid promotion journal UUID suffix")?;

    // Candidate root must exist, be canonical, and distinct from primary root
    let cand_canonical = canonical_existing_dir(expected_candidate_root, "candidate root")?;
    if cand_canonical == root {
        return Err("candidate root cannot be identical to primary root".into());
    }

    // Parse and validate journal and compare exact snapshot digest
    let (_journal_data, _count, actual_digest) = load_and_validate_journal_with_digest(&journal)?;
    if actual_digest != expected_digest {
        return Err(format!(
            "journal snapshot digest mismatch: expected {}, got {}",
            expected_digest, actual_digest
        ));
    }

    // Reject unknown files in journal directory
    for entry in fs::read_dir(&journal).map_err(|e| format!("read journal directory: {e}"))? {
        let entry = entry.map_err(|e| format!("read journal entry: {e}"))?;
        let file_type = entry.file_type().map_err(|e| format!("read file type: {e}"))?;
        if !file_type.is_file() {
            return Err(format!("unexpected non-file entry in journal dir: {:?}", entry.file_name()));
        }
        let fname = entry.file_name();
        let fname_str = fname.to_str().ok_or("journal file name not utf-8")?;
        let valid = fname_str == "journal.json"
            || (fname_str.starts_with("journal-update-") && fname_str.ends_with(".json"))
            || fname_str.starts_with("backup-")
            || fname_str.starts_with("staged-");
        if !valid {
            return Err(format!("unexpected foreign file in journal directory: {fname_str}"));
        }
    }

    fs::remove_dir_all(&journal).map_err(|error| format!("remove recovered journal: {error}"))
}

pub fn recover_retained_promotion_ext(
    primary_root: &Path,
    journal_dir: &Path,
    fail_after_restores: Option<usize>,
    expected_digest: Option<&str>,
) -> Result<(), String> {
    recover_retained_promotion_restore_with_digest(
        primary_root,
        journal_dir,
        fail_after_restores,
        expected_digest,
    )?;
    cleanup_recovered_promotion_journal(primary_root, journal_dir)
}

fn entry_index(journal: &PromotionJournal, entry: &JournalEntry) -> Result<usize, String> {
    journal
        .entries
        .iter()
        .position(|candidate| candidate.path == entry.path)
        .ok_or_else(|| "journal entry index not found".to_string())
}

pub fn load_and_validate_journal(dir: &Path) -> Result<(PromotionJournal, usize), String> {
    load_and_validate_journal_with_digest(dir).map(|(journal, count, _)| (journal, count))
}

pub fn load_and_validate_journal_with_digest(
    dir: &Path,
) -> Result<(PromotionJournal, usize, String), String> {
    let dir_meta = fs::symlink_metadata(dir)
        .map_err(|error| format!("inspect journal directory metadata: {error}"))?;
    if is_path_alias_or_reparse(&dir_meta) {
        return Err("journal directory is a symlink or reparse point".into());
    }
    if !dir_meta.is_dir() {
        return Err("journal directory path is not a directory".into());
    }
    let mut snapshots = Vec::<(usize, PromotionJournal, String)>::new();
    let mut allowed = BTreeSet::from(["journal.json".to_string()]);
    for item in fs::read_dir(dir).map_err(|error| format!("read journal directory: {error}"))? {
        let item = item.map_err(|error| error.to_string())?;
        let name = item.file_name().to_string_lossy().into_owned();
        let path = item.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
            return Err("journal contains a non-regular or aliased entry".into());
        }
        if name == "journal.json" {
            let data = read_regular_file_no_alias(&path)?;
            let digest = sha256_hex(&data);
            snapshots.push((
                0,
                serde_json::from_slice(&data).map_err(|e| format!("parse journal: {e}"))?,
                digest,
            ));
            continue;
        }
        if let Some(rest) = name.strip_prefix("journal-update-") {
            let (count, id) = rest
                .split_once('-')
                .ok_or("malformed journal snapshot name")?;
            let count = count
                .parse::<usize>()
                .map_err(|_| "invalid snapshot count")?;
            Uuid::parse_str(id.strip_suffix(".json").ok_or("invalid snapshot suffix")?)
                .map_err(|_| "invalid snapshot identifier")?;
            allowed.insert(name.clone());
            let data = read_regular_file_no_alias(&path)?;
            let digest = sha256_hex(&data);
            snapshots.push((
                count,
                serde_json::from_slice(&data)
                    .map_err(|e| format!("parse journal snapshot: {e}"))?,
                digest,
            ));
            continue;
        }
        if name.starts_with("backup-") || name.starts_with("staged-") {
            allowed.insert(name);
            continue;
        }
        if name.starts_with("recovery-staged-") {
            allowed.insert(name);
            continue;
        }
        return Err("journal contains an unexpected entry".into());
    }
    if snapshots.is_empty() {
        return Err("journal has no valid snapshots".into());
    }
    let zero_snapshots = snapshots
        .iter()
        .filter(|(count, _, _)| *count == 0)
        .collect::<Vec<_>>();
    if zero_snapshots.len() != 1 {
        return Err("journal must contain exactly one initial snapshot".into());
    }
    let base = &zero_snapshots[0].1;
    if base.version != 2 {
        return Err("unsupported promotion journal version".into());
    }
    let paths = base
        .entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    if normalize_allowlist(&paths)? != paths || paths.is_empty() {
        return Err("journal entry paths are not a unique normalized allowlist".into());
    }
    let mut observed_counts = BTreeSet::new();
    for (count, snapshot, _) in &snapshots {
        if snapshot.version != 2
            || snapshot.entries != base.entries
            || *count != snapshot.mutation_intents.len()
            || snapshot.mutation_intents.len() > snapshot.entries.len()
            || snapshot.mutation_intents != paths[..snapshot.mutation_intents.len()]
            || !observed_counts.insert(*count)
        {
            return Err("journal snapshot is inconsistent or has non-prefix intents".into());
        }
    }
    for (index, entry) in base.entries.iter().enumerate() {
        if entry.existed_before != entry.before_sha256.is_some()
            || entry.existed_before != entry.before_readonly.is_some()
            || entry.after_sha256.is_some() != entry.after_readonly.is_some()
        {
            return Err("journal entry state metadata is inconsistent".into());
        }
        let normalized_created = entry
            .created_parent_dirs
            .iter()
            .map(|path| normalize_allowlist(&[path.clone()]).map(|items| items[0].clone()))
            .collect::<Result<Vec<_>, _>>()?;
        if normalized_created != entry.created_parent_dirs
            || entry.created_parent_dirs.iter().any(|path| {
                !Path::new(&entry.path).starts_with(Path::new(path))
                    || Path::new(&entry.path).components().count()
                        <= Path::new(path).components().count()
            })
            || entry.backup_name != format!("backup-{index}")
            || entry.staged_name != format!("staged-{index}")
        {
            return Err("journal entry path or payload names are invalid".into());
        }
    }
    let max_count = snapshots.iter().map(|(count, _, _)| *count).max().unwrap_or(0);
    let latest = snapshots
        .iter()
        .filter(|(count, _, _)| *count == max_count)
        .collect::<Vec<_>>();
    if latest.len() != 1 {
        return Err("ambiguous latest journal snapshot".into());
    }
    let mut required = BTreeSet::from(["journal.json".to_string()]);
    for (index, entry) in base.entries.iter().enumerate() {
        if entry.existed_before {
            let backup = format!("backup-{index}");
            allowed.insert(backup.clone());
            required.insert(backup);
        }
        if base.entries[index].after_sha256.is_some() {
            allowed.insert(format!("staged-{index}"));
        }
        if entry.existed_before {
            allowed.insert(format!("recovery-staged-{index}"));
        }
    }
    let actual = fs::read_dir(dir)
        .map_err(|error| error.to_string())?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<BTreeSet<_>, _>>()
        .map_err(|error| error.to_string())?;
    if !actual.is_subset(&allowed) || !actual.is_superset(&required) {
        return Err("journal entry set does not match its manifest".into());
    }
    Ok((latest[0].1.clone(), max_count, latest[0].2.clone()))
}

/// Inspect all ancestors of candidate_parent before creating anything, rejecting
/// symlink/reparse points, and creating missing path components with create-new semantics.
pub fn validate_and_create_candidate_parent(candidate_parent: &Path) -> Result<PathBuf, String> {
    if !candidate_parent.is_absolute() {
        return Err("candidate parent must be absolute".into());
    }
    let mut cursor = PathBuf::new();
    for component in candidate_parent.components() {
        match component {
            Component::Prefix(prefix) => {
                cursor.push(prefix.as_os_str());
                continue;
            }
            Component::RootDir => {
                cursor.push(component.as_os_str());
                continue;
            }
            Component::Normal(part) => {
                cursor.push(part);
            }
            Component::CurDir | Component::ParentDir => {
                return Err("candidate parent contains non-normal component".into());
            }
        }
        if cursor.exists() {
            let meta = fs::symlink_metadata(&cursor)
                .map_err(|e| format!("inspect candidate parent component {}: {e}", cursor.display()))?;
            if meta.file_type().is_symlink() || is_reparse(&meta) {
                return Err(format!(
                    "symlink/reparse ancestor is forbidden in candidate parent: {}",
                    cursor.display()
                ));
            }
            if !meta.is_dir() {
                return Err(format!(
                    "candidate parent ancestor is not a directory: {}",
                    cursor.display()
                ));
            }
        } else {
            fs::create_dir(&cursor)
                .map_err(|e| format!("create candidate parent component {}: {e}", cursor.display()))?;
            let meta = fs::symlink_metadata(&cursor)
                .map_err(|e| format!("inspect newly created candidate parent component {}: {e}", cursor.display()))?;
            if meta.file_type().is_symlink() || is_reparse(&meta) {
                return Err(format!(
                    "newly created candidate parent component is reparse/symlink: {}",
                    cursor.display()
                ));
            }
        }
    }
    candidate_parent
        .canonicalize()
        .map_err(|e| format!("canonicalize candidate parent: {e}"))
}

fn read_regular_file_no_alias(path: &Path) -> Result<Vec<u8>, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("inspect journal file: {error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
        return Err("journal payload is not a regular non-alias file".into());
    }
    fs::read(path).map_err(|error| format!("read journal payload: {error}"))
}

fn rollback(
    root: &Path,
    journal_dir: &Path,
    journal: &PromotionJournal,
    fail: bool,
) -> Result<(), String> {
    if fail {
        return Err("injected rollback failure".into());
    }
    for path in journal.mutation_intents.iter().rev() {
        let entry = journal
            .entries
            .iter()
            .find(|e| &e.path == path)
            .ok_or("journal entry missing")?;
        let target = checked_path(root, &entry.path, true)?;
        if target.exists() {
            remove_target_file(&target)?;
        }
        if entry.existed_before {
            let backup = fs::read(journal_dir.join(&entry.backup_name))
                .map_err(|e| format!("read rollback backup: {e}"))?;
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            write_new_file(&target, &backup)?;
            if entry.before_readonly.unwrap_or(false) {
                let mut permissions = fs::metadata(&target)
                    .map_err(|e| e.to_string())?
                    .permissions();
                permissions.set_readonly(true);
                fs::set_permissions(&target, permissions).map_err(|e| e.to_string())?;
            }
        }
        for rel_dir in entry.created_parent_dirs.iter().rev() {
            let dir = checked_path(root, rel_dir, true)?;
            match fs::remove_dir(&dir) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
                Err(error) => return Err(format!("remove created rollback directory: {error}")),
            }
        }
    }
    Ok(())
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, PathBuf, PathBuf, PathBuf) {
        let temp = TempDir::new().unwrap();
        let primary = temp.path().join("primary");
        let engine = temp.path().join("engine");
        let candidate = temp.path().join("candidate");
        fs::create_dir(&primary).unwrap();
        fs::create_dir(&engine).unwrap();
        fs::write(primary.join("change.txt"), b"before").unwrap();
        fs::write(primary.join("keep.txt"), b"untouched").unwrap();
        fs::write(engine.join("key.fixture"), b"engine-state").unwrap();
        (temp, primary, engine, candidate)
    }

    fn approve_current_delta(workspace: &ImplementerWorkspace) -> PromotionPermit {
        let delta_digest = workspace.delta_digest().unwrap();
        workspace
            .authorize_delta(workspace.baseline_digest(), &delta_digest)
            .unwrap()
    }

    #[test]
    fn stage2_candidate_copies_only_allowlist_and_detects_delta() {
        let (_temp, primary, engine, candidate) = fixture();
        let workspace = ImplementerWorkspace::create(
            &primary,
            &engine,
            &candidate,
            &["change.txt".into(), "new.txt".into()],
        )
        .unwrap();
        assert_eq!(
            fs::read(workspace.candidate_root().join("change.txt")).unwrap(),
            b"before"
        );
        assert!(!workspace.candidate_root().join("keep.txt").exists());
        fs::write(workspace.candidate_root().join("change.txt"), b"after").unwrap();
        fs::write(workspace.candidate_root().join("new.txt"), b"added").unwrap();
        let delta = workspace.delta().unwrap();
        assert_eq!(delta.len(), 2);
        assert!(workspace
            .authorize_delta(workspace.baseline_digest(), "wrong-digest")
            .is_err());
        assert!(workspace
            .authorize_delta("wrong-baseline", &workspace.delta_digest().unwrap())
            .is_err());
        assert_eq!(fs::read(primary.join("change.txt")).unwrap(), b"before");
        assert_eq!(fs::read(primary.join("keep.txt")).unwrap(), b"untouched");
    }

    #[test]
    fn stage2_rejects_unsafe_or_case_colliding_allowlists() {
        for invalid in ["../escape", "C:/drive", "/absolute", "a\\b"] {
            assert!(normalize_allowlist(&[invalid.into()]).is_err(), "{invalid}");
        }
        assert!(normalize_allowlist(&["Dir/A.txt".into(), "dir/a.txt".into()]).is_err());
    }

    #[test]
    fn stage2_rejects_candidate_files_outside_allowlist() {
        let (_temp, primary, engine, candidate) = fixture();
        let workspace =
            ImplementerWorkspace::create(&primary, &engine, &candidate, &["change.txt".into()])
                .unwrap();
        fs::write(
            workspace.candidate_root().join("unapproved.txt"),
            b"unexpected",
        )
        .unwrap();
        assert!(workspace.delta().is_err());
    }

    #[test]
    fn stage2_rejects_case_mismatched_candidate_path() {
        let (_temp, primary, engine, candidate) = fixture();
        let workspace =
            ImplementerWorkspace::create(&primary, &engine, &candidate, &["change.txt".into()])
                .unwrap();
        let existing = workspace.candidate_root().join("change.txt");
        fs::rename(&existing, workspace.candidate_root().join("CHANGE.txt")).unwrap();
        assert!(workspace.delta().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn stage2_rejects_candidate_root_symlink_alias() {
        use std::os::unix::fs::symlink;

        let (_temp, primary, engine, candidate) = fixture();
        let workspace =
            ImplementerWorkspace::create(&primary, &engine, &candidate, &["change.txt".into()])
                .unwrap();
        fs::remove_dir_all(workspace.candidate_root()).unwrap();
        symlink(&primary, workspace.candidate_root()).unwrap();
        assert!(workspace.delta().is_err());
        assert_eq!(fs::read(primary.join("change.txt")).unwrap(), b"before");
        fs::remove_file(workspace.candidate_root()).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn stage2_rejects_candidate_root_junction_alias() {
        let (_temp, primary, engine, candidate) = fixture();
        let workspace =
            ImplementerWorkspace::create(&primary, &engine, &candidate, &["change.txt".into()])
                .unwrap();
        fs::remove_dir_all(workspace.candidate_root()).unwrap();
        let output = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                workspace.candidate_root().to_str().unwrap(),
                primary.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success() && workspace.candidate_root().exists(),
            "[UNVERIFIED] candidate-root junction fixture setup failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(workspace.delta().is_err());
        assert_eq!(fs::read(primary.join("change.txt")).unwrap(), b"before");
        fs::remove_dir(workspace.candidate_root()).unwrap();
    }

    #[test]
    fn stage2_validates_and_promotes_deletions() {
        let (_temp, primary, engine, candidate) = fixture();
        let workspace =
            ImplementerWorkspace::create(&primary, &engine, &candidate, &["change.txt".into()])
                .unwrap();
        fs::remove_file(workspace.candidate_root().join("change.txt")).unwrap();
        let delta = workspace.delta().unwrap();
        assert_eq!(delta.len(), 1);
        assert!(delta[0].before_sha256.is_some());
        assert!(delta[0].after_sha256.is_none());
        let permit = approve_current_delta(&workspace);
        assert_eq!(
            workspace.promote(&permit).unwrap(),
            PromotionOutcome::Promoted { changed_files: 1 }
        );
        assert!(!primary.join("change.txt").exists());
        assert_eq!(fs::read(primary.join("keep.txt")).unwrap(), b"untouched");
    }

    #[test]
    fn stage2_replaces_and_rolls_back_readonly_primary_files() {
        let (_temp, primary, engine, candidate) = fixture();
        let original = primary.join("change.txt");
        let mut original_permissions = fs::metadata(&original).unwrap().permissions();
        original_permissions.set_readonly(true);
        fs::set_permissions(&original, original_permissions).unwrap();
        let workspace =
            ImplementerWorkspace::create(&primary, &engine, &candidate, &["change.txt".into()])
                .unwrap();
        let candidate_file = workspace.candidate_root().join("change.txt");
        let mut candidate_permissions = fs::metadata(&candidate_file).unwrap().permissions();
        candidate_permissions.set_readonly(false);
        fs::set_permissions(&candidate_file, candidate_permissions).unwrap();
        fs::write(&candidate_file, b"after").unwrap();
        let permit = approve_current_delta(&workspace);
        assert!(workspace
            .promote_with_failure(&permit, Some(0), false, None)
            .is_err());
        assert_eq!(fs::read(&original).unwrap(), b"before");
        assert!(fs::metadata(&original).unwrap().permissions().readonly());
        assert_eq!(
            workspace.promote(&permit).unwrap(),
            PromotionOutcome::Promoted { changed_files: 1 }
        );
        assert_eq!(fs::read(&original).unwrap(), b"after");
    }

    #[cfg(windows)]
    #[test]
    fn stage2_rejects_candidate_junction_aliases() {
        let (_temp, primary, engine, candidate) = fixture();
        let workspace =
            ImplementerWorkspace::create(&primary, &engine, &candidate, &["change.txt".into()])
                .unwrap();
        let alias = workspace.candidate_root().join("alias");
        let output = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                alias.to_str().unwrap(),
                primary.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success() && alias.exists(),
            "[UNVERIFIED] junction fixture setup failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            workspace.delta().is_err(),
            "candidate junction must be rejected"
        );
    }

    #[test]
    fn stage2_promotion_requires_current_baseline_and_changes_only_allowlist() {
        let (_temp, primary, engine, candidate) = fixture();
        let workspace =
            ImplementerWorkspace::create(&primary, &engine, &candidate, &["change.txt".into()])
                .unwrap();
        fs::write(workspace.candidate_root().join("change.txt"), b"approved").unwrap();
        let permit = approve_current_delta(&workspace);
        assert_eq!(
            workspace.promote(&permit).unwrap(),
            PromotionOutcome::Promoted { changed_files: 1 }
        );
        assert_eq!(fs::read(primary.join("change.txt")).unwrap(), b"approved");
        assert_eq!(fs::read(primary.join("keep.txt")).unwrap(), b"untouched");

        let (_temp2, primary2, engine2, candidate2) = fixture();
        let stale =
            ImplementerWorkspace::create(&primary2, &engine2, &candidate2, &["change.txt".into()])
                .unwrap();
        fs::write(stale.candidate_root().join("change.txt"), b"approved").unwrap();
        let stale_permit = approve_current_delta(&stale);
        fs::write(primary2.join("change.txt"), b"concurrent change").unwrap();
        assert_eq!(
            stale.promote(&stale_permit).unwrap(),
            PromotionOutcome::RejectedBaselineChanged
        );
    }

    fn stage2_multi_delta_fixture() -> (
        TempDir,
        PathBuf,
        ImplementerWorkspace,
        Vec<(PathBuf, Vec<u8>)>,
    ) {
        let (temp, primary, engine, candidate) = fixture();
        fs::write(primary.join("old-name.txt"), b"rename source").unwrap();
        fs::create_dir(primary.join(".git")).unwrap();
        fs::write(primary.join(".git/index"), b"staged-index-sentinel").unwrap();
        fs::write(primary.join("ignored.cache"), b"ignored-sentinel").unwrap();
        fs::write(primary.join("untracked.txt"), b"untracked-sentinel").unwrap();
        let protected = [
            primary.join("keep.txt"),
            primary.join(".git/index"),
            primary.join("ignored.cache"),
            primary.join("untracked.txt"),
        ]
        .into_iter()
        .map(|path| (path.clone(), fs::read(path).unwrap()))
        .collect();
        let workspace = ImplementerWorkspace::create(
            &primary,
            &engine,
            &candidate,
            &[
                "change.txt".into(),
                "new.bin".into(),
                "new-name.txt".into(),
                "old-name.txt".into(),
            ],
        )
        .unwrap();
        fs::write(
            workspace.candidate_root().join("change.txt"),
            b"changed bytes",
        )
        .unwrap();
        fs::write(workspace.candidate_root().join("new.bin"), [0, 1, 255, 0]).unwrap();
        fs::rename(
            workspace.candidate_root().join("old-name.txt"),
            workspace.candidate_root().join("new-name.txt"),
        )
        .unwrap();
        (temp, primary, workspace, protected)
    }

    #[test]
    fn stage2_successful_multi_file_binary_rename_promotion_preserves_sentinels() {
        let (_temp, primary, workspace, protected) = stage2_multi_delta_fixture();
        let delta = workspace.delta().unwrap();
        assert_eq!(delta.len(), 4, "modify + binary add + rename delete/add");
        let permit = approve_current_delta(&workspace);
        assert_eq!(
            workspace.promote(&permit).unwrap(),
            PromotionOutcome::Promoted { changed_files: 4 }
        );
        assert_eq!(
            fs::read(primary.join("change.txt")).unwrap(),
            b"changed bytes"
        );
        assert_eq!(fs::read(primary.join("new.bin")).unwrap(), [0, 1, 255, 0]);
        assert!(!primary.join("old-name.txt").exists());
        assert_eq!(
            fs::read(primary.join("new-name.txt")).unwrap(),
            b"rename source"
        );
        for (path, bytes) in protected {
            assert_eq!(
                fs::read(&path).unwrap(),
                bytes,
                "protected sentinel: {path:?}"
            );
        }
    }

    #[test]
    fn stage2_rolls_back_exact_baseline_at_every_apply_boundary() {
        let (_temp, primary, workspace, _protected) = stage2_multi_delta_fixture();
        let approved_baseline = capture_states(
            &primary,
            &[
                "change.txt".into(),
                "new.bin".into(),
                "new-name.txt".into(),
                "old-name.txt".into(),
            ],
        )
        .unwrap();
        let permit = approve_current_delta(&workspace);
        let delta_count = permit.entries.len();
        for applied_count in 0..=delta_count {
            let (_temp, primary, workspace, protected) = stage2_multi_delta_fixture();
            let permit = approve_current_delta(&workspace);
            assert_eq!(permit.entries.len(), delta_count);
            assert!(workspace
                .promote_with_failure(&permit, Some(applied_count), false, None)
                .is_err());
            assert_eq!(
                capture_states(
                    &primary,
                    &[
                        "change.txt".into(),
                        "new.bin".into(),
                        "new-name.txt".into(),
                        "old-name.txt".into(),
                    ],
                )
                .unwrap(),
                approved_baseline,
                "allowlisted baseline after injected failure at boundary {applied_count}"
            );
            for (path, bytes) in protected {
                assert_eq!(
                    fs::read(&path).unwrap(),
                    bytes,
                    "protected sentinel: {path:?}"
                );
            }
            assert!(!primary.join("new.bin").exists());
            assert!(!primary.join("new-name.txt").exists());
            assert!(!primary.parent().unwrap().read_dir().unwrap().any(|entry| {
                entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".pct-promotion-")
            }));
        }
    }

    #[test]
    fn stage2_injected_promotion_failure_rolls_back_or_retains_recovery_journal() {
        let (_temp, primary, engine, candidate) = fixture();
        let workspace = ImplementerWorkspace::create(
            &primary,
            &engine,
            &candidate,
            &["change.txt".into(), "new.txt".into()],
        )
        .unwrap();
        fs::write(workspace.candidate_root().join("change.txt"), b"after").unwrap();
        fs::write(workspace.candidate_root().join("new.txt"), b"new").unwrap();
        let permit = approve_current_delta(&workspace);
        assert!(workspace
            .promote_with_failure(&permit, Some(1), false, None)
            .is_err());
        assert_eq!(fs::read(primary.join("change.txt")).unwrap(), b"before");
        assert!(!primary.join("new.txt").exists());

        let outcome = workspace
            .promote_with_failure(&permit, Some(1), true, None)
            .unwrap();
        let PromotionOutcome::RecoveryRequired { journal, .. } = outcome else {
            panic!("expected recovery-required")
        };
        assert!(journal.join("journal.json").is_file());
        assert!(journal.join("backup-0").is_file());
    }

    #[test]
    fn stage2_rollback_removes_new_empty_parent_directories() {
        let (_temp, primary, engine, candidate) = fixture();
        fs::write(primary.join("z.txt"), b"before second delta").unwrap();
        let workspace = ImplementerWorkspace::create(
            &primary,
            &engine,
            &candidate,
            &["nested/new.txt".into(), "z.txt".into()],
        )
        .unwrap();
        fs::create_dir(workspace.candidate_root().join("nested")).unwrap();
        fs::write(workspace.candidate_root().join("nested/new.txt"), b"new").unwrap();
        fs::write(workspace.candidate_root().join("z.txt"), b"second delta").unwrap();
        let permit = approve_current_delta(&workspace);

        assert!(workspace
            .promote_with_failure(&permit, Some(1), false, None)
            .is_err());
        assert!(!primary.join("nested").exists());
        assert_eq!(
            fs::read(primary.join("z.txt")).unwrap(),
            b"before second delta"
        );
        assert_eq!(fs::read(primary.join("keep.txt")).unwrap(), b"untouched");
    }

    #[test]
    fn stage2_journal_update_failure_rolls_back_prior_applied_entries() {
        let (_temp, primary, engine, candidate) = fixture();
        fs::write(primary.join("z.txt"), b"before z").unwrap();
        let workspace = ImplementerWorkspace::create(
            &primary,
            &engine,
            &candidate,
            &["change.txt".into(), "z.txt".into()],
        )
        .unwrap();
        fs::write(
            workspace.candidate_root().join("change.txt"),
            b"after change",
        )
        .unwrap();
        fs::write(workspace.candidate_root().join("z.txt"), b"after z").unwrap();
        let permit = approve_current_delta(&workspace);

        assert!(workspace
            .promote_with_failure(&permit, None, false, Some(1))
            .is_err());
        assert_eq!(fs::read(primary.join("change.txt")).unwrap(), b"before");
        assert_eq!(fs::read(primary.join("z.txt")).unwrap(), b"before z");
        assert_eq!(fs::read(primary.join("keep.txt")).unwrap(), b"untouched");
    }

    #[test]
    fn stage2_recovery_restores_retained_partial_promotion_and_removes_journal() {
        let (_temp, primary, engine, candidate) = fixture();
        let workspace = ImplementerWorkspace::create(
            &primary,
            &engine,
            &candidate,
            &[
                "aa/nested-new.txt".into(),
                "change.txt".into(),
                "zz-new.txt".into(),
            ],
        )
        .unwrap();
        fs::create_dir(workspace.candidate_root().join("aa")).unwrap();
        fs::write(
            workspace.candidate_root().join("aa/nested-new.txt"),
            b"nested new",
        )
        .unwrap();
        fs::write(workspace.candidate_root().join("change.txt"), b"after").unwrap();
        fs::write(
            workspace.candidate_root().join("zz-new.txt"),
            b"not applied",
        )
        .unwrap();
        let permit = approve_current_delta(&workspace);
        let outcome = workspace
            .promote_with_failure(&permit, Some(2), true, None)
            .unwrap();
        let PromotionOutcome::RecoveryRequired { journal, .. } = outcome else {
            panic!("expected retained recovery journal")
        };
        assert_eq!(
            fs::read(primary.join("aa/nested-new.txt")).unwrap(),
            b"nested new"
        );
        assert_eq!(fs::read(primary.join("change.txt")).unwrap(), b"after");
        assert!(!primary.join("zz-new.txt").exists());

        assert!(recover_retained_promotion_with_failure(&primary, &journal, Some(1)).is_err());
        assert!(
            journal.exists(),
            "interrupted recovery must retain its journal"
        );
        recover_retained_promotion(&primary, &journal).unwrap();
        assert!(!primary.join("aa/nested-new.txt").exists());
        assert!(primary.join("aa").is_dir());
        assert_eq!(fs::read(primary.join("change.txt")).unwrap(), b"before");
        assert!(!primary.join("zz-new.txt").exists());
        assert_eq!(fs::read(primary.join("keep.txt")).unwrap(), b"untouched");
        assert!(!journal.exists());
    }

    #[test]
    fn stage2_recovery_rejects_unrecognized_target_change_without_mutation() {
        let (_temp, primary, engine, candidate) = fixture();
        let workspace = ImplementerWorkspace::create(
            &primary,
            &engine,
            &candidate,
            &["change.txt".into(), "new.txt".into()],
        )
        .unwrap();
        fs::write(workspace.candidate_root().join("change.txt"), b"after").unwrap();
        fs::write(workspace.candidate_root().join("new.txt"), b"new").unwrap();
        let permit = approve_current_delta(&workspace);
        let outcome = workspace
            .promote_with_failure(&permit, Some(1), true, None)
            .unwrap();
        let PromotionOutcome::RecoveryRequired { journal, .. } = outcome else {
            panic!("expected retained recovery journal")
        };
        fs::write(primary.join("change.txt"), b"external edit").unwrap();

        assert!(recover_retained_promotion(&primary, &journal).is_err());
        assert_eq!(
            fs::read(primary.join("change.txt")).unwrap(),
            b"external edit"
        );
        assert!(journal.exists(), "recovery evidence must be retained");
    }

    #[test]
    fn stage2_recovery_restores_an_interrupted_deletion() {
        let (_temp, primary, engine, candidate) = fixture();
        let workspace = ImplementerWorkspace::create(
            &primary,
            &engine,
            &candidate,
            &["change.txt".into(), "keep.txt".into()],
        )
        .unwrap();
        fs::remove_file(workspace.candidate_root().join("change.txt")).unwrap();
        fs::write(workspace.candidate_root().join("keep.txt"), b"after").unwrap();
        let permit = approve_current_delta(&workspace);
        let outcome = workspace
            .promote_with_failure(&permit, Some(1), true, None)
            .unwrap();
        let PromotionOutcome::RecoveryRequired { journal, .. } = outcome else {
            panic!("expected retained recovery journal")
        };
        assert!(!primary.join("change.txt").exists());

        recover_retained_promotion(&primary, &journal).unwrap();
        assert_eq!(fs::read(primary.join("change.txt")).unwrap(), b"before");
        assert_eq!(fs::read(primary.join("keep.txt")).unwrap(), b"untouched");
    }

    #[test]
    fn stage2_recovery_rejects_tampered_backup_without_mutation() {
        let (_temp, primary, engine, candidate) = fixture();
        let workspace = ImplementerWorkspace::create(
            &primary,
            &engine,
            &candidate,
            &["change.txt".into(), "new.txt".into()],
        )
        .unwrap();
        fs::write(workspace.candidate_root().join("change.txt"), b"after").unwrap();
        fs::write(workspace.candidate_root().join("new.txt"), b"new").unwrap();
        let permit = approve_current_delta(&workspace);
        let outcome = workspace
            .promote_with_failure(&permit, Some(1), true, None)
            .unwrap();
        let PromotionOutcome::RecoveryRequired { journal, .. } = outcome else {
            panic!("expected retained recovery journal")
        };
        fs::write(journal.join("backup-0"), b"tampered backup").unwrap();
        let current = fs::read(primary.join("change.txt")).unwrap();

        assert!(recover_retained_promotion(&primary, &journal).is_err());
        assert_eq!(fs::read(primary.join("change.txt")).unwrap(), current);
        assert!(journal.exists(), "tampered recovery evidence must remain");
    }

    #[test]
    fn stage2_recovery_rejects_tampered_recovery_stage_without_mutation() {
        let (_temp, primary, engine, candidate) = fixture();
        let workspace = ImplementerWorkspace::create(
            &primary,
            &engine,
            &candidate,
            &["change.txt".into(), "new.txt".into()],
        )
        .unwrap();
        fs::write(workspace.candidate_root().join("change.txt"), b"after").unwrap();
        fs::write(workspace.candidate_root().join("new.txt"), b"new").unwrap();
        let permit = approve_current_delta(&workspace);
        let outcome = workspace
            .promote_with_failure(&permit, Some(1), true, None)
            .unwrap();
        let PromotionOutcome::RecoveryRequired { journal, .. } = outcome else {
            panic!("expected retained recovery journal")
        };
        fs::write(journal.join("recovery-staged-0"), b"corrupt recovery stage").unwrap();
        let current = fs::read(primary.join("change.txt")).unwrap();

        assert!(recover_retained_promotion(&primary, &journal).is_err());
        assert_eq!(fs::read(primary.join("change.txt")).unwrap(), current);
        assert!(journal.exists());
    }

    #[test]
    fn stage2_recovery_never_deletes_preexisting_empty_parent_from_journal_claim() {
        let (_temp, primary, engine, candidate) = fixture();
        fs::create_dir(primary.join("preexisting-empty")).unwrap();
        let workspace = ImplementerWorkspace::create(
            &primary,
            &engine,
            &candidate,
            &["preexisting-empty/new.txt".into(), "zz-new.txt".into()],
        )
        .unwrap();
        fs::create_dir(workspace.candidate_root().join("preexisting-empty")).unwrap();
        fs::write(
            workspace.candidate_root().join("preexisting-empty/new.txt"),
            b"new",
        )
        .unwrap();
        fs::write(workspace.candidate_root().join("zz-new.txt"), b"unapplied").unwrap();
        let permit = approve_current_delta(&workspace);
        let outcome = workspace
            .promote_with_failure(&permit, Some(1), true, None)
            .unwrap();
        let PromotionOutcome::RecoveryRequired { journal, .. } = outcome else {
            panic!("expected retained recovery journal")
        };
        for snapshot in [journal.join("journal.json")] {
            let mut data: PromotionJournal =
                serde_json::from_slice(&fs::read(&snapshot).unwrap()).unwrap();
            data.entries[0].created_parent_dirs = vec!["preexisting-empty".into()];
            fs::write(&snapshot, serde_json::to_vec(&data).unwrap()).unwrap();
        }
        let update = fs::read_dir(&journal)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("journal-update-")
            })
            .unwrap();
        let mut data: PromotionJournal =
            serde_json::from_slice(&fs::read(&update).unwrap()).unwrap();
        data.entries[0].created_parent_dirs = vec!["preexisting-empty".into()];
        fs::write(&update, serde_json::to_vec(&data).unwrap()).unwrap();

        recover_retained_promotion(&primary, &journal).unwrap();
        assert!(primary.join("preexisting-empty").is_dir());
        assert!(!primary.join("preexisting-empty/new.txt").exists());
    }

    #[test]
    fn stage2_recovery_rejects_inconsistent_snapshot_without_mutation() {
        let (_temp, primary, engine, candidate) = fixture();
        let workspace = ImplementerWorkspace::create(
            &primary,
            &engine,
            &candidate,
            &["change.txt".into(), "new.txt".into()],
        )
        .unwrap();
        fs::write(workspace.candidate_root().join("change.txt"), b"after").unwrap();
        fs::write(workspace.candidate_root().join("new.txt"), b"new").unwrap();
        let permit = approve_current_delta(&workspace);
        let outcome = workspace
            .promote_with_failure(&permit, Some(1), true, None)
            .unwrap();
        let PromotionOutcome::RecoveryRequired { journal, .. } = outcome else {
            panic!("expected retained recovery journal")
        };
        let snapshot = fs::read_dir(&journal)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("journal-update-")
            })
            .unwrap();
        let mut data: PromotionJournal =
            serde_json::from_slice(&fs::read(&snapshot).unwrap()).unwrap();
        data.mutation_intents = vec!["new.txt".into()];
        fs::write(&snapshot, serde_json::to_vec(&data).unwrap()).unwrap();
        let current = fs::read(primary.join("change.txt")).unwrap();

        assert!(recover_retained_promotion(&primary, &journal).is_err());
        assert_eq!(fs::read(primary.join("change.txt")).unwrap(), current);
        assert!(
            journal.exists(),
            "inconsistent recovery evidence must remain"
        );
    }

    #[test]
    fn stage2_recovery_is_readonly_safe_after_journal_was_removed() {
        let (_temp, primary, engine, candidate) = fixture();
        let workspace = ImplementerWorkspace::create(
            &primary,
            &engine,
            &candidate,
            &["change.txt".into(), "new.txt".into()],
        )
        .unwrap();
        fs::write(workspace.candidate_root().join("change.txt"), b"after").unwrap();
        fs::write(workspace.candidate_root().join("new.txt"), b"new").unwrap();
        let permit = approve_current_delta(&workspace);
        let outcome = workspace
            .promote_with_failure(&permit, Some(1), true, None)
            .unwrap();
        let PromotionOutcome::RecoveryRequired { journal, .. } = outcome else {
            panic!("expected retained recovery journal")
        };
        recover_retained_promotion(&primary, &journal).unwrap();
        assert!(recover_retained_promotion(&primary, &journal).is_err());
        assert_eq!(fs::read(primary.join("change.txt")).unwrap(), b"before");
    }

    #[test]
    fn stage2_recovery_restores_readonly_metadata() {
        let (_temp, primary, engine, candidate) = fixture();
        let original = primary.join("change.txt");
        let mut permissions = fs::metadata(&original).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&original, permissions).unwrap();
        let workspace = ImplementerWorkspace::create(
            &primary,
            &engine,
            &candidate,
            &["change.txt".into(), "new.txt".into()],
        )
        .unwrap();
        let candidate_file = workspace.candidate_root().join("change.txt");
        let mut candidate_permissions = fs::metadata(&candidate_file).unwrap().permissions();
        candidate_permissions.set_readonly(false);
        fs::set_permissions(&candidate_file, candidate_permissions).unwrap();
        fs::write(&candidate_file, b"after").unwrap();
        fs::write(workspace.candidate_root().join("new.txt"), b"new").unwrap();
        let permit = approve_current_delta(&workspace);
        let outcome = workspace
            .promote_with_failure(&permit, Some(1), true, None)
            .unwrap();
        let PromotionOutcome::RecoveryRequired { journal, .. } = outcome else {
            panic!("expected retained recovery journal")
        };

        recover_retained_promotion(&primary, &journal).unwrap();
        assert_eq!(fs::read(&original).unwrap(), b"before");
        assert!(fs::metadata(&original).unwrap().permissions().readonly());
    }

    #[test]
    fn stage2_recovery_rejects_unsupported_journal_version_before_mutation() {
        let (_temp, primary, engine, candidate) = fixture();
        let workspace = ImplementerWorkspace::create(
            &primary,
            &engine,
            &candidate,
            &["change.txt".into(), "new.txt".into()],
        )
        .unwrap();
        fs::write(workspace.candidate_root().join("change.txt"), b"after").unwrap();
        fs::write(workspace.candidate_root().join("new.txt"), b"new").unwrap();
        let permit = approve_current_delta(&workspace);
        let outcome = workspace
            .promote_with_failure(&permit, Some(1), true, None)
            .unwrap();
        let PromotionOutcome::RecoveryRequired { journal, .. } = outcome else {
            panic!("expected retained recovery journal")
        };
        let mut data: PromotionJournal =
            serde_json::from_slice(&fs::read(journal.join("journal.json")).unwrap()).unwrap();
        data.version = 99;
        fs::write(
            journal.join("journal.json"),
            serde_json::to_vec(&data).unwrap(),
        )
        .unwrap();
        let current = fs::read(primary.join("change.txt")).unwrap();

        assert!(recover_retained_promotion(&primary, &journal).is_err());
        assert_eq!(fs::read(primary.join("change.txt")).unwrap(), current);
        assert!(journal.exists());
    }

    #[cfg(unix)]
    #[test]
    fn stage2_recovery_rejects_journal_symlink_alias() {
        use std::os::unix::fs::symlink;

        let (temp, primary, engine, candidate) = fixture();
        let workspace = ImplementerWorkspace::create(
            &primary,
            &engine,
            &candidate,
            &["change.txt".into(), "new.txt".into()],
        )
        .unwrap();
        fs::write(workspace.candidate_root().join("change.txt"), b"after").unwrap();
        fs::write(workspace.candidate_root().join("new.txt"), b"new").unwrap();
        let permit = approve_current_delta(&workspace);
        let outcome = workspace
            .promote_with_failure(&permit, Some(1), true, None)
            .unwrap();
        let PromotionOutcome::RecoveryRequired { journal, .. } = outcome else {
            panic!("expected retained recovery journal")
        };
        let alias = temp.path().join("journal-alias");
        symlink(&journal, &alias).unwrap();

        assert!(recover_retained_promotion(&primary, &alias).is_err());
        assert!(journal.exists());
        assert_eq!(fs::read(primary.join("change.txt")).unwrap(), b"after");
    }

    #[cfg(windows)]
    #[test]
    fn stage2_recovery_rejects_journal_junction_alias() {
        let (temp, primary, engine, candidate) = fixture();
        let workspace = ImplementerWorkspace::create(
            &primary,
            &engine,
            &candidate,
            &["change.txt".into(), "new.txt".into()],
        )
        .unwrap();
        fs::write(workspace.candidate_root().join("change.txt"), b"after").unwrap();
        fs::write(workspace.candidate_root().join("new.txt"), b"new").unwrap();
        let permit = approve_current_delta(&workspace);
        let outcome = workspace
            .promote_with_failure(&permit, Some(1), true, None)
            .unwrap();
        let PromotionOutcome::RecoveryRequired { journal, .. } = outcome else {
            panic!("expected retained recovery journal")
        };
        let alias = temp.path().join("journal-alias");
        let output = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                alias.to_str().unwrap(),
                journal.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success() && alias.exists(),
            "junction setup failed"
        );

        assert!(recover_retained_promotion(&primary, &alias).is_err());
        assert!(journal.exists());
        assert_eq!(fs::read(primary.join("change.txt")).unwrap(), b"after");
    }
}
