use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const GLOBAL_RULE_CODEX_CLI_MD: &str =
    include_str!("../resources/antigravity-global-rules/codex-cli.md");
pub const GLOBAL_RULE_DEEPSEEK_HARNESS_MD: &str =
    include_str!("../resources/antigravity-global-rules/deepseek-harness.md");

const FIXED_RULES: [(&str, &str, &str); 2] = [
    ("codex_cli", "codex-cli.md", GLOBAL_RULE_CODEX_CLI_MD),
    (
        "deepseek_harness",
        "deepseek-harness.md",
        GLOBAL_RULE_DEEPSEEK_HARNESS_MD,
    ),
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GlobalRuleTemplateStatus {
    NotInstalled,
    Installed,
    ModifiedOrProtected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlobalRuleItemInfo {
    pub id: String,
    pub file_name: String,
    pub path: String,
    pub status: GlobalRuleTemplateStatus,
    pub can_remove: bool,
    pub error: Option<String>,
}

fn is_removal_supported() -> bool {
    cfg!(windows)
}

fn resolve_rule(id: &str) -> Result<(&'static str, &'static str, &'static str), String> {
    FIXED_RULES
        .iter()
        .copied()
        .find(|(candidate, _, _)| *candidate == id)
        .ok_or_else(|| format!("Unsupported global rule id: '{id}'. Only fixed templates 'codex_cli' and 'deepseek_harness' are supported."))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
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

pub fn inspect_global_rules_status_at(dir: &Path) -> Result<Vec<GlobalRuleItemInfo>, String> {
    let mut results = Vec::new();
    for (id, _, _) in FIXED_RULES {
        results.push(inspect_single_global_rule_at(dir, id)?);
    }
    Ok(results)
}

pub fn inspect_single_global_rule_at(dir: &Path, id: &str) -> Result<GlobalRuleItemInfo, String> {
    let (rule_id, file_name, template) = resolve_rule(id)?;
    let target = dir.join(file_name);
    let path_str = target.to_string_lossy().to_string();

    if let Err(err) = ensure_no_reparse(dir) {
        return Ok(GlobalRuleItemInfo {
            id: rule_id.to_string(),
            file_name: file_name.to_string(),
            path: path_str,
            status: GlobalRuleTemplateStatus::ModifiedOrProtected,
            can_remove: is_removal_supported(),
            error: Some(err),
        });
    }

    if let Err(err) = ensure_no_reparse(&target) {
        return Ok(GlobalRuleItemInfo {
            id: rule_id.to_string(),
            file_name: file_name.to_string(),
            path: path_str,
            status: GlobalRuleTemplateStatus::ModifiedOrProtected,
            can_remove: is_removal_supported(),
            error: Some(err),
        });
    }

    let meta = match std::fs::symlink_metadata(&target) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(GlobalRuleItemInfo {
                id: rule_id.to_string(),
                file_name: file_name.to_string(),
                path: path_str,
                status: GlobalRuleTemplateStatus::NotInstalled,
                can_remove: is_removal_supported(),
                error: None,
            });
        }
        Err(e) => {
            return Ok(GlobalRuleItemInfo {
                id: rule_id.to_string(),
                file_name: file_name.to_string(),
                path: path_str,
                status: GlobalRuleTemplateStatus::ModifiedOrProtected,
                can_remove: is_removal_supported(),
                error: Some(format!("Failed to inspect rule file metadata: {e}")),
            });
        }
    };

    if !meta.is_file() {
        return Ok(GlobalRuleItemInfo {
            id: rule_id.to_string(),
            file_name: file_name.to_string(),
            path: path_str,
            status: GlobalRuleTemplateStatus::ModifiedOrProtected,
            can_remove: is_removal_supported(),
            error: Some("Rule path exists but is not a regular file".to_string()),
        });
    }

    match std::fs::read(&target) {
        Ok(bytes) => {
            if bytes == template.as_bytes() {
                Ok(GlobalRuleItemInfo {
                    id: rule_id.to_string(),
                    file_name: file_name.to_string(),
                    path: path_str,
                    status: GlobalRuleTemplateStatus::Installed,
                    can_remove: is_removal_supported(),
                    error: None,
                })
            } else {
                Ok(GlobalRuleItemInfo {
                    id: rule_id.to_string(),
                    file_name: file_name.to_string(),
                    path: path_str,
                    status: GlobalRuleTemplateStatus::ModifiedOrProtected,
                    can_remove: is_removal_supported(),
                    error: Some(
                        "File exists but content differs from standard template; protected from overwrite/removal".to_string(),
                    ),
                })
            }
        }
        Err(e) => Ok(GlobalRuleItemInfo {
            id: rule_id.to_string(),
            file_name: file_name.to_string(),
            path: path_str,
            status: GlobalRuleTemplateStatus::ModifiedOrProtected,
            can_remove: is_removal_supported(),
            error: Some(format!("Failed to read rule file: {e}")),
        }),
    }
}

pub fn install_global_rule_at(dir: &Path, id: &str) -> Result<GlobalRuleItemInfo, String> {
    install_global_rule_internal(dir, id, || Ok(()))
}

pub(crate) fn install_global_rule_internal<F>(
    dir: &Path,
    id: &str,
    pre_placement_hook: F,
) -> Result<GlobalRuleItemInfo, String>
where
    F: FnOnce() -> Result<(), String>,
{
    let (rule_id, file_name, template) = resolve_rule(id)?;
    let target = dir.join(file_name);

    ensure_no_reparse(dir)?;
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create global rules directory: {e}"))?;

    ensure_no_reparse(&target)?;

    let current = inspect_single_global_rule_at(dir, rule_id)?;
    match current.status {
        GlobalRuleTemplateStatus::Installed => return Ok(current),
        GlobalRuleTemplateStatus::ModifiedOrProtected => {
            return Err(
                "Rule file already exists with modified or unmanaged content; refusing to overwrite"
                    .to_string(),
            );
        }
        GlobalRuleTemplateStatus::NotInstalled => {}
    }

    let temp_name = format!(".{file_name}.tmp-{}", uuid::Uuid::new_v4());
    let temp_path = dir.join(&temp_name);

    let write_res = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        use std::io::Write;
        file.write_all(template.as_bytes())?;
        file.sync_all()?;
        drop(file);

        // Execute hook before placement
        pre_placement_hook().map_err(std::io::Error::other)?;

        ensure_no_reparse(&target).map_err(std::io::Error::other)?;
        if target.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Concurrent modification conflict: rule file appeared before placement; refusing to overwrite",
            ));
        }

        #[cfg(windows)]
        {
            move_file_new_only_win(&temp_path, &target)?;
        }
        #[cfg(not(windows))]
        {
            let link_res = std::fs::hard_link(&temp_path, &target);
            let _ = std::fs::remove_file(&temp_path);
            link_res?;
        }

        Ok::<(), std::io::Error>(())
    })();

    if let Err(err) = write_res {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("Failed to install global rule template: {err}"));
    }

    let read_back = std::fs::read(&target)
        .map_err(|e| format!("Rule installed but read-back verification failed: {e}"))?;
    if read_back != template.as_bytes() {
        return Err("Rule installed but read-back content mismatch".to_string());
    }

    inspect_single_global_rule_at(dir, rule_id)
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
fn delete_verified_template_file(path: &Path, expected_template: &str) -> Result<(), String> {
    delete_verified_template_file_with_hook(path, expected_template, |_p, _h| Ok(()))
}

