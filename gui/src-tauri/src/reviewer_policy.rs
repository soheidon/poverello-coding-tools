use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const CURRENT_REVIEWER_POLICY_SCHEMA_VERSION: u32 = 6;

static MODEL_CATALOG_JSON: &str = include_str!("../../src/shared/model_catalog.json");

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewerPolicy {
    None,
    Antigravity,
    CodexOnly,
    DeepseekOnly,
    CodexPlusDeepseek,
}

pub fn reviewers_to_policy(reviewers: &[String]) -> ReviewerPolicy {
    let has_codex = reviewers.iter().any(|r| r == "codex");
    let has_deepseek = reviewers.iter().any(|r| r == "deepseek");
    let has_antigravity = reviewers.iter().any(|r| r == "antigravity");

    if has_codex && has_deepseek {
        ReviewerPolicy::CodexPlusDeepseek
    } else if has_codex {
        ReviewerPolicy::CodexOnly
    } else if has_deepseek {
        ReviewerPolicy::DeepseekOnly
    } else if has_antigravity {
        ReviewerPolicy::Antigravity
    } else {
        ReviewerPolicy::None
    }
}

pub fn policy_to_reviewers(policy: ReviewerPolicy) -> Vec<String> {
    match policy {
        ReviewerPolicy::None => vec![],
        ReviewerPolicy::Antigravity => vec!["antigravity".to_string()],
        ReviewerPolicy::CodexOnly => vec!["codex".to_string()],
        ReviewerPolicy::DeepseekOnly => vec!["deepseek".to_string()],
        ReviewerPolicy::CodexPlusDeepseek => vec!["codex".to_string(), "deepseek".to_string()],
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AntigravityPriorityReviewer {
    #[serde(rename = "codex_cli")]
    CodexCli,
    #[serde(rename = "deepseek")]
    Deepseek,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewerPreset {
    Fast,
    Standard,
    Strict,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexReviewerConfig {
    pub model: String,
    pub reasoning_effort: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeepSeekReviewerConfig {
    pub model: String,
    pub reasoning_effort: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewerPolicyConfig {
    pub schema_version: u32,
    pub policy: ReviewerPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_review_policy: Option<ReviewerPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_priority_reviewer: Option<AntigravityPriorityReviewer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_reviewers: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_review_policy: Option<ReviewerPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_priority_reviewer: Option<AntigravityPriorityReviewer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_reviewers: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<ReviewerPreset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex: Option<CodexReviewerConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deepseek: Option<DeepSeekReviewerConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority_reviewer: Option<AntigravityPriorityReviewer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unconfirmed_migration: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_plan_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planner_role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviser_role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
}

impl Default for ReviewerPolicyConfig {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_REVIEWER_POLICY_SCHEMA_VERSION,
            policy: ReviewerPolicy::None,
            plan_review_policy: Some(ReviewerPolicy::None),
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: Some(ReviewerPolicy::None),
            build_priority_reviewer: None,
            build_reviewers: Some(vec![]),
            preset: None,
            codex: None,
            deepseek: None,
            priority_reviewer: None,
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowSkillsPolicyStatus {
    pub compliant: bool,
    pub details: Vec<crate::AntigravityCommandItemInfo>,
}

#[derive(Debug, Clone)]
pub struct CatalogReviewerModel {
    pub cli_model_id: String,
    pub reviewer_reasoning_efforts: Vec<String>,
    pub reviewer_default_effort: String,
}

pub struct ReviewerCatalog {
    pub codex_models: Vec<CatalogReviewerModel>,
    pub deepseek_models: Vec<CatalogReviewerModel>,
}

pub fn load_reviewer_catalog() -> Result<ReviewerCatalog, String> {
    let parsed: serde_json::Value = serde_json::from_str(MODEL_CATALOG_JSON)
        .map_err(|e| format!("Failed to parse embedded model_catalog.json: {e}"))?;

    let models = parsed
        .get("models")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "model_catalog.json missing 'models' object".to_string())?;

    let mut codex_models = Vec::new();
    let mut deepseek_models = Vec::new();

    for (_key, model_val) in models {
        let caps = match model_val.get("capabilities").and_then(|c| c.as_object()) {
            Some(c) => c,
            None => continue,
        };

        let cli_id = caps
            .get("cliModelId")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let efforts: Vec<String> = caps
            .get("reviewerReasoningEfforts")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| e.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let default_effort = caps
            .get("reviewerDefaultEffort")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        if caps.get("codexReviewerEligible").and_then(|v| v.as_bool()).unwrap_or(false) {
            codex_models.push(CatalogReviewerModel {
                cli_model_id: cli_id.clone(),
                reviewer_reasoning_efforts: efforts.clone(),
                reviewer_default_effort: default_effort.clone(),
            });
        }

        if caps.get("deepseekReviewerEligible").and_then(|v| v.as_bool()).unwrap_or(false) {
            deepseek_models.push(CatalogReviewerModel {
                cli_model_id: cli_id,
                reviewer_reasoning_efforts: efforts,
                reviewer_default_effort: default_effort,
            });
        }
    }

    Ok(ReviewerCatalog {
        codex_models,
        deepseek_models,
    })
}

pub fn validate_reviewer_policy_config(config: &ReviewerPolicyConfig) -> Result<(), String> {
    if config.schema_version != CURRENT_REVIEWER_POLICY_SCHEMA_VERSION {
        return Err(format!(
            "Invalid schema_version: expected {}, got {}",
            CURRENT_REVIEWER_POLICY_SCHEMA_VERSION, config.schema_version
        ));
    }

    let catalog = load_reviewer_catalog()?;

    if let Some(reviewers) = &config.build_reviewers {
        for r in reviewers {
            if r != "antigravity" && r != "codex" && r != "deepseek" {
                return Err(format!(
                    "Unknown reviewer in build_reviewers: '{r}'. Allowed: 'antigravity', 'codex', 'deepseek'"
                ));
            }
        }
        let has_codex = reviewers.iter().any(|r| r == "codex");
        let has_deepseek = reviewers.iter().any(|r| r == "deepseek");
        if has_codex {
            let codex = config
                .codex
                .as_ref()
                .ok_or_else(|| "codex configuration is required when codex is in build_reviewers".to_string())?;
            validate_codex_config(codex, &catalog)?;
        }
        if has_deepseek {
            let deepseek = config
                .deepseek
                .as_ref()
                .ok_or_else(|| "deepseek configuration is required when deepseek is in build_reviewers".to_string())?;
            validate_deepseek_config(deepseek, &catalog)?;
        }
        if has_codex && has_deepseek {
            let priority = config.build_priority_reviewer.or(config.priority_reviewer);
            match priority {
                Some(AntigravityPriorityReviewer::CodexCli) | Some(AntigravityPriorityReviewer::Deepseek) => {}
                None => {
                    return Err(
                        "priority_reviewer is required when both codex and deepseek are selected in build_reviewers".to_string(),
                    );
                }
            }
        }
    }

    if let Some(reviewers) = &config.plan_reviewers {
        for r in reviewers {
            if r != "antigravity" && r != "codex" && r != "deepseek" {
                return Err(format!(
                    "Unknown reviewer in plan_reviewers: '{r}'. Allowed: 'antigravity', 'codex', 'deepseek'"
                ));
            }
        }
        let has_codex = reviewers.iter().any(|r| r == "codex");
        let has_deepseek = reviewers.iter().any(|r| r == "deepseek");
        if has_codex {
            let codex = config
                .codex
                .as_ref()
                .ok_or_else(|| "codex configuration is required when codex is in plan_reviewers".to_string())?;
            validate_codex_config(codex, &catalog)?;
        }
        if has_deepseek {
            let deepseek = config
                .deepseek
                .as_ref()
                .ok_or_else(|| "deepseek configuration is required when deepseek is in plan_reviewers".to_string())?;
            validate_deepseek_config(deepseek, &catalog)?;
        }
        if has_codex && has_deepseek {
            let priority = config.plan_priority_reviewer.or(config.priority_reviewer);
            match priority {
                Some(AntigravityPriorityReviewer::CodexCli) | Some(AntigravityPriorityReviewer::Deepseek) => {}
                None => {
                    return Err(
                        "priority_reviewer is required when both codex and deepseek are selected in plan_reviewers".to_string(),
                    );
                }
            }
        }
    }

    let effective_policy = if let Some(ref revs) = config.build_reviewers {
        reviewers_to_policy(revs)
    } else {
        config.policy
    };

    match effective_policy {
        ReviewerPolicy::None | ReviewerPolicy::Antigravity => {
            if config.priority_reviewer.is_some() {
                return Err(format!(
                    "priority_reviewer must be null when policy is {:?}",
                    config.policy
                ));
            }
            if let Some(codex) = &config.codex {
                validate_codex_config(codex, &catalog)?;
            }
            if let Some(deepseek) = &config.deepseek {
                validate_deepseek_config(deepseek, &catalog)?;
            }
        }
        ReviewerPolicy::CodexOnly => {
            if config.priority_reviewer.is_some() {
                return Err("priority_reviewer must be null when policy is codex_only".to_string());
            }
            let codex = config
                .codex
                .as_ref()
                .ok_or_else(|| "codex configuration is required when policy is codex_only".to_string())?;
            validate_codex_config(codex, &catalog)?;
            if let Some(deepseek) = &config.deepseek {
                validate_deepseek_config(deepseek, &catalog)?;
            }
        }
        ReviewerPolicy::DeepseekOnly => {
            if config.priority_reviewer.is_some() {
                return Err("priority_reviewer must be null when policy is deepseek_only".to_string());
            }
            let deepseek = config
                .deepseek
                .as_ref()
                .ok_or_else(|| "deepseek configuration is required when policy is deepseek_only".to_string())?;
            validate_deepseek_config(deepseek, &catalog)?;
            if let Some(codex) = &config.codex {
                validate_codex_config(codex, &catalog)?;
            }
        }
        ReviewerPolicy::CodexPlusDeepseek => {
            match config.priority_reviewer {
                Some(AntigravityPriorityReviewer::CodexCli) | Some(AntigravityPriorityReviewer::Deepseek) => {}
                None => {
                    return Err(
                        "priority_reviewer is required when policy is codex_plus_deepseek (expected 'codex_cli' or 'deepseek')"
                            .to_string(),
                    );
                }
            }

            let codex = config
                .codex
                .as_ref()
                .ok_or_else(|| "codex configuration is required when policy is codex_plus_deepseek".to_string())?;
            validate_codex_config(codex, &catalog)?;

            let deepseek = config
                .deepseek
                .as_ref()
                .ok_or_else(|| "deepseek configuration is required when policy is codex_plus_deepseek".to_string())?;
            validate_deepseek_config(deepseek, &catalog)?;
        }
    }

    if let Some(plan_policy) = config.plan_review_policy {
        match plan_policy {
            ReviewerPolicy::None | ReviewerPolicy::Antigravity => {}
            ReviewerPolicy::CodexOnly => {
                let codex = config
                    .codex
                    .as_ref()
                    .ok_or_else(|| "codex configuration is required when plan_review_policy is codex_only".to_string())?;
                validate_codex_config(codex, &catalog)?;
            }
            ReviewerPolicy::DeepseekOnly => {
                let deepseek = config
                    .deepseek
                    .as_ref()
                    .ok_or_else(|| "deepseek configuration is required when plan_review_policy is deepseek_only".to_string())?;
                validate_deepseek_config(deepseek, &catalog)?;
            }
            ReviewerPolicy::CodexPlusDeepseek => {
                let priority = config.plan_priority_reviewer.or(config.priority_reviewer);
                match priority {
                    Some(AntigravityPriorityReviewer::CodexCli) | Some(AntigravityPriorityReviewer::Deepseek) => {}
                    None => {
                        return Err(
                            "priority_reviewer is required when plan_review_policy is codex_plus_deepseek".to_string(),
                        );
                    }
                }
                let codex = config
                    .codex
                    .as_ref()
                    .ok_or_else(|| "codex configuration is required when plan_review_policy is codex_plus_deepseek".to_string())?;
                validate_codex_config(codex, &catalog)?;
                let deepseek = config
                    .deepseek
                    .as_ref()
                    .ok_or_else(|| "deepseek configuration is required when plan_review_policy is codex_plus_deepseek".to_string())?;
                validate_deepseek_config(deepseek, &catalog)?;
            }
        }
    }

    Ok(())
}

fn validate_codex_config(codex: &CodexReviewerConfig, catalog: &ReviewerCatalog) -> Result<(), String> {
    let model_entry = catalog
        .codex_models
        .iter()
        .find(|m| m.cli_model_id == codex.model)
        .ok_or_else(|| format!("Model '{}' is not eligible for Codex reviewer", codex.model))?;

    if !model_entry.reviewer_reasoning_efforts.contains(&codex.reasoning_effort) {
        return Err(format!(
            "Reasoning effort '{}' is not supported for Codex reviewer model '{}' (supported: {:?})",
            codex.reasoning_effort, codex.model, model_entry.reviewer_reasoning_efforts
        ));
    }
    Ok(())
}

fn validate_deepseek_config(deepseek: &DeepSeekReviewerConfig, catalog: &ReviewerCatalog) -> Result<(), String> {
    let model_entry = catalog
        .deepseek_models
        .iter()
        .find(|m| m.cli_model_id == deepseek.model)
        .ok_or_else(|| format!("Model '{}' is not eligible for DeepSeek reviewer", deepseek.model))?;

    if !model_entry.reviewer_reasoning_efforts.contains(&deepseek.reasoning_effort) {
        return Err(format!(
            "Reasoning effort '{}' is not supported for DeepSeek reviewer model '{}' (supported: {:?})",
            deepseek.reasoning_effort, deepseek.model, model_entry.reviewer_reasoning_efforts
        ));
    }
    Ok(())
}

fn ensure_no_reparse(path: &Path) -> Result<(), String> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!(
                    "Refusing symlink/reparse path: {}",
                    current.display()
                ))
            }
            Ok(meta) => {
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
                    if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                        return Err(format!(
                            "Refusing symlink/reparse path: {}",
                            current.display()
                        ));
                    }
                }
                let _ = meta;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(e) => return Err(format!("Cannot inspect {}: {e}", current.display())),
        }
    }
    Ok(())
}

