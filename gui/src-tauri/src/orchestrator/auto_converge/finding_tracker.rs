use std::collections::BTreeMap;

use super::types::{sha256_hex, ReviewFinding, ReviewFindingKey, TrackedFinding};

pub const CANONICAL_FAILURE_CODES: &[&str] = &[
    "INV-001",
    "INV-002",
    "INV-003",
    "INV-004",
    "INV-005",
    "INV-006",
    "INV-007",
    "INV-008",
    "INV-009",
    "INV-010",
    "INV-011",
    "INV-012",
    "INV-013",
    "INV-014",
    "SEC-001",
    "SEC-002",
    "SEC-003",
    "SEC-004",
    "COR-001",
    "COR-002",
    "COR-003",
    "COR-004",
    "CON-001",
    "CON-002",
    "CON-003",
    "PERF-001",
    "PERF-002",
    "PERF-003",
    "SCOPE-001",
    "SCOPE-002",
];

/// Normalizes a `failure_code` into the canonical uppercase Failure Code Taxonomy (§5).
/// Maps well-known aliases/synonyms; any code still outside the taxonomy returns an `Err`
/// (schema violation) and is never silently remapped to a catch-all code.
pub fn normalize_failure_code(raw: &str) -> Result<String, String> {
    let cleaned = raw.trim().to_ascii_uppercase().replace('_', "-");
    if CANONICAL_FAILURE_CODES.contains(&cleaned.as_str()) {
        return Ok(cleaned);
    }

    let mapped = match cleaned.as_str() {
        "NULL-PTR" | "NULL-DEREF" | "EMPTY-BUFFER" | "NULL-POINTER" => Some("COR-001"),
        "OUT-OF-BOUNDS" | "INDEX-OOB" | "BOUNDS-CHECK" => Some("COR-002"),
        "UNHANDLED-ERR" | "UNHANDLED-ERROR" | "PANIC" => Some("COR-003"),
        "LOGIC-DEFECT" | "LOGIC-MISMATCH" | "OFF-BY-ONE" => Some("COR-004"),
        "INJECTION" | "SHELL-HAZARD" | "CMD-INJECTION" => Some("SEC-001"),
        "PATH-TRAVERSAL" | "SYMLINK-ESCAPE" => Some("SEC-002"),
        "CRED-LEAK" | "SECRET-LEAK" => Some("SEC-003"),
        "PERM-BYPASS" | "PERMISSION-BYPASS" => Some("SEC-004"),
        "SCHEMA-ERR" | "SCHEMA-VIOLATION" => Some("CON-001"),
        "MISSING-FIELD" | "MISSING-REQUIRED-FIELD" => Some("CON-002"),
        "TYPE-MISMATCH" | "SIGNATURE-MISMATCH" => Some("CON-003"),
        "MEM-LEAK" | "RESOURCE-LEAK" => Some("PERF-001"),
        "INFINITE-LOOP" | "UNBOUNDED-LOOP" => Some("PERF-002"),
        "REDUNDANT-WORK" | "REDUNDANT-COMPUTATION" => Some("PERF-003"),
        "OUT-OF-ALLOWLIST" | "OUT-OF-SCOPE-WRITE" => Some("SCOPE-001"),
        "FEATURE-CREEP" | "UNSPECIFIED-SCOPE" => Some("SCOPE-002"),
        _ => None,
    };

    match mapped {
        Some(code) => Ok(code.to_string()),
        None => Err(format!(
            "Schema violation: failure_code '{raw}' is outside the canonical Failure Code Taxonomy"
        )),
    }
}

/// Normalizes a file path for comparison keys.
/// Preserves real filesystem casing when `case_insensitive_fs == false`; folds to lowercase
/// only for comparison keys on case-insensitive filesystems (§5, §13.2).
pub fn normalize_file_for_finding_key(path: &str, case_insensitive_fs: bool) -> String {
    let mut normalized = path.trim().replace('\\', "/");
    while normalized.starts_with("./") {
        normalized = normalized[2..].to_string();
    }
    if case_insensitive_fs {
        normalized.to_ascii_lowercase()
    } else {
        normalized
    }
}

/// Computes the deterministic canonical `FindingID` (§5.2):
/// `SHA256(category + "|" + normalized_file + "|" + normalized_symbol + "|" + normalized_failure_code)`
pub fn compute_finding_id(
    key: &ReviewFindingKey,
    case_insensitive_fs: bool,
) -> Result<String, String> {
    let code = normalize_failure_code(&key.failure_code)?;
    let norm_file = normalize_file_for_finding_key(&key.file, case_insensitive_fs);
    let norm_symbol = key.symbol.trim().to_ascii_lowercase();
    let payload = format!(
        "{}|{}|{}|{}",
        key.category.as_str(),
        norm_file,
        norm_symbol,
        code
    );
    Ok(sha256_hex(payload.as_bytes()))
}

/// Updates tracked findings across review cycles and evaluates whether any blocking finding
/// has persisted for `>= same_blocker_threshold` consecutive cycles (§5.3, §7.2).
pub fn update_tracked_findings(
    previous: &[TrackedFinding],
    current_findings: &[ReviewFinding],
    same_blocker_threshold: u32,
    case_insensitive_fs: bool,
) -> Result<(Vec<TrackedFinding>, bool), String> {
    let mut prev_by_id: BTreeMap<String, &TrackedFinding> = BTreeMap::new();
    for item in previous {
        prev_by_id.insert(item.finding_id.clone(), item);
    }

    let mut updated = Vec::new();
    let mut seen_ids = std::collections::BTreeSet::new();
    let mut same_blocker_triggered = false;

    for finding in current_findings {
        let finding_id = compute_finding_id(&finding.finding_key, case_insensitive_fs)?;
        if !seen_ids.insert(finding_id.clone()) {
            continue;
        }
        let prev_cycles = prev_by_id
            .get(&finding_id)
            .filter(|prev| !prev.resolved)
            .map(|prev| prev.consecutive_cycles)
            .unwrap_or(0);
        let consecutive_cycles = prev_cycles + 1;
        if finding.blocking && consecutive_cycles >= same_blocker_threshold {
            same_blocker_triggered = true;
        }
        updated.push(TrackedFinding {
            finding_id,
            finding: finding.clone(),
            consecutive_cycles,
            resolved: false,
        });
    }

    for prev in previous {
        if !seen_ids.contains(&prev.finding_id) {
            let mut resolved_copy = prev.clone();
            resolved_copy.resolved = true;
            updated.push(resolved_copy);
        }
    }

    Ok((updated, same_blocker_triggered))
}
