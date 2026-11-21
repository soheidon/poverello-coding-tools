use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::pov_direct::{
    self, PovDirectFileState, PovDirectSettings, PovDirectStatus,
};

pub const PINNED_AGENT_OPT_VERSION: &str = "0.1.0";
pub const PINNED_AGENT_OPT_TARBALL_URL: &str =
    "https://github.com/tradertanmay/agent-opt/releases/download/v0.1.0/agent-opt-0.1.0.tgz";
pub const MIN_NODE_MAJOR: u64 = 20;

static ENGINE_TRANSITION_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NerfEngine {
    PoverelloRules,
    Agentopt,
}

impl Default for NerfEngine {
    fn default() -> Self {
        Self::PoverelloRules
    }
}

impl NerfEngine {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim() {
            "poverello_rules" => Ok(Self::PoverelloRules),
            "agentopt" => Ok(Self::Agentopt),
            other => Err(format!(
                "Unsupported Nerf engine '{other}' (expected 'poverello_rules' or 'agentopt')"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentOptWorkspaceState {
    NotInstalled,
    NotInitialized,
    Disabled,
    Enabled,
    InvalidConfig,
    UnsupportedVersion,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentOptWorkspaceMode {
    #[serde(rename = "follow_registered_projects", alias = "follow_active_plan")]
    FollowRegisteredProjects,
    #[serde(rename = "manual_override")]
    ManualOverride,
}

impl Default for AgentOptWorkspaceMode {
    fn default() -> Self {
        Self::FollowRegisteredProjects
    }
}

impl AgentOptWorkspaceMode {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim() {
            "follow_registered_projects" | "follow_active_plan" => {
                Ok(Self::FollowRegisteredProjects)
            }
            "manual_override" => Ok(Self::ManualOverride),
            other => Err(format!(
                "Unsupported workspace mode '{other}' (expected 'follow_registered_projects' or 'manual_override')"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct NerfEngineSettings {
    #[serde(default)]
    pub engine: NerfEngine,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_mode: Option<AgentOptWorkspaceMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_workspace: Option<String>,
    #[serde(default)]
    pub registered_workspaces: Vec<String>,
    #[serde(default)]
    pub project_preferences: BTreeMap<String, AgentOptProjectPreference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_pov_direct_settings: Option<PovDirectSettings>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentOptProjectPreference {
    pub repository_path: String,
    #[serde(default)]
    pub use_agent_opt: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_workspace: Option<String>,
}

pub fn resolve_persisted_workspace_selection(
    settings: &NerfEngineSettings,
) -> (AgentOptWorkspaceMode, Option<String>) {
    let non_empty = |opt: &Option<String>| -> Option<String> {
        opt.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    match settings.workspace_mode {
        Some(AgentOptWorkspaceMode::FollowRegisteredProjects) => {
            (AgentOptWorkspaceMode::FollowRegisteredProjects, None)
        }
        Some(AgentOptWorkspaceMode::ManualOverride) => (
            AgentOptWorkspaceMode::ManualOverride,
            non_empty(&settings.manual_workspace).or_else(|| non_empty(&settings.selected_workspace)),
        ),
        None => {
            if let Some(legacy_ws) =
                non_empty(&settings.manual_workspace).or_else(|| non_empty(&settings.selected_workspace))
            {
                (AgentOptWorkspaceMode::ManualOverride, Some(legacy_ws))
            } else {
                (AgentOptWorkspaceMode::FollowRegisteredProjects, None)
            }
        }
    }
}

pub fn normalize_workspace_path_for_display(path: &str) -> String {
    if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{}", unc)
    } else if path.starts_with(r"\\?\") {
        path[4..].to_string()
    } else {
        path.to_string()
    }
}

type ResolvedWorkspaceTarget = (String, PathBuf, Option<String>);

fn resolve_workspace_targets(
    project_slots: &[crate::plan_manager::ProjectPlanSlot],
    workspace_mode: AgentOptWorkspaceMode,
    manual_workspace: Option<&str>,
) -> (Vec<ResolvedWorkspaceTarget>, Vec<String>) {
    let mut errors = Vec::new();
    let mut targets = Vec::new();
    match workspace_mode {
        AgentOptWorkspaceMode::FollowRegisteredProjects => {
            let mut seen = std::collections::HashSet::new();
            for slot in project_slots {
                let repo_path = slot.repo_path.as_deref().map(str::trim).filter(|s| !s.is_empty());
                let plan_path = slot.plan_path.as_deref().map(str::trim).filter(|s| !s.is_empty());
                if repo_path.is_none() && plan_path.is_none() {
                    continue;
                }
                let Some(repo_path) = repo_path else {
                    errors.push(format!(
                        "Project {} has a plan path but no repository root",
                        slot.slot_id
                    ));
                    continue;
                };
                match canonicalize_workspace_path(repo_path) {
                    Ok(path) => {
                        if seen.insert(normalize_workspace_key(&path)) {
                            targets.push((slot.slot_id.clone(), path, plan_path.map(str::to_string)));
                        }
                    }
                    Err(error) => errors.push(format!(
                        "Project {} repository '{}' is unavailable: {error}",
                        slot.slot_id, repo_path
                    )),
                }
            }
        }
        AgentOptWorkspaceMode::ManualOverride => {
            if let Some(raw) = manual_workspace.map(str::trim).filter(|s| !s.is_empty()) {
                match canonicalize_workspace_path(raw) {
                    Ok(path) => targets.push(("manual".to_string(), path, None)),
                    Err(error) => errors.push(format!(
                        "Manual workspace '{}' is unavailable: {error}",
                        raw
                    )),
                }
            } else {
                errors.push("Manual workspace mode is active but no directory is saved".to_string());
            }
        }
    }
    (targets, errors)
}

type ProjectAgentOptTarget = (String, PathBuf, PathBuf, Option<String>, Option<String>, bool);

fn resolve_project_agent_opt_targets(
    project_slots: &[crate::plan_manager::ProjectPlanSlot],
    settings: &NerfEngineSettings,
) -> (Vec<ProjectAgentOptTarget>, Vec<String>) {
    let mut targets = Vec::new();
    let mut errors = Vec::new();
    for slot in project_slots {
        let Some(raw_repo) = slot.repo_path.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        let repo = match canonicalize_workspace_path(raw_repo) {
            Ok(path) => path,
            Err(error) => {
                errors.push(format!("Project {} repository '{}' is unavailable: {error}", slot.slot_id, raw_repo));
                continue;
            }
        };
        let preference = settings.project_preferences.get(&slot.slot_id).filter(|pref| {
            canonicalize_workspace_path(&pref.repository_path)
                .map(|saved| normalize_workspace_key(&saved) == normalize_workspace_key(&repo))
                .unwrap_or(false)
        });
        let manual = preference.and_then(|pref| pref.manual_workspace.clone());
        let target = match manual.as_deref() {
            Some(raw) if !raw.trim().is_empty() => match canonicalize_workspace_path(raw) {
                Ok(path) => path,
                Err(error) => {
                    errors.push(format!("Project {} manual workspace '{}' is unavailable: {error}", slot.slot_id, raw));
                    repo.clone()
                }
            },
            _ => repo.clone(),
        };
        targets.push((
            slot.slot_id.clone(),
            repo,
            target,
            slot.plan_path.clone(),
            manual,
            settings.project_preferences.contains_key(&slot.slot_id),
        ));
    }
    let mut target_owners: BTreeMap<String, (String, bool)> = BTreeMap::new();
    for (slot_id, _repo, target, _plan, _manual, _) in &targets {
        let Some(pref) = settings.project_preferences.get(slot_id).filter(|pref| {
            project_slots.iter().find(|slot| slot.slot_id == *slot_id)
                .and_then(|slot| slot.repo_path.as_deref())
                .and_then(|raw| canonicalize_workspace_path(raw).ok())
                .zip(canonicalize_workspace_path(&pref.repository_path).ok())
                .is_some_and(|(repo, saved)| normalize_workspace_key(&repo) == normalize_workspace_key(&saved))
        }) else { continue; };
        let key = normalize_workspace_key(target);
        if let Some((other_slot, other_value)) = target_owners.get(&key) {
            if *other_value != pref.use_agent_opt {
                errors.push(format!(
                    "Project {slot_id} conflicts with Project {other_slot}: both slots target the same workspace but request different AgentOpt states."
                ));
            }
        } else {
            target_owners.insert(key, (slot_id.clone(), pref.use_agent_opt));
        }
    }
    (targets, errors)
}

fn single_workspace_project_slots(
    workspace: Option<&str>,
) -> Vec<crate::plan_manager::ProjectPlanSlot> {
    workspace
        .filter(|path| !path.trim().is_empty())
        .map(|repo_path| {
            vec![crate::plan_manager::ProjectPlanSlot {
                slot_id: "primary".to_string(),
                repo_path: Some(repo_path.to_string()),
                plan_path: None,
            }]
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentOptCliInfo {
    pub installed: bool,
    pub version: Option<String>,
    pub supported_version: bool,
    pub cli_path: Option<String>,
    pub node_version: Option<String>,
    pub node_supported: bool,
    pub npm_available: bool,
    pub pinned_version: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentOptConfigView {
    pub raw_valid: bool,
    pub schema_supported: bool,
    pub enabled: Option<bool>,
    pub mode: Option<String>,
    pub raw: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentOptWorkspaceInspection {
    pub workspace_path: String,
    pub registered: bool,
    pub state: AgentOptWorkspaceState,
    pub antigravity_plugin_present: bool,
    pub config_present: bool,
    pub hooks_enabled: Option<bool>,
    pub config: Option<AgentOptConfigView>,
    pub unregistered_files_detected: bool,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentOptWorkspaceTarget {
    pub project_slot_id: String,
    pub repository_path: String,
    pub workspace_path: String,
    pub plan_path: Option<String>,
    pub use_agent_opt: bool,
    pub manual_workspace: Option<String>,
    pub inspection: AgentOptWorkspaceInspection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineMismatchInfo {
    pub has_mismatch: bool,
    pub reason: Option<String>,
    pub observed_rule_state: PovDirectFileState,
    pub enabled_workspaces: Vec<String>,
    pub selected_workspace_state: Option<AgentOptWorkspaceState>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AntigravityGlobalHookDiagnosticState {
    UnsupportedPlatform,
    CliUnavailable,
    PluginAbsent,
    Disabled,
    RiskPatternDetected,
    Unverified,
    InvalidPluginList,
    UnsafePath,
    InvalidManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AntigravityGlobalHookDiagnostic {
    pub state: AntigravityGlobalHookDiagnosticState,
    pub plugin_version: Option<String>,
    pub event: Option<String>,
}

pub trait AntigravityPluginListRunner: Send + Sync {
    fn list_plugins(&self, agy_executable: &Path) -> Result<CliExecOutput, String>;
}

pub struct RealAntigravityPluginListRunner;

impl AntigravityPluginListRunner for RealAntigravityPluginListRunner {
    fn list_plugins(&self, agy_executable: &Path) -> Result<CliExecOutput, String> {
        run_simple_command(
            agy_executable.to_string_lossy().as_ref(),
            &["plugin", "list", "--json"],
            5,
        )
    }
}

pub trait AntigravityPluginRemediationRunner: AntigravityPluginListRunner {
    fn disable_wt_agent_hooks(&self, agy_executable: &Path) -> Result<CliExecOutput, String>;
}

impl AntigravityPluginRemediationRunner for RealAntigravityPluginListRunner {
    fn disable_wt_agent_hooks(&self, agy_executable: &Path) -> Result<CliExecOutput, String> {
        run_simple_command(
            agy_executable.to_string_lossy().as_ref(),
            &["plugin", "disable", "wt-agent-hooks"],
            10,
        )
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NerfEngineOverview {
    pub persisted_engine: NerfEngine,
    pub workspace_mode: AgentOptWorkspaceMode,
    pub manual_workspace: Option<String>,
    pub active_plan_workspace: Option<String>,
    pub selected_workspace: Option<String>,
    pub registered_workspaces: Vec<String>,
    pub workspace_targets: Vec<AgentOptWorkspaceTarget>,
    pub target_resolution_errors: Vec<String>,
    pub cli: AgentOptCliInfo,
    pub workspace_inspection: Option<AgentOptWorkspaceInspection>,
    pub pov_direct_status: PovDirectStatus,
    pub mismatch: EngineMismatchInfo,
    pub exclusivity_scope_note: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global_hook_diagnostic: Option<AntigravityGlobalHookDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineTransitionResult {
    pub overview: NerfEngineOverview,
    pub changed: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliExecOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub trait AgentOptCliRunner: Send + Sync {
    fn node_version(&self) -> Result<String, String>;
    fn npm_version(&self) -> Result<String, String>;
    fn install_pinned_cli(&self, cli_dir: &Path, tarball_url: &str) -> Result<CliExecOutput, String>;
    fn run_aopt(&self, cli_entry: &Path, args: &[&str], timeout_secs: u64) -> Result<CliExecOutput, String>;
}

pub struct RealAgentOptCliRunner;

impl AgentOptCliRunner for RealAgentOptCliRunner {
    fn node_version(&self) -> Result<String, String> {
        run_simple_command("node", &["--version"], 10)
            .map(|o| o.stdout.trim().to_string())
    }

    fn npm_version(&self) -> Result<String, String> {
        let npm_bin = if cfg!(windows) { "npm.cmd" } else { "npm" };
        run_simple_command(npm_bin, &["--version"], 10)
            .map(|o| o.stdout.trim().to_string())
    }

    fn install_pinned_cli(&self, cli_dir: &Path, tarball_url: &str) -> Result<CliExecOutput, String> {
        let npm_bin = if cfg!(windows) { "npm.cmd" } else { "npm" };
        let prefix = cli_dir.to_string_lossy().to_string();
        run_simple_command(
            npm_bin,
            &["install", "--no-save", "--prefix", &prefix, tarball_url],
            120,
        )
    }

    fn run_aopt(&self, cli_entry: &Path, args: &[&str], timeout_secs: u64) -> Result<CliExecOutput, String> {
        let entry_str = cli_entry.to_string_lossy().to_string();
        let mut full_args = vec![entry_str.as_str()];
        full_args.extend_from_slice(args);
        run_simple_command("node", &full_args, timeout_secs)
    }
}

const MAX_CLI_OUTPUT_BYTES: usize = 1024 * 1024;

fn read_capped_stream<R: std::io::Read + Send + 'static>(
    mut stream: R,
    max_bytes: usize,
) -> std::thread::JoinHandle<Result<Vec<u8>, String>> {
    std::thread::spawn(move || {
        let mut collected = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if collected.len() < max_bytes {
                        let remaining = max_bytes - collected.len();
                        let to_take = n.min(remaining);
                        collected.extend_from_slice(&buf[..to_take]);
                    }
                    // Continue draining remaining bytes so the child process never blocks on a full pipe.
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(format!("Failed to read child stream: {e}")),
            }
        }
        Ok(collected)
    })
}

fn run_simple_command(program: &str, args: &[&str], timeout_secs: u64) -> Result<CliExecOutput, String> {
    let mut cmd = std::process::Command::new(program);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn '{program}': {e}"))?;

    let stdout_handle = child
        .stdout
        .take()
        .map(|s| read_capped_stream(s, MAX_CLI_OUTPUT_BYTES));
    let stderr_handle = child
        .stderr
        .take()
        .map(|s| read_capped_stream(s, MAX_CLI_OUTPUT_BYTES));

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    if let Some(h) = stdout_handle {
                        let _ = h.join();
                    }
                    if let Some(h) = stderr_handle {
                        let _ = h.join();
                    }
                    return Err(format!("Command '{program}' timed out after {timeout_secs}s"));
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Failed waiting for '{program}': {e}"));
            }
        }
    };

    let stdout_bytes = match stdout_handle {
        Some(h) => h
            .join()
            .map_err(|_| format!("Stdout reader thread panicked for '{program}'"))??,
        None => Vec::new(),
    };
    let stderr_bytes = match stderr_handle {
        Some(h) => h
            .join()
            .map_err(|_| format!("Stderr reader thread panicked for '{program}'"))??,
        None => Vec::new(),
    };

    Ok(CliExecOutput {
        exit_code: status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
        stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
    })
}

pub fn parse_node_major(raw: &str) -> Option<u64> {
    let cleaned = raw.trim().trim_start_matches('v');
    cleaned.split('.').next()?.parse::<u64>().ok()
}

pub fn canonicalize_workspace_path(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Workspace path cannot be empty".to_string());
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(format!("Workspace path must be absolute: '{trimmed}'"));
    }
    if !path.exists() || !path.is_dir() {
        return Err(format!("Workspace directory does not exist: '{trimmed}'"));
    }
    path.canonicalize()
        .map_err(|e| format!("Failed to canonicalize workspace '{trimmed}': {e}"))
}

fn normalize_workspace_key(path: &Path) -> String {
    let s = path.to_string_lossy().to_string();
    #[cfg(windows)]
    {
        s.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        s
    }
}

fn load_nerf_engine_settings(settings_file: &Path) -> Result<NerfEngineSettings, String> {
    if !settings_file.exists() {
        return Ok(NerfEngineSettings::default());
    }
    let raw = std::fs::read_to_string(settings_file)
        .map_err(|e| format!("Failed to read Nerf engine settings: {e}"))?;
    serde_json::from_str::<NerfEngineSettings>(&raw)
        .map_err(|e| format!("Failed to parse Nerf engine settings: {e}"))
}

fn save_nerf_engine_settings(settings_file: &Path, settings: &NerfEngineSettings) -> Result<(), String> {
    save_nerf_engine_settings_with_rename(settings_file, settings, |src, dst| {
        std::fs::rename(src, dst)
    })
}

fn save_nerf_engine_settings_with_rename<F>(
    settings_file: &Path,
    settings: &NerfEngineSettings,
    mut rename_fn: F,
) -> Result<(), String>
where
    F: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    let parent = settings_file
        .parent()
        .ok_or_else(|| "Engine settings file has no parent directory".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("Failed to create settings directory: {e}"))?;
    if settings_file.exists() {
        let backup_dir = parent.join("backups");
        pov_direct::create_backup_unpruned(settings_file, &backup_dir)?;
    }
    let bytes = serde_json::to_vec_pretty(settings)
        .map_err(|e| format!("Failed to serialize Nerf engine settings: {e}"))?;
    let tmp = parent.join(format!(
        ".antigravity-nerf-engine.json.tmp-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&tmp, bytes)
        .map_err(|e| format!("Failed to write staged Nerf engine settings: {e}"))?;

    if !settings_file.exists() {
        return rename_fn(&tmp, settings_file).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("Failed to commit Nerf engine settings: {e}")
        });
    }

    #[cfg(not(windows))]
    {
        return rename_fn(&tmp, settings_file).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("Failed to commit Nerf engine settings: {e}")
        });
    }

    #[cfg(windows)]
    {
        // On Windows, rename over an existing file can fail; move existing aside to a swap path first,
        // then commit tmp, and restore the swap path if committing tmp fails.
        let swap = parent.join(format!(
            ".antigravity-nerf-engine.json.swap-{}",
            uuid::Uuid::new_v4()
        ));
        if let Err(e) = std::fs::rename(settings_file, &swap) {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("Failed to stage existing Nerf engine settings for replacement: {e}"));
        }
        match rename_fn(&tmp, settings_file) {
            Ok(()) => {
                let _ = std::fs::remove_file(&swap);
                Ok(())
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                if let Err(re) = std::fs::rename(&swap, settings_file) {
                    return Err(format!(
                        "Failed to commit Nerf engine settings ({e}) and failed to restore swap ({re})"
                    ));
                }
                Err(format!("Failed to commit Nerf engine settings (original preserved): {e}"))
            }
        }
    }
}

pub fn inspect_cli_info(cli_dir: &Path, runner: &dyn AgentOptCliRunner) -> AgentOptCliInfo {
    let node_ver = runner.node_version().ok();
    let node_supported = node_ver
        .as_deref()
        .and_then(parse_node_major)
        .map(|m| m >= MIN_NODE_MAJOR)
        .unwrap_or(false);
    let npm_available = runner.npm_version().is_ok();

    let pkg_json = cli_dir
        .join("node_modules")
        .join("agent-opt")
        .join("package.json");
    let cli_entry = cli_dir
        .join("node_modules")
        .join("agent-opt")
        .join("dist")
        .join("cli")
        .join("index.js");

    if !pkg_json.is_file() || !cli_entry.is_file() {
        return AgentOptCliInfo {
            installed: false,
            version: None,
            supported_version: false,
            cli_path: None,
            node_version: node_ver,
            node_supported,
            npm_available,
            pinned_version: PINNED_AGENT_OPT_VERSION.to_string(),
            error: None,
        };
    }

    match std::fs::read_to_string(&pkg_json)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("version").and_then(|x| x.as_str()).map(|s| s.to_string()))
    {
        Some(ver) => {
            let supported = ver == PINNED_AGENT_OPT_VERSION;
            AgentOptCliInfo {
                installed: true,
                version: Some(ver.clone()),
                supported_version: supported,
                cli_path: Some(cli_entry.to_string_lossy().to_string()),
                node_version: node_ver,
                node_supported,
                npm_available,
                pinned_version: PINNED_AGENT_OPT_VERSION.to_string(),
                error: if supported {
                    None
                } else {
                    Some(format!(
                        "Unsupported AgentOpt version '{ver}'; expected pinned '{PINNED_AGENT_OPT_VERSION}'"
                    ))
                },
            }
        }
        None => AgentOptCliInfo {
            installed: true,
            version: None,
            supported_version: false,
            cli_path: Some(cli_entry.to_string_lossy().to_string()),
            node_version: node_ver,
            node_supported,
            npm_available,
            pinned_version: PINNED_AGENT_OPT_VERSION.to_string(),
            error: Some("Failed to read installed AgentOpt package.json version".to_string()),
        },
    }
}

fn validate_opt_bool(obj: &serde_json::Map<String, serde_json::Value>, section: &str, key: &str) -> Result<(), String> {
    if let Some(v) = obj.get(key) {
        if !v.is_boolean() {
            return Err(format!("AgentOpt config '{section}.{key}' must be a boolean"));
        }
    }
    Ok(())
}

fn validate_opt_pos_u64(obj: &serde_json::Map<String, serde_json::Value>, section: &str, key: &str, allow_zero: bool) -> Result<(), String> {
    if let Some(v) = obj.get(key) {
        let n = v
            .as_u64()
            .ok_or_else(|| format!("AgentOpt config '{section}.{key}' must be a non-negative integer"))?;
        if !allow_zero && n == 0 {
            return Err(format!("AgentOpt config '{section}.{key}' must be > 0"));
        }
    }
    Ok(())
}

fn validate_opt_unit_float(obj: &serde_json::Map<String, serde_json::Value>, section: &str, key: &str) -> Result<(), String> {
    if let Some(v) = obj.get(key) {
        let n = v
            .as_f64()
            .ok_or_else(|| format!("AgentOpt config '{section}.{key}' must be a number between 0.0 and 1.0"))?;
        if !n.is_finite() || !(0.0..=1.0).contains(&n) {
            return Err(format!("AgentOpt config '{section}.{key}' must be between 0.0 and 1.0"));
        }
    }
    Ok(())
}

pub fn validate_agent_opt_config_value(val: &serde_json::Value) -> Result<(Option<bool>, Option<String>), String> {
    let obj = val
        .as_object()
        .ok_or_else(|| "AgentOpt config root must be a JSON object".to_string())?;

    let enabled = match obj.get("enabled") {
        Some(v) => Some(
            v.as_bool()
                .ok_or_else(|| "AgentOpt config 'enabled' must be a boolean".to_string())?,
        ),
        None => None,
    };

    let mode = match obj.get("mode") {
        Some(v) => {
            let s = v
                .as_str()
                .ok_or_else(|| "AgentOpt config 'mode' must be a string".to_string())?;
            if !matches!(s, "FAST" | "BALANCED" | "STRICT") {
                return Err(format!("Unsupported AgentOpt mode '{s}' in config"));
            }
            Some(s.to_string())
        }
        None => None,
    };

    if let Some(sec_val) = obj.get("context") {
        let sec = sec_val
            .as_object()
            .ok_or_else(|| "AgentOpt config 'context' must be a JSON object".to_string())?;
        validate_opt_bool(sec, "context", "enabled")?;
        validate_opt_pos_u64(sec, "context", "maxRankedFiles", false)?;
        validate_opt_pos_u64(sec, "context", "maxRepoMapTokens", false)?;
        validate_opt_bool(sec, "context", "deduplicateReads")?;
        validate_opt_pos_u64(sec, "context", "blockRedundantFullReadsAfter", false)?;
        validate_opt_bool(sec, "context", "minQuerySpecificityForInjection")?;
    }

    if let Some(sec_val) = obj.get("verification") {
        let sec = sec_val
            .as_object()
            .ok_or_else(|| "AgentOpt config 'verification' must be a JSON object".to_string())?;
        validate_opt_bool(sec, "verification", "enabled")?;
        validate_opt_bool(sec, "verification", "enforceOnStop")?;
        validate_opt_bool(sec, "verification", "autoRunFastTestsOnStop")?;
        validate_opt_pos_u64(sec, "verification", "maxStopBlocksPerSession", true)?;
        validate_opt_bool(sec, "verification", "requireTestsWhenCodeModified")?;
        validate_opt_bool(sec, "verification", "exemptDocOnlyChanges")?;
    }

    if let Some(sec_val) = obj.get("failureDetection") {
        let sec = sec_val
            .as_object()
            .ok_or_else(|| "AgentOpt config 'failureDetection' must be a JSON object".to_string())?;
        validate_opt_bool(sec, "failureDetection", "enabled")?;
        validate_opt_pos_u64(sec, "failureDetection", "maxRepeatedCommandFailures", false)?;
        validate_opt_pos_u64(sec, "failureDetection", "maxIdenticalErrors", false)?;
        validate_opt_pos_u64(sec, "failureDetection", "maxRedundantReadsOfUnchangedFile", false)?;
        validate_opt_bool(sec, "failureDetection", "detectEditOscillation")?;
    }

    if let Some(sec_val) = obj.get("memory") {
        let sec = sec_val
            .as_object()
            .ok_or_else(|| "AgentOpt config 'memory' must be a JSON object".to_string())?;
        validate_opt_bool(sec, "memory", "enabled")?;
        validate_opt_pos_u64(sec, "memory", "maxEntries", false)?;
        validate_opt_unit_float(sec, "memory", "minConfidenceToInject")?;
        validate_opt_bool(sec, "memory", "autoPruneContradictions")?;
    }

    Ok((enabled, mode))
}

fn dir_has_entries(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|mut it| it.next().is_some())
        .unwrap_or(false)
}

pub fn inspect_workspace_read_only(
    workspace: &Path,
    registered: bool,
    cli: &AgentOptCliInfo,
) -> AgentOptWorkspaceInspection {
    let ws_str = workspace.to_string_lossy().to_string();
    let plugin_dir = workspace.join(".agents").join("plugins").join("agent-opt");
    let plugin_manifest = plugin_dir.join("plugin.json");
    let hooks_file = plugin_dir.join("hooks.json");
    let config_dir = workspace.join(".agent-opt");
    let config_file = config_dir.join("config.json");

    let plugin_present = plugin_manifest.is_file();
    let (hooks_enabled, hooks_error) = if plugin_present {
        match read_agent_opt_hook_group_enabled(&hooks_file) {
            Ok(enabled) => (Some(enabled), None),
            Err(error) => (None, Some(error)),
        }
    } else {
        (None, None)
    };
    let config_present = config_file.is_file();
    let unregistered_files_detected = !registered
        && (plugin_present
            || config_present
            || dir_has_entries(&plugin_dir)
            || dir_has_entries(&config_dir));

    if !cli.installed {
        return AgentOptWorkspaceInspection {
            workspace_path: ws_str,
            registered,
            state: AgentOptWorkspaceState::NotInstalled,
            antigravity_plugin_present: plugin_present,
            config_present,
            hooks_enabled,
            config: None,
            unregistered_files_detected,
            detail: Some("AgentOpt CLI is not installed".to_string()),
        };
    }

    if !cli.supported_version {
        return AgentOptWorkspaceInspection {
            workspace_path: ws_str,
            registered,
            state: AgentOptWorkspaceState::UnsupportedVersion,
            antigravity_plugin_present: plugin_present,
            config_present,
            hooks_enabled,
            config: None,
            unregistered_files_detected,
            detail: cli.error.clone(),
        };
    }

    if !plugin_present || !config_present {
        return AgentOptWorkspaceInspection {
            workspace_path: ws_str,
            registered,
            state: AgentOptWorkspaceState::NotInitialized,
            antigravity_plugin_present: plugin_present,
            config_present,
            hooks_enabled,
            config: None,
            unregistered_files_detected,
            detail: Some(if hooks_error.is_some() {
                format!("AgentOpt Antigravity hooks cannot be verified: {}", hooks_error.unwrap())
            } else {
                "AgentOpt is not initialized for Antigravity in this workspace".to_string()
            }),
        };
    }

    let raw_str = match std::fs::read_to_string(&config_file) {
        Ok(s) => s,
        Err(e) => {
            return AgentOptWorkspaceInspection {
                workspace_path: ws_str,
                registered,
                state: AgentOptWorkspaceState::InvalidConfig,
                antigravity_plugin_present: plugin_present,
                config_present,
                hooks_enabled,
                config: Some(AgentOptConfigView {
                    raw_valid: false,
                    schema_supported: false,
                    enabled: None,
                    mode: None,
                    raw: None,
                    error: Some(format!("Failed to read .agent-opt/config.json: {e}")),
                }),
                unregistered_files_detected,
                detail: Some(format!("Failed to read .agent-opt/config.json: {e}")),
            };
        }
    };

    let val: serde_json::Value = match serde_json::from_str(&raw_str) {
        Ok(v) => v,
        Err(e) => {
            return AgentOptWorkspaceInspection {
                workspace_path: ws_str,
                registered,
                state: AgentOptWorkspaceState::InvalidConfig,
                antigravity_plugin_present: plugin_present,
                config_present,
                hooks_enabled,
                config: Some(AgentOptConfigView {
                    raw_valid: false,
                    schema_supported: false,
                    enabled: None,
                    mode: None,
                    raw: None,
                    error: Some(format!("Malformed .agent-opt/config.json: {e}")),
                }),
                unregistered_files_detected,
                detail: Some(format!("Malformed .agent-opt/config.json: {e}")),
            };
        }
    };

    match validate_agent_opt_config_value(&val) {
        Ok((enabled, mode)) => {
            let config_enabled = enabled.unwrap_or(true);
            let Some(hooks_are_enabled) = hooks_enabled else {
                let error = hooks_error.unwrap_or_else(|| "AgentOpt Antigravity hooks cannot be verified".to_string());
                return AgentOptWorkspaceInspection {
                    workspace_path: ws_str,
                    registered,
                    state: AgentOptWorkspaceState::Error,
                    antigravity_plugin_present: plugin_present,
                    config_present,
                    hooks_enabled: None,
                    config: Some(AgentOptConfigView {
                        raw_valid: true,
                        schema_supported: true,
                        enabled: Some(config_enabled),
                        mode,
                        raw: Some(val),
                        error: Some(error.clone()),
                    }),
                    unregistered_files_detected,
                    detail: Some(error),
                };
            };
            let mismatch = config_enabled != hooks_are_enabled;
            AgentOptWorkspaceInspection {
                workspace_path: ws_str,
                registered,
                state: if mismatch {
                    AgentOptWorkspaceState::Error
                } else if config_enabled {
                    AgentOptWorkspaceState::Enabled
                } else {
                    AgentOptWorkspaceState::Disabled
                },
                antigravity_plugin_present: plugin_present,
                config_present,
                hooks_enabled: Some(hooks_are_enabled),
                config: Some(AgentOptConfigView {
                    raw_valid: true,
                    schema_supported: true,
                    enabled: Some(config_enabled),
                    mode,
                    raw: Some(val),
                    error: mismatch.then(|| "AgentOpt config and Antigravity hooks have different enabled states".to_string()),
                }),
                unregistered_files_detected,
                detail: mismatch.then(|| "AgentOpt config and Antigravity hooks have different enabled states".to_string()),
            }
        }
        Err(err) => AgentOptWorkspaceInspection {
            workspace_path: ws_str,
            registered,
            state: AgentOptWorkspaceState::InvalidConfig,
            antigravity_plugin_present: plugin_present,
            config_present,
            hooks_enabled,
            config: Some(AgentOptConfigView {
                raw_valid: true,
                schema_supported: false,
                enabled: None,
                mode: None,
                raw: Some(val),
                error: Some(err.clone()),
            }),
            unregistered_files_detected,
            detail: Some(err),
        },
    }
}

pub fn inspect_global_antigravity_hook_diagnostic(
    home: Option<&Path>,
    runner: &dyn AntigravityPluginListRunner,
) -> AntigravityGlobalHookDiagnostic {
    inspect_global_antigravity_hook_diagnostic_with_platform(home, runner, cfg!(windows))
}

fn global_hook_diagnostic(
    state: AntigravityGlobalHookDiagnosticState,
    plugin_version: Option<String>,
    event: Option<String>,
) -> AntigravityGlobalHookDiagnostic {
    AntigravityGlobalHookDiagnostic {
        state,
        plugin_version,
        event,
    }
}

fn inspect_global_antigravity_hook_diagnostic_with_platform(
    home: Option<&Path>,
    runner: &dyn AntigravityPluginListRunner,
    is_windows: bool,
) -> AntigravityGlobalHookDiagnostic {
    use AntigravityGlobalHookDiagnosticState as State;

    if !is_windows {
        return global_hook_diagnostic(State::UnsupportedPlatform, None, None);
    }
    let Some(home) = home else {
        return global_hook_diagnostic(State::CliUnavailable, None, None);
    };
    let Ok(home) = home.canonicalize() else {
        return global_hook_diagnostic(State::CliUnavailable, None, None);
    };

    let agy_executable = home.join(".gemini").join("bin").join("agy.exe");
    match inspect_safe_path(&home, &agy_executable) {
        Ok(true) if agy_executable.is_file() => {}
        Ok(false) | Ok(true) => {
            return global_hook_diagnostic(State::CliUnavailable, None, None);
        }
        Err(()) => return global_hook_diagnostic(State::UnsafePath, None, None),
    }

    let output = match runner.list_plugins(&agy_executable) {
        Ok(output) if output.exit_code == 0 => output,
        Ok(_) | Err(_) => return global_hook_diagnostic(State::CliUnavailable, None, None),
    };
    let plugins: Vec<AntigravityPluginListEntry> = match serde_json::from_str(&output.stdout) {
        Ok(plugins) => plugins,
        Err(_) => return global_hook_diagnostic(State::InvalidPluginList, None, None),
    };
    let mut matches = plugins
        .into_iter()
        .filter(|plugin| plugin.name == "wt-agent-hooks");
    let Some(plugin) = matches.next() else {
        return global_hook_diagnostic(State::PluginAbsent, None, None);
    };
    if matches.next().is_some() {
        return global_hook_diagnostic(State::InvalidPluginList, None, None);
    }
    if !plugin.enabled {
        return global_hook_diagnostic(State::Disabled, None, None);
    }
    let Some(reported_path) = plugin.path.as_deref().map(Path::new) else {
        return global_hook_diagnostic(State::Unverified, plugin.version, None);
    };
    let expected_plugin_dir = home
        .join(".gemini")
        .join("config")
        .join("plugins")
        .join("wt-agent-hooks");
    if !reported_path.is_absolute() || !same_global_path(reported_path, &expected_plugin_dir) {
        return global_hook_diagnostic(State::UnsafePath, plugin.version, None);
    }
    match inspect_safe_path(&home, &expected_plugin_dir) {
        Ok(true) if expected_plugin_dir.is_dir() => {}
        Ok(false) | Ok(true) => {
            return global_hook_diagnostic(State::Unverified, plugin.version, None);
        }
        Err(()) => return global_hook_diagnostic(State::UnsafePath, plugin.version, None),
    }

    let metadata_path = expected_plugin_dir.join("intelligent-terminal.json");
    let hooks_path = expected_plugin_dir.join("hooks.json");
    for path in [&metadata_path, &hooks_path] {
        match inspect_safe_path(&home, path) {
            Ok(true) if path.is_file() => {}
            Ok(false) | Ok(true) => {
                return global_hook_diagnostic(State::InvalidManifest, plugin.version, None);
            }
            Err(()) => return global_hook_diagnostic(State::UnsafePath, plugin.version, None),
        }
    }

    let metadata: serde_json::Value = match std::fs::read_to_string(&metadata_path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
    {
        Some(metadata) => metadata,
        None => return global_hook_diagnostic(State::InvalidManifest, plugin.version, None),
    };
    let Some(metadata_version) = metadata.get("version").and_then(serde_json::Value::as_str) else {
        return global_hook_diagnostic(State::Unverified, plugin.version, None);
    };
    let version = metadata_version.to_string();
    if metadata.get("name").and_then(serde_json::Value::as_str) != Some("wt-agent-hooks")
        || !metadata
            .get("managed_by")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|managed_by| managed_by.starts_with("Intelligent Terminal"))
        || plugin
            .version
            .as_deref()
            .is_some_and(|listed| listed != metadata_version)
    {
        return global_hook_diagnostic(State::Unverified, Some(version), None);
    }

    let hooks: serde_json::Value = match std::fs::read_to_string(&hooks_path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
    {
        Some(hooks) => hooks,
        None => return global_hook_diagnostic(State::InvalidManifest, Some(version), None),
    };
    let has_quote_sensitive_pre_tool_use = hooks
        .get("wt-agent-hooks")
        .and_then(|plugin| plugin.get("PreToolUse"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|groups| {
            groups.iter().any(|group| {
                let matcher = group.get("matcher").and_then(serde_json::Value::as_str);
                if !matches!(matcher, Some("*" | "")) {
                    return false;
                }
                group
                    .get("hooks")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|handlers| {
                        handlers.iter().any(|handler| {
                            handler.get("type").and_then(serde_json::Value::as_str)
                                == Some("command")
                                && handler
                                    .get("command")
                                    .and_then(serde_json::Value::as_str)
                                    .is_some_and(|command| {
                                        command.contains(
                                            r#"[Console]::Out.WriteLine('{"decision":"ask"}')"#,
                                        )
                                    })
                        })
                    })
            })
        });
    if has_quote_sensitive_pre_tool_use && metadata_version == "0.1.2" {
        global_hook_diagnostic(
            State::RiskPatternDetected,
            Some(version),
            Some("PreToolUse".to_string()),
        )
    } else {
        global_hook_diagnostic(State::Unverified, Some(version), None)
    }
}

pub fn disable_global_wt_agent_hooks(
    home: Option<&Path>,
    runner: &dyn AntigravityPluginRemediationRunner,
) -> Result<AntigravityGlobalHookDiagnostic, String> {
    disable_global_wt_agent_hooks_with_platform(home, runner, cfg!(windows))
}

fn disable_global_wt_agent_hooks_with_platform(
    home: Option<&Path>,
    runner: &dyn AntigravityPluginRemediationRunner,
    is_windows: bool,
) -> Result<AntigravityGlobalHookDiagnostic, String> {
    use AntigravityGlobalHookDiagnosticState as State;

    if !is_windows {
        return Err("Global wt-agent-hooks remediation is supported only on Windows.".to_string());
    }

    let current = inspect_global_antigravity_hook_diagnostic_with_platform(home, runner, is_windows);
    if current.state == State::Disabled {
        return Ok(current);
    }
    if current.state != State::RiskPatternDetected {
        return Err(format!(
            "Refusing to disable wt-agent-hooks because its current state is {:?}.",
            current.state
        ));
    }

    let home = home.ok_or_else(|| "Unable to determine the Windows user home directory.".to_string())?
        .canonicalize()
        .map_err(|e| format!("Unable to resolve the Windows user home directory: {e}"))?;
    let agy_executable = home.join(".gemini").join("bin").join("agy.exe");
    match inspect_safe_path(&home, &agy_executable) {
        Ok(true) if agy_executable.is_file() => {}
        Ok(false) | Ok(true) => return Err("The canonical Antigravity CLI is unavailable.".to_string()),
        Err(()) => return Err("The canonical Antigravity CLI path is unsafe.".to_string()),
    }

    let output = runner.disable_wt_agent_hooks(&agy_executable)?;
    if output.exit_code != 0 {
        return Err(format!(
            "Antigravity could not disable wt-agent-hooks (exit code {}).{}",
            output.exit_code,
            if output.stderr.trim().is_empty() {
                String::new()
            } else {
                format!(" {}", output.stderr.trim())
            }
        ));
    }

    let after = inspect_global_antigravity_hook_diagnostic_with_platform(Some(home.as_path()), runner, is_windows);
    if after.state != State::Disabled {
        return Err(format!(
            "The command completed, but a fresh Antigravity plugin list did not confirm wt-agent-hooks disabled (state: {:?}).",
            after.state
        ));
    }
    Ok(after)
}

#[derive(serde::Deserialize)]
struct AntigravityPluginListEntry {
    name: String,
    enabled: bool,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

fn same_global_path(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        let normalize = |path: &Path| {
            normalize_workspace_path_for_display(&path.to_string_lossy()).replace('/', "\\")
        };
        normalize(left).eq_ignore_ascii_case(&normalize(right))
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn inspect_safe_path(root: &Path, target: &Path) -> Result<bool, ()> {
    let root = root.canonicalize().map_err(|_| ())?;
    let relative = target.strip_prefix(&root).map_err(|_| ())?;
    let mut current = root;
    for component in relative.components() {
        current.push(component);
        let metadata = match std::fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(_) => return Err(()),
        };
        if is_symlink_or_reparse_point(&metadata) {
            return Err(());
        }
    }
    Ok(true)
}

fn is_symlink_or_reparse_point(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    metadata.file_type().is_symlink()
}
pub fn inspect_nerf_engine_overview_at(
    settings_file: &Path,
    cli_dir: &Path,
    rules_dir: &Path,
    skill_file: &Path,
    selected_workspace_override: Option<&str>,
    runner: &dyn AgentOptCliRunner,
) -> Result<NerfEngineOverview, String> {
    let project_slots = single_workspace_project_slots(selected_workspace_override);
    inspect_nerf_engine_overview_for_projects_at(
        settings_file,
        cli_dir,
        rules_dir,
        skill_file,
        &project_slots,
        runner,
    )
}

pub fn inspect_nerf_engine_overview_for_projects_at(
    settings_file: &Path,
    cli_dir: &Path,
    rules_dir: &Path,
    skill_file: &Path,
    project_slots: &[crate::plan_manager::ProjectPlanSlot],
    runner: &dyn AgentOptCliRunner,
) -> Result<NerfEngineOverview, String> {
    let settings = load_nerf_engine_settings(settings_file)?;
    let cli = inspect_cli_info(cli_dir, runner);
    let pov_status = pov_direct::inspect_pov_direct_status_at(rules_dir, skill_file)?;

    let (legacy_mode, legacy_manual) = resolve_persisted_workspace_selection(&settings);
    // New settings are attached to each repository slot. A legacy global
    // manual target has no safe slot identity, so never silently assign it.
    let workspace_mode = legacy_mode;
    let manual_workspace = legacy_manual.clone();
    let (configured_targets, mut target_resolution_errors) =
        resolve_project_agent_opt_targets(project_slots, &settings);
    if legacy_mode == AgentOptWorkspaceMode::ManualOverride && legacy_manual.is_some() {
        target_resolution_errors.push(
            "A legacy manual workspace is not assigned to either project. Set a manual path on the intended project card.".to_string(),
        );
    }

    let registered_keys: Vec<String> = settings
        .registered_workspaces
        .iter()
        .filter_map(|w| canonicalize_workspace_path(w).ok())
        .map(|p| normalize_workspace_key(&p))
        .collect();

    let workspace_targets: Vec<AgentOptWorkspaceTarget> = configured_targets
        .iter()
        .map(|(slot_id, repo_path, path, plan_path, manual_path, has_preference)| {
            let is_reg = registered_keys.contains(&normalize_workspace_key(path));
            let mut inspection = inspect_workspace_read_only(path, is_reg, &cli);
            inspection.workspace_path = normalize_workspace_path_for_display(&inspection.workspace_path);
            let use_agent_opt = settings.project_preferences.get(slot_id)
                .filter(|pref| canonicalize_workspace_path(&pref.repository_path)
                    .map(|saved| normalize_workspace_key(&saved) == normalize_workspace_key(repo_path))
                    .unwrap_or(false))
                .map(|pref| pref.use_agent_opt)
                .unwrap_or(!*has_preference && settings.engine == NerfEngine::Agentopt
                    && inspection.state == AgentOptWorkspaceState::Enabled);
            AgentOptWorkspaceTarget {
                project_slot_id: slot_id.clone(),
                repository_path: normalize_workspace_path_for_display(&repo_path.to_string_lossy()),
                workspace_path: normalize_workspace_path_for_display(&path.to_string_lossy()),
                plan_path: plan_path.clone(),
                use_agent_opt,
                manual_workspace: manual_path.as_ref().map(|p| normalize_workspace_path_for_display(p)),
                inspection,
            }
        })
        .collect();
    let workspace_inspections: Vec<AgentOptWorkspaceInspection> = workspace_targets
        .iter()
        .map(|target| target.inspection.clone())
        .collect();
    let workspace_inspection = workspace_inspections.first().cloned();

    let mut enabled_workspaces = Vec::new();
    let mut unverified_workspaces = Vec::new();
    for reg in &settings.registered_workspaces {
        if let Ok(canon) = canonicalize_workspace_path(reg) {
            let insp = inspect_workspace_read_only(&canon, true, &cli);
            if agent_opt_workspace_has_active_component(&insp) {
                enabled_workspaces.push(normalize_workspace_path_for_display(&insp.workspace_path));
            } else if insp.state == AgentOptWorkspaceState::NotInstalled
                && (insp.antigravity_plugin_present || insp.config_present)
            {
                unverified_workspaces.push(insp.workspace_path);
            } else if insp.state == AgentOptWorkspaceState::Error {
                unverified_workspaces.push(insp.workspace_path);
            }
        }
    }

    let selected_ws_state = if !workspace_inspections.is_empty()
        && workspace_inspections
            .iter()
            .all(|workspace| workspace.state == AgentOptWorkspaceState::Enabled)
    {
        Some(AgentOptWorkspaceState::Enabled)
    } else {
        workspace_inspections
            .iter()
            .find(|workspace| workspace.state != AgentOptWorkspaceState::Enabled)
            .map(|workspace| workspace.state)
    };
    let active_plan_workspace = project_slots
        .iter()
        .filter_map(|slot| slot.repo_path.as_deref())
        .find_map(|path| canonicalize_workspace_path(path).ok())
        .map(|path| normalize_workspace_path_for_display(&path.to_string_lossy()));
    let desired_workspace_keys: std::collections::HashSet<String> = workspace_targets
        .iter()
        .filter(|target| target.use_agent_opt)
        .filter_map(|target| canonicalize_workspace_path(&target.workspace_path).ok())
        .map(|path| normalize_workspace_key(&path))
        .collect();
    let enabled_non_target: Vec<&String> = enabled_workspaces
        .iter()
        .filter(|path| {
            canonicalize_workspace_path(path)
                .map(|canon| !desired_workspace_keys.contains(&normalize_workspace_key(&canon)))
                .unwrap_or(true)
        })
        .collect();
    let (has_mismatch, reason) = match settings.engine {
        NerfEngine::PoverelloRules => {
            if !enabled_workspaces.is_empty() {
                (
                    true,
                    Some(format!(
                        "Poverello Rules is selected, but AgentOpt is still enabled in {} registered workspace(s)",
                        enabled_workspaces.len()
                    )),
                )
            } else if !unverified_workspaces.is_empty() {
                (
                    true,
                    Some(format!(
                        "Poverello Rules is selected, but AgentOpt CLI is absent while AgentOpt files remain in {} registered workspace(s) (state unknown)",
                        unverified_workspaces.len()
                    )),
                )
            } else if pov_status.state == PovDirectFileState::Absent {
                (
                    true,
                    Some("Poverello Rules is selected, but pov-direct.md is absent".to_string()),
                )
            } else {
                (false, None)
            }
        }
        NerfEngine::Agentopt => {
            if pov_status.state != PovDirectFileState::Absent {
                (
                    true,
                    Some(format!(
                        "AgentOpt is selected, but global Poverello Rule pov-direct.md is still active ({:?})",
                        pov_status.state
                    )),
                )
            } else if !target_resolution_errors.is_empty() {
                (true, Some(target_resolution_errors.join("; ")))
            } else if workspace_targets.is_empty() {
                (
                    true,
                    Some("AgentOpt is selected, but no project workspaces are configured".to_string()),
                )
            } else if let Some(workspace) = workspace_targets
                .iter()
                .find(|target| target.use_agent_opt && target.inspection.state != AgentOptWorkspaceState::Enabled)
            {
                let reason = format!(
                    "AgentOpt is selected, but '{}' is not active (observed state: {:?})",
                    workspace.inspection.workspace_path, workspace.inspection.state
                );
                (true, Some(reason))
            } else if let Some(workspace) = workspace_targets.iter()
                .find(|target| !target.use_agent_opt && target.inspection.state == AgentOptWorkspaceState::Enabled)
            {
                (true, Some(format!("AgentOpt is enabled for Project {} although its per-project toggle is OFF", workspace.project_slot_id)))
            } else if !enabled_non_target.is_empty() {
                (
                    true,
                    Some(format!(
                        "AgentOpt is enabled in registered workspace(s) outside the target set: {}",
                        enabled_non_target
                            .iter()
                            .map(|path| path.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),
                )
            } else {
                (false, None)
            }
        }
    };

    Ok(NerfEngineOverview {
        persisted_engine: settings.engine,
        workspace_mode,
        manual_workspace: manual_workspace.clone(),
        active_plan_workspace,
        selected_workspace: if workspace_mode == AgentOptWorkspaceMode::ManualOverride {
            manual_workspace.clone().map(|path| normalize_workspace_path_for_display(&path))
        } else {
            workspace_targets.first().map(|target| target.workspace_path.clone())
        },
        registered_workspaces: settings.registered_workspaces,
        workspace_targets,
        target_resolution_errors,
        cli,
        workspace_inspection,
        pov_direct_status: pov_status.clone(),
        mismatch: EngineMismatchInfo {
            has_mismatch,
            reason,
            observed_rule_state: pov_status.state,
            enabled_workspaces,
            selected_workspace_state: selected_ws_state,
        },
        exclusivity_scope_note: "Exclusivity is enforced across the global Poverello Rule (pov-direct.md) and Poverello-registered AgentOpt workspaces. Unregistered workspaces are never modified automatically.".to_string(),
        global_hook_diagnostic: None,
    })
}

pub fn install_agent_opt_cli_at(
    cli_dir: &Path,
    runner: &dyn AgentOptCliRunner,
) -> Result<AgentOptCliInfo, String> {
    let _lock = ENGINE_TRANSITION_LOCK
        .lock()
        .map_err(|e| format!("Mutex poisoned: {e}"))?;
    let node_ver = runner
        .node_version()
        .map_err(|e| format!("Node.js is required (>= {MIN_NODE_MAJOR}): {e}"))?;
    let major = parse_node_major(&node_ver)
        .ok_or_else(|| format!("Could not parse Node.js version '{node_ver}'"))?;
    if major < MIN_NODE_MAJOR {
        return Err(format!(
            "Node.js >= {MIN_NODE_MAJOR} is required, found '{node_ver}'"
        ));
    }
    runner
        .npm_version()
        .map_err(|e| format!("npm is required to install AgentOpt: {e}"))?;

    std::fs::create_dir_all(cli_dir)
        .map_err(|e| format!("Failed to create AgentOpt CLI directory: {e}"))?;

    let out = runner.install_pinned_cli(cli_dir, PINNED_AGENT_OPT_TARBALL_URL)?;
    if out.exit_code != 0 {
        return Err(format!(
            "AgentOpt installation failed (exit {}): {}",
            out.exit_code,
            out.stderr.trim()
        ));
    }

    let info = inspect_cli_info(cli_dir, runner);
    if !info.installed || !info.supported_version {
        return Err(info
            .error
            .unwrap_or_else(|| "AgentOpt installation verification failed".to_string()));
    }
    Ok(info)
}

fn require_supported_cli_entry(cli_dir: &Path, runner: &dyn AgentOptCliRunner) -> Result<PathBuf, String> {
    let info = inspect_cli_info(cli_dir, runner);
    if !info.installed {
        return Err("AgentOpt CLI is not installed. Run explicit installation first.".to_string());
    }
    if !info.supported_version {
        return Err(info
            .error
            .unwrap_or_else(|| "Installed AgentOpt version is unsupported".to_string()));
    }
    info.cli_path
        .map(PathBuf::from)
        .ok_or_else(|| "AgentOpt CLI entrypoint missing".to_string())
}

fn is_workspace_registered(settings: &NerfEngineSettings, ws: &Path) -> bool {
    let key = normalize_workspace_key(ws);
    settings
        .registered_workspaces
        .iter()
        .filter_map(|w| canonicalize_workspace_path(w).ok())
        .any(|p| normalize_workspace_key(&p) == key)
}

fn register_workspace_in_settings(settings_file: &Path, ws: &Path) -> Result<(), String> {
    let mut settings = load_nerf_engine_settings(settings_file)?;
    if !is_workspace_registered(&settings, ws) {
        settings
            .registered_workspaces
            .push(ws.to_string_lossy().to_string());
        save_nerf_engine_settings(settings_file, &settings)?;
    }
    Ok(())
}

pub fn init_agent_opt_workspace_at(
    settings_file: &Path,
    cli_dir: &Path,
    workspace_raw: &str,
    mode: Option<&str>,
    runner: &dyn AgentOptCliRunner,
) -> Result<AgentOptWorkspaceInspection, String> {
    init_agent_opt_workspace_with_reg(
        settings_file,
        cli_dir,
        workspace_raw,
        mode,
        runner,
        register_workspace_in_settings,
    )
}

fn init_agent_opt_workspace_with_reg<F>(
    settings_file: &Path,
    cli_dir: &Path,
    workspace_raw: &str,
    mode: Option<&str>,
    runner: &dyn AgentOptCliRunner,
    mut register_fn: F,
) -> Result<AgentOptWorkspaceInspection, String>
where
    F: FnMut(&Path, &Path) -> Result<(), String>,
{
    let _lock = ENGINE_TRANSITION_LOCK
        .lock()
        .map_err(|e| format!("Mutex poisoned: {e}"))?;
    let ws = canonicalize_workspace_path(workspace_raw)?;
    let settings = load_nerf_engine_settings(settings_file)?;
    let is_registered = is_workspace_registered(&settings, &ws);
    let cli_info = inspect_cli_info(cli_dir, runner);
    let pre = inspect_workspace_read_only(&ws, is_registered, &cli_info);
    let plugin_dir = ws.join(".agents").join("plugins").join("agent-opt");
    let config_dir = ws.join(".agent-opt");
    if pre.antigravity_plugin_present
        || pre.config_present
        || dir_has_entries(&plugin_dir)
        || dir_has_entries(&config_dir)
    {
        return Err(
            "Cannot initialize workspace: AgentOpt files or directories already have existing content (possibly partial or unregistered). Register/adopt or inspect existing files first."
                .to_string(),
        );
    }

    let cli_entry = require_supported_cli_entry(cli_dir, runner)?;
    let ws_str = ws.to_string_lossy().to_string();

    let mut args = vec!["init", "--platform", "google-antigravity", "--yes", "--cwd", &ws_str];
    let mode_upper;
    if let Some(m) = mode {
        mode_upper = m.trim().to_uppercase();
        if !matches!(mode_upper.as_str(), "FAST" | "BALANCED" | "STRICT") {
            return Err(format!("Unsupported AgentOpt mode '{m}'"));
        }
        args.push("--mode");
        args.push(&mode_upper);
    }

    let out = runner.run_aopt(&cli_entry, &args, 60)?;
    if out.exit_code != 0 {
        return Err(format!(
            "AgentOpt init failed (exit {}): {}",
            out.exit_code,
            out.stderr.trim()
        ));
    }

    if let Err(reg_err) = register_fn(settings_file, &ws) {
        let mut cleanup_errs = Vec::new();
        let plugin_manifest = plugin_dir.join("plugin.json");
        if plugin_manifest.is_file() {
            if let Err(e) = std::fs::remove_file(&plugin_manifest) {
                cleanup_errs.push(format!("failed to remove {}: {e}", plugin_manifest.display()));
            }
        }
        let hooks_manifest = plugin_dir.join("hooks.json");
        if hooks_manifest.is_file() {
            if let Err(e) = std::fs::remove_file(&hooks_manifest) {
                cleanup_errs.push(format!("failed to remove {}: {e}", hooks_manifest.display()));
            }
        }
        if plugin_dir.is_dir() && !dir_has_entries(&plugin_dir) {
            let _ = std::fs::remove_dir(&plugin_dir);
        }
        let config_file = config_dir.join("config.json");
        if config_file.is_file() {
            if let Err(e) = std::fs::remove_file(&config_file) {
                cleanup_errs.push(format!("failed to remove {}: {e}", config_file.display()));
            }
        }
        if config_dir.is_dir() && !dir_has_entries(&config_dir) {
            let _ = std::fs::remove_dir(&config_dir);
        }
        if cleanup_errs.is_empty() {
            return Err(format!(
                "Failed to register workspace after init ({reg_err}); rolled back initialized AgentOpt files."
            ));
        }
        return Err(format!(
            "Failed to register workspace after init ({reg_err}); cleanup incomplete: {}",
            cleanup_errs.join("; ")
        ));
    }
    let cli = inspect_cli_info(cli_dir, runner);
    let insp = inspect_workspace_read_only(&ws, true, &cli);
    if matches!(
        insp.state,
        AgentOptWorkspaceState::NotInitialized | AgentOptWorkspaceState::InvalidConfig
    ) {
        return Err(format!(
            "AgentOpt init completed but workspace verification failed: {:?}",
            insp.detail
        ));
    }
    Ok(insp)
}

pub fn adopt_agent_opt_workspace_at(
    settings_file: &Path,
    cli_dir: &Path,
    workspace_raw: &str,
    runner: &dyn AgentOptCliRunner,
) -> Result<AgentOptWorkspaceInspection, String> {
    let _lock = ENGINE_TRANSITION_LOCK
        .lock()
        .map_err(|e| format!("Mutex poisoned: {e}"))?;
    let ws = canonicalize_workspace_path(workspace_raw)?;
    let cli = inspect_cli_info(cli_dir, runner);
    let pre = inspect_workspace_read_only(&ws, false, &cli);
    if !pre.antigravity_plugin_present || !pre.config_present {
        return Err("Cannot adopt workspace: AgentOpt Antigravity files are not initialized there".to_string());
    }
    if pre.state == AgentOptWorkspaceState::InvalidConfig {
        return Err("Cannot adopt workspace: .agent-opt/config.json is malformed or unsupported".to_string());
    }
    register_workspace_in_settings(settings_file, &ws)?;
    Ok(inspect_workspace_read_only(&ws, true, &cli))
}

fn backup_agent_opt_config(ws: &Path) -> Result<Option<Vec<u8>>, String> {
    let config_file = ws.join(".agent-opt").join("config.json");
    if !config_file.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(&config_file)
        .map_err(|e| format!("Failed to read .agent-opt/config.json for backup: {e}"))?;
    let backup_dir = ws.join(".agent-opt").join("backups");
    pov_direct::create_backup_unpruned(&config_file, &backup_dir)?;
    Ok(Some(bytes))
}

fn agent_opt_hooks_file(ws: &Path) -> PathBuf {
    ws.join(".agents").join("plugins").join("agent-opt").join("hooks.json")
}

fn validate_agent_opt_hook_parent_dirs(hooks_file: &Path) -> Result<(), String> {
    let mut current = hooks_file.parent().ok_or_else(|| "AgentOpt hooks.json has no parent directory".to_string())?;
    for _ in 0..3 {
        let metadata = std::fs::symlink_metadata(current)
            .map_err(|e| format!("Failed to inspect AgentOpt hook directory '{}': {e}", current.display()))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(format!("AgentOpt hook directory '{}' is not a real directory", current.display()));
        }
        current = current.parent().ok_or_else(|| "AgentOpt hook directory chain is incomplete".to_string())?;
    }
    Ok(())
}

fn read_agent_opt_hook_group_enabled(hooks_file: &Path) -> Result<bool, String> {
    validate_agent_opt_hook_parent_dirs(hooks_file)?;
    let metadata = std::fs::symlink_metadata(hooks_file)
        .map_err(|e| format!("Failed to inspect AgentOpt hooks.json: {e}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("AgentOpt hooks.json is not a regular file".to_string());
    }
    let bytes = std::fs::read(hooks_file)
        .map_err(|e| format!("Failed to read AgentOpt hooks.json: {e}"))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("AgentOpt hooks.json is malformed: {e}"))?;
    let group = value
        .as_object()
        .and_then(|root| root.get("agent-opt"))
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "AgentOpt hook group is missing or invalid".to_string())?;
    match group.get("enabled") {
        Some(value) => value
            .as_bool()
            .ok_or_else(|| "AgentOpt hook group's enabled field must be a boolean".to_string()),
        None => Ok(true), // Antigravity's documented default is enabled.
    }
}

fn backup_agent_opt_hooks(ws: &Path) -> Result<Vec<u8>, String> {
    let hooks_file = agent_opt_hooks_file(ws);
    validate_agent_opt_hook_parent_dirs(&hooks_file)?;
    let bytes = std::fs::read(&hooks_file)
        .map_err(|e| format!("Failed to read AgentOpt hooks.json for backup: {e}"))?;
    let config_dir = ws.join(".agent-opt");
    let config_metadata = std::fs::symlink_metadata(&config_dir)
        .map_err(|e| format!("Failed to inspect .agent-opt directory for backup: {e}"))?;
    if !config_metadata.is_dir() || config_metadata.file_type().is_symlink() {
        return Err(".agent-opt is not a real directory; refusing to write a backup through it".to_string());
    }
    let backup_dir = config_dir.join("backups");
    match std::fs::symlink_metadata(&backup_dir) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Err(".agent-opt/backups is not a real directory; refusing to write through it".to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(&backup_dir)
                .map_err(|e| format!("Failed to create AgentOpt backup directory: {e}"))?;
        }
        Err(error) => return Err(format!("Failed to inspect AgentOpt backup directory: {error}")),
    }
    let backup_path = backup_dir.join(format!("agent-opt-hooks-{}.json", uuid::Uuid::new_v4()));
    std::fs::write(&backup_path, &bytes)
        .map_err(|e| format!("Failed to write AgentOpt hooks backup: {e}"))?;
    if std::fs::read(&backup_path).map_err(|e| format!("Failed to verify AgentOpt hooks backup: {e}"))? != bytes {
        let _ = std::fs::remove_file(&backup_path);
        return Err("AgentOpt hooks backup verification failed".to_string());
    }
    Ok(bytes)
}

fn set_agent_opt_hook_group_enabled(ws: &Path, enabled: bool) -> Result<(), String> {
    let hooks_file = agent_opt_hooks_file(ws);
    validate_agent_opt_hook_parent_dirs(&hooks_file)?;
    let metadata = std::fs::symlink_metadata(&hooks_file)
        .map_err(|e| format!("Failed to inspect AgentOpt hooks.json: {e}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("AgentOpt hooks.json is not a regular file".to_string());
    }
    let bytes = std::fs::read(&hooks_file)
        .map_err(|e| format!("Failed to read AgentOpt hooks.json: {e}"))?;
    let mut value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("AgentOpt hooks.json is malformed: {e}"))?;
    let group = value
        .as_object_mut()
        .and_then(|root| root.get_mut("agent-opt"))
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| "AgentOpt hook group is missing or invalid".to_string())?;
    if group.get("enabled").is_some_and(|value| !value.is_boolean()) {
        return Err("AgentOpt hook group's enabled field must be a boolean".to_string());
    }
    group.insert("enabled".to_string(), serde_json::Value::Bool(enabled));
    let updated = serde_json::to_vec_pretty(&value)
        .map_err(|e| format!("Failed to serialize AgentOpt hooks.json: {e}"))?;
    let parent = hooks_file.parent().ok_or_else(|| "AgentOpt hooks.json has no parent directory".to_string())?;
    let temp = parent.join(format!(".hooks.json.tmp-{}", uuid::Uuid::new_v4()));
    std::fs::write(&temp, updated)
        .map_err(|e| format!("Failed to write temporary AgentOpt hooks.json: {e}"))?;
    #[cfg(windows)]
    {
        let swap = parent.join(format!(".hooks.json.swap-{}", uuid::Uuid::new_v4()));
        replace_via_swap_with_rename(&temp, &hooks_file, &swap, |src, dst| std::fs::rename(src, dst))?;
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(&temp, &hooks_file)
            .map_err(|e| {
                let _ = std::fs::remove_file(&temp);
                format!("Failed to replace AgentOpt hooks.json: {e}")
            })?;
    }
    if read_agent_opt_hook_group_enabled(&hooks_file)? != enabled {
        return Err("AgentOpt hooks.json verification failed after update".to_string());
    }
    Ok(())
}

fn restore_agent_opt_hooks(ws: &Path, bytes: &[u8]) -> Result<(), String> {
    let hooks_file = agent_opt_hooks_file(ws);
    validate_agent_opt_hook_parent_dirs(&hooks_file)?;
    let parent = hooks_file.parent().ok_or_else(|| "AgentOpt hooks.json has no parent directory".to_string())?;
    let temp = parent.join(format!(".hooks.json.rollback-{}", uuid::Uuid::new_v4()));
    std::fs::write(&temp, bytes)
        .map_err(|e| format!("Failed to write AgentOpt hooks rollback file: {e}"))?;
    #[cfg(windows)]
    {
        let swap = parent.join(format!(".hooks.json.swap-{}", uuid::Uuid::new_v4()));
        replace_via_swap_with_rename(&temp, &hooks_file, &swap, |src, dst| std::fs::rename(src, dst))?;
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(&temp, &hooks_file)
            .map_err(|e| {
                let _ = std::fs::remove_file(&temp);
                format!("Failed to restore AgentOpt hooks.json: {e}")
            })?;
    }
    if std::fs::read(&hooks_file).map_err(|e| format!("Failed to verify AgentOpt hooks rollback: {e}"))? != bytes {
        return Err("AgentOpt hooks rollback verification mismatch".to_string());
    }
    Ok(())
}

fn agent_opt_workspace_config_enabled(inspection: &AgentOptWorkspaceInspection) -> bool {
    inspection.config.as_ref().and_then(|config| config.enabled).unwrap_or(false)
}

fn agent_opt_workspace_has_active_component(inspection: &AgentOptWorkspaceInspection) -> bool {
    agent_opt_workspace_config_enabled(inspection) || inspection.hooks_enabled.unwrap_or(false)
}

fn replace_via_swap_with_rename<F>(
    tmp: &Path,
    target: &Path,
    swap: &Path,
    mut rename_fn: F,
) -> Result<(), String>
where
    F: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    if let Err(e) = std::fs::rename(target, swap) {
        let _ = std::fs::remove_file(tmp);
        return Err(format!(
            "Failed to stage existing {} for replacement: {e}",
            target.display()
        ));
    }
    match rename_fn(tmp, target) {
        Ok(()) => {
            let _ = std::fs::remove_file(swap);
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(tmp);
            if let Err(re) = std::fs::rename(swap, target) {
                return Err(format!(
                    "Failed to replace {} ({e}) and failed to restore swap ({re})",
                    target.display()
                ));
            }
            Err(format!(
                "Failed to replace {} (original preserved): {e}",
                target.display()
            ))
        }
    }
}

fn restore_agent_opt_config_bytes(ws: &Path, bytes: &[u8]) -> Result<(), String> {
    restore_agent_opt_config_bytes_with_rename(ws, bytes, |src, dst| std::fs::rename(src, dst))
}

fn restore_agent_opt_config_bytes_with_rename<F>(
    ws: &Path,
    bytes: &[u8],
    mut rename_fn: F,
) -> Result<(), String>
where
    F: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    let dir = ws.join(".agent-opt");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create .agent-opt directory during rollback: {e}"))?;
    let config_file = dir.join("config.json");
    let tmp = dir.join(format!(".config.json.rollback-{}", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, bytes)
        .map_err(|e| format!("Failed to write rollback config temp file: {e}"))?;

    if !config_file.exists() {
        return rename_fn(&tmp, &config_file).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("Failed to write .agent-opt/config.json: {e}")
        });
    }

    #[cfg(not(windows))]
    {
        return rename_fn(&tmp, &config_file).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("Failed to replace .agent-opt/config.json: {e}")
        });
    }

    #[cfg(windows)]
    {
        let swap = dir.join(format!(".config.json.swap-{}", uuid::Uuid::new_v4()));
        replace_via_swap_with_rename(&tmp, &config_file, &swap, rename_fn)
    }
}

pub fn enable_agent_opt_workspace_at(
    settings_file: &Path,
    cli_dir: &Path,
    rules_dir: &Path,
    skill_file: &Path,
    workspace_raw: &str,
    runner: &dyn AgentOptCliRunner,
) -> Result<AgentOptWorkspaceInspection, String> {    let _ = (settings_file, cli_dir, rules_dir, skill_file, workspace_raw, runner);
    Err("Per-workspace AgentOpt enable is disabled. Apply the selected engine to all configured projects.".to_string())
}
pub fn set_agent_opt_mode_at(
    settings_file: &Path,
    cli_dir: &Path,
    workspace_raw: &str,
    mode: &str,
    runner: &dyn AgentOptCliRunner,
) -> Result<AgentOptWorkspaceInspection, String> {
    let _lock = ENGINE_TRANSITION_LOCK
        .lock()
        .map_err(|e| format!("Mutex poisoned: {e}"))?;
    let ws = canonicalize_workspace_path(workspace_raw)?;
    let settings = load_nerf_engine_settings(settings_file)?;
    if !is_workspace_registered(&settings, &ws) {
        return Err(
            "Cannot set AgentOpt mode for an unregistered workspace. Explicitly register/adopt the workspace first."
                .to_string(),
        );
    }
    let mode_upper = mode.trim().to_uppercase();
    if !matches!(mode_upper.as_str(), "FAST" | "BALANCED" | "STRICT") {
        return Err(format!("Unsupported AgentOpt mode '{mode}'"));
    }
    let cli = inspect_cli_info(cli_dir, runner);
    let cli_entry = require_supported_cli_entry(cli_dir, runner)?;
    let pre = inspect_workspace_read_only(&ws, true, &cli);
    if matches!(
        pre.state,
        AgentOptWorkspaceState::NotInitialized
            | AgentOptWorkspaceState::InvalidConfig
            | AgentOptWorkspaceState::UnsupportedVersion
    ) {
        return Err(pre
            .detail
            .unwrap_or_else(|| format!("Cannot set mode in state {:?}", pre.state)));
    }

    let backup_bytes = backup_agent_opt_config(&ws)?;
    let ws_str = ws.to_string_lossy().to_string();
    let out = runner.run_aopt(&cli_entry, &["mode", &mode_upper, "--cwd", &ws_str], 30)?;
    if out.exit_code != 0 {
        let mut rb_err = None;
        if let Some(ref b) = backup_bytes {
            if let Err(re) = restore_agent_opt_config_bytes(&ws, b) {
                rb_err = Some(re);
            }
        }
        return Err(match rb_err {
            Some(re) => format!(
                "AgentOpt mode switch failed (exit {}): {}; rollback also failed: {re}",
                out.exit_code,
                out.stderr.trim()
            ),
            None => format!(
                "AgentOpt mode switch failed (exit {}): {}",
                out.exit_code,
                out.stderr.trim()
            ),
        });
    }
    let post = inspect_workspace_read_only(&ws, true, &cli);
    let observed_mode = post.config.as_ref().and_then(|c| c.mode.as_deref());
    if post.state == AgentOptWorkspaceState::InvalidConfig || observed_mode != Some(mode_upper.as_str()) {
        let mut rb_err = None;
        if let Some(ref b) = backup_bytes {
            if let Err(re) = restore_agent_opt_config_bytes(&ws, b) {
                rb_err = Some(re);
            }
        }
        return Err(match rb_err {
            Some(re) => format!(
                "Verification failed after mode switch (expected {mode_upper}, observed {observed_mode:?}); rollback also failed: {re}"
            ),
            None => format!(
                "Verification failed after mode switch (expected {mode_upper}, observed {observed_mode:?}); rolled back."
            ),
        });
    }
    Ok(post)
}

pub fn update_agent_opt_config_at(
    settings_file: &Path,
    cli_dir: &Path,
    workspace_raw: &str,
    patch: &serde_json::Value,
    runner: &dyn AgentOptCliRunner,
) -> Result<AgentOptWorkspaceInspection, String> {
    update_agent_opt_config_with_rename(
        settings_file,
        cli_dir,
        workspace_raw,
        patch,
        runner,
        |src, dst| std::fs::rename(src, dst),
    )
}

fn update_agent_opt_config_with_rename<F>(
    settings_file: &Path,
    cli_dir: &Path,
    workspace_raw: &str,
    patch: &serde_json::Value,
    runner: &dyn AgentOptCliRunner,
    rename_fn: F,
) -> Result<AgentOptWorkspaceInspection, String>
where
    F: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    let _lock = ENGINE_TRANSITION_LOCK
        .lock()
        .map_err(|e| format!("Mutex poisoned: {e}"))?;
    let ws = canonicalize_workspace_path(workspace_raw)?;
    let settings = load_nerf_engine_settings(settings_file)?;
    if !is_workspace_registered(&settings, &ws) {
        return Err(
            "Cannot update AgentOpt config for an unregistered workspace. Explicitly register/adopt the workspace first."
                .to_string(),
        );
    }
    let cli = inspect_cli_info(cli_dir, runner);
    let pre = inspect_workspace_read_only(&ws, true, &cli);
    if matches!(
        pre.state,
        AgentOptWorkspaceState::NotInstalled
            | AgentOptWorkspaceState::NotInitialized
            | AgentOptWorkspaceState::InvalidConfig
            | AgentOptWorkspaceState::UnsupportedVersion
    ) {
        return Err(pre
            .detail
            .unwrap_or_else(|| format!("Cannot update config in state {:?}", pre.state)));
    }

    let patch_obj = patch
        .as_object()
        .ok_or_else(|| "Config update payload must be a JSON object".to_string())?;
    if patch_obj.contains_key("enabled") {
        return Err(
            "Top-level 'enabled' must be changed via the per-project AgentOpt ON/OFF toggle."
                .to_string(),
        );
    }

    let mut current_obj = pre
        .config
        .and_then(|c| c.raw)
        .and_then(|v| v.as_object().cloned())
        .ok_or_else(|| "Existing config is not a JSON object".to_string())?;

    for (k, v) in patch_obj {
        if let (Some(existing_sub), Some(patch_sub)) = (
            current_obj.get_mut(k).and_then(|x| x.as_object_mut()),
            v.as_object(),
        ) {
            for (sk, sv) in patch_sub {
                existing_sub.insert(sk.clone(), sv.clone());
            }
        } else {
            current_obj.insert(k.clone(), v.clone());
        }
    }

    let merged = serde_json::Value::Object(current_obj);
    validate_agent_opt_config_value(&merged)?;

    let backup_bytes = backup_agent_opt_config(&ws)?;
    let bytes = serde_json::to_vec_pretty(&merged)
        .map_err(|e| format!("Failed to serialize AgentOpt config: {e}"))?;
    restore_agent_opt_config_bytes_with_rename(&ws, &bytes, rename_fn)?;

    let post = inspect_workspace_read_only(&ws, true, &cli);
    let valid = post.state != AgentOptWorkspaceState::InvalidConfig
        && post
            .config
            .as_ref()
            .is_some_and(|c| c.raw_valid && c.schema_supported);
    if !valid {
        let mut rb_err = None;
        if let Some(ref b) = backup_bytes {
            if let Err(re) = restore_agent_opt_config_bytes(&ws, b) {
                rb_err = Some(re);
            }
        }
        return Err(match rb_err {
            Some(re) => format!(
                "Verification failed after config update; rollback also failed: {re}"
            ),
            None => "Verification failed after config update; rolled back.".to_string(),
        });
    }

    Ok(post)
}

pub fn switch_nerf_engine_at(
    settings_file: &Path,
    cli_dir: &Path,
    rules_dir: &Path,
    skill_file: &Path,
    target_engine: NerfEngine,
    selected_workspace_raw: Option<&str>,
    runner: &dyn AgentOptCliRunner,
) -> Result<EngineTransitionResult, String> {
    let _lock = ENGINE_TRANSITION_LOCK
        .lock()
        .map_err(|e| format!("Mutex poisoned: {e}"))?;
    switch_nerf_engine_locked(
        settings_file,
        cli_dir,
        rules_dir,
        skill_file,
        target_engine,
        selected_workspace_raw,
        runner,
    )
}

pub fn switch_nerf_engine_for_projects_at(
    settings_file: &Path,
    cli_dir: &Path,
    rules_dir: &Path,
    skill_file: &Path,
    target_engine: NerfEngine,
    project_slots: &[crate::plan_manager::ProjectPlanSlot],
    runner: &dyn AgentOptCliRunner,
) -> Result<EngineTransitionResult, String> {
    let _lock = ENGINE_TRANSITION_LOCK
        .lock()
        .map_err(|e| format!("Mutex poisoned: {e}"))?;
    switch_nerf_engine_locked_with_project_slots_and_hooks(
        settings_file,
        cli_dir,
        rules_dir,
        skill_file,
        target_engine,
        project_slots,
        runner,
        save_nerf_engine_settings,
        backup_agent_opt_config,
    )
}

fn switch_nerf_engine_locked(
    settings_file: &Path,
    cli_dir: &Path,
    rules_dir: &Path,
    skill_file: &Path,
    target_engine: NerfEngine,
    selected_workspace_raw: Option<&str>,
    runner: &dyn AgentOptCliRunner,
) -> Result<EngineTransitionResult, String> {
    switch_nerf_engine_locked_with_hooks(
        settings_file,
        cli_dir,
        rules_dir,
        skill_file,
        target_engine,
        selected_workspace_raw,
        runner,
        save_nerf_engine_settings,
        backup_agent_opt_config,
    )
}

fn switch_nerf_engine_locked_with_save<F>(
    settings_file: &Path,
    cli_dir: &Path,
    rules_dir: &Path,
    skill_file: &Path,
    target_engine: NerfEngine,
    selected_workspace_raw: Option<&str>,
    runner: &dyn AgentOptCliRunner,
    save_fn: F,
) -> Result<EngineTransitionResult, String>
where
    F: FnMut(&Path, &NerfEngineSettings) -> Result<(), String>,
{
    switch_nerf_engine_locked_with_hooks(
        settings_file,
        cli_dir,
        rules_dir,
        skill_file,
        target_engine,
        selected_workspace_raw,
        runner,
        save_fn,
        backup_agent_opt_config,
    )
}

fn rollback_agentopt_switch(
    ws_rollback: Option<(PathBuf, Vec<u8>)>,
    expected_ws_state: AgentOptWorkspaceState,
    rule_rollback_bytes: &Option<Vec<u8>>,
    expected_rule_state: PovDirectFileState,
    rules_dir: &Path,
    skill_file: &Path,
    cli: &AgentOptCliInfo,
) -> Vec<String> {
    let mut rb_errors = Vec::new();
    if let Some((rb_ws, rb_bytes)) = ws_rollback {
        if let Err(e) = restore_agent_opt_config_bytes(&rb_ws, &rb_bytes) {
            rb_errors.push(format!("workspace config rollback failed: {e}"));
        } else {
            let post_ws = inspect_workspace_read_only(&rb_ws, true, cli);
            if post_ws.state != expected_ws_state {
                rb_errors.push(format!(
                    "workspace rollback verification mismatch (expected {expected_ws_state:?}, observed {:?})",
                    post_ws.state
                ));
            }
        }
    }
    if let Some(ref bytes) = rule_rollback_bytes {
        if let Err(e) = pov_direct::restore_pov_direct_rule_bytes_at(rules_dir, bytes) {
            rb_errors.push(format!("pov-direct.md rollback failed: {e}"));
        }
    }
    match pov_direct::inspect_pov_direct_status_at(rules_dir, skill_file) {
        Ok(st) if st.state != expected_rule_state => {
            rb_errors.push(format!(
                "pov-direct.md rollback verification mismatch (expected {expected_rule_state:?}, observed {:?})",
                st.state
            ));
        }
        Err(ve) => rb_errors.push(format!("pov-direct.md rollback verification failed: {ve}")),
        _ => {}
    }
    rb_errors
}

fn rollback_agentopt_bulk_switch(
    changed_workspaces: Vec<(PathBuf, AgentOptWorkspaceState, Vec<u8>, Vec<u8>)>,
    rule_rollback_bytes: &Option<Vec<u8>>,
    expected_rule_state: PovDirectFileState,
    rules_dir: &Path,
    skill_file: &Path,
    cli: &AgentOptCliInfo,
) -> Vec<String> {
    let mut errors = Vec::new();
    for (workspace, expected_state, bytes, hooks_bytes) in changed_workspaces.into_iter().rev() {
        if let Err(error) = restore_agent_opt_config_bytes(&workspace, &bytes) {
            errors.push(format!("{}: workspace rollback failed: {error}", workspace.display()));
            continue;
        }
        if let Err(error) = restore_agent_opt_hooks(&workspace, &hooks_bytes) {
            errors.push(format!("{}: hooks rollback failed: {error}", workspace.display()));
            continue;
        }
        let observed = inspect_workspace_read_only(&workspace, true, cli).state;
        if observed != expected_state {
            errors.push(format!(
                "{}: workspace rollback verification mismatch (expected {expected_state:?}, observed {observed:?})",
                workspace.display()
            ));
        }
    }
    if let Some(bytes) = rule_rollback_bytes {
        if let Err(error) = pov_direct::restore_pov_direct_rule_bytes_at(rules_dir, bytes) {
            errors.push(format!("pov-direct.md rollback failed: {error}"));
        }
    }
    match pov_direct::inspect_pov_direct_status_at(rules_dir, skill_file) {
        Ok(status) if status.state != expected_rule_state => errors.push(format!(
            "pov-direct.md rollback verification mismatch (expected {expected_rule_state:?}, observed {:?})",
            status.state
        )),
        Err(error) => errors.push(format!("pov-direct.md rollback verification failed: {error}")),
        _ => {}
    }
    errors
}

fn rollback_agentopt_files(
    workspaces: Vec<(PathBuf, Option<Vec<u8>>, Vec<u8>)>,
) -> Vec<String> {
    let mut errors = Vec::new();
    for (workspace, config_bytes, hooks_bytes) in workspaces.into_iter().rev() {
        if let Some(bytes) = config_bytes {
            if let Err(error) = restore_agent_opt_config_bytes(&workspace, &bytes) {
                errors.push(format!("{}: config rollback failed: {error}", workspace.display()));
            } else if std::fs::read(workspace.join(".agent-opt").join("config.json")).ok().as_deref() != Some(bytes.as_slice()) {
                errors.push(format!("{}: config rollback verification mismatch", workspace.display()));
            }
        }
        if let Err(error) = restore_agent_opt_hooks(&workspace, &hooks_bytes) {
            errors.push(format!("{}: hooks rollback failed: {error}", workspace.display()));
        } else if std::fs::read(agent_opt_hooks_file(&workspace)).ok().as_deref() != Some(hooks_bytes.as_slice()) {
            errors.push(format!("{}: hooks rollback verification mismatch", workspace.display()));
        }
    }
    errors
}

fn switch_nerf_engine_locked_with_hooks<F, B>(
    settings_file: &Path,
    cli_dir: &Path,
    rules_dir: &Path,
    skill_file: &Path,
    target_engine: NerfEngine,
    selected_workspace_raw: Option<&str>,
    runner: &dyn AgentOptCliRunner,
    mut save_fn: F,
    mut backup_fn: B,
) -> Result<EngineTransitionResult, String>
where
    F: FnMut(&Path, &NerfEngineSettings) -> Result<(), String>,
    B: FnMut(&Path) -> Result<Option<Vec<u8>>, String>,
{
    let project_slots = single_workspace_project_slots(selected_workspace_raw);
    switch_nerf_engine_locked_with_project_slots_and_hooks(
        settings_file,
        cli_dir,
        rules_dir,
        skill_file,
        target_engine,
        &project_slots,
        runner,
        save_fn,
        backup_fn,
    )
}

fn switch_nerf_engine_locked_with_project_slots_and_hooks<F, B>(
    settings_file: &Path,
    cli_dir: &Path,
    rules_dir: &Path,
    skill_file: &Path,
    target_engine: NerfEngine,
    project_slots: &[crate::plan_manager::ProjectPlanSlot],
    runner: &dyn AgentOptCliRunner,
    mut save_fn: F,
    mut backup_fn: B,
) -> Result<EngineTransitionResult, String>
where
    F: FnMut(&Path, &NerfEngineSettings) -> Result<(), String>,
    B: FnMut(&Path) -> Result<Option<Vec<u8>>, String>,
{
    let mut settings = load_nerf_engine_settings(settings_file)?;
    let cli = inspect_cli_info(cli_dir, runner);

    let (workspace_mode, legacy_manual) = resolve_persisted_workspace_selection(&settings);
    let (project_targets, all_target_resolution_errors) =
        resolve_project_agent_opt_targets(project_slots, &settings);
    let target_resolution_errors: Vec<String> = all_target_resolution_errors.into_iter().filter(|error| {
        project_slots.iter().any(|slot| {
            error.starts_with(&format!("Project {} ", slot.slot_id))
                && settings.project_preferences.get(&slot.slot_id).is_some_and(|pref| {
                    pref.use_agent_opt && slot.repo_path.as_deref().is_some_and(|raw| {
                        match (canonicalize_workspace_path(&pref.repository_path), canonicalize_workspace_path(raw)) {
                            (Ok(saved), Ok(repo)) => normalize_workspace_key(&saved) == normalize_workspace_key(&repo),
                            _ => true, // An unavailable configured target must fail closed when ON.
                        }
                    })
                })
        })
    }).collect();
    let mut desired_targets: Vec<(String, PathBuf, Option<String>)> = Vec::new();
    for (slot_id, repo, workspace, plan_path, _, has_preference) in &project_targets {
        let registered = is_workspace_registered(&settings, workspace);
        let observed = inspect_workspace_read_only(workspace, registered, &cli);
        let requested = settings.project_preferences.get(slot_id)
            .filter(|pref| canonicalize_workspace_path(&pref.repository_path)
                .map(|saved| normalize_workspace_key(&saved) == normalize_workspace_key(repo))
                .unwrap_or(false))
            .map(|pref| pref.use_agent_opt)
            .unwrap_or(!*has_preference && settings.engine == NerfEngine::Agentopt
                    && agent_opt_workspace_config_enabled(&observed)
                    && observed.hooks_enabled == Some(true));
        if requested {
            desired_targets.push((slot_id.clone(), workspace.clone(), plan_path.clone()));
        }
    }
    let selected_ws = desired_targets.first().map(|(_, path, _)| path.clone());

    match target_engine {
        NerfEngine::Agentopt => {
            if !target_resolution_errors.is_empty() {
                return Err(format!(
                    "Cannot switch to AgentOpt: {}",
                    target_resolution_errors.join("; ")
                ));
            }
            if project_targets.is_empty() {
                return Err("Cannot switch to AgentOpt: no configured project workspaces are available.".to_string());
            }
            if workspace_mode == AgentOptWorkspaceMode::ManualOverride && legacy_manual.is_some() {
                return Err("Cannot switch to AgentOpt: assign the legacy manual workspace to a project card first.".to_string());
            }

            // Preflight the Rule, CLI, every desired target, and any enabled
            // registered workspace that must be turned off before mutating.
            let pre_rule = pov_direct::inspect_pov_direct_status_at(rules_dir, skill_file)?;
            if matches!(
                pre_rule.state,
                PovDirectFileState::UnmanagedCustom | PovDirectFileState::LegacyPoverello
            ) || pre_rule.settings_state == pov_direct::PovDirectSettingsState::MetadataInvalid
            {
                return Err(format!(
                    "Cannot switch to AgentOpt: Poverello Rule pov-direct.md is in protected/invalid state ({:?})",
                    pre_rule.state
                ));
            }
            let has_enabled_registered = settings.registered_workspaces.iter().any(|raw| {
                canonicalize_workspace_path(raw)
                    .ok()
                .is_some_and(|path| agent_opt_workspace_has_active_component(&inspect_workspace_read_only(&path, true, &cli)))
            });
            let cli_entry = if !desired_targets.is_empty() || has_enabled_registered {
                Some(require_supported_cli_entry(cli_dir, runner)?)
            } else {
                None
            };

            let mut transition_steps: Vec<(
                PathBuf,
                AgentOptWorkspaceState,
                AgentOptWorkspaceState,
                Vec<u8>,
                Vec<u8>,
            )> = Vec::new();
            let mut registered_paths = std::collections::HashSet::new();
            let mut transition_paths = std::collections::HashSet::new();
            let desired_target_keys: std::collections::HashSet<String> = desired_targets
                .iter()
                .map(|(_, path, _)| normalize_workspace_key(path))
                .collect();

            for reg in &settings.registered_workspaces {
                let workspace = canonicalize_workspace_path(reg).map_err(|error| {
                    format!(
                        "Cannot switch to AgentOpt: registered workspace '{reg}' is inaccessible ({error})"
                    )
                })?;
                let key = normalize_workspace_key(&workspace);
                if !registered_paths.insert(key.clone()) {
                    continue;
                }
                let inspection = inspect_workspace_read_only(&workspace, true, &cli);
                if matches!(
                    inspection.state,
                    AgentOptWorkspaceState::InvalidConfig | AgentOptWorkspaceState::UnsupportedVersion
                ) || (inspection.state == AgentOptWorkspaceState::NotInstalled
                    && (inspection.antigravity_plugin_present || inspection.config_present))
                    || (inspection.state == AgentOptWorkspaceState::Error
                        && (inspection.hooks_enabled.is_none()
                            || inspection.config.as_ref().is_none_or(|config| !config.raw_valid || !config.schema_supported)))
                {
                    return Err(format!(
                        "Cannot switch to AgentOpt: registered workspace '{}' cannot be verified (state: {:?}, plugin_present: {}, config_present: {})",
                        inspection.workspace_path,
                        inspection.state,
                        inspection.antigravity_plugin_present,
                        inspection.config_present
                    ));
                }
                let should_disable = agent_opt_workspace_has_active_component(&inspection)
                    && (target_engine == NerfEngine::PoverelloRules
                        || (target_engine == NerfEngine::Agentopt
                            && !desired_target_keys.contains(&key)));
                if should_disable && transition_paths.insert(key) {
                    let bytes = backup_fn(&workspace)?.ok_or_else(|| {
                        format!(
                            "Cannot switch engine: .agent-opt/config.json missing for backup in '{}'",
                            workspace.display()
                        )
                    })?;
                    let hooks_backup = backup_agent_opt_hooks(&workspace)?;
                    transition_steps.push((
                        workspace,
                        inspection.state,
                        AgentOptWorkspaceState::Disabled,
                        bytes,
                        hooks_backup,
                    ));
                }
            }

            for (project_slot_id, workspace, _) in &desired_targets {
                if workspace_mode == AgentOptWorkspaceMode::FollowRegisteredProjects
                    && !matches!(project_slot_id.as_str(), "primary" | "secondary")
                {
                    return Err(format!(
                        "Cannot switch to AgentOpt: selected plan repository '{}' is not present in the configured Project 1 / Project 2 slots.",
                        workspace.display()
                    ));
                }
                // Auto mode is sourced from global Project 1/2 registrations;
                // ensure each configured root is registered for AgentOpt before
                // mutating any target so the two registries cannot diverge.
                if !is_workspace_registered(&settings, workspace) {
                    return Err(format!(
                        "Cannot switch to AgentOpt: Project {} workspace '{}' is not registered. Register it explicitly first.",
                        project_slot_id, workspace.display()
                    ));
                }
                let inspection = inspect_workspace_read_only(workspace, true, &cli);
                if !matches!(
                    inspection.state,
                    AgentOptWorkspaceState::Disabled | AgentOptWorkspaceState::Enabled | AgentOptWorkspaceState::Error
                ) {
                    return Err(format!(
                        "Cannot switch to AgentOpt: Project {} workspace '{}' is in state {:?}. Initialize or fix configuration first.",
                        project_slot_id,
                        inspection.workspace_path,
                        inspection.state
                    ));
                }
                if inspection.state == AgentOptWorkspaceState::Error
                    && (inspection.hooks_enabled.is_none()
                        || inspection.config.as_ref().is_none_or(|config| !config.raw_valid || !config.schema_supported))
                {
                    return Err(format!(
                        "Cannot switch to AgentOpt: Project {} workspace '{}' has an invalid AgentOpt hook manifest or config.",
                        project_slot_id, inspection.workspace_path
                    ));
                }
                let key = normalize_workspace_key(workspace);
                if target_engine == NerfEngine::Agentopt
                    && (!agent_opt_workspace_config_enabled(&inspection) || inspection.hooks_enabled != Some(true))
                    && transition_paths.insert(key)
                {
                    let bytes = backup_fn(workspace)?.ok_or_else(|| {
                        format!(
                            "Cannot switch to AgentOpt: .agent-opt/config.json missing for backup in '{}'",
                            workspace.display()
                        )
                    })?;
                    let hooks_backup = backup_agent_opt_hooks(workspace)?;
                    transition_steps.push((
                        workspace.clone(),
                        inspection.state,
                        AgentOptWorkspaceState::Enabled,
                        bytes,
                        hooks_backup,
                    ));
                }
            }

            // Snapshot every affected workspace first. The precomputed steps
            // disable the old target before enabling the selected-plan target.
            let (captured_rule_settings, rule_rollback_bytes) =
                if pre_rule.state == PovDirectFileState::Absent {
                    (settings.saved_pov_direct_settings.clone().unwrap_or_default(), None)
                } else {
                    pov_direct::disable_pov_direct_rule_at(rules_dir, skill_file)?
                };
            if pre_rule.state != PovDirectFileState::Absent {
                settings.saved_pov_direct_settings = Some(captured_rule_settings);
            }

            let mut changed_workspaces = Vec::new();
            for (workspace, previous_state, desired_state, backup, hooks_backup) in transition_steps {
                // Record before invoking the CLI because a failed command may
                // still have partially changed the workspace config.
                changed_workspaces.push((workspace.clone(), previous_state, backup, hooks_backup.clone()));
                let workspace_arg = workspace.to_string_lossy().to_string();
                let action = if desired_state == AgentOptWorkspaceState::Enabled {
                    "enable"
                } else {
                    "disable"
                };
                let Some(cli_entry) = cli_entry.as_deref() else {
                    return Err("Cannot disable a previously enabled AgentOpt workspace because the AgentOpt CLI is unavailable.".to_string());
                };
                if desired_state == AgentOptWorkspaceState::Disabled {
                    if let Err(error) = set_agent_opt_hook_group_enabled(&workspace, false) {
                        let _ = rollback_agentopt_bulk_switch(
                            changed_workspaces, &rule_rollback_bytes, pre_rule.state, rules_dir, skill_file, &cli,
                        );
                        return Err(format!("Failed to disable AgentOpt hooks in '{}': {error}", workspace.display()));
                    }
                }
                let command_ok = matches!(
                    runner.run_aopt(cli_entry, &[action, "--cwd", &workspace_arg], 30),
                    Ok(output) if output.exit_code == 0
                );
                let hook_update = if command_ok && desired_state == AgentOptWorkspaceState::Enabled {
                    set_agent_opt_hook_group_enabled(&workspace, true)
                } else {
                    Ok(())
                };
                let observed = inspect_workspace_read_only(&workspace, true, &cli).state;
                if !command_ok || hook_update.is_err() || observed != desired_state {
                    let rollback_errors = rollback_agentopt_bulk_switch(
                        changed_workspaces,
                        &rule_rollback_bytes,
                        pre_rule.state,
                        rules_dir,
                        skill_file,
                        &cli,
                    );
                    let cause = format!(
                        "Failed to {action} AgentOpt in '{}' (observed {observed:?}{} )",
                        workspace.display(),
                        hook_update.err().map(|error| format!(", hook update failed: {error}")).unwrap_or_default()
                    );
                    if rollback_errors.is_empty() {
                        return Err(format!("{cause}; all workspace and Rule state was rolled back."));
                    }
                    return Err(format!(
                        "{cause}; rollback incomplete: {}",
                        rollback_errors.join("; ")
                    ));
                }
            }

            // Persist only after every desired workspace is verified enabled
            // and every registered non-target workspace is verified disabled.
            settings.engine = NerfEngine::Agentopt;
            settings.workspace_mode = Some(AgentOptWorkspaceMode::FollowRegisteredProjects);
            settings.selected_workspace = selected_ws.as_ref().map(|path| path.to_string_lossy().to_string());
            if let Err(e) = save_fn(settings_file, &settings) {
                let rb_errors = rollback_agentopt_bulk_switch(
                    changed_workspaces,
                    &rule_rollback_bytes,
                    pre_rule.state,
                    rules_dir,
                    skill_file,
                    &cli,
                );
                if rb_errors.is_empty() {
                    return Err(format!("Failed to persist engine preference ({e}); rolled back cleanly."));
                } else {
                    return Err(format!(
                        "Failed to persist engine preference ({e}); rollback incomplete: {}",
                        rb_errors.join("; ")
                    ));
                }
            }
        }
        NerfEngine::PoverelloRules => {
            // 1. Preflight Rule state
            let pre_rule = pov_direct::inspect_pov_direct_status_at(rules_dir, skill_file)?;
            if matches!(
                pre_rule.state,
                PovDirectFileState::UnmanagedCustom | PovDirectFileState::LegacyPoverello
            ) || pre_rule.settings_state == pov_direct::PovDirectSettingsState::MetadataInvalid
            {
                return Err(format!(
                    "Cannot restore Poverello Rules: pov-direct.md is in protected/invalid state ({:?})",
                    pre_rule.state
                ));
            }

            // 2. Inspect ALL registered workspaces first; refuse if any cannot be inspected or disabled
            let mut to_disable: Vec<(PathBuf, Option<Vec<u8>>, Vec<u8>)> = Vec::new();
            for reg in &settings.registered_workspaces {
                let ws = canonicalize_workspace_path(reg).map_err(|e| {
                    format!("Cannot switch to Poverello Rules: registered workspace '{reg}' is inaccessible ({e})")
                })?;
                let insp = inspect_workspace_read_only(&ws, true, &cli);
                if matches!(
                    insp.state,
                    AgentOptWorkspaceState::InvalidConfig | AgentOptWorkspaceState::UnsupportedVersion
                ) || (insp.state == AgentOptWorkspaceState::NotInstalled
                    && (insp.antigravity_plugin_present || insp.config_present))
                    || (insp.state == AgentOptWorkspaceState::Error
                        && (insp.hooks_enabled.is_none()
                            || insp.config.as_ref().is_none_or(|config| !config.raw_valid || !config.schema_supported)))
                {
                    return Err(format!(
                        "Cannot switch to Poverello Rules: workspace '{}' cannot be verified/disabled (state: {:?}, plugin_present: {}, config_present: {})",
                        insp.workspace_path, insp.state, insp.antigravity_plugin_present, insp.config_present
                    ));
                }
                if agent_opt_workspace_has_active_component(&insp) {
                    let backup = backup_agent_opt_config(&ws)?;
                    let hooks_backup = backup_agent_opt_hooks(&ws)?;
                    to_disable.push((ws, backup, hooks_backup));
                }
            }

            // Capture prior Rule bytes so we can roll back if settings save fails
            let pre_rule_file = rules_dir.join("pov-direct.md");
            let pre_rule_bytes = if pre_rule.state == PovDirectFileState::Absent {
                None
            } else {
                Some(
                    std::fs::read(&pre_rule_file)
                        .map_err(|e| format!("Failed to snapshot pov-direct.md before Rules switch: {e}"))?,
                )
            };

            let mut disabled_so_far: Vec<(PathBuf, Option<Vec<u8>>, Vec<u8>)> = Vec::new();
            if !to_disable.is_empty() {
                let cli_entry = require_supported_cli_entry(cli_dir, runner)?;
                for (ws, backup, hooks_backup) in to_disable {
                    let ws_str = ws.to_string_lossy().to_string();
                    if let Err(error) = set_agent_opt_hook_group_enabled(&ws, false) {
                        let mut attempts = disabled_so_far;
                        attempts.push((ws.clone(), backup, hooks_backup));
                        let rollback_errors = rollback_agentopt_files(attempts);
                        if rollback_errors.is_empty() {
                            return Err(format!("Failed to disable AgentOpt hooks in '{}': {error}; workspace files were restored.", ws.display()));
                        }
                        return Err(format!("Failed to disable AgentOpt hooks in '{}': {error}; rollback incomplete: {}", ws.display(), rollback_errors.join("; ")));
                    }
                    let out = runner.run_aopt(&cli_entry, &["disable", "--cwd", &ws_str], 30);
                    let ok = matches!(out, Ok(ref o) if o.exit_code == 0)
                        && inspect_workspace_read_only(&ws, true, &cli).state
                            == AgentOptWorkspaceState::Disabled;
                    if !ok {
                        let mut attempts = disabled_so_far;
                        attempts.push((ws.clone(), backup, hooks_backup));
                        let rb_errors = rollback_agentopt_files(attempts);
                        if rb_errors.is_empty() {
                            return Err(format!(
                                "Failed to disable AgentOpt in '{}'; rolled back previously disabled workspaces.",
                                ws.display()
                            ));
                        } else {
                            return Err(format!(
                                "Failed to disable AgentOpt in '{}' and rollback was incomplete: {}",
                                ws.display(),
                                rb_errors.join("; ")
                            ));
                        }
                    }
                    disabled_so_far.push((ws, backup, hooks_backup));
                }
            }

            // 3. Restore/enable Poverello Rule
            let target_rule_settings = settings
                .saved_pov_direct_settings
                .clone()
                .unwrap_or_default();
            if let Err(e) = pov_direct::enable_pov_direct_rule_at(rules_dir, skill_file, &target_rule_settings) {
                let rb_errors = rollback_agentopt_files(disabled_so_far);
                if rb_errors.is_empty() {
                    return Err(format!("Failed to restore Poverello Rule ({e}); rolled back AgentOpt workspaces."));
                } else {
                    return Err(format!(
                        "Failed to restore Poverello Rule ({e}); rollback incomplete: {}",
                        rb_errors.join("; ")
                    ));
                }
            }

            // 4. Persist preference; if save fails, roll back Rule and all disabled AgentOpt workspaces and verify
            settings.engine = NerfEngine::PoverelloRules;
            if let Some(ref ws) = selected_ws {
                settings.selected_workspace = Some(ws.to_string_lossy().to_string());
            }
            if let Err(e) = save_fn(settings_file, &settings) {
                let mut rb_errors = Vec::new();
                // Roll back Rule first (reverse order of step 3 -> step 2)
                if let Some(ref bytes) = pre_rule_bytes {
                    if let Err(re) = pov_direct::restore_pov_direct_rule_bytes_at(rules_dir, bytes) {
                        rb_errors.push(format!("pov-direct.md rollback failed: {re}"));
                    }
                } else if pre_rule_file.exists() {
                    if let Err(re) = std::fs::remove_file(&pre_rule_file) {
                        rb_errors.push(format!("pov-direct.md removal rollback failed: {re}"));
                    }
                }
                match pov_direct::inspect_pov_direct_status_at(rules_dir, skill_file) {
                    Ok(st) if st.state != pre_rule.state => {
                        rb_errors.push(format!(
                            "pov-direct.md rollback verification mismatch (expected {:?}, observed {:?})",
                            pre_rule.state, st.state
                        ));
                    }
                    Err(ve) => rb_errors.push(format!("pov-direct.md rollback verification failed: {ve}")),
                    _ => {}
                }

                // Roll back disabled AgentOpt workspaces in reverse order and verify
                rb_errors.extend(rollback_agentopt_files(disabled_so_far));

                if rb_errors.is_empty() {
                    return Err(format!(
                        "Failed to persist engine preference ({e}); rolled back Rule and AgentOpt workspaces cleanly."
                    ));
                } else {
                    return Err(format!(
                        "Failed to persist engine preference ({e}); rollback incomplete: {}",
                        rb_errors.join("; ")
                    ));
                }
            }
        }
    }

    let overview = inspect_nerf_engine_overview_for_projects_at(
        settings_file,
        cli_dir,
        rules_dir,
        skill_file,
        project_slots,
        runner,
    )?;
    let message = if overview.mismatch.has_mismatch {
        format!(
            "Engine set to {:?}, but verification requires follow-up: {}",
            target_engine,
            overview
                .mismatch
                .reason
                .as_deref()
                .unwrap_or("state mismatch")
        )
    } else {
        format!("Switched engine to {:?}", target_engine)
    };
    Ok(EngineTransitionResult {
        overview,
        changed: true,
        message,
    })
}

pub fn apply_selected_nerf_engine_at(
    settings_file: &Path,
    cli_dir: &Path,
    rules_dir: &Path,
    skill_file: &Path,
    selected_workspace_raw: Option<&str>,
    runner: &dyn AgentOptCliRunner,
) -> Result<EngineTransitionResult, String> {
    let _lock = ENGINE_TRANSITION_LOCK
        .lock()
        .map_err(|e| format!("Mutex poisoned: {e}"))?;
    let overview = inspect_nerf_engine_overview_at(
        settings_file,
        cli_dir,
        rules_dir,
        skill_file,
        selected_workspace_raw,
        runner,
    )?;
    if !overview.mismatch.has_mismatch {
        return Ok(EngineTransitionResult {
            overview,
            changed: false,
            message: "Observed state already matches selected engine; no changes made.".to_string(),
        });
    }
    switch_nerf_engine_locked(
        settings_file,
        cli_dir,
        rules_dir,
        skill_file,
        overview.persisted_engine,
        selected_workspace_raw,
        runner,
    )
}

pub fn apply_selected_nerf_engine_for_projects_at(
    settings_file: &Path,
    cli_dir: &Path,
    rules_dir: &Path,
    skill_file: &Path,
    project_slots: &[crate::plan_manager::ProjectPlanSlot],
    runner: &dyn AgentOptCliRunner,
) -> Result<EngineTransitionResult, String> {
    let _lock = ENGINE_TRANSITION_LOCK
        .lock()
        .map_err(|e| format!("Mutex poisoned: {e}"))?;
    let overview = inspect_nerf_engine_overview_for_projects_at(
        settings_file, cli_dir, rules_dir, skill_file, project_slots, runner,
    )?;
    if !overview.mismatch.has_mismatch {
        return Ok(EngineTransitionResult {
            overview,
            changed: false,
            message: "Observed state already matches selected engine; no changes made.".to_string(),
        });
    }
    switch_nerf_engine_locked_with_project_slots_and_hooks(
        settings_file,
        cli_dir,
        rules_dir,
        skill_file,
        overview.persisted_engine,
        project_slots,
        runner,
        save_nerf_engine_settings,
        backup_agent_opt_config,
    )
}

pub fn set_agent_opt_workspace_selection_at(
    settings_file: &Path,
    cli_dir: &Path,
    rules_dir: &Path,
    skill_file: &Path,
    mode: AgentOptWorkspaceMode,
    manual_workspace_raw: Option<&str>,
    project_slots: &[crate::plan_manager::ProjectPlanSlot],
    runner: &dyn AgentOptCliRunner,
) -> Result<NerfEngineOverview, String> {
    set_agent_opt_workspace_selection_with_save(
        settings_file,
        cli_dir,
        rules_dir,
        skill_file,
        mode,
        manual_workspace_raw,
        project_slots,
        runner,
        save_nerf_engine_settings,
    )
}

pub fn set_agent_opt_project_preference_at(
    settings_file: &Path,
    cli_dir: &Path,
    rules_dir: &Path,
    skill_file: &Path,
    slot_id: &str,
    use_agent_opt: Option<bool>,
    manual_workspace_raw: Option<Option<&str>>,
    project_slots: &[crate::plan_manager::ProjectPlanSlot],
    runner: &dyn AgentOptCliRunner,
) -> Result<NerfEngineOverview, String> {
    let _lock = ENGINE_TRANSITION_LOCK.lock().map_err(|e| format!("Mutex poisoned: {e}"))?;
    let mut settings = load_nerf_engine_settings(settings_file)?;
    let original = settings.clone();
    let slot = project_slots.iter().find(|slot| slot.slot_id == slot_id)
        .ok_or_else(|| format!("Unknown project slot '{slot_id}'"))?;
    let raw_repo = slot.repo_path.as_deref().ok_or_else(|| format!("Project {slot_id} has no repository root"))?;
    let repo = canonicalize_workspace_path(raw_repo)?;
    if use_agent_opt == Some(true) {
        let (legacy_mode, legacy_manual) = resolve_persisted_workspace_selection(&settings);
        if legacy_mode == AgentOptWorkspaceMode::ManualOverride && legacy_manual.is_some() {
            return Err("Assign or clear the legacy manual workspace on a project card before enabling AgentOpt.".to_string());
        }
    }
    let existing = settings.project_preferences.get(slot_id).filter(|pref| {
        canonicalize_workspace_path(&pref.repository_path)
            .map(|saved| normalize_workspace_key(&saved) == normalize_workspace_key(&repo))
            .unwrap_or(false)
    }).cloned();
    let mut preference = existing.unwrap_or(AgentOptProjectPreference {
        repository_path: repo.to_string_lossy().to_string(),
        use_agent_opt: false,
        manual_workspace: None,
    });
    preference.repository_path = repo.to_string_lossy().to_string();
    if let Some(value) = use_agent_opt {
        if value {
            let effective = manual_workspace_raw.flatten()
                .map(str::to_string)
                .or_else(|| preference.manual_workspace.clone())
                .unwrap_or_else(|| repo.to_string_lossy().to_string());
            let ws = canonicalize_workspace_path(&effective)?;
            if !is_workspace_registered(&settings, &ws) {
                return Err(format!("Project {slot_id} workspace '{}' is not registered with Poverello", ws.display()));
            }
            let cli = inspect_cli_info(cli_dir, runner);
            let inspection = inspect_workspace_read_only(&ws, true, &cli);
            if !matches!(inspection.state, AgentOptWorkspaceState::Disabled | AgentOptWorkspaceState::Enabled)
                && !(inspection.state == AgentOptWorkspaceState::Error
                    && inspection.hooks_enabled.is_some()
                    && inspection.config.as_ref().is_some_and(|config| config.raw_valid && config.schema_supported))
            {
                return Err(format!("Project {slot_id} workspace is not ready for AgentOpt (state: {:?})", inspection.state));
            }
        }
        preference.use_agent_opt = value;
    }
    if let Some(manual) = manual_workspace_raw {
        preference.manual_workspace = match manual {
            Some(raw) if !raw.trim().is_empty() => Some(canonicalize_workspace_path(raw)?.to_string_lossy().to_string()),
            _ => None,
        };
        // An explicit per-project path action resolves the ambiguous legacy
        // global override; it must no longer be carried independently.
        settings.workspace_mode = Some(AgentOptWorkspaceMode::FollowRegisteredProjects);
        settings.manual_workspace = None;
        settings.selected_workspace = None;
    }
    settings.project_preferences.insert(slot_id.to_string(), preference);
    if let Some(conflict) = resolve_project_agent_opt_targets(project_slots, &settings).1.into_iter()
        .find(|error| error.contains("conflicts with Project"))
    {
        return Err(conflict);
    }
    save_nerf_engine_settings(settings_file, &settings)?;
    if settings.engine == NerfEngine::Agentopt || use_agent_opt == Some(false) {
        let target_engine = settings.engine;
        match switch_nerf_engine_locked_with_project_slots_and_hooks(
            settings_file, cli_dir, rules_dir, skill_file, target_engine,
            project_slots, runner, save_nerf_engine_settings, backup_agent_opt_config,
        ) {
            Ok(result) => return Ok(result.overview),
            Err(error) => {
                save_nerf_engine_settings(settings_file, &original).map_err(|restore| {
                    format!("{error}; failed to restore previous project preference: {restore}")
                })?;
                return Err(error);
            }
        }
    }
    inspect_nerf_engine_overview_for_projects_at(settings_file, cli_dir, rules_dir, skill_file, project_slots, runner)
}

fn set_agent_opt_workspace_selection_with_save<F>(
    settings_file: &Path,
    cli_dir: &Path,
    rules_dir: &Path,
    skill_file: &Path,
    mode: AgentOptWorkspaceMode,
    manual_workspace_raw: Option<&str>,
    project_slots: &[crate::plan_manager::ProjectPlanSlot],
    runner: &dyn AgentOptCliRunner,
    mut save_fn: F,
) -> Result<NerfEngineOverview, String>
where
    F: FnMut(&Path, &NerfEngineSettings) -> Result<(), String>,
{
    let _lock = ENGINE_TRANSITION_LOCK
        .lock()
        .map_err(|e| format!("Mutex poisoned: {e}"))?;
    let mut settings = load_nerf_engine_settings(settings_file)?;
    let original_settings = settings.clone();
    match mode {
        AgentOptWorkspaceMode::ManualOverride => {
            let raw = manual_workspace_raw
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "Manual workspace path is required for manual_override mode".to_string())?;
            let canon = canonicalize_workspace_path(raw)?;
            let canon_str = canon.to_string_lossy().to_string();
            settings.workspace_mode = Some(AgentOptWorkspaceMode::ManualOverride);
            settings.manual_workspace = Some(canon_str.clone());
            settings.selected_workspace = Some(canon_str);
        }
        AgentOptWorkspaceMode::FollowRegisteredProjects => {
            let (targets, errors) = resolve_workspace_targets(project_slots, mode, None);
            if !errors.is_empty() {
                return Err(errors.join("; "));
            }
            settings.workspace_mode = Some(AgentOptWorkspaceMode::FollowRegisteredProjects);
            settings.manual_workspace = None;
            settings.selected_workspace = targets.first().map(|(_, path, _)| path.to_string_lossy().to_string());
        }
    }
    if original_settings.engine == NerfEngine::Agentopt {
        // Keep the persisted effective workspace pinned to the previous target
        // until the new target passes preflight and the transition succeeds.
        settings.selected_workspace = original_settings.selected_workspace.clone();
    }
    let pending_selection = settings.clone();
    save_fn(settings_file, &settings)?;
    if settings.engine == NerfEngine::Agentopt {
        match switch_nerf_engine_locked_with_project_slots_and_hooks(
            settings_file,
            cli_dir,
            rules_dir,
            skill_file,
            NerfEngine::Agentopt,
            project_slots,
            runner,
            |file, updated| save_fn(file, updated),
            backup_agent_opt_config,
        ) {
            Ok(result) => return Ok(result.overview),
            Err(transition_error) => {
                save_fn(settings_file, &pending_selection).map_err(|restore_error| {
                    format!(
                        "{transition_error}; failed to preserve the pending workspace selection: {restore_error}"
                    )
                })?;
                let mut overview = inspect_nerf_engine_overview_for_projects_at(
                    settings_file,
                    cli_dir,
                    rules_dir,
                    skill_file,
                    project_slots,
                    runner,
                )?;
                overview.mismatch.has_mismatch = true;
                overview.mismatch.reason = Some(transition_error.clone());
                overview.target_resolution_errors.push(transition_error);
                return Ok(overview);
            }
        }
    }
    inspect_nerf_engine_overview_for_projects_at(
        settings_file,
        cli_dir,
        rules_dir,
        skill_file,
        project_slots,
        runner,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    struct MockCliRunner {
        node_ver: Result<String, String>,
        npm_ver: Result<String, String>,
        installed_version: String,
        fail_commands: StdMutex<Vec<String>>,
        calls: StdMutex<Vec<Vec<String>>>,
    }

    impl MockCliRunner {
        fn new() -> Self {
            Self {
                node_ver: Ok("v22.11.0".to_string()),
                npm_ver: Ok("10.9.0".to_string()),
                installed_version: PINNED_AGENT_OPT_VERSION.to_string(),
                fail_commands: StdMutex::new(Vec::new()),
                calls: StdMutex::new(Vec::new()),
            }
        }
    }

    impl AgentOptCliRunner for MockCliRunner {
        fn node_version(&self) -> Result<String, String> {
            self.node_ver.clone()
        }
        fn npm_version(&self) -> Result<String, String> {
            self.npm_ver.clone()
        }
        fn install_pinned_cli(&self, cli_dir: &Path, _tarball_url: &str) -> Result<CliExecOutput, String> {
            let pkg_dir = cli_dir.join("node_modules").join("agent-opt");
            let cli_js_dir = pkg_dir.join("dist").join("cli");
            std::fs::create_dir_all(&cli_js_dir).unwrap();
            std::fs::write(
                pkg_dir.join("package.json"),
                format!(r#"{{"name":"agent-opt","version":"{}"}}"#, self.installed_version),
            ).unwrap();
            std::fs::write(cli_js_dir.join("index.js"), "// mock").unwrap();
            Ok(CliExecOutput { exit_code: 0, stdout: "ok".into(), stderr: "".into() })
        }
        fn run_aopt(&self, _cli_entry: &Path, args: &[&str], _timeout_secs: u64) -> Result<CliExecOutput, String> {
            let arg_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            self.calls.lock().unwrap().push(arg_vec.clone());
            let sub = args.first().copied().unwrap_or("");
            if self.fail_commands.lock().unwrap().iter().any(|f| f == sub) {
                return Ok(CliExecOutput { exit_code: 1, stdout: "".into(), stderr: format!("mock fail {sub}") });
            }
            let cwd_idx = args.iter().position(|a| *a == "--cwd").unwrap();
            let ws = PathBuf::from(args[cwd_idx + 1]);
            match sub {
                "init" => {
                    std::fs::create_dir_all(ws.join(".agent-opt")).unwrap();
                    std::fs::create_dir_all(ws.join(".agents").join("plugins").join("agent-opt")).unwrap();
                    std::fs::write(
                        ws.join(".agents").join("plugins").join("agent-opt").join("plugin.json"),
                        r#"{"name":"agent-opt"}"#,
                    ).unwrap();
                    std::fs::write(
                        ws.join(".agents").join("plugins").join("agent-opt").join("hooks.json"),
                        r#"{"agent-opt":{"enabled":true,"PreToolUse":[{"matcher":"*","hooks":[{"type":"command","command":"npx agent-opt hook"}]}]},"custom-group":{"enabled":true}}"#,
                    ).unwrap();
                    std::fs::write(
                        ws.join(".agent-opt").join("config.json"),
                        r#"{"enabled":true,"mode":"FAST","customKey":"keep"}"#,
                    ).unwrap();
                }
                "enable" | "disable" => {
                    let cfg_p = ws.join(".agent-opt").join("config.json");
                    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&cfg_p).unwrap()).unwrap();
                    v.as_object_mut().unwrap().insert("enabled".into(), serde_json::Value::Bool(sub == "enable"));
                    std::fs::write(&cfg_p, serde_json::to_string_pretty(&v).unwrap()).unwrap();
                }
                "mode" => {
                    let cfg_p = ws.join(".agent-opt").join("config.json");
                    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&cfg_p).unwrap()).unwrap();
                    v.as_object_mut().unwrap().insert("mode".into(), serde_json::Value::String(args[1].to_string()));
                    std::fs::write(&cfg_p, serde_json::to_string_pretty(&v).unwrap()).unwrap();
                }
                _ => {}
            }
            Ok(CliExecOutput { exit_code: 0, stdout: "ok".into(), stderr: "".into() })
        }
    }

