use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewerPolicy {
    CodexOnly,
    CodexPlusDeepseek,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewerPolicyConfig {
    pub schema_version: u32,
    pub policy: ReviewerPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
}

impl Default for ReviewerPolicyConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            policy: ReviewerPolicy::CodexPlusDeepseek,
            content_hash: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowSkillsPolicyStatus {
    pub compliant: bool,
    pub details: Vec<crate::AntigravityCommandItemInfo>,
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

pub fn inspect_reviewer_policy_at(policy_path: &Path) -> Result<ReviewerPolicyConfig, String> {
    if let Some(parent) = policy_path.parent() {
        if parent.exists() {
            ensure_no_reparse(parent)?;
        }
    }

    let meta = match std::fs::symlink_metadata(policy_path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Missing file returns safe backward-compatible default
            return Ok(ReviewerPolicyConfig::default());
        }
        Err(e) => return Err(format!("Cannot inspect policy file metadata: {e}")),
    };

    if !meta.is_file() {
        return Err("Policy path exists but is not a regular file".to_string());
    }

    ensure_no_reparse(policy_path)?;

    let bytes = std::fs::read(policy_path)
        .map_err(|e| format!("Cannot read reviewer policy file: {e}"))?;

    let hash = format!("{:x}", Sha256::digest(&bytes));

    let parsed: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Malformed reviewer policy JSON: {e}"))?;

    let version = parsed
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "Missing or invalid 'schema_version' in reviewer policy".to_string())?;

    if version != 1 {
        return Err(format!(
            "Unsupported reviewer policy schema_version: {version}. Expected 1."
        ));
    }

    let policy_str = parsed
        .get("policy")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing or invalid 'policy' field in reviewer policy".to_string())?;

    let policy = match policy_str {
        "codex_only" => ReviewerPolicy::CodexOnly,
        "codex_plus_deepseek" => ReviewerPolicy::CodexPlusDeepseek,
        other => return Err(format!("Unknown reviewer policy value: '{other}'")),
    };

    Ok(ReviewerPolicyConfig {
        schema_version: 1,
        policy,
        content_hash: Some(hash),
    })
}

pub fn save_reviewer_policy_at(
    policy_path: &Path,
    policy: ReviewerPolicy,
    skills_root: &Path,
    expected_hash: Option<&str>,
) -> Result<ReviewerPolicyConfig, String> {
    save_reviewer_policy_internal(policy_path, policy, skills_root, expected_hash, || Ok(()))
}

