use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Mutex;

pub const MARKER_PREFIX: &str = "<!-- poverello:managed:v1 ";
pub const MARKER_SUFFIX: &str = "-->";
pub const TEMPLATE_VERSION_1_0: &str = "1.0";
pub const MARKER_FORMAT_VERSION_1: u32 = 1;

pub const BALANCED_SKILL_NAME: &str = "pov-nerf-balanced";
pub const LIGHTWEIGHT_SKILL_NAME: &str = "pov-nerf-lightweight";
pub const LEGACY_SKILL_NAME: &str = "pov-nerf";

pub const BALANCED_DIGEST_V1: &str =
    "a7ca0bd952904b609a6b929a4ef44341f47d94b6e97d22c6446ed8e5256e10ed";
pub const LIGHTWEIGHT_DIGEST_V1: &str =
    "5a0bb13347fbba0478acda01eb3d9bc513582f85baaed232cccf113842c1e36a";

static POV_NERF_MUTATION_LOCK: Mutex<()> = Mutex::new(());

/// Stable filesystem object identity (§9.3.1 & §9.3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileIdentity {
    #[cfg(windows)]
    pub volume_serial_number: u32,
    #[cfg(windows)]
    pub file_index: u64,
    #[cfg(unix)]
    pub dev: u64,
    #[cfg(unix)]
    pub ino: u64,
}

impl FileIdentity {
    pub fn from_file(file: &std::fs::File) -> Option<Self> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::Storage::FileSystem::{
                GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
            };
            let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
            let res =
                unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, info.as_mut_ptr()) };
            if res != 0 {
                let info = unsafe { info.assume_init() };
                let file_index = ((info.nFileIndexHigh as u64) << 32) | (info.nFileIndexLow as u64);
                Some(Self {
                    volume_serial_number: info.dwVolumeSerialNumber,
                    file_index,
                })
            } else {
                None
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let meta = file.metadata().ok()?;
            Some(Self {
                dev: meta.dev(),
                ino: meta.ino(),
            })
        }
        #[cfg(not(any(windows, unix)))]
        {
            let _ = file;
            None
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
            use windows_sys::Win32::Storage::FileSystem::{
                CreateFileW, GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
                FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE,
                FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
            };

            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let handle = unsafe {
                CreateFileW(
                    wide.as_ptr(),
                    0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                    std::ptr::null_mut(),
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                return None;
            }
            let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
            let res = unsafe { GetFileInformationByHandle(handle as _, info.as_mut_ptr()) };
            unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };
            if res != 0 {
                let info = unsafe { info.assume_init() };
                let file_index = ((info.nFileIndexHigh as u64) << 32) | (info.nFileIndexLow as u64);
                Some(Self {
                    volume_serial_number: info.dwVolumeSerialNumber,
                    file_index,
                })
            } else {
                None
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let meta = path.symlink_metadata().ok()?;
            Some(Self {
                dev: meta.dev(),
                ino: meta.ino(),
            })
        }
        #[cfg(not(any(windows, unix)))]
        {
            let _ = path;
            None
        }
    }
}

#[cfg(test)]
pub(crate) mod test_seams {
    use std::cell::Cell;

    thread_local! {
        static INJECT_EXTERNAL_CHANGE_BEFORE_REMOVE: Cell<bool> = const { Cell::new(false) };
        static INJECT_TARGET_DELETE_BEFORE_REMOVE: Cell<bool> = const { Cell::new(false) };
        static INJECT_TARGET_REPLACE_CANONICAL_BYTES_BEFORE_REMOVE: Cell<bool> = const { Cell::new(false) };
        static INJECT_TARGET_REPLACE_UNSAFE_TYPE_BEFORE_REMOVE: Cell<bool> = const { Cell::new(false) };
        static INJECT_WRITE_FAILURE_INSTALL: Cell<bool> = const { Cell::new(false) };
        static INJECT_FLUSH_FAILURE_INSTALL: Cell<bool> = const { Cell::new(false) };
        static INJECT_SYNC_FAILURE_INSTALL: Cell<bool> = const { Cell::new(false) };
        static INJECT_READ_BACK_FAILURE_INSTALL: Cell<bool> = const { Cell::new(false) };
        static INJECT_REPLACE_TARGET_BEFORE_INSTALL_CLEANUP: Cell<bool> = const { Cell::new(false) };
        static INJECT_METADATA_ERROR_PATH_SAFETY: Cell<bool> = const { Cell::new(false) };
        static INJECT_IDENTITY_CAPTURE_FAILURE_INSTALL: Cell<bool> = const { Cell::new(false) };
        static INJECT_IDENTITY_LOOKUP_FAILURE_PATH_SAFETY: Cell<bool> = const { Cell::new(false) };
    }

    pub fn set_inject_external_change_before_remove(val: bool) {
        INJECT_EXTERNAL_CHANGE_BEFORE_REMOVE.with(|c| c.set(val));
    }

    pub fn set_inject_target_delete_before_remove(val: bool) {
        INJECT_TARGET_DELETE_BEFORE_REMOVE.with(|c| c.set(val));
    }

    pub fn set_inject_target_replace_canonical_bytes_before_remove(val: bool) {
        INJECT_TARGET_REPLACE_CANONICAL_BYTES_BEFORE_REMOVE.with(|c| c.set(val));
    }

    pub fn set_inject_target_replace_unsafe_type_before_remove(val: bool) {
        INJECT_TARGET_REPLACE_UNSAFE_TYPE_BEFORE_REMOVE.with(|c| c.set(val));
    }

    pub fn set_inject_write_failure_install(val: bool) {
        INJECT_WRITE_FAILURE_INSTALL.with(|c| c.set(val));
    }

    pub fn set_inject_flush_failure_install(val: bool) {
        INJECT_FLUSH_FAILURE_INSTALL.with(|c| c.set(val));
    }

    pub fn set_inject_sync_failure_install(val: bool) {
        INJECT_SYNC_FAILURE_INSTALL.with(|c| c.set(val));
    }

    pub fn set_inject_read_back_failure_install(val: bool) {
        INJECT_READ_BACK_FAILURE_INSTALL.with(|c| c.set(val));
    }

    pub fn set_inject_replace_target_before_install_cleanup(val: bool) {
        INJECT_REPLACE_TARGET_BEFORE_INSTALL_CLEANUP.with(|c| c.set(val));
    }

    pub fn set_inject_metadata_error_path_safety(val: bool) {
        INJECT_METADATA_ERROR_PATH_SAFETY.with(|c| c.set(val));
    }

    pub fn set_inject_identity_capture_failure_install(val: bool) {
        INJECT_IDENTITY_CAPTURE_FAILURE_INSTALL.with(|c| c.set(val));
    }

    pub fn set_inject_identity_lookup_failure_path_safety(val: bool) {
        INJECT_IDENTITY_LOOKUP_FAILURE_PATH_SAFETY.with(|c| c.set(val));
    }

    pub fn should_inject_external_change_before_remove() -> bool {
        INJECT_EXTERNAL_CHANGE_BEFORE_REMOVE.with(|c| c.get())
    }

    pub fn should_inject_target_delete_before_remove() -> bool {
        INJECT_TARGET_DELETE_BEFORE_REMOVE.with(|c| c.get())
    }

    pub fn should_inject_target_replace_canonical_bytes_before_remove() -> bool {
        INJECT_TARGET_REPLACE_CANONICAL_BYTES_BEFORE_REMOVE.with(|c| c.get())
    }

    pub fn should_inject_target_replace_unsafe_type_before_remove() -> bool {
        INJECT_TARGET_REPLACE_UNSAFE_TYPE_BEFORE_REMOVE.with(|c| c.get())
    }

    pub fn should_inject_write_failure_install() -> bool {
        INJECT_WRITE_FAILURE_INSTALL.with(|c| c.get())
    }

    pub fn should_inject_flush_failure_install() -> bool {
        INJECT_FLUSH_FAILURE_INSTALL.with(|c| c.get())
    }

    pub fn should_inject_sync_failure_install() -> bool {
        INJECT_SYNC_FAILURE_INSTALL.with(|c| c.get())
    }

    pub fn should_inject_read_back_failure_install() -> bool {
        INJECT_READ_BACK_FAILURE_INSTALL.with(|c| c.get())
    }

    pub fn should_inject_replace_target_before_install_cleanup() -> bool {
        INJECT_REPLACE_TARGET_BEFORE_INSTALL_CLEANUP.with(|c| c.get())
    }

    pub fn should_inject_metadata_error_path_safety() -> bool {
        INJECT_METADATA_ERROR_PATH_SAFETY.with(|c| c.get())
    }

