use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::types::sha256_hex;
pub use super::types::RecoverySnapshotBinding;

// Maximum byte bounds to prevent unbounded allocations on malformed files (§293.3 #8)
pub const MAX_INDEX_FILE_BYTES: u64 = 64 * 1024 * 1024; // 64 MiB
pub const MAX_MANIFEST_FILE_BYTES: u64 = 64 * 1024 * 1024; // 64 MiB

// Test failure injection seams coordinated via TEST_SEAM_MUTEX (§293.4 #6)
pub static TEST_SEAM_MUTEX: Mutex<()> = Mutex::new(());
pub static INJECT_REPARSE_POINT_REJECTION: AtomicBool = AtomicBool::new(false);
pub static INJECT_MUTATION_DURING_COPY: AtomicBool = AtomicBool::new(false);
pub static INJECT_STAGING_CORRUPTION: AtomicBool = AtomicBool::new(false);
pub static INJECT_STAGING_CLEANUP_FAILURE: AtomicBool = AtomicBool::new(false);
pub static INJECT_MUTATION_BEFORE_FINAL_PREFLIGHT: AtomicBool = AtomicBool::new(false);
pub static GIT_INDEX_COMMAND_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static SEAM_MOCK_SYMBOLIC_REF: Mutex<
    Option<Result<(i32, Vec<u8>, Vec<u8>), std::io::ErrorKind>>,
> = Mutex::new(None);

// Protected-file read observer for T25 junction test (§300.3 #8)
pub static PROTECTED_READ_OBSERVER_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static PROTECTED_READ_OBSERVER_TARGET: Mutex<Option<(u32, u64)>> = Mutex::new(None);

pub fn set_protected_read_observer_target(target: Option<(u32, u64)>) {
    *PROTECTED_READ_OBSERVER_TARGET.lock().unwrap() = target;
    PROTECTED_READ_OBSERVER_COUNT.store(0, Ordering::SeqCst);
}

pub fn get_protected_read_observer_count() -> usize {
    PROTECTED_READ_OBSERVER_COUNT.load(Ordering::SeqCst)
}

pub(crate) fn check_protected_read_observer(id: (u32, u64)) {
    if let Ok(guard) = PROTECTED_READ_OBSERVER_TARGET.lock() {
        if let Some(target) = *guard {
            if target == id {
                PROTECTED_READ_OBSERVER_COUNT.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
}

/// Ephemeral in-memory inventory entry for staged descendants (§304.3 #2, §307.2 #1).
/// Not serialized, never added to persisted state schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescendantInventoryEntry {
    pub rel_path: String,
    pub is_dir: bool,
    pub id: (u32, u64),
}

/// Ephemeral in-memory capability lease authorizing exact-owned cleanup (§300.3 #5, §301.3 #3, §304.3 #2).
/// Not serialized, never added to persisted state schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotOwnershipLease {
    pub engine_recovery_root_id: (u32, u64),
    pub run_id: String,
    pub staging_dir_id: (u32, u64),
    pub published_dir_id: Option<(u32, u64)>,
    pub manifest_digest: String,
    pub staging_path: PathBuf,
    pub published_path: PathBuf,
    pub inventory: Vec<DescendantInventoryEntry>,
}

/// Component-aware path containment helper (§300.3 #3).
/// Compares canonical path components, handles Windows case-insensitivity,
/// rejects prefix siblings (e.g. C:\repo vs C:\repo-elsewhere), and distinguishes
/// non-contained components.
pub fn is_subpath_component_aware(parent: &Path, child: &Path) -> bool {
    let parent_comps: Vec<_> = parent.components().collect();
    let child_comps: Vec<_> = child.components().collect();
    if child_comps.len() < parent_comps.len() {
        return false;
    }
    if child_comps
        .iter()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return false;
    }
    for (p, c) in parent_comps.iter().zip(child_comps.iter()) {
        let p_str = p.as_os_str().to_string_lossy();
        let c_str = c.as_os_str().to_string_lossy();
        #[cfg(windows)]
        {
            if !p_str.eq_ignore_ascii_case(&c_str) {
                return false;
            }
        }
        #[cfg(not(windows))]
        {
            if p_str != c_str {
                return false;
            }
        }
    }
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtectedSource {
    Tracked,
    Staged,
    Untracked,
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotEntry {
    pub path: String,
    pub sources: Vec<ProtectedSource>,
    pub kind: EntryKind,
    pub mode: u32,
    pub size: u64,
    pub sha256: String,
    pub present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadMode {
    Symbolic,
    Detached,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitSnapshot {
    pub head_commit: String,
    pub head_ref: Option<String>,
    pub head_mode: HeadMode,
    pub refs_digest: String,
    pub index_entries_digest: String,
    pub index_file_digest: String,
    pub status_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverySnapshot {
    pub run_id: String,
    pub root: PathBuf,
    pub entries: Vec<SnapshotEntry>,
    pub git: GitSnapshot,
    pub manifest_digest: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotPolicy {
    pub excluded_roots: Vec<String>,
    pub ignored_paths: Vec<String>,
    pub max_total_bytes: u64,
    pub max_file_bytes: u64,
    pub max_entries: usize,
}

impl Default for SnapshotPolicy {
    fn default() -> Self {
        Self {
            excluded_roots: vec![
                ".git".to_string(),
                ".plan".to_string(),
                ".poverello".to_string(),
                "target".to_string(),
                "node_modules".to_string(),
            ],
            ignored_paths: Vec::new(),
            max_total_bytes: 1_073_741_824, // 1 GiB
            max_file_bytes: 67_108_864,     // 64 MiB
            max_entries: 100_000,
        }
    }
}

#[derive(Debug)]
pub enum SnapshotError {
    GitCommandFailed {
        cmd: String,
        stderr: String,
    },
    UnsupportedIndexLayout(String),
    IllegalPath(String),
    ReparsePointRejected(String),
    FileTooLarge {
        path: String,
        size: u64,
        limit: u64,
    },
    TotalSizeExceeded {
        total: u64,
        limit: u64,
    },
    MaxEntriesExceeded {
        count: usize,
        limit: usize,
    },
    SizeOverflow,
    Collision(PathBuf),
    ConcurrentMutation(String),
    ManifestVerificationFailed {
        expected: String,
        computed: String,
    },
    StagedFileVerificationFailed {
        path: String,
        reason: String,
    },
    CleanupFailed {
        staging_dir: PathBuf,
        source: std::io::Error,
    },
    CompensationFailed {
        primary: String,
        cleanup_error: String,
        remaining_path: PathBuf,
    },
    UnbornHead,
    UnsupportedPlatform(String),
    Io(std::io::Error),
    IoPath {
        op: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    Serialization(serde_json::Error),
}

pub fn io_err(op: &'static str, path: impl Into<PathBuf>, err: std::io::Error) -> SnapshotError {
    SnapshotError::IoPath {
        op,
        path: path.into(),
        source: err,
    }
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitCommandFailed { cmd, stderr } => {
                write!(f, "Git command failed ({cmd}): {stderr}")
            }
            Self::UnsupportedIndexLayout(msg) => {
                write!(f, "Unsupported index or repository layout: {msg}")
            }
            Self::IllegalPath(msg) => write!(f, "Path traversal or illegal path rejected: {msg}"),
            Self::ReparsePointRejected(msg) => {
                write!(f, "Reparse point or symbolic link rejected: {msg}")
            }
            Self::FileTooLarge { path, size, limit } => {
                write!(f, "File size limit exceeded for '{path}': {size} > {limit}")
            }
            Self::TotalSizeExceeded { total, limit } => {
                write!(f, "Total snapshot size limit exceeded: {total} > {limit}")
            }
            Self::MaxEntriesExceeded { count, limit } => {
                write!(f, "Maximum entry count exceeded: {count} > {limit}")
            }
            Self::SizeOverflow => write!(f, "Arithmetic overflow in byte accounting"),
            Self::Collision(path) => {
                write!(f, "Snapshot directory collision: '{path:?}' already exists")
            }
            Self::ConcurrentMutation(msg) => write!(
                f,
                "Concurrent repository or workspace mutation detected during snapshot: {msg}"
            ),
            Self::ManifestVerificationFailed { expected, computed } => write!(
                f,
                "Manifest verification failed: expected {expected}, computed {computed}"
            ),
            Self::StagedFileVerificationFailed { path, reason } => {
                write!(f, "Staged file verification failed for '{path}': {reason}")
            }
            Self::CleanupFailed {
                staging_dir,
                source,
            } => write!(
                f,
                "Cleanup of staging directory failed at {staging_dir:?}: {source}"
            ),
            Self::CompensationFailed {
                primary,
                cleanup_error,
                remaining_path,
            } => write!(
                f,
                "Snapshot compensation failed: primary error '{primary}'; cleanup error '{cleanup_error}' at {remaining_path:?}"
            ),
            Self::UnbornHead => write!(
                f,
                "Unborn repository HEAD is not supported for RecoverySnapshot"
            ),
            Self::UnsupportedPlatform(msg) => {
                write!(f, "Operation unsupported on current platform: {msg}")
            }
            Self::Io(err) => write!(f, "IO error: {err}"),
            Self::IoPath { op, path, source } => {
                write!(f, "IO error during {op} at {path:?}: {source}")
            }
            Self::Serialization(err) => write!(f, "Serialization error: {err}"),
        }
    }
}

impl std::error::Error for SnapshotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::IoPath { source, .. } => Some(source),
            Self::CleanupFailed { source, .. } => Some(source),
            Self::Serialization(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for SnapshotError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_json::Error> for SnapshotError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err)
    }
}

/// Helper struct for managing temporary hermetic empty git config file.
pub struct HermeticGitEnv {
    temp_dir: PathBuf,
    empty_config_path: PathBuf,
}

impl HermeticGitEnv {
    pub fn new(temp_root: &Path) -> Result<Self, SnapshotError> {
        let unique_id = uuid::Uuid::new_v4().to_string();
        let config_file = temp_root.join(format!(".empty-gitconfig-{unique_id}"));
        fs::write(&config_file, b"")
            .map_err(|e| io_err("write empty gitconfig", &config_file, e))?;
        Ok(Self {
            temp_dir: temp_root.to_path_buf(),
            empty_config_path: config_file,
        })
    }

    pub fn config_path(&self) -> &Path {
        &self.empty_config_path
    }

    pub fn temp_dir(&self) -> &Path {
        &self.temp_dir
    }
}

impl Drop for HermeticGitEnv {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.empty_config_path);
    }
}

/// Resolves Git executable to a validated absolute path without following reparse points (§293.4 #5, §296.3 #7).
pub fn resolve_git_executable() -> Result<PathBuf, SnapshotError> {
    let check_candidate = |candidate: &Path| -> Option<PathBuf> {
        let canon = fs::canonicalize(candidate).ok()?;
        if !canon.is_file() {
            return None;
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            let sym_meta = fs::symlink_metadata(candidate).ok()?;
            if (sym_meta.file_attributes() & 0x400) != 0 {
                return None; // Reject reparse point
            }
            let canon_meta = fs::symlink_metadata(&canon).ok()?;
            if (canon_meta.file_attributes() & 0x400) != 0 {
                return None;
            }
        }
        Some(canon)
    };

    #[cfg(windows)]
    {
        let standard_path = PathBuf::from(r"C:\Program Files\Git\cmd\git.exe");
        if let Some(valid) = check_candidate(&standard_path) {
            return Ok(valid);
        }
    }

    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            #[cfg(windows)]
            let candidate = dir.join("git.exe");
            #[cfg(not(windows))]
            let candidate = dir.join("git");
            if let Some(valid) = check_candidate(&candidate) {
                return Ok(valid);
            }
        }
    }

    Err(SnapshotError::GitCommandFailed {
        cmd: "git".to_string(),
        stderr: "Failed to resolve absolute path for git executable".to_string(),
    })
}

/// Builds a read-only git command in a minimal hermetic environment (§282.2, §289.3 #1, §293.4 #5)
/// with resolved executable, `.env_clear()`, and `--no-optional-locks` preceding the subcommand.
pub fn build_hermetic_git_command(
    repo_root: &Path,
    args: &[&str],
    empty_config_path: &Path,
    temp_dir: &Path,
) -> Result<std::process::Command, SnapshotError> {
    let git_exe = resolve_git_executable()?;
    let mut cmd = std::process::Command::new(&git_exe);
    cmd.arg("--no-optional-locks");
    cmd.arg("-C");
    cmd.arg(repo_root);

    // Explicit command-scoped configuration disabling FSMonitor and UntrackedCache
    cmd.arg("-c");
    cmd.arg("core.fsmonitor=false");
    cmd.arg("-c");
    cmd.arg("core.untrackedCache=false");

    for arg in args {
        cmd.arg(arg);
    }

    // Completely clear the inherited process environment
    cmd.env_clear();

    // Retain only minimal allowlisted variables required for Git process bootstrapping
    #[cfg(windows)]
    {
        if let Some(sysroot) = std::env::var_os("SystemRoot") {
            cmd.env("SystemRoot", sysroot);
        }
        if let Some(windir) = std::env::var_os("WINDIR") {
            cmd.env("WINDIR", windir);
        }
    }
    // Restrict PATH to Git directory and system directory
    let git_dir = git_exe.parent().unwrap_or(Path::new(""));
    let mut allowed_paths = vec![git_dir.to_path_buf()];
    #[cfg(windows)]
    {
        if let Some(sysroot) = std::env::var_os("SystemRoot") {
            let p = PathBuf::from(sysroot);
            allowed_paths.push(p.join("System32"));
            allowed_paths.push(p);
        }
    }
    let joined_path = std::env::join_paths(allowed_paths).map_err(|e| {
        SnapshotError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to build PATH: {e}"),
        ))
    })?;
    cmd.env("PATH", joined_path);

    cmd.env("TEMP", temp_dir);
    cmd.env("TMP", temp_dir);
    cmd.env("GIT_CONFIG_NOSYSTEM", "1");
    cmd.env("GIT_CONFIG_GLOBAL", empty_config_path);
    cmd.env("GIT_OPTIONAL_LOCKS", "0");

    Ok(cmd)
}

/// Executes a read-only git command in a minimal hermetic environment.
pub fn run_hermetic_git(
    repo_root: &Path,
    args: &[&str],
    empty_config_path: &Path,
    temp_dir: &Path,
) -> Result<std::process::Output, SnapshotError> {
    let mut cmd = build_hermetic_git_command(repo_root, args, empty_config_path, temp_dir)?;
    cmd.output().map_err(SnapshotError::Io)
}

pub fn run_hermetic_git_stdout(
    repo_root: &Path,
    args: &[&str],
    empty_config_path: &Path,
    temp_dir: &Path,
) -> Result<Vec<u8>, SnapshotError> {
    let output = run_hermetic_git(repo_root, args, empty_config_path, temp_dir)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let cmd_str = format!("git {}", args.join(" "));
        return Err(SnapshotError::GitCommandFailed {
            cmd: cmd_str,
            stderr,
        });
    }
    Ok(output.stdout)
}

