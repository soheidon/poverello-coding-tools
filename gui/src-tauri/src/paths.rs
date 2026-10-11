use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// AppChannel — compile-time channel selection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppChannel {
    Dev,
    Stable,
}

/// Channel-unset defaults to Dev (safe side).
/// Only an explicit ANTHRO_BRIDGE_CHANNEL=stable selects Stable.
pub(crate) fn app_channel() -> AppChannel {
    match option_env!("ANTHRO_BRIDGE_CHANNEL") {
        Some("stable") => AppChannel::Stable,
        Some("dev") | None => AppChannel::Dev,
        Some(other) => {
            eprintln!(
                "Unknown ANTHRO_BRIDGE_CHANNEL={other}; using isolated dev data directory"
            );
            AppChannel::Dev
        }
    }
}

// ---------------------------------------------------------------------------
// Pure path functions — no env var access, testable without #[ignore]
// ---------------------------------------------------------------------------

/// Pure — no env var access. Testable with temp paths.
pub(crate) fn user_data_dir_for(base: &Path, channel: AppChannel) -> PathBuf {
    match channel {
        AppChannel::Dev => base.join("Poverello Dev"),
        AppChannel::Stable => base.join("Poverello"),
    }
}

const MIGRATION_MARKER: &str = ".poverello-data-migration-v1";
const MIGRATION_MARKER_CONTENT: &[u8] = b"poverello-user-data-migration-v1\n";

/// Copy a legacy profile to the new product data root before any app subsystem
/// reads or writes it. The source is deliberately retained as a recovery copy.
pub(crate) fn migrate_user_data_dir(base: &Path, channel: AppChannel) -> Result<(), String> {
    migrate_user_data_dir_with(base, channel, copy_tree_verified, publish_no_replace)
}

