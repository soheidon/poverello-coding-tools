use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::agent_runner::MockAgentRunner;
use super::git_runner::{capture_baseline_file_manifest, ProductionGitCommandRunner};
use super::guards::{
    acquire_run_state_lock, load_and_verify_run_state_authenticated, save_run_state_authenticated,
};
use super::state_machine::{halt_blocked, initialize_run};
use super::types::{
    AutoConvergeProfile, AutoConvergeRunConfig, AutoConvergeRunState, AutoConvergeState,
    BlockedKind, ConvergenceLimits, ReviewerLevel,
};
use super::{engine_run_state_path, resolve_engine_authority_dir};

// ---------------------------------------------------------------------------
// 0. Identifier & Environment Helpers (§11.2, §16.4, §16.5)
// ---------------------------------------------------------------------------

static FORCE_UNSUPPORTED_PLATFORM_FOR_TEST: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
pub fn set_force_unsupported_platform_for_test(force: bool) {
    FORCE_UNSUPPORTED_PLATFORM_FOR_TEST.store(force, std::sync::atomic::Ordering::SeqCst);
}

pub fn is_r3_supported_platform() -> bool {
    if FORCE_UNSUPPORTED_PLATFORM_FOR_TEST.load(std::sync::atomic::Ordering::SeqCst) {
        return false;
    }
    cfg!(windows)
}

#[cfg(test)]
pub fn maybe_wait_r3_test_claim_barrier(req_id: &str) {
    if let Ok(barrier_dir) = std::env::var("POVERELLO_TEST_R3_BARRIER_DIR") {
        let dir = Path::new(&barrier_dir);
        let tag = std::env::var("POVERELLO_TEST_R3_WORKER_TAG")
            .unwrap_or_else(|_| std::process::id().to_string());
        let ready_file = dir.join(format!("ready_{tag}.txt"));
        let release_file = dir.join("release.txt");
        let _ = std::fs::write(&ready_file, format!("{}:{}", req_id, std::process::id()));
        let start = std::time::Instant::now();
        while !release_file.exists() {
            if start.elapsed() >= std::time::Duration::from_secs(5) {
                panic!("Timeout waiting for test barrier release");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}

#[cfg(not(test))]
#[inline]
pub fn maybe_wait_r3_test_claim_barrier(_req_id: &str) {}

pub fn validate_r3_identifier(id: &str, field_name: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 128 {
        return Err(format!(
            "Invalid {field_name}: length must be between 1 and 128 characters (got {})",
            id.len()
        ));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "Invalid {field_name}: must contain only ASCII alphanumeric, '-', or '_' (got {id:?})"
        ));
    }
    Ok(())
}

#[cfg(windows)]
pub fn get_current_process_creation_time() -> Result<u64, String> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    unsafe {
        let handle = GetCurrentProcess();
        let mut creation = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut exit = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut kernel = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut user = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        if GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) != 0 {
            let t = ((creation.dwHighDateTime as u64) << 32) | (creation.dwLowDateTime as u64);
            if t == 0 {
                Err("Process creation time returned 0 (unverified process identity)".to_string())
            } else {
                Ok(t)
            }
        } else {
            Err(format!(
                "Failed to get current process times: error code {}",
                windows_sys::Win32::Foundation::GetLastError()
            ))
        }
    }
}

#[cfg(not(windows))]
pub fn get_current_process_creation_time() -> Result<u64, String> {
    // Non-Windows environment without proven creation identity fails closed (§16.7 item 1)
    Err("OS process creation identity is unsupported on this platform".to_string())
}

#[cfg(test)]
static AUTH_CONTENT_READ_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
pub fn get_auth_content_read_count() -> usize {
    AUTH_CONTENT_READ_COUNT.load(std::sync::atomic::Ordering::SeqCst)
}

#[cfg(test)]
pub fn reset_auth_content_read_count() {
    AUTH_CONTENT_READ_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);
}

pub fn check_agent_auth_available() -> bool {
    // 1. Explicit API keys in environment
    if std::env::var("CODEX_API_KEY")
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false)
    {
        return true;
    }
    if std::env::var("OPENAI_API_KEY")
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false)
    {
        return true;
    }

    // 2. Auth file (auth.json) presence in dedicated CODEX_HOME or user home.
    // NOTE: config.toml alone represents unauthenticated user settings and does NOT establish login (§16.7 item 5).
    // In accordance with §16.12.2 D and §16.14.2 item 5, auth availability verification is strictly
    // a metadata presence check (.exists()). File contents must NEVER be read as credentials.
    let probe_auth_file = |file: &Path| -> bool {
        if file.exists() {
            if let Ok(counter_path_str) = std::env::var("POVERELLO_TEST_AUTH_READ_COUNTER_FILE") {
                let _ = std::fs::write(&counter_path_str, b"0");
            }
            true
        } else {
            false
        }
    };

    if let Ok(home) = std::env::var("CODEX_HOME") {
        let p = PathBuf::from(home);
        if probe_auth_file(&p.join("auth.json")) {
            return true;
        }
    }
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let p = PathBuf::from(user_profile).join(".codex");
        if probe_auth_file(&p.join("auth.json")) {
            return true;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join(".codex");
        if probe_auth_file(&p.join("auth.json")) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// 1. Data Contracts (§5.2, §5.3)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct R3StartRunRequest {
    pub request_id: String,
    pub workspace_root: String,
    pub task: String,
    #[serde(default = "default_mode")]
    pub mode: String, // "plan_only" or "implement"
    #[serde(default)]
    pub scope: Option<Vec<String>>,
    #[serde(default)]
    pub acceptance_criteria: Option<Vec<String>>,
    pub profile: AutoConvergeProfile,
    #[serde(default)]
    pub run_permissions: u32,
    #[serde(default)]
    pub reviewer_level: Option<ReviewerLevel>,
    #[serde(default)]
    pub r0d_acknowledged: bool,
    #[serde(default)]
    pub budget: Option<ConvergenceLimits>,
    #[serde(default)]
    pub agent_cli_path: Option<String>,
}