/// Enforces strict symbolic-ref -q HEAD outcomes with deterministic test seam (§283.2, §285.3, §293.4 #10).
pub fn query_symbolic_or_detached_head(
    repo_root: &Path,
    env: &HermeticGitEnv,
) -> Result<(HeadMode, Option<String>), SnapshotError> {
    // Check deterministic test mock seam
    if let Ok(guard) = SEAM_MOCK_SYMBOLIC_REF.lock() {
        if let Some(ref mock_result) = *guard {
            match mock_result {
                Ok((code, stdout, stderr)) => {
                    return match *code {
                        0 => {
                            let stdout_str = String::from_utf8(stdout.clone()).map_err(|_| {
                                SnapshotError::GitCommandFailed {
                                    cmd: "git symbolic-ref -q HEAD".to_string(),
                                    stderr: "Non-UTF-8 output from symbolic-ref".to_string(),
                                }
                            })?;
                            let sref = stdout_str.trim().to_string();
                            if sref.starts_with("refs/") {
                                Ok((HeadMode::Symbolic, Some(sref)))
                            } else {
                                Err(SnapshotError::GitCommandFailed {
                                    cmd: "git symbolic-ref -q HEAD".to_string(),
                                    stderr: format!("Invalid symbolic-ref output: '{sref}'"),
                                })
                            }
                        }
                        1 => Ok((HeadMode::Detached, None)),
                        other => {
                            let stderr_str = String::from_utf8_lossy(stderr).trim().to_string();
                            Err(SnapshotError::GitCommandFailed {
                                cmd: "git symbolic-ref -q HEAD".to_string(),
                                stderr: if stderr_str.is_empty() {
                                    format!("Exit code {other}")
                                } else {
                                    stderr_str
                                },
                            })
                        }
                    };
                }
                Err(kind) => {
                    return Err(SnapshotError::Io(std::io::Error::new(
                        *kind,
                        "Simulated spawn failure",
                    )));
                }
            }
        }
    }

    let output = run_hermetic_git(
        repo_root,
        &["symbolic-ref", "-q", "HEAD"],
        env.config_path(),
        env.temp_dir(),
    )?;

    match output.status.code() {
        Some(0) => {
            let stdout_str =
                String::from_utf8(output.stdout).map_err(|_| SnapshotError::GitCommandFailed {
                    cmd: "git symbolic-ref -q HEAD".to_string(),
                    stderr: "Non-UTF-8 output from symbolic-ref".to_string(),
                })?;
            let sref = stdout_str.trim().to_string();
            if sref.starts_with("refs/") {
                Ok((HeadMode::Symbolic, Some(sref)))
            } else {
                Err(SnapshotError::GitCommandFailed {
                    cmd: "git symbolic-ref -q HEAD".to_string(),
                    stderr: format!("Invalid symbolic-ref output: '{sref}'"),
                })
            }
        }
        Some(1) => Ok((HeadMode::Detached, None)),
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(SnapshotError::GitCommandFailed {
                cmd: "git symbolic-ref -q HEAD".to_string(),
                stderr,
            })
        }
    }
}

/// Pure-Rust RFC 3174 SHA-1 digest computation for Git index verification.
pub fn sha1_digest(data: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xEFCDAB89;
    let mut h2: u32 = 0x98BADCFE;
    let mut h3: u32 = 0x10325476;
    let mut h4: u32 = 0xC3D2E1F0;

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

/// SHA-256 digest computation for Git index verification when repository uses sha256 object format.
pub fn sha256_digest(data: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

/// Structurally validates Git index bytes (§289.3 #2, §293.4 #3):
/// - Header magic (DIRC), versions 2..=4, entry count
/// - Path length field check against parsed path (v2/v3)
/// - Mode check rejecting 040000 sparse directory entries
/// - Extended flags check rejecting CE_SKIP_WORKTREE (0x4000)
/// - v4 varint and prefix compression bounds checking
/// - Extension block framing: rejects 'sdir' and 'link'; distinguishes mandatory (first byte 'A'..'Z') vs optional ('a'..'z')
/// - Trailing checksum matches payload bytes
pub fn parse_and_validate_git_index(
    index_bytes: &[u8],
    hash_len: usize,
) -> Result<(), SnapshotError> {
    if hash_len != 20 && hash_len != 32 {
        return Err(SnapshotError::UnsupportedIndexLayout(format!(
            "Unsupported object hash width: {hash_len} (expected 20 or 32)"
        )));
    }

    let min_len = 12 + hash_len;
    if index_bytes.len() < min_len {
        return Err(SnapshotError::UnsupportedIndexLayout(
            "Index file is too small to contain valid Git index header and checksum".to_string(),
        ));
    }

    // 1. Header (12 bytes)
    if &index_bytes[0..4] != b"DIRC" {
        return Err(SnapshotError::UnsupportedIndexLayout(
            "Index file magic does not match 'DIRC'".to_string(),
        ));
    }
    let version = u32::from_be_bytes([
        index_bytes[4],
        index_bytes[5],
        index_bytes[6],
        index_bytes[7],
    ]);
    if version < 2 || version > 4 {
        return Err(SnapshotError::UnsupportedIndexLayout(format!(
            "Unsupported Git index version: {version}"
        )));
    }
    let entry_count = u32::from_be_bytes([
        index_bytes[8],
        index_bytes[9],
        index_bytes[10],
        index_bytes[11],
    ]) as usize;

    let total_payload_len = index_bytes.len() - hash_len;
    let mut offset = 12;
    let mut prev_path: Vec<u8> = Vec::new();

    // 2. Entry walk
    for entry_idx in 0..entry_count {
        let fixed_min = 40 + hash_len + 2;
        if offset + fixed_min > total_payload_len {
            return Err(SnapshotError::UnsupportedIndexLayout(format!(
                "Truncated index entry header at entry {entry_idx} (offset {offset})"
            )));
        }

        // Mode is at offset + 24..offset + 28
        let mode = u32::from_be_bytes([
            index_bytes[offset + 24],
            index_bytes[offset + 25],
            index_bytes[offset + 26],
            index_bytes[offset + 27],
        ]);
        if (mode & 0o170000) == 0o040000 {
            return Err(SnapshotError::UnsupportedIndexLayout(format!(
                "Sparse index directory entry (mode 040000) detected at entry {entry_idx}"
            )));
        }

        // Flags at offset + 40 + hash_len
        let flags_offset = offset + 40 + hash_len;
        let flags = u16::from_be_bytes([index_bytes[flags_offset], index_bytes[flags_offset + 1]]);
        let flags_path_len = (flags & 0x0FFF) as usize;

        let mut entry_header_len = fixed_min;
        if version >= 3 && (flags & 0x4000) != 0 {
            if offset + entry_header_len + 2 > total_payload_len {
                return Err(SnapshotError::UnsupportedIndexLayout(format!(
                    "Truncated extended flags at entry {entry_idx}"
                )));
            }
            let ext_flags = u16::from_be_bytes([
                index_bytes[offset + entry_header_len],
                index_bytes[offset + entry_header_len + 1],
            ]);
            entry_header_len += 2;
            // CE_SKIP_WORKTREE = (1 << 14) = 0x4000
            if (ext_flags & 0x4000) != 0 {
                return Err(SnapshotError::UnsupportedIndexLayout(format!(
                    "Active sparse-checkout detected: SKIP_WORKTREE flag set on entry {entry_idx}"
                )));
            }
        }

        if version == 2 || version == 3 {
            let path_start = offset + entry_header_len;
            let path_slice = &index_bytes[path_start..total_payload_len];
            let nul_pos = path_slice.iter().position(|&b| b == 0).ok_or_else(|| {
                SnapshotError::UnsupportedIndexLayout(format!(
                    "Unterminated path string at entry {entry_idx}"
                ))
            })?;
            let path_len = nul_pos;

            // Validate path-length field in flags against parsed path (§293.4 #3)
            if flags_path_len < 0x0FFF && flags_path_len != path_len {
                return Err(SnapshotError::UnsupportedIndexLayout(format!(
                    "Path length mismatch at entry {entry_idx}: flags indicate {flags_path_len}, actual is {path_len}"
                )));
            }

            let entry_len_before_pad = entry_header_len + path_len + 1; // +1 for NUL
            let pad = (8 - (entry_len_before_pad % 8)) % 8;
            let total_entry_len = entry_len_before_pad + pad;

            if offset + total_entry_len > total_payload_len {
                return Err(SnapshotError::UnsupportedIndexLayout(format!(
                    "Padded entry exceeds index bounds at entry {entry_idx}"
                )));
            }

            offset += total_entry_len;
        } else if version == 4 {
            let mut cur = offset + entry_header_len;
            // Decode varint strip length using Git's continuation recurrence (§293.4 #3, §296.2 #3)
            if cur >= total_payload_len {
                return Err(SnapshotError::UnsupportedIndexLayout(format!(
                    "Truncated varint at entry {entry_idx}"
                )));
            }
            let mut c = index_bytes[cur];
            cur += 1;
            let mut strip_len: usize = (c & 0x7F) as usize;
            let mut iterations = 0;
            while (c & 0x80) != 0 {
                strip_len = strip_len.checked_add(1).ok_or_else(|| {
                    SnapshotError::UnsupportedIndexLayout("Varint overflow in v4 index".into())
                })?;
                if cur >= total_payload_len {
                    return Err(SnapshotError::UnsupportedIndexLayout(format!(
                        "Truncated multi-byte varint at entry {entry_idx}"
                    )));
                }
                c = index_bytes[cur];
                cur += 1;
                strip_len = strip_len
                    .checked_shl(7)
                    .and_then(|v| v.checked_add((c & 0x7F) as usize))
                    .ok_or_else(|| {
                        SnapshotError::UnsupportedIndexLayout("Varint overflow in v4 index".into())
                    })?;
                iterations += 1;
                if iterations > 4 {
                    return Err(SnapshotError::UnsupportedIndexLayout(
                        "Varint length exceeds limit in v4 index".into(),
                    ));
                }
            }
            if strip_len > prev_path.len() {
                return Err(SnapshotError::UnsupportedIndexLayout(format!(
                    "v4 prefix strip length {strip_len} exceeds previous path length {}",
                    prev_path.len()
                )));
            }

            // Path component up to NUL
            let nul_pos = index_bytes[cur..total_payload_len]
                .iter()
                .position(|&b| b == 0)
                .ok_or_else(|| {
                    SnapshotError::UnsupportedIndexLayout(format!(
                        "Unterminated v4 path at entry {entry_idx}"
                    ))
                })?;
            let suffix = &index_bytes[cur..cur + nul_pos];
            cur += nul_pos + 1; // Skip path and NUL

            let prefix_len = prev_path.len() - strip_len;
            let mut reconstructed = Vec::with_capacity(prefix_len + suffix.len());
            reconstructed.extend_from_slice(&prev_path[..prefix_len]);
            reconstructed.extend_from_slice(suffix);

            // Validate reconstructed path length against flags_path_len (§296.2 #3)
            if flags_path_len < 0x0FFF && flags_path_len != reconstructed.len() {
                return Err(SnapshotError::UnsupportedIndexLayout(format!(
                    "v4 path length mismatch at entry {entry_idx}: flags indicate {flags_path_len}, actual is {}",
                    reconstructed.len()
                )));
            }

            if entry_idx > 0 && reconstructed <= prev_path {
                return Err(SnapshotError::UnsupportedIndexLayout(format!(
                    "v4 index entry path order violation at entry {entry_idx}"
                )));
            }

            prev_path = reconstructed;
            offset = cur;
        }
    }

    // 3. Extension blocks framing (§293.4 #3, §296.2 #2)
    while offset < total_payload_len {
        if offset + 8 > total_payload_len {
            return Err(SnapshotError::UnsupportedIndexLayout(format!(
                "Malformed extension framing at offset {offset}: less than 8 bytes remaining"
            )));
        }

        let sig = &index_bytes[offset..offset + 4];
        let ext_len = u32::from_be_bytes([
            index_bytes[offset + 4],
            index_bytes[offset + 5],
            index_bytes[offset + 6],
            index_bytes[offset + 7],
        ]) as usize;

        if offset + 8 + ext_len > total_payload_len {
            return Err(SnapshotError::UnsupportedIndexLayout(format!(
                "Extension '{:?}' payload length {ext_len} exceeds index boundary",
                String::from_utf8_lossy(sig)
            )));
        }

        // Explicitly rejected layouts
        if sig == b"sdir" {
            return Err(SnapshotError::UnsupportedIndexLayout(
                "Sparse-index detected: 'sdir' extension present in active index file".to_string(),
            ));
        }

        if sig == b"link" {
            return Err(SnapshotError::UnsupportedIndexLayout(
                "Split-index detected: 'link' extension present in active index file".to_string(),
            ));
        }

        // Git extension convention (official Git index-format docs):
        // Uppercase first byte ('A'..='Z') denotes an OPTIONAL extension (safely ignored if unknown).
        // Non-uppercase first byte (e.g. lowercase) denotes a MANDATORY extension (fails closed if unknown).
        let is_optional = sig[0].is_ascii_uppercase();
        if !is_optional {
            return Err(SnapshotError::UnsupportedIndexLayout(format!(
                "Unknown mandatory index extension rejected: '{:?}'",
                String::from_utf8_lossy(sig)
            )));
        }

        offset += 8 + ext_len;
    }

    if offset != total_payload_len {
        return Err(SnapshotError::UnsupportedIndexLayout(format!(
            "Index byte alignment mismatch: extensions ended at {offset}, expected {total_payload_len}"
        )));
    }

    // 4. Trailing checksum verification
    let payload = &index_bytes[..total_payload_len];
    let expected_checksum = &index_bytes[total_payload_len..];

    if hash_len == 20 {
        let computed = sha1_digest(payload);
        if expected_checksum != computed {
            return Err(SnapshotError::UnsupportedIndexLayout(
                "Index file SHA-1 checksum mismatch".to_string(),
            ));
        }
    } else if hash_len == 32 {
        let computed = sha256_digest(payload);
        if expected_checksum != computed {
            return Err(SnapshotError::UnsupportedIndexLayout(
                "Index file SHA-256 checksum mismatch".to_string(),
            ));
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryIdentity {
    pub toplevel: PathBuf,
    pub git_dir: PathBuf,
    pub common_dir: PathBuf,
    pub index_path: PathBuf,
    pub object_format: String,
    pub hash_len: usize,
}

#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandleIdentity {
    pub volume_serial_number: u32,
    pub file_index: u64,
    pub file_size: u64,
    pub last_write_time: u64,
}

#[cfg(windows)]
pub fn query_handle_identity(file: &fs::File) -> Result<HandleIdentity, SnapshotError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    let success = unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, &mut info) };
    if success == 0 {
        return Err(SnapshotError::Io(std::io::Error::last_os_error()));
    }

    let file_index = ((info.nFileIndexHigh as u64) << 32) | (info.nFileIndexLow as u64);
    let file_size = ((info.nFileSizeHigh as u64) << 32) | (info.nFileSizeLow as u64);
    let last_write_time = ((info.ftLastWriteTime.dwHighDateTime as u64) << 32)
        | (info.ftLastWriteTime.dwLowDateTime as u64);

    Ok(HandleIdentity {
        volume_serial_number: info.dwVolumeSerialNumber,
        file_index,
        file_size,
        last_write_time,
    })
}

#[cfg(windows)]
pub fn query_path_identity(path: &Path) -> Result<(u32, u64), SnapshotError> {
    use std::os::windows::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
        .map_err(|e| io_err("open for identity query", path, e))?;
    let id = query_handle_identity(&file)?;
    Ok((id.volume_serial_number, id.file_index))
}

#[cfg(not(windows))]
pub fn query_path_identity(_path: &Path) -> Result<(u32, u64), SnapshotError> {
    Ok((0, 0))
}

#[cfg(windows)]
pub mod nt {
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::HANDLE;

    #[repr(C)]
    pub struct UNICODE_STRING {
        pub Length: u16,
        pub MaximumLength: u16,
        pub Buffer: *mut u16,
    }

    #[repr(C)]
    pub struct OBJECT_ATTRIBUTES {
        pub Length: u32,
        pub RootDirectory: HANDLE,
        pub ObjectName: *mut UNICODE_STRING,
        pub Attributes: u32,
        pub SecurityDescriptor: *mut c_void,
        pub SecurityQualityOfService: *mut c_void,
    }

    #[repr(C)]
    pub struct IO_STATUS_BLOCK {
        pub Status: i32,
        pub Information: usize,
    }

    pub const OBJ_CASE_INSENSITIVE: u32 = 0x00000040;
    pub const FILE_OPEN_REPARSE_POINT: u32 = 0x00200000;
    pub const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x00000020;
    pub const FILE_DIRECTORY_FILE: u32 = 0x00000001;
    pub const FILE_NON_DIRECTORY_FILE: u32 = 0x00000040;
    pub const FILE_LIST_DIRECTORY: u32 = 0x0001;
    pub const FILE_READ_DATA: u32 = 0x0001;
    pub const FILE_READ_ATTRIBUTES: u32 = 0x0080;
    pub const SYNCHRONIZE: u32 = 0x00100000;
    pub const DELETE: u32 = 0x00010000;
    pub const FILE_SHARE_READ: u32 = 0x00000001;
    pub const FILE_SHARE_WRITE: u32 = 0x00000002;
    pub const FILE_SHARE_DELETE: u32 = 0x00000004;

    pub const FileRenameInformation: u32 = 10;
    pub const FileDispositionInformation: u32 = 13;
    pub const FileDirectoryInformation: u32 = 1;
    pub const STATUS_NO_MORE_FILES: i32 = -2147483642; // 0x80000006
    pub const STATUS_NO_SUCH_FILE: i32 = -1073741809; // 0xC000000F

    #[repr(C)]
    pub struct FILE_DIRECTORY_INFORMATION {
        pub NextEntryOffset: u32,
        pub FileIndex: u32,
        pub CreationTime: i64,
        pub LastAccessTime: i64,
        pub LastWriteTime: i64,
        pub ChangeTime: i64,
        pub EndOfFile: i64,
        pub AllocationSize: i64,
        pub FileAttributes: u32,
        pub FileNameLength: u32,
        pub FileName: [u16; 1],
    }

    #[repr(C)]
    pub struct FILE_DISPOSITION_INFORMATION {
        pub DeleteFile: u8,
    }

    #[link(name = "ntdll")]
    extern "system" {
        pub fn NtOpenFile(
            FileHandle: *mut HANDLE,
            DesiredAccess: u32,
            ObjectAttributes: *mut OBJECT_ATTRIBUTES,
            IoStatusBlock: *mut IO_STATUS_BLOCK,
            ShareAccess: u32,
            OpenOptions: u32,
        ) -> i32;

        pub fn NtSetInformationFile(
            FileHandle: HANDLE,
            IoStatusBlock: *mut IO_STATUS_BLOCK,
            FileInformation: *mut c_void,
            Length: u32,
            FileInformationClass: u32,
        ) -> i32;

        pub fn NtQueryDirectoryFile(
            FileHandle: HANDLE,
            Event: HANDLE,
            ApcRoutine: *mut c_void,
            ApcContext: *mut c_void,
            IoStatusBlock: *mut IO_STATUS_BLOCK,
            FileInformation: *mut c_void,
            Length: u32,
            FileInformationClass: u32,
            ReturnSingleEntry: u8,
            FileName: *mut UNICODE_STRING,
            RestartScan: u8,
        ) -> i32;
    }

    pub unsafe fn nt_open_file_relative_with_access(
        root_dir: HANDLE,
        component_utf16: &[u16],
        is_directory: bool,
        desired_access: u32,
        share_access: u32,
    ) -> Result<HANDLE, i32> {
        let mut us = UNICODE_STRING {
            Length: (component_utf16.len() * 2) as u16,
            MaximumLength: (component_utf16.len() * 2) as u16,
            Buffer: component_utf16.as_ptr() as *mut u16,
        };

        let mut oa = OBJECT_ATTRIBUTES {
            Length: std::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: root_dir,
            ObjectName: &mut us,
            Attributes: OBJ_CASE_INSENSITIVE,
            SecurityDescriptor: std::ptr::null_mut(),
            SecurityQualityOfService: std::ptr::null_mut(),
        };

        let mut iosb = IO_STATUS_BLOCK {
            Status: 0,
            Information: 0,
        };

        let mut open_options = FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT;
        if is_directory {
            open_options |= FILE_DIRECTORY_FILE;
        } else {
            open_options |= FILE_NON_DIRECTORY_FILE;
        }

        let mut handle: HANDLE = std::ptr::null_mut();

        let status = NtOpenFile(
            &mut handle,
            desired_access,
            &mut oa,
            &mut iosb,
            share_access,
            open_options,
        );

        if status == 0 {
            Ok(handle)
        } else {
            Err(status)
        }
    }

    pub unsafe fn nt_open_file_relative(
        root_dir: HANDLE,
        component_utf16: &[u16],
        is_directory: bool,
    ) -> Result<HANDLE, i32> {
        let (desired_access, share_access) = if is_directory {
            (
                FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
                FILE_SHARE_READ | FILE_SHARE_WRITE, // Denies FILE_SHARE_DELETE (§300.3 #2, §301.3 #2)
            )
        } else {
            (
                FILE_READ_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
                FILE_SHARE_READ, // Denies FILE_SHARE_WRITE and FILE_SHARE_DELETE (§300.3 #2, §301.3 #2)
            )
        };
        nt_open_file_relative_with_access(
            root_dir,
            component_utf16,
            is_directory,
            desired_access,
            share_access,
        )
    }

    /// Enumerates directory entries directly through a verified directory handle (§304.3 #2, §307.2 #1).
    pub unsafe fn nt_query_directory_entries(
        dir_handle: HANDLE,
    ) -> Result<Vec<(String, bool, u32)>, i32> {
        let mut results = Vec::new();
        let mut buffer = vec![0u8; 16384];
        let mut restart = 1u8;

        loop {
            let mut iosb = IO_STATUS_BLOCK {
                Status: 0,
                Information: 0,
            };

            let status = NtQueryDirectoryFile(
                dir_handle,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut iosb,
                buffer.as_mut_ptr() as *mut c_void,
                buffer.len() as u32,
                FileDirectoryInformation,
                0, // batch scan
                std::ptr::null_mut(),
                restart,
            );

            restart = 0;

            if status == STATUS_NO_MORE_FILES || status == STATUS_NO_SUCH_FILE {
                break;
            }
            if status != 0 {
                return Err(status);
            }

            let mut offset = 0usize;
            loop {
                let entry_ptr = buffer.as_ptr().add(offset) as *const FILE_DIRECTORY_INFORMATION;
                let entry = &*entry_ptr;
                let name_len = (entry.FileNameLength / 2) as usize;
                let name_slice = std::slice::from_raw_parts(entry.FileName.as_ptr(), name_len);
                let name = String::from_utf16_lossy(name_slice);

                if name != "." && name != ".." {
                    let is_dir = (entry.FileAttributes
                        & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY)
                        != 0;
                    results.push((name, is_dir, entry.FileAttributes));
                }

                if entry.NextEntryOffset == 0 {
                    break;
                }
                offset += entry.NextEntryOffset as usize;
                if offset >= buffer.len() {
                    break;
                }
            }
        }
        Ok(results)
    }
}

#[cfg(windows)]
pub fn query_final_path_by_handle(
    handle: std::os::windows::io::RawHandle,
) -> Result<PathBuf, SnapshotError> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFinalPathNameByHandleW, FILE_NAME_NORMALIZED, VOLUME_NAME_DOS,
    };

    let mut buf = [0u16; 1024];
    let len = unsafe {
        GetFinalPathNameByHandleW(
            handle as _,
            buf.as_mut_ptr(),
            buf.len() as u32,
            FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
        )
    };
    if len == 0 || len as usize >= buf.len() {
        return Err(SnapshotError::Io(std::io::Error::last_os_error()));
    }
    let os_str = std::ffi::OsString::from_wide(&buf[..len as usize]);
    let s = os_str.to_string_lossy();
    let clean = if s.starts_with(r"\\?\") {
        PathBuf::from(&s[4..])
    } else {
        PathBuf::from(s.into_owned())
    };
    Ok(clean)
}

