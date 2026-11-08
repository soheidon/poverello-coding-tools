use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::orchestrator::process_runner::ProcessRunner;
use crate::paths;

static CLI_MUTEX: Mutex<()> = Mutex::const_new(());
static TEST_RUNNER_OVERRIDE_ENABLED: AtomicBool = AtomicBool::new(false);
static TEST_MOCK_VERSIONS: StdMutex<Option<HashMap<String, String>>> = StdMutex::new(None);

static CLI_TOOLS_CACHE: StdMutex<Option<(Instant, Vec<CliToolInfo>)>> = StdMutex::new(None);
static CLI_TOOLS_GATE: Mutex<()> = Mutex::const_new(());

static NODE_PREREQS_CACHE: StdMutex<Option<(Instant, NodePrerequisiteStatus)>> = StdMutex::new(None);
static NODE_PREREQS_GATE: Mutex<()> = Mutex::const_new(());

static AUTH_STATUS_CACHE: StdMutex<Option<(Instant, Vec<CliAuthInfo>)>> = StdMutex::new(None);
static AUTH_STATUS_GATE: Mutex<()> = Mutex::const_new(());

static CODEX_ACCOUNT_CACHE: StdMutex<Option<(Instant, CodexAccountInfo)>> = StdMutex::new(None);
static CODEX_ACCOUNT_GATE: Mutex<()> = Mutex::const_new(());

pub const CLI_TOOLS_CACHE_TTL: Duration = Duration::from_secs(60);
pub const NODE_PREREQS_CACHE_TTL: Duration = Duration::from_secs(60);
pub const AUTH_STATUS_CACHE_TTL: Duration = Duration::from_secs(30);
pub const CODEX_ACCOUNT_CACHE_TTL: Duration = Duration::from_secs(30);

pub fn invalidate_all_cli_caches() {
    invalidate_cli_tools_cache();
    invalidate_prerequisites_cache();
    invalidate_auth_caches();
    let mut guard = LATEST_CACHE.lock().unwrap();
    *guard = None;
}

pub fn invalidate_cli_tools_cache() {
    let mut guard = CLI_TOOLS_CACHE.lock().unwrap();
    *guard = None;
}

pub fn invalidate_prerequisites_cache() {
    let mut guard = NODE_PREREQS_CACHE.lock().unwrap();
    *guard = None;
}

pub fn invalidate_auth_caches() {
    {
        let mut guard = AUTH_STATUS_CACHE.lock().unwrap();
        *guard = None;
    }
    {
        let mut guard = CODEX_ACCOUNT_CACHE.lock().unwrap();
        *guard = None;
    }
}

pub fn set_test_runner_override(enabled: bool, mock_versions: Option<HashMap<String, String>>) {
    TEST_RUNNER_OVERRIDE_ENABLED.store(enabled, Ordering::SeqCst);
    let mut guard = TEST_MOCK_VERSIONS.lock().unwrap();
    *guard = mock_versions;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CliToolStatus {
    #[serde(rename = "not_installed")]
    NotInstalled,
    #[serde(rename = "installed")]
    Installed,
    #[serde(rename = "execution_error")]
    ExecutionError,
    #[serde(rename = "unknown")]
    Unknown,
}

pub fn default_cli_tool_status() -> CliToolStatus {
    CliToolStatus::NotInstalled
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CliToolInfo {
    pub name: String,
    pub display_name: String,
    pub installed: bool,
    pub path: Option<String>,
    pub installed_version: Option<String>,
    pub latest_version: Option<String>,
    pub latest_checked_at: Option<String>,
    pub check_id: Option<String>,
    pub package_name: String,
    pub is_preview: bool,
    pub update_available: bool,
    pub prerequisite_error: Option<String>,
    #[serde(default)]
    pub execution_error: Option<String>,
    #[serde(default)]
    pub latest_check_error: Option<String>,
    #[serde(default)]
    pub outcome_unknown: Option<String>,
    #[serde(default = "default_cli_tool_status")]
    pub status: CliToolStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CliAuthInfo {
    pub name: String,
    pub status: String, // "authenticated" | "not_authenticated" | "unknown"
    pub auth_type: Option<String>,
    pub message: Option<String>,
    pub can_launch_login: bool,
    #[serde(default)]
    pub account_identity: Option<String>,
    #[serde(default)]
    pub codex_home: Option<String>,
    #[serde(default)]
    pub last_checked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodePrerequisiteStatus {
    pub node_installed: bool,
    pub node_version: Option<String>,
    pub node_major_version: Option<u32>,
    pub node_path: Option<String>,
    pub npm_installed: bool,
    pub npm_version: Option<String>,
    pub npm_path: Option<String>,
    pub node_npm_ready: bool,
    pub gemini_node_ready: bool,
    pub message: Option<String>,
    #[serde(default)]
    pub npm_global_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexAccountInfo {
    pub installed: bool,
    pub executable_path: Option<String>,
    pub version: Option<String>,
    pub auth_method: Option<String>,
    pub account_identity: Option<String>,
    pub config_profile: Option<String>,
    pub requires_openai_auth: Option<bool>,
    #[serde(default)]
    pub codex_home: Option<String>,
    #[serde(default)]
    pub last_checked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CliToolsState {
    pub outcome_unknown: Option<CliToolOutcomeUnknown>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CliToolOutcomeUnknown {
    pub tool_name: String,
    pub message: String,
    pub recorded_at: String,
}

#[derive(Debug, Clone)]
struct EphemeralLatestCache {
    check_id: String,
    checked_at: Instant,
    checked_at_rfc3339: String,
    versions: HashMap<String, String>,
    errors: HashMap<String, String>,
    node_path: Option<String>,
    node_version: Option<String>,
    npm_path: Option<String>,
    npm_version: Option<String>,
    cli_paths: HashMap<String, Option<String>>,
}

static LATEST_CACHE: StdMutex<Option<EphemeralLatestCache>> = StdMutex::new(None);

pub fn get_cli_tools_env_allowlist() -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
    let mut vars = Vec::new();
    let keys = [
        "PATH",
        "SYSTEMROOT",
        "COMSPEC",
        "PATHEXT",
        "TMP",
        "TEMP",
        "USERPROFILE",
        "HOME",
    ];
    for k in keys {
        if let Ok(v) = std::env::var(k) {
            vars.push((k.into(), v.into()));
        }
    }
    vars
}

static STATE_FILE_OVERRIDE: StdMutex<Option<PathBuf>> = StdMutex::new(None);
static PREFIX_OVERRIDE: StdMutex<Option<PathBuf>> = StdMutex::new(None);
static PREREQS_OVERRIDE: StdMutex<Option<NodePrerequisiteStatus>> = StdMutex::new(None);

pub fn cli_tools_state_path() -> PathBuf {
    if let Ok(guard) = STATE_FILE_OVERRIDE.lock() {
        if let Some(ref path) = *guard {
            return path.clone();
        }
    }
    paths::user_data_dir().join("cli_tools_state.json")
}

#[cfg(test)]
pub fn set_state_file_override(path: Option<PathBuf>) {
    let mut guard = STATE_FILE_OVERRIDE.lock().unwrap();
    *guard = path;
}

#[cfg(test)]
pub fn set_prefix_override(path: Option<PathBuf>) {
    let mut guard = PREFIX_OVERRIDE.lock().unwrap();
    *guard = path;
}

#[cfg(test)]
pub fn set_prereqs_override(status: Option<NodePrerequisiteStatus>) {
    let mut guard = PREREQS_OVERRIDE.lock().unwrap();
    *guard = status;
}

pub fn get_effective_npm_prefix(prereq_prefix: Option<&str>) -> Option<PathBuf> {
    if let Ok(guard) = PREFIX_OVERRIDE.lock() {
        if let Some(ref path) = *guard {
            return Some(path.clone());
        }
    }
    prereq_prefix.map(PathBuf::from)
}

pub fn read_cli_tools_state() -> CliToolsState {
    let p = cli_tools_state_path();
    if p.exists() {
        if let Ok(bytes) = std::fs::read(&p) {
            if let Ok(state) = serde_json::from_slice::<CliToolsState>(&bytes) {
                return state;
            }
        }
    }
    CliToolsState {
        outcome_unknown: None,
    }
}

pub fn write_cli_tools_state(state: &CliToolsState) -> Result<(), String> {
    let p = cli_tools_state_path();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_vec_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(&p, data).map_err(|e| e.to_string())
}

pub fn resolve_cli_path_with_prefix(name: &str, npm_prefix: Option<&Path>) -> Option<PathBuf> {
    // 1. Check custom npm_prefix first (if provided)
    if let Some(prefix) = npm_prefix {
        #[cfg(windows)]
        {
            let cmd_path = prefix.join(format!("{name}.cmd"));
            if cmd_path.is_file() {
                return Some(cmd_path);
            }
            let exe_path = prefix.join(format!("{name}.exe"));
            if exe_path.is_file() {
                return Some(exe_path);
            }
            let bat_path = prefix.join(format!("{name}.bat"));
            if bat_path.is_file() {
                return Some(bat_path);
            }
        }
        #[cfg(not(windows))]
        {
            let bin_path = prefix.join("bin").join(name);
            if bin_path.is_file() {
                return Some(bin_path);
            }
            let direct = prefix.join(name);
            if direct.is_file() {
                return Some(direct);
            }
        }
        // If an explicit test prefix override is active, do not leak host PATH / APPDATA
        let is_test_override = {
            let guard = PREFIX_OVERRIDE.lock().unwrap();
            guard.is_some()
        };
        if is_test_override {
            return None;
        }
    }

    // 2. Check npm global directory on Windows
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let npm_dir = PathBuf::from(appdata).join("npm");
            let cmd_path = npm_dir.join(format!("{name}.cmd"));
            if cmd_path.is_file() {
                return Some(cmd_path);
            }
            let exe_path = npm_dir.join(format!("{name}.exe"));
            if exe_path.is_file() {
                return Some(exe_path);
            }
            let bat_path = npm_dir.join(format!("{name}.bat"));
            if bat_path.is_file() {
                return Some(bat_path);
            }
        }
    }

    // 3. Scan PATH
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            #[cfg(windows)]
            {
                let cmd = dir.join(format!("{name}.cmd"));
                if cmd.is_file() {
                    return Some(cmd);
                }
                let exe = dir.join(format!("{name}.exe"));
                if exe.is_file() {
                    return Some(exe);
                }
                let bat = dir.join(format!("{name}.bat"));
                if bat.is_file() {
                    return Some(bat);
                }
            }
            #[cfg(not(windows))]
            {
                let bin = dir.join(name);
                if bin.is_file() {
                    return Some(bin);
                }
            }
        }
    }

    None
}

pub fn resolve_cli_path(name: &str) -> Option<PathBuf> {
    let prefix = {
        let guard = NODE_PREREQS_CACHE.lock().unwrap();
        guard
            .as_ref()
            .and_then(|(_, p)| p.npm_global_prefix.as_deref().map(PathBuf::from))
    };
    resolve_cli_path_with_prefix(name, prefix.as_deref())
}

pub fn resolve_node_path() -> Result<PathBuf, String> {
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            #[cfg(windows)]
            {
                let exe = dir.join("node.exe");
                if exe.is_file() {
                    return Ok(exe);
                }
                let cmd = dir.join("node.cmd");
                if cmd.is_file() {
                    return Ok(cmd);
                }
            }
            #[cfg(not(windows))]
            {
                let bin = dir.join("node");
                if bin.is_file() {
                    return Ok(bin);
                }
            }
        }
    }
    Err("Node.js executable ('node') not found on system PATH.".to_string())
}