    pub fn should_inject_identity_capture_failure_install() -> bool {
        INJECT_IDENTITY_CAPTURE_FAILURE_INSTALL.with(|c| c.get())
    }

    pub fn should_inject_identity_lookup_failure_path_safety() -> bool {
        INJECT_IDENTITY_LOOKUP_FAILURE_PATH_SAFETY.with(|c| c.get())
    }

    pub fn reset_all() {
        set_inject_external_change_before_remove(false);
        set_inject_target_delete_before_remove(false);
        set_inject_target_replace_canonical_bytes_before_remove(false);
        set_inject_target_replace_unsafe_type_before_remove(false);
        set_inject_write_failure_install(false);
        set_inject_flush_failure_install(false);
        set_inject_sync_failure_install(false);
        set_inject_read_back_failure_install(false);
        set_inject_replace_target_before_install_cleanup(false);
        set_inject_metadata_error_path_safety(false);
        set_inject_identity_capture_failure_install(false);
        set_inject_identity_lookup_failure_path_safety(false);
    }
}

/// Closed enum identifying the two supported Antigravity Nerf skills.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PovNerfSkillId {
    Balanced,
    Lightweight,
}

impl PovNerfSkillId {
    pub fn skill_name(&self) -> &'static str {
        match self {
            Self::Balanced => BALANCED_SKILL_NAME,
            Self::Lightweight => LIGHTWEIGHT_SKILL_NAME,
        }
    }

    pub fn slash_command(&self) -> &'static str {
        match self {
            Self::Balanced => "/pov-nerf-balanced",
            Self::Lightweight => "/pov-nerf-lightweight",
        }
    }

    pub fn expected_digest(&self) -> &'static str {
        match self {
            Self::Balanced => BALANCED_DIGEST_V1,
            Self::Lightweight => LIGHTWEIGHT_DIGEST_V1,
        }
    }

    pub fn canonical_frontmatter(&self) -> &'static str {
        match self {
            Self::Balanced => "---\nname: pov-nerf-balanced\ndescription: Poverello guidance for focused, balanced implementation work.\n---\n",
            Self::Lightweight => "---\nname: pov-nerf-lightweight\ndescription: Poverello guidance for small, localized implementation tasks.\n---\n",
        }
    }

    pub fn canonical_markdown_body(&self) -> &'static str {
        match self {
            Self::Balanced => "\n# Poverello Nerf — Balanced\n\nThese are advisory instructions for the task submitted with this command only. They do not persist as a product promise to later turns, and do not override higher-priority instructions, the user's request, or applicable repository workflows.\n\nKeep the work focused on the assigned task and avoid unrelated scope expansion. Start with directly relevant files, then inspect additional context, dependencies, tests, or constraints when needed for correctness, safety, or a sound implementation. Avoid broad unrelated exploration.\n\nUse proportionate planning: provide a plan or review when the user requests one or the project workflow requires it. If the task is clear and no planning step is required, proceed directly with the requested work.\n\nKeep explanations and progress updates concise while including the evidence and verification needed to understand the result. Never skip correctness, safety constraints, error handling, or required tests.\n",
            Self::Lightweight => "\n# Poverello Nerf — Lightweight\n\nThese are advisory instructions for the task submitted with this command only. They do not persist as a product promise to later turns, and do not override higher-priority instructions, the user's request, or applicable repository workflows.\n\nFor a small, well-defined task, make only the requested changes and avoid unrelated scope expansion. Inspect the target and enough surrounding context, dependencies, tests, and constraints to make the change safely and correctly; expand that inspection when the task requires it, but avoid unrelated broad exploration.\n\nDo not create a formal plan when the task is clear and none is requested or required by the project workflow. If the user requests a plan or review, or the project workflow requires one, follow that requirement.\n\nKeep the response brief but include material results and necessary verification. Never skip correctness, safety constraints, error handling, or required tests.\n",
        }
    }

    pub fn canonical_managed_marker(&self) -> String {
        format!(
            "{MARKER_PREFIX}skill={} template_version={} digest={} {MARKER_SUFFIX}\n",
            self.skill_name(),
            TEMPLATE_VERSION_1_0,
            self.expected_digest()
        )
    }

    /// Full canonical file serialization:
    /// frontmatter (starting at byte zero) + marker line + blank line + Markdown body (ending with LF).
    pub fn render_full_file(&self) -> String {
        format!(
            "{}{}{}",
            self.canonical_frontmatter(),
            self.canonical_managed_marker(),
            self.canonical_markdown_body()
        )
    }

    /// Digest input: full file excluding the marker line and its terminating LF.
    pub fn digest_input(&self) -> String {
        format!(
            "{}{}",
            self.canonical_frontmatter(),
            self.canonical_markdown_body()
        )
    }
}

/// The 5 lifecycle states for an Antigravity Nerf skill (§3.3 & §8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PovNerfSkillState {
    NotInstalled,
    Installed,
    Outdated,
    Modified,
    Invalid,
}

/// Permitted actions on a skill (§3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PovNerfAction {
    Install,
    Update,
    Remove,
}

/// Structured entry for an individual skill in the stable-order list (§3.3 & §8.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PovNerfSkillEntry {
    pub skill_id: PovNerfSkillId,
    pub skill_name: String,
    pub slash_command: String,
    pub target_path: String,
    pub state: PovNerfSkillState,
    pub permitted_actions: Vec<PovNerfAction>,
    pub diagnostic_reason: Option<String>,
}

/// Legacy single-skill notice if ~/.gemini/config/skills/pov-nerf exists (§3.3 & §8.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacySingleSkillNotice {
    pub present: bool,
    pub path: String,
    pub message: String,
}

/// Overall status report containing the fixed two-entry `skills` list and legacy notice (§3.3 & §8.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PovNerfCommandsInfo {
    pub skills_dir: String,
    pub skills: Vec<PovNerfSkillEntry>,
    pub legacy_single_skill: Option<LegacySingleSkillNotice>,
}

/// Outcome of a mutation operation (§8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PovNerfMutationOutcome {
    Success,
    Refused,
    VerificationUnavailable,
}

/// Typed result of a mutation operation (§8.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PovNerfOperationResult {
    pub outcome: PovNerfMutationOutcome,
    pub skill: PovNerfSkillEntry,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMarker {
    pub skill: String,
    pub template_version: String,
    pub digest: String,
}