fn migrate_user_data_dir_with<C, P>(
    base: &Path,
    channel: AppChannel,
    copy_tree: C,
    publish_tree: P,
) -> Result<(), String>
where
    C: Fn(&Path, &Path) -> Result<(), String>,
    P: Fn(&Path, &Path) -> Result<(), String>,
{
    let (old_name, new_root_name) = match channel {
        AppChannel::Stable => ("Anthro Bridge", "Poverello"),
        AppChannel::Dev => ("Anthro Bridge Dev", "Poverello Dev"),
    };
    let source = base.join(old_name);
    let destination = base.join(new_root_name);
    let marker = destination.join(MIGRATION_MARKER);
    let destination_metadata = symlink_metadata_if_exists(&destination)?;

    if destination_metadata.is_some() {
        reject_reparse_point(&destination)?;
        if !destination_metadata
            .as_ref()
            .is_some_and(|metadata| metadata.is_dir())
        {
            return Err(format!(
                "Poverello data destination is not a directory: {}",
                destination.display()
            ));
        }
        if symlink_metadata_if_exists(&marker)?.is_some() {
            reject_reparse_point(&marker)?;
            let marker_bytes = std::fs::read(&marker).map_err(|error| {
                format!(
                    "Cannot verify Poverello data migration marker at {}: {error}",
                    marker.display()
                )
            })?;
            if marker_bytes != MIGRATION_MARKER_CONTENT {
                return Err(format!(
                    "Poverello data migration marker is invalid at {}. Both the old and new data folders were preserved; do not delete either folder.",
                    marker.display()
                ));
            }
            return Ok(());
        }
        if symlink_metadata_if_exists(&source)?.is_some() {
            return Err(format!(
                "Both legacy data folder {} and unverified Poverello data folder {} exist. Startup stopped without changing either folder. Back up both folders and resolve the conflict before restarting.",
                source.display(), destination.display()
            ));
        }
        return Ok(());
    }

    if symlink_metadata_if_exists(&source)?.is_none() {
        return Ok(());
    }

    reject_reparse_point(&source)?;
    std::fs::create_dir_all(base).map_err(|error| {
        format!(
            "Cannot prepare the Poverello data parent {}: {error}",
            base.display()
        )
    })?;

    let staging = base.join(format!(".poverello-migration-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&staging).map_err(|error| {
        format!(
            "Cannot create Poverello migration staging directory {}: {error}",
            staging.display()
        )
    })?;

    let result = (|| {
        let before = tree_manifest(&source)?;
        copy_tree(&source, &staging)?;
        let after = tree_manifest(&source)?;
        let staged = tree_manifest(&staging)?;
        if before != after || after != staged {
            return Err("Legacy user data changed during migration or the staged copy did not verify; no destination was published.".to_string());
        }

        let staged_marker = staging.join(MIGRATION_MARKER);
        if staged_marker.exists() {
            return Err(format!(
                "Legacy data contains the reserved migration marker path {}.",
                staged_marker.display()
            ));
        }
        std::fs::write(&staged_marker, MIGRATION_MARKER_CONTENT).map_err(|error| {
            format!(
                "Cannot write migration completion marker {}: {error}",
                staged_marker.display()
            )
        })?;
        let source_metadata = std::fs::symlink_metadata(&source).map_err(|error| {
            format!(
                "Cannot inspect legacy data root {}: {error}",
                source.display()
            )
        })?;
        std::fs::set_permissions(&staging, source_metadata.permissions()).map_err(|error| {
            format!(
                "Cannot preserve legacy root permissions for {}: {error}",
                staging.display()
            )
        })?;
        publish_tree(&staging, &destination)?;

        let published_marker = destination.join(MIGRATION_MARKER);
        let published = std::fs::read(&published_marker).map_err(|error| {
            format!("Poverello data was published but its completion marker could not be verified at {}: {error}", published_marker.display())
        })?;
        if published != MIGRATION_MARKER_CONTENT {
            return Err(format!(
                "Poverello data was published but its completion marker is invalid at {}.",
                published_marker.display()
            ));
        }
        Ok(())
    })();

    let cleanup_result = match symlink_metadata_if_exists(&staging) {
        Ok(Some(_)) => std::fs::remove_dir_all(&staging).map_err(|error| error.to_string()),
        Ok(None) => Ok(()),
        Err(error) => Err(error),
    };
    if let Err(cleanup_error) = cleanup_result {
        return Err(match result {
            Ok(()) => format!("Poverello data migration completed but temporary staging cleanup failed at {}: {cleanup_error}", staging.display()),
            Err(migration_error) => format!("{migration_error}; temporary staging data remains at {} because cleanup failed: {cleanup_error}", staging.display()),
        });
    }
    result
}

fn symlink_metadata_if_exists(path: &Path) -> Result<Option<std::fs::Metadata>, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "Cannot inspect migration path {}: {error}",
            path.display()
        )),
    }
}

fn copy_tree_verified(source: &Path, destination: &Path) -> Result<(), String> {
    reject_reparse_point(source)?;
    copy_directory_contents(source, destination)
}