#[cfg(windows)]
pub(crate) fn delete_verified_template_file_with_hook<F>(
    path: &Path,
    expected_template: &str,
    post_verify_pre_disp_hook: F,
) -> Result<(), String>
where
    F: FnOnce(&Path, windows_sys::Win32::Foundation::HANDLE) -> Result<(), String>,
{
    use std::io::{Read, Seek, SeekFrom};
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, SetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        FileDispositionInfo, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
        FILE_DISPOSITION_INFO, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_GENERIC_READ, DELETE,
    };

    // Open exclusively against writers: share_mode excludes FILE_SHARE_WRITE so no other process
    // can open or modify the file while verification and deletion disposition are taking place.
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .access_mode(FILE_GENERIC_READ | DELETE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|e| format!("Cannot open file with verified exclusive delete rights: {e}"))?;

    let handle = file.as_raw_handle() as HANDLE;

    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    let info_ok = unsafe { GetFileInformationByHandle(handle, &mut info) };
    if info_ok == 0 {
        return Err("Failed to query file information from open handle".to_string());
    }

    if (info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
        return Err("Cannot remove reparse point or symlink".to_string());
    }
    if (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0 {
        return Err("Target is a directory, not a regular file".to_string());
    }

    let mut content = Vec::new();
    file.read_to_end(&mut content)
        .map_err(|e| format!("Failed to read content from open handle: {e}"))?;

    if content != expected_template.as_bytes() {
        return Err(
            "Rule file content differs from standard template; refusing to remove user-modified or protected file"
                .to_string(),
        );
    }

    // Call hook between read verification and disposition (used by deterministic tests)
    post_verify_pre_disp_hook(path, handle)?;

    // Re-verify content through handle to guarantee no in-place modification occurred
    file.seek(SeekFrom::Start(0))
        .map_err(|e| format!("Failed to rewind handle: {e}"))?;
    let mut recheck = Vec::new();
    file.read_to_end(&mut recheck)
        .map_err(|e| format!("Failed to re-read content from handle: {e}"))?;
    if recheck != expected_template.as_bytes() {
        return Err("Rule file content changed before disposition; refusing deletion".to_string());
    }

    let mut disp = FILE_DISPOSITION_INFO { DeleteFile: 1 };
    let disp_ok = unsafe {
        SetFileInformationByHandle(
            handle,
            FileDispositionInfo,
            &mut disp as *mut _ as *mut _,
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    };
    if disp_ok == 0 {
        return Err("Failed to set delete disposition on verified handle".to_string());
    }

    // Closing the handle commits the deletion bound to the verified file object
    drop(file);

    if path.exists() {
        return Err("Rule file still exists after handle-bound deletion".to_string());
    }

    Ok(())
}

#[cfg(not(windows))]
fn delete_verified_template_file(_path: &Path, _expected_template: &str) -> Result<(), String> {
    Err("Verified-handle deletion is not supported on this platform; removal disabled".to_string())
}

pub fn remove_global_rule_at(dir: &Path, id: &str) -> Result<GlobalRuleItemInfo, String> {
    let (rule_id, file_name, template) = resolve_rule(id)?;
    let target = dir.join(file_name);

    ensure_no_reparse(dir)?;
    ensure_no_reparse(&target)?;

    let current = inspect_single_global_rule_at(dir, rule_id)?;
    match current.status {
        GlobalRuleTemplateStatus::NotInstalled => return Ok(current),
        GlobalRuleTemplateStatus::ModifiedOrProtected => {
            return Err(
                "Rule file content differs from standard template; refusing to remove user-modified or protected file"
                    .to_string(),
            );
        }
        GlobalRuleTemplateStatus::Installed => {}
    }

    // Atomically verify content on an open handle and bind deletion to that exact handle
    delete_verified_template_file(&target, template)?;

    inspect_single_global_rule_at(dir, rule_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspect_absent_rules_returns_not_installed() {
        let temp = tempfile::tempdir().unwrap();
        let list = inspect_global_rules_status_at(temp.path()).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "codex_cli");
        assert_eq!(list[0].status, GlobalRuleTemplateStatus::NotInstalled);
        assert_eq!(list[1].id, "deepseek_harness");
        assert_eq!(list[1].status, GlobalRuleTemplateStatus::NotInstalled);
    }

    #[test]
    fn install_and_remove_lifecycle() {
        let temp = tempfile::tempdir().unwrap();
        let installed = install_global_rule_at(temp.path(), "codex_cli").unwrap();
        assert_eq!(installed.status, GlobalRuleTemplateStatus::Installed);
        assert!(temp.path().join("codex-cli.md").exists());
        assert_eq!(
            std::fs::read(temp.path().join("codex-cli.md")).unwrap(),
            GLOBAL_RULE_CODEX_CLI_MD.as_bytes()
        );

        // Idempotent install returns Installed without error
        let reinstalled = install_global_rule_at(temp.path(), "codex_cli").unwrap();
        assert_eq!(reinstalled.status, GlobalRuleTemplateStatus::Installed);

        // Remove exact match succeeds
        let removed = remove_global_rule_at(temp.path(), "codex_cli").unwrap();
        assert_eq!(removed.status, GlobalRuleTemplateStatus::NotInstalled);
        assert!(!temp.path().join("codex-cli.md").exists());

        // Idempotent remove returns NotInstalled
        let removed2 = remove_global_rule_at(temp.path(), "codex_cli").unwrap();
        assert_eq!(removed2.status, GlobalRuleTemplateStatus::NotInstalled);
    }

    #[test]
    fn modified_file_is_protected_from_overwrite_and_removal() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("deepseek-harness.md");
        std::fs::write(&target, "user customized content").unwrap();

        let status = inspect_single_global_rule_at(temp.path(), "deepseek_harness").unwrap();
        assert_eq!(status.status, GlobalRuleTemplateStatus::ModifiedOrProtected);

        // Overwrite refused
        let install_err = install_global_rule_at(temp.path(), "deepseek_harness").unwrap_err();
        assert!(install_err.contains("already exists with modified"));
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"user customized content"
        );

        // Removal refused
        let remove_err = remove_global_rule_at(temp.path(), "deepseek_harness").unwrap_err();
        assert!(remove_err.contains("refusing to remove user-modified"));
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"user customized content"
        );
    }

    #[test]
    fn invalid_rule_id_rejected() {
        let temp = tempfile::tempdir().unwrap();
        assert!(inspect_single_global_rule_at(temp.path(), "unknown_rule").is_err());
        assert!(install_global_rule_at(temp.path(), "unknown_rule").is_err());
        assert!(remove_global_rule_at(temp.path(), "unknown_rule").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn test_global_rule_removal_prevents_in_place_write_and_preserves_on_conflict() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("codex-cli.md");
        std::fs::write(&target, GLOBAL_RULE_CODEX_CLI_MD.as_bytes()).unwrap();

        // 1. In-place write attempted while verified handle is open must be rejected by Windows kernel
        let mut write_rejected = false;
        let res = delete_verified_template_file_with_hook(&target, GLOBAL_RULE_CODEX_CLI_MD, |p, _h| {
            match std::fs::OpenOptions::new().write(true).open(p) {
                Ok(_) => Err("Should have been blocked from opening for write!".to_string()),
                Err(_) => {
                    write_rejected = true;
                    Ok(())
                }
            }
        });
        assert!(res.is_ok());
        assert!(write_rejected);

        // 2. Intervening conflict reported before disposition aborts deletion and preserves file
        std::fs::write(&target, GLOBAL_RULE_CODEX_CLI_MD.as_bytes()).unwrap();
        let err = delete_verified_template_file_with_hook(&target, GLOBAL_RULE_CODEX_CLI_MD, |_p, _h| {
            Err("Simulated intervening race condition".to_string())
        }).unwrap_err();
        assert!(err.contains("Simulated intervening race condition"));
        assert!(target.exists());
        assert_eq!(
            std::fs::read(&target).unwrap(),
            GLOBAL_RULE_CODEX_CLI_MD.as_bytes()
        );
    }

    #[test]
    fn test_global_rule_install_rejects_concurrent_creation_and_preserves_content() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("codex-cli.md");
        let concurrent_content = b"concurrently created by another process";

        let err = install_global_rule_internal(temp.path(), "codex_cli", || {
            // Concurrent creation happens between absence check and placement
            std::fs::write(&target, concurrent_content).unwrap();
            Ok(())
        })
        .unwrap_err();

        assert!(
            err.contains("refusing to overwrite")
                || err.contains("appeared before placement")
                || err.contains("already exists")
                || err.contains("Cannot install")
        );
        // The concurrently created file must be preserved byte-for-byte!
        assert_eq!(std::fs::read(&target).unwrap(), concurrent_content);
    }
}