pub fn normalize_line_endings_lf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn compute_sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Path safety status with no-follow metadata semantics (§9.3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSafetyStatus {
    SafeFileExists(Option<FileIdentity>),
    SafeTargetNotFound,
    UnsafeOrError(String),
}

/// Validates every component along `path` with no-follow metadata semantics (§9.3.2).
/// Rejects dangling symlinks, Windows reparse points, non-regular leaf targets, non-directory ancestors,
/// and permission/I/O errors.
pub fn check_path_safety(path: &Path, expect_leaf_file: bool) -> PathSafetyStatus {
    #[cfg(test)]
    if test_seams::should_inject_metadata_error_path_safety() {
        return PathSafetyStatus::UnsafeOrError(
            "Injected metadata error for path safety".to_string(),
        );
    }

    let components: Vec<_> = path.components().collect();
    let mut current = std::path::PathBuf::new();
    let mut encountered_not_found = false;
    let mut leaf_identity = None;

    for (idx, comp) in components.iter().enumerate() {
        current.push(comp);
        let is_leaf = idx == components.len() - 1;

        if encountered_not_found {
            match current.symlink_metadata() {
                Ok(_) => {
                    return PathSafetyStatus::UnsafeOrError(format!(
                        "Path component {} exists after an absent ancestor",
                        current.display()
                    ));
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return PathSafetyStatus::UnsafeOrError(format!(
                        "Failed to read metadata for {}: {e}",
                        current.display()
                    ));
                }
            }
            continue;
        }

        match current.symlink_metadata() {
            Ok(meta) => {
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;
                    if (meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0
                        || meta.file_type().is_symlink()
                    {
                        return PathSafetyStatus::UnsafeOrError(format!(
                            "Path component {} is a symlink or Windows reparse point",
                            current.display()
                        ));
                    }
                }
                #[cfg(not(windows))]
                if meta.file_type().is_symlink() {
                    return PathSafetyStatus::UnsafeOrError(format!(
                        "Path component {} is a symlink",
                        current.display()
                    ));
                }

                if is_leaf && expect_leaf_file {
                    if !meta.is_file() {
                        return PathSafetyStatus::UnsafeOrError(format!(
                            "Target {} is not a regular file",
                            current.display()
                        ));
                    }
                    #[cfg(test)]
                    if test_seams::should_inject_identity_lookup_failure_path_safety() {
                        leaf_identity = None;
                    } else {
                        leaf_identity = FileIdentity::from_path(&current);
                    }
                    #[cfg(not(test))]
                    {
                        leaf_identity = FileIdentity::from_path(&current);
                    }
                } else if !meta.is_dir() {
                    return PathSafetyStatus::UnsafeOrError(format!(
                        "Path component {} is not a directory",
                        current.display()
                    ));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                encountered_not_found = true;
            }
            Err(e) => {
                return PathSafetyStatus::UnsafeOrError(format!(
                    "Failed to read metadata for {}: {e}",
                    current.display()
                ));
            }
        }
    }

    if encountered_not_found {
        PathSafetyStatus::SafeTargetNotFound
    } else {
        PathSafetyStatus::SafeFileExists(leaf_identity)
    }
}

pub fn validate_path_safety(path: &Path, expect_leaf_file: bool) -> Result<(), String> {
    match check_path_safety(path, expect_leaf_file) {
        PathSafetyStatus::SafeFileExists(Some(_)) => Ok(()),
        PathSafetyStatus::SafeFileExists(None) if !expect_leaf_file => Ok(()),
        PathSafetyStatus::SafeFileExists(None) => {
            Err("Target file exists but stable file identity could not be verified".to_string())
        }
        PathSafetyStatus::SafeTargetNotFound => Ok(()),
        PathSafetyStatus::UnsafeOrError(err) => Err(err),
    }
}

/// Parses the frontmatter and the single managed marker line immediately following the closing `---`.
/// Enforces strict marker syntax (§8.3 #1).
pub fn extract_marker_and_digest_input(content: &str) -> Result<(ParsedMarker, String), String> {
    if !content.starts_with("---\n") {
        return Err("Content does not begin with frontmatter delimiter at byte zero".to_string());
    }

    let lines: Vec<&str> = content.split('\n').collect();
    let mut closing_idx = None;
    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.trim() == "---" {
            closing_idx = Some(i);
            break;
        }
    }

    let closing_idx = match closing_idx {
        Some(idx) => idx,
        None => return Err("Frontmatter is not closed with '---'".to_string()),
    };

    let marker_line_idx = closing_idx + 1;
    if marker_line_idx >= lines.len() {
        return Err(
            "File ends immediately after frontmatter without content or marker".to_string(),
        );
    }

    let marker_line = lines[marker_line_idx].trim();
    if !marker_line.starts_with(MARKER_PREFIX) || !marker_line.ends_with(MARKER_SUFFIX) {
        return Err(
            "Ownership marker is misplaced or missing immediately after frontmatter delimiter"
                .to_string(),
        );
    }

    // Ensure no duplicate markers exist anywhere else
    for (i, line) in lines.iter().enumerate() {
        if i != marker_line_idx {
            let trimmed = line.trim();
            if trimmed.contains("<!-- poverello:managed:") || trimmed.contains(MARKER_PREFIX.trim())
            {
                return Err("Duplicate or misplaced marker detected".to_string());
            }
        }
    }

    let inner = &marker_line[MARKER_PREFIX.len()..marker_line.len() - MARKER_SUFFIX.len()].trim();
    let tokens: Vec<&str> = inner.split_whitespace().collect();

    let mut skill = None;
    let mut template_version = None;
    let mut digest = None;

    for token in tokens {
        if let Some(val) = token.strip_prefix("skill=") {
            if skill.is_some() {
                return Err("Duplicate 'skill' attribute in marker".to_string());
            }
            skill = Some(val.to_string());
        } else if let Some(val) = token.strip_prefix("template_version=") {
            if template_version.is_some() {
                return Err("Duplicate 'template_version' attribute in marker".to_string());
            }
            template_version = Some(val.to_string());
        } else if let Some(val) = token.strip_prefix("digest=") {
            if digest.is_some() {
                return Err("Duplicate 'digest' attribute in marker".to_string());
            }
            digest = Some(val.to_string());
        } else {
            return Err(format!("Unknown or extra attribute in marker: {token}"));
        }
    }

    let parsed = ParsedMarker {
        skill: skill.ok_or_else(|| "Missing 'skill' attribute in marker".to_string())?,
        template_version: template_version
            .ok_or_else(|| "Missing 'template_version' attribute in marker".to_string())?,
        digest: digest.ok_or_else(|| "Missing 'digest' attribute in marker".to_string())?,
    };

    if parsed.template_version != TEMPLATE_VERSION_1_0 {
        return Err(format!(
            "Unsupported template version: {}",
            parsed.template_version
        ));
    }
    if parsed.digest.len() != 64 || !parsed.digest.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "Malformed SHA-256 digest in marker: {}",
            parsed.digest
        ));
    }

    // Marker must be followed by a blank line
    if marker_line_idx + 1 >= lines.len() || !lines[marker_line_idx + 1].trim().is_empty() {
        return Err(
            "Ownership marker must be followed by exactly one blank line before body".to_string(),
        );
    }

    // Reconstruct digest-input excluding the exact marker line and its immediate LF
    let mut digest_lines = Vec::with_capacity(lines.len() - 1);
    for (i, line) in lines.iter().enumerate() {
        if i != marker_line_idx {
            digest_lines.push(*line);
        }
    }
    let digest_input = digest_lines.join("\n");

    Ok((parsed, digest_input))
}

/// Inspects the status of a specific skill file at `skill_file` (§9.3.2).
/// Uses no-follow metadata checks and never uses `Path::exists()` as safety authority.
pub fn inspect_skill_status_at(skill_id: PovNerfSkillId, skill_file: &Path) -> PovNerfSkillEntry {
    let target_path = skill_file.to_string_lossy().to_string();

    match check_path_safety(skill_file, true) {
        PathSafetyStatus::SafeTargetNotFound => PovNerfSkillEntry {
            skill_id,
            skill_name: skill_id.skill_name().to_string(),
            slash_command: skill_id.slash_command().to_string(),
            target_path,
            state: PovNerfSkillState::NotInstalled,
            permitted_actions: vec![PovNerfAction::Install],
            diagnostic_reason: None,
        },
        PathSafetyStatus::UnsafeOrError(err) => PovNerfSkillEntry {
            skill_id,
            skill_name: skill_id.skill_name().to_string(),
            slash_command: skill_id.slash_command().to_string(),
            target_path,
            state: PovNerfSkillState::Invalid,
            permitted_actions: vec![],
            diagnostic_reason: Some(err),
        },
        PathSafetyStatus::SafeFileExists(None) => PovNerfSkillEntry {
            skill_id,
            skill_name: skill_id.skill_name().to_string(),
            slash_command: skill_id.slash_command().to_string(),
            target_path,
            state: PovNerfSkillState::Invalid,
            permitted_actions: vec![],
            diagnostic_reason: Some(
                "Target file exists but stable file identity could not be verified".to_string(),
            ),
        },
        PathSafetyStatus::SafeFileExists(Some(_)) => {
            let raw_bytes = match std::fs::read(skill_file) {
                Ok(b) => b,
                Err(e) => {
                    return PovNerfSkillEntry {
                        skill_id,
                        skill_name: skill_id.skill_name().to_string(),
                        slash_command: skill_id.slash_command().to_string(),
                        target_path,
                        state: PovNerfSkillState::Invalid,
                        permitted_actions: vec![],
                        diagnostic_reason: Some(format!("Failed to read file: {e}")),
                    };
                }
            };

            let content = match std::str::from_utf8(&raw_bytes) {
                Ok(s) => s,
                Err(_) => {
                    return PovNerfSkillEntry {
                        skill_id,
                        skill_name: skill_id.skill_name().to_string(),
                        slash_command: skill_id.slash_command().to_string(),
                        target_path,
                        state: PovNerfSkillState::Invalid,
                        permitted_actions: vec![],
                        diagnostic_reason: Some("File content is not valid UTF-8".to_string()),
                    };
                }
            };

            match extract_marker_and_digest_input(content) {
                Ok((marker, digest_input)) => {
                    if marker.skill != skill_id.skill_name() {
                        return PovNerfSkillEntry {
                            skill_id,
                            skill_name: skill_id.skill_name().to_string(),
                            slash_command: skill_id.slash_command().to_string(),
                            target_path,
                            state: PovNerfSkillState::Invalid,
                            permitted_actions: vec![],
                            diagnostic_reason: Some(format!(
                                "Marker skill identity mismatch: expected {}, found {}",
                                skill_id.skill_name(),
                                marker.skill
                            )),
                        };
                    }

                    let computed = compute_sha256_hex(digest_input.as_bytes());
                    let embedded = marker.digest;

                    // Exact match to current approved canonical template
                    if computed == embedded && computed == skill_id.expected_digest() {
                        PovNerfSkillEntry {
                            skill_id,
                            skill_name: skill_id.skill_name().to_string(),
                            slash_command: skill_id.slash_command().to_string(),
                            target_path,
                            state: PovNerfSkillState::Installed,
                            permitted_actions: vec![PovNerfAction::Remove],
                            diagnostic_reason: None,
                        }
                    } else {
                        // Modified body or marker/digest edit
                        PovNerfSkillEntry {
                            skill_id,
                            skill_name: skill_id.skill_name().to_string(),
                            slash_command: skill_id.slash_command().to_string(),
                            target_path,
                            state: PovNerfSkillState::Modified,
                            permitted_actions: vec![],
                            diagnostic_reason: Some(
                                "File content or digest differs from canonical template"
                                    .to_string(),
                            ),
                        }
                    }
                }
                Err(e) => PovNerfSkillEntry {
                    skill_id,
                    skill_name: skill_id.skill_name().to_string(),
                    slash_command: skill_id.slash_command().to_string(),
                    target_path,
                    state: PovNerfSkillState::Invalid,
                    permitted_actions: vec![],
                    diagnostic_reason: Some(e),
                },
            }
        }
    }
}