pub fn resolve_npm_path() -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let npm_cmd = PathBuf::from(appdata).join("npm").join("npm.cmd");
            if npm_cmd.is_file() {
                return Ok(npm_cmd);
            }
        }
    }

    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            #[cfg(windows)]
            {
                let cmd = dir.join("npm.cmd");
                if cmd.is_file() {
                    return Ok(cmd);
                }
                let exe = dir.join("npm.exe");
                if exe.is_file() {
                    return Ok(exe);
                }
            }
            #[cfg(not(windows))]
            {
                let bin = dir.join("npm");
                if bin.is_file() {
                    return Ok(bin);
                }
            }
        }
    }

    Err(
        "npm executable not found on system PATH. Please ensure Node.js and npm are installed."
            .to_string(),
    )
}

pub async fn query_node_version(path: &Path) -> Result<(String, u32), String> {
    let runner = ProcessRunner::new();
    let res = runner
        .run(
            &path.to_string_lossy(),
            &["-v".to_string()],
            None,
            Duration::from_secs(5),
            None,
        )
        .await?;

    if res.exit_code == Some(0) {
        let raw = res.stdout.text.trim().to_string();
        let cleaned = raw.trim_start_matches('v').trim();
        let major = cleaned
            .split('.')
            .next()
            .and_then(|m| m.parse::<u32>().ok())
            .unwrap_or(0);
        Ok((cleaned.to_string(), major))
    } else {
        let diag = crate::orchestrator::process_runner::format_exit_code_diagnostic(res.exit_code);
        Err(format!("'node -v' failed with {}: {}", diag, res.stderr.text))
    }
}

pub async fn query_npm_version(path: &Path) -> Result<String, String> {
    let runner = ProcessRunner::new();
    let res = runner
        .run(
            &path.to_string_lossy(),
            &["-v".to_string()],
            None,
            Duration::from_secs(5),
            None,
        )
        .await?;

    if res.exit_code == Some(0) {
        Ok(res.stdout.text.trim().to_string())
    } else {
        let diag = crate::orchestrator::process_runner::format_exit_code_diagnostic(res.exit_code);
        Err(format!("'npm -v' failed with {}: {}", diag, res.stderr.text))
    }
}

pub async fn query_npm_global_prefix(npm_path: &Path) -> Option<PathBuf> {
    let runner = ProcessRunner::new();
    let env_allowlist = get_cli_tools_env_allowlist();
    let res = runner
        .run_with_stdin_in_environment(
            &npm_path.to_string_lossy(),
            &["prefix".to_string(), "-g".to_string()],
            None,
            None,
            Duration::from_secs(5),
            None,
            &env_allowlist,
        )
        .await
        .ok()?;

    if res.exit_code == Some(0) {
        let p_str = res.stdout.text.trim();
        if !p_str.is_empty() {
            let p = PathBuf::from(p_str);
            if p.is_dir() {
                return Some(p);
            }
        }
    }
    None
}

pub async fn get_prerequisites_status() -> NodePrerequisiteStatus {
    get_prerequisites_status_cached(false).await
}

pub async fn get_prerequisites_status_cached(force_refresh: bool) -> NodePrerequisiteStatus {
    {
        if let Ok(guard) = PREREQS_OVERRIDE.lock() {
            if let Some(ref override_status) = *guard {
                return override_status.clone();
            }
        }
    }

    if !force_refresh {
        let guard = NODE_PREREQS_CACHE.lock().unwrap();
        if let Some((instant, ref status)) = *guard {
            if instant.elapsed() < NODE_PREREQS_CACHE_TTL {
                return status.clone();
            }
        }
    }

    let _gate = NODE_PREREQS_GATE.lock().await;

    if !force_refresh {
        let guard = NODE_PREREQS_CACHE.lock().unwrap();
        if let Some((instant, ref status)) = *guard {
            if instant.elapsed() < NODE_PREREQS_CACHE_TTL {
                return status.clone();
            }
        }
    }

    let status = get_prerequisites_status_uncached().await;

    {
        let mut guard = NODE_PREREQS_CACHE.lock().unwrap();
        *guard = Some((Instant::now(), status.clone()));
    }

    status
}

pub async fn get_prerequisites_status_uncached() -> NodePrerequisiteStatus {
    let node_path_res = resolve_node_path();
    let npm_path_res = resolve_npm_path();

    let mut node_installed = false;
    let mut node_version = None;
    let mut node_major_version = None;
    let mut node_path = None;

    if let Ok(np) = node_path_res {
        node_path = Some(np.to_string_lossy().to_string());
        if let Ok((ver, major)) = query_node_version(&np).await {
            node_installed = true;
            node_version = Some(ver);
            node_major_version = Some(major);
        }
    }

    let mut npm_installed = false;
    let mut npm_version = None;
    let mut npm_path = None;
    let mut npm_global_prefix = None;

    if let Ok(np) = npm_path_res {
        npm_path = Some(np.to_string_lossy().to_string());
        if let Ok(ver) = query_npm_version(&np).await {
            npm_installed = true;
            npm_version = Some(ver);
            if let Some(prefix) = query_npm_global_prefix(&np).await {
                npm_global_prefix = Some(prefix.to_string_lossy().to_string());
            }
        }
    }

    let node_npm_ready = node_installed && npm_installed;
    let gemini_node_ready = node_npm_ready && node_major_version.unwrap_or(0) >= 20;

    let message = if !node_installed {
        Some("Node.js is not installed or not found on PATH.".to_string())
    } else if !npm_installed {
        Some("npm is not installed or not found on PATH.".to_string())
    } else if !gemini_node_ready {
        Some(format!(
            "Node.js v{} found. Gemini CLI requires Node.js >= 20.",
            node_version.as_deref().unwrap_or("unknown")
        ))
    } else {
        None
    };

    NodePrerequisiteStatus {
        node_installed,
        node_version,
        node_major_version,
        node_path,
        npm_installed,
        npm_version,
        npm_path,
        node_npm_ready,
        gemini_node_ready,
        message,
        npm_global_prefix,
    }
}

pub async fn query_cli_version(path: &Path) -> Result<String, String> {
    let runner = ProcessRunner::new();
    let res = runner
        .run(
            &path.to_string_lossy(),
            &["--version".to_string()],
            None,
            Duration::from_secs(5),
            None,
        )
        .await?;

    if res.exit_code == Some(0) {
        let raw = res.stdout.text.trim();
        let ver = if let Some(last) = raw.split_whitespace().last() {
            last.to_string()
        } else {
            raw.to_string()
        };
        Ok(ver)
    } else {
        let diag = crate::orchestrator::process_runner::format_exit_code_diagnostic(res.exit_code);
        Err(format!(
            "Process failed with {}: {}",
            diag, res.stderr.text
        ))
    }
}

fn compare_semver_update(installed: &str, latest: &str) -> bool {
    let inst_clean = installed.trim_start_matches('v').trim();
    let latest_clean = latest.trim_start_matches('v').trim();

    if let (Ok(v_inst), Ok(v_lat)) = (
        semver::Version::parse(inst_clean),
        semver::Version::parse(latest_clean),
    ) {
        v_lat > v_inst
    } else {
        false
    }
}

pub async fn get_cli_tools_status() -> Result<Vec<CliToolInfo>, String> {
    get_cli_tools_status_cached(false).await
}

pub async fn get_cli_tools_status_cached(force_refresh: bool) -> Result<Vec<CliToolInfo>, String> {
    if !force_refresh {
        let guard = CLI_TOOLS_CACHE.lock().unwrap();
        if let Some((instant, ref data)) = *guard {
            if instant.elapsed() < CLI_TOOLS_CACHE_TTL {
                return Ok(data.clone());
            }
        }
    }

    let _gate = CLI_TOOLS_GATE.lock().await;

    if !force_refresh {
        let guard = CLI_TOOLS_CACHE.lock().unwrap();
        if let Some((instant, ref data)) = *guard {
            if instant.elapsed() < CLI_TOOLS_CACHE_TTL {
                return Ok(data.clone());
            }
        }
    }

    let data = get_cli_tools_status_uncached().await?;

    {
        let mut guard = CLI_TOOLS_CACHE.lock().unwrap();
        *guard = Some((Instant::now(), data.clone()));
    }

    Ok(data)
}

pub async fn get_cli_tools_status_uncached() -> Result<Vec<CliToolInfo>, String> {
    let tools = [
        ("codex", "Codex CLI", "@openai/codex", false),
        ("dsh", "DeepSeek Harness", "@deepseek-ai/dsh", true),
        ("gemini", "Gemini CLI", "@google/gemini-cli", false),
    ];

    let prereqs = get_prerequisites_status_cached(false).await;
    let npm_prefix_buf = get_effective_npm_prefix(prereqs.npm_global_prefix.as_deref());
    let npm_prefix_path = npm_prefix_buf.as_deref();

    let state = read_cli_tools_state();

    // Check cached latest versions (valid for 10 min)
    let cached_data: Option<(String, String, HashMap<String, String>, HashMap<String, String>)> = {
        let guard = LATEST_CACHE.lock().unwrap();
        guard.as_ref().and_then(|c| {
            if c.checked_at.elapsed() < Duration::from_secs(600)
                && c.node_version == prereqs.node_version
                && c.npm_version == prereqs.npm_version
            {
                Some((
                    c.checked_at_rfc3339.clone(),
                    c.check_id.clone(),
                    c.versions.clone(),
                    c.errors.clone(),
                ))
            } else {
                None
            }
        })
    };

    let mut result = Vec::new();

    for (name, display_name, pkg, is_preview) in tools {
        let path_opt = resolve_cli_path_with_prefix(name, npm_prefix_path);
        let path_str = path_opt.as_ref().map(|p| p.to_string_lossy().to_string());

        let mut installed = false;
        let mut installed_version = None;
        let mut execution_error = None;

        if let Some(ref p) = path_opt {
            match query_cli_version(p).await {
                Ok(ver) => {
                    installed = true;
                    installed_version = Some(ver);
                }
                Err(e) => {
                    installed = false;
                    execution_error = Some(e);
                }
            }
        }

        let mut prerequisite_error = None;
        if !prereqs.node_npm_ready {
            prerequisite_error = Some("Node.js and npm are required.".to_string());
        } else if name == "gemini" && !prereqs.gemini_node_ready {
            prerequisite_error = Some(format!(
                "Node.js >= 20 is required (found: v{}).",
                prereqs.node_version.as_deref().unwrap_or("unknown")
            ));
        }

        let mut latest_version = None;
        let mut latest_check_error = None;
        let mut latest_checked_at = None;
        let mut check_id = None;
        let mut update_available = false;

        if let Some((ref checked_at_rfc, ref chk_id, ref versions_map, ref errors_map)) = cached_data {
            if let Some(lat) = versions_map.get(name) {
                latest_version = Some(lat.clone());
                latest_checked_at = Some(checked_at_rfc.clone());
                check_id = Some(chk_id.clone());
                if let Some(inst) = &installed_version {
                    update_available = compare_semver_update(inst, lat);
                }
            } else if let Some(err) = errors_map.get(name) {
                latest_check_error = Some(err.clone());
                latest_checked_at = Some(checked_at_rfc.clone());
                check_id = Some(chk_id.clone());
            }
        }

        let outcome_unknown = if let Some(ref unk) = state.outcome_unknown {
            if unk.tool_name == name {
                Some(unk.message.clone())
            } else {
                None
            }
        } else {
            None
        };

        let status = if outcome_unknown.is_some() {
            CliToolStatus::Unknown
        } else if execution_error.is_some() {
            CliToolStatus::ExecutionError
        } else if installed {
            CliToolStatus::Installed
        } else {
            CliToolStatus::NotInstalled
        };

        result.push(CliToolInfo {
            name: name.to_string(),
            display_name: display_name.to_string(),
            installed,
            path: path_str,
            installed_version,
            latest_version,
            latest_checked_at,
            check_id,
            package_name: pkg.to_string(),
            is_preview,
            update_available,
            prerequisite_error,
            execution_error,
            latest_check_error,
            outcome_unknown,
            status,
        });
    }

    Ok(result)
}