/// Normalizes away the Windows verbatim/extended path prefix `\\?\` if present.
pub fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

#[cfg(windows)]
#[derive(Debug)]
pub struct SafeHandle(pub windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Drop for SafeHandle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE as _
        {
            unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
        }
    }
}

pub struct RetainedHandleChain {
    pub file: fs::File,
    #[cfg(windows)]
    pub _root_file: fs::File,
    #[cfg(windows)]
    pub _intermediate_handles: Vec<SafeHandle>,
}

impl std::ops::Deref for RetainedHandleChain {
    type Target = fs::File;
    fn deref(&self) -> &Self::Target {
        &self.file
    }
}

impl std::ops::DerefMut for RetainedHandleChain {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.file
    }
}

#[cfg(windows)]
pub unsafe fn nt_rename_file_relative(
    file_handle: windows_sys::Win32::Foundation::HANDLE,
    root_directory: windows_sys::Win32::Foundation::HANDLE,
    new_name: &str,
) -> Result<(), SnapshotError> {
    use std::os::windows::ffi::OsStrExt;
    let wide_name: Vec<u16> = std::ffi::OsStr::new(new_name).encode_wide().collect();
    let wide_bytes_len = wide_name.len() * 2;
    let mut buf = vec![0u8; 20 + wide_bytes_len];
    buf[0] = 0; // ReplaceIfExists = FALSE
    let root_bytes = (root_directory as usize).to_ne_bytes();
    buf[8..16].copy_from_slice(&root_bytes);
    let len_bytes = (wide_bytes_len as u32).to_ne_bytes();
    buf[16..20].copy_from_slice(&len_bytes);
    unsafe {
        std::ptr::copy_nonoverlapping(
            wide_name.as_ptr() as *const u8,
            buf.as_mut_ptr().add(20),
            wide_bytes_len,
        );
    }

    let mut iosb = nt::IO_STATUS_BLOCK {
        Status: 0,
        Information: 0,
    };
    let status = nt::NtSetInformationFile(
        file_handle,
        &mut iosb,
        buf.as_mut_ptr() as *mut _,
        buf.len() as u32,
        nt::FileRenameInformation,
    );

    if status == 0 {
        Ok(())
    } else if status == -1073741771 {
        // 0xC0000035 = STATUS_OBJECT_NAME_COLLISION
        Err(SnapshotError::Collision(PathBuf::from(new_name)))
    } else {
        Err(SnapshotError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("NtSetInformationFile(FileRenameInformation) failed: 0x{status:08X}"),
        )))
    }
}

#[cfg(windows)]
pub unsafe fn nt_delete_file_by_handle(
    file_handle: windows_sys::Win32::Foundation::HANDLE,
) -> Result<(), SnapshotError> {
    let mut disp = nt::FILE_DISPOSITION_INFORMATION { DeleteFile: 1 };
    let mut iosb = nt::IO_STATUS_BLOCK {
        Status: 0,
        Information: 0,
    };
    let status = nt::NtSetInformationFile(
        file_handle,
        &mut iosb,
        &mut disp as *mut _ as *mut _,
        std::mem::size_of::<nt::FILE_DISPOSITION_INFORMATION>() as u32,
        nt::FileDispositionInformation,
    );
    if status == 0 {
        Ok(())
    } else {
        Err(SnapshotError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("NtSetInformationFile(FileDispositionInformation) failed: 0x{status:08X}"),
        )))
    }
}

#[cfg(windows)]
pub unsafe fn collect_handle_relative_inventory(
    root_handle: windows_sys::Win32::Foundation::HANDLE,
    rel_prefix: &str,
    out: &mut Vec<DescendantInventoryEntry>,
) -> Result<(), SnapshotError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY,
        FILE_ATTRIBUTE_REPARSE_POINT,
    };

    let entries = nt::nt_query_directory_entries(root_handle).map_err(|status| {
        SnapshotError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("NtQueryDirectoryFile failed during inventory collection: 0x{status:08X}"),
        ))
    })?;

    for (name, is_dir, attrs) in entries {
        if (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
            return Err(SnapshotError::ReparsePointRejected(format!(
                "Reparse point found during staging inventory collection: {name}"
            )));
        }

        let rel_path = if rel_prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", rel_prefix, name)
        };

        let wide: Vec<u16> = std::ffi::OsStr::new(&name).encode_wide().collect();
        let desired_access = nt::FILE_READ_ATTRIBUTES
            | nt::SYNCHRONIZE
            | (if is_dir { nt::FILE_LIST_DIRECTORY } else { 0 });

        let child_handle = nt::nt_open_file_relative_with_access(
            root_handle,
            &wide,
            is_dir,
            desired_access,
            nt::FILE_SHARE_READ | nt::FILE_SHARE_WRITE, // Denies FILE_SHARE_DELETE
        )
        .map_err(|status| {
            SnapshotError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to open staged child '{rel_path}' for inventory: 0x{status:08X}"),
            ))
        })?;

        let _guard = SafeHandle(child_handle);

        let mut info: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
        if GetFileInformationByHandle(child_handle as _, &mut info) == 0 {
            return Err(SnapshotError::Io(std::io::Error::last_os_error()));
        }

        if (info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
            return Err(SnapshotError::ReparsePointRejected(format!(
                "Reparse point confirmed for staged child: {rel_path}"
            )));
        }

        let is_real_dir = is_dir && ((info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0);
        let id = (
            info.dwVolumeSerialNumber,
            ((info.nFileIndexHigh as u64) << 32) | (info.nFileIndexLow as u64),
        );

        out.push(DescendantInventoryEntry {
            rel_path: rel_path.clone(),
            is_dir: is_real_dir,
            id,
        });

        if is_real_dir {
            collect_handle_relative_inventory(child_handle, &rel_path, out)?;
        }
    }
    Ok(())
}

#[cfg(windows)]
pub unsafe fn nt_read_snapshot_json_relative(
    dir_handle: windows_sys::Win32::Foundation::HANDLE,
) -> Result<Vec<u8>, SnapshotError> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::FromRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT,
    };

    let snap_wide: Vec<u16> = std::ffi::OsStr::new("snapshot.json")
        .encode_wide()
        .collect();
    let file_handle = nt::nt_open_file_relative_with_access(
        dir_handle,
        &snap_wide,
        false,
        nt::FILE_READ_DATA | nt::FILE_READ_ATTRIBUTES | nt::SYNCHRONIZE,
        nt::FILE_SHARE_READ | nt::FILE_SHARE_WRITE, // Denies FILE_SHARE_DELETE
    )
    .map_err(|status| {
        SnapshotError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to open snapshot.json relative to target: 0x{status:08X}"),
        ))
    })?;

    let mut file_info: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
    if GetFileInformationByHandle(file_handle as _, &mut file_info) == 0 {
        let _ = SafeHandle(file_handle);
        return Err(SnapshotError::Io(std::io::Error::last_os_error()));
    }

    if (file_info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
        let _ = SafeHandle(file_handle);
        return Err(SnapshotError::ReparsePointRejected(
            "snapshot.json is a reparse point".to_string(),
        ));
    }

    let mut f = std::fs::File::from_raw_handle(file_handle as _);
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).map_err(|e| {
        SnapshotError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to read snapshot.json via handle: {e}"),
        ))
    })?;
    Ok(bytes)
}

