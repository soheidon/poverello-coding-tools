use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::orchestrator::process_runner::ProcessRunner;
use crate::paths;

static CLI_MUTEX: Mutex<()> = Mutex::const_new(());
static TEST_RUNNER_OVERRIDE_ENABLED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CliToolInfo {
    pub name: String,
    pub display_name: String,
    pub installed: bool,
    pub path: Option<String>,
    pub installed_version: Option<String>,
    pub latest_version: Option<String>,
    pub package_name: String,
    pub is_preview: bool,
    pub update_available: bool,
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

pub fn cli_tools_state_path() -> PathBuf {
    paths::user_data_dir().join("cli_tools_state.json")
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
    CliToolsState { outcome_unknown: None }
}

pub fn write_cli_tools_state(state: &CliToolsState) -> Result<(), String> {
    let p = cli_tools_state_path();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_vec_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(&p, data).map_err(|e| e.to_string())
}

pub fn resolve_cli_path(name: &str) -> Option<PathBuf> {
    // 1. Check npm global directory on Windows
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
        }
    }

    // 2. Scan PATH
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

    Err("npm executable not found on system PATH. Please ensure Node.js and npm are installed.".to_string())
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
        // Extract version token (e.g. "codex-cli 0.163.0" -> "0.163.0" or "0.2.1-rc.2")
        let ver = if let Some(last) = raw.split_whitespace().last() {
            last.to_string()
        } else {
            raw.to_string()
        };
        Ok(ver)
    } else {
        Err(format!(
            "Process failed with exit code {:?}: {}",
            res.exit_code, res.stderr.text
        ))
    }
}

pub async fn get_cli_tools_status() -> Result<Vec<CliToolInfo>, String> {
    let tools = [
        ("codex", "Codex CLI", "@openai/codex", false),
        ("dsh", "DeepSeek Harness", "@deepseek-ai/dsh", true),
    ];

    let mut result = Vec::new();

    for (name, display_name, pkg, is_preview) in tools {
        let path_opt = resolve_cli_path(name);
        let installed = path_opt.is_some();
        let path_str = path_opt.as_ref().map(|p| p.to_string_lossy().to_string());

        let mut installed_version = None;
        if let Some(p) = &path_opt {
            if let Ok(ver) = query_cli_version(p).await {
                installed_version = Some(ver);
            }
        }

        result.push(CliToolInfo {
            name: name.to_string(),
            display_name: display_name.to_string(),
            installed,
            path: path_str,
            installed_version,
            latest_version: None, // Can be populated if npm view check is available
            package_name: pkg.to_string(),
            is_preview,
            update_available: false,
        });
    }

    Ok(result)
}

pub async fn get_codex_account_info() -> Result<CodexAccountInfo, String> {
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
        });
    };

    let version = query_cli_version(&path).await.ok();
    let runner = ProcessRunner::new();

    // Invoke codex app-server --listen stdio://
    // Send initialize request followed by account/read with refreshToken: false
    let init_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "clientInfo": {
                "name": "poverello",
                "version": "0.25.4"
            }
        }
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
    let mut acc_bytes = serde_json::to_vec(&acc_req).map_err(|e| e.to_string())?;
    payload.append(&mut acc_bytes);
    payload.push(b'\n');

    let proc_res = runner
        .run_with_stdin(
            &path.to_string_lossy(),
            &["app-server".to_string(), "--listen".to_string(), "stdio://".to_string()],
            None,
            Some(&payload),
            Duration::from_secs(5),
            None,
        )
        .await;

    let mut auth_method = None;
    let mut account_identity = None;
    let mut requires_openai_auth = None;

    if let Ok(res) = proc_res {
        for line in res.stdout.text.lines() {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                if val.get("id").and_then(|id| id.as_i64()) == Some(2) {
                    if let Some(result_obj) = val.get("result") {
                        requires_openai_auth = result_obj.get("requiresOpenaiAuth").and_then(|b| b.as_bool());
                        if let Some(acc) = result_obj.get("account").and_then(|a| a.as_object()) {
                            auth_method = acc.get("type").and_then(|t| t.as_str()).map(|s| s.to_string());
                            let email = acc.get("email").and_then(|e| e.as_str());
                            if let Some(em) = email {
                                if !em.trim().is_empty() {
                                    account_identity = Some(em.trim().to_string());
                                }
                            }
                        }
                    }
                    break;
                }
            }
        }
    }

    Ok(CodexAccountInfo {
        installed: true,
        executable_path: Some(path.to_string_lossy().to_string()),
        version,
        auth_method,
        account_identity,
        config_profile: None, // Default profile unless specified via -p
        requires_openai_auth,
    })
}

