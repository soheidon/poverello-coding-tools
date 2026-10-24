//! Production Codex CLI transport primitives.
//!
//! This module builds the exact `codex exec` argument protocol, binds JSONL
//! sessions to explicit UUIDs, confines all artifacts to a run directory, and
//! executes with an explicit child environment. It does not establish an OS
//! sandbox boundary; callers must pass the independent role-isolation preflight
//! before any production launch.

use serde_json::Value;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use crate::orchestrator::process_runner::ProcessRunner;
use crate::orchestrator::types::ProcessResult;

pub const MAX_CODEX_ARTIFACT_BYTES: u64 = 1024 * 1024;
pub const MAX_CODEX_SCHEMA_BYTES: u64 = 1024 * 1024;
pub const CODEX_JSONL_LIMIT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CodexRoleProfile {
    Planner,
    Reviewer,
    Implementer,
}

impl CodexRoleProfile {
    fn cli_name(self) -> &'static str {
        match self {
            Self::Planner => "planner",
            Self::Reviewer => "reviewer",
            Self::Implementer => "implementer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodexExecPaths {
    /// A dedicated, persistent Poverello-owned directory for one run. It must
    /// not be the repository or the user's ordinary Codex home.
    pub run_root: PathBuf,
    pub working_dir: PathBuf,
    pub codex_home: PathBuf,
    pub sqlite_home: PathBuf,
    pub output_last_message: PathBuf,
    pub output_schema: PathBuf,
    #[cfg(test)]
    pub is_explicit_test_home: bool,
    pub is_candidate_working_dir: bool,
}

pub(crate) fn normalize_for_comparison(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    let s = if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
        stripped
    } else if let Some(stripped) = s.strip_prefix(r"\\?\") {
        stripped
    } else {
        &s
    };
    #[cfg(windows)]
    {
        let s = s.replace('/', "\\").to_lowercase();
        PathBuf::from(s)
    }
    #[cfg(not(windows))]
    {
        PathBuf::from(s)
    }
}

pub(crate) fn paths_overlap(left: &Path, right: &Path) -> bool {
    let left_norm = normalize_for_comparison(left);
    let right_norm = normalize_for_comparison(right);
    left_norm.starts_with(&right_norm) || right_norm.starts_with(&left_norm)
}

/// Inspects a candidate path and each of its ancestors from root to leaf for symlinks and Windows reparse points (§64.4.1),
/// canonicalizing the nearest verified existing non-alias ancestor and appending only the absent suffix (§64.4.2).
/// Propagates all metadata and canonicalization errors; treats NotFound as absent only when explicitly reported.
pub(crate) fn inspect_and_resolve_candidate_path(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!(
            "candidate path {} must be absolute",
            path.display()
        ));
    }
    let normalized = normalize_absolute(path)?;

    // Collect all ancestors from root down to the candidate path
    let mut chain: Vec<PathBuf> = normalized.ancestors().map(|p| p.to_path_buf()).collect();
    chain.reverse();

    let mut last_verified_canonical: Option<PathBuf> = None;
    let mut last_verified_original: Option<PathBuf> = None;
    let mut hit_not_found = false;

    for ancestor in chain {
        if hit_not_found {
            // Once an ancestor is NotFound, verify subsequent components do not exist
            match std::fs::symlink_metadata(&ancestor) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Ok(_) => {
                    return Err(format!(
                        "inconsistent filesystem state: child path {} exists under absent parent",
                        ancestor.display()
                    ));
                }
                Err(e) => {
                    return Err(format!(
                        "cannot inspect metadata of {}: {e}",
                        ancestor.display()
                    ));
                }
            }
            continue;
        }

        match std::fs::symlink_metadata(&ancestor) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(format!(
                        "path component {} must not be a symlink",
                        ancestor.display()
                    ));
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
                    if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                        return Err(format!(
                            "path component {} must not be a reparse point",
                            ancestor.display()
                        ));
                    }
                }
                let canonical = ancestor
                    .canonicalize()
                    .map_err(|e| format!("cannot canonicalize {}: {e}", ancestor.display()))?;
                last_verified_canonical = Some(canonical);
                last_verified_original = Some(ancestor);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                hit_not_found = true;
            }
            Err(e) => {
                return Err(format!(
                    "cannot inspect metadata of {}: {e}",
                    ancestor.display()
                ));
            }
        }
    }

    let canonical_base = last_verified_canonical.ok_or_else(|| {
        format!(
            "cannot find any verified existing ancestor for path {}",
            path.display()
        )
    })?;

    if let Some(orig_base) = last_verified_original {
        if let Ok(suffix) = normalized.strip_prefix(&orig_base) {
            if suffix.as_os_str().is_empty() {
                Ok(canonical_base)
            } else {
                Ok(canonical_base.join(suffix))
            }
        } else {
            Ok(canonical_base)
        }
    } else {
        Ok(canonical_base)
    }
}

/// Explicit input context for validating an explicit Reviewer CODEX_HOME without ambient assumptions (§63.4.2).
#[derive(Debug, Clone)]
pub(crate) struct ExplicitHomeValidationContext {
    pub run_root: PathBuf,
    pub workspace_root: PathBuf,
    pub repo_root: Result<PathBuf, String>,
    pub user_profile_or_home: Result<PathBuf, String>,
    pub appdata: Option<PathBuf>,
}

impl ExplicitHomeValidationContext {
    pub fn from_ambient(run_root: &Path, workspace_root: &Path) -> Self {
        let repo_root = std::env::current_dir()
            .map_err(|e| format!("cannot determine current working directory: {e}"));
        let user_profile_or_home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .ok_or_else(|| "cannot resolve user profile or HOME directory".to_string());
        let appdata = std::env::var_os("APPDATA").map(PathBuf::from);

        Self {
            run_root: run_root.to_path_buf(),
            workspace_root: workspace_root.to_path_buf(),
            repo_root,
            user_profile_or_home,
            appdata,
        }
    }
}

pub(crate) fn validate_explicit_reviewer_codex_home_with_context(
    home: &Path,
    ctx: &ExplicitHomeValidationContext,
) -> Result<PathBuf, String> {
    if !home.is_absolute() {
        return Err("explicit CODEX_HOME must be absolute".into());
    }
    let normalized = normalize_absolute(home)?;
    if !normalized.exists() {
        return Err("explicit CODEX_HOME does not exist".into());
    }
    if !normalized.is_dir() {
        return Err("explicit CODEX_HOME must be a directory".into());
    }

    let meta = std::fs::symlink_metadata(&normalized)
        .map_err(|e| format!("cannot inspect explicit CODEX_HOME metadata: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("explicit CODEX_HOME must not be a symbolic link".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err("explicit CODEX_HOME must not be a reparse point".into());
        }
    }

    let canonical_home = normalized
        .canonicalize()
        .map_err(|e| format!("cannot resolve explicit CODEX_HOME: {e}"))?;
    if !canonical_home.is_dir() {
        return Err("canonical explicit CODEX_HOME must be a directory".into());
    }

    // 1. Disjoint from run_root
    let canonical_run_root = if ctx.run_root.exists() {
        ctx.run_root
            .canonicalize()
            .map_err(|e| format!("cannot resolve run root: {e}"))?
    } else {
        normalize_absolute(&ctx.run_root)?
    };
    if paths_overlap(&canonical_home, &canonical_run_root) {
        return Err("explicit CODEX_HOME must be disjoint from run root".into());
    }

    // 2. Disjoint from workspace_root
    if !ctx.workspace_root.is_absolute() {
        return Err("workspace root must be absolute".into());
    }
    let canonical_ws = if ctx.workspace_root.exists() {
        ctx.workspace_root
            .canonicalize()
            .map_err(|e| format!("cannot resolve workspace root: {e}"))?
    } else {
        normalize_absolute(&ctx.workspace_root)?
    };
    if paths_overlap(&canonical_home, &canonical_ws) {
        return Err("explicit CODEX_HOME must be disjoint from workspace root".into());
    }

    // 3. Disjoint from repository root (current working directory) - fail closed on error (§63.4.2)
    let repo_root = ctx
        .repo_root
        .as_ref()
        .map_err(|e| format!("cannot establish required repository root: {e}"))?;
    if !repo_root.is_absolute() {
        return Err("repository root must be absolute".into());
    }
    if !repo_root.exists() {
        return Err("repository root does not exist".into());
    }
    let canonical_repo = repo_root
        .canonicalize()
        .map_err(|e| format!("cannot resolve repository root: {e}"))?;
    if paths_overlap(&canonical_home, &canonical_repo) {
        return Err("explicit CODEX_HOME must be disjoint from repository root".into());
    }

    // 4. Disjoint from ordinary user ~/.codex - fail closed on error (§63.4.2)
    let user_profile = ctx
        .user_profile_or_home
        .as_ref()
        .map_err(|e| format!("cannot establish required user profile for ~/.codex check: {e}"))?;
    let default_codex_raw = user_profile.join(".codex");
    let candidate_codex = inspect_and_resolve_candidate_path(&default_codex_raw)
        .map_err(|e| format!("cannot resolve default ~/.codex candidate path: {e}"))?;
    if paths_overlap(&canonical_home, &candidate_codex) {
        return Err("explicit CODEX_HOME must not be ordinary user ~/.codex".into());
    }

    // 5. Disjoint from Engine authority directory - fail closed on error (§63.4.2)
    let engine_dir_raw =
        crate::orchestrator::auto_converge::state_store::resolve_engine_authority_dir_path(
            ctx.appdata.as_deref(),
            ctx.user_profile_or_home.as_ref().ok().map(|p| p.as_path()),
        )
        .map_err(|e| format!("cannot resolve engine authority path: {e}"))?;
    let candidate_engine = inspect_and_resolve_candidate_path(&engine_dir_raw)
        .map_err(|e| format!("cannot resolve engine authority candidate path: {e}"))?;
    if paths_overlap(&canonical_home, &candidate_engine) {
        return Err("explicit CODEX_HOME must be disjoint from engine authority directory".into());
    }

    Ok(canonical_home)
}

pub(crate) fn validate_explicit_reviewer_codex_home(
    home: &Path,
    run_root: &Path,
    workspace_root: &Path,
) -> Result<PathBuf, String> {
    let ctx = ExplicitHomeValidationContext::from_ambient(run_root, workspace_root);
    validate_explicit_reviewer_codex_home_with_context(home, &ctx)
}

impl CodexExecPaths {
    pub fn validate(&self) -> Result<(), String> {
        let root = normalize_absolute(&self.run_root)?;
        let canonical_root = root
            .canonicalize()
            .map_err(|error| format!("cannot resolve Codex run root: {error}"))?;
        if !canonical_root.is_dir() {
            return Err("Codex run root must be a directory".into());
        }
        for (name, path, must_exist) in [
            ("working directory", &self.working_dir, true),
            ("CODEX_HOME", &self.codex_home, true),
            ("CODEX_SQLITE_HOME", &self.sqlite_home, true),
            ("output schema", &self.output_schema, true),
        ] {
            let normalized = normalize_absolute(path)?;
            if normalized.to_str().is_none() {
                return Err(format!("{name} path must be representable as Unicode"));
            }
            let resolved = normalized
                .canonicalize()
                .map_err(|error| format!("cannot resolve {name}: {error}"))?;

            #[cfg(test)]
            if name == "CODEX_HOME" && self.is_explicit_test_home {
                if paths_overlap(&resolved, &canonical_root) {
                    return Err(format!("{name} must be disjoint from the run root"));
                }
                if must_exist && !resolved.exists() {
                    return Err(format!("{name} does not exist"));
                }
                continue;
            }

            if name == "working directory" && self.is_candidate_working_dir {
                if paths_overlap(&resolved, &canonical_root) {
                    return Err(format!("{name} must be disjoint from the run root"));
                }
                if must_exist && !resolved.exists() {
                    return Err(format!("{name} does not exist"));
                }
                continue;
            }

            if resolved == canonical_root || !resolved.starts_with(&canonical_root) {
                return Err(format!("{name} must be strictly inside the run root"));
            }
            if must_exist && !resolved.exists() {
                return Err(format!("{name} does not exist"));
            }
        }
        if !self.working_dir.is_dir()
            || !self.codex_home.is_dir()
            || !self.sqlite_home.is_dir()
            || !self.output_schema.is_file()
        {
            return Err("working, Codex, SQLite and schema paths have invalid types".into());
        }
        let canonical_working = self
            .working_dir
            .canonicalize()
            .map_err(|error| format!("cannot resolve working directory: {error}"))?;
        let canonical_codex_home = self
            .codex_home
            .canonicalize()
            .map_err(|error| format!("cannot resolve CODEX_HOME: {error}"))?;
        let canonical_sqlite_home = self
            .sqlite_home
            .canonicalize()
            .map_err(|error| format!("cannot resolve CODEX_SQLITE_HOME: {error}"))?;
        if paths_overlap(&canonical_working, &canonical_codex_home)
            || paths_overlap(&canonical_working, &canonical_sqlite_home)
            || paths_overlap(&canonical_codex_home, &canonical_sqlite_home)
        {
            return Err("working directory, CODEX_HOME and SQLite state must be disjoint".into());
        }
        let artifact_parent = self
            .output_last_message
            .parent()
            .ok_or_else(|| "output artifact has no parent directory".to_string())?
            .canonicalize()
            .map_err(|error| format!("cannot resolve output artifact directory: {error}"))?;
        if artifact_parent == canonical_root || !artifact_parent.starts_with(&canonical_root) {
            return Err("output artifact must be strictly inside the run root".into());
        }
        let schema_path = self
            .output_schema
            .canonicalize()
            .map_err(|error| format!("cannot resolve output schema: {error}"))?;
        if [
            &canonical_working,
            &canonical_codex_home,
            &canonical_sqlite_home,
        ]
        .iter()
        .any(|protected| {
            paths_overlap(protected, &artifact_parent) || paths_overlap(protected, &schema_path)
        }) || paths_overlap(&artifact_parent, &schema_path)
        {
            return Err(
                "workspace, runtime state, schema and output artifacts must be disjoint".into(),
            );
        }
        if std::fs::symlink_metadata(&self.output_last_message).is_ok() {
            let metadata = std::fs::symlink_metadata(&self.output_last_message)
                .map_err(|error| format!("cannot inspect output artifact: {error}"))?;
            if metadata.file_type().is_symlink() {
                return Err("output artifact must not be a symbolic link".into());
            }
            let resolved = self
                .output_last_message
                .canonicalize()
                .map_err(|error| format!("cannot resolve output artifact: {error}"))?;
            if !resolved.starts_with(&canonical_root) || resolved == canonical_root {
                return Err("output artifact resolves outside the run root".into());
            }
        }
        let schema =
            read_bounded_json(&self.output_schema, MAX_CODEX_SCHEMA_BYTES, "output schema")?;
        validate_object_schema(&schema)?;
        Ok(())
    }

