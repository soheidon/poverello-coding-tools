use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const MANAGED_BLOCK_TEMPLATE: &str = include_str!("../resources/codex-global-rules/managed-block.md");
const START_MARKER: &str = "<!-- BEGIN POVERELLO MANAGED RULES -->";
const END_MARKER: &str = "<!-- END POVERELLO MANAGED RULES -->";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CodexGlobalRulesState {
    NotInstalled,
    Installed,
    UpdateAvailable,
    Shadowed,
    Conflict,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexGlobalRulesStatus {
    pub codex_home: String,
    pub agents_md_path: String,
    pub state: CodexGlobalRulesState,
    pub is_shadowed: bool,
    pub shadowed_by: Option<String>,
    pub detail_message: Option<String>,
    pub resolution_error: Option<String>,
}

pub fn compute_sha256(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

pub fn resolve_codex_home() -> Result<PathBuf, String> {
    if let Some(codex_home) = std::env::var_os("CODEX_HOME") {
        if !codex_home.is_empty() {
            return Ok(PathBuf::from(codex_home));
        }
    }

    #[cfg(windows)]
    {
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            Ok(PathBuf::from(userprofile).join(".codex"))
        } else {
            Err("Unable to determine user profile directory (%USERPROFILE% not set)".to_string())
        }
    }

    #[cfg(not(windows))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            Ok(PathBuf::from(home).join(".codex"))
        } else {
            Err("Unable to determine user home directory ($HOME not set)".to_string())
        }
    }
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

pub fn check_ancestors_symlink_or_reparse(path: &Path) -> bool {
    let mut cur = Some(path);
    while let Some(p) = cur {
        if check_symlink_or_reparse(p) {
            return true;
        }
        cur = p.parent();
    }
    false
}

pub fn get_global_rules_template_content() -> String {
    MANAGED_BLOCK_TEMPLATE.to_string()
}

pub fn create_global_rules_backup(codex_home: &Path) -> Result<Option<PathBuf>, String> {
    let agents_md = codex_home.join("AGENTS.md");
    if !agents_md.exists() {
        return Ok(None);
    }
    let backups_dir = codex_home.join(".poverello_backups").join("global-rules");
    std::fs::create_dir_all(&backups_dir).map_err(|e| format!("Failed to create global rules backup dir: {e}"))?;
    let backup_file = backups_dir.join(format!("AGENTS.md.backup-{}", uuid::Uuid::new_v4()));
    std::fs::copy(&agents_md, &backup_file).map_err(|e| format!("Failed to backup AGENTS.md before write: {e}"))?;
    Ok(Some(backup_file))
}