    fn setup_env(temp: &Path) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        let settings_file = temp.join("antigravity-nerf-engine.json");
        let cli_dir = temp.join("agent-opt-cli");
        let rules_dir = temp.join("rules");
        let skill_dir = temp.join("skills").join("pov-direct");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_file = skill_dir.join("SKILL.md");
        std::fs::write(&skill_file, "# Pov Direct Skill\nBody").unwrap();
        (settings_file, cli_dir, rules_dir, skill_file)
    }

    fn mark_project_agent_opt_enabled(settings_file: &Path, slot_id: &str, repo: &Path) {
        let mut settings = load_nerf_engine_settings(settings_file).unwrap();
        settings.project_preferences.insert(slot_id.to_string(), AgentOptProjectPreference {
            repository_path: repo.to_string_lossy().to_string(),
            use_agent_opt: true,
            manual_workspace: None,
        });
        save_nerf_engine_settings(settings_file, &settings).unwrap();
    }

    #[test]
    fn test_read_only_load_and_workspace_navigation_do_not_mutate_files() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, rules_dir, skill_file) = setup_env(temp.path());
        let ws_a = temp.path().join("ws a with spaces");
        let ws_b = temp.path().join("ws_b");
        std::fs::create_dir_all(&ws_a).unwrap();
        std::fs::create_dir_all(&ws_b).unwrap();

        let runner = MockCliRunner::new();
        let ov1 = inspect_nerf_engine_overview_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            Some(&ws_a.to_string_lossy()), &runner,
        ).unwrap();
        assert_eq!(ov1.persisted_engine, NerfEngine::PoverelloRules);
        assert_eq!(ov1.workspace_inspection.unwrap().state, AgentOptWorkspaceState::NotInstalled);
        assert!(!settings_file.exists());
        assert!(!ws_a.join(".agent-opt").exists());

        // Install CLI, initialize ws_a, switch to AgentOpt, then navigate to ws_b (read-only)
        install_agent_opt_cli_at(&cli_dir, &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws_a.to_string_lossy(), None, &runner).unwrap();
        let ov2 = inspect_nerf_engine_overview_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            Some(&ws_b.to_string_lossy()), &runner,
        ).unwrap();
        assert_eq!(ov2.workspace_inspection.unwrap().state, AgentOptWorkspaceState::NotInitialized);
        assert!(!ws_b.join(".agent-opt").exists());
    }

    #[test]
    fn test_engine_switch_exclusivity_disables_all_registered_and_preserves_settings() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, rules_dir, skill_file) = setup_env(temp.path());
        let ws_a = temp.path().join("ws_a");
        let ws_b = temp.path().join("ws_b");
        std::fs::create_dir_all(&ws_a).unwrap();
        std::fs::create_dir_all(&ws_b).unwrap();

        // Apply custom PovDirect settings first
        let initial_pov = pov_direct::inspect_pov_direct_status_at(&rules_dir, &skill_file).unwrap();
        let custom_settings = PovDirectSettings {
            auto_activation: false,
            web_research: pov_direct::WebResearchPolicy::Unrestricted,
            response_verbosity: pov_direct::ResponseVerbosityPolicy::Standard,
            repository_exploration: pov_direct::ExplorationPolicy::Standard,
            verification: pov_direct::VerificationPolicy::Standard,
        };
        pov_direct::save_pov_direct_settings_at(
            &rules_dir, &skill_file,
            pov_direct::PovDirectApplyRequest {
                operation: pov_direct::PovDirectApplyOperation::Apply,
                settings: custom_settings.clone(),
                expected_state: initial_pov.state,
                expected_content_hash: initial_pov.content_hash,
            },
        ).unwrap();

        let runner = MockCliRunner::new();
        install_agent_opt_cli_at(&cli_dir, &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws_a.to_string_lossy(), Some("BALANCED"), &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws_b.to_string_lossy(), None, &runner).unwrap();

        // Switch to AgentOpt -> pov-direct.md disabled, settings preserved
        let slots = vec![
            crate::plan_manager::ProjectPlanSlot { slot_id: "primary".into(), repo_path: Some(ws_a.to_string_lossy().to_string()), plan_path: Some(".plan/a.md".into()) },
            crate::plan_manager::ProjectPlanSlot { slot_id: "secondary".into(), repo_path: Some(ws_b.to_string_lossy().to_string()), plan_path: Some(".plan/b.md".into()) },
        ];
        mark_project_agent_opt_enabled(&settings_file, "primary", &ws_a);
        mark_project_agent_opt_enabled(&settings_file, "secondary", &ws_b);
        let sw_ao = switch_nerf_engine_for_projects_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            NerfEngine::Agentopt, &slots, &runner,
        ).unwrap();
        assert_eq!(sw_ao.overview.pov_direct_status.state, PovDirectFileState::Absent);
        assert!(!sw_ao.overview.mismatch.has_mismatch);

        assert_eq!(sw_ao.overview.workspace_targets.len(), 2);
        assert_eq!(sw_ao.overview.workspace_targets.iter().filter(|target| target.inspection.state == AgentOptWorkspaceState::Enabled).count(), 2);
        for workspace in [&ws_a, &ws_b] {
            let hooks: serde_json::Value = serde_json::from_slice(&std::fs::read(agent_opt_hooks_file(workspace)).unwrap()).unwrap();
            assert_eq!(hooks["agent-opt"]["enabled"], true);
            assert_eq!(hooks["custom-group"]["enabled"], true);
            assert!(hooks["agent-opt"]["PreToolUse"].is_array());
        }

        // Switch back to PoverelloRules -> disables BOTH ws_a and ws_b, restores custom_settings!
        let sw_rules = switch_nerf_engine_for_projects_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            NerfEngine::PoverelloRules, &slots, &runner,
        ).unwrap();
        assert_eq!(sw_rules.overview.pov_direct_status.state, PovDirectFileState::ManagedSynced);
        assert_eq!(sw_rules.overview.pov_direct_status.settings, custom_settings);
        let cli = inspect_cli_info(&cli_dir, &runner);
        assert_eq!(inspect_workspace_read_only(&ws_a.canonicalize().unwrap(), true, &cli).state, AgentOptWorkspaceState::Disabled);
        assert_eq!(inspect_workspace_read_only(&ws_b.canonicalize().unwrap(), true, &cli).state, AgentOptWorkspaceState::Disabled);
        for workspace in [&ws_a, &ws_b] {
            let hooks: serde_json::Value = serde_json::from_slice(&std::fs::read(agent_opt_hooks_file(workspace)).unwrap()).unwrap();
            assert_eq!(hooks["agent-opt"]["enabled"], false);
            assert_eq!(hooks["custom-group"]["enabled"], true);
            assert!(hooks["agent-opt"]["PreToolUse"].is_array());
        }
        // Unknown key preserved in AgentOpt config
        let cfg_raw = std::fs::read_to_string(ws_a.join(".agent-opt").join("config.json")).unwrap();
        assert!(cfg_raw.contains("customKey"));
    }

    #[test]
    fn test_agentopt_per_project_toggles_are_independent() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, rules_dir, skill_file) = setup_env(temp.path());
        let ws_a = temp.path().join("project_a");
        let ws_b = temp.path().join("project_b");
        std::fs::create_dir_all(&ws_a).unwrap();
        std::fs::create_dir_all(&ws_b).unwrap();
        let runner = MockCliRunner::new();
        install_agent_opt_cli_at(&cli_dir, &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws_a.to_string_lossy(), None, &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws_b.to_string_lossy(), None, &runner).unwrap();

        let slots = [
            crate::plan_manager::ProjectPlanSlot {
                slot_id: "primary".into(),
                repo_path: Some(ws_a.to_string_lossy().to_string()),
                plan_path: Some(".plan/a.md".into()),
            },
            crate::plan_manager::ProjectPlanSlot {
                slot_id: "secondary".into(),
                repo_path: Some(ws_b.to_string_lossy().to_string()),
                plan_path: Some(".plan/b.md".into()),
            },
        ];
        let mut prefs = load_nerf_engine_settings(&settings_file).unwrap();
        for slot in &slots {
            prefs.project_preferences.insert(slot.slot_id.clone(), AgentOptProjectPreference {
                repository_path: slot.repo_path.clone().unwrap(),
                use_agent_opt: true,
                manual_workspace: None,
            });
        }
        save_nerf_engine_settings(&settings_file, &prefs).unwrap();
        let switched = switch_nerf_engine_for_projects_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            NerfEngine::Agentopt, &slots, &runner,
        ).unwrap();
        assert_eq!(switched.overview.workspace_targets.len(), 2);
        assert!(switched.overview.workspace_targets.iter().all(|target| target.inspection.state == AgentOptWorkspaceState::Enabled));
        let cli = inspect_cli_info(&cli_dir, &runner);
        assert_eq!(inspect_workspace_read_only(&ws_a.canonicalize().unwrap(), true, &cli).state, AgentOptWorkspaceState::Enabled);
        assert_eq!(inspect_workspace_read_only(&ws_b.canonicalize().unwrap(), true, &cli).state, AgentOptWorkspaceState::Enabled);

        // Turning Project 1 off leaves Project 2 enabled and does not activate the global Rule.
        set_agent_opt_project_preference_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            "primary", Some(false), None, &slots, &runner,
        ).unwrap();
        let after_off = inspect_nerf_engine_overview_for_projects_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file, &slots, &runner,
        ).unwrap();
        assert!(!after_off.workspace_targets[0].use_agent_opt);
        assert!(after_off.workspace_targets[1].use_agent_opt);
        assert_eq!(after_off.workspace_targets[0].inspection.state, AgentOptWorkspaceState::Disabled);
        assert_eq!(after_off.workspace_targets[1].inspection.state, AgentOptWorkspaceState::Enabled);
        assert_eq!(after_off.workspace_targets[0].inspection.hooks_enabled, Some(false));
        assert_eq!(after_off.workspace_targets[1].inspection.hooks_enabled, Some(true));
        assert_eq!(after_off.pov_direct_status.state, PovDirectFileState::Absent);

        // Re-applying respects the saved per-project choices.
        let reapplied = apply_selected_nerf_engine_for_projects_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file, &slots, &runner,
        ).unwrap();
        assert_eq!(reapplied.overview.workspace_targets.len(), 2);
        assert_eq!(reapplied.overview.workspace_targets[0].inspection.state, AgentOptWorkspaceState::Disabled);
        assert_eq!(reapplied.overview.workspace_targets[1].inspection.state, AgentOptWorkspaceState::Enabled);
        assert_eq!(inspect_workspace_read_only(&ws_a.canonicalize().unwrap(), true, &cli).state, AgentOptWorkspaceState::Disabled);
        assert_eq!(inspect_workspace_read_only(&ws_b.canonicalize().unwrap(), true, &cli).state, AgentOptWorkspaceState::Enabled);
        assert_eq!(read_agent_opt_hook_group_enabled(&agent_opt_hooks_file(&ws_a)).unwrap(), false);
        assert_eq!(read_agent_opt_hook_group_enabled(&agent_opt_hooks_file(&ws_b)).unwrap(), true);
    }

    #[test]
    fn test_rules_switch_disables_residual_hook_when_config_is_already_off() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, rules_dir, skill_file) = setup_env(temp.path());
        let ws = temp.path().join("residual-hook-project");
        std::fs::create_dir_all(&ws).unwrap();
        let runner = MockCliRunner::new();
        install_agent_opt_cli_at(&cli_dir, &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws.to_string_lossy(), None, &runner).unwrap();
        let mut config: serde_json::Value = serde_json::from_slice(&std::fs::read(ws.join(".agent-opt/config.json")).unwrap()).unwrap();
        config["enabled"] = serde_json::Value::Bool(false);
        std::fs::write(ws.join(".agent-opt/config.json"), serde_json::to_vec_pretty(&config).unwrap()).unwrap();
        assert_eq!(inspect_workspace_read_only(&ws, true, &inspect_cli_info(&cli_dir, &runner)).state, AgentOptWorkspaceState::Error);
        assert_eq!(read_agent_opt_hook_group_enabled(&agent_opt_hooks_file(&ws)).unwrap(), true);

        let slots = [crate::plan_manager::ProjectPlanSlot {
            slot_id: "primary".into(),
            repo_path: Some(ws.to_string_lossy().to_string()),
            plan_path: Some(".plan/a.md".into()),
        }];
        let result = switch_nerf_engine_for_projects_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            NerfEngine::PoverelloRules, &slots, &runner,
        ).unwrap();
        assert!(!result.overview.mismatch.has_mismatch);
        assert_eq!(read_agent_opt_hook_group_enabled(&agent_opt_hooks_file(&ws)).unwrap(), false);
        assert_eq!(inspect_workspace_read_only(&ws, true, &inspect_cli_info(&cli_dir, &runner)).state, AgentOptWorkspaceState::Disabled);
    }

    #[test]
    fn test_project_manual_workspace_paths_are_slot_scoped_and_identity_bound() {
        let temp = tempfile::tempdir().unwrap();
        let repo_a = temp.path().join("repo_a");
        let repo_b = temp.path().join("repo_b");
        let manual_a = temp.path().join("manual_a");
        let manual_b = temp.path().join("manual_b");
        for path in [&repo_a, &repo_b, &manual_a, &manual_b] {
            std::fs::create_dir_all(path).unwrap();
        }
        let slots = [
            crate::plan_manager::ProjectPlanSlot { slot_id: "primary".into(), repo_path: Some(repo_a.to_string_lossy().to_string()), plan_path: Some(".plan/a.md".into()) },
            crate::plan_manager::ProjectPlanSlot { slot_id: "secondary".into(), repo_path: Some(repo_b.to_string_lossy().to_string()), plan_path: Some(".plan/b.md".into()) },
        ];
        let mut settings = NerfEngineSettings::default();
        settings.project_preferences.insert("primary".into(), AgentOptProjectPreference {
            repository_path: repo_a.to_string_lossy().to_string(), use_agent_opt: false,
            manual_workspace: Some(manual_a.to_string_lossy().to_string()),
        });
        settings.project_preferences.insert("secondary".into(), AgentOptProjectPreference {
            repository_path: repo_b.to_string_lossy().to_string(), use_agent_opt: true,
            manual_workspace: Some(manual_b.to_string_lossy().to_string()),
        });
        let (targets, errors) = resolve_project_agent_opt_targets(&slots, &settings);
        assert!(errors.is_empty());
        assert_eq!(targets[0].2, manual_a.canonicalize().unwrap());
        assert_eq!(targets[1].2, manual_b.canonicalize().unwrap());
        settings.project_preferences.get_mut("primary").unwrap().manual_workspace =
            Some(manual_b.to_string_lossy().to_string());
        let (_, conflicts) = resolve_project_agent_opt_targets(&slots, &settings);
        assert!(conflicts.iter().any(|error| error.contains("conflicts with Project")));

        // Replacing Project 1's repository does not carry its previous manual path.
        let replacement = temp.path().join("replacement");
        std::fs::create_dir_all(&replacement).unwrap();
        let replacement_slots = [crate::plan_manager::ProjectPlanSlot {
            slot_id: "primary".into(), repo_path: Some(replacement.to_string_lossy().to_string()), plan_path: None,
        }];
        let (replaced, errors) = resolve_project_agent_opt_targets(&replacement_slots, &settings);
        assert!(errors.is_empty());
        assert_eq!(replaced[0].2, replacement.canonicalize().unwrap());
        assert!(replaced[0].4.is_none());
        assert!(replaced[0].5); // A stale slot record exists, so legacy enabled-state inference is disabled.
    }

    #[test]
    fn test_legacy_manual_workspace_requires_explicit_project_assignment() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, rules_dir, skill_file) = setup_env(temp.path());
        let repo = temp.path().join("repo");
        let legacy_manual = temp.path().join("legacy-manual");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&legacy_manual).unwrap();
        let slots = [crate::plan_manager::ProjectPlanSlot {
            slot_id: "primary".into(), repo_path: Some(repo.to_string_lossy().to_string()), plan_path: None,
        }];
        std::fs::write(&settings_file, serde_json::to_vec_pretty(&serde_json::json!({
            "engine": "poverello_rules",
            "workspace_mode": "manual_override",
            "manual_workspace": legacy_manual.to_string_lossy(),
            "selected_workspace": legacy_manual.to_string_lossy(),
            "customUnknown": "preserve-me"
        })).unwrap()).unwrap();
        let runner = MockCliRunner::new();
        let overview = inspect_nerf_engine_overview_for_projects_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file, &slots, &runner,
        ).unwrap();
        assert!(overview.target_resolution_errors.iter().any(|error| error.contains("legacy manual workspace")));
        let err = set_agent_opt_project_preference_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file, "primary", Some(true), None, &slots, &runner,
        ).unwrap_err();
        assert!(err.contains("Assign or clear the legacy manual workspace"));

        set_agent_opt_project_preference_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file, "primary", None,
            Some(Some(repo.to_string_lossy().as_ref())), &slots, &runner,
        ).unwrap();
        let migrated = load_nerf_engine_settings(&settings_file).unwrap();
        assert_eq!(migrated.workspace_mode, Some(AgentOptWorkspaceMode::FollowRegisteredProjects));
        assert_eq!(migrated.manual_workspace, None);
        assert_eq!(
            migrated.project_preferences["primary"].manual_workspace.as_deref().map(normalize_workspace_path_for_display),
            Some(normalize_workspace_path_for_display(&repo.to_string_lossy())),
        );
        assert_eq!(migrated.extra.get("customUnknown").and_then(|value| value.as_str()), Some("preserve-me"));
    }

    #[test]
    fn test_repository_replacement_does_not_inherit_project_preference() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, rules_dir, skill_file) = setup_env(temp.path());
        let ws_a = temp.path().join("project_a");
        let ws_b = temp.path().join("project_b");
        std::fs::create_dir_all(&ws_a).unwrap();
        std::fs::create_dir_all(&ws_b).unwrap();
        let runner = MockCliRunner::new();
        install_agent_opt_cli_at(&cli_dir, &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws_a.to_string_lossy(), None, &runner).unwrap();
        let slot_a = [crate::plan_manager::ProjectPlanSlot {
            slot_id: "primary".into(),
            repo_path: Some(ws_a.to_string_lossy().to_string()),
            plan_path: Some(".plan/a.md".into()),
        }];
        let slot_b = [crate::plan_manager::ProjectPlanSlot {
            slot_id: "secondary".into(),
            repo_path: Some(ws_b.to_string_lossy().to_string()),
            plan_path: Some(".plan/b.md".into()),
        }];
        let mut settings = load_nerf_engine_settings(&settings_file).unwrap();
        settings.project_preferences.insert("primary".into(), AgentOptProjectPreference {
            repository_path: ws_a.to_string_lossy().to_string(),
            use_agent_opt: true,
            manual_workspace: None,
        });
        save_nerf_engine_settings(&settings_file, &settings).unwrap();
        switch_nerf_engine_for_projects_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            NerfEngine::Agentopt, &slot_a, &runner,
        ).unwrap();
        let after = switch_nerf_engine_for_projects_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            NerfEngine::Agentopt, &slot_b, &runner,
        ).unwrap();
        assert_eq!(after.overview.workspace_targets[0].repository_path, normalize_workspace_path_for_display(&ws_b.canonicalize().unwrap().to_string_lossy()));
        assert!(!after.overview.workspace_targets[0].use_agent_opt);
        assert_eq!(after.overview.workspace_targets[0].inspection.state, AgentOptWorkspaceState::NotInitialized);
        let cli = inspect_cli_info(&cli_dir, &runner);
        assert_eq!(inspect_workspace_read_only(&ws_a.canonicalize().unwrap(), true, &cli).state, AgentOptWorkspaceState::Disabled);
    }

    #[test]
    fn test_switch_rollback_on_failure_and_unmanaged_rule_protection() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, rules_dir, skill_file) = setup_env(temp.path());
        let ws = temp.path().join("ws");
        std::fs::create_dir_all(&ws).unwrap();

        // 1. Unmanaged custom rule blocks switching
        std::fs::write(rules_dir.join("pov-direct.md"), "---\ntrigger: manual\n---\nUser rule\n").unwrap();
        let runner = MockCliRunner::new();
        let err = switch_nerf_engine_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            NerfEngine::Agentopt, Some(&ws.to_string_lossy()), &runner,
        ).unwrap_err();
        assert!(err.contains("UnmanagedCustom"));
        assert!(rules_dir.join("pov-direct.md").exists());

        // 2. Managed rule + failing AgentOpt enable rolls back pov-direct.md cleanly
        std::fs::remove_file(rules_dir.join("pov-direct.md")).unwrap();
        pov_direct::enable_pov_direct_rule_at(&rules_dir, &skill_file, &PovDirectSettings::default()).unwrap();
        install_agent_opt_cli_at(&cli_dir, &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws.to_string_lossy(), None, &runner).unwrap();
        mark_project_agent_opt_enabled(&settings_file, "primary", &ws);
        // Disable ws first so switching to AgentOpt will attempt `enable`
        let cli_entry = cli_dir.join("node_modules").join("agent-opt").join("dist").join("cli").join("index.js");
        runner.run_aopt(&cli_entry, &["disable", "--cwd", &ws.canonicalize().unwrap().to_string_lossy()], 30).unwrap();

        runner.fail_commands.lock().unwrap().push("enable".to_string());
        let fail_sw = switch_nerf_engine_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            NerfEngine::Agentopt, Some(&ws.to_string_lossy()), &runner,
        ).unwrap_err();
        assert!(fail_sw.contains("rolled back"));
        assert_eq!(
            pov_direct::inspect_pov_direct_status_at(&rules_dir, &skill_file).unwrap().state,
            PovDirectFileState::ManagedSynced
        );
    }

    #[test]
    fn test_invalid_config_read_only_unknown_keys_and_mismatch_apply() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, rules_dir, skill_file) = setup_env(temp.path());
        let ws = temp.path().join("ws");
        std::fs::create_dir_all(&ws).unwrap();

        let runner = MockCliRunner::new();
        install_agent_opt_cli_at(&cli_dir, &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws.to_string_lossy(), None, &runner).unwrap();

        // Update config preserves unknown keys
        let updated = update_agent_opt_config_at(
            &settings_file,
            &cli_dir,
            &ws.to_string_lossy(),
            &serde_json::json!({"mode": "STRICT", "context": {"maxRankedFiles": 12}}),
            &runner,
        ).unwrap();
        let raw = updated.config.unwrap().raw.unwrap();
        assert_eq!(raw.get("customKey").and_then(|v| v.as_str()), Some("keep"));
        assert_eq!(raw.get("mode").and_then(|v| v.as_str()), Some("STRICT"));

        // Malformed config becomes InvalidConfig and refuses updates
        std::fs::write(ws.join(".agent-opt").join("config.json"), "{broken json").unwrap();
        let cli = inspect_cli_info(&cli_dir, &runner);
        assert_eq!(inspect_workspace_read_only(&ws.canonicalize().unwrap(), true, &cli).state, AgentOptWorkspaceState::InvalidConfig);
        assert!(update_agent_opt_config_at(&settings_file, &cli_dir, &ws.to_string_lossy(), &serde_json::json!({"mode":"FAST"}), &runner).is_err());

        // Known field with wrong type (e.g. context.maxRankedFiles = "bad") becomes InvalidConfig / read-only
        std::fs::write(ws.join(".agent-opt").join("config.json"), r#"{"enabled":true,"mode":"BALANCED","context":{"maxRankedFiles":"bad"}}"#).unwrap();
        assert_eq!(inspect_workspace_read_only(&ws.canonicalize().unwrap(), true, &cli).state, AgentOptWorkspaceState::InvalidConfig);
        assert!(update_agent_opt_config_at(&settings_file, &cli_dir, &ws.to_string_lossy(), &serde_json::json!({"mode":"FAST"}), &runner).is_err());

        // Fix config, create mismatch (persisted Rules, but Rule file absent) -> Apply reconciles; second Apply is no-op
        std::fs::write(ws.join(".agent-opt").join("config.json"), r#"{"enabled":false,"mode":"FAST"}"#).unwrap();
        let ov_mismatch = inspect_nerf_engine_overview_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            Some(&ws.to_string_lossy()), &runner,
        ).unwrap();
        assert!(ov_mismatch.mismatch.has_mismatch);
        let applied = apply_selected_nerf_engine_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            Some(&ws.to_string_lossy()), &runner,
        ).unwrap();
        assert!(applied.changed);
        assert!(!applied.overview.mismatch.has_mismatch);

        let noop = apply_selected_nerf_engine_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            Some(&ws.to_string_lossy()), &runner,
        ).unwrap();
        assert!(!noop.changed);
    }

    #[test]
    fn test_rules_switch_refuses_when_cli_absent_with_remaining_files_and_unprepared_agentopt_refuses() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, rules_dir, skill_file) = setup_env(temp.path());
        let ws = temp.path().join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        pov_direct::enable_pov_direct_rule_at(&rules_dir, &skill_file, &PovDirectSettings::default()).unwrap();

        let runner = MockCliRunner::new();
        mark_project_agent_opt_enabled(&settings_file, "primary", &ws);

        // 1. Unprepared AgentOpt (CLI not installed / ws not initialized) refuses without touching pov-direct.md or settings
        let err_unprepared = switch_nerf_engine_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            NerfEngine::Agentopt, Some(&ws.to_string_lossy()), &runner,
        ).unwrap_err();
        assert!(!err_unprepared.is_empty());
        assert_eq!(
            pov_direct::inspect_pov_direct_status_at(&rules_dir, &skill_file).unwrap().state,
            PovDirectFileState::ManagedSynced
        );
        assert_eq!(load_nerf_engine_settings(&settings_file).unwrap().engine, NerfEngine::PoverelloRules);

        // 2. Install, init, switch to AgentOpt, then delete CLI while workspace files remain -> switch to Rules MUST refuse
        install_agent_opt_cli_at(&cli_dir, &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws.to_string_lossy(), None, &runner).unwrap();
        mark_project_agent_opt_enabled(&settings_file, "primary", &ws);
        switch_nerf_engine_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            NerfEngine::Agentopt, Some(&ws.to_string_lossy()), &runner,
        ).unwrap();
        std::fs::remove_dir_all(&cli_dir).unwrap();

        let err_unknown = switch_nerf_engine_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            NerfEngine::PoverelloRules, Some(&ws.to_string_lossy()), &runner,
        ).unwrap_err();
        assert!(err_unknown.contains("cannot be verified/disabled"));
        assert_eq!(
            pov_direct::inspect_pov_direct_status_at(&rules_dir, &skill_file).unwrap().state,
            PovDirectFileState::Absent
        );
    }

    #[test]
    fn test_unregistered_mode_config_refused_and_settings_atomic_replace_preserves_original() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, _rules_dir, _skill_file) = setup_env(temp.path());
        let ws = temp.path().join("ws_unreg");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::create_dir_all(ws.join(".agents").join("plugins").join("agent-opt")).unwrap();
        std::fs::write(ws.join(".agents").join("plugins").join("agent-opt").join("plugin.json"), "{}").unwrap();
        std::fs::write(ws.join(".agents").join("plugins").join("agent-opt").join("hooks.json"), r#"{"agent-opt":{"enabled":true}}"#).unwrap();
        std::fs::create_dir_all(ws.join(".agent-opt")).unwrap();
        let orig_cfg = r#"{"enabled":false,"mode":"BALANCED"}"#;
        std::fs::write(ws.join(".agent-opt").join("config.json"), orig_cfg).unwrap();

        let runner = MockCliRunner::new();
        install_agent_opt_cli_at(&cli_dir, &runner).unwrap();

        // Mode and config updates on unregistered workspace must fail and leave file untouched
        assert!(set_agent_opt_mode_at(&settings_file, &cli_dir, &ws.to_string_lossy(), "STRICT", &runner).is_err());
        assert!(update_agent_opt_config_at(
            &settings_file, &cli_dir, &ws.to_string_lossy(),
            &serde_json::json!({"mode": "FAST"}), &runner
        ).is_err());
        assert_eq!(std::fs::read_to_string(ws.join(".agent-opt").join("config.json")).unwrap(), orig_cfg);

        // Atomic settings replacement preserves original file on rename failure
        let initial_settings = NerfEngineSettings {
            engine: NerfEngine::Agentopt,
            selected_workspace: Some(ws.to_string_lossy().to_string()),
            registered_workspaces: vec![ws.to_string_lossy().to_string()],
            ..Default::default()
        };
        save_nerf_engine_settings(&settings_file, &initial_settings).unwrap();
        let mut updated_settings = initial_settings.clone();
        updated_settings.engine = NerfEngine::PoverelloRules;
        let err_replace = save_nerf_engine_settings_with_rename(&settings_file, &updated_settings, |_src, _dst| {
            Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "injected rename failure"))
        }).unwrap_err();
        assert!(err_replace.contains("injected rename failure"));
        assert_eq!(load_nerf_engine_settings(&settings_file).unwrap().engine, NerfEngine::Agentopt);
    }

    #[test]
    fn test_init_cleans_up_when_registration_fails_and_large_cli_output_drains_without_deadlock() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, _rules_dir, _skill_file) = setup_env(temp.path());
        let ws = temp.path().join("ws_init_fail");
        std::fs::create_dir_all(&ws).unwrap();

        let runner = MockCliRunner::new();
        install_agent_opt_cli_at(&cli_dir, &runner).unwrap();
        let err = init_agent_opt_workspace_with_reg(
            &settings_file,
            &cli_dir,
            &ws.to_string_lossy(),
            Some("BALANCED"),
            &runner,
            |_sf, _ws| Err("injected registration save failure".to_string()),
        ).unwrap_err();
        assert!(err.contains("rolled back initialized AgentOpt files"));
        assert!(!ws.join(".agents").join("plugins").join("agent-opt").exists());
        assert!(!ws.join(".agent-opt").exists());

        // Large CLI stdout (>128KB, exceeds Windows/POSIX pipe buffer) completes without hanging and caps output
        #[cfg(windows)]
        let out = run_simple_command(
            "powershell",
            &["-NoProfile", "-Command", "Write-Output ('x' * 200000)"],
            10,
        ).unwrap();
        #[cfg(not(windows))]
        let out = run_simple_command(
            "sh",
            &["-c", "head -c 200000 /dev/zero | tr '\\0' 'x'"],
            10,
        ).unwrap();
        assert_eq!(out.exit_code, 0);
        assert!(out.stdout.len() >= 100_000);
        assert!(out.stdout.len() <= MAX_CLI_OUTPUT_BYTES);
    }

    #[test]
    fn test_install_cli_serializes_under_engine_transition_lock() {
        struct LockCheckingRunner {
            inner: MockCliRunner,
        }
        impl AgentOptCliRunner for LockCheckingRunner {
            fn node_version(&self) -> Result<String, String> {
                self.inner.node_version()
            }
            fn npm_version(&self) -> Result<String, String> {
                self.inner.npm_version()
            }
            fn install_pinned_cli(&self, cli_dir: &Path, tarball_url: &str) -> Result<CliExecOutput, String> {
                // While install_agent_opt_cli_at is executing, ENGINE_TRANSITION_LOCK MUST be held.
                assert!(ENGINE_TRANSITION_LOCK.try_lock().is_err());
                self.inner.install_pinned_cli(cli_dir, tarball_url)
            }
            fn run_aopt(&self, cli_entry: &Path, args: &[&str], timeout_secs: u64) -> Result<CliExecOutput, String> {
                self.inner.run_aopt(cli_entry, args, timeout_secs)
            }
        }

        let temp = tempfile::tempdir().unwrap();
        let (_settings_file, cli_dir, _rules_dir, _skill_file) = setup_env(temp.path());
        let runner = LockCheckingRunner { inner: MockCliRunner::new() };
        let info = install_agent_opt_cli_at(&cli_dir, &runner).unwrap();
        assert!(info.installed);
    }

    #[test]
    fn test_rules_switch_rolls_back_rule_and_workspaces_when_settings_save_fails() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, rules_dir, skill_file) = setup_env(temp.path());
        let ws_a = temp.path().join("ws_a");
        let ws_b = temp.path().join("ws_b");
        std::fs::create_dir_all(&ws_a).unwrap();
        std::fs::create_dir_all(&ws_b).unwrap();

        let runner = MockCliRunner::new();
        install_agent_opt_cli_at(&cli_dir, &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws_a.to_string_lossy(), None, &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws_b.to_string_lossy(), None, &runner).unwrap();
        mark_project_agent_opt_enabled(&settings_file, "primary", &ws_a);
        switch_nerf_engine_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            NerfEngine::Agentopt, Some(&ws_a.to_string_lossy()), &runner,
        ).unwrap();

        // Inject settings-save failure when switching back to PoverelloRules
        let err = switch_nerf_engine_locked_with_save(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            NerfEngine::PoverelloRules, Some(&ws_a.to_string_lossy()), &runner,
            |_sf, _settings| Err("injected engine settings save failure".to_string()),
        ).unwrap_err();
        assert!(err.contains("rolled back Rule and AgentOpt workspaces cleanly"));

        // Verify persisted engine, Rule (Absent), and the single active workspace
        // are all back to their pre-switch state.
        assert_eq!(load_nerf_engine_settings(&settings_file).unwrap().engine, NerfEngine::Agentopt);
        assert_eq!(
            pov_direct::inspect_pov_direct_status_at(&rules_dir, &skill_file).unwrap().state,
            PovDirectFileState::Absent
        );
        let cli = inspect_cli_info(&cli_dir, &runner);
        assert_eq!(inspect_workspace_read_only(&ws_a.canonicalize().unwrap(), true, &cli).state, AgentOptWorkspaceState::Enabled);
        assert_eq!(inspect_workspace_read_only(&ws_b.canonicalize().unwrap(), true, &cli).state, AgentOptWorkspaceState::Disabled);
    }

    #[test]
    fn test_update_agent_opt_config_preserves_original_on_windows_rename_failure() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, _rules_dir, _skill_file) = setup_env(temp.path());
        let ws = temp.path().join("ws_cfg_fail");
        std::fs::create_dir_all(&ws).unwrap();

        let runner = MockCliRunner::new();
        install_agent_opt_cli_at(&cli_dir, &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws.to_string_lossy(), Some("BALANCED"), &runner).unwrap();
        mark_project_agent_opt_enabled(&settings_file, "primary", &ws);
        let cfg_path = ws.join(".agent-opt").join("config.json");
        let original_bytes = std::fs::read(&cfg_path).unwrap();

        let err = update_agent_opt_config_with_rename(
            &settings_file,
            &cli_dir,
            &ws.to_string_lossy(),
            &serde_json::json!({"mode": "STRICT"}),
            &runner,
            |_src, _dst| Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "injected config rename failure")),
        ).unwrap_err();
        assert!(err.contains("injected config rename failure"));
        assert_eq!(std::fs::read(&cfg_path).unwrap(), original_bytes);

        // Also exercise the platform-independent swap-and-restore sequence directly (covers non-Windows hosts too)
        let tmp_direct = ws.join(".agent-opt").join(".config.json.tmp-direct");
        let swap_direct = ws.join(".agent-opt").join(".config.json.swap-direct");
        std::fs::write(&tmp_direct, b"{}").unwrap();
        let swap_err = replace_via_swap_with_rename(&tmp_direct, &cfg_path, &swap_direct, |_src, _dst| {
            Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "injected swap rename failure"))
        }).unwrap_err();
        assert!(swap_err.contains("original preserved"));
        assert_eq!(std::fs::read(&cfg_path).unwrap(), original_bytes);
        assert!(!tmp_direct.exists());
        assert!(!swap_direct.exists());
    }

    #[test]
    fn test_init_refuses_when_agent_opt_dir_has_unrelated_files_and_preserves_them() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, _rules_dir, _skill_file) = setup_env(temp.path());
        let ws = temp.path().join("ws_existing_other");
        let custom_dir = ws.join(".agents").join("plugins").join("agent-opt");
        std::fs::create_dir_all(&custom_dir).unwrap();
        let custom_file = custom_dir.join("custom.txt");
        std::fs::write(&custom_file, "user custom note").unwrap();

        let runner = MockCliRunner::new();
        install_agent_opt_cli_at(&cli_dir, &runner).unwrap();
        let cli = inspect_cli_info(&cli_dir, &runner);
        let insp = inspect_workspace_read_only(&ws.canonicalize().unwrap(), false, &cli);
        assert!(insp.unregistered_files_detected);

        let err = init_agent_opt_workspace_with_reg(
            &settings_file,
            &cli_dir,
            &ws.to_string_lossy(),
            Some("BALANCED"),
            &runner,
            |_sf, _ws| Err("injected reg failure".to_string()),
        ).unwrap_err();
        assert!(err.contains("already have existing content"));
        assert_eq!(std::fs::read_to_string(&custom_file).unwrap(), "user custom note");
    }

    #[test]
    fn test_agentopt_switch_aborts_on_backup_failure_and_verifies_rollback_on_save_failure() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, rules_dir, skill_file) = setup_env(temp.path());
        let ws = temp.path().join("ws_backup_fail");
        std::fs::create_dir_all(&ws).unwrap();
        pov_direct::enable_pov_direct_rule_at(&rules_dir, &skill_file, &PovDirectSettings::default()).unwrap();

        let runner = MockCliRunner::new();
        install_agent_opt_cli_at(&cli_dir, &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws.to_string_lossy(), Some("BALANCED"), &runner).unwrap();
        mark_project_agent_opt_enabled(&settings_file, "primary", &ws);
        let cli_entry = cli_dir.join("node_modules").join("agent-opt").join("dist").join("cli").join("index.js");
        runner.run_aopt(&cli_entry, &["disable", "--cwd", &ws.canonicalize().unwrap().to_string_lossy()], 30).unwrap();

        let orig_cfg = std::fs::read(ws.join(".agent-opt").join("config.json")).unwrap();
        let orig_rule = std::fs::read(rules_dir.join("pov-direct.md")).unwrap();

        // 1. Injected backup failure aborts before touching Rule, AgentOpt config, or persisted engine
        let err_backup = switch_nerf_engine_locked_with_hooks(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            NerfEngine::Agentopt, Some(&ws.to_string_lossy()), &runner,
            save_nerf_engine_settings,
            |_ws| Err("injected backup failure".to_string()),
        ).unwrap_err();
        assert!(err_backup.contains("injected backup failure"));
        assert_eq!(std::fs::read(ws.join(".agent-opt").join("config.json")).unwrap(), orig_cfg);
        assert_eq!(std::fs::read(rules_dir.join("pov-direct.md")).unwrap(), orig_rule);
        assert_eq!(load_nerf_engine_settings(&settings_file).unwrap().engine, NerfEngine::PoverelloRules);

        // 2. Inject save_fn failure AND corrupt backup bytes so rollback verification detects state mismatch
        let err_mismatch = switch_nerf_engine_locked_with_hooks(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            NerfEngine::Agentopt, Some(&ws.to_string_lossy()), &runner,
            |_sf, _settings| Err("injected settings save failure".to_string()),
            |_ws| Ok(Some(br#"{"enabled":true,"mode":"BALANCED"}"#.to_vec())),
        ).unwrap_err();
        assert!(err_mismatch.contains("rollback verification mismatch"));
    }

    #[test]
    fn test_workspace_follow_plan_manual_override_legacy_migration_and_save_failure() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, rules_dir, skill_file) = setup_env(temp.path());
        let ws_plan1 = temp.path().join("ws_plan1");
        let ws_plan2 = temp.path().join("ws_plan2");
        let ws_manual = temp.path().join("ws_manual");
        std::fs::create_dir_all(&ws_plan1).unwrap();
        std::fs::create_dir_all(&ws_plan2).unwrap();
        std::fs::create_dir_all(&ws_manual).unwrap();
        let runner = MockCliRunner::new();

        let slots = vec![
            crate::plan_manager::ProjectPlanSlot { slot_id: "primary".into(), repo_path: Some(ws_plan1.to_string_lossy().to_string()), plan_path: Some(".plan/one.md".into()) },
            crate::plan_manager::ProjectPlanSlot { slot_id: "secondary".into(), repo_path: Some(ws_plan2.to_string_lossy().to_string()), plan_path: Some(".plan/two.md".into()) },
        ];
        // Follow mode resolves both registered project roots, independent of an active plan.
        let ov_fresh = inspect_nerf_engine_overview_for_projects_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file, &slots, &runner,
        ).unwrap();
        assert_eq!(ov_fresh.workspace_mode, AgentOptWorkspaceMode::FollowRegisteredProjects);
        assert_eq!(ov_fresh.workspace_targets.len(), 2);

        // 2. Legacy settings with selected_workspace and no workspace_mode -> migrates as ManualOverride, preserving unknown keys & registered_workspaces
        let legacy_json = serde_json::json!({
            "engine": "poverello_rules",
            "selected_workspace": ws_manual.to_string_lossy().to_string(),
            "registered_workspaces": [ws_plan1.to_string_lossy().to_string()],
            "customUnknown": "preserve-me"
        });
        std::fs::create_dir_all(settings_file.parent().unwrap()).unwrap();
        std::fs::write(&settings_file, serde_json::to_vec_pretty(&legacy_json).unwrap()).unwrap();

        let ov_legacy = inspect_nerf_engine_overview_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            Some(&ws_plan1.to_string_lossy()), &runner,
        ).unwrap();
        assert_eq!(ov_legacy.workspace_mode, AgentOptWorkspaceMode::ManualOverride);
        assert_eq!(ov_legacy.selected_workspace.as_deref(), Some(normalize_workspace_path_for_display(&ws_manual.canonicalize().unwrap().to_string_lossy()).as_str()));
        assert_eq!(
            ov_legacy.active_plan_workspace.as_deref(),
            Some(normalize_workspace_path_for_display(&ws_plan1.canonicalize().unwrap().to_string_lossy()).as_str())
        );

        // Plan change to ws_plan2 keeps manual override in effect
        let ov_legacy_p2 = inspect_nerf_engine_overview_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            Some(&ws_plan2.to_string_lossy()), &runner,
        ).unwrap();
        assert_eq!(ov_legacy_p2.selected_workspace.as_deref(), Some(normalize_workspace_path_for_display(&ws_manual.canonicalize().unwrap().to_string_lossy()).as_str()));
        assert_eq!(
            ov_legacy_p2.active_plan_workspace.as_deref(),
            Some(normalize_workspace_path_for_display(&ws_plan2.canonicalize().unwrap().to_string_lossy()).as_str())
        );

        // 3. Failed save_fn leaves previously persisted selection intact
        let err_save = set_agent_opt_workspace_selection_with_save(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            AgentOptWorkspaceMode::FollowRegisteredProjects, None, &slots, &runner,
            |_sf, _s| Err("injected selection save failure".to_string()),
        ).unwrap_err();
        assert!(err_save.contains("injected selection save failure"));
        assert_eq!(
            resolve_persisted_workspace_selection(&load_nerf_engine_settings(&settings_file).unwrap()).0,
            AgentOptWorkspaceMode::ManualOverride
        );

        // 4. Explicit return to follow-registered-projects clears override and preserves metadata.
        let ov_follow = set_agent_opt_workspace_selection_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            AgentOptWorkspaceMode::FollowRegisteredProjects, None, &slots, &runner,
        ).unwrap();
        assert_eq!(ov_follow.workspace_mode, AgentOptWorkspaceMode::FollowRegisteredProjects);
        assert_eq!(ov_follow.manual_workspace, None);
        assert_eq!(ov_follow.workspace_targets.len(), 2);
        assert!(runner.calls.lock().unwrap().is_empty());

        // Stale selected_workspace never overrides the registered project list.
        let mut loaded = load_nerf_engine_settings(&settings_file).unwrap();
        assert_eq!(loaded.extra.get("customUnknown").and_then(|v| v.as_str()), Some("preserve-me"));
        assert_eq!(loaded.registered_workspaces.len(), 1);
        loaded.selected_workspace = Some(ws_manual.to_string_lossy().to_string());
        save_nerf_engine_settings(&settings_file, &loaded).unwrap();

        let ov_unresolved = inspect_nerf_engine_overview_for_projects_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file,
            &slots, &runner,
        ).unwrap();
        assert_eq!(ov_unresolved.workspace_mode, AgentOptWorkspaceMode::FollowRegisteredProjects);
        assert_eq!(ov_unresolved.workspace_targets.len(), 2);
    }

    #[test]
    fn test_bulk_agentopt_preflights_all_projects_and_never_partially_enables() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, rules_dir, skill_file) = setup_env(temp.path());
        let ws_a = temp.path().join("project_a");
        let ws_b = temp.path().join("project_b");
        std::fs::create_dir_all(&ws_a).unwrap();
        std::fs::create_dir_all(&ws_b).unwrap();
        let runner = MockCliRunner::new();
        install_agent_opt_cli_at(&cli_dir, &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws_a.to_string_lossy(), None, &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws_b.to_string_lossy(), None, &runner).unwrap();
        let slots = vec![
            crate::plan_manager::ProjectPlanSlot { slot_id: "primary".into(), repo_path: Some(ws_a.to_string_lossy().to_string()), plan_path: Some(".plan/a.md".into()) },
            crate::plan_manager::ProjectPlanSlot { slot_id: "secondary".into(), repo_path: Some(ws_b.to_string_lossy().to_string()), plan_path: Some(".plan/b.md".into()) },
        ];
        mark_project_agent_opt_enabled(&settings_file, "primary", &ws_a);
        mark_project_agent_opt_enabled(&settings_file, "secondary", &ws_b);
        let mut persisted = load_nerf_engine_settings(&settings_file).unwrap();
        persisted.registered_workspaces.retain(|path| !path.to_lowercase().contains("project_b"));
        save_nerf_engine_settings(&settings_file, &persisted).unwrap();
        // B is ready on disk but absent from AgentOpt's registry; the transition must abort before A changes.
        let err = switch_nerf_engine_for_projects_at(
            &settings_file, &cli_dir, &rules_dir, &skill_file, NerfEngine::Agentopt, &slots, &runner,
        ).unwrap_err();
        assert!(err.contains("is not registered"));
        let cli = inspect_cli_info(&cli_dir, &runner);
        assert_eq!(inspect_workspace_read_only(&ws_a.canonicalize().unwrap(), true, &cli).state, AgentOptWorkspaceState::Enabled);
        assert_eq!(pov_direct::inspect_pov_direct_status_at(&rules_dir, &skill_file).unwrap().state, PovDirectFileState::Absent);
        assert_eq!(load_nerf_engine_settings(&settings_file).unwrap().engine, NerfEngine::PoverelloRules);
    }

    #[test]
    fn test_normalize_extended_windows_paths_for_display_only() {
        assert_eq!(normalize_workspace_path_for_display(r"\\?\C:\Users\Sohei\dev\repo"), r"C:\Users\Sohei\dev\repo");
        assert_eq!(normalize_workspace_path_for_display(r"\\?\UNC\server\share\repo"), r"\\server\share\repo");
        assert_eq!(normalize_workspace_path_for_display(r"C:\Users\Sohei\dev\repo"), r"C:\Users\Sohei\dev\repo");
    }

    #[test]
    fn test_pinned_v010_config_schema_validation_and_patch_isolation() {
        let temp = tempfile::tempdir().unwrap();
        let (settings_file, cli_dir, _rules_dir, _skill_file) = setup_env(temp.path());
        let ws_a = temp.path().join("ws_a");
        let ws_b = temp.path().join("ws_b");
        std::fs::create_dir_all(&ws_a).unwrap();
        std::fs::create_dir_all(&ws_b).unwrap();

        let runner = MockCliRunner::new();
        install_agent_opt_cli_at(&cli_dir, &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws_a.to_string_lossy(), Some("FAST"), &runner).unwrap();
        init_agent_opt_workspace_at(&settings_file, &cli_dir, &ws_b.to_string_lossy(), Some("BALANCED"), &runner).unwrap();

        let ws_b_before = std::fs::read(ws_b.join(".agent-opt").join("config.json")).unwrap();

        // 1. Valid pinned v0.1.0 patch across all 4 sections + mode, preserving unknown keys
        let updated = update_agent_opt_config_at(
            &settings_file,
            &cli_dir,
            &ws_a.to_string_lossy(),
            &serde_json::json!({
                "mode": "BALANCED",
                "context": {
                    "enabled": true,
                    "maxRankedFiles": 12,
                    "maxRepoMapTokens": 800,
                    "deduplicateReads": false,
                    "blockRedundantFullReadsAfter": 3,
                    "minQuerySpecificityForInjection": false
                },
                "verification": {
                    "enabled": true,
                    "enforceOnStop": true,
                    "autoRunFastTestsOnStop": true,
                    "maxStopBlocksPerSession": 2,
                    "requireTestsWhenCodeModified": false,
                    "exemptDocOnlyChanges": false
                },
                "failureDetection": {
                    "enabled": true,
                    "maxRepeatedCommandFailures": 3,
                    "maxIdenticalErrors": 4,
                    "maxRedundantReadsOfUnchangedFile": 5,
                    "detectEditOscillation": false
                },
                "memory": {
                    "enabled": true,
                    "maxEntries": 100,
                    "minConfidenceToInject": 0.85,
                    "autoPruneContradictions": false
                }
            }),
            &runner,
        ).unwrap();
        let raw = updated.config.unwrap().raw.unwrap();
        assert_eq!(raw.get("customKey").and_then(|v| v.as_str()), Some("keep"));
        assert_eq!(raw["context"]["maxRepoMapTokens"], 800);
        assert_eq!(raw["verification"]["maxStopBlocksPerSession"], 2);
        assert_eq!(raw["failureDetection"]["maxRepeatedCommandFailures"], 3);
        assert_eq!(raw["memory"]["minConfidenceToInject"], 0.85);
        // Project B untouched
        assert_eq!(std::fs::read(ws_b.join(".agent-opt").join("config.json")).unwrap(), ws_b_before);

        // 2. Rejects top-level `enabled` inside config editor patch
        let err_enabled = update_agent_opt_config_at(
            &settings_file, &cli_dir, &ws_a.to_string_lossy(),
            &serde_json::json!({ "enabled": false }), &runner,
        ).unwrap_err();
        assert!(err_enabled.contains("enabled"));

        // 3. Rejects out-of-range memory.minConfidenceToInject and context.maxRepoMapTokens = 0
        assert!(validate_agent_opt_config_value(&serde_json::json!({
            "memory": { "minConfidenceToInject": 1.5 }
        })).is_err());
        assert!(validate_agent_opt_config_value(&serde_json::json!({
            "context": { "maxRepoMapTokens": 0 }
        })).is_err());
        // Allows verification.maxStopBlocksPerSession = 0 (FAST default)
        assert!(validate_agent_opt_config_value(&serde_json::json!({
            "verification": { "maxStopBlocksPerSession": 0 }
        })).is_ok());
    }
}

