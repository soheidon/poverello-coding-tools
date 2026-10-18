use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::types::{sha256_hex, ReleasePermissionIntent};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseCandidateManifest {
    pub version_tag: String,
    pub release_repository: String,
    pub changelog_summary: String,
    pub artifact_hashes: BTreeMap<String, String>,
}

impl ReleaseCandidateManifest {
    pub fn canonical_digest(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        sha256_hex(json.as_bytes())
    }
}

/// Validates a `ReleaseCandidateManifest` before or during `ReleaseReview` (§9.2).
/// Ensures non-empty version tag, non-empty changelog, and exact match with `ReleasePermissionIntent` if present.
pub fn validate_release_candidate_manifest(
    manifest: &ReleaseCandidateManifest,
    intent: Option<&ReleasePermissionIntent>,
) -> Result<String, String> {
    if manifest.version_tag.trim().is_empty() {
        return Err("ReleaseCandidateManifest.version_tag cannot be empty".to_string());
    }
    if manifest.release_repository.trim().is_empty() {
        return Err("ReleaseCandidateManifest.release_repository cannot be empty".to_string());
    }
    if manifest.changelog_summary.trim().is_empty() {
        return Err("ReleaseCandidateManifest.changelog_summary cannot be empty".to_string());
    }

    if let Some(intent) = intent {
        if manifest.version_tag != intent.requested_version_tag {
            return Err(format!(
                "ReleaseCandidateManifest version_tag '{}' does not match ReleasePermissionIntent '{}'",
                manifest.version_tag, intent.requested_version_tag
            ));
        }
        if manifest.release_repository != intent.release_repository {
            return Err(format!(
                "ReleaseCandidateManifest release_repository '{}' does not match ReleasePermissionIntent '{}'",
                manifest.release_repository, intent.release_repository
            ));
        }
    }

    Ok(manifest.canonical_digest())
}