pub fn inspect_codex_global_rules_at(codex_home: &Path) -> CodexGlobalRulesStatus {
    let home_str = codex_home.to_string_lossy().to_string();
    let agents_md = codex_home.join("AGENTS.md");
    let agents_md_str = agents_md.to_string_lossy().to_string();
    let override_md = codex_home.join("AGENTS.override.md");

    let is_shadowed = override_md.is_file();
    let shadowed_by = if is_shadowed {
        Some(override_md.to_string_lossy().to_string())
    } else {
        None
    };

    if check_ancestors_symlink_or_reparse(codex_home) || check_symlink_or_reparse(&agents_md) || (override_md.exists() && check_symlink_or_reparse(&override_md)) {
        return CodexGlobalRulesStatus {
            codex_home: home_str,
            agents_md_path: agents_md_str,
            state: CodexGlobalRulesState::Blocked,
            is_shadowed,
            shadowed_by,
            detail_message: Some("File or parent/ancestor directory is a symlink or reparse point. Modifications blocked.".to_string()),
            resolution_error: None,
        };
    }

    if !agents_md.exists() {
        return CodexGlobalRulesStatus {
            codex_home: home_str,
            agents_md_path: agents_md_str,
            state: if is_shadowed {
                CodexGlobalRulesState::Shadowed
            } else {
                CodexGlobalRulesState::NotInstalled
            },
            is_shadowed,
            shadowed_by,
            detail_message: if is_shadowed {
                Some("AGENTS.override.md is present and takes precedence over AGENTS.md.".to_string())
            } else {
                None
            },
            resolution_error: None,
        };
    }

    let content = match std::fs::read_to_string(&agents_md) {
        Ok(c) => c,
        Err(e) => {
            return CodexGlobalRulesStatus {
                codex_home: home_str,
                agents_md_path: agents_md_str,
                state: CodexGlobalRulesState::Blocked,
                is_shadowed,
                shadowed_by,
                detail_message: Some(format!("Failed to read AGENTS.md: {e}")),
                resolution_error: None,
            };
        }
    };

    let start_matches: Vec<_> = content.match_indices(START_MARKER).collect();
    let end_matches: Vec<_> = content.match_indices(END_MARKER).collect();

    if start_matches.is_empty() && end_matches.is_empty() {
        return CodexGlobalRulesStatus {
            codex_home: home_str,
            agents_md_path: agents_md_str,
            state: if is_shadowed {
                CodexGlobalRulesState::Shadowed
            } else {
                CodexGlobalRulesState::NotInstalled
            },
            is_shadowed,
            shadowed_by,
            detail_message: if is_shadowed {
                Some("AGENTS.override.md is present and takes precedence. AGENTS.md contains unmanaged content.".to_string())
            } else {
                Some("AGENTS.md exists but contains no Poverello managed rules block. Existing content will be preserved on install.".to_string())
            },
            resolution_error: None,
        };
    }

    if start_matches.len() != 1 || end_matches.len() != 1 || start_matches[0].0 >= end_matches[0].0 {
        return CodexGlobalRulesStatus {
            codex_home: home_str,
            agents_md_path: agents_md_str,
            state: CodexGlobalRulesState::Conflict,
            is_shadowed,
            shadowed_by,
            detail_message: Some("Corrupted or multiple managed block markers detected in AGENTS.md. Manual resolution required.".to_string()),
            resolution_error: None,
        };
    }

    let block_start = start_matches[0].0;
    let block_end = end_matches[0].0 + END_MARKER.len();
    let existing_block = &content[block_start..block_end];

    let template_block = MANAGED_BLOCK_TEMPLATE.trim();
    let existing_trimmed = existing_block.trim();

    if is_shadowed {
        CodexGlobalRulesStatus {
            codex_home: home_str,
            agents_md_path: agents_md_str,
            state: CodexGlobalRulesState::Shadowed,
            is_shadowed: true,
            shadowed_by,
            detail_message: Some("This AGENTS.md is currently shadowed and inactive because AGENTS.override.md is present.".to_string()),
            resolution_error: None,
        }
    } else if existing_trimmed == template_block {
        CodexGlobalRulesStatus {
            codex_home: home_str,
            agents_md_path: agents_md_str,
            state: CodexGlobalRulesState::Installed,
            is_shadowed: false,
            shadowed_by: None,
            detail_message: None,
            resolution_error: None,
        }
    } else {
        CodexGlobalRulesStatus {
            codex_home: home_str,
            agents_md_path: agents_md_str,
            state: CodexGlobalRulesState::UpdateAvailable,
            is_shadowed: false,
            shadowed_by: None,
            detail_message: Some("A new template version is available for the managed rules block.".to_string()),
            resolution_error: None,
        }
    }
}

pub fn get_codex_global_rules_status() -> CodexGlobalRulesStatus {
    match resolve_codex_home() {
        Ok(dir) => inspect_codex_global_rules_at(&dir),
        Err(e) => CodexGlobalRulesStatus {
            codex_home: String::new(),
            agents_md_path: String::new(),
            state: CodexGlobalRulesState::Blocked,
            is_shadowed: false,
            shadowed_by: None,
            detail_message: None,
            resolution_error: Some(e),
        },
    }
}

