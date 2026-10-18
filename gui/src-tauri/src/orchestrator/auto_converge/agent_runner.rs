use super::isolation::{AutoConvergeRole, PlatformIsolationGuard};
use std::path::PathBuf;
use std::process::Command;
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

/// Production runner launching real CLI subprocesses within role boundaries.
#[derive(Debug, Clone, Default)]
pub(crate) struct ProductionAgentRunner {
    pub cli_path: Option<PathBuf>,
}

pub const MAX_CAPTURE_BYTES: usize = 1024 * 1024; // 1 MiB cap (§22.4.4)

impl ProductionAgentRunner {
    pub fn new() -> Self {
        Self { cli_path: None }
    }

    pub fn with_cli(cli_path: PathBuf) -> Self {
        Self {
            cli_path: Some(cli_path),
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

        // Enforce Reviewer isolation preflight and OS DACL boundary before spawning child
        let _reviewer_guard = if request.role == AutoConvergeRole::Reviewer {
            let snap = request.snapshot_dir.as_deref().ok_or_else(|| {
                "Reviewer role execution requires an isolated snapshot directory".to_string()
            })?;
            PlatformIsolationGuard::verify_reviewer_os_boundary(&request.workspace_root, snap)
                .map_err(|e| format!("Reviewer isolation preflight failed: {e}"))?;
            Some(
                super::isolation::ReviewerIsolationGuard::new(snap)
                    .map_err(|e| format!("Failed to apply Reviewer OS isolation: {e}"))?,
            )
        } else {
            None
        };

        let mut cmd = Command::new(&cli);
        cmd.arg("--role")
            .arg(format!("{:?}", request.role).to_lowercase())
            .arg("--session")
            .arg(&request.session_id);

        let run_dir = if let Some(ref snap) = request.snapshot_dir {
            snap.clone()
        } else {
            request.workspace_root.clone()
        };

        let exec_str = cli.to_string_lossy().to_string();
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

        let timeout_secs = request.timeout_seconds;

        // ProcessRunner enforces:
        // 1. Windows Job Object creation before process spawn
        // 2. Child created suspended (CREATE_SUSPENDED)
        // 3. Child assigned to Job Object before any instructions execute
        // 4. Verification of Job Object assignment; fail-closed termination on failure
        // 5. Output limit bounding (1 MiB cap)
        // 6. Whole-process-tree termination on timeout or cancellation
        let res = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Failed to create runtime: {e}"))?;
            let runner = crate::orchestrator::process_runner::ProcessRunner::new();
            rt.block_on(runner.run_with_stdin(
                &exec_str,
                &args,
                Some(&run_dir),
                Some(&payload_bytes),
                std::time::Duration::from_secs(timeout_secs),
                None,
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

        // Parse and validate structured verdict against typed schema if Reviewer
        let verdict_json = if request.role == AutoConvergeRole::Reviewer {
            let start = stdout.find('{').ok_or_else(|| {
                "Reviewer stdout did not contain opening JSON object delimiter '{'".to_string()
            })?;
            let end = stdout.rfind('}').ok_or_else(|| {
                "Reviewer stdout did not contain closing JSON object delimiter '}'".to_string()
            })?;
            if end < start {
                return Err("Reviewer stdout has invalid JSON delimiters".to_string());
            }
            let candidate = &stdout[start..=end];
            let verdict: super::types::CodexReviewVerdict = serde_json::from_str(candidate)
                .map_err(|e| {
                    format!("Reviewer verdict failed schema validation for CodexReviewVerdict: {e}")
                })?;
            Some(serde_json::to_string(&verdict).unwrap_or_else(|_| candidate.to_string()))
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
            Ok(RoleExecutionResponse {
                exit_code: 0,
                stdout: "Mock execution succeeded".into(),
                stderr: String::new(),
                verdict_json: Some(r#"{"verdict":"APPROVE","summary":"Mock approval","findings":[],"approval_binding":{"target_type":"plan","candidate_plan_digest":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","baseline_workspace_digest":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","allowlist_digest":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","policy_profile_digest":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}}"#.into()),
            })
        }
    }
}
