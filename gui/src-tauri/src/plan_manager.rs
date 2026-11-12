use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PLAN_DIR_NAME: &str = ".plan";
pub const ACTIVE_PLAN_FILE_NAME: &str = ".active-plan.json";
static ACTIVE_PLAN_STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanFileEntry {
    pub rel_path: String,
    pub file_name: String,
    pub title: String,
    pub last_modified_unix: u64,
    pub file_size_bytes: u64,
    pub sha256_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanFileItem {
    pub path: String,
    pub relative_path: String,
    pub title: Option<String>,
    pub status: Option<String>,
    pub last_modified_ms: u64,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanOverview {
    pub repo_root: String,
    pub plans_dir: String,
    pub selected_folder: Option<String>,
    pub active_plan_path: Option<String>,
    pub active_plan_relative_path: Option<String>,
    pub plans: Vec<PlanFileItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanFileDetails {
    pub rel_path: String,
    pub abs_path: String,
    pub file_name: String,
    pub title: String,
    pub status: Option<String>,
    pub content: String,
    pub sha256_hash: String,
    pub file_identity: Option<String>,
    pub last_modified_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectPlanSlot {
    pub slot_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SurfacePlanState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_plan_path: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub slots: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivePlanState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_plan_path: Option<String>,
    pub last_updated_unix: u64,
    #[serde(default)]
    pub role_assignments: std::collections::HashMap<String, RoleAssignment>,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub surfaces: std::collections::HashMap<String, SurfacePlanState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RoleAssignment {
    pub planner_id: Option<String>,
    pub reviser_id: Option<String>,
}

pub fn find_enclosing_repo_root(file_path: &Path) -> Result<PathBuf, String> {
    let mut current = file_path;
    if current.is_file() {
        current = match current.parent() {
            Some(p) => p,
            None => return Err(format!("File '{}' has no parent directory", file_path.display())),
        };
    }
    loop {
        let git_dir = current.join(".git");
        if git_dir.exists() {
            return Ok(current.to_path_buf());
        }
        match current.parent() {
            Some(parent) if parent != current => current = parent,
            _ => break,
        }
    }
    Err(format!(
        "Selected plan file '{}' is not contained in a Git repository",
        file_path.display()
    ))
}

fn load_active_plan_state(repo_root: &Path) -> Result<ActivePlanState, String> {
    let path = repo_root.join(PLAN_DIR_NAME).join(ACTIVE_PLAN_FILE_NAME);
    if !path.exists() {
        return Ok(ActivePlanState {
            active_plan_path: None,
            last_updated_unix: 0,
            role_assignments: Default::default(),
            surfaces: Default::default(),
        });
    }
    let raw = std::fs::read_to_string(path).map_err(|e| format!("Failed to read active plan state: {e}"))?;
    let mut state: ActivePlanState = serde_json::from_str(&raw).map_err(|e| format!("Failed to parse active plan state: {e}"))?;

    // Migrate per-surface legacy active_plan_path to primary slot if slots is empty
    for surf_state in state.surfaces.values_mut() {
        if surf_state.slots.is_empty() {
            if let Some(ref legacy) = surf_state.active_plan_path {
                surf_state.slots.insert("primary".to_string(), legacy.clone());
            }
        }
    }

    Ok(state)
}

pub fn get_role_assignment(repo_root: &Path, surface: &str) -> Result<RoleAssignment, String> {
    if !matches!(surface, "codex" | "antigravity") { return Err(format!("Unsupported Autocode surface: '{surface}'")); }
    Ok(load_active_plan_state(repo_root)?.role_assignments.get(surface).cloned().unwrap_or_default())
}

pub fn set_role_assignment(repo_root: &Path, surface: &str, assignment: RoleAssignment) -> Result<RoleAssignment, String> {
    if !matches!(surface, "codex" | "antigravity") { return Err(format!("Unsupported Autocode surface: '{surface}'")); }
    let allowed: &[&str] = if surface == "codex" { &["codex_session", "codex"] } else { &["antigravity", "codex", "codex_cli"] };
    for role in [&assignment.planner_id, &assignment.reviser_id].into_iter().flatten() {
        if !allowed.contains(&role.as_str()) { return Err(format!("Unsupported role '{role}' for {surface} surface")); }
    }
    let _lock = ACTIVE_PLAN_STATE_LOCK.lock().map_err(|e| format!("Failed to acquire active plan state lock: {e}"))?;
    let mut state = load_active_plan_state(repo_root)?;
    state.role_assignments.insert(surface.to_string(), assignment.clone());
    state.last_updated_unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    write_active_plan_state_unlocked(repo_root, &state)?;
    Ok(assignment)
}

/// Read and verify both per-surface role assignments and the active plan from
/// the repository state file. Workflow launchers can use this to fail closed
/// until role execution is wired to their dispatcher.
pub fn get_role_assignment_for_active_plan(
    repo_root: &Path,
    surface: &str,
    slot_id: Option<&str>,
) -> Result<RoleAssignment, String> {
    let state = load_active_plan_state(repo_root)?;
    let surf_state = state
        .surfaces
        .get(surface)
        .ok_or_else(|| format!("No active plan file is configured for {surface} surface"))?;
    let sid = slot_id.unwrap_or("primary");
    let active = surf_state
        .slots
        .get(sid)
        .map(|s| s.as_str())
        .ok_or_else(|| format!("No active plan file is configured for {surface} surface slot {sid:?}"))?;
    let path = validate_plan_path(repo_root, active)?;
    if !path.is_file() {
        return Err(format!("Active plan file does not exist: '{active}'"));
    }
    if !matches!(surface, "codex" | "antigravity") {
        return Err(format!("Unsupported Autocode surface: '{surface}'"));
    }
    Ok(state.role_assignments.get(surface).cloned().unwrap_or_default())
}

fn write_active_plan_state_unlocked(repo_root: &Path, state: &ActivePlanState) -> Result<(), String> {
    let plan_dir = repo_root.join(PLAN_DIR_NAME);
    if !plan_dir.exists() { std::fs::create_dir_all(&plan_dir).map_err(|e| format!("Failed to create .plan directory: {e}"))?; }
    let bytes = serde_json::to_vec_pretty(state).map_err(|e| format!("Failed to serialize active plan state: {e}"))?;
    let active_file = plan_dir.join(ACTIVE_PLAN_FILE_NAME);
    let nonce = TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let tmp_file = plan_dir.join(format!("{ACTIVE_PLAN_FILE_NAME}.tmp.{}.{}", std::process::id(), nonce));
    std::fs::write(&tmp_file, bytes).map_err(|e| format!("Failed to write temporary active plan file: {e}"))?;
    std::fs::rename(&tmp_file, &active_file).map_err(|e| format!("Failed to commit active plan file: {e}"))?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePlanRequest {
    pub name: String,
    pub folder: Option<String>,
    pub title: Option<String>,
    #[serde(default, alias = "setAsActive")]
    pub set_as_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditTrailEvent {
    pub phase: String,
    pub role_actor: String,
    pub result: String,
    pub notes: String,
    pub expected_sha256: String,
    #[serde(default)]
    pub expected_file_identity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionCheckpoint {
    pub checkpoint_version: u32,
    pub workflow: String,
    pub status: String,
    pub last_completed: String,
    pub next_action: String,
    pub repository: String,
    pub active_plan: String,
    pub plan_revision: String,
    #[serde(default)]
    pub plan_file_identity: String,
    pub workflow_run_id: String,
    #[serde(default)]
    pub completed_work: Vec<String>,
    #[serde(default)]
    pub modified_files: Vec<String>,
    #[serde(default)]
    pub validation: Vec<String>,
    #[serde(default)]
    pub reviewer: String,
    #[serde(default)]
    pub review_result: String,
    #[serde(default)]
    pub unresolved_findings: Vec<String>,
    #[serde(default)]
    pub pending_work: Vec<String>,
    #[serde(default)]
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResumptionPreflightResult {
    pub is_resumable: bool,
    pub next_action: String,
    pub active_plan: String,
    pub workflow: String,
    pub status: String,
    pub reasons: Vec<String>,
}

pub const CHECKPOINT_START_MARKER: &str = "<!-- POVERELLO-CHECKPOINT:START -->";
pub const CHECKPOINT_END_MARKER: &str = "<!-- POVERELLO-CHECKPOINT:END -->";
pub const CHECKPOINT_DATA_PREFIX: &str = "<!-- POVERELLO-CHECKPOINT-DATA:";
pub const CHECKPOINT_SECTION_HEADER: &str = "## Poverello Execution Checkpoint";

/// Resolves the repository root, either from an optional hint or by searching upward from the current working directory.
pub fn resolve_repo_root(hint: Option<&str>) -> Result<PathBuf, String> {
    if let Some(h) = hint {
        let trimmed = h.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            if p.is_dir() {
                return p.canonicalize().map_err(|e| format!("Failed to canonicalize repo path '{}': {e}", p.display()));
            }
        }
    }

    let mut cur = std::env::current_dir().map_err(|e| format!("Failed to get current directory: {e}"))?;
    for _ in 0..12 {
        if cur.join(".git").exists() || cur.join(PLAN_DIR_NAME).exists() {
            return cur.canonicalize().map_err(|e| format!("Failed to canonicalize repo path: {e}"));
        }
        if let Some(parent) = cur.parent() {
            cur = parent.to_path_buf();
        } else {
            break;
        }
    }

    std::env::current_dir()
        .map_err(|e| format!("Failed to get fallback current directory: {e}"))
        .and_then(|p| p.canonicalize().map_err(|e| format!("Failed to canonicalize directory: {e}")))
}

/// Validates a relative plan file path to prevent path traversal and ensure it targets a markdown file within the repository.
pub fn validate_plan_path(repo_root: &Path, rel_path: &str) -> Result<PathBuf, String> {
    let normalized = rel_path.trim().replace('\\', "/");
    if normalized.is_empty() {
        return Err("Plan file path cannot be empty".to_string());
    }

    if normalized.starts_with('/') || normalized.contains(':') || normalized.split('/').any(|seg| seg == "..") {
        return Err(format!("Invalid plan file path (traversal or absolute path detected): '{rel_path}'"));
    }

    if !normalized.ends_with(".md") {
        return Err(format!("Plan file must have a .md extension: '{rel_path}'"));
    }

    let target = repo_root.join(&normalized);
    let canonical_repo = repo_root.canonicalize().map_err(|e| format!("Cannot canonicalize repo root: {e}"))?;

    // Validate that nearest existing ancestor directory stays strictly within canonical repo
    let mut check_ancestor = target.parent();
    while let Some(anc) = check_ancestor {
        if anc.exists() {
            let canonical_anc = anc.canonicalize().map_err(|e| format!("Cannot canonicalize directory: {e}"))?;
            if !canonical_anc.starts_with(&canonical_repo) {
                return Err("Path traversal attempt detected: directory resolves outside repo root".to_string());
            }
            break;
        }
        check_ancestor = anc.parent();
    }

    // If target exists, ensure its canonical path stays strictly inside canonical repo
    if target.exists() {
        let canonical_target = target.canonicalize().map_err(|e| format!("Cannot canonicalize plan path: {e}"))?;
        if !canonical_target.starts_with(&canonical_repo) {
            return Err(format!("Path traversal attempt detected: '{rel_path}' resolves outside repo root"));
        }
        Ok(canonical_target)
    } else {
        Ok(target)
    }
}

pub fn extract_title_and_status(content: &str, fallback_name: &str) -> (String, Option<String>) {
    let mut title = String::new();
    let mut status = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if title.is_empty() && trimmed.starts_with("# ") {
            title = trimmed.trim_start_matches("# ").trim().to_string();
        }

        if status.is_none() {
            if let Some(idx) = trimmed.find("**Status:**") {
                let rest = trimmed[idx + "**Status:**".len()..].trim();
                let clean = rest.trim_matches('`').trim();
                if !clean.is_empty() {
                    status = Some(clean.to_string());
                }
            } else if let Some(idx) = trimmed.find("**状態:**") {
                let rest = trimmed[idx + "**状態:**".len()..].trim();
                let clean = rest.trim_matches('`').trim();
                if !clean.is_empty() {
                    status = Some(clean.to_string());
                }
            } else if let Some(idx) = trimmed.find("Status:") {
                let rest = trimmed[idx + "Status:".len()..].trim();
                let clean = rest.trim_matches('`').trim();
                if !clean.is_empty() {
                    status = Some(clean.to_string());
                }
            }
        }

        if !title.is_empty() && status.is_some() {
            break;
        }
    }

    if title.is_empty() {
        title = fallback_name.trim_end_matches(".md").to_string();
    }

    (title, status)
}

pub fn list_available_plans(repo_root: &Path, folder: Option<&str>) -> Result<Vec<PlanFileEntry>, String> {
    let canonical_repo = repo_root.canonicalize().map_err(|e| format!("Cannot canonicalize repo root: {e}"))?;

    let target_dir = if let Some(f) = folder {
        let trimmed = f.trim().replace('\\', "/");
        if trimmed.is_empty() || trimmed == "." {
            repo_root.to_path_buf()
        } else {
            if trimmed.starts_with('/') || trimmed.contains(':') || trimmed.split('/').any(|seg| seg == "..") {
                return Err(format!("Invalid folder path (traversal or absolute path detected): '{f}'"));
            }
            repo_root.join(&trimmed)
        }
    } else {
        let plan_dir = repo_root.join(PLAN_DIR_NAME);
        if plan_dir.is_dir() {
            plan_dir
        } else {
            return Ok(Vec::new());
        }
    };

    if !target_dir.exists() || !target_dir.is_dir() {
        return Ok(Vec::new());
    }

    let canonical_target = target_dir.canonicalize().map_err(|e| format!("Cannot canonicalize target folder: {e}"))?;
    if !canonical_target.starts_with(&canonical_repo) {
        return Err("Target folder resolves outside repo root".to_string());
    }

    let entries = std::fs::read_dir(&target_dir)
        .map_err(|e| format!("Failed to read plan directory '{}': {e}", target_dir.display()))?;

    let mut result = Vec::new();

    for entry_res in entries {
        let entry = match entry_res {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let file_name = entry.file_name().to_string_lossy().to_string();
        if !file_name.ends_with(".md") || file_name.starts_with('.') {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let mtime_unix = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let size = metadata.len();

        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let (title, _) = extract_title_and_status(&content, &file_name);
        let sha256_hash = format!("{:x}", Sha256::digest(content.as_bytes()));

        let canonical_file = match path.canonicalize() {
            Ok(cp) => cp,
            Err(_) => continue,
        };
        let rel_path = match canonical_file.strip_prefix(&canonical_repo) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };

        result.push(PlanFileEntry {
            rel_path,
            file_name,
            title,
            last_modified_unix: mtime_unix,
            file_size_bytes: size,
            sha256_hash,
        });
    }

    // Sort by last modified descending (newest first)
    result.sort_by(|a, b| b.last_modified_unix.cmp(&a.last_modified_unix));

    Ok(result)
}

pub fn get_active_plan(
    repo_root: &Path,
    surface: Option<&str>,
    slot_id: Option<&str>,
) -> Result<Option<PlanFileDetails>, String> {
    let active_plan_file = repo_root.join(PLAN_DIR_NAME).join(ACTIVE_PLAN_FILE_NAME);
    let rel_path = if active_plan_file.exists() {
        if let Ok(st) = load_active_plan_state(repo_root) {
            if let Some(surf) = surface {
                st.surfaces.get(surf).and_then(|s| {
                    let sid = slot_id.unwrap_or("primary");
                    s.slots.get(sid).cloned()
                })
            } else {
                st.active_plan_path
            }
        } else {
            None
        }
    } else {
        None
    };

    let Some(rel) = rel_path else {
        return Ok(None);
    };

    let trimmed = rel.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    // Safely validate that the stored path inside .active-plan.json is within repo root and has .md extension
    let abs_path = match validate_plan_path(repo_root, trimmed) {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };

    if !abs_path.exists() || !abs_path.is_file() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&abs_path).map_err(|e| format!("Failed to read plan file: {e}"))?;
    let file_name = abs_path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let (title, status) = extract_title_and_status(&content, &file_name);
    let sha256_hash = format!("{:x}", Sha256::digest(content.as_bytes()));
    let last_modified_unix = abs_path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let normalized_rel = trimmed.replace('\\', "/");

    Ok(Some(PlanFileDetails {
        rel_path: normalized_rel,
        abs_path: abs_path.to_string_lossy().to_string(),
        file_name,
        title,
        status,
        content,
        sha256_hash,
        file_identity: plan_file_identity(&abs_path).ok(),
        last_modified_unix,
    }))
}

pub fn get_plan_overview(
    repo_root: &Path,
    folder: Option<&str>,
    surface: Option<&str>,
    slot_id: Option<&str>,
) -> Result<PlanOverview, String> {
    let plans_dir = repo_root.join(PLAN_DIR_NAME);
    let active_details = get_active_plan(repo_root, surface, slot_id)?;
    let active_rel = active_details.as_ref().map(|d| d.rel_path.clone());
    let active_abs = active_details.as_ref().map(|d| d.abs_path.clone());

    let effective_folder: Option<String> = if let Some(f) = folder {
        let trimmed = f.trim().replace('\\', "/");
        if trimmed.is_empty() { None } else { Some(trimmed) }
    } else if let Some(ref ar) = active_rel {
        let parent = Path::new(ar).parent().and_then(|p| p.to_str()).unwrap_or(PLAN_DIR_NAME);
        let norm = parent.replace('\\', "/");
        Some(if norm.is_empty() { ".".to_string() } else { norm })
    } else if plans_dir.is_dir() {
        Some(PLAN_DIR_NAME.to_string())
    } else {
        None
    };

    let raw_entries = list_available_plans(repo_root, effective_folder.as_deref())?;
    let mut items = Vec::new();

    for e in raw_entries {
        let abs_path = repo_root.join(&e.rel_path);
        let is_active = active_rel.as_deref() == Some(&e.rel_path);

        let content = std::fs::read_to_string(&abs_path).unwrap_or_default();
        let (title, status) = extract_title_and_status(&content, &e.file_name);

        items.push(PlanFileItem {
            path: abs_path.to_string_lossy().to_string(),
            relative_path: e.rel_path,
            title: Some(title),
            status,
            last_modified_ms: e.last_modified_unix * 1000,
            is_active,
        });
    }

    Ok(PlanOverview {
        repo_root: repo_root.to_string_lossy().to_string(),
        plans_dir: plans_dir.to_string_lossy().to_string(),
        selected_folder: effective_folder,
        active_plan_path: active_abs,
        active_plan_relative_path: active_rel,
        plans: items,
    })
}

pub fn set_active_plan(
    repo_root: &Path,
    rel_path: &str,
    surface: Option<&str>,
    slot_id: Option<&str>,
) -> Result<PlanFileDetails, String> {
    let validated = validate_plan_path(repo_root, rel_path)?;
    if !validated.exists() || !validated.is_file() {
        return Err(format!("Plan file does not exist: '{rel_path}'"));
    }

    let abs_path = validated;
    let content = std::fs::read_to_string(&abs_path).map_err(|e| format!("Failed to read plan file: {e}"))?;
    let file_name = abs_path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let (title, status) = extract_title_and_status(&content, &file_name);
    let sha256_hash = format!("{:x}", Sha256::digest(content.as_bytes()));
    let last_modified_unix = abs_path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let normalized_rel = rel_path.trim().replace('\\', "/");

    // Persist active plan state in repo
    let _state_lock = ACTIVE_PLAN_STATE_LOCK.lock().map_err(|e| format!("Failed to acquire active plan state lock: {e}"))?;
    let plan_dir = repo_root.join(PLAN_DIR_NAME);
    if !plan_dir.exists() {
        std::fs::create_dir_all(&plan_dir).map_err(|e| format!("Failed to create .plan directory: {e}"))?;
    }

    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut state = load_active_plan_state(repo_root)?;
    let sid = slot_id.unwrap_or("primary");
    if let Some(surf) = surface {
        let entry = state.surfaces.entry(surf.to_string()).or_default();
        entry.slots.insert(sid.to_string(), normalized_rel.clone());
        if sid == "primary" {
            entry.active_plan_path = Some(normalized_rel.clone());
        }
        // Do NOT populate or overwrite the shared scalar with a surface-specific plan
    } else {
        state.active_plan_path = Some(normalized_rel.clone());
        for surf in ["antigravity", "codex"] {
            let entry = state.surfaces.entry(surf.to_string()).or_default();
            entry.slots.insert(sid.to_string(), normalized_rel.clone());
            if sid == "primary" {
                entry.active_plan_path = Some(normalized_rel.clone());
            }
        }
    }
    state.last_updated_unix = now_unix;
    write_active_plan_state_unlocked(repo_root, &state)?;

    Ok(PlanFileDetails {
        rel_path: normalized_rel,
        abs_path: abs_path.to_string_lossy().to_string(),
        file_name,
        title,
        status,
        content,
        sha256_hash,
        file_identity: plan_file_identity(&abs_path).ok(),
        last_modified_unix,
    })
}

pub fn clear_active_plan_slot(
    repo_root: &Path,
    surface: &str,
    slot_id: &str,
) -> Result<(), String> {
    let _state_lock = ACTIVE_PLAN_STATE_LOCK.lock().map_err(|e| format!("Failed to acquire active plan state lock: {e}"))?;
    let mut state = load_active_plan_state(repo_root)?;
    if let Some(surf_state) = state.surfaces.get_mut(surface) {
        surf_state.slots.remove(slot_id);
        if slot_id == "primary" {
            surf_state.active_plan_path = None;
        }
        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        state.last_updated_unix = now_unix;
        write_active_plan_state_unlocked(repo_root, &state)?;
    }
    Ok(())
}

pub fn create_plan_file(repo_root: &Path, req: &CreatePlanRequest) -> Result<PlanFileDetails, String> {
    let clean_name = req.name.trim();
    if clean_name.is_empty() {
        return Err("Plan name cannot be empty".to_string());
    }

    let file_name = if clean_name.ends_with(".md") {
        clean_name.to_string()
    } else {
        format!("{clean_name}.md")
    };

    // Prevent directory traversal or invalid characters in file_name
    if file_name.contains('/') || file_name.contains('\\') || file_name.contains(':') || file_name.contains("..") {
        return Err(format!("Invalid characters in plan filename: '{clean_name}'"));
    }

    let folder_str = req.folder.as_deref().unwrap_or(PLAN_DIR_NAME).trim().replace('\\', "/");
    let target_dir = if folder_str.is_empty() || folder_str == "." {
        repo_root.to_path_buf()
    } else {
        if folder_str.starts_with('/') || folder_str.contains(':') || folder_str.split('/').any(|seg| seg == "..") {
            return Err(format!("Invalid folder path (traversal or absolute path detected): '{folder_str}'"));
        }
        repo_root.join(&folder_str)
    };

    let canonical_repo = repo_root.canonicalize().map_err(|e| format!("Cannot canonicalize repo root: {e}"))?;

    // Validate the nearest existing ancestor before creating anything. A missing
    // child beneath an in-repository symlink can otherwise cause create_dir_all
    // to write outside the repository before the later canonical-path check.
    let mut ancestor = target_dir.as_path();
    while !ancestor.exists() {
        ancestor = ancestor.parent().ok_or_else(|| "Cannot find existing ancestor for plan directory".to_string())?;
    }
    let canonical_ancestor = ancestor.canonicalize().map_err(|e| format!("Cannot canonicalize plan directory ancestor: {e}"))?;
    if !canonical_ancestor.starts_with(&canonical_repo) {
        return Err("Target folder resolves outside repo root".to_string());
    }

    if !target_dir.exists() {
        std::fs::create_dir_all(&target_dir).map_err(|e| format!("Failed to create plan directory '{}': {e}", target_dir.display()))?;
    }

    let canonical_target_dir = target_dir.canonicalize().map_err(|e| format!("Cannot canonicalize target directory: {e}"))?;
    if !canonical_target_dir.starts_with(&canonical_repo) {
        return Err("Target folder resolves outside repo root".to_string());
    }

    let target_path = target_dir.join(&file_name);
    if target_path.exists() {
        return Err(format!("Plan file already exists: '{file_name}'"));
    }

    let rel_path = if folder_str.is_empty() || folder_str == "." {
        file_name.clone()
    } else {
        format!("{folder_str}/{file_name}")
    };

    let title = match &req.title {
        Some(t) if !t.trim().is_empty() => t.trim().to_string(),
        _ => file_name.trim_end_matches(".md").to_string(),
    };

    let now = chrono::Utc::now().to_rfc3339();
    let template = format!(
r#"# {title}

- **Plan ID:** `{plan_id}`
- **Status:** `PROPOSED / DRAFT`
- **Created At:** `{now}`

---

## 1. Overview & Objectives

{title}

---

## 2. Requirements & Scope

- **Allowed Scope:**
- **Forbidden / Out of Scope:**

---

## 3. Implementation Plan

### Step 1:
### Step 2:
### Step 3:

---

## 4. Verification & Acceptance Criteria

- [ ] Automated Tests:
- [ ] Manual Smoke:

---

## 5. Execution History & Audit Trail

| Timestamp | Phase | Role / Actor | Result | Notes |
| --- | --- | --- | --- | --- |
| {now} | Created | Planner | Draft | Initial creation |
"#,
        title = title,
        plan_id = file_name.trim_end_matches(".md"),
        now = now
    );

    // Atomic write
    let tmp_path = target_dir.join(format!("{file_name}.tmp.{}", std::process::id()));
    std::fs::write(&tmp_path, &template)
        .map_err(|e| format!("Failed to write temporary plan file: {e}"))?;
    std::fs::rename(&tmp_path, &target_path)
        .map_err(|e| format!("Failed to commit new plan file: {e}"))?;

    if req.set_as_active.unwrap_or(true) {
        set_active_plan(repo_root, &rel_path, None, None)
    } else {
        let abs_path = target_path;
        let content = std::fs::read_to_string(&abs_path).map_err(|e| format!("Failed to read plan file: {e}"))?;
        let (title, status) = extract_title_and_status(&content, &file_name);
        let sha256_hash = format!("{:x}", Sha256::digest(content.as_bytes()));
        let last_modified_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Ok(PlanFileDetails {
            rel_path,
            abs_path: abs_path.to_string_lossy().to_string(),
            file_name,
            title,
            status,
            content,
            sha256_hash,
            file_identity: plan_file_identity(&abs_path).ok(),
            last_modified_unix,
        })
    }
}

static PLAN_MUTATION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
static TMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn plan_file_identity(path: &Path) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Failed to open plan for identity check: {e}"))?;
    let id = crate::pov_nerf::FileIdentity::from_file(&file)
        .ok_or_else(|| "Could not determine plan file identity".to_string())?;
    #[cfg(windows)]
    { return Ok(format!("{}:{}", id.volume_serial_number, id.file_index)); }
    #[cfg(unix)]
    { return Ok(format!("{}:{}", id.dev, id.ino)); }
    #[cfg(not(any(windows, unix)))]
    { let _ = id; Err("Plan file identity is unsupported on this platform".to_string()) }
}

pub fn apply_plan_candidate_if_unchanged(
    repo_root: &Path,
    rel_path: &str,
    expected_file_identity: &str,
    expected_sha256: &str,
    candidate: &str,
) -> Result<PlanFileDetails, String> {
    if expected_file_identity.trim().is_empty() || expected_sha256.trim().is_empty() {
        return Err("Plan candidate update requires expected file identity and SHA-256".to_string());
    }
    let abs_path = validate_plan_path(repo_root, rel_path)?;
    let _lock = PLAN_MUTATION_LOCK.lock().map_err(|e| format!("Failed to acquire plan mutation lock: {e}"))?;
    let current = std::fs::read_to_string(&abs_path).map_err(|e| format!("Failed to read plan: {e}"))?;
    let current_hash = format!("{:x}", Sha256::digest(current.as_bytes()));
    let current_identity = plan_file_identity(&abs_path)?;
    if current_hash != expected_sha256 || current_identity != expected_file_identity {
        return Err("Plan candidate compare-and-swap conflict: file identity or SHA-256 changed".to_string());
    }
    let counter = TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let tmp_path = abs_path.with_extension(format!("tmp.{}.{}", std::process::id(), counter));
    std::fs::write(&tmp_path, candidate).map_err(|e| format!("Failed to write plan candidate: {e}"))?;
    let before_commit = std::fs::read_to_string(&abs_path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        format!("Failed to re-read plan before candidate commit: {e}")
    })?;
    let before_hash = format!("{:x}", Sha256::digest(before_commit.as_bytes()));
    let before_identity = plan_file_identity(&abs_path)?;
    if before_hash != expected_sha256 || before_identity != expected_file_identity {
        let _ = std::fs::remove_file(&tmp_path);
        return Err("Plan candidate compare-and-swap conflict before commit".to_string());
    }
    std::fs::rename(&tmp_path, &abs_path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        format!("Failed to atomically commit plan candidate: {e}")
    })?;
    let details = plan_details(rel_path, abs_path)?;
    if details.file_identity.is_none() {
        return Err("Could not determine the updated plan file identity".to_string());
    }
    Ok(details)
}

fn plan_details(rel_path: &str, abs_path: PathBuf) -> Result<PlanFileDetails, String> {
    let content = std::fs::read_to_string(&abs_path).map_err(|e| format!("Failed to read updated plan: {e}"))?;
    let file_name = abs_path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let (title, status) = extract_title_and_status(&content, &file_name);
    let sha256_hash = format!("{:x}", Sha256::digest(content.as_bytes()));
    let file_identity = plan_file_identity(&abs_path).ok();
    let last_modified_unix = abs_path.metadata().ok().and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    Ok(PlanFileDetails { rel_path: rel_path.trim().replace('\\', "/"), abs_path: abs_path.to_string_lossy().to_string(), file_name, title, status, content, sha256_hash, file_identity, last_modified_unix })
}

pub fn append_audit_trail_event(
    repo_root: &Path,
    rel_path: &str,
    event: &AuditTrailEvent,
) -> Result<PlanFileDetails, String> {
    if event.expected_sha256.trim().is_empty() || event.expected_file_identity.as_deref().unwrap_or("").trim().is_empty() {
        return Err("AuditTrailEvent requires non-empty expected_file_identity and expected_sha256 for Compare-And-Swap (CAS) integrity".to_string());
    }

    let abs_path = validate_plan_path(repo_root, rel_path)?;
    if !abs_path.exists() || !abs_path.is_file() {
        return Err(format!("Plan file does not exist: '{rel_path}'"));
    }

    let _lock = PLAN_MUTATION_LOCK
        .lock()
        .map_err(|e| format!("Failed to acquire plan mutation lock: {e}"))?;

    let content = std::fs::read_to_string(&abs_path)
        .map_err(|e| format!("Failed to read plan file: {e}"))?;
    // Compare-And-Swap verification
    let current_hash = format!("{:x}", Sha256::digest(content.as_bytes()));
    if !current_hash.eq_ignore_ascii_case(event.expected_sha256.trim()) {
        return Err(format!(
            "Compare-and-swap conflict on plan '{rel_path}': expected hash '{}', got '{current_hash}'",
            event.expected_sha256
        ));
    }
    let expected_identity = event.expected_file_identity.as_deref().expect("validated above");
    if plan_file_identity(&abs_path)? != expected_identity {
        return Err("Compare-and-swap conflict: plan file identity changed".to_string());
    }

    let now = chrono::Utc::now().to_rfc3339();
    let row = format!(
        "| {} | {} | {} | {} | {} |\n",
        now, event.phase, event.role_actor, event.result, event.notes
    );

    let updated_content = append_audit_trail_row(&content, &row);

    let counter = TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let tmp_path = abs_path.with_extension(format!("tmp.{}.{}", std::process::id(), counter));
    std::fs::write(&tmp_path, &updated_content)
        .map_err(|e| format!("Failed to write temporary updated plan: {e}"))?;

    // Pre-commit verification: re-read source file directly before atomic rename
    let pre_commit_content = std::fs::read_to_string(&abs_path)
        .map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            format!("Failed to re-read plan file before commit: {e}")
        })?;
    let pre_commit_hash = format!("{:x}", Sha256::digest(pre_commit_content.as_bytes()));
    if !pre_commit_hash.eq_ignore_ascii_case(event.expected_sha256.trim()) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!(
            "Compare-and-swap conflict on plan '{rel_path}' prior to commit: expected hash '{}', got '{pre_commit_hash}'",
            event.expected_sha256
        ));
    }
    if plan_file_identity(&abs_path)? != expected_identity {
        let _ = std::fs::remove_file(&tmp_path);
        return Err("Compare-and-swap conflict prior to commit: plan file identity changed".to_string());
    }

    std::fs::rename(&tmp_path, &abs_path)
        .map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            format!("Failed to commit updated plan with audit trail: {e}")
        })?;

    let file_name = abs_path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let (title, status) = extract_title_and_status(&updated_content, &file_name);
    let sha256_hash = format!("{:x}", Sha256::digest(updated_content.as_bytes()));
    let last_modified_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Ok(PlanFileDetails {
        rel_path: rel_path.trim().replace('\\', "/"),
        abs_path: abs_path.to_string_lossy().to_string(),
        file_name,
        title,
        status,
        content: updated_content,
        sha256_hash,
        file_identity: plan_file_identity(&abs_path).ok(),
        last_modified_unix,
    })
}

