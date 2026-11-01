use std::path::Path;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use crate::codex_global_rules::{check_ancestors_symlink_or_reparse, resolve_codex_home};

pub const REVIEW_SETTINGS_FILE_NAME: &str = "pov-review-settings.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewersSelection {
    pub codex: bool,
    pub gemini: bool,
    pub deepseek: bool,
}

impl Default for ReviewersSelection {
    fn default() -> Self {
        Self {
            codex: true,
            gemini: false,
            deepseek: false,
        }
    }
}

impl ReviewersSelection {
    pub fn count_selected(&self) -> usize {
        (if self.codex { 1 } else { 0 })
            + (if self.gemini { 1 } else { 0 })
            + (if self.deepseek { 1 } else { 0 })
    }

    pub fn is_selected(&self, id: &str) -> bool {
        match id {
            "codex" => self.codex,
            "gemini" => self.gemini,
            "deepseek" => self.deepseek,
            _ => false,
        }
    }

    pub fn single_selected_id(&self) -> Option<&'static str> {
        if self.count_selected() == 1 {
            if self.codex {
                Some("codex")
            } else if self.gemini {
                Some("gemini")
            } else if self.deepseek {
                Some("deepseek")
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexReviewSettings {
    pub schema_version: u32,
    pub reviewers: ReviewersSelection,
    pub priority_reviewer: Option<String>,
    pub gemini_cli_enabled: bool,
}

impl Default for CodexReviewSettings {
    fn default() -> Self {
        Self {
            schema_version: 2,
            reviewers: ReviewersSelection::default(),
            priority_reviewer: Some("codex".to_string()),
            gemini_cli_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct CodexReviewSettingsV1 {
    schema_version: u32,
    reviewers: ReviewersSelection,
    priority_reviewer: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SchemaVersionProbe {
    schema_version: u32,
}

fn compute_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(windows)]
fn check_symlink_or_reparse(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return true;
        }
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;
        if (meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
            return true;
        }
    }
    false
}

#[cfg(not(windows))]
fn check_symlink_or_reparse(path: &Path) -> bool {
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return true;
        }
    }
    false
}

pub fn validate_codex_review_settings(settings: &mut CodexReviewSettings) -> Result<(), String> {
    if settings.schema_version != 2 {
        return Err(format!(
            "Unsupported schema_version {} (expected 2)",
            settings.schema_version
        ));
    }

    if !settings.gemini_cli_enabled && settings.reviewers.gemini {
        return Err("Gemini CLI reviewer cannot be enabled when gemini_cli_enabled is false".to_string());
    }

    let count = settings.reviewers.count_selected();
    if count == 0 {
        settings.priority_reviewer = None;
    } else if count == 1 {
        let single = settings.reviewers.single_selected_id().unwrap();
        settings.priority_reviewer = Some(single.to_string());
    } else {
        // >= 2
        match &settings.priority_reviewer {
            Some(id) if !id.trim().is_empty() => {
                let id_trimmed = id.trim();
                if !settings.reviewers.is_selected(id_trimmed) {
                    return Err(format!(
                        "Priority reviewer '{id_trimmed}' is not among selected active reviewers"
                    ));
                }
                settings.priority_reviewer = Some(id_trimmed.to_string());
            }
            _ => {
                return Err("When multiple reviewers are selected, a priority reviewer must be designated".to_string());
            }
        }
    }

    Ok(())
}

pub fn get_codex_review_settings() -> Result<CodexReviewSettings, String> {
    let codex_home = resolve_codex_home()?;
    get_codex_review_settings_at(&codex_home)
}

pub fn get_codex_review_settings_at(codex_home: &Path) -> Result<CodexReviewSettings, String> {
    if check_ancestors_symlink_or_reparse(codex_home) {
        return Err("Codex home or an ancestor directory is a symlink or reparse point.".to_string());
    }

    let settings_path = codex_home.join(REVIEW_SETTINGS_FILE_NAME);
    if !settings_path.exists() {
        return Ok(CodexReviewSettings::default());
    }

    if check_symlink_or_reparse(&settings_path) {
        return Err(format!(
            "Review settings file is a symlink or reparse point: {}",
            settings_path.display()
        ));
    }

    let raw = std::fs::read_to_string(&settings_path)
        .map_err(|e| format!("Failed to read {}: {e}", settings_path.display()))?;

    let probe: SchemaVersionProbe = serde_json::from_str(&raw)
        .map_err(|e| format!("Corrupted or invalid JSON in {}: {e}", settings_path.display()))?;

    let mut settings = match probe.schema_version {
        1 => {
            let v1: CodexReviewSettingsV1 = serde_json::from_str(&raw)
                .map_err(|e| format!("Invalid schema_version 1 JSON in {}: {e}", settings_path.display()))?;
            let gemini_cli_enabled = v1.reviewers.gemini;
            CodexReviewSettings {
                schema_version: 2,
                reviewers: v1.reviewers,
                priority_reviewer: v1.priority_reviewer,
                gemini_cli_enabled,
            }
        }
        2 => {
            serde_json::from_str::<CodexReviewSettings>(&raw)
                .map_err(|e| format!("Invalid schema_version 2 JSON in {}: {e}", settings_path.display()))?
        }
        other => {
            return Err(format!(
                "Unsupported schema_version {} in {} (expected 1 or 2)",
                other,
                settings_path.display()
            ));
        }
    };

    validate_codex_review_settings(&mut settings)?;

    Ok(settings)
}

pub fn save_codex_review_settings(mut settings: CodexReviewSettings) -> Result<CodexReviewSettings, String> {
    let codex_home = resolve_codex_home()?;
    save_codex_review_settings_at(&codex_home, &mut settings)?;
    Ok(settings)
}

pub fn save_codex_review_settings_at(
    codex_home: &Path,
    settings: &mut CodexReviewSettings,
) -> Result<(), String> {
    validate_codex_review_settings(settings)?;

    if check_ancestors_symlink_or_reparse(codex_home) {
        return Err("Cannot save review settings: Codex home or an ancestor directory is a symlink or reparse point.".to_string());
    }

    std::fs::create_dir_all(codex_home)
        .map_err(|e| format!("Failed to create codex home directory: {e}"))?;

    let settings_path = codex_home.join(REVIEW_SETTINGS_FILE_NAME);

    // Pre-save backup if existing
    if settings_path.exists() {
        if check_symlink_or_reparse(&settings_path) {
            return Err(format!(
                "Cannot overwrite: review settings file is a symlink or reparse point: {}",
                settings_path.display()
            ));
        }
        let backups_dir = codex_home.join(".poverello_backups").join("review-settings");
        std::fs::create_dir_all(&backups_dir)
            .map_err(|e| format!("Failed to create backup dir: {e}"))?;
        let backup_file = backups_dir.join(format!(
            "{}.backup-{}",
            REVIEW_SETTINGS_FILE_NAME,
            uuid::Uuid::new_v4()
        ));
        let _ = std::fs::copy(&settings_path, &backup_file);
    }

    let serialized = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize review settings: {e}"))?
        + "\n";

    // Staged write with safe replace and read-back
    let staged = codex_home.join(format!("{}.staged-{}", REVIEW_SETTINGS_FILE_NAME, uuid::Uuid::new_v4()));
    std::fs::write(&staged, &serialized)
        .map_err(|e| format!("Failed to write staged settings file: {e}"))?;

    let staged_bytes = std::fs::read(&staged)
        .map_err(|e| format!("Failed to read staged settings file: {e}"))?;
    if compute_sha256(&staged_bytes) != compute_sha256(serialized.as_bytes()) {
        let _ = std::fs::remove_file(&staged);
        return Err("Staged settings write integrity check failed".to_string());
    }

    #[cfg(windows)]
    {
        if settings_path.exists() {
            let old_temp = codex_home.join(format!("{}.old-{}", REVIEW_SETTINGS_FILE_NAME, uuid::Uuid::new_v4()));
            std::fs::rename(&settings_path, &old_temp)
                .map_err(|e| format!("Failed to move existing settings file: {e}"))?;
            if let Err(e) = std::fs::rename(&staged, &settings_path) {
                let _ = std::fs::rename(&old_temp, &settings_path);
                return Err(format!("Failed to place new settings file (restored original): {e}"));
            }
            let _ = std::fs::remove_file(&old_temp);
        } else {
            std::fs::rename(&staged, &settings_path)
                .map_err(|e| format!("Failed to rename staged settings file: {e}"))?;
        }
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(&staged, &settings_path)
            .map_err(|e| format!("Failed to place new settings file: {e}"))?;
    }

    // Read-back verification
    let read_back = std::fs::read_to_string(&settings_path)
        .map_err(|e| format!("Read-back verification failed: {e}"))?;
    if read_back != serialized {
        return Err("Read-back verification failed after saving review settings".to_string());
    }

    let parsed: CodexReviewSettings = serde_json::from_str(&read_back)
        .map_err(|e| format!("Read-back parsed JSON invalid: {e}"))?;
    if parsed != *settings {
        return Err("Read-back settings data does not match in-memory settings".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_review_settings() {
        let def = CodexReviewSettings::default();
        assert_eq!(def.schema_version, 2);
        assert!(def.reviewers.codex);
        assert!(!def.reviewers.gemini);
        assert!(!def.reviewers.deepseek);
        assert_eq!(def.priority_reviewer.as_deref(), Some("codex"));
        assert!(!def.gemini_cli_enabled);
    }

    #[test]
    fn test_validation_rules() {
        // 0 reviewers -> priority is None
        let mut s0 = CodexReviewSettings {
            schema_version: 2,
            reviewers: ReviewersSelection {
                codex: false,
                gemini: false,
                deepseek: false,
            },
            priority_reviewer: Some("codex".to_string()),
            gemini_cli_enabled: false,
        };
        assert!(validate_codex_review_settings(&mut s0).is_ok());
        assert_eq!(s0.priority_reviewer, None);

        // 1 reviewer -> priority automatically assigned to that reviewer
        let mut s1 = CodexReviewSettings {
            schema_version: 2,
            reviewers: ReviewersSelection {
                codex: false,
                gemini: true,
                deepseek: false,
            },
            priority_reviewer: None,
            gemini_cli_enabled: true,
        };
        assert!(validate_codex_review_settings(&mut s1).is_ok());
        assert_eq!(s1.priority_reviewer.as_deref(), Some("gemini"));

        // 2 reviewers with valid priority
        let mut s2 = CodexReviewSettings {
            schema_version: 2,
            reviewers: ReviewersSelection {
                codex: true,
                gemini: true,
                deepseek: false,
            },
            priority_reviewer: Some("gemini".to_string()),
            gemini_cli_enabled: true,
        };
        assert!(validate_codex_review_settings(&mut s2).is_ok());
        assert_eq!(s2.priority_reviewer.as_deref(), Some("gemini"));

        // 2 reviewers with missing priority -> error
        let mut s2_err = CodexReviewSettings {
            schema_version: 2,
            reviewers: ReviewersSelection {
                codex: true,
                gemini: true,
                deepseek: false,
            },
            priority_reviewer: None,
            gemini_cli_enabled: true,
        };
        assert!(validate_codex_review_settings(&mut s2_err).is_err());

        // 2 reviewers with unselected priority -> error
        let mut s2_unsel = CodexReviewSettings {
            schema_version: 2,
            reviewers: ReviewersSelection {
                codex: true,
                gemini: false,
                deepseek: true,
            },
            priority_reviewer: Some("gemini".to_string()),
            gemini_cli_enabled: false,
        };
        assert!(validate_codex_review_settings(&mut s2_unsel).is_err());

        // gemini_cli_enabled == false but reviewers.gemini == true -> error
        let mut s_dis_gem = CodexReviewSettings {
            schema_version: 2,
            reviewers: ReviewersSelection {
                codex: true,
                gemini: true,
                deepseek: false,
            },
            priority_reviewer: Some("codex".to_string()),
            gemini_cli_enabled: false,
        };
        assert!(validate_codex_review_settings(&mut s_dis_gem).is_err());

        // Invalid schema_version -> error
        let mut s_bad_ver = CodexReviewSettings {
            schema_version: 1,
            reviewers: ReviewersSelection::default(),
            priority_reviewer: Some("codex".to_string()),
            gemini_cli_enabled: false,
        };
        assert!(validate_codex_review_settings(&mut s_bad_ver).is_err());
    }

    #[test]
    fn test_v1_migration_gemini_true() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        let settings_path = home.join(REVIEW_SETTINGS_FILE_NAME);

        let v1_json = r#"{
  "schema_version": 1,
  "reviewers": {
    "codex": true,
    "gemini": true,
    "deepseek": false
  },
  "priority_reviewer": "gemini"
}"#;
        std::fs::write(&settings_path, v1_json).unwrap();

        let loaded = get_codex_review_settings_at(home).unwrap();
        assert_eq!(loaded.schema_version, 2);
        assert!(loaded.gemini_cli_enabled);
        assert!(loaded.reviewers.codex);
        assert!(loaded.reviewers.gemini);
        assert!(!loaded.reviewers.deepseek);
        assert_eq!(loaded.priority_reviewer.as_deref(), Some("gemini"));

        // Verify disk file was not modified by reading
        let raw_on_disk = std::fs::read_to_string(&settings_path).unwrap();
        assert_eq!(raw_on_disk, v1_json);
    }

    #[test]
    fn test_v1_migration_gemini_false() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        let settings_path = home.join(REVIEW_SETTINGS_FILE_NAME);

        let v1_json = r#"{
  "schema_version": 1,
  "reviewers": {
    "codex": true,
    "gemini": false,
    "deepseek": true
  },
  "priority_reviewer": "deepseek"
}"#;
        std::fs::write(&settings_path, v1_json).unwrap();

        let loaded = get_codex_review_settings_at(home).unwrap();
        assert_eq!(loaded.schema_version, 2);
        assert!(!loaded.gemini_cli_enabled);
        assert!(loaded.reviewers.codex);
        assert!(!loaded.reviewers.gemini);
        assert!(loaded.reviewers.deepseek);
        assert_eq!(loaded.priority_reviewer.as_deref(), Some("deepseek"));
    }