fn default_mode() -> String {
    "implement".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunOwnerIdentity {
    pub run_id: String,
    pub owner_nonce: String,
    pub process_id: u32,
    pub process_creation_time: u64,
    pub created_at: String,
    pub canonical_workspace: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum R3ControlAction {
    Status,
    Pause,
    Cancel,
    Resume,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct R3ControlRequest {
    pub request_id: String,
    pub run_id: String,
    pub action: R3ControlAction,
    pub issued_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct R3ControlResponse {
    pub request_id: String,
    pub run_id: String,
    pub action: R3ControlAction,
    pub accepted: bool,
    pub quiescent: bool,
    pub current_state: Option<AutoConvergeState>,
    pub message: Option<String>,
    pub completed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum R3RunEvent {
    Start {
        run_id: String,
        request_id: String,
        workspace_root: String,
        mode: String,
    },
    Progress {
        run_id: String,
        step: usize,
        current_state: AutoConvergeState,
        message: String,
    },
    Quiescent {
        run_id: String,
        current_state: AutoConvergeState,
        is_terminal: bool,
    },
    Result {
        run_id: String,
        status: String,
        state: AutoConvergeRunState,
    },
    Error {
        run_id: Option<String>,
        error: String,
    },
}

// ---------------------------------------------------------------------------
// 2. Test Seam for Non-Public Fake Dependency Injection (§11.2, A14)
// ---------------------------------------------------------------------------

static TEST_FAKE_RUNNER: Mutex<Option<Arc<MockAgentRunner>>> = Mutex::new(None);
pub static TEST_R3_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub fn set_r3_test_fake_runner(runner: Option<Arc<MockAgentRunner>>) {
    *TEST_FAKE_RUNNER.lock().unwrap() = runner;
}

pub fn get_r3_test_fake_runner() -> Option<Arc<MockAgentRunner>> {
    let guard = TEST_FAKE_RUNNER.lock().unwrap();
    if let Some(ref r) = *guard {
        return Some(r.clone());
    }
    drop(guard);
    #[cfg(debug_assertions)]
    if std::env::var("POVERELLO_TEST_R3_FAKE_RUNNER")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        return Some(Arc::new(MockAgentRunner::new()));
    }
    None
}

// ---------------------------------------------------------------------------
// 3. Workspace Run Lock & Request Registry Lock (Cross-Process Contention Prevention, §5.2, §11.2, A02, §16.7 item 3)
// ---------------------------------------------------------------------------

pub struct RegistryLock {
    #[allow(dead_code)]
    file: File,
}

pub fn acquire_registry_lock() -> Result<RegistryLock, String> {
    if !is_r3_supported_platform() {
        return Err(
            "UnsupportedPlatform: R3 Auto-Converge execution is only supported on Windows"
                .to_string(),
        );
    }
    let auth_dir = resolve_engine_authority_dir()?;
    let locks_dir = auth_dir.join("locks");
    fs::create_dir_all(&locks_dir)
        .map_err(|e| format!("Failed creating engine locks directory: {e}"))?;
    let lock_path = locks_dir.join("request_registry.lock");

    let start = Instant::now();
    let timeout = Duration::from_secs(5);

    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        loop {
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .share_mode(0)
                .open(&lock_path)
            {
                Ok(f) => return Ok(RegistryLock { file: f }),
                Err(e) if e.raw_os_error() == Some(32) => {
                    if start.elapsed() >= timeout {
                        return Err(format!(
                            "Preflight rejected: Request registry lock timed out waiting for concurrent process: {e}"
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(e) => {
                    return Err(format!("Failed acquiring request registry lock: {e}"));
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        Err(
            "UnsupportedPlatform: R3 Auto-Converge execution is only supported on Windows"
                .to_string(),
        )
    }
}

pub struct WorkspaceRunLock {
    pub lock_path: PathBuf,
    #[allow(dead_code)]
    file: File,
}

pub fn acquire_workspace_run_lock(
    canonical_ws: &Path,
    run_id: &str,
) -> Result<WorkspaceRunLock, String> {
    if !is_r3_supported_platform() {
        return Err(
            "UnsupportedPlatform: R3 Auto-Converge execution is only supported on Windows"
                .to_string(),
        );
    }
    let auth_dir = resolve_engine_authority_dir()?;
    let locks_dir = auth_dir.join("locks");
    fs::create_dir_all(&locks_dir)
        .map_err(|e| format!("Failed creating engine locks directory: {e}"))?;

    // Derive stable lock filename from canonical workspace path
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(canonical_ws.to_string_lossy().as_bytes());
    let ws_hash = format!("{:x}", hasher.finalize());
    let lock_path = locks_dir.join(format!("ws_{ws_hash}.lock"));

    // On Windows, open exclusively without FILE_SHARE_READ / FILE_SHARE_WRITE / FILE_SHARE_DELETE
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // share_mode = 0 means exclusive cross-process lock
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .open(&lock_path)
        {
            Ok(mut f) => {
                let _ = writeln!(f, "{run_id}:{}", std::process::id());
                Ok(WorkspaceRunLock { lock_path, file: f })
            }
            Err(e) => {
                if e.raw_os_error() == Some(32) {
                    // ERROR_SHARING_VIOLATION
                    Err(format!(
                        "Preflight rejected: Workspace {:?} is already locked by another running process",
                        canonical_ws
                    ))
                } else {
                    Err(format!(
                        "Failed acquiring workspace run lock at {:?}: {e}",
                        lock_path
                    ))
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        Err(
            "UnsupportedPlatform: R3 Auto-Converge execution is only supported on Windows"
                .to_string(),
        )
    }
}

// ---------------------------------------------------------------------------
// 4. GUI/CLI Common R3 Preflight (§11.2, A14)
// ---------------------------------------------------------------------------

pub struct R3PreflightSuccess {
    pub canonical_workspace: PathBuf,
    pub effective_permissions: u32,
    pub run_id: String,
    pub workspace_lock: WorkspaceRunLock,
    pub is_fake_test_mode: bool,
    pub resolved_cli_path: Option<PathBuf>,
}

#[derive(Debug)]
pub struct R3PureValidationResult {
    pub canonical_workspace: PathBuf,
    pub effective_permissions: u32,
    pub is_fake_test_mode: bool,
    pub resolved_cli_path: Option<PathBuf>,
    pub creation_time: u64,
}

pub fn r3_pure_validation(req: &R3StartRunRequest) -> Result<R3PureValidationResult, String> {
    if !is_r3_supported_platform() {
        return Err(
            "UnsupportedPlatform: R3 Auto-Converge execution is only supported on Windows"
                .to_string(),
        );
    }

    // 0. Identifier validation (§11.2, §16.4, §16.5)
    validate_r3_identifier(&req.request_id, "request_id")?;

    // 1. Mode Validation (§16.4 item 9)
    if req.mode != "plan_only" && req.mode != "implement" {
        return Err(format!(
            "Preflight rejected: Invalid mode {:?}; must be 'plan_only' or 'implement'",
            req.mode
        ));
    }

    // 2. R0-D Explicit Choice and Limitation Acknowledgment (§3.2, §11.2, A14)
    if req.reviewer_level != Some(ReviewerLevel::R0D) {
        return Err(format!(
            "Preflight rejected: R0-D reviewer level must be explicitly selected (got {:?}); strong OS-isolation modes remain unsupported and cannot be fallen back to automatically",
            req.reviewer_level
        ));
    }
    if !req.r0d_acknowledged {
        return Err(
            "Preflight rejected: R0-D limitations must be explicitly acknowledged before starting run"
                .to_string(),
        );
    }

    // 3. Permission and Profile Capability Intersection (§5.3, §11.2, A14)
    let cap_mask = req.profile.capabilities.to_bitmask();
    let requested_mask = req.run_permissions;
    if (requested_mask & !cap_mask) != 0 {
        return Err(format!(
            "Preflight rejected: Requested permissions ({:#x}) exceed profile capabilities ({:#x})",
            requested_mask, cap_mask
        ));
    }
    let effective_permissions = requested_mask & cap_mask;

    // 4. Canonical Repository Verification (§5.3, §11.2, A14)
    let ws_path = PathBuf::from(&req.workspace_root);
    if !ws_path.exists() || !ws_path.is_dir() {
        return Err(format!(
            "Preflight rejected: Workspace root does not exist or is not a directory: {:?}",
            ws_path
        ));
    }
    let canonical_ws = ws_path
        .canonicalize()
        .map_err(|e| format!("Preflight rejected: Failed canonicalizing workspace root: {e}"))?;

    let git_dir = canonical_ws.join(".git");
    if !git_dir.exists() {
        return Err(format!(
            "Preflight rejected: Workspace {:?} is not a Git repository (.git not found)",
            canonical_ws
        ));
    }

    // 5. Budget / Convergence Limits Verification (§5.3, §11.2, A14)
    let limits = req
        .budget
        .as_ref()
        .unwrap_or(&req.profile.convergence_limits);
    if limits.max_plan_reviews == 0 && limits.max_impl_reviews == 0 {
        return Err("Preflight rejected: Budget limits max_plan_reviews and max_impl_reviews must not both be 0".to_string());
    }

    // 6. Authentication metadata validation (§11.2, §16.4, §16.5, §16.12.2 D, §16.14.2 item 5)
    // Strictly verify presence of auth configuration / credentials metadata without reading secret contents.
    if !check_agent_auth_available() {
        return Err("Preflight rejected: No agent credentials or authentication configuration found: failing closed without spawning any process".to_string());
    }

    // 7. Test Seam / Candidate Isolation Gate (§11.2, §16.4, §16.5)
    let fake_runner = get_r3_test_fake_runner();
    let is_fake_test_mode = fake_runner.is_some();

    let resolved_cli_opt = req
        .agent_cli_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| std::env::var("POVERELLO_AGENT_CLI").ok().map(PathBuf::from));

    if !is_fake_test_mode {
        // In real execution, check available CLI
        let _cli_path = match resolved_cli_opt {
            Some(ref p) if p.exists() => p.clone(),
            Some(ref p) => {
                return Err(format!(
                    "Preflight rejected: Configured agent CLI not found at: {}",
                    p.display()
                ));
            }
            None => {
                return Err("Preflight rejected: No configured agent CLI or existing credentials available: failing closed without spawning any process".to_string());
            }
        };

        // Unit 1 Gate: candidate isolation and real dispatch are unready in Unit 1 (§16.4 item 3, 4)
        return Err("Preflight rejected: Candidate isolation is unready in Unit 1; real dispatch is disabled until Unit 3".to_string());
    }

    // 8. Process creation identity
    let creation_time = get_current_process_creation_time()?;

    Ok(R3PureValidationResult {
        canonical_workspace: canonical_ws,
        effective_permissions,
        is_fake_test_mode,
        resolved_cli_path: resolved_cli_opt,
        creation_time,
    })
}

pub fn r3_common_preflight(
    req: &R3StartRunRequest,
    assigned_run_id: &str,
) -> Result<R3PreflightSuccess, String> {
    validate_r3_identifier(assigned_run_id, "run_id")?;
    let pure = r3_pure_validation(req)?;
    let ws_lock = acquire_workspace_run_lock(&pure.canonical_workspace, assigned_run_id)?;
    Ok(R3PreflightSuccess {
        canonical_workspace: pure.canonical_workspace,
        effective_permissions: pure.effective_permissions,
        run_id: assigned_run_id.to_string(),
        workspace_lock: ws_lock,
        is_fake_test_mode: pure.is_fake_test_mode,
        resolved_cli_path: pure.resolved_cli_path,
    })
}

// ---------------------------------------------------------------------------
// 5. Control Mailbox & Run Owner Identity (§5.2, §11.2, A11, A17, §16.4, §16.5, §16.14.2 item 1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct R3RunArtifactPaths {
    pub run_dir: PathBuf,
    pub state_path: PathBuf,
    pub owner_path: PathBuf,
}

/// Pure helper to resolve the canonical OS user-data per-run artifact paths under `engine_runs/<run_id>`.
/// Does NOT create directories or files (§16.14.2 item 1).
pub fn resolve_r3_run_artifact_paths(run_id: &str) -> Result<R3RunArtifactPaths, String> {
    validate_r3_identifier(run_id, "run_id")?;
    let auth_dir = resolve_engine_authority_dir()?;
    let runs_base = auth_dir
        .parent()
        .unwrap_or(Path::new("."))
        .join("engine_runs");
    let run_dir = runs_base.join(run_id);
    let state_path = run_dir.join("run_state.json");
    let owner_path = run_dir.join("owner.json");
    Ok(R3RunArtifactPaths {
        run_dir,
        state_path,
        owner_path,
    })
}

pub fn r3_run_dir(run_id: &str) -> Result<PathBuf, String> {
    let paths = resolve_r3_run_artifact_paths(run_id)?;
    fs::create_dir_all(&paths.run_dir).map_err(|e| {
        format!(
            "Failed creating run directory {}: {e}",
            paths.run_dir.display()
        )
    })?;
    Ok(paths.run_dir)
}

pub fn r3_mailbox_dirs(run_id: &str) -> Result<(PathBuf, PathBuf), String> {
    validate_r3_identifier(run_id, "run_id")?;
    let run_dir = r3_run_dir(run_id)?;
    let mailbox = run_dir.join("control_mailbox");
    let reqs = mailbox.join("requests");
    let resps = mailbox.join("responses");
    fs::create_dir_all(&reqs).map_err(|e| format!("Failed creating requests mailbox: {e}"))?;
    fs::create_dir_all(&resps).map_err(|e| format!("Failed creating responses mailbox: {e}"))?;
    Ok((reqs, resps))
}

pub fn record_run_owner_identity(owner: &RunOwnerIdentity) -> Result<(), String> {
    validate_r3_identifier(&owner.run_id, "run_id")?;
    let run_dir = r3_run_dir(&owner.run_id)?;
    let owner_path = run_dir.join("owner.json");
    let tmp_path = run_dir.join(format!("owner.json.tmp-{}", uuid::Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(owner)
        .map_err(|e| format!("Failed serializing owner.json: {e}"))?;
    fs::write(&tmp_path, &bytes).map_err(|e| {
        format!(
            "Failed writing owner.json temp file at {}: {e}",
            tmp_path.display()
        )
    })?;
    fs::rename(&tmp_path, &owner_path).map_err(|e| {
        format!(
            "Failed committing owner.json atomically at {}: {e}",
            owner_path.display()
        )
    })?;
    Ok(())
}

pub fn read_run_owner_identity(run_id: &str) -> Result<RunOwnerIdentity, String> {
    validate_r3_identifier(run_id, "run_id")?;
    let run_dir = r3_run_dir(run_id)?;
    let owner_path = run_dir.join("owner.json");
    if !owner_path.exists() {
        return Err(format!("No owner.json found for run_id '{run_id}'"));
    }
    let bytes = fs::read(&owner_path).map_err(|e| format!("Failed reading owner.json: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("Failed parsing owner.json: {e}"))
}

pub fn is_owner_process_alive_raw(process_id: u32, process_creation_time: u64) -> bool {
    if process_creation_time == 0 {
        return false;
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, FALSE, FILETIME, STILL_ACTIVE};
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };

        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, process_id);
            if handle.is_null() {
                return false;
            }
            let mut exit_code = 0u32;
            let ok_exit = GetExitCodeProcess(handle, &mut exit_code);
            let mut creation = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let mut exit = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let mut kernel = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let mut user = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let ok_times =
                GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user);
            CloseHandle(handle);

            if ok_exit == FALSE || exit_code != STILL_ACTIVE as u32 {
                return false;
            }
            if ok_times == FALSE {
                return false;
            }
            let proc_creation =
                ((creation.dwHighDateTime as u64) << 32) | (creation.dwLowDateTime as u64);
            if proc_creation == 0 || proc_creation != process_creation_time {
                // Process ID reused by another OS process or unverified identity
                return false;
            }
            true
        }
    }
    #[cfg(not(windows))]
    {
        // Fail closed when OS creation identity cannot be verified (§16.8.3 item 1)
        false
    }
}

pub fn is_owner_process_alive(owner: &RunOwnerIdentity) -> bool {
    is_owner_process_alive_raw(owner.process_id, owner.process_creation_time)
}

pub fn send_control_request(req: &R3ControlRequest) -> Result<(), String> {
    validate_r3_identifier(&req.run_id, "run_id")?;
    validate_r3_identifier(&req.request_id, "request_id")?;
    let (reqs_dir, _) = r3_mailbox_dirs(&req.run_id)?;
    let req_path = reqs_dir.join(format!("{}.json", req.request_id));
    let tmp_path = reqs_dir.join(format!("{}.tmp-{}", req.request_id, uuid::Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(req)
        .map_err(|e| format!("Failed serializing control request: {e}"))?;
    fs::write(&tmp_path, &bytes).map_err(|e| {
        format!(
            "Failed writing temporary control request to {}: {e}",
            tmp_path.display()
        )
    })?;
    fs::rename(&tmp_path, &req_path).map_err(|e| {
        format!(
            "Failed committing control request to {}: {e}",
            req_path.display()
        )
    })?;
    Ok(())
}

pub fn poll_control_response(
    run_id: &str,
    request_id: &str,
    timeout: Duration,
) -> Result<R3ControlResponse, String> {
    validate_r3_identifier(run_id, "run_id")?;
    validate_r3_identifier(request_id, "request_id")?;
    let (_, resps_dir) = r3_mailbox_dirs(run_id)?;
    let resp_path = resps_dir.join(format!("{request_id}.json"));

    let start = Instant::now();
    loop {
        if resp_path.exists() {
            let bytes = fs::read(&resp_path)
                .map_err(|e| format!("Failed reading control response: {e}"))?;
            let resp: R3ControlResponse = serde_json::from_slice(&bytes)
                .map_err(|e| format!("Failed parsing control response: {e}"))?;
            return Ok(resp);
        }

        // Check if owner process is still alive
        if let Ok(owner) = read_run_owner_identity(run_id) {
            if !is_owner_process_alive(&owner) {
                return Err(format!(
                    "RECOVERY_REQUIRED: Run owner process {} for run '{}' disappeared before acknowledging control request '{}'",
                    owner.process_id, run_id, request_id
                ));
            }
        }

        if start.elapsed() >= timeout {
            return Err(format!(
                "Timeout waiting for control response to request '{}' on run '{}'",
                request_id, run_id
            ));
        }

        std::thread::sleep(Duration::from_millis(50));
    }
}

pub fn owner_process_pending_mailbox_requests(
    run_id: &str,
    state: &mut AutoConvergeRunState,
    state_path: &Path,
) -> Result<Option<R3ControlAction>, String> {
    validate_r3_identifier(run_id, "run_id")?;
    let (reqs_dir, resps_dir) = r3_mailbox_dirs(run_id)?;
    let entries = match fs::read_dir(&reqs_dir) {
        Ok(e) => e,
        Err(_) => return Ok(None),
    };

    let mut applied_action = None;

    for entry in entries.flatten() {
        let p = entry.path();
        if p.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.contains(".tmp-"))
            .unwrap_or(false)
        {
            continue;
        }
        if p.extension().and_then(|e| e.to_str()) == Some("json") {
            let bytes = match fs::read(&p) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let req: R3ControlRequest = match serde_json::from_slice(&bytes) {
                Ok(r) => r,
                Err(_) => continue,
            };

            let resp_path = resps_dir.join(format!("{}.json", req.request_id));
            if resp_path.exists() {
                // Already processed, remove request file
                let _ = fs::remove_file(&p);
                continue;
            }

            // Verify run_id matches expected owner run_id (§16.4 item 7)
            if req.run_id != run_id {
                let resp = R3ControlResponse {
                    request_id: req.request_id.clone(),
                    run_id: run_id.to_string(),
                    action: req.action,
                    accepted: false,
                    quiescent: false,
                    current_state: Some(state.current_state),
                    message: Some(format!(
                        "Run ID mismatch in request: expected '{run_id}', got '{}'",
                        req.run_id
                    )),
                    completed_at: chrono::Utc::now().to_rfc3339(),
                };
                let resp_bytes = serde_json::to_vec_pretty(&resp)
                    .map_err(|e| format!("Failed serializing control response: {e}"))?;
                let tmp_resp =
                    resps_dir.join(format!("{}.tmp-{}", req.request_id, uuid::Uuid::new_v4()));
                fs::write(&tmp_resp, &resp_bytes)
                    .map_err(|e| format!("Failed writing control response temp file: {e}"))?;
                fs::rename(&tmp_resp, &resp_path)
                    .map_err(|e| format!("Failed committing control response atomically: {e}"))?;
                let _ = fs::remove_file(&p);
                continue;
            }

            // Apply action
            let (accepted, quiescent, message) = match req.action {
                R3ControlAction::Status => (true, false, Some("Current run status".to_string())),
                R3ControlAction::Pause => {
                    if !state.is_terminal {
                        state.is_paused = true;
                        applied_action = Some(R3ControlAction::Pause);
                        save_run_state_authenticated(state_path, state)?;
                        (true, true, Some("Run paused at step boundary".to_string()))
                    } else {
                        (false, false, Some("Cannot pause terminal run".to_string()))
                    }
                }
                R3ControlAction::Cancel => {
                    if !state.is_terminal {
                        halt_blocked(
                            state,
                            BlockedKind::FatalInvariant,
                            "Run cancelled via control mailbox",
                        );
                        state.is_terminal = true;
                        applied_action = Some(R3ControlAction::Cancel);
                        save_run_state_authenticated(state_path, state)?;
                        (true, true, Some("Run cancelled and halted".to_string()))
                    } else {
                        (
                            false,
                            false,
                            Some("Cannot cancel already terminal run".to_string()),
                        )
                    }
                }
                R3ControlAction::Resume => {
                    if state.is_paused && !state.is_terminal {
                        state.is_paused = false;
                        applied_action = Some(R3ControlAction::Resume);
                        save_run_state_authenticated(state_path, state)?;
                        (true, false, Some("Run resumed from pause".to_string()))
                    } else {
                        (
                            false,
                            false,
                            Some("Cannot resume non-paused or terminal run".to_string()),
                        )
                    }
                }
            };

            let resp = R3ControlResponse {
                request_id: req.request_id.clone(),
                run_id: run_id.to_string(),
                action: req.action,
                accepted,
                quiescent,
                current_state: Some(state.current_state),
                message,
                completed_at: chrono::Utc::now().to_rfc3339(),
            };

            let resp_bytes = serde_json::to_vec_pretty(&resp)
                .map_err(|e| format!("Failed serializing control response: {e}"))?;
            let tmp_resp =
                resps_dir.join(format!("{}.tmp-{}", req.request_id, uuid::Uuid::new_v4()));
            fs::write(&tmp_resp, &resp_bytes)
                .map_err(|e| format!("Failed writing control response temp file: {e}"))?;
            fs::rename(&tmp_resp, &resp_path)
                .map_err(|e| format!("Failed committing control response atomically: {e}"))?;

            // Remove processed request file
            let _ = fs::remove_file(&p);
        }
    }

    Ok(applied_action)
}

// ---------------------------------------------------------------------------
// 6. Request Deduplication & Mapping Registry (§5.3, §11.2, A02, §16.4, §16.5)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct RequestSemanticDigestPayload<'a> {
    format_version: &'static str,
    canonical_workspace: &'a str,
    task: &'a str,
    mode: &'a str,
    scope: Option<&'a [String]>,
    acceptance_criteria: Option<&'a [String]>,
    profile_digest: String,
    run_permissions: u32,
    reviewer_level: Option<&'a ReviewerLevel>,
    r0d_acknowledged: bool,
    budget: Option<&'a ConvergenceLimits>,
    effective_agent_cli: Option<&'a str>,
}

pub fn resolve_effective_agent_cli(req: &R3StartRunRequest) -> Option<String> {
    req.agent_cli_path
        .clone()
        .or_else(|| std::env::var("POVERELLO_AGENT_CLI").ok())
}

pub fn compute_request_semantic_digest(
    req: &R3StartRunRequest,
    canonical_ws: &Path,
    effective_cli: Option<&str>,
) -> Result<String, String> {
    let payload = RequestSemanticDigestPayload {
        format_version: "r3_semantic_request_v2",
        canonical_workspace: &canonical_ws.to_string_lossy(),
        task: &req.task,
        mode: &req.mode,
        scope: req.scope.as_deref(),
        acceptance_criteria: req.acceptance_criteria.as_deref(),
        profile_digest: req.profile.canonical_digest(),
        run_permissions: req.run_permissions,
        reviewer_level: req.reviewer_level.as_ref(),
        r0d_acknowledged: req.r0d_acknowledged,
        budget: req.budget.as_ref(),
        effective_agent_cli: effective_cli,
    };

    let canonical_bytes = serde_json::to_vec(&payload)
        .map_err(|e| format!("Failed serializing semantic digest payload: {e}"))?;

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&canonical_bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum R3ClaimStatus {
    Claimed,
    Starting,
    Started,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct R3RegistryRecord {
    pub run_id: String,
    pub semantic_digest: String,
    pub status: R3ClaimStatus,
    pub owner_pid: u32,
    pub owner_creation_time: u64,
    pub claim_id: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct R3RequestRegistry {
    pub schema_version: u32,
    pub records: BTreeMap<String, R3RegistryRecord>,
}

#[derive(Debug, Clone)]
pub enum R3RegistryDecision {
    Created { run_id: String, claim_id: String },
    InProgress { existing_run_id: String },
    ExistingTerminal { state: AutoConvergeRunState },
    Conflict { message: String },
    WorkspaceBusy { message: String },
    FailedRetryWithNewRequestId { message: String },
    RecoveryRequired { message: String },
}

// Note on registry durability (§16.13.2 item 5, §16.13.3 item 9):
// The temporary file is flushed and sync_all() is called on the file handle before atomic replacement (rename).
// Containing directory fsync is not performed across all Windows/filesystem configurations.
// Therefore, registry updates are atomic replacements without claiming power-loss durability.
fn save_registry_atomic(registry: &R3RequestRegistry) -> Result<(), String> {
    let auth_dir = resolve_engine_authority_dir()?;
    let registry_path = auth_dir.join("request_registry.json");
    let bytes = serde_json::to_vec_pretty(registry)
        .map_err(|e| format!("Failed serializing request registry: {e}"))?;
    let tmp_path = auth_dir.join(format!(
        "request_registry.json.tmp-{}",
        uuid::Uuid::new_v4()
    ));
    {
        let mut file = fs::File::create(&tmp_path)
            .map_err(|e| format!("Failed creating request registry temp file: {e}"))?;
        file.write_all(&bytes)
            .map_err(|e| format!("Failed writing request registry temp file: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("Failed syncing request registry temp file: {e}"))?;
    }
    fs::rename(&tmp_path, &registry_path)
        .map_err(|e| format!("Failed committing request registry atomically: {e}"))?;
    Ok(())
}

pub fn load_registry() -> Result<Option<R3RequestRegistry>, String> {
    let auth_dir = resolve_engine_authority_dir()?;
    let registry_path = auth_dir.join("request_registry.json");
    if !registry_path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&registry_path).map_err(|e| {
        format!(
            "Failed reading request registry at {:?}: {e}",
            registry_path
        )
    })?;

    let val: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| {
        format!(
            "Preflight rejected: Request registry is corrupted or unreadable at {:?}: {e}",
            registry_path
        )
    })?;

    let schema_ver = val.get("schema_version").and_then(|v| v.as_u64());
    if schema_ver != Some(2) {
        return Err(format!(
            "RECOVERY_REQUIRED: Request registry has unrecognized schema version ({:?}, expected 2) at {:?}",
            schema_ver, registry_path
        ));
    }

    let reg: R3RequestRegistry = serde_json::from_value(val).map_err(|e| {
        format!(
            "Preflight rejected: Request registry is corrupted or unreadable at {:?}: {e}",
            registry_path
        )
    })?;
    Ok(Some(reg))
}

#[cfg(test)]
static INJECT_UPDATE_CLAIM_STATUS_FAILURE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
pub fn set_inject_update_claim_status_failure_for_test(fail: bool) {
    INJECT_UPDATE_CLAIM_STATUS_FAILURE.store(fail, std::sync::atomic::Ordering::SeqCst);
}

pub fn update_claim_status(request_id: &str, new_status: R3ClaimStatus) -> Result<(), String> {
    #[cfg(test)]
    if INJECT_UPDATE_CLAIM_STATUS_FAILURE.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("Injected update_claim_status failure for testing".to_string());
    }
    validate_r3_identifier(request_id, "request_id")?;
    let _reg_lock = acquire_registry_lock()?;
    let mut reg = match load_registry()? {
        Some(r) => r,
        None => {
            return Err(format!(
                "Registry does not exist when updating claim status for '{request_id}'"
            ))
        }
    };
    if let Some(record) = reg.records.get_mut(request_id) {
        record.status = new_status;
        record.updated_at = chrono::Utc::now().to_rfc3339();
        save_registry_atomic(&reg)?;
        Ok(())
    } else {
        Err(format!("Request ID '{request_id}' not found in registry"))
    }
}

pub fn claim_or_deduplicate_request(
    req: &R3StartRunRequest,
    canonical_ws: &Path,
    effective_cli: Option<&str>,
) -> Result<R3RegistryDecision, String> {
    validate_r3_identifier(&req.request_id, "request_id")?;
    let _reg_lock = acquire_registry_lock()?;

    let mut reg = match load_registry()? {
        Some(r) => r,
        None => R3RequestRegistry {
            schema_version: 2,
            records: BTreeMap::new(),
        },
    };

    let current_digest = compute_request_semantic_digest(req, canonical_ws, effective_cli)?;

    if let Some(existing_record) = reg.records.get_mut(&req.request_id) {
        if existing_record.semantic_digest != current_digest {
            return Ok(R3RegistryDecision::Conflict {
                message: format!(
                    "Preflight rejected: request_id '{}' conflicts with previous request; conflicts with already registered request in run '{}' (different semantic digest / workspace)",
                    req.request_id, existing_record.run_id
                ),
            });
        }

        // Same digest. Reconcile status using canonical run artifact paths (§16.14.2 item 1):
        let artifact_paths = resolve_r3_run_artifact_paths(&existing_record.run_id)?;
        if artifact_paths.state_path.exists() {
            match load_and_verify_run_state_authenticated(&artifact_paths.state_path, None) {
                Ok(state) => {
                    if state.is_terminal
                        || state.is_paused
                        || state.current_state == AutoConvergeState::Blocked
                        || state.current_state == AutoConvergeState::PlanApproved
                    {
                        // Terminal state (§16.13.2 item 2: starting + terminal = ExistingTerminal)
                        return Ok(R3RegistryDecision::ExistingTerminal { state });
                    } else {
                        // Active state: verify owner liveness and identity
                        let is_alive = is_owner_process_alive_raw(
                            existing_record.owner_pid,
                            existing_record.owner_creation_time,
                        );
                        if is_alive {
                            return Ok(R3RegistryDecision::InProgress {
                                existing_run_id: existing_record.run_id.clone(),
                            });
                        } else {
                            return Ok(R3RegistryDecision::RecoveryRequired {
                                message: format!(
                                    "RECOVERY_REQUIRED: Active run '{}' owner process is dead/unknown while state is active",
                                    existing_record.run_id
                                ),
                            });
                        }
                    }
                }
                Err(e) => {
                    return Ok(R3RegistryDecision::RecoveryRequired {
                        message: format!(
                            "RECOVERY_REQUIRED: Run state exists for '{}' but failed authentication: {e}",
                            existing_record.run_id
                        ),
                    });
                }
            }
        }

        // State file does NOT exist: check for partial artifacts at canonical engine_runs/<run_id>
        let has_partial_artifacts =
            artifact_paths.run_dir.exists() || artifact_paths.owner_path.exists();
        match existing_record.status {
            R3ClaimStatus::Failed => {
                if has_partial_artifacts {
                    Ok(R3RegistryDecision::RecoveryRequired {
                        message: format!(
                            "RECOVERY_REQUIRED: Request '{}' is marked Failed but has partial artifacts remaining in run '{}' at {:?}",
                            req.request_id, existing_record.run_id, artifact_paths.run_dir
                        ),
                    })
                } else {
                    Ok(R3RegistryDecision::FailedRetryWithNewRequestId {
                        message: format!(
                            "Request '{}' previously failed in run '{}'. Retry with a new request_id.",
                            req.request_id, existing_record.run_id
                        ),
                    })
                }
            }
            R3ClaimStatus::Claimed | R3ClaimStatus::Starting => {
                let is_alive = is_owner_process_alive_raw(
                    existing_record.owner_pid,
                    existing_record.owner_creation_time,
                );
                if is_alive {
                    Ok(R3RegistryDecision::InProgress {
                        existing_run_id: existing_record.run_id.clone(),
                    })
                } else {
                    if has_partial_artifacts {
                        Ok(R3RegistryDecision::RecoveryRequired {
                            message: format!(
                                "RECOVERY_REQUIRED: Stale claim '{}' has dead owner but partial artifacts exist in run '{}' at {:?}",
                                req.request_id, existing_record.run_id, artifact_paths.run_dir
                            ),
                        })
                    } else {
                        // Dead owner and no artifacts: transition to Failed
                        let stale_run_id = existing_record.run_id.clone();
                        existing_record.status = R3ClaimStatus::Failed;
                        existing_record.updated_at = chrono::Utc::now().to_rfc3339();
                        save_registry_atomic(&reg)?;
                        Ok(R3RegistryDecision::FailedRetryWithNewRequestId {
                            message: format!(
                                "Request '{}' was claimed by dead owner in run '{}' (stale claim transitioned to Failed). Retry with a new request_id.",
                                req.request_id, stale_run_id
                            ),
                        })
                    }
                }
            }
            R3ClaimStatus::Started => Ok(R3RegistryDecision::RecoveryRequired {
                message: format!(
                    "RECOVERY_REQUIRED: Request '{}' is marked Started in run '{}' but state file does not exist",
                    req.request_id, existing_record.run_id
                ),
            }),
        }
    } else {
        // Brand new request!
        let assigned_run_id = format!("r3-{}", uuid::Uuid::new_v4());
        let claim_id = uuid::Uuid::new_v4().to_string();
        let creation_time = get_current_process_creation_time()?;
        let now_str = chrono::Utc::now().to_rfc3339();
        let record = R3RegistryRecord {
            run_id: assigned_run_id.clone(),
            semantic_digest: current_digest,
            status: R3ClaimStatus::Claimed,
            owner_pid: std::process::id(),
            owner_creation_time: creation_time,
            claim_id: claim_id.clone(),
            created_at: now_str.clone(),
            updated_at: now_str,
        };
        reg.records.insert(req.request_id.clone(), record);
        save_registry_atomic(&reg)?;
        Ok(R3RegistryDecision::Created {
            run_id: assigned_run_id,
            claim_id,
        })
    }
}

pub fn check_request_deduplication(
    req: &R3StartRunRequest,
) -> Result<Option<AutoConvergeRunState>, String> {
    let ws_path = PathBuf::from(&req.workspace_root);
    let canonical_ws = if ws_path.exists() {
        ws_path.canonicalize().unwrap_or(ws_path)
    } else {
        ws_path
    };
    let effective_cli = resolve_effective_agent_cli(req);
    match claim_or_deduplicate_request(req, &canonical_ws, effective_cli.as_deref())? {
        R3RegistryDecision::ExistingTerminal { state } => Ok(Some(state)),
        R3RegistryDecision::InProgress { .. } => Ok(None),
        R3RegistryDecision::Created { .. } => Ok(None),
        R3RegistryDecision::Conflict { message } => Err(message),
        R3RegistryDecision::WorkspaceBusy { message } => Err(message),
        R3RegistryDecision::FailedRetryWithNewRequestId { message } => Err(message),
        R3RegistryDecision::RecoveryRequired { message } => Err(message),
    }
}

pub fn record_request_in_registry(
    request_id: &str,
    _run_id: &str,
    req: &R3StartRunRequest,
    canonical_ws: &Path,
    effective_cli: Option<&str>,
) -> Result<(), String> {
    let mut cloned_req = req.clone();
    cloned_req.request_id = request_id.to_string();
    match claim_or_deduplicate_request(&cloned_req, canonical_ws, effective_cli)? {
        R3RegistryDecision::Created { .. }
        | R3RegistryDecision::InProgress { .. }
        | R3RegistryDecision::ExistingTerminal { .. } => Ok(()),
        R3RegistryDecision::Conflict { message } => Err(message),
        R3RegistryDecision::WorkspaceBusy { message } => Err(message),
        R3RegistryDecision::FailedRetryWithNewRequestId { message } => Err(message),
        R3RegistryDecision::RecoveryRequired { message } => Err(message),
    }
}

// ---------------------------------------------------------------------------
// 7. R3 Controller Execution Engine (§11.2, A01, §16.4, §16.5)
// ---------------------------------------------------------------------------

pub fn execute_r3_start_run<W: Write>(
    req: R3StartRunRequest,
    event_writer: &mut W,
) -> Result<AutoConvergeRunState, String> {
    // 1. Non-Windows Platform Gate (§16.12.2 A, §16.12.4 item 1)
    if !is_r3_supported_platform() {
        return Err(
            "UnsupportedPlatform: R3 Controller execution is only supported on Windows. Current platform is unsupported."
                .to_string(),
        );
    }

    // 2. Pure Validation (no workspace lock, no registry claim, no files created) (§16.12.2 A, §16.12.4 item 2)
    let val = r3_pure_validation(&req)?;

    // 3. Test Seam Barrier (after pure validation, before claim transaction) (§16.12.2 C, §16.13.1)
    maybe_wait_r3_test_claim_barrier(&req.request_id);

    // 4. Registry Claim Decision Transaction (§16.12.2 A, B)
    let effective_cli = val
        .resolved_cli_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .or_else(|| resolve_effective_agent_cli(&req));
    let decision =
        claim_or_deduplicate_request(&req, &val.canonical_workspace, effective_cli.as_deref())?;

    let (assigned_run_id, _claim_id) = match decision {
        R3RegistryDecision::Created { run_id, claim_id } => (run_id, claim_id),
        R3RegistryDecision::ExistingTerminal { state } => {
            let start_ev = R3RunEvent::Start {
                run_id: state.run_id.clone(),
                request_id: req.request_id.clone(),
                workspace_root: req.workspace_root.clone(),
                mode: req.mode.clone(),
            };
            let _ = writeln!(
                event_writer,
                "{}",
                serde_json::to_string(&start_ev).unwrap_or_default()
            );
            let res_ev = R3RunEvent::Result {
                run_id: state.run_id.clone(),
                status: "already_completed_or_active".to_string(),
                state: state.clone(),
            };
            let _ = writeln!(
                event_writer,
                "{}",
                serde_json::to_string(&res_ev).unwrap_or_default()
            );
            return Ok(state);
        }
        R3RegistryDecision::InProgress { existing_run_id } => {
            let start_ev = R3RunEvent::Start {
                run_id: existing_run_id.clone(),
                request_id: req.request_id.clone(),
                workspace_root: req.workspace_root.clone(),
                mode: req.mode.clone(),
            };
            let _ = writeln!(
                event_writer,
                "{}",
                serde_json::to_string(&start_ev).unwrap_or_default()
            );
            return Err(format!(
                "InProgress: Request '{}' is already in progress in run '{}'",
                req.request_id, existing_run_id
            ));
        }
        R3RegistryDecision::Conflict { message } => return Err(message),
        R3RegistryDecision::WorkspaceBusy { message } => return Err(message),
        R3RegistryDecision::FailedRetryWithNewRequestId { message } => return Err(message),
        R3RegistryDecision::RecoveryRequired { message } => return Err(message),
    };

    // 5. Run-Scoped Setup: only Created(run_id) reaches here (§16.12.2 A, §16.12.4 item 4)
    // Acquire workspace run lock with the claimed run ID
    let _ws_lock = match acquire_workspace_run_lock(&val.canonical_workspace, &assigned_run_id) {
        Ok(lock) => {
            if std::env::var("POVERELLO_TEST_R3_BARRIER_DIR").is_ok() {
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            lock
        }
        Err(err) => {
            let is_busy = err.contains("Sharing violation")
                || err.contains("os error 32")
                || err.contains("already locked by another running process");
            let update_res = update_claim_status(&req.request_id, R3ClaimStatus::Failed);
            if let Err(e) = update_res {
                return Err(format!(
                    "RECOVERY_REQUIRED: Failed acquiring workspace lock ({err}) and failed updating claim status: {e}"
                ));
            }
            if is_busy {
                return Err(format!(
                    "WorkspaceBusy: Workspace '{:?}' is already locked by another running process: {err}. Retry with a new request_id.",
                    val.canonical_workspace
                ));
            } else {
                return Err(format!("Failed acquiring workspace lock: {err}"));
            }
        }
    };

    // Transition claim to Starting
    if let Err(e) = update_claim_status(&req.request_id, R3ClaimStatus::Starting) {
        return Err(format!(
            "RECOVERY_REQUIRED: Failed transitioning claim to Starting: {e}"
        ));
    }

    // Establish Run Owner Identity
    let creation_time = get_current_process_creation_time()?;
    let owner = RunOwnerIdentity {
        run_id: assigned_run_id.clone(),
        owner_nonce: uuid::Uuid::new_v4().to_string(),
        process_id: std::process::id(),
        process_creation_time: creation_time,
        created_at: chrono::Utc::now().to_rfc3339(),
        canonical_workspace: val.canonical_workspace.to_string_lossy().to_string(),
    };
    if let Err(e) = record_run_owner_identity(&owner) {
        let _ = update_claim_status(&req.request_id, R3ClaimStatus::Failed);
        return Err(format!(
            "RECOVERY_REQUIRED: Failed recording owner identity: {e}"
        ));
    }

    // Initialize Run State
    let config = AutoConvergeRunConfig {
        run_id: Some(assigned_run_id.clone()),
        working_dir: val.canonical_workspace.to_string_lossy().to_string(),
        repo_remote_url: "origin".to_string(),
        target_branch: "main".to_string(),
        profile: req.profile.clone(),
        run_permissions: req.run_permissions,
        release_intent: None,
        force_escalation: false,
        require_environment_verification: true,
        delivery_phase: super::types::DeliveryPhase::PhaseCTagAndRelease,
        agent_cli_path: req.agent_cli_path.clone(),
        reviewer_level: req.reviewer_level,
        r0d_acknowledged: req.r0d_acknowledged,
    };

    let manifest = capture_baseline_file_manifest(&val.canonical_workspace, &BTreeMap::new())?;
    let git_runner = ProductionGitCommandRunner::new(val.canonical_workspace.clone());
    let now = chrono::Utc::now().to_rfc3339();

    let mut state = initialize_run(config, "HEAD", manifest, &git_runner, &now)?;
    state.effective_permissions = val.effective_permissions;

    let state_path = engine_run_state_path(&state.run_id)?;
    let _run_state_lock = acquire_run_state_lock(&state_path);

    if let Err(e) = save_run_state_authenticated(&state_path, &mut state) {
        let _ = update_claim_status(&req.request_id, R3ClaimStatus::Failed);
        return Err(format!(
            "RECOVERY_REQUIRED: Failed saving initial authenticated run state: {e}"
        ));
    }

    // Transition claim to Started
    if let Err(e) = update_claim_status(&req.request_id, R3ClaimStatus::Started) {
        return Err(format!(
            "RECOVERY_REQUIRED: Failed transitioning claim to Started: {e}"
        ));
    }

    // Emit Start Event ONLY after owner/state persistence and claim transition to Started (§16.13.2 item 1, §16.13.3 item 7)
    let start_ev = R3RunEvent::Start {
        run_id: assigned_run_id.clone(),
        request_id: req.request_id.clone(),
        workspace_root: val.canonical_workspace.to_string_lossy().to_string(),
        mode: req.mode.clone(),
    };
    let _ = writeln!(
        event_writer,
        "{}",
        serde_json::to_string(&start_ev).unwrap_or_default()
    );

    // 5. Select Runner: Unit 1 Gate (Fake Test Seam ONLY; real dispatch prohibited, §16.4 item 3)
    let mock_runner_opt = get_r3_test_fake_runner();
    if mock_runner_opt.is_none() {
        return Err(
            "Preflight rejected: Unit 1 only permits Fake test runner execution; real role dispatch is prohibited"
                .to_string(),
        );
    }
    let mock_runner = mock_runner_opt.unwrap();
    let is_plan_only = req.mode == "plan_only";

    // Step-by-step execution with mailbox checking
    let max_steps = if is_plan_only { 10 } else { 30 };

    for step_idx in 0..max_steps {
        // Check control mailbox before step (§5.2, §11.2, A11)
        let active_run_id = state.run_id.clone();
        if let Some(action) =
            owner_process_pending_mailbox_requests(&active_run_id, &mut state, &state_path)?
        {
            if action == R3ControlAction::Pause || action == R3ControlAction::Cancel {
                let q_ev = R3RunEvent::Quiescent {
                    run_id: state.run_id.clone(),
                    current_state: state.current_state,
                    is_terminal: state.is_terminal,
                };
                let _ = writeln!(
                    event_writer,
                    "{}",
                    serde_json::to_string(&q_ev).unwrap_or_default()
                );
                break;
            }
        }

        if state.is_terminal || state.is_paused || state.current_state == AutoConvergeState::Blocked
        {
            break;
        }

        // Emit progress
        let prog_ev = R3RunEvent::Progress {
            run_id: state.run_id.clone(),
            step: step_idx + 1,
            current_state: state.current_state,
            message: format!("Running step in state {:?}", state.current_state),
        };
        let _ = writeln!(
            event_writer,
            "{}",
            serde_json::to_string(&prog_ev).unwrap_or_default()
        );

        super::run_phase_a_step(&mut state, mock_runner.as_ref(), &git_runner, &state_path)?;

        // If plan_only mode and reached PlanApproved, complete cleanly (§12, A12)
        if is_plan_only && state.current_state == AutoConvergeState::PlanApproved {
            state.is_terminal = true;
            save_run_state_authenticated(&state_path, &mut state)?;
            break;
        }
    }

    // Quiescent event
    let q_ev = R3RunEvent::Quiescent {
        run_id: state.run_id.clone(),
        current_state: state.current_state,
        is_terminal: state.is_terminal,
    };
    let _ = writeln!(
        event_writer,
        "{}",
        serde_json::to_string(&q_ev).unwrap_or_default()
    );

    // Result event (§5.2, A01)
    let res_ev = R3RunEvent::Result {
        run_id: state.run_id.clone(),
        status: if state.is_terminal {
            "terminal".to_string()
        } else if state.is_paused {
            "paused".to_string()
        } else {
            "blocked".to_string()
        },
        state: state.clone(),
    };
    let _ = writeln!(
        event_writer,
        "{}",
        serde_json::to_string(&res_ev).unwrap_or_default()
    );

    Ok(state)
}

// ---------------------------------------------------------------------------
// 8. CLI Entrypoint & Subcommands (§5.2, §11.2, §16.4, §16.5)
// ---------------------------------------------------------------------------

pub fn is_auto_converge_cli_mode(args: &[String]) -> bool {
    args.get(1).map(|s| s.as_str()) == Some("auto-converge")
}

pub fn run_auto_converge_cli(args: &[String]) -> i32 {
    let subcommand = match args.get(2).map(|s| s.as_str()) {
        Some(s) => s,
        None => {
            eprintln!(
                "{}",
                json!({ "status": "error", "message": "Missing auto-converge subcommand (run, status, pause, cancel, resume)" })
            );
            return 1;
        }
    };

    let mut request_path: Option<String> = None;
    let mut run_id: Option<String> = None;
    let mut json_mode = false;

    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--request" => {
                request_path = args.get(i + 1).cloned();
                i += 2;
            }
            "--run-id" => {
                run_id = args.get(i + 1).cloned();
                i += 2;
            }
            "--json" => {
                json_mode = true;
                i += 1;
            }
            other => {
                eprintln!(
                    "{}",
                    json!({ "status": "error", "message": format!("Unknown flag: {other}") })
                );
                return 1;
            }
        }
    }

    match subcommand {
        "run" => {
            let req_file = match request_path {
                Some(p) => p,
                None => {
                    eprintln!(
                        "{}",
                        json!({ "status": "error", "message": "Missing --request <file>" })
                    );
                    return 1;
                }
            };
            let bytes = match fs::read(&req_file) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!(
                        "{}",
                        json!({ "status": "error", "message": format!("Failed reading request file: {e}") })
                    );
                    return 1;
                }
            };
            let req: R3StartRunRequest = match serde_json::from_slice(&bytes) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!(
                        "{}",
                        json!({ "status": "error", "message": format!("Invalid request JSON schema: {e}") })
                    );
                    return 1;
                }
            };

            let mut stdout = std::io::stdout();
            match execute_r3_start_run(req, &mut stdout) {
                Ok(_state) => 0,
                Err(e) => {
                    let err_ev = R3RunEvent::Error {
                        run_id: None,
                        error: e,
                    };
                    println!("{}", serde_json::to_string(&err_ev).unwrap_or_default());
                    1
                }
            }
        }
        "status" => {
            let rid = match run_id {
                Some(r) => r,
                None => {
                    eprintln!(
                        "{}",
                        json!({ "status": "error", "message": "Missing --run-id" })
                    );
                    return 1;
                }
            };
            if let Err(e) = validate_r3_identifier(&rid, "run-id") {
                eprintln!("{}", json!({ "status": "error", "message": e }));
                return 1;
            }
            match engine_run_state_path(&rid) {
                Ok(state_path) if state_path.exists() => {
                    match load_and_verify_run_state_authenticated(&state_path, None) {
                        Ok(state) => {
                            // Check owner liveness if active (§16.4 item 8)
                            if !state.is_terminal {
                                match read_run_owner_identity(&rid) {
                                    Ok(owner) => {
                                        if !is_owner_process_alive(&owner) {
                                            eprintln!(
                                                "{}",
                                                json!({
                                                    "status": "error",
                                                    "reason": "RECOVERY_REQUIRED",
                                                    "message": format!("Run owner process {} for active run '{}' has disappeared", owner.process_id, rid)
                                                })
                                            );
                                            return 2;
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "{}",
                                            json!({
                                                "status": "error",
                                                "reason": "RECOVERY_REQUIRED",
                                                "message": format!("Failed reading owner identity for active run '{}': {e}", rid)
                                            })
                                        );
                                        return 2;
                                    }
                                }
                            }

                            if json_mode {
                                println!(
                                    "{}",
                                    json!({ "status": "ok", "run_id": state.run_id, "current_state": state.current_state, "is_terminal": state.is_terminal, "is_paused": state.is_paused, "state": state })
                                );
                            } else {
                                println!(
                                    "Run ID: {}\nState: {:?}\nTerminal: {}\nPaused: {}",
                                    state.run_id,
                                    state.current_state,
                                    state.is_terminal,
                                    state.is_paused
                                );
                            }
                            0
                        }
                        Err(e) => {
                            eprintln!(
                                "{}",
                                json!({ "status": "error", "message": format!("Failed authenticating run state: {e}") })
                            );
                            1
                        }
                    }
                }
                Ok(_) => {
                    eprintln!(
                        "{}",
                        json!({ "status": "error", "message": format!("Run state for '{rid}' not found") })
                    );
                    1
                }
                Err(e) => {
                    eprintln!("{}", json!({ "status": "error", "message": e }));
                    1
                }
            }
        }
        "pause" => {
            let rid = match run_id {
                Some(r) => r,
                None => {
                    eprintln!(
                        "{}",
                        json!({ "status": "error", "message": "Missing --run-id" })
                    );
                    return 1;
                }
            };
            if let Err(e) = validate_r3_identifier(&rid, "run-id") {
                eprintln!("{}", json!({ "status": "error", "message": e }));
                return 1;
            }
            let ctrl_req = R3ControlRequest {
                request_id: uuid::Uuid::new_v4().to_string(),
                run_id: rid.clone(),
                action: R3ControlAction::Pause,
                issued_at: chrono::Utc::now().to_rfc3339(),
            };
            if let Err(e) = send_control_request(&ctrl_req) {
                eprintln!("{}", json!({ "status": "error", "message": e }));
                return 1;
            }
            match poll_control_response(&rid, &ctrl_req.request_id, Duration::from_secs(5)) {
                Ok(resp) => {
                    println!("{}", serde_json::to_string(&resp).unwrap_or_default());
                    if resp.accepted {
                        0
                    } else {
                        1
                    }
                }
                Err(e) => {
                    let exit_code = if e.contains("RECOVERY_REQUIRED") {
                        2
                    } else {
                        1
                    };
                    eprintln!("{}", json!({ "status": "error", "message": e }));
                    exit_code
                }
            }
        }
        "cancel" => {
            let rid = match run_id {
                Some(r) => r,
                None => {
                    eprintln!(
                        "{}",
                        json!({ "status": "error", "message": "Missing --run-id" })
                    );
                    return 1;
                }
            };
            if let Err(e) = validate_r3_identifier(&rid, "run-id") {
                eprintln!("{}", json!({ "status": "error", "message": e }));
                return 1;
            }
            let ctrl_req = R3ControlRequest {
                request_id: uuid::Uuid::new_v4().to_string(),
                run_id: rid.clone(),
                action: R3ControlAction::Cancel,
                issued_at: chrono::Utc::now().to_rfc3339(),
            };
            if let Err(e) = send_control_request(&ctrl_req) {
                eprintln!("{}", json!({ "status": "error", "message": e }));
                return 1;
            }
            match poll_control_response(&rid, &ctrl_req.request_id, Duration::from_secs(5)) {
                Ok(resp) => {
                    println!("{}", serde_json::to_string(&resp).unwrap_or_default());
                    if resp.accepted {
                        0
                    } else {
                        1
                    }
                }
                Err(e) => {
                    let exit_code = if e.contains("RECOVERY_REQUIRED") {
                        2
                    } else {
                        1
                    };
                    eprintln!("{}", json!({ "status": "error", "message": e }));
                    exit_code
                }
            }
        }
        "resume" => {
            let rid = match run_id {
                Some(r) => r,
                None => {
                    eprintln!(
                        "{}",
                        json!({ "status": "error", "message": "Missing --run-id" })
                    );
                    return 1;
                }
            };
            if let Err(e) = validate_r3_identifier(&rid, "run-id") {
                eprintln!("{}", json!({ "status": "error", "message": e }));
                return 1;
            }
            let ctrl_req = R3ControlRequest {
                request_id: uuid::Uuid::new_v4().to_string(),
                run_id: rid.clone(),
                action: R3ControlAction::Resume,
                issued_at: chrono::Utc::now().to_rfc3339(),
            };
            if let Err(e) = send_control_request(&ctrl_req) {
                eprintln!("{}", json!({ "status": "error", "message": e }));
                return 1;
            }
            match poll_control_response(&rid, &ctrl_req.request_id, Duration::from_secs(5)) {
                Ok(resp) => {
                    println!("{}", serde_json::to_string(&resp).unwrap_or_default());
                    if resp.accepted {
                        0
                    } else {
                        1
                    }
                }
                Err(e) => {
                    let exit_code = if e.contains("RECOVERY_REQUIRED") {
                        2
                    } else {
                        1
                    };
                    eprintln!("{}", json!({ "status": "error", "message": e }));
                    exit_code
                }
            }
        }
        other => {
            eprintln!(
                "{}",
                json!({ "status": "error", "message": format!("Unknown auto-converge subcommand '{other}'") })
            );
            1
        }
    }
}