pub fn inspect_workflow_skills_policy_compliance(
    skills_root: &Path,
) -> Result<WorkflowSkillsPolicyStatus, String> {
    let skill_names = ["pov-plan-only", "pov-build-review", "pov-full-cycle"];
    let mut details = Vec::new();
    let mut all_installed = true;

    for name in skill_names {
        let info = crate::inspect_antigravity_command_status_at(skills_root, name)?;
        if info.status != crate::AntigravitySkillStatus::Installed {
            all_installed = false;
        }
        details.push(info);
    }

    Ok(WorkflowSkillsPolicyStatus {
        compliant: all_installed,
        details,
    })
}

fn build_v2_migrated_config(old_policy: ReviewerPolicy) -> ReviewerPolicyConfig {
    match old_policy {
        ReviewerPolicy::None | ReviewerPolicy::Antigravity => ReviewerPolicyConfig {
            schema_version: CURRENT_REVIEWER_POLICY_SCHEMA_VERSION,
            policy: old_policy,
            plan_review_policy: Some(old_policy),
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: Some(old_policy),
            build_priority_reviewer: None,
            build_reviewers: Some(policy_to_reviewers(old_policy)),
            preset: None,
            codex: None,
            deepseek: None,
            priority_reviewer: None,
            unconfirmed_migration: Some(true),
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        },
        ReviewerPolicy::CodexOnly => ReviewerPolicyConfig {
            schema_version: CURRENT_REVIEWER_POLICY_SCHEMA_VERSION,
            policy: ReviewerPolicy::CodexOnly,
            plan_review_policy: Some(ReviewerPolicy::CodexOnly),
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: Some(ReviewerPolicy::CodexOnly),
            build_priority_reviewer: None,
            build_reviewers: Some(policy_to_reviewers(ReviewerPolicy::CodexOnly)),
            preset: None,
            codex: Some(CodexReviewerConfig {
                model: "gpt-6.1-terra".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            deepseek: None,
            priority_reviewer: None,
            unconfirmed_migration: Some(true),
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        },
        ReviewerPolicy::DeepseekOnly => ReviewerPolicyConfig {
            schema_version: CURRENT_REVIEWER_POLICY_SCHEMA_VERSION,
            policy: ReviewerPolicy::DeepseekOnly,
            plan_review_policy: Some(ReviewerPolicy::DeepseekOnly),
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: Some(ReviewerPolicy::DeepseekOnly),
            build_priority_reviewer: None,
            build_reviewers: Some(policy_to_reviewers(ReviewerPolicy::DeepseekOnly)),
            preset: None,
            codex: None,
            deepseek: Some(DeepSeekReviewerConfig {
                model: "deepseek-flash".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            priority_reviewer: None,
            unconfirmed_migration: Some(true),
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        },
        ReviewerPolicy::CodexPlusDeepseek => ReviewerPolicyConfig {
            schema_version: CURRENT_REVIEWER_POLICY_SCHEMA_VERSION,
            policy: ReviewerPolicy::CodexPlusDeepseek,
            plan_review_policy: Some(ReviewerPolicy::CodexPlusDeepseek),
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: Some(ReviewerPolicy::CodexPlusDeepseek),
            build_priority_reviewer: None,
            build_reviewers: Some(policy_to_reviewers(ReviewerPolicy::CodexPlusDeepseek)),
            preset: None,
            codex: Some(CodexReviewerConfig {
                model: "gpt-6.1-terra".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            deepseek: Some(DeepSeekReviewerConfig {
                model: "deepseek-flash".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            priority_reviewer: None,
            unconfirmed_migration: Some(true),
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        },
    }
}

pub fn inspect_reviewer_policy_at(policy_path: &Path) -> Result<ReviewerPolicyConfig, String> {
    if let Some(parent) = policy_path.parent() {
        if parent.exists() {
            ensure_no_reparse(parent)?;
        }
    }

    let meta = match std::fs::symlink_metadata(policy_path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ReviewerPolicyConfig::default());
        }
        Err(e) => return Err(format!("Cannot inspect policy file metadata: {e}")),
    };

    if !meta.is_file() {
        return Err("Policy path exists but is not a regular file".to_string());
    }

    ensure_no_reparse(policy_path)?;

    let bytes = match std::fs::read(policy_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Warning: Cannot read reviewer policy file at {}: {e}", policy_path.display());
            return Ok(ReviewerPolicyConfig::default());
        }
    };

    let hash = format!("{:x}", Sha256::digest(&bytes));

    let parsed: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Warning: Malformed reviewer policy JSON at {}: {e}", policy_path.display());
            return Ok(ReviewerPolicyConfig::default());
        }
    };

    let version = match parsed.get("schema_version").and_then(|v| v.as_u64()) {
        Some(v) => v,
        None => {
            eprintln!("Warning: Missing or invalid schema_version in reviewer policy at {}", policy_path.display());
            return Ok(ReviewerPolicyConfig::default());
        }
    };

    if version == 1 {
        return Ok(ReviewerPolicyConfig {
            schema_version: CURRENT_REVIEWER_POLICY_SCHEMA_VERSION,
            policy: ReviewerPolicy::None,
            plan_review_policy: Some(ReviewerPolicy::None),
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: Some(ReviewerPolicy::None),
            build_priority_reviewer: None,
            build_reviewers: Some(vec![]),
            preset: None,
            codex: None,
            deepseek: None,
            priority_reviewer: None,
            unconfirmed_migration: Some(true),
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: Some(hash),
        });
    }

    if version == 2 {
        let old_policy_str = parsed.get("policy").and_then(|v| v.as_str()).unwrap_or("none");
        let old_policy = match old_policy_str {
            "codex_only" => ReviewerPolicy::CodexOnly,
            "deepseek_only" => ReviewerPolicy::DeepseekOnly,
            "codex_plus_deepseek" => ReviewerPolicy::CodexPlusDeepseek,
            _ => ReviewerPolicy::None,
        };
        let mut c = build_v2_migrated_config(old_policy);
        c.content_hash = Some(hash);
        return Ok(c);
    }

    if version == 3 {
        let old_policy_str = parsed.get("policy").and_then(|v| v.as_str()).unwrap_or("none");
        let old_policy = match old_policy_str {
            "codex_only" => ReviewerPolicy::CodexOnly,
            "deepseek_only" => ReviewerPolicy::DeepseekOnly,
            "codex_plus_deepseek" => ReviewerPolicy::CodexPlusDeepseek,
            _ => ReviewerPolicy::None,
        };
        let codex: Option<CodexReviewerConfig> = parsed
            .get("codex")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let deepseek: Option<DeepSeekReviewerConfig> = parsed
            .get("deepseek")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        return Ok(ReviewerPolicyConfig {
            schema_version: CURRENT_REVIEWER_POLICY_SCHEMA_VERSION,
            policy: old_policy,
            plan_review_policy: Some(old_policy),
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: Some(old_policy),
            build_priority_reviewer: None,
            build_reviewers: Some(policy_to_reviewers(old_policy)),
            preset: None,
            codex,
            deepseek,
            priority_reviewer: None,
            unconfirmed_migration: Some(true),
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: Some(hash),
        });
    }

    if version == 4 {
        let old_policy_str = parsed.get("policy").and_then(|v| v.as_str()).unwrap_or("none");
        let old_policy = match old_policy_str {
            "codex_only" => ReviewerPolicy::CodexOnly,
            "deepseek_only" => ReviewerPolicy::DeepseekOnly,
            "codex_plus_deepseek" => ReviewerPolicy::CodexPlusDeepseek,
            _ => ReviewerPolicy::None,
        };
        let codex: Option<CodexReviewerConfig> = parsed
            .get("codex")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let deepseek: Option<DeepSeekReviewerConfig> = parsed
            .get("deepseek")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let priority_reviewer: Option<AntigravityPriorityReviewer> = parsed
            .get("priority_reviewer")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let unconfirmed = parsed.get("unconfirmed_migration").and_then(|v| v.as_bool());

        return Ok(ReviewerPolicyConfig {
            schema_version: CURRENT_REVIEWER_POLICY_SCHEMA_VERSION,
            policy: old_policy,
            plan_review_policy: Some(old_policy),
            plan_priority_reviewer: priority_reviewer,
            plan_reviewers: None,
            build_review_policy: Some(old_policy),
            build_priority_reviewer: priority_reviewer,
            build_reviewers: Some(policy_to_reviewers(old_policy)),
            preset: None,
            codex,
            deepseek,
            priority_reviewer,
            unconfirmed_migration: unconfirmed,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: Some(hash),
        });
    }

    if version == 5 {
        let old_policy_str = parsed.get("policy").and_then(|v| v.as_str()).unwrap_or("none");
        let old_policy = match old_policy_str {
            "antigravity" => ReviewerPolicy::Antigravity,
            "codex_only" => ReviewerPolicy::CodexOnly,
            "deepseek_only" => ReviewerPolicy::DeepseekOnly,
            "codex_plus_deepseek" => ReviewerPolicy::CodexPlusDeepseek,
            _ => ReviewerPolicy::None,
        };
        let codex: Option<CodexReviewerConfig> = parsed
            .get("codex")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let deepseek: Option<DeepSeekReviewerConfig> = parsed
            .get("deepseek")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let priority_reviewer: Option<AntigravityPriorityReviewer> = parsed
            .get("priority_reviewer")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let unconfirmed = parsed.get("unconfirmed_migration").and_then(|v| v.as_bool());
        let active_plan_path = parsed.get("active_plan_path").and_then(|v| v.as_str()).map(|s| s.to_string());
        let planner_role = parsed.get("planner_role").and_then(|v| v.as_str()).map(|s| s.to_string());
        let reviser_role = parsed.get("reviser_role").and_then(|v| v.as_str()).map(|s| s.to_string());

        return Ok(ReviewerPolicyConfig {
            schema_version: CURRENT_REVIEWER_POLICY_SCHEMA_VERSION,
            policy: old_policy,
            plan_review_policy: Some(old_policy),
            plan_priority_reviewer: priority_reviewer,
            plan_reviewers: None,
            build_review_policy: Some(old_policy),
            build_priority_reviewer: priority_reviewer,
            build_reviewers: Some(policy_to_reviewers(old_policy)),
            preset: None,
            codex,
            deepseek,
            priority_reviewer,
            unconfirmed_migration: unconfirmed,
            active_plan_path,
            planner_role,
            reviser_role,
            content_hash: Some(hash),
        });
    }

    if version != CURRENT_REVIEWER_POLICY_SCHEMA_VERSION as u64 {
        eprintln!(
            "Warning: Unsupported reviewer policy schema_version: {version} at {}. Falling back to default none.",
            policy_path.display()
        );
        return Ok(ReviewerPolicyConfig::default());
    }

    let mut config: ReviewerPolicyConfig = match serde_json::from_value(parsed) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: Failed to deserialize schema v6 reviewer policy at {}: {e}. Falling back to default none.", policy_path.display());
            return Ok(ReviewerPolicyConfig::default());
        }
    };

    if config.build_reviewers.is_none() {
        config.build_reviewers = Some(policy_to_reviewers(config.build_review_policy.unwrap_or(config.policy)));
    }

    if let Err(e) = validate_reviewer_policy_config(&config) {
        eprintln!("Warning: Invalid reviewer policy configuration at {}: {e}.", policy_path.display());
        return Ok(ReviewerPolicyConfig {
            schema_version: CURRENT_REVIEWER_POLICY_SCHEMA_VERSION,
            policy: config.policy,
            plan_review_policy: config.plan_review_policy,
            plan_priority_reviewer: config.plan_priority_reviewer,
            plan_reviewers: config.plan_reviewers,
            build_review_policy: config.build_review_policy,
            build_priority_reviewer: config.build_priority_reviewer,
            build_reviewers: Some(policy_to_reviewers(config.build_review_policy.unwrap_or(config.policy))),
            preset: None,
            codex: config.codex,
            deepseek: config.deepseek,
            priority_reviewer: None,
            unconfirmed_migration: Some(true),
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: Some(hash),
        });
    }

    config.preset = None; // per R2-C01: ignore legacy preset on load
    config.unconfirmed_migration = None;
    config.content_hash = Some(hash);
    Ok(config)
}

pub fn save_reviewer_policy_at(
    policy_path: &Path,
    config: &ReviewerPolicyConfig,
    skills_root: &Path,
    expected_hash: Option<&str>,
) -> Result<ReviewerPolicyConfig, String> {
    save_reviewer_policy_internal(policy_path, config, skills_root, expected_hash, || Ok(()))
}

pub(crate) fn save_reviewer_policy_internal<F>(
    policy_path: &Path,
    config: &ReviewerPolicyConfig,
    skills_root: &Path,
    expected_hash: Option<&str>,
    pre_replace_hook: F,
) -> Result<ReviewerPolicyConfig, String>
where
    F: FnOnce() -> Result<(), String>,
{
    validate_reviewer_policy_config(config)?;

    let compliance = inspect_workflow_skills_policy_compliance(skills_root)?;
    if !compliance.compliant {
        return Err(
            "Reviewer policy cannot be configured: all 3 workflow skills (pov-plan-only, pov-build-review, pov-full-cycle) must be installed and up-to-date first."
                .to_string(),
        );
    }

    // 1. Initial verification against current on-disk state
    if policy_path.exists() {
        let existing_bytes = std::fs::read(policy_path)
            .map_err(|e| format!("Cannot read existing reviewer policy for hash verification: {e}"))?;
        let current_hash = format!("{:x}", Sha256::digest(&existing_bytes));
        match expected_hash {
            Some(expected) if expected.eq_ignore_ascii_case(&current_hash) => {
                // Expected hash matches current disk state
            }
            Some(expected) => {
                return Err(format!(
                    "Concurrent modification conflict: reviewer policy on-disk hash ({current_hash}) does not match expected hash ({expected}). Please reload before saving."
                ));
            }
            None => {
                return Err(format!(
                    "Concurrent modification conflict: reviewer policy exists on disk ({current_hash}) but no expected hash was provided. Please reload before saving."
                ));
            }
        }
    } else if let Some(expected) = expected_hash {
        return Err(format!(
            "Concurrent modification conflict: reviewer policy does not exist on disk, but expected hash was provided ({expected}). Please reload before saving."
        ));
    }

    let parent = policy_path
        .parent()
        .ok_or_else(|| "Policy path has no parent directory".to_string())?;

    ensure_no_reparse(parent)?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("Cannot create policy directory {}: {e}", parent.display()))?;

    ensure_no_reparse(policy_path)?;

    let mut config_to_write = config.clone();
    config_to_write.schema_version = CURRENT_REVIEWER_POLICY_SCHEMA_VERSION;
    config_to_write.content_hash = None;
    config_to_write.preset = None; // per R2-C01: do not write preset on save
    config_to_write.unconfirmed_migration = None;
    if let Some(ref revs) = config_to_write.build_reviewers {
        let derived_policy = reviewers_to_policy(revs);
        config_to_write.build_review_policy = Some(derived_policy);
        config_to_write.policy = derived_policy;
    } else {
        if config_to_write.build_review_policy.is_none() {
            config_to_write.build_review_policy = Some(config_to_write.policy);
        }
        config_to_write.policy = config_to_write.build_review_policy.unwrap_or(config_to_write.policy);
        config_to_write.build_reviewers = Some(policy_to_reviewers(config_to_write.policy));
    }
    if let Some(ref revs) = config_to_write.plan_reviewers {
        let derived_plan_policy = reviewers_to_policy(revs);
        config_to_write.plan_review_policy = Some(derived_plan_policy);
    } else if config_to_write.plan_review_policy.is_none() {
        config_to_write.plan_review_policy = Some(config_to_write.policy);
    }

    let serialized = serde_json::to_vec_pretty(&config_to_write)
        .map_err(|e| format!("Failed to serialize reviewer policy: {e}"))?;

    let temp_name = format!(
        ".pov-reviewer-policy.json.tmp-{}",
        uuid::Uuid::new_v4()
    );
    let temp_path = parent.join(&temp_name);

    let write_res: Result<(), std::io::Error> = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        use std::io::Write;
        file.write_all(&serialized)?;
        file.sync_all()?;
        drop(file);

        ensure_no_reparse(policy_path).map_err(std::io::Error::other)?;

        // Execute hook before replacement
        pre_replace_hook().map_err(std::io::Error::other)?;

        // 2. Pre-replacement re-verification: verify target on-disk state has not changed since initial check
        if policy_path.exists() {
            let disk_bytes = std::fs::read(policy_path)?;
            let disk_hash = format!("{:x}", Sha256::digest(&disk_bytes));
            match expected_hash {
                Some(expected) if expected.eq_ignore_ascii_case(&disk_hash) => {}
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        format!(
                            "Concurrent modification conflict: reviewer policy on disk changed to ({disk_hash}) before replacement. Write aborted."
                        ),
                    ));
                }
            }
        } else if expected_hash.is_some() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Concurrent modification conflict: reviewer policy was deleted before replacement. Write aborted.",
            ));
        }

        #[cfg(windows)]
        {
            if expected_hash.is_none() {
                move_file_new_only_win(&temp_path, policy_path)?;
            } else {
                let backup_name = format!(".pov-reviewer-policy.json.bak-{}", uuid::Uuid::new_v4());
                let backup_path = parent.join(&backup_name);
                replace_file_with_backup_win(&temp_path, policy_path, &backup_path)?;

                let replaced_bytes = std::fs::read(&backup_path)?;
                let replaced_hash = format!("{:x}", Sha256::digest(&replaced_bytes));
                let expected = expected_hash.unwrap();
                if !replaced_hash.eq_ignore_ascii_case(expected) {
                    let restore_res = replace_file_win(&backup_path, policy_path);
                    if let Err(restore_err) = restore_res {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::AlreadyExists,
                            format!(
                                "Concurrent modification conflict: replaced file differed from expected, and rollback failed: {restore_err}. Intervening content preserved in backup: {}",
                                backup_path.display()
                            ),
                        ));
                    }
                    let _ = std::fs::remove_file(&backup_path);
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        format!(
                            "Concurrent modification conflict: replaced file hash ({replaced_hash}) differed from expected ({expected}). Intervening content restored."
                        ),
                    ));
                }
                let _ = std::fs::remove_file(&backup_path);
            }
        }
        #[cfg(not(windows))]
        {
            if expected_hash.is_none() {
                let link_res = std::fs::hard_link(&temp_path, policy_path);
                let _ = std::fs::remove_file(&temp_path);
                link_res.map_err(|e| {
                    std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        format!("Concurrent modification conflict: target policy was created concurrently: {e}"),
                    )
                })?;
            } else {
                replace_file_conditional_unix(&temp_path, policy_path, expected_hash.unwrap())?;
            }
        }

        Ok::<(), std::io::Error>(())
    })();

    if let Err(err) = write_res {
        let err_str = err.to_string();
        if !err_str.contains("Intervening content preserved in") {
            let _ = std::fs::remove_file(&temp_path);
        }
        return Err(format!("Failed to persist reviewer policy: {err_str}"));
    }

    let read_back = inspect_reviewer_policy_at(policy_path)?;
    if read_back.schema_version != CURRENT_REVIEWER_POLICY_SCHEMA_VERSION
        || read_back.policy != config.policy
        || read_back.preset != config_to_write.preset
        || read_back.codex != config.codex
        || read_back.deepseek != config.deepseek
    {
        return Err("Policy written but read-back verification failed".to_string());
    }

    Ok(read_back)
}