fn append_audit_trail_row(content: &str, row: &str) -> String {
    let accepted_headers = [
        "## 5. Execution History & Audit Trail",
        "## 11. Execution History & Audit Trail",
        "## Execution History & Audit Trail",
        "## Audit Trail",
    ];

    let mut fence: Option<(u8, usize)> = None;
    let mut offset = 0;
    let mut section: Option<(usize, usize)> = None;
    for line in content.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let bytes = trimmed.as_bytes();
        if let Some(ch @ (b'`' | b'~')) = bytes.first().copied() {
            let run_len = bytes.iter().take_while(|&&b| b == ch).count();
            if run_len >= 3 {
                match fence {
                    None => fence = Some((ch, run_len)),
                    Some((open_ch, open_len)) if ch == open_ch && run_len >= open_len => fence = None,
                    _ => {}
                }
            }
        } else if fence.is_none() {
            let heading = line.trim_end_matches(|ch| ch == '\r' || ch == '\n');
            if section.is_none() && accepted_headers.contains(&heading) {
                section = Some((offset, offset + line.len()));
            } else if let Some((start, _)) = section {
                if heading.starts_with("## ") {
                    section = Some((start, offset));
                    break;
                }
                section = Some((start, offset + line.len()));
            }
        }
        offset += line.len();
    }

    if let Some((start, end)) = section {
        let section_text = &content[start..end];
        if let Some(last_row) = section_text.rfind("\n|") {
            let row_start = last_row + 1;
            let insertion = section_text[row_start..]
                .find('\n')
                .map(|line_end| start + row_start + line_end + 1)
                .unwrap_or(end);
            return format!("{}{}{}", &content[..insertion], row, &content[insertion..]);
        }
        let insertion = end;
        return format!("{}\n{}{}", content[..insertion].trim_end_matches(|ch| ch == '\r' || ch == '\n'), row, &content[insertion..]);
    }

    format!(
        "{}\n\n## Audit Trail\n\n| Timestamp | Phase | Role / Actor | Result | Notes |\n| --- | --- | --- | --- | --- |\n{}",
        content.trim_end(),
        row
    )
}