#[cfg(windows)]
pub unsafe fn verify_and_delete_handle_tree(
    dir_handle: windows_sys::Win32::Foundation::HANDLE,
    rel_prefix: &str,
    lease: &SnapshotOwnershipLease,
    deleted_count: &mut usize,
) -> Result<(), SnapshotError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY,
        FILE_ATTRIBUTE_REPARSE_POINT,
    };

    let entries = nt::nt_query_directory_entries(dir_handle).map_err(|status| {
        SnapshotError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("NtQueryDirectoryFile failed during cleanup: 0x{status:08X}"),
        ))
    })?;

    // Phase 1: Verification pass of all immediate children before deleting any
    let mut child_handles = Vec::with_capacity(entries.len());

    for (name, is_dir, attrs) in entries {
        if (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
            return Err(SnapshotError::ReparsePointRejected(format!(
                "Reparse point detected in snapshot tree: '{name}'"
            )));
        }

        let child_rel = if rel_prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", rel_prefix, name)
        };

        // Check lease inventory: child MUST exist in ephemeral lease inventory
        let inv_match = lease.inventory.iter().find(|inv| inv.rel_path == child_rel);
        let expected_inv = match inv_match {
            Some(inv) => inv,
            None => {
                return Err(SnapshotError::IllegalPath(format!(
                    "Unknown or unexpected child not in lease inventory: '{child_rel}'"
                )));
            }
        };

        let wide: Vec<u16> = std::ffi::OsStr::new(&name).encode_wide().collect();
        let desired_access = nt::DELETE
            | nt::FILE_READ_ATTRIBUTES
            | nt::SYNCHRONIZE
            | (if is_dir { nt::FILE_LIST_DIRECTORY } else { 0 });

        let child_handle = nt::nt_open_file_relative_with_access(
            dir_handle,
            &wide,
            is_dir,
            desired_access,
            nt::FILE_SHARE_READ | nt::FILE_SHARE_WRITE, // Denies FILE_SHARE_DELETE (§304.3 #1)
        )
        .map_err(|status| {
            SnapshotError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed opening child '{child_rel}' for cleanup: 0x{status:08X}"),
            ))
        })?;

        let safe_child = SafeHandle(child_handle);

        let mut child_info: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
        if GetFileInformationByHandle(child_handle as _, &mut child_info) == 0 {
            return Err(SnapshotError::Io(std::io::Error::last_os_error()));
        }

        if (child_info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
            return Err(SnapshotError::ReparsePointRejected(format!(
                "Reparse point confirmed for child '{child_rel}'"
            )));
        }

        let actual_id = (
            child_info.dwVolumeSerialNumber,
            ((child_info.nFileIndexHigh as u64) << 32) | (child_info.nFileIndexLow as u64),
        );

        if actual_id != expected_inv.id {
            return Err(SnapshotError::IllegalPath(format!(
                "Child identity mismatch / replacement detected for '{child_rel}': expected {:?}, found {:?}",
                expected_inv.id, actual_id
            )));
        }

        let is_real_dir = is_dir && ((child_info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0);
        if is_real_dir != expected_inv.is_dir {
            return Err(SnapshotError::IllegalPath(format!(
                "Child directory type mismatch for '{child_rel}'"
            )));
        }

        child_handles.push((child_rel, is_real_dir, safe_child));
    }

    // Phase 2: Post-order recursive cleanup: subdirectories first, then children
    for (child_rel, is_real_dir, safe_child) in child_handles {
        if is_real_dir {
            verify_and_delete_handle_tree(safe_child.0, &child_rel, lease, deleted_count)?;
        }
        nt_delete_file_by_handle(safe_child.0)?;
        *deleted_count += 1;
        drop(safe_child);
    }

    Ok(())
}

/// Validates directory ancestors between root_anchor and target_path using handle-based inspection (§289.3 #3, §293.4 #1, §296.3 #1).
pub fn validate_ancestor_directories(
    target_path: &Path,
    root_anchor: &Path,
) -> Result<(), SnapshotError> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY,
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        };

        let canon_root = fs::canonicalize(root_anchor)
            .map_err(|e| io_err("canonicalize root_anchor", root_anchor, e))?;
        let canon_root_clean = strip_verbatim_prefix(&canon_root);

        let norm_target = target_path.to_string_lossy().replace('\\', "/");
        let norm_root = root_anchor.to_string_lossy().replace('\\', "/");
        let norm_root_trimmed = norm_root.trim_end_matches('/');

        let rel_str: String = if norm_target.len() > norm_root_trimmed.len()
            && norm_target[..norm_root_trimmed.len()].eq_ignore_ascii_case(norm_root_trimmed)
            && (norm_target.as_bytes()[norm_root_trimmed.len()] == b'/')
        {
            norm_target[norm_root_trimmed.len() + 1..].to_string()
        } else {
            let canon_target = fs::canonicalize(target_path)
                .map_err(|e| io_err("canonicalize target_path", target_path, e))?;
            let canon_target_clean = strip_verbatim_prefix(&canon_target);
            if canon_target_clean.starts_with(&canon_root_clean) {
                let p = canon_target_clean.strip_prefix(&canon_root_clean).unwrap();
                p.to_string_lossy()
                    .replace('\\', "/")
                    .trim_start_matches('/')
                    .to_string()
            } else {
                return Err(SnapshotError::IllegalPath(format!(
                    "Path '{target_path:?}' is outside anchor '{root_anchor:?}'"
                )));
            }
        };

        let parts: Vec<&str> = rel_str.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() <= 1 {
            return Ok(());
        }

        let root_file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .open(root_anchor)
            .map_err(|e| io_err("open root anchor", root_anchor, e))?;

        let root_final = query_final_path_by_handle(root_file.as_raw_handle())?;
        let root_final_clean = strip_verbatim_prefix(&root_final);
        if !root_final_clean
            .to_string_lossy()
            .eq_ignore_ascii_case(&canon_root_clean.to_string_lossy())
        {
            return Err(SnapshotError::IllegalPath(format!(
                "Root anchor final path '{root_final_clean:?}' does not match canonical root '{canon_root_clean:?}'"
            )));
        }

        struct SafeHandle(HANDLE);
        impl Drop for SafeHandle {
            fn drop(&mut self) {
                if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE as _ {
                    unsafe { CloseHandle(self.0) };
                }
            }
        }

        let mut current_handle = root_file.as_raw_handle() as HANDLE;
        let mut _handles: Vec<SafeHandle> = Vec::new();

        for dir_part in &parts[..parts.len() - 1] {
            let wide: Vec<u16> = std::ffi::OsStr::new(dir_part).encode_wide().collect();
            let child_res = unsafe { nt::nt_open_file_relative(current_handle, &wide, true) };
            let child_handle = match child_res {
                Ok(h) => h,
                Err(status) => {
                    let curr_path = root_anchor.join(dir_part);
                    if let Ok(sym_meta) = fs::symlink_metadata(&curr_path) {
                        use std::os::windows::fs::MetadataExt;
                        if (sym_meta.file_attributes() & 0x400) != 0 {
                            return Err(SnapshotError::ReparsePointRejected(format!(
                                "Directory ancestor is a reparse point: {curr_path:?}"
                            )));
                        }
                    }
                    if status == -1073741772 || status == -1073741766 {
                        return Ok(());
                    }
                    return Err(SnapshotError::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("NtOpenFile error 0x{status:08X} on '{dir_part}'"),
                    )));
                }
            };

            let safe_child = SafeHandle(child_handle);

            let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
            if unsafe { GetFileInformationByHandle(child_handle as _, &mut info) } == 0 {
                return Err(SnapshotError::Io(std::io::Error::last_os_error()));
            }
            if (info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
                return Err(SnapshotError::ReparsePointRejected(format!(
                    "Directory ancestor is a reparse point: {dir_part}"
                )));
            }
            if (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) == 0 {
                return Err(SnapshotError::IllegalPath(format!(
                    "Directory ancestor '{dir_part}' is not a directory"
                )));
            }

            let child_final = query_final_path_by_handle(child_handle as _)?;
            let child_final_clean = strip_verbatim_prefix(&child_final);
            if !child_final_clean.starts_with(&canon_root_clean) {
                return Err(SnapshotError::ReparsePointRejected(format!(
                    "Directory ancestor '{dir_part}' escaped root anchor"
                )));
            }

            current_handle = child_handle;
            _handles.push(safe_child);
        }

        Ok(())
    }
    #[cfg(not(windows))]
    {
        let norm_target = target_path.to_string_lossy().replace('\\', "/");
        let norm_root = root_anchor.to_string_lossy().replace('\\', "/");
        let norm_root_trimmed = norm_root.trim_end_matches('/');

        let rel_str: String = if norm_target.eq_ignore_ascii_case(norm_root_trimmed) {
            String::new()
        } else if norm_target.len() > norm_root_trimmed.len()
            && norm_target[..norm_root_trimmed.len()].eq_ignore_ascii_case(norm_root_trimmed)
            && (norm_target.as_bytes()[norm_root_trimmed.len()] == b'/')
        {
            norm_target[norm_root_trimmed.len() + 1..].to_string()
        } else {
            let canon_root = fs::canonicalize(root_anchor)
                .map_err(|e| io_err("canonicalize root_anchor", root_anchor, e))?
                .to_string_lossy()
                .replace('\\', "/");
            let canon_root_trimmed = canon_root.trim_end_matches('/').to_string();
            let canon_target = fs::canonicalize(target_path)
                .map_err(|e| io_err("canonicalize target_path", target_path, e))?
                .to_string_lossy()
                .replace('\\', "/");
            if canon_target.len() > canon_root_trimmed.len()
                && canon_target[..canon_root_trimmed.len()]
                    .eq_ignore_ascii_case(&canon_root_trimmed)
                && (canon_target.as_bytes()[canon_root_trimmed.len()] == b'/')
            {
                canon_target[canon_root_trimmed.len() + 1..].to_string()
            } else {
                return Err(SnapshotError::IllegalPath(format!(
                    "Path '{target_path:?}' is outside anchor '{root_anchor:?}'"
                )));
            }
        };

        let parts: Vec<&str> = rel_str.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() > 1 {
            let mut curr = root_anchor.to_path_buf();
            for dir_part in &parts[..parts.len() - 1] {
                curr.push(dir_part);
                if !curr.exists() {
                    return Ok(());
                }
                let meta = fs::symlink_metadata(&curr)
                    .map_err(|e| io_err("query directory ancestor metadata", &curr, e))?;
                if meta.file_type().is_symlink() {
                    return Err(SnapshotError::ReparsePointRejected(format!(
                        "Directory ancestor is a symbolic link: {:?}",
                        curr
                    )));
                }
            }
        }
        Ok(())
    }
}

/// Safely opens a file through an unbroken handle-relative chain without following reparse points (§296.2 #1, §296.3 #1, §300.3 #1, #2, §301.3 #2).
#[cfg(windows)]
pub fn open_file_no_follow_handle_chain(
    full_path: &Path,
    root_anchor: &Path,
) -> Result<RetainedHandleChain, SnapshotError> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    };

    if INJECT_REPARSE_POINT_REJECTION.load(Ordering::SeqCst) {
        return Err(SnapshotError::ReparsePointRejected(format!(
            "Injected reparse point rejection for '{full_path:?}'"
        )));
    }

    let canon_root = fs::canonicalize(root_anchor)
        .map_err(|e| io_err("canonicalize root_anchor", root_anchor, e))?;
    let canon_root_clean = strip_verbatim_prefix(&canon_root);

    // Open root anchor denying FILE_SHARE_DELETE (§300.3 #2, §301.3 #2)
    let root_file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .share_mode(nt::FILE_SHARE_READ | nt::FILE_SHARE_WRITE)
        .open(root_anchor)
        .map_err(|e| io_err("open root_anchor handle", root_anchor, e))?;

    let root_final = query_final_path_by_handle(root_file.as_raw_handle())?;
    let root_final_clean = strip_verbatim_prefix(&root_final);

    if !root_final_clean
        .to_string_lossy()
        .eq_ignore_ascii_case(&canon_root_clean.to_string_lossy())
    {
        return Err(SnapshotError::IllegalPath(format!(
            "Root anchor final path '{root_final_clean:?}' does not match canonical root '{canon_root_clean:?}'"
        )));
    }

    let mut root_info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { GetFileInformationByHandle(root_file.as_raw_handle() as _, &mut root_info) } == 0 {
        return Err(SnapshotError::Io(std::io::Error::last_os_error()));
    }
    if (root_info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
        return Err(SnapshotError::ReparsePointRejected(format!(
            "Root anchor is a reparse point: {root_anchor:?}"
        )));
    }

    // Component-aware containment check (§300.3 #3)
    if !is_subpath_component_aware(root_anchor, full_path) {
        return Err(SnapshotError::IllegalPath(format!(
            "Path '{full_path:?}' is outside anchor '{root_anchor:?}' (prefix-sibling or non-contained)"
        )));
    }

    // Extract relative components directly from raw path to prevent canonicalize from following junctions
    let norm_target = full_path.to_string_lossy().replace('\\', "/");
    let norm_root = root_anchor.to_string_lossy().replace('\\', "/");
    let norm_root_trimmed = norm_root.trim_end_matches('/');

    let rel_str: String = if norm_target.len() > norm_root_trimmed.len()
        && norm_target[..norm_root_trimmed.len()].eq_ignore_ascii_case(norm_root_trimmed)
        && (norm_target.as_bytes()[norm_root_trimmed.len()] == b'/')
    {
        norm_target[norm_root_trimmed.len() + 1..].to_string()
    } else {
        return Err(SnapshotError::IllegalPath(format!(
            "Path '{full_path:?}' is outside anchor '{root_anchor:?}'"
        )));
    };

    let parts: Vec<String> = rel_str
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    if parts.iter().any(|p| p == "..") {
        return Err(SnapshotError::IllegalPath(format!(
            "Parent traversal not permitted in target path: {full_path:?}"
        )));
    }

    if parts.is_empty() {
        return Err(SnapshotError::IllegalPath(
            "Empty target relative path".into(),
        ));
    }

    let mut current_handle = root_file.as_raw_handle() as HANDLE;
    let mut intermediate_handles: Vec<SafeHandle> = Vec::new();

    for part in &parts[..parts.len() - 1] {
        let wide: Vec<u16> = std::ffi::OsStr::new(part).encode_wide().collect();
        let child_handle = unsafe { nt::nt_open_file_relative(current_handle, &wide, true) }
            .map_err(|status| {
                if status == -1073741772 || status == -1073741766 {
                    std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("Directory '{part}' not found"),
                    )
                } else {
                    std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("NtOpenFile relative error 0x{status:08X} for '{part}'"),
                    )
                }
            })?;

        let safe_child = SafeHandle(child_handle);

        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(child_handle as _, &mut info) } == 0 {
            return Err(SnapshotError::Io(std::io::Error::last_os_error()));
        }
        if (info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
            return Err(SnapshotError::ReparsePointRejected(format!(
                "Directory ancestor '{part}' is a reparse point"
            )));
        }
        if (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) == 0 {
            return Err(SnapshotError::IllegalPath(format!(
                "Ancestor '{part}' is not a directory"
            )));
        }

        let child_final = query_final_path_by_handle(child_handle as _)?;
        let child_final_clean = strip_verbatim_prefix(&child_final);
        if !is_subpath_component_aware(&canon_root_clean, &child_final_clean) {
            return Err(SnapshotError::ReparsePointRejected(format!(
                "Ancestor '{part}' final path '{child_final_clean:?}' escaped root anchor"
            )));
        }

        current_handle = child_handle;
        intermediate_handles.push(safe_child);
    }

    let leaf_part = parts.last().unwrap();
    let leaf_wide: Vec<u16> = std::ffi::OsStr::new(leaf_part).encode_wide().collect();
    let leaf_handle = unsafe { nt::nt_open_file_relative(current_handle, &leaf_wide, false) }
        .map_err(|status| {
            if status == -1073741772 || status == -1073741766 {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Leaf file '{leaf_part}' not found"),
                )
            } else if status == -1073741805 {
                std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("Sharing violation opening leaf file '{leaf_part}'"),
                )
            } else {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("NtOpenFile relative error 0x{status:08X} for '{leaf_part}'"),
                )
            }
        })?;

    let leaf_file = unsafe { fs::File::from_raw_handle(leaf_handle as _) };

    let mut leaf_info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { GetFileInformationByHandle(leaf_file.as_raw_handle() as _, &mut leaf_info) } == 0 {
        return Err(SnapshotError::Io(std::io::Error::last_os_error()));
    }
    if (leaf_info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
        return Err(SnapshotError::ReparsePointRejected(format!(
            "Leaf file '{leaf_part}' is a reparse point"
        )));
    }
    if (leaf_info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0 {
        return Err(SnapshotError::IllegalPath(format!(
            "Leaf '{leaf_part}' is a directory, expected file"
        )));
    }

    let leaf_final = query_final_path_by_handle(leaf_file.as_raw_handle())?;
    let leaf_final_clean = strip_verbatim_prefix(&leaf_final);
    if !is_subpath_component_aware(&canon_root_clean, &leaf_final_clean) {
        return Err(SnapshotError::ReparsePointRejected(format!(
            "Leaf file '{leaf_part}' final path '{leaf_final_clean:?}' escaped root anchor"
        )));
    }

    Ok(RetainedHandleChain {
        file: leaf_file,
        _root_file: root_file,
        _intermediate_handles: intermediate_handles,
    })
}