    fn validate_shared_session_store(&self, prior: Option<&Self>) -> Result<(), String> {
        self.validate()?;
        if let Some(prior) = prior {
            prior.validate()?;
            let canonical = |path: &Path| {
                path.canonicalize()
                    .map_err(|error| format!("cannot resolve Codex session store: {error}"))
            };
            let same_store = canonical(&self.codex_home)? == canonical(&prior.codex_home)?
                && canonical(&self.sqlite_home)? == canonical(&prior.sqlite_home)?;
            if !same_store {
                return Err("exec/resume must use the same CODEX_HOME and SQLite store".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodexInvocation {
    pub role: CodexRoleProfile,
    pub args: Vec<String>,
    pub stdin: Vec<u8>,
    pub thread_id: Option<String>,
}

pub(crate) fn initial_invocation(
    role: CodexRoleProfile,
    model: &str,
    reasoning_effort: &str,
    prompt: &str,
    paths: &CodexExecPaths,
) -> Result<CodexInvocation, String> {
    paths.validate_shared_session_store(None)?;
    validate_model_and_effort(model, reasoning_effort)?;
    if prompt.trim().is_empty() {
        return Err("Codex prompt must not be empty".into());
    }
    Ok(CodexInvocation {
        role,
        args: codex_exec_args(role, model, reasoning_effort, None, paths)?,
        stdin: prompt.as_bytes().to_vec(),
        thread_id: None,
    })
}

pub(crate) fn resume_invocation(
    prior_paths: &CodexExecPaths,
    role: CodexRoleProfile,
    model: &str,
    reasoning_effort: &str,
    thread_id: &str,
    prompt: &str,
    paths: &CodexExecPaths,
) -> Result<CodexInvocation, String> {
    if role != CodexRoleProfile::Planner {
        return Err("only Planner continuations may resume a Codex thread".into());
    }
    paths.validate_shared_session_store(Some(prior_paths))?;
    validate_model_and_effort(model, reasoning_effort)?;
    if prompt.trim().is_empty() {
        return Err("Codex resume prompt must not be empty".into());
    }
    let thread_id = validate_thread_id(thread_id)?;
    Ok(CodexInvocation {
        role,
        args: codex_exec_args(role, model, reasoning_effort, Some(&thread_id), paths)?,
        stdin: prompt.as_bytes().to_vec(),
        thread_id: Some(thread_id),
    })
}

fn validate_model_and_effort(model: &str, effort: &str) -> Result<(), String> {
    let valid_component = |value: &str| {
        !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    };
    if !valid_component(model) || !valid_component(effort) {
        return Err("model or reasoning effort contains unsupported characters".into());
    }
    Ok(())
}

fn codex_exec_args(
    role: CodexRoleProfile,
    model: &str,
    effort: &str,
    thread_id: Option<&str>,
    paths: &CodexExecPaths,
) -> Result<Vec<String>, String> {
    let path_text = |path: &Path| {
        path.to_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| "Codex CLI paths must be representable as Unicode".to_string())
    };
    let mut args = vec!["exec".to_string()];
    if let Some(thread_id) = thread_id {
        args.push("resume".into());
        args.push(validate_thread_id(thread_id)?);
    }
    args.extend([
        "--json".into(),
        "--profile".into(),
        role.cli_name().into(),
        "--ignore-rules".into(),
        "--model".into(),
        model.to_string(),
        "--config".into(),
        format!("model_reasoning_effort=\"{effort}\""),
        "--output-last-message".into(),
        path_text(&paths.output_last_message)?,
        "--output-schema".into(),
        path_text(&paths.output_schema)?,
        "-".into(),
    ]);
    // Deliberately do not add --ignore-user-config: the managed Permission
    // Profile lives in CODEX_HOME/config.toml and must remain active.
    Ok(args)
}

fn check_path_no_symlink_or_reparse(path: &Path) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| format!("cannot inspect metadata of {}: {e}", path.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!(
            "path component {} must not be a symlink",
            path.display()
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(format!(
                "path component {} must not be a reparse point",
                path.display()
            ));
        }
    }
    Ok(())
}

fn has_managed_codex_layout_marker(path: &Path) -> bool {
    path.components().any(|component| {
        let value = component.as_os_str().to_string_lossy();
        value.eq_ignore_ascii_case("vendor")
            || value.eq_ignore_ascii_case("codex-win32-x64")
            || value.to_ascii_lowercase().starts_with("codex-")
    })
}

fn parse_managed_codex_entrypoint(entrypoint: &str) -> Result<PathBuf, String> {
    if entrypoint.is_empty()
        || entrypoint.starts_with('/')
        || entrypoint.starts_with('\\')
        || entrypoint.contains('\\')
        || entrypoint.contains(':')
    {
        return Err("Codex package entrypoint must use canonical relative '/' components".into());
    }

    let components = entrypoint.split('/').collect::<Vec<_>>();
    if components
        .iter()
        .any(|component| component.is_empty() || *component == "." || *component == "..")
    {
        return Err("Codex package entrypoint contains an empty or traversal component".into());
    }

    let mut relative = PathBuf::new();
    for component in components {
        relative.push(component);
    }
    if relative.is_absolute() {
        return Err("Codex package entrypoint must not be absolute".into());
    }
    Ok(relative)
}

/// Resolves the managed package root from an explicit Codex CLI executable path.
///
/// Recognizes only the validated npm layout:
/// `<codex-package-root>\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe`.
/// Returns:
/// - `Ok(Some(package_root))` if the executable matches this layout and all metadata validations pass.
/// - `Ok(None)` for clearly standalone executables outside this recognized layout.
/// - `Err(...)` if the path contains package layout markers but fails validation, or traversal involves symlinks/reparse points.
pub(crate) fn resolve_codex_managed_package_root(
    executable: &Path,
) -> Result<Option<PathBuf>, String> {
    if !executable.is_absolute() {
        if has_managed_codex_layout_marker(executable) {
            return Err(
                "relative Codex executable path resembles a managed package layout and cannot be validated safely".into(),
            );
        }
        return Ok(None);
    }
    if executable.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    }) {
        return Err("Codex executable path must not contain '.' or '..' components".into());
    }
    if !executable.is_file() {
        return Err("Codex executable path must be an existing file".into());
    }

    check_path_no_symlink_or_reparse(executable)?;

    let bin_dir = executable.parent();
    let target_dir = bin_dir.and_then(|p| p.parent());
    let vendor_dir = target_dir.and_then(|p| p.parent());
    let native_pkg_dir = vendor_dir.and_then(|p| p.parent());

    let has_packaged_marker = has_managed_codex_layout_marker(executable);

    let (bin_dir, target_dir, vendor_dir, native_pkg_dir) = match (
        bin_dir,
        target_dir,
        vendor_dir,
        native_pkg_dir,
    ) {
        (Some(b), Some(t), Some(v), Some(n)) => (b, t, v, n),
        _ => {
            if has_packaged_marker {
                return Err(format!(
                    "Codex executable '{}' contains package directory markers but does not match expected npm layout",
                    executable.display()
                ));
            }
            return Ok(None);
        }
    };

    let is_bin = bin_dir.file_name().and_then(|s| s.to_str()) == Some("bin");
    let is_vendor = vendor_dir.file_name().and_then(|s| s.to_str()) == Some("vendor");
    let is_native = native_pkg_dir
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s == "codex-win32-x64")
        .unwrap_or(false);

    if !is_bin || !is_vendor || !is_native {
        if has_packaged_marker {
            return Err(format!(
                "Codex executable '{}' contains package directory markers but does not match expected npm layout",
                executable.display()
            ));
        }
        return Ok(None);
    }

    let target_name = target_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if target_name.is_empty() {
        return Err("missing target directory in Codex npm layout".into());
    }

    let scope_dir = match native_pkg_dir.parent() {
        Some(p) => p,
        None => return Err("native package directory has no parent scope directory".into()),
    };
    if scope_dir.file_name().and_then(|s| s.to_str()) != Some("@openai") {
        return Err(format!(
            "expected '@openai' scope directory above native package, got '{}'",
            scope_dir.display()
        ));
    }

    let node_modules_dir = match scope_dir.parent() {
        Some(p) => p,
        None => return Err("scope directory has no parent node_modules directory".into()),
    };
    if node_modules_dir.file_name().and_then(|s| s.to_str()) != Some("node_modules") {
        return Err(format!(
            "expected 'node_modules' directory above scope, got '{}'",
            node_modules_dir.display()
        ));
    }

    let package_root = match node_modules_dir.parent() {
        Some(p) => p,
        None => return Err("node_modules directory has no parent package root".into()),
    };

    // Inspect every existing component to the volume/filesystem root. Checking only
    // the package subtree would miss a package reached through a junction ancestor.
    let mut ancestors = executable.ancestors().collect::<Vec<_>>();
    ancestors.reverse();
    for ancestor in ancestors {
        check_path_no_symlink_or_reparse(ancestor)?;
    }

    let root_pkg_path = package_root.join("package.json");
    let native_pkg_path = native_pkg_dir.join("package.json");
    let codex_pkg_path = target_dir.join("codex-package.json");

    for file in [&root_pkg_path, &native_pkg_path, &codex_pkg_path] {
        if !file.is_file() {
            return Err(format!(
                "required metadata file '{}' does not exist",
                file.display()
            ));
        }
        check_path_no_symlink_or_reparse(file)?;
    }

    let root_pkg_data = std::fs::read(&root_pkg_path)
        .map_err(|e| format!("cannot read {}: {e}", root_pkg_path.display()))?;
    let root_json: serde_json::Value = serde_json::from_slice(&root_pkg_data)
        .map_err(|e| format!("invalid JSON in {}: {e}", root_pkg_path.display()))?;

    let native_pkg_data = std::fs::read(&native_pkg_path)
        .map_err(|e| format!("cannot read {}: {e}", native_pkg_path.display()))?;
    let native_json: serde_json::Value = serde_json::from_slice(&native_pkg_data)
        .map_err(|e| format!("invalid JSON in {}: {e}", native_pkg_path.display()))?;

    let codex_pkg_data = std::fs::read(&codex_pkg_path)
        .map_err(|e| format!("cannot read {}: {e}", codex_pkg_path.display()))?;
    let codex_json: serde_json::Value = serde_json::from_slice(&codex_pkg_data)
        .map_err(|e| format!("invalid JSON in {}: {e}", codex_pkg_path.display()))?;

    // 1. Root package.json: name must be "@openai/codex", version non-empty
    let root_name = root_json
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("missing string 'name' in {}", root_pkg_path.display()))?;
    if root_name != "@openai/codex" {
        return Err(format!(
            "root package.json name must be '@openai/codex', got '{root_name}'"
        ));
    }
    let root_version = root_json
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("missing string 'version' in {}", root_pkg_path.display()))?;
    if root_version.is_empty() {
        return Err(format!("empty 'version' in {}", root_pkg_path.display()));
    }

    // 2. Native package.json may use the canonical package name or npm's alias target name.
    // The alias form is accepted only when the parent package declares the exact npm alias.
    let native_name = native_json
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("missing string 'name' in {}", native_pkg_path.display()))?;
    if native_name != "@openai/codex-win32-x64" && native_name != "@openai/codex" {
        return Err(format!(
            "native package.json name must be '@openai/codex-win32-x64' or an exactly declared '@openai/codex' npm alias, got '{native_name}'"
        ));
    }
    let native_version = native_json
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("missing string 'version' in {}", native_pkg_path.display()))?;
    let native_version_clean = native_version
        .strip_suffix("-win32-x64")
        .unwrap_or(native_version);
    if native_name == "@openai/codex" {
        if native_version_clean == native_version {
            return Err(format!(
                "aliased native package version '{native_version}' must end with '-win32-x64'"
            ));
        }
        let alias_target = root_json
            .get("optionalDependencies")
            .and_then(|value| value.get("@openai/codex-win32-x64"))
            .and_then(|value| value.as_str())
            .ok_or_else(|| {
                "root package.json must declare optionalDependencies['@openai/codex-win32-x64'] for the npm alias"
                    .to_string()
            })?;
        let expected_alias = format!("npm:@openai/codex@{native_version}");
        if alias_target != expected_alias {
            return Err(format!(
                "root npm alias target '{alias_target}' does not exactly match '{expected_alias}'"
            ));
        }
    }
    if native_version_clean != root_version {
        return Err(format!(
            "native package version '{native_version}' does not agree with root version '{root_version}'"
        ));
    }

    // 3. codex-package.json: version agrees with root, target matches directory, entrypoint matches executable
    let codex_version = codex_json
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("missing string 'version' in {}", codex_pkg_path.display()))?;
    if codex_version != root_version {
        return Err(format!(
            "codex-package.json version '{codex_version}' does not agree with root version '{root_version}'"
        ));
    }
    let codex_target = codex_json
        .get("target")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("missing string 'target' in {}", codex_pkg_path.display()))?;
    if codex_target != target_name {
        return Err(format!(
            "codex-package.json target '{codex_target}' does not match target directory name '{target_name}'"
        ));
    }
    let entrypoint = codex_json
        .get("entrypoint")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            format!(
                "missing string 'entrypoint' in {}",
                codex_pkg_path.display()
            )
        })?;
    let entrypoint_rel = parse_managed_codex_entrypoint(entrypoint)?;
    let expected_exe = target_dir.join(entrypoint_rel);
    if expected_exe != executable {
        return Err(format!(
            "codex-package.json entrypoint '{}' does not match executable '{}'",
            expected_exe.display(),
            executable.display()
        ));
    }

    Ok(Some(package_root.to_path_buf()))
}

#[cfg(test)]
pub(crate) fn create_synthetic_codex_npm_layout(base: &Path) -> (PathBuf, PathBuf) {
    let target = "x86_64-pc-windows-msvc";
    let native_pkg = base
        .join("node_modules")
        .join("@openai")
        .join("codex-win32-x64");
    let target_dir = native_pkg.join("vendor").join(target);
    let bin_dir = target_dir.join("bin");
    std::fs::create_dir_all(&bin_dir).expect("create synthetic bin dir");

    let exe = bin_dir.join("codex.exe");
    std::fs::write(&exe, b"synthetic-codex-binary").expect("write synthetic exe");

    let root_pkg = serde_json::json!({
        "name": "@openai/codex",
        "version": "0.163.0"
    });
    std::fs::write(
        base.join("package.json"),
        serde_json::to_vec_pretty(&root_pkg).unwrap(),
    )
    .expect("write root package.json");

    let native_pkg_json = serde_json::json!({
        "name": "@openai/codex-win32-x64",
        "version": "0.163.0"
    });
    std::fs::write(
        native_pkg.join("package.json"),
        serde_json::to_vec_pretty(&native_pkg_json).unwrap(),
    )
    .expect("write native package.json");

    let codex_pkg_json = serde_json::json!({
        "version": "0.163.0",
        "target": target,
        "entrypoint": "bin/codex.exe"
    });
    std::fs::write(
        target_dir.join("codex-package.json"),
        serde_json::to_vec_pretty(&codex_pkg_json).unwrap(),
    )
    .expect("write codex-package.json");

    (base.to_path_buf(), exe)
}

pub(crate) fn build_child_environment(
    run_id: &str,
    codex_home: &Path,
    sqlite_home: &Path,
) -> Result<Vec<(std::ffi::OsString, std::ffi::OsString)>, String> {
    build_child_environment_with_executable(run_id, codex_home, sqlite_home, None)
}

pub(crate) fn build_child_environment_with_executable(
    run_id: &str,
    codex_home: &Path,
    sqlite_home: &Path,
    executable: Option<&Path>,
) -> Result<Vec<(std::ffi::OsString, std::ffi::OsString)>, String> {
    build_child_environment_with_executable_from_lookup(
        run_id,
        codex_home,
        sqlite_home,
        executable,
        &|k| std::env::var_os(k),
    )
}

#[allow(dead_code)]
pub(crate) fn build_child_environment_from_lookup(
    run_id: &str,
    codex_home: &Path,
    sqlite_home: &Path,
    lookup: &dyn Fn(&str) -> Option<std::ffi::OsString>,
) -> Result<Vec<(std::ffi::OsString, std::ffi::OsString)>, String> {
    build_child_environment_with_executable_from_lookup(
        run_id,
        codex_home,
        sqlite_home,
        None,
        lookup,
    )
}

pub(crate) fn build_child_environment_with_executable_from_lookup(
    run_id: &str,
    codex_home: &Path,
    sqlite_home: &Path,
    executable: Option<&Path>,
    lookup: &dyn Fn(&str) -> Option<std::ffi::OsString>,
) -> Result<Vec<(std::ffi::OsString, std::ffi::OsString)>, String> {
    crate::orchestrator::auto_converge::state_store::validate_auto_converge_run_id(run_id)
        .map_err(|error| error.to_string())?;
    let codex_home = normalize_absolute(codex_home)?;
    let sqlite_home = normalize_absolute(sqlite_home)?;
    for path in [&codex_home, &sqlite_home] {
        if !path.is_dir() {
            return Err("dedicated Codex runtime directories must already exist".into());
        }
    }
    let mut environment: Vec<(std::ffi::OsString, std::ffi::OsString)> = Vec::new();
    for name in [
        "PATH",
        "TEMP",
        "TMP",
        "APPDATA",
        "LOCALAPPDATA",
        "USERPROFILE",
    ] {
        if let Some(value) = lookup(name) {
            environment.push((name.into(), value));
        }
    }
    environment.push(("CODEX_HOME".into(), codex_home.into_os_string()));
    environment.push(("CODEX_SQLITE_HOME".into(), sqlite_home.into_os_string()));

    if let Some(exec) = executable {
        if let Some(managed_root) = resolve_codex_managed_package_root(exec)? {
            environment.push((
                "CODEX_MANAGED_PACKAGE_ROOT".into(),
                managed_root.into_os_string(),
            ));
        }
    }

    #[cfg(windows)]
    {
        let mut case_map: std::collections::BTreeMap<
            String,
            (std::ffi::OsString, std::ffi::OsString),
        > = std::collections::BTreeMap::new();
        for (k, v) in environment {
            let upper = k.to_string_lossy().to_ascii_uppercase();
            case_map.insert(upper, (k, v));
        }
        environment = case_map.into_values().collect();
    }
    #[cfg(not(windows))]
    {
        environment.sort_by(|left, right| left.0.to_string_lossy().cmp(&right.0.to_string_lossy()));
    }

    if environment.iter().any(|(key, _)| {
        [
            "OPENAI_API_KEY",
            "CODEX_API_KEY",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
        ]
        .iter()
        .any(|denied| key.to_string_lossy().eq_ignore_ascii_case(denied))
    }) {
        return Err(
            "credential and proxy variables are forbidden in the Codex child environment".into(),
        );
    }
    Ok(environment)
}

/// Windows sandbox backend discriminator for Reviewer process isolation (§36.3 Candidate A).
/// Enforces fail-closed validation: only supported, proven container/sandbox backends are accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowsSandboxBackend {
    Mxc,
}