fn copy_directory_contents(source: &Path, destination: &Path) -> Result<(), String> {
    for entry in std::fs::read_dir(source)
        .map_err(|error| format!("Cannot enumerate legacy data {}: {error}", source.display()))?
    {
        let entry = entry.map_err(|error| {
            format!(
                "Cannot read a legacy data entry under {}: {error}",
                source.display()
            )
        })?;
        let source_path = entry.path();
        reject_reparse_point(&source_path)?;
        let target_path = destination.join(entry.file_name());
        let metadata = std::fs::symlink_metadata(&source_path).map_err(|error| {
            format!(
                "Cannot inspect legacy data {}: {error}",
                source_path.display()
            )
        })?;
        if metadata.is_dir() {
            std::fs::create_dir(&target_path).map_err(|error| {
                format!("Cannot stage directory {}: {error}", target_path.display())
            })?;
            copy_directory_contents(&source_path, &target_path)?;
            std::fs::set_permissions(&target_path, metadata.permissions()).map_err(|error| {
                format!(
                    "Cannot preserve directory permissions for {}: {error}",
                    target_path.display()
                )
            })?;
        } else if metadata.is_file() {
            std::fs::copy(&source_path, &target_path)
                .map_err(|error| format!("Cannot stage file {}: {error}", source_path.display()))?;
            std::fs::set_permissions(&target_path, metadata.permissions()).map_err(|error| {
                format!(
                    "Cannot preserve file permissions for {}: {error}",
                    target_path.display()
                )
            })?;
        } else {
            return Err(format!(
                "Unsupported special file in legacy data: {}",
                source_path.display()
            ));
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum EntryDigest {
    Directory,
    File(u64, [u8; 32]),
}

fn tree_manifest(root: &Path) -> Result<BTreeMap<PathBuf, EntryDigest>, String> {
    reject_reparse_point(root)?;
    let mut manifest = BTreeMap::new();
    collect_tree_manifest(root, root, &mut manifest)?;
    Ok(manifest)
}

fn collect_tree_manifest(
    root: &Path,
    current: &Path,
    manifest: &mut BTreeMap<PathBuf, EntryDigest>,
) -> Result<(), String> {
    for entry in std::fs::read_dir(current)
        .map_err(|error| format!("Cannot enumerate user data {}: {error}", current.display()))?
    {
        let entry = entry.map_err(|error| {
            format!(
                "Cannot read a user data entry under {}: {error}",
                current.display()
            )
        })?;
        let path = entry.path();
        reject_reparse_point(&path)?;
        let relative = path
            .strip_prefix(root)
            .map_err(|_| format!("User data entry escaped its root: {}", path.display()))?
            .to_path_buf();
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|error| format!("Cannot inspect user data {}: {error}", path.display()))?;
        if metadata.is_dir() {
            manifest.insert(relative, EntryDigest::Directory);
            collect_tree_manifest(root, &path, manifest)?;
        } else if metadata.is_file() {
            let bytes = std::fs::read(&path)
                .map_err(|error| format!("Cannot hash user data {}: {error}", path.display()))?;
            let digest: [u8; 32] = Sha256::digest(&bytes).into();
            manifest.insert(relative, EntryDigest::File(metadata.len(), digest));
        } else {
            return Err(format!(
                "Unsupported special file in user data: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn reject_reparse_point(path: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("Cannot inspect migration path {}: {error}", path.display()))?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(format!(
                "Migration refuses symlinks, junctions, and reparse points: {}",
                path.display()
            ));
        }
    }
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "Migration refuses symlinks, junctions, and reparse points: {}",
            path.display()
        ));
    }
    Ok(())
}

