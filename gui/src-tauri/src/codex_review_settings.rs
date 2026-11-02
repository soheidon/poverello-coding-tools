use std::path::Path;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use crate::codex_global_rules::{check_ancestors_symlink_or_reparse, resolve_codex_home};

pub const REVIEW_SETTINGS_FILE_NAME: &str = "pov-review-settings.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewMode {
    CodexSelfOnly,
    ExternalOnly,
    CodexPlusExternal,
    None,
}

impl Default for ReviewMode {
    fn default() -> Self {
        Self::CodexSelfOnly
    }
}

impl ReviewMode {
    pub fn allows_codex_self_review(&self) -> bool {
        matches!(self, Self::CodexSelfOnly | Self::CodexPlusExternal)
    }

    pub fn allows_external_review(&self) -> bool {
        matches!(self, Self::ExternalOnly | Self::CodexPlusExternal)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CodexSelfOnly => "codex_self_only",
            Self::ExternalOnly => "external_only",
            Self::CodexPlusExternal => "codex_plus_external",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalReviewerConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewersSelection {
    #[serde(default)]
    pub codex: bool,
    pub gemini: bool,
    pub deepseek: bool,
}

impl Default for ReviewersSelection {
    fn default() -> Self {
        Self {
            codex: false,
            gemini: false,
            deepseek: false,
        }
    }
}

impl ReviewersSelection {
    /// In v0.26.2, counts only external reviewers (gemini, deepseek).
    /// Codex is the work agent, not an external reviewer.
    pub fn count_selected(&self) -> usize {
        (if self.gemini { 1 } else { 0 }) + (if self.deepseek { 1 } else { 0 })
    }

    pub fn is_selected(&self, id: &str) -> bool {
        match id {
            "gemini" => self.gemini,
            "deepseek" => self.deepseek,
            _ => false,
        }
    }

    pub fn single_selected_id(&self) -> Option<&'static str> {
        if self.count_selected() == 1 {
            if self.gemini {
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
    #[serde(default)]
    pub review_mode: ReviewMode,
    pub reviewers: ReviewersSelection,
    pub priority_reviewer: Option<String>,
    pub gemini_cli_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deepseek: Option<ExternalReviewerConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gemini: Option<ExternalReviewerConfig>,
    #[serde(default)]
    pub unconfirmed_migration: bool,
}

impl Default for CodexReviewSettings {
    fn default() -> Self {
        Self {
            schema_version: 4,
            review_mode: ReviewMode::CodexSelfOnly,
            reviewers: ReviewersSelection::default(),
            priority_reviewer: Some("codex".to_string()),
            gemini_cli_enabled: false,
            deepseek: Some(ExternalReviewerConfig {
                model: Some("deepseek-flash".to_string()),
                reasoning_effort: Some("high".to_string()),
            }),
            gemini: Some(ExternalReviewerConfig {
                model: None,
                reasoning_effort: None,
            }),
            unconfirmed_migration: false,
        }
    }
}

impl CodexReviewSettings {
    pub fn count_external_reviewers(&self) -> usize {
        (if self.gemini_cli_enabled && self.reviewers.gemini { 1 } else { 0 })
            + (if self.reviewers.deepseek { 1 } else { 0 })
    }

    pub fn is_reviewer_active(&self, id: &str) -> bool {
        match id {
            "gemini" => self.gemini_cli_enabled && self.reviewers.gemini,
            "deepseek" => self.reviewers.deepseek,
            _ => false,
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
struct CodexReviewSettingsV2 {
    schema_version: u32,
    reviewers: ReviewersSelection,
    priority_reviewer: Option<String>,
    gemini_cli_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct CodexReviewSettingsV3 {
    schema_version: u32,
    reviewers: ReviewersSelection,
    priority_reviewer: Option<String>,
    gemini_cli_enabled: bool,
    deepseek: Option<ExternalReviewerConfig>,
    gemini: Option<ExternalReviewerConfig>,
    #[serde(default)]
    unconfirmed_migration: bool,
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

const ALLOWED_GEMINI_MODELS: &[&str] = &["gemini-2.5-pro", "gemini-2.5-flash"];
const ALLOWED_DEEPSEEK_MODELS: &[&str] = &["deepseek-flash", "deepseek-v4-pro"];
const ALLOWED_DEEPSEEK_EFFORTS: &[&str] = &["low", "high", "max"];

pub fn validate_codex_review_settings(settings: &mut CodexReviewSettings) -> Result<(), String> {
    if settings.schema_version != 4 {
        return Err(format!(
            "Unsupported schema_version {} (expected 4)",
            settings.schema_version
        ));
    }

    // Codex is the working agent, not an external reviewer in reviewers list.
    settings.reviewers.codex = false;

    if !settings.gemini_cli_enabled && settings.reviewers.gemini {
        return Err("Gemini CLI reviewer cannot be enabled when gemini_cli_enabled is false".to_string());
    }

    // Validation of Gemini model and options
    if let Some(gem) = &settings.gemini {
        if gem.reasoning_effort.is_some() {
            return Err("Reasoning effort is not supported for Gemini CLI".to_string());
        }
        if let Some(m) = &gem.model {
            let m_trimmed = m.trim();
            if !ALLOWED_GEMINI_MODELS.contains(&m_trimmed) {
                return Err(format!(
                    "Unsupported or deprecated Gemini model '{m_trimmed}' (allowed: {})",
                    ALLOWED_GEMINI_MODELS.join(", ")
                ));
            }
        }
    }

    // Validation of DeepSeek model and options
    if let Some(ds) = &settings.deepseek {
        if let Some(m) = &ds.model {
            let m_trimmed = m.trim();
            if !ALLOWED_DEEPSEEK_MODELS.contains(&m_trimmed) {
                return Err(format!(
                    "Unsupported or deprecated DeepSeek model '{m_trimmed}' (allowed: {})",
                    ALLOWED_DEEPSEEK_MODELS.join(", ")
                ));
            }
        }
        if let Some(e) = &ds.reasoning_effort {
            let e_trimmed = e.trim();
            if !ALLOWED_DEEPSEEK_EFFORTS.contains(&e_trimmed) {
                return Err(format!(
                    "Unsupported DeepSeek reasoning effort '{e_trimmed}' (allowed: {})",
                    ALLOWED_DEEPSEEK_EFFORTS.join(", ")
                ));
            }
        }
    }

    let count = (if settings.gemini_cli_enabled && settings.reviewers.gemini { 1 } else { 0 })
        + (if settings.reviewers.deepseek { 1 } else { 0 });

    match settings.review_mode {
        ReviewMode::CodexSelfOnly => {
            // Codex self-review is the sole reviewer
            settings.priority_reviewer = Some("codex".to_string());
        }
        ReviewMode::None => {
            // No review; priority reviewer disabled
            settings.priority_reviewer = None;
        }
        ReviewMode::ExternalOnly => {
            if count == 0 {
                return Err("When external review is selected, at least one external reviewer must be configured".to_string());
            }
            if let Some(priority) = &settings.priority_reviewer {
                if priority == "codex" {
                    return Err("Codex cannot be designated as a priority reviewer in external_only mode".to_string());
                }
            }
            if count == 1 {
                let single = if settings.gemini_cli_enabled && settings.reviewers.gemini {
                    "gemini"
                } else {
                    "deepseek"
                };
                settings.priority_reviewer = Some(single.to_string());
            } else {
                match &settings.priority_reviewer {
                    Some(id) if !id.trim().is_empty() => {
                        let id_trimmed = id.trim();
                        let is_valid = match id_trimmed {
                            "gemini" => settings.gemini_cli_enabled && settings.reviewers.gemini,
                            "deepseek" => settings.reviewers.deepseek,
                            _ => false,
                        };
                        if !is_valid {
                            return Err(format!(
                                "Priority reviewer '{id_trimmed}' is not among selected active external reviewers"
                            ));
                        }
                        settings.priority_reviewer = Some(id_trimmed.to_string());
                    }
                    _ => {
                        return Err("When multiple reviewers are selected, a priority reviewer must be designated".to_string());
                    }
                }
            }
        }
        ReviewMode::CodexPlusExternal => {
            if count == 0 {
                return Err("When external review is selected, at least one external reviewer must be configured".to_string());
            }
            // Candidate priority reviewers: "codex" OR any active external reviewer
            match &settings.priority_reviewer {
                Some(id) if !id.trim().is_empty() => {
                    let id_trimmed = id.trim();
                    let is_valid = match id_trimmed {
                        "codex" => true,
                        "gemini" => settings.gemini_cli_enabled && settings.reviewers.gemini,
                        "deepseek" => settings.reviewers.deepseek,
                        _ => false,
                    };
                    if !is_valid {
                        return Err(format!(
                            "Priority reviewer '{id_trimmed}' is not valid for codex_plus_external mode"
                        ));
                    }
                    settings.priority_reviewer = Some(id_trimmed.to_string());
                }
                _ => {
                    // Default to codex self-review if not specified
                    settings.priority_reviewer = Some("codex".to_string());
                }
            }
        }
    }

    Ok(())
}

pub fn can_launch_external_reviews(settings: &CodexReviewSettings) -> Result<(), String> {
    if settings.unconfirmed_migration {
        return Err("External review cannot be launched: review configuration was migrated from an older version and has unconfirmed changes. Please review and save in GUI before dispatching.".to_string());
    }
    if settings.schema_version != 4 {
        return Err(format!("Unsupported review settings schema version {}", settings.schema_version));
    }
    if !settings.review_mode.allows_external_review() {
        return Err(format!(
            "External review cannot be launched: review_mode is '{}'",
            settings.review_mode.as_str()
        ));
    }
    let count = (if settings.gemini_cli_enabled && settings.reviewers.gemini { 1 } else { 0 })
        + (if settings.reviewers.deepseek { 1 } else { 0 });
    if count == 0 {
        return Err("External review cannot be launched: no external reviewers are selected.".to_string());
    }
    if settings.review_mode == ReviewMode::ExternalOnly && count >= 2 && settings.priority_reviewer.is_none() {
        return Err("External review cannot be launched: multiple reviewers are selected but no priority reviewer is designated.".to_string());
    }
    if settings.review_mode == ReviewMode::CodexPlusExternal {
        match settings.priority_reviewer.as_deref() {
            Some("codex") => {}
            Some(p) if settings.is_reviewer_active(p) => {}
            _ => {
                return Err("External review cannot be launched: invalid or missing priority reviewer for codex_plus_external mode.".to_string());
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
            let mut reviewers = v1.reviewers;
            let had_codex = reviewers.codex;
            reviewers.codex = false;

            let count = (if gemini_cli_enabled && reviewers.gemini { 1 } else { 0 })
                + (if reviewers.deepseek { 1 } else { 0 });
            let had_external = count > 0;

            // Determine review_mode safely
            let (review_mode, unconfirmed_migration) = if !had_codex && !had_external {
                // When all reviewers were disabled, default to codex_self_only with unconfirmed_migration = true
                (ReviewMode::CodexSelfOnly, true)
            } else if had_codex && had_external {
                (ReviewMode::CodexPlusExternal, false)
            } else if had_codex && !had_external {
                (ReviewMode::CodexSelfOnly, false)
            } else {
                (ReviewMode::ExternalOnly, false)
            };

            let priority = match review_mode {
                ReviewMode::CodexSelfOnly => Some("codex".to_string()),
                ReviewMode::None => None,
                ReviewMode::CodexPlusExternal => {
                    match v1.priority_reviewer.as_deref() {
                        Some("gemini") if gemini_cli_enabled && reviewers.gemini => Some("gemini".to_string()),
                        Some("deepseek") if reviewers.deepseek => Some("deepseek".to_string()),
                        _ => Some("codex".to_string()),
                    }
                }
                ReviewMode::ExternalOnly => {
                    match v1.priority_reviewer.as_deref() {
                        Some("gemini") if gemini_cli_enabled && reviewers.gemini => Some("gemini".to_string()),
                        Some("deepseek") if reviewers.deepseek => Some("deepseek".to_string()),
                        _ => if count == 1 {
                            if reviewers.deepseek { Some("deepseek".to_string()) } else { Some("gemini".to_string()) }
                        } else {
                            None
                        }
                    }
                }
            };

            CodexReviewSettings {
                schema_version: 4,
                review_mode,
                reviewers,
                priority_reviewer: priority,
                gemini_cli_enabled,
                deepseek: Some(ExternalReviewerConfig {
                    model: Some("deepseek-flash".to_string()),
                    reasoning_effort: None,
                }),
                gemini: Some(ExternalReviewerConfig {
                    model: None,
                    reasoning_effort: None,
                }),
                unconfirmed_migration,
            }
        }
        2 => {
            let v2: CodexReviewSettingsV2 = serde_json::from_str(&raw)
                .map_err(|e| format!("Invalid schema_version 2 JSON in {}: {e}", settings_path.display()))?;
            let mut reviewers = v2.reviewers;
            let had_codex = reviewers.codex;
            reviewers.codex = false;

            let count = (if v2.gemini_cli_enabled && reviewers.gemini { 1 } else { 0 })
                + (if reviewers.deepseek { 1 } else { 0 });
            let had_external = count > 0;

            let (review_mode, unconfirmed_migration) = if !had_codex && !had_external {
                (ReviewMode::CodexSelfOnly, true)
            } else if had_codex && had_external {
                (ReviewMode::CodexPlusExternal, false)
            } else if had_codex && !had_external {
                (ReviewMode::CodexSelfOnly, false)
            } else {
                (ReviewMode::ExternalOnly, false)
            };

            let priority = match review_mode {
                ReviewMode::CodexSelfOnly => Some("codex".to_string()),
                ReviewMode::None => None,
                ReviewMode::CodexPlusExternal => {
                    match v2.priority_reviewer.as_deref() {
                        Some("gemini") if v2.gemini_cli_enabled && reviewers.gemini => Some("gemini".to_string()),
                        Some("deepseek") if reviewers.deepseek => Some("deepseek".to_string()),
                        _ => Some("codex".to_string()),
                    }
                }
                ReviewMode::ExternalOnly => {
                    match v2.priority_reviewer.as_deref() {
                        Some("gemini") if v2.gemini_cli_enabled && reviewers.gemini => Some("gemini".to_string()),
                        Some("deepseek") if reviewers.deepseek => Some("deepseek".to_string()),
                        _ => if count == 1 {
                            if reviewers.deepseek { Some("deepseek".to_string()) } else { Some("gemini".to_string()) }
                        } else {
                            None
                        }
                    }
                }
            };

            CodexReviewSettings {
                schema_version: 4,
                review_mode,
                reviewers,
                priority_reviewer: priority,
                gemini_cli_enabled: v2.gemini_cli_enabled,
                deepseek: Some(ExternalReviewerConfig {
                    model: Some("deepseek-flash".to_string()),
                    reasoning_effort: None,
                }),
                gemini: Some(ExternalReviewerConfig {
                    model: None,
                    reasoning_effort: None,
                }),
                unconfirmed_migration,
            }
        }
        3 => {
            let v3: CodexReviewSettingsV3 = serde_json::from_str(&raw)
                .map_err(|e| format!("Invalid schema_version 3 JSON in {}: {e}", settings_path.display()))?;
            let mut reviewers = v3.reviewers;
            reviewers.codex = false;

            let count = (if v3.gemini_cli_enabled && reviewers.gemini { 1 } else { 0 })
                + (if reviewers.deepseek { 1 } else { 0 });
            let had_external = count > 0;

            let (review_mode, unconfirmed_migration) = if !had_external {
                (ReviewMode::CodexSelfOnly, v3.unconfirmed_migration)
            } else {
                (ReviewMode::CodexPlusExternal, v3.unconfirmed_migration)
            };

            let priority = match review_mode {
                ReviewMode::CodexSelfOnly => Some("codex".to_string()),
                ReviewMode::None => None,
                ReviewMode::CodexPlusExternal => {
                    match v3.priority_reviewer.as_deref() {
                        Some("gemini") if v3.gemini_cli_enabled && reviewers.gemini => Some("gemini".to_string()),
                        Some("deepseek") if reviewers.deepseek => Some("deepseek".to_string()),
                        _ => Some("codex".to_string()),
                    }
                }
                ReviewMode::ExternalOnly => {
                    match v3.priority_reviewer.as_deref() {
                        Some("gemini") if v3.gemini_cli_enabled && reviewers.gemini => Some("gemini".to_string()),
                        Some("deepseek") if reviewers.deepseek => Some("deepseek".to_string()),
                        _ => if count == 1 {
                            if reviewers.deepseek { Some("deepseek".to_string()) } else { Some("gemini".to_string()) }
                        } else {
                            None
                        }
                    }
                }
            };

            CodexReviewSettings {
                schema_version: 4,
                review_mode,
                reviewers,
                priority_reviewer: priority,
                gemini_cli_enabled: v3.gemini_cli_enabled,
                deepseek: v3.deepseek.or_else(|| {
                    Some(ExternalReviewerConfig {
                        model: Some("deepseek-flash".to_string()),
                        reasoning_effort: None,
                    })
                }),
                gemini: v3.gemini.or_else(|| {
                    Some(ExternalReviewerConfig {
                        model: None,
                        reasoning_effort: None,
                    })
                }),
                unconfirmed_migration,
            }
        }
        4 => {
            serde_json::from_str::<CodexReviewSettings>(&raw)
                .map_err(|e| format!("Invalid schema_version 4 JSON in {}: {e}", settings_path.display()))?
        }
        other => {
            return Err(format!(
                "Unsupported schema_version {} in {} (expected 1, 2, 3, or 4)",
                other,
                settings_path.display()
            ));
        }
    };

    if let Some(ds) = &mut settings.deepseek {
        if ds.model.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true) {
            ds.model = Some("deepseek-flash".to_string());
        }
        if ds.reasoning_effort.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true) {
            ds.reasoning_effort = Some("high".to_string());
        }
    } else {
        settings.deepseek = Some(ExternalReviewerConfig {
            model: Some("deepseek-flash".to_string()),
            reasoning_effort: Some("high".to_string()),
        });
    }
    if settings.gemini.is_none() {
        settings.gemini = Some(ExternalReviewerConfig {
            model: None,
            reasoning_effort: None,
        });
    }

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
    settings.schema_version = 4;
    settings.reviewers.codex = false;
    settings.unconfirmed_migration = false;

    if let Some(ds) = &mut settings.deepseek {
        if ds.model.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true) {
            ds.model = Some("deepseek-flash".to_string());
        }
        if ds.reasoning_effort.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true) {
            ds.reasoning_effort = Some("high".to_string());
        }
    } else {
        settings.deepseek = Some(ExternalReviewerConfig {
            model: Some("deepseek-flash".to_string()),
            reasoning_effort: Some("high".to_string()),
        });
    }
    if settings.gemini.is_none() {
        settings.gemini = Some(ExternalReviewerConfig {
            model: None,
            reasoning_effort: None,
        });
    }

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
        .map_err(|e| format!("Failed to read-back staged settings file: {e}"))?;

    let staged_hash = compute_sha256(&staged_bytes);
    let expected_hash = compute_sha256(serialized.as_bytes());

    if staged_hash != expected_hash {
        let _ = std::fs::remove_file(&staged);
        return Err("Read-back verification failed for staged settings write.".to_string());
    }

    // Atomic replace
    #[cfg(windows)]
    {
        if settings_path.exists() {
            let backup_before_replace = codex_home.join(format!("{}.old-{}", REVIEW_SETTINGS_FILE_NAME, uuid::Uuid::new_v4()));
            std::fs::rename(&settings_path, &backup_before_replace)
                .map_err(|e| format!("Failed to move old settings file before replace: {e}"))?;
            if let Err(e) = std::fs::rename(&staged, &settings_path) {
                let _ = std::fs::rename(&backup_before_replace, &settings_path);
                return Err(format!("Failed to rename staged file to target: {e}"));
            }
            let _ = std::fs::remove_file(&backup_before_replace);
        } else {
            std::fs::rename(&staged, &settings_path)
                .map_err(|e| format!("Failed to rename staged file to target: {e}"))?;
        }
    }

    #[cfg(not(windows))]
    {
        std::fs::rename(&staged, &settings_path)
            .map_err(|e| format!("Failed to replace settings file: {e}"))?;
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
        assert_eq!(def.schema_version, 4);
        assert_eq!(def.review_mode, ReviewMode::CodexSelfOnly);
        assert!(!def.reviewers.codex);
        assert!(!def.reviewers.gemini);
        assert!(!def.reviewers.deepseek);
        assert_eq!(def.priority_reviewer.as_deref(), Some("codex"));
        assert!(!def.gemini_cli_enabled);
        assert!(!def.unconfirmed_migration);
        assert_eq!(def.deepseek.as_ref().unwrap().model.as_deref(), Some("deepseek-flash"));
        assert_eq!(def.gemini.as_ref().unwrap().model, None);
    }

    #[test]
    fn test_validation_rules() {
        // 0 reviewers with codex_self_only -> priority is automatically Some("codex"), codex forced to false
        let mut s0 = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::CodexSelfOnly,
            reviewers: ReviewersSelection {
                codex: true,
                gemini: false,
                deepseek: false,
            },
            priority_reviewer: None,
            gemini_cli_enabled: false,
            deepseek: None,
            gemini: None,
            unconfirmed_migration: false,
        };
        assert!(validate_codex_review_settings(&mut s0).is_ok());
        assert!(!s0.reviewers.codex);
        assert_eq!(s0.priority_reviewer.as_deref(), Some("codex"));

        // Codex as priority reviewer in external_only is rejected
        let mut s_codex_pri_ext = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::ExternalOnly,
            reviewers: ReviewersSelection {
                codex: false,
                gemini: false,
                deepseek: true,
            },
            priority_reviewer: Some("codex".to_string()),
            gemini_cli_enabled: false,
            deepseek: None,
            gemini: None,
            unconfirmed_migration: false,
        };
        assert!(validate_codex_review_settings(&mut s_codex_pri_ext).is_err());

        // Codex as priority reviewer in codex_plus_external is valid
        let mut s_codex_pri_plus = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::CodexPlusExternal,
            reviewers: ReviewersSelection {
                codex: false,
                gemini: false,
                deepseek: true,
            },
            priority_reviewer: Some("codex".to_string()),
            gemini_cli_enabled: false,
            deepseek: None,
            gemini: None,
            unconfirmed_migration: false,
        };
        assert!(validate_codex_review_settings(&mut s_codex_pri_plus).is_ok());
        assert_eq!(s_codex_pri_plus.priority_reviewer.as_deref(), Some("codex"));

        // external_only mode with 0 external reviewers is rejected
        let mut s_ext_zero = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::ExternalOnly,
            reviewers: ReviewersSelection::default(),
            priority_reviewer: None,
            gemini_cli_enabled: false,
            deepseek: None,
            gemini: None,
            unconfirmed_migration: false,
        };
        assert!(validate_codex_review_settings(&mut s_ext_zero).is_err());

        // 1 reviewer (deepseek) with external_only -> priority automatically assigned to deepseek
        let mut s1 = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::ExternalOnly,
            reviewers: ReviewersSelection {
                codex: false,
                gemini: false,
                deepseek: true,
            },
            priority_reviewer: None,
            gemini_cli_enabled: false,
            deepseek: None,
            gemini: None,
            unconfirmed_migration: false,
        };
        assert!(validate_codex_review_settings(&mut s1).is_ok());
        assert_eq!(s1.priority_reviewer.as_deref(), Some("deepseek"));

        // 2 reviewers with valid priority
        let mut s2 = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::CodexPlusExternal,
            reviewers: ReviewersSelection {
                codex: false,
                gemini: true,
                deepseek: true,
            },
            priority_reviewer: Some("gemini".to_string()),
            gemini_cli_enabled: true,
            deepseek: None,
            gemini: None,
            unconfirmed_migration: false,
        };
        assert!(validate_codex_review_settings(&mut s2).is_ok());
        assert_eq!(s2.priority_reviewer.as_deref(), Some("gemini"));

        // Deprecated Gemini model (gemini-2.0-flash) is rejected
        let mut s_dep_gem = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::ExternalOnly,
            reviewers: ReviewersSelection {
                codex: false,
                gemini: true,
                deepseek: false,
            },
            priority_reviewer: Some("gemini".to_string()),
            gemini_cli_enabled: true,
            deepseek: None,
            gemini: Some(ExternalReviewerConfig {
                model: Some("gemini-2.0-flash".to_string()),
                reasoning_effort: None,
            }),
            unconfirmed_migration: false,
        };
        assert!(validate_codex_review_settings(&mut s_dep_gem).is_err());