#[cfg(windows)]
fn move_file_new_only_win(temp: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new_file: *const u16, flags: u32) -> i32;
    }
    let existing_w = temp
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let new_file_w = target
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let ok = unsafe { MoveFileExW(existing_w.as_ptr(), new_file_w.as_ptr(), 0) };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn replace_file_with_backup_win(temp: &Path, target: &Path, backup: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    unsafe extern "system" {
        fn ReplaceFileW(
            replaced: *const u16,
            replacement: *const u16,
            backup: *const u16,
            flags: u32,
            exclude: *mut std::ffi::c_void,
            reserved: *mut std::ffi::c_void,
        ) -> i32;
    }
    let target_w = target
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let temp_w = temp
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let backup_w = backup
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let ok = unsafe {
        ReplaceFileW(
            target_w.as_ptr(),
            temp_w.as_ptr(),
            backup_w.as_ptr(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn replace_file_win(temp: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    unsafe extern "system" {
        fn ReplaceFileW(
            replaced: *const u16,
            replacement: *const u16,
            backup: *const u16,
            flags: u32,
            exclude: *mut std::ffi::c_void,
            reserved: *mut std::ffi::c_void,
        ) -> i32;
    }
    let target_w = target
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let temp_w = temp
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let ok = unsafe {
        ReplaceFileW(
            target_w.as_ptr(),
            temp_w.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file_conditional_unix(
    temp: &Path,
    target: &Path,
    expected_hash: &str,
) -> Result<(), std::io::Error> {
    let initial_bytes = std::fs::read(target)?;
    let initial_hash = format!("{:x}", Sha256::digest(&initial_bytes));
    if !initial_hash.eq_ignore_ascii_case(expected_hash) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "Concurrent modification conflict: target policy on disk changed to ({initial_hash}) before replacement (expected {expected_hash})."
            ),
        ));
    }

    let swapped = try_atomic_exchange_unix(temp, target);
    match swapped {
        Ok(()) => {
            let replaced_bytes = std::fs::read(temp)?;
            let replaced_hash = format!("{:x}", Sha256::digest(&replaced_bytes));
            if !replaced_hash.eq_ignore_ascii_case(expected_hash) {
                let swap_back = try_atomic_exchange_unix(temp, target);
                if let Err(e) = swap_back {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        format!(
                            "Concurrent modification conflict: replaced file differed from expected, and swap rollback failed: {e}. Intervening content preserved in: {}",
                            temp.display()
                        ),
                    ));
                }
                let _ = std::fs::remove_file(temp);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!(
                        "Concurrent modification conflict: replaced file hash ({replaced_hash}) differed from expected ({expected_hash}). Intervening content restored."
                    ),
                ));
            }
            let _ = std::fs::remove_file(temp);
            Ok(())
        }
        Err(_) => {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Atomic compare-and-replace exchange is not supported on this platform; policy update is disabled to prevent concurrent modification races per §6.2.",
            ))
        }
    }
}