pub fn render_checkpoint_markdown(cp: &ExecutionCheckpoint) -> String {
    let mut out = String::new();
    out.push_str(CHECKPOINT_SECTION_HEADER);
    out.push_str("\n\n");
    out.push_str(CHECKPOINT_START_MARKER);
    out.push('\n');
    if let Ok(json_str) = serde_json::to_string(cp) {
        out.push_str(CHECKPOINT_DATA_PREFIX);
        out.push(' ');
        out.push_str(&json_str);
        out.push_str(" -->\n");
    }
    out.push_str(&format!("Checkpoint Version: {}\n", cp.checkpoint_version));
    out.push_str(&format!("Workflow: {}\n", cp.workflow));
    out.push_str(&format!("Status: {}\n", cp.status));
    out.push_str(&format!("Last Completed: {}\n", cp.last_completed));
    out.push_str(&format!("Next Action: {}\n", cp.next_action));
    if !cp.timestamp.is_empty() {
        out.push_str(&format!("Timestamp: {}\n", cp.timestamp));
    }
    out.push('\n');
    out.push_str("### Execution Identity\n");
    out.push_str(&format!("- Repository: {}\n", cp.repository));
    out.push_str(&format!("- Active Plan: {}\n", cp.active_plan));
    out.push_str(&format!("- Plan Revision: {}\n", cp.plan_revision));
    out.push_str(&format!("- Plan File Identity: {}\n", cp.plan_file_identity));
    out.push_str(&format!("- Workflow Run ID: {}\n", cp.workflow_run_id));
    out.push('\n');
    out.push_str("### Completed Work\n");
    if cp.completed_work.is_empty() {
        out.push_str("- None\n");
    } else {
        for item in &cp.completed_work {
            out.push_str(&format!("- {}\n", item));
        }
    }
    out.push('\n');
    out.push_str("### Modified Files\n");
    if cp.modified_files.is_empty() {
        out.push_str("- None\n");
    } else {
        for file in &cp.modified_files {
            out.push_str(&format!("- {}\n", file));
        }
    }
    out.push('\n');
    out.push_str("### Validation\n");
    if cp.validation.is_empty() {
        out.push_str("- None\n");
    } else {
        for val in &cp.validation {
            out.push_str(&format!("- {}\n", val));
        }
    }
    out.push('\n');
    out.push_str("### Review State\n");
    out.push_str(&format!("- Reviewer: {}\n", if cp.reviewer.is_empty() { "None" } else { &cp.reviewer }));
    out.push_str(&format!("- Result: {}\n", if cp.review_result.is_empty() { "PENDING" } else { &cp.review_result }));
    out.push_str("- Unresolved Findings:\n");
    if cp.unresolved_findings.is_empty() {
        out.push_str("  - None\n");
    } else {
        for finding in &cp.unresolved_findings {
            out.push_str(&format!("  - {}\n", finding));
        }
    }
    out.push('\n');
    out.push_str("### Pending Work\n");
    if cp.pending_work.is_empty() {
        out.push_str("- None\n");
    } else {
        for item in &cp.pending_work {
            out.push_str(&format!("- {}\n", item));
        }
    }
    out.push('\n');
    out.push_str(CHECKPOINT_END_MARKER);
    out
}

