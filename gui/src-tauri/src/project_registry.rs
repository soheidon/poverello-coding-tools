use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub const PROJECT_REGISTRY_SCHEMA_VERSION: u32 = 1;

static PROJECT_REGISTRY_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectRecord {
    pub project_id: String,
    pub display_name: String,
    pub repo_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_path: Option<String>,
    /// Surface-specific plan associations. A present key with `None` explicitly clears
    /// that surface and prevents the legacy `plan_path` fallback from restoring it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub plan_paths: BTreeMap<String, Option<String>>,
    #[serde(default)]
    pub created_unix: u64,
    #[serde(default)]
    pub updated_unix: u64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl ProjectRecord {
    pub fn plan_path_for_surface(&self, surface: &str) -> Option<&str> {
        self.plan_paths
            .get(surface)
            .map(|value| value.as_deref())
            .unwrap_or_else(|| self.plan_path.as_deref())
    }

    fn set_plan_path_for_surface(
        &mut self,
        surface: &str,
        plan_path: Option<String>,
    ) -> Result<(), String> {
        let other_surface = match surface {
            "codex" => "antigravity",
            "antigravity" => "codex",
            _ => return Err(format!("Unsupported Autocode surface: '{surface}'")),
        };
        // Legacy plan_path was shared by both screens. Freeze its meaning for the
        // untouched surface before this surface diverges from it.
        if !self.plan_paths.contains_key(other_surface) {
            if let Some(legacy_path) = self.plan_path.clone() {
                self.plan_paths
                    .insert(other_surface.to_string(), Some(legacy_path));
            }
        }
        self.plan_paths.insert(surface.to_string(), plan_path);
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationConflictOption {
    pub source: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationConflict {
    pub conflict_id: String,
    pub kind: String,
    pub repo_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,
    pub description: String,
    pub options: Vec<MigrationConflictOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectRegistryState {
    pub schema_version: u32,
    #[serde(default)]
    pub projects: Vec<ProjectRecord>,
    #[serde(default)]
    pub migrated_from_legacy: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_conflicts: Vec<MigrationConflict>,
    #[serde(default)]
    pub last_updated_unix: u64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Default for ProjectRegistryState {
    fn default() -> Self {
        Self {
            schema_version: PROJECT_REGISTRY_SCHEMA_VERSION,
            projects: Vec::new(),
            migrated_from_legacy: false,
            pending_conflicts: Vec::new(),
            last_updated_unix: 0,
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedProjectContext {
    pub project_id: String,
    pub display_name: String,
    pub surface: String,
    pub repo_root: String,
    pub plan_path: String,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectRegistryView {
    pub schema_version: u32,
    pub projects: Vec<ProjectRecord>,
    pub pending_conflicts: Vec<MigrationConflict>,
    pub migrated_from_legacy: bool,
}

impl From<ProjectRegistryState> for ProjectRegistryView {
    fn from(s: ProjectRegistryState) -> Self {
        Self {
            schema_version: s.schema_version,
            projects: s.projects,
            pending_conflicts: s.pending_conflicts,
            migrated_from_legacy: s.migrated_from_legacy,
        }
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Strip Windows verbatim path prefixes (`\\?\` / `\\?\UNC\`) for clean UI display (`C:\...`).
pub fn strip_windows_verbatim_prefix(path: &str) -> String {
    let trimmed = path.trim();
    if let Some(unc) = trimmed.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else if let Some(rest) = trimmed.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Normalize a repository root for display & storage (strips `\\?\`, trims trailing separators).
pub fn normalize_repo_display_path(raw: &str) -> Result<String, String> {
    let stripped = strip_windows_verbatim_prefix(raw);
    if stripped.is_empty() {
        return Err("Repository path cannot be empty".to_string());
    }
    let path = Path::new(&stripped);
    let resolved = if path.exists() {
        let canon = path
            .canonicalize()
            .map_err(|e| format!("Failed to canonicalize repository '{}': {e}", stripped))?;
        strip_windows_verbatim_prefix(&canon.to_string_lossy())
    } else {
        stripped
    };
    let clean = resolved.trim_end_matches(['/', '\\']);
    if clean.len() == 2 && clean.ends_with(':') {
        Ok(format!("{clean}\\"))
    } else if clean.is_empty() {
        Ok(resolved)
    } else {
        Ok(clean.to_string())
    }
}

/// Canonical comparison key for repository roots (case-insensitive on Windows).
pub fn normalize_repo_key(raw: &str) -> String {
    let clean = strip_windows_verbatim_prefix(raw)
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();
    #[cfg(windows)]
    {
        clean.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        clean
    }
}

/// Stable deterministic opaque project ID derived from the normalized repository root.
pub fn derive_project_id(repo_root: &str) -> String {
    let key = normalize_repo_key(repo_root);
    let mut hasher = Sha256::new();
    hasher.update(b"poverello-project-v1:");
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("proj_{hex}")
}

pub fn normalize_plan_rel_path(raw: &str) -> Option<String> {
    let trimmed = raw.trim().replace('\\', "/");
    let stripped = trimmed.trim_start_matches("./").trim();
    if stripped.is_empty() {
        None
    } else {
        Some(stripped.to_string())
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn ensure_no_symlink_or_reparse(path: &Path) -> Result<(), String> {
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err(format!(
                "Refusing symlink or reparse point at {}",
                path.display()
            ));
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
            if (meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
                return Err(format!("Refusing reparse point at {}", path.display()));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Default)]
pub struct PersistenceHooks {
    pub fail_backup: bool,
    pub fail_write: bool,
    pub fail_readback: bool,
}

pub fn load_project_registry_at(registry_path: &Path) -> Result<ProjectRegistryState, String> {
    if !registry_path.exists() {
        return Ok(ProjectRegistryState::default());
    }
    ensure_no_symlink_or_reparse(registry_path)?;
    let raw = std::fs::read_to_string(registry_path).map_err(|e| {
        format!(
            "Failed to read project registry {}: {e}",
            registry_path.display()
        )
    })?;
    let mut state: ProjectRegistryState = serde_json::from_str(&raw).map_err(|e| {
        format!(
            "Failed to parse project registry {}: {e}",
            registry_path.display()
        )
    })?;
    for proj in &mut state.projects {
        proj.repo_root = strip_windows_verbatim_prefix(&proj.repo_root);
        proj.plan_path = proj.plan_path.as_deref().and_then(normalize_plan_rel_path);
        for path in proj.plan_paths.values_mut().flatten() {
            *path = normalize_plan_rel_path(path)
                .ok_or_else(|| "Surface plan path must be a repository-relative Markdown file".to_string())?;
        }
    }
    Ok(state)
}

pub fn save_project_registry_at(
    registry_path: &Path,
    state: &ProjectRegistryState,
) -> Result<ProjectRegistryState, String> {
    save_project_registry_at_with_hooks(registry_path, state, &PersistenceHooks::default())
}

pub fn save_project_registry_at_with_hooks(
    registry_path: &Path,
    state: &ProjectRegistryState,
    hooks: &PersistenceHooks,
) -> Result<ProjectRegistryState, String> {
    let _guard = PROJECT_REGISTRY_LOCK
        .lock()
        .map_err(|e| format!("Failed to acquire project registry lock: {e}"))?;
    save_project_registry_unlocked(registry_path, state, hooks)
}

fn save_project_registry_unlocked(
    registry_path: &Path,
    state: &ProjectRegistryState,
    hooks: &PersistenceHooks,
) -> Result<ProjectRegistryState, String> {
    let parent = registry_path.parent().ok_or_else(|| {
        format!(
            "Project registry path has no parent: {}",
            registry_path.display()
        )
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("Failed to create registry parent directory: {e}"))?;
    ensure_no_symlink_or_reparse(parent)?;
    ensure_no_symlink_or_reparse(registry_path)?;

    let mut next = state.clone();
    next.schema_version = PROJECT_REGISTRY_SCHEMA_VERSION;
    next.last_updated_unix = now_unix();

    // Validate unique project IDs and unique normalized repo roots
    let mut seen_ids = BTreeSet::new();
    let mut seen_repos = BTreeSet::new();
    for proj in &mut next.projects {
        proj.repo_root = strip_windows_verbatim_prefix(&proj.repo_root);
        proj.plan_path = proj.plan_path.as_deref().and_then(normalize_plan_rel_path);
        for path in proj.plan_paths.values_mut().flatten() {
            *path = normalize_plan_rel_path(path)
                .ok_or_else(|| "Surface plan path must be a repository-relative Markdown file".to_string())?;
        }
        if !seen_ids.insert(proj.project_id.clone()) {
            return Err(format!(
                "Duplicate project_id in registry: '{}'",
                proj.project_id
            ));
        }
        if !seen_repos.insert(normalize_repo_key(&proj.repo_root)) {
            return Err(format!(
                "Duplicate repository root in registry: '{}'",
                proj.repo_root
            ));
        }
    }

    let original_bytes = if registry_path.exists() {
        Some(
            std::fs::read(registry_path)
                .map_err(|e| format!("Failed to read existing project registry for backup: {e}"))?,
        )
    } else {
        None
    };

    let mut backup_path: Option<PathBuf> = None;
    if let Some(orig) = &original_bytes {
        if hooks.fail_backup {
            return Err("Backup creation failed (injected)".to_string());
        }
        let backups_dir = parent.join(".poverello_backups").join("project-registry");
        std::fs::create_dir_all(&backups_dir)
            .map_err(|e| format!("Failed to create registry backup directory: {e}"))?;
        let bp = backups_dir.join(format!(
            "project-registry.json.backup-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&bp, orig).map_err(|e| format!("Failed to write registry backup: {e}"))?;
        let bp_bytes =
            std::fs::read(&bp).map_err(|e| format!("Failed to verify registry backup: {e}"))?;
        if sha256_hex(&bp_bytes) != sha256_hex(orig) {
            let _ = std::fs::remove_file(&bp);
            return Err("Registry backup SHA-256 verification failed".to_string());
        }
        backup_path = Some(bp);
    }

    let serialized = serde_json::to_string_pretty(&next)
        .map_err(|e| format!("Failed to serialize project registry: {e}"))?
        + "\n";
    let expected_hash = sha256_hex(serialized.as_bytes());

    let staged = parent.join(format!(
        "project-registry.json.staged-{}",
        uuid::Uuid::new_v4()
    ));
    if hooks.fail_write {
        return Err("Registry staged write failed (injected)".to_string());
    }
    std::fs::write(&staged, serialized.as_bytes())
        .map_err(|e| format!("Failed to write staged project registry: {e}"))?;
    let staged_bytes = std::fs::read(&staged)
        .map_err(|e| format!("Failed to read back staged project registry: {e}"))?;
    if sha256_hex(&staged_bytes) != expected_hash {
        let _ = std::fs::remove_file(&staged);
        return Err("Staged project registry SHA-256 verification failed".to_string());
    }

    // Atomic replacement
    #[cfg(windows)]
    {
        if registry_path.exists() {
            let old_tmp = parent.join(format!(
                "project-registry.json.old-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::rename(registry_path, &old_tmp)
                .map_err(|e| format!("Failed to stage old registry for replace: {e}"))?;
            if let Err(e) = std::fs::rename(&staged, registry_path) {
                let _ = std::fs::rename(&old_tmp, registry_path);
                let _ = std::fs::remove_file(&staged);
                return Err(format!("Failed to commit staged project registry: {e}"));
            }
            let _ = std::fs::remove_file(&old_tmp);
        } else if let Err(e) = std::fs::rename(&staged, registry_path) {
            let _ = std::fs::remove_file(&staged);
            return Err(format!("Failed to commit staged project registry: {e}"));
        }
    }
    #[cfg(not(windows))]
    {
        if let Err(e) = std::fs::rename(&staged, registry_path) {
            let _ = std::fs::remove_file(&staged);
            return Err(format!("Failed to commit staged project registry: {e}"));
        }
    }

    // Post-commit readback verification with rollback on failure
    let readback_result: Result<ProjectRegistryState, String> = (|| {
        if hooks.fail_readback {
            return Err("Registry readback verification failed (injected)".to_string());
        }
        let committed_bytes = std::fs::read(registry_path)
            .map_err(|e| format!("Failed to read back committed project registry: {e}"))?;
        if sha256_hex(&committed_bytes) != expected_hash {
            return Err("Committed project registry SHA-256 mismatch".to_string());
        }
        load_project_registry_at(registry_path)
    })();

    match readback_result {
        Ok(verified) => Ok(verified),
        Err(err) => {
            if let Some(bp) = backup_path {
                let _ = std::fs::copy(&bp, registry_path);
            } else if original_bytes.is_none() {
                let _ = std::fs::remove_file(registry_path);
            }
            Err(format!("{err}; rolled back project registry"))
        }
    }
}

fn next_default_display_name(existing: &[ProjectRecord]) -> String {
    let taken: BTreeSet<String> = existing.iter().map(|p| p.display_name.clone()).collect();
    let mut n = 1usize;
    loop {
        let candidate = format!("Project {n}");
        if !taken.contains(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

pub fn register_or_update_project_at(
    registry_path: &Path,
    repo_path: &str,
    plan_path: Option<&str>,
    display_name: Option<&str>,
) -> Result<ProjectRegistryState, String> {
    let _guard = PROJECT_REGISTRY_LOCK
        .lock()
        .map_err(|e| format!("Failed to acquire project registry lock: {e}"))?;
    let mut state = load_project_registry_at(registry_path)?;
    let norm_repo = normalize_repo_display_path(repo_path)?;
    let key = normalize_repo_key(&norm_repo);
    let norm_plan = plan_path.and_then(normalize_plan_rel_path);
    let now = now_unix();

    let _project_id = if let Some(existing) = state
        .projects
        .iter_mut()
        .find(|p| normalize_repo_key(&p.repo_root) == key)
    {
        existing.repo_root = norm_repo;
        if let Some(plan) = norm_plan {
            existing.plan_path = Some(plan);
        }
        if let Some(name) = display_name.map(str::trim).filter(|s| !s.is_empty()) {
            existing.display_name = name.to_string();
        }
        existing.updated_unix = now;
        existing.project_id.clone()
    } else {
        let pid = derive_project_id(&norm_repo);
        let name = display_name
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| next_default_display_name(&state.projects));
        state.projects.push(ProjectRecord {
            project_id: pid.clone(),
            display_name: name,
            repo_root: norm_repo,
            plan_path: norm_plan,
            plan_paths: BTreeMap::new(),
            created_unix: now,
            updated_unix: now,
            extra: BTreeMap::new(),
        });
        pid
    };

    save_project_registry_unlocked(registry_path, &state, &PersistenceHooks::default())
}

pub fn set_project_plan_path_at(
    registry_path: &Path,
    project_id: &str,
    plan_path: Option<&str>,
) -> Result<ProjectRegistryState, String> {
    let _guard = PROJECT_REGISTRY_LOCK
        .lock()
        .map_err(|e| format!("Failed to acquire project registry lock: {e}"))?;
    let mut state = load_project_registry_at(registry_path)?;
    let proj = state
        .projects
        .iter_mut()
        .find(|p| p.project_id == project_id)
        .ok_or_else(|| format!("Project '{project_id}' is not registered"))?;
    proj.plan_path = plan_path.and_then(normalize_plan_rel_path);
    proj.updated_unix = now_unix();
    save_project_registry_unlocked(registry_path, &state, &PersistenceHooks::default())
}

pub fn register_or_update_project_for_surface_at(
    registry_path: &Path,
    surface: &str,
    repo_path: &str,
    plan_path: Option<&str>,
    display_name: Option<&str>,
) -> Result<ProjectRegistryState, String> {
    if !matches!(surface, "codex" | "antigravity") {
        return Err(format!("Unsupported Autocode surface: '{surface}'"));
    }
    let _guard = PROJECT_REGISTRY_LOCK
        .lock()
        .map_err(|e| format!("Failed to acquire project registry lock: {e}"))?;
    let mut state = load_project_registry_at(registry_path)?;
    let norm_repo = normalize_repo_display_path(repo_path)?;
    let key = normalize_repo_key(&norm_repo);
    let norm_plan = plan_path.and_then(normalize_plan_rel_path);
    let now = now_unix();
    if let Some(existing) = state
        .projects
        .iter_mut()
        .find(|project| normalize_repo_key(&project.repo_root) == key)
    {
        existing.repo_root = norm_repo;
        if plan_path.is_some() {
            existing.set_plan_path_for_surface(surface, norm_plan)?;
        }
        if let Some(name) = display_name.map(str::trim).filter(|name| !name.is_empty()) {
            existing.display_name = name.to_string();
        }
        existing.updated_unix = now;
    } else {
        let mut plan_paths = BTreeMap::new();
        if plan_path.is_some() {
            plan_paths.insert(surface.to_string(), norm_plan);
        }
        let project_id = derive_project_id(&norm_repo);
        let display_name = display_name
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| next_default_display_name(&state.projects));
        state.projects.push(ProjectRecord {
            project_id,
            display_name,
            repo_root: norm_repo,
            plan_path: None,
            plan_paths,
            created_unix: now,
            updated_unix: now,
            extra: BTreeMap::new(),
        });
    }
    save_project_registry_unlocked(registry_path, &state, &PersistenceHooks::default())
}

pub fn set_project_plan_path_for_surface_at(
    registry_path: &Path,
    project_id: &str,
    surface: &str,
    plan_path: Option<&str>,
) -> Result<ProjectRegistryState, String> {
    if !matches!(surface, "codex" | "antigravity") {
        return Err(format!("Unsupported Autocode surface: '{surface}'"));
    }
    let _guard = PROJECT_REGISTRY_LOCK
        .lock()
        .map_err(|e| format!("Failed to acquire project registry lock: {e}"))?;
    let mut state = load_project_registry_at(registry_path)?;
    let project = state
        .projects
        .iter_mut()
        .find(|project| project.project_id == project_id)
        .ok_or_else(|| format!("Project '{project_id}' is not registered"))?;
    project.set_plan_path_for_surface(
        surface,
        plan_path.and_then(normalize_plan_rel_path),
    )?;
    project.updated_unix = now_unix();
    save_project_registry_unlocked(registry_path, &state, &PersistenceHooks::default())
}

/// Replace one registered association with a plan selected from any repository.
/// The selected plan determines the destination repository; source removal and
/// destination upsert are committed as one registry transaction.
pub fn replace_project_plan_association_at(
    registry_path: &Path,
    source_project_id: &str,
    target_repo_path: &str,
    target_plan_path: &str,
) -> Result<ProjectRegistryState, String> {
    replace_project_plan_association_with_hooks(
        registry_path,
        source_project_id,
        target_repo_path,
        target_plan_path,
        &PersistenceHooks::default(),
    )
}

pub fn replace_project_plan_association_for_surface_at(
    registry_path: &Path,
    source_project_id: &str,
    target_repo_path: &str,
    target_plan_path: &str,
    surface: &str,
) -> Result<ProjectRegistryState, String> {
    replace_project_plan_association_for_surface_with_hooks(
        registry_path,
        source_project_id,
        target_repo_path,
        target_plan_path,
        surface,
        &PersistenceHooks::default(),
    )
}

fn replace_project_plan_association_for_surface_with_hooks(
    registry_path: &Path,
    source_project_id: &str,
    target_repo_path: &str,
    target_plan_path: &str,
    surface: &str,
    hooks: &PersistenceHooks,
) -> Result<ProjectRegistryState, String> {
    if !matches!(surface, "codex" | "antigravity") {
        return Err(format!("Unsupported Autocode surface: '{surface}'"));
    }
    let target_plan = normalize_plan_rel_path(target_plan_path)
        .ok_or_else(|| "Plan path cannot be empty".to_string())?;
    let segments: Vec<&str> = target_plan.split('/').collect();
    if target_plan.starts_with('/')
        || target_plan.ends_with('/')
        || !target_plan.to_ascii_lowercase().ends_with(".md")
        || segments.iter().any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return Err("Plan path must be a repository-relative Markdown file".to_string());
    }
    let _guard = PROJECT_REGISTRY_LOCK
        .lock()
        .map_err(|e| format!("Failed to acquire project registry lock: {e}"))?;
    let mut state = load_project_registry_at(registry_path)?;
    let source_index = state.projects.iter().position(|project| project.project_id == source_project_id)
        .ok_or_else(|| format!("Project '{source_project_id}' is not registered"))?;
    let target_repo = normalize_repo_display_path(target_repo_path)?;
    let target_key = normalize_repo_key(&target_repo);
    let source_key = normalize_repo_key(&state.projects[source_index].repo_root);
    if state.pending_conflicts.iter().any(|conflict| {
        let key = normalize_repo_key(&conflict.repo_root);
        key == source_key || key == target_key
    }) {
        return Err("Cannot replace a plan association while either repository has unresolved migration conflicts".to_string());
    }
    let target_indices: Vec<usize> = state.projects.iter().enumerate()
        .filter_map(|(index, project)| (normalize_repo_key(&project.repo_root) == target_key).then_some(index))
        .collect();
    if target_indices.len() > 1 {
        return Err(format!("Repository '{target_repo}' is registered more than once; resolve duplicates before switching plans"));
    }
    let other_surface = if surface == "codex" { "antigravity" } else { "codex" };
    let now = now_unix();
    if source_key == target_key {
        state.projects[source_index].repo_root = target_repo;
        state.projects[source_index].set_plan_path_for_surface(surface, Some(target_plan))?;
        state.projects[source_index].updated_unix = now;
    } else {
        // Clear this surface from the old repository. The setter freezes any legacy
        // shared value onto the opposite surface before diverging.
        state.projects[source_index].set_plan_path_for_surface(surface, None)?;
        state.projects[source_index].updated_unix = now;
        let remove_source = state.projects[source_index]
            .plan_path_for_surface(other_surface)
            .is_none();
        if let Some(target_index) = target_indices.first().copied() {
            state.projects[target_index].repo_root = target_repo;
            state.projects[target_index].set_plan_path_for_surface(surface, Some(target_plan))?;
            state.projects[target_index].updated_unix = now;
        } else {
            let mut plan_paths = BTreeMap::new();
            plan_paths.insert(surface.to_string(), Some(target_plan));
            let project_id = derive_project_id(&target_repo);
            let projects_for_name: Vec<ProjectRecord> = if remove_source {
                state
                    .projects
                    .iter()
                    .enumerate()
                    .filter_map(|(index, project)| (index != source_index).then_some(project.clone()))
                    .collect()
            } else {
                state.projects.clone()
            };
            let display_name = next_default_display_name(&projects_for_name);
            state.projects.push(ProjectRecord {
                project_id,
                display_name,
                repo_root: target_repo,
                plan_path: None,
                plan_paths,
                created_unix: now,
                updated_unix: now,
                extra: BTreeMap::new(),
            });
        }
        if remove_source {
            state.projects.remove(source_index);
        }
    }
    save_project_registry_unlocked(registry_path, &state, hooks)
}

pub fn unregister_project_surface_at(
    registry_path: &Path,
    project_id: &str,
    surface: &str,
) -> Result<ProjectRegistryState, String> {
    if !matches!(surface, "codex" | "antigravity") {
        return Err(format!("Unsupported Autocode surface: '{surface}'"));
    }
    let _guard = PROJECT_REGISTRY_LOCK
        .lock()
        .map_err(|e| format!("Failed to acquire project registry lock: {e}"))?;
    let mut state = load_project_registry_at(registry_path)?;
    let index = state.projects.iter().position(|project| project.project_id == project_id)
        .ok_or_else(|| format!("Project '{project_id}' is not registered"))?;
    let other_surface = if surface == "codex" { "antigravity" } else { "codex" };
    state.projects[index].set_plan_path_for_surface(surface, None)?;
    state.projects[index].updated_unix = now_unix();
    if state.projects[index].plan_path_for_surface(other_surface).is_none() {
        state.projects.remove(index);
    }
    save_project_registry_unlocked(registry_path, &state, &PersistenceHooks::default())
}

fn replace_project_plan_association_with_hooks(
    registry_path: &Path,
    source_project_id: &str,
    target_repo_path: &str,
    target_plan_path: &str,
    hooks: &PersistenceHooks,
) -> Result<ProjectRegistryState, String> {
    let target_plan = normalize_plan_rel_path(target_plan_path)
        .ok_or_else(|| "Plan path cannot be empty".to_string())?;
    let segments: Vec<&str> = target_plan.split('/').collect();
    if target_plan.starts_with('/')
        || target_plan.ends_with('/')
        || !target_plan.to_ascii_lowercase().ends_with(".md")
        || segments.iter().any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return Err("Plan path must be a repository-relative Markdown file".to_string());
    }

    let _guard = PROJECT_REGISTRY_LOCK
        .lock()
        .map_err(|e| format!("Failed to acquire project registry lock: {e}"))?;
    let mut state = load_project_registry_at(registry_path)?;
    let source_indices: Vec<usize> = state
        .projects
        .iter()
        .enumerate()
        .filter_map(|(index, project)| (project.project_id == source_project_id).then_some(index))
        .collect();
    if source_indices.len() != 1 {
        return Err(if source_indices.is_empty() {
            format!("Project '{source_project_id}' is not registered")
        } else {
            format!("Project ID '{source_project_id}' is duplicated; resolve the registry before switching plans")
        });
    }
    let source_index = source_indices[0];
    let target_repo = normalize_repo_display_path(target_repo_path)?;
    let target_key = normalize_repo_key(&target_repo);
    let source_key = normalize_repo_key(&state.projects[source_index].repo_root);

    if state.pending_conflicts.iter().any(|conflict| {
        let key = normalize_repo_key(&conflict.repo_root);
        key == source_key || key == target_key
    }) {
        return Err("Cannot replace a plan association while either repository has unresolved migration conflicts".to_string());
    }

    let now = now_unix();
    let target_indices: Vec<usize> = state
        .projects
        .iter()
        .enumerate()
        .filter_map(|(index, project)| (normalize_repo_key(&project.repo_root) == target_key).then_some(index))
        .collect();
    if target_indices.len() > 1 {
        return Err(format!("Repository '{target_repo}' is registered more than once; resolve duplicates before switching plans"));
    }
    if source_key == target_key {
        if target_indices.first().copied() != Some(source_index) {
            return Err(format!("Repository '{target_repo}' is associated with a different project ID; resolve the registry before switching plans"));
        }
        state.projects[source_index].repo_root = target_repo;
        state.projects[source_index].plan_path = Some(target_plan);
        state.projects[source_index].updated_unix = now;
    } else {
        if let Some(index) = target_indices.first().copied() {
            state.projects[index].repo_root = target_repo;
            state.projects[index].plan_path = Some(target_plan);
            state.projects[index].updated_unix = now;
            state.projects.remove(source_index);
        } else {
            state.projects.remove(source_index);
            let display_name = next_default_display_name(&state.projects);
            state.projects.push(ProjectRecord {
                project_id: derive_project_id(&target_repo),
                display_name,
                repo_root: target_repo,
                plan_path: Some(target_plan),
                plan_paths: BTreeMap::new(),
                created_unix: now,
                updated_unix: now,
                extra: BTreeMap::new(),
            });
        }
    }

    save_project_registry_unlocked(registry_path, &state, hooks)
}

pub fn rename_project_at(
    registry_path: &Path,
    project_id: &str,
    display_name: &str,
) -> Result<ProjectRegistryState, String> {
    let clean = display_name.trim();
    if clean.is_empty() {
        return Err("Project display name cannot be empty".to_string());
    }
    let _guard = PROJECT_REGISTRY_LOCK
        .lock()
        .map_err(|e| format!("Failed to acquire project registry lock: {e}"))?;
    let mut state = load_project_registry_at(registry_path)?;
    let proj = state
        .projects
        .iter_mut()
        .find(|p| p.project_id == project_id)
        .ok_or_else(|| format!("Project '{project_id}' is not registered"))?;
    proj.display_name = clean.to_string();
    proj.updated_unix = now_unix();
    save_project_registry_unlocked(registry_path, &state, &PersistenceHooks::default())
}

pub fn reorder_projects_at(
    registry_path: &Path,
    ordered_project_ids: &[String],
) -> Result<ProjectRegistryState, String> {
    let _guard = PROJECT_REGISTRY_LOCK
        .lock()
        .map_err(|e| format!("Failed to acquire project registry lock: {e}"))?;
    let mut state = load_project_registry_at(registry_path)?;
    if ordered_project_ids.len() != state.projects.len() {
        return Err("Ordered project list length does not match registered projects".to_string());
    }
    let mut by_id: BTreeMap<String, ProjectRecord> = state
        .projects
        .drain(..)
        .map(|p| (p.project_id.clone(), p))
        .collect();
    let mut reordered = Vec::with_capacity(ordered_project_ids.len());
    for pid in ordered_project_ids {
        let proj = by_id
            .remove(pid)
            .ok_or_else(|| format!("Unknown project_id '{pid}' in reorder request"))?;
        reordered.push(proj);
    }
    state.projects = reordered;
    save_project_registry_unlocked(registry_path, &state, &PersistenceHooks::default())
}

/// Unregister a project without deleting repository, plan, or AgentOpt files.
/// Any surface selecting the removed project becomes unselected (None), never auto-switching.
pub fn unregister_project_at(
    registry_path: &Path,
    project_id: &str,
) -> Result<ProjectRegistryState, String> {
    let _guard = PROJECT_REGISTRY_LOCK
        .lock()
        .map_err(|e| format!("Failed to acquire project registry lock: {e}"))?;
    let mut state = load_project_registry_at(registry_path)?;
    let before_len = state.projects.len();
    state.projects.retain(|p| p.project_id != project_id);
    if state.projects.len() == before_len {
        return Err(format!("Project '{project_id}' is not registered"));
    }
    save_project_registry_unlocked(registry_path, &state, &PersistenceHooks::default())
}

/// Read-only project context resolver shared by both Autocode UIs, backend workflow commands,
/// and direct Skill invocations. Resolves strictly from the task's repository/workspace without
/// any global or per-surface active-project selection.
pub fn resolve_project_context_for_repo_at(
    registry_path: &Path,
    surface: &str,
    repo_path: &Path,
) -> Result<ResolvedProjectContext, String> {
    if !matches!(surface, "codex" | "antigravity") {
        return Err(format!("Unsupported Autocode surface: '{surface}'"));
    }
    let state = load_project_registry_at(registry_path)?;
    let norm_repo = normalize_repo_display_path(&repo_path.to_string_lossy())?;
    let key = normalize_repo_key(&norm_repo);

    let repo_conflicts: Vec<&MigrationConflict> = state
        .pending_conflicts
        .iter()
        .filter(|c| normalize_repo_key(&c.repo_root) == key)
        .collect();
    if !repo_conflicts.is_empty() {
        return Err(format!(
            "Repository '{norm_repo}' has {} unresolved migration conflict(s); resolve them in Poverello Autocode before running '{surface}' workflows.",
            repo_conflicts.len()
        ));
    }

    let matches: Vec<&ProjectRecord> = state
        .projects
        .iter()
        .filter(|p| normalize_repo_key(&p.repo_root) == key)
        .collect();
    if matches.is_empty() {
        return Err(format!(
            "Repository '{norm_repo}' is not registered in project-registry.json. Register it by selecting its plan file in Poverello Autocode."
        ));
    }
    if matches.len() > 1 {
        return Err(format!(
            "Multiple project records match repository '{norm_repo}'; resolve the ambiguity before running '{surface}' workflows."
        ));
    }
    let project = matches[0];
    let repo_root_display = strip_windows_verbatim_prefix(&project.repo_root);
    let resolved_repo = Path::new(&repo_root_display);
    if !resolved_repo.is_dir() {
        return Err(format!(
            "Registered project ({}) repository directory does not exist: '{}'",
            project.project_id, repo_root_display
        ));
    }
    let plan_rel = project.plan_path_for_surface(surface).ok_or_else(|| {
        format!(
            "No plan file is associated with repository '{repo_root_display}' ({}). Select a plan file in Poverello Autocode.",
            project.project_id
        )
    })?;
    let plan_abs = resolved_repo.join(plan_rel);
    if !plan_abs.is_file() {
        return Err(format!(
            "Associated plan file '{plan_rel}' does not exist in repository '{repo_root_display}' ({})",
            project.project_id
        ));
    }
    Ok(ResolvedProjectContext {
        project_id: project.project_id.clone(),
        display_name: project.display_name.clone(),
        surface: surface.to_string(),
        repo_root: repo_root_display,
        plan_path: plan_rel.to_string(),
        resolved: true,
    })
}

/// Resolve a workflow context from the exact selected plan-file path. The file's enclosing
/// repository is authoritative; invocation cwd and row order are never consulted.
pub fn resolve_project_context_for_plan_at(
    registry_path: &Path,
    surface: &str,
    selected_plan_path: &Path,
) -> Result<ResolvedProjectContext, String> {
    if !matches!(surface, "codex" | "antigravity") {
        return Err(format!("Unsupported Autocode surface: '{surface}'"));
    }
    if !selected_plan_path.is_absolute() {
        return Err(format!(
            "Selected plan path must be absolute; relative paths are not resolved from the working directory: '{}'",
            selected_plan_path.display()
        ));
    }
    let selected_plan = selected_plan_path.canonicalize().map_err(|e| {
        format!(
            "Selected plan file '{}' cannot be resolved: {e}",
            selected_plan_path.display()
        )
    })?;
    if !selected_plan.is_file() {
        return Err(format!(
            "Selected plan file is not a file: '{}'",
            selected_plan.display()
        ));
    }
    if !selected_plan
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
    {
        return Err(format!(
            "Selected plan file must be Markdown: '{}'",
            selected_plan.display()
        ));
    }

    let repo_root = crate::plan_manager::find_enclosing_repo_root(&selected_plan)?
        .canonicalize()
        .map_err(|e| format!("Cannot canonicalize selected plan repository: {e}"))?;
    let relative = selected_plan.strip_prefix(&repo_root).map_err(|_| {
        format!(
            "Selected plan '{}' is outside its enclosing repository",
            selected_plan.display()
        )
    })?;
    let relative = relative.to_string_lossy().replace('\\', "/");
    let context = resolve_project_context_for_repo_at(registry_path, surface, &repo_root)?;
    let registered_plan = Path::new(&context.repo_root).join(&context.plan_path);
    let registered_plan = registered_plan.canonicalize().map_err(|e| {
        format!(
            "Registered plan '{}' cannot be resolved: {e}",
            registered_plan.display()
        )
    })?;
    if registered_plan != selected_plan {
        return Err(format!(
            "Selected plan '{}' is not the plan currently associated with repository '{}'. Save that plan-file selection in Poverello Autocode, then retry.",
            selected_plan.display(), context.repo_root
        ));
    }

    Ok(ResolvedProjectContext {
        plan_path: relative,
        ..context
    })
}

#[derive(Debug, Clone)]
struct LegacySlotInput {
    source_surface: &'static str,
    slot_id: String,
    repo_path: Option<String>,
    plan_path: Option<String>,
}

fn read_legacy_surface_slots(
    path: &Path,
    surface: &'static str,
) -> Result<(Vec<LegacySlotInput>, Option<String>, Option<String>), String> {
    if !path.exists() {
        return Ok((Vec::new(), None, None));
    }
    let raw = std::fs::read_to_string(path).map_err(|e| {
        format!(
            "Failed to read legacy {surface} settings {}: {e}",
            path.display()
        )
    })?;
    let val: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
        format!(
            "Failed to parse legacy {surface} settings {}: {e}",
            path.display()
        )
    })?;
    let mut inputs = Vec::new();
    if let Some(arr) = val.get("slots").and_then(|v| v.as_array()) {
        for item in arr {
            let slot_id = item
                .get("slot_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let repo_path = item
                .get("repo_path")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let plan_path = item
                .get("plan_path")
                .and_then(|v| v.as_str())
                .and_then(normalize_plan_rel_path);
            if repo_path.is_some() || plan_path.is_some() {
                inputs.push(LegacySlotInput {
                    source_surface: surface,
                    slot_id,
                    repo_path,
                    plan_path,
                });
            }
        }
    }
    let active_plan = val
        .get("active_plan_path")
        .and_then(|v| v.as_str())
        .and_then(normalize_plan_rel_path);
    let active_repo = val
        .get("active_plan_repo_path")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Ok((inputs, active_repo, active_plan))
}

fn resolve_legacy_selection(
    inputs: &[LegacySlotInput],
    active_repo: Option<&str>,
    active_plan: Option<&str>,
) -> Option<String> {
    if let Some(repo) = active_repo {
        let key = normalize_repo_key(repo);
        let matched: Vec<_> = inputs
            .iter()
            .filter(|i| {
                i.repo_path.as_deref().map(normalize_repo_key).as_deref() == Some(key.as_str())
            })
            .collect();
        if !matched.is_empty() {
            if let Some(plan) = active_plan {
                if matched
                    .iter()
                    .all(|m| m.plan_path.as_deref() == Some(plan) || m.plan_path.is_none())
                {
                    return Some(derive_project_id(repo));
                }
                return None;
            }
            return Some(derive_project_id(repo));
        }
        return Some(derive_project_id(repo));
    }
    if let Some(plan) = active_plan {
        let matched: Vec<_> = inputs
            .iter()
            .filter(|i| i.plan_path.as_deref() == Some(plan) && i.repo_path.is_some())
            .collect();
        let distinct_keys: BTreeSet<String> = matched
            .iter()
            .filter_map(|m| m.repo_path.as_deref().map(normalize_repo_key))
            .collect();
        if distinct_keys.len() == 1 {
            if let Some(repo) = matched[0].repo_path.as_deref() {
                return Some(derive_project_id(repo));
            }
        }
    }
    None
}

/// Safely migrate legacy Codex/Antigravity slot lists into `project-registry.json` once.
/// Preserves existing registry if already migrated; records conflicts without silent overwrite.
pub fn migrate_legacy_projects_if_needed(
    registry_path: &Path,
    codex_settings_path: Option<&Path>,
    antigravity_policy_path: Option<&Path>,
) -> Result<ProjectRegistryState, String> {
    let _guard = PROJECT_REGISTRY_LOCK
        .lock()
        .map_err(|e| format!("Failed to acquire project registry lock: {e}"))?;
    if registry_path.exists() {
        let existing = load_project_registry_at(registry_path)?;
        if existing.migrated_from_legacy || !existing.projects.is_empty() {
            return Ok(existing);
        }
    }

    let (codex_inputs, codex_active_repo, codex_active_plan) = match codex_settings_path {
        Some(p) => read_legacy_surface_slots(p, "codex")?,
        None => (Vec::new(), None, None),
    };
    let (ag_inputs, ag_active_repo, ag_active_plan) = match antigravity_policy_path {
        Some(p) => read_legacy_surface_slots(p, "antigravity")?,
        None => (Vec::new(), None, None),
    };

    let mut all_inputs = Vec::new();
    all_inputs.extend(codex_inputs.clone());
    all_inputs.extend(ag_inputs.clone());

    let mut grouped: BTreeMap<String, Vec<LegacySlotInput>> = BTreeMap::new();
    let mut conflicts = Vec::new();

    for input in all_inputs {
        let Some(ref raw_repo) = input.repo_path else {
            if let Some(ref plan) = input.plan_path {
                conflicts.push(MigrationConflict {
                    conflict_id: format!("missing-repo-{}-{}", input.source_surface, input.slot_id),
                    kind: "missing_repository_root".to_string(),
                    repo_root: String::new(),
                    surface: Some(input.source_surface.to_string()),
                    description: format!(
                        "Legacy {} slot '{}' specifies plan '{}' without a repository root.",
                        input.source_surface, input.slot_id, plan
                    ),
                    options: vec![MigrationConflictOption {
                        source: format!("{}:{}", input.source_surface, input.slot_id),
                        label: plan.clone(),
                        project_id: None,
                        plan_path: Some(plan.clone()),
                        value: None,
                    }],
                });
            }
            continue;
        };
        let norm_display = strip_windows_verbatim_prefix(raw_repo);
        let key = normalize_repo_key(&norm_display);
        grouped.entry(key).or_default().push(LegacySlotInput {
            repo_path: Some(norm_display),
            ..input
        });
    }

    let mut projects = Vec::new();
    let now = now_unix();
    let mut counter = 1usize;

    for (_key, entries) in grouped {
        let repo_root = entries[0].repo_path.clone().unwrap_or_default();
        let pid = derive_project_id(&repo_root);
        let mut plan_paths = BTreeMap::new();
        for surface in ["codex", "antigravity"] {
            let surface_entries: Vec<&LegacySlotInput> = entries
                .iter()
                .filter(|entry| entry.source_surface == surface)
                .collect();
            let distinct_plans: BTreeSet<String> = surface_entries
                .iter()
                .filter_map(|entry| entry.plan_path.clone())
                .collect();
            if distinct_plans.len() <= 1 {
                if let Some(plan) = distinct_plans.into_iter().next() {
                    plan_paths.insert(surface.to_string(), Some(plan));
                }
                continue;
            }
            let options = surface_entries
                .iter()
                .filter_map(|entry| {
                    entry.plan_path.as_ref().map(|path| MigrationConflictOption {
                        source: format!("{}:{}", entry.source_surface, entry.slot_id),
                        label: format!("{path} ({} {})", entry.source_surface, entry.slot_id),
                        project_id: None,
                        plan_path: Some(path.clone()),
                        value: None,
                    })
                })
                .collect();
            conflicts.push(MigrationConflict {
                conflict_id: format!("plan-conflict-{pid}-{surface}"),
                kind: "plan_path_conflict".to_string(),
                repo_root: repo_root.clone(),
                surface: Some(surface.to_string()),
                description: format!(
                    "Multiple distinct {surface} plan paths were found for repository '{}'. Choose which path to keep for {surface}.",
                    repo_root
                ),
                options,
            });
        }

        projects.push(ProjectRecord {
            project_id: pid,
            display_name: format!("Project {counter}"),
            repo_root,
            plan_path: None,
            plan_paths,
            created_unix: now,
            updated_unix: now,
            extra: BTreeMap::new(),
        });
        counter += 1;
    }

    let state = ProjectRegistryState {
        schema_version: PROJECT_REGISTRY_SCHEMA_VERSION,
        projects,
        migrated_from_legacy: true,
        pending_conflicts: conflicts,
        last_updated_unix: now,
        extra: BTreeMap::new(),
    };

    save_project_registry_unlocked(registry_path, &state, &PersistenceHooks::default())
}

pub fn resolve_migration_conflict_at(
    registry_path: &Path,
    conflict_id: &str,
    chosen_plan_path: Option<&str>,
) -> Result<ProjectRegistryState, String> {
    let _guard = PROJECT_REGISTRY_LOCK
        .lock()
        .map_err(|e| format!("Failed to acquire project registry lock: {e}"))?;
    let mut state = load_project_registry_at(registry_path)?;
    let idx = state
        .pending_conflicts
        .iter()
        .position(|c| c.conflict_id == conflict_id)
        .ok_or_else(|| format!("Migration conflict '{conflict_id}' not found"))?;
    let conflict = state.pending_conflicts.remove(idx);
    if conflict.kind == "plan_path_conflict" {
        let pid = derive_project_id(&conflict.repo_root);
        let proj = state
            .projects
            .iter_mut()
            .find(|p| p.project_id == pid)
            .ok_or_else(|| format!("Project for conflict '{conflict_id}' is not registered"))?;
        let chosen = chosen_plan_path.and_then(normalize_plan_rel_path);
        if let Some(surface) = conflict.surface.as_deref() {
            if !matches!(surface, "codex" | "antigravity") {
                return Err(format!("Unsupported surface '{}' in migration conflict", surface));
            }
            proj.plan_paths.insert(surface.to_string(), chosen);
        } else {
            proj.plan_path = chosen;
        }
        proj.updated_unix = now_unix();
    }
    save_project_registry_unlocked(registry_path, &state, &PersistenceHooks::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_strip_windows_verbatim_prefix_and_stable_project_id() {
        assert_eq!(
            strip_windows_verbatim_prefix(r"\\?\C:\Users\Sohei\dev\ASR-Composer"),
            r"C:\Users\Sohei\dev\ASR-Composer"
        );
        assert_eq!(
            strip_windows_verbatim_prefix(r"\\?\UNC\server\share\repo"),
            r"\\server\share\repo"
        );
        let id1 = derive_project_id(r"\\?\C:\Users\Sohei\dev\ASR-Composer");
        let id2 = derive_project_id(r"C:\Users\Sohei\dev\ASR-Composer");
        assert_eq!(id1, id2);
        assert!(id1.starts_with("proj_"));
    }

    #[test]
    fn test_replacing_plan_association_switches_repository_without_growing_registry() {
        let dir = tempdir().unwrap();
        let registry_path = dir.path().join("project-registry.json");
        let repo_asr = dir.path().join("ASR-Composer");
        let repo_poverello = dir.path().join("poverello");
        let repo_ncvroc = dir.path().join("NCVROC");
        for repo in [&repo_asr, &repo_poverello, &repo_ncvroc] {
            std::fs::create_dir_all(repo).unwrap();
        }
        register_or_update_project_at(
            &registry_path,
            &repo_asr.to_string_lossy(),
            Some(".plan/v0.3.0.md"),
            None,
        )
        .unwrap();
        register_or_update_project_at(
            &registry_path,
            &repo_poverello.to_string_lossy(),
            Some(".plan/V0.28.1.md"),
            None,
        )
        .unwrap();
        let source_id = derive_project_id(&normalize_repo_display_path(&repo_asr.to_string_lossy()).unwrap());

        let updated = replace_project_plan_association_at(
            &registry_path,
            &source_id,
            &repo_ncvroc.to_string_lossy(),
            ".plan/v0.34.1.md",
        )
        .unwrap();

        assert_eq!(updated.projects.len(), 2);
        assert!(!updated.projects.iter().any(|p| p.project_id == source_id));
        let switched = updated
            .projects
            .iter()
            .find(|p| normalize_repo_key(&p.repo_root) == normalize_repo_key(&repo_ncvroc.to_string_lossy()))
            .unwrap();
        assert_eq!(switched.project_id, derive_project_id(&normalize_repo_display_path(&repo_ncvroc.to_string_lossy()).unwrap()));
        assert_eq!(switched.plan_path.as_deref(), Some(".plan/v0.34.1.md"));
        assert!(updated.projects.iter().any(|p| normalize_repo_key(&p.repo_root) == normalize_repo_key(&repo_poverello.to_string_lossy())));
    }

    #[test]
    fn test_codex_and_antigravity_plan_paths_are_independent_and_resolve_by_surface() {
        let dir = tempdir().unwrap();
        let registry_path = dir.path().join("project-registry.json");
        let repo = dir.path().join("shared-repo");
        std::fs::create_dir_all(repo.join(".plan")).unwrap();
        std::fs::write(repo.join(".plan/codex.md"), "# Codex").unwrap();
        std::fs::write(repo.join(".plan/antigravity.md"), "# Antigravity").unwrap();

        register_or_update_project_for_surface_at(
            &registry_path,
            "codex",
            &repo.to_string_lossy(),
            Some(".plan/codex.md"),
            None,
        )
        .unwrap();
        let state = register_or_update_project_for_surface_at(
            &registry_path,
            "antigravity",
            &repo.to_string_lossy(),
            Some(".plan/antigravity.md"),
            None,
        )
        .unwrap();

        assert_eq!(state.projects.len(), 1);
        let project = &state.projects[0];
        assert_eq!(project.plan_path_for_surface("codex"), Some(".plan/codex.md"));
        assert_eq!(project.plan_path_for_surface("antigravity"), Some(".plan/antigravity.md"));
        assert_eq!(
            resolve_project_context_for_repo_at(&registry_path, "codex", &repo)
                .unwrap()
                .plan_path,
            ".plan/codex.md"
        );
        assert_eq!(
            resolve_project_context_for_repo_at(&registry_path, "antigravity", &repo)
                .unwrap()
                .plan_path,
            ".plan/antigravity.md"
        );
        assert!(resolve_project_context_for_plan_at(
            &registry_path,
            "codex",
            &repo.join(".plan/antigravity.md")
        )
        .is_err());
    }

    #[test]
    fn test_first_surface_change_preserves_legacy_path_for_other_surface() {
        let dir = tempdir().unwrap();
        let registry_path = dir.path().join("project-registry.json");
        let repo = dir.path().join("legacy-repo");
        std::fs::create_dir_all(repo.join(".plan")).unwrap();
        let initial = register_or_update_project_at(
            &registry_path,
            &repo.to_string_lossy(),
            Some(".plan/legacy.md"),
            None,
        )
        .unwrap();
        let project_id = initial.projects[0].project_id.clone();

        let updated = set_project_plan_path_for_surface_at(
            &registry_path,
            &project_id,
            "codex",
            Some(".plan/codex.md"),
        )
        .unwrap();
        let project = &updated.projects[0];
        assert_eq!(project.plan_path.as_deref(), Some(".plan/legacy.md"));
        assert_eq!(project.plan_path_for_surface("codex"), Some(".plan/codex.md"));
        assert_eq!(project.plan_path_for_surface("antigravity"), Some(".plan/legacy.md"));
    }

    #[test]
    fn test_surface_replacement_and_removal_keep_other_surface_association() {
        let dir = tempdir().unwrap();
        let registry_path = dir.path().join("project-registry.json");
        let repo_a = dir.path().join("repo-a");
        let repo_b = dir.path().join("repo-b");
        std::fs::create_dir_all(repo_a.join(".plan")).unwrap();
        std::fs::create_dir_all(repo_b.join(".plan")).unwrap();
        for (repo, plan) in [
            (&repo_a, "codex-a.md"),
            (&repo_a, "ag-a.md"),
            (&repo_b, "codex-b.md"),
            (&repo_b, "ag-b.md"),
        ] {
            std::fs::write(repo.join(".plan").join(plan), "# Plan").unwrap();
        }
        let source_state = register_or_update_project_for_surface_at(
            &registry_path,
            "codex",
            &repo_a.to_string_lossy(),
            Some(".plan/codex-a.md"),
            None,
        )
        .unwrap();
        let source_id = source_state.projects[0].project_id.clone();
        register_or_update_project_for_surface_at(
            &registry_path,
            "antigravity",
            &repo_a.to_string_lossy(),
            Some(".plan/ag-a.md"),
            None,
        )
        .unwrap();
        register_or_update_project_for_surface_at(
            &registry_path,
            "antigravity",
            &repo_b.to_string_lossy(),
            Some(".plan/ag-b.md"),
            None,
        )
        .unwrap();

        let switched = replace_project_plan_association_for_surface_at(
            &registry_path,
            &source_id,
            &repo_b.to_string_lossy(),
            ".plan/codex-b.md",
            "codex",
        )
        .unwrap();
        let source = switched.projects.iter().find(|project| project.project_id == source_id).unwrap();
        let target = switched.projects.iter().find(|project| normalize_repo_key(&project.repo_root) == normalize_repo_key(&repo_b.to_string_lossy())).unwrap();
        assert_eq!(source.plan_path_for_surface("codex"), None);
        assert_eq!(source.plan_path_for_surface("antigravity"), Some(".plan/ag-a.md"));
        assert_eq!(target.plan_path_for_surface("codex"), Some(".plan/codex-b.md"));
        assert_eq!(target.plan_path_for_surface("antigravity"), Some(".plan/ag-b.md"));

        let after_remove_codex = unregister_project_surface_at(&registry_path, &source_id, "codex").unwrap();
        assert!(after_remove_codex.projects.iter().any(|project| project.project_id == source_id));
        assert_eq!(
            resolve_project_context_for_repo_at(&registry_path, "antigravity", &repo_a)
                .unwrap()
                .plan_path,
            ".plan/ag-a.md"
        );
        let after_remove_ag = unregister_project_surface_at(&registry_path, &source_id, "antigravity").unwrap();
        assert!(!after_remove_ag.projects.iter().any(|project| project.project_id == source_id));
    }

    #[test]
    fn test_replacing_with_existing_repository_updates_target_and_removes_source_once() {
        let dir = tempdir().unwrap();
        let registry_path = dir.path().join("project-registry.json");
        let repo_asr = dir.path().join("ASR-Composer");
        let repo_ncvroc = dir.path().join("NCVROC");
        std::fs::create_dir_all(&repo_asr).unwrap();
        std::fs::create_dir_all(&repo_ncvroc).unwrap();
        for (repo, plan) in [(&repo_asr, ".plan/old.md"), (&repo_ncvroc, ".plan/previous.md")] {
            register_or_update_project_at(&registry_path, &repo.to_string_lossy(), Some(plan), None).unwrap();
        }
        let source_id = derive_project_id(&normalize_repo_display_path(&repo_asr.to_string_lossy()).unwrap());

        let updated = replace_project_plan_association_at(
            &registry_path,
            &source_id,
            &repo_ncvroc.to_string_lossy(),
            ".plan/v0.34.1.md",
        )
        .unwrap();

        assert_eq!(updated.projects.len(), 1);
        let target = &updated.projects[0];
        assert_eq!(target.project_id, derive_project_id(&normalize_repo_display_path(&repo_ncvroc.to_string_lossy()).unwrap()));
        assert_eq!(target.plan_path.as_deref(), Some(".plan/v0.34.1.md"));
    }

    #[test]
    fn test_replacement_validation_and_persistence_failure_leave_registry_unchanged() {
        let dir = tempdir().unwrap();
        let registry_path = dir.path().join("project-registry.json");
        let repo_asr = dir.path().join("ASR-Composer");
        let repo_ncvroc = dir.path().join("NCVROC");
        std::fs::create_dir_all(&repo_asr).unwrap();
        std::fs::create_dir_all(&repo_ncvroc).unwrap();
        let before = register_or_update_project_at(
            &registry_path,
            &repo_asr.to_string_lossy(),
            Some(".plan/v0.3.0.md"),
            None,
        )
        .unwrap();
        let source_id = before.projects[0].project_id.clone();

        assert!(replace_project_plan_association_at(
            &registry_path,
            &source_id,
            &repo_ncvroc.to_string_lossy(),
            "../outside.md",
        )
        .is_err());
        assert_eq!(load_project_registry_at(&registry_path).unwrap(), before);

        let hooks = PersistenceHooks { fail_write: true, ..PersistenceHooks::default() };
        assert!(replace_project_plan_association_with_hooks(
            &registry_path,
            &source_id,
            &repo_ncvroc.to_string_lossy(),
            ".plan/v0.34.1.md",
            &hooks,
        )
        .is_err());
        assert_eq!(load_project_registry_at(&registry_path).unwrap(), before);
    }

    #[test]
    fn test_replacement_refuses_duplicate_destination_without_mutating_registry() {
        let dir = tempdir().unwrap();
        let registry_path = dir.path().join("project-registry.json");
        let source_repo = dir.path().join("source");
        let target_repo = dir.path().join("target");
        std::fs::create_dir_all(&source_repo).unwrap();
        std::fs::create_dir_all(&target_repo).unwrap();
        let source = register_or_update_project_at(&registry_path, &source_repo.to_string_lossy(), Some(".plan/source.md"), None).unwrap();
        let mut corrupted = register_or_update_project_at(&registry_path, &target_repo.to_string_lossy(), Some(".plan/target.md"), None).unwrap();
        let duplicate = corrupted.projects.iter().find(|p| normalize_repo_key(&p.repo_root) == normalize_repo_key(&target_repo.to_string_lossy())).unwrap().clone();
        corrupted.projects.push(ProjectRecord { project_id: "proj_duplicate_target".into(), ..duplicate });
        std::fs::write(&registry_path, serde_json::to_vec_pretty(&corrupted).unwrap()).unwrap();
        let before = load_project_registry_at(&registry_path).unwrap();
        let source_id = source.projects[0].project_id.clone();

        let error = replace_project_plan_association_at(
            &registry_path,
            &source_id,
            &target_repo.to_string_lossy(),
            ".plan/new-target.md",
        )
        .unwrap_err();

        assert!(error.contains("registered more than once"));
        assert_eq!(load_project_registry_at(&registry_path).unwrap(), before);
    }

    #[test]
    fn test_register_select_rename_reorder_and_unregister_without_fallback() {
        let dir = tempdir().unwrap();
        let registry_path = dir.path().join("project-registry.json");
        let repo_a = dir.path().join("poverello");
        let repo_b = dir.path().join("ASR-Composer");
        std::fs::create_dir_all(repo_a.join(".plan")).unwrap();
        std::fs::create_dir_all(repo_b.join(".plan")).unwrap();
        std::fs::write(repo_a.join(".plan/V0.28.1.md"), "# Poverello\n").unwrap();
        std::fs::write(repo_b.join(".plan/v0.3.0.md"), b"").unwrap(); // 0-byte plan

        let s1 = register_or_update_project_at(
            &registry_path,
            &repo_a.to_string_lossy(),
            Some(".plan/V0.28.1.md"),
            None,
        )
        .unwrap();
        let s2 = register_or_update_project_at(
            &registry_path,
            &repo_b.to_string_lossy(),
            Some(".plan/v0.3.0.md"),
            None,
        )
        .unwrap();

        assert_eq!(s2.projects.len(), 2);
        let id_a = s1.projects[0].project_id.clone();
        let id_b = s2.projects[1].project_id.clone();
        assert_eq!(s2.projects[0].display_name, "Project 1");
        assert_eq!(s2.projects[1].display_name, "Project 2");

        // Reorder rows and rename without changing ID or repo-based resolution
        let s3 = reorder_projects_at(&registry_path, &[id_b.clone(), id_a.clone()]).unwrap();
        let s4 = rename_project_at(&registry_path, &id_b, "ASR Composer").unwrap();
        assert_eq!(s4.projects[0].project_id, id_b);
        assert_eq!(s4.projects[0].display_name, "ASR Composer");
        let _ = s3;

        // Context resolver resolves each repository independently for both surfaces
        let codex_ctx_b =
            resolve_project_context_for_repo_at(&registry_path, "codex", &repo_b).unwrap();
        assert_eq!(codex_ctx_b.project_id, id_b);
        assert_eq!(codex_ctx_b.plan_path, ".plan/v0.3.0.md");
        let ag_ctx_a =
            resolve_project_context_for_repo_at(&registry_path, "antigravity", &repo_a).unwrap();
        assert_eq!(ag_ctx_a.project_id, id_a);
        assert_eq!(ag_ctx_a.plan_path, ".plan/V0.28.1.md");

        // Unregistered repository fails closed without substituting another project
        let repo_unreg = dir.path().join("unregistered");
        std::fs::create_dir_all(&repo_unreg).unwrap();
        assert!(resolve_project_context_for_repo_at(&registry_path, "codex", &repo_unreg).is_err());

        // Unregistering project B removes only its registry entry, fails closed for repo_b,
        // and leaves repo_b/.plan/v0.3.0.md untouched (0 bytes).
        let s5 = unregister_project_at(&registry_path, &id_b).unwrap();
        assert_eq!(s5.projects.len(), 1);
        assert!(resolve_project_context_for_repo_at(&registry_path, "codex", &repo_b).is_err());
        assert!(
            resolve_project_context_for_repo_at(&registry_path, "antigravity", &repo_a).is_ok()
        );
        assert_eq!(
            std::fs::metadata(repo_b.join(".plan/v0.3.0.md"))
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn test_registry_accepts_more_than_two_repositories_and_updates_normalized_duplicate() {
        let dir = tempdir().unwrap();
        let registry_path = dir.path().join("project-registry.json");
        let repos = ["repo-a", "repo-b", "repo-c"].map(|name| dir.path().join(name));
        for repo in &repos {
            std::fs::create_dir_all(repo.join(".plan")).unwrap();
        }
        for index in 0..repos.len() {
            std::fs::write(
                repos[index].join(format!(".plan/plan-{index}.md")),
                "# Test plan\n",
            )
            .unwrap();
        }
        std::fs::write(repos[0].join(".plan/updated.md"), "# Updated plan\n").unwrap();

        let mut state = ProjectRegistryState::default();
        for (index, repo) in repos.iter().enumerate() {
            state = register_or_update_project_at(
                &registry_path,
                &repo.to_string_lossy(),
                Some(&format!(".plan/plan-{index}.md")),
                None,
            )
            .unwrap();
        }
        assert_eq!(state.projects.len(), 3);
        let original_a_id = state.projects[0].project_id.clone();

        let normalized_duplicate = repos[0].join(".");
        let updated = register_or_update_project_at(
            &registry_path,
            &normalized_duplicate.to_string_lossy(),
            Some(".plan/updated.md"),
            None,
        )
        .unwrap();

        assert_eq!(updated.projects.len(), 3);
        let repo_a = updated
            .projects
            .iter()
            .find(|p| p.project_id == original_a_id)
            .unwrap();
        assert_eq!(repo_a.plan_path.as_deref(), Some(".plan/updated.md"));
        assert_eq!(
            updated
                .projects
                .iter()
                .filter(|p| p.project_id == original_a_id)
                .count(),
            1
        );
        assert_eq!(
            updated.projects[1].plan_path.as_deref(),
            Some(".plan/plan-1.md")
        );
        assert_eq!(
            updated.projects[2].plan_path.as_deref(),
            Some(".plan/plan-2.md")
        );
        let third_repo_context =
            resolve_project_context_for_repo_at(&registry_path, "codex", &repos[2]).unwrap();
        assert_eq!(third_repo_context.plan_path, ".plan/plan-2.md");
    }

    #[test]
    fn test_plan_path_first_resolver_derives_repository_and_fails_closed_on_mismatch() {
        let dir = tempdir().unwrap();
        let registry_path = dir.path().join("project-registry.json");
        let repo_a = dir.path().join("repo-a");
        let repo_b = dir.path().join("repo-b");
        for repo in [&repo_a, &repo_b] {
            std::fs::create_dir_all(repo.join(".git")).unwrap();
            std::fs::create_dir_all(repo.join(".plan")).unwrap();
        }
        let plan_a = repo_a.join(".plan/a.md");
        let plan_b = repo_b.join(".plan/b.md");
        let other_b = repo_b.join(".plan/other.md");
        std::fs::write(&plan_a, "# A\n").unwrap();
        std::fs::write(&plan_b, "# B\n").unwrap();
        std::fs::write(&other_b, "# Other B\n").unwrap();
        let registered_a = register_or_update_project_at(
            &registry_path,
            &repo_a.to_string_lossy(),
            Some(".plan/a.md"),
            None,
        )
        .unwrap();
        let registered_b = register_or_update_project_at(
            &registry_path,
            &repo_b.to_string_lossy(),
            Some(".plan/b.md"),
            None,
        )
        .unwrap();
        reorder_projects_at(
            &registry_path,
            &[
                registered_b.projects[1].project_id.clone(),
                registered_a.projects[0].project_id.clone(),
            ],
        )
        .unwrap();

        // The selected plan path itself chooses repo B, independent of registry row order.
        let resolved =
            resolve_project_context_for_plan_at(&registry_path, "codex", &plan_b).unwrap();
        assert_eq!(resolved.project_id, registered_b.projects[1].project_id);
        assert_eq!(
            resolved.repo_root,
            strip_windows_verbatim_prefix(&repo_b.canonicalize().unwrap().to_string_lossy())
        );
        assert_eq!(resolved.plan_path, ".plan/b.md");

        // A different plan in the same repo is not silently substituted by the registry mapping.
        let mismatch =
            resolve_project_context_for_plan_at(&registry_path, "codex", &other_b).unwrap_err();
        assert!(mismatch.contains("not the plan currently associated"));
        assert!(resolve_project_context_for_plan_at(
            &registry_path,
            "codex",
            &dir.path().join("missing.md")
        )
        .is_err());
        assert!(resolve_project_context_for_plan_at(
            &registry_path,
            "codex",
            Path::new(".plan/b.md")
        )
        .unwrap_err()
        .contains("must be absolute"));
        assert!(resolve_project_context_for_plan_at(
            &registry_path,
            "codex",
            &repo_a.join(".plan/a.txt")
        )
        .is_err());
        let repo_unregistered = dir.path().join("repo-unregistered");
        std::fs::create_dir_all(repo_unregistered.join(".git")).unwrap();
        std::fs::create_dir_all(repo_unregistered.join(".plan")).unwrap();
        let unregistered_plan = repo_unregistered.join(".plan/unregistered.md");
        std::fs::write(&unregistered_plan, "# Unregistered\n").unwrap();
        assert!(
            resolve_project_context_for_plan_at(&registry_path, "codex", &unregistered_plan)
                .is_err()
        );
    }

    #[test]
    fn test_bundled_workflow_skills_require_plan_path_first_resolution() {
        let skills = [
            include_str!("../resources/codex-skills/pov-plan-only/SKILL.md"),
            include_str!("../resources/codex-skills/pov-build-review/SKILL.md"),
            include_str!("../resources/codex-skills/pov-full-cycle/SKILL.md"),
            include_str!("../resources/antigravity-skills/pov-plan-only/SKILL.md"),
            include_str!("../resources/antigravity-skills/pov-build-review/SKILL.md"),
            include_str!("../resources/antigravity-skills/pov-full-cycle/SKILL.md"),
        ];
        for skill in skills {
            assert!(skill.contains("絶対パスの `selected_plan_path`"));
            assert!(skill.contains("resolve_project_context_for_plan"));
            assert!(skill.contains("launch_pinned_plan_workflow_for_selected_plan"));
            assert!(skill.contains("verify_plan_resumption"));
            assert!(!skill.contains("NO ACTIVE PLAN FILE SPECIFIED"));
            assert!(!skill.contains("resolve_project_context_for_repo_at`）で解決した `plan_path`"));
        }
    }

    #[test]
    fn test_atomic_persistence_failure_and_rollback_preserves_previous_registry() {
        let dir = tempdir().unwrap();
        let registry_path = dir.path().join("project-registry.json");
        let repo_a = dir.path().join("repo-a");
        std::fs::create_dir_all(&repo_a).unwrap();

        let initial = register_or_update_project_at(
            &registry_path,
            &repo_a.to_string_lossy(),
            Some(".plan/a.md"),
            Some("Repo A"),
        )
        .unwrap();

        let mut modified = initial.clone();
        modified.projects[0].display_name = "Corrupted Change".to_string();

        for hooks in [
            PersistenceHooks {
                fail_backup: true,
                ..Default::default()
            },
            PersistenceHooks {
                fail_write: true,
                ..Default::default()
            },
            PersistenceHooks {
                fail_readback: true,
                ..Default::default()
            },
        ] {
            assert!(
                save_project_registry_at_with_hooks(&registry_path, &modified, &hooks).is_err()
            );
            let after = load_project_registry_at(&registry_path).unwrap();
            assert_eq!(after.projects[0].display_name, "Repo A");
        }
    }

    #[test]
    fn test_legacy_migration_consolidates_reordered_duplicates_and_flags_conflicts() {
        let dir = tempdir().unwrap();
        let repo_a = dir.path().join("poverello");
        let repo_b = dir.path().join("ASR-Composer");
        std::fs::create_dir_all(&repo_a).unwrap();
        std::fs::create_dir_all(&repo_b).unwrap();

        let codex_file = dir.path().join("pov-review-settings.json");
        let ag_file = dir.path().join("pov-reviewer-policy.json");

        // Legacy active_plan_repo_path / active_plan_path are NOT migrated into any global active pointer
        std::fs::write(
            &codex_file,
            serde_json::to_string_pretty(&serde_json::json!({
                "active_plan_path": ".plan/v0.3.0.md",
                "active_plan_repo_path": repo_b.to_string_lossy(),
                "slots": [
                    { "slot_id": "primary", "repo_path": repo_a.to_string_lossy(), "plan_path": ".plan/V0.28.1.md" },
                    { "slot_id": "secondary", "repo_path": repo_b.to_string_lossy(), "plan_path": ".plan/v0.3.0.md" }
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        std::fs::write(
            &ag_file,
            serde_json::to_string_pretty(&serde_json::json!({
                "active_plan_path": ".plan/V0.28.1.md",
                "active_plan_repo_path": repo_a.to_string_lossy(),
                "slots": [
                    { "slot_id": "secondary", "repo_path": repo_b.to_string_lossy(), "plan_path": ".plan/ag-v0.3.0.md" },
                    { "slot_id": "primary", "repo_path": repo_a.to_string_lossy(), "plan_path": ".plan/V0.28.1.md" }
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        let reg_path = dir.path().join("project-registry.json");
        let migrated =
            migrate_legacy_projects_if_needed(&reg_path, Some(&codex_file), Some(&ag_file))
                .unwrap();
        assert!(migrated.pending_conflicts.is_empty());
        assert_eq!(migrated.projects.len(), 2);
        let migrated_b = migrated
            .projects
            .iter()
            .find(|project| project.project_id == derive_project_id(&repo_b.to_string_lossy()))
            .unwrap();
        assert_eq!(migrated_b.plan_path_for_surface("codex"), Some(".plan/v0.3.0.md"));
        assert_eq!(migrated_b.plan_path_for_surface("antigravity"), Some(".plan/ag-v0.3.0.md"));

        // Conflicting plan paths for same repo -> preserved as pending conflict, no silent overwrite
        let reg_conflict_path = dir.path().join("project-registry-conflict.json");
        std::fs::write(
            &ag_file,
            serde_json::to_string_pretty(&serde_json::json!({
                "slots": [
                    { "slot_id": "primary", "repo_path": repo_b.to_string_lossy(), "plan_path": ".plan/other.md" },
                    { "slot_id": "secondary", "repo_path": repo_b.to_string_lossy(), "plan_path": ".plan/other-2.md" }
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        let conflict_state = migrate_legacy_projects_if_needed(
            &reg_conflict_path,
            Some(&codex_file),
            Some(&ag_file),
        )
        .unwrap();
        assert_eq!(conflict_state.pending_conflicts.len(), 1);
        assert_eq!(
            conflict_state.pending_conflicts[0].kind,
            "plan_path_conflict"
        );

        let resolved = resolve_migration_conflict_at(
            &reg_conflict_path,
            &conflict_state.pending_conflicts[0].conflict_id,
            Some(".plan/other.md"),
        )
        .unwrap();
        assert!(resolved.pending_conflicts.is_empty());
        let id_b = derive_project_id(&repo_b.to_string_lossy());
        let proj_b = resolved
            .projects
            .iter()
            .find(|p| p.project_id == id_b)
            .unwrap();
        assert_eq!(proj_b.plan_path_for_surface("codex"), Some(".plan/v0.3.0.md"));
        assert_eq!(proj_b.plan_path_for_surface("antigravity"), Some(".plan/other.md"));
    }
}