#[cfg(all(not(windows), target_os = "linux"))]
fn try_atomic_exchange_unix(temp: &Path, target: &Path) -> Result<(), std::io::Error> {
    use std::os::unix::ffi::OsStrExt;
    const AT_FDCWD: std::ffi::c_int = -100;
    const RENAME_EXCHANGE: std::ffi::c_uint = 2;

    unsafe extern "C" {
        fn renameat2(
            olddirfd: std::ffi::c_int,
            oldpath: *const std::ffi::c_char,
            newdirfd: std::ffi::c_int,
            newpath: *const std::ffi::c_char,
            flags: std::ffi::c_uint,
        ) -> std::ffi::c_int;
    }

    let temp_c = std::ffi::CString::new(temp.as_os_str().as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let target_c = std::ffi::CString::new(target.as_os_str().as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    let res = unsafe {
        renameat2(
            AT_FDCWD,
            temp_c.as_ptr(),
            AT_FDCWD,
            target_c.as_ptr(),
            RENAME_EXCHANGE,
        )
    };
    if res == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(all(not(windows), target_os = "macos"))]
fn try_atomic_exchange_unix(temp: &Path, target: &Path) -> Result<(), std::io::Error> {
    use std::os::unix::ffi::OsStrExt;
    const RENAME_SWAP: std::ffi::c_uint = 2;

    unsafe extern "C" {
        fn renamex_np(
            from: *const std::ffi::c_char,
            to: *const std::ffi::c_char,
            flags: std::ffi::c_uint,
        ) -> std::ffi::c_int;
    }

    let temp_c = std::ffi::CString::new(temp.as_os_str().as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let target_c = std::ffi::CString::new(target.as_os_str().as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    let res = unsafe { renamex_np(temp_c.as_ptr(), target_c.as_ptr(), RENAME_SWAP) };
    if res == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(all(not(windows), not(target_os = "linux"), not(target_os = "macos")))]
fn try_atomic_exchange_unix(_temp: &Path, _target: &Path) -> Result<(), std::io::Error> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Atomic exchange not supported on this platform",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_skills_static_contract_inspection() {
        let skills = [
            crate::ANTIGRAVITY_POV_PLAN_ONLY_SKILL_MD,
            crate::ANTIGRAVITY_POV_BUILD_REVIEW_SKILL_MD,
            crate::ANTIGRAVITY_POV_FULL_CYCLE_SKILL_MD,
        ];

        for skill_content in skills {
            assert!(!skill_content.contains(r#"{"schema_version":1,"policy":"codex_plus_deepseek"}"#));
            assert!(skill_content.contains("Evaluation Precedence"));
            assert!(skill_content.contains("実行時明示指定"));
            assert!(skill_content.contains("pov-reviewer-policy.json"));
            assert!(skill_content.contains("`none` の場合（外部レビュー無効）"));
            assert!(skill_content.contains("呼出数厳密に 0"));
            assert!(skill_content.contains("EXTERNAL REVIEW DISABLED"));
            assert!(skill_content.contains("No-Fallback"));
        }
    }

    #[test]
    fn catalog_eligible_models_loaded_correctly() {
        let catalog = load_reviewer_catalog().expect("catalog must parse");
        let codex_ids: Vec<_> = catalog.codex_models.iter().map(|m| m.cli_model_id.as_str()).collect();
        assert!(codex_ids.contains(&"gpt-6-luna"));
        assert!(codex_ids.contains(&"gpt-6.1-sol"));
        assert!(codex_ids.contains(&"gpt-6.1-terra"));

        let deepseek_ids: Vec<_> = catalog.deepseek_models.iter().map(|m| m.cli_model_id.as_str()).collect();
        assert!(deepseek_ids.contains(&"deepseek-flash"));
        assert!(deepseek_ids.contains(&"deepseek-v4-pro"));

        let terra = catalog.codex_models.iter().find(|m| m.cli_model_id == "gpt-6.1-terra").unwrap();
        assert!(terra.reviewer_reasoning_efforts.contains(&"none".to_string()));
        assert!(terra.reviewer_reasoning_efforts.contains(&"high".to_string()));
    }

    #[test]
    fn missing_policy_returns_default_none() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pov-reviewer-policy.json");
        let loaded = inspect_reviewer_policy_at(&path).unwrap();
        assert_eq!(loaded, ReviewerPolicyConfig::default());
        assert_eq!(loaded.schema_version, 6);
        assert_eq!(loaded.policy, ReviewerPolicy::None);
        assert_eq!(loaded.preset, None);
        assert_eq!(loaded.codex, None);
        assert_eq!(loaded.deepseek, None);
        assert_eq!(loaded.priority_reviewer, None);
        assert_eq!(loaded.unconfirmed_migration, None);
        assert_eq!(loaded.active_plan_path, None);
        assert_eq!(loaded.planner_role, None);
        assert_eq!(loaded.reviser_role, None);
        assert_eq!(loaded.content_hash, None);
    }

    #[test]
    fn valid_policy_round_trips() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pov-reviewer-policy.json");
        let content = r#"{
            "schema_version": 6,
            "policy": "codex_plus_deepseek",
            "preset": "standard",
            "codex": {
                "model": "gpt-6.1-terra",
                "reasoning_effort": "high"
            },
            "deepseek": {
                "model": "deepseek-flash",
                "reasoning_effort": "high"
            },
            "priority_reviewer": "codex_cli"
        }"#;
        std::fs::write(&path, content).unwrap();

        let loaded = inspect_reviewer_policy_at(&path).unwrap();
        assert_eq!(loaded.schema_version, 6);
        assert_eq!(loaded.policy, ReviewerPolicy::CodexPlusDeepseek);
        assert_eq!(loaded.preset, None);
        assert_eq!(
            loaded.codex,
            Some(CodexReviewerConfig {
                model: "gpt-6.1-terra".to_string(),
                reasoning_effort: "high".to_string(),
            })
        );
        assert_eq!(
            loaded.deepseek,
            Some(DeepSeekReviewerConfig {
                model: "deepseek-flash".to_string(),
                reasoning_effort: "high".to_string(),
            })
        );
        assert_eq!(
            loaded.priority_reviewer,
            Some(AntigravityPriorityReviewer::CodexCli)
        );
        assert_eq!(loaded.unconfirmed_migration, None);
        assert_eq!(loaded.planner_role, None);
        assert_eq!(loaded.reviser_role, None);
        assert_eq!(loaded.active_plan_path, None);
        assert!(loaded.content_hash.is_some());
    }

    #[test]
    fn v1_policy_migrates_in_memory_without_overwriting_disk() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pov-reviewer-policy.json");
        std::fs::write(&path, r#"{"schema_version":1,"policy":"codex_only"}"#).unwrap();

        let loaded = inspect_reviewer_policy_at(&path).unwrap();
        assert_eq!(loaded.schema_version, 6);
        assert_eq!(loaded.policy, ReviewerPolicy::None);
        assert_eq!(loaded.unconfirmed_migration, Some(true));
        assert!(loaded.content_hash.is_some());

        let on_disk_str = std::fs::read_to_string(&path).unwrap();
        assert!(on_disk_str.contains(r#""schema_version":1"#));
        assert!(on_disk_str.contains(r#""policy":"codex_only""#));
    }

    #[test]
    fn v2_policy_migrates_in_memory_without_overwriting_disk() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pov-reviewer-policy.json");
        std::fs::write(&path, r#"{"schema_version":2,"policy":"codex_plus_deepseek"}"#).unwrap();

        let loaded = inspect_reviewer_policy_at(&path).unwrap();
        assert_eq!(loaded.schema_version, 6);
        assert_eq!(loaded.policy, ReviewerPolicy::CodexPlusDeepseek);
        assert_eq!(loaded.preset, None);
        assert_eq!(
            loaded.codex,
            Some(CodexReviewerConfig {
                model: "gpt-6.1-terra".to_string(),
                reasoning_effort: "high".to_string(),
            })
        );
        assert_eq!(
            loaded.deepseek,
            Some(DeepSeekReviewerConfig {
                model: "deepseek-flash".to_string(),
                reasoning_effort: "high".to_string(),
            })
        );
        assert_eq!(loaded.priority_reviewer, None);
        assert_eq!(loaded.unconfirmed_migration, Some(true));
        assert!(loaded.content_hash.is_some());

        let on_disk_str = std::fs::read_to_string(&path).unwrap();
        assert!(on_disk_str.contains(r#""schema_version":2"#));
    }

    #[test]
    fn v3_policy_migrates_in_memory_without_overwriting_disk() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pov-reviewer-policy.json");
        let v3_json = r#"{
            "schema_version": 3,
            "policy": "codex_plus_deepseek",
            "codex": { "model": "gpt-6.1-sol", "reasoning_effort": "high" },
            "deepseek": { "model": "deepseek-v4-pro", "reasoning_effort": "max" }
        }"#;
        std::fs::write(&path, v3_json).unwrap();

        let loaded = inspect_reviewer_policy_at(&path).unwrap();
        assert_eq!(loaded.schema_version, 6);
        assert_eq!(loaded.policy, ReviewerPolicy::CodexPlusDeepseek);
        assert_eq!(
            loaded.codex,
            Some(CodexReviewerConfig {
                model: "gpt-6.1-sol".to_string(),
                reasoning_effort: "high".to_string(),
            })
        );
        assert_eq!(
            loaded.deepseek,
            Some(DeepSeekReviewerConfig {
                model: "deepseek-v4-pro".to_string(),
                reasoning_effort: "max".to_string(),
            })
        );
        assert_eq!(loaded.priority_reviewer, None);
        assert_eq!(loaded.unconfirmed_migration, Some(true));
        assert!(loaded.content_hash.is_some());

        let on_disk_str = std::fs::read_to_string(&path).unwrap();
        assert!(on_disk_str.contains(r#""schema_version": 3"#));
    }

    #[test]
    fn v4_policy_migrates_in_memory_without_overwriting_disk() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pov-reviewer-policy.json");
        let v4_json = r#"{
            "schema_version": 4,
            "policy": "codex_plus_deepseek",
            "codex": { "model": "gpt-6.1-sol", "reasoning_effort": "high" },
            "deepseek": { "model": "deepseek-v4-pro", "reasoning_effort": "max" },
            "priority_reviewer": "codex_cli"
        }"#;
        std::fs::write(&path, v4_json).unwrap();

        let loaded = inspect_reviewer_policy_at(&path).unwrap();
        assert_eq!(loaded.schema_version, 6);
        assert_eq!(loaded.policy, ReviewerPolicy::CodexPlusDeepseek);
        assert_eq!(loaded.priority_reviewer, Some(AntigravityPriorityReviewer::CodexCli));
        assert_eq!(loaded.planner_role, None);
        assert_eq!(loaded.reviser_role, None);
        assert_eq!(loaded.active_plan_path, None);
        assert!(loaded.content_hash.is_some());

        let on_disk_str = std::fs::read_to_string(&path).unwrap();
        assert!(on_disk_str.contains(r#""schema_version": 4"#));
    }

    #[test]
    fn malformed_json_fails_safe_to_none() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pov-reviewer-policy.json");
        std::fs::write(&path, "{not-json}").unwrap();

        let loaded = inspect_reviewer_policy_at(&path).unwrap();
        assert_eq!(loaded.schema_version, 6);
        assert_eq!(loaded.policy, ReviewerPolicy::None);
        assert_eq!(loaded.content_hash, None);
    }

    #[test]
    fn unknown_version_fails_safe_to_none() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pov-reviewer-policy.json");
        std::fs::write(&path, r#"{"schema_version":99,"policy":"codex_only"}"#).unwrap();

        let loaded = inspect_reviewer_policy_at(&path).unwrap();
        assert_eq!(loaded.schema_version, 6);
        assert_eq!(loaded.policy, ReviewerPolicy::None);
    }

    #[test]
    fn missing_policy_field_fails_safe_to_none() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pov-reviewer-policy.json");
        std::fs::write(&path, r#"{"schema_version":6}"#).unwrap();

        let loaded = inspect_reviewer_policy_at(&path).unwrap();
        assert_eq!(loaded.schema_version, 6);
        assert_eq!(loaded.policy, ReviewerPolicy::None);
    }

    #[test]
    fn unknown_policy_value_fails_safe_to_none() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pov-reviewer-policy.json");
        std::fs::write(&path, r#"{"schema_version":6,"policy":"claude_super"}"#).unwrap();

        let loaded = inspect_reviewer_policy_at(&path).unwrap();
        assert_eq!(loaded.schema_version, 6);
        assert_eq!(loaded.policy, ReviewerPolicy::None);
    }

    #[test]
    fn validation_rejects_ineligible_codex_model() {
        let config = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexOnly,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: None,
            codex: Some(CodexReviewerConfig {
                model: "unknown-codex-model".to_string(),
                reasoning_effort: "medium".to_string(),
            }),
            deepseek: None,
            priority_reviewer: None,
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };
        let err = validate_reviewer_policy_config(&config).unwrap_err();
        assert!(err.contains("not eligible for Codex reviewer"));
    }

    #[test]
    fn validation_rejects_unsupported_codex_effort() {
        let config = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexOnly,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: None,
            codex: Some(CodexReviewerConfig {
                model: "gpt-6.1-sol".to_string(),
                reasoning_effort: "ultra".to_string(),
            }),
            deepseek: None,
            priority_reviewer: None,
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };
        let err = validate_reviewer_policy_config(&config).unwrap_err();
        assert!(err.contains("Reasoning effort 'ultra' is not supported"));
    }

    #[test]
    fn validation_rejects_ineligible_deepseek_model() {
        let config = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexPlusDeepseek,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: None,
            codex: Some(CodexReviewerConfig {
                model: "gpt-6-luna".to_string(),
                reasoning_effort: "medium".to_string(),
            }),
            deepseek: Some(DeepSeekReviewerConfig {
                model: "deepseek-chat-unknown".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            priority_reviewer: Some(AntigravityPriorityReviewer::CodexCli),
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };
        let err = validate_reviewer_policy_config(&config).unwrap_err();
        assert!(err.contains("not eligible for DeepSeek reviewer"));
    }

    #[test]
    fn validation_rejects_unsupported_deepseek_effort() {
        let config = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexPlusDeepseek,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: None,
            codex: Some(CodexReviewerConfig {
                model: "gpt-6-luna".to_string(),
                reasoning_effort: "medium".to_string(),
            }),
            deepseek: Some(DeepSeekReviewerConfig {
                model: "deepseek-flash".to_string(),
                reasoning_effort: "medium".to_string(),
            }),
            priority_reviewer: Some(AntigravityPriorityReviewer::CodexCli),
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };
        let err = validate_reviewer_policy_config(&config).unwrap_err();
        assert!(err.contains("Reasoning effort 'medium' is not supported"));
    }

    #[test]
    fn validation_rejects_missing_priority_for_codex_plus_deepseek() {
        let config = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexPlusDeepseek,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: None,
            codex: Some(CodexReviewerConfig {
                model: "gpt-6.1-terra".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            deepseek: Some(DeepSeekReviewerConfig {
                model: "deepseek-flash".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            priority_reviewer: None,
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };
        let err = validate_reviewer_policy_config(&config).unwrap_err();
        assert!(err.contains("priority_reviewer is required"));
    }

    #[test]
    fn validation_rejects_priority_for_none_and_codex_only() {
        let none_config = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::None,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: None,
            codex: None,
            deepseek: None,
            priority_reviewer: Some(AntigravityPriorityReviewer::CodexCli),
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };
        let err1 = validate_reviewer_policy_config(&none_config).unwrap_err();
        assert!(err1.contains("priority_reviewer must be null"));

        let codex_config = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexOnly,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: None,
            codex: Some(CodexReviewerConfig {
                model: "gpt-6-luna".to_string(),
                reasoning_effort: "medium".to_string(),
            }),
            deepseek: None,
            priority_reviewer: Some(AntigravityPriorityReviewer::Deepseek),
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };
        let err2 = validate_reviewer_policy_config(&codex_config).unwrap_err();
        assert!(err2.contains("priority_reviewer must be null"));

        let deepseek_config = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::DeepseekOnly,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: None,
            codex: None,
            deepseek: Some(DeepSeekReviewerConfig {
                model: "deepseek-flash".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            priority_reviewer: Some(AntigravityPriorityReviewer::CodexCli),
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };
        let err3 = validate_reviewer_policy_config(&deepseek_config).unwrap_err();
        assert!(err3.contains("priority_reviewer must be null"));
    }

    #[test]
    fn validation_accepts_valid_presets() {
        // Fast
        let fast = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexPlusDeepseek,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: Some(ReviewerPreset::Fast),
            codex: Some(CodexReviewerConfig {
                model: "gpt-6-luna".to_string(),
                reasoning_effort: "medium".to_string(),
            }),
            deepseek: Some(DeepSeekReviewerConfig {
                model: "deepseek-flash".to_string(),
                reasoning_effort: "low".to_string(),
            }),
            priority_reviewer: Some(AntigravityPriorityReviewer::CodexCli),
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };
        assert!(validate_reviewer_policy_config(&fast).is_ok());

        // Standard
        let standard = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexPlusDeepseek,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: Some(ReviewerPreset::Standard),
            codex: Some(CodexReviewerConfig {
                model: "gpt-6.1-terra".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            deepseek: Some(DeepSeekReviewerConfig {
                model: "deepseek-flash".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            priority_reviewer: Some(AntigravityPriorityReviewer::Deepseek),
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };
        assert!(validate_reviewer_policy_config(&standard).is_ok());

        // Strict
        let strict = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexPlusDeepseek,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: Some(ReviewerPreset::Strict),
            codex: Some(CodexReviewerConfig {
                model: "gpt-6.1-sol".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            deepseek: Some(DeepSeekReviewerConfig {
                model: "deepseek-v4-pro".to_string(),
                reasoning_effort: "max".to_string(),
            }),
            priority_reviewer: Some(AntigravityPriorityReviewer::CodexCli),
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };
        assert!(validate_reviewer_policy_config(&strict).is_ok());
    }

    #[test]
    fn save_policy_blocked_when_skills_not_compliant() {
        let temp_ide = tempfile::tempdir().unwrap();
        let temp_skills = tempfile::tempdir().unwrap();
        let policy_path = temp_ide.path().join("pov-reviewer-policy.json");

        let config = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexOnly,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: None,
            codex: Some(CodexReviewerConfig {
                model: "gpt-6-luna".to_string(),
                reasoning_effort: "medium".to_string(),
            }),
            deepseek: None,
            priority_reviewer: None,
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };

        let err = save_reviewer_policy_at(&policy_path, &config, temp_skills.path(), None)
            .unwrap_err();
        assert!(err.contains("must be installed and up-to-date"));
        assert!(!policy_path.exists());
    }

    #[test]
    fn save_policy_succeeds_when_skills_compliant() {
        let temp_ide = tempfile::tempdir().unwrap();
        let temp_skills = tempfile::tempdir().unwrap();
        let policy_path = temp_ide.path().join("pov-reviewer-policy.json");

        for name in ["pov-plan-only", "pov-build-review", "pov-full-cycle"] {
            crate::install_antigravity_command_at(temp_skills.path(), name).unwrap();
        }

        let none_config = ReviewerPolicyConfig::default();
        let saved = save_reviewer_policy_at(&policy_path, &none_config, temp_skills.path(), None)
            .unwrap();
        assert_eq!(saved.policy, ReviewerPolicy::None);
        assert_eq!(saved.schema_version, 6);
        assert_eq!(saved.priority_reviewer, None);
        assert!(saved.content_hash.is_some());

        let reloaded = inspect_reviewer_policy_at(&policy_path).unwrap();
        assert_eq!(reloaded, saved);

        let codex_config = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexOnly,
            plan_review_policy: Some(ReviewerPolicy::CodexOnly),
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: Some(ReviewerPolicy::CodexOnly),
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: Some(ReviewerPreset::Fast),
            codex: Some(CodexReviewerConfig {
                model: "gpt-6-luna".to_string(),
                reasoning_effort: "medium".to_string(),
            }),
            deepseek: None,
            priority_reviewer: None,
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };

        let saved_codex = save_reviewer_policy_at(
            &policy_path,
            &codex_config,
            temp_skills.path(),
            saved.content_hash.as_deref(),
        )
        .unwrap();
        assert_eq!(saved_codex.policy, ReviewerPolicy::CodexOnly);
        assert_eq!(saved_codex.schema_version, 6);
        assert_eq!(saved_codex.codex, codex_config.codex);
        assert_eq!(saved_codex.priority_reviewer, None);

        // Also test saving CodexPlusDeepseek with priority_reviewer
        let codex_plus = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexPlusDeepseek,
            plan_review_policy: Some(ReviewerPolicy::CodexPlusDeepseek),
            plan_priority_reviewer: Some(AntigravityPriorityReviewer::CodexCli),
            plan_reviewers: None,
            build_review_policy: Some(ReviewerPolicy::CodexPlusDeepseek),
            build_priority_reviewer: Some(AntigravityPriorityReviewer::CodexCli),
            build_reviewers: None,
            preset: None,
            codex: Some(CodexReviewerConfig {
                model: "gpt-6.1-terra".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            deepseek: Some(DeepSeekReviewerConfig {
                model: "deepseek-flash".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            priority_reviewer: Some(AntigravityPriorityReviewer::CodexCli),
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };

        let saved_plus = save_reviewer_policy_at(
            &policy_path,
            &codex_plus,
            temp_skills.path(),
            saved_codex.content_hash.as_deref(),
        )
        .unwrap();
        assert_eq!(saved_plus.policy, ReviewerPolicy::CodexPlusDeepseek);
        assert_eq!(saved_plus.schema_version, 6);
        assert_eq!(
            saved_plus.priority_reviewer,
            Some(AntigravityPriorityReviewer::CodexCli)
        );
        assert_eq!(saved_plus.unconfirmed_migration, None);

        let reloaded_plus = inspect_reviewer_policy_at(&policy_path).unwrap();
        assert_eq!(reloaded_plus, saved_plus);

        // Also test saving DeepseekOnly
        let deepseek_only_config = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::DeepseekOnly,
            plan_review_policy: Some(ReviewerPolicy::DeepseekOnly),
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: Some(ReviewerPolicy::DeepseekOnly),
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: None,
            codex: None,
            deepseek: Some(DeepSeekReviewerConfig {
                model: "deepseek-flash".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            priority_reviewer: None,
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };

        let saved_ds_only = save_reviewer_policy_at(
            &policy_path,
            &deepseek_only_config,
            temp_skills.path(),
            saved_plus.content_hash.as_deref(),
        )
        .unwrap();
        assert_eq!(saved_ds_only.policy, ReviewerPolicy::DeepseekOnly);
        assert_eq!(saved_ds_only.schema_version, 6);
        assert_eq!(saved_ds_only.codex, None);
        assert_eq!(saved_ds_only.deepseek, deepseek_only_config.deepseek);
        assert_eq!(saved_ds_only.priority_reviewer, None);

        let reloaded_ds_only = inspect_reviewer_policy_at(&policy_path).unwrap();
        assert_eq!(reloaded_ds_only, saved_ds_only);
    }

    #[test]
    fn save_policy_detects_concurrent_modification_conflict() {
        let temp_ide = tempfile::tempdir().unwrap();
        let temp_skills = tempfile::tempdir().unwrap();
        let policy_path = temp_ide.path().join("pov-reviewer-policy.json");

        for name in ["pov-plan-only", "pov-build-review", "pov-full-cycle"] {
            crate::install_antigravity_command_at(temp_skills.path(), name).unwrap();
        }

        let initial_config = ReviewerPolicyConfig::default();
        let initial = save_reviewer_policy_at(&policy_path, &initial_config, temp_skills.path(), None)
            .unwrap();
        let hash1 = initial.content_hash.expect("must have content hash");

        let codex_plus = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexPlusDeepseek,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: Some(ReviewerPreset::Fast),
            codex: Some(CodexReviewerConfig {
                model: "gpt-6-luna".to_string(),
                reasoning_effort: "medium".to_string(),
            }),
            deepseek: Some(DeepSeekReviewerConfig {
                model: "deepseek-flash".to_string(),
                reasoning_effort: "low".to_string(),
            }),
            priority_reviewer: Some(AntigravityPriorityReviewer::CodexCli),
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };

        let updated = save_reviewer_policy_at(
            &policy_path,
            &codex_plus,
            temp_skills.path(),
            Some(&hash1),
        )
        .unwrap();
        let hash2 = updated.content_hash.expect("must have updated content hash");
        assert_ne!(hash1, hash2);

        let codex_only = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexOnly,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: Some(ReviewerPreset::Fast),
            codex: Some(CodexReviewerConfig {
                model: "gpt-6-luna".to_string(),
                reasoning_effort: "medium".to_string(),
            }),
            deepseek: None,
            priority_reviewer: None,
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };

        let conflict_err = save_reviewer_policy_at(
            &policy_path,
            &codex_only,
            temp_skills.path(),
            Some(&hash1),
        )
        .unwrap_err();
        assert!(conflict_err.contains("Concurrent modification conflict"));

        let none_err = save_reviewer_policy_at(
            &policy_path,
            &codex_only,
            temp_skills.path(),
            None,
        )
        .unwrap_err();
        assert!(none_err.contains("Concurrent modification conflict"));

        std::fs::remove_file(&policy_path).unwrap();
        let deleted_err = save_reviewer_policy_at(
            &policy_path,
            &codex_only,
            temp_skills.path(),
            Some(&hash2),
        )
        .unwrap_err();
        assert!(deleted_err.contains("does not exist on disk, but expected hash was provided"));
    }

    #[test]
    fn save_policy_detects_intervening_modification_between_check_and_replacement() {
        let temp_ide = tempfile::tempdir().unwrap();
        let temp_skills = tempfile::tempdir().unwrap();
        let policy_path = temp_ide.path().join("pov-reviewer-policy.json");

        for name in ["pov-plan-only", "pov-build-review", "pov-full-cycle"] {
            crate::install_antigravity_command_at(temp_skills.path(), name).unwrap();
        }

        let initial_config = ReviewerPolicyConfig::default();
        let initial = save_reviewer_policy_at(&policy_path, &initial_config, temp_skills.path(), None)
            .unwrap();
        let initial_hash = initial.content_hash.unwrap();

        let intervening_content = r#"{"schema_version":6,"policy":"none","tampered":true}"#;

        let codex_plus = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexPlusDeepseek,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: Some(ReviewerPreset::Fast),
            codex: Some(CodexReviewerConfig {
                model: "gpt-6-luna".to_string(),
                reasoning_effort: "medium".to_string(),
            }),
            deepseek: Some(DeepSeekReviewerConfig {
                model: "deepseek-flash".to_string(),
                reasoning_effort: "low".to_string(),
            }),
            priority_reviewer: Some(AntigravityPriorityReviewer::Deepseek),
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };

        let err = save_reviewer_policy_internal(
            &policy_path,
            &codex_plus,
            temp_skills.path(),
            Some(&initial_hash),
            || {
                std::fs::write(&policy_path, intervening_content).unwrap();
                Ok(())
            },
        )
        .unwrap_err();

        assert!(err.contains("Concurrent modification conflict"));
        assert_eq!(std::fs::read_to_string(&policy_path).unwrap(), intervening_content);
    }

    #[test]
    fn save_policy_detects_intervening_creation_between_check_and_replacement() {
        let temp_ide = tempfile::tempdir().unwrap();
        let temp_skills = tempfile::tempdir().unwrap();
        let policy_path = temp_ide.path().join("pov-reviewer-policy.json");

        for name in ["pov-plan-only", "pov-build-review", "pov-full-cycle"] {
            crate::install_antigravity_command_at(temp_skills.path(), name).unwrap();
        }

        let created_concurrently = r#"{"schema_version":5,"policy":"none","created":true}"#;

        let none_config = ReviewerPolicyConfig::default();
        let err = save_reviewer_policy_internal(
            &policy_path,
            &none_config,
            temp_skills.path(),
            None,
            || {
                std::fs::write(&policy_path, created_concurrently).unwrap();
                Ok(())
            },
        )
        .unwrap_err();

        assert!(err.contains("Concurrent modification conflict"));
        assert_eq!(std::fs::read_to_string(&policy_path).unwrap(), created_concurrently);
    }

    #[test]
    fn schema_v4_ignores_legacy_preset_on_read_and_omits_on_save() {
        let temp_ide = tempfile::tempdir().unwrap();
        let temp_skills = tempfile::tempdir().unwrap();
        let policy_path = temp_ide.path().join("pov-reviewer-policy.json");

        for name in ["pov-plan-only", "pov-build-review", "pov-full-cycle"] {
            crate::install_antigravity_command_at(temp_skills.path(), name).unwrap();
        }

        // 1. Write a legacy JSON that contains preset
        let legacy_json = r#"{
  "schema_version": 4,
  "policy": "codex_only",
  "preset": "standard",
  "codex": {
    "model": "gpt-6.1-terra",
    "reasoning_effort": "high"
  }
}"#;
        std::fs::write(&policy_path, legacy_json).unwrap();

        // 2. Read it back via inspect_reviewer_policy_at
        let loaded = inspect_reviewer_policy_at(&policy_path).unwrap();
        assert_eq!(loaded.policy, ReviewerPolicy::CodexOnly);
        assert_eq!(loaded.preset, None); // Preset must be ignored on load per R2-C01

        // 3. Save it back
        let saved = save_reviewer_policy_at(
            &policy_path,
            &loaded,
            temp_skills.path(),
            loaded.content_hash.as_deref(),
        )
        .unwrap();

        assert_eq!(saved.preset, None);

        // 4. Inspect raw file content on disk: "preset" key must NOT be present
        let saved_disk_content = std::fs::read_to_string(&policy_path).unwrap();
        assert!(!saved_disk_content.contains(r#""preset""#));
        assert!(saved_disk_content.contains(r#""policy": "codex_only""#));
        assert!(saved_disk_content.contains(r#""model": "gpt-6.1-terra""#));
    }

    #[test]
    fn schema_v5_migrates_to_v6_preserving_none_without_converting_to_antigravity() {
        let temp_ide = tempfile::tempdir().unwrap();
        let policy_path = temp_ide.path().join("pov-reviewer-policy.json");

        let v5_json = r#"{
  "schema_version": 5,
  "policy": "none",
  "active_plan_path": ".plan/V0.26.3.md",
  "planner_role": "antigravity",
  "reviser_role": "codex"
}"#;
        std::fs::write(&policy_path, v5_json).unwrap();

        let loaded = inspect_reviewer_policy_at(&policy_path).unwrap();
        assert_eq!(loaded.schema_version, 6);
        assert_eq!(loaded.policy, ReviewerPolicy::None);
        assert_eq!(loaded.plan_review_policy, Some(ReviewerPolicy::None));
        assert_eq!(loaded.build_review_policy, Some(ReviewerPolicy::None));
        assert_eq!(loaded.planner_role, Some("antigravity".to_string()));
        assert_eq!(loaded.reviser_role, Some("codex".to_string()));
        assert_eq!(loaded.active_plan_path, Some(".plan/V0.26.3.md".to_string()));
    }

    #[test]
    fn antigravity_policy_validates_and_round_trips() {
        let temp_ide = tempfile::tempdir().unwrap();
        let temp_skills = tempfile::tempdir().unwrap();
        let policy_path = temp_ide.path().join("pov-reviewer-policy.json");

        for name in ["pov-plan-only", "pov-build-review", "pov-full-cycle"] {
            crate::install_antigravity_command_at(temp_skills.path(), name).unwrap();
        }

        let config = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::Antigravity,
            plan_review_policy: Some(ReviewerPolicy::Antigravity),
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: Some(ReviewerPolicy::Antigravity),
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: None,
            codex: None,
            deepseek: None,
            priority_reviewer: None,
            unconfirmed_migration: None,
            active_plan_path: Some(".plan/V0.26.3.md".to_string()),
            planner_role: Some("antigravity".to_string()),
            reviser_role: Some("antigravity".to_string()),
            content_hash: None,
        };

        assert!(validate_reviewer_policy_config(&config).is_ok());

        let saved = save_reviewer_policy_at(&policy_path, &config, temp_skills.path(), None).unwrap();
        assert_eq!(saved.policy, ReviewerPolicy::Antigravity);
        assert_eq!(saved.plan_review_policy, Some(ReviewerPolicy::Antigravity));
        assert_eq!(saved.build_review_policy, Some(ReviewerPolicy::Antigravity));

        let reloaded = inspect_reviewer_policy_at(&policy_path).unwrap();
        assert_eq!(reloaded.policy, ReviewerPolicy::Antigravity);
        assert_eq!(reloaded.plan_review_policy, Some(ReviewerPolicy::Antigravity));
        assert_eq!(reloaded.build_review_policy, Some(ReviewerPolicy::Antigravity));
    }

    #[test]
    fn independent_plan_and_build_review_policies_round_trip() {
        let temp_ide = tempfile::tempdir().unwrap();
        let temp_skills = tempfile::tempdir().unwrap();
        let policy_path = temp_ide.path().join("pov-reviewer-policy.json");

        for name in ["pov-plan-only", "pov-build-review", "pov-full-cycle"] {
            crate::install_antigravity_command_at(temp_skills.path(), name).unwrap();
        }

        let config = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexOnly,
            plan_review_policy: Some(ReviewerPolicy::Antigravity),
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: Some(ReviewerPolicy::CodexOnly),
            build_priority_reviewer: None,
            build_reviewers: None,
            preset: None,
            codex: Some(CodexReviewerConfig {
                model: "gpt-6-luna".to_string(),
                reasoning_effort: "medium".to_string(),
            }),
            deepseek: None,
            priority_reviewer: None,
            unconfirmed_migration: None,
            active_plan_path: Some(".plan/V0.26.3.md".to_string()),
            planner_role: Some("antigravity".to_string()),
            reviser_role: Some("antigravity".to_string()),
            content_hash: None,
        };

        assert!(validate_reviewer_policy_config(&config).is_ok());

        let saved = save_reviewer_policy_at(&policy_path, &config, temp_skills.path(), None).unwrap();
        assert_eq!(saved.plan_review_policy, Some(ReviewerPolicy::Antigravity));
        assert_eq!(saved.build_review_policy, Some(ReviewerPolicy::CodexOnly));

        let reloaded = inspect_reviewer_policy_at(&policy_path).unwrap();
        assert_eq!(reloaded.plan_review_policy, Some(ReviewerPolicy::Antigravity));
        assert_eq!(reloaded.build_review_policy, Some(ReviewerPolicy::CodexOnly));
    }

    #[test]
    fn multi_select_reviewers_lifecycle_and_arbitration() {
        let temp_ide = tempfile::tempdir().unwrap();
        let temp_skills = tempfile::tempdir().unwrap();
        let policy_path = temp_ide.path().join("pov-reviewer-policy.json");

        for name in ["pov-plan-only", "pov-build-review", "pov-full-cycle"] {
            crate::install_antigravity_command_at(temp_skills.path(), name).unwrap();
        }

        // 1. Single selection: Antigravity only
        let config_antigravity = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::Antigravity,
            plan_review_policy: Some(ReviewerPolicy::Antigravity),
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: Some(ReviewerPolicy::Antigravity),
            build_priority_reviewer: None,
            build_reviewers: Some(vec!["antigravity".to_string()]),
            preset: None,
            codex: None,
            deepseek: None,
            priority_reviewer: None,
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };
        assert!(validate_reviewer_policy_config(&config_antigravity).is_ok());
        let saved_ag = save_reviewer_policy_at(&policy_path, &config_antigravity, temp_skills.path(), None).unwrap();
        assert_eq!(saved_ag.build_reviewers, Some(vec!["antigravity".to_string()]));
        assert_eq!(saved_ag.policy, ReviewerPolicy::Antigravity);

        // 2. Multi selection: Codex CLI + DeepSeek Harness (requires priority reviewer)
        let config_multi_external_missing_priority = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexPlusDeepseek,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: None,
            build_reviewers: Some(vec!["codex".to_string(), "deepseek".to_string()]),
            preset: None,
            codex: Some(CodexReviewerConfig {
                model: "gpt-6.1-terra".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            deepseek: Some(DeepSeekReviewerConfig {
                model: "deepseek-flash".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            priority_reviewer: None,
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };
        assert!(validate_reviewer_policy_config(&config_multi_external_missing_priority).is_err());

        // Multi selection with valid priority reviewer
        let mut config_multi_external = config_multi_external_missing_priority.clone();
        config_multi_external.priority_reviewer = Some(AntigravityPriorityReviewer::CodexCli);
        config_multi_external.build_priority_reviewer = Some(AntigravityPriorityReviewer::CodexCli);
        assert!(validate_reviewer_policy_config(&config_multi_external).is_ok());
        let saved_multi = save_reviewer_policy_at(
            &policy_path,
            &config_multi_external,
            temp_skills.path(),
            saved_ag.content_hash.as_deref(),
        )
        .unwrap();
        assert_eq!(saved_multi.build_reviewers, Some(vec!["codex".to_string(), "deepseek".to_string()]));
        assert_eq!(saved_multi.policy, ReviewerPolicy::CodexPlusDeepseek);

        // 3. Multi selection including Antigravity (Antigravity + Codex + DeepSeek)
        let config_all_three = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexPlusDeepseek,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: Some(AntigravityPriorityReviewer::Deepseek),
            build_reviewers: Some(vec![
                "antigravity".to_string(),
                "codex".to_string(),
                "deepseek".to_string(),
            ]),
            preset: None,
            codex: Some(CodexReviewerConfig {
                model: "gpt-6.1-terra".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            deepseek: Some(DeepSeekReviewerConfig {
                model: "deepseek-flash".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            priority_reviewer: Some(AntigravityPriorityReviewer::Deepseek),
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };
        assert!(validate_reviewer_policy_config(&config_all_three).is_ok());
        let saved_all = save_reviewer_policy_at(
            &policy_path,
            &config_all_three,
            temp_skills.path(),
            saved_multi.content_hash.as_deref(),
        )
        .unwrap();
        assert_eq!(
            saved_all.build_reviewers,
            Some(vec![
                "antigravity".to_string(),
                "codex".to_string(),
                "deepseek".to_string(),
            ])
        );

        // 4. Total unselect (全解除: build_reviewers is empty)
        let config_none = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::None,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: None,
            build_priority_reviewer: None,
            build_reviewers: Some(vec![]),
            preset: None,
            codex: None,
            deepseek: None,
            priority_reviewer: None,
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };
        assert!(validate_reviewer_policy_config(&config_none).is_ok());
        let saved_none = save_reviewer_policy_at(
            &policy_path,
            &config_none,
            temp_skills.path(),
            saved_all.content_hash.as_deref(),
        )
        .unwrap();
        assert_eq!(saved_none.build_reviewers, Some(vec![]));
        assert_eq!(saved_none.policy, ReviewerPolicy::None);

        // Reload from disk and verify empty list is preserved without converting to antigravity
        let reloaded_none = inspect_reviewer_policy_at(&policy_path).unwrap();
        assert_eq!(reloaded_none.build_reviewers, Some(vec![]));
        assert_eq!(reloaded_none.policy, ReviewerPolicy::None);

        // 5. Test legacy migration from v5 with codex_plus_deepseek
        let v5_multi_json = r#"{
  "schema_version": 5,
  "policy": "codex_plus_deepseek",
  "priority_reviewer": "codex_cli",
  "codex": { "model": "gpt-6.1-terra", "reasoning_effort": "high" },
  "deepseek": { "model": "deepseek-flash", "reasoning_effort": "high" }
}"#;
        let v5_path = temp_ide.path().join("v5-policy.json");
        std::fs::write(&v5_path, v5_multi_json).unwrap();
        let loaded_v5 = inspect_reviewer_policy_at(&v5_path).unwrap();
        assert_eq!(loaded_v5.build_reviewers, Some(vec!["codex".to_string(), "deepseek".to_string()]));
        assert_eq!(loaded_v5.policy, ReviewerPolicy::CodexPlusDeepseek);

        // 6. Test legacy migration from v5 with none
        let v5_none_json = r#"{
  "schema_version": 5,
  "policy": "none"
}"#;
        let v5_none_path = temp_ide.path().join("v5-none-policy.json");
        std::fs::write(&v5_none_path, v5_none_json).unwrap();
        let loaded_v5_none = inspect_reviewer_policy_at(&v5_none_path).unwrap();
        assert_eq!(loaded_v5_none.build_reviewers, Some(vec![]));
        assert_eq!(loaded_v5_none.policy, ReviewerPolicy::None);
    }

    #[test]
    fn plan_reviewers_multi_select_and_build_reviewers_inheritance() {
        let temp_ide = tempfile::tempdir().unwrap();
        let temp_skills = tempfile::tempdir().unwrap();
        let policy_path = temp_ide.path().join("pov-reviewer-policy.json");

        for name in ["pov-plan-only", "pov-build-review", "pov-full-cycle"] {
            crate::install_antigravity_command_at(temp_skills.path(), name).unwrap();
        }

        // 1. Inherit mode: plan_reviewers is None -> inherits from build_reviewers
        let config_inherit = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexPlusDeepseek,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: None,
            build_review_policy: Some(ReviewerPolicy::CodexPlusDeepseek),
            build_priority_reviewer: Some(AntigravityPriorityReviewer::CodexCli),
            build_reviewers: Some(vec!["codex".to_string(), "deepseek".to_string()]),
            preset: None,
            codex: Some(CodexReviewerConfig {
                model: "gpt-6.1-terra".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            deepseek: Some(DeepSeekReviewerConfig {
                model: "deepseek-flash".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            priority_reviewer: Some(AntigravityPriorityReviewer::CodexCli),
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };

        assert!(validate_reviewer_policy_config(&config_inherit).is_ok());
        let saved_inherit = save_reviewer_policy_at(&policy_path, &config_inherit, temp_skills.path(), None).unwrap();
        assert_eq!(saved_inherit.plan_reviewers, None);
        assert_eq!(saved_inherit.plan_review_policy, Some(ReviewerPolicy::CodexPlusDeepseek));

        let reloaded_inherit = inspect_reviewer_policy_at(&policy_path).unwrap();
        assert_eq!(reloaded_inherit.plan_reviewers, None);
        assert_eq!(reloaded_inherit.plan_review_policy, Some(ReviewerPolicy::CodexPlusDeepseek));

        // 2. Individual mode: plan_reviewers set to antigravity only while build_reviewers is codex+deepseek
        let config_individual = ReviewerPolicyConfig {
            schema_version: 6,
            policy: ReviewerPolicy::CodexPlusDeepseek,
            plan_review_policy: None,
            plan_priority_reviewer: None,
            plan_reviewers: Some(vec!["antigravity".to_string()]),
            build_review_policy: Some(ReviewerPolicy::CodexPlusDeepseek),
            build_priority_reviewer: Some(AntigravityPriorityReviewer::CodexCli),
            build_reviewers: Some(vec!["codex".to_string(), "deepseek".to_string()]),
            preset: None,
            codex: Some(CodexReviewerConfig {
                model: "gpt-6.1-terra".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            deepseek: Some(DeepSeekReviewerConfig {
                model: "deepseek-flash".to_string(),
                reasoning_effort: "high".to_string(),
            }),
            priority_reviewer: Some(AntigravityPriorityReviewer::CodexCli),
            unconfirmed_migration: None,
            active_plan_path: None,
            planner_role: None,
            reviser_role: None,
            content_hash: None,
        };

        assert!(validate_reviewer_policy_config(&config_individual).is_ok());
        let saved_indiv = save_reviewer_policy_at(
            &policy_path,
            &config_individual,
            temp_skills.path(),
            saved_inherit.content_hash.as_deref(),
        )
        .unwrap();
        assert_eq!(saved_indiv.plan_reviewers, Some(vec!["antigravity".to_string()]));
        assert_eq!(saved_indiv.plan_review_policy, Some(ReviewerPolicy::Antigravity));
        assert_eq!(saved_indiv.build_review_policy, Some(ReviewerPolicy::CodexPlusDeepseek));

        let reloaded_indiv = inspect_reviewer_policy_at(&policy_path).unwrap();
        assert_eq!(reloaded_indiv.plan_reviewers, Some(vec!["antigravity".to_string()]));
        assert_eq!(reloaded_indiv.plan_review_policy, Some(ReviewerPolicy::Antigravity));
        assert_eq!(reloaded_indiv.build_review_policy, Some(ReviewerPolicy::CodexPlusDeepseek));
    }
}