fn publish_no_replace(staging: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err(format!(
            "Refusing to replace an existing Poverello data directory: {}",
            destination.display()
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::MoveFileW;
        let from: Vec<u16> = staging.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        // MoveFileW is same-volume and fails if the destination already exists.
        let ok = unsafe { MoveFileW(from.as_ptr(), to.as_ptr()) };
        if ok == 0 {
            return Err(format!(
                "Cannot atomically publish Poverello data directory {}: {}",
                destination.display(),
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(staging, destination).map_err(|error| {
            format!(
                "Cannot atomically publish Poverello data directory {}: {error}",
                destination.display()
            )
        })
    }
}

/// Resolves APPDATA (or USERPROFILE) at runtime.
pub(crate) fn user_data_dir() -> PathBuf {
    let appdata = std::env::var("APPDATA")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    user_data_dir_for(Path::new(&appdata), app_channel())
}

// ---------------------------------------------------------------------------
// Convenience combinators — all rooted in user_data_dir()
// ---------------------------------------------------------------------------

pub(crate) fn config_path() -> PathBuf {
    user_data_dir().join("config.json")
}

pub(crate) fn user_prefs_path() -> PathBuf {
    user_data_dir().join("user_prefs.json")
}

pub(crate) fn openrouter_models_cache_path() -> PathBuf {
    user_data_dir().join("openrouter_models.json")
}

pub(crate) fn log_dir() -> PathBuf {
    user_data_dir().join("Communication-Logs")
}

pub(crate) fn orchestrator_runs_dir() -> PathBuf {
    user_data_dir().join("orchestrator").join("runs")
}

// ---------------------------------------------------------------------------
// Pure _for variants — testable with temp paths, no env var access
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) fn config_path_for(base: &Path, channel: AppChannel) -> PathBuf {
    user_data_dir_for(base, channel).join("config.json")
}

#[cfg(test)]
pub(crate) fn user_prefs_path_for(base: &Path, channel: AppChannel) -> PathBuf {
    user_data_dir_for(base, channel).join("user_prefs.json")
}

#[cfg(test)]
pub(crate) fn openrouter_cache_path_for(base: &Path, channel: AppChannel) -> PathBuf {
    user_data_dir_for(base, channel).join("openrouter_models.json")
}

#[cfg(test)]
pub(crate) fn log_dir_for(base: &Path, channel: AppChannel) -> PathBuf {
    user_data_dir_for(base, channel).join("Communication-Logs")
}

#[cfg(test)]
pub(crate) fn orchestrator_runs_dir_for(base: &Path, channel: AppChannel) -> PathBuf {
    user_data_dir_for(base, channel).join("orchestrator").join("runs")
}

// ---------------------------------------------------------------------------
// Migration policy
// ---------------------------------------------------------------------------

/// Only stable channel migrates from old-product configs (Terra Bridge, APG).
/// Dev channel never touches stable data.
pub(crate) fn should_migrate_old_config(channel: AppChannel) -> bool {
    channel == AppChannel::Stable
}

// ---------------------------------------------------------------------------
// Model-set number parsing — shared with frontend semantics
// ---------------------------------------------------------------------------

/// Parse a canonical "Model N" name where N is a positive integer in standard
/// decimal notation (no leading zeros, no trailing text).
///
/// Accepted:  "Model 1", "Model 42"
/// Rejected:  "Model 0", "Model 01", "Model -1", "Model 1 extra"
///
/// The frontend must use the identical regex: `^Model ([1-9]\d*)$`
pub(crate) fn parse_model_set_number(name: &str) -> Option<u32> {
    let value = name.strip_prefix("Model ")?;
    if value.is_empty()
        || value.starts_with('0')
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    value.parse::<u32>().ok().filter(|n| *n > 0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── AppChannel ──────────────────────────────────────────────────

    #[test]
    fn dev_and_stable_data_dirs_differ() {
        let base = Path::new("test-appdata");
        assert_ne!(
            user_data_dir_for(base, AppChannel::Dev),
            user_data_dir_for(base, AppChannel::Stable),
        );
    }

    #[test]
    fn product_data_roots_use_the_renamed_brand_and_remain_channel_isolated() {
        let base = Path::new("test-appdata");
        assert_eq!(
            user_data_dir_for(base, AppChannel::Stable),
            base.join("Poverello")
        );
        assert_eq!(
            user_data_dir_for(base, AppChannel::Dev),
            base.join("Poverello Dev")
        );
    }

    #[test]
    fn stable_migration_copies_complete_tree_keeps_source_and_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path();
        let source = base.join("Anthro Bridge");
        std::fs::create_dir_all(source.join("orchestrator/runs/run-a/checkpoints")).unwrap();
        std::fs::create_dir_all(source.join("Communication-Logs")).unwrap();
        std::fs::write(source.join("config.json"), b"{\"stable\":true}").unwrap();
        std::fs::write(source.join("user_prefs.json"), b"prefs").unwrap();
        std::fs::write(source.join("openrouter_models.json"), b"cache").unwrap();
        std::fs::write(source.join("Communication-Logs/proxy.log"), b"log-data").unwrap();
        std::fs::write(
            source.join("orchestrator/runs/run-a/journal.json"),
            b"journal",
        )
        .unwrap();
        std::fs::write(
            source.join("orchestrator/runs/run-a/checkpoints/payload"),
            b"checkpoint",
        )
        .unwrap();

        migrate_user_data_dir(base, AppChannel::Stable).unwrap();
        let destination = base.join("Poverello");
        for relative in [
            "config.json",
            "user_prefs.json",
            "openrouter_models.json",
            "Communication-Logs/proxy.log",
            "orchestrator/runs/run-a/journal.json",
            "orchestrator/runs/run-a/checkpoints/payload",
        ] {
            assert_eq!(
                std::fs::read(destination.join(relative)).unwrap(),
                std::fs::read(source.join(relative)).unwrap(),
                "{relative}"
            );
        }
        assert!(destination.join(MIGRATION_MARKER).is_file());
        assert_eq!(
            std::fs::read(source.join("config.json")).unwrap(),
            b"{\"stable\":true}"
        );

        std::fs::write(destination.join("config.json"), b"newer-data").unwrap();
        migrate_user_data_dir(base, AppChannel::Stable).unwrap();
        assert_eq!(
            std::fs::read(destination.join("config.json")).unwrap(),
            b"newer-data"
        );
        assert_eq!(
            std::fs::read(source.join("config.json")).unwrap(),
            b"{\"stable\":true}"
        );
    }

    #[test]
    fn stable_and_dev_migrations_never_cross_copy() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path();
        std::fs::create_dir_all(base.join("Anthro Bridge")).unwrap();
        std::fs::create_dir_all(base.join("Anthro Bridge Dev")).unwrap();
        std::fs::write(base.join("Anthro Bridge/stable.txt"), b"stable").unwrap();
        std::fs::write(base.join("Anthro Bridge Dev/dev.txt"), b"dev").unwrap();

        migrate_user_data_dir(base, AppChannel::Stable).unwrap();
        assert!(base.join("Poverello/stable.txt").exists());
        assert!(!base.join("Poverello/dev.txt").exists());
        migrate_user_data_dir(base, AppChannel::Dev).unwrap();
        assert!(base.join("Poverello Dev/dev.txt").exists());
        assert!(!base.join("Poverello Dev/stable.txt").exists());
    }

    #[test]
    fn unverified_existing_destination_and_legacy_source_fail_closed() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path();
        std::fs::create_dir_all(base.join("Anthro Bridge")).unwrap();
        std::fs::write(base.join("Anthro Bridge/config.json"), b"old").unwrap();
        std::fs::create_dir_all(base.join("Poverello")).unwrap();
        std::fs::write(base.join("Poverello/config.json"), b"new").unwrap();

        let error = migrate_user_data_dir(base, AppChannel::Stable).unwrap_err();
        assert!(error.contains("Both legacy data folder"));
        assert_eq!(
            std::fs::read(base.join("Anthro Bridge/config.json")).unwrap(),
            b"old"
        );
        assert_eq!(
            std::fs::read(base.join("Poverello/config.json")).unwrap(),
            b"new"
        );
    }

    #[test]
    fn failed_staging_verification_never_publishes_destination_or_changes_source() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path();
        let source = base.join("Anthro Bridge");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("config.json"), b"original").unwrap();
        let corrupting_copy = |source: &Path, destination: &Path| {
            copy_tree_verified(source, destination)?;
            std::fs::write(destination.join("config.json"), b"corrupt")
                .map_err(|error| error.to_string())
        };

        let error = migrate_user_data_dir_with(
            base,
            AppChannel::Stable,
            corrupting_copy,
            publish_no_replace,
        )
        .unwrap_err();
        assert!(error.contains("did not verify"));
        assert!(!base.join("Poverello").exists());
        assert_eq!(
            std::fs::read(source.join("config.json")).unwrap(),
            b"original"
        );
        assert!(std::fs::read_dir(base).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".poverello-migration-")));
    }

    #[test]
    fn failed_atomic_publish_preserves_source_and_leaves_no_destination() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path();
        let source = base.join("Anthro Bridge");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("config.json"), b"original").unwrap();
        let fail_publish =
            |_staging: &Path, _destination: &Path| Err("injected publish failure".to_string());

        let error =
            migrate_user_data_dir_with(base, AppChannel::Stable, copy_tree_verified, fail_publish)
                .unwrap_err();
        assert!(error.contains("injected publish failure"));
        assert!(!base.join("Poverello").exists());
        assert_eq!(
            std::fs::read(source.join("config.json")).unwrap(),
            b"original"
        );
    }

    #[test]
    fn dev_channel_never_migrates_old_config() {
        assert!(!should_migrate_old_config(AppChannel::Dev));
    }

    #[test]
    fn stable_channel_does_migrate_old_config() {
        assert!(should_migrate_old_config(AppChannel::Stable));
    }

    // ── Path functions ──────────────────────────────────────────────

    #[test]
    fn all_dev_paths_share_dev_root() {
        let base = Path::new("test-appdata");
        let channel = AppChannel::Dev;
        let root = user_data_dir_for(base, channel);

        assert_eq!(
            config_path_for(base, channel).parent(),
            Some(root.as_path())
        );
        assert_eq!(
            user_prefs_path_for(base, channel).parent(),
            Some(root.as_path())
        );
        assert_eq!(
            openrouter_cache_path_for(base, channel).parent(),
            Some(root.as_path())
        );
        assert_eq!(
            log_dir_for(base, channel).parent(),
            Some(root.as_path())
        );
    }

    #[test]
    fn all_stable_paths_share_stable_root() {
        let base = Path::new("test-appdata");
        let channel = AppChannel::Stable;
        let root = user_data_dir_for(base, channel);

        assert_eq!(
            config_path_for(base, channel).parent(),
            Some(root.as_path())
        );
        assert_eq!(
            user_prefs_path_for(base, channel).parent(),
            Some(root.as_path())
        );
        assert_eq!(
            openrouter_cache_path_for(base, channel).parent(),
            Some(root.as_path())
        );
        assert_eq!(
            log_dir_for(base, channel).parent(),
            Some(root.as_path())
        );
    }

    // ── parse_model_set_number ──────────────────────────────────────

    #[test]
    fn parse_model_set_number_valid() {
        assert_eq!(parse_model_set_number("Model 1"), Some(1));
        assert_eq!(parse_model_set_number("Model 42"), Some(42));
        assert_eq!(parse_model_set_number("Model 999"), Some(999));
    }

    #[test]
    fn parse_model_set_number_rejects_zero() {
        assert_eq!(parse_model_set_number("Model 0"), None);
    }

    #[test]
    fn parse_model_set_number_rejects_leading_zeros() {
        assert_eq!(parse_model_set_number("Model 01"), None);
        assert_eq!(parse_model_set_number("Model 001"), None);
    }

    #[test]
    fn parse_model_set_number_rejects_trailing_text() {
        assert_eq!(parse_model_set_number("Model 1 extra"), None);
        assert_eq!(parse_model_set_number("Model 2a"), None);
    }

    #[test]
    fn parse_model_set_number_rejects_non_matching() {
        assert_eq!(parse_model_set_number("Model X"), None);
        assert_eq!(parse_model_set_number("Custom"), None);
        assert_eq!(parse_model_set_number("OpenRouter"), None);
        assert_eq!(parse_model_set_number("OpenRouter: Hy3"), None);
    }

    #[test]
    fn parse_model_set_number_rejects_negative() {
        assert_eq!(parse_model_set_number("Model -1"), None);
    }
}