fn checkpoint_revision_digest(content: &str) -> String {
    let revision_content = if let Some((start_idx, end_idx)) = find_checkpoint_markers(content) {
        let section_start = content[..start_idx].rfind(CHECKPOINT_SECTION_HEADER).unwrap_or(start_idx);
        let after_end = end_idx + CHECKPOINT_END_MARKER.len();
        let prefix = content[..section_start].trim_end();
        let suffix = content[after_end..].trim_start();
        if suffix.starts_with("---") {
            let suffix = suffix.trim_start_matches('-').trim_start();
            format!("{}\n\n{}", prefix, suffix)
        } else {
            format!("{}{}", prefix, suffix)
        }
    } else {
        content.to_string()
    };
    format!("{:x}", Sha256::digest(revision_content.as_bytes()))
}

pub fn extract_execution_checkpoint(content: &str) -> Option<ExecutionCheckpoint> {
    let (start_idx, end_idx) = find_checkpoint_markers(content)?;
    let block_start = start_idx + CHECKPOINT_START_MARKER.len();
    let block = &content[block_start..end_idx];

    // Priority 1: Exact structured JSON comment if present
    if let Some(json_start) = block.find(CHECKPOINT_DATA_PREFIX) {
        let after_prefix = &block[json_start + CHECKPOINT_DATA_PREFIX.len()..];
        if let Some(json_end) = after_prefix.find("-->") {
            let json_slice = after_prefix[..json_end].trim();
            if let Ok(cp) = serde_json::from_str::<ExecutionCheckpoint>(json_slice) {
                return Some(cp);
            }
        }
    }

    // Priority 2: Fallback markdown parsing
    let mut cp = ExecutionCheckpoint {
        checkpoint_version: 1,
        workflow: String::new(),
        status: String::new(),
        last_completed: String::new(),
        next_action: String::new(),
        repository: String::new(),
        active_plan: String::new(),
        plan_revision: String::new(),
        plan_file_identity: String::new(),
        workflow_run_id: String::new(),
        completed_work: Vec::new(),
        modified_files: Vec::new(),
        validation: Vec::new(),
        reviewer: String::new(),
        review_result: String::new(),
        unresolved_findings: Vec::new(),
        pending_work: Vec::new(),
        timestamp: String::new(),
    };

    let mut current_section = "";
    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Checkpoint Version:") {
            if let Ok(v) = trimmed["Checkpoint Version:".len()..].trim().parse::<u32>() {
                cp.checkpoint_version = v;
            }
        } else if trimmed.starts_with("Workflow:") {
            cp.workflow = trimmed["Workflow:".len()..].trim().to_string();
        } else if trimmed.starts_with("Status:") {
            cp.status = trimmed["Status:".len()..].trim().to_string();
        } else if trimmed.starts_with("Last Completed:") {
            cp.last_completed = trimmed["Last Completed:".len()..].trim().to_string();
        } else if trimmed.starts_with("Next Action:") {
            cp.next_action = trimmed["Next Action:".len()..].trim().to_string();
        } else if trimmed.starts_with("Timestamp:") {
            cp.timestamp = trimmed["Timestamp:".len()..].trim().to_string();
        } else if trimmed.starts_with("### Execution Identity") {
            current_section = "identity";
        } else if trimmed.starts_with("### Completed Work") {
            current_section = "completed";
        } else if trimmed.starts_with("### Modified Files") {
            current_section = "files";
        } else if trimmed.starts_with("### Validation") {
            current_section = "validation";
        } else if trimmed.starts_with("### Review State") {
            current_section = "review";
        } else if trimmed.starts_with("### Pending Work") {
            current_section = "pending";
        } else if current_section == "identity" {
            if trimmed.starts_with("- Repository:") {
                cp.repository = trimmed["- Repository:".len()..].trim().to_string();
            } else if trimmed.starts_with("- Active Plan:") {
                cp.active_plan = trimmed["- Active Plan:".len()..].trim().to_string();
            } else if trimmed.starts_with("- Plan Revision:") {
                cp.plan_revision = trimmed["- Plan Revision:".len()..].trim().to_string();
            } else if trimmed.starts_with("- Plan File Identity:") {
                cp.plan_file_identity = trimmed["- Plan File Identity:".len()..].trim().to_string();
            } else if trimmed.starts_with("- Workflow Run ID:") {
                cp.workflow_run_id = trimmed["- Workflow Run ID:".len()..].trim().to_string();
            }
        } else if current_section == "completed" && trimmed.starts_with("- ") {
            let item = trimmed[2..].trim();
            if !item.eq_ignore_ascii_case("None") && !item.is_empty() {
                cp.completed_work.push(item.to_string());
            }
        } else if current_section == "files" && trimmed.starts_with("- ") {
            let file = trimmed[2..].trim();
            if !file.eq_ignore_ascii_case("None") && !file.is_empty() {
                cp.modified_files.push(file.to_string());
            }
        } else if current_section == "validation" && trimmed.starts_with("- ") {
            let val = trimmed[2..].trim();
            if !val.eq_ignore_ascii_case("None") && !val.is_empty() {
                cp.validation.push(val.to_string());
            }
        } else if current_section == "review" {
            if trimmed.starts_with("- Reviewer:") {
                let rev = trimmed["- Reviewer:".len()..].trim();
                if !rev.eq_ignore_ascii_case("None") {
                    cp.reviewer = rev.to_string();
                }
            } else if trimmed.starts_with("- Result:") {
                cp.review_result = trimmed["- Result:".len()..].trim().to_string();
            } else if trimmed.starts_with("- ") || trimmed.starts_with("-") {
                let finding = trimmed.trim_start_matches('-').trim();
                if !finding.eq_ignore_ascii_case("None") && !finding.eq_ignore_ascii_case("Unresolved Findings:") && !finding.is_empty() {
                    cp.unresolved_findings.push(finding.to_string());
                }
            }
        } else if current_section == "pending" && trimmed.starts_with("- ") {
            let item = trimmed[2..].trim();
            if !item.eq_ignore_ascii_case("None") && !item.is_empty() {
                cp.pending_work.push(item.to_string());
            }
        }
    }

    Some(cp)
}

/// Finds the sole complete checkpoint marker pair outside Markdown fenced code.
/// Marker examples in documentation are deliberately ignored.
fn find_checkpoint_markers(content: &str) -> Option<(usize, usize)> {
    let mut fence: Option<(u8, usize)> = None;
    let mut start = None;
    let mut end = None;
    let mut offset = 0;

    for line in content.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let bytes = trimmed.as_bytes();
        let fence_char = bytes.first().copied();
        if matches!(fence_char, Some(b'`' | b'~')) {
            let ch = fence_char.unwrap();
            let run_len = bytes.iter().take_while(|&&b| b == ch).count();
            if run_len >= 3 {
                match fence {
                    None => fence = Some((ch, run_len)),
                    Some((open_ch, open_len)) if ch == open_ch && run_len >= open_len => fence = None,
                    _ => {}
                }
            }
        } else if fence.is_none() {
            if let Some(pos) = line.find(CHECKPOINT_START_MARKER) {
                if start.is_some() { return None; }
                start = Some(offset + pos);
            }
            if let Some(pos) = line.find(CHECKPOINT_END_MARKER) {
                if end.is_some() { return None; }
                end = Some(offset + pos);
            }
        }
        offset += line.len();
    }

    match (start, end) {
        (Some(s), Some(e)) if s < e => Some((s, e)),
        (None, None) => None,
        _ => None,
    }
}