/// Inspects the overall commands status at `skills_root` (§3.3 & §8.2).
pub fn inspect_pov_nerf_commands_status_at(skills_root: &Path) -> PovNerfCommandsInfo {
    let balanced_path = skills_root
        .join(PovNerfSkillId::Balanced.skill_name())
        .join("SKILL.md");
    let lightweight_path = skills_root
        .join(PovNerfSkillId::Lightweight.skill_name())
        .join("SKILL.md");

    let balanced = inspect_skill_status_at(PovNerfSkillId::Balanced, &balanced_path);
    let lightweight = inspect_skill_status_at(PovNerfSkillId::Lightweight, &lightweight_path);

    let legacy_file = skills_root.join(LEGACY_SKILL_NAME).join("SKILL.md");
    let legacy_single_skill = match check_path_safety(&legacy_file, true) {
        PathSafetyStatus::SafeFileExists(_) => Some(LegacySingleSkillNotice {
            present: true,
            path: legacy_file.to_string_lossy().to_string(),
            message: "A legacy /pov-nerf Skill was detected. It is left untouched and can be removed manually if no longer needed.".to_string(),
        }),
        _ => None,
    };

    PovNerfCommandsInfo {
        skills_dir: skills_root.to_string_lossy().to_string(),
        skills: vec![balanced, lightweight],
        legacy_single_skill,
    }
}

/// Helper to safely cleanup partial target on install failure only if ownership is proven (§9.3.1).
fn cleanup_partial_install_target_if_owned(
    target_file: &Path,
    expected_identity: &FileIdentity,
) -> Result<bool, String> {
    let meta = match target_file.symlink_metadata() {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(true);
        }
        Err(e) => {
            return Err(format!(
                "Failed to read target metadata during cleanup: {e}"
            ));
        }
    };

    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(
            "Target is no longer a regular file; refusing cleanup to preserve external object"
                .to_string(),
        );
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;
        if (meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
            return Err(
                "Target is a Windows reparse point; refusing cleanup to preserve external object"
                    .to_string(),
            );
        }
    }

    let current_identity = match FileIdentity::from_path(target_file) {
        Some(id) => id,
        None => {
            return Err(
                "Could not determine current target file identity; refusing cleanup".to_string(),
            )
        }
    };

    if &current_identity != expected_identity {
        return Err(
            "Target identity does not match the file created by this operation; preserving external replacement"
                .to_string(),
        );
    }

    std::fs::remove_file(target_file)
        .map_err(|e| format!("Failed to remove partial target file: {e}"))?;
    Ok(true)
}

/// Installs a skill exclusively (§8.2, §8.3, & §9.3.1).
pub fn install_nerf_skill_at(
    skills_root: &Path,
    skill_id: PovNerfSkillId,
) -> Result<PovNerfOperationResult, String> {
    let _guard = POV_NERF_MUTATION_LOCK
        .lock()
        .map_err(|e| format!("Lock poisoned: {e}"))?;

    let skill_folder = skills_root.join(skill_id.skill_name());
    let target_file = skill_folder.join("SKILL.md");

    // Path safety validation of parent directory
    validate_path_safety(skills_root, false)?;

    // Initial inspection
    let initial_status = inspect_skill_status_at(skill_id, &target_file);
    if initial_status.state != PovNerfSkillState::NotInstalled {
        let msg = format!(
            "Cannot install {}: skill state is {:?}. Only uninstalled skills can be installed.",
            skill_id.skill_name(),
            initial_status.state
        );
        return Ok(PovNerfOperationResult {
            outcome: PovNerfMutationOutcome::Refused,
            skill: initial_status,
            message: Some(msg),
        });
    }

    match check_path_safety(&skill_folder, false) {
        PathSafetyStatus::SafeFileExists(_) => {}
        PathSafetyStatus::SafeTargetNotFound => {
            std::fs::create_dir_all(&skill_folder)
                .map_err(|e| format!("Failed to create skill directory: {e}"))?;
            validate_path_safety(&skill_folder, false)?;
        }
        PathSafetyStatus::UnsafeOrError(err) => {
            return Err(format!("Skill directory path is unsafe: {err}"));
        }
    }

    let content = skill_id.render_full_file();

    // Exclusive create (create_new)
    use std::fs::OpenOptions;
    use std::io::Write;
    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target_file)
    {
        Ok(f) => f,
        Err(e) => {
            let status = inspect_skill_status_at(skill_id, &target_file);
            return Ok(PovNerfOperationResult {
                outcome: PovNerfMutationOutcome::Refused,
                skill: status,
                message: Some(format!("Target file appeared concurrently: {e}")),
            });
        }
    };

    let id_opt = {
        #[cfg(test)]
        if test_seams::should_inject_identity_capture_failure_install() {
            None
        } else {
            FileIdentity::from_file(&file)
        }
        #[cfg(not(test))]
        FileIdentity::from_file(&file)
    };

    let expected_identity = match id_opt {
        Some(id) => id,
        None => {
            drop(file);
            let status = inspect_skill_status_at(skill_id, &target_file);
            return Ok(PovNerfOperationResult {
                outcome: PovNerfMutationOutcome::VerificationUnavailable,
                skill: status,
                message: Some(
                    "Install failed: unable to capture stable file identity from created handle; target was preserved without deletion (recovery required)"
                        .to_string(),
                ),
            });
        }
    };

    let handle_install_failure = |file: std::fs::File,
                                  failure_kind: &str|
     -> PovNerfOperationResult {
        drop(file);

        #[cfg(test)]
        if test_seams::should_inject_replace_target_before_install_cleanup() {
            let dummy = target_file.with_extension("tmp_dummy");
            let _ = std::fs::write(&dummy, "dummy allocation to bump MFT/inode");
            let _ = std::fs::remove_file(&target_file);
            let _ = std::fs::write(&target_file, "external replacement during install failure");
            let _ = std::fs::remove_file(&dummy);
        }

        match cleanup_partial_install_target_if_owned(&target_file, &expected_identity) {
            Ok(true) => {
                let status = inspect_skill_status_at(skill_id, &target_file);
                PovNerfOperationResult {
                    outcome: PovNerfMutationOutcome::VerificationUnavailable,
                    skill: status,
                    message: Some(format!(
                        "Install {failure_kind} failed; partial file was safely cleaned up"
                    )),
                }
            }
            Ok(false) => {
                let status = inspect_skill_status_at(skill_id, &target_file);
                PovNerfOperationResult {
                    outcome: PovNerfMutationOutcome::VerificationUnavailable,
                    skill: status,
                    message: Some(format!(
                        "Install {failure_kind} failed and target file was already absent"
                    )),
                }
            }
            Err(err) => {
                let status = inspect_skill_status_at(skill_id, &target_file);
                PovNerfOperationResult {
                    outcome: PovNerfMutationOutcome::VerificationUnavailable,
                    skill: status,
                    message: Some(format!(
                        "Install {failure_kind} failed and target could not be verified for safe cleanup ({err}); target was preserved (recovery required)"
                    )),
                }
            }
        }
    };

    // 1. Write
    #[cfg(test)]
    if test_seams::should_inject_write_failure_install() {
        return Ok(handle_install_failure(file, "write"));
    }

    if let Err(_e) = file.write_all(content.as_bytes()) {
        return Ok(handle_install_failure(file, "write"));
    }

    // 2. Flush
    #[cfg(test)]
    if test_seams::should_inject_flush_failure_install() {
        return Ok(handle_install_failure(file, "flush"));
    }

    if let Err(_e) = file.flush() {
        return Ok(handle_install_failure(file, "flush"));
    }

    // 3. Sync
    #[cfg(test)]
    if test_seams::should_inject_sync_failure_install() {
        return Ok(handle_install_failure(file, "sync"));
    }

    if let Err(_e) = file.sync_all() {
        return Ok(handle_install_failure(file, "sync"));
    }

    drop(file);

    // 4. Read-back verification (§9.4)
    #[cfg(test)]
    if test_seams::should_inject_read_back_failure_install() {
        let dummy_handle = match OpenOptions::new().read(true).open(&target_file) {
            Ok(h) => h,
            Err(_) => OpenOptions::new()
                .create(true)
                .write(true)
                .open(&target_file)
                .unwrap(),
        };
        return Ok(handle_install_failure(dummy_handle, "read-back"));
    }

    let final_status = inspect_skill_status_at(skill_id, &target_file);
    if final_status.state != PovNerfSkillState::Installed {
        return Ok(PovNerfOperationResult {
            outcome: PovNerfMutationOutcome::VerificationUnavailable,
            skill: final_status,
            message: Some("Installation read-back verification failed".to_string()),
        });
    }

    Ok(PovNerfOperationResult {
        outcome: PovNerfMutationOutcome::Success,
        skill: final_status,
        message: None,
    })
}

