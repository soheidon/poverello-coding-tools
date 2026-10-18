use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

use super::types::{
    AutoConvergeProfile, ConvergenceLimits, EscalationPolicy, ProfilePolicyCapabilities,
    ReviewReusePolicy,
};

pub const CUSTOM_PROFILES_SCHEMA_VERSION: u32 = 1;
pub const BUILTIN_PROFILE_IDS: &[&str] = &["standard", "strict", "fast"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomProfilesFile {
    pub schema_version: u32,
    pub profiles: Vec<AutoConvergeProfile>,
}

/// Returns the three immutable built-in presets (`Standard`, `Strict`, `Fast`) defined in §8.1.
pub fn builtin_profiles() -> Vec<AutoConvergeProfile> {
    vec![
        AutoConvergeProfile {
            id: "standard".to_string(),
            name: "Standard (Default)".to_string(),
            is_builtin: true,
            plan_review_mandatory: true,
            implementation_review_mandatory: true,
            corrective_plan_review_mandatory: true,
            targeted_tests: true,
            full_test_suite: true,
            frontend_rust_build: true,
            lint_and_format_check: true,
            git_diff_check: true,
            manual_smoke_test: false,
            live_api_smoke: false,
            installer_smoke: false,
            artifact_verification: true,
            release_readiness_review: false,
            review_reuse_policy: ReviewReusePolicy::ChangedOnly,
            escalation_policy: EscalationPolicy::Enabled,
            capabilities: ProfilePolicyCapabilities {
                allow_stage: true,
                allow_commit: true,
                allow_push_branch: true,
                allow_tag: false,
                allow_push_tag: false,
                allow_release_publish: false,
                allow_paid_live_api: false,
            },
            convergence_limits: ConvergenceLimits {
                max_plan_reviews: 3,
                max_impl_reviews: 3,
                max_corrective_reviews: 3,
                max_verification_retries: 2,
                max_release_reviews: 2,
                same_blocker_threshold: 2,
                max_escalations: 2,
                max_escalation_reviews: 2,
                max_transport_retries: 2,
            },
            pre_granted_run_permissions: 0,
        },
        AutoConvergeProfile {
            id: "strict".to_string(),
            name: "Strict".to_string(),
            is_builtin: true,
            plan_review_mandatory: true,
            implementation_review_mandatory: true,
            corrective_plan_review_mandatory: true,
            targeted_tests: true,
            full_test_suite: true,
            frontend_rust_build: true,
            lint_and_format_check: true,
            git_diff_check: true,
            manual_smoke_test: true,
            live_api_smoke: true,
            installer_smoke: true,
            artifact_verification: true,
            release_readiness_review: true,
            review_reuse_policy: ReviewReusePolicy::ChangedOnly,
            escalation_policy: EscalationPolicy::Enabled,
            capabilities: ProfilePolicyCapabilities {
                allow_stage: true,
                allow_commit: true,
                allow_push_branch: true,
                allow_tag: true,
                allow_push_tag: true,
                allow_release_publish: true,
                allow_paid_live_api: true,
            },
            convergence_limits: ConvergenceLimits {
                max_plan_reviews: 3,
                max_impl_reviews: 3,
                max_corrective_reviews: 3,
                max_verification_retries: 2,
                max_release_reviews: 2,
                same_blocker_threshold: 2,
                max_escalations: 2,
                max_escalation_reviews: 2,
                max_transport_retries: 2,
            },
            pre_granted_run_permissions: 0,
        },
        AutoConvergeProfile {
            id: "fast".to_string(),
            name: "Fast".to_string(),
            is_builtin: true,
            plan_review_mandatory: true,
            implementation_review_mandatory: true,
            corrective_plan_review_mandatory: true,
            targeted_tests: true,
            full_test_suite: false,
            frontend_rust_build: true,
            lint_and_format_check: false,
            git_diff_check: true,
            manual_smoke_test: false,
            live_api_smoke: false,
            installer_smoke: false,
            artifact_verification: false,
            release_readiness_review: false,
            review_reuse_policy: ReviewReusePolicy::ValidBinding,
            escalation_policy: EscalationPolicy::BlockingOnly,
            capabilities: ProfilePolicyCapabilities {
                allow_stage: true,
                allow_commit: true,
                allow_push_branch: false,
                allow_tag: false,
                allow_push_tag: false,
                allow_release_publish: false,
                allow_paid_live_api: false,
            },
            convergence_limits: ConvergenceLimits {
                max_plan_reviews: 2,
                max_impl_reviews: 2,
                max_corrective_reviews: 2,
                max_verification_retries: 1,
                max_release_reviews: 1,
                same_blocker_threshold: 2,
                max_escalations: 1,
                max_escalation_reviews: 2,
                max_transport_retries: 2,
            },
            pre_granted_run_permissions: 0,
        },
    ]
}

/// Validates a profile against Hard Invariants 3, 4, 13, 14 and §8.2 Rules 1–6.
pub fn validate_profile_against_invariants(profile: &AutoConvergeProfile) -> Result<(), String> {
    if !profile.plan_review_mandatory {
        return Err("Invariant 3 violation: plan_review_mandatory cannot be disabled".to_string());
    }
    if !profile.implementation_review_mandatory {
        return Err(
            "Invariant 3 violation: implementation_review_mandatory cannot be disabled".to_string(),
        );
    }
    if !profile.corrective_plan_review_mandatory {
        return Err(
            "Invariant 4 violation: corrective_plan_review_mandatory cannot be disabled"
                .to_string(),
        );
    }

    let limits = &profile.convergence_limits;
    if !(1..=5).contains(&limits.max_plan_reviews)
        || !(1..=5).contains(&limits.max_impl_reviews)
        || !(1..=5).contains(&limits.max_corrective_reviews)
    {
        return Err(
            "Invariant 13 violation: stage review counters must be bounded within 1..=5"
                .to_string(),
        );
    }
    if !(1..=3).contains(&limits.max_verification_retries)
        || !(1..=3).contains(&limits.max_release_reviews)
        || !(1..=3).contains(&limits.same_blocker_threshold)
    {
        return Err(
            "Invariant 13 violation: verification/release/blocker counters must be within 1..=3"
                .to_string(),
        );
    }
    if !(1..=3).contains(&limits.max_escalation_reviews)
        || !(1..=3).contains(&limits.max_transport_retries)
    {
        return Err(
            "Invariant 13 violation: escalation_reviews and transport_retries must be within 1..=3"
                .to_string(),
        );
    }

    match profile.escalation_policy {
        EscalationPolicy::Disabled => {
            if limits.max_escalations != 0 {
                return Err(
                    "Invariant 13 violation: max_escalations must be 0 when escalation_policy is Disabled"
                        .to_string(),
                );
            }
        }
        EscalationPolicy::Enabled | EscalationPolicy::BlockingOnly => {
            if !(1..=3).contains(&limits.max_escalations) {
                return Err(
                    "Invariant 13 violation: max_escalations must be within 1..=3 when escalation_policy is Enabled or BlockingOnly"
                        .to_string(),
                );
            }
        }
    }

    let caps = &profile.capabilities;
    if (caps.allow_tag || caps.allow_push_tag || caps.allow_release_publish)
        && !profile.release_readiness_review
    {
        return Err(
            "§8.2 Rule 5 violation: tag, push_tag, and release_publish capabilities unconditionally require release_readiness_review = true"
                .to_string(),
        );
    }

    if profile.pre_granted_run_permissions != 0 {
        return Err(
            "Invariant 14 violation: profiles cannot pre-grant run permissions".to_string(),
        );
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct ProfileStore {
    custom_profiles_path: PathBuf,
}

impl ProfileStore {
    pub fn new(custom_profiles_path: PathBuf) -> Self {
        Self {
            custom_profiles_path,
        }
    }

    pub fn default_custom_profiles_path() -> PathBuf {
        crate::paths::user_data_dir()
            .join("profiles")
            .join("custom_profiles.json")
    }

    pub fn custom_profiles_path(&self) -> &Path {
        &self.custom_profiles_path
    }

    /// Loads built-in presets + valid custom profiles.
    /// If `custom_profiles.json` is malformed or fails validation, quarantines it to
    /// `custom_profiles.json.corrupt.<timestamp>` and falls back safely to built-in presets (§8.3).
    pub fn load_all_profiles(&self) -> Result<Vec<AutoConvergeProfile>, String> {
        let mut all = builtin_profiles();
        if !self.custom_profiles_path.exists() {
            return Ok(all);
        }

        let raw = match std::fs::read_to_string(&self.custom_profiles_path) {
            Ok(s) => s,
            Err(e) => {
                self.quarantine_corrupt_file();
                eprintln!("[auto-converge] WARN: Failed reading custom_profiles.json ({e}); falling back to presets");
                return Ok(all);
            }
        };

        let parsed: CustomProfilesFile = match serde_json::from_str::<CustomProfilesFile>(&raw) {
            Ok(p) if p.schema_version == CUSTOM_PROFILES_SCHEMA_VERSION => p,
            _ => {
                self.quarantine_corrupt_file();
                return Ok(all);
            }
        };

        for mut custom in parsed.profiles {
            if BUILTIN_PROFILE_IDS.contains(&custom.id.to_ascii_lowercase().as_str()) {
                self.quarantine_corrupt_file();
                return Ok(builtin_profiles());
            }
            if validate_profile_against_invariants(&custom).is_err() {
                self.quarantine_corrupt_file();
                return Ok(builtin_profiles());
            }
            custom.is_builtin = false;
            all.push(custom);
        }

        Ok(all)
    }

    /// Saves or updates a custom profile with atomic write (`custom_profiles.json.tmp` -> `sync_all()` -> rename).
    pub fn save_custom_profile(&self, mut profile: AutoConvergeProfile) -> Result<(), String> {
        let normalized_id = profile.id.trim().to_ascii_lowercase();
        if normalized_id.is_empty() {
            return Err("Custom profile ID cannot be empty".to_string());
        }
        if BUILTIN_PROFILE_IDS.contains(&normalized_id.as_str()) {
            return Err(format!(
                "Cannot overwrite or shadow built-in preset '{}'",
                profile.id
            ));
        }

        validate_profile_against_invariants(&profile)?;
        profile.is_builtin = false;

        let existing = self.load_all_profiles()?;
        let mut customs: Vec<AutoConvergeProfile> = existing
            .into_iter()
            .filter(|p| !p.is_builtin && p.id != profile.id)
            .collect();
        customs.push(profile);

        let payload = CustomProfilesFile {
            schema_version: CUSTOM_PROFILES_SCHEMA_VERSION,
            profiles: customs,
        };
        let bytes = serde_json::to_vec_pretty(&payload)
            .map_err(|e| format!("Failed to serialize custom profiles: {e}"))?;

        if let Some(parent) = self.custom_profiles_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create profiles directory: {e}"))?;
        }

        let tmp_path = self.custom_profiles_path.with_extension("json.tmp");
        {
            let mut file = std::fs::File::create(&tmp_path)
                .map_err(|e| format!("Failed to create temp profile file: {e}"))?;
            file.write_all(&bytes)
                .map_err(|e| format!("Failed to write temp profile file: {e}"))?;
            file.sync_all()
                .map_err(|e| format!("Failed to sync temp profile file: {e}"))?;
        }

        std::fs::rename(&tmp_path, &self.custom_profiles_path)
            .map_err(|e| format!("Failed atomic rename of custom_profiles.json: {e}"))?;

        Ok(())
    }

    fn quarantine_corrupt_file(&self) {
        if self.custom_profiles_path.exists() {
            let ts = chrono::Utc::now().format("%Y%m%d%H%M%S");
            let corrupt_name = format!("custom_profiles.json.corrupt.{ts}");
            let corrupt_path = self.custom_profiles_path.with_file_name(corrupt_name);
            let _ = std::fs::rename(&self.custom_profiles_path, corrupt_path);
        }
    }
}
