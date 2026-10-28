use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const FILES: [(&str, &str); 2] = [
    ("deepseek_harness", "deepseek-harness.md"),
    ("codex_cli", "codex-cli.md"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalRuleStatus {
    pub id: String,
    pub file_name: String,
    pub path: String,
    pub exists: bool,
    pub content: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveGlobalRuleRequest {
    pub id: String,
    pub content: String,
    pub expected_hash: String,
}

fn resolve_rule_file(dir: &Path, id: &str) -> Result<PathBuf, String> {
    let file = FILES
        .iter()
        .find(|(candidate, _)| *candidate == id)
        .ok_or_else(|| format!("Unsupported global rule id: {id}"))?;
    Ok(dir.join(file.1))
}

fn digest(bytes: &[u8]) -> String {
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
                    if meta.file_attributes() & 0x400 != 0 {
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

pub fn read_at(dir: &Path, id: &str) -> Result<GlobalRuleStatus, String> {
    let path = resolve_rule_file(dir, id)?;
    ensure_no_reparse(dir)?;
    ensure_no_reparse(&path)?;
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(format!("Cannot read global rule {}: {e}", path.display())),
    };
    let content = String::from_utf8(bytes.clone())
        .map_err(|_| format!("Global rule is not UTF-8: {}", path.display()))?;
    Ok(GlobalRuleStatus {
        id: id.to_string(),
        file_name: path.file_name().unwrap().to_string_lossy().to_string(),
        path: path.to_string_lossy().to_string(),
        exists: path.exists(),
        content,
        content_hash: digest(&bytes),
    })
}

#[cfg(windows)]
fn replace_file(temp: &Path, target: &Path) -> std::io::Result<()> {
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
fn replace_file(temp: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::rename(temp, target)
}

pub fn save_at(dir: &Path, request: SaveGlobalRuleRequest) -> Result<GlobalRuleStatus, String> {
    save_at_with_replace(dir, request, replace_file)
}

fn save_at_with_replace<F>(
    dir: &Path,
    request: SaveGlobalRuleRequest,
    replace: F,
) -> Result<GlobalRuleStatus, String>
where
    F: Fn(&Path, &Path) -> std::io::Result<()>,
{
    let target = resolve_rule_file(dir, &request.id)?;
    ensure_no_reparse(dir)?;
    ensure_no_reparse(&target)?;
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Cannot create Antigravity rules directory: {e}"))?;
    let current = match std::fs::read(&target) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(format!("Cannot read current rule before saving: {e}")),
    };
    if digest(&current) != request.expected_hash {
        return Err("ConcurrentModificationConflict: reload the rule before saving".to_string());
    }
    let existed = target.exists();
    let backup = if existed {
        let path = target.with_file_name(format!(
            "{}.backup-{}",
            target.file_name().unwrap().to_string_lossy(),
            uuid::Uuid::new_v4()
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("Cannot create backup {}: {e}", path.display()))?;
        use std::io::Write;
        file.write_all(&current)
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("Cannot persist backup {}: {e}", path.display()))?;
        drop(file);
        Some(path)
    } else {
        None
    };
    let temp = target.with_file_name(format!(
        ".{}.tmp-{}",
        target.file_name().unwrap().to_string_lossy(),
        uuid::Uuid::new_v4()
    ));
    let write_result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        use std::io::Write;
        file.write_all(request.content.as_bytes())?;
        file.sync_all()?;
        drop(file);
        ensure_no_reparse(&target).map_err(std::io::Error::other)?;
        let now = match std::fs::read(&target) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !existed => Vec::new(),
            Err(e) => {
                return Err(std::io::Error::other(format!(
                    "Cannot re-read rule before replacement: {e}"
                )))
            }
        };
        if digest(&now) != request.expected_hash {
            return Err(std::io::Error::other("ConcurrentModificationConflict"));
        }
        if existed {
            replace(&temp, &target)?;
        } else {
            std::fs::rename(&temp, &target)?;
        }
        Ok::<(), std::io::Error>(())
    })();
    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&temp);
        return Err(match backup {
            Some(path) => format!(
                "Global rule save failed; prior content preserved at {}: {error}",
                path.display()
            ),
            None => format!("Global rule save failed: {error}"),
        });
    }
    let actual = std::fs::read(&target).map_err(|e| {
        format!(
            "Rule replaced but read-back failed; backup retained at {:?}: {e}",
            backup
        )
    })?;
    if digest(&actual) != digest(request.content.as_bytes()) {
        return Err(format!(
            "Rule replaced but verification failed; backup retained at {:?}",
            backup
        ));
    }
    Ok(read_at(dir, &request.id)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_rule_ids_load_absent_without_creating_files() {
        let temp = tempfile::tempdir().unwrap();
        let status = read_at(temp.path(), "codex_cli").unwrap();
        assert!(!status.exists);
        assert_eq!(status.content, "");
        assert!(!temp.path().join("codex-cli.md").exists());
        assert!(read_at(temp.path(), "../../secret").is_err());
    }
    #[test]
    fn save_creates_only_fixed_rule_and_detects_conflicts() {
        let temp = tempfile::tempdir().unwrap();
        let initial = read_at(temp.path(), "deepseek_harness").unwrap();
        let saved = save_at(
            temp.path(),
            SaveGlobalRuleRequest {
                id: initial.id.clone(),
                content: "日本語\r\nrule".into(),
                expected_hash: initial.content_hash.clone(),
            },
        )
        .unwrap();
        assert_eq!(saved.content, "日本語\r\nrule");
        assert!(saved.exists);
        assert!(save_at(
            temp.path(),
            SaveGlobalRuleRequest {
                id: initial.id,
                content: "stale".into(),
                expected_hash: initial.content_hash
            }
        )
        .unwrap_err()
        .contains("ConcurrentModificationConflict"));
        let file = temp.path().join("deepseek-harness.md");
        assert_eq!(std::fs::read_to_string(file).unwrap(), "日本語\r\nrule");
    }
    #[test]
    fn save_existing_rule_retains_backup() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path()).unwrap();
        std::fs::write(temp.path().join("codex-cli.md"), "before").unwrap();
        let status = read_at(temp.path(), "codex_cli").unwrap();
        save_at(
            temp.path(),
            SaveGlobalRuleRequest {
                id: status.id,
                content: "after".into(),
                expected_hash: status.content_hash,
            },
        )
        .unwrap();
        let backups = std::fs::read_dir(temp.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains("backup-"))
            .collect::<Vec<_>>();
        assert_eq!(backups.len(), 1);
        assert_eq!(std::fs::read(backups[0].path()).unwrap(), b"before");
    }
    #[test]
    fn replacement_failure_preserves_original_and_backup() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("deepseek-harness.md");
        std::fs::write(&target, "before").unwrap();
        let status = read_at(temp.path(), "deepseek_harness").unwrap();
        let error = save_at_with_replace(
            temp.path(),
            SaveGlobalRuleRequest {
                id: status.id,
                content: "after".into(),
                expected_hash: status.content_hash,
            },
            |_temp, _target| Err(std::io::Error::other("injected replace failure")),
        )
        .unwrap_err();
        assert!(error.contains("injected replace failure"));
        assert_eq!(std::fs::read(&target).unwrap(), b"before");
        let entries = std::fs::read_dir(temp.path())
            .unwrap()
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        let backups = entries
            .iter()
            .filter(|entry| entry.file_name().to_string_lossy().contains("backup-"))
            .collect::<Vec<_>>();
        assert_eq!(backups.len(), 1);
        assert_eq!(std::fs::read(backups[0].path()).unwrap(), b"before");
        assert_eq!(
            entries.len(),
            2,
            "failed temporary replacement must be removed"
        );
    }
}