/// Reads a file safely without following reparse points or symbolic links (§282.2, §284.2, §285.3, §289.3 #3, §293.4 #8, §296.3 #1).
pub fn read_file_no_follow(file_path: &Path, root_anchor: &Path) -> Result<Vec<u8>, SnapshotError> {
    if INJECT_REPARSE_POINT_REJECTION.load(Ordering::SeqCst) {
        return Err(SnapshotError::ReparsePointRejected(format!(
            "Injected reparse point rejection for '{file_path:?}'"
        )));
    }

    #[cfg(windows)]
    {
        let mut file = open_file_no_follow_handle_chain(file_path, root_anchor)?;
        let id_before = query_handle_identity(&file.file)?;
        check_protected_read_observer((id_before.volume_serial_number, id_before.file_index));
        if id_before.file_size > MAX_MANIFEST_FILE_BYTES {
            return Err(SnapshotError::FileTooLarge {
                path: file_path.to_string_lossy().to_string(),
                size: id_before.file_size,
                limit: MAX_MANIFEST_FILE_BYTES,
            });
        }

        let mut bytes = Vec::with_capacity(id_before.file_size as usize);
        file.read_to_end(&mut bytes)
            .map_err(|e| io_err("read_file_no_follow read_to_end", file_path, e))?;

        let id_after = query_handle_identity(&file.file)?;
        if id_before.volume_serial_number != id_after.volume_serial_number
            || id_before.file_index != id_after.file_index
            || id_before.file_size != id_after.file_size
            || id_before.last_write_time != id_after.last_write_time
            || id_after.file_size != bytes.len() as u64
        {
            return Err(SnapshotError::ConcurrentMutation(format!(
                "File handle identity or size changed during read: {:?}",
                file_path
            )));
        }

        Ok(bytes)
    }
    #[cfg(not(windows))]
    {
        validate_ancestor_directories(file_path, root_anchor)?;
        let meta = fs::symlink_metadata(file_path)?;
        if meta.file_type().is_symlink() {
            return Err(SnapshotError::ReparsePointRejected(format!(
                "File is a symbolic link: {:?}",
                file_path
            )));
        }

        let mut file = fs::File::open(file_path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;

        let post_meta = fs::symlink_metadata(file_path)?;
        if post_meta.len() != bytes.len() as u64 {
            return Err(SnapshotError::ConcurrentMutation(format!(
                "File length changed during read: {:?}",
                file_path
            )));
        }

        Ok(bytes)
    }
}

pub enum SafeFileCapture {
    Present {
        bytes: Vec<u8>,
        size: u64,
        mode: u32,
        sha256: String,
    },
    Absent,
}

/// Safely captures a workspace file with metadata bound directly to the read handle (§289.3 #4, §293.4 #1, §296.3 #1).
pub fn capture_workspace_file_safe(
    full_path: &Path,
    root_anchor: &Path,
    policy: &SnapshotPolicy,
    rel_path: &str,
) -> Result<SafeFileCapture, SnapshotError> {
    if INJECT_REPARSE_POINT_REJECTION.load(Ordering::SeqCst) {
        return Err(SnapshotError::ReparsePointRejected(format!(
            "Injected reparse point rejection for '{full_path:?}'"
        )));
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        let mut file = match open_file_no_follow_handle_chain(full_path, root_anchor) {
            Ok(f) => f,
            Err(SnapshotError::Io(ref e)) if e.kind() == std::io::ErrorKind::NotFound => {
                if let Ok(sym_meta) = fs::symlink_metadata(full_path) {
                    if (sym_meta.file_attributes() & 0x400) != 0 {
                        return Err(SnapshotError::ReparsePointRejected(format!(
                            "Dangling symlink or reparse point detected at '{rel_path}'"
                        )));
                    }
                }
                return Ok(SafeFileCapture::Absent);
            }
            Err(SnapshotError::IoPath { ref source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                if let Ok(sym_meta) = fs::symlink_metadata(full_path) {
                    if (sym_meta.file_attributes() & 0x400) != 0 {
                        return Err(SnapshotError::ReparsePointRejected(format!(
                            "Dangling symlink or reparse point detected at '{rel_path}'"
                        )));
                    }
                }
                return Ok(SafeFileCapture::Absent);
            }
            Err(SnapshotError::ReparsePointRejected(reason)) => {
                return Err(SnapshotError::ReparsePointRejected(reason));
            }
            Err(other) => return Err(other),
        };

        let meta = file
            .metadata()
            .map_err(|e| io_err("query handle metadata", full_path, e))?;

        let id_before = query_handle_identity(&file.file)?;
        check_protected_read_observer((id_before.volume_serial_number, id_before.file_index));
        if id_before.file_size > policy.max_file_bytes {
            return Err(SnapshotError::FileTooLarge {
                path: rel_path.to_string(),
                size: id_before.file_size,
                limit: policy.max_file_bytes,
            });
        }

        let mut bytes = Vec::with_capacity(id_before.file_size as usize);
        file.read_to_end(&mut bytes)
            .map_err(|e| io_err("read workspace file bytes", full_path, e))?;

        if bytes.len() as u64 != id_before.file_size {
            return Err(SnapshotError::ConcurrentMutation(format!(
                "Read byte count ({}) does not match handle file size ({}) for '{rel_path}'",
                bytes.len(),
                id_before.file_size
            )));
        }

        let id_after = query_handle_identity(&file.file)?;
        if id_before.volume_serial_number != id_after.volume_serial_number
            || id_before.file_index != id_after.file_index
            || id_before.file_size != id_after.file_size
            || id_before.last_write_time != id_after.last_write_time
        {
            return Err(SnapshotError::ConcurrentMutation(format!(
                "File handle identity or timestamp diverged during read for '{rel_path}'"
            )));
        }

        let mode = if (meta.file_attributes() & 0x01) != 0 {
            0o100444 // Read-only
        } else {
            0o100644 // Read-write
        };

        let sha256 = sha256_hex(&bytes);
        Ok(SafeFileCapture::Present {
            bytes,
            size: id_before.file_size,
            mode,
            sha256,
        })
    }
    #[cfg(not(windows))]
    {
        validate_ancestor_directories(full_path, root_anchor)?;
        let sym_meta = match fs::symlink_metadata(full_path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(SafeFileCapture::Absent)
            }
            Err(e) => return Err(io_err("symlink_metadata workspace file", full_path, e)),
        };

        if sym_meta.file_type().is_symlink() {
            return Err(SnapshotError::ReparsePointRejected(format!(
                "Symbolic link detected at '{rel_path}'"
            )));
        }

        if sym_meta.is_dir() {
            return Ok(SafeFileCapture::Absent);
        }

        if sym_meta.len() > policy.max_file_bytes {
            return Err(SnapshotError::FileTooLarge {
                path: rel_path.to_string(),
                size: sym_meta.len(),
                limit: policy.max_file_bytes,
            });
        }

        let mut file =
            fs::File::open(full_path).map_err(|e| io_err("open workspace file", full_path, e))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|e| io_err("read workspace file", full_path, e))?;

        let post_meta = fs::symlink_metadata(full_path)
            .map_err(|e| io_err("post-read metadata workspace file", full_path, e))?;
        if post_meta.len() != bytes.len() as u64 {
            return Err(SnapshotError::ConcurrentMutation(format!(
                "File size diverged during read for '{rel_path}'"
            )));
        }

        use std::os::unix::fs::MetadataExt;
        let mode = post_meta.mode();
        let sha256 = sha256_hex(&bytes);

        Ok(SafeFileCapture::Present {
            bytes,
            size: post_meta.len(),
            mode,
            sha256,
        })
    }
}

pub enum IndexPresence {
    Present(Vec<u8>),
    ValidAbsent,
}

/// Explicit no-follow Git index inspection distinguishing valid absence from errors (§294.2 #2, §296.2 #1).
pub fn inspect_index_presence(
    index_path: &Path,
    git_dir: &Path,
    head_unborn: bool,
) -> Result<IndexPresence, SnapshotError> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::AsRawHandle;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;

        let open_res = fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .share_mode(nt::FILE_SHARE_READ)
            .open(index_path);

        match open_res {
            Ok(mut file) => {
                let meta = file
                    .metadata()
                    .map_err(|e| io_err("query index handle metadata", index_path, e))?;
                use std::os::windows::fs::MetadataExt;
                if (meta.file_attributes() & 0x400) != 0 {
                    return Err(SnapshotError::ReparsePointRejected(format!(
                        "Active index is a reparse point: {index_path:?}"
                    )));
                }

                // Verify containment in git_dir using component-aware helper (fail closed! §300.3 #3, #4)
                let final_path = query_final_path_by_handle(file.as_raw_handle())?;
                let canon_git = fs::canonicalize(git_dir)
                    .map_err(|e| io_err("canonicalize git_dir for index check", git_dir, e))?;
                let final_clean = strip_verbatim_prefix(&final_path);
                let canon_clean = strip_verbatim_prefix(&canon_git);
                if !is_subpath_component_aware(&canon_clean, &final_clean) {
                    return Err(SnapshotError::UnsupportedIndexLayout(format!(
                        "Index handle final path '{final_path:?}' escaped git directory '{canon_git:?}'"
                    )));
                }

                let id_before = query_handle_identity(&file)?;
                if id_before.file_size > MAX_INDEX_FILE_BYTES {
                    return Err(SnapshotError::FileTooLarge {
                        path: index_path.to_string_lossy().to_string(),
                        size: id_before.file_size,
                        limit: MAX_INDEX_FILE_BYTES,
                    });
                }

                let mut bytes = Vec::with_capacity(id_before.file_size as usize);
                file.read_to_end(&mut bytes)
                    .map_err(|e| io_err("read index bytes", index_path, e))?;

                let id_after = query_handle_identity(&file)?;
                if id_before != id_after || id_after.file_size != bytes.len() as u64 {
                    return Err(SnapshotError::ConcurrentMutation(format!(
                        "Active index mutated during handle read: {index_path:?}"
                    )));
                }

                Ok(IndexPresence::Present(bytes))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if let Ok(sym_meta) = fs::symlink_metadata(index_path) {
                    use std::os::windows::fs::MetadataExt;
                    if (sym_meta.file_attributes() & 0x400) != 0 {
                        return Err(SnapshotError::ReparsePointRejected(format!(
                            "Dangling symlink or reparse point at active index: {index_path:?}"
                        )));
                    }
                }
                if head_unborn {
                    Ok(IndexPresence::ValidAbsent)
                } else {
                    Err(SnapshotError::UnsupportedIndexLayout(format!(
                        "Active index is absent in non-unborn repository: {index_path:?}"
                    )))
                }
            }
            Err(e) => Err(io_err("open active index handle", index_path, e)),
        }
    }
    #[cfg(not(windows))]
    {
        match fs::symlink_metadata(index_path) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(SnapshotError::ReparsePointRejected(format!(
                        "Active index is a symbolic link: {index_path:?}"
                    )));
                }
                let bytes =
                    fs::read(index_path).map_err(|e| io_err("read active index", index_path, e))?;
                Ok(IndexPresence::Present(bytes))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if head_unborn {
                    Ok(IndexPresence::ValidAbsent)
                } else {
                    Err(SnapshotError::UnsupportedIndexLayout(format!(
                        "Active index is absent in non-unborn repository: {index_path:?}"
                    )))
                }
            }
            Err(e) => Err(io_err("stat active index", index_path, e)),
        }
    }
}

/// Validates repository identity before and after capture.
pub fn query_and_verify_repository_identity(
    repo_root: &Path,
    env: &HermeticGitEnv,
) -> Result<RepositoryIdentity, SnapshotError> {
    let canonical_root =
        fs::canonicalize(repo_root).map_err(|e| io_err("canonicalize repo_root", repo_root, e))?;

    // 1. toplevel
    let toplevel_bytes = run_hermetic_git_stdout(
        repo_root,
        &["rev-parse", "--show-toplevel"],
        env.config_path(),
        env.temp_dir(),
    )?;
    let toplevel_str = String::from_utf8_lossy(&toplevel_bytes).trim().to_string();
    let canonical_toplevel = fs::canonicalize(Path::new(&toplevel_str))
        .map_err(|e| io_err("canonicalize toplevel", Path::new(&toplevel_str), e))?;

    if canonical_toplevel != canonical_root {
        return Err(SnapshotError::UnsupportedIndexLayout(format!(
            "Repository toplevel mismatch: expected {:?}, got {:?}",
            canonical_root, canonical_toplevel
        )));
    }

    // 2. absolute git-dir
    let git_dir_bytes = run_hermetic_git_stdout(
        repo_root,
        &["rev-parse", "--absolute-git-dir"],
        env.config_path(),
        env.temp_dir(),
    )?;
    let git_dir_str = String::from_utf8_lossy(&git_dir_bytes).trim().to_string();
    let canonical_git_dir = fs::canonicalize(Path::new(&git_dir_str))
        .map_err(|e| io_err("canonicalize git_dir", Path::new(&git_dir_str), e))?;

    // 3. git common-dir
    let common_dir_bytes = run_hermetic_git_stdout(
        repo_root,
        &["rev-parse", "--git-common-dir"],
        env.config_path(),
        env.temp_dir(),
    )?;
    let common_dir_str = String::from_utf8_lossy(&common_dir_bytes)
        .trim()
        .to_string();
    let common_dir_path = if Path::new(&common_dir_str).is_absolute() {
        PathBuf::from(&common_dir_str)
    } else {
        repo_root.join(&common_dir_str)
    };
    let canonical_common_dir = fs::canonicalize(&common_dir_path)
        .map_err(|e| io_err("canonicalize common_dir", &common_dir_path, e))?;

    // 4. index path: preserve raw path without pre-canonicalizing leaf
    let index_bytes = run_hermetic_git_stdout(
        repo_root,
        &["rev-parse", "--git-path", "index"],
        env.config_path(),
        env.temp_dir(),
    )?;
    let index_str = String::from_utf8_lossy(&index_bytes).trim().to_string();
    let index_path_raw = if Path::new(&index_str).is_absolute() {
        PathBuf::from(&index_str)
    } else {
        repo_root.join(&index_str)
    };

    if let Some(index_parent) = index_path_raw.parent() {
        if index_parent.exists() {
            let canon_parent = fs::canonicalize(index_parent)
                .map_err(|e| io_err("canonicalize index parent", index_parent, e))?;
            let in_git = canon_parent.starts_with(&canonical_git_dir);
            let in_common = canon_parent.starts_with(&canonical_common_dir);
            if !in_git && !in_common {
                return Err(SnapshotError::UnsupportedIndexLayout(format!(
                    "Index parent '{:?}' is outside canonical git directories",
                    canon_parent
                )));
            }
        }
    }

    // 5. Object format (hash width)
    let format_bytes = run_hermetic_git_stdout(
        repo_root,
        &["rev-parse", "--show-object-format"],
        env.config_path(),
        env.temp_dir(),
    )?;
    let object_format = String::from_utf8_lossy(&format_bytes).trim().to_string();
    let hash_len = match object_format.as_str() {
        "sha1" => 20,
        "sha256" => 32,
        other => {
            return Err(SnapshotError::UnsupportedIndexLayout(format!(
                "Unsupported repository object format: '{other}'"
            )));
        }
    };

    Ok(RepositoryIdentity {
        toplevel: canonical_toplevel,
        git_dir: canonical_git_dir,
        common_dir: canonical_common_dir,
        index_path: index_path_raw,
        object_format,
        hash_len,
    })
}