pub async fn check_cli_tools_latest_version() -> Result<Vec<CliToolInfo>, String> {
    let _lock = CLI_MUTEX.lock().await;

    // Invalidate previous cache immediately at start of new check
    {
        let mut guard = LATEST_CACHE.lock().unwrap();
        *guard = None;
    }

    let prereqs = get_prerequisites_status().await;
    if !prereqs.node_npm_ready {
        return Err("Cannot check latest versions: Node.js and npm must be installed.".to_string());
    }

    let tools = [
        ("codex", "@openai/codex"),
        ("dsh", "@deepseek-ai/dsh"),
        ("gemini", "@google/gemini-cli"),
    ];

    let npm_prefix_path = prereqs.npm_global_prefix.as_deref().map(Path::new);
    let mut current_cli_paths = HashMap::new();
    for (name, _) in tools {
        current_cli_paths.insert(
            name.to_string(),
            resolve_cli_path_with_prefix(name, npm_prefix_path).map(|p| p.to_string_lossy().to_string()),
        );
    }

    let mut latest_results = HashMap::new();
    let mut latest_errors = HashMap::new();

    // Check if test override is active
    if TEST_RUNNER_OVERRIDE_ENABLED.load(Ordering::SeqCst) {
        let guard = TEST_MOCK_VERSIONS.lock().unwrap();
        if let Some(mocks) = guard.as_ref() {
            for (name, _) in tools {
                if let Some(ver) = mocks.get(name) {
                    latest_results.insert(name.to_string(), ver.clone());
                } else {
                    latest_errors.insert(name.to_string(), "Mock update check failure".to_string());
                }
            }
        }
    } else {
        let npm_path = resolve_npm_path()?;
        let env_allowlist = get_cli_tools_env_allowlist();

        for (name, pkg) in tools {
            if name == "gemini" && !prereqs.gemini_node_ready {
                // Skip npm check for Gemini if Node.js < 20
                continue;
            }

            // Create temporary userconfig & cache directories
            let temp_dir =
                std::env::temp_dir().join(format!("pct-npm-check-{}", uuid::Uuid::new_v4()));
            let _ = std::fs::create_dir_all(&temp_dir);
            let temp_cfg = temp_dir.join("npmrc");
            let temp_cache = temp_dir.join("cache");
            let _ = std::fs::write(&temp_cfg, "");
            let _ = std::fs::create_dir_all(&temp_cache);

            let args = vec![
                "view".to_string(),
                pkg.to_string(),
                "version".to_string(),
                "--json".to_string(),
                "--registry=https://registry.npmjs.org/".to_string(),
                format!("--userconfig={}", temp_cfg.display()),
                format!("--globalconfig={}", temp_cfg.display()),
                format!("--cache={}", temp_cache.display()),
                "--prefer-online".to_string(),
            ];

            let runner = ProcessRunner::new();
            let proc_res = runner
                .run_with_stdin_in_environment(
                    &npm_path.to_string_lossy(),
                    &args,
                    None,
                    None,
                    Duration::from_secs(15),
                    None,
                    &env_allowlist,
                )
                .await;

            // Cleanup temp dir
            let _ = std::fs::remove_dir_all(&temp_dir);

            match proc_res {
                Ok(res) => {
                    if res.exit_code == Some(0) {
                        let raw = res.stdout.text.trim();
                        let ver = if let Ok(val) = serde_json::from_str::<serde_json::Value>(raw) {
                            if let Some(s) = val.as_str() {
                                s.to_string()
                            } else {
                                raw.replace('"', "").trim().to_string()
                            }
                        } else {
                            raw.replace('"', "").trim().to_string()
                        };
                        if !ver.is_empty() {
                            latest_results.insert(name.to_string(), ver);
                        } else {
                            latest_errors.insert(
                                name.to_string(),
                                "Empty version response from npm registry.".to_string(),
                            );
                        }
                    } else {
                        let diag = crate::orchestrator::process_runner::format_exit_code_diagnostic(
                            res.exit_code,
                        );
                        let msg = if !res.stderr.text.trim().is_empty() {
                            res.stderr.text.trim().to_string()
                        } else {
                            format!("npm view failed with {diag}")
                        };
                        latest_errors.insert(name.to_string(), msg);
                    }
                }
                Err(e) => {
                    latest_errors.insert(
                        name.to_string(),
                        format!("Failed to spawn npm view: {e}"),
                    );
                }
            }
        }
    }

    let new_check_id = uuid::Uuid::new_v4().to_string();
    let now_rfc = chrono::Utc::now().to_rfc3339();
    {
        let mut guard = LATEST_CACHE.lock().unwrap();
        *guard = Some(EphemeralLatestCache {
            check_id: new_check_id,
            checked_at: Instant::now(),
            checked_at_rfc3339: now_rfc,
            versions: latest_results,
            errors: latest_errors,
            node_path: prereqs.node_path.clone(),
            node_version: prereqs.node_version.clone(),
            npm_path: prereqs.npm_path.clone(),
            npm_version: prereqs.npm_version.clone(),
            cli_paths: current_cli_paths,
        });
    }

    invalidate_cli_tools_cache();
    get_cli_tools_status_cached(true).await
}

pub async fn run_cli_install(tool_name: &str) -> Result<CliToolInfo, String> {
    let _lock = CLI_MUTEX.lock().await;

    let prereqs = get_prerequisites_status().await;
    let npm_prefix_path = prereqs.npm_global_prefix.as_deref().map(Path::new);

    // Check if already installed and functional
    if let Some(p) = resolve_cli_path_with_prefix(tool_name, npm_prefix_path) {
        if query_cli_version(&p).await.is_ok() {
            return Err(format!(
                "CLI tool '{tool_name}' is already installed. Use update instead."
            ));
        }
    }

    // Check if an outcome_unknown state is currently recorded
    let state = read_cli_tools_state();
    if let Some(ref unknown) = state.outcome_unknown {
        if unknown.tool_name == tool_name {
            return Err(format!(
                "Previous installation/update of '{}' ended with an unknown outcome: {}. Please verify the installation manually before retrying.",
                tool_name, unknown.message
            ));
        }
    }

    if !prereqs.node_npm_ready {
        return Err("Cannot install: Node.js and npm are required.".to_string());
    }

    if tool_name == "gemini" && !prereqs.gemini_node_ready {
        return Err(format!(
            "Gemini CLI requires Node.js >= 20. Found Node.js v{}.",
            prereqs.node_version.as_deref().unwrap_or("unknown")
        ));
    }

    let (pkg, display_name, is_preview) = match tool_name {
        "codex" => ("@openai/codex", "Codex CLI", false),
        "dsh" => ("@deepseek-ai/dsh", "DeepSeek Harness", true),
        "gemini" => ("@google/gemini-cli", "Gemini CLI", false),
        _ => return Err(format!("Unsupported CLI tool: '{tool_name}'")),
    };

    let npm_path = resolve_npm_path()?;
    let install_target = format!("{pkg}@latest");

    let env_allowlist = get_cli_tools_env_allowlist();

    let runner = ProcessRunner::new();
    let res = runner
        .run_with_stdin_in_environment(
            &npm_path.to_string_lossy(),
            &["install".to_string(), "-g".to_string(), install_target],
            None,
            None,
            Duration::from_secs(120),
            None,
            &env_allowlist,
        )
        .await;

    match res {
        Ok(proc_res) => {
            if proc_res.timed_out {
                #[cfg(windows)]
                {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }

                let mut new_state = state;
                new_state.outcome_unknown = Some(CliToolOutcomeUnknown {
                    tool_name: tool_name.to_string(),
                    message: "Installation process timed out after 120 seconds".to_string(),
                    recorded_at: chrono::Utc::now().to_rfc3339(),
                });
                let _ = write_cli_tools_state(&new_state);

                return Err(format!(
                    "Installation of '{tool_name}' timed out after 120 seconds. Outcome unknown."
                ));
            }

            if proc_res.exit_code != Some(0) {
                return Err(format!(
                    "npm install failed with exit code {:?}: {}",
                    proc_res.exit_code, proc_res.stderr.text
                ));
            }

            // Post-install verification (§8.2: npm package status + prefix consistency + --version check)
            if !check_npm_package_installed(npm_prefix_path, tool_name) {
                return Err(format!(
                    "Installation completed with exit code 0, but package '{pkg}' was not found in npm global prefix."
                ));
            }

            let path_opt = resolve_cli_path_with_prefix(tool_name, npm_prefix_path);
            let Some(path) = path_opt else {
                return Err(format!(
                    "Installation completed, but executable for '{tool_name}' was not found on PATH or npm global prefix."
                ));
            };

            let version = query_cli_version(&path).await.map_err(|e| {
                format!("Installation completed, but executable for '{tool_name}' failed '--version' check: {e}")
            })?;

            // Clear outcome_unknown if it was recorded
            let mut current_state = read_cli_tools_state();
            if let Some(ref unk) = current_state.outcome_unknown {
                if unk.tool_name == tool_name {
                    current_state.outcome_unknown = None;
                    let _ = write_cli_tools_state(&current_state);
                }
            }

            invalidate_all_cli_caches();

            Ok(CliToolInfo {
                name: tool_name.to_string(),
                display_name: display_name.to_string(),
                installed: true,
                path: Some(path.to_string_lossy().to_string()),
                installed_version: Some(version),
                latest_version: None,
                latest_checked_at: None,
                check_id: None,
                package_name: pkg.to_string(),
                is_preview,
                update_available: false,
                prerequisite_error: None,
                execution_error: None,
                latest_check_error: None,
                outcome_unknown: None,
                status: CliToolStatus::Installed,
            })
        }
        Err(e) => Err(format!("Process spawn error: {e}")),
    }
}