pub fn save_execution_checkpoint(
    repo_root: &Path,
    rel_path: &str,
    checkpoint: &ExecutionCheckpoint,
    expected_file_identity: &str,
    expected_sha256: &str,
) -> Result<PlanFileDetails, String> {
    if expected_file_identity.trim().is_empty() || expected_sha256.trim().is_empty() {
        return Err("save_execution_checkpoint requires non-empty expected_file_identity and expected_sha256 for CAS integrity".to_string());
    }

    let abs_path = validate_plan_path(repo_root, rel_path)?;
    if !abs_path.exists() || !abs_path.is_file() {
        return Err(format!("Plan file does not exist: '{rel_path}'"));
    }

    let _lock = PLAN_MUTATION_LOCK
        .lock()
        .map_err(|e| format!("Failed to acquire plan mutation lock: {e}"))?;

    let content = std::fs::read_to_string(&abs_path)
        .map_err(|e| format!("Failed to read plan file: {e}"))?;
    let source_file_identity = plan_file_identity(&abs_path)?;
    if source_file_identity != expected_file_identity {
        return Err("Compare-and-swap conflict: plan file identity changed".to_string());
    }

    // Compare-And-Swap verification
    let current_hash = format!("{:x}", Sha256::digest(content.as_bytes()));
    if !current_hash.eq_ignore_ascii_case(expected_sha256.trim()) {
        return Err(format!(
            "Compare-and-swap conflict on plan '{rel_path}': expected hash '{expected_sha256}', got '{current_hash}'"
        ));
    }

    // Hash all human/audit content while excluding the checkpoint itself. Store
    // this stable revision digest in the checkpoint so its own save is not
    // self-referential and repeated checkpoint updates keep the same revision.
    let mut checkpoint_to_save = checkpoint.clone();
    checkpoint_to_save.plan_revision = checkpoint_revision_digest(&content);
    checkpoint_to_save.plan_file_identity.clear();
    let rendered_checkpoint = render_checkpoint_markdown(&checkpoint_to_save);

    let updated_content = if let Some((start_marker_idx, end_marker_start_idx)) = find_checkpoint_markers(&content) {
        let end_marker_idx = end_marker_start_idx + CHECKPOINT_END_MARKER.len();
        let section_start = match content[..start_marker_idx].rfind(CHECKPOINT_SECTION_HEADER) {
            Some(idx) => idx,
            None => start_marker_idx,
        };
        format!(
            "{}{}{}",
            &content[..section_start],
            rendered_checkpoint,
            &content[end_marker_idx..]
        )
    } else {
        // Insert before Audit Trail section if present
        let audit_markers = [
            "## 5. Execution History & Audit Trail",
            "## Execution History & Audit Trail",
            "## Audit Trail",
            "## 11. Execution History & Audit Trail",
        ];
        let mut insert_pos = None;
        for marker in audit_markers {
            if let Some(pos) = content.find(marker) {
                insert_pos = Some(pos);
                break;
            }
        }

        if let Some(pos) = insert_pos {
            format!(
                "{}\n\n{}\n\n---\n\n{}",
                content[..pos].trim_end(),
                rendered_checkpoint,
                &content[pos..]
            )
        } else {
            format!(
                "{}\n\n---\n\n{}\n",
                content.trim_end(),
                rendered_checkpoint
            )
        }
    };

    let counter = TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let tmp_path = abs_path.with_extension(format!("tmp.{}.{}", std::process::id(), counter));
    std::fs::write(&tmp_path, &updated_content)
        .map_err(|e| format!("Failed to write temporary updated plan: {e}"))?;

    // A checkpoint replacement changes the plan file identity. Record the
    // identity of the already-created temporary file, then rewrite its bytes
    // in place so rename preserves that identity without self-reference.
    checkpoint_to_save.plan_file_identity = plan_file_identity(&tmp_path)?;
    let final_rendered_checkpoint = render_checkpoint_markdown(&checkpoint_to_save);
    let final_content = updated_content.replacen(&rendered_checkpoint, &final_rendered_checkpoint, 1);
    if final_content == updated_content {
        let _ = std::fs::remove_file(&tmp_path);
        return Err("Could not embed checkpoint plan file identity".to_string());
    }
    std::fs::write(&tmp_path, &final_content).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        format!("Failed to finalize temporary checkpoint: {e}")
    })?;

    // Pre-commit verification: re-read source file directly before atomic rename
    let pre_commit_content = std::fs::read_to_string(&abs_path)
        .map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            format!("Failed to re-read plan file before commit: {e}")
        })?;
    let pre_commit_hash = format!("{:x}", Sha256::digest(pre_commit_content.as_bytes()));
    if !pre_commit_hash.eq_ignore_ascii_case(expected_sha256.trim()) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!(
            "Compare-and-swap conflict on plan '{rel_path}' prior to commit: expected hash '{expected_sha256}', got '{pre_commit_hash}'"
        ));
    }
    if plan_file_identity(&abs_path)? != source_file_identity {
        let _ = std::fs::remove_file(&tmp_path);
        return Err("Compare-and-swap conflict prior to checkpoint commit: plan file identity changed".to_string());
    }

    std::fs::rename(&tmp_path, &abs_path)
        .map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            format!("Failed to commit updated plan with checkpoint: {e}")
        })?;

    let file_name = abs_path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let (title, status) = extract_title_and_status(&final_content, &file_name);
    let sha256_hash = format!("{:x}", Sha256::digest(final_content.as_bytes()));
    let last_modified_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Ok(PlanFileDetails {
        rel_path: rel_path.trim().replace('\\', "/"),
        abs_path: abs_path.to_string_lossy().to_string(),
        file_name,
        title,
        status,
        content: final_content,
        sha256_hash,
        file_identity: plan_file_identity(&abs_path).ok(),
        last_modified_unix,
    })
}

pub fn get_execution_checkpoint(
    repo_root: &Path,
    rel_path: &str,
) -> Result<Option<ExecutionCheckpoint>, String> {
    let abs_path = validate_plan_path(repo_root, rel_path)?;
    if !abs_path.exists() || !abs_path.is_file() {
        return Err(format!("Plan file does not exist: '{rel_path}'"));
    }

    let content = std::fs::read_to_string(&abs_path)
        .map_err(|e| format!("Failed to read plan file: {e}"))?;

    Ok(extract_execution_checkpoint(&content))
}