pub fn install_or_update_codex_global_rules_at(codex_home: &Path) -> Result<CodexGlobalRulesStatus, String> {
    if check_ancestors_symlink_or_reparse(codex_home) {
        return Err("Cannot install or update: Codex home or an ancestor directory is a symlink or reparse point.".to_string());
    }

    std::fs::create_dir_all(codex_home).map_err(|e| format!("Failed to create codex home dir: {e}"))?;
    let agents_md = codex_home.join("AGENTS.md");

    // Pre-write backup
    let _backup = create_global_rules_backup(codex_home)?;

    let template = MANAGED_BLOCK_TEMPLATE.trim();

    let new_content = if !agents_md.exists() {
        format!("{template}\n")
    } else {
        let current = std::fs::read_to_string(&agents_md).map_err(|e| format!("Failed to read AGENTS.md: {e}"))?;
        let start_matches: Vec<_> = current.match_indices(START_MARKER).collect();
        let end_matches: Vec<_> = current.match_indices(END_MARKER).collect();

        if start_matches.is_empty() && end_matches.is_empty() {
            // Append managed block preserving all existing text
            let sep = if current.ends_with("\n\n") {
                ""
            } else if current.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            format!("{current}{sep}{template}\n")
        } else if start_matches.len() == 1 && end_matches.len() == 1 && start_matches[0].0 < end_matches[0].0 {
            // Replace managed block
            let before = &current[..start_matches[0].0];
            let after = &current[end_matches[0].0 + END_MARKER.len()..];
            format!("{before}{template}{after}")
        } else {
            return Err("Cannot install or update: AGENTS.md has corrupted or multiple managed markers.".to_string());
        }
    };

    // Staged write with safe replace and read-back
    let staged = codex_home.join(format!("AGENTS.md.staged-{}", uuid::Uuid::new_v4()));
    std::fs::write(&staged, &new_content).map_err(|e| format!("Failed to write staged AGENTS.md: {e}"))?;

    let staged_bytes = std::fs::read(&staged).map_err(|e| format!("Failed to read staged AGENTS.md: {e}"))?;
    if compute_sha256(&staged_bytes) != compute_sha256(new_content.as_bytes()) {
        let _ = std::fs::remove_file(&staged);
        return Err("Staged write integrity check failed".to_string());
    }

    #[cfg(windows)]
    {
        if agents_md.exists() {
            let old_temp = codex_home.join(format!("AGENTS.md.old-{}", uuid::Uuid::new_v4()));
            std::fs::rename(&agents_md, &old_temp).map_err(|e| format!("Failed to move old AGENTS.md: {e}"))?;
            if let Err(e) = std::fs::rename(&staged, &agents_md) {
                let _ = std::fs::rename(&old_temp, &agents_md);
                return Err(format!("Failed to place new AGENTS.md (restored original): {e}"));
            }
            let _ = std::fs::remove_file(&old_temp);
        } else {
            std::fs::rename(&staged, &agents_md).map_err(|e| format!("Failed to rename staged AGENTS.md: {e}"))?;
        }
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(&staged, &agents_md).map_err(|e| format!("Failed to replace AGENTS.md: {e}"))?;
    }

    // Read-back verification
    let read_back = std::fs::read_to_string(&agents_md).map_err(|e| format!("Read-back failed: {e}"))?;
    if read_back != new_content {
        return Err("Read-back verification failed after writing AGENTS.md".to_string());
    }

    Ok(inspect_codex_global_rules_at(codex_home))
}