impl WindowsSandboxBackend {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mxc" => Ok(Self::Mxc),
            "unelevated" => Err("Windows sandbox backend 'unelevated' is rejected: weaker isolation does not satisfy reviewer read/write denial (§36.3 Candidate A)".into()),
            "elevated" => Err("Windows sandbox backend 'elevated' is unsupported for automated reviewer execution".into()),
            other => Err(format!("unrecognized or unsupported Windows sandbox backend: '{other}'")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mxc => "mxc",
        }
    }
}

/// App-managed Windows sandbox configuration and argument construction for Reviewer isolation (§36.4 Stage 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReviewerSandboxConfig {
    pub bundle_root: PathBuf,
    pub backend: WindowsSandboxBackend,
    pub profile_name: String,
}

impl ReviewerSandboxConfig {
    pub fn new(bundle_root: PathBuf) -> Result<Self, String> {
        Self::with_backend(bundle_root, WindowsSandboxBackend::Mxc)
    }

    pub fn with_backend(
        bundle_root: PathBuf,
        backend: WindowsSandboxBackend,
    ) -> Result<Self, String> {
        let normalized = normalize_absolute(&bundle_root)?;
        if !normalized.is_dir() {
            return Err(format!(
                "reviewer bundle root does not exist or is not a directory: {}",
                normalized.display()
            ));
        }
        let canonical = normalized
            .canonicalize()
            .map_err(|e| format!("cannot canonicalize reviewer bundle root: {e}"))?;

        if bundle_root
            .components()
            .any(|c| matches!(c, Component::ParentDir))
        {
            return Err(
                "reviewer bundle root path contains forbidden parent directory navigation".into(),
            );
        }

        Ok(Self {
            bundle_root: canonical,
            backend,
            profile_name: "pct-reviewer".into(),
        })
    }

    /// Preflight check verifying that the bundle root does not collide with workspace or engine authority roots.
    pub fn preflight_check(
        &self,
        workspace_root: Option<&Path>,
        engine_auth_dir: Option<&Path>,
    ) -> Result<(), String> {
        if let Some(ws) = workspace_root {
            let canonical_ws = ws
                .canonicalize()
                .map_err(|e| format!("cannot resolve workspace root boundary: {e}"))?;
            if self.bundle_root == canonical_ws
                || self.bundle_root.starts_with(&canonical_ws)
                || canonical_ws.starts_with(&self.bundle_root)
            {
                return Err("reviewer bundle root must not equal, reside inside, or contain the workspace checkout".into());
            }
        }
        if let Some(auth) = engine_auth_dir {
            let canonical_auth = auth
                .canonicalize()
                .map_err(|e| format!("cannot resolve engine authority directory boundary: {e}"))?;
            if self.bundle_root == canonical_auth
                || self.bundle_root.starts_with(&canonical_auth)
                || canonical_auth.starts_with(&self.bundle_root)
            {
                return Err("reviewer bundle root must not equal, reside inside, or contain the engine authority state".into());
            }
        }
        Ok(())
    }

    /// Renders the app-managed `config.toml` enforcing:
    /// - `default_permissions = "<profile_name>"`
    /// - `[permissions.<profile_name>.workspace_roots]` with exact bundle_root
    /// - `[permissions.<profile_name>.filesystem]` with `:root = "deny"`, `:minimal = "read"`
    /// - `[permissions.<profile_name>.filesystem.":workspace_roots"]` with `"." = "read"` (strictly read-only)
    /// - `[permissions.<profile_name>.network]` with `enabled = false`
    /// - `[windows]` `sandbox = "<backend>"`
    /// - Explicit absence of MCP servers and web search tools
    pub fn render_config_toml(&self) -> Result<String, String> {
        let bundle_str = self
            .bundle_root
            .to_str()
            .ok_or_else(|| "reviewer bundle root path must be valid Unicode".to_string())?;
        let escaped_bundle = serde_json::to_string(bundle_str)
            .map_err(|e| format!("failed to serialize bundle path: {e}"))?;

        Ok(format!(
            r#"default_permissions = "{profile}"

[permissions.{profile}.workspace_roots]
{bundle_path} = true

[permissions.{profile}.filesystem]
":root" = "deny"
":minimal" = "read"

[permissions.{profile}.filesystem.":workspace_roots"]
"." = "read"

[permissions.{profile}.network]
enabled = false

[windows]
sandbox = "{backend}"
"#,
            profile = self.profile_name,
            bundle_path = escaped_bundle,
            backend = self.backend.as_str(),
        ))
    }

    /// Constructs the exact argument vector to launch a sandboxed command through Codex CLI.
    pub fn build_sandbox_cli_args(
        &self,
        command: &str,
        command_args: &[String],
    ) -> Result<Vec<String>, String> {
        let bundle_str = self
            .bundle_root
            .to_str()
            .ok_or_else(|| "reviewer bundle root path must be valid Unicode".to_string())?;
        let mut args = vec![
            "-c".into(),
            format!("windows.sandbox={}", self.backend.as_str()),
            "sandbox".into(),
            "--permission-profile".into(),
            self.profile_name.clone(),
            "--cd".into(),
            bundle_str.into(),
            "--".into(),
            command.into(),
        ];
        args.extend(command_args.iter().cloned());
        Ok(args)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ReviewerLaunchConfig {
    pub paths: CodexExecPaths,
    pub invocation: CodexInvocation,
    pub environment: Vec<(std::ffi::OsString, std::ffi::OsString)>,
}

fn create_reviewer_config_file(path: &Path, contents: &str) -> Result<(), String> {
    use std::io::Write;

    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = std::fs::symlink_metadata(path).map_err(|metadata_error| {
                format!("cannot inspect existing reviewer config.toml: {metadata_error}")
            })?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(
                    "existing reviewer config.toml must be a regular non-symlink file".into(),
                );
            }
            let existing = std::fs::read(path).map_err(|read_error| {
                format!("cannot read existing reviewer config.toml: {read_error}")
            })?;
            if existing == contents.as_bytes() {
                // Reuse only an exact managed configuration; never replace bytes on disk.
                return Ok(());
            }
            return Err("existing reviewer config.toml differs from the required managed config; refusing to overwrite".into());
        }
        Err(error) => {
            return Err(format!("cannot create reviewer config.toml: {error}"));
        }
    };

    if let Err(error) = file.write_all(contents.as_bytes()) {
        // Leave the incomplete artifact for diagnosis; the next attempt receives a
        // different invocation root and cannot accidentally consume or replace it.
        return Err(format!("cannot write reviewer config.toml: {error}"));
    }
    Ok(())
}

pub(crate) fn build_reviewer_launch_configuration(
    request: &crate::orchestrator::auto_converge::agent_runner::RoleExecutionRequest,
) -> Result<ReviewerLaunchConfig, String> {
    build_reviewer_launch_configuration_internal(request, None, None, None, None, None)
}

pub(crate) fn build_reviewer_launch_configuration_with_executable(
    request: &crate::orchestrator::auto_converge::agent_runner::RoleExecutionRequest,
    executable: Option<&Path>,
) -> Result<ReviewerLaunchConfig, String> {
    build_reviewer_launch_configuration_internal(request, None, executable, None, None, None)
}

#[cfg(test)]
pub(crate) fn build_reviewer_launch_configuration_with_explicit_home(
    request: &crate::orchestrator::auto_converge::agent_runner::RoleExecutionRequest,
    explicit_home: Option<&Path>,
) -> Result<ReviewerLaunchConfig, String> {
    build_reviewer_launch_configuration_internal(request, explicit_home, None, None, None, None)
}

#[cfg(test)]
pub(crate) fn build_reviewer_launch_configuration_for_test(
    request: &crate::orchestrator::auto_converge::agent_runner::RoleExecutionRequest,
    explicit_home: Option<&Path>,
    test_run_root: Option<&Path>,
    validation_ctx: Option<&ExplicitHomeValidationContext>,
    env_lookup: Option<&dyn Fn(&str) -> Option<std::ffi::OsString>>,
) -> Result<ReviewerLaunchConfig, String> {
    build_reviewer_launch_configuration_internal(
        request,
        explicit_home,
        None,
        test_run_root,
        validation_ctx,
        env_lookup,
    )
}

#[cfg(test)]
pub(crate) fn build_reviewer_launch_configuration_for_test_with_executable(
    request: &crate::orchestrator::auto_converge::agent_runner::RoleExecutionRequest,
    explicit_home: Option<&Path>,
    executable: Option<&Path>,
    test_run_root: Option<&Path>,
    validation_ctx: Option<&ExplicitHomeValidationContext>,
    env_lookup: Option<&dyn Fn(&str) -> Option<std::ffi::OsString>>,
) -> Result<ReviewerLaunchConfig, String> {
    build_reviewer_launch_configuration_internal(
        request,
        explicit_home,
        executable,
        test_run_root,
        validation_ctx,
        env_lookup,
    )
}

fn build_reviewer_launch_configuration_internal(
    request: &crate::orchestrator::auto_converge::agent_runner::RoleExecutionRequest,
    explicit_home: Option<&Path>,
    executable: Option<&Path>,
    #[cfg(test)] test_run_root: Option<&Path>,
    #[cfg(not(test))] _test_run_root: Option<&Path>,
    #[cfg(test)] validation_ctx: Option<&ExplicitHomeValidationContext>,
    #[cfg(not(test))] _validation_ctx: Option<&ExplicitHomeValidationContext>,
    #[cfg(test)] env_lookup: Option<&dyn Fn(&str) -> Option<std::ffi::OsString>>,
    #[cfg(not(test))] _env_lookup: Option<&dyn Fn(&str) -> Option<std::ffi::OsString>>,
) -> Result<ReviewerLaunchConfig, String> {
    if request.role != crate::orchestrator::auto_converge::isolation::AutoConvergeRole::Reviewer {
        return Err(
            "build_reviewer_launch_configuration only supports AutoConvergeRole::Reviewer".into(),
        );
    }

    let snapshot_dir = request.snapshot_dir.as_ref().ok_or_else(|| {
        "Reviewer role execution requires an isolated snapshot directory".to_string()
    })?;
    if !snapshot_dir.exists() {
        return Err(format!(
            "Reviewer snapshot directory does not exist: {}",
            snapshot_dir.display()
        ));
    }

    let run_id = request
        .run_id
        .clone()
        .or_else(|| {
            let s = &request.session_id;
            s.split('-').next().map(|p| p.to_string())
        })
        .unwrap_or_else(|| "run-1".to_string());

    let run_root = {
        #[cfg(test)]
        if let Some(custom_root) = test_run_root {
            custom_root.to_path_buf()
        } else {
            std::env::temp_dir()
                .join("poverello")
                .join("reviewer_runs")
                .join(&run_id)
                .join(&request.session_id)
                .join(uuid::Uuid::new_v4().to_string())
        }
        #[cfg(not(test))]
        std::env::temp_dir()
            .join("poverello")
            .join("reviewer_runs")
            .join(&run_id)
            .join(&request.session_id)
            .join(uuid::Uuid::new_v4().to_string())
    };

    // Pre-creation validation of explicit home: if validation fails, nothing is created on disk (§63.10.2)
    let codex_home = if let Some(home) = explicit_home {
        #[cfg(test)]
        if let Some(ctx) = validation_ctx {
            validate_explicit_reviewer_codex_home_with_context(home, ctx)?
        } else {
            let default_ctx =
                ExplicitHomeValidationContext::from_ambient(&run_root, &request.workspace_root);
            validate_explicit_reviewer_codex_home_with_context(home, &default_ctx)?
        }
        #[cfg(not(test))]
        {
            let default_ctx =
                ExplicitHomeValidationContext::from_ambient(&run_root, &request.workspace_root);
            validate_explicit_reviewer_codex_home_with_context(home, &default_ctx)?
        }
    } else {
        run_root.join("home")
    };

    let work_dir = run_root.join("work");
    let sqlite_home = run_root.join("sqlite");
    let artifacts_dir = run_root.join("artifacts");
    let schemas_dir = run_root.join("schemas");

    std::fs::create_dir_all(&work_dir)
        .map_err(|e| format!("cannot create reviewer work dir: {e}"))?;
    if explicit_home.is_none() {
        std::fs::create_dir_all(&codex_home)
            .map_err(|e| format!("cannot create reviewer codex home: {e}"))?;
    }
    std::fs::create_dir_all(&sqlite_home)
        .map_err(|e| format!("cannot create reviewer sqlite home: {e}"))?;
    std::fs::create_dir_all(&artifacts_dir)
        .map_err(|e| format!("cannot create reviewer artifacts dir: {e}"))?;
    std::fs::create_dir_all(&schemas_dir)
        .map_err(|e| format!("cannot create reviewer schemas dir: {e}"))?;

    let schema_path = schemas_dir.join("verdict_schema.json");
    let schema_json = serde_json::json!({
        "type": "object",
        "required": ["verdict", "summary", "findings", "approval_binding"],
        "properties": {
            "verdict": { "type": "string" },
            "summary": { "type": "string" },
            "findings": { "type": "array" },
            "approval_binding": { "type": "object" },
            "architectural_defect": { "type": "boolean" },
            "escalation_recommended": { "type": "boolean" }
        },
        "additionalProperties": false
    });
    std::fs::write(
        &schema_path,
        serde_json::to_string_pretty(&schema_json).unwrap(),
    )
    .map_err(|e| format!("cannot write reviewer schema: {e}"))?;

    let output_last_message = artifacts_dir.join("last_message.json");
    if let Ok(_) = std::fs::symlink_metadata(&output_last_message) {
        std::fs::remove_file(&output_last_message).map_err(|e| {
            format!("cannot remove pre-existing reviewer output artifact before spawn: {e}")
        })?;
    }
    match std::fs::symlink_metadata(&output_last_message) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Ok(_) => {
            return Err("stale reviewer output artifact still exists after removal".to_string());
        }
        Err(e) => {
            return Err(format!(
                "cannot verify absence of reviewer output artifact: {e}"
            ));
        }
    }

    // Dedicated managed config.toml in CODEX_HOME
    let config_toml_path = codex_home.join("config.toml");
    let canonical_snap = snapshot_dir
        .canonicalize()
        .unwrap_or_else(|_| snapshot_dir.clone());
    let snap_str = canonical_snap
        .to_str()
        .ok_or_else(|| "Reviewer snapshot path must be valid Unicode".to_string())?;
    let escaped_snap = serde_json::to_string(snap_str)
        .map_err(|e| format!("failed to serialize snapshot path: {e}"))?;

    let config_content = format!(
        r#"default_permissions = "reviewer"

[permissions.reviewer.workspace_roots]
{escaped_snap} = true

[permissions.reviewer.filesystem]
":root" = "deny"
":minimal" = "read"

[permissions.reviewer.filesystem.":workspace_roots"]
"." = "read"

[permissions.reviewer.network]
enabled = false
"#
    );
    create_reviewer_config_file(&config_toml_path, &config_content)?;

    let paths = CodexExecPaths {
        run_root: run_root.clone(),
        working_dir: work_dir,
        codex_home: codex_home.clone(),
        sqlite_home: sqlite_home.clone(),
        output_last_message,
        output_schema: schema_path,
        #[cfg(test)]
        is_explicit_test_home: explicit_home.is_some(),
        is_candidate_working_dir: false,
    };
    paths.validate()?;

    let model = request.model.as_deref().unwrap_or("gpt-5-codex");
    let effort = request.reasoning_effort.as_deref().unwrap_or("high");
    let prompt = if request.input_prompt.trim().is_empty() {
        "Review candidate against requirements and provide structured verdict"
    } else {
        request.input_prompt.as_str()
    };

    let invocation = initial_invocation(CodexRoleProfile::Reviewer, model, effort, prompt, &paths)?;

    #[cfg(test)]
    let environment = if let Some(lookup) = env_lookup {
        build_child_environment_with_executable_from_lookup(
            &run_id,
            &paths.codex_home,
            &paths.sqlite_home,
            executable,
            lookup,
        )?
    } else {
        build_child_environment_with_executable(
            &run_id,
            &paths.codex_home,
            &paths.sqlite_home,
            executable,
        )?
    };
    #[cfg(not(test))]
    let environment = build_child_environment_with_executable(
        &run_id,
        &paths.codex_home,
        &paths.sqlite_home,
        executable,
    )?;

    Ok(ReviewerLaunchConfig {
        paths,
        invocation,
        environment,
    })
}

#[derive(Debug, Clone)]
pub(crate) struct ImplementerLaunchConfig {
    pub paths: CodexExecPaths,
    pub invocation: CodexInvocation,
    pub environment: Vec<(std::ffi::OsString, std::ffi::OsString)>,
}

fn create_implementer_config_file(path: &Path, contents: &str) -> Result<(), String> {
    use std::io::Write;

    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = std::fs::symlink_metadata(path).map_err(|metadata_error| {
                format!("cannot inspect existing implementer config.toml: {metadata_error}")
            })?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(
                    "existing implementer config.toml must be a regular non-symlink file".into(),
                );
            }
            let existing = std::fs::read(path).map_err(|read_error| {
                format!("cannot read existing implementer config.toml: {read_error}")
            })?;
            if existing == contents.as_bytes() {
                return Ok(());
            }
            return Err("existing implementer config.toml differs from the required managed config; refusing to overwrite".into());
        }
        Err(error) => {
            return Err(format!("cannot create implementer config.toml: {error}"));
        }
    };

    if let Err(error) = file.write_all(contents.as_bytes()) {
        return Err(format!("cannot write implementer config.toml: {error}"));
    }
    Ok(())
}