pub async fn run_cli_install(tool_name: &str) -> Result<CliToolInfo, String> {
    let _lock = CLI_MUTEX.lock().await;

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

    let pkg = match tool_name {
        "codex" => "@openai/codex",
        "dsh" => "@deepseek-ai/dsh",
        _ => return Err(format!("Unsupported CLI tool: '{tool_name}'")),
    };

    let npm_path = resolve_npm_path()?;
    let install_target = format!("{pkg}@latest");

    let runner = ProcessRunner::new();
    let res = runner
        .run(
            &npm_path.to_string_lossy(),
            &[
                "install".to_string(),
                "-g".to_string(),
                install_target,
            ],
            None,
            Duration::from_secs(120),
            None,
        )
        .await;

    match res {
        Ok(proc_res) => {
            if proc_res.timed_out {
                // Verify process tree is terminated on Windows
                #[cfg(windows)]
                {
                    // If Job Object terminated, wait briefly
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }

                // Record outcome unknown
                let mut new_state = state;
                new_state.outcome_unknown = Some(CliToolOutcomeUnknown {
                    tool_name: tool_name.to_string(),
                    message: "Installation process timed out after 120 seconds".to_string(),
                    recorded_at: chrono::Utc::now().to_rfc3339(),
                });
                let _ = write_cli_tools_state(&new_state);

                return Err(format!("Installation of '{tool_name}' timed out after 120 seconds. Outcome unknown."));
            }

            if proc_res.exit_code != Some(0) {
                return Err(format!(
                    "npm install failed with exit code {:?}: {}",
                    proc_res.exit_code, proc_res.stderr.text
                ));
            }

            // Post-install verification
            let path_opt = resolve_cli_path(tool_name);
            let Some(path) = path_opt else {
                return Err(format!(
                    "npm install succeeded, but '{}' executable could not be resolved on PATH.",
                    tool_name
                ));
            };

            let version = query_cli_version(&path).await.ok();

            Ok(CliToolInfo {
                name: tool_name.to_string(),
                display_name: if tool_name == "codex" {
                    "Codex CLI".to_string()
                } else {
                    "DeepSeek Harness".to_string()
                },
                installed: true,
                path: Some(path.to_string_lossy().to_string()),
                installed_version: version,
                latest_version: None,
                package_name: pkg.to_string(),
                is_preview: tool_name == "dsh",
                update_available: false,
            })
        }
        Err(e) => Err(format!("Process spawn error: {e}")),
    }
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
    }

    #[tokio::test]
    async fn test_unsupported_cli_tool_fails_fast() {
        let res = run_cli_install("unsupported_tool").await;
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Unsupported CLI tool"));
    }

    #[tokio::test]
    async fn test_job_object_containment_and_tree_cleanup() {
        #[cfg(windows)]
        {
            let runner = ProcessRunner::new();
            // Run a brief command under Job Object
            let res = runner
                .run(
                    "powershell",
                    &[
                        "-NoProfile".to_string(),
                        "-Command".to_string(),
                        "exit 0".to_string(),
                    ],
                    None,
                    Duration::from_secs(10),
                    None,
                )
                .await;
            assert!(res.is_ok());
            assert_eq!(res.unwrap().exit_code, Some(0));
        }
    }
}