    #[test]
    fn test_save_and_read_roundtrip() {
        let dir = tempdir().unwrap();
        let home = dir.path();

        // Initially does not exist -> returns default
        let loaded_default = get_codex_review_settings_at(home).unwrap();
        assert_eq!(loaded_default, CodexReviewSettings::default());

        // Save multiple reviewers with priority
        let mut to_save = CodexReviewSettings {
            schema_version: 2,
            reviewers: ReviewersSelection {
                codex: true,
                gemini: true,
                deepseek: true,
            },
            priority_reviewer: Some("deepseek".to_string()),
            gemini_cli_enabled: true,
        };
        save_codex_review_settings_at(home, &mut to_save).unwrap();

        // Read back
        let read = get_codex_review_settings_at(home).unwrap();
        assert_eq!(read, to_save);

        // Verify backup was created on second save
        let mut to_save_2 = CodexReviewSettings {
            schema_version: 2,
            reviewers: ReviewersSelection {
                codex: true,
                gemini: false,
                deepseek: false,
            },
            priority_reviewer: Some("codex".to_string()),
            gemini_cli_enabled: false,
        };
        save_codex_review_settings_at(home, &mut to_save_2).unwrap();

        let backup_dir = home.join(".poverello_backups").join("review-settings");
        assert!(backup_dir.exists());
        let entries: Vec<_> = std::fs::read_dir(&backup_dir).unwrap().collect();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_corrupted_json_fails_closed() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        let settings_path = home.join(REVIEW_SETTINGS_FILE_NAME);
        std::fs::write(&settings_path, "{ invalid json ").unwrap();

        let res = get_codex_review_settings_at(home);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Corrupted or invalid JSON"));
    }

    #[test]
    fn test_unsupported_schema_fails_closed() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        let settings_path = home.join(REVIEW_SETTINGS_FILE_NAME);
        std::fs::write(&settings_path, r#"{"schema_version": 99}"#).unwrap();

        let res = get_codex_review_settings_at(home);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Unsupported schema_version 99"));
    }
}