pub async fn run_cli_update(tool_name: &str, check_id: &str) -> Result<CliToolInfo, String> {
    let _lock = CLI_MUTEX.lock().await;

    // Validate check_id and retrieve exact version & cached identity FIRST
    let (exact_version, cached_node_path, cached_npm_path, cached_cli_path) = {
        let guard = LATEST_CACHE.lock().unwrap();
        let cache = guard.as_ref().ok_or_else(|| {
            "No update check has been performed. Please check for updates first.".to_string()
        })?;

        if cache.checked_at.elapsed() >= Duration::from_secs(600) {
            return Err("The update check result has expired (older than 10 minutes). Please check for updates again.".to_string());
        }

        if cache.check_id != check_id {
            return Err(
                "Invalid or superseded update check ID. Please check for updates again."
                    .to_string(),
            );
        }

        let ver = cache
            .versions
            .get(tool_name)
            .ok_or_else(|| format!("No checked version available for tool '{tool_name}'."))?
            .clone();

        let cached_cli = cache.cli_paths.get(tool_name).cloned().flatten();

        (
            ver,
            cache.node_path.clone(),
            cache.npm_path.clone(),
            cached_cli,
        )
    };

    // Check outcome_unknown
    let state = read_cli_tools_state();
    if let Some(ref unknown) = state.outcome_unknown {
        if unknown.tool_name == tool_name {
            return Err(format!(
                "Previous installation/update of '{}' ended with an unknown outcome: {}. Please verify the installation manually before retrying.",
                tool_name, unknown.message
            ));
        }
    }

    let prereqs = get_prerequisites_status().await;
    let npm_prefix_path = prereqs.npm_global_prefix.as_deref().map(Path::new);

    let existing_path = resolve_cli_path_with_prefix(tool_name, npm_prefix_path)
        .ok_or_else(|| format!("CLI tool '{tool_name}' is not installed. Use install instead."))?;

    let existing_version = query_cli_version(&existing_path)
        .await
        .map_err(|e| format!("Failed to determine installed version of '{tool_name}': {e}"))?;

    // Validate that latest version is strictly newer than installed version
    if !compare_semver_update(&existing_version, &exact_version) {
        return Err(format!(
            "Tool '{tool_name}' is already up-to-date (installed: {}, checked: {}).",
            existing_version, exact_version
        ));
    }

    // Validate identity binding: Node, npm, and CLI paths must match check time
    if !prereqs.node_npm_ready {
        return Err("Cannot update: Node.js and npm are required.".to_string());
    }

    if tool_name == "gemini" && !prereqs.gemini_node_ready {
        return Err(format!(
            "Gemini CLI requires Node.js >= 20. Found Node.js v{}.",
            prereqs.node_version.as_deref().unwrap_or("unknown")
        ));
    }

    if prereqs.node_path != cached_node_path || prereqs.npm_path != cached_npm_path {
        return Err("Node.js or npm runtime path changed since the update check was performed. Please check for updates again.".to_string());
    }

    if Some(existing_path.to_string_lossy().to_string()) != cached_cli_path {
        return Err("CLI executable path changed since the update check was performed. Please check for updates again.".to_string());
    }

    let (pkg, display_name, is_preview) = match tool_name {
        "codex" => ("@openai/codex", "Codex CLI", false),
        "dsh" => ("@deepseek-ai/dsh", "DeepSeek Harness", true),
        "gemini" => ("@google/gemini-cli", "Gemini CLI", false),
        _ => return Err(format!("Unsupported CLI tool: '{tool_name}'")),
    };

    let npm_path = resolve_npm_path()?;
    let install_target = format!("{pkg}@{exact_version}");

    let env_allowlist = get_cli_tools_env_allowlist();

    let runner = ProcessRunner::new();
    let res = runner
        .run_with_stdin_in_environment(
            &npm_path.to_string_lossy(),
            &["install".to_string(), "-g".to_string(), install_target],
            None,
            None,
            Duration::from_secs(120),
            None,
            &env_allowlist,
        )
        .await;

    match res {
        Ok(proc_res) => {
            if proc_res.timed_out {
                #[cfg(windows)]
                {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }

                let mut new_state = state;
                new_state.outcome_unknown = Some(CliToolOutcomeUnknown {
                    tool_name: tool_name.to_string(),
                    message: "Update process timed out after 120 seconds".to_string(),
                    recorded_at: chrono::Utc::now().to_rfc3339(),
                });
                let _ = write_cli_tools_state(&new_state);

                return Err(format!(
                    "Update of '{tool_name}' timed out after 120 seconds. Outcome unknown."
                ));
            }

            if proc_res.exit_code != Some(0) {
                return Err(format!(
                    "npm install failed with exit code {:?}: {}",
                    proc_res.exit_code, proc_res.stderr.text
                ));
            }

            // Post-install verification (§8.2: npm package status + prefix consistency + --version check)
            if !check_npm_package_installed(npm_prefix_path, tool_name) {
                return Err(format!(
                    "Installation completed with exit code 0, but package '{pkg}' was not found in npm global prefix."
                ));
            }

            let path_opt = resolve_cli_path_with_prefix(tool_name, npm_prefix_path);
            let Some(path) = path_opt else {
                return Err(format!(
                    "Update completed, but executable for '{tool_name}' was not found on PATH or npm global prefix."
                ));
            };

            let version = query_cli_version(&path).await.map_err(|e| {
                format!("Update completed, but executable for '{tool_name}' failed '--version' check: {e}")
            })?;

            // Clear outcome_unknown if it was recorded
            let mut current_state = read_cli_tools_state();
            if let Some(ref unk) = current_state.outcome_unknown {
                if unk.tool_name == tool_name {
                    current_state.outcome_unknown = None;
                    let _ = write_cli_tools_state(&current_state);
                }
            }

            invalidate_all_cli_caches();

            Ok(CliToolInfo {
                name: tool_name.to_string(),
                display_name: display_name.to_string(),
                installed: true,
                path: Some(path.to_string_lossy().to_string()),
                installed_version: Some(version),
                latest_version: Some(exact_version),
                latest_checked_at: Some(chrono::Utc::now().to_rfc3339()),
                check_id: Some(check_id.to_string()),
                package_name: pkg.to_string(),
                is_preview,
                update_available: false,
                prerequisite_error: None,
                execution_error: None,
                latest_check_error: None,
                outcome_unknown: None,
                status: CliToolStatus::Installed,
            })
        }
        Err(e) => Err(format!("Process spawn error: {e}")),
    }
}

fn tool_name_to_package_name(tool_name: &str) -> Option<&'static str> {
    match tool_name {
        "codex" => Some("@openai/codex"),
        "dsh" => Some("@deepseek-ai/dsh"),
        "gemini" => Some("@google/gemini-cli"),
        _ => None,
    }
}

fn check_npm_package_dir_exists(prefix: &Path, pkg: &str) -> bool {
    let p1 = prefix.join("node_modules").join(pkg);
    let p2 = prefix.join("lib").join("node_modules").join(pkg);
    p1.is_dir() || p2.is_dir()
}

pub fn check_npm_package_installed(npm_prefix: Option<&Path>, tool_name: &str) -> bool {
    let Some(pkg) = tool_name_to_package_name(tool_name) else {
        return false;
    };
    if let Some(prefix) = npm_prefix {
        return check_npm_package_dir_exists(prefix, pkg);
    }
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let win_prefix = PathBuf::from(appdata).join("npm");
            if check_npm_package_dir_exists(&win_prefix, pkg) {
                return true;
            }
        }
    }
    false
}

pub async fn recover_cli_tool_state(tool_name: &str) -> Result<CliToolInfo, String> {
    // 1. Acquire mutex to guarantee no active install/update process runner task in Poverello
    let _lock = CLI_MUTEX.lock().await;

    let state = read_cli_tools_state();
    if let Some(ref unknown) = state.outcome_unknown {
        if unknown.tool_name == tool_name {
            // 2. Debounce/drain check: Ensure valid RFC3339 timestamp and that at least 5 seconds
            // have elapsed since recorded_at so any background process has completely terminated
            match chrono::DateTime::parse_from_rfc3339(&unknown.recorded_at) {
                Ok(dt) => {
                    let elapsed = chrono::Utc::now().signed_duration_since(dt);
                    if elapsed.num_seconds() < 5 {
                        return Err("Process termination drain in progress. Please wait a few seconds before rechecking.".to_string());
                    }
                }
                Err(_) => {
                    return Err("Cannot verify recorded timestamp for unknown outcome state; OUTCOME_UNKNOWN retained.".to_string());
                }
            }

            // 3. Query fresh prerequisites and prefix to ensure state confirmation is possible
            let prereqs = get_prerequisites_status_cached(true).await;
            if !prereqs.node_npm_ready {
                return Err("Cannot recover CLI tool state: Node.js and npm are required to verify tool state. OUTCOME_UNKNOWN retained. Please verify that Node.js and npm are installed and on PATH, then retry recovery.".to_string());
            }

            // 4. Safe recovery (§8.2):
            // Prior process has terminated (drain period elapsed) and environment check succeeded.
            // Clear OUTCOME_UNKNOWN and persist state so the user is unblocked to retry
            // if the tool is absent/broken, or to use the tool if it is verified installed.
            let mut new_state = state;
            new_state.outcome_unknown = None;
            write_cli_tools_state(&new_state).map_err(|e| {
                format!("Failed to persist recovered state: {e}. OUTCOME_UNKNOWN retained.")
            })?;
        }
    }

    invalidate_all_cli_caches();
    let tools = get_cli_tools_status_cached(true).await?;
    tools
        .into_iter()
        .find(|t| t.name == tool_name)
        .ok_or_else(|| format!("CLI tool '{tool_name}' not found."))
}

/// Pure response parsing for 'codex login status'.
/// Distinguishes verified authenticated, explicit unauthenticated, and unknown failure conditions (§12.28.3).
pub fn parse_codex_login_status(
    stdout: &str,
    stderr: &str,
    exit_code: Option<i32>,
    timed_out: bool,
) -> (String, Option<String>, Option<String>) {
    if timed_out {
        return (
            "unknown".to_string(),
            None,
            Some("Command timed out while checking login status".to_string()),
        );
    }

    if crate::orchestrator::process_runner::is_dll_initialization_failure(exit_code) {
        return (
            "unknown".to_string(),
            None,
            Some("Codex CLI failed to initialize: STATUS_DLL_INIT_FAILED (0xC0000142)".to_string()),
        );
    }

    let combined = format!("{} {}", stdout, stderr);
    let combined_lower = combined.to_lowercase();

    if exit_code == Some(0) {
        let auth_type = if combined_lower.contains("chatgpt") {
            Some("chatgpt".to_string())
        } else if combined_lower.contains("api key") || combined_lower.contains("apikey") {
            Some("api_key".to_string())
        } else if combined_lower.contains("logged in") || combined_lower.contains("authenticated") {
            Some("authenticated".to_string())
        } else {
            None
        };

        if let Some(at) = auth_type {
            return ("authenticated".to_string(), Some(at), None);
        } else {
            return (
                "unknown".to_string(),
                None,
                Some("Status check succeeded with code 0 but output did not confirm authentication".to_string()),
            );
        }
    }

    // Non-zero exit code or unknown exit code
    let is_explicit_unauthenticated = combined_lower.contains("not logged in")
        || combined_lower.contains("unauthenticated")
        || combined_lower.contains("not authenticated")
        || combined_lower.contains("no active login");

    let has_error_signals = combined_lower.contains("error:")
        || combined_lower.contains("fatal:")
        || combined_lower.contains("panic")
        || combined_lower.contains("failed to")
        || combined_lower.contains("command not found")
        || combined_lower.contains("permission denied");

    if is_explicit_unauthenticated && !has_error_signals {
        (
            "not_authenticated".to_string(),
            None,
            Some("Not logged in. Click Login to authenticate.".to_string()),
        )
    } else {
        let diag = crate::orchestrator::process_runner::format_exit_code_diagnostic(exit_code);
        (
            "unknown".to_string(),
            None,
            Some(format!(
                "Status check exit code: {:?} ({})",
                exit_code, diag
            )),
        )
    }
}

