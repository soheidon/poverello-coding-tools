use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const MAX_EXPANDED_BYTES: usize = 24_000;
pub const MANAGED_BEGIN_MARKER: &str = "<!-- pov-direct:managed:begin -->";
pub const MANAGED_END_MARKER: &str = "<!-- pov-direct:managed:end -->";
const METADATA_V1_PREFIX: &str = "<!-- pov-direct:settings:v1 ";

static MUTATION_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebResearchPolicy {
    RequestedOnly,
    RequestedOrNecessary,
    Unrestricted,
}

impl Default for WebResearchPolicy {
    fn default() -> Self {
        Self::RequestedOrNecessary
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseVerbosityPolicy {
    Minimal,
    Concise,
    Standard,
    Detailed,
}

impl Default for ResponseVerbosityPolicy {
    fn default() -> Self {
        Self::Concise
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExplorationPolicy {
    Minimal,
    Standard,
    Detailed,
}

impl Default for ExplorationPolicy {
    fn default() -> Self {
        Self::Minimal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationPolicy {
    Focused,
    Standard,
    Broad,
}

impl Default for VerificationPolicy {
    fn default() -> Self {
        Self::Focused
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PovDirectSettings {
    pub auto_activation: bool,
    pub web_research: WebResearchPolicy,
    pub response_verbosity: ResponseVerbosityPolicy,
    pub repository_exploration: ExplorationPolicy,
    pub verification: VerificationPolicy,
}

impl Default for PovDirectSettings {
    fn default() -> Self {
        Self {
            auto_activation: true,
            web_research: WebResearchPolicy::default(),
            response_verbosity: ResponseVerbosityPolicy::default(),
            repository_exploration: ExplorationPolicy::default(),
            verification: VerificationPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EmbeddedSettingsV1 {
    pub(crate) web_research: WebResearchPolicy,
    pub(crate) response_verbosity: ResponseVerbosityPolicy,
    pub(crate) repository_exploration: ExplorationPolicy,
    pub(crate) verification: VerificationPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PovDirectFileState {
    Absent,
    ManagedSynced,
    ManagedDrifted,
    LegacyPoverello,
    UnmanagedCustom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PovDirectApplyOperation {
    Apply,
    MigrateWithBackup,
    AdoptWithBackup,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PovDirectApplyRequest {
    pub settings: PovDirectSettings,
    pub operation: PovDirectApplyOperation,
    pub expected_state: PovDirectFileState,
    pub expected_content_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PovDirectSettingsState {
    EmbeddedValid,
    DefaultsPending,
    LegacyRequiresImport,
    MetadataInvalid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PovDirectStatus {
    pub rule_path: String,
    pub skill_path: String,
    pub skill_present: bool,
    pub state: PovDirectFileState,
    pub settings_state: PovDirectSettingsState,
    pub settings: PovDirectSettings,
    pub preview_text: String,
    pub expanded_byte_size: usize,
    pub byte_limit: usize,
    pub can_apply: bool,
    pub available_operations: Vec<PovDirectApplyOperation>,
    pub block_reason: Option<String>,
    pub content_hash: String,
    pub legacy_sidecar_present: bool,
}

pub fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn render_preview_text(settings: &PovDirectSettings) -> String {
    let auto = if settings.auto_activation {
        "Automatic activation: Enabled (model_decision)"
    } else {
        "Automatic activation: Disabled (manual only: activated via @pov-direct mention; /pov-direct invokes canonical Skill only)"
    };

    let web = match settings.web_research {
        WebResearchPolicy::RequestedOnly => "Web research: When requested only",
        WebResearchPolicy::RequestedOrNecessary => "Web research: When requested or necessary",
        WebResearchPolicy::Unrestricted => "Web research: Unrestricted",
    };

    let verbosity = match settings.response_verbosity {
        ResponseVerbosityPolicy::Minimal => "Response verbosity: minimal",
        ResponseVerbosityPolicy::Concise => "Response verbosity: concise",
        ResponseVerbosityPolicy::Standard => "Response verbosity: standard",
        ResponseVerbosityPolicy::Detailed => "Response verbosity: detailed",
    };

    let exploration = match settings.repository_exploration {
        ExplorationPolicy::Minimal => "Repository exploration: minimal",
        ExplorationPolicy::Standard => "Repository exploration: standard",
        ExplorationPolicy::Detailed => "Repository exploration: detailed",
    };

    let verification = match settings.verification {
        VerificationPolicy::Focused => "Verification: focused",
        VerificationPolicy::Standard => "Verification: standard",
        VerificationPolicy::Broad => "Verification: broad",
    };

    format!("{auto}\n{web}\n{verbosity}\n{exploration}\n{verification}")
}

fn render_metadata_comment(settings: &PovDirectSettings) -> String {
    let embedded = EmbeddedSettingsV1 {
        web_research: settings.web_research,
        response_verbosity: settings.response_verbosity,
        repository_exploration: settings.repository_exploration,
        verification: settings.verification,
    };
    let json = serde_json::to_string(&embedded).unwrap_or_default();
    format!("{METADATA_V1_PREFIX}{json} -->")
}

pub fn render_managed_overrides_block(settings: &PovDirectSettings) -> String {
    let metadata_comment = render_metadata_comment(settings);

    let activation_line = if settings.auto_activation {
        "- Activation: Apply this rule automatically when the task is an implementation or bug-fix task, as judged by model decision."
    } else {
        "- Activation: Do NOT auto-apply this rule. It applies strictly when explicitly mentioned in context via `@pov-direct`. Invoking the /pov-direct Skill runs the canonical Skill and does not load this generated rule."
    };

    let web_line = match settings.web_research {
        WebResearchPolicy::RequestedOnly => {
            "- Web research: Perform web searches ONLY when the user explicitly requests a search. Do not search for implementation necessity."
        }
        WebResearchPolicy::RequestedOrNecessary => {
            "- Web research: Search only when requested or strictly necessary for implementation. Do not search to pad context or speculate."
        }
        WebResearchPolicy::Unrestricted => {
            "- Web research: Use normal judgment; search whenever it improves correctness. This overrides any restrictive search guidance in SKILL.md."
        }
    };

    let verbosity_line = match settings.response_verbosity {
        ResponseVerbosityPolicy::Minimal => {
            "- Response verbosity: Minimal. Lead with the result. No preamble, no restating the request, no summary of the summary, no optional next steps unless asked."
        }
        ResponseVerbosityPolicy::Concise => {
            "- Response verbosity: Concise. Result first, then only the reasoning or evidence strictly needed to justify it. Keep messages information-dense."
        }
        ResponseVerbosityPolicy::Standard => {
            "- Response verbosity: Standard. Concise by default, but explain non-obvious decisions, trade-offs, and any deviation from the request."
        }
        ResponseVerbosityPolicy::Detailed => {
            "- Response verbosity: Detailed. Explain approach, alternatives considered, and rationale in full. This overrides any brevity guidance in SKILL.md."
        }
    };

    let exploration_line = match settings.repository_exploration {
        ExplorationPolicy::Minimal => {
            "- Repository exploration: Minimal. Search and inspect only directly affected areas. Do not perform broad scans or read unrelated modules for context."
        }
        ExplorationPolicy::Standard => {
            "- Repository exploration: Standard. Inspect direct dependencies and immediate callers/callees of the code being changed."
        }
        ExplorationPolicy::Detailed => {
            "- Repository exploration: Detailed. Trace full call paths and related modules before changing code. This overrides any minimal-exploration guidance in SKILL.md."
        }
    };

    let verification_line = match settings.verification {
        VerificationPolicy::Focused => {
            "- Verification: Focused. Smallest meaningful verification only. Verify only the changed behaviour and direct dependents."
        }
        VerificationPolicy::Standard => {
            "- Verification: Standard. Verify the changed behaviour plus the affected module's existing tests."
        }
        VerificationPolicy::Broad => {
            "- Verification: Broad. Run the full relevant test suite and regression checks. This overrides any narrow-verification guidance in SKILL.md."
        }
    };

    format!(
        "{MANAGED_BEGIN_MARKER}\n\
{metadata_comment}\n\
## Poverello Direct Coding — Applied Overrides\n\n\
This section is applied in addition to and with precedence over the defaults in the pov-direct skill included above. Where this section and SKILL.md conflict, this section takes precedence.\n\
The `pov-direct:settings` comment above is Poverello UI metadata only, not an instruction.\n\n\
{activation_line}\n\
{web_line}\n\
{verbosity_line}\n\
{exploration_line}\n\
{verification_line}\n\
{MANAGED_END_MARKER}"
    )
}

pub fn render_rule_file(settings: &PovDirectSettings, include_path_str: &str) -> String {
    let trigger_val = if settings.auto_activation { "model_decision" } else { "manual" };
    let trigger_frontmatter = format!(
        "---\n\
trigger: {trigger_val}\n\
description: >-\n  \
  Apply Poverello Direct Coding to straightforward software implementation, localized bug fixes, and narrowly scoped reviewer-requested corrections.\n  \
  Do not activate for read-only investigations, reviews, research, architecture discussions, or planning-only tasks.\n  \
  Explicit user requests for detailed planning or external planner workflows take precedence.\n\
---"
    );

    let overrides = render_managed_overrides_block(settings);

    format!(
        "{trigger_frontmatter}\n\n\
# Poverello Direct Coding\n\n\
@[pov-direct]({include_path_str})\n\n\
{overrides}\n"
    )
}

/// Strip YAML frontmatter from canonical skill if present (per Antigravity docs/rules.md)
pub fn strip_frontmatter(content: &str) -> &str {
    if content.starts_with("---\n") || content.starts_with("---\r\n") {
        let after_first = if content.starts_with("---\r\n") { &content[5..] } else { &content[4..] };
        if let Some(pos) = after_first.find("\n---\n") {
            return &after_first[pos + 5..];
        } else if let Some(pos) = after_first.find("\r\n---\r\n") {
            return &after_first[pos + 7..];
        } else if let Some(pos) = after_first.find("\n---\r\n") {
            return &after_first[pos + 6..];
        }
    }
    content
}

pub fn render_expanded_rule_content(
    settings: &PovDirectSettings,
    canonical_skill_content: &str,
) -> String {
    let body_only = strip_frontmatter(canonical_skill_content);
    let trigger_val = if settings.auto_activation { "model_decision" } else { "manual" };
    let trigger_frontmatter = format!(
        "---\n\
trigger: {trigger_val}\n\
description: >-\n  \
  Apply Poverello Direct Coding to straightforward software implementation, localized bug fixes, and narrowly scoped reviewer-requested corrections.\n  \
  Do not activate for read-only investigations, reviews, research, architecture discussions, or planning-only tasks.\n  \
  Explicit user requests for detailed planning or external planner workflows take precedence.\n\
---"
    );

    let overrides = render_managed_overrides_block(settings);

    format!(
        "{trigger_frontmatter}\n\n\
# Poverello Direct Coding\n\n\
{body_only}\n\n\
{overrides}\n"
    )
}

/// Information extracted about frontmatter structure
#[derive(Debug, PartialEq, Eq)]
pub struct FrontmatterParse {
    pub start: usize,
    pub end: usize,
    pub trigger_value: String,
    pub trigger_value_start: usize,
    pub trigger_value_end: usize,
}

pub fn parse_strict_frontmatter(content: &str) -> Result<FrontmatterParse, String> {
    if !(content.starts_with("---\n") || content.starts_with("---\r\n")) {
        return Err("Missing opening frontmatter delimiter".to_string());
    }

    let search_start = if content.starts_with("---\r\n") { 5 } else { 4 };
    let end_delim_match = if let Some(pos) = content[search_start..].find("\n---\n") {
        Some((search_start + pos + 1, search_start + pos + 5))
    } else if let Some(pos) = content[search_start..].find("\r\n---\r\n") {
        Some((search_start + pos + 2, search_start + pos + 7))
    } else if let Some(pos) = content[search_start..].find("\n---\r\n") {
        Some((search_start + pos + 1, search_start + pos + 6))
    } else {
        None
    };

    let (closing_start, closing_end) = end_delim_match
        .ok_or_else(|| "Missing closing frontmatter delimiter".to_string())?;

    let fm_text = &content[search_start..closing_start];

    // Reject duplicate delimiter inside frontmatter
    if fm_text.contains("---") {
        return Err("Ambiguous frontmatter: contains internal delimiter".to_string());
    }

    // Find trigger key strictly: "^trigger:\s*(\S+)" on its own line
    let mut trigger_matches = Vec::new();
    let mut current_offset = search_start;

    for line in fm_text.split_inclusive('\n') {
        if line.starts_with("trigger:") {
            let after_key = &line[8..];
            let val_trimmed = after_key.trim();
            if !val_trimmed.is_empty() {
                let val_start_in_line = line.find(val_trimmed).unwrap();
                let v_start = current_offset + val_start_in_line;
                let v_end = v_start + val_trimmed.len();
                trigger_matches.push((val_trimmed.to_string(), v_start, v_end));
            }
        }
        current_offset += line.len();
    }

    if trigger_matches.is_empty() {
        return Err("Missing trigger key in frontmatter".to_string());
    }
    if trigger_matches.len() > 1 {
        return Err("Duplicate trigger keys in frontmatter".to_string());
    }

    let (t_val, t_start, t_end) = trigger_matches.remove(0);
    if t_val != "model_decision" && t_val != "manual" {
        return Err(format!("Invalid trigger value: {t_val}"));
    }

    Ok(FrontmatterParse {
        start: 0,
        end: closing_end,
        trigger_value: t_val,
        trigger_value_start: t_start,
        trigger_value_end: t_end,
    })
}

#[derive(Debug, PartialEq, Eq)]
pub struct ManagedBlockParse {
    pub block_start: usize,
    pub block_end: usize,
    pub metadata_json: Option<String>,
}

pub fn parse_managed_block(content: &str) -> Result<Option<ManagedBlockParse>, String> {
    let begin_count = content.matches(MANAGED_BEGIN_MARKER).count();
    let end_count = content.matches(MANAGED_END_MARKER).count();

    if begin_count == 0 && end_count == 0 {
        return Ok(None);
    }
    if begin_count != 1 || end_count != 1 {
        return Err(format!("Malformed markers: begin={begin_count}, end={end_count}"));
    }

    let b_pos = content.find(MANAGED_BEGIN_MARKER).unwrap();
    let e_pos = content.find(MANAGED_END_MARKER).unwrap();

    if e_pos <= b_pos {
        return Err("Reversed managed markers".to_string());
    }

    let block_end = e_pos + MANAGED_END_MARKER.len();
    let inner = &content[b_pos + MANAGED_BEGIN_MARKER.len()..e_pos];

    let metadata_json = if let Some(meta_idx) = inner.find(METADATA_V1_PREFIX) {
        let meta_start = meta_idx + METADATA_V1_PREFIX.len();
        if let Some(close_idx) = inner[meta_start..].find("-->") {
            Some(inner[meta_start..meta_start + close_idx].trim().to_string())
        } else {
            None
        }
    } else {
        None
    };

    Ok(Some(ManagedBlockParse {
        block_start: b_pos,
        block_end,
        metadata_json,
    }))
}

pub(crate) fn parse_metadata_v1(json_str: &str) -> Option<EmbeddedSettingsV1> {
    serde_json::from_str::<EmbeddedSettingsV1>(json_str).ok()
}

pub fn classify_rule_file(
    rule_path: &Path,
    _expected_synced_content: &str,
) -> (PovDirectFileState, PovDirectSettingsState, PovDirectSettings, String) {
    if !rule_path.exists() {
        return (
            PovDirectFileState::Absent,
            PovDirectSettingsState::DefaultsPending,
            PovDirectSettings::default(),
            "absent".to_string(),
        );
    }

    let Ok(content) = std::fs::read_to_string(rule_path) else {
        return (
            PovDirectFileState::UnmanagedCustom,
            PovDirectSettingsState::MetadataInvalid,
            PovDirectSettings::default(),
            String::new(),
        );
    };

    let content_hash = hash_content(&content);

    let fm_parse = parse_strict_frontmatter(&content);
    let managed_parse = parse_managed_block(&content);

    // If frontmatter is invalid or managed block parsing errored (duplicate/reversed markers)
    let (fm, mb) = match (fm_parse, managed_parse) {
        (Ok(fm), Ok(mb)) => (fm, mb),
        _ => {
            return (
                PovDirectFileState::UnmanagedCustom,
                PovDirectSettingsState::MetadataInvalid,
                PovDirectSettings::default(),
                content_hash,
            );
        }
    };

    let auto_activation = fm.trigger_value == "model_decision";

    if let Some(block) = mb {
        if let Some(ref json_str) = block.metadata_json {
            if let Some(emb) = parse_metadata_v1(json_str) {
                let parsed_settings = PovDirectSettings {
                    auto_activation,
                    web_research: emb.web_research,
                    response_verbosity: emb.response_verbosity,
                    repository_exploration: emb.repository_exploration,
                    verification: emb.verification,
                };

                let expected_managed = render_managed_overrides_block(&parsed_settings);
                let actual_managed = &content[block.block_start..block.block_end];

                let trigger_matches = if parsed_settings.auto_activation {
                    fm.trigger_value == "model_decision"
                } else {
                    fm.trigger_value == "manual"
                };

                let state = if trigger_matches && actual_managed.trim() == expected_managed.trim() {
                    PovDirectFileState::ManagedSynced
                } else {
                    PovDirectFileState::ManagedDrifted
                };

                return (state, PovDirectSettingsState::EmbeddedValid, parsed_settings, content_hash);
            }
        }
        // Has markers but missing or invalid metadata
        let fallback_settings = PovDirectSettings {
            auto_activation,
            ..Default::default()
        };
        (
            PovDirectFileState::ManagedDrifted,
            PovDirectSettingsState::MetadataInvalid,
            fallback_settings,
            content_hash,
        )
    } else {
        // No markers
        let has_include = content.contains("@[pov-direct]");
        let heading_count = content.matches("# Poverello Direct Coding").count();
        let include_count = content.matches("@[pov-direct]").count();

        if has_include && heading_count == 1 && include_count == 1 {
            // Recognized Legacy shape
            let legacy_settings = PovDirectSettings {
                auto_activation,
                ..Default::default()
            };
            (
                PovDirectFileState::LegacyPoverello,
                PovDirectSettingsState::DefaultsPending,
                legacy_settings,
                content_hash,
            )
        } else {
            (
                PovDirectFileState::UnmanagedCustom,
                PovDirectSettingsState::DefaultsPending,
                PovDirectSettings::default(),
                content_hash,
            )
        }
    }
}

fn replace_file_atomically(temp: &Path, target: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "Kernel32")]
        unsafe extern "system" {
            fn ReplaceFileW(
                replaced_file_name: *const u16,
                replacement_file_name: *const u16,
                backup_file_name: *const u16,
                replace_flags: u32,
                exclude: *mut std::ffi::c_void,
                reserved: *mut std::ffi::c_void,
            ) -> i32;
        }

        if !target.exists() {
            return std::fs::rename(temp, target);
        }

        let target_wide = target.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
        let temp_wide = temp.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
        let result = unsafe {
            ReplaceFileW(
                target_wide.as_ptr(),
                temp_wide.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if result == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(temp, target)
    }
}

pub fn create_backup_unpruned(
    rule_path: &Path,
    backup_dir: &Path,
) -> Result<PathBuf, String> {
    if !rule_path.exists() {
        return Err("Cannot backup non-existent rule file".to_string());
    }

    std::fs::create_dir_all(backup_dir)
        .map_err(|e| format!("Failed to create backup directory: {e}"))?;

    let original_bytes = std::fs::read(rule_path)
        .map_err(|e| format!("Failed to read source file for backup: {e}"))?;
    let original_hash = {
        let mut hasher = Sha256::new();
        hasher.update(&original_bytes);
        format!("{:x}", hasher.finalize())
    };

    use std::io::Write;
    let mut last_err = None;
    for _ in 0..10 {
        let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let random_suffix: u32 = rand::random();
        let filename = format!("pov-direct.{timestamp}.{random_suffix:08x}.md");
        let target = backup_dir.join(filename);

        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut file) => {
                if let Err(e) = file.write_all(&original_bytes) {
                    let _ = std::fs::remove_file(&target);
                    return Err(format!("Failed to write backup contents: {e}"));
                }
                if let Err(e) = file.flush() {
                    let _ = std::fs::remove_file(&target);
                    return Err(format!("Failed to flush backup file: {e}"));
                }
                drop(file);

                // Read back and assert exact byte and hash equality
                let read_back_bytes = std::fs::read(&target)
                    .map_err(|e| format!("Failed to read back backup copy: {e}"))?;
                let read_back_hash = {
                    let mut hasher = Sha256::new();
                    hasher.update(&read_back_bytes);
                    format!("{:x}", hasher.finalize())
                };

                if read_back_bytes != original_bytes || read_back_hash != original_hash {
                    let _ = std::fs::remove_file(&target);
                    return Err("Backup read-back verification failed: byte or hash mismatch with source.".to_string());
                }

                return Ok(target);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                last_err = Some(e);
                continue;
            }
            Err(e) => {
                return Err(format!("Failed to create exclusive backup file: {e}"));
            }
        }
    }

    Err(format!(
        "BackupNameCollision: Failed to allocate collision-free backup file name after 10 attempts: {:?}",
        last_err
    ))
}

pub fn inspect_pov_direct_status_at(
    rules_dir: &Path,
    skill_file: &Path,
) -> Result<PovDirectStatus, String> {
    let rule_file = rules_dir.join("pov-direct.md");

    let skill_present = skill_file.exists() && skill_file.is_file();
    let canonical_skill_content = if skill_present {
        std::fs::read_to_string(skill_file).unwrap_or_default()
    } else {
        String::new()
    };

    // Preliminary classification to get current settings
    let portable_include_str = "~/.gemini/config/skills/pov-direct/SKILL.md";
    let dummy_expected = render_rule_file(&PovDirectSettings::default(), portable_include_str);
    let (_temp_state, _temp_meta_state, current_settings, content_hash) =
        classify_rule_file(&rule_file, &dummy_expected);

    // Exact expected content for currently loaded settings
    let expected_for_current = render_rule_file(&current_settings, portable_include_str);
    let (state, settings_state, settings, _) =
        classify_rule_file(&rule_file, &expected_for_current);

    let expanded_content = render_expanded_rule_content(&settings, &canonical_skill_content);
    let expanded_byte_size = expanded_content.as_bytes().len();

    let legacy_sidecar_path = rules_dir.join("pov-direct.settings.json");
    let legacy_sidecar_present = legacy_sidecar_path.exists() && legacy_sidecar_path.is_file();

    let settings_state = if legacy_sidecar_present && settings_state == PovDirectSettingsState::DefaultsPending {
        PovDirectSettingsState::LegacyRequiresImport
    } else {
        settings_state
    };

    let mut can_apply = true;
    let mut block_reason = None;

    if !skill_present {
        can_apply = false;
        block_reason = Some("Canonical pov-direct/SKILL.md not found. Skill must be installed first.".to_string());
    } else if expanded_byte_size > MAX_EXPANDED_BYTES {
        can_apply = false;
        block_reason = Some(format!(
            "Expanded rule size ({expanded_byte_size} bytes) exceeds Antigravity limit of {MAX_EXPANDED_BYTES} bytes."
        ));
    } else if settings_state == PovDirectSettingsState::MetadataInvalid {
        can_apply = false;
        block_reason = Some("Embedded metadata in rule file is malformed, missing, or has invalid schema. Ordinary Apply is disabled.".to_string());
    } else if legacy_sidecar_present {
        can_apply = false;
        block_reason = Some("Legacy pov-direct.settings.json detected. Automatic migration is unsupported in this version. Operations are disabled to prevent settings loss.".to_string());
    }

    let available_operations = if settings_state == PovDirectSettingsState::MetadataInvalid || legacy_sidecar_present {
        vec![]
    } else {
        match state {
            PovDirectFileState::Absent => vec![PovDirectApplyOperation::Apply],
            PovDirectFileState::ManagedSynced | PovDirectFileState::ManagedDrifted => {
                vec![PovDirectApplyOperation::Apply]
            }
            PovDirectFileState::LegacyPoverello => vec![PovDirectApplyOperation::MigrateWithBackup],
            PovDirectFileState::UnmanagedCustom => {
                // AdoptWithBackup is only available if frontmatter has valid trigger and file has NO corrupt/malformed managed markers
                let Ok(content) = std::fs::read_to_string(&rule_file) else {
                    return Err("Failed to read rule file for operation check".to_string());
                };
                let has_any_marker = content.contains(MANAGED_BEGIN_MARKER) || content.contains(MANAGED_END_MARKER);
                let fm_ok = parse_strict_frontmatter(&content).is_ok();
                if !has_any_marker && fm_ok {
                    vec![PovDirectApplyOperation::AdoptWithBackup]
                } else {
                    vec![]
                }
            }
        }
    };

    if available_operations.is_empty() && can_apply {
        can_apply = false;
        if block_reason.is_none() {
            block_reason = Some("File structure cannot be safely managed or adopted. Mutation is disabled.".to_string());
        }
    }

    let preview_text = render_preview_text(&settings);

    Ok(PovDirectStatus {
        rule_path: rule_file.to_string_lossy().to_string(),
        skill_path: skill_file.to_string_lossy().to_string(),
        skill_present,
        state,
        settings_state,
        settings,
        preview_text,
        expanded_byte_size,
        byte_limit: MAX_EXPANDED_BYTES,
        can_apply,
        available_operations,
        block_reason,
        content_hash,
        legacy_sidecar_present,
    })
}

pub fn save_pov_direct_settings_at(
    rules_dir: &Path,
    skill_file: &Path,
    request: PovDirectApplyRequest,
) -> Result<PovDirectStatus, String> {
    save_pov_direct_settings_at_internal(rules_dir, skill_file, request, None)
}

#[cfg(test)]
pub(crate) fn save_pov_direct_settings_at_with_pre_replace_hook<F>(
    rules_dir: &Path,
    skill_file: &Path,
    request: PovDirectApplyRequest,
    pre_replace_hook: F,
) -> Result<PovDirectStatus, String>
where
    F: FnOnce(&Path) + 'static,
{
    save_pov_direct_settings_at_internal(rules_dir, skill_file, request, Some(Box::new(pre_replace_hook)))
}

fn save_pov_direct_settings_at_internal(
    rules_dir: &Path,
    skill_file: &Path,
    request: PovDirectApplyRequest,
    #[allow(unused_mut)]
    mut pre_replace_hook: Option<Box<dyn FnOnce(&Path)>>,
) -> Result<PovDirectStatus, String> {
    let _lock = MUTATION_LOCK.lock().map_err(|e| format!("Mutex poisoned: {e}"))?;

    crate::paths::ensure_path_contained_in_dir(rules_dir, rules_dir)?;
    std::fs::create_dir_all(rules_dir)
        .map_err(|e| format!("Failed to create rules directory: {e}"))?;

    let rule_file = rules_dir.join("pov-direct.md");
    let backup_dir = rules_dir.join("backups");

    // 1. Verify Skill presence
    if !skill_file.exists() || !skill_file.is_file() {
        return Err("Cannot apply: Canonical pov-direct/SKILL.md does not exist.".to_string());
    }

    let canonical_skill_content = std::fs::read_to_string(skill_file)
        .map_err(|e| format!("Failed to read canonical SKILL.md: {e}"))?;

    // 2. Verify Expanded Size Guard
    let expanded_content = render_expanded_rule_content(&request.settings, &canonical_skill_content);
    let expanded_byte_size = expanded_content.as_bytes().len();
    if expanded_byte_size > MAX_EXPANDED_BYTES {
        return Err(format!(
            "Expanded rule size ({expanded_byte_size} bytes) exceeds Antigravity limit of {MAX_EXPANDED_BYTES} bytes."
        ));
    }

    // 3. Re-read and re-classify rule_file immediately before mutation
    let portable_include_str = "~/.gemini/config/skills/pov-direct/SKILL.md";
    let rendered_target = render_rule_file(&request.settings, portable_include_str);
    let (actual_state, actual_meta_state, _, actual_hash) = classify_rule_file(&rule_file, &rendered_target);

    // 4. Validate expected_state and expected_content_hash
    if actual_state != request.expected_state {
        return Err(format!(
            "StateMismatch: File state changed (expected: {:?}, actual: {:?}). Zero writes.",
            request.expected_state, actual_state
        ));
    }
    if actual_hash != request.expected_content_hash {
        return Err(format!(
            "HashMismatch: File hash changed (expected: '{}', actual: '{}'). Zero writes.",
            request.expected_content_hash, actual_hash
        ));
    }
    let legacy_sidecar_path = rules_dir.join("pov-direct.settings.json");
    if legacy_sidecar_path.exists() && legacy_sidecar_path.is_file() {
        return Err("Cannot apply: Legacy pov-direct.settings.json exists but migration is unsupported in this version. Operation blocked with zero writes to protect settings.".to_string());
    }

    if actual_meta_state == PovDirectSettingsState::MetadataInvalid {
        return Err("Cannot apply: Embedded metadata in rule file is malformed, missing, or invalid schema. Ordinary Apply is disabled. Zero writes.".to_string());
    }

    // 5. Validate operation / state pairing per §4.2 & §17.2
    match request.operation {
        PovDirectApplyOperation::Apply => {
            if actual_state != PovDirectFileState::Absent
                && actual_state != PovDirectFileState::ManagedSynced
                && actual_state != PovDirectFileState::ManagedDrifted
            {
                return Err(format!(
                    "InvalidOperation: 'apply' is only allowed for Absent or Managed states. Current state: {:?}. Zero writes.",
                    actual_state
                ));
            }
        }
        PovDirectApplyOperation::MigrateWithBackup => {
            if actual_state != PovDirectFileState::LegacyPoverello {
                return Err(format!(
                    "InvalidOperation: 'migrate_with_backup' is only allowed for LegacyPoverello. Current state: {:?}. Zero writes.",
                    actual_state
                ));
            }
        }
        PovDirectApplyOperation::AdoptWithBackup => {
            if actual_state != PovDirectFileState::UnmanagedCustom {
                return Err(format!(
                    "InvalidOperation: 'adopt_with_backup' is only allowed for UnmanagedCustom. Current state: {:?}. Zero writes.",
                    actual_state
                ));
            }
        }
    }

    // 6. Idempotent short-circuit: ManagedSynced with identical expected render -> 0 writes, 0 backups
    if actual_state == PovDirectFileState::ManagedSynced {
        if let Ok(existing) = std::fs::read_to_string(&rule_file) {
            if hash_content(&existing) == hash_content(&rendered_target) {
                return inspect_pov_direct_status_at(rules_dir, skill_file);
            }
        }
    }

    // 7. Render final target bytes strictly preserving non-owned regions
    let final_rule_bytes = match request.operation {
        PovDirectApplyOperation::Apply => {
            if actual_state == PovDirectFileState::Absent {
                rendered_target
            } else {
                let existing = std::fs::read_to_string(&rule_file)
                    .map_err(|e| format!("Failed to read rule file: {e}"))?;
                let fm = parse_strict_frontmatter(&existing)
                    .map_err(|e| format!("Strict frontmatter error: {e}"))?;
                let mb = parse_managed_block(&existing)
                    .map_err(|e| format!("Strict managed block error: {e}"))?
                    .ok_or_else(|| "Missing managed block in managed state".to_string())?;

                let new_trigger = if request.settings.auto_activation { "model_decision" } else { "manual" };
                let new_managed = render_managed_overrides_block(&request.settings);

                // Splice: replace only trigger value, replace only managed block
                let mut output = String::with_capacity(existing.len() + 500);
                output.push_str(&existing[..fm.trigger_value_start]);
                output.push_str(new_trigger);
                output.push_str(&existing[fm.trigger_value_end..mb.block_start]);
                output.push_str(&new_managed);
                output.push_str(&existing[mb.block_end..]);
                output
            }
        }
        PovDirectApplyOperation::MigrateWithBackup => {
            let existing = std::fs::read_to_string(&rule_file)
                .map_err(|e| format!("Failed to read rule file: {e}"))?;
            let fm = parse_strict_frontmatter(&existing)
                .map_err(|e| format!("Strict frontmatter error: {e}"))?;

            // Find existing include line
            let inc_idx = existing.find("@[pov-direct]")
                .ok_or_else(|| "Missing canonical @[pov-direct] include".to_string())?;
            let inc_end = existing[inc_idx..].find('\n')
                .map(|p| inc_idx + p + 1)
                .unwrap_or(existing.len());

            let new_trigger = if request.settings.auto_activation { "model_decision" } else { "manual" };
            let new_managed = render_managed_overrides_block(&request.settings);

            // Reuses existing include, replaces trigger scalar, inserts managed block adjacent to include
            let mut output = String::with_capacity(existing.len() + 500);
            output.push_str(&existing[..fm.trigger_value_start]);
            output.push_str(new_trigger);
            output.push_str(&existing[fm.trigger_value_end..inc_end]);
            output.push('\n');
            output.push_str(&new_managed);
            output.push_str(&existing[inc_end..]);
            output
        }
        PovDirectApplyOperation::AdoptWithBackup => {
            let existing = std::fs::read_to_string(&rule_file)
                .map_err(|e| format!("Failed to read rule file: {e}"))?;
            if existing.contains(MANAGED_BEGIN_MARKER) || existing.contains(MANAGED_END_MARKER) {
                return Err("Cannot adopt: file already contains managed markers. Safe insertion cannot be guaranteed. Zero writes.".to_string());
            }
            let fm = parse_strict_frontmatter(&existing)
                .map_err(|e| format!("Cannot adopt: file must have valid frontmatter with single trigger: {e}"))?;

            let new_trigger = if request.settings.auto_activation { "model_decision" } else { "manual" };
            let new_managed = render_managed_overrides_block(&request.settings);

            // Append managed block at end while preserving entire file and updating trigger scalar
            let mut output = String::with_capacity(existing.len() + 500);
            output.push_str(&existing[..fm.trigger_value_start]);
            output.push_str(new_trigger);
            output.push_str(&existing[fm.trigger_value_end..]);
            if !output.ends_with('\n') {
                output.push('\n');
            }
            output.push('\n');
            output.push_str(&new_managed);
            output.push('\n');
            output
        }
    };

    // 8. Create verified backup for existing files (NEVER prune or rotate)
    let backup_path = if rule_file.exists() {
        Some(create_backup_unpruned(&rule_file, &backup_dir)?)
    } else {
        None
    };

    // 9. Atomic write via sibling temp file
    let random_id: u32 = rand::random();
    let tmp_file = rules_dir.join(format!("pov-direct.tmp.{random_id:08x}"));
    std::fs::write(&tmp_file, &final_rule_bytes)
        .map_err(|e| format!("Failed to write temporary rule file: {e}"))?;

    // Pre-replacement re-check: verify rule_file has not been changed concurrently right before replace
    if let Some(hook) = pre_replace_hook.take() {
        hook(&rule_file);
    }

    // If the rule file existed at Apply start (backup_path is Some), it must still exist
    // AND still match the expected hash. Absence is a conflict; never recreate it.
    if backup_path.is_some() {
        let current_bytes = match std::fs::read_to_string(&rule_file) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let _ = std::fs::remove_file(&tmp_file);
                return Err("ConcurrentModificationConflict: Rule file was deleted externally immediately before replacement. Aborted with zero writes to rule file (not recreated; prior backup retained).".to_string());
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp_file);
                return Err(format!("Failed to re-check rule file before atomic replacement: {e}"));
            }
        };
        if hash_content(&current_bytes) != actual_hash {
            let _ = std::fs::remove_file(&tmp_file);
            return Err("ConcurrentModificationConflict: File changed externally immediately before replacement. Aborted with zero writes to rule file (prior backup retained).".to_string());
        }
    }

    if let Err(replace_err) = replace_file_atomically(&tmp_file, &rule_file) {
        let _ = std::fs::remove_file(&tmp_file);
        return Err(format!("Failed to atomically replace rule file: {replace_err}"));
    }

    // 10. Read back and verify
    let read_back_result = std::fs::read_to_string(&rule_file);
    verify_and_recover_post_replace(&rule_file, backup_path.as_deref(), &final_rule_bytes, read_back_result)?;

    inspect_pov_direct_status_at(rules_dir, skill_file)
}

pub(crate) fn verify_and_recover_post_replace(
    rule_file: &Path,
    backup_path: Option<&Path>,
    final_rule_bytes: &str,
    read_back_result: Result<String, std::io::Error>,
) -> Result<(), String> {
    let read_back_matches = match &read_back_result {
        Ok(read_back) => hash_content(read_back) == hash_content(final_rule_bytes),
        Err(_) => false,
    };

    if !read_back_matches {
        let current_target_result = std::fs::read_to_string(rule_file);
        let current_matches = match &current_target_result {
            Ok(content) => content == final_rule_bytes,
            Err(_) => false,
        };

        if let Some(bk) = backup_path {
            if current_matches {
                // Target has our exact written bytes; attempt restoration from backup
                let restore_res = std::fs::copy(bk, rule_file);
                let restored_bytes = std::fs::read_to_string(rule_file).ok();
                let bk_bytes = std::fs::read_to_string(bk).ok();
                if restore_res.is_ok() && restored_bytes.is_some() && restored_bytes == bk_bytes {
                    return Err("ReadBackVerificationFailed: Target verification failed after write. Rolled back from backup.".to_string());
                } else {
                    return Err("RecoveryRequired: Target verification failed and rollback from backup failed. Manual recovery required.".to_string());
                }
            } else {
                return Err("RecoveryRequired: Target verification failed and target modified externally or unreadable. Intervening file preserved without rollback.".to_string());
            }
        } else {
            // Target was originally absent
            if current_matches {
                let remove_res = std::fs::remove_file(rule_file);
                if remove_res.is_ok() && !rule_file.exists() {
                    return Err("ReadBackVerificationFailed: Target verification failed after write. Created file removed.".to_string());
                } else {
                    return Err("RecoveryRequired: Target verification failed and removal of created file failed. Manual recovery required.".to_string());
                }
            } else {
                return Err("RecoveryRequired: Target verification failed and created file modified externally or unreadable. Intervening file preserved.".to_string());
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_frontmatter_trigger_on_and_off_strict_ownership() {
        let on_settings = PovDirectSettings {
            auto_activation: true,
            ..Default::default()
        };
        let on_rendered = render_rule_file(&on_settings, "~/.gemini/config/skills/pov-direct/SKILL.md");
        let fm = parse_strict_frontmatter(&on_rendered).unwrap();
        assert_eq!(fm.trigger_value, "model_decision");

        let off_settings = PovDirectSettings {
            auto_activation: false,
            ..Default::default()
        };
        let off_rendered = render_rule_file(&off_settings, "~/.gemini/config/skills/pov-direct/SKILL.md");
        let fm_off = parse_strict_frontmatter(&off_rendered).unwrap();
        assert_eq!(fm_off.trigger_value, "manual");
    }

    #[test]
    fn test_strict_frontmatter_rejects_duplicates_and_malformed() {
        // Missing trigger
        let no_trigger = "---\ndescription: test\n---\n";
        assert!(parse_strict_frontmatter(no_trigger).is_err());

        // Duplicate trigger
        let dup_trigger = "---\ntrigger: manual\ntrigger: model_decision\n---\n";
        assert!(parse_strict_frontmatter(dup_trigger).is_err());

        // Invalid trigger
        let invalid_trigger = "---\ntrigger: invalid_mode\n---\n";
        assert!(parse_strict_frontmatter(invalid_trigger).is_err());
    }

    #[test]
    fn test_expanded_size_accounting_boundary() {
        let settings = PovDirectSettings::default();
        let tiny_skill = "---\nname: test\n---\nSkill content";
        let rendered_tiny = render_expanded_rule_content(&settings, tiny_skill);
        assert!(rendered_tiny.as_bytes().len() < MAX_EXPANDED_BYTES);

        let base_len = render_expanded_rule_content(&settings, "").as_bytes().len();
        let target_len = MAX_EXPANDED_BYTES - base_len;
        let exact_skill = "A".repeat(target_len);
        let rendered_exact = render_expanded_rule_content(&settings, &exact_skill);
        assert_eq!(rendered_exact.as_bytes().len(), MAX_EXPANDED_BYTES);

        let over_skill = "A".repeat(target_len + 1);
        let rendered_over = render_expanded_rule_content(&settings, &over_skill);
        assert_eq!(rendered_over.as_bytes().len(), 24_001);
    }

    #[test]
    fn test_single_file_state_machine_and_unpruned_backups() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().join("rules");
        let skills_dir = temp_dir.path().join("skills");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::create_dir_all(&skills_dir).unwrap();

        let skill_file = skills_dir.join("SKILL.md");
        std::fs::write(&skill_file, "Canonical skill content").unwrap();

        // 1. Absent
        let status = inspect_pov_direct_status_at(&rules_dir, &skill_file).unwrap();
        assert_eq!(status.state, PovDirectFileState::Absent);
        assert_eq!(status.available_operations, vec![PovDirectApplyOperation::Apply]);

        // 2. Apply on Absent
        let req = PovDirectApplyRequest {
            settings: PovDirectSettings::default(),
            operation: PovDirectApplyOperation::Apply,
            expected_state: PovDirectFileState::Absent,
            expected_content_hash: "absent".to_string(),
        };
        let res = save_pov_direct_settings_at(&rules_dir, &skill_file, req).unwrap();
        assert_eq!(res.state, PovDirectFileState::ManagedSynced);
        assert_eq!(res.settings_state, PovDirectSettingsState::EmbeddedValid);

        // Verify NO sidecar JSON exists
        assert!(!rules_dir.join("pov-direct.settings.json").exists());

        // 3. Stale hash mismatch -> 0 writes, Err
        let bad_hash_req = PovDirectApplyRequest {
            settings: PovDirectSettings::default(),
            operation: PovDirectApplyOperation::Apply,
            expected_state: PovDirectFileState::ManagedSynced,
            expected_content_hash: "stale_hash".to_string(),
        };
        assert!(save_pov_direct_settings_at(&rules_dir, &skill_file, bad_hash_req).is_err());

        // 4. Update setting -> Backup created, NO pruning
        let mut new_settings = PovDirectSettings::default();
        new_settings.response_verbosity = ResponseVerbosityPolicy::Detailed;
        let update_req = PovDirectApplyRequest {
            settings: new_settings,
            operation: PovDirectApplyOperation::Apply,
            expected_state: PovDirectFileState::ManagedSynced,
            expected_content_hash: res.content_hash.clone(),
        };
        let updated = save_pov_direct_settings_at(&rules_dir, &skill_file, update_req).unwrap();
        assert_eq!(updated.state, PovDirectFileState::ManagedSynced);
        assert_eq!(updated.settings.response_verbosity, ResponseVerbosityPolicy::Detailed);

        let backup_dir = rules_dir.join("backups");
        // 5. Test indefinite backup retention (create 15 backups, none pruned)
        for i in 0..15 {
            let status = inspect_pov_direct_status_at(&rules_dir, &skill_file).unwrap();
            let mut s = status.settings.clone();
            s.web_research = if i % 2 == 0 { WebResearchPolicy::RequestedOnly } else { WebResearchPolicy::Unrestricted };
            let r = PovDirectApplyRequest {
                settings: s,
                operation: PovDirectApplyOperation::Apply,
                expected_state: PovDirectFileState::ManagedSynced,
                expected_content_hash: status.content_hash,
            };
            save_pov_direct_settings_at(&rules_dir, &skill_file, r).unwrap();
        }
        let backup_count = std::fs::read_dir(&backup_dir).unwrap().count();
        assert!(backup_count >= 16, "Expected at least 16 backups, found {backup_count}");
    }

    #[test]
    fn test_legacy_rule_migration_reuses_skill_include() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().join("rules");
        let skills_dir = temp_dir.path().join("skills");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::create_dir_all(&skills_dir).unwrap();

        let skill_file = skills_dir.join("SKILL.md");
        std::fs::write(&skill_file, "Canonical skill content").unwrap();

        let rule_file = rules_dir.join("pov-direct.md");
        let legacy_content = "---\n\
trigger: model_decision\n\
description: >-\n  \
  Legacy description preserved strictly.\n\
---\n\n\
# Poverello Direct Coding\n\n\
@[pov-direct](~/.gemini/config/skills/pov-direct/SKILL.md)\n\n\
Extra custom user notes at the bottom.\n";
        std::fs::write(&rule_file, legacy_content).unwrap();

        let status = inspect_pov_direct_status_at(&rules_dir, &skill_file).unwrap();
        assert_eq!(status.state, PovDirectFileState::LegacyPoverello);
        assert_eq!(status.available_operations, vec![PovDirectApplyOperation::MigrateWithBackup]);

        let req = PovDirectApplyRequest {
            settings: PovDirectSettings::default(),
            operation: PovDirectApplyOperation::MigrateWithBackup,
            expected_state: PovDirectFileState::LegacyPoverello,
            expected_content_hash: status.content_hash,
        };

        let res = save_pov_direct_settings_at(&rules_dir, &skill_file, req).unwrap();
        assert_eq!(res.state, PovDirectFileState::ManagedSynced);

        let final_text = std::fs::read_to_string(&rule_file).unwrap();
        // Assert description preserved
        assert!(final_text.contains("Legacy description preserved strictly."));
        // Assert user notes at bottom preserved
        assert!(final_text.contains("Extra custom user notes at the bottom."));
        // Assert exactly one include
        assert_eq!(final_text.matches("@[pov-direct]").count(), 1);
        // Assert exactly one level 1 heading
        assert_eq!(
            final_text.lines().filter(|l| l.trim() == "# Poverello Direct Coding").count(),
            1
        );
    }

    #[test]
    fn test_invalid_metadata_disallows_apply_and_zero_writes() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().join("rules");
        let skills_dir = temp_dir.path().join("skills");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::create_dir_all(&skills_dir).unwrap();

        let skill_file = skills_dir.join("SKILL.md");
        std::fs::write(&skill_file, "Skill body").unwrap();

        let rule_file = rules_dir.join("pov-direct.md");
        // Rule file with managed markers but invalid metadata JSON
        let malformed = format!(
            "---\ntrigger: model_decision\n---\n{}\n<!-- pov-direct:settings:v1 {{invalid_json}} -->\n{}\n",
            MANAGED_BEGIN_MARKER, MANAGED_END_MARKER
        );
        std::fs::write(&rule_file, &malformed).unwrap();

        let status = inspect_pov_direct_status_at(&rules_dir, &skill_file).unwrap();
        assert_eq!(status.settings_state, PovDirectSettingsState::MetadataInvalid);
        assert!(!status.can_apply, "can_apply must be false for MetadataInvalid");
        assert!(status.available_operations.is_empty(), "available_operations must be empty");

        // Attempting to apply should be rejected with zero writes
        let req = PovDirectApplyRequest {
            settings: PovDirectSettings::default(),
            operation: PovDirectApplyOperation::Apply,
            expected_state: status.state,
            expected_content_hash: status.content_hash,
        };

        let err = save_pov_direct_settings_at(&rules_dir, &skill_file, req);
        assert!(err.is_err());
        // Verify original file is untouched
        let on_disk = std::fs::read_to_string(&rule_file).unwrap();
        assert_eq!(on_disk, malformed);
    }

    #[test]
    fn test_corrupt_or_partial_markers_reject_adopt_with_zero_writes() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().join("rules");
        let skills_dir = temp_dir.path().join("skills");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::create_dir_all(&skills_dir).unwrap();

        let skill_file = skills_dir.join("SKILL.md");
        std::fs::write(&skill_file, "Skill body").unwrap();

        let rule_file = rules_dir.join("pov-direct.md");
        // Single-sided marker: only begin marker
        let corrupt = format!("---\ntrigger: model_decision\n---\n{}\nCustom prose without end marker\n", MANAGED_BEGIN_MARKER);
        std::fs::write(&rule_file, &corrupt).unwrap();

        let status = inspect_pov_direct_status_at(&rules_dir, &skill_file).unwrap();
        assert_eq!(status.state, PovDirectFileState::UnmanagedCustom);
        assert!(status.available_operations.is_empty(), "Cannot offer AdoptWithBackup when markers are corrupted");
        assert!(!status.can_apply);

        // Attempting adopt_with_backup must be rejected
        let req = PovDirectApplyRequest {
            settings: PovDirectSettings::default(),
            operation: PovDirectApplyOperation::AdoptWithBackup,
            expected_state: status.state,
            expected_content_hash: status.content_hash,
        };
        let err = save_pov_direct_settings_at(&rules_dir, &skill_file, req);
        assert!(err.is_err());
        assert_eq!(std::fs::read_to_string(&rule_file).unwrap(), corrupt);
    }

    #[test]
    fn test_concurrent_modification_pre_replace_check_rejects() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().join("rules");
        let skills_dir = temp_dir.path().join("skills");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::create_dir_all(&skills_dir).unwrap();

        let skill_file = skills_dir.join("SKILL.md");
        std::fs::write(&skill_file, "Skill body").unwrap();

        let rule_file = rules_dir.join("pov-direct.md");
        let initial = render_rule_file(&PovDirectSettings::default(), "~/.gemini/config/skills/pov-direct/SKILL.md");
        std::fs::write(&rule_file, &initial).unwrap();

        let status = inspect_pov_direct_status_at(&rules_dir, &skill_file).unwrap();

        // 1. Initial HashMismatch before any backup creation
        let req = PovDirectApplyRequest {
            settings: PovDirectSettings {
                auto_activation: false,
                ..Default::default()
            },
            operation: PovDirectApplyOperation::Apply,
            expected_state: status.state,
            expected_content_hash: "stale_hash_value".to_string(),
        };

        let err = save_pov_direct_settings_at(&rules_dir, &skill_file, req);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("HashMismatch"));
        let backup_dir = rules_dir.join("backups");
        assert!(!backup_dir.exists() || std::fs::read_dir(&backup_dir).unwrap().count() == 0);
    }

    #[test]
    fn test_concurrent_modification_between_backup_and_replace_protects_external_change() {
        // Test step: save_pov_direct_settings_at runs, passes initial state/hash check, creates backup,
        // then right before pre-replacement re-check an external edit is injected.
        // The production pre-replacement check detects mismatch, aborts replacement with zero writes to rule file.
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().join("rules");
        let skills_dir = temp_dir.path().join("skills");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::create_dir_all(&skills_dir).unwrap();

        let skill_file = skills_dir.join("SKILL.md");
        std::fs::write(&skill_file, "Canonical skill content").unwrap();

        let rule_file = rules_dir.join("pov-direct.md");
        let initial_rule = render_rule_file(&PovDirectSettings::default(), "~/.gemini/config/skills/pov-direct/SKILL.md");
        std::fs::write(&rule_file, &initial_rule).unwrap();
        let initial_hash = hash_content(&initial_rule);

        let backup_dir = rules_dir.join("backups");
        assert!(!backup_dir.exists() || std::fs::read_dir(&backup_dir).unwrap().count() == 0);

        let external_content = "# External concurrent edit preserved byte-for-byte\n";
        let external_hash = hash_content(external_content);

        // Prepare valid apply request matching initial file
        let status = inspect_pov_direct_status_at(&rules_dir, &skill_file).unwrap();
        assert_eq!(status.content_hash, initial_hash);

        let req = PovDirectApplyRequest {
            settings: PovDirectSettings {
                auto_activation: false, // change setting so it's not short-circuited as identical
                ..Default::default()
            },
            operation: PovDirectApplyOperation::Apply,
            expected_state: status.state,
            expected_content_hash: initial_hash.clone(),
        };

        // Execute save_pov_direct_settings_at with the hook injecting external change right before replace check
        let save_res = save_pov_direct_settings_at_with_pre_replace_hook(
            &rules_dir,
            &skill_file,
            req,
            move |target_file| {
                // Injected failure point: after backup is created, right before pre-replacement hash check
                std::fs::write(target_file, external_content).unwrap();
            },
        );

        // 1. Assert: save operation returns ConcurrentModificationConflict error
        assert!(save_res.is_err());
        let err_msg = save_res.unwrap_err();
        assert!(err_msg.contains("ConcurrentModificationConflict"));
        assert!(err_msg.contains("zero writes to rule file (prior backup retained)"));

        // 2. Assert: rule file content is preserved exactly as the injected external content (NOT overwritten)
        let current_rule_on_disk = std::fs::read_to_string(&rule_file).unwrap();
        assert_eq!(current_rule_on_disk, external_content);
        assert_eq!(hash_content(&current_rule_on_disk), external_hash);

        // 3. Assert: rule file was NOT replaced with the new settings
        assert_ne!(current_rule_on_disk, initial_rule);
        assert!(!current_rule_on_disk.contains("trigger: manual"));

        // 4. Assert: backup count increased by exactly 1, and backup content matches original file prior to external modification
        let backup_entries = std::fs::read_dir(&backup_dir).unwrap().collect::<Vec<_>>();
        assert_eq!(backup_entries.len(), 1, "Exactly 1 backup should exist");
        let backup_entry = backup_entries[0].as_ref().unwrap();
        let backup_content = std::fs::read_to_string(backup_entry.path()).unwrap();
        assert_eq!(backup_content, initial_rule);
        assert_eq!(hash_content(&backup_content), initial_hash);
    }

    #[test]
    fn test_external_deletion_before_replace_is_conflict_and_not_recreated() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().join("rules");
        let skills_dir = temp_dir.path().join("skills");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::create_dir_all(&skills_dir).unwrap();

        let skill_file = skills_dir.join("SKILL.md");
        std::fs::write(&skill_file, "Canonical skill content").unwrap();

        let rule_file = rules_dir.join("pov-direct.md");
        let initial_rule = render_rule_file(&PovDirectSettings::default(), "~/.gemini/config/skills/pov-direct/SKILL.md");
        std::fs::write(&rule_file, &initial_rule).unwrap();
        let initial_hash = hash_content(&initial_rule);

        let status = inspect_pov_direct_status_at(&rules_dir, &skill_file).unwrap();
        let req = PovDirectApplyRequest {
            settings: PovDirectSettings {
                auto_activation: false,
                ..Default::default()
            },
            operation: PovDirectApplyOperation::Apply,
            expected_state: status.state,
            expected_content_hash: initial_hash.clone(),
        };

        // Inject external deletion after backup creation, before final check
        let res = save_pov_direct_settings_at_with_pre_replace_hook(
            &rules_dir,
            &skill_file,
            req,
            |target| std::fs::remove_file(target).unwrap(),
        );

        let err = res.unwrap_err();
        assert!(err.contains("ConcurrentModificationConflict"));
        assert!(err.contains("deleted externally"));

        // Not recreated => atomic replacement never ran
        assert!(!rule_file.exists());

        // No leftover temp file
        let leftovers = std::fs::read_dir(&rules_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("pov-direct.tmp."))
            .count();
        assert_eq!(leftovers, 0);

        // Exactly 1 backup, byte-identical to the pre-deletion rule
        let backups: Vec<_> = std::fs::read_dir(rules_dir.join("backups")).unwrap().collect();
        assert_eq!(backups.len(), 1);
        let backup_content = std::fs::read_to_string(backups[0].as_ref().unwrap().path()).unwrap();
        assert_eq!(backup_content, initial_rule);
        assert_eq!(hash_content(&backup_content), initial_hash);
    }

    #[test]
    fn test_legacy_sidecar_detection_read_only() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().join("rules");
        let skills_dir = temp_dir.path().join("skills");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::create_dir_all(&skills_dir).unwrap();

        let skill_file = skills_dir.join("SKILL.md");
        std::fs::write(&skill_file, "Skill body").unwrap();

        // Create legacy sidecar
        let sidecar_file = rules_dir.join("pov-direct.settings.json");
        let sidecar_content = r#"{"web_research":"unrestricted"}"#;
        std::fs::write(&sidecar_file, sidecar_content).unwrap();

        let status = inspect_pov_direct_status_at(&rules_dir, &skill_file).unwrap();
        assert!(status.legacy_sidecar_present);
        assert_eq!(status.settings_state, PovDirectSettingsState::LegacyRequiresImport);
        assert!(!status.can_apply);
        assert!(status.available_operations.is_empty());

        // Verify sidecar was untouched read-only
        assert_eq!(std::fs::read_to_string(&sidecar_file).unwrap(), sidecar_content);

        // Verify all 3 operations are rejected by save_pov_direct_settings_at
        let ops = [
            PovDirectApplyOperation::Apply,
            PovDirectApplyOperation::MigrateWithBackup,
            PovDirectApplyOperation::AdoptWithBackup,
        ];
        for op in ops {
            let req = PovDirectApplyRequest {
                settings: PovDirectSettings::default(),
                operation: op,
                expected_state: status.state,
                expected_content_hash: status.content_hash.clone(),
            };
            let err = save_pov_direct_settings_at(&rules_dir, &skill_file, req);
            assert!(err.is_err(), "Expected operation {op:?} to be rejected when sidecar exists");
            assert!(err.unwrap_err().contains("Legacy pov-direct.settings.json exists"));
        }
        // Assert sidecar file is still unchanged
        assert_eq!(std::fs::read_to_string(&sidecar_file).unwrap(), sidecar_content);
    }

    #[test]
    fn test_strict_frontmatter_only_matches_root_trigger() {
        // Indented trigger should not be matched as root trigger
        let indented = "---\n  trigger: manual\n---\n";
        assert!(parse_strict_frontmatter(indented).is_err());

        // Root trigger with indented trigger should fail as duplicate or invalid
        let nested = "---\ntrigger: manual\n  sub:\n    trigger: model_decision\n---\n";
        let parsed = parse_strict_frontmatter(nested).unwrap();
        assert_eq!(parsed.trigger_value, "manual");
    }

    #[test]
    fn test_failure_injection_read_back_error_restores_from_backup_and_verifies_hash() {
        // Case 1: Post-replacement read-back fails -> restored from backup -> hash matches original
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        let rule_file = rules_dir.join("pov-direct.md");
        let backup_dir = rules_dir.join("backups");
        std::fs::create_dir_all(&backup_dir).unwrap();

        let original_content = "---\ntrigger: manual\n---\n# Original content\n";
        let original_hash = hash_content(original_content);
        std::fs::write(&rule_file, original_content).unwrap();

        // Create verified backup
        let backup_path = create_backup_unpruned(&rule_file, &backup_dir).unwrap();
        assert_eq!(std::fs::read_to_string(&backup_path).unwrap(), original_content);

        // Now simulate replace wrote target bytes
        let final_rule_bytes = "---\ntrigger: model_decision\n---\n# Final written content\n";
        std::fs::write(&rule_file, final_rule_bytes).unwrap();

        // Inject simulated read-back I/O error
        let read_back_result = Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Simulated disk I/O read failure"));

        let res = verify_and_recover_post_replace(&rule_file, Some(&backup_path), final_rule_bytes, read_back_result);
        assert!(res.is_err());
        let err_msg = res.unwrap_err();
        assert!(err_msg.contains("ReadBackVerificationFailed"));
        assert!(err_msg.contains("Rolled back from backup"));

        // Verify rule_file was restored from backup and its hash equals original_hash
        let restored_content = std::fs::read_to_string(&rule_file).unwrap();
        assert_eq!(restored_content, original_content);
        assert_eq!(hash_content(&restored_content), original_hash);

        // Verify backup count unchanged (1 backup retained)
        assert_eq!(std::fs::read_dir(&backup_dir).unwrap().count(), 1);
    }

    #[test]
    fn test_failure_injection_read_back_error_with_failed_restore_reports_recovery_required() {
        // Case 2: Post-replacement read-back fails -> backup restore also fails -> returns RecoveryRequired
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        let rule_file = rules_dir.join("pov-direct.md");
        let final_rule_bytes = "---\ntrigger: model_decision\n---\n# Final written content\n";
        std::fs::write(&rule_file, final_rule_bytes).unwrap();

        // Non-existent or inaccessible backup path
        let invalid_backup_path = rules_dir.join("non_existent_backup.md");

        // Inject simulated read-back I/O error
        let read_back_result = Err(std::io::Error::new(std::io::ErrorKind::Other, "Simulated I/O read error"));

        let res = verify_and_recover_post_replace(&rule_file, Some(&invalid_backup_path), final_rule_bytes, read_back_result);
        assert!(res.is_err());
        let err_msg = res.unwrap_err();
        assert!(err_msg.contains("RecoveryRequired"));
        assert!(err_msg.contains("rollback from backup failed"));
        assert!(!err_msg.contains("Rolled back"));
    }

    #[test]
    fn test_failure_injection_hash_mismatch_preserves_external_modification() {
        // Case 3: Post-replacement read-back hash mismatch due to external modification -> preserves intervening file without rollback
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        let rule_file = rules_dir.join("pov-direct.md");
        let backup_dir = rules_dir.join("backups");
        std::fs::create_dir_all(&backup_dir).unwrap();

        let original_content = "---\ntrigger: manual\n---\n# Original content\n";
        std::fs::write(&rule_file, original_content).unwrap();
        let backup_path = create_backup_unpruned(&rule_file, &backup_dir).unwrap();

        let final_rule_bytes = "---\ntrigger: model_decision\n---\n# Final written content\n";

        // External process concurrently modified target file to custom content!
        let external_intervening_content = "---\ntrigger: manual\n---\n# Intervening external modification\n";
        std::fs::write(&rule_file, external_intervening_content).unwrap();
        let external_hash = hash_content(external_intervening_content);

        // Read back reads the intervening content
        let read_back_result = Ok(external_intervening_content.to_string());

        let res = verify_and_recover_post_replace(&rule_file, Some(&backup_path), final_rule_bytes, read_back_result);
        assert!(res.is_err());
        let err_msg = res.unwrap_err();
        assert!(err_msg.contains("RecoveryRequired"));
        assert!(err_msg.contains("target modified externally"));
        assert!(err_msg.contains("Intervening file preserved without rollback"));

        // Crucial: rule_file MUST NOT have been overwritten by backup; external modification preserved
        let current_on_disk = std::fs::read_to_string(&rule_file).unwrap();
        assert_eq!(current_on_disk, external_intervening_content);
        assert_eq!(hash_content(&current_on_disk), external_hash);
        assert_ne!(current_on_disk, original_content);
    }

    #[test]
    fn test_normal_apply_succeeds_when_no_failure_injected() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().join("rules");
        let skills_dir = temp_dir.path().join("skills");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::create_dir_all(&skills_dir).unwrap();

        let skill_file = skills_dir.join("SKILL.md");
        std::fs::write(&skill_file, "Skill body").unwrap();

        let rule_file = rules_dir.join("pov-direct.md");
        let initial_settings = PovDirectSettings {
            auto_activation: true,
            ..Default::default()
        };
        let initial_rule = render_rule_file(&initial_settings, "~/.gemini/config/skills/pov-direct/SKILL.md");
        std::fs::write(&rule_file, &initial_rule).unwrap();

        let status = inspect_pov_direct_status_at(&rules_dir, &skill_file).unwrap();

        let req = PovDirectApplyRequest {
            settings: PovDirectSettings {
                auto_activation: false,
                ..Default::default()
            },
            operation: PovDirectApplyOperation::Apply,
            expected_state: status.state,
            expected_content_hash: status.content_hash,
        };
        let res = save_pov_direct_settings_at(&rules_dir, &skill_file, req).unwrap();
        assert_eq!(res.state, PovDirectFileState::ManagedSynced);
        assert_eq!(res.settings.auto_activation, false);
    }
}