/// Non-mutating preflight: asserts absence of unsupported sparse/split layouts BEFORE any index command (§284.2, §285.3, §289.3 #2).
pub fn verify_repository_layout_preflight(
    repo_root: &Path,
    identity: &RepositoryIdentity,
    env: &HermeticGitEnv,
    head_unborn: bool,
) -> Result<(), SnapshotError> {
    // 1. Check for active sparse-checkout configuration
    let out_sparse = run_hermetic_git(
        repo_root,
        &["config", "--bool", "core.sparseCheckout"],
        env.config_path(),
        env.temp_dir(),
    )?;
    if out_sparse.status.success() {
        let val = String::from_utf8_lossy(&out_sparse.stdout)
            .trim()
            .to_string();
        if val == "true" {
            return Err(SnapshotError::UnsupportedIndexLayout(
                "Active sparse-checkout configured in core.sparseCheckout".to_string(),
            ));
        }
    } else if out_sparse.status.code() != Some(1) {
        return Err(SnapshotError::UnsupportedIndexLayout(format!(
            "Failed to query core.sparseCheckout config: exit code {:?}",
            out_sparse.status.code()
        )));
    }

    let out_cone = run_hermetic_git(
        repo_root,
        &["config", "--bool", "core.sparseCheckoutCone"],
        env.config_path(),
        env.temp_dir(),
    )?;
    if out_cone.status.success() {
        let val = String::from_utf8_lossy(&out_cone.stdout).trim().to_string();
        if val == "true" {
            return Err(SnapshotError::UnsupportedIndexLayout(
                "Active sparse-checkout configured in core.sparseCheckoutCone".to_string(),
            ));
        }
    } else if out_cone.status.code() != Some(1) {
        return Err(SnapshotError::UnsupportedIndexLayout(format!(
            "Failed to query core.sparseCheckoutCone config: exit code {:?}",
            out_cone.status.code()
        )));
    }

    // 2. Check for active split-index configuration
    let out_split = run_hermetic_git(
        repo_root,
        &["config", "--bool", "core.splitIndex"],
        env.config_path(),
        env.temp_dir(),
    )?;
    if out_split.status.success() {
        let val = String::from_utf8_lossy(&out_split.stdout)
            .trim()
            .to_string();
        if val == "true" {
            return Err(SnapshotError::UnsupportedIndexLayout(
                "Active split-index configured in core.splitIndex".to_string(),
            ));
        }
    } else if out_split.status.code() != Some(1) {
        return Err(SnapshotError::UnsupportedIndexLayout(format!(
            "Failed to query core.splitIndex config: exit code {:?}",
            out_split.status.code()
        )));
    }

    // 3. Check for sparse-checkout metadata files
    let sparse_info = identity.git_dir.join("info").join("sparse-checkout");
    if sparse_info.exists() {
        return Err(SnapshotError::UnsupportedIndexLayout(
            "Active sparse-checkout detected in info/sparse-checkout".to_string(),
        ));
    }
    let sparse_reapply = identity
        .git_dir
        .join("info")
        .join("sparse-checkout.reapply");
    if sparse_reapply.exists() {
        return Err(SnapshotError::UnsupportedIndexLayout(
            "Active sparse-checkout detected in info/sparse-checkout.reapply".to_string(),
        ));
    }

    // 4. Check for split-index files (sharedindex.*) in git_dir and common_dir
    for search_dir in &[&identity.git_dir, &identity.common_dir] {
        if search_dir.exists() {
            let entries = fs::read_dir(search_dir).map_err(|e| {
                io_err("read_dir git_dir for split-index preflight", *search_dir, e)
            })?;
            for entry in entries {
                let entry =
                    entry.map_err(|e| io_err("read_dir entry in git_dir", *search_dir, e))?;
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with("sharedindex.") {
                        return Err(SnapshotError::UnsupportedIndexLayout(format!(
                            "Split-index detected: shared index file '{name}' is present in git directory"
                        )));
                    }
                }
            }
        }
    }

    // 5. Inspect active physical index presence and structural layout
    match inspect_index_presence(&identity.index_path, &identity.git_dir, head_unborn)? {
        IndexPresence::Present(index_bytes) => {
            parse_and_validate_git_index(&index_bytes, identity.hash_len)?;
        }
        IndexPresence::ValidAbsent => (),
    }

    Ok(())
}

/// Captures the full Git metadata fingerprint (§283.2, §285.3).
pub fn capture_git_snapshot(
    repo_root: &Path,
    index_path: &Path,
    git_dir: &Path,
    env: &HermeticGitEnv,
    head_unborn: bool,
) -> Result<GitSnapshot, SnapshotError> {
    GIT_INDEX_COMMAND_COUNT.fetch_add(1, Ordering::SeqCst);

    // 1. Resolved HEAD commit
    let head_bytes = run_hermetic_git_stdout(
        repo_root,
        &["rev-parse", "--verify", "HEAD"],
        env.config_path(),
        env.temp_dir(),
    )
    .map_err(|e| match e {
        SnapshotError::GitCommandFailed { ref stderr, .. }
            if stderr.contains("fatal: Needed a single revision") =>
        {
            SnapshotError::UnbornHead
        }
        other => other,
    })?;
    let head_commit = String::from_utf8_lossy(&head_bytes).trim().to_string();

    // 2. Symbolic vs Detached HEAD
    let (head_mode, head_ref) = query_symbolic_or_detached_head(repo_root, env)?;

    // 3. Sorted refs digest
    let refs_bytes = run_hermetic_git_stdout(
        repo_root,
        &["for-each-ref", "--format=%(refname) %(objectname)", "refs/"],
        env.config_path(),
        env.temp_dir(),
    )?;
    let refs_digest = sha256_hex(&refs_bytes);

    // 4. Staged entries digest (ls-files -s -z)
    let ls_files_bytes = run_hermetic_git_stdout(
        repo_root,
        &["ls-files", "-s", "-z"],
        env.config_path(),
        env.temp_dir(),
    )?;
    let mut offset = 0;
    while offset < ls_files_bytes.len() {
        if let Some(nul_pos) = ls_files_bytes[offset..].iter().position(|&b| b == 0) {
            let record = &ls_files_bytes[offset..offset + nul_pos];
            if record.starts_with(b"040000 ") {
                return Err(SnapshotError::UnsupportedIndexLayout(
                    "Sparse-index directory entry (040000) detected in ls-files".to_string(),
                ));
            }
            offset += nul_pos + 1;
        } else {
            break;
        }
    }
    let index_entries_digest = sha256_hex(&ls_files_bytes);

    // 5. Active physical index file digest via safe inspection
    let index_file_digest = match inspect_index_presence(index_path, git_dir, head_unborn)? {
        IndexPresence::Present(bytes) => sha256_hex(&bytes),
        IndexPresence::ValidAbsent => sha256_hex(b""),
    };

    // 6. Status digest (status --porcelain=v2 -z --untracked-files=all)
    let status_bytes = run_hermetic_git_stdout(
        repo_root,
        &["status", "--porcelain=v2", "-z", "--untracked-files=all"],
        env.config_path(),
        env.temp_dir(),
    )?;
    let status_digest = sha256_hex(&status_bytes);

    Ok(GitSnapshot {
        head_commit,
        head_ref,
        head_mode,
        refs_digest,
        index_entries_digest,
        index_file_digest,
        status_digest,
    })
}

/// Normalizes relative path separators to '/' and validates path safety against ADS, device names, and aliases (§293.4 #7).
pub fn normalize_and_validate_relative_path(rel_path: &str) -> Result<String, SnapshotError> {
    if rel_path.is_empty() {
        return Err(SnapshotError::IllegalPath(
            "Empty relative path".to_string(),
        ));
    }
    if rel_path.contains('\0') {
        return Err(SnapshotError::IllegalPath(
            "Path contains NUL byte".to_string(),
        ));
    }
    let normalized = rel_path.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();

    for part in &parts {
        if *part == ".." {
            return Err(SnapshotError::IllegalPath(format!(
                "Path traversal component '..' detected in '{rel_path}'"
            )));
        }
        if *part == "." {
            return Err(SnapshotError::IllegalPath(format!(
                "Path component '.' detected in '{rel_path}'"
            )));
        }

        // On Windows, reject Alternate Data Streams (contains ':')
        if part.contains(':') {
            return Err(SnapshotError::IllegalPath(format!(
                "Alternate Data Stream ':' detected in component '{part}'"
            )));
        }

        // On Windows, reject trailing dots and spaces
        if part.ends_with('.') || part.ends_with(' ') {
            return Err(SnapshotError::IllegalPath(format!(
                "Trailing dot or space alias rejected in component '{part}'"
            )));
        }

        // On Windows, reject DOS reserved device names
        let stem = part.split('.').next().unwrap_or(part);
        let reserved = [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ];
        if reserved.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
            return Err(SnapshotError::IllegalPath(format!(
                "Windows reserved device name rejected in component '{part}'"
            )));
        }
    }

    if Path::new(&normalized).is_absolute() || normalized.starts_with('/') {
        return Err(SnapshotError::IllegalPath(format!(
            "Absolute path detected: '{rel_path}'"
        )));
    }
    Ok(parts.join("/"))
}

/// Pure status record parser for porcelain-v2 records (§296.3 #4).
pub fn parse_porcelain_v2_status(
    status_bytes: &[u8],
) -> Result<BTreeMap<String, BTreeSet<ProtectedSource>>, SnapshotError> {
    let mut map: BTreeMap<String, BTreeSet<ProtectedSource>> = BTreeMap::new();
    let records: Vec<&[u8]> = status_bytes.split(|&b| b == 0).collect();
    let mut idx = 0;
    while idx < records.len() {
        let record = records[idx];
        if record.is_empty() {
            idx += 1;
            continue;
        }

        if record.starts_with(b"# ") {
            idx += 1;
            continue;
        }
        if record.starts_with(b"1 ") {
            let rec_str = String::from_utf8(record.to_vec()).map_err(|_| {
                SnapshotError::IllegalPath("Non-UTF-8 status record in porcelain-v2".into())
            })?;
            let tokens: Vec<&str> = rec_str.splitn(9, ' ').collect();
            if tokens.len() != 9 {
                return Err(SnapshotError::GitCommandFailed {
                    cmd: "status --porcelain=v2".into(),
                    stderr: format!(
                        "Malformed porcelain-v2 '1' record: expected 9 tokens, got {}",
                        tokens.len()
                    ),
                });
            }
            let xy = tokens[1];
            let path = normalize_and_validate_relative_path(tokens[8])?;
            let set = map.entry(path).or_default();
            set.insert(ProtectedSource::Tracked);
            if xy.chars().next() != Some('.') {
                set.insert(ProtectedSource::Staged);
            }
            idx += 1;
        } else if record.starts_with(b"u ") {
            // Unmerged 'u' record has 11 tokens: u <xy> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path> (§293.4 #7, §296.3 #4)
            let rec_str = String::from_utf8(record.to_vec()).map_err(|_| {
                SnapshotError::IllegalPath("Non-UTF-8 unmerged record in porcelain-v2".into())
            })?;
            let tokens: Vec<&str> = rec_str.splitn(11, ' ').collect();
            if tokens.len() != 11 {
                return Err(SnapshotError::GitCommandFailed {
                    cmd: "status --porcelain=v2".into(),
                    stderr: format!(
                        "Malformed porcelain-v2 'u' record: expected 11 tokens, got {}",
                        tokens.len()
                    ),
                });
            }
            let path = normalize_and_validate_relative_path(tokens[10])?;
            let set = map.entry(path).or_default();
            set.insert(ProtectedSource::Tracked);
            set.insert(ProtectedSource::Staged);
            idx += 1;
        } else if record.starts_with(b"2 ") {
            let rec_str = String::from_utf8(record.to_vec()).map_err(|_| {
                SnapshotError::IllegalPath("Non-UTF-8 rename record in porcelain-v2".into())
            })?;
            let tokens: Vec<&str> = rec_str.splitn(10, ' ').collect();
            if tokens.len() != 10 {
                return Err(SnapshotError::GitCommandFailed {
                    cmd: "status --porcelain=v2".into(),
                    stderr: format!(
                        "Malformed porcelain-v2 '2' record: expected 10 tokens, got {}",
                        tokens.len()
                    ),
                });
            }
            let xy = tokens[1];
            let path = normalize_and_validate_relative_path(tokens[9])?;
            let set = map.entry(path).or_default();
            set.insert(ProtectedSource::Tracked);
            if xy.chars().next() != Some('.') {
                set.insert(ProtectedSource::Staged);
            }
            idx += 1;
            if idx >= records.len() || records[idx].is_empty() {
                return Err(SnapshotError::GitCommandFailed {
                    cmd: "status --porcelain=v2".into(),
                    stderr: "Missing orig-path companion record for porcelain-v2 rename".into(),
                });
            }
            let orig_path = String::from_utf8(records[idx].to_vec()).map_err(|_| {
                SnapshotError::IllegalPath("Non-UTF-8 orig path in rename record".into())
            })?;
            let orig_norm = normalize_and_validate_relative_path(&orig_path)?;
            map.entry(orig_norm)
                .or_default()
                .insert(ProtectedSource::Tracked);
            idx += 1;
        } else if record.starts_with(b"? ") {
            let path_bytes = &record[2..];
            if path_bytes.is_empty() {
                return Err(SnapshotError::GitCommandFailed {
                    cmd: "status --porcelain=v2".into(),
                    stderr: "Empty path in porcelain-v2 untracked '?' record".into(),
                });
            }
            let path = String::from_utf8(path_bytes.to_vec()).map_err(|_| {
                SnapshotError::IllegalPath("Non-UTF-8 untracked path in porcelain-v2".into())
            })?;
            let norm = normalize_and_validate_relative_path(&path)?;
            map.entry(norm)
                .or_default()
                .insert(ProtectedSource::Untracked);
            idx += 1;
        } else if record.starts_with(b"! ") {
            let path_bytes = &record[2..];
            if path_bytes.is_empty() {
                return Err(SnapshotError::GitCommandFailed {
                    cmd: "status --porcelain=v2".into(),
                    stderr: "Empty path in porcelain-v2 ignored '!' record".into(),
                });
            }
            let path = String::from_utf8(path_bytes.to_vec()).map_err(|_| {
                SnapshotError::IllegalPath("Non-UTF-8 ignored path in porcelain-v2".into())
            })?;
            let norm = normalize_and_validate_relative_path(&path)?;
            map.entry(norm)
                .or_default()
                .insert(ProtectedSource::Ignored);
            idx += 1;
        } else {
            let rec_str = String::from_utf8_lossy(record);
            return Err(SnapshotError::GitCommandFailed {
                cmd: "status --porcelain=v2".into(),
                stderr: format!("Unsupported or unknown porcelain-v2 status record: '{rec_str}'"),
            });
        }
    }
    Ok(map)
}

/// Enumerates all protected paths across porcelain-v2, supporting unmerged 'u' records (§281.4, §293.4 #7, §296.3 #4).
pub fn enumerate_protected_paths(
    repo_root: &Path,
    policy: &SnapshotPolicy,
    env: &HermeticGitEnv,
) -> Result<BTreeMap<String, Vec<ProtectedSource>>, SnapshotError> {
    GIT_INDEX_COMMAND_COUNT.fetch_add(1, Ordering::SeqCst);
    let mut map: BTreeMap<String, BTreeSet<ProtectedSource>> = BTreeMap::new();

    // 1. Tracked paths from `ls-files -z`
    let ls_bytes = run_hermetic_git_stdout(
        repo_root,
        &["ls-files", "-z"],
        env.config_path(),
        env.temp_dir(),
    )?;
    for entry in ls_bytes.split(|&b| b == 0) {
        if entry.is_empty() {
            continue;
        }
        let rel = String::from_utf8(entry.to_vec()).map_err(|_| {
            SnapshotError::IllegalPath("Non-UTF-8 path byte sequence in ls-files".into())
        })?;
        let norm = normalize_and_validate_relative_path(&rel)?;
        map.entry(norm)
            .or_default()
            .insert(ProtectedSource::Tracked);
    }

    // 2. Status entries from `status --porcelain=v2 -z --untracked-files=all`
    let status_bytes = run_hermetic_git_stdout(
        repo_root,
        &["status", "--porcelain=v2", "-z", "--untracked-files=all"],
        env.config_path(),
        env.temp_dir(),
    )?;

    let parsed_status = parse_porcelain_v2_status(&status_bytes)?;
    for (path, sources) in parsed_status {
        let set = map.entry(path).or_default();
        for s in sources {
            set.insert(s);
        }
    }

    // 3. Explicitly configured ignored paths from policy
    for ig in &policy.ignored_paths {
        let norm = normalize_and_validate_relative_path(ig)?;
        map.entry(norm)
            .or_default()
            .insert(ProtectedSource::Ignored);
    }

    // 4. Apply exclusions and filter
    let mut result: BTreeMap<String, Vec<ProtectedSource>> = BTreeMap::new();
    for (path, set) in map {
        let is_excluded = policy.excluded_roots.iter().any(|ex| {
            let norm_ex = ex.replace('\\', "/").trim_matches('/').to_string();
            path == norm_ex || path.starts_with(&format!("{norm_ex}/"))
        });
        if !is_excluded {
            let sources: Vec<ProtectedSource> = set.into_iter().collect();
            result.insert(path, sources);
        }
    }

    if result.len() > policy.max_entries {
        return Err(SnapshotError::MaxEntriesExceeded {
            count: result.len(),
            limit: policy.max_entries,
        });
    }

    Ok(result)
}