#[cfg(test)]
mod global_hook_diagnostic_tests {
    use super::*;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use tempfile::TempDir;

    struct StaticPluginListRunner {
        response: Result<CliExecOutput, String>,
        calls: AtomicUsize,
    }

    impl StaticPluginListRunner {
        fn with_stdout(stdout: &str) -> Self {
            Self {
                response: Ok(CliExecOutput {
                    exit_code: 0,
                    stdout: stdout.to_string(),
                    stderr: String::new(),
                }),
                calls: AtomicUsize::new(0),
            }
        }
    }

    impl AntigravityPluginListRunner for StaticPluginListRunner {
        fn list_plugins(&self, _agy_executable: &Path) -> Result<CliExecOutput, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.response.clone()
        }
    }

    fn risky_hooks_json() -> String {
        let command = r#"powershell.exe -NoLogo -NoProfile -NonInteractive -Command [Console]::Out.WriteLine('{"decision":"ask"}')"#;
        json!({
            "wt-agent-hooks": {
                "PreToolUse": [{
                    "matcher": "*",
                    "hooks": [{
                        "type": "command",
                        "command": command
                    }]
                }]
            }
        })
        .to_string()
    }

    fn setup_home(temp: &TempDir, hooks_json: &str, version: &str) -> PathBuf {
        let home = temp.path().to_path_buf();
        let cli_dir = home.join(".gemini").join("bin");
        let plugin_dir = home
            .join(".gemini")
            .join("config")
            .join("plugins")
            .join("wt-agent-hooks");
        std::fs::create_dir_all(&cli_dir).unwrap();
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(cli_dir.join("agy.exe"), b"mock executable").unwrap();
        std::fs::write(
            plugin_dir.join("intelligent-terminal.json"),
            json!({
                "name": "wt-agent-hooks",
                "managed_by": "Intelligent Terminal: wt-agent-hooks",
                "version": version
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(plugin_dir.join("hooks.json"), hooks_json).unwrap();
        home
    }

    fn plugin_list(path: &Path, enabled: bool) -> String {
        json!([{
            "name": "wt-agent-hooks",
            "enabled": enabled,
            "version": null,
            "source": "local-install",
            "path": path.to_string_lossy()
        }])
        .to_string()
    }

    fn inspect(home: &Path, runner: &StaticPluginListRunner) -> AntigravityGlobalHookDiagnostic {
        inspect_global_antigravity_hook_diagnostic_with_platform(Some(home), runner, true)
    }

    #[test]
    fn detects_known_quote_sensitive_pre_tool_use_pattern_without_running_it() {
        let temp = TempDir::new().unwrap();
        let home = setup_home(&temp, &risky_hooks_json(), "0.1.2");
        let plugin_dir = home.join(".gemini/config/plugins/wt-agent-hooks");
        let runner = StaticPluginListRunner::with_stdout(&plugin_list(&plugin_dir, true));

        let result = inspect(&home, &runner);

        assert_eq!(
            result.state,
            AntigravityGlobalHookDiagnosticState::RiskPatternDetected
        );
        assert_eq!(result.plugin_version.as_deref(), Some("0.1.2"));
        assert_eq!(result.event.as_deref(), Some("PreToolUse"));
        assert_eq!(runner.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn distinguishes_disabled_and_absent_global_plugin_states() {
        let temp = TempDir::new().unwrap();
        let home = setup_home(&temp, &risky_hooks_json(), "0.1.2");
        let plugin_dir = home.join(".gemini/config/plugins/wt-agent-hooks");
        let disabled_runner = StaticPluginListRunner::with_stdout(&plugin_list(&plugin_dir, false));
        assert_eq!(
            inspect(&home, &disabled_runner).state,
            AntigravityGlobalHookDiagnosticState::Disabled
        );

        let absent_runner = StaticPluginListRunner::with_stdout("[]");
        assert_eq!(
            inspect(&home, &absent_runner).state,
            AntigravityGlobalHookDiagnosticState::PluginAbsent
        );
    }

    #[test]
    fn rejects_malformed_plugin_list_and_hook_manifest() {
        let temp = TempDir::new().unwrap();
        let home = setup_home(&temp, "{", "0.1.2");
        let plugin_dir = home.join(".gemini/config/plugins/wt-agent-hooks");

        let malformed_list = StaticPluginListRunner::with_stdout("not json");
        assert_eq!(
            inspect(&home, &malformed_list).state,
            AntigravityGlobalHookDiagnosticState::InvalidPluginList
        );

        let valid_list = StaticPluginListRunner::with_stdout(&plugin_list(&plugin_dir, true));
        assert_eq!(
            inspect(&home, &valid_list).state,
            AntigravityGlobalHookDiagnosticState::InvalidManifest
        );
    }

    #[test]
    fn unknown_managed_version_is_not_reported_as_confirmed_risk() {
        let temp = TempDir::new().unwrap();
        let home = setup_home(&temp, &risky_hooks_json(), "0.1.3");
        let plugin_dir = home.join(".gemini/config/plugins/wt-agent-hooks");
        let runner = StaticPluginListRunner::with_stdout(&plugin_list(&plugin_dir, true));

        let result = inspect(&home, &runner);

        assert_eq!(
            result.state,
            AntigravityGlobalHookDiagnosticState::Unverified
        );
        assert_eq!(result.plugin_version.as_deref(), Some("0.1.3"));
        assert_eq!(result.event, None);
    }

    #[test]
    fn rejects_a_plugin_list_path_outside_the_expected_global_plugin_directory() {
        let temp = TempDir::new().unwrap();
        let home = setup_home(&temp, &risky_hooks_json(), "0.1.2");
        let unexpected = temp.path().join("outside-plugin");
        let runner = StaticPluginListRunner::with_stdout(&plugin_list(&unexpected, true));

        let result = inspect(&home, &runner);

        assert_eq!(
            result.state,
            AntigravityGlobalHookDiagnosticState::UnsafePath
        );
        assert_eq!(result.event, None);
    }

    #[test]
    fn does_not_call_cli_when_platform_or_executable_is_unavailable() {
        let temp = TempDir::new().unwrap();
        let runner = StaticPluginListRunner::with_stdout("[]");
        let unsupported = inspect_global_antigravity_hook_diagnostic_with_platform(
            Some(temp.path()),
            &runner,
            false,
        );
        assert_eq!(
            unsupported.state,
            AntigravityGlobalHookDiagnosticState::UnsupportedPlatform
        );
        assert_eq!(runner.calls.load(Ordering::SeqCst), 0);

        let unavailable = inspect_global_antigravity_hook_diagnostic_with_platform(
            Some(temp.path()),
            &runner,
            true,
        );
        assert_eq!(
            unavailable.state,
            AntigravityGlobalHookDiagnosticState::CliUnavailable
        );
        assert_eq!(runner.calls.load(Ordering::SeqCst), 0);
    }

    struct RemediationRunner {
        lists: Mutex<VecDeque<String>>,
        disable_result: Result<CliExecOutput, String>,
        list_calls: AtomicUsize,
        disable_calls: AtomicUsize,
        executable: Mutex<Option<PathBuf>>,
    }

    impl RemediationRunner {
        fn new(lists: Vec<String>, disable_result: Result<CliExecOutput, String>) -> Self {
            Self {
                lists: Mutex::new(lists.into()),
                disable_result,
                list_calls: AtomicUsize::new(0),
                disable_calls: AtomicUsize::new(0),
                executable: Mutex::new(None),
            }
        }
    }

    impl AntigravityPluginListRunner for RemediationRunner {
        fn list_plugins(&self, _agy_executable: &Path) -> Result<CliExecOutput, String> {
            self.list_calls.fetch_add(1, Ordering::SeqCst);
            let stdout = self.lists.lock().unwrap().pop_front().unwrap_or_default();
            Ok(CliExecOutput {
                exit_code: 0,
                stdout,
                stderr: String::new(),
            })
        }
    }

    impl AntigravityPluginRemediationRunner for RemediationRunner {
        fn disable_wt_agent_hooks(&self, agy_executable: &Path) -> Result<CliExecOutput, String> {
            self.disable_calls.fetch_add(1, Ordering::SeqCst);
            *self.executable.lock().unwrap() = Some(agy_executable.to_path_buf());
            self.disable_result.clone()
        }
    }

    fn disable_output(exit_code: i32) -> Result<CliExecOutput, String> {
        Ok(CliExecOutput {
            exit_code,
            stdout: String::new(),
            stderr: String::new(),
        })
    }

    #[test]
    fn confirmed_disable_revalidates_risk_uses_canonical_cli_and_checks_disabled_state() {
        let temp = TempDir::new().unwrap();
        let home = setup_home(&temp, &risky_hooks_json(), "0.1.2");
        let plugin_dir = home.join(".gemini/config/plugins/wt-agent-hooks");
        let runner = RemediationRunner::new(
            vec![plugin_list(&plugin_dir, true), plugin_list(&plugin_dir, false)],
            disable_output(0),
        );

        let result = disable_global_wt_agent_hooks_with_platform(Some(&home), &runner, true).unwrap();

        assert_eq!(result.state, AntigravityGlobalHookDiagnosticState::Disabled);
        assert_eq!(runner.list_calls.load(Ordering::SeqCst), 2);
        assert_eq!(runner.disable_calls.load(Ordering::SeqCst), 1);
        let expected_executable = home.join(".gemini/bin/agy.exe").canonicalize().unwrap();
        assert_eq!(
            runner.executable.lock().unwrap().as_deref(),
            Some(expected_executable.as_path())
        );
    }

    #[test]
    fn already_disabled_is_idempotent_and_unverified_state_never_disables() {
        let temp = TempDir::new().unwrap();
        let home = setup_home(&temp, &risky_hooks_json(), "0.1.2");
        let plugin_dir = home.join(".gemini/config/plugins/wt-agent-hooks");
        let disabled = RemediationRunner::new(vec![plugin_list(&plugin_dir, false)], disable_output(0));
        assert_eq!(
            disable_global_wt_agent_hooks_with_platform(Some(&home), &disabled, true)
                .unwrap()
                .state,
            AntigravityGlobalHookDiagnosticState::Disabled
        );
        assert_eq!(disabled.disable_calls.load(Ordering::SeqCst), 0);

        let unknown_home = setup_home(&temp, &risky_hooks_json(), "0.1.3");
        let unknown_dir = unknown_home.join(".gemini/config/plugins/wt-agent-hooks");
        let unknown = RemediationRunner::new(vec![plugin_list(&unknown_dir, true)], disable_output(0));
        assert!(disable_global_wt_agent_hooks_with_platform(Some(&unknown_home), &unknown, true).is_err());
        assert_eq!(unknown.disable_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn failed_disable_or_unverified_postcheck_never_reports_success() {
        let temp = TempDir::new().unwrap();
        let home = setup_home(&temp, &risky_hooks_json(), "0.1.2");
        let plugin_dir = home.join(".gemini/config/plugins/wt-agent-hooks");
        let nonzero = RemediationRunner::new(
            vec![plugin_list(&plugin_dir, true)],
            disable_output(1),
        );
        assert!(disable_global_wt_agent_hooks_with_platform(Some(&home), &nonzero, true).is_err());
        assert_eq!(nonzero.list_calls.load(Ordering::SeqCst), 1);

        let remains_enabled = RemediationRunner::new(
            vec![plugin_list(&plugin_dir, true), plugin_list(&plugin_dir, true)],
            disable_output(0),
        );
        assert!(disable_global_wt_agent_hooks_with_platform(Some(&home), &remains_enabled, true).is_err());
        assert_eq!(remains_enabled.list_calls.load(Ordering::SeqCst), 2);
        assert_eq!(remains_enabled.disable_calls.load(Ordering::SeqCst), 1);

        let timeout = RemediationRunner::new(
            vec![plugin_list(&plugin_dir, true)],
            Err("timed out".to_string()),
        );
        assert!(disable_global_wt_agent_hooks_with_platform(Some(&home), &timeout, true).is_err());
        assert_eq!(timeout.disable_calls.load(Ordering::SeqCst), 1);
    }
}