pub fn remove_codex_global_rules_at(codex_home: &Path) -> Result<CodexGlobalRulesStatus, String> {
    if check_ancestors_symlink_or_reparse(codex_home) {
        return Err("Cannot remove: Codex home or an ancestor directory is a symlink or reparse point.".to_string());
    }

    let agents_md = codex_home.join("AGENTS.md");
    if !agents_md.exists() {
        return Ok(inspect_codex_global_rules_at(codex_home));
    }

    // Pre-removal backup
    let _backup = create_global_rules_backup(codex_home)?;

    let current = std::fs::read_to_string(&agents_md).map_err(|e| format!("Failed to read AGENTS.md: {e}"))?;
    let start_matches: Vec<_> = current.match_indices(START_MARKER).collect();
    let end_matches: Vec<_> = current.match_indices(END_MARKER).collect();

    if start_matches.len() != 1 || end_matches.len() != 1 || start_matches[0].0 >= end_matches[0].0 {
        return Err("Cannot remove managed block: markers are missing or corrupted.".to_string());
    }

    let before = &current[..start_matches[0].0];
    let after = &current[end_matches[0].0 + END_MARKER.len()..];
    let remaining = format!("{}{}", before.trim_end(), after);
    let trimmed_remaining = remaining.trim();

    if trimmed_remaining.is_empty() {
        // If file was solely the managed block, remove file and verify
        let _ = std::fs::remove_file(&agents_md);
        if agents_md.exists() {
            return Err("Failed to verify removal of AGENTS.md".to_string());
        }
    } else {
        let new_content = format!("{trimmed_remaining}\n");
        let staged = codex_home.join(format!("AGENTS.md.staged-{}", uuid::Uuid::new_v4()));
        std::fs::write(&staged, &new_content).map_err(|e| format!("Failed to write staged AGENTS.md: {e}"))?;

        #[cfg(windows)]
        {
            let old_temp = codex_home.join(format!("AGENTS.md.old-{}", uuid::Uuid::new_v4()));
            std::fs::rename(&agents_md, &old_temp).map_err(|e| format!("Failed to move old AGENTS.md: {e}"))?;
            if let Err(e) = std::fs::rename(&staged, &agents_md) {
                let _ = std::fs::rename(&old_temp, &agents_md);
                return Err(format!("Failed to place updated AGENTS.md (restored original): {e}"));
            }
            let _ = std::fs::remove_file(&old_temp);
        }
        #[cfg(not(windows))]
        {
            std::fs::rename(&staged, &agents_md).map_err(|e| format!("Failed to replace AGENTS.md: {e}"))?;
        }

        // Read-back verification
        let read_back = std::fs::read_to_string(&agents_md).map_err(|e| format!("Read-back failed: {e}"))?;
        if read_back != new_content {
            return Err("Read-back verification failed after removing managed block from AGENTS.md".to_string());
        }
    }

    Ok(inspect_codex_global_rules_at(codex_home))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_codex_global_rules_lifecycle() {
        let dir = tempdir().unwrap();
        let home = dir.path();

        // 1. Initial: NotInstalled
        let st = inspect_codex_global_rules_at(home);
        assert_eq!(st.state, CodexGlobalRulesState::NotInstalled);
        assert!(!st.is_shadowed);

        // 2. Install into clean home
        let installed = install_or_update_codex_global_rules_at(home).unwrap();
        assert_eq!(installed.state, CodexGlobalRulesState::Installed);

        // 3. Shadowed by AGENTS.override.md
        let override_file = home.join("AGENTS.override.md");
        std::fs::write(&override_file, "# Override instructions").unwrap();
        let shadowed_st = inspect_codex_global_rules_at(home);
        assert_eq!(shadowed_st.state, CodexGlobalRulesState::Shadowed);
        assert!(shadowed_st.is_shadowed);
        assert!(shadowed_st.shadowed_by.is_some());
        // Verify override file was NOT touched
        assert_eq!(std::fs::read_to_string(&override_file).unwrap(), "# Override instructions");

        // Remove override
        std::fs::remove_file(&override_file).unwrap();
        assert_eq!(inspect_codex_global_rules_at(home).state, CodexGlobalRulesState::Installed);

        // 4. Preserve existing user content on append
        let home2 = tempdir().unwrap();
        let custom_file = home2.path().join("AGENTS.md");
        std::fs::write(&custom_file, "# My Custom Instructions\nSome user content here.\n").unwrap();
        let appended = install_or_update_codex_global_rules_at(home2.path()).unwrap();
        assert_eq!(appended.state, CodexGlobalRulesState::Installed);
        let final_text = std::fs::read_to_string(&custom_file).unwrap();
        assert!(final_text.contains("# My Custom Instructions"));
        assert!(final_text.contains(START_MARKER));
        assert!(final_text.contains(END_MARKER));

        // 5. Remove managed block preserves custom content
        let removed = remove_codex_global_rules_at(home2.path()).unwrap();
        assert_eq!(removed.state, CodexGlobalRulesState::NotInstalled);
        let text_after_remove = std::fs::read_to_string(&custom_file).unwrap();
        assert!(text_after_remove.contains("# My Custom Instructions"));
        assert!(!text_after_remove.contains(START_MARKER));
    }

    #[test]
    fn test_corrupted_markers_conflict() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        let file = home.join("AGENTS.md");
        std::fs::write(&file, format!("{START_MARKER}\nDuplicate {START_MARKER}\n{END_MARKER}")).unwrap();
        let st = inspect_codex_global_rules_at(home);
        assert_eq!(st.state, CodexGlobalRulesState::Conflict);
    }

    #[test]
    fn test_global_rules_backup_creation() {
        let dir = tempdir().unwrap();
        let home = dir.path();

        let agents_md = home.join("AGENTS.md");
        std::fs::write(&agents_md, "# Original user text\n").unwrap();

        // Mutating creates backup in .poverello_backups/global-rules
        let _ = install_or_update_codex_global_rules_at(home).unwrap();

        let backups_dir = home.join(".poverello_backups").join("global-rules");
        assert!(backups_dir.is_dir());
        let entries: Vec<_> = std::fs::read_dir(&backups_dir).unwrap().flatten().collect();
        assert!(!entries.is_empty());
        let backup_content = std::fs::read_to_string(&entries[0].path()).unwrap();
        assert_eq!(backup_content, "# Original user text\n");
    }
}