pub(crate) fn build_implementer_launch_configuration(
    request: &crate::orchestrator::auto_converge::agent_runner::RoleExecutionRequest,
) -> Result<ImplementerLaunchConfig, String> {
    build_implementer_launch_configuration_internal(request, None, None, None, None, None)
}

pub(crate) fn build_implementer_launch_configuration_with_executable(
    request: &crate::orchestrator::auto_converge::agent_runner::RoleExecutionRequest,
    executable: Option<&Path>,
) -> Result<ImplementerLaunchConfig, String> {
    build_implementer_launch_configuration_internal(request, None, executable, None, None, None)
}

#[cfg(test)]
pub(crate) fn build_implementer_launch_configuration_with_explicit_home(
    request: &crate::orchestrator::auto_converge::agent_runner::RoleExecutionRequest,
    explicit_home: Option<&Path>,
) -> Result<ImplementerLaunchConfig, String> {
    build_implementer_launch_configuration_internal(request, explicit_home, None, None, None, None)
}

#[cfg(test)]
pub(crate) fn build_implementer_launch_configuration_for_test(
    request: &crate::orchestrator::auto_converge::agent_runner::RoleExecutionRequest,
    explicit_home: Option<&Path>,
    test_run_root: Option<&Path>,
    validation_ctx: Option<&ExplicitHomeValidationContext>,
    env_lookup: Option<&dyn Fn(&str) -> Option<std::ffi::OsString>>,
) -> Result<ImplementerLaunchConfig, String> {
    build_implementer_launch_configuration_internal(
        request,
        explicit_home,
        None,
        test_run_root,
        validation_ctx,
        env_lookup,
    )
}

#[cfg(test)]
pub(crate) fn build_implementer_launch_configuration_for_test_with_executable(
    request: &crate::orchestrator::auto_converge::agent_runner::RoleExecutionRequest,
    explicit_home: Option<&Path>,
    executable: Option<&Path>,
    test_run_root: Option<&Path>,
    validation_ctx: Option<&ExplicitHomeValidationContext>,
    env_lookup: Option<&dyn Fn(&str) -> Option<std::ffi::OsString>>,
) -> Result<ImplementerLaunchConfig, String> {
    build_implementer_launch_configuration_internal(
        request,
        explicit_home,
        executable,
        test_run_root,
        validation_ctx,
        env_lookup,
    )
}

fn build_implementer_launch_configuration_internal(
    request: &crate::orchestrator::auto_converge::agent_runner::RoleExecutionRequest,
    explicit_home: Option<&Path>,
    executable: Option<&Path>,
    #[cfg(test)] test_run_root: Option<&Path>,
    #[cfg(not(test))] _test_run_root: Option<&Path>,
    #[cfg(test)] validation_ctx: Option<&ExplicitHomeValidationContext>,
    #[cfg(not(test))] _validation_ctx: Option<&ExplicitHomeValidationContext>,
    #[cfg(test)] env_lookup: Option<&dyn Fn(&str) -> Option<std::ffi::OsString>>,
    #[cfg(not(test))] _env_lookup: Option<&dyn Fn(&str) -> Option<std::ffi::OsString>>,
) -> Result<ImplementerLaunchConfig, String> {
    if request.role != crate::orchestrator::auto_converge::isolation::AutoConvergeRole::Implementer
    {
        return Err(
            "build_implementer_launch_configuration only supports AutoConvergeRole::Implementer"
                .into(),
        );
    }

    let candidate_dir = &request.workspace_root;
    if !candidate_dir.is_absolute() {
        return Err("candidate workspace directory must be absolute".into());
    }
    if !candidate_dir.exists() {
        return Err(format!(
            "candidate workspace directory does not exist: {}",
            candidate_dir.display()
        ));
    }
    let meta = std::fs::symlink_metadata(candidate_dir)
        .map_err(|e| format!("cannot inspect candidate directory metadata: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("candidate workspace directory must not be a symbolic link".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err("candidate workspace directory must not be a reparse point".into());
        }
    }
    let canonical_candidate = candidate_dir
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize candidate workspace directory: {e}"))?;
    if !canonical_candidate.is_dir() {
        return Err("candidate workspace directory must be a directory".into());
    }

    let run_id = request
        .run_id
        .clone()
        .or_else(|| {
            let s = &request.session_id;
            s.split('-').next().map(|p| p.to_string())
        })
        .unwrap_or_else(|| "run-1".to_string());

    let run_root = {
        #[cfg(test)]
        if let Some(custom_root) = test_run_root {
            custom_root.to_path_buf()
        } else {
            std::env::temp_dir()
                .join("poverello")
                .join("implementer_runs")
                .join(&run_id)
                .join(&request.session_id)
                .join(uuid::Uuid::new_v4().to_string())
        }
        #[cfg(not(test))]
        std::env::temp_dir()
            .join("poverello")
            .join("implementer_runs")
            .join(&run_id)
            .join(&request.session_id)
            .join(uuid::Uuid::new_v4().to_string())
    };

    let codex_home = if let Some(home) = explicit_home {
        #[cfg(test)]
        if let Some(ctx) = validation_ctx {
            validate_explicit_reviewer_codex_home_with_context(home, ctx)?
        } else {
            let default_ctx = ExplicitHomeValidationContext::from_ambient(&run_root, candidate_dir);
            validate_explicit_reviewer_codex_home_with_context(home, &default_ctx)?
        }
        #[cfg(not(test))]
        {
            let default_ctx = ExplicitHomeValidationContext::from_ambient(&run_root, candidate_dir);
            validate_explicit_reviewer_codex_home_with_context(home, &default_ctx)?
        }
    } else {
        run_root.join("home")
    };

    let sqlite_home = run_root.join("sqlite");
    let artifacts_dir = run_root.join("artifacts");
    let schemas_dir = run_root.join("schemas");

    std::fs::create_dir_all(&run_root)
        .map_err(|e| format!("cannot create implementer run root: {e}"))?;
    if explicit_home.is_none() {
        std::fs::create_dir_all(&codex_home)
            .map_err(|e| format!("cannot create implementer codex home: {e}"))?;
    }
    std::fs::create_dir_all(&sqlite_home)
        .map_err(|e| format!("cannot create implementer sqlite home: {e}"))?;
    std::fs::create_dir_all(&artifacts_dir)
        .map_err(|e| format!("cannot create implementer artifacts dir: {e}"))?;
    std::fs::create_dir_all(&schemas_dir)
        .map_err(|e| format!("cannot create implementer schemas dir: {e}"))?;

    let schema_path = schemas_dir.join("implementer_schema.json");
    let schema_json = serde_json::json!({
        "type": "object",
        "required": ["summary"],
        "properties": {
            "summary": { "type": "string" },
            "modified_files": { "type": "array" }
        },
        "additionalProperties": false
    });
    std::fs::write(
        &schema_path,
        serde_json::to_string_pretty(&schema_json).unwrap(),
    )
    .map_err(|e| format!("cannot write implementer schema: {e}"))?;

    let output_last_message = artifacts_dir.join("last_message.json");
    if let Ok(_) = std::fs::symlink_metadata(&output_last_message) {
        let _ = std::fs::remove_file(&output_last_message);
    }

    let config_toml_path = codex_home.join("config.toml");
    let cand_str = canonical_candidate
        .to_str()
        .ok_or_else(|| "candidate path must be valid Unicode".to_string())?;
    let escaped_cand = serde_json::to_string(cand_str)
        .map_err(|e| format!("failed to serialize candidate path: {e}"))?;

    let config_content = format!(
        r#"default_permissions = "implementer"

[permissions.implementer.workspace_roots]
{escaped_cand} = true

[permissions.implementer.filesystem]
":root" = "deny"
":minimal" = "read"

[permissions.implementer.filesystem.":workspace_roots"]
"." = "write"

[permissions.implementer.network]
enabled = false
"#
    );
    create_implementer_config_file(&config_toml_path, &config_content)?;

    let paths = CodexExecPaths {
        run_root: run_root.clone(),
        working_dir: canonical_candidate,
        codex_home: codex_home.clone(),
        sqlite_home: sqlite_home.clone(),
        output_last_message,
        output_schema: schema_path,
        #[cfg(test)]
        is_explicit_test_home: explicit_home.is_some(),
        is_candidate_working_dir: true,
    };
    paths.validate()?;

    let model = request.model.as_deref().unwrap_or("gpt-5-codex");
    let effort = request.reasoning_effort.as_deref().unwrap_or("high");
    let prompt = if request.input_prompt.trim().is_empty() {
        "Implement approved plan changes strictly within candidate workspace"
    } else {
        request.input_prompt.as_str()
    };

    let invocation =
        initial_invocation(CodexRoleProfile::Implementer, model, effort, prompt, &paths)?;

    #[cfg(test)]
    let environment = if let Some(lookup) = env_lookup {
        build_child_environment_with_executable_from_lookup(
            &run_id,
            &paths.codex_home,
            &paths.sqlite_home,
            executable,
            lookup,
        )?
    } else {
        build_child_environment_with_executable(
            &run_id,
            &paths.codex_home,
            &paths.sqlite_home,
            executable,
        )?
    };
    #[cfg(not(test))]
    let environment = build_child_environment_with_executable(
        &run_id,
        &paths.codex_home,
        &paths.sqlite_home,
        executable,
    )?;

    Ok(ImplementerLaunchConfig {
        paths,
        invocation,
        environment,
    })
}

pub(crate) async fn execute(
    runner: &ProcessRunner,
    executable: &Path,
    invocation: &CodexInvocation,
    paths: &CodexExecPaths,
    run_id: &str,
    timeout: Duration,
) -> Result<CodexTransportResponse, String> {
    paths.validate()?;
    if !executable.is_absolute() || !executable.is_file() {
        return Err("Codex CLI executable must be an existing absolute file".into());
    }
    if invocation.args.iter().any(|arg| arg.contains('\0')) {
        return Err("Codex invocation contains an invalid NUL argument".into());
    }
    let environment = build_child_environment_with_executable(
        run_id,
        &paths.codex_home,
        &paths.sqlite_home,
        Some(executable),
    )?;
    if std::fs::symlink_metadata(&paths.output_last_message).is_ok() {
        return Err("Codex output artifact already exists; refusing stale-output reuse".into());
    }
    let executable = executable
        .to_str()
        .ok_or_else(|| "Codex CLI executable path must be Unicode".to_string())?;
    let working_dir = paths
        .working_dir
        .canonicalize()
        .map_err(|error| format!("cannot resolve Codex working directory: {error}"))?;
    let result = runner
        .run_with_stdin_in_environment(
            executable,
            &invocation.args,
            Some(&working_dir),
            Some(&invocation.stdin),
            timeout,
            None,
            &environment,
        )
        .await?;
    build_response(result, invocation, paths)
}

fn build_response(
    result: ProcessResult,
    invocation: &CodexInvocation,
    paths: &CodexExecPaths,
) -> Result<CodexTransportResponse, String> {
    ensure_process_succeeded(&result)?;
    let thread_id = parse_thread_started_jsonl(result.stdout.text.as_bytes())?;
    if let Some(expected) = invocation.thread_id.as_deref() {
        if expected != thread_id {
            return Err("Codex resume returned a different thread UUID".into());
        }
    }
    let schema = read_bounded_json(
        &paths.output_schema,
        MAX_CODEX_SCHEMA_BYTES,
        "output schema",
    )?;
    validate_object_schema(&schema)?;
    let artifact = read_bounded_artifact(&paths.output_last_message)?;
    validate_artifact_against_schema(&schema, &artifact)?;
    Ok(CodexTransportResponse {
        thread_id,
        stdout: result.stdout.text.clone(),
        stderr: result.stderr.text.clone(),
        artifact,
        process: result,
    })
}

#[derive(Debug)]
pub(crate) struct CodexTransportResponse {
    pub thread_id: String,
    pub stdout: String,
    pub stderr: String,
    pub artifact: Value,
    pub process: ProcessResult,
}

pub(crate) fn parse_thread_started_jsonl(stdout: &[u8]) -> Result<String, String> {
    if stdout.len() > CODEX_JSONL_LIMIT_BYTES {
        return Err("Codex JSONL exceeded the 1 MiB limit".into());
    }
    let text = std::str::from_utf8(stdout)
        .map_err(|error| format!("Codex JSONL is not UTF-8: {error}"))?;
    if text.is_empty() || !text.ends_with('\n') {
        return Err("Codex JSONL must end with newline and include thread.started".into());
    }
    let mut thread_id = None;
    let mut completed = false;
    let mut terminal_event_seen = false;
    for (index, line) in text[..text.len() - 1].split('\n').enumerate() {
        if line.is_empty() {
            return Err(format!("Codex JSONL record {} is empty", index + 1));
        }
        let event: Value = serde_json::from_str(line)
            .map_err(|error| format!("Codex JSONL record {} is invalid: {error}", index + 1))?;
        let event_type = event
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("Codex JSONL record {} has no event type", index + 1))?;
        if terminal_event_seen {
            return Err("Codex JSONL contains events after a terminal turn event".into());
        }
        match event_type {
            "thread.started" => {
                if thread_id.is_some() {
                    return Err("Codex JSONL contains duplicate thread.started events".into());
                }
                let id = event
                    .get("thread_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "thread.started has no thread_id".to_string())?;
                thread_id = Some(validate_thread_id(id)?);
            }
            "turn.completed" => {
                if completed {
                    return Err("Codex JSONL contains duplicate turn.completed events".into());
                }
                completed = true;
                terminal_event_seen = true;
            }
            "turn.failed" | "error" => {
                return Err(format!(
                    "Codex JSONL contains unsuccessful terminal event {event_type}"
                ));
            }
            _ if event.get("thread_id").is_some() => {
                return Err(format!("unexpected thread_id on {event_type} event"));
            }
            _ => {}
        }
    }
    if !completed {
        return Err("Codex JSONL has no successful turn.completed event".into());
    }
    thread_id.ok_or_else(|| "Codex JSONL has no thread.started event".into())
}

pub(crate) fn read_bounded_artifact(path: &Path) -> Result<Value, String> {
    read_bounded_json(path, MAX_CODEX_ARTIFACT_BYTES, "Codex output artifact")
}

pub(crate) fn read_bounded_json(
    path: &Path,
    limit: u64,
    description: &str,
) -> Result<Value, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect {description}: {error}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(format!("{description} is not a regular file"));
    }
    let mut file =
        std::fs::File::open(path).map_err(|error| format!("cannot open {description}: {error}"))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {description}: {error}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!("{description} exceeds the 1 MiB limit"));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("{description} is invalid JSON: {error}"))
}

