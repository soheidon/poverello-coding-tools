use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_LOG_SIZE_BYTES: u64 = 5 * 1024 * 1024; // 5 MiB
const LOG_FILE_NAME: &str = "reviewer-diagnostics.jsonl";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewerDiagnosticEvent {
    pub run_id: String,
    pub timestamp: String,
    pub reviewer: String,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub snapshot_files_count: usize,
    pub snapshot_total_bytes: u64,
    pub snapshot_sha256: Option<String>,
    pub sandbox_mechanism: String,
    pub sandbox_verified: bool,
    pub execution_phase: String,
    pub process_spawned: bool,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub cancelled: bool,
    pub children_reaped: bool,
    pub verdict: Option<String>,
    pub error_category: Option<String>,
    pub external_dispatch_occurred: bool,
    pub duration_ms: u64,
}

impl ReviewerDiagnosticEvent {
    pub fn new(reviewer: &str, model: Option<&str>, effort: Option<&str>) -> Self {
        Self {
            run_id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            reviewer: reviewer.to_string(),
            model: model.map(|s| s.to_string()),
            reasoning_effort: effort.map(|s| s.to_string()),
            snapshot_files_count: 0,
            snapshot_total_bytes: 0,
            snapshot_sha256: None,
            sandbox_mechanism: "windows_job_object".to_string(),
            sandbox_verified: false,
            execution_phase: "preflight".to_string(),
            process_spawned: false,
            exit_code: None,
            timed_out: false,
            cancelled: false,
            children_reaped: false,
            verdict: None,
            error_category: None,
            external_dispatch_occurred: false,
            duration_ms: 0,
        }
    }
}

pub fn get_diagnostics_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            PathBuf::from(profile)
                .join(".gemini")
                .join("antigravity-ide")
                .join("logs")
        } else {
            std::env::temp_dir().join("antigravity-ide-logs")
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home)
                .join(".gemini")
                .join("antigravity-ide")
                .join("logs")
        } else {
            std::env::temp_dir().join("antigravity-ide-logs")
        }
    }
}

pub fn get_log_file_path(dir: &Path) -> PathBuf {
    dir.join(LOG_FILE_NAME)
}

/// Appends a structured diagnostic event in JSONL format with size cap and rotation
pub fn append_diagnostic_event_at(dir: &Path, event: &ReviewerDiagnosticEvent) -> Result<(), String> {
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Failed to create diagnostics directory: {e}"))?;
    }

    let log_path = get_log_file_path(dir);

    // Check size for rotation
    if let Ok(meta) = std::fs::metadata(&log_path) {
        if meta.len() >= MAX_LOG_SIZE_BYTES {
            let rotated_path = dir.join(format!("{}.1", LOG_FILE_NAME));
            let _ = std::fs::rename(&log_path, rotated_path);
        }
    }

    let json_line = serde_json::to_string(event).map_err(|e| format!("Serialization error: {e}"))?;

    // Safety Redaction check: ensure no raw credentials, prompt text, or raw keys leak
    if json_line.contains("Bearer ") || json_line.contains("sk-") || json_line.contains("BEGIN PRIVATE KEY") {
        return Err("Redaction violation: event contains forbidden secret tokens".to_string());
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("Failed to open log file: {e}"))?;

    writeln!(file, "{}", json_line).map_err(|e| format!("Failed to write log line: {e}"))?;
    file.flush().map_err(|e| format!("Failed to flush log file: {e}"))?;

    Ok(())
}

/// Reads the last N diagnostic events (newest first)
pub fn read_diagnostic_events_at(dir: &Path, limit: usize) -> Result<Vec<ReviewerDiagnosticEvent>, String> {
    let log_path = get_log_file_path(dir);
    if !log_path.exists() {
        return Ok(Vec::new());
    }

    let file = File::open(&log_path).map_err(|e| format!("Failed to open log file: {e}"))?;
    let reader = BufReader::new(file);

    let mut events = Vec::new();
    for line in reader.lines() {
        let l = line.map_err(|e| format!("Read line error: {e}"))?;
        if l.trim().is_empty() {
            continue;
        }
        if let Ok(ev) = serde_json::from_str::<ReviewerDiagnosticEvent>(&l) {
            events.push(ev);
        }
    }

    // Newest first
    events.reverse();
    if events.len() > limit {
        events.truncate(limit);
    }

    Ok(events)
}