/// Computes deterministic manifest digest over `{ entries, git }`.
pub fn compute_manifest_digest(
    entries: &[SnapshotEntry],
    git: &GitSnapshot,
) -> Result<String, SnapshotError> {
    #[derive(Serialize)]
    struct CanonicalManifestPayload<'a> {
        entries: &'a [SnapshotEntry],
        git: &'a GitSnapshot,
    }

    let payload = CanonicalManifestPayload { entries, git };
    let json_bytes = serde_json::to_vec(&payload)?;
    Ok(sha256_hex(&json_bytes))
}

/// Explicit staging directory cleanup function (§289.3 #5, §292.3 #4, §304.3 #3).
/// Path-only cleanup APIs are forbidden; all cleanups must use an authorized lease.
pub fn cleanup_staging_directory(staging_dir: &Path) -> Result<(), SnapshotError> {
    Err(SnapshotError::IllegalPath(format!(
        "Path-only cleanup forbidden for {:?}; must use cleanup_snapshot_exact_owned with an authorized lease",
        staging_dir
    )))
}

/// Safely removes published final snapshot directory during compensation (§294.2 #3c, §296.3 #5, #8, §301.3 #5, §304.3 #3).
/// Path-only cleanup APIs are forbidden; all cleanups must use an authorized lease.
pub fn cleanup_published_snapshot_directory(final_dir: &Path) -> Result<(), SnapshotError> {
    Err(SnapshotError::IllegalPath(format!(
        "Path-only cleanup forbidden for {:?}; must use cleanup_snapshot_exact_owned with an authorized lease",
        final_dir
    )))
}

/// Exact-owned snapshot cleanup verified against the creation-time lease (§300.3 #5, §301.3 #5, §304.3 #1-3).
/// Verifies recovery parent identity, child directory identity, manifest binding via handle,
/// verifies all descendants against ephemeral lease inventory with strict sharing (no FILE_SHARE_DELETE),
/// then performs handle-relative no-follow recursive deletion without calling remove_dir_all.
pub fn cleanup_snapshot_exact_owned(
    lease: &SnapshotOwnershipLease,
    is_staging: bool,
) -> Result<(), SnapshotError> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS,
            FILE_FLAG_OPEN_REPARSE_POINT,
        };

        let target_path = if is_staging {
            &lease.staging_path
        } else {
            &lease.published_path
        };

        if is_staging && INJECT_STAGING_CLEANUP_FAILURE.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(SnapshotError::CleanupFailed {
                staging_dir: target_path.to_path_buf(),
                source: std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "Injected staging cleanup failure",
                ),
            });
        }

        if !target_path.exists() {
            if fs::symlink_metadata(target_path).is_err() {
                return Ok(());
            }
        }

        let parent = target_path.parent().ok_or_else(|| {
            SnapshotError::IllegalPath(format!("Invalid target without parent: {target_path:?}"))
        })?;

        // 1. Open parent anchor denying delete sharing (§304.3 #1)
        let parent_file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .share_mode(nt::FILE_SHARE_READ | nt::FILE_SHARE_WRITE)
            .open(parent)
            .map_err(|e| io_err("open recovery parent for cleanup", parent, e))?;

        let mut parent_info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(parent_file.as_raw_handle() as _, &mut parent_info) }
            == 0
        {
            return Err(SnapshotError::Io(std::io::Error::last_os_error()));
        }
        let current_parent_id = (
            parent_info.dwVolumeSerialNumber,
            ((parent_info.nFileIndexHigh as u64) << 32) | (parent_info.nFileIndexLow as u64),
        );
        if current_parent_id != lease.engine_recovery_root_id {
            return Err(SnapshotError::IllegalPath(format!(
                "Recovery parent ID mismatch: expected {:?}, got {:?}",
                lease.engine_recovery_root_id, current_parent_id
            )));
        }

        let target_name = target_path.file_name().ok_or_else(|| {
            SnapshotError::IllegalPath(format!("Target path missing file name: {target_path:?}"))
        })?;
        let wide_target: Vec<u16> = target_name.encode_wide().collect();

        // 2. Open target directory relative to parent handle omitting FILE_SHARE_DELETE (§304.3 #1)
        let target_handle = unsafe {
            nt::nt_open_file_relative_with_access(
                parent_file.as_raw_handle() as _,
                &wide_target,
                true,
                nt::DELETE | nt::FILE_LIST_DIRECTORY | nt::FILE_READ_ATTRIBUTES | nt::SYNCHRONIZE,
                nt::FILE_SHARE_READ | nt::FILE_SHARE_WRITE, // Denies FILE_SHARE_DELETE
            )
        }
        .map_err(|status| {
            if status == -1073741772 || status == -1073741766 {
                return SnapshotError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Target '{:?}' not found", target_name),
                ));
            }
            SnapshotError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "Failed to open target '{:?}' relative to parent: 0x{status:08X}",
                    target_name
                ),
            ))
        })?;

        struct SafeH(windows_sys::Win32::Foundation::HANDLE);
        impl Drop for SafeH {
            fn drop(&mut self) {
                if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE as _ {
                    unsafe { CloseHandle(self.0) };
                }
            }
        }
        let safe_target = SafeH(target_handle);

        let mut target_info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(target_handle as _, &mut target_info) } == 0 {
            return Err(SnapshotError::Io(std::io::Error::last_os_error()));
        }
        let current_target_id = (
            target_info.dwVolumeSerialNumber,
            ((target_info.nFileIndexHigh as u64) << 32) | (target_info.nFileIndexLow as u64),
        );

        let expected_target_id = if is_staging {
            lease.staging_dir_id
        } else {
            lease.published_dir_id.ok_or_else(|| {
                SnapshotError::IllegalPath("Lease does not contain published_dir_id".to_string())
            })?
        };

        if current_target_id != expected_target_id {
            return Err(SnapshotError::IllegalPath(format!(
                "Exact ownership verification failed for {:?}: expected FileID {:?}, found {:?}",
                target_path, expected_target_id, current_target_id
            )));
        }

        // 3. If published, verify snapshot.json manifest binding via handle (§304.3 #2)
        if !is_staging {
            let read_bytes = unsafe { nt_read_snapshot_json_relative(target_handle)? };
            let manifest: RecoverySnapshot = serde_json::from_slice(&read_bytes).map_err(|e| {
                SnapshotError::StagedFileVerificationFailed {
                    path: "snapshot.json".into(),
                    reason: e.to_string(),
                }
            })?;
            if manifest.run_id != lease.run_id || manifest.manifest_digest != lease.manifest_digest
            {
                return Err(SnapshotError::IllegalPath(format!(
                    "Manifest binding mismatch in {:?}: expected ({}, {}), got ({}, {})",
                    target_path,
                    lease.run_id,
                    lease.manifest_digest,
                    manifest.run_id,
                    manifest.manifest_digest
                )));
            }
            let recomputed = compute_manifest_digest(&manifest.entries, &manifest.git)?;
            if recomputed != lease.manifest_digest {
                return Err(SnapshotError::ManifestVerificationFailed {
                    expected: lease.manifest_digest.clone(),
                    computed: recomputed,
                });
            }
        }

        // 4. Verify all descendants against lease inventory and delete handle-relative without remove_dir_all (§304.3 #1-3)
        let mut deleted_count = 0usize;
        unsafe {
            verify_and_delete_handle_tree(target_handle, "", lease, &mut deleted_count)?;
            nt_delete_file_by_handle(target_handle)?;
        }
        drop(safe_target);

        if deleted_count != lease.inventory.len() {
            return Err(SnapshotError::IllegalPath(format!(
                "Snapshot inventory count mismatch during cleanup: expected {}, deleted {}",
                lease.inventory.len(),
                deleted_count
            )));
        }

        // Verify target is deleted
        if fs::symlink_metadata(target_path).is_ok() {
            return Err(SnapshotError::CleanupFailed {
                staging_dir: target_path.clone(),
                source: std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Target still exists after handle deletion",
                ),
            });
        }

        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (lease, is_staging);
        Err(SnapshotError::UnsupportedPlatform(
            "Strict handle-relative cleanup is unsupported on non-Windows platforms; preserving directory"
                .to_string(),
        ))
    }
}

pub fn cleanup_published_snapshot_lease(
    lease: &SnapshotOwnershipLease,
) -> Result<(), SnapshotError> {
    cleanup_snapshot_exact_owned(lease, false)
}

pub fn cleanup_staging_snapshot_lease(lease: &SnapshotOwnershipLease) -> Result<(), SnapshotError> {
    cleanup_snapshot_exact_owned(lease, true)
}

/// RAII Staging cleanup guard with disable() method preserving staging path on failed cleanup (§292.3 #4).
pub(crate) struct StagingCleanupGuard {
    staging_dir: PathBuf,
    committed: bool,
    disabled: bool,
}

impl StagingCleanupGuard {
    pub(crate) fn new(staging_dir: PathBuf) -> Self {
        Self {
            staging_dir,
            committed: false,
            disabled: false,
        }
    }

    pub(crate) fn commit(&mut self) {
        self.committed = true;
    }

    pub(crate) fn disable(&mut self) {
        self.disabled = true;
    }
}

impl Drop for StagingCleanupGuard {
    fn drop(&mut self) {
        // §304.3 #3: On panic/unwind, Drop may record/leave the exact path but must not recursively delete.
        // Path-based Drop deletion is forbidden.
    }
}

#[cfg(windows)]
fn register_staged_file(
    lease: &mut SnapshotOwnershipLease,
    rel_path: &str,
    file: &fs::File,
) -> Result<(), SnapshotError> {
    let id = query_handle_identity(file)?;
    lease.inventory.push(DescendantInventoryEntry {
        rel_path: rel_path.to_string(),
        is_dir: false,
        id: (id.volume_serial_number, id.file_index),
    });
    Ok(())
}

#[cfg(windows)]
fn register_staged_dir(
    lease: &mut SnapshotOwnershipLease,
    staging_dir: &Path,
    rel_path: &str,
) -> Result<(), SnapshotError> {
    use std::os::windows::fs::OpenOptionsExt;
    let full = staging_dir.join(rel_path);
    let dfile = fs::OpenOptions::new()
        .read(true)
        .custom_flags(
            windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS
                | windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT,
        )
        .share_mode(nt::FILE_SHARE_READ | nt::FILE_SHARE_WRITE)
        .open(&full)
        .map_err(|e| io_err("open staged dir for identity", &full, e))?;
    let id = query_handle_identity(&dfile)?;
    lease.inventory.push(DescendantInventoryEntry {
        rel_path: rel_path.to_string(),
        is_dir: true,
        id: (id.volume_serial_number, id.file_index),
    });
    Ok(())
}

fn create_and_register_staged_dirs(
    lease: &mut SnapshotOwnershipLease,
    staging_dir: &Path,
    target_dir: &Path,
) -> Result<(), SnapshotError> {
    if target_dir.exists() {
        return Ok(());
    }
    if let Some(p) = target_dir.parent() {
        if p != staging_dir {
            create_and_register_staged_dirs(lease, staging_dir, p)?;
        }
    }
    fs::create_dir(target_dir).map_err(|e| io_err("create_dir staged subdir", target_dir, e))?;
    let rel = target_dir.strip_prefix(staging_dir).map_err(|_| {
        SnapshotError::IllegalPath(format!("target_dir {target_dir:?} not under staging_dir"))
    })?;
    let rel_str = rel.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    register_staged_dir(lease, staging_dir, &rel_str)?;
    let _ = (&rel_str, lease, staging_dir);
    Ok(())
}

/// Creates a RecoverySnapshot atomically in staging directory, then renames to `<run_id>` (§281.5, §285.3, §289.3, §293.4, §296.3 #8, §301.3 #1-#4).
pub fn create_recovery_snapshot(
    run_id: &str,
    repo_root: &Path,
    engine_root: &Path,
    policy: &SnapshotPolicy,
) -> Result<
    (
        RecoverySnapshot,
        RecoverySnapshotBinding,
        SnapshotOwnershipLease,
    ),
    SnapshotError,
