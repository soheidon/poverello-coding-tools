use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const SKILL_NAMES: [&str; 3] = ["pov-plan-only", "pov-build-review", "pov-full-cycle"];

const POV_PLAN_ONLY_SKILL_MD: &str = include_str!("../resources/codex-skills/pov-plan-only/SKILL.md");
const POV_PLAN_ONLY_OPENAI_YAML: &str = include_str!("../resources/codex-skills/pov-plan-only/agents/openai.yaml");

const POV_BUILD_REVIEW_SKILL_MD: &str = include_str!("../resources/codex-skills/pov-build-review/SKILL.md");
const POV_BUILD_REVIEW_OPENAI_YAML: &str = include_str!("../resources/codex-skills/pov-build-review/agents/openai.yaml");

const POV_FULL_CYCLE_SKILL_MD: &str = include_str!("../resources/codex-skills/pov-full-cycle/SKILL.md");
const POV_FULL_CYCLE_OPENAI_YAML: &str = include_str!("../resources/codex-skills/pov-full-cycle/agents/openai.yaml");
const CODEX_REVIEW_DISPATCHER_PS1: &str = include_str!("../resources/codex-skills/pov-plan-only/scripts/codex-review-dispatcher.ps1");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CodexSkillState {
    NotInstalled,
    Installed,
    UpdateAvailable,
    ProtectedConflict,
    RecoveryRequired,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexSkillItemInfo {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub state: CodexSkillState,
    pub path: String,
    pub discovery_verified: bool,
    pub detail_message: Option<String>,
    pub has_backup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexSkillsStatus {
    pub skills_dir: String,
    pub skills: Vec<CodexSkillItemInfo>,
    pub resolution_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFileManifest {
    pub relative_path: String,
    pub content: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTransactionJournal {
    pub skill_name: String,
    pub backup_dir: String,
    pub started_at: String,
    pub files_replaced: Vec<String>,
    pub completed: bool,
}

pub fn compute_sha256(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

static TEST_HISTORICAL_MANIFESTS: std::sync::Mutex<Option<HashMap<String, Vec<Vec<SkillFileManifest>>>>> = std::sync::Mutex::new(None);

pub fn set_test_historical_manifests(manifests: Option<HashMap<String, Vec<Vec<SkillFileManifest>>>>) {
    let mut guard = TEST_HISTORICAL_MANIFESTS.lock().unwrap();
    *guard = manifests;
}

pub fn get_known_historical_manifests(skill_name: &str) -> Vec<Vec<SkillFileManifest>> {
    let guard = TEST_HISTORICAL_MANIFESTS.lock().unwrap();
    if let Some(mocks) = guard.as_ref() {
        if let Some(list) = mocks.get(skill_name) {
            return list.clone();
        }
    }
    // Fixed known official historical release manifests (v1 schema release)
    match skill_name {
        "pov-plan-only" => vec![vec![
            SkillFileManifest {
                relative_path: "SKILL.md".to_string(),
                content: String::new(),
                sha256: "78b41864b5c3af244ac9605ae48151704764c325499ee7a325df5656f1da36da".to_string(),
            },
            SkillFileManifest {
                relative_path: "agents/openai.yaml".to_string(),
                content: String::new(),
                sha256: "fc619bc1dad7e8b44d4d2da80c618b7876ba09841993d2b7fec405055ef372ed".to_string(),
            },
            SkillFileManifest {
                relative_path: "scripts/codex-review-dispatcher.ps1".to_string(),
                content: String::new(),
                sha256: "ef8a38731bc26bcf0c8a14d5d54d2fbcba9f0cf94ede6b5e13f6c3d765f487b8".to_string(),
            },
        ]],
        "pov-build-review" => vec![vec![
            SkillFileManifest {
                relative_path: "SKILL.md".to_string(),
                content: String::new(),
                sha256: "0ef4c0e94b7d0f0ac33e26e22b31eeea7416d5ab7ab0b2150c97dc73de3268de".to_string(),
            },
            SkillFileManifest {
                relative_path: "agents/openai.yaml".to_string(),
                content: String::new(),
                sha256: "fc619bc1dad7e8b44d4d2da80c618b7876ba09841993d2b7fec405055ef372ed".to_string(),
            },
            SkillFileManifest {
                relative_path: "scripts/codex-review-dispatcher.ps1".to_string(),
                content: String::new(),
                sha256: "ef8a38731bc26bcf0c8a14d5d54d2fbcba9f0cf94ede6b5e13f6c3d765f487b8".to_string(),
            },
        ]],
        "pov-full-cycle" => vec![vec![
            SkillFileManifest {
                relative_path: "SKILL.md".to_string(),
                content: String::new(),
                sha256: "18aed681e4f9276594b35565d2fc8171bdc54ee3ca4006d40b3d3afae30ec050".to_string(),
            },
            SkillFileManifest {
                relative_path: "agents/openai.yaml".to_string(),
                content: String::new(),
                sha256: "fc619bc1dad7e8b44d4d2da80c618b7876ba09841993d2b7fec405055ef372ed".to_string(),
            },
            SkillFileManifest {
                relative_path: "scripts/codex-review-dispatcher.ps1".to_string(),
                content: String::new(),
                sha256: "ef8a38731bc26bcf0c8a14d5d54d2fbcba9f0cf94ede6b5e13f6c3d765f487b8".to_string(),
            },
        ]],
        _ => vec![],
    }
}

pub fn get_skill_template_content(skill_name: &str) -> Option<String> {
    match skill_name {
        "pov-plan-only" => Some(POV_PLAN_ONLY_SKILL_MD.to_string()),
        "pov-build-review" => Some(POV_BUILD_REVIEW_SKILL_MD.to_string()),
        "pov-full-cycle" => Some(POV_FULL_CYCLE_SKILL_MD.to_string()),
        _ => None,
    }
}

pub fn get_template_manifest(skill_name: &str) -> Result<Vec<SkillFileManifest>, String> {
    match skill_name {
        "pov-plan-only" => Ok(vec![
            SkillFileManifest {
                relative_path: "SKILL.md".to_string(),
                content: POV_PLAN_ONLY_SKILL_MD.to_string(),
                sha256: compute_sha256(POV_PLAN_ONLY_SKILL_MD.as_bytes()),
            },
            SkillFileManifest {
                relative_path: "agents/openai.yaml".to_string(),
                content: POV_PLAN_ONLY_OPENAI_YAML.to_string(),
                sha256: compute_sha256(POV_PLAN_ONLY_OPENAI_YAML.as_bytes()),
            },
            SkillFileManifest {
                relative_path: "scripts/codex-review-dispatcher.ps1".to_string(),
                content: CODEX_REVIEW_DISPATCHER_PS1.to_string(),
                sha256: compute_sha256(CODEX_REVIEW_DISPATCHER_PS1.as_bytes()),
            },
        ]),
        "pov-build-review" => Ok(vec![
            SkillFileManifest {
                relative_path: "SKILL.md".to_string(),
                content: POV_BUILD_REVIEW_SKILL_MD.to_string(),
                sha256: compute_sha256(POV_BUILD_REVIEW_SKILL_MD.as_bytes()),
            },
            SkillFileManifest {
                relative_path: "agents/openai.yaml".to_string(),
                content: POV_BUILD_REVIEW_OPENAI_YAML.to_string(),
                sha256: compute_sha256(POV_BUILD_REVIEW_OPENAI_YAML.as_bytes()),
            },
            SkillFileManifest {
                relative_path: "scripts/codex-review-dispatcher.ps1".to_string(),
                content: CODEX_REVIEW_DISPATCHER_PS1.to_string(),
                sha256: compute_sha256(CODEX_REVIEW_DISPATCHER_PS1.as_bytes()),
            },
        ]),
        "pov-full-cycle" => Ok(vec![
            SkillFileManifest {
                relative_path: "SKILL.md".to_string(),
                content: POV_FULL_CYCLE_SKILL_MD.to_string(),
                sha256: compute_sha256(POV_FULL_CYCLE_SKILL_MD.as_bytes()),
            },
            SkillFileManifest {
                relative_path: "agents/openai.yaml".to_string(),
                content: POV_FULL_CYCLE_OPENAI_YAML.to_string(),
                sha256: compute_sha256(POV_FULL_CYCLE_OPENAI_YAML.as_bytes()),
            },
            SkillFileManifest {
                relative_path: "scripts/codex-review-dispatcher.ps1".to_string(),
                content: CODEX_REVIEW_DISPATCHER_PS1.to_string(),
                sha256: compute_sha256(CODEX_REVIEW_DISPATCHER_PS1.as_bytes()),
            },
        ]),
        _ => Err(format!("Unknown skill: {skill_name}")),
    }
}

pub fn resolve_codex_user_skills_dir() -> Result<PathBuf, String> {
    // Resolve user home without confusing with CODEX_HOME
    #[cfg(windows)]
    {
        let userprofile = std::env::var_os("USERPROFILE");
        let homedrive = std::env::var_os("HOMEDRIVE");
        let homepath = std::env::var_os("HOMEPATH");

        let resolved_profile = userprofile.map(PathBuf::from);
        let resolved_combo = match (homedrive, homepath) {
            (Some(d), Some(p)) => Some(PathBuf::from(d).join(p)),
            _ => None,
        };

        let home = match (resolved_profile, resolved_combo) {
            (Some(p1), Some(p2)) => {
                if p1 == p2 || p1.to_string_lossy().to_lowercase() == p2.to_string_lossy().to_lowercase() {
                    p1
                } else {
                    return Err(format!(
                        "Ambiguous user home resolution on Windows: USERPROFILE='{}', HOMEDRIVE+HOMEPATH='{}'",
                        p1.display(),
                        p2.display()
                    ));
                }
            }
            (Some(p), None) => p,
            (None, Some(p)) => p,
            (None, None) => return Err("Unable to determine Windows user home directory (%USERPROFILE% not set)".to_string()),
        };

        Ok(home.join(".agents").join("skills"))
    }

    #[cfg(not(windows))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            Ok(PathBuf::from(home).join(".agents").join("skills"))
        } else {
            Err("Unable to determine user home directory ($HOME not set)".to_string())
        }
    }
}

pub fn get_backups_dir(skills_dir: &Path) -> PathBuf {
    skills_dir.join(".poverello_backups")
}

pub fn get_journal_path(skill_dir: &Path) -> PathBuf {
    skill_dir.join(".poverello_journal.json")
}

fn check_symlink_or_reparse(path: &Path) -> bool {
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;
            if (meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
                return true;
            }
        }
        meta.file_type().is_symlink()
    } else {
        false
    }
}

pub fn inspect_codex_skill_status(skills_dir: &Path, skill_name: &str) -> CodexSkillItemInfo {
    let skill_dir = skills_dir.join(skill_name);
    let path_str = skill_dir.to_string_lossy().to_string();

    let display_name = match skill_name {
        "pov-plan-only" => "Plan Only (/pov-plan-only)",
        "pov-build-review" => "Build & Review (/pov-build-review)",
        "pov-full-cycle" => "Full Cycle (/pov-full-cycle)",
        _ => skill_name,
    }
    .to_string();

    let description = match skill_name {
        "pov-plan-only" => "計画作成と独立計画レビュー専用Skill（承認後停止）",
        "pov-build-review" => "承認済み計画に基づく実装・テスト・独立実装レビューSkill",
        "pov-full-cycle" => "計画から実装・レビュー・是正までのフルサイクル管理Skill（最大3サイクル）",
        _ => "",
    }
    .to_string();

    // Check if recovery journal exists
    let journal_path = get_journal_path(&skill_dir);
    if journal_path.is_file() {
        return CodexSkillItemInfo {
            name: skill_name.to_string(),
            display_name,
            description,
            state: CodexSkillState::RecoveryRequired,
            path: path_str,
            discovery_verified: false,
            detail_message: Some("An interrupted operation was detected. Recovery required.".to_string()),
            has_backup: true,
        };
    }

    if !skill_dir.exists() {
        return CodexSkillItemInfo {
            name: skill_name.to_string(),
            display_name,
            description,
            state: CodexSkillState::NotInstalled,
            path: path_str,
            discovery_verified: false,
            detail_message: None,
            has_backup: false,
        };
    }

    if check_symlink_or_reparse(&skill_dir) {
        return CodexSkillItemInfo {
            name: skill_name.to_string(),
            display_name,
            description,
            state: CodexSkillState::Blocked,
            path: path_str,
            discovery_verified: false,
            detail_message: Some("Directory is a symlink or reparse point. Modifications blocked.".to_string()),
            has_backup: false,
        };
    }

    let manifest = match get_template_manifest(skill_name) {
        Ok(m) => m,
        Err(e) => {
            return CodexSkillItemInfo {
                name: skill_name.to_string(),
                display_name,
                description,
                state: CodexSkillState::Blocked,
                path: path_str,
                discovery_verified: false,
                detail_message: Some(e),
                has_backup: false,
            };
        }
    };

    // Scan actual directory files
    let mut actual_files = HashMap::new();
    let mut unknown_entries = Vec::new();

    let mut dirs_to_visit = vec![skill_dir.clone()];
    while let Some(current_dir) = dirs_to_visit.pop() {
        let entries = match std::fs::read_dir(&current_dir) {
            Ok(e) => e,
            Err(_) => {
                return CodexSkillItemInfo {
                    name: skill_name.to_string(),
                    display_name,
                    description,
                    state: CodexSkillState::Blocked,
                    path: path_str,
                    discovery_verified: false,
                    detail_message: Some("Cannot read directory".to_string()),
                    has_backup: false,
                };
            }
        };

        for entry in entries.flatten() {
            let p = entry.path();
            if check_symlink_or_reparse(&p) {
                return CodexSkillItemInfo {
                    name: skill_name.to_string(),
                    display_name,
                    description,
                    state: CodexSkillState::Blocked,
                    path: path_str,
                    discovery_verified: false,
                    detail_message: Some(format!("Entry is a symlink or reparse point: {}", p.display())),
                    has_backup: false,
                };
            }

            if p.is_dir() {
                dirs_to_visit.push(p);
            } else if p.is_file() {
                if let Ok(rel) = p.strip_prefix(&skill_dir) {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    if rel_str == ".poverello_journal.json" {
                        continue;
                    }
                    if manifest.iter().any(|m| m.relative_path == rel_str) {
                        if let Ok(bytes) = std::fs::read(&p) {
                            actual_files.insert(rel_str, compute_sha256(&bytes));
                        }
                    } else {
                        unknown_entries.push(rel_str);
                    }
                }
            }
        }
    }

    if !unknown_entries.is_empty() {
        return CodexSkillItemInfo {
            name: skill_name.to_string(),
            display_name,
            description,
            state: CodexSkillState::ProtectedConflict,
            path: path_str,
            discovery_verified: false,
            detail_message: Some(format!(
                "Directory contains unknown or unmanaged entries: {}",
                unknown_entries.join(", ")
            )),
            has_backup: false,
        };
    }

    if actual_files.is_empty() {
        return CodexSkillItemInfo {
            name: skill_name.to_string(),
            display_name,
            description,
            state: CodexSkillState::NotInstalled,
            path: path_str,
            discovery_verified: false,
            detail_message: None,
            has_backup: false,
        };
    }

    let mut all_match = true;
    for m in &manifest {
        match actual_files.get(&m.relative_path) {
            Some(hash) if hash == &m.sha256 => {}
            _ => {
                all_match = false;
                break;
            }
        }
    }

    if all_match && actual_files.len() == manifest.len() {
        CodexSkillItemInfo {
            name: skill_name.to_string(),
            display_name,
            description,
            state: CodexSkillState::Installed,
            path: path_str,
            discovery_verified: false, // File is present; Codex live discovery is separate & unverified offline
            detail_message: None,
            has_backup: false,
        }
    } else {
        // Check if actual files match any known official historical manifest
        let historical_manifests = get_known_historical_manifests(skill_name);
        let matches_historical = historical_manifests.iter().any(|hist| {
            if hist.len() != actual_files.len() {
                return false;
            }
            hist.iter().all(|f| {
                actual_files.get(&f.relative_path).map(|h| h == &f.sha256).unwrap_or(false)
            })
        });

        if matches_historical {
            CodexSkillItemInfo {
                name: skill_name.to_string(),
                display_name,
                description,
                state: CodexSkillState::UpdateAvailable,
                path: path_str,
                discovery_verified: false,
                detail_message: Some("A standard template update is available.".to_string()),
                has_backup: false,
            }
        } else {
            // User-modified or unknown file content is protected from overwrite!
            CodexSkillItemInfo {
                name: skill_name.to_string(),
                display_name,
                description,
                state: CodexSkillState::ProtectedConflict,
                path: path_str,
                discovery_verified: false,
                detail_message: Some("Skill files have been modified or do not match a known standard template. Overwrite protected.".to_string()),
                has_backup: false,
            }
        }
    }
}

pub fn get_codex_skills_status_at(skills_dir: &Path) -> CodexSkillsStatus {
    let mut skills = Vec::new();
    for name in SKILL_NAMES {
        skills.push(inspect_codex_skill_status(skills_dir, name));
    }
    CodexSkillsStatus {
        skills_dir: skills_dir.to_string_lossy().to_string(),
        skills,
        resolution_error: None,
    }
}

pub fn get_codex_skills_status() -> CodexSkillsStatus {
    match resolve_codex_user_skills_dir() {
        Ok(dir) => get_codex_skills_status_at(&dir),
        Err(e) => CodexSkillsStatus {
            skills_dir: String::new(),
            skills: Vec::new(),
            resolution_error: Some(e),
        },
    }
}

pub fn install_codex_skill_at(skills_dir: &Path, skill_name: &str) -> Result<CodexSkillItemInfo, String> {
    let skill_dir = skills_dir.join(skill_name);
    let manifest = get_template_manifest(skill_name)?;

    if skill_dir.exists() {
        let status = inspect_codex_skill_status(skills_dir, skill_name);
        if status.state != CodexSkillState::NotInstalled {
            return Err(format!(
                "Cannot install skill '{}': directory already exists with state {:?}",
                skill_name, status.state
            ));
        }
    }

    std::fs::create_dir_all(&skill_dir).map_err(|e| format!("Failed to create skill dir: {e}"))?;

    for m in &manifest {
        let target_file = skill_dir.join(&m.relative_path);
        if let Some(parent) = target_file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create parent dir: {e}"))?;
        }
        std::fs::write(&target_file, &m.content).map_err(|e| format!("Failed to write {}: {e}", m.relative_path))?;

        // Read-back verification
        let read_back = std::fs::read(&target_file).map_err(|e| format!("Failed to read-back {}: {e}", m.relative_path))?;
        let hash = compute_sha256(&read_back);
        if hash != m.sha256 {
            return Err(format!("Hash mismatch on install read-back for {}", m.relative_path));
        }
    }

    Ok(inspect_codex_skill_status(skills_dir, skill_name))
}