        // Valid Gemini model (gemini-2.5-pro) is accepted
        let mut s_ok_gem = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::ExternalOnly,
            reviewers: ReviewersSelection {
                codex: false,
                gemini: true,
                deepseek: false,
            },
            priority_reviewer: Some("gemini".to_string()),
            gemini_cli_enabled: true,
            deepseek: None,
            gemini: Some(ExternalReviewerConfig {
                model: Some("gemini-2.5-pro".to_string()),
                reasoning_effort: None,
            }),
            unconfirmed_migration: false,
        };
        assert!(validate_codex_review_settings(&mut s_ok_gem).is_ok());
    }

    #[test]
    fn test_v1_migration_gemini_true() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(REVIEW_SETTINGS_FILE_NAME);
        let raw = r#"{
            "schema_version": 1,
            "reviewers": {
                "codex": true,
                "gemini": true,
                "deepseek": false
            },
            "priority_reviewer": "codex"
        }"#;
        std::fs::write(&path, raw).unwrap();

        let s = get_codex_review_settings_at(dir.path()).unwrap();
        assert_eq!(s.schema_version, 4);
        assert_eq!(s.review_mode, ReviewMode::CodexPlusExternal);
        assert!(!s.reviewers.codex);
        assert!(s.reviewers.gemini);
        assert!(!s.reviewers.deepseek);
        // Had codex priority in v1 -> keeps Some("codex") in CodexPlusExternal
        assert_eq!(s.priority_reviewer.as_deref(), Some("codex"));
        assert!(s.gemini_cli_enabled);
        assert!(!s.unconfirmed_migration);
    }

    #[test]
    fn test_v2_migration_with_all_disabled_migrates_to_codex_self_only_unconfirmed() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(REVIEW_SETTINGS_FILE_NAME);
        let raw = r#"{
            "schema_version": 2,
            "reviewers": {
                "codex": false,
                "gemini": false,
                "deepseek": false
            },
            "priority_reviewer": null,
            "gemini_cli_enabled": false
        }"#;
        std::fs::write(&path, raw).unwrap();

        let s = get_codex_review_settings_at(dir.path()).unwrap();
        assert_eq!(s.schema_version, 4);
        // Condition 2: Must NOT migrate implicitly to none; must default to codex_self_only with unconfirmed_migration = true
        assert_eq!(s.review_mode, ReviewMode::CodexSelfOnly);
        assert!(s.unconfirmed_migration);
    }

    #[test]
    fn test_v3_migration_with_codex_plus_external() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(REVIEW_SETTINGS_FILE_NAME);
        let raw = r#"{
            "schema_version": 3,
            "reviewers": {
                "codex": false,
                "gemini": false,
                "deepseek": true
            },
            "priority_reviewer": "deepseek",
            "gemini_cli_enabled": false,
            "deepseek": { "model": "deepseek-flash" },
            "unconfirmed_migration": false
        }"#;
        std::fs::write(&path, raw).unwrap();

        let s = get_codex_review_settings_at(dir.path()).unwrap();
        assert_eq!(s.schema_version, 4);
        assert_eq!(s.review_mode, ReviewMode::CodexPlusExternal);
        assert_eq!(s.priority_reviewer.as_deref(), Some("deepseek"));
    }

    #[test]
    fn test_corrupted_json_fails_closed() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(REVIEW_SETTINGS_FILE_NAME);
        std::fs::write(&path, "{ not valid json").unwrap();
        let err = get_codex_review_settings_at(dir.path()).unwrap_err();
        assert!(err.contains("Corrupted or invalid JSON"));
    }

    #[test]
    fn test_unsupported_schema_fails_closed() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(REVIEW_SETTINGS_FILE_NAME);
        std::fs::write(&path, r#"{"schema_version": 99}"#).unwrap();
        let err = get_codex_review_settings_at(dir.path()).unwrap_err();
        assert!(err.contains("Unsupported schema_version 99"));
    }

    #[test]
    fn test_save_clears_unconfirmed_migration_and_writes_v4() {
        let dir = tempdir().unwrap();
        let mut s = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::CodexSelfOnly,
            reviewers: ReviewersSelection::default(),
            priority_reviewer: None,
            gemini_cli_enabled: false,
            deepseek: None,
            gemini: None,
            unconfirmed_migration: true,
        };
        save_codex_review_settings_at(dir.path(), &mut s).unwrap();
        assert!(!s.unconfirmed_migration);
        assert_eq!(s.schema_version, 4);

        let read_back = get_codex_review_settings_at(dir.path()).unwrap();
        assert_eq!(read_back.schema_version, 4);
        assert!(!read_back.unconfirmed_migration);
        assert_eq!(read_back.review_mode, ReviewMode::CodexSelfOnly);
    }

    #[test]
    fn test_can_launch_external_reviews_gate() {
        // codex_self_only -> cannot launch external review
        let s_self = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::CodexSelfOnly,
            reviewers: ReviewersSelection { codex: false, gemini: false, deepseek: true },
            priority_reviewer: Some("deepseek".to_string()),
            gemini_cli_enabled: false,
            deepseek: None,
            gemini: None,
            unconfirmed_migration: false,
        };
        assert!(can_launch_external_reviews(&s_self).is_err());

        // none -> cannot launch external review
        let s_none = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::None,
            reviewers: ReviewersSelection { codex: false, gemini: false, deepseek: true },
            priority_reviewer: Some("deepseek".to_string()),
            gemini_cli_enabled: false,
            deepseek: None,
            gemini: None,
            unconfirmed_migration: false,
        };
        assert!(can_launch_external_reviews(&s_none).is_err());

        // unconfirmed_migration -> cannot launch
        let s_unconf = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::ExternalOnly,
            reviewers: ReviewersSelection { codex: false, gemini: false, deepseek: true },
            priority_reviewer: Some("deepseek".to_string()),
            gemini_cli_enabled: false,
            deepseek: None,
            gemini: None,
            unconfirmed_migration: true,
        };
        assert!(can_launch_external_reviews(&s_unconf).is_err());

        // valid external_only with 1 active reviewer -> can launch
        let s_valid = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::ExternalOnly,
            reviewers: ReviewersSelection { codex: false, gemini: false, deepseek: true },
            priority_reviewer: Some("deepseek".to_string()),
            gemini_cli_enabled: false,
            deepseek: None,
            gemini: None,
            unconfirmed_migration: false,
        };
        assert!(can_launch_external_reviews(&s_valid).is_ok());

        // valid codex_plus_external with codex priority -> can launch
        let s_valid_plus_codex = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::CodexPlusExternal,
            reviewers: ReviewersSelection { codex: false, gemini: false, deepseek: true },
            priority_reviewer: Some("codex".to_string()),
            gemini_cli_enabled: false,
            deepseek: None,
            gemini: None,
            unconfirmed_migration: false,
        };
        assert!(can_launch_external_reviews(&s_valid_plus_codex).is_ok());

        // valid codex_plus_external with deepseek priority -> can launch
        let s_valid_plus_dsh = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::CodexPlusExternal,
            reviewers: ReviewersSelection { codex: false, gemini: false, deepseek: true },
            priority_reviewer: Some("deepseek".to_string()),
            gemini_cli_enabled: false,
            deepseek: None,
            gemini: None,
            unconfirmed_migration: false,
        };
        assert!(can_launch_external_reviews(&s_valid_plus_dsh).is_ok());
    }

    #[test]
    fn test_save_and_reload_codex_plus_external_priority_codex_and_deepseek() {
        let dir = tempdir().unwrap();

        // 1. Save with Codex self-review as priority
        let mut s_codex = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::CodexPlusExternal,
            reviewers: ReviewersSelection { codex: false, gemini: false, deepseek: true },
            priority_reviewer: Some("codex".to_string()),
            gemini_cli_enabled: false,
            deepseek: None,
            gemini: None,
            unconfirmed_migration: false,
        };
        save_codex_review_settings_at(dir.path(), &mut s_codex).unwrap();

        let read1 = get_codex_review_settings_at(dir.path()).unwrap();
        assert_eq!(read1.schema_version, 4);
        assert_eq!(read1.review_mode, ReviewMode::CodexPlusExternal);
        assert_eq!(read1.priority_reviewer, Some("codex".to_string()));
        assert!(!read1.unconfirmed_migration);

        // 2. Change priority to DeepSeek Harness and save
        let mut s_dsh = read1;
        s_dsh.priority_reviewer = Some("deepseek".to_string());
        save_codex_review_settings_at(dir.path(), &mut s_dsh).unwrap();

        let read2 = get_codex_review_settings_at(dir.path()).unwrap();
        assert_eq!(read2.schema_version, 4);
        assert_eq!(read2.review_mode, ReviewMode::CodexPlusExternal);
        assert_eq!(read2.priority_reviewer, Some("deepseek".to_string()));
        assert!(!read2.unconfirmed_migration);
    }

    #[test]
    fn test_deepseek_model_and_effort_validation() {
        let mut s = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::CodexPlusExternal,
            reviewers: ReviewersSelection { codex: false, gemini: false, deepseek: true },
            priority_reviewer: Some("deepseek".to_string()),
            gemini_cli_enabled: false,
            deepseek: Some(ExternalReviewerConfig {
                model: Some("deepseek-v4-pro".to_string()),
                reasoning_effort: Some("max".to_string()),
            }),
            gemini: None,
            unconfirmed_migration: false,
        };
        assert!(validate_codex_review_settings(&mut s).is_ok());

        // Invalid model
        let mut s_bad_model = s.clone();
        s_bad_model.deepseek = Some(ExternalReviewerConfig {
            model: Some("deepseek-nonexistent".to_string()),
            reasoning_effort: Some("high".to_string()),
        });
        assert!(validate_codex_review_settings(&mut s_bad_model).is_err());

        // Invalid effort
        let mut s_bad_effort = s.clone();
        s_bad_effort.deepseek = Some(ExternalReviewerConfig {
            model: Some("deepseek-flash".to_string()),
            reasoning_effort: Some("extreme".to_string()),
        });
        assert!(validate_codex_review_settings(&mut s_bad_effort).is_err());
    }

    #[test]
    fn test_deepseek_model_and_effort_save_and_reload() {
        let dir = tempdir().unwrap();

        let mut s = CodexReviewSettings {
            schema_version: 4,
            review_mode: ReviewMode::CodexPlusExternal,
            reviewers: ReviewersSelection { codex: false, gemini: false, deepseek: true },
            priority_reviewer: Some("deepseek".to_string()),
            gemini_cli_enabled: false,
            deepseek: Some(ExternalReviewerConfig {
                model: Some("deepseek-v4-pro".to_string()),
                reasoning_effort: Some("max".to_string()),
            }),
            gemini: None,
            unconfirmed_migration: false,
        };
        save_codex_review_settings_at(dir.path(), &mut s).unwrap();

        let read = get_codex_review_settings_at(dir.path()).unwrap();
        assert_eq!(read.deepseek.as_ref().unwrap().model.as_deref(), Some("deepseek-v4-pro"));
        assert_eq!(read.deepseek.as_ref().unwrap().reasoning_effort.as_deref(), Some("max"));

        // When effort is None, it defaults to "high"
        let mut s2 = s.clone();
        s2.deepseek = Some(ExternalReviewerConfig {
            model: Some("deepseek-flash".to_string()),
            reasoning_effort: None,
        });
        save_codex_review_settings_at(dir.path(), &mut s2).unwrap();
        let read2 = get_codex_review_settings_at(dir.path()).unwrap();
        assert_eq!(read2.deepseek.as_ref().unwrap().model.as_deref(), Some("deepseek-flash"));
        assert_eq!(read2.deepseek.as_ref().unwrap().reasoning_effort.as_deref(), Some("high"));
    }
}