/// Updates an existing managed skill (§9.3.4).
/// As specified in §9.3.4, no prior canonical template is approved.
/// Outdated is unreachable and Update is unavailable.
/// Every Update request must refuse without creating a temp file or changing the target.
/// Zero target writes, zero temp writes.
pub fn update_nerf_skill_at(
    skills_root: &Path,
    skill_id: PovNerfSkillId,
) -> Result<PovNerfOperationResult, String> {
    let _guard = POV_NERF_MUTATION_LOCK
        .lock()
        .map_err(|e| format!("Lock poisoned: {e}"))?;

    let skill_folder = skills_root.join(skill_id.skill_name());
    let target_file = skill_folder.join("SKILL.md");

    let current_status = inspect_skill_status_at(skill_id, &target_file);
    Ok(PovNerfOperationResult {
        outcome: PovNerfMutationOutcome::Refused,
        skill: current_status,
        message: Some(
            "Update is unavailable: no prior canonical template is currently approved".to_string(),
        ),
    })
}

/// Removes a managed skill file if it matches expected or recognized managed content (§8.2, §8.3, & §9.3.3).
/// Never deletes the directory or sibling files.
/// Validates that target identity matches initial inspection before removal.
pub fn remove_nerf_skill_at(
    skills_root: &Path,
    skill_id: PovNerfSkillId,
) -> Result<PovNerfOperationResult, String> {
    let _guard = POV_NERF_MUTATION_LOCK
        .lock()
        .map_err(|e| format!("Lock poisoned: {e}"))?;

    let skill_folder = skills_root.join(skill_id.skill_name());
    let target_file = skill_folder.join("SKILL.md");

    // Initial inspection and identity capture
    let initial_safety = check_path_safety(&target_file, true);
    let initial_identity = match initial_safety {
        PathSafetyStatus::SafeFileExists(Some(id)) => id,
        PathSafetyStatus::SafeFileExists(None) => {
            let status = inspect_skill_status_at(skill_id, &target_file);
            return Ok(PovNerfOperationResult {
                outcome: PovNerfMutationOutcome::Refused,
                skill: status,
                message: Some(
                    "Cannot establish stable file identity for target; refusing removal"
                        .to_string(),
                ),
            });
        }
        PathSafetyStatus::SafeTargetNotFound => {
            let status = inspect_skill_status_at(skill_id, &target_file);
            return Ok(PovNerfOperationResult {
                outcome: PovNerfMutationOutcome::Refused,
                skill: status,
                message: Some("Target file is not installed".to_string()),
            });
        }
        PathSafetyStatus::UnsafeOrError(err) => {
            let status = inspect_skill_status_at(skill_id, &target_file);
            return Ok(PovNerfOperationResult {
                outcome: PovNerfMutationOutcome::Refused,
                skill: status,
                message: Some(format!(
                    "Cannot remove: target path is unsafe or inaccessible: {err}"
                )),
            });
        }
    };

    let initial_status = inspect_skill_status_at(skill_id, &target_file);
    if !initial_status
        .permitted_actions
        .contains(&PovNerfAction::Remove)
    {
        let msg = format!(
            "Cannot remove {}: state is {:?}. Only managed installed skills can be removed.",
            skill_id.skill_name(),
            initial_status.state
        );
        return Ok(PovNerfOperationResult {
            outcome: PovNerfMutationOutcome::Refused,
            skill: initial_status,
            message: Some(msg),
        });
    }

    #[cfg(test)]
    if test_seams::should_inject_external_change_before_remove() {
        let _ = std::fs::write(&target_file, "external modification between checks");
    }

    #[cfg(test)]
    if test_seams::should_inject_target_delete_before_remove() {
        let _ = std::fs::remove_file(&target_file);
    }

    #[cfg(test)]
    if test_seams::should_inject_target_replace_canonical_bytes_before_remove() {
        let content = skill_id.render_full_file();
        let dummy = target_file.with_extension("tmp_dummy");
        let _ = std::fs::write(&dummy, "dummy allocation to bump MFT/inode");
        let _ = std::fs::remove_file(&target_file);
        let _ = std::fs::write(&target_file, &content);
        let _ = std::fs::remove_file(&dummy);
    }

    #[cfg(test)]
    if test_seams::should_inject_target_replace_unsafe_type_before_remove() {
        let _ = std::fs::remove_file(&target_file);
        let _ = std::fs::create_dir(&target_file);
    }

    // Pre-delete recheck: target must still exist with exact initial identity (§9.3.3)
    let reval_safety = check_path_safety(&target_file, true);
    let reval_identity = match reval_safety {
        PathSafetyStatus::SafeFileExists(Some(id)) => id,
        _ => {
            let reval_status = inspect_skill_status_at(skill_id, &target_file);
            return Ok(PovNerfOperationResult {
                outcome: PovNerfMutationOutcome::Refused,
                skill: reval_status,
                message: Some(
                    "Pre-delete recheck failed: target file disappeared or became unsafe; preserving state"
                        .to_string(),
                ),
            });
        }
    };

    if reval_identity != initial_identity {
        let reval_status = inspect_skill_status_at(skill_id, &target_file);
        return Ok(PovNerfOperationResult {
            outcome: PovNerfMutationOutcome::Refused,
            skill: reval_status,
            message: Some(
                "Pre-delete recheck failed: target file identity changed (external replacement detected); preserving replacement"
                    .to_string(),
            ),
        });
    }

    let reval_status = inspect_skill_status_at(skill_id, &target_file);
    let reval_state = reval_status.state;
    if reval_state != PovNerfSkillState::Installed {
        return Ok(PovNerfOperationResult {
            outcome: PovNerfMutationOutcome::Refused,
            skill: reval_status,
            message: Some(format!(
                "Pre-delete recheck failed: target file state changed to {:?}",
                reval_state
            )),
        });
    }

    // Single-file removal (never remove directory or sibling files)
    // Note: Revalidation compares identity immediately before deletion.
    // An unobservable OS race between this final check and remove_file is not an atomic compare-and-delete guarantee.
    std::fs::remove_file(&target_file).map_err(|e| format!("Failed to remove skill file: {e}"))?;

    let post_status = inspect_skill_status_at(skill_id, &target_file);
    if post_status.state != PovNerfSkillState::NotInstalled {
        return Ok(PovNerfOperationResult {
            outcome: PovNerfMutationOutcome::VerificationUnavailable,
            skill: post_status,
            message: Some(
                "Post-delete verification failed: skill file still appears present".to_string(),
            ),
        });
    }

    Ok(PovNerfOperationResult {
        outcome: PovNerfMutationOutcome::Success,
        skill: post_status,
        message: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_canonical_frontmatter_at_byte_zero_and_marker_placement() {
        for skill_id in [PovNerfSkillId::Balanced, PovNerfSkillId::Lightweight] {
            let rendered = skill_id.render_full_file();
            assert!(
                rendered.starts_with("---\n"),
                "Skill {} must begin with frontmatter at byte zero",
                skill_id.skill_name()
            );

            let (marker, digest_input) = extract_marker_and_digest_input(&rendered)
                .expect("Valid canonical file must parse");
            assert_eq!(marker.skill, skill_id.skill_name());
            assert_eq!(marker.template_version, TEMPLATE_VERSION_1_0);
            assert_eq!(marker.digest, skill_id.expected_digest());

            let computed = compute_sha256_hex(digest_input.as_bytes());
            assert_eq!(computed, skill_id.expected_digest());
        }
    }

    #[test]
    fn test_canonical_digests_match_approved_plan_values() {
        assert_eq!(
            compute_sha256_hex(PovNerfSkillId::Balanced.digest_input().as_bytes()),
            BALANCED_DIGEST_V1
        );
        assert_eq!(
            compute_sha256_hex(PovNerfSkillId::Lightweight.digest_input().as_bytes()),
            LIGHTWEIGHT_DIGEST_V1
        );
    }

    #[test]
    fn test_status_response_shape_and_stable_order() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let status = inspect_pov_nerf_commands_status_at(root);
        assert_eq!(status.skills.len(), 2);
        assert_eq!(status.skills[0].skill_id, PovNerfSkillId::Balanced);
        assert_eq!(status.skills[0].skill_name, BALANCED_SKILL_NAME);
        assert_eq!(status.skills[0].slash_command, "/pov-nerf-balanced");
        assert_eq!(status.skills[0].state, PovNerfSkillState::NotInstalled);
        assert_eq!(
            status.skills[0].permitted_actions,
            vec![PovNerfAction::Install]
        );

        assert_eq!(status.skills[1].skill_id, PovNerfSkillId::Lightweight);
        assert_eq!(status.skills[1].skill_name, LIGHTWEIGHT_SKILL_NAME);
        assert_eq!(status.skills[1].slash_command, "/pov-nerf-lightweight");
        assert_eq!(status.skills[1].state, PovNerfSkillState::NotInstalled);
        assert_eq!(
            status.skills[1].permitted_actions,
            vec![PovNerfAction::Install]
        );

        assert!(status.legacy_single_skill.is_none());
    }

    #[test]
    fn test_operation_matrix_install_update_refusal_and_remove() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let skill_id = PovNerfSkillId::Balanced;

        // 1. NotInstalled -> Install allowed
        let res = install_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(res.outcome, PovNerfMutationOutcome::Success);
        assert_eq!(res.skill.state, PovNerfSkillState::Installed);
        assert_eq!(res.skill.permitted_actions, vec![PovNerfAction::Remove]);

        // 2. Repeated install -> Refused without modifying bytes
        let rep = install_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(rep.outcome, PovNerfMutationOutcome::Refused);

        // 3. Update request on Installed state -> Refused with zero target/temp writes (§9.3.4)
        let skill_file = root.join(skill_id.skill_name()).join("SKILL.md");
        let bytes_before = std::fs::read(&skill_file).unwrap();
        let upd = update_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(upd.outcome, PovNerfMutationOutcome::Refused);
        assert_eq!(std::fs::read(&skill_file).unwrap(), bytes_before);

        // Verify zero temp files exist in folder
        let entries: Vec<_> = std::fs::read_dir(root.join(skill_id.skill_name()))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(entries, vec!["SKILL.md"]);

        // 4. Remove allowed on Installed state
        let rem = remove_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(rem.outcome, PovNerfMutationOutcome::Success);
        assert_eq!(rem.skill.state, PovNerfSkillState::NotInstalled);
    }

    #[test]
    fn test_modified_and_unmanaged_files_refuse_destructive_operations() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let skill_id = PovNerfSkillId::Balanced;

        let skill_dir = root.join(skill_id.skill_name());
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_file = skill_dir.join("SKILL.md");

        // Case A: Custom unmanaged file
        std::fs::write(&skill_file, "custom user script without marker").unwrap();
        let status = inspect_skill_status_at(skill_id, &skill_file);
        assert_eq!(status.state, PovNerfSkillState::Invalid);
        assert!(status.permitted_actions.is_empty());

        let res_rem = remove_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(res_rem.outcome, PovNerfMutationOutcome::Refused);
        assert_eq!(
            std::fs::read_to_string(&skill_file).unwrap(),
            "custom user script without marker"
        );

        // Case B: Coordinated body and marker edit (different digest than canonical)
        let modified_content = format!(
            "---\nname: pov-nerf-balanced\ndescription: test\n---\n<!-- poverello:managed:v1 skill=pov-nerf-balanced template_version=1.0 digest=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef -->\n\n# Custom body\n"
        );
        std::fs::write(&skill_file, &modified_content).unwrap();
        let status_mod = inspect_skill_status_at(skill_id, &skill_file);
        assert_eq!(status_mod.state, PovNerfSkillState::Modified);
        assert!(status_mod.permitted_actions.is_empty());

        let res_rem2 = remove_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(res_rem2.outcome, PovNerfMutationOutcome::Refused);
        assert_eq!(
            std::fs::read_to_string(&skill_file).unwrap(),
            modified_content
        );
    }

    #[test]
    fn test_invalid_utf8_preserved_and_non_destructive() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let skill_id = PovNerfSkillId::Balanced;

        let skill_dir = root.join(skill_id.skill_name());
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_file = skill_dir.join("SKILL.md");

        // Invalid UTF-8 bytes (e.g. 0xFF, 0xFE)
        let invalid_bytes = vec![0xFF, 0xFE, 0x00, 0x12];
        std::fs::write(&skill_file, &invalid_bytes).unwrap();

        let status = inspect_skill_status_at(skill_id, &skill_file);
        assert_eq!(status.state, PovNerfSkillState::Invalid);
        assert!(status.permitted_actions.is_empty());

        // Remove must refuse and preserve exact bytes
        let rem = remove_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(rem.outcome, PovNerfMutationOutcome::Refused);
        assert_eq!(std::fs::read(&skill_file).unwrap(), invalid_bytes);
    }

    #[test]
    fn test_marker_syntax_strictness_rejects_malformed_misplaced_or_extra_attributes() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let skill_id = PovNerfSkillId::Balanced;
        let skill_dir = root.join(skill_id.skill_name());
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_file = skill_dir.join("SKILL.md");

        // Extra attribute in marker
        let extra_attr = format!(
            "---\nname: pov-nerf-balanced\ndescription: test\n---\n<!-- poverello:managed:v1 skill=pov-nerf-balanced template_version=1.0 digest={} extra=bad -->\n\n# Body\n",
            BALANCED_DIGEST_V1
        );
        std::fs::write(&skill_file, &extra_attr).unwrap();
        assert_eq!(
            inspect_skill_status_at(skill_id, &skill_file).state,
            PovNerfSkillState::Invalid
        );

        // Misplaced marker (blank line before marker)
        let misplaced = format!(
            "---\nname: pov-nerf-balanced\ndescription: test\n---\n\n<!-- poverello:managed:v1 skill=pov-nerf-balanced template_version=1.0 digest={} -->\n\n# Body\n",
            BALANCED_DIGEST_V1
        );
        std::fs::write(&skill_file, &misplaced).unwrap();
        assert_eq!(
            inspect_skill_status_at(skill_id, &skill_file).state,
            PovNerfSkillState::Invalid
        );

        // Duplicate markers
        let duplicate = format!(
            "---\nname: pov-nerf-balanced\ndescription: test\n---\n<!-- poverello:managed:v1 skill=pov-nerf-balanced template_version=1.0 digest={} -->\n\n<!-- poverello:managed:v1 skill=pov-nerf-balanced template_version=1.0 digest={} -->\n\n# Body\n",
            BALANCED_DIGEST_V1, BALANCED_DIGEST_V1
        );
        std::fs::write(&skill_file, &duplicate).unwrap();
        assert_eq!(
            inspect_skill_status_at(skill_id, &skill_file).state,
            PovNerfSkillState::Invalid
        );
    }

    #[test]
    fn test_install_failure_injection_proves_ownership_and_handles_replacements() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let skill_id = PovNerfSkillId::Balanced;
        let skill_file = root.join(skill_id.skill_name()).join("SKILL.md");

        // Case 1: Write failure - target is cleaned when owned
        test_seams::reset_all();
        test_seams::set_inject_write_failure_install(true);
        let res1 = install_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(
            res1.outcome,
            PovNerfMutationOutcome::VerificationUnavailable
        );
        assert!(!skill_file.exists());

        // Case 2: Flush failure - target is cleaned when owned
        test_seams::reset_all();
        test_seams::set_inject_flush_failure_install(true);
        let res2 = install_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(
            res2.outcome,
            PovNerfMutationOutcome::VerificationUnavailable
        );
        assert!(!skill_file.exists());

        // Case 3: Sync failure - target is cleaned when owned
        test_seams::reset_all();
        test_seams::set_inject_sync_failure_install(true);
        let res3 = install_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(
            res3.outcome,
            PovNerfMutationOutcome::VerificationUnavailable
        );
        assert!(!skill_file.exists());

        // Case 4: Target replaced before cleanup -> replacement preserved, recovery required
        test_seams::reset_all();
        test_seams::set_inject_write_failure_install(true);
        test_seams::set_inject_replace_target_before_install_cleanup(true);
        let res4 = install_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(
            res4.outcome,
            PovNerfMutationOutcome::VerificationUnavailable
        );
        assert!(
            res4.message.as_ref().unwrap().contains("recovery required"),
            "Message must indicate recovery required"
        );
        assert!(skill_file.exists());
        assert_eq!(
            std::fs::read_to_string(&skill_file).unwrap(),
            "external replacement during install failure"
        );

        // Case 5: Read-back failure with target replacement -> preserved
        let _ = std::fs::remove_file(&skill_file);
        test_seams::reset_all();
        test_seams::set_inject_read_back_failure_install(true);
        test_seams::set_inject_replace_target_before_install_cleanup(true);
        let res5 = install_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(
            res5.outcome,
            PovNerfMutationOutcome::VerificationUnavailable
        );
        assert!(skill_file.exists());
        assert_eq!(
            std::fs::read_to_string(&skill_file).unwrap(),
            "external replacement during install failure"
        );

        test_seams::reset_all();
    }

    #[test]
    fn test_path_safety_rejects_metadata_error_and_non_regular_target() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let skill_id = PovNerfSkillId::Balanced;
        let skill_dir = root.join(skill_id.skill_name());
        let skill_file = skill_dir.join("SKILL.md");

        // Case A: Leaf is a directory instead of a regular file
        std::fs::create_dir_all(&skill_file).unwrap();
        let status = inspect_skill_status_at(skill_id, &skill_file);
        assert_eq!(status.state, PovNerfSkillState::Invalid);
        assert!(status.permitted_actions.is_empty());
        assert!(status
            .diagnostic_reason
            .as_ref()
            .unwrap()
            .contains("not a regular file"));

        // Mutation refuses and directory preserved
        let rem = remove_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(rem.outcome, PovNerfMutationOutcome::Refused);
        assert!(skill_file.is_dir());
        std::fs::remove_dir(&skill_file).unwrap();

        // Case B: Injected metadata inspection error fails closed
        test_seams::reset_all();
        test_seams::set_inject_metadata_error_path_safety(true);

        let status2 = inspect_skill_status_at(skill_id, &skill_file);
        assert_eq!(status2.state, PovNerfSkillState::Invalid);
        assert!(status2.permitted_actions.is_empty());

        let inst = install_nerf_skill_at(root, skill_id);
        assert!(inst.is_err() || inst.unwrap().outcome == PovNerfMutationOutcome::Refused);

        let rem = remove_nerf_skill_at(root, skill_id);
        assert!(rem.is_err() || rem.unwrap().outcome == PovNerfMutationOutcome::Refused);

        test_seams::reset_all();
    }

    #[test]
    fn test_remove_revalidation_refuses_replaced_canonical_target_and_unsafe_changes() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let skill_id = PovNerfSkillId::Balanced;
        let skill_file = root.join(skill_id.skill_name()).join("SKILL.md");

        // 1. External replacement with exact canonical bytes -> Refused, replacement preserved (§9.3.3)
        install_nerf_skill_at(root, skill_id).unwrap();
        test_seams::reset_all();
        test_seams::set_inject_target_replace_canonical_bytes_before_remove(true);

        let rem1 = remove_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(rem1.outcome, PovNerfMutationOutcome::Refused);
        assert!(
            rem1.message
                .as_ref()
                .unwrap()
                .contains("replacement detected"),
            "Message must report replacement detection"
        );
        assert!(skill_file.exists());
        assert_eq!(
            std::fs::read_to_string(&skill_file).unwrap(),
            skill_id.render_full_file()
        );

        // 2. Target replaced with unsafe type (directory) between checks -> Refused, preserved
        test_seams::reset_all();
        test_seams::set_inject_target_replace_unsafe_type_before_remove(true);

        let rem2 = remove_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(rem2.outcome, PovNerfMutationOutcome::Refused);
        assert!(skill_file.is_dir());
        std::fs::remove_dir(&skill_file).unwrap();

        // 3. Target deleted between checks -> Refused
        install_nerf_skill_at(root, skill_id).unwrap();
        test_seams::reset_all();
        test_seams::set_inject_target_delete_before_remove(true);

        let rem3 = remove_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(rem3.outcome, PovNerfMutationOutcome::Refused);
        assert!(!skill_file.exists());

        test_seams::reset_all();
    }

    #[test]
    fn test_directory_and_siblings_preserved_on_removal() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let skill_id = PovNerfSkillId::Balanced;

        install_nerf_skill_at(root, skill_id).unwrap();

        let skill_dir = root.join(skill_id.skill_name());
        let sibling = skill_dir.join("sibling.txt");
        std::fs::write(&sibling, "keep me").unwrap();

        let rem = remove_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(rem.outcome, PovNerfMutationOutcome::Success);

        assert!(skill_dir.exists(), "Skill directory must be preserved");
        assert!(sibling.exists(), "Sibling file must be preserved");
        assert_eq!(std::fs::read_to_string(&sibling).unwrap(), "keep me");
    }

    #[test]
    fn test_legacy_skill_detected_read_only() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let legacy_dir = root.join(LEGACY_SKILL_NAME);
        std::fs::create_dir_all(&legacy_dir).unwrap();
        let legacy_file = legacy_dir.join("SKILL.md");
        std::fs::write(&legacy_file, "old legacy skill").unwrap();

        let status = inspect_pov_nerf_commands_status_at(root);
        assert!(status.legacy_single_skill.is_some());
        let notice = status.legacy_single_skill.unwrap();
        assert!(notice.present);
        assert_eq!(notice.path, legacy_file.to_string_lossy().to_string());

        // File remains untouched
        assert_eq!(
            std::fs::read_to_string(&legacy_file).unwrap(),
            "old legacy skill"
        );
    }

    #[test]
    fn test_symlinks_rejected_when_supported() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let skill_id = PovNerfSkillId::Balanced;

        let external_target = root.join("external_target");
        std::fs::create_dir_all(&external_target).unwrap();
        let target = external_target.join("target_file.txt");
        std::fs::write(&target, "external target content").unwrap();
        let sibling = external_target.join("sibling.txt");
        std::fs::write(&sibling, "external sibling content").unwrap();

        let link_leaf = root.join(skill_id.skill_name()).join("SKILL.md");
        std::fs::create_dir_all(root.join(skill_id.skill_name())).unwrap();

        #[cfg(windows)]
        let link_res = std::os::windows::fs::symlink_file(&target, &link_leaf);
        #[cfg(unix)]
        let link_res = std::os::unix::fs::symlink(&target, &link_leaf);
        #[cfg(not(any(windows, unix)))]
        let link_res: std::io::Result<()> = Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "symlinks unsupported",
        ));

        match link_res {
            Ok(_) => {
                let status = inspect_skill_status_at(skill_id, &link_leaf);
                assert_eq!(status.state, PovNerfSkillState::Invalid);
                assert!(status.permitted_actions.is_empty());
                assert!(status
                    .diagnostic_reason
                    .as_ref()
                    .unwrap()
                    .contains("is a symlink"));

                let inst = install_nerf_skill_at(root, skill_id);
                assert!(inst.is_err() || inst.unwrap().outcome == PovNerfMutationOutcome::Refused);

                let upd = update_nerf_skill_at(root, skill_id);
                assert!(upd.is_err() || upd.unwrap().outcome == PovNerfMutationOutcome::Refused);

                let rem = remove_nerf_skill_at(root, skill_id);
                assert!(rem.is_err() || rem.unwrap().outcome == PovNerfMutationOutcome::Refused);

                assert!(target.exists());
                assert_eq!(
                    std::fs::read_to_string(&target).unwrap(),
                    "external target content"
                );
                assert!(sibling.exists());
                assert_eq!(
                    std::fs::read_to_string(&sibling).unwrap(),
                    "external sibling content"
                );
            }
            Err(e) => {
                println!("Explicitly skipping leaf symlink creation: privilege/unsupported: {e}");
            }
        }
    }

    #[test]
    fn test_ancestor_symlink_rejected_when_supported() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let skill_id = PovNerfSkillId::Balanced;

        let external_target = root.join("external_ancestor");
        std::fs::create_dir_all(&external_target).unwrap();
        let sibling = external_target.join("sibling.txt");
        std::fs::write(&sibling, "external ancestor sibling").unwrap();

        let ancestor_link = root.join(skill_id.skill_name());

        #[cfg(windows)]
        let link_res = std::os::windows::fs::symlink_dir(&external_target, &ancestor_link);
        #[cfg(unix)]
        let link_res = std::os::unix::fs::symlink(&external_target, &ancestor_link);
        #[cfg(not(any(windows, unix)))]
        let link_res: std::io::Result<()> = Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "symlinks unsupported",
        ));

        match link_res {
            Ok(_) => {
                let target_file = ancestor_link.join("SKILL.md");
                let status = inspect_skill_status_at(skill_id, &target_file);
                assert_eq!(status.state, PovNerfSkillState::Invalid);
                assert!(status.permitted_actions.is_empty());
                assert!(
                    status
                        .diagnostic_reason
                        .as_ref()
                        .unwrap()
                        .contains("is a symlink"),
                    "Diagnostic must report symlink"
                );

                let inst = install_nerf_skill_at(root, skill_id);
                assert!(inst.is_err() || inst.unwrap().outcome == PovNerfMutationOutcome::Refused);

                let upd = update_nerf_skill_at(root, skill_id);
                assert!(upd.is_err() || upd.unwrap().outcome == PovNerfMutationOutcome::Refused);

                let rem = remove_nerf_skill_at(root, skill_id);
                assert!(rem.is_err() || rem.unwrap().outcome == PovNerfMutationOutcome::Refused);

                assert!(sibling.exists());
                assert_eq!(
                    std::fs::read_to_string(&sibling).unwrap(),
                    "external ancestor sibling"
                );
            }
            Err(e) => {
                println!(
                    "Explicitly skipping ancestor symlink creation: privilege/unsupported: {e}"
                );
            }
        }
    }

    #[test]
    fn test_windows_directory_junction_or_reparse_point_rejected_when_supported() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let skill_id = PovNerfSkillId::Balanced;

        let external_target = root.join("external_target_dir");
        std::fs::create_dir_all(&external_target).unwrap();
        let sibling = external_target.join("sibling.txt");
        std::fs::write(&sibling, "external sibling").unwrap();

        let skill_dir = root.join(skill_id.skill_name());

        #[cfg(windows)]
        let junc_res = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(skill_dir.as_os_str())
            .arg(external_target.as_os_str())
            .output();
        #[cfg(not(windows))]
        let junc_res: std::io::Result<std::process::Output> = Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Windows junctions unsupported on this platform",
        ));

        match junc_res {
            Ok(output) if output.status.success() => {
                let target_file = skill_dir.join("SKILL.md");
                let status = inspect_skill_status_at(skill_id, &target_file);
                assert_eq!(status.state, PovNerfSkillState::Invalid);
                assert!(status.permitted_actions.is_empty());
                assert!(
                    status
                        .diagnostic_reason
                        .as_ref()
                        .unwrap()
                        .contains("Windows reparse point"),
                    "Diagnostic must report Windows reparse point, got: {:?}",
                    status.diagnostic_reason
                );

                let inst = install_nerf_skill_at(root, skill_id);
                assert!(inst.is_err() || inst.unwrap().outcome == PovNerfMutationOutcome::Refused);

                let upd = update_nerf_skill_at(root, skill_id);
                assert!(upd.is_err() || upd.unwrap().outcome == PovNerfMutationOutcome::Refused);

                let rem = remove_nerf_skill_at(root, skill_id);
                assert!(rem.is_err() || rem.unwrap().outcome == PovNerfMutationOutcome::Refused);

                // Preserve external target and sibling
                assert!(sibling.exists());
                assert_eq!(
                    std::fs::read_to_string(&sibling).unwrap(),
                    "external sibling"
                );

                // Cleanup junction
                let _ = std::fs::remove_dir(&skill_dir);
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!(
                    "Explicitly skipping Windows directory junction creation: mklink failed: {stderr}"
                );
            }
            Err(e) => {
                println!(
                    "Explicitly skipping Windows directory junction creation: privilege/unsupported: {e}"
                );
            }
        }
    }

    #[test]
    fn test_windows_ancestor_junction_rejected_when_supported() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let skill_id = PovNerfSkillId::Balanced;

        let real_sub = root.join("real_sub");
        std::fs::create_dir_all(&real_sub).unwrap();
        let sibling = real_sub.join("sibling.txt");
        std::fs::write(&sibling, "ancestor sibling").unwrap();

        let ancestor_junc = root.join("ancestor_junc");

        #[cfg(windows)]
        let junc_res = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(ancestor_junc.as_os_str())
            .arg(real_sub.as_os_str())
            .output();
        #[cfg(not(windows))]
        let junc_res: std::io::Result<std::process::Output> = Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Windows junctions unsupported on this platform",
        ));

        match junc_res {
            Ok(output) if output.status.success() => {
                let target_file = ancestor_junc.join(skill_id.skill_name()).join("SKILL.md");
                let status = inspect_skill_status_at(skill_id, &target_file);
                assert_eq!(status.state, PovNerfSkillState::Invalid);
                assert!(status.permitted_actions.is_empty());
                assert!(
                    status
                        .diagnostic_reason
                        .as_ref()
                        .unwrap()
                        .contains("Windows reparse point"),
                    "Diagnostic must report Windows reparse point in ancestor"
                );

                let inst = install_nerf_skill_at(&ancestor_junc, skill_id);
                assert!(inst.is_err() || inst.unwrap().outcome == PovNerfMutationOutcome::Refused);

                let upd = update_nerf_skill_at(&ancestor_junc, skill_id);
                assert!(upd.is_err() || upd.unwrap().outcome == PovNerfMutationOutcome::Refused);

                let rem = remove_nerf_skill_at(&ancestor_junc, skill_id);
                assert!(rem.is_err() || rem.unwrap().outcome == PovNerfMutationOutcome::Refused);

                assert!(sibling.exists());
                assert_eq!(
                    std::fs::read_to_string(&sibling).unwrap(),
                    "ancestor sibling"
                );

                // Cleanup junction
                let _ = std::fs::remove_dir(&ancestor_junc);
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!(
                    "Explicitly skipping Windows ancestor junction creation: mklink failed: {stderr}"
                );
            }
            Err(e) => {
                println!(
                    "Explicitly skipping Windows ancestor junction creation: privilege/unsupported: {e}"
                );
            }
        }
    }

    #[test]
    fn test_install_identity_capture_failure_preserves_target_and_requires_recovery() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let skill_id = PovNerfSkillId::Balanced;
        let skill_file = root.join(skill_id.skill_name()).join("SKILL.md");

        test_seams::reset_all();
        test_seams::set_inject_identity_capture_failure_install(true);

        let res = install_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(res.outcome, PovNerfMutationOutcome::VerificationUnavailable);
        assert!(
            res.message.as_ref().unwrap().contains("recovery required"),
            "Message must require recovery"
        );
        assert!(
            skill_file.exists(),
            "Target file must be preserved on identity capture failure"
        );

        let status = inspect_skill_status_at(skill_id, &skill_file);
        assert_eq!(status.state, PovNerfSkillState::Invalid);
        assert!(status.permitted_actions.is_empty());

        test_seams::reset_all();
    }

    #[test]
    fn test_status_inspection_fails_closed_when_identity_unavailable() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let skill_id = PovNerfSkillId::Balanced;
        let skill_dir = root.join(skill_id.skill_name());
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_file = skill_dir.join("SKILL.md");

        // Write exact canonical content
        let content = skill_id.render_full_file();
        std::fs::write(&skill_file, &content).unwrap();

        // Inject identity lookup failure for SafeFileExists(None)
        test_seams::reset_all();
        test_seams::set_inject_identity_lookup_failure_path_safety(true);

        let status = inspect_skill_status_at(skill_id, &skill_file);
        assert_eq!(status.state, PovNerfSkillState::Invalid);
        assert!(status.permitted_actions.is_empty());
        assert!(
            status
                .diagnostic_reason
                .as_ref()
                .unwrap()
                .contains("identity could not be verified"),
            "Diagnostic must explain identity verification failure"
        );

        // Direct mutations must all refuse with zero target changes
        let inst = install_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(inst.outcome, PovNerfMutationOutcome::Refused);

        let rem = remove_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(rem.outcome, PovNerfMutationOutcome::Refused);

        let upd = update_nerf_skill_at(root, skill_id).unwrap();
        assert_eq!(upd.outcome, PovNerfMutationOutcome::Refused);

        // Target file is preserved with exact canonical bytes
        assert_eq!(std::fs::read_to_string(&skill_file).unwrap(), content);

        test_seams::reset_all();
    }
}