/// Pure protocol response parsing for Codex app-server JSON-RPC session (§12.28.3, §12.28.5).
/// Enforces initialize response (id: 1) -> initialized notification -> account/read (id: 2).
/// Separates errors, timeouts, and failures from successful responses, never converting failures
/// into empty Ok results. Excludes PII and raw process outputs from error strings.
pub fn parse_codex_app_server_response(
    stdout: &str,
    _stderr: &str,
    exit_code: Option<i32>,
    timed_out: bool,
) -> Result<(Option<String>, Option<String>, Option<bool>), String> {
    if timed_out {
        return Err("Codex app-server request timed out".to_string());
    }

    if crate::orchestrator::process_runner::is_dll_initialization_failure(exit_code) {
        return Err("Codex app-server failed to initialize: STATUS_DLL_INIT_FAILED (0xC0000142)".to_string());
    }

    if let Some(code) = exit_code {
        if code != 0 {
            let diag = crate::orchestrator::process_runner::format_exit_code_diagnostic(Some(code));
            return Err(format!("Codex app-server exited with non-zero {}", diag));
        }
    } else {
        return Err("Codex app-server terminated abnormally (no exit code)".to_string());
    }

    let mut init_success = false;
    let mut account_res: Option<Result<(Option<String>, Option<String>, Option<bool>), String>> = None;

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let val: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue, // ignore non-JSON or banner lines
        };

        let id = val.get("id").and_then(|i| i.as_i64());

        // Check initialize response (id: 1)
        if id == Some(1) {
            if let Some(err) = val.get("error") {
                let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
                return Err(format!("JSON-RPC initialize error code {code}"));
            }
            if val.get("result").is_some() {
                init_success = true;
            }
        }

        // Check account/read response (id: 2)
        if id == Some(2) {
            if !init_success {
                return Err("Received account/read response before successful initialize".to_string());
            }
            if let Some(err) = val.get("error") {
                let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
                account_res = Some(Err(format!("JSON-RPC account/read error code {code}")));
                break;
            }
            if let Some(result_obj) = val.get("result").and_then(|r| r.as_object()) {
                let requires_openai_auth = result_obj
                    .get("requiresOpenaiAuth")
                    .and_then(|b| b.as_bool());

                let mut auth_method = None;
                let mut account_identity = None;

                if let Some(acc) = result_obj.get("account").and_then(|a| a.as_object()) {
                    auth_method = acc.get("type").and_then(|t| t.as_str()).map(|s| s.to_string());
                    if let Some(email) = acc.get("email").and_then(|e| e.as_str()) {
                        let trimmed_email = email.trim();
                        if !trimmed_email.is_empty() {
                            account_identity = Some(trimmed_email.to_string());
                        }
                    }
                }
                account_res = Some(Ok((auth_method, account_identity, requires_openai_auth)));
                break;
            } else {
                account_res = Some(Err("Invalid account/read response: missing result object".to_string()));
                break;
            }
        }
    }

    if !init_success {
        return Err("Missing or failed initialize handshake from Codex app-server".to_string());
    }

    match account_res {
        Some(res) => res,
        None => Err("Missing account/read response from Codex app-server".to_string()),
    }
}

pub async fn get_codex_account_info() -> Result<CodexAccountInfo, String> {
    get_codex_account_info_cached(false).await
}

pub async fn get_codex_account_info_cached(force_refresh: bool) -> Result<CodexAccountInfo, String> {
    if !force_refresh {
        let guard = CODEX_ACCOUNT_CACHE.lock().unwrap();
        if let Some((instant, ref data)) = *guard {
            if instant.elapsed() < CODEX_ACCOUNT_CACHE_TTL {
                return Ok(data.clone());
            }
        }
    }

    let _gate = CODEX_ACCOUNT_GATE.lock().await;

    if !force_refresh {
        let guard = CODEX_ACCOUNT_CACHE.lock().unwrap();
        if let Some((instant, ref data)) = *guard {
            if instant.elapsed() < CODEX_ACCOUNT_CACHE_TTL {
                return Ok(data.clone());
            }
        }
    }

    let data = get_codex_account_info_uncached().await?;

    {
        let mut guard = CODEX_ACCOUNT_CACHE.lock().unwrap();
        *guard = Some((Instant::now(), data.clone()));
    }

    Ok(data)
}

pub async fn get_codex_account_info_uncached() -> Result<CodexAccountInfo, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let effective_codex_home = crate::codex_global_rules::resolve_codex_home().ok()
        .map(|p| p.to_string_lossy().to_string());

    let path_opt = resolve_cli_path("codex");
    let Some(path) = path_opt else {
        return Ok(CodexAccountInfo {
            installed: false,
            executable_path: None,
            version: None,
            auth_method: None,
            account_identity: None,
            config_profile: None,
            requires_openai_auth: None,
            codex_home: effective_codex_home,
            last_checked_at: Some(now),
        });
    };

    let version = query_cli_version(&path).await.ok();
    let runner = ProcessRunner::new();

    let init_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "clientInfo": {
                "name": "poverello",
                "version": "0.26.1"
            }
        }
    });

    let notif_req = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {}
    });

    let acc_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "account/read",
        "params": {
            "refreshToken": false
        }
    });

    let mut payload = serde_json::to_vec(&init_req).map_err(|e| e.to_string())?;
    payload.push(b'\n');
    let mut notif_bytes = serde_json::to_vec(&notif_req).map_err(|e| e.to_string())?;
    payload.append(&mut notif_bytes);
    payload.push(b'\n');
    let mut acc_bytes = serde_json::to_vec(&acc_req).map_err(|e| e.to_string())?;
    payload.append(&mut acc_bytes);
    payload.push(b'\n');

    let env_vars = get_cli_auth_env_allowlist();

    let proc_res = runner
        .run_with_stdin_in_environment(
            &path.to_string_lossy(),
            &[
                "app-server".to_string(),
                "--listen".to_string(),
                "stdio://".to_string(),
            ],
            None,
            Some(&payload),
            Duration::from_secs(5),
            None,
            &env_vars,
        )
        .await;

    let (auth_method, account_identity, requires_openai_auth) = match proc_res {
        Ok(res) => {
            parse_codex_app_server_response(
                &res.stdout.text,
                &res.stderr.text,
                res.exit_code,
                res.timed_out,
            )?
        }
        Err(e) => return Err(format!("Process spawn error: {e}")),
    };

    Ok(CodexAccountInfo {
        installed: true,
        executable_path: Some(path.to_string_lossy().to_string()),
        version,
        auth_method,
        account_identity,
        config_profile: None,
        requires_openai_auth,
        codex_home: effective_codex_home,
        last_checked_at: Some(now),
    })
}

pub fn get_cli_auth_env_allowlist() -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
    let mut vars = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for (k, v) in get_cli_tools_env_allowlist() {
        let k_str = k.to_string_lossy().to_ascii_lowercase();
        #[cfg(windows)]
        {
            if k_str == "systemroot" || k_str == "windir" {
                continue;
            }
        }
        if seen.insert(k_str) {
            vars.push((k, v));
        }
    }

    if let Ok(ch) = std::env::var("CODEX_HOME") {
        let trimmed = ch.trim();
        if !trimmed.is_empty() {
            let k_lower = "codex_home".to_string();
            if seen.insert(k_lower) {
                vars.push(("CODEX_HOME".into(), trimmed.into()));
            }
        }
    }

    vars
}

pub async fn get_cli_auth_status() -> Result<Vec<CliAuthInfo>, String> {
    get_cli_auth_status_cached(false).await
}

pub async fn get_cli_auth_status_cached(force_refresh: bool) -> Result<Vec<CliAuthInfo>, String> {
    if !force_refresh {
        let guard = AUTH_STATUS_CACHE.lock().unwrap();
        if let Some((instant, ref data)) = *guard {
            if instant.elapsed() < AUTH_STATUS_CACHE_TTL {
                return Ok(data.clone());
            }
        }
    }

    let _gate = AUTH_STATUS_GATE.lock().await;

    if !force_refresh {
        let guard = AUTH_STATUS_CACHE.lock().unwrap();
        if let Some((instant, ref data)) = *guard {
            if instant.elapsed() < AUTH_STATUS_CACHE_TTL {
                return Ok(data.clone());
            }
        }
    }

    let data = get_cli_auth_status_uncached().await?;

    {
        let mut guard = AUTH_STATUS_CACHE.lock().unwrap();
        *guard = Some((Instant::now(), data.clone()));
    }

    Ok(data)
}

pub async fn get_cli_auth_status_uncached() -> Result<Vec<CliAuthInfo>, String> {
    let mut results = Vec::new();
    let now = chrono::Utc::now().to_rfc3339();
    let effective_codex_home = crate::codex_global_rules::resolve_codex_home().ok()
        .map(|p| p.to_string_lossy().to_string());

    // 1. Codex CLI
    let codex_path = resolve_cli_path("codex");
    let codex_auth = if let Some(path) = codex_path {
        let runner = ProcessRunner::new();
        let env_vars = get_cli_auth_env_allowlist();
        let res = runner
            .run_with_stdin_in_environment(
                &path.to_string_lossy(),
                &["login".to_string(), "status".to_string()],
                None,
                None,
                Duration::from_secs(5),
                None,
                &env_vars,
            )
            .await;

        match res {
            Ok(proc_res) => {
                let (status, auth_type, message) = parse_codex_login_status(
                    &proc_res.stdout.text,
                    &proc_res.stderr.text,
                    proc_res.exit_code,
                    proc_res.timed_out,
                );

                // If authenticated, attempt to retrieve verified account identity via app-server read
                let account_identity = if status == "authenticated" {
                    get_codex_account_info_cached(false).await.ok().and_then(|info| info.account_identity)
                } else {
                    None
                };

                CliAuthInfo {
                    name: "codex".to_string(),
                    status,
                    auth_type,
                    message,
                    can_launch_login: true,
                    account_identity,
                    codex_home: effective_codex_home.clone(),
                    last_checked_at: Some(now.clone()),
                }
            }
            Err(e) => CliAuthInfo {
                name: "codex".to_string(),
                status: "unknown".to_string(),
                auth_type: None,
                message: Some(format!("Failed to execute 'codex login status': {e}")),
                can_launch_login: true,
                account_identity: None,
                codex_home: effective_codex_home.clone(),
                last_checked_at: Some(now.clone()),
            },
        }
    } else {
        CliAuthInfo {
            name: "codex".to_string(),
            status: "unknown".to_string(),
            auth_type: None,
            message: Some("Codex CLI is not installed.".to_string()),
            can_launch_login: false,
            account_identity: None,
            codex_home: effective_codex_home.clone(),
            last_checked_at: Some(now.clone()),
        }
    };
    results.push(codex_auth);

    // 2. Gemini CLI
    let gemini_installed = resolve_cli_path("gemini").is_some();
    results.push(CliAuthInfo {
        name: "gemini".to_string(),
        status: "unknown".to_string(),
        auth_type: None,
        message: if gemini_installed {
            Some(
                "Official read-only status interface not available. Sign in via interactive CLI."
                    .to_string(),
            )
        } else {
            Some("Gemini CLI is not installed.".to_string())
        },
        can_launch_login: gemini_installed,
        account_identity: None,
        codex_home: None,
        last_checked_at: Some(now.clone()),
    });

    // 3. DeepSeek Harness (catalog ID: dsh)
    let dsh_installed = resolve_cli_path("dsh").is_some();
    results.push(CliAuthInfo {
        name: "dsh".to_string(),
        status: "unknown".to_string(),
        auth_type: None,
        message: if dsh_installed {
            Some("Provider credentials managed via local Web UI.".to_string())
        } else {
            Some("DeepSeek Harness is not installed.".to_string())
        },
        can_launch_login: dsh_installed,
        account_identity: None,
        codex_home: None,
        last_checked_at: Some(now),
    });

    Ok(results)
}

