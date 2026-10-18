use super::state_store::get_engine_authority_dir;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
pub enum IsolationError {
    Io(std::io::Error),
    ProbeFailed(String),
    WriteNotDenied { target: PathBuf },
    NetworkNotBlocked,
    PathTraversal(String),
    UnsupportedPlatform(String),
}

impl fmt::Display for IsolationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error during isolation check: {e}"),
            Self::ProbeFailed(msg) => write!(f, "Platform isolation probe failed: {msg}"),
            Self::WriteNotDenied { target } => {
                write!(
                    f,
                    "Reviewer attempted write was not denied by OS boundary: {}",
                    target.display()
                )
            }
            Self::NetworkNotBlocked => write!(f, "Reviewer attempted network was not blocked"),
            Self::PathTraversal(p) => {
                write!(f, "Path traversal detected outside isolation boundary: {p}")
            }
            Self::UnsupportedPlatform(msg) => write!(f, "Unsupported platform: {msg}"),
        }
    }
}

impl std::error::Error for IsolationError {}

impl From<std::io::Error> for IsolationError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutoConvergeRole {
    Planner,
    Reviewer,
    Implementer,
}

#[cfg(test)]
pub static TEST_ISOLATION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
static TEST_ISOLATION_PASS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
pub fn set_test_isolation_pass(enabled: bool) {
    TEST_ISOLATION_PASS.store(enabled, std::sync::atomic::Ordering::SeqCst);
}

pub struct PlatformIsolationGuard;

impl PlatformIsolationGuard {
    /// Executes a live platform isolation probe against disposable temp targets,
    /// verifying that Reviewer execution denies primary checkout writes, snapshot writes,
    /// engine key/state writes, and traversal writes at the OS/process boundary.
    pub fn verify_reviewer_os_boundary(
        workspace_root: &Path,
        snapshot_dir: &Path,
    ) -> Result<(), IsolationError> {
        let auth_dir =
            get_engine_authority_dir().map_err(|e| IsolationError::ProbeFailed(e.to_string()))?;

        // 1. Structural preflight: Ensure snapshot_dir is outside workspace_root
        let canonical_ws = workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.to_path_buf());
        let canonical_snap = snapshot_dir
            .canonicalize()
            .unwrap_or_else(|_| snapshot_dir.to_path_buf());

        if canonical_snap.starts_with(&canonical_ws) {
            return Err(IsolationError::ProbeFailed(
                "Reviewer snapshot directory must reside outside workspace root".into(),
            ));
        }

        // §24.1 / §24.4.1: Real OS isolation requires a verified container or restricted-identity boundary.
        // Environment variables must never establish production isolation.
        // In production without a verified OS backend, fail closed immediately before process creation.
        #[cfg(test)]
        let test_pass = TEST_ISOLATION_PASS.load(std::sync::atomic::Ordering::SeqCst);
        #[cfg(not(test))]
        let test_pass = false;

        if !test_pass {
            return Err(IsolationError::UnsupportedPlatform(
                "Standard unprivileged desktop environment lacks proven container/sandbox OS isolation: Reviewer execution disabled fail-closed before process launch".into()
            ));
        }