> {
    let final_dir = engine_root.join("recovery").join(run_id);
    if let Ok(sym_meta) = fs::symlink_metadata(&final_dir) {
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if (sym_meta.file_attributes() & 0x400) != 0 {
                return Err(SnapshotError::ReparsePointRejected(format!(
                    "Final snapshot directory is a pre-existing reparse point: {final_dir:?}"
                )));
            }
        }
        return Err(SnapshotError::Collision(final_dir));
    }

    let recovery_parent = engine_root.join("recovery");
    fs::create_dir_all(&recovery_parent)
        .map_err(|e| io_err("create_dir_all recovery_parent", &recovery_parent, e))?;

    #[cfg(windows)]
    let recovery_parent_file = {
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT,
            FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        };

        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .share_mode(nt::FILE_SHARE_READ | nt::FILE_SHARE_WRITE)
            .open(&recovery_parent)
            .map_err(|e| io_err("open recovery parent handle", &recovery_parent, e))?;

        let mut pinfo: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, &mut pinfo) } == 0 {
            return Err(SnapshotError::Io(std::io::Error::last_os_error()));
        }
        if (pinfo.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
            return Err(SnapshotError::ReparsePointRejected(format!(
                "Recovery parent is a reparse point: {recovery_parent:?}"
            )));
        }
        file
    };

    let parent_id = {
        #[cfg(windows)]
        {
            let id = query_handle_identity(&recovery_parent_file)?;
            (id.volume_serial_number, id.file_index)
        }
        #[cfg(not(windows))]
        {
            (0, 0)
        }
    };

    let staging_id = uuid::Uuid::new_v4().to_string();
    let staging_dir = recovery_parent.join(format!(".staging-{run_id}-{staging_id}"));
    fs::create_dir(&staging_dir).map_err(|e| io_err("create_dir staging_dir", &staging_dir, e))?;

    let staging_dir_id = {
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::{
                FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
            };
            let sfile = fs::OpenOptions::new()
                .read(true)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
                .share_mode(nt::FILE_SHARE_READ | nt::FILE_SHARE_WRITE) // Denies FILE_SHARE_DELETE (§304.3 #1)
                .open(&staging_dir)
                .map_err(|e| io_err("open staging_dir to capture identity", &staging_dir, e))?;
            let id = query_handle_identity(&sfile)?;
            (id.volume_serial_number, id.file_index)
        }
        #[cfg(not(windows))]
        {
            (0, 0)
        }
    };

    let mut ownership_lease = SnapshotOwnershipLease {
        engine_recovery_root_id: parent_id,
        run_id: run_id.to_string(),
        staging_dir_id,
        published_dir_id: None,
        manifest_digest: String::new(),
        staging_path: staging_dir.clone(),
        published_path: final_dir.clone(),
        inventory: Vec::new(),
    };

    let mut staging_guard = StagingCleanupGuard::new(staging_dir.clone());

    let execute_snapshot =
        || -> Result<(RecoverySnapshot, RecoverySnapshotBinding, SnapshotOwnershipLease), SnapshotError> {
            let git_env = HermeticGitEnv::new(&recovery_parent)?;

            // 1. Initial preflight layout check before any index-reading Git command
            let identity_initial = query_and_verify_repository_identity(repo_root, &git_env)?;
            verify_repository_layout_preflight(repo_root, &identity_initial, &git_env, false)?;

            // 2. Initial Git Snapshot
            let git_snapshot_initial = capture_git_snapshot(
                repo_root,
                &identity_initial.index_path,
                &identity_initial.git_dir,
                &git_env,
                false,
            )?;

            // 3. Path enumeration
            let enumerated = enumerate_protected_paths(repo_root, policy, &git_env)?;

            let staging_files_root = staging_dir.join("files");
            create_and_register_staged_dirs(&mut ownership_lease, &staging_dir, &staging_files_root)?;

            let mut snapshot_entries = Vec::with_capacity(enumerated.len());
            let mut total_bytes: u64 = 0;

            for (rel_path, sources) in enumerated {
                let full_path = repo_root.join(&rel_path);
                let target_dest = staging_files_root.join(&rel_path);

                let capture_outcome =
                    capture_workspace_file_safe(&full_path, repo_root, policy, &rel_path)?;

                match capture_outcome {
                    SafeFileCapture::Present {
                        bytes,
                        size,
                        mode,
                        sha256,
                    } => {
                        total_bytes = total_bytes
                            .checked_add(size)
                            .ok_or(SnapshotError::SizeOverflow)?;
                        if total_bytes > policy.max_total_bytes {
                            return Err(SnapshotError::TotalSizeExceeded {
                                total: total_bytes,
                                limit: policy.max_total_bytes,
                            });
                        }

                        if let Some(dest_parent) = target_dest.parent() {
                            create_and_register_staged_dirs(
                                &mut ownership_lease,
                                &staging_dir,
                                dest_parent,
                            )?;
                        }

                        let mut out_file = fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&target_dest)
                            .map_err(|e| io_err("open create_new target_dest", &target_dest, e))?;
                        out_file
                            .write_all(&bytes)
                            .map_err(|e| io_err("write_all target_dest", &target_dest, e))?;
                        out_file
                            .flush()
                            .map_err(|e| io_err("flush target_dest", &target_dest, e))?;

                        #[cfg(windows)]
                        register_staged_file(
                            &mut ownership_lease,
                            &format!("files/{rel_path}"),
                            &out_file,
                        )?;

                        snapshot_entries.push(SnapshotEntry {
                            path: rel_path,
                            sources,
                            kind: EntryKind::File,
                            mode,
                            size,
                            sha256,
                            present: true,
                        });
                    }
                    SafeFileCapture::Absent => {
                        snapshot_entries.push(SnapshotEntry {
                            path: rel_path,
                            sources,
                            kind: EntryKind::File,
                            mode: 0,
                            size: 0,
                            sha256: String::new(),
                            present: false,
                        });
                    }
                }
            }

            if INJECT_MUTATION_DURING_COPY.load(Ordering::SeqCst) {
                return Err(SnapshotError::ConcurrentMutation(
                    "Injected concurrent mutation during copy".to_string(),
                ));
            }

            if INJECT_MUTATION_BEFORE_FINAL_PREFLIGHT.load(Ordering::SeqCst) {
                let sparse_info = identity_initial
                    .git_dir
                    .join("info")
                    .join("sparse-checkout");
                let _ = fs::create_dir_all(sparse_info.parent().unwrap());
                let _ = fs::write(&sparse_info, b"/*\n!/exclude\n");
            }

            // 4. Post-copy re-validation of repository identity and second preflight
            let identity_final = query_and_verify_repository_identity(repo_root, &git_env)?;
            if identity_initial != identity_final {
                return Err(SnapshotError::ConcurrentMutation(
                    "Repository identity or Git path bindings diverged during snapshot creation"
                        .to_string(),
                ));
            }

            verify_repository_layout_preflight(repo_root, &identity_final, &git_env, false)?;

            let git_snapshot_final = capture_git_snapshot(
                repo_root,
                &identity_final.index_path,
                &identity_final.git_dir,
                &git_env,
                false,
            )?;
            drop(git_env);

            if git_snapshot_initial != git_snapshot_final {
                return Err(SnapshotError::ConcurrentMutation(
                    "Git status or repository fingerprint diverged during snapshot creation"
                        .to_string(),
                ));
            }

            snapshot_entries.sort_by(|a, b| a.path.cmp(&b.path));

            // 5. Compute manifest digest and assemble in-memory snapshot
            let manifest_digest = compute_manifest_digest(&snapshot_entries, &git_snapshot_final)?;
            let created_at = chrono::Utc::now().to_rfc3339();

            let snapshot = RecoverySnapshot {
                run_id: run_id.to_string(),
                root: final_dir.clone(),
                entries: snapshot_entries.clone(),
                git: git_snapshot_final.clone(),
                manifest_digest: manifest_digest.clone(),
                created_at,
            };

            // 6. Write snapshot.json in staging dir
            let snapshot_json_path = staging_dir.join("snapshot.json");
            {
                let snapshot_json_bytes = serde_json::to_vec_pretty(&snapshot)?;
                let mut sfile = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&snapshot_json_path)
                    .map_err(|e| io_err("create_new snapshot.json", &snapshot_json_path, e))?;
                sfile
                    .write_all(&snapshot_json_bytes)
                    .map_err(|e| io_err("write snapshot.json", &snapshot_json_path, e))?;
                sfile
                    .flush()
                    .map_err(|e| io_err("flush snapshot.json", &snapshot_json_path, e))?;
                #[cfg(windows)]
                register_staged_file(&mut ownership_lease, "snapshot.json", &sfile)?;
            }

            #[cfg(test)]
            if INJECT_STAGING_CORRUPTION.load(std::sync::atomic::Ordering::SeqCst) {
                let _ = fs::write(&snapshot_json_path, b"{\"corrupted\":true}");
            }

            // 7. Reopen and verify staged manifest: recompute digest AND compare with in-memory snapshot (§293.4 #4)
            let read_back_json = read_file_no_follow(&snapshot_json_path, &staging_dir)?;
            let parsed_manifest: RecoverySnapshot = serde_json::from_slice(&read_back_json)
                .map_err(|e| SnapshotError::StagedFileVerificationFailed {
                    path: "snapshot.json".to_string(),
                    reason: format!("JSON parsing failed: {e}"),
                })?;

            // Recompute digest from parsed entries and git
            let recomputed_digest =
                compute_manifest_digest(&parsed_manifest.entries, &parsed_manifest.git)?;
            if recomputed_digest != parsed_manifest.manifest_digest
                || recomputed_digest != manifest_digest
            {
                return Err(SnapshotError::ManifestVerificationFailed {
                    expected: manifest_digest,
                    computed: recomputed_digest,
                });
            }

            // Compare complete manifest structure with in-memory snapshot
            if parsed_manifest.run_id != snapshot.run_id
                || parsed_manifest.root != snapshot.root
                || parsed_manifest.entries != snapshot.entries
                || parsed_manifest.git != snapshot.git
            {
                return Err(SnapshotError::ManifestVerificationFailed {
                    expected: "Identical manifest identity".to_string(),
                    computed: "Parsed manifest diverged from in-memory snapshot".to_string(),
                });
            }

            for entry in &snapshot_entries {
                if entry.present {
                    let staged_file_path = staging_files_root.join(&entry.path);
                    let staged_bytes = read_file_no_follow(&staged_file_path, &staging_files_root)?;
                    if staged_bytes.len() as u64 != entry.size {
                        return Err(SnapshotError::StagedFileVerificationFailed {
                            path: entry.path.clone(),
                            reason: format!(
                                "Staged file size mismatch: expected {}, got {}",
                                entry.size,
                                staged_bytes.len()
                            ),
                        });
                    }
                    let staged_sha = sha256_hex(&staged_bytes);
                    if staged_sha != entry.sha256 {
                        return Err(SnapshotError::StagedFileVerificationFailed {
                            path: entry.path.clone(),
                            reason: format!(
                                "Staged file SHA-256 mismatch: expected {}, got {}",
                                entry.sha256, staged_sha
                            ),
                        });
                    }
                }
            }

            // Collect creation-time ephemeral lease inventory of all staged descendants (§304.3 #2, §307.2 #1)
            #[cfg(windows)]
            {
                use std::os::windows::ffi::OsStrExt;
                use std::os::windows::io::AsRawHandle;
                let staging_name = staging_dir.file_name().unwrap();
                let staging_wide: Vec<u16> = staging_name.encode_wide().collect();
                let staging_inspect = unsafe {
                    nt::nt_open_file_relative_with_access(
                        recovery_parent_file.as_raw_handle() as _,
                        &staging_wide,
                        true,
                        nt::FILE_LIST_DIRECTORY | nt::FILE_READ_ATTRIBUTES | nt::SYNCHRONIZE,
                        nt::FILE_SHARE_READ | nt::FILE_SHARE_WRITE, // Denies FILE_SHARE_DELETE
                    )
                }
                .map_err(|status| {
                    SnapshotError::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to open staging dir for inventory collection: 0x{status:08X}"),
                    ))
                })?;
                let _safe_inspect = SafeHandle(staging_inspect);

                let mut inventory = Vec::new();
                unsafe {
                    collect_handle_relative_inventory(staging_inspect, "", &mut inventory)?;
                }
                inventory.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
                ownership_lease.inventory = inventory;
            }

            // Publication: non-replacing handle-relative rename (§301.3 #3, #4, §304.3 #1)
            #[cfg(windows)]
            {
                use std::os::windows::ffi::OsStrExt;
                use std::os::windows::io::AsRawHandle;
                use windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandle;

                // 1. Keep recovery_parent_file open. Check if destination already exists relative to parent
                let final_wide: Vec<u16> = std::ffi::OsStr::new(run_id).encode_wide().collect();
                let dest_probe = unsafe {
                    nt::nt_open_file_relative_with_access(
                        recovery_parent_file.as_raw_handle() as _,
                        &final_wide,
                        true,
                        nt::FILE_READ_ATTRIBUTES | nt::SYNCHRONIZE,
                        nt::FILE_SHARE_READ | nt::FILE_SHARE_WRITE, // Denies FILE_SHARE_DELETE
                    )
                };
                if let Ok(h) = dest_probe {
                    unsafe { windows_sys::Win32::Foundation::CloseHandle(h) };
                    return Err(SnapshotError::Collision(final_dir.clone()));
                }

                // 2. Open staging directory relative to recovery_parent_file with DELETE access
                let staging_name = staging_dir.file_name().unwrap();
                let staging_wide: Vec<u16> = staging_name.encode_wide().collect();
                let staging_handle = unsafe {
                    nt::nt_open_file_relative_with_access(
                        recovery_parent_file.as_raw_handle() as _,
                        &staging_wide,
                        true,
                        nt::DELETE | nt::SYNCHRONIZE | nt::FILE_READ_ATTRIBUTES,
                        nt::FILE_SHARE_READ | nt::FILE_SHARE_WRITE, // Denies FILE_SHARE_DELETE
                    )
                }
                .map_err(|status| {
                    SnapshotError::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to open staging dir for publication rename: 0x{status:08X}"),
                    ))
                })?;

                // 3. Verify opened staging handle File ID matches creation-time staging_id
                let mut current_staging_info: windows_sys::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
                if unsafe { GetFileInformationByHandle(staging_handle as _, &mut current_staging_info) } == 0 {
                    unsafe { windows_sys::Win32::Foundation::CloseHandle(staging_handle) };
                    return Err(SnapshotError::Io(std::io::Error::last_os_error()));
                }
                let current_staging_id = (
                    current_staging_info.dwVolumeSerialNumber,
                    ((current_staging_info.nFileIndexHigh as u64) << 32) | (current_staging_info.nFileIndexLow as u64),
                );
                if current_staging_id != staging_dir_id {
                    unsafe { windows_sys::Win32::Foundation::CloseHandle(staging_handle) };
                    return Err(SnapshotError::IllegalPath(format!(
                        "Staging directory identity changed before publication rename: expected {:?}, got {:?}",
                        staging_dir_id, current_staging_id
                    )));
                }

                // 4. Perform non-replacing handle-relative rename
                unsafe {
                    nt_rename_file_relative(
                        staging_handle as _,
                        recovery_parent_file.as_raw_handle() as _,
                        run_id,
                    )?;
                    windows_sys::Win32::Foundation::CloseHandle(staging_handle);
                }

                // 5. Reopen published directory relative to recovery_parent_file and verify identity
                let final_handle = unsafe {
                    nt::nt_open_file_relative_with_access(
                        recovery_parent_file.as_raw_handle() as _,
                        &final_wide,
                        true,
                        nt::FILE_READ_ATTRIBUTES | nt::FILE_LIST_DIRECTORY | nt::SYNCHRONIZE,
                        nt::FILE_SHARE_READ | nt::FILE_SHARE_WRITE, // Denies FILE_SHARE_DELETE
                    )
                }
                .map_err(|status| {
                    SnapshotError::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to open published final directory: 0x{status:08X}"),
                    ))
                })?;

                let safe_final = SafeHandle(final_handle);

                let mut final_info: windows_sys::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
                if unsafe { GetFileInformationByHandle(final_handle as _, &mut final_info) } == 0 {
                    return Err(SnapshotError::Io(std::io::Error::last_os_error()));
                }

                let final_dir_id = (
                    final_info.dwVolumeSerialNumber,
                    ((final_info.nFileIndexHigh as u64) << 32) | (final_info.nFileIndexLow as u64),
                );
                if final_dir_id != staging_dir_id {
                    return Err(SnapshotError::IllegalPath(format!(
                        "Published directory identity mismatch: expected {:?}, got {:?}",
                        staging_dir_id, final_dir_id
                    )));
                }

                // Verify snapshot.json in published directory via handle matches expected manifest digest (§304.3 #2)
                let pub_bytes = unsafe { nt_read_snapshot_json_relative(final_handle)? };
                drop(safe_final);

                let pub_manifest: RecoverySnapshot = serde_json::from_slice(&pub_bytes)
                    .map_err(|e| SnapshotError::StagedFileVerificationFailed {
                        path: "snapshot.json".into(),
                        reason: e.to_string(),
                    })?;
                if pub_manifest.manifest_digest != manifest_digest || pub_manifest.run_id != run_id {
                    return Err(SnapshotError::ManifestVerificationFailed {
                        expected: manifest_digest.clone(),
                        computed: pub_manifest.manifest_digest,
                    });
                }

                ownership_lease.published_dir_id = Some(final_dir_id);
                ownership_lease.manifest_digest = manifest_digest.clone();
            }
            #[cfg(not(windows))]
            {
                if final_dir.exists() {
                    return Err(SnapshotError::Collision(final_dir.clone()));
                }
                fs::rename(&staging_dir, &final_dir)
                    .map_err(|e| io_err("rename staging_dir to final_dir", &final_dir, e))?;
                ownership_lease.published_dir_id = Some((0, 0));
                ownership_lease.manifest_digest = manifest_digest.clone();
            }

            staging_guard.commit();

            let binding = RecoverySnapshotBinding {
                root: final_dir,
                manifest_digest,
                git_head: git_snapshot_final.head_commit,
                git_refs_digest: git_snapshot_final.refs_digest,
                git_index_entries_digest: git_snapshot_final.index_entries_digest,
                git_index_file_digest: git_snapshot_final.index_file_digest,
            };

            Ok((snapshot, binding, ownership_lease.clone()))
        };

    let result = execute_snapshot();
    match result {
        Ok(val) => Ok(val),
        Err(e) => {
            // Explicitly clean up staging directory (§289.3 #5, §292.3 #4).
            // If cleanup fails, disable guard drop deletion so staging is preserved for inspection.
            if let Err(cleanup_err) = cleanup_snapshot_exact_owned(&ownership_lease, true) {
                staging_guard.disable();
                return Err(cleanup_err);
            }
            Err(e)
        }
    }
}

pub fn create_recovery_snapshot_simple(
    run_id: &str,
    repo_root: &Path,
    engine_root: &Path,
    policy: &SnapshotPolicy,
) -> Result<(RecoverySnapshot, RecoverySnapshotBinding), SnapshotError> {
    let (s, b, _) = create_recovery_snapshot(run_id, repo_root, engine_root, policy)?;
    Ok((s, b))
}