pub fn update_codex_skill_at(skills_dir: &Path, skill_name: &str) -> Result<CodexSkillItemInfo, String> {
    let skill_dir = skills_dir.join(skill_name);
    let manifest = get_template_manifest(skill_name)?;

    let status = inspect_codex_skill_status(skills_dir, skill_name);
    if status.state == CodexSkillState::Blocked {
        return Err("Cannot update: Skill directory is blocked (symlink or reparse point).".to_string());
    }
    if status.state == CodexSkillState::ProtectedConflict {
        return Err("Cannot update: Skill files have been modified or contain unmanaged files. Overwrite protected.".to_string());
    }
    if status.state != CodexSkillState::UpdateAvailable {
        return Err(format!("Cannot update: Skill is not eligible for update (current state: {:?}).", status.state));
    }

    // Step 1: Create backup
    let backups_dir = get_backups_dir(skills_dir);
    std::fs::create_dir_all(&backups_dir).map_err(|e| format!("Failed to create backups dir: {e}"))?;
    let backup_uuid = Uuid::new_v4().to_string();
    let this_backup_dir = backups_dir.join(format!("{}-{}-backup", skill_name, backup_uuid));
    std::fs::create_dir_all(&this_backup_dir).map_err(|e| format!("Failed to create backup dir: {e}"))?;

    // Copy existing files to backup
    for m in &manifest {
        let src = skill_dir.join(&m.relative_path);
        if src.is_file() {
            let dst = this_backup_dir.join(&m.relative_path);
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create backup sub-dir: {e}"))?;
            }
            std::fs::copy(&src, &dst).map_err(|e| format!("Failed to backup file {}: {e}", m.relative_path))?;
            // Verify backup hash
            let b_bytes = std::fs::read(&dst).map_err(|e| format!("Failed to read backup: {e}"))?;
            let s_bytes = std::fs::read(&src).map_err(|e| format!("Failed to read source: {e}"))?;
            if compute_sha256(&b_bytes) != compute_sha256(&s_bytes) {
                return Err(format!("Backup hash verification failed for {}", m.relative_path));
            }
        }
    }

    // Step 2: Write transaction journal
    let journal = SkillTransactionJournal {
        skill_name: skill_name.to_string(),
        backup_dir: this_backup_dir.to_string_lossy().to_string(),
        started_at: chrono::Utc::now().to_rfc3339(),
        files_replaced: Vec::new(),
        completed: false,
    };
    let journal_path = get_journal_path(&skill_dir);
    let j_bytes = serde_json::to_vec_pretty(&journal).map_err(|e| format!("Journal error: {e}"))?;
    std::fs::write(&journal_path, j_bytes).map_err(|e| format!("Failed to write journal: {e}"))?;

    // Step 3: Replace files one by one with atomic replace and read-back
    let mut updated_journal = journal;
    for m in &manifest {
        let target_file = skill_dir.join(&m.relative_path);
        if let Some(parent) = target_file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create parent dir: {e}"))?;
        }

        // Staged temporary file on same volume
        let temp_file = skill_dir.join(format!("{}.staged-{}", m.relative_path.replace('/', "_"), Uuid::new_v4()));
        std::fs::write(&temp_file, &m.content).map_err(|e| format!("Failed to write staged file: {e}"))?;

        // Replace
        #[cfg(windows)]
        {
            let _ = std::fs::remove_file(&target_file);
        }
        std::fs::rename(&temp_file, &target_file).map_err(|e| format!("Atomic replace failed for {}: {e}", m.relative_path))?;

        // Read-back verification
        let read_back = std::fs::read(&target_file).map_err(|e| format!("Read-back failed for {}: {e}", m.relative_path))?;
        if compute_sha256(&read_back) != m.sha256 {
            return Err(format!("Hash verification failed after replace for {}", m.relative_path));
        }

        updated_journal.files_replaced.push(m.relative_path.clone());
        let j_bytes = serde_json::to_vec_pretty(&updated_journal).map_err(|e| format!("Journal error: {e}"))?;
        let _ = std::fs::write(&journal_path, j_bytes);
    }

    // Step 4: Remove journal on complete success
    let _ = std::fs::remove_file(&journal_path);

    Ok(inspect_codex_skill_status(skills_dir, skill_name))
}

