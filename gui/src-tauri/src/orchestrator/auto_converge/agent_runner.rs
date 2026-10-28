use super::isolation::{AutoConvergeRole, PlatformIsolationGuard};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct RoleExecutionRequest {
    pub role: AutoConvergeRole,
    pub session_id: String,
    pub workspace_root: PathBuf,
    pub snapshot_dir: Option<PathBuf>,
    pub plan_path: PathBuf,
    pub runtime_allowlist: Vec<String>,
    pub input_prompt: String,
    pub timeout_seconds: u64,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub run_id: Option<String>,
}

impl RoleExecutionRequest {
    pub fn new(
        role: AutoConvergeRole,
        session_id: String,
        workspace_root: PathBuf,
        snapshot_dir: Option<PathBuf>,
        plan_path: PathBuf,
        runtime_allowlist: Vec<String>,
        input_prompt: String,
        timeout_seconds: u64,
    ) -> Self {
        Self {
            role,
            session_id,
            workspace_root,
            snapshot_dir,
            plan_path,
            runtime_allowlist,
            input_prompt,
            timeout_seconds,
            model: None,
            reasoning_effort: None,
            run_id: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RoleExecutionResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub verdict_json: Option<String>,
}

pub(crate) trait AgentCommandRunner: Send + Sync {
    fn execute_role_session(
        &self,
        request: &RoleExecutionRequest,
    ) -> Result<RoleExecutionResponse, String>;
}

use super::types::ReviewerLevel;

/// Determines whether OS DACL isolation guard and platform boundary preflight must be enforced (§209.3 A2, §209.15 B2, §211.4).
/// ReviewerLevel::R2 (default) strictly enforces fail-closed OS DACL boundary.
/// ReviewerLevel::R0D skips OS DACL boundary guard (detection-only mode with explicit consent).
pub fn requires_reviewer_os_boundary_guard(role: AutoConvergeRole, level: ReviewerLevel) -> bool {
    match (role, level) {
        (AutoConvergeRole::Reviewer, ReviewerLevel::R2) => true,
        (AutoConvergeRole::Reviewer, ReviewerLevel::R0D) => false,
        _ => false,
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewerDispatchStage {
    Initial,
    OsBoundaryChecked,
    OsBoundarySkipped,
    LaunchConfigBuilt,
    PreSpawnChecked,
    ProcessSpawnAttempted,
}

#[cfg(test)]
thread_local! {
    static TEST_REVIEWER_DISPATCH_STAGE: std::cell::Cell<Option<ReviewerDispatchStage>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub fn get_test_last_reviewer_dispatch_stage() -> Option<ReviewerDispatchStage> {
    TEST_REVIEWER_DISPATCH_STAGE.with(|c| c.get())
}

#[cfg(test)]
pub fn set_test_reviewer_dispatch_stage(stage: Option<ReviewerDispatchStage>) {
    TEST_REVIEWER_DISPATCH_STAGE.with(|c| c.set(stage));
}

/// Production runner launching real CLI subprocesses within role boundaries.
#[derive(Debug, Clone)]
pub(crate) struct ProductionAgentRunner {
    pub cli_path: Option<PathBuf>,
    pub reviewer_level: ReviewerLevel,
    #[cfg(test)]
    pub explicit_codex_home: Option<PathBuf>,
    #[cfg(test)]
    pub test_run_root: Option<PathBuf>,
}

impl Default for ProductionAgentRunner {
    fn default() -> Self {
        Self::new()
    }
}

pub const MAX_CAPTURE_BYTES: usize = 1024 * 1024; // 1 MiB cap (§22.4.4)

impl ProductionAgentRunner {
    pub fn new() -> Self {
        Self {
            cli_path: None,
            reviewer_level: ReviewerLevel::R2,
            #[cfg(test)]
            explicit_codex_home: None,
            #[cfg(test)]
            test_run_root: None,
        }
    }

    pub fn with_cli(cli_path: PathBuf) -> Self {
        Self {
            cli_path: Some(cli_path),
            reviewer_level: ReviewerLevel::R2,
            #[cfg(test)]
            explicit_codex_home: None,
            #[cfg(test)]
            test_run_root: None,
        }
    }

    pub fn with_reviewer_level(mut self, level: ReviewerLevel) -> Self {
        self.reviewer_level = level;
        self
    }

    pub fn try_with_reviewer_level_str(mut self, level: &str) -> Result<Self, String> {
        self.reviewer_level = ReviewerLevel::parse(level)?;
        Ok(self)
    }

    #[cfg(test)]
    pub fn with_cli_and_explicit_home(cli_path: PathBuf, home: PathBuf) -> Self {
        Self {
            cli_path: Some(cli_path),
            reviewer_level: ReviewerLevel::R2,
            explicit_codex_home: Some(home),
            test_run_root: None,
        }
    }

    #[cfg(test)]
    pub fn with_cli_explicit_home_and_run_root(
        cli_path: PathBuf,
        home: PathBuf,
        run_root: PathBuf,
    ) -> Self {
        Self {
            cli_path: Some(cli_path),
            reviewer_level: ReviewerLevel::R2,
            explicit_codex_home: Some(home),
            test_run_root: Some(run_root),
        }
    }
}

impl AgentCommandRunner for ProductionAgentRunner {
    fn execute_role_session(
        &self,
        request: &RoleExecutionRequest,
    ) -> Result<RoleExecutionResponse, String> {
        // Safe process invocation: strictly resolve configured CLI or fail closed immediately.
        // Never fall back to shell or a guard script! (§22.1.2)
        let cli = self
            .cli_path
            .clone()
            .or_else(|| std::env::var("POVERELLO_AGENT_CLI").ok().map(PathBuf::from))
            .ok_or_else(|| {
                "No configured agent CLI available: failing closed without spawning any process"
                    .to_string()
            })?;

        if !cli.exists() {
            return Err(format!(
                "Configured agent CLI not found at: {}",
                cli.display()
            ));
        }

        #[cfg(test)]
        set_test_reviewer_dispatch_stage(Some(ReviewerDispatchStage::Initial));

        // Enforce Reviewer isolation preflight and OS DACL boundary before spawning child (§209.3 A2, §209.15 B2, §211.4)
        // ReviewerLevel::R2 (default): strictly enforces OS DACL boundary and ReviewerIsolationGuard (fail-closed)
        // ReviewerLevel::R0D: skips OS DACL boundary guard (detection-only; requires explicit consent & Phase C)
        let _reviewer_guard = if request.role == AutoConvergeRole::Reviewer {
            let snap = request.snapshot_dir.as_deref().ok_or_else(|| {
                "Reviewer role execution requires an isolated snapshot directory".to_string()
            })?;
            if requires_reviewer_os_boundary_guard(request.role, self.reviewer_level) {
                PlatformIsolationGuard::verify_reviewer_os_boundary(&request.workspace_root, snap)
                    .map_err(|e| format!("Reviewer isolation preflight failed: {e}"))?;
                let guard = super::isolation::ReviewerIsolationGuard::new(snap)
                    .map_err(|e| format!("Failed to apply Reviewer OS isolation: {e}"))?;
                #[cfg(test)]
                set_test_reviewer_dispatch_stage(Some(ReviewerDispatchStage::OsBoundaryChecked));
                Some(guard)
            } else {
                #[cfg(test)]
                set_test_reviewer_dispatch_stage(Some(ReviewerDispatchStage::OsBoundarySkipped));
                None
            }
        } else {
            None
        };

        let (exec_str, args, run_dir, payload_bytes, child_env, paths_for_verdict) =
            if request.role == AutoConvergeRole::Reviewer {
                #[cfg(not(test))]
                let launch_config =
                    super::codex_exec::build_reviewer_launch_configuration_with_executable(
                        request,
                        Some(&cli),
                    )?;
                #[cfg(test)]
            let launch_config =
                super::codex_exec::build_reviewer_launch_configuration_for_test_with_executable(
                    request,
                    self.explicit_codex_home.as_deref(),
                    Some(&cli),
                    self.test_run_root.as_deref(),
                    None,
                    None,
                )?;
                let args = launch_config.invocation.args;
                let payload_bytes = launch_config.invocation.stdin;
                let run_dir = launch_config.paths.working_dir.clone();
                let env = launch_config.environment;
                (
                    cli.to_string_lossy().to_string(),
                    args,
                    run_dir,
                    payload_bytes,
                    env,
                    Some(launch_config.paths),
                )
            } else if request.role == AutoConvergeRole::Implementer {
                #[cfg(not(test))]
                let launch_config =
                    super::codex_exec::build_implementer_launch_configuration_with_executable(
                        request,
                        Some(&cli),
                    )?;
                #[cfg(test)]
            let launch_config =
                super::codex_exec::build_implementer_launch_configuration_for_test_with_executable(
                    request,
                    self.explicit_codex_home.as_deref(),
                    Some(&cli),
                    self.test_run_root.as_deref(),
                    None,
                    None,
                )?;
                let args = launch_config.invocation.args;
                let payload_bytes = launch_config.invocation.stdin;
                let run_dir = launch_config.paths.working_dir.clone();
                let env = launch_config.environment;
                (
                    cli.to_string_lossy().to_string(),
                    args,
                    run_dir,
                    payload_bytes,
                    env,
                    Some(launch_config.paths),
                )
            } else {
                let args = vec![
                    "--role".to_string(),
                    format!("{:?}", request.role).to_lowercase(),
                    "--session".to_string(),
                    request.session_id.clone(),
                ];
                let payload = serde_json::json!({
                    "role": format!("{:?}", request.role),
                    "session_id": request.session_id,
                    "plan_path": request.plan_path.to_string_lossy(),
                    "runtime_allowlist": request.runtime_allowlist,
                    "prompt": request.input_prompt,
                    "timeout_seconds": request.timeout_seconds,
                });
                let payload_bytes = serde_json::to_vec(&payload)
                    .map_err(|e| format!("Failed to serialize structured stdin payload: {e}"))?;
                let run_dir = if let Some(ref snap) = request.snapshot_dir {
                    snap.clone()
                } else {
                    request.workspace_root.clone()
                };
                let mut env = Vec::new();
                for name in [
                    "PATH",
                    "TEMP",
                    "TMP",
                    "APPDATA",
                    "LOCALAPPDATA",
                    "USERPROFILE",
                ] {
                    if let Some(val) = std::env::var_os(name) {
                        env.push((name.into(), val));
                    }
                }
                (
                    cli.to_string_lossy().to_string(),
                    args,
                    run_dir,
                    payload_bytes,
                    env,
                    None,
                )
            };

        #[cfg(test)]
        if request.role == AutoConvergeRole::Reviewer {
            set_test_reviewer_dispatch_stage(Some(ReviewerDispatchStage::LaunchConfigBuilt));
        }

        let timeout_secs = request.timeout_seconds;

        // Pre-spawn check: require output artifact absence before child launch
        if let Some(ref paths) = paths_for_verdict {
            match std::fs::symlink_metadata(&paths.output_last_message) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Ok(_) => {
                    return Err(
                        "Reviewer output artifact already exists before process spawn; refusing execution"
                            .to_string(),
                    );
                }
                Err(e) => {
                    return Err(format!(
                        "Cannot verify pre-spawn absence of reviewer output artifact: {e}"
                    ));
                }
            }
        }

        #[cfg(test)]
        if request.role == AutoConvergeRole::Reviewer {
            set_test_reviewer_dispatch_stage(Some(ReviewerDispatchStage::PreSpawnChecked));
        }

        // ProcessRunner enforces:
        // 1. Windows Job Object creation before process spawn
        // 2. Child created suspended (CREATE_SUSPENDED)
        // 3. Child assigned to Job Object before any instructions execute
        // 4. Verification of Job Object assignment; fail-closed termination on failure
        // 5. Output limit bounding (1 MiB cap)
        // 6. Whole-process-tree termination on timeout or cancellation
        #[cfg(test)]
        if request.role == AutoConvergeRole::Reviewer {
            set_test_reviewer_dispatch_stage(Some(ReviewerDispatchStage::ProcessSpawnAttempted));
        }
        let res = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Failed to create runtime: {e}"))?;
            let runner = crate::orchestrator::process_runner::ProcessRunner::new();
            rt.block_on(runner.run_with_stdin_in_environment(
                &exec_str,
                &args,
                Some(&run_dir),
                Some(&payload_bytes),
                std::time::Duration::from_secs(timeout_secs),
                None,
                &child_env,
            ))
        })
        .join()
        .map_err(|_| "Process execution thread panicked".to_string())??;

        if res.timed_out {
            return Err(format!(
                "Agent process execution timed out after {} seconds and was terminated",
                request.timeout_seconds
            ));
        }

        if res.stdout.is_truncated || res.stderr.is_truncated {
            return Err(
                "Agent process output exceeded maximum capture limit (1 MiB) and was truncated"
                    .to_string(),
            );
        }

        let stdout = res.stdout.text;
        let stderr = res.stderr.text;
        let exit_code = res.exit_code;

        // Require exit code exactly zero for Reviewer and Implementer execution
        if request.role == AutoConvergeRole::Reviewer {
            validate_reviewer_exit_status(exit_code)?;
        } else if request.role == AutoConvergeRole::Implementer {
            match exit_code {
                Some(0) => {}
                Some(code) => {
                    return Err(format!(
                        "Implementer process exited unsuccessfully with exit code {code}"
                    ));
                }
                None => {
                    return Err(
                        "Implementer process terminated abnormally without exit code".to_string(),
                    );
                }
            }
        }

        // Parse and validate structured verdict against typed schema if Reviewer
        let verdict_json = if request.role == AutoConvergeRole::Reviewer {
            if stdout.contains("thread.started") {
                let _ = super::codex_exec::parse_thread_started_jsonl(stdout.as_bytes())?;
            }

            let paths = paths_for_verdict.as_ref().ok_or_else(|| {
                "Reviewer launch paths missing from ProductionAgentRunner".to_string()
            })?;

            let artifact_path = &paths.output_last_message;
            validate_reviewer_output_artifact_metadata(artifact_path)?;

            // Confirm artifact resides within the run's artifact directory
            let canonical_artifact = artifact_path.canonicalize().map_err(|e| {
                format!("Failed to resolve canonical path for reviewer output artifact: {e}")
            })?;
            let artifacts_parent = artifact_path
                .parent()
                .ok_or_else(|| "Reviewer output artifact has no parent directory".to_string())?;
            let canonical_artifacts_dir = artifacts_parent.canonicalize().map_err(|e| {
                format!("Failed to resolve canonical path for reviewer artifacts directory: {e}")
            })?;
            if !canonical_artifact.starts_with(&canonical_artifacts_dir) {
                return Err("Reviewer output artifact escapes run artifacts directory".to_string());
            }

            let schema = super::codex_exec::read_bounded_artifact(&paths.output_schema)?;
            super::codex_exec::validate_object_schema(&schema)?;
            let artifact = super::codex_exec::read_bounded_artifact(artifact_path)?;
            super::codex_exec::validate_artifact_against_schema(&schema, &artifact)?;

            let candidate = serde_json::to_string(&artifact)
                .map_err(|e| format!("Failed to serialize reviewer artifact: {e}"))?;

            let verdict: super::types::CodexReviewVerdict = serde_json::from_str(&candidate)
                .map_err(|e| {
                    format!("Reviewer verdict failed schema validation for CodexReviewVerdict: {e}")
                })?;
            Some(serde_json::to_string(&verdict).unwrap_or(candidate))
        } else {
            None
        };

        Ok(RoleExecutionResponse {
            exit_code: exit_code.unwrap_or(-1),
            stdout,
            stderr,
            verdict_json,
        })
    }
}

/// Mock command runner retained for deterministic unit/adversarial tests.
#[derive(Default, Clone)]
pub struct MockAgentRunner {
    responses: Arc<Mutex<Vec<RoleExecutionResponse>>>,
    executed_requests: Arc<Mutex<Vec<RoleExecutionRequest>>>,
}

impl MockAgentRunner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn queue_response(&self, response: RoleExecutionResponse) {
        self.responses.lock().unwrap().push(response);
    }

    pub fn get_executed_requests(&self) -> Vec<RoleExecutionRequest> {
        self.executed_requests.lock().unwrap().clone()
    }
}

impl AgentCommandRunner for MockAgentRunner {
    fn execute_role_session(
        &self,
        request: &RoleExecutionRequest,
    ) -> Result<RoleExecutionResponse, String> {
        self.executed_requests.lock().unwrap().push(request.clone());

        let mut queue = self.responses.lock().unwrap();
        if !queue.is_empty() {
            Ok(queue.remove(0))
        } else {
            match request.role {
                AutoConvergeRole::Planner => {
                    let plan = "# Implementation Plan\n\n```json auto-converge-scope\n{\n  \"allowlist\": [\"src/lib.rs\"]\n}\n```\n".to_string();
                    if let Some(parent) = request.plan_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::write(&request.plan_path, &plan);
                    Ok(RoleExecutionResponse {
                        exit_code: 0,
                        stdout: plan,
                        stderr: String::new(),
                        verdict_json: None,
                    })
                }
                AutoConvergeRole::Reviewer => {
                    let plan_content = if request.plan_path.exists() {
                        std::fs::read_to_string(&request.plan_path).unwrap_or_default()
                    } else {
                        String::new()
                    };
                    let cand_digest = super::types::sha256_hex(plan_content.as_bytes());
                    let allowlist_digest = super::guards::extract_and_validate_allowlist_from_plan(&plan_content, cfg!(windows))
                        .map(|(_, d)| d)
                        .unwrap_or_else(|_| "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string());
                    let (profile_digest, base_ws_digest) = if let Some(ref rid) = request.run_id {
                        if let Ok(state_p) = super::engine_run_state_path(rid) {
                            if let Ok(bytes) = std::fs::read(&state_p) {
                                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                                    let prof = val.get("policy_profile_digest")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string())
                                        .unwrap_or_else(|| {
                                            super::profile_store::builtin_profiles()
                                                .into_iter()
                                                .find(|p| p.id == "standard")
                                                .map(|p| p.canonical_digest())
                                                .unwrap_or_default()
                                        });
                                    let ws_dig = val.get("workspace")
                                        .and_then(|w| w.get("baseline_workspace_digest"))
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string())
                                        .unwrap_or_else(|| "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string());
                                    (prof, ws_dig)
                                } else {
                                    (
                                        super::profile_store::builtin_profiles().into_iter().find(|p| p.id == "standard").map(|p| p.canonical_digest()).unwrap_or_default(),
                                        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                                    )
                                }
                            } else {
                                (
                                    super::profile_store::builtin_profiles().into_iter().find(|p| p.id == "standard").map(|p| p.canonical_digest()).unwrap_or_default(),
                                    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                                )
                            }
                        } else {
                            (
                                super::profile_store::builtin_profiles().into_iter().find(|p| p.id == "standard").map(|p| p.canonical_digest()).unwrap_or_default(),
                                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                            )
                        }
                    } else {
                        (
                            super::profile_store::builtin_profiles()
                                .into_iter()
                                .find(|p| p.id == "standard")
                                .map(|p| p.canonical_digest())
                                .unwrap_or_else(|| "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string()),
                            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                        )
                    };

                    let verdict = serde_json::json!({
                        "verdict": "APPROVE",
                        "summary": "Mock approval generated from plan snapshot",
                        "findings": [],
                        "approval_binding": {
                            "target_type": "plan",
                            "candidate_plan_digest": cand_digest,
                            "allowlist_digest": allowlist_digest,
                            "policy_profile_digest": profile_digest,
                            "baseline_workspace_digest": base_ws_digest,
                        }
                    });

                    Ok(RoleExecutionResponse {
                        exit_code: 0,
                        stdout: "Mock reviewer approved plan".into(),
                        stderr: String::new(),
                        verdict_json: Some(verdict.to_string()),
                    })
                }
                _ => {
                    Ok(RoleExecutionResponse {
                        exit_code: 0,
                        stdout: "Mock execution succeeded".into(),
                        stderr: String::new(),
                        verdict_json: Some(r#"{"verdict":"APPROVE","summary":"Mock approval","findings":[],"approval_binding":{"target_type":"plan","candidate_plan_digest":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","baseline_workspace_digest":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","allowlist_digest":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","policy_profile_digest":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}}"#.into()),
                    })
                }
            }
        }
    }
}

/// Strictly validate the exit status of a Reviewer subprocess.
/// Only exit code exactly zero is accepted. Non-zero exit codes, abnormal termination,
/// or unknown exit status (None) fail closed.
pub(crate) fn validate_reviewer_exit_status(exit_code: Option<i32>) -> Result<(), String> {
    match exit_code {
        Some(0) => Ok(()),
        Some(code) => Err(format!(
            "Reviewer process exited unsuccessfully with exit code {code}"
        )),
        None => Err("Reviewer process terminated abnormally without exit code".to_string()),
    }
}

/// Strictly validate metadata for a Reviewer structured output artifact.
/// The artifact must exist and be a regular non-symlink file without reparse point attributes.
pub(crate) fn validate_reviewer_output_artifact_metadata(
    artifact_path: &std::path::Path,
) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(artifact_path)
        .map_err(|e| format!("Reviewer output artifact missing or inaccessible: {e}"))?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err("Reviewer output artifact must be a regular non-symlink file".to_string());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err("Reviewer output artifact must be a regular non-symlink file".to_string());
        }
    }
    Ok(())
}