pub(crate) fn save_reviewer_policy_internal<F>(
    policy_path: &Path,
    policy: ReviewerPolicy,
    skills_root: &Path,
    expected_hash: Option<&str>,
    pre_replace_hook: F,
) -> Result<ReviewerPolicyConfig, String>
where
    F: FnOnce() -> Result<(), String>,
{
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

    let config_to_write = ReviewerPolicyConfig {
        schema_version: 1,
        policy,
        content_hash: None,
    };

    let serialized = serde_json::to_vec_pretty(&config_to_write)
        .map_err(|e| format!("Failed to serialize reviewer policy: {e}"))?;

    let temp_name = format!(
        ".pov-reviewer-policy.json.tmp-{}",
        uuid::Uuid::new_v4()
    );
    let temp_path = parent.join(&temp_name);

    let write_res = (|| {
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
                // Target must not exist. Move without MOVEFILE_REPLACE_EXISTING ensures
                // it fails if target was created concurrently.
                move_file_new_only_win(&temp_path, policy_path)?;
            } else {
                // Target exists. ReplaceFileW with backup so we can verify the replaced bytes.
                let backup_name = format!(".pov-reviewer-policy.json.bak-{}", uuid::Uuid::new_v4());
                let backup_path = parent.join(&backup_name);
                replace_file_with_backup_win(&temp_path, policy_path, &backup_path)?;

                let replaced_bytes = std::fs::read(&backup_path)?;
                let replaced_hash = format!("{:x}", Sha256::digest(&replaced_bytes));
                let expected = expected_hash.unwrap();
                if !replaced_hash.eq_ignore_ascii_case(expected) {
                    // Intervening change occurred at replacement: attempt rollback!
                    let restore_res = replace_file_win(&backup_path, policy_path);
                    if let Err(restore_err) = restore_res {
                        // DO NOT remove backup! Retain it so user's content is preserved!
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
                // Atomic no-replace on Unix: hard_link fails with EEXIST if destination exists
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
    if read_back.schema_version != 1 || read_back.policy != policy {
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
    // flags = 0 ensures it fails if target exists (rejects concurrent creation)
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
    // 1. Initial verification before swap/replace
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

    // 2. Attempt atomic exchange (Linux renameat2 RENAME_EXCHANGE / macOS renamex_np RENAME_SWAP)
    let swapped = try_atomic_exchange_unix(temp, target);
    match swapped {
        Ok(()) => {
            // temp now contains the replaced file bytes!
            let replaced_bytes = std::fs::read(temp)?;
            let replaced_hash = format!("{:x}", Sha256::digest(&replaced_bytes));
            if !replaced_hash.eq_ignore_ascii_case(expected_hash) {
                // Intervening change occurred! Rollback by swapping back.
                let swap_back = try_atomic_exchange_unix(temp, target);
                if let Err(e) = swap_back {
                    // Retain temp file so user data is never destroyed
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
            // Systems without atomic exchange: disable policy update to prevent race conditions per §6.2
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
    fn missing_policy_returns_default() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pov-reviewer-policy.json");
        let loaded = inspect_reviewer_policy_at(&path).unwrap();
        assert_eq!(loaded, ReviewerPolicyConfig::default());
        assert_eq!(loaded.schema_version, 1);
        assert_eq!(loaded.policy, ReviewerPolicy::CodexPlusDeepseek);
    }

    #[test]
    fn valid_policy_round_trips() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pov-reviewer-policy.json");
        let content = r#"{"schema_version":1,"policy":"codex_only"}"#;
        std::fs::write(&path, content).unwrap();

        let loaded = inspect_reviewer_policy_at(&path).unwrap();
        assert_eq!(loaded.schema_version, 1);
        assert_eq!(loaded.policy, ReviewerPolicy::CodexOnly);
    }

    #[test]
    fn malformed_json_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pov-reviewer-policy.json");
        std::fs::write(&path, "{not-json}").unwrap();

        let err = inspect_reviewer_policy_at(&path).unwrap_err();
        assert!(err.contains("Malformed reviewer policy JSON"));
    }

    #[test]
    fn unknown_version_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pov-reviewer-policy.json");
        std::fs::write(&path, r#"{"schema_version":99,"policy":"codex_only"}"#).unwrap();

        let err = inspect_reviewer_policy_at(&path).unwrap_err();
        assert!(err.contains("Unsupported reviewer policy schema_version"));
    }

    #[test]
    fn unknown_policy_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pov-reviewer-policy.json");
        std::fs::write(&path, r#"{"schema_version":1,"policy":"claude_super"}"#).unwrap();

        let err = inspect_reviewer_policy_at(&path).unwrap_err();
        assert!(err.contains("Unknown reviewer policy value"));
    }

    #[test]
    fn save_policy_blocked_when_skills_not_compliant() {
        let temp_ide = tempfile::tempdir().unwrap();
        let temp_skills = tempfile::tempdir().unwrap();
        let policy_path = temp_ide.path().join("pov-reviewer-policy.json");

        // With no skills installed, save must fail
        let err = save_reviewer_policy_at(&policy_path, ReviewerPolicy::CodexOnly, temp_skills.path(), None)
            .unwrap_err();
        assert!(err.contains("must be installed and up-to-date"));
        assert!(!policy_path.exists());
    }

    #[test]
    fn save_policy_succeeds_when_skills_compliant() {
        let temp_ide = tempfile::tempdir().unwrap();
        let temp_skills = tempfile::tempdir().unwrap();
        let policy_path = temp_ide.path().join("pov-reviewer-policy.json");

        // Install all 3 workflow skills into temp_skills
        for name in ["pov-plan-only", "pov-build-review", "pov-full-cycle"] {
            crate::install_antigravity_command_at(temp_skills.path(), name).unwrap();
        }

        let saved = save_reviewer_policy_at(&policy_path, ReviewerPolicy::CodexOnly, temp_skills.path(), None)
            .unwrap();
        assert_eq!(saved.policy, ReviewerPolicy::CodexOnly);
        assert_eq!(saved.schema_version, 1);
        assert!(saved.content_hash.is_some());

        let reloaded = inspect_reviewer_policy_at(&policy_path).unwrap();
        assert_eq!(reloaded, saved);
    }

    #[test]
    fn save_policy_detects_concurrent_modification_conflict() {
        let temp_ide = tempfile::tempdir().unwrap();
        let temp_skills = tempfile::tempdir().unwrap();
        let policy_path = temp_ide.path().join("pov-reviewer-policy.json");

        for name in ["pov-plan-only", "pov-build-review", "pov-full-cycle"] {
            crate::install_antigravity_command_at(temp_skills.path(), name).unwrap();
        }

        // 1. Initial save without expected_hash when file is absent succeeds
        let initial = save_reviewer_policy_at(&policy_path, ReviewerPolicy::CodexOnly, temp_skills.path(), None)
            .unwrap();
        let hash1 = initial.content_hash.expect("must have content hash");

        // 2. Saving with correct expected_hash succeeds
        let updated = save_reviewer_policy_at(
            &policy_path,
            ReviewerPolicy::CodexPlusDeepseek,
            temp_skills.path(),
            Some(&hash1),
        )
        .unwrap();
        let hash2 = updated.content_hash.expect("must have updated content hash");
        assert_ne!(hash1, hash2);

        // 3. Saving with stale hash1 fails with conflict
        let conflict_err = save_reviewer_policy_at(
            &policy_path,
            ReviewerPolicy::CodexOnly,
            temp_skills.path(),
            Some(&hash1),
        )
        .unwrap_err();
        assert!(conflict_err.contains("Concurrent modification conflict"));

        // 4. Saving with None when file exists fails with conflict
        let none_err = save_reviewer_policy_at(
            &policy_path,
            ReviewerPolicy::CodexOnly,
            temp_skills.path(),
            None,
        )
        .unwrap_err();
        assert!(none_err.contains("Concurrent modification conflict"));

        // 5. If file is deleted externally, saving with expected_hash fails with conflict
        std::fs::remove_file(&policy_path).unwrap();
        let deleted_err = save_reviewer_policy_at(
            &policy_path,
            ReviewerPolicy::CodexOnly,
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

        // Save initial policy
        let initial = save_reviewer_policy_at(&policy_path, ReviewerPolicy::CodexOnly, temp_skills.path(), None)
            .unwrap();
        let initial_hash = initial.content_hash.unwrap();

        let intervening_content = r#"{"schema_version":1,"policy":"codex_only","tampered":true}"#;

        // Try saving with expected_hash, but an intervening modification happens between initial check and replace
        let err = save_reviewer_policy_internal(
            &policy_path,
            ReviewerPolicy::CodexPlusDeepseek,
            temp_skills.path(),
            Some(&initial_hash),
            || {
                // Intervening write to policy_path
                std::fs::write(&policy_path, intervening_content).unwrap();
                Ok(())
            },
        )
        .unwrap_err();

        assert!(err.contains("Concurrent modification conflict"));
        // The intervening content must remain intact and preserved!
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

        let created_concurrently = r#"{"schema_version":1,"policy":"codex_plus_deepseek","created":true}"#;

        // Target does not exist initially (expected_hash = None), but is created concurrently before replace
        let err = save_reviewer_policy_internal(
            &policy_path,
            ReviewerPolicy::CodexOnly,
            temp_skills.path(),
            None,
            || {
                std::fs::write(&policy_path, created_concurrently).unwrap();
                Ok(())
            },
        )
        .unwrap_err();

        assert!(err.contains("Concurrent modification conflict"));
        // The created content must remain intact and preserved!
        assert_eq!(std::fs::read_to_string(&policy_path).unwrap(), created_concurrently);
    }
}