pub fn restore_codex_skill_backup_at(skills_dir: &Path, skill_name: &str) -> Result<CodexSkillItemInfo, String> {
    let skill_dir = skills_dir.join(skill_name);
    let journal_path = get_journal_path(&skill_dir);

    if !journal_path.is_file() {
        return Err("No active transaction journal found to restore from.".to_string());
    }

    let bytes = std::fs::read(&journal_path).map_err(|e| format!("Failed to read journal: {e}"))?;
    let journal: SkillTransactionJournal = serde_json::from_slice(&bytes).map_err(|e| format!("Invalid journal: {e}"))?;

    let backup_dir = PathBuf::from(&journal.backup_dir);
    if !backup_dir.is_dir() {
        return Err(format!("Backup directory '{}' does not exist.", backup_dir.display()));
    }

    // Restore files from backup
    let manifest = get_template_manifest(skill_name)?;
    for m in &manifest {
        let b_file = backup_dir.join(&m.relative_path);
        let dst_file = skill_dir.join(&m.relative_path);
        if b_file.is_file() {
            if let Some(parent) = dst_file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::copy(&b_file, &dst_file).map_err(|e| format!("Failed to restore file {}: {e}", m.relative_path))?;
        }
    }

    // Remove journal
    let _ = std::fs::remove_file(&journal_path);

    Ok(inspect_codex_skill_status(skills_dir, skill_name))
}