pub fn verify_resumption_preflight(
    repo_root: &Path,
    rel_path: &str,
    checkpoint: &ExecutionCheckpoint,
    surface: Option<&str>,
    slot_id: Option<&str>,
) -> Result<ResumptionPreflightResult, String> {
    let canonical_repo = repo_root.canonicalize().map_err(|e| format!("Cannot canonicalize repo root: {e}"))?;
    let mut reasons = Vec::new();
    let mut is_resumable = true;

    // 1. Verify repository identity; absent or unverifiable identity fails closed.
    let cp_repo_path = PathBuf::from(checkpoint.repository.trim());
    match cp_repo_path.canonicalize() {
        Ok(canonical_cp_repo) if canonical_cp_repo == canonical_repo => {}
        Ok(canonical_cp_repo) => {
            is_resumable = false;
            reasons.push(format!("Repository mismatch: checkpoint repo '{}' differs from current repo '{}'", canonical_cp_repo.display(), canonical_repo.display()));
        }
        Err(_) => {
            is_resumable = false;
            reasons.push("Checkpoint repository identity is missing or cannot be verified".to_string());
        }
    }

    // 2. Verify active plan path
    let normalized_rel = rel_path.trim().replace('\\', "/");
    let normalized_cp_plan = checkpoint.active_plan.trim().replace('\\', "/");
    if normalized_cp_plan.is_empty() || normalized_rel != normalized_cp_plan {
        is_resumable = false;
        reasons.push(format!(
            "Plan file path is missing or mismatched: checkpoint references '{}', requested '{}'",
            normalized_cp_plan, normalized_rel
        ));
    }
    let state = load_active_plan_state(repo_root)?;
    let matches_active = if let Some(s) = surface {
        let sid = slot_id.unwrap_or("primary");
        state
            .surfaces
            .get(s)
            .and_then(|st| st.slots.get(sid))
            .map(|p| p.replace('\\', "/"))
            == Some(normalized_rel.clone())
    } else {
        state.active_plan_path.as_deref().map(|p| p.replace('\\', "/")) == Some(normalized_rel.clone())
            || state.surfaces.values().any(|s| {
                s.active_plan_path.as_deref().map(|p| p.replace('\\', "/")) == Some(normalized_rel.clone())
                    || s.slots.values().any(|p| p.replace('\\', "/") == normalized_rel)
            })
    };
    if !matches_active {
        is_resumable = false;
        reasons.push("Requested plan does not match the repository's configured Active Plan File".to_string());
    }

    // Unknown checkpoint fields cannot safely select a workflow stage.
    if checkpoint.checkpoint_version != 1
        || checkpoint.workflow.trim().is_empty()
        || checkpoint.status.trim().is_empty()
        || checkpoint.next_action.trim().is_empty()
    {
        is_resumable = false;
        reasons.push("Checkpoint workflow state is incomplete or uses an unsupported version".to_string());
    }

    // 3. Verify plan file exists
    let abs_path = validate_plan_path(repo_root, rel_path)?;
    if !abs_path.exists() {
        is_resumable = false;
        reasons.push(format!("Plan file not found on disk: '{rel_path}'"));
    }

    // Checkpoint revision is the hash of the plan with its live checkpoint block
    // removed, avoiding a self-referential digest while detecting external edits.
    if abs_path.exists() {
        let plan_content = std::fs::read_to_string(&abs_path).map_err(|e| format!("Failed to read plan for revision check: {e}"))?;
        let actual_revision = checkpoint_revision_digest(&plan_content);
        if checkpoint.plan_revision.trim().is_empty() || !checkpoint.plan_revision.eq_ignore_ascii_case(&actual_revision) {
            is_resumable = false;
            reasons.push("Plan revision hash is missing or does not match the current plan".to_string());
        }
        if checkpoint.plan_file_identity.trim().is_empty()
            || plan_file_identity(&abs_path).map(|identity| identity != checkpoint.plan_file_identity).unwrap_or(true)
        {
            is_resumable = false;
            reasons.push("Plan file identity is missing or does not match the current file".to_string());
        }
    }

    // 4. Check completed status
    if checkpoint.status.eq_ignore_ascii_case("COMPLETED") {
        is_resumable = false;
        reasons.push("Workflow is already marked as COMPLETED in checkpoint".to_string());
    }

    // 5. Check halted status
    if checkpoint.status.eq_ignore_ascii_case("HALTED") {
        is_resumable = false;
        reasons.push("Workflow was previously HALTED; requires explicit confirmation to resume".to_string());
    }

    if !checkpoint.unresolved_findings.is_empty() {
        is_resumable = false;
        reasons.push("Unresolved review findings remain in checkpoint".to_string());
    }

    // 6. Verify modified files still exist
    for f in &checkpoint.modified_files {
        if f.trim().is_empty() || f.eq_ignore_ascii_case("None") {
            continue;
        }
        let relative = PathBuf::from(f);
        let invalid_relative = relative.is_absolute() || relative.components().any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::Prefix(_)));
        let full_p = repo_root.join(&relative);
        let escapes_repo = full_p.canonicalize().map(|p| !p.starts_with(&canonical_repo)).unwrap_or(false);
        if invalid_relative || escapes_repo || !full_p.is_file() {
            is_resumable = false;
            reasons.push(format!("Modified file noted in checkpoint is missing, unsafe, or outside the repository: '{f}'"));
        }
    }

    let next_action = if is_resumable {
        checkpoint.next_action.clone()
    } else {
        "STOP".to_string()
    };

    Ok(ResumptionPreflightResult {
        is_resumable,
        next_action,
        active_plan: normalized_rel,
        workflow: checkpoint.workflow.clone(),
        status: checkpoint.status.clone(),
        reasons,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_plan_path_rejects_traversal_and_escapes() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();

        assert!(validate_plan_path(repo, "../secret.md").is_err());
        assert!(validate_plan_path(repo, "sub/../../secret.md").is_err());
        assert!(validate_plan_path(repo, "/absolute/path.md").is_err());
        assert!(validate_plan_path(repo, "C:/absolute/path.md").is_err());
        assert!(validate_plan_path(repo, ".plan/not_markdown.txt").is_err());
        assert!(validate_plan_path(repo, "").is_err());

        // Valid plan path inside .plan or custom folder within repo
        assert!(validate_plan_path(repo, ".plan/V0.26.3.md").is_ok());
        assert!(validate_plan_path(repo, "plans/my_plan.md").is_ok());
        assert!(validate_plan_path(repo, "docs/my_plan.md").is_ok());
    }

    #[test]
    fn test_create_plan_rejects_symlink_escape_before_creating_children() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&outside, repo.join("linked")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, repo.join("linked")).unwrap();

        let req = CreatePlanRequest { name: "escape".into(), folder: Some("linked/new/nested".into()), title: None, set_as_active: Some(false) };
        assert!(create_plan_file(&repo, &req).is_err());
        assert!(!outside.join("new").exists(), "external directory must not be created before validation");
    }

    #[test]
    fn test_create_and_get_and_list_plan_lifecycle() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();

        // 1. Initial list should be empty
        let initial_list = list_available_plans(repo, None).unwrap();
        assert!(initial_list.is_empty());

        let initial_active = get_active_plan(repo, None, None).unwrap();
        assert!(initial_active.is_none());

        // 2. Create plan with set_as_active = true
        let req = CreatePlanRequest {
            name: "feature-auth".to_string(),
            folder: None,
            title: Some("User Authentication Plan".to_string()),
            set_as_active: Some(true),
        };
        let created = create_plan_file(repo, &req).unwrap();
        assert_eq!(created.rel_path, ".plan/feature-auth.md");
        assert_eq!(created.title, "User Authentication Plan");

        // 3. Active plan should now be set
        let active = get_active_plan(repo, None, None).unwrap().expect("should have active plan");
        assert_eq!(active.rel_path, ".plan/feature-auth.md");
        assert_eq!(active.title, "User Authentication Plan");

        // 4. Plan overview should include the item marked as active
        let overview = get_plan_overview(repo, None, None, None).unwrap();
        assert_eq!(overview.active_plan_relative_path.as_deref(), Some(".plan/feature-auth.md"));
        assert_eq!(overview.plans.len(), 1);
        assert!(overview.plans[0].is_active);

        // 5. Creating duplicate file name must fail (prevent overwrite)
        let dup_err = create_plan_file(repo, &req);
        assert!(dup_err.is_err());
        assert!(dup_err.unwrap_err().contains("already exists"));

        // 6. Create plan in a custom folder
        let req_custom = CreatePlanRequest {
            name: "custom-spec".to_string(),
            folder: Some("specs".to_string()),
            title: Some("Specifications Plan".to_string()),
            set_as_active: Some(false),
        };
        let created_custom = create_plan_file(repo, &req_custom).unwrap();
        assert_eq!(created_custom.rel_path, "specs/custom-spec.md");

        let specs_list = list_available_plans(repo, Some("specs")).unwrap();
        assert_eq!(specs_list.len(), 1);
        assert_eq!(specs_list[0].rel_path, "specs/custom-spec.md");

        // 7. Create a second plan with set_as_active = false
        let req2 = CreatePlanRequest {
            name: "feature-billing".to_string(),
            folder: None,
            title: Some("Billing Integration".to_string()),
            set_as_active: Some(false),
        };
        let created2 = create_plan_file(repo, &req2).unwrap();
        assert_eq!(created2.rel_path, ".plan/feature-billing.md");

        // Active plan should still be feature-auth
        let active2 = get_active_plan(repo, None, None).unwrap().expect("should have active plan");
        assert_eq!(active2.rel_path, ".plan/feature-auth.md");

        // 8. Switch active plan to billing
        set_active_plan(repo, ".plan/feature-billing.md", None, None).unwrap();
        let active3 = get_active_plan(repo, None, None).unwrap().expect("should have active plan");
        assert_eq!(active3.rel_path, ".plan/feature-billing.md");
    }

    #[test]
    fn test_audit_trail_append_and_cas_verification() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();

        let req = CreatePlanRequest {
            name: "test-cas".to_string(),
            folder: None,
            title: Some("CAS Test Plan".to_string()),
            set_as_active: Some(true),
        };
        let created = create_plan_file(repo, &req).unwrap();
        let initial_hash = created.sha256_hash.clone();

        // 1. Successful CAS append
        let event = AuditTrailEvent {
            phase: "PlanReview".to_string(),
            role_actor: "CodexReviewer".to_string(),
            result: "APPROVED".to_string(),
            notes: "All requirements met".to_string(),
            expected_sha256: initial_hash.clone(),
            expected_file_identity: Some(created.file_identity.clone().unwrap()),
        };
        let updated = append_audit_trail_event(repo, &created.rel_path, &event).unwrap();
        assert!(updated.content.contains("CodexReviewer"));
        assert!(updated.content.contains("PlanReview"));
        assert_ne!(updated.sha256_hash, initial_hash);

        // 2. Stale CAS append should fail
        let stale_event = AuditTrailEvent {
            phase: "BuildReview".to_string(),
            role_actor: "DeepSeekReviewer".to_string(),
            result: "APPROVED".to_string(),
            notes: "Stale attempt".to_string(),
            expected_sha256: initial_hash, // using old hash
            expected_file_identity: Some(created.file_identity.clone().unwrap()),
        };
        let err = append_audit_trail_event(repo, &created.rel_path, &stale_event);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("Compare-and-swap conflict"));

        // 3. Missing expected hash should fail
        let missing_hash_event = AuditTrailEvent {
            phase: "BuildReview".to_string(),
            role_actor: "DeepSeekReviewer".to_string(),
            result: "APPROVED".to_string(),
            notes: "No hash".to_string(),
            expected_sha256: "   ".to_string(),
            expected_file_identity: Some(created.file_identity.clone().unwrap()),
        };
        let err_missing = append_audit_trail_event(repo, &created.rel_path, &missing_hash_event);
        assert!(err_missing.is_err());
        assert!(err_missing.unwrap_err().contains("expected_sha256"));

        let mut missing_identity_event = event;
        missing_identity_event.expected_sha256 = updated.sha256_hash;
        missing_identity_event.expected_file_identity = None;
        let err_missing_identity = append_audit_trail_event(repo, &created.rel_path, &missing_identity_event);
        assert!(err_missing_identity.unwrap_err().contains("expected_file_identity"));
    }

    #[test]
    fn test_audit_append_targets_numbered_section_before_following_sections() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();
        let created = create_plan_file(
            repo,
            &CreatePlanRequest {
                name: "numbered-audit".into(),
                folder: None,
                title: Some("Numbered audit section".into()),
                set_as_active: Some(false),
            },
        )
        .unwrap();
        let content = "# Numbered audit section\n\n## 11. Execution History & Audit Trail\n\n| Timestamp | Phase | Role / Actor | Result | Notes |\n| --- | --- | --- | --- | --- |\n| old | PlanReview | Reviewer | APPROVED | prior event |\n\n## 12. Next Plan Section\n\nKeep this section after the audit table.\n";
        let path = repo.join(&created.rel_path);
        std::fs::write(&path, content).unwrap();
        let expected_hash = format!("{:x}", Sha256::digest(content.as_bytes()));
        let expected_identity = plan_file_identity(&path).unwrap();
        let event = AuditTrailEvent {
            phase: "BuildReview".into(),
            role_actor: "Codex".into(),
            result: "APPROVED".into(),
            notes: "new audit event".into(),
            expected_sha256: expected_hash,
            expected_file_identity: Some(expected_identity),
        };

        let updated = append_audit_trail_event(repo, &created.rel_path, &event).unwrap();
        let audit_section = updated.content.split("## 12. Next Plan Section").next().unwrap();
        let following_section = updated.content.split("## 12. Next Plan Section").nth(1).unwrap();
        assert!(audit_section.contains("new audit event"));
        assert!(following_section.contains("Keep this section"));
        assert!(!following_section.contains("new audit event"));
    }

    #[test]
    fn test_extract_title_and_status() {
        let content = r#"# My Awesome Feature
- **Status:** `IN_PROGRESS`
Some description here...
"#;
        let (title, status) = extract_title_and_status(content, "default.md");
        assert_eq!(title, "My Awesome Feature");
        assert_eq!(status.as_deref(), Some("IN_PROGRESS"));

        let content_ja = r#"# 機能拡張計画
- **状態:** `承認済み`
内容...
"#;
        let (title_ja, status_ja) = extract_title_and_status(content_ja, "default.md");
        assert_eq!(title_ja, "機能拡張計画");
        assert_eq!(status_ja.as_deref(), Some("承認済み"));
    }

    #[test]
    fn test_checkpoint_render_and_extract_round_trip() {
        let cp = ExecutionCheckpoint {
            checkpoint_version: 1,
            workflow: "pov-full-cycle".to_string(),
            status: "IMPLEMENTATION_REVIEW".to_string(),
            last_completed: "IMPLEMENTATION".to_string(),
            next_action: "REVIEW".to_string(),
            repository: "C:/test-repo".to_string(),
            active_plan: ".plan/feature-test.md".to_string(),
            plan_revision: "abc123hash".to_string(),
            plan_file_identity: "".to_string(),
            workflow_run_id: "run-20261108-01".to_string(),
            completed_work: vec!["Task 1 finished".to_string(), "Task 2 tested".to_string()],
            modified_files: vec!["src/lib.rs".to_string(), "src/main.rs".to_string()],
            validation: vec!["cargo test PASS".to_string()],
            reviewer: "Codex CLI".to_string(),
            review_result: "PENDING".to_string(),
            unresolved_findings: vec!["None confirmed".to_string()],
            pending_work: vec!["Independent review".to_string()],
            timestamp: "2026-11-08T04:00:00Z".to_string(),
        };

        let rendered = render_checkpoint_markdown(&cp);
        assert!(rendered.contains("## Poverello Execution Checkpoint"));
        assert!(rendered.contains("<!-- POVERELLO-CHECKPOINT:START -->"));
        assert!(rendered.contains("<!-- POVERELLO-CHECKPOINT:END -->"));
        assert!(rendered.contains("Checkpoint Version: 1"));
        assert!(rendered.contains("Workflow: pov-full-cycle"));
        assert!(rendered.contains("Status: IMPLEMENTATION_REVIEW"));

        let extracted = extract_execution_checkpoint(&rendered).expect("checkpoint should be extracted");
        assert_eq!(extracted.checkpoint_version, 1);
        assert_eq!(extracted.workflow, "pov-full-cycle");
        assert_eq!(extracted.status, "IMPLEMENTATION_REVIEW");
        assert_eq!(extracted.last_completed, "IMPLEMENTATION");
        assert_eq!(extracted.next_action, "REVIEW");
        assert_eq!(extracted.repository, "C:/test-repo");
        assert_eq!(extracted.active_plan, ".plan/feature-test.md");
        assert_eq!(extracted.plan_revision, "abc123hash");
        assert_eq!(extracted.workflow_run_id, "run-20261108-01");
        assert_eq!(extracted.completed_work, vec!["Task 1 finished", "Task 2 tested"]);
        assert_eq!(extracted.modified_files, vec!["src/lib.rs", "src/main.rs"]);
        assert_eq!(extracted.validation, vec!["cargo test PASS"]);
        assert_eq!(extracted.reviewer, "Codex CLI");
        assert_eq!(extracted.review_result, "PENDING");
        assert_eq!(extracted.unresolved_findings, vec!["None confirmed"]);
        assert_eq!(extracted.pending_work, vec!["Independent review"]);
        assert_eq!(extracted.timestamp, "2026-11-08T04:00:00Z");
    }

    #[test]
    fn test_checkpoint_markers_in_fenced_example_are_ignored_and_preserved() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();
        let req = CreatePlanRequest { name: "fenced-checkpoint".into(), folder: None, title: None, set_as_active: Some(false) };
        let created = create_plan_file(repo, &req).unwrap();
        let example = format!("\n```markdown\n{}\nStatus: EXAMPLE\n{}\n```\n", CHECKPOINT_START_MARKER, CHECKPOINT_END_MARKER);
        let content = format!("{}{}", std::fs::read_to_string(repo.join(&created.rel_path)).unwrap(), example);
        std::fs::write(repo.join(&created.rel_path), &content).unwrap();
        assert!(extract_execution_checkpoint(&content).is_none());

        let hash = format!("{:x}", Sha256::digest(content.as_bytes()));
        let cp = ExecutionCheckpoint { checkpoint_version: 1, workflow: "pov-full-cycle".into(), status: "IMPLEMENTATION".into(), last_completed: "TESTS".into(), next_action: "REVIEW".into(), repository: repo.to_string_lossy().into_owned(), active_plan: created.rel_path.clone(), plan_revision: String::new(), plan_file_identity: String::new(), workflow_run_id: "test".into(), completed_work: vec![], modified_files: vec![], validation: vec![], reviewer: "Codex".into(), review_result: "PENDING".into(), unresolved_findings: vec![], pending_work: vec![], timestamp: "now".into() };
        let saved = save_execution_checkpoint(repo, &created.rel_path, &cp, &created.file_identity.clone().unwrap(), &hash).unwrap();
        assert!(saved.content.contains(&example), "documentation example must remain byte-for-byte present");
        assert_eq!(saved.content.matches(CHECKPOINT_START_MARKER).count(), 2);
        assert_eq!(get_execution_checkpoint(repo, &created.rel_path).unwrap().unwrap().status, "IMPLEMENTATION");
    }

    #[test]
    fn test_fenced_checkpoint_example_in_plan_is_ignored_without_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();
        let created = create_plan_file(repo, &CreatePlanRequest { name: "documented-checkpoint".into(), folder: None, title: None, set_as_active: Some(false) }).unwrap();
        let example = format!("\n### Example\n\n```markdown\n{}\nStatus: EXAMPLE\n{}\n```\n", CHECKPOINT_START_MARKER, CHECKPOINT_END_MARKER);
        let original = std::fs::read_to_string(repo.join(&created.rel_path)).unwrap();
        let content = format!("{}{}", original, example);
        std::fs::write(repo.join(&created.rel_path), &content).unwrap();
        assert!(extract_execution_checkpoint(&content).is_none());

        let hash = format!("{:x}", Sha256::digest(content.as_bytes()));
        let cp = ExecutionCheckpoint { checkpoint_version: 1, workflow: "pov-full-cycle".into(), status: "REVIEW".into(), last_completed: "TESTS".into(), next_action: "REVIEW".into(), repository: repo.to_string_lossy().into_owned(), active_plan: created.rel_path.clone(), plan_revision: String::new(), plan_file_identity: String::new(), workflow_run_id: "test".into(), completed_work: vec![], modified_files: vec![], validation: vec![], reviewer: "Codex".into(), review_result: "PENDING".into(), unresolved_findings: vec![], pending_work: vec![], timestamp: "now".into() };
        let saved = save_execution_checkpoint(repo, &created.rel_path, &cp, &created.file_identity.clone().unwrap(), &hash).unwrap();
        assert!(saved.content.contains(&example));
        assert_eq!(saved.content.matches(CHECKPOINT_START_MARKER).count(), 2);
        assert_eq!(get_execution_checkpoint(repo, &created.rel_path).unwrap().unwrap().status, "REVIEW");
    }

    #[test]
    fn test_fenced_marker_examples_and_ambiguous_markers_fail_closed() {
        let example = format!("```markdown\n{}\n{}\n```", CHECKPOINT_START_MARKER, CHECKPOINT_END_MARKER);
        assert!(extract_execution_checkpoint(&example).is_none());
        let pair = format!("{}\n{}", CHECKPOINT_START_MARKER, CHECKPOINT_END_MARKER);
        assert!(extract_execution_checkpoint(&format!("{}\n{}", pair, pair)).is_none());
        assert!(extract_execution_checkpoint(CHECKPOINT_START_MARKER).is_none());
        assert!(extract_execution_checkpoint(CHECKPOINT_END_MARKER).is_none());
    }

    #[test]
    fn test_checkpoint_revision_round_trip_is_stable_and_preflight_detects_edits() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();
        let created = create_plan_file(repo, &CreatePlanRequest { name: "revision-checkpoint".into(), folder: None, title: None, set_as_active: Some(true) }).unwrap();
        let cp = ExecutionCheckpoint { checkpoint_version: 1, workflow: "pov-full-cycle".into(), status: "IMPLEMENTATION".into(), last_completed: "TESTS".into(), next_action: "REVIEW".into(), repository: repo.to_string_lossy().into_owned(), active_plan: created.rel_path.clone(), plan_revision: String::new(), plan_file_identity: String::new(), workflow_run_id: "run".into(), completed_work: vec![], modified_files: vec![], validation: vec![], reviewer: "Codex".into(), review_result: "PENDING".into(), unresolved_findings: vec![], pending_work: vec![], timestamp: "now".into() };
        let saved = save_execution_checkpoint(repo, &created.rel_path, &cp, &created.file_identity.clone().unwrap(), &created.sha256_hash).unwrap();
        let stored = get_execution_checkpoint(repo, &created.rel_path).unwrap().unwrap();
        let good = verify_resumption_preflight(repo, &created.rel_path, &stored, None, None).unwrap();
        assert!(good.is_resumable, "{:?}", good.reasons);

        let mut edited = saved.content.clone();
        edited.push_str("\nExternal plan edit\n");
        std::fs::write(repo.join(&created.rel_path), edited).unwrap();
        let stale = verify_resumption_preflight(repo, &created.rel_path, &stored, None, None).unwrap();
        assert!(!stale.is_resumable);
        assert!(stale.reasons.iter().any(|reason| reason.contains("revision hash")));
    }

    #[test]
    fn test_extract_checkpoint_rejects_multiple_or_unbalanced_live_markers() {
        let pair = format!("{}\n{}", CHECKPOINT_START_MARKER, CHECKPOINT_END_MARKER);
        assert!(extract_execution_checkpoint(&format!("{}\n{}", pair, pair)).is_none());
        assert!(extract_execution_checkpoint(CHECKPOINT_START_MARKER).is_none());
        assert!(extract_execution_checkpoint(CHECKPOINT_END_MARKER).is_none());
    }

    #[test]
    fn test_save_execution_checkpoint_preserves_plan_and_audit_trail() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();

        let req = CreatePlanRequest {
            name: "feature-checkpoint".to_string(),
            folder: None,
            title: Some("Checkpoint Integration Plan".to_string()),
            set_as_active: Some(true),
        };
        let created = create_plan_file(repo, &req).unwrap();
        let plan_path = &created.rel_path;
        let mut cur_hash = created.sha256_hash;

        // 1. Append an audit trail event first
        let event = AuditTrailEvent {
            phase: "PlanCreated".to_string(),
            role_actor: "Planner".to_string(),
            result: "PROPOSED".to_string(),
            notes: "Initial draft".to_string(),
            expected_sha256: cur_hash.clone(),
            expected_file_identity: Some(created.file_identity.clone().unwrap()),
        };
        let with_audit = append_audit_trail_event(repo, plan_path, &event).unwrap();
        cur_hash = with_audit.sha256_hash;
        assert!(with_audit.content.contains("Initial draft"));
        assert!(with_audit.content.contains("## 5. Execution History & Audit Trail"));

        // 2. Save execution checkpoint
        let cp1 = ExecutionCheckpoint {
            checkpoint_version: 1,
            workflow: "pov-full-cycle".to_string(),
            status: "PLAN_REVIEW".to_string(),
            last_completed: "PLANNING".to_string(),
            next_action: "REVIEW".to_string(),
            repository: repo.to_string_lossy().to_string(),
            active_plan: plan_path.clone(),
            plan_revision: cur_hash.clone(),
            plan_file_identity: String::new(),
            workflow_run_id: "run-01".to_string(),
            completed_work: vec!["Draft created".to_string()],
            modified_files: vec![],
            validation: vec![],
            reviewer: "Codex CLI".to_string(),
            review_result: "PENDING".to_string(),
            unresolved_findings: vec![],
            pending_work: vec!["Review plan".to_string()],
            timestamp: "2026-11-08T04:05:00Z".to_string(),
        };
        let saved1 = save_execution_checkpoint(repo, plan_path, &cp1, with_audit.file_identity.as_deref().unwrap(), &cur_hash).unwrap();
        cur_hash = saved1.sha256_hash;

        // Verify plan contents, checkpoint, and audit trail are all present
        assert!(saved1.content.contains("# Checkpoint Integration Plan"));
        assert!(saved1.content.contains("## 1. Overview & Objectives"));
        assert!(saved1.content.contains("## Poverello Execution Checkpoint"));
        assert!(saved1.content.contains("Status: PLAN_REVIEW"));
        assert!(saved1.content.contains("## 5. Execution History & Audit Trail"));
        assert!(saved1.content.contains("Initial draft"));

        // Verify get_execution_checkpoint retrieves it accurately
        let fetched1 = get_execution_checkpoint(repo, plan_path).unwrap().expect("should find checkpoint");
        assert_eq!(fetched1.status, "PLAN_REVIEW");
        assert_eq!(fetched1.next_action, "REVIEW");

        // 3. Update checkpoint in-place for next stage (IMPLEMENTATION)
        let cp2 = ExecutionCheckpoint {
            checkpoint_version: 1,
            workflow: "pov-full-cycle".to_string(),
            status: "IMPLEMENTATION".to_string(),
            last_completed: "PLAN_REVIEW".to_string(),
            next_action: "IMPLEMENT".to_string(),
            repository: repo.to_string_lossy().to_string(),
            active_plan: plan_path.clone(),
            plan_revision: cur_hash.clone(),
            plan_file_identity: String::new(),
            workflow_run_id: "run-01".to_string(),
            completed_work: vec!["Plan reviewed and approved".to_string()],
            modified_files: vec!["src/lib.rs".to_string()],
            validation: vec!["Tests pending".to_string()],
            reviewer: "Codex CLI".to_string(),
            review_result: "APPROVED".to_string(),
            unresolved_findings: vec![],
            pending_work: vec!["Write code".to_string()],
            timestamp: "2026-11-08T04:10:00Z".to_string(),
        };
        let saved2 = save_execution_checkpoint(repo, plan_path, &cp2, saved1.file_identity.as_deref().unwrap(), &cur_hash).unwrap();
        let _ = cur_hash;

        // Verify it was updated in-place without duplicating the section header
        let checkpoint_header_count = saved2.content.matches("## Poverello Execution Checkpoint").count();
        assert_eq!(checkpoint_header_count, 1, "Checkpoint section must not be duplicated");
        assert!(saved2.content.contains("Status: IMPLEMENTATION"));
        assert!(saved2.content.contains("Initial draft"));

        // 4. Stale CAS update must fail
        let cp_stale = cp2.clone();
        let stale_res = save_execution_checkpoint(repo, plan_path, &cp_stale, saved2.file_identity.as_deref().unwrap(), "stale_hash_value");
        assert!(stale_res.is_err());
        assert!(stale_res.unwrap_err().contains("Compare-and-swap conflict"));
    }

    #[test]
    fn test_verify_resumption_preflight() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();

        let req = CreatePlanRequest {
            name: "preflight-test".to_string(),
            folder: None,
            title: Some("Preflight Test Plan".to_string()),
            set_as_active: Some(true),
        };
        let created = create_plan_file(repo, &req).unwrap();

        // Create a dummy modified file
        std::fs::write(repo.join("changed_file.txt"), "hello").unwrap();

        let valid_cp = ExecutionCheckpoint {
            checkpoint_version: 1,
            workflow: "pov-full-cycle".to_string(),
            status: "IMPLEMENTATION".to_string(),
            last_completed: "PLAN_REVIEW".to_string(),
            next_action: "IMPLEMENT".to_string(),
            repository: repo.to_string_lossy().to_string(),
            active_plan: created.rel_path.clone(),
            plan_revision: format!("{:x}", Sha256::digest(std::fs::read_to_string(repo.join(&created.rel_path)).unwrap().as_bytes())),
            plan_file_identity: plan_file_identity(&repo.join(&created.rel_path)).unwrap(),
            workflow_run_id: "run-preflight".to_string(),
            completed_work: vec![],
            modified_files: vec!["changed_file.txt".to_string()],
            validation: vec![],
            reviewer: "".to_string(),
            review_result: "".to_string(),
            unresolved_findings: vec![],
            pending_work: vec![],
            timestamp: "2026-11-08T04:15:00Z".to_string(),
        };

        // 1. Valid resumption preflight
        let res_ok = verify_resumption_preflight(repo, &created.rel_path, &valid_cp, None, None).unwrap();
        assert!(res_ok.is_resumable);
        assert_eq!(res_ok.next_action, "IMPLEMENT");
        assert!(res_ok.reasons.is_empty());

        // Plan edits and missing revision hashes fail closed.
        let mut changed_plan_cp = valid_cp.clone();
        std::fs::write(repo.join(&created.rel_path), "# externally changed plan\n").unwrap();
        let changed = verify_resumption_preflight(repo, &created.rel_path, &changed_plan_cp, None, None).unwrap();
        assert!(!changed.is_resumable);
        assert!(changed.reasons.iter().any(|r| r.contains("revision hash")));
        std::fs::write(repo.join(&created.rel_path), &created.content).unwrap();
        changed_plan_cp.plan_revision = "".to_string();
        let missing_revision = verify_resumption_preflight(repo, &created.rel_path, &changed_plan_cp, None, None).unwrap();
        assert!(!missing_revision.is_resumable);
        assert!(missing_revision.reasons.iter().any(|r| r.contains("revision hash")));

        // 2. Mismatched repo preflight
        let mut invalid_repo_cp = valid_cp.clone();
        let other_temp = tempfile::tempdir().unwrap();
        invalid_repo_cp.repository = other_temp.path().to_string_lossy().to_string();
        let res_repo_mismatch = verify_resumption_preflight(repo, &created.rel_path, &invalid_repo_cp, None, None).unwrap();
        assert!(!res_repo_mismatch.is_resumable);
        assert!(res_repo_mismatch.reasons.iter().any(|r| r.contains("Repository mismatch")));

        // 3. Completed workflow cannot resume as active
        let mut completed_cp = valid_cp.clone();
        completed_cp.status = "COMPLETED".to_string();
        let res_completed = verify_resumption_preflight(repo, &created.rel_path, &completed_cp, None, None).unwrap();
        assert!(!res_completed.is_resumable);
        assert_eq!(res_completed.next_action, "STOP");
        assert!(res_completed.reasons.iter().any(|r| r.contains("already marked as COMPLETED")));

        // 4. Missing modified file flagged
        let mut missing_file_cp = valid_cp.clone();
        missing_file_cp.modified_files = vec!["nonexistent_file.rs".to_string()];
        let res_missing_file = verify_resumption_preflight(repo, &created.rel_path, &missing_file_cp, None, None).unwrap();
        assert!(res_missing_file.reasons.iter().any(|r| r.contains("nonexistent_file.rs")));
    }

    #[test]
    fn test_role_assignments_are_surface_scoped_and_preserved_when_active_plan_changes() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();
        let codex = RoleAssignment { planner_id: Some("codex_session".into()), reviser_id: None };
        let ag = RoleAssignment { planner_id: Some("antigravity".into()), reviser_id: Some("codex".into()) };
        set_role_assignment(repo, "codex", codex.clone()).unwrap();
        set_role_assignment(repo, "antigravity", ag.clone()).unwrap();
        assert_eq!(get_role_assignment(repo, "codex").unwrap(), codex);
        assert_eq!(get_role_assignment(repo, "antigravity").unwrap(), ag);
        assert!(set_role_assignment(repo, "codex", RoleAssignment { planner_id: Some("antigravity".into()), reviser_id: None }).is_err());

        let created = create_plan_file(repo, &CreatePlanRequest { name: "roles".into(), folder: None, title: None, set_as_active: Some(true) }).unwrap();
        set_active_plan(repo, &created.rel_path, None, None).unwrap();
        assert_eq!(get_role_assignment(repo, "codex").unwrap(), codex);
        assert_eq!(get_role_assignment(repo, "antigravity").unwrap(), ag);
    }

    #[test]
    fn test_legacy_active_plan_state_is_preserved_when_assigning_roles() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();
        let plan_dir = repo.join(PLAN_DIR_NAME);
        std::fs::create_dir_all(&plan_dir).unwrap();
        std::fs::write(plan_dir.join(ACTIVE_PLAN_FILE_NAME), r#"{"active_plan_path":".plan/legacy.md","last_updated_unix":1}"#).unwrap();
        let assignment = RoleAssignment { planner_id: Some("codex_session".into()), reviser_id: Some("codex_session".into()) };
        set_role_assignment(repo, "codex", assignment.clone()).unwrap();
        let state = load_active_plan_state(repo).unwrap();
        assert_eq!(state.active_plan_path.as_deref(), Some(".plan/legacy.md"));
        assert_eq!(state.role_assignments.get("codex"), Some(&assignment));
    }

    #[test]
    fn test_role_assignment_requires_valid_active_plan() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();
        let assignment = RoleAssignment { planner_id: Some("codex_session".into()), reviser_id: None };
        set_role_assignment(repo, "codex", assignment.clone()).unwrap();
        assert!(get_role_assignment_for_active_plan(repo, "codex", None).is_err());

        let created = create_plan_file(repo, &CreatePlanRequest { name: "active-role-plan".into(), folder: None, title: None, set_as_active: Some(true) }).unwrap();
        assert_eq!(get_role_assignment_for_active_plan(repo, "codex", None).unwrap(), assignment);
        std::fs::remove_file(repo.join(created.rel_path)).unwrap();
        assert!(get_role_assignment_for_active_plan(repo, "codex", None).is_err());
    }

    #[test]
    fn test_resumption_fails_closed_for_missing_state_halted_or_missing_files() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();
        let created = create_plan_file(repo, &CreatePlanRequest { name: "resume-guards".into(), folder: None, title: None, set_as_active: Some(false) }).unwrap();
        let cp = ExecutionCheckpoint {
            checkpoint_version: 1,
            workflow: "pov-full-cycle".into(),
            status: "IMPLEMENTATION".into(),
            last_completed: "TESTS".into(),
            next_action: "REVIEW".into(),
            repository: repo.to_string_lossy().into_owned(),
            active_plan: created.rel_path.clone(),
            plan_revision: checkpoint_revision_digest(&created.content),
            plan_file_identity: String::new(),
            workflow_run_id: "resume-test".into(),
            completed_work: vec![],
            modified_files: vec![],
            validation: vec![],
            reviewer: "Codex".into(),
            review_result: "PENDING".into(),
            unresolved_findings: vec![],
            pending_work: vec![],
            timestamp: "now".into(),
        };

        let mut missing_path = cp.clone();
        missing_path.active_plan.clear();
        assert!(!verify_resumption_preflight(repo, &created.rel_path, &missing_path, None, None).unwrap().is_resumable);

        let mut halted = cp.clone();
        halted.status = "HALTED".into();
        assert!(!verify_resumption_preflight(repo, &created.rel_path, &halted, None, None).unwrap().is_resumable);

        let mut missing_modified = cp.clone();
        missing_modified.modified_files.push("src/missing.rs".into());
        assert!(!verify_resumption_preflight(repo, &created.rel_path, &missing_modified, None, None).unwrap().is_resumable);

        let mut unknown = cp;
        unknown.next_action.clear();
        assert!(!verify_resumption_preflight(repo, &created.rel_path, &unknown, None, None).unwrap().is_resumable);
    }

    #[test]
    fn test_plan_candidate_checks_identity_and_hash_and_returns_new_state() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();
        let created = create_plan_file(repo, &CreatePlanRequest { name: "candidate-cas".into(), folder: None, title: None, set_as_active: Some(false) }).unwrap();
        let identity = created.file_identity.clone().expect("filesystem identity is supported");
        let candidate = format!("{}\nCandidate revision\n", created.content);
        let updated = apply_plan_candidate_if_unchanged(repo, &created.rel_path, &identity, &created.sha256_hash, &candidate).unwrap();
        assert!(updated.content.contains("Candidate revision"));
        assert!(updated.file_identity.is_some());

        let stale = apply_plan_candidate_if_unchanged(repo, &created.rel_path, "wrong-identity", &updated.sha256_hash, "stale");
        assert!(stale.unwrap_err().contains("identity or SHA-256 changed"));

        let event = AuditTrailEvent {
            phase: "Review".into(), role_actor: "Reviewer".into(), result: "APPROVED".into(),
            notes: "Identity-bound audit append".into(), expected_sha256: updated.sha256_hash.clone(),
            expected_file_identity: Some(updated.file_identity.unwrap()),
        };
        let audited = append_audit_trail_event(repo, &created.rel_path, &event).unwrap();
        assert!(audited.content.contains("Identity-bound audit append"));

        let mut stale_event = event;
        stale_event.expected_sha256 = audited.sha256_hash;
        stale_event.expected_file_identity = Some("wrong-identity".into());
        assert!(append_audit_trail_event(repo, &created.rel_path, &stale_event).is_err());
    }

    #[test]
    fn test_find_enclosing_repo_root() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("my-repo");
        let sub = repo.join("deep").join("nested");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::create_dir_all(repo.join(".git")).unwrap();

        let plan_file = sub.join("plan.md");
        std::fs::write(&plan_file, "# Plan").unwrap();

        let found = find_enclosing_repo_root(&plan_file).unwrap();
        assert_eq!(found.canonicalize().unwrap(), repo.canonicalize().unwrap());

        // Test worktree where .git is a file
        let worktree_temp = tempfile::tempdir().unwrap();
        let wt = worktree_temp.path().join("wt");
        let wt_sub = wt.join("folder");
        std::fs::create_dir_all(&wt_sub).unwrap();
        std::fs::write(wt.join(".git"), "gitdir: /path/to/gitdir").unwrap();
        let wt_plan = wt_sub.join("plan.md");
        std::fs::write(&wt_plan, "# WT Plan").unwrap();

        let found_wt = find_enclosing_repo_root(&wt_plan).unwrap();
        assert_eq!(found_wt.canonicalize().unwrap(), wt.canonicalize().unwrap());

        // Test non-repo rejection
        let non_repo = tempfile::tempdir().unwrap();
        let non_repo_file = non_repo.path().join("outside.md");
        std::fs::write(&non_repo_file, "# Outside").unwrap();
        assert!(find_enclosing_repo_root(&non_repo_file).is_err());
    }

    #[test]
    fn test_per_surface_active_plan_isolation_and_legacy_compatibility() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();

        let plan_ag = create_plan_file(repo, &CreatePlanRequest {
            name: "plan-ag".into(),
            folder: None,
            title: Some("AG Plan".into()),
            set_as_active: Some(false),
        }).unwrap();

        let plan_codex = create_plan_file(repo, &CreatePlanRequest {
            name: "plan-codex".into(),
            folder: None,
            title: Some("Codex Plan".into()),
            set_as_active: Some(false),
        }).unwrap();

        // Initially no active plan for either
        assert!(get_active_plan(repo, Some("antigravity"), None).unwrap().is_none());
        assert!(get_active_plan(repo, Some("codex"), None).unwrap().is_none());

        // Set Antigravity active plan
        set_active_plan(repo, &plan_ag.rel_path, Some("antigravity"), None).unwrap();
        assert_eq!(
            get_active_plan(repo, Some("antigravity"), None).unwrap().unwrap().rel_path,
            plan_ag.rel_path
        );
        // Codex has not set its plan yet and surfaces are strictly isolated: returns None
        assert!(get_active_plan(repo, Some("codex"), None).unwrap().is_none());

        // Now set Codex active plan independently
        set_active_plan(repo, &plan_codex.rel_path, Some("codex"), None).unwrap();
        assert_eq!(
            get_active_plan(repo, Some("antigravity"), None).unwrap().unwrap().rel_path,
            plan_ag.rel_path
        );
        assert_eq!(
            get_active_plan(repo, Some("codex"), None).unwrap().unwrap().rel_path,
            plan_codex.rel_path
        );

        // Test legacy JSON without surfaces section falls back gracefully when surface has no specific plan
        let plan_dir = repo.join(PLAN_DIR_NAME);
        std::fs::write(
            plan_dir.join(ACTIVE_PLAN_FILE_NAME),
            r#"{"active_plan_path":".plan/plan-ag.md","last_updated_unix":123}"#,
        ).unwrap();
        assert_eq!(
            get_active_plan(repo, None, None).unwrap().unwrap().rel_path,
            plan_ag.rel_path
        );

        // After setting codex active plan, antigravity has not set its own plan yet
        set_active_plan(repo, &plan_codex.rel_path, Some("codex"), None).unwrap();
        assert!(get_active_plan(repo, Some("antigravity"), None).unwrap().is_none());
        assert_eq!(
            get_active_plan(repo, Some("codex"), None).unwrap().unwrap().rel_path,
            plan_codex.rel_path
        );
    }

    #[test]
    fn test_per_slot_active_plan_isolation_in_same_repo() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();

        let plan_p1 = create_plan_file(repo, &CreatePlanRequest {
            name: "plan-slot-1".into(),
            folder: None,
            title: Some("Slot 1 Plan".into()),
            set_as_active: Some(false),
        }).unwrap();

        let plan_p2 = create_plan_file(repo, &CreatePlanRequest {
            name: "plan-slot-2".into(),
            folder: None,
            title: Some("Slot 2 Plan".into()),
            set_as_active: Some(false),
        }).unwrap();

        // Set primary slot plan
        set_active_plan(repo, &plan_p1.rel_path, Some("codex"), Some("primary")).unwrap();
        assert_eq!(
            get_active_plan(repo, Some("codex"), Some("primary")).unwrap().unwrap().rel_path,
            plan_p1.rel_path
        );
        // Secondary slot is still empty
        assert!(get_active_plan(repo, Some("codex"), Some("secondary")).unwrap().is_none());

        // Set secondary slot plan
        set_active_plan(repo, &plan_p2.rel_path, Some("codex"), Some("secondary")).unwrap();
        assert_eq!(
            get_active_plan(repo, Some("codex"), Some("primary")).unwrap().unwrap().rel_path,
            plan_p1.rel_path
        );
        assert_eq!(
            get_active_plan(repo, Some("codex"), Some("secondary")).unwrap().unwrap().rel_path,
            plan_p2.rel_path
        );

        // Clear secondary slot: primary remains untouched, secondary is removed
        clear_active_plan_slot(repo, "codex", "secondary").unwrap();
        assert_eq!(
            get_active_plan(repo, Some("codex"), Some("primary")).unwrap().unwrap().rel_path,
            plan_p1.rel_path
        );
        assert!(get_active_plan(repo, Some("codex"), Some("secondary")).unwrap().is_none());
    }
}