pub fn launch_cli_auth(tool_name: &str) -> Result<(), String> {
    invalidate_auth_caches();
    let clean_tool = tool_name.trim().to_lowercase();
    let (cmd, args) = match clean_tool.as_str() {
        "codex" => ("codex", vec!["login"]),
        "gemini" => ("gemini", vec![]),
        "dsh" | "deepseek" => ("dsh", vec!["web"]),
        _ => return Err(format!("Unsupported tool for auth launch: {tool_name}")),
    };

    let temp_launch_dir =
        std::env::temp_dir().join(format!("poverello-launch-{}", uuid::Uuid::new_v4()));
    let _ = std::fs::create_dir_all(&temp_launch_dir);

    #[cfg(windows)]
    {
        let full_args = if args.is_empty() {
            cmd.to_string()
        } else {
            format!("{cmd} {}", args.join(" "))
        };
        std::process::Command::new("cmd.exe")
            .arg("/c")
            .arg("start")
            .arg(format!("Poverello - {cmd} Auth"))
            .arg("cmd.exe")
            .arg("/k")
            .arg(&full_args)
            .current_dir(&temp_launch_dir)
            .spawn()
            .map_err(|e| format!("Failed to launch visible console for {cmd}: {e}"))?;
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new(cmd)
            .args(&args)
            .current_dir(&temp_launch_dir)
            .spawn()
            .map_err(|e| format!("Failed to spawn {cmd}: {e}"))?;
    }

    Ok(())
}