pub fn remove_codex_skill_at(skills_dir: &Path, skill_name: &str) -> Result<CodexSkillItemInfo, String> {
    let skill_dir = skills_dir.join(skill_name);

    let status = inspect_codex_skill_status(skills_dir, skill_name);
    if status.state == CodexSkillState::NotInstalled {
        return Ok(status);
    }
    if status.state == CodexSkillState::ProtectedConflict {
        return Err("Cannot remove: Skill contains unknown or conflicting files. Please remove manually.".to_string());
    }
    if status.state == CodexSkillState::Blocked {
        return Err("Cannot remove: Skill directory is a symlink or reparse point.".to_string());
    }

    // Quarantine rename: rename <skill-id> to <skill-id>.quarantined-<uuid> in parent
    let quarantine_name = format!("{}.quarantined-{}", skill_name, Uuid::new_v4());
    let quarantine_path = skills_dir.join(quarantine_name);

    std::fs::rename(&skill_dir, &quarantine_path)
        .map_err(|e| format!("Failed to quarantine rename skill dir: {e}"))?;

    Ok(inspect_codex_skill_status(skills_dir, skill_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_template_manifests_validity() {
        for name in SKILL_NAMES {
            let m = get_template_manifest(name).expect("manifest should load");
            assert_eq!(m.len(), 3);
            assert!(m.iter().any(|f| f.relative_path == "SKILL.md"));
            assert!(m.iter().any(|f| f.relative_path == "agents/openai.yaml"));
            assert!(m.iter().any(|f| f.relative_path == "scripts/codex-review-dispatcher.ps1"));
            for file in &m {
                assert!(!file.content.is_empty());
                assert_eq!(compute_sha256(file.content.as_bytes()), file.sha256);
            }
        }
    }

    #[test]
    fn test_full_cycle_template_contains_cycle_limits() {
        let m = get_template_manifest("pov-full-cycle").unwrap();
        let skill_md = m.iter().find(|f| f.relative_path == "SKILL.md").unwrap();
        assert!(skill_md.content.contains("最大3"));
        assert!(skill_md.content.contains("上限到達時"));
    }

    #[test]
    fn test_install_and_inspect_codex_skill() {
        let dir = tempdir().unwrap();
        let skills_dir = dir.path();

        let initial_status = inspect_codex_skill_status(skills_dir, "pov-plan-only");
        assert_eq!(initial_status.state, CodexSkillState::NotInstalled);
        assert!(!initial_status.discovery_verified);

        let installed = install_codex_skill_at(skills_dir, "pov-plan-only").expect("install should succeed");
        assert_eq!(installed.state, CodexSkillState::Installed);
        assert!(!installed.discovery_verified);

        // Modify a file to simulate unknown user modification -> ProtectedConflict
        let skill_md = skills_dir.join("pov-plan-only").join("SKILL.md");
        std::fs::write(&skill_md, "modified content").unwrap();
        let updated_status = inspect_codex_skill_status(skills_dir, "pov-plan-only");
        assert_eq!(updated_status.state, CodexSkillState::ProtectedConflict);

        // Add unknown file to simulate protected conflict
        let unknown = skills_dir.join("pov-plan-only").join("extra.txt");
        std::fs::write(&unknown, "extra").unwrap();
        let conflict_status = inspect_codex_skill_status(skills_dir, "pov-plan-only");
        assert_eq!(conflict_status.state, CodexSkillState::ProtectedConflict);
        // Remove cannot proceed with unknown files
        let rem_err = remove_codex_skill_at(skills_dir, "pov-plan-only");
        assert!(rem_err.is_err());

        // Remove unknown file and restore SKILL.md to standard, then remove skill via quarantine
        std::fs::remove_file(&unknown).unwrap();
        let m = get_template_manifest("pov-plan-only").unwrap();
        let skill_content = &m.iter().find(|f| f.relative_path == "SKILL.md").unwrap().content;
        std::fs::write(&skill_md, skill_content).unwrap();

        let rem_ok = remove_codex_skill_at(skills_dir, "pov-plan-only");
        assert!(rem_ok.is_ok());
        assert_eq!(rem_ok.unwrap().state, CodexSkillState::NotInstalled);
        // Verify quarantine folder exists
        let entries: Vec<_> = std::fs::read_dir(skills_dir).unwrap().flatten().collect();
        assert!(entries.iter().any(|e| e.file_name().to_string_lossy().contains("quarantined")));
    }

    #[test]
    fn test_user_modified_skill_is_protected_from_overwrite() {
        let dir = tempdir().unwrap();
        let skills_dir = dir.path();

        install_codex_skill_at(skills_dir, "pov-plan-only").unwrap();

        // User modifies SKILL.md
        let skill_md = skills_dir.join("pov-plan-only").join("SKILL.md");
        std::fs::write(&skill_md, "# My Custom Modifications\nDo not overwrite!").unwrap();

        let status = inspect_codex_skill_status(skills_dir, "pov-plan-only");
        assert_eq!(status.state, CodexSkillState::ProtectedConflict);
        assert!(status.detail_message.as_ref().unwrap().contains("Overwrite protected"));

        // Attempting to update must fail and preserve custom content
        let update_res = update_codex_skill_at(skills_dir, "pov-plan-only");
        assert!(update_res.is_err());
        assert!(update_res.unwrap_err().contains("Overwrite protected"));

        let content_after = std::fs::read_to_string(&skill_md).unwrap();
        assert_eq!(content_after, "# My Custom Modifications\nDo not overwrite!");
    }

    #[test]
    fn test_update_with_backup_and_recovery() {
        let dir = tempdir().unwrap();
        let skills_dir = dir.path();

        // Register fake historical manifest with "old backup version"
        let old_hash = compute_sha256(b"old backup version");
        let yaml_manifest = get_template_manifest("pov-build-review").unwrap();
        let yaml_file = yaml_manifest.iter().find(|f| f.relative_path == "agents/openai.yaml").unwrap();
        let disp_file = yaml_manifest.iter().find(|f| f.relative_path == "scripts/codex-review-dispatcher.ps1").unwrap();
        let mut mocks = HashMap::new();
        mocks.insert(
            "pov-build-review".to_string(),
            vec![vec![
                SkillFileManifest {
                    relative_path: "SKILL.md".to_string(),
                    content: "old backup version".to_string(),
                    sha256: old_hash,
                },
                yaml_file.clone(),
                disp_file.clone(),
            ]],
        );
        set_test_historical_manifests(Some(mocks));

        install_codex_skill_at(skills_dir, "pov-build-review").unwrap();

        // Write historical file to simulate outdated official version
        let skill_md = skills_dir.join("pov-build-review").join("SKILL.md");
        std::fs::write(&skill_md, "old backup version").unwrap();
        let st_outdated = inspect_codex_skill_status(skills_dir, "pov-build-review");
        assert_eq!(st_outdated.state, CodexSkillState::UpdateAvailable);

        // Update succeeds when matching historical manifest
        let updated = update_codex_skill_at(skills_dir, "pov-build-review").unwrap();
        assert_eq!(updated.state, CodexSkillState::Installed);

        // Simulate interrupted journal with actual backup files
        let skill_dir = skills_dir.join("pov-build-review");
        let journal_path = get_journal_path(&skill_dir);
        let backups_dir = get_backups_dir(skills_dir);
        let fake_backup = backups_dir.join("fake-backup");
        std::fs::create_dir_all(&fake_backup).unwrap();
        std::fs::write(fake_backup.join("SKILL.md"), "old backup version").unwrap();
        let journal = SkillTransactionJournal {
            skill_name: "pov-build-review".to_string(),
            backup_dir: fake_backup.to_string_lossy().to_string(),
            started_at: chrono::Utc::now().to_rfc3339(),
            files_replaced: vec!["SKILL.md".to_string()],
            completed: false,
        };
        std::fs::write(&journal_path, serde_json::to_vec(&journal).unwrap()).unwrap();

        let st = inspect_codex_skill_status(skills_dir, "pov-build-review");
        assert_eq!(st.state, CodexSkillState::RecoveryRequired);

        // Restore backup restores historical content -> state becomes UpdateAvailable
        let restored = restore_codex_skill_backup_at(skills_dir, "pov-build-review").unwrap();
        assert_eq!(restored.state, CodexSkillState::UpdateAvailable);

        set_test_historical_manifests(None);
    }

    #[test]
    fn test_dispatcher_script_contract_and_job_object_mechanisms() {
        for name in SKILL_NAMES {
            let manifest = get_template_manifest(name).expect("manifest should load");
            let disp = manifest
                .iter()
                .find(|f| f.relative_path == "scripts/codex-review-dispatcher.ps1")
                .expect("dispatcher must be in manifest");

            // Verify Windows Job Object & confinement mechanisms
            assert!(disp.content.contains("CREATE_SUSPENDED"));
            assert!(disp.content.contains("JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE"));
            assert!(disp.content.contains("AssignProcessToJobObject"));
            assert!(disp.content.contains("ResumeThread"));
            assert!(disp.content.contains("TerminateJobObject"));
            assert!(disp.content.contains("PoverelloProcessJobRunner"));
            assert!(disp.content.contains("SetHandleInformation"));

            // Verify Gemini strict sandbox and UNAVAILABLE fail-closed contract
            assert!(disp.content.contains("--approval-mode=plan"));
            assert!(disp.content.contains("Gemini CLI read-only sandbox environment cannot be verified on this platform; reviewer is UNAVAILABLE"));

            // Verify snapshot reparse point / symlink recursive escape protection
            assert!(disp.content.contains("reparseChildren"));

            // Verify DeepSeek headless positional task argument (no -p)
            assert!(disp.content.contains("--permission-mode"));
            assert!(disp.content.contains("read-only"));
            assert!(disp.content.contains("--approval-policy"));
            assert!(disp.content.contains("never"));
            assert!(disp.content.contains("Official headless dsh syntax takes positional task argument without -p"));

            // Verify SKILL.md parallel review contract updates
            let skill_md = manifest
                .iter()
                .find(|f| f.relative_path == "SKILL.md")
                .expect("SKILL.md must be in manifest");
            assert!(skill_md.content.contains("Windows Job Object"));
            assert!(skill_md.content.contains("CREATE_SUSPENDED"));
            assert!(skill_md.content.contains("UNAVAILABLE"));
            assert!(skill_md.content.contains("位置引数"));
            assert!(skill_md.content.contains("再帰検証"));
        }
    }

    #[test]
    #[cfg(windows)]
    fn test_dispatcher_offline_synthetic_enforcement() {
        let dir = tempdir().unwrap();
        let snapshot_dir = dir.path().join("snapshot");
        std::fs::create_dir_all(&snapshot_dir).unwrap();
        std::fs::write(snapshot_dir.join("test.txt"), "hello").unwrap();

        let script_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("codex-skills")
            .join("pov-plan-only")
            .join("scripts")
            .join("codex-review-dispatcher.ps1");

        assert!(script_path.is_file(), "dispatcher script must exist");

        // 1. Synthetic test: Gemini without verified sandbox must return UNAVAILABLE
        let output = std::process::Command::new("pwsh")
            .arg("-File")
            .arg(&script_path)
            .arg("-SnapshotDir")
            .arg(&snapshot_dir)
            .arg("-Reviewers")
            .arg("gemini")
            .arg("-TimeoutSeconds")
            .arg("5")
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            assert!(
                stdout.contains("UNAVAILABLE"),
                "Gemini without sandbox must be UNAVAILABLE, got stdout: {stdout}"
            );
        }

        // 2. Synthetic test: 0 reviewers must return REVIEW_SKIPPED
        let output_skip = std::process::Command::new("pwsh")
            .arg("-Command")
            .arg(format!(
                "& '{}' -SnapshotDir '{}' -Reviewers @() -TimeoutSeconds 5",
                script_path.display(),
                snapshot_dir.display()
            ))
            .output();

        if let Ok(out) = output_skip {
            let stdout = String::from_utf8_lossy(&out.stdout);
            assert!(
                stdout.contains("REVIEW_SKIPPED"),
                "0 reviewers must return REVIEW_SKIPPED, got: {stdout}"
            );
        }
    }
}
