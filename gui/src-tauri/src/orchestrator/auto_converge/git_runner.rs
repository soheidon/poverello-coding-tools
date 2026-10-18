use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use super::guards::{
    seal_concrete_release_target_and_execution_binding, verify_release_execution_binding,
    verify_run_start_permission_binding,
};
use super::types::{
    sha256_hex, AutoConvergeRunState, AutoConvergeState, BaselineFileEntry, BaselineFileStatus,
    BlockedContext, BlockedKind, RunDeltaEntry, PERM_COMMIT, PERM_PUSH_BRANCH, PERM_PUSH_TAG,
    PERM_RELEASE_PUBLISH, PERM_STAGE, PERM_TAG, TAG_OR_RELEASE_MASK,
};

pub const PREFERRED_GIT_AUTHOR: &str = "soheidon <soheidon@yahoo.co.jp>";
pub const FORBIDDEN_AI_ATTRIBUTIONS: &[&str] = &[
    "Co-Authored-By",
    "Claude",
    "Anthropic",
    "Gemini",
    "DeepSeek",
    "OpenAI",
    "Codex",
    "Antigravity",
    "SoheiUFukui",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandInvocation {
    pub step: String,
    pub program: String,
    pub args: Vec<String>,
}

pub trait CommandRunner: Send + Sync {
    fn has_preexisting_staged_work(&self) -> Result<bool, String>;
    fn run_step(&self, step: &str, program: &str, args: &[String]) -> Result<String, String>;
    fn committed_head_sha(&self) -> Result<String, String>;
    fn committed_changed_files(&self) -> Result<Vec<String>, String>;
    fn committed_delta_manifest(
        &self,
        runtime_allowlist: &[String],
        baseline_manifest: &[BaselineFileEntry],
    ) -> Result<Vec<RunDeltaEntry>, String>;
}

/// Deterministic mock runner for unit/integration tests (§12).
/// Zero live git or network mutations are ever performed.
#[derive(Debug, Clone)]
pub struct MockCommandRunner {
    pub preexisting_staged_work: Arc<Mutex<bool>>,
    pub invocations: Arc<Mutex<Vec<CommandInvocation>>>,
    pub fail_on_step: Arc<Mutex<Option<String>>>,
    pub head_commit_sha: String,
    pub committed_files_override: Option<Vec<String>>,
    pub committed_delta_override: Option<Vec<RunDeltaEntry>>,
    pub working_tree_delta: Vec<RunDeltaEntry>,
}

impl Default for MockCommandRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl MockCommandRunner {
    pub fn new() -> Self {
        Self {
            preexisting_staged_work: Arc::new(Mutex::new(false)),
            invocations: Arc::new(Mutex::new(Vec::new())),
            fail_on_step: Arc::new(Mutex::new(None)),
            head_commit_sha: "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678".to_string(),
            committed_files_override: None,
            committed_delta_override: None,
            working_tree_delta: Vec::new(),
        }
    }

    pub fn recorded_steps(&self) -> Vec<String> {
        self.invocations
            .lock()
            .unwrap()
            .iter()
            .map(|i| i.step.clone())
            .collect()
    }

    pub fn recorded_invocations(&self) -> Vec<CommandInvocation> {
        self.invocations.lock().unwrap().clone()
    }
}

impl CommandRunner for MockCommandRunner {
    fn has_preexisting_staged_work(&self) -> Result<bool, String> {
        Ok(*self.preexisting_staged_work.lock().unwrap())
    }

    fn run_step(&self, step: &str, program: &str, args: &[String]) -> Result<String, String> {
        if let Some(ref fail_step) = *self.fail_on_step.lock().unwrap() {
            if fail_step == step {
                return Err(format!("Injected mock failure at step '{step}'"));
            }
        }
        self.invocations.lock().unwrap().push(CommandInvocation {
            step: step.to_string(),
            program: program.to_string(),
            args: args.to_vec(),
        });
        Ok(format!("mock ok: {step}"))
    }

    fn committed_head_sha(&self) -> Result<String, String> {
        Ok(self.head_commit_sha.clone())
    }

    fn committed_changed_files(&self) -> Result<Vec<String>, String> {
        if let Some(ref files) = self.committed_files_override {
            return Ok(files.clone());
        }
        Ok(self
            .working_tree_delta
            .iter()
            .map(|d| d.path.clone())
            .collect())
    }

    fn committed_delta_manifest(
        &self,
        _runtime_allowlist: &[String],
        _baseline_manifest: &[BaselineFileEntry],
    ) -> Result<Vec<RunDeltaEntry>, String> {
        if let Some(ref delta) = self.committed_delta_override {
            return Ok(delta.clone());
        }
        Ok(self.working_tree_delta.clone())
    }
}

/// Production command runner for live Git environment verification and operations.
#[derive(Debug, Clone)]
pub struct ProductionGitCommandRunner {
    pub workspace_root: std::path::PathBuf,
}

impl ProductionGitCommandRunner {
    pub fn new(workspace_root: std::path::PathBuf) -> Self {
        Self { workspace_root }
    }
}

impl CommandRunner for ProductionGitCommandRunner {
    fn has_preexisting_staged_work(&self) -> Result<bool, String> {
        let output = std::process::Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(&self.workspace_root)
            .status()
            .map_err(|e| format!("git diff --cached failed: {e}"))?;
        Ok(output.code() == Some(1))
    }

    fn run_step(&self, step: &str, program: &str, args: &[String]) -> Result<String, String> {
        let output = std::process::Command::new(program)
            .args(args)
            .current_dir(&self.workspace_root)
            .output()
            .map_err(|e| format!("Failed executing step '{step}': {e}"))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(format!(
                "Step '{step}' failed with exit code {}: {}",
                output.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    fn committed_head_sha(&self) -> Result<String, String> {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.workspace_root)
            .output()
            .map_err(|e| format!("git rev-parse HEAD failed: {e}"))?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn committed_changed_files(&self) -> Result<Vec<String>, String> {
        let output = std::process::Command::new("git")
            .args(["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"])
            .current_dir(&self.workspace_root)
            .output()
            .map_err(|e| format!("git diff-tree failed: {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    }

    fn committed_delta_manifest(
        &self,
        runtime_allowlist: &[String],
        baseline_manifest: &[BaselineFileEntry],
    ) -> Result<Vec<RunDeltaEntry>, String> {
        compute_run_delta_manifest(&self.workspace_root, runtime_allowlist, baseline_manifest)
    }
}

/// Returns true if a repository-relative path is excluded from `baseline_file_manifest` (§2.4):
/// `.plan/`, `.poverello/runs/`, `.git/`.
pub fn is_excluded_from_baseline(rel_path: &str) -> bool {
    let norm = rel_path.trim().replace('\\', "/");
    let norm = norm.strip_prefix("./").unwrap_or(&norm);
    norm == ".plan"
        || norm.starts_with(".plan/")
        || norm == ".poverello/runs"
        || norm.starts_with(".poverello/runs/")
        || norm == ".git"
        || norm.starts_with(".git/")
}

/// Captures the per-file `baseline_file_manifest` across `workspace_root`, excluding
/// `.plan/`, `.poverello/runs/`, and `.git/` (§2.4).
pub fn capture_baseline_file_manifest(
    workspace_root: &Path,
    status_overrides: &BTreeMap<String, BaselineFileStatus>,
) -> Result<Vec<BaselineFileEntry>, String> {
    let mut entries = Vec::new();
    collect_baseline_files(
        workspace_root,
        workspace_root,
        status_overrides,
        &mut entries,
    )?;
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

fn collect_baseline_files(
    root: &Path,
    current: &Path,
    status_overrides: &BTreeMap<String, BaselineFileStatus>,
    out: &mut Vec<BaselineFileEntry>,
) -> Result<(), String> {
    if !current.exists() {
        return Ok(());
    }
    let read_dir = std::fs::read_dir(current).map_err(|e| {
        format!(
            "Failed reading workspace directory {}: {e}",
            current.display()
        )
    })?;
    for entry in read_dir {
        let entry = entry.map_err(|e| format!("Directory entry error: {e}"))?;
        let full_path = entry.path();
        let rel = full_path
            .strip_prefix(root)
            .map_err(|e| format!("Strip prefix error: {e}"))?
            .to_string_lossy()
            .replace('\\', "/");

        if is_excluded_from_baseline(&rel) {
            continue;
        }

        let meta = std::fs::symlink_metadata(&full_path)
            .map_err(|e| format!("Metadata error at {}: {e}", full_path.display()))?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            collect_baseline_files(root, &full_path, status_overrides, out)?;
        } else if meta.is_file() {
            let bytes = std::fs::read(&full_path)
                .map_err(|e| format!("Failed reading {}: {e}", full_path.display()))?;
            let content_sha256 = sha256_hex(&bytes);
            let file_mode = if meta.permissions().readonly() {
                "100444".to_string()
            } else {
                "100644".to_string()
            };
            let status = status_overrides
                .get(&rel)
                .copied()
                .unwrap_or(BaselineFileStatus::Clean);
            out.push(BaselineFileEntry {
                path: rel,
                status,
                content_sha256,
                file_mode,
            });
        }
    }
    Ok(())
}

/// Computes `baseline_workspace_digest = SHA256(canonical_json(baseline_file_manifest))` (§2.4).
pub fn compute_baseline_workspace_digest(manifest: &[BaselineFileEntry]) -> String {
    let mut sorted = manifest.to_vec();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let json = serde_json::to_string(&sorted).unwrap_or_default();
    sha256_hex(json.as_bytes())
}

/// Computes `baseline_dirty_hash` over all non-clean files in `baseline_file_manifest` (Invariant 10).
pub fn compute_baseline_dirty_hash(manifest: &[BaselineFileEntry]) -> String {
    let mut dirty: Vec<&BaselineFileEntry> = manifest
        .iter()
        .filter(|e| e.status != BaselineFileStatus::Clean)
        .collect();
    dirty.sort_by(|a, b| a.path.cmp(&b.path));
    let json = serde_json::to_string(&dirty).unwrap_or_default();
    sha256_hex(json.as_bytes())
}

/// Computes the canonical `RunDeltaManifest` over `runtime_allowlist` against `baseline_file_manifest` (§3.2).
/// Unrelated dirty files outside `runtime_allowlist` are excluded, and `current_status` is normalized
/// so pre-commit working-tree state and post-commit `HEAD` tree state yield identical digests.
pub fn compute_run_delta_manifest(
    workspace_root: &Path,
    runtime_allowlist: &[String],
    baseline_manifest: &[BaselineFileEntry],
) -> Result<Vec<RunDeltaEntry>, String> {
    let baseline_map: BTreeMap<&str, &BaselineFileEntry> = baseline_manifest
        .iter()
        .map(|e| (e.path.as_str(), e))
        .collect();

    let mut sorted_allowlist: Vec<String> = runtime_allowlist.to_vec();
    sorted_allowlist.sort();
    sorted_allowlist.dedup();

    let mut delta = Vec::with_capacity(sorted_allowlist.len());
    for rel_path in sorted_allowlist {
        let full_path = workspace_root.join(&rel_path);
        let base_entry = baseline_map.get(rel_path.as_str()).copied();
        let (current_status, current_sha256, current_mode) = if full_path.is_file() {
            let bytes = std::fs::read(&full_path)
                .map_err(|e| format!("Failed reading allowlisted file {rel_path}: {e}"))?;
            let sha = sha256_hex(&bytes);
            let meta = std::fs::metadata(&full_path)
                .map_err(|e| format!("Failed metadata for {rel_path}: {e}"))?;
            let mode = if meta.permissions().readonly() {
                "100444".to_string()
            } else {
                "100644".to_string()
            };
            let status = match base_entry {
                Some(b) if b.content_sha256 == sha => BaselineFileStatus::Clean,
                Some(_) => BaselineFileStatus::Modified,
                None => BaselineFileStatus::Modified,
            };
            (status, sha, mode)
        } else {
            (
                BaselineFileStatus::Modified,
                "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                "000000".to_string(),
            )
        };

        delta.push(RunDeltaEntry {
            path: rel_path,
            baseline_status: base_entry.map(|b| b.status),
            baseline_sha256: base_entry.map(|b| b.content_sha256.clone()),
            current_status,
            current_sha256,
            current_mode,
        });
    }

    Ok(delta)
}

/// Computes `diff_digest = SHA256(canonical_json(RunDeltaManifest))` (§3.2).
pub fn compute_diff_digest(delta: &[RunDeltaEntry]) -> String {
    let mut sorted = delta.to_vec();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let json = serde_json::to_string(&sorted).unwrap_or_default();
    sha256_hex(json.as_bytes())
}

/// Sanitizes commit messages to enforce preferred Git identity and forbid AI attribution trailers.
pub fn validate_and_sanitize_commit_message(message: &str) -> Result<String, String> {
    for forbidden in FORBIDDEN_AI_ATTRIBUTIONS {
        if message
            .to_ascii_lowercase()
            .contains(&forbidden.to_ascii_lowercase())
        {
            return Err(format!(
                "Commit message rejected: contains forbidden attribution or trailer '{forbidden}'"
            ));
        }
    }
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Err("Commit message cannot be empty".to_string());
    }
    Ok(trimmed.to_string())
}

/// Executes authorized Git and Release actions in strict linear succession (§9.2, §9.3)
/// with stop-on-failure guarantees and Two-Phase Release Authorization verification.
pub fn execute_authorized_release_actions(
    run_state: &mut AutoConvergeRunState,
    commit_message: &str,
    runner: &dyn CommandRunner,
) -> Result<(), String> {
    if run_state.current_state != AutoConvergeState::ReleaseActionsPending {
        return Err(format!(
            "Cannot execute release actions in state {:?}",
            run_state.current_state
        ));
    }

    let eff = run_state.effective_permissions;
    if eff == 0 {
        halt_blocked_fatal(
            run_state,
            "Invariant 14: Attempted git/release execution with zero effective permissions",
        );
        return Err("Zero effective permissions".to_string());
    }

    // Verify Phase 1 RunStartPermissionBinding before any local/branch action
    if let Err(e) = verify_run_start_permission_binding(run_state) {
        halt_blocked_fatal(
            run_state,
            &format!("RunStartPermissionBinding invalid: {e}"),
        );
        return Err(e);
    }

    // Step 1: Pre-flight staged work re-check immediately before `stage` / `commit` (Invariant 10 / §9.3)
    if (eff & PERM_COMMIT) != 0 && runner.has_preexisting_staged_work()? {
        run_state.blocked_context = Some(BlockedContext {
            kind: BlockedKind::UserConfirmationRequired,
            pre_blocked_state: run_state.current_state,
            reason: "Invariant 10: Pre-existing staged work detected in .git/index before stage/commit; index must be clean".to_string(),
        });
        run_state.current_state = AutoConvergeState::Blocked;
        run_state.is_terminal = true;
        return Err("Pre-existing staged work detected in .git/index".to_string());
    }

    // Step 2: `stage`
    if (eff & PERM_STAGE) != 0 {
        if run_state.runtime_allowlist.is_empty() {
            halt_blocked_fatal(run_state, "Cannot stage with empty runtime_allowlist");
            return Err("Empty runtime_allowlist".to_string());
        }
        let mut args = vec!["add".to_string(), "--".to_string()];
        args.extend(run_state.runtime_allowlist.iter().cloned());
        if let Err(e) = runner.run_step("stage", "git", &args) {
            halt_blocked_fatal(run_state, &format!("Stage failed: {e}"));
            return Err(e);
        }
        run_state
            .completed_release_actions
            .push("stage".to_string());
    }

    // Step 3: `commit` + Post-Commit RunDeltaManifest Verification + Phase-2 ConcreteReleaseTarget Sealing
    if (eff & PERM_COMMIT) != 0 {
        if (eff & PERM_STAGE) == 0 {
            halt_blocked_fatal(run_state, "commit requires stage permission");
            return Err("commit requires stage permission".to_string());
        }
        let clean_msg = match validate_and_sanitize_commit_message(commit_message) {
            Ok(m) => m,
            Err(e) => {
                halt_blocked_fatal(run_state, &e);
                return Err(e);
            }
        };
        let args = vec![
            "-c".to_string(),
            "user.name=soheidon".to_string(),
            "-c".to_string(),
            "user.email=soheidon@yahoo.co.jp".to_string(),
            "commit".to_string(),
            "--author".to_string(),
            PREFERRED_GIT_AUTHOR.to_string(),
            "-m".to_string(),
            clean_msg,
        ];
        if let Err(e) = runner.run_step("commit", "git", &args) {
            halt_blocked_fatal(run_state, &format!("Commit failed: {e}"));
            return Err(e);
        }
        run_state
            .completed_release_actions
            .push("commit".to_string());

        // §9.3 Step 3(a): Verify all committed files in HEAD are strictly within runtime_allowlist
        let allow_set: BTreeSet<&str> = run_state
            .runtime_allowlist
            .iter()
            .map(|s| s.as_str())
            .collect();
        let committed_files = runner.committed_changed_files()?;
        for file in &committed_files {
            if !allow_set.contains(file.as_str()) {
                let msg = format!(
                    "Post-commit scope violation: committed file '{file}' is outside runtime_allowlist"
                );
                halt_blocked_fatal(run_state, &msg);
                return Err(msg);
            }
        }

        // §9.3 Step 3(b): Verify HEAD RunDeltaManifest matches approved diff_digest byte-for-byte
        let expected_diff_digest = match run_state.current_diff_digest.clone() {
            Some(d) => d,
            None => {
                halt_blocked_fatal(
                    run_state,
                    "Missing current_diff_digest at post-commit verification",
                );
                return Err("Missing current_diff_digest".to_string());
            }
        };
        let head_delta = runner.committed_delta_manifest(
            &run_state.runtime_allowlist,
            &run_state.workspace.baseline_file_manifest,
        )?;
        let head_diff_digest = compute_diff_digest(&head_delta);
        if head_diff_digest != expected_diff_digest {
            let msg = format!(
                "Post-commit RunDeltaManifest mismatch: HEAD digest {head_diff_digest} != approved {expected_diff_digest}"
            );
            halt_blocked_fatal(run_state, &msg);
            return Err(msg);
        }

        let head_sha = runner.committed_head_sha()?;
        run_state.workspace.git_head_commit = head_sha.clone();

        // Seal ConcreteReleaseTarget & ReleaseExecutionPermissionBinding if any tag/release action is requested
        if (eff & TAG_OR_RELEASE_MASK) != 0 {
            if let Err(e) = seal_concrete_release_target_and_execution_binding(run_state, &head_sha)
            {
                halt_blocked_fatal(
                    run_state,
                    &format!("Failed sealing ConcreteReleaseTarget: {e}"),
                );
                return Err(e);
            }
        }
    }

    // Step 4: `push_branch`
    if (eff & PERM_PUSH_BRANCH) != 0 {
        if (eff & PERM_COMMIT) == 0 {
            halt_blocked_fatal(run_state, "push_branch requires commit permission");
            return Err("push_branch requires commit permission".to_string());
        }
        let args = vec![
            "push".to_string(),
            run_state.workspace.repo_remote_url.clone(),
            run_state.workspace.branch_name.clone(),
        ];
        if let Err(e) = runner.run_step("push_branch", "git", &args) {
            halt_blocked_fatal(run_state, &format!("push_branch failed: {e}"));
            return Err(e);
        }
        run_state
            .completed_release_actions
            .push("push_branch".to_string());
    }

    // Steps 5, 6, 7: `tag`, `push_tag`, `release_publish` (require Phase 2 ConcreteReleaseTarget & ReleaseExecutionPermissionBinding)
    if (eff & TAG_OR_RELEASE_MASK) != 0 {
        if let Err(e) = verify_release_execution_binding(run_state) {
            halt_blocked_fatal(
                run_state,
                &format!("ReleaseExecutionPermissionBinding verification failed: {e}"),
            );
            return Err(e);
        }
    }

    // Step 5: `tag`
    if (eff & PERM_TAG) != 0 {
        let target = run_state
            .release_authorization
            .concrete_release_target
            .as_ref()
            .ok_or_else(|| "Missing ConcreteReleaseTarget for tag".to_string())?
            .clone();
        let args = vec![
            "-c".to_string(),
            "user.name=soheidon".to_string(),
            "-c".to_string(),
            "user.email=soheidon@yahoo.co.jp".to_string(),
            "tag".to_string(),
            "-a".to_string(),
            target.version_tag.clone(),
            target.target_commit_sha.clone(),
            "-m".to_string(),
            format!("Release {}", target.version_tag),
        ];
        if let Err(e) = runner.run_step("tag", "git", &args) {
            halt_blocked_fatal(run_state, &format!("tag failed: {e}"));
            return Err(e);
        }
        run_state.completed_release_actions.push("tag".to_string());
    }

    // Step 6: `push_tag`
    if (eff & PERM_PUSH_TAG) != 0 {
        if (eff & PERM_TAG) == 0 {
            halt_blocked_fatal(run_state, "push_tag requires tag permission");
            return Err("push_tag requires tag permission".to_string());
        }
        let target = run_state
            .release_authorization
            .concrete_release_target
            .as_ref()
            .ok_or_else(|| "Missing ConcreteReleaseTarget for push_tag".to_string())?
            .clone();
        let args = vec![
            "push".to_string(),
            run_state.workspace.repo_remote_url.clone(),
            target.version_tag,
        ];
        if let Err(e) = runner.run_step("push_tag", "git", &args) {
            halt_blocked_fatal(run_state, &format!("push_tag failed: {e}"));
            return Err(e);
        }
        run_state
            .completed_release_actions
            .push("push_tag".to_string());
    }

    // Step 7: `release_publish`
    if (eff & PERM_RELEASE_PUBLISH) != 0 {
        if (eff & PERM_PUSH_TAG) == 0 {
            halt_blocked_fatal(run_state, "release_publish requires push_tag permission");
            return Err("release_publish requires push_tag permission".to_string());
        }
        let target = run_state
            .release_authorization
            .concrete_release_target
            .as_ref()
            .ok_or_else(|| "Missing ConcreteReleaseTarget for release_publish".to_string())?
            .clone();
        let args = vec![
            "release".to_string(),
            "create".to_string(),
            target.version_tag.clone(),
            "--repo".to_string(),
            target.release_repository.clone(),
            "--target".to_string(),
            target.target_commit_sha.clone(),
        ];
        if let Err(e) = runner.run_step("release_publish", "gh", &args) {
            halt_blocked_fatal(run_state, &format!("release_publish failed: {e}"));
            return Err(e);
        }
        run_state
            .completed_release_actions
            .push("release_publish".to_string());
    }

    run_state.current_state = AutoConvergeState::ReleaseCompleted;
    run_state.is_terminal = true;
    Ok(())
}

fn halt_blocked_fatal(run_state: &mut AutoConvergeRunState, reason: &str) {
    run_state.blocked_context = Some(BlockedContext {
        kind: BlockedKind::FatalInvariant,
        pre_blocked_state: run_state.current_state,
        reason: reason.to_string(),
    });
    run_state.current_state = AutoConvergeState::Blocked;
    run_state.is_terminal = true;
}