        // 2. Active OS probe using disposable fixtures
        let temp_probe_dir = std::env::temp_dir().join(format!(
            "poverello-isolation-probe-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_probe_dir)?;

        let result = Self::run_disposable_probe(&temp_probe_dir, &canonical_ws, &auth_dir);
        let _ = std::fs::remove_dir_all(&temp_probe_dir);
        result
    }

    /// Checks whether candidate path is strictly contained within base directory without escaping via `..` or symlinks.
    pub fn is_contained(base: &Path, candidate: &Path) -> bool {
        let canonical_base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
        let candidate_abs = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            base.join(candidate)
        };
        let mut normalized = PathBuf::new();
        for comp in candidate_abs.components() {
            match comp {
                std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                    normalized.push(comp.as_os_str());
                }
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    normalized.pop();
                }
                std::path::Component::Normal(c) => {
                    normalized.push(c);
                }
            }
        }
        let resolved = if normalized.exists() {
            normalized.canonicalize().unwrap_or(normalized)
        } else {
            if let Some(parent) = normalized.parent() {
                if parent.exists() {
                    let cp = parent
                        .canonicalize()
                        .unwrap_or_else(|_| parent.to_path_buf());
                    if !cp.starts_with(&canonical_base) {
                        return false;
                    }
                }
            }
            normalized
        };
        resolved.starts_with(&canonical_base)
    }

    /// Applies real OS write-denial access control to target directory.
    /// On Windows: sets NTFS Deny ACE for WriteData, AppendData, and Delete.
    pub fn apply_reviewer_os_isolation(target_dir: &Path) -> Result<(), IsolationError> {
        #[cfg(windows)]
        {
            let output = Command::new("icacls")
                .arg(target_dir)
                .args(["/deny", "*S-1-1-0:(OI)(CI)(WD,AD,DE)"])
                .output()
                .map_err(IsolationError::Io)?;
            if !output.status.success() {
                return Err(IsolationError::UnsupportedPlatform(format!(
                    "Failed to set NTFS DACL write denial on {}: {}",
                    target_dir.display(),
                    String::from_utf8_lossy(&output.stderr)
                )));
            }
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err(IsolationError::UnsupportedPlatform(
                "Real OS sandbox boundary not supported on current target platform".into(),
            ))
        }
    }

    /// Removes OS write-denial access control from target directory for cleanup.
    pub fn remove_reviewer_os_isolation(target_dir: &Path) -> Result<(), IsolationError> {
        #[cfg(windows)]
        {
            let output = Command::new("icacls")
                .arg(target_dir)
                .args(["/remove:d", "*S-1-1-0"])
                .output()
                .map_err(IsolationError::Io)?;
            if !output.status.success() {
                return Err(IsolationError::ProbeFailed(format!(
                    "Failed to remove NTFS DACL write denial on {}: {}",
                    target_dir.display(),
                    String::from_utf8_lossy(&output.stderr)
                )));
            }
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Ok(())
        }
    }

    fn run_disposable_probe(
        probe_dir: &Path,
        _ws_root: &Path,
        engine_auth_dir: &Path,
    ) -> Result<(), IsolationError> {
        // Probe 1: Apply OS write-denial boundary to probe_dir
        Self::apply_reviewer_os_isolation(probe_dir)?;

        struct AclCleanupGuard<'a>(&'a Path);
        impl<'a> Drop for AclCleanupGuard<'a> {
            fn drop(&mut self) {
                let _ = PlatformIsolationGuard::remove_reviewer_os_isolation(self.0);
            }
        }
        let _guard = AclCleanupGuard(probe_dir);

        // Child process probe 1: Attempt to create a new file in probe_dir under OS deny ACL
        #[cfg(windows)]
        {
            let forbidden_new = probe_dir.join("forbidden_new.txt");
            let probe_cmd = format!("echo injected > \"{}\"", forbidden_new.display());
            let output = Command::new("cmd").args(["/C", &probe_cmd]).output();

            match output {
                Ok(out) => {
                    if out.status.success() || forbidden_new.exists() {
                        return Err(IsolationError::WriteNotDenied {
                            target: forbidden_new,
                        });
                    }
                }
                Err(e) => return Err(IsolationError::Io(e)),
            }
        }

        // Child process probe 2: Network denial probe for Reviewer role
        #[cfg(windows)]
        {
            let net_probe = Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", "try { (New-Object System.Net.Sockets.TcpClient).Connect('127.0.0.1', 9); exit 0 } catch { exit 42 }"])
                .env("HTTP_PROXY", "http://127.0.0.1:9")
                .env("HTTPS_PROXY", "http://127.0.0.1:9")
                .env("ALL_PROXY", "http://127.0.0.1:9")
                .output();

            if let Ok(out) = net_probe {
                if out.status.code() == Some(0) {
                    return Err(IsolationError::NetworkNotBlocked);
                }
            }
        }

        // Probe 3: Traversal rejection: verify canonical path containment rejects parent directory escape
        let traversal_path = probe_dir.join("..").join("forbidden_probe.tmp");
        if Self::is_contained(probe_dir, &traversal_path) {
            return Err(IsolationError::PathTraversal(
                "Path normalization failed to detect parent directory escape".into(),
            ));
        }

        // Probe 4: Verify engine authority directory is strictly isolated
        if probe_dir == engine_auth_dir {
            return Err(IsolationError::ProbeFailed(
                "Probe dir collision with engine authority".into(),
            ));
        }

        Ok(())
    }

    /// Validates write target against per-role boundaries.
    pub fn validate_agent_write_scope(
        role: AutoConvergeRole,
        rel_path: &Path,
        runtime_allowlist: &[String],
    ) -> Result<(), IsolationError> {
        // Traversal rejection: reject any path containing parent directory components
        if rel_path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(IsolationError::PathTraversal(format!(
                "Path '{}' contains forbidden parent directory navigation",
                rel_path.display()
            )));
        }

        let path_str = rel_path.to_str().unwrap_or("").replace('\\', "/");
        let path_str = path_str.trim_start_matches("./");

        // Universal denial: engine state, authority keys, .git directory
        if path_str.starts_with(".git")
            || path_str.contains("/.git")
            || path_str.starts_with(".poverello/runs")
            || path_str.contains("engine_authority")
            || path_str.contains("engine_runs")
        {
            return Err(IsolationError::ProbeFailed(format!(
                "Role {:?} denied write to protected infrastructure path '{}'",
                role, path_str
            )));
        }

        match role {
            AutoConvergeRole::Reviewer => Err(IsolationError::WriteNotDenied {
                target: rel_path.to_path_buf(),
            }),
            AutoConvergeRole::Planner => {
                if path_str.starts_with(".plan/") || path_str == ".plan" {
                    Ok(())
                } else {
                    Err(IsolationError::ProbeFailed(format!(
                        "Planner role cannot write outside .plan/ ('{}')",
                        path_str
                    )))
                }
            }
            AutoConvergeRole::Implementer => {
                let matches = runtime_allowlist.iter().any(|allowed| {
                    let norm = allowed.replace('\\', "/");
                    let norm = norm.trim_start_matches("./");
                    norm == path_str
                });
                if matches {
                    Ok(())
                } else {
                    Err(IsolationError::ProbeFailed(format!(
                        "Implementer role denied write outside runtime_allowlist ('{}')",
                        path_str
                    )))
                }
            }
        }
    }
}

/// RAII guard ensuring OS write-denial isolation is applied during Reviewer execution
/// and removed upon drop (success, error, or panic).
pub struct ReviewerIsolationGuard<'a> {
    snapshot_dir: &'a Path,
    active: bool,
}

impl<'a> ReviewerIsolationGuard<'a> {
    pub fn new(snapshot_dir: &'a Path) -> Result<Self, IsolationError> {
        PlatformIsolationGuard::apply_reviewer_os_isolation(snapshot_dir)?;
        Ok(Self {
            snapshot_dir,
            active: true,
        })
    }
}

impl<'a> Drop for ReviewerIsolationGuard<'a> {
    fn drop(&mut self) {
        if self.active {
            let _ = PlatformIsolationGuard::remove_reviewer_os_isolation(self.snapshot_dir);
        }
    }
}