pub fn open_node_download_page() -> Result<(), String> {
    const NODE_URL: &str = "https://nodejs.org/en/download/";
    #[cfg(windows)]
    {
        std::process::Command::new("cmd.exe")
            .arg("/c")
            .arg("start")
            .arg(NODE_URL)
            .spawn()
            .map_err(|e| format!("Failed to open browser: {e}"))?;
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open")
            .arg(NODE_URL)
            .spawn()
            .map_err(|e| format!("Failed to open browser: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_path_resolution_non_existent() {
        let res = resolve_cli_path("non_existent_random_tool_xyz_12345");
        assert!(res.is_none());
    }

    #[test]
    fn test_cli_tools_state_serialization() {
        let state = CliToolsState {
            outcome_unknown: Some(CliToolOutcomeUnknown {
                tool_name: "codex".to_string(),
                message: "Process timed out after 120s".to_string(),
                recorded_at: "2026-11-01T00:00:00Z".to_string(),
            }),
        };

        let json = serde_json::to_string_pretty(&state).unwrap();
        let parsed: CliToolsState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, state);
    }

    #[test]
    fn test_parse_cli_version_tokens() {
        let sample1 = "codex-cli 0.163.0\n";
        let ver1 = sample1.split_whitespace().last().unwrap();
        assert_eq!(ver1, "0.163.0");

        let sample2 = "0.2.1-rc.2\n";
        let ver2 = sample2.split_whitespace().last().unwrap();
        assert_eq!(ver2, "0.2.1-rc.2");

        let sample3 = "gemini 0.1.0\n";
        let ver3 = sample3.split_whitespace().last().unwrap();
        assert_eq!(ver3, "0.1.0");
    }

    #[test]
    fn test_semver_comparison() {
        assert!(compare_semver_update("0.163.0", "0.164.0"));
        assert!(compare_semver_update("v0.1.0", "0.2.0"));
        assert!(!compare_semver_update("1.0.0", "1.0.0"));
        assert!(!compare_semver_update("2.0.0", "1.9.9"));
    }

    #[test]
    fn test_three_cli_tools_catalog() {
        let tools = [
            ("codex", "Codex CLI", "@openai/codex", false),
            ("dsh", "DeepSeek Harness", "@deepseek-ai/dsh", true),
            ("gemini", "Gemini CLI", "@google/gemini-cli", false),
        ];
        assert_eq!(tools.len(), 3);
        assert_eq!(tools[2].0, "gemini");
        assert_eq!(tools[2].2, "@google/gemini-cli");
    }

    pub static TEST_OVERRIDE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[tokio::test]
    async fn test_unsupported_cli_tool_fails_fast() {
        let _test_lock = TEST_OVERRIDE_LOCK.lock().unwrap();
        let res = run_cli_install("unsupported_tool").await;
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Unsupported CLI tool"));
    }

    #[tokio::test]
    async fn test_check_cli_tools_latest_version_with_mock() {
        let _test_lock = TEST_OVERRIDE_LOCK.lock().unwrap();
        let mut mocks = HashMap::new();
        mocks.insert("codex".to_string(), "0.164.0".to_string());
        mocks.insert("dsh".to_string(), "0.3.0".to_string());
        mocks.insert("gemini".to_string(), "0.2.0".to_string());

        set_test_runner_override(true, Some(mocks));

        let res = check_cli_tools_latest_version().await;
        set_test_runner_override(false, None);

        // If node/npm are present on test host, check succeeds
        if let Ok(tools) = res {
            assert_eq!(tools.len(), 3);
            let codex = tools.iter().find(|t| t.name == "codex").unwrap();
            assert_eq!(codex.latest_version.as_deref(), Some("0.164.0"));
            assert!(codex.check_id.is_some());
        }
    }

    #[tokio::test]
    async fn test_cli_update_requires_check_and_valid_id() {
        let _test_lock = TEST_OVERRIDE_LOCK.lock().unwrap();
        // Clear cache
        {
            let mut guard = LATEST_CACHE.lock().unwrap();
            *guard = None;
        }

        // Calling update without check must fail
        let res = run_cli_update("codex", "fake-check-id-123").await;
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(
            err.contains("No update check has been performed")
                || err.contains("Invalid or superseded update check ID"),
            "unexpected error message: {err}"
        );

        // Inject cache with a specific check_id
        {
            let mut guard = LATEST_CACHE.lock().unwrap();
            let mut versions = HashMap::new();
            versions.insert("codex".to_string(), "99.0.0".to_string());
            *guard = Some(EphemeralLatestCache {
                check_id: "valid-check-id-456".to_string(),
                checked_at: Instant::now(),
                checked_at_rfc3339: chrono::Utc::now().to_rfc3339(),
                versions,
                errors: HashMap::new(),
                node_path: resolve_node_path()
                    .ok()
                    .map(|p| p.to_string_lossy().to_string()),
                node_version: Some("v20.0.0".to_string()),
                npm_path: resolve_npm_path()
                    .ok()
                    .map(|p| p.to_string_lossy().to_string()),
                npm_version: Some("10.0.0".to_string()),
                cli_paths: HashMap::new(),
            });
        }

        // Mismatched check ID must fail
        let mismatch_res = run_cli_update("codex", "wrong-check-id-789").await;
        assert!(mismatch_res.is_err());
        assert!(mismatch_res
            .unwrap_err()
            .contains("Invalid or superseded update check ID"));

        // Clean up
        {
            let mut guard = LATEST_CACHE.lock().unwrap();
            *guard = None;
        }
    }

    #[test]
    fn test_env_allowlist_contents() {
        let allowlist = get_cli_tools_env_allowlist();
        let allowed_keys = [
            "PATH",
            "SYSTEMROOT",
            "COMSPEC",
            "PATHEXT",
            "TMP",
            "TEMP",
            "USERPROFILE",
            "HOME",
        ];
        for (k, _) in &allowlist {
            let key_str = k.to_string_lossy();
            assert!(
                allowed_keys.contains(&key_str.as_ref()),
                "Key '{}' is not in approved allowlist",
                key_str
            );
        }
    }

    #[test]
    fn test_cli_auth_env_allowlist_preserves_codex_home_and_excludes_credentials() {
        let _test_lock = TEST_OVERRIDE_LOCK.lock().unwrap();
        std::env::set_var("CODEX_HOME", "C:\\mock\\codex\\home");
        std::env::set_var("OPENAI_API_KEY", "sk-secret-12345");
        std::env::set_var("CODEX_API_KEY", "secret-token-67890");

        let allowlist = get_cli_auth_env_allowlist();
        let keys: Vec<String> = allowlist
            .iter()
            .map(|(k, _)| k.to_string_lossy().to_string())
            .collect();

        assert!(
            keys.contains(&"CODEX_HOME".to_string()),
            "CODEX_HOME should be preserved"
        );
        assert!(
            !keys.contains(&"OPENAI_API_KEY".to_string()),
            "OPENAI_API_KEY must never be in auth status allowlist"
        );
        assert!(
            !keys.contains(&"CODEX_API_KEY".to_string()),
            "CODEX_API_KEY must never be in auth status allowlist"
        );

        // Clean up
        std::env::remove_var("CODEX_HOME");
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("CODEX_API_KEY");
    }

    #[test]
    fn test_launch_cli_auth_rejects_arbitrary_commands() {
        assert!(launch_cli_auth("arbitrary_command").is_err());
        assert!(launch_cli_auth("calc.exe").is_err());
        assert!(launch_cli_auth("codex & calc.exe").is_err());
        assert!(launch_cli_auth("").is_err());
    }

    #[tokio::test]
    async fn test_cli_auth_status_returns_unknown_for_gemini_and_dsh() {
        let _test_lock = TEST_OVERRIDE_LOCK.lock().unwrap();
        let auth_list = get_cli_auth_status().await.unwrap();
        assert_eq!(auth_list.len(), 3);

        let codex_auth = auth_list.iter().find(|a| a.name == "codex").unwrap();
        assert!(codex_auth.can_launch_login);

        let gemini_auth = auth_list.iter().find(|a| a.name == "gemini").unwrap();
        assert_eq!(gemini_auth.status, "unknown");
        assert!(gemini_auth.can_launch_login);

        let dsh_auth = auth_list.iter().find(|a| a.name == "dsh").unwrap();
        assert_eq!(dsh_auth.status, "unknown");
        assert!(dsh_auth.can_launch_login);
    }

    #[test]
    fn test_parse_codex_login_status_variants() {
        // 1. Logged in via ChatGPT
        let (st, at, msg) = parse_codex_login_status("Logged in using ChatGPT\nuser@example.com", "", Some(0), false);
        assert_eq!(st, "authenticated");
        assert_eq!(at.as_deref(), Some("chatgpt"));
        assert!(msg.is_none());

        // 2. Logged in via API key
        let (st, at, msg) = parse_codex_login_status("Logged in using API Key", "", Some(0), false);
        assert_eq!(st, "authenticated");
        assert_eq!(at.as_deref(), Some("api_key"));
        assert!(msg.is_none());

        // 3. Exit 0 but ambiguous/unverified output
        let (st, at, msg) = parse_codex_login_status("Some random banner", "", Some(0), false);
        assert_eq!(st, "unknown");
        assert!(at.is_none());
        assert!(msg.unwrap().contains("did not confirm authentication"));

        // 4. Explicit unauthenticated
        let (st, at, msg) = parse_codex_login_status("Not logged in. Use codex login.", "", Some(1), false);
        assert_eq!(st, "not_authenticated");
        assert!(at.is_none());
        assert!(msg.unwrap().contains("Not logged in"));

        // 5. Exit 1 with error signal (must be unknown, NOT not_authenticated)
        let (st, at, msg) = parse_codex_login_status("", "error: could not connect to auth endpoint", Some(1), false);
        assert_eq!(st, "unknown");
        assert!(at.is_none());
        assert!(msg.unwrap().contains("Status check exit code: Some(1)"));

        // 6. Exit 1 with arbitrary output (must be unknown, NOT not_authenticated)
        let (st, at, _msg) = parse_codex_login_status("random failure", "", Some(1), false);
        assert_eq!(st, "unknown");
        assert!(at.is_none());

        // 7. Timeout (must be unknown)
        let (st, at, msg) = parse_codex_login_status("", "", None, true);
        assert_eq!(st, "unknown");
        assert!(at.is_none());
        assert!(msg.unwrap().contains("timed out"));
    }

    #[test]
    fn test_parse_codex_app_server_response_handshake_and_errors() {
        // 1. Successful initialize followed by successful account/read with email
        let stdout = r#"
{"jsonrpc":"2.0","id":1,"result":{"serverInfo":{"name":"codex","version":"0.163.0"}}}
{"jsonrpc":"2.0","id":2,"result":{"requiresOpenaiAuth":true,"account":{"type":"chatgpt","email":"test@example.com"}}}
"#;
        let res = parse_codex_app_server_response(stdout, "", Some(0), false).unwrap();
        assert_eq!(res.0.as_deref(), Some("chatgpt"));
        assert_eq!(res.1.as_deref(), Some("test@example.com"));
        assert_eq!(res.2, Some(true));

        // 2. Successful account/read without email (e.g. anonymous or missing email field)
        let stdout_no_email = r#"
{"jsonrpc":"2.0","id":1,"result":{"serverInfo":{"name":"codex","version":"0.163.0"}}}
{"jsonrpc":"2.0","id":2,"result":{"requiresOpenaiAuth":false,"account":{"type":"api_key"}}}
"#;
        let res_no_email = parse_codex_app_server_response(stdout_no_email, "", Some(0), false).unwrap();
        assert_eq!(res_no_email.0.as_deref(), Some("api_key"));
        assert_eq!(res_no_email.1, None);
        assert_eq!(res_no_email.2, Some(false));

        // 3. Initialize error returned
        let stdout_init_err = r#"
{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid Request"}}
"#;
        let err_init = parse_codex_app_server_response(stdout_init_err, "", Some(0), false).unwrap_err();
        assert!(err_init.contains("initialize error code -32600"));

        // 4. Missing initialize (account/read received directly)
        let stdout_missing_init = r#"
{"jsonrpc":"2.0","id":2,"result":{"requiresOpenaiAuth":true,"account":{"type":"chatgpt","email":"test@example.com"}}}
"#;
        let err_no_init = parse_codex_app_server_response(stdout_missing_init, "", Some(0), false).unwrap_err();
        assert!(err_no_init.contains("before successful initialize") || err_no_init.contains("Missing or failed initialize"));

        // 5. Account/read RPC error
        let stdout_acc_err = r#"
{"jsonrpc":"2.0","id":1,"result":{"serverInfo":{"name":"codex","version":"0.163.0"}}}
{"jsonrpc":"2.0","id":2,"error":{"code":-32001,"message":"Account read failed"}}
"#;
        let err_acc = parse_codex_app_server_response(stdout_acc_err, "", Some(0), false).unwrap_err();
        assert!(err_acc.contains("account/read error code -32001"));

        // 6. Process timed out
        let err_timeout = parse_codex_app_server_response("", "", None, true).unwrap_err();
        assert!(err_timeout.contains("timed out"));

        // 7. Non-zero exit code
        let err_nonzero = parse_codex_app_server_response("", "crash", Some(1), false).unwrap_err();
        assert!(err_nonzero.contains("exited with non-zero"));
        assert!(err_nonzero.contains("0x00000001"));

        // 8. Missing account/read response
        let stdout_only_init = r#"
{"jsonrpc":"2.0","id":1,"result":{"serverInfo":{"name":"codex","version":"0.163.0"}}}
"#;
        let err_missing_acc = parse_codex_app_server_response(stdout_only_init, "", Some(0), false).unwrap_err();
        assert!(err_missing_acc.contains("Missing account/read response"));
    }

    #[test]
    fn test_parse_codex_dll_init_failure() {
        let code = Some(-1073741502i32); // 0xC0000142
        let (status, auth_type, msg) = parse_codex_login_status("", "", code, false);
        assert_eq!(status, "unknown");
        assert_eq!(auth_type, None);
        let msg_str = msg.unwrap();
        assert!(msg_str.contains("0xC0000142"));
        assert!(msg_str.contains("STATUS_DLL_INIT_FAILED"));

        let err = parse_codex_app_server_response("", "", code, false).unwrap_err();
        assert!(err.contains("0xC0000142"));
        assert!(err.contains("STATUS_DLL_INIT_FAILED"));
    }

    #[tokio::test]
    async fn test_cli_tools_cache_bounded_freshness_and_invalidation() {
        let _test_lock = TEST_OVERRIDE_LOCK.lock().unwrap();
        invalidate_all_cli_caches();

        // 1. Manually seed CLI tools cache
        let fake_tools = vec![CliToolInfo {
            name: "codex".to_string(),
            display_name: "Codex CLI".to_string(),
            installed: true,
            path: Some("C:\\fake\\codex.exe".to_string()),
            installed_version: Some("0.26.0".to_string()),
            latest_version: None,
            latest_checked_at: None,
            check_id: None,
            package_name: "@openai/codex".to_string(),
            is_preview: false,
            update_available: false,
            prerequisite_error: None,
            execution_error: None,
            latest_check_error: None,
            outcome_unknown: None,
            status: CliToolStatus::Installed,
        }];

        {
            let mut guard = CLI_TOOLS_CACHE.lock().unwrap();
            *guard = Some((Instant::now(), fake_tools.clone()));
        }

        // Fresh cache returns seeded data without probing
        let cached = get_cli_tools_status_cached(false).await.unwrap();
        assert_eq!(cached, fake_tools);

        // Invalidate cache
        invalidate_cli_tools_cache();
        {
            let guard = CLI_TOOLS_CACHE.lock().unwrap();
            assert!(guard.is_none());
        }

        // 2. Auth status cache seeding and invalidation
        let fake_auth = vec![CliAuthInfo {
            name: "codex".to_string(),
            status: "authenticated".to_string(),
            auth_type: Some("chatgpt".to_string()),
            message: None,
            can_launch_login: true,
            account_identity: Some("user@example.com".to_string()),
            codex_home: None,
            last_checked_at: Some("2026-11-02T00:00:00Z".to_string()),
        }];

        {
            let mut guard = AUTH_STATUS_CACHE.lock().unwrap();
            *guard = Some((Instant::now(), fake_auth.clone()));
        }

        let cached_auth = get_cli_auth_status_cached(false).await.unwrap();
        assert_eq!(cached_auth, fake_auth);

        invalidate_auth_caches();
        {
            let guard = AUTH_STATUS_CACHE.lock().unwrap();
            assert!(guard.is_none());
        }

        // 3. Node prerequisites cache seeding and invalidation
        let fake_prereqs = NodePrerequisiteStatus {
            node_installed: true,
            node_version: Some("20.18.0".to_string()),
            node_major_version: Some(20),
            node_path: Some("C:\\fake\\node.exe".to_string()),
            npm_installed: true,
            npm_version: Some("10.8.2".to_string()),
            npm_path: Some("C:\\fake\\npm.cmd".to_string()),
            node_npm_ready: true,
            gemini_node_ready: true,
            message: None,
            npm_global_prefix: Some("C:\\fake\\npm_global".to_string()),
        };

        {
            let mut guard = NODE_PREREQS_CACHE.lock().unwrap();
            *guard = Some((Instant::now(), fake_prereqs.clone()));
        }

        let cached_prereqs = get_prerequisites_status_cached(false).await;
        assert_eq!(cached_prereqs, fake_prereqs);

        invalidate_prerequisites_cache();
        {
            let guard = NODE_PREREQS_CACHE.lock().unwrap();
            assert!(guard.is_none());
        }

        invalidate_all_cli_caches();
    }

    #[test]
    fn test_resolve_cli_path_with_prefix_custom_dir() {
        let temp = std::env::temp_dir().join(format!("pct-test-prefix-{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp);

        #[cfg(windows)]
        let target_file = temp.join("mocktool.cmd");
        #[cfg(not(windows))]
        let target_file = temp.join("mocktool");

        let _ = std::fs::write(&target_file, "echo hello");

        let resolved = resolve_cli_path_with_prefix("mocktool", Some(&temp));
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap(), target_file);

        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_cli_tool_info_serde_defaults() {
        let json = r#"{
            "name": "testtool",
            "display_name": "Test Tool",
            "installed": true,
            "path": "/bin/testtool",
            "installed_version": "1.0.0",
            "latest_version": null,
            "latest_checked_at": null,
            "check_id": null,
            "package_name": "@test/testtool",
            "is_preview": false,
            "update_available": false,
            "prerequisite_error": null
        }"#;

        let parsed: CliToolInfo = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.name, "testtool");
        assert_eq!(parsed.execution_error, None);
        assert_eq!(parsed.latest_check_error, None);
        assert_eq!(parsed.outcome_unknown, None);
        assert_eq!(parsed.status, CliToolStatus::NotInstalled);
    }

    #[test]
    fn test_cli_tool_status_serde_representation() {
        assert_eq!(serde_json::to_string(&CliToolStatus::NotInstalled).unwrap(), "\"not_installed\"");
        assert_eq!(serde_json::to_string(&CliToolStatus::Installed).unwrap(), "\"installed\"");
        assert_eq!(serde_json::to_string(&CliToolStatus::ExecutionError).unwrap(), "\"execution_error\"");
        assert_eq!(serde_json::to_string(&CliToolStatus::Unknown).unwrap(), "\"unknown\"");

        assert_eq!(serde_json::from_str::<CliToolStatus>("\"not_installed\"").unwrap(), CliToolStatus::NotInstalled);
        assert_eq!(serde_json::from_str::<CliToolStatus>("\"installed\"").unwrap(), CliToolStatus::Installed);
        assert_eq!(serde_json::from_str::<CliToolStatus>("\"execution_error\"").unwrap(), CliToolStatus::ExecutionError);
        assert_eq!(serde_json::from_str::<CliToolStatus>("\"unknown\"").unwrap(), CliToolStatus::Unknown);
    }

    #[tokio::test]
    async fn test_recover_cli_tool_state_timestamp_validation() {
        let _test_lock = TEST_OVERRIDE_LOCK.lock().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        let isolated_state_path = temp_dir.path().join("cli_tools_state.json");
        set_state_file_override(Some(isolated_state_path));

        let ready_prereqs = NodePrerequisiteStatus {
            node_installed: true,
            node_version: Some("v20.10.0".to_string()),
            node_major_version: Some(20),
            node_path: Some("C:\\fake\\node.exe".to_string()),
            npm_installed: true,
            npm_version: Some("10.2.0".to_string()),
            npm_path: Some("C:\\fake\\npm.cmd".to_string()),
            npm_global_prefix: Some("C:\\fake\\prefix".to_string()),
            node_npm_ready: true,
            gemini_node_ready: true,
            message: None,
        };
        set_prereqs_override(Some(ready_prereqs.clone()));

        struct CleanupGuard;
        impl Drop for CleanupGuard {
            fn drop(&mut self) {
                set_state_file_override(None);
                set_prereqs_override(None);
            }
        }
        let _guard = CleanupGuard;

        // 1. Invalid timestamp must be rejected and OUTCOME_UNKNOWN retained
        let invalid_state = CliToolsState {
            outcome_unknown: Some(CliToolOutcomeUnknown {
                tool_name: "codex".to_string(),
                message: "timeout".to_string(),
                recorded_at: "not-an-rfc3339-timestamp".to_string(),
            }),
        };
        let _ = write_cli_tools_state(&invalid_state);

        let err = recover_cli_tool_state("codex").await.unwrap_err();
        assert!(err.contains("Cannot verify recorded timestamp"));
        let state_after = read_cli_tools_state();
        assert!(state_after.outcome_unknown.is_some());

        // 2. Fresh timestamp (< 5s) must be rejected for debounce
        let fresh_state = CliToolsState {
            outcome_unknown: Some(CliToolOutcomeUnknown {
                tool_name: "codex".to_string(),
                message: "timeout".to_string(),
                recorded_at: chrono::Utc::now().to_rfc3339(),
            }),
        };
        let _ = write_cli_tools_state(&fresh_state);

        let debounce_err = recover_cli_tool_state("codex").await.unwrap_err();
        assert!(debounce_err.contains("Process termination drain in progress"));
        let state_after_debounce = read_cli_tools_state();
        assert!(state_after_debounce.outcome_unknown.is_some());

        let past_time = (chrono::Utc::now() - chrono::Duration::seconds(10)).to_rfc3339();

        // 2b. Missing or broken Node.js/npm prerequisites (§8.2: 状態確認に失敗する場合はブロックを維持し、理由を表示)
        // When Node.js and npm are unavailable, tool state cannot be established; OUTCOME_UNKNOWN must be retained.
        set_prereqs_override(Some(NodePrerequisiteStatus {
            node_installed: false,
            node_version: None,
            node_major_version: None,
            node_path: None,
            npm_installed: false,
            npm_version: None,
            npm_path: None,
            npm_global_prefix: None,
            node_npm_ready: false,
            gemini_node_ready: false,
            message: Some("Node.js is not installed".to_string()),
        }));

        let unready_state = CliToolsState {
            outcome_unknown: Some(CliToolOutcomeUnknown {
                tool_name: "codex".to_string(),
                message: "timeout".to_string(),
                recorded_at: past_time.clone(),
            }),
        };
        let _ = write_cli_tools_state(&unready_state);

        let prereq_err = recover_cli_tool_state("codex").await.unwrap_err();
        assert!(prereq_err.contains("Node.js and npm are required to verify tool state"));
        assert!(prereq_err.contains("OUTCOME_UNKNOWN retained"));
        let state_after_unready = read_cli_tools_state();
        assert!(
            state_after_unready.outcome_unknown.is_some(),
            "OUTCOME_UNKNOWN must be retained when environment check cannot establish status due to missing Node/npm"
        );

        // Restore ready prerequisites for verified absent, broken, and installed recovery cases
        set_prereqs_override(Some(ready_prereqs.clone()));

        // 3. Elapsed timestamp (> 5s) on absent tool in isolated empty prefix
        // §8.2: Prior process ended, tool is absent -> clear OUTCOME_UNKNOWN and allow retry (status = NotInstalled)
        let empty_prefix_dir = tempfile::tempdir().unwrap();
        set_prefix_override(Some(empty_prefix_dir.path().to_path_buf()));
        struct EmptyPrefixCleanup;
        impl Drop for EmptyPrefixCleanup {
            fn drop(&mut self) {
                set_prefix_override(None);
            }
        }
        let _empty_prefix_guard = EmptyPrefixCleanup;

        let past_time = (chrono::Utc::now() - chrono::Duration::seconds(10)).to_rfc3339();
        let past_missing_state = CliToolsState {
            outcome_unknown: Some(CliToolOutcomeUnknown {
                tool_name: "gemini".to_string(),
                message: "timeout".to_string(),
                recorded_at: past_time.clone(),
            }),
        };
        let _ = write_cli_tools_state(&past_missing_state);

        let recovered_missing = recover_cli_tool_state("gemini").await.expect("Recovery must clear unknown state on absent tool");
        assert_eq!(recovered_missing.status, CliToolStatus::NotInstalled);
        assert_eq!(recovered_missing.outcome_unknown, None);
        let state_after_missing = read_cli_tools_state();
        assert_eq!(
            state_after_missing.outcome_unknown, None,
            "OUTCOME_UNKNOWN must be cleared to allow retry when prior process ended and tool is absent"
        );
        drop(_empty_prefix_guard);

        // 4. Elapsed timestamp (> 5s) on broken/corrupted tool in prefix
        // §8.2: Prior process ended, tool is broken -> clear OUTCOME_UNKNOWN and allow retry (status = ExecutionError)
        let broken_prefix_dir = tempfile::tempdir().unwrap();
        #[cfg(windows)]
        {
            let broken_batch = broken_prefix_dir.path().join("dsh.cmd");
            std::fs::write(&broken_batch, "@exit /b 1\r\n").unwrap();
        }
        #[cfg(not(windows))]
        {
            let broken_script = broken_prefix_dir.path().join("dsh");
            std::fs::write(&broken_script, "#!/bin/sh\nexit 1\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&broken_script).unwrap().permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&broken_script, perms).unwrap();
            }
        }
        set_prefix_override(Some(broken_prefix_dir.path().to_path_buf()));
        struct BrokenPrefixCleanup;
        impl Drop for BrokenPrefixCleanup {
            fn drop(&mut self) {
                set_prefix_override(None);
            }
        }
        let _broken_prefix_guard = BrokenPrefixCleanup;

        let past_broken_state = CliToolsState {
            outcome_unknown: Some(CliToolOutcomeUnknown {
                tool_name: "dsh".to_string(),
                message: "timeout".to_string(),
                recorded_at: past_time.clone(),
            }),
        };
        let _ = write_cli_tools_state(&past_broken_state);

        let recovered_broken = recover_cli_tool_state("dsh").await.expect("Recovery must clear unknown state on broken tool");
        assert_eq!(recovered_broken.status, CliToolStatus::ExecutionError);
        assert_eq!(recovered_broken.outcome_unknown, None);
        let state_after_recover = read_cli_tools_state();
        assert_eq!(
            state_after_recover.outcome_unknown, None,
            "OUTCOME_UNKNOWN must be cleared to allow retry when prior process ended and tool is broken"
        );
        drop(_broken_prefix_guard);

        // 5. Deterministic hermetic verified success using PREFIX_OVERRIDE
        let prefix_dir = tempfile::tempdir().unwrap();
        let prefix = prefix_dir.path();
        let codex_pkg = prefix.join("node_modules").join("@openai").join("codex");
        std::fs::create_dir_all(&codex_pkg).unwrap();

        #[cfg(windows)]
        {
            let batch = prefix.join("codex.cmd");
            std::fs::write(&batch, "@echo 0.166.0\r\n").unwrap();
        }
        #[cfg(not(windows))]
        {
            let script = prefix.join("codex");
            std::fs::write(&script, "#!/bin/sh\necho 0.166.0\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&script).unwrap().permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&script, perms).unwrap();
            }
        }

        set_prefix_override(Some(prefix.to_path_buf()));
        struct PrefixCleanup;
        impl Drop for PrefixCleanup {
            fn drop(&mut self) {
                set_prefix_override(None);
            }
        }
        let _prefix_guard = PrefixCleanup;

        let past_codex_state = CliToolsState {
            outcome_unknown: Some(CliToolOutcomeUnknown {
                tool_name: "codex".to_string(),
                message: "timeout".to_string(),
                recorded_at: past_time,
            }),
        };
        write_cli_tools_state(&past_codex_state).unwrap();

        let tool = recover_cli_tool_state("codex").await.expect("Deterministic recovery must succeed");
        assert_eq!(tool.status, CliToolStatus::Installed);
        assert_eq!(tool.outcome_unknown, None);
        assert!(tool.installed_version.is_some());
        let state_after_codex = read_cli_tools_state();
        assert_eq!(state_after_codex.outcome_unknown, None);
    }

    #[test]
    fn test_check_npm_package_installed_matrix() {
        let temp_dir1 = tempfile::tempdir().unwrap();
        let prefix1 = temp_dir1.path();

        let temp_dir2 = tempfile::tempdir().unwrap();
        let prefix2 = temp_dir2.path();

        // 1. Missing package in empty prefixes
        assert!(!check_npm_package_installed(Some(prefix1), "codex"));
        assert!(!check_npm_package_installed(Some(prefix2), "codex"));

        // 2. Package present in prefix1 node_modules/@openai/codex
        let pkg_dir = prefix1.join("node_modules").join("@openai").join("codex");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        assert!(check_npm_package_installed(Some(prefix1), "codex"));
        assert!(!check_npm_package_installed(Some(prefix1), "dsh"));

        // 3. Prefix isolation: prefix2 must NOT see package installed in prefix1
        assert!(!check_npm_package_installed(Some(prefix2), "codex"));

        // 4. Package present in prefix2 lib/node_modules/@deepseek-ai/dsh
        let dsh_pkg_dir = prefix2.join("lib").join("node_modules").join("@deepseek-ai").join("dsh");
        std::fs::create_dir_all(&dsh_pkg_dir).unwrap();
        assert!(check_npm_package_installed(Some(prefix2), "dsh"));
        // Prefix isolation: prefix1 must NOT see package installed in prefix2
        assert!(!check_npm_package_installed(Some(prefix1), "dsh"));
    }
}