pub(crate) fn validate_object_schema(schema: &Value) -> Result<(), String> {
    let object = schema
        .as_object()
        .ok_or_else(|| "Codex output schema must be a JSON object".to_string())?;
    let allowed = ["type", "required", "properties", "additionalProperties"];
    if object.len() != allowed.len() || object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("Codex output schema has missing or unsupported top-level keywords".into());
    }
    if object.get("type").and_then(Value::as_str) != Some("object")
        || object.get("additionalProperties").and_then(Value::as_bool) != Some(false)
    {
        return Err("Codex output schema must be a strict object schema".into());
    }
    let properties = object
        .get("properties")
        .and_then(Value::as_object)
        .filter(|properties| !properties.is_empty())
        .ok_or_else(|| "Codex output schema properties must be a non-empty object".to_string())?;
    for (name, property) in properties {
        let property = property
            .as_object()
            .ok_or_else(|| format!("Codex output property {name} must be an object"))?;
        if property.len() != 1 || !property.contains_key("type") {
            return Err(format!("Codex output property {name} supports only type"));
        }
        let kind = property["type"]
            .as_str()
            .ok_or_else(|| format!("Codex output property {name} type must be a string"))?;
        if ![
            "string", "boolean", "integer", "number", "array", "object", "null",
        ]
        .contains(&kind)
        {
            return Err(format!(
                "Codex output property {name} has unsupported type {kind}"
            ));
        }
    }
    let required = object
        .get("required")
        .and_then(Value::as_array)
        .ok_or_else(|| "Codex output schema required must be an array".to_string())?;
    let mut seen = std::collections::BTreeSet::new();
    for name in required {
        let name = name
            .as_str()
            .ok_or_else(|| "Codex output schema required entries must be strings".to_string())?;
        if !seen.insert(name) || !properties.contains_key(name) {
            return Err(format!(
                "Codex output schema has invalid required property {name}"
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_artifact_against_schema(
    schema: &Value,
    artifact: &Value,
) -> Result<(), String> {
    validate_object_schema(schema)?;
    let schema = schema.as_object().expect("validated schema object");
    let properties = schema["properties"]
        .as_object()
        .expect("validated properties");
    let required = schema["required"].as_array().expect("validated required");
    let artifact = artifact
        .as_object()
        .ok_or_else(|| "Codex output artifact must be an object".to_string())?;
    for name in required.iter().filter_map(Value::as_str) {
        if !artifact.contains_key(name) {
            return Err(format!(
                "Codex output artifact is missing required property {name}"
            ));
        }
    }
    for (name, value) in artifact {
        let declaration = properties
            .get(name)
            .ok_or_else(|| format!("Codex output artifact has undeclared property {name}"))?;
        let expected = declaration["type"].as_str().expect("validated type");
        let matches = match expected {
            "string" => value.is_string(),
            "boolean" => value.is_boolean(),
            "integer" => value.as_i64().is_some(),
            "number" => value.as_f64().is_some_and(f64::is_finite),
            "array" => value.is_array(),
            "object" => value.is_object(),
            "null" => value.is_null(),
            _ => unreachable!("schema type already validated"),
        };
        if !matches {
            return Err(format!(
                "Codex output property {name} does not match {expected}"
            ));
        }
    }
    Ok(())
}

fn validate_thread_id(value: &str) -> Result<String, String> {
    let parsed =
        uuid::Uuid::parse_str(value).map_err(|_| "Codex thread ID must be a UUID".to_string())?;
    let canonical = parsed.to_string();
    if value != canonical {
        return Err("Codex thread UUID must use canonical lowercase formatting".into());
    }
    Ok(canonical)
}

fn ensure_process_succeeded(result: &ProcessResult) -> Result<(), String> {
    if result.timed_out || result.cancelled {
        return Err("Codex process timed out or was cancelled".into());
    }
    if result.stdout.is_truncated || result.stderr.is_truncated {
        return Err("Codex process output was truncated".into());
    }
    if result.exit_code != Some(0) {
        return Err(format!(
            "Codex process exited unsuccessfully (exit code {:?})",
            result.exit_code
        ));
    }
    Ok(())
}

fn normalize_absolute(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("Codex runtime paths must be absolute".into());
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => return Err("Codex runtime paths cannot contain '..'".into()),
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::types::CapturedOutput;
    use tempfile::tempdir;

    fn fixture_paths(root: &Path) -> CodexExecPaths {
        for directory in ["work", "home", "sqlite", "artifacts", "schemas"] {
            std::fs::create_dir_all(root.join(directory)).unwrap();
        }
        std::fs::write(
            root.join("schemas/output.json"),
            r#"{"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"],"additionalProperties":false}"#,
        )
        .unwrap();
        CodexExecPaths {
            run_root: root.to_path_buf(),
            working_dir: root.join("work"),
            codex_home: root.join("home"),
            sqlite_home: root.join("sqlite"),
            output_last_message: root.join("artifacts/output.json"),
            output_schema: root.join("schemas/output.json"),
            #[cfg(test)]
            is_explicit_test_home: false,
            is_candidate_working_dir: false,
        }
    }

    #[test]
    fn builds_exact_cli_args_and_resumes_only_the_explicit_thread_in_the_same_store() {
        let dir = tempdir().unwrap();
        let paths = fixture_paths(dir.path());
        let initial = initial_invocation(
            CodexRoleProfile::Planner,
            "gpt-6-luna",
            "high",
            "synthetic review prompt",
            &paths,
        )
        .unwrap();
        assert_eq!(initial.args[0], "exec");
        assert_eq!(initial.args[1], "--json");
        assert!(initial.args.contains(&"--ignore-rules".to_string()));
        assert!(!initial.args.contains(&"--ignore-user-config".to_string()));
        assert_eq!(initial.args.last().map(String::as_str), Some("-"));
        assert_eq!(initial.stdin, b"synthetic review prompt");

        let resumed = resume_invocation(
            &paths,
            CodexRoleProfile::Planner,
            "gpt-6-luna",
            "high",
            "01a15152-eddc-72b0-8eb9-1906203b1255",
            "continue synthetic review",
            &paths,
        )
        .unwrap();
        assert_eq!(
            &resumed.args[..4],
            [
                "exec",
                "resume",
                "01a15152-eddc-72b0-8eb9-1906203b1255",
                "--json"
            ]
        );
        assert_eq!(
            resumed.thread_id.as_deref(),
            Some("01a15152-eddc-72b0-8eb9-1906203b1255")
        );
        assert!(resume_invocation(
            &paths,
            CodexRoleProfile::Reviewer,
            "gpt-6-luna",
            "high",
            "01a15152-eddc-72b0-8eb9-1906203b1255",
            "reviewers always start fresh",
            &paths,
        )
        .is_err());
        let mut changed_store = paths.clone();
        changed_store.sqlite_home = paths.run_root.join("different-sqlite");
        std::fs::create_dir_all(&changed_store.sqlite_home).unwrap();
        assert!(resume_invocation(
            &paths,
            CodexRoleProfile::Planner,
            "gpt-6-luna",
            "high",
            "thread-1",
            "resume",
            &changed_store
        )
        .is_err());
        assert!(resume_invocation(
            &paths,
            CodexRoleProfile::Planner,
            "gpt-6-luna",
            "high",
            "thread-1",
            "resume",
            &paths,
        )
        .is_err());
    }

    #[test]
    fn strict_jsonl_requires_one_valid_thread_started_event() {
        assert_eq!(
            parse_thread_started_jsonl(
                b"{\"type\":\"thread.started\",\"thread_id\":\"01a15152-eddc-72b0-8eb9-1906203b1255\"}\n{\"type\":\"turn.completed\"}\n"
            )
            .unwrap(),
            "01a15152-eddc-72b0-8eb9-1906203b1255"
        );
        for invalid in [
            b"".as_slice(),
            b"{\"type\":\"thread.started\",\"thread_id\":\"not-a-uuid\"}\n",
            b"{\"type\":\"thread.started\",\"thread_id\":\"01a15152-eddc-72b0-8eb9-1906203b1255\"}\n{\"type\":\"thread.started\",\"thread_id\":\"01a15152-eddc-72b0-8eb9-1906203b1256\"}\n{\"type\":\"turn.completed\"}\n",
            b"{\"type\":\"turn.completed\",\"thread_id\":\"x\"}\n",
            b"{\"type\":\"thread.started\",\"thread_id\":\"01a15152-eddc-72b0-8eb9-1906203b1255\"}\n",
            b"{\"type\":\"thread.started\",\"thread_id\":\"01a15152-eddc-72b0-8eb9-1906203b1255\"}\n{\"type\":\"turn.failed\"}\n",
            b"{\"type\":\"thread.started\",\"thread_id\":\"01a15152-eddc-72b0-8eb9-1906203b1255\"}\n{\"type\":\"turn.completed\"}\n{\"type\":\"other\"}\n",
            b"{broken}\n",
        ] {
            assert!(parse_thread_started_jsonl(invalid).is_err());
        }
    }

    #[test]
    fn child_environment_contains_dedicated_paths_but_not_credential_or_proxy_variables() {
        let dir = tempdir().unwrap();
        let paths = fixture_paths(dir.path());
        let environment =
            build_child_environment("run-1", &paths.codex_home, &paths.sqlite_home).unwrap();
        let keys = environment
            .iter()
            .map(|(key, _)| key.to_string_lossy().to_ascii_uppercase())
            .collect::<Vec<_>>();
        assert!(keys.contains(&"CODEX_HOME".to_string()));
        assert!(keys.contains(&"CODEX_SQLITE_HOME".to_string()));
        assert!(!keys
            .iter()
            .any(|key| key.contains("API_KEY") || key.contains("PROXY")));
    }

    fn synthetic_process(stdout: &str, exit_code: Option<i32>) -> ProcessResult {
        ProcessResult {
            exit_code,
            stdout: CapturedOutput {
                text: stdout.into(),
                is_truncated: false,
                total_bytes: stdout.len(),
            },
            stderr: CapturedOutput {
                text: String::new(),
                is_truncated: false,
                total_bytes: 0,
            },
            timed_out: false,
            cancelled: false,
            duration_ms: 1,
        }
    }

    #[test]
    fn structured_artifact_is_read_only_after_success_and_bound_to_requested_thread() {
        let dir = tempdir().unwrap();
        let paths = fixture_paths(dir.path());
        std::fs::write(&paths.output_last_message, r#"{"ok":true}"#).unwrap();
        let thread_id = "01a15152-eddc-72b0-8eb9-1906203b1255";
        let invocation = CodexInvocation {
            role: CodexRoleProfile::Planner,
            args: Vec::new(),
            stdin: b"synthetic".to_vec(),
            thread_id: Some(thread_id.into()),
        };
        let stdout = format!("{{\"type\":\"thread.started\",\"thread_id\":\"{thread_id}\"}}\n{{\"type\":\"turn.completed\"}}\n");
        let response =
            build_response(synthetic_process(&stdout, Some(0)), &invocation, &paths).unwrap();
        assert_eq!(response.thread_id, thread_id);
        assert_eq!(response.artifact, serde_json::json!({"ok": true}));

        let mismatched =
            format!("{{\"type\":\"thread.started\",\"thread_id\":\"different-thread\"}}\n");
        assert!(
            build_response(synthetic_process(&mismatched, Some(0)), &invocation, &paths).is_err()
        );
        assert!(build_response(synthetic_process(&stdout, Some(1)), &invocation, &paths).is_err());
    }

    #[test]
    fn validates_schema_contract_and_artifact_shape_locally() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {"ok": {"type": "boolean"}, "name": {"type": "string"}},
            "required": ["ok"],
            "additionalProperties": false
        });
        validate_object_schema(&schema).unwrap();
        validate_artifact_against_schema(&schema, &serde_json::json!({"ok": true})).unwrap();
        for invalid in [
            serde_json::json!({"ok": true, "extra": 1}),
            serde_json::json!({"ok": "true"}),
            serde_json::json!({"name": "missing required"}),
            serde_json::json!([]),
        ] {
            assert!(validate_artifact_against_schema(&schema, &invalid).is_err());
        }
        assert!(validate_object_schema(&serde_json::json!({
            "type": "object", "properties": {}, "required": [], "additionalProperties": false
        }))
        .is_err());
        assert!(validate_object_schema(&serde_json::json!({
            "type": "object", "properties": {"ok": {"type": "boolean"}}, "required": ["unknown"], "additionalProperties": false
        }))
        .is_err());
    }

    #[tokio::test]
    async fn execute_uses_process_runner_with_a_synthetic_cli_child() {
        let dir = tempdir().unwrap();
        let paths = fixture_paths(dir.path());
        let source = dir.path().join("codex_fixture.rs");
        let executable = dir.path().join("codex_fixture.exe");
        std::fs::write(
            &source,
            r#"
use std::{env, io::{self, Read}, path::PathBuf};
fn quote(value: &str) -> String { serde_quote(value) }
fn serde_quote(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => return String::new(),
            ch => out.push(ch),
        }
    }
    out.push('"'); out
}
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut stdin = String::new(); io::stdin().read_to_string(&mut stdin).unwrap();
    let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
    let home = env::var("CODEX_HOME").unwrap();
    let sqlite = env::var("CODEX_SQLITE_HOME").unwrap();
    let expected_id = "01a15152-eddc-72b0-8eb9-1906203b1255";
    let is_resume = args.get(1).map(String::as_str) == Some("resume");
    let id_ok = if is_resume { args.get(2).map(String::as_str) == Some(expected_id) } else { true };
    let has_value = |flag: &str, value: &str| args.windows(2).any(|pair| pair[0] == flag && pair[1] == value);
    let schema_path = args.windows(2).find(|pair| pair[0] == "--output-schema").map(|pair| PathBuf::from(&pair[1]));
    let artifact_path = args.windows(2).find(|pair| pair[0] == "--output-last-message").map(|pair| PathBuf::from(&pair[1]));
    let args_ok = args.first().map(String::as_str) == Some("exec")
        && args.last().map(String::as_str) == Some("-")
        && args.contains(&"--json".into())
        && args.contains(&"--ignore-rules".into())
        && !args.contains(&"--ignore-user-config".into())
        && has_value("--profile", "planner")
        && has_value("--model", "gpt-6-luna")
        && has_value("--config", "model_reasoning_effort=\"high\"")
        && schema_path.as_ref().is_some_and(|path| path.is_file())
        && artifact_path.as_ref().and_then(|path| path.parent()).is_some_and(|path| path.is_dir())
        && id_ok;
    let forbidden_env_absent = ["OPENAI_API_KEY", "CODEX_API_KEY", "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"]
        .iter().all(|name| env::var_os(name).is_none());
    if !args_ok || !forbidden_env_absent || stdin.is_empty() { std::process::exit(19); }
    let value = format!("{{\"type\":\"fixture.observed\",\"cwd\":{},\"home\":{},\"sqlite\":{},\"args_ok\":true,\"stdin_ok\":{},\"forbidden_env_absent\":true}}",
        quote(&cwd), quote(&home), quote(&sqlite), stdin == "synthetic fixture prompt");
    std::fs::write(artifact_path.unwrap(), "{\"ok\":true}").unwrap();
    println!("{{\"type\":\"thread.started\",\"thread_id\":{}}}", quote(expected_id));
    println!("{value}");
    println!("{{\"type\":\"turn.completed\"}}");
}
"#,
        )
        .unwrap();
        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let compile = std::process::Command::new(rustc)
            .arg("--edition=2021")
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("rustc must be available when running Rust tests");
        assert!(
            compile.status.success(),
            "fixture compilation failed: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let invocation = initial_invocation(
            CodexRoleProfile::Planner,
            "gpt-6-luna",
            "high",
            "synthetic fixture prompt",
            &paths,
        )
        .unwrap();
        let result = execute(
            &ProcessRunner::new(),
            &executable,
            &invocation,
            &paths,
            "run-1",
            Duration::from_secs(10),
        )
        .await
        .unwrap();
        assert_eq!(result.thread_id, "01a15152-eddc-72b0-8eb9-1906203b1255");
        assert_eq!(result.artifact, serde_json::json!({"ok": true}));
        assert!(result.stdout.contains("\"args_ok\":true"));
        assert!(result.stdout.contains("\"stdin_ok\":true"));
        assert!(result.stdout.contains("\"forbidden_env_absent\":true"));
        let cwd = paths.working_dir.canonicalize().unwrap();
        assert!(result
            .stdout
            .contains(&serde_json::to_string(cwd.to_str().unwrap()).unwrap()));
        assert!(result
            .stdout
            .contains(&serde_json::to_string(paths.codex_home.to_str().unwrap()).unwrap()));
        assert!(result
            .stdout
            .contains(&serde_json::to_string(paths.sqlite_home.to_str().unwrap()).unwrap()));

        let mut resume_paths = paths.clone();
        resume_paths.output_last_message = dir.path().join("artifacts/resumed.json");
        let resumed = resume_invocation(
            &paths,
            CodexRoleProfile::Planner,
            "gpt-6-luna",
            "high",
            &result.thread_id,
            "synthetic fixture prompt",
            &resume_paths,
        )
        .unwrap();
        let resumed_result = execute(
            &ProcessRunner::new(),
            &executable,
            &resumed,
            &resume_paths,
            "run-1",
            Duration::from_secs(10),
        )
        .await
        .unwrap();
        assert_eq!(resumed_result.thread_id, result.thread_id);
        assert_eq!(resumed_result.artifact, serde_json::json!({"ok": true}));
    }

    #[cfg(windows)]
    fn sandbox_probe_environment(
        codex_home: &Path,
        sqlite_home: &Path,
        fixture_root: &Path,
    ) -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
        let mut environment = Vec::new();
        for name in ["PATH", "SystemDrive", "ComSpec"] {
            if let Some(value) = std::env::var_os(name) {
                environment.push((name.into(), value));
            }
        }
        for (name, value) in [
            ("TEMP", fixture_root),
            ("TMP", fixture_root),
            ("USERPROFILE", codex_home),
            ("APPDATA", codex_home),
            ("LOCALAPPDATA", codex_home),
            ("CODEX_HOME", codex_home),
            ("CODEX_SQLITE_HOME", sqlite_home),
        ] {
            environment.push((name.into(), value.as_os_str().to_owned()));
        }
        environment
    }

    #[cfg(windows)]
    async fn run_windows_sandbox_probe(
        runner: &ProcessRunner,
        codex_cli: &Path,
        codex_home: &Path,
        sqlite_home: &Path,
        fixture_root: &Path,
        sandbox_config: &ReviewerSandboxConfig,
        command: &str,
        command_args: &[String],
        timeout: Duration,
    ) -> ProcessResult {
        let args = sandbox_config
            .build_sandbox_cli_args(command, command_args)
            .expect("build_sandbox_cli_args must succeed");
        runner
            .run_with_stdin_in_environment(
                codex_cli.to_str().expect("Codex CLI path must be Unicode"),
                &args,
                Some(&sandbox_config.bundle_root),
                Some(&[]),
                timeout,
                None,
                &sandbox_probe_environment(codex_home, sqlite_home, fixture_root),
            )
            .await
            .expect("Poverello ProcessRunner must launch the bounded sandbox probe")
    }

    /// Exercises the real Codex Windows sandbox command boundary through Poverello's
    /// bounded ProcessRunner, without starting a model session or touching user state.
    /// Run explicitly on a compatible Windows host with `PCT_CODEX_CLI` set to the
    /// absolute Codex CLI executable. This filesystem-only prototype makes no network
    /// request and does not start a model session or touch user state.
    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "manual Windows MXC isolation probe; bounded to a disposable temp fixture"]
    async fn windows_mxc_reviewer_profile_enforces_fixture_read_write_boundaries() {
        let codex_cli = std::env::var_os("PCT_CODEX_CLI")
            .map(PathBuf::from)
            .expect("set PCT_CODEX_CLI to the absolute codex.exe path");
        assert!(codex_cli.is_absolute() && codex_cli.is_file());

        let temp = tempdir().unwrap();
        let fixture_root = temp.path();
        let reviewer_root = fixture_root.join("review-root");
        let outside_root = fixture_root.join("outside");
        let codex_home = fixture_root.join("codex-home");
        let sqlite_home = fixture_root.join("sqlite-home");
        for path in [&reviewer_root, &outside_root, &codex_home, &sqlite_home] {
            std::fs::create_dir_all(path).unwrap();
        }
        let allowed_file = reviewer_root.join("allowed.txt");
        let outside_file = outside_root.join("protected.txt");
        let allowed_bytes = b"PCT_ALLOWED_FIXTURE\n";
        let outside_bytes = b"PCT_OUTSIDE_FIXTURE\n";
        std::fs::write(&allowed_file, allowed_bytes).unwrap();
        std::fs::write(&outside_file, outside_bytes).unwrap();

        fn hash_bytes(data: &[u8]) -> [u8; 32] {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(data);
            hasher.finalize().into()
        }
        let initial_allowed_hash = hash_bytes(allowed_bytes);
        let initial_outside_hash = hash_bytes(outside_bytes);

        let probe_rs = reviewer_root.join("probe.rs");
        let probe_exe = reviewer_root.join("probe.exe");
        std::fs::write(
            &probe_rs,
            r#"
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("");
    let target = args.get(1).map(PathBuf::from).expect("target path required");

    match mode {
        "read" => match fs::read_to_string(&target) {
            Ok(content) => {
                print!("{content}");
                std::process::exit(0);
            }
            Err(e) => {
                let code = e.raw_os_error().unwrap_or(1);
                eprintln!("READ_DENIED: {e} (code {code})");
                std::process::exit(code);
            }
        },
        "write" => match fs::write(&target, "ATTEMPTED_WRITE") {
            Ok(_) => {
                eprintln!("WRITE_UNEXPECTEDLY_SUCCEEDED");
                std::process::exit(0);
            }
            Err(e) => {
                let code = e.raw_os_error().unwrap_or(1);
                eprintln!("WRITE_DENIED: {e} (code {code})");
                std::process::exit(code);
            }
        },
        "connect" => match std::net::TcpStream::connect(target.to_string_lossy().as_ref()) {
            Ok(_stream) => {
                println!("NETWORK_CONNECTED");
                std::process::exit(0);
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("NETWORK_ACCESS_DENIED: {e} (code {:?})", e.raw_os_error());
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("NETWORK_PROBE_OTHER_ERROR: {e} (code {:?})", e.raw_os_error());
                std::process::exit(2);
            }
        },
        _ => std::process::exit(1),
    }
}
"#,
        )
        .unwrap();

        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let compile = std::process::Command::new(rustc)
            .arg("--edition=2021")
            .arg(&probe_rs)
            .arg("-o")
            .arg(&probe_exe)
            .output()
            .expect("rustc must compile probe executable");
        assert!(
            compile.status.success(),
            "compilation error: {}",
            String::from_utf8_lossy(&compile.stderr)
        );

        let loopback_listener = std::net::TcpListener::bind(("127.0.0.1", 0))
            .expect("loopback-only network fixture must bind");
        let loopback_address = loopback_listener
            .local_addr()
            .expect("loopback fixture must expose its ephemeral address")
            .to_string();

        let sandbox_config = ReviewerSandboxConfig::new(reviewer_root.clone()).unwrap();
        let config_path = codex_home.join("config.toml");
        std::fs::write(&config_path, sandbox_config.render_config_toml().unwrap()).unwrap();

        let runner = ProcessRunner::new();
        let baseline_env = sandbox_probe_environment(&codex_home, &sqlite_home, fixture_root);

        // Alias and traversal targets:
        let lexical_outside = reviewer_root
            .join("..")
            .join("outside")
            .join("protected.txt");
        let symlink_file_path = reviewer_root.join("symlink_outside.txt");
        let symlink_status = std::os::windows::fs::symlink_file(&outside_file, &symlink_file_path);
        assert!(
            symlink_status.is_ok(),
            "[UNVERIFIED] Required file-symlink fixture could not be created: {}",
            symlink_status.as_ref().unwrap_err()
        );

        let junction_dir_path = reviewer_root.join("junction_outside");
        let junction_status = std::os::windows::fs::symlink_dir(&outside_root, &junction_dir_path);
        assert!(
            junction_status.is_ok(),
            "[UNVERIFIED] Required directory-junction fixture could not be created: {}",
            junction_status.as_ref().unwrap_err()
        );
        let junction_outside_file = junction_dir_path.join("protected.txt");

        let write_target = reviewer_root.join("write-probe.txt");

        let assert_fixtures_unchanged = || {
            let cur_allowed =
                std::fs::read(&allowed_file).expect("allowed fixture must remain readable");
            assert_eq!(
                cur_allowed, allowed_bytes,
                "allowed fixture content was modified"
            );
            assert_eq!(
                hash_bytes(&cur_allowed),
                initial_allowed_hash,
                "allowed fixture hash was modified"
            );

            let cur_outside =
                std::fs::read(&outside_file).expect("outside fixture must remain readable");
            assert_eq!(
                cur_outside, outside_bytes,
                "outside fixture content was modified"
            );
            assert_eq!(
                hash_bytes(&cur_outside),
                initial_outside_hash,
                "outside fixture hash was modified"
            );

            assert!(
                !write_target.exists(),
                "write target file was unexpectedly created"
            );
        };

        // Positive baseline control 1: Unsandboxed direct read succeeds
        let baseline_read = runner
            .run_with_stdin_in_environment(
                probe_exe.to_str().unwrap(),
                &["read".into(), allowed_file.to_str().unwrap().into()],
                Some(&reviewer_root),
                Some(&[]),
                Duration::from_secs(10),
                None,
                &baseline_env,
            )
            .await
            .expect("unsandboxed baseline read must run");
        assert_eq!(
            baseline_read.exit_code,
            Some(0),
            "baseline read stdout: '{}', stderr: '{}'",
            baseline_read.stdout.text,
            baseline_read.stderr.text
        );
        assert!(baseline_read.stdout.text.contains("PCT_ALLOWED_FIXTURE"));

        // Positive baseline control 2: Unsandboxed direct write succeeds
        let baseline_write_file = reviewer_root.join("baseline-write.txt");
        let baseline_write = runner
            .run_with_stdin_in_environment(
                probe_exe.to_str().unwrap(),
                &["write".into(), baseline_write_file.to_str().unwrap().into()],
                Some(&reviewer_root),
                Some(&[]),
                Duration::from_secs(10),
                None,
                &baseline_env,
            )
            .await
            .expect("unsandboxed baseline write must run");
        assert_eq!(baseline_write.exit_code, Some(0));
        assert!(baseline_write_file.exists());
        std::fs::remove_file(&baseline_write_file)
            .expect("baseline write fixture must be removable");
        assert!(
            std::fs::symlink_metadata(&baseline_write_file)
                .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
            "baseline write fixture must be absent after cleanup"
        );

        // Positive control: the synthetic child can connect to the live loopback fixture without MXC.
        let baseline_network = runner
            .run_with_stdin_in_environment(
                probe_exe.to_str().unwrap(),
                &["connect".into(), loopback_address.clone()],
                Some(&reviewer_root),
                Some(&[]),
                Duration::from_secs(10),
                None,
                &baseline_env,
            )
            .await
            .expect("unsandboxed loopback connection control must run");
        assert_eq!(
            baseline_network.exit_code,
            Some(0),
            "unsandboxed loopback control must connect: stdout='{}', stderr='{}'",
            baseline_network.stdout.text,
            baseline_network.stderr.text
        );
        assert!(baseline_network.stdout.text.contains("NETWORK_CONNECTED"));

        // Positive baseline control 3: Unsandboxed outside read succeeds
        let baseline_outside = runner
            .run_with_stdin_in_environment(
                probe_exe.to_str().unwrap(),
                &["read".into(), outside_file.to_str().unwrap().into()],
                Some(&reviewer_root),
                Some(&[]),
                Duration::from_secs(10),
                None,
                &baseline_env,
            )
            .await
            .expect("unsandboxed outside read must run");
        assert_eq!(baseline_outside.exit_code, Some(0));
        assert!(baseline_outside.stdout.text.contains("PCT_OUTSIDE_FIXTURE"));

        // Positive baseline control 4: Unsandboxed lexical .. traversal read succeeds
        let baseline_lexical = runner
            .run_with_stdin_in_environment(
                probe_exe.to_str().unwrap(),
                &["read".into(), lexical_outside.to_str().unwrap().into()],
                Some(&reviewer_root),
                Some(&[]),
                Duration::from_secs(10),
                None,
                &baseline_env,
            )
            .await
            .expect("unsandboxed lexical traversal read must run");
        assert_eq!(baseline_lexical.exit_code, Some(0));
        assert!(baseline_lexical.stdout.text.contains("PCT_OUTSIDE_FIXTURE"));

        // Positive baseline control 5: Unsandboxed symlink read succeeds.
        if symlink_status.is_ok() {
            let baseline_symlink = runner
                .run_with_stdin_in_environment(
                    probe_exe.to_str().unwrap(),
                    &["read".into(), symlink_file_path.to_str().unwrap().into()],
                    Some(&reviewer_root),
                    Some(&[]),
                    Duration::from_secs(10),
                    None,
                    &baseline_env,
                )
                .await
                .expect("unsandboxed symlink read must run");
            assert_eq!(baseline_symlink.exit_code, Some(0));
            assert!(baseline_symlink.stdout.text.contains("PCT_OUTSIDE_FIXTURE"));
        }

        // Positive baseline control 6: Unsandboxed junction read succeeds.
        if junction_status.is_ok() {
            let baseline_junction = runner
                .run_with_stdin_in_environment(
                    probe_exe.to_str().unwrap(),
                    &[
                        "read".into(),
                        junction_outside_file.to_str().unwrap().into(),
                    ],
                    Some(&reviewer_root),
                    Some(&[]),
                    Duration::from_secs(10),
                    None,
                    &baseline_env,
                )
                .await
                .expect("unsandboxed junction read must run");
            assert_eq!(baseline_junction.exit_code, Some(0));
            assert!(baseline_junction
                .stdout
                .text
                .contains("PCT_OUTSIDE_FIXTURE"));
        }

        assert_fixtures_unchanged();

        // 1. Sandboxed positive probe: the explicitly registered bundle root is readable under MXC sandbox.
        let allowed = run_windows_sandbox_probe(
            &runner,
            &codex_cli,
            &codex_home,
            &sqlite_home,
            fixture_root,
            &sandbox_config,
            probe_exe.to_str().unwrap(),
            &["read".into(), allowed_file.to_str().unwrap().into()],
            Duration::from_secs(20),
        )
        .await;
        assert_eq!(
            allowed.exit_code,
            Some(0),
            "allowed-read stderr: {}",
            allowed.stderr.text
        );
        assert!(!allowed.timed_out && !allowed.stdout.is_truncated && !allowed.stderr.is_truncated);
        assert!(allowed.stdout.text.contains("PCT_ALLOWED_FIXTURE"));
        assert_fixtures_unchanged();

        // Sandboxed negative control: the same child must receive an access-denied socket error.
        let network_res = run_windows_sandbox_probe(
            &runner,
            &codex_cli,
            &codex_home,
            &sqlite_home,
            fixture_root,
            &sandbox_config,
            probe_exe.to_str().unwrap(),
            &["connect".into(), loopback_address],
            Duration::from_secs(10),
        )
        .await;
        assert_eq!(
            network_res.exit_code,
            Some(1),
            "sandboxed loopback must be denied with PermissionDenied: stdout='{}', stderr='{}'",
            network_res.stdout.text,
            network_res.stderr.text
        );
        assert!(
            !network_res.timed_out
                && !network_res.stdout.is_truncated
                && !network_res.stderr.is_truncated
        );
        assert!(network_res.stderr.text.contains("NETWORK_ACCESS_DENIED"));
        assert!(!network_res
            .stderr
            .text
            .contains("NETWORK_PROBE_OTHER_ERROR"));
        assert!(!network_res.stdout.text.contains("NETWORK_CONNECTED"));
        assert_fixtures_unchanged();

        // 2. Sandboxed negative probe: the reviewer workspace is read-only and write must fail with Win32 access denied.
        let write_res = run_windows_sandbox_probe(
            &runner,
            &codex_cli,
            &codex_home,
            &sqlite_home,
            fixture_root,
            &sandbox_config,
            probe_exe.to_str().unwrap(),
            &["write".into(), write_target.to_str().unwrap().into()],
            Duration::from_secs(20),
        )
        .await;
        assert_eq!(
            write_res.exit_code,
            Some(5),
            "write probe must fail with Win32 ERROR_ACCESS_DENIED (code 5): stdout='{}', stderr='{}'",
            write_res.stdout.text,
            write_res.stderr.text
        );
        assert!(write_res.stderr.text.contains("WRITE_DENIED"));
        assert!(!write_target.exists(), "denied write created its target");
        assert_fixtures_unchanged();

        // 3. Sandboxed negative probe: a sibling fixture outside the readable root is denied.
        let outside_res = run_windows_sandbox_probe(
            &runner,
            &codex_cli,
            &codex_home,
            &sqlite_home,
            fixture_root,
            &sandbox_config,
            probe_exe.to_str().unwrap(),
            &["read".into(), outside_file.to_str().unwrap().into()],
            Duration::from_secs(20),
        )
        .await;
        assert_eq!(
            outside_res.exit_code,
            Some(5),
            "outside read probe must fail with Win32 ERROR_ACCESS_DENIED (code 5): stdout='{}', stderr='{}'",
            outside_res.stdout.text,
            outside_res.stderr.text
        );
        assert!(outside_res.stderr.text.contains("READ_DENIED"));
        assert!(!outside_res.stdout.text.contains("PCT_OUTSIDE_FIXTURE"));
        assert_fixtures_unchanged();

        // 4. Sandboxed negative probe: lexical .. path traversal escape is denied under MXC.
        let lexical_res = run_windows_sandbox_probe(
            &runner,
            &codex_cli,
            &codex_home,
            &sqlite_home,
            fixture_root,
            &sandbox_config,
            probe_exe.to_str().unwrap(),
            &["read".into(), lexical_outside.to_str().unwrap().into()],
            Duration::from_secs(20),
        )
        .await;
        assert_eq!(
            lexical_res.exit_code,
            Some(5),
            "lexical traversal probe must fail with Win32 ERROR_ACCESS_DENIED (code 5): stdout='{}', stderr='{}'",
            lexical_res.stdout.text,
            lexical_res.stderr.text
        );
        assert!(lexical_res.stderr.text.contains("READ_DENIED"));
        assert!(!lexical_res.stdout.text.contains("PCT_OUTSIDE_FIXTURE"));
        assert_fixtures_unchanged();

        // 5. Sandboxed negative probe: symbolic-link alias escape is denied under MXC.
        match &symlink_status {
            Ok(_) => {
                let symlink_res = run_windows_sandbox_probe(
                    &runner,
                    &codex_cli,
                    &codex_home,
                    &sqlite_home,
                    fixture_root,
                    &sandbox_config,
                    probe_exe.to_str().unwrap(),
                    &["read".into(), symlink_file_path.to_str().unwrap().into()],
                    Duration::from_secs(20),
                )
                .await;
                assert_eq!(
                    symlink_res.exit_code,
                    Some(5),
                    "symlink escape probe must fail with Win32 ERROR_ACCESS_DENIED (code 5): stdout='{}', stderr='{}'",
                    symlink_res.stdout.text,
                    symlink_res.stderr.text
                );
                assert!(symlink_res.stderr.text.contains("READ_DENIED"));
                assert!(!symlink_res.stdout.text.contains("PCT_OUTSIDE_FIXTURE"));
            }
            Err(e) => {
                panic!("[UNVERIFIED] Required file-symlink fixture is unavailable: {e}");
            }
        }
        assert_fixtures_unchanged();

        // 6. Sandboxed negative probe: directory-junction alias escape is denied under MXC.
        match &junction_status {
            Ok(_) => {
                let junction_res = run_windows_sandbox_probe(
                    &runner,
                    &codex_cli,
                    &codex_home,
                    &sqlite_home,
                    fixture_root,
                    &sandbox_config,
                    probe_exe.to_str().unwrap(),
                    &[
                        "read".into(),
                        junction_outside_file.to_str().unwrap().into(),
                    ],
                    Duration::from_secs(20),
                )
                .await;
                assert_eq!(
                    junction_res.exit_code,
                    Some(5),
                    "junction escape probe must fail with Win32 ERROR_ACCESS_DENIED (code 5): stdout='{}', stderr='{}'",
                    junction_res.stdout.text,
                    junction_res.stderr.text
                );
                assert!(junction_res.stderr.text.contains("READ_DENIED"));
                assert!(!junction_res.stdout.text.contains("PCT_OUTSIDE_FIXTURE"));
            }
            Err(e) => {
                panic!("[UNVERIFIED] Required directory-junction fixture is unavailable: {e}");
            }
        }
        assert_fixtures_unchanged();

        // Cleanup links
        if symlink_status.is_ok() {
            std::fs::remove_file(&symlink_file_path)
                .expect("created symlink fixture must be removable");
            assert!(
                std::fs::symlink_metadata(&symlink_file_path)
                    .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
                "created symlink fixture must be absent after cleanup"
            );
        }
        if junction_status.is_ok() {
            std::fs::remove_dir(&junction_dir_path)
                .expect("created junction fixture must be removable");
            assert!(
                std::fs::symlink_metadata(&junction_dir_path)
                    .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
                "created junction fixture must be absent after cleanup"
            );
        }

        // Final fixture integrity assertion
        assert_fixtures_unchanged();

        // Assert that outside directory contains only the original protected file and no unexpected files
        let outside_entries: Vec<_> = std::fs::read_dir(&outside_root)
            .unwrap()
            .map(|r| r.unwrap().file_name())
            .collect();
        assert_eq!(
            outside_entries,
            vec![std::ffi::OsString::from("protected.txt")]
        );

        // Close explicitly so a successful test cannot hide TempDir cleanup failure.
        let _ = assert_fixtures_unchanged;
        temp.close()
            .expect("complete disposable MXC fixture tree must be removable");
    }

    #[test]
    fn codex_paths_reject_nested_working_directory_or_runtime_stores() {
        let dir = tempdir().unwrap();
        let mut paths = fixture_paths(dir.path());
        paths.working_dir = paths.codex_home.join("work");
        std::fs::create_dir_all(&paths.working_dir).unwrap();
        assert!(paths.validate().is_err());
    }

    #[test]
    fn reviewer_sandbox_backend_rejects_weaker_or_unsupported_fallbacks() {
        assert_eq!(
            WindowsSandboxBackend::parse("mxc").unwrap(),
            WindowsSandboxBackend::Mxc
        );
        assert_eq!(
            WindowsSandboxBackend::parse("  MXC  ").unwrap(),
            WindowsSandboxBackend::Mxc
        );

        let unelevated_err = WindowsSandboxBackend::parse("unelevated").unwrap_err();
        assert!(
            unelevated_err.contains("weaker isolation does not satisfy reviewer read/write denial")
        );

        let elevated_err = WindowsSandboxBackend::parse("elevated").unwrap_err();
        assert!(elevated_err.contains("unsupported"));

        assert!(WindowsSandboxBackend::parse("auto").is_err());
        assert!(WindowsSandboxBackend::parse("").is_err());
    }

    #[test]
    fn reviewer_sandbox_config_preflight_validates_and_rejects_invalid_roots() {
        let dir = tempdir().unwrap();
        let bundle_root = dir.path().join("bundle");
        std::fs::create_dir_all(&bundle_root).unwrap();

        let config = ReviewerSandboxConfig::new(bundle_root.clone()).unwrap();
        assert_eq!(config.backend, WindowsSandboxBackend::Mxc);
        assert_eq!(config.profile_name, "pct-reviewer");

        let ws_root = dir.path().join("workspace");
        let auth_root = dir.path().join("engine-auth");
        std::fs::create_dir_all(&ws_root).unwrap();
        std::fs::create_dir_all(&auth_root).unwrap();
        config
            .preflight_check(Some(&ws_root), Some(&auth_root))
            .unwrap();

        // Rejects collision where bundle_root is inside workspace
        let inside_ws = ws_root.join("bundle-inside");
        std::fs::create_dir_all(&inside_ws).unwrap();
        let bad_config = ReviewerSandboxConfig::new(inside_ws).unwrap();
        assert!(bad_config.preflight_check(Some(&ws_root), None).is_err());

        // Rejects collision where bundle_root is inside engine authority dir
        let inside_auth = auth_root.join("bundle-inside");
        std::fs::create_dir_all(&inside_auth).unwrap();
        let bad_config2 = ReviewerSandboxConfig::new(inside_auth).unwrap();
        assert!(bad_config2.preflight_check(None, Some(&auth_root)).is_err());

        // Rejects collision where bundle_root contains workspace
        let parent_ws = dir.path().join("bundle-parent-ws");
        let child_ws = parent_ws.join("ws");
        std::fs::create_dir_all(&child_ws).unwrap();
        let bad_config3 = ReviewerSandboxConfig::new(parent_ws).unwrap();
        assert!(bad_config3.preflight_check(Some(&child_ws), None).is_err());

        // Rejects collision where bundle_root contains engine authority dir
        let parent_auth = dir.path().join("bundle-parent-auth");
        let child_auth = parent_auth.join("auth");
        std::fs::create_dir_all(&child_auth).unwrap();
        let bad_config4 = ReviewerSandboxConfig::new(parent_auth).unwrap();
        assert!(bad_config4
            .preflight_check(None, Some(&child_auth))
            .is_err());

        // Rejects equality with workspace or auth root
        let bad_config_eq_ws = ReviewerSandboxConfig::new(ws_root.clone()).unwrap();
        assert!(bad_config_eq_ws
            .preflight_check(Some(&ws_root), None)
            .is_err());

        let bad_config_eq_auth = ReviewerSandboxConfig::new(auth_root.clone()).unwrap();
        assert!(bad_config_eq_auth
            .preflight_check(None, Some(&auth_root))
            .is_err());

        // Rejects unresolvable / nonexistent boundary roots
        assert!(config
            .preflight_check(Some(&dir.path().join("nonexistent_ws")), None)
            .is_err());
        assert!(config
            .preflight_check(None, Some(&dir.path().join("nonexistent_auth")))
            .is_err());

        // Rejects nonexistent bundle root
        assert!(ReviewerSandboxConfig::new(dir.path().join("nonexistent")).is_err());

        // Rejects path traversal
        assert!(ReviewerSandboxConfig::new(dir.path().join("..").join("traversal")).is_err());
    }

    #[test]
    fn reviewer_sandbox_config_renders_strict_denial_toml() {
        let dir = tempdir().unwrap();
        let bundle_root = dir.path().join("bundle");
        std::fs::create_dir_all(&bundle_root).unwrap();

        let config = ReviewerSandboxConfig::new(bundle_root.clone()).unwrap();
        let toml = config.render_config_toml().unwrap();

        assert!(toml.contains("default_permissions = \"pct-reviewer\""));
        assert!(toml.contains("[permissions.pct-reviewer.workspace_roots]"));
        assert!(toml.contains(":root\" = \"deny\""));
        assert!(toml.contains(":minimal\" = \"read\""));
        assert!(toml.contains("\".\" = \"read\""));
        assert!(toml.contains("enabled = false"));
        assert!(toml.contains("sandbox = \"mxc\""));
        // Explicit absence of optional tools
        assert!(!toml.contains("mcp_servers"));
        assert!(!toml.contains("web_search"));
    }

    #[test]
    fn reviewer_sandbox_builds_exact_cli_args() {
        let dir = tempdir().unwrap();
        let bundle_root = dir.path().join("bundle");
        std::fs::create_dir_all(&bundle_root).unwrap();

        let config = ReviewerSandboxConfig::new(bundle_root.clone()).unwrap();
        let args = config
            .build_sandbox_cli_args("powershell.exe", &["-Command".into(), "echo hi".into()])
            .unwrap();

        assert_eq!(args[0], "-c");
        assert_eq!(args[1], "windows.sandbox=mxc");
        assert_eq!(args[2], "sandbox");
        assert_eq!(args[3], "--permission-profile");
        assert_eq!(args[4], "pct-reviewer");
        assert_eq!(args[5], "--cd");
        assert_eq!(
            args[6],
            bundle_root.canonicalize().unwrap().to_str().unwrap()
        );
        assert_eq!(args[7], "--");
        assert_eq!(args[8], "powershell.exe");
        assert_eq!(args[9], "-Command");
        assert_eq!(args[10], "echo hi");
    }

    #[tokio::test]
    async fn local_fixture_process_runner_plumbing_helper_test() {
        let dir = tempdir().unwrap();
        let fixture_root = dir.path();
        let bundle_root = fixture_root.join("bundle-root");
        let outside_root = fixture_root.join("outside-root");
        let codex_home = fixture_root.join("codex-home");
        let sqlite_home = fixture_root.join("sqlite-home");

        for p in [&bundle_root, &outside_root, &codex_home, &sqlite_home] {
            std::fs::create_dir_all(p).unwrap();
        }

        let allowed_file = bundle_root.join("allowed.txt");
        std::fs::write(&allowed_file, "ALLOWED_CONTENT\n").unwrap();

        let source_path = fixture_root.join("stage1_fixture.rs");
        let exec_path = fixture_root.join("stage1_fixture.exe");
        std::fs::write(
            &source_path,
            r#"
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("");
    let target = args.get(1).map(PathBuf::from);

    match mode {
        "read" => {
            let path = target.expect("target required");
            match fs::read_to_string(&path) {
                Ok(content) => {
                    print!("{content}");
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("READ_ERROR: {e}");
                    std::process::exit(5);
                }
            }
        }
        "echo" => {
            let text = args.get(1).map(String::as_str).unwrap_or("");
            print!("{text}");
            std::process::exit(0);
        }
        _ => std::process::exit(1),
    }
}
"#,
        )
        .unwrap();

        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let compile = std::process::Command::new(rustc)
            .arg("--edition=2021")
            .arg(&source_path)
            .arg("-o")
            .arg(&exec_path)
            .output()
            .expect("rustc must compile stage1_fixture");
        assert!(
            compile.status.success(),
            "compilation error: {}",
            String::from_utf8_lossy(&compile.stderr)
        );

        let runner = ProcessRunner::new();
        let timeout = Duration::from_secs(10);
        let env = sandbox_probe_environment(&codex_home, &sqlite_home, fixture_root);

        // ProcessRunner plumbing verification: read fixture file
        let res_read = runner
            .run_with_stdin_in_environment(
                exec_path.to_str().unwrap(),
                &["read".into(), allowed_file.to_str().unwrap().into()],
                Some(&bundle_root),
                None,
                timeout,
                None,
                &env,
            )
            .await
            .unwrap();
        assert_eq!(res_read.exit_code, Some(0));
        assert!(res_read.stdout.text.contains("ALLOWED_CONTENT"));

        // ProcessRunner plumbing verification: argument passing and echo
        let res_echo = runner
            .run_with_stdin_in_environment(
                exec_path.to_str().unwrap(),
                &["echo".into(), "PLUMBING_OK".into()],
                Some(&bundle_root),
                None,
                timeout,
                None,
                &env,
            )
            .await
            .unwrap();
        assert_eq!(res_echo.exit_code, Some(0));
        assert!(res_echo.stdout.text.contains("PLUMBING_OK"));

        // Path containment helper verification: sibling outside root is not contained in bundle root
        let traversal_target = bundle_root
            .join("..")
            .join("outside-root")
            .join("secret.txt");
        assert!(
            !super::super::isolation::PlatformIsolationGuard::is_contained(
                &bundle_root,
                &traversal_target
            )
        );

        // Invariant test: production UnsupportedPlatform guard remains strictly fail-closed
        let _isolation_lock = super::super::isolation::TEST_ISOLATION_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        super::super::isolation::set_test_isolation_pass(false);
        let guard_check =
            super::super::isolation::PlatformIsolationGuard::verify_reviewer_os_boundary(
                &outside_root,
                &bundle_root,
            );
        assert!(
            guard_check.is_err(),
            "production guard must remain fail-closed without test pass override"
        );
        if let Err(e) = guard_check {
            assert!(matches!(
                e,
                super::super::isolation::IsolationError::UnsupportedPlatform(_)
            ));
        }
    }

    #[test]
    fn reviewer_launch_config_builds_expected_argv_and_dedicated_paths() {
        let dir = tempdir().unwrap();
        let ws = dir.path().join("workspace");
        let snap = dir.path().join("snapshot");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::create_dir_all(&snap).unwrap();
        let plan_file = snap.join("plan.md");
        std::fs::write(&plan_file, "# Test Plan").unwrap();

        let req = crate::orchestrator::auto_converge::agent_runner::RoleExecutionRequest {
            role: crate::orchestrator::auto_converge::isolation::AutoConvergeRole::Reviewer,
            session_id: "test-run-1-plan-reviewer-1".to_string(),
            workspace_root: ws.clone(),
            snapshot_dir: Some(snap.clone()),
            plan_path: plan_file,
            runtime_allowlist: vec!["src/lib.rs".to_string()],
            input_prompt: "Review plan against requirements".to_string(),
            timeout_seconds: 300,
            model: Some("gpt-5-codex".to_string()),
            reasoning_effort: Some("high".to_string()),
            run_id: Some("test-run-1".to_string()),
        };

        let launch =
            build_reviewer_launch_configuration(&req).expect("must build reviewer launch config");
        assert_eq!(launch.invocation.role, CodexRoleProfile::Reviewer);
        assert_eq!(launch.invocation.args[0], "exec");
        assert_eq!(launch.invocation.args[1], "--json");
        assert!(launch.invocation.args.contains(&"--profile".to_string()));
        assert!(launch.invocation.args.contains(&"reviewer".to_string()));
        assert!(launch
            .invocation
            .args
            .contains(&"--ignore-rules".to_string()));
        assert!(launch.invocation.args.contains(&"--model".to_string()));
        assert!(launch.invocation.args.contains(&"gpt-5-codex".to_string()));
        assert!(launch.invocation.args.contains(&"--config".to_string()));
        assert!(launch
            .invocation
            .args
            .contains(&"model_reasoning_effort=\"high\"".to_string()));
        assert!(launch
            .invocation
            .args
            .contains(&"--output-last-message".to_string()));
        assert!(launch
            .invocation
            .args
            .contains(&"--output-schema".to_string()));
        assert_eq!(launch.invocation.args.last().map(String::as_str), Some("-"));
        assert_eq!(launch.invocation.stdin, b"Review plan against requirements");
        assert!(
            launch.invocation.thread_id.is_none(),
            "Reviewers must always start fresh sessions"
        );

        // Verify dedicated config.toml
        let config_toml_path = launch.paths.codex_home.join("config.toml");
        assert!(config_toml_path.exists());
        let config_str = std::fs::read_to_string(&config_toml_path).unwrap();
        assert!(config_str.contains("default_permissions = \"reviewer\""));
        assert!(config_str.contains("\":root\" = \"deny\""));
        assert!(config_str.contains("\".\" = \"read\""));
        assert!(config_str.contains("enabled = false"));

        // Verify child environment sanitization: no credentials, has dedicated CODEX_HOME
        let env_keys: Vec<String> = launch
            .environment
            .iter()
            .map(|(k, _)| k.to_string_lossy().to_ascii_uppercase())
            .collect();
        assert!(env_keys.contains(&"CODEX_HOME".to_string()));
        assert!(env_keys.contains(&"CODEX_SQLITE_HOME".to_string()));
        for forbidden in ["API_KEY", "PROXY", "GIT_AUTHOR", "GITHUB_TOKEN"] {
            assert!(
                !env_keys.iter().any(|k| k.contains(forbidden)),
                "forbidden key present: {forbidden}"
            );
        }

        // Verify output schema exists and is valid
        let schema_val = read_bounded_json(
            &launch.paths.output_schema,
            MAX_CODEX_SCHEMA_BYTES,
            "schema",
        )
        .unwrap();
        validate_object_schema(&schema_val).unwrap();
        std::fs::remove_dir_all(&launch.paths.run_root).unwrap();
    }

    #[test]
    fn reviewer_config_creation_never_overwrites_existing_file() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let existing = b"user-owned config must remain unchanged\n";
        std::fs::write(&config_path, existing).unwrap();

        let result =
            create_reviewer_config_file(&config_path, "default_permissions = \\\"reviewer\\\"\\n");

        assert!(result.is_err(), "existing config must be rejected");
        assert_eq!(std::fs::read(&config_path).unwrap(), existing);

        let managed_path = dir.path().join("managed-config.toml");
        let managed = "default_permissions = \\\"reviewer\\\"\\n";
        create_reviewer_config_file(&managed_path, managed).unwrap();
        let before = std::fs::read(&managed_path).unwrap();
        create_reviewer_config_file(&managed_path, managed).unwrap();
        assert_eq!(std::fs::read(&managed_path).unwrap(), before);
    }

    #[test]
    fn test_resolve_codex_managed_package_root_valid_layout() {
        let dir = tempdir().unwrap();
        let (root, exe) = create_synthetic_codex_npm_layout(dir.path());
        let resolved = resolve_codex_managed_package_root(&exe).unwrap();
        assert_eq!(resolved, Some(root.clone()));

        // Also test with -win32-x64 version suffix in native package.json
        let native_pkg = root
            .join("node_modules")
            .join("@openai")
            .join("codex-win32-x64")
            .join("package.json");
        let native_json = serde_json::json!({
            "name": "@openai/codex-win32-x64",
            "version": "0.163.0-win32-x64"
        });
        std::fs::write(
            &native_pkg,
            serde_json::to_vec_pretty(&native_json).unwrap(),
        )
        .unwrap();
        let resolved2 = resolve_codex_managed_package_root(&exe).unwrap();
        assert_eq!(resolved2, Some(root));
    }

    #[test]
    fn test_resolve_codex_managed_package_root_npm_alias_contract() {
        let dir = tempdir().unwrap();
        let (root, exe) = create_synthetic_codex_npm_layout(dir.path());
        let native_pkg_path = root.join("node_modules/@openai/codex-win32-x64/package.json");
        let native_pkg = serde_json::json!({
            "name": "@openai/codex",
            "version": "0.163.0-win32-x64"
        });
        std::fs::write(
            &native_pkg_path,
            serde_json::to_vec_pretty(&native_pkg).unwrap(),
        )
        .unwrap();

        let root_pkg_path = root.join("package.json");
        let root_pkg = serde_json::json!({
            "name": "@openai/codex",
            "version": "0.163.0",
            "optionalDependencies": {
                "@openai/codex-win32-x64": "npm:@openai/codex@0.163.0-win32-x64"
            }
        });
        std::fs::write(
            &root_pkg_path,
            serde_json::to_vec_pretty(&root_pkg).unwrap(),
        )
        .unwrap();
        assert_eq!(
            resolve_codex_managed_package_root(&exe).unwrap(),
            Some(root.clone())
        );

        // An alias identity without the exact parent declaration must fail closed.
        let root_pkg_without_alias = serde_json::json!({
            "name": "@openai/codex",
            "version": "0.163.0"
        });
        std::fs::write(
            &root_pkg_path,
            serde_json::to_vec_pretty(&root_pkg_without_alias).unwrap(),
        )
        .unwrap();
        assert!(resolve_codex_managed_package_root(&exe).is_err());

        // A declared alias to another package/version must not bind this executable.
        let root_pkg_wrong_alias = serde_json::json!({
            "name": "@openai/codex",
            "version": "0.163.0",
            "optionalDependencies": {
                "@openai/codex-win32-x64": "npm:@openai/codex@0.164.0-win32-x64"
            }
        });
        std::fs::write(
            &root_pkg_path,
            serde_json::to_vec_pretty(&root_pkg_wrong_alias).unwrap(),
        )
        .unwrap();
        assert!(resolve_codex_managed_package_root(&exe).is_err());

        // The alias form requires the platform suffix and matching base version.
        let native_pkg_wrong_version = serde_json::json!({
            "name": "@openai/codex",
            "version": "0.164.0-win32-x64"
        });
        std::fs::write(
            &native_pkg_path,
            serde_json::to_vec_pretty(&native_pkg_wrong_version).unwrap(),
        )
        .unwrap();
        let root_pkg_matching_wrong_alias = serde_json::json!({
            "name": "@openai/codex",
            "version": "0.163.0",
            "optionalDependencies": {
                "@openai/codex-win32-x64": "npm:@openai/codex@0.164.0-win32-x64"
            }
        });
        std::fs::write(
            &root_pkg_path,
            serde_json::to_vec_pretty(&root_pkg_matching_wrong_alias).unwrap(),
        )
        .unwrap();
        assert!(resolve_codex_managed_package_root(&exe).is_err());
    }

    #[test]
    fn test_resolve_codex_managed_package_root_standalone() {
        let dir = tempdir().unwrap();
        let standalone_exe = dir.path().join("standalone_codex.exe");
        std::fs::write(&standalone_exe, b"not a packaged binary").unwrap();
        let resolved = resolve_codex_managed_package_root(&standalone_exe).unwrap();
        assert_eq!(resolved, None);

        // A relative standalone path must preserve the pre-resolver launch behavior.
        let relative_standalone = std::path::Path::new("codex.exe");
        assert_eq!(
            resolve_codex_managed_package_root(relative_standalone).unwrap(),
            None
        );
        let relative_managed = std::path::Path::new(
            "node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe",
        );
        assert!(resolve_codex_managed_package_root(relative_managed).is_err());
    }

    #[test]
    fn test_resolve_codex_managed_package_root_invalid_metadata_fails_closed() {
        // (a) Root package.json wrong name
        {
            let dir = tempdir().unwrap();
            let (root, exe) = create_synthetic_codex_npm_layout(dir.path());
            let wrong_root_pkg = serde_json::json!({
                "name": "@wrong/codex",
                "version": "0.163.0"
            });
            std::fs::write(
                root.join("package.json"),
                serde_json::to_vec(&wrong_root_pkg).unwrap(),
            )
            .unwrap();
            assert!(resolve_codex_managed_package_root(&exe).is_err());
        }
        // (b) Native package.json wrong name
        {
            let dir = tempdir().unwrap();
            let (root, exe) = create_synthetic_codex_npm_layout(dir.path());
            let wrong_native_pkg = serde_json::json!({
                "name": "@openai/codex-darwin-arm64",
                "version": "0.163.0"
            });
            let native_json_path = root.join("node_modules/@openai/codex-win32-x64/package.json");
            std::fs::write(
                native_json_path,
                serde_json::to_vec(&wrong_native_pkg).unwrap(),
            )
            .unwrap();
            assert!(resolve_codex_managed_package_root(&exe).is_err());
        }
        // (b2) The parent package identity is not valid for the native package directory.
        {
            let dir = tempdir().unwrap();
            let (root, exe) = create_synthetic_codex_npm_layout(dir.path());
            let wrong_native_pkg = serde_json::json!({
                "name": "@openai/codex",
                "version": "0.163.0"
            });
            let native_json_path = root.join("node_modules/@openai/codex-win32-x64/package.json");
            std::fs::write(
                native_json_path,
                serde_json::to_vec(&wrong_native_pkg).unwrap(),
            )
            .unwrap();
            assert!(resolve_codex_managed_package_root(&exe).is_err());
        }
        // (c) Version mismatch between root and native
        {
            let dir = tempdir().unwrap();
            let (root, exe) = create_synthetic_codex_npm_layout(dir.path());
            let mismatch_native_pkg = serde_json::json!({
                "name": "@openai/codex-win32-x64",
                "version": "0.164.0"
            });
            let native_json_path = root.join("node_modules/@openai/codex-win32-x64/package.json");
            std::fs::write(
                native_json_path,
                serde_json::to_vec(&mismatch_native_pkg).unwrap(),
            )
            .unwrap();
            assert!(resolve_codex_managed_package_root(&exe).is_err());
        }
        // (d) Version mismatch between root and codex-package.json
        {
            let dir = tempdir().unwrap();
            let (root, exe) = create_synthetic_codex_npm_layout(dir.path());
            let mismatch_codex_pkg = serde_json::json!({
                "version": "0.162.0",
                "target": "x86_64-pc-windows-msvc",
                "entrypoint": "bin/codex.exe"
            });
            let codex_pkg_path = root.join(
                "node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex-package.json",
            );
            std::fs::write(
                codex_pkg_path,
                serde_json::to_vec(&mismatch_codex_pkg).unwrap(),
            )
            .unwrap();
            assert!(resolve_codex_managed_package_root(&exe).is_err());
        }
        // (e) Target mismatch between directory name and codex-package.json
        {
            let dir = tempdir().unwrap();
            let (root, exe) = create_synthetic_codex_npm_layout(dir.path());
            let mismatch_target = serde_json::json!({
                "version": "0.163.0",
                "target": "aarch64-pc-windows-msvc",
                "entrypoint": "bin/codex.exe"
            });
            let codex_pkg_path = root.join(
                "node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex-package.json",
            );
            std::fs::write(
                codex_pkg_path,
                serde_json::to_vec(&mismatch_target).unwrap(),
            )
            .unwrap();
            assert!(resolve_codex_managed_package_root(&exe).is_err());
        }
        // (f) Entrypoint mismatch
        {
            let dir = tempdir().unwrap();
            let (root, exe) = create_synthetic_codex_npm_layout(dir.path());
            let mismatch_entrypoint = serde_json::json!({
                "version": "0.163.0",
                "target": "x86_64-pc-windows-msvc",
                "entrypoint": "bin/other.exe"
            });
            let codex_pkg_path = root.join(
                "node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex-package.json",
            );
            std::fs::write(
                codex_pkg_path,
                serde_json::to_vec(&mismatch_entrypoint).unwrap(),
            )
            .unwrap();
            assert!(resolve_codex_managed_package_root(&exe).is_err());
        }
        // (f2) Traversal, absolute, and ambiguous separator entrypoints are rejected.
        for invalid_entrypoint in ["../bin/codex.exe", "/bin/codex.exe", "bin\\codex.exe"] {
            let dir = tempdir().unwrap();
            let (root, exe) = create_synthetic_codex_npm_layout(dir.path());
            let invalid_metadata = serde_json::json!({
                "version": "0.163.0",
                "target": "x86_64-pc-windows-msvc",
                "entrypoint": invalid_entrypoint
            });
            let codex_pkg_path = root.join(
                "node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex-package.json",
            );
            std::fs::write(
                codex_pkg_path,
                serde_json::to_vec(&invalid_metadata).unwrap(),
            )
            .unwrap();
            assert!(
                resolve_codex_managed_package_root(&exe).is_err(),
                "invalid entrypoint {invalid_entrypoint:?} must fail closed"
            );
        }
        // (g) Malformed JSON
        {
            let dir = tempdir().unwrap();
            let (root, exe) = create_synthetic_codex_npm_layout(dir.path());
            std::fs::write(root.join("package.json"), b"not valid json").unwrap();
            assert!(resolve_codex_managed_package_root(&exe).is_err());
        }
        // (h) Appears packaged (contains 'vendor' in path) but missing parent structure
        {
            let dir = tempdir().unwrap();
            let fake_vendor = dir.path().join("vendor").join("bin");
            std::fs::create_dir_all(&fake_vendor).unwrap();
            let fake_exe = fake_vendor.join("codex.exe");
            std::fs::write(&fake_exe, b"fake").unwrap();
            assert!(resolve_codex_managed_package_root(&fake_exe).is_err());
        }
    }

    #[test]
    #[cfg(windows)]
    fn test_resolve_codex_managed_package_root_symlink_reparse_rejected() {
        let dir = tempdir().unwrap();
        let target_dir = dir.path().join("real_pkg");
        let (_root, _exe) = create_synthetic_codex_npm_layout(&target_dir);

        let junction_root = dir.path().join("junction_root");
        let output = std::process::Command::new("cmd")
            .args([
                "/c",
                "mklink",
                "/J",
                &junction_root.to_string_lossy(),
                &target_dir.to_string_lossy(),
            ])
            .output()
            .expect("[UNVERIFIED] failed to invoke cmd for mklink");
        if !output.status.success() {
            panic!(
                "[UNVERIFIED] could not create NTFS junction fixture: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let junction_exe = junction_root
            .join("node_modules")
            .join("@openai")
            .join("codex-win32-x64")
            .join("vendor")
            .join("x86_64-pc-windows-msvc")
            .join("bin")
            .join("codex.exe");

        let result = resolve_codex_managed_package_root(&junction_exe);

        // Clean up junction before assertions so tempdir deletion succeeds cleanly
        let _ = std::fs::remove_dir(&junction_root);

        assert!(
            result.is_err(),
            "resolving package root through a junction must fail closed"
        );
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("reparse point") || err_msg.contains("symlink"),
            "expected error mentioning reparse point or symlink, got: {err_msg}"
        );
    }

    #[test]
    #[cfg(windows)]
    fn test_resolve_codex_managed_package_root_reparse_ancestor_above_package_root_rejected() {
        let dir = tempdir().unwrap();
        let real_parent = dir.path().join("real_parent");
        let package_root = real_parent.join("codex-package");
        std::fs::create_dir_all(&real_parent).unwrap();
        let (_root, real_exe) = create_synthetic_codex_npm_layout(&package_root);

        let junction_parent = dir.path().join("junction_parent");
        let output = std::process::Command::new("cmd")
            .args([
                "/c",
                "mklink",
                "/J",
                &junction_parent.to_string_lossy(),
                &real_parent.to_string_lossy(),
            ])
            .output()
            .expect("[UNVERIFIED] failed to invoke cmd for ancestor junction fixture");
        if !output.status.success() {
            panic!(
                "[UNVERIFIED] could not create ancestor NTFS junction fixture: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let aliased_exe = junction_parent
            .join("codex-package")
            .join("node_modules")
            .join("@openai")
            .join("codex-win32-x64")
            .join("vendor")
            .join("x86_64-pc-windows-msvc")
            .join("bin")
            .join("codex.exe");
        assert!(real_exe.exists());
        let result = resolve_codex_managed_package_root(&aliased_exe);
        std::fs::remove_dir(&junction_parent).expect("remove only the created junction");
        assert!(
            result.is_err(),
            "a reparse point above the package root must fail closed"
        );
    }

    #[test]
    fn test_build_child_environment_with_executable_managed_root_and_case_uniqueness() {
        let dir = tempdir().unwrap();
        let codex_home = dir.path().join("home");
        let sqlite_home = dir.path().join("sqlite");
        std::fs::create_dir_all(&codex_home).unwrap();
        std::fs::create_dir_all(&sqlite_home).unwrap();

        let (root, exe) = create_synthetic_codex_npm_layout(&dir.path().join("pkg"));

        // Ambient lookup attempts to inject conflicting CODEX_MANAGED_PACKAGE_ROOT (and case variants)
        // and secrets
        let lookup = |key: &str| -> Option<std::ffi::OsString> {
            match key {
                "PATH" => Some(r"C:\Windows\System32".into()),
                "CODEX_MANAGED_PACKAGE_ROOT" | "codex_managed_package_root" => {
                    Some(r"C:\untrusted_ambient_root".into())
                }
                "OPENAI_API_KEY" => Some("secret".into()),
                _ => None,
            }
        };

        let env = build_child_environment_with_executable_from_lookup(
            "run-1",
            &codex_home,
            &sqlite_home,
            Some(&exe),
            &lookup,
        )
        .expect("build environment successfully");

        // Verify CODEX_MANAGED_PACKAGE_ROOT matches the synthetic package root, NOT the ambient value
        let mut managed_root_count = 0;
        let mut managed_root_val = None;
        for (k, v) in &env {
            let k_str = k.to_string_lossy();
            if k_str.to_ascii_uppercase() == "CODEX_MANAGED_PACKAGE_ROOT" {
                managed_root_count += 1;
                managed_root_val = Some(v.to_string_lossy().into_owned());
            }
            assert!(!k_str.contains("API_KEY"));
        }

        assert_eq!(
            managed_root_count, 1,
            "exactly one CODEX_MANAGED_PACKAGE_ROOT entry must exist"
        );
        assert_eq!(
            managed_root_val.unwrap(),
            root.to_str().unwrap(),
            "managed root must match derived package root"
        );

        // Standalone executable produces no CODEX_MANAGED_PACKAGE_ROOT
        let standalone_exe = dir.path().join("standalone.exe");
        std::fs::write(&standalone_exe, b"standalone").unwrap();
        let env_standalone = build_child_environment_with_executable_from_lookup(
            "run-1",
            &codex_home,
            &sqlite_home,
            Some(&standalone_exe),
            &lookup,
        )
        .expect("build standalone environment successfully");

        assert!(
            !env_standalone
                .iter()
                .any(|(k, _)| k.to_string_lossy().to_ascii_uppercase()
                    == "CODEX_MANAGED_PACKAGE_ROOT"),
            "standalone executable must not set CODEX_MANAGED_PACKAGE_ROOT"
        );

        // A clearly standalone relative executable remains compatible and does not
        // acquire a derived managed root.
        let env_relative_standalone = build_child_environment_with_executable_from_lookup(
            "run-1",
            &codex_home,
            &sqlite_home,
            Some(std::path::Path::new("codex.exe")),
            &lookup,
        )
        .expect("relative standalone executable keeps existing environment behavior");
        assert!(
            !env_relative_standalone.iter().any(|(k, _)| k
                .to_string_lossy()
                .eq_ignore_ascii_case("CODEX_MANAGED_PACKAGE_ROOT")),
            "relative standalone executable must not set CODEX_MANAGED_PACKAGE_ROOT"
        );
    }
}