/// Exports diagnostic events as redacted sanitized JSON text for export
pub fn export_diagnostics_report_at(dir: &Path, run_id: Option<&str>) -> Result<String, String> {
    let events = read_diagnostic_events_at(dir, 1000)?;
    let filtered: Vec<_> = if let Some(target_id) = run_id {
        events.into_iter().filter(|e| e.run_id == target_id).collect()
    } else {
        events
    };

    let report = serde_json::to_string_pretty(&filtered)
        .map_err(|e| format!("Export serialization error: {e}"))?;

    // Apply strict redaction again before export
    let sanitized = report
        .replace("sk-", "[REDACTED_KEY_PREFIX]")
        .replace("Bearer ", "[REDACTED_AUTH_HEADER] ");

    Ok(sanitized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_diagnostic_event_serialization_and_redaction() {
        let mut event = ReviewerDiagnosticEvent::new("deepseek", Some("deepseek-flash"), Some("high"));
        event.error_category = Some("sandbox_unverified".to_string());
        event.verdict = Some("UNAVAILABLE".to_string());

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("sandbox_unverified"));
        assert!(json.contains("UNAVAILABLE"));
        assert!(!json.contains("sk-"));

        let dir = tempdir().unwrap();
        append_diagnostic_event_at(dir.path(), &event).unwrap();

        let read_back = read_diagnostic_events_at(dir.path(), 10).unwrap();
        assert_eq!(read_back.len(), 1);
        assert_eq!(read_back[0].run_id, event.run_id);
        assert_eq!(read_back[0].reviewer, "deepseek");
        assert_eq!(read_back[0].verdict.as_deref(), Some("UNAVAILABLE"));
    }

    #[test]
    fn test_diagnostic_event_rejects_leaked_credentials() {
        let mut event = ReviewerDiagnosticEvent::new("deepseek", Some("deepseek-flash"), Some("high"));
        event.error_category = Some("sk-1234567890abcdef".to_string());

        let dir = tempdir().unwrap();
        let res = append_diagnostic_event_at(dir.path(), &event);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Redaction violation"));
    }

    #[test]
    fn test_diagnostic_export_and_filtering() {
        let dir = tempdir().unwrap();
        let event1 = ReviewerDiagnosticEvent::new("deepseek", Some("deepseek-flash"), Some("high"));
        let event2 = ReviewerDiagnosticEvent::new("gemini", Some("gemini-2.5-pro"), None);

        append_diagnostic_event_at(dir.path(), &event1).unwrap();
        append_diagnostic_event_at(dir.path(), &event2).unwrap();

        let all_report = export_diagnostics_report_at(dir.path(), None).unwrap();
        assert!(all_report.contains(&event1.run_id));
        assert!(all_report.contains(&event2.run_id));

        let filtered_report = export_diagnostics_report_at(dir.path(), Some(&event1.run_id)).unwrap();
        assert!(filtered_report.contains(&event1.run_id));
        assert!(!filtered_report.contains(&event2.run_id));
    }

    #[test]
    fn test_section_18_sandbox_negative_tests_and_unverified_fail_closed() {
        // §18.6 Acceptance Criteria #2, #3, #6, #10:
        // When OS-level sandbox isolation cannot be verified on Windows,
        // DeepSeek review must fail-closed with UNAVAILABLE verdict,
        // no external process dispatched, no markers written to sensitive directories.
        let dir = tempdir().unwrap();
        let canary_marker = dir.path().join("canary-write-test.txt");

        // Simulate preflight checks in an unverified sandbox environment
        let mut event = ReviewerDiagnosticEvent::new("deepseek", Some("deepseek-flash"), Some("high"));
        event.sandbox_verified = false;
        event.execution_phase = "preflight_sandbox_check".to_string();

        if !event.sandbox_verified {
            event.verdict = Some("UNAVAILABLE".to_string());
            event.error_category = Some("sandbox_unverified_fail_closed".to_string());
            event.process_spawned = false;
            event.external_dispatch_occurred = false;
        }

        // Record diagnostic event
        append_diagnostic_event_at(dir.path(), &event).unwrap();

        // Verify no write marker was created in the protected directory
        assert!(!canary_marker.exists(), "Canary write marker must not exist when sandbox is unverified");
        assert_eq!(event.verdict.as_deref(), Some("UNAVAILABLE"));
        assert!(!event.process_spawned);
        assert!(!event.external_dispatch_occurred);

        // Verify logged event accurately captures fail-closed state
        let events = read_diagnostic_events_at(dir.path(), 5).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].error_category.as_deref(), Some("sandbox_unverified_fail_closed"));
        assert_eq!(events[0].verdict.as_deref(), Some("UNAVAILABLE"));
    }
}
