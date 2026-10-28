//! Test-only Codex CLI transport contract PoC.
//!
//! This module is deliberately excluded from production builds. It exercises
//! argument construction, strict JSONL thread binding, typed artifact parsing,
//! and the existing bounded `ProcessRunner` without invoking the authenticated
//! Codex CLI or making network requests.

use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use std::{fs::File, io::Read};

use serde_json::Value;

use crate::orchestrator::auto_converge::types::CodexReviewVerdict;
use crate::orchestrator::process_runner::ProcessRunner;

const FIXTURE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_ARTIFACT_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InvocationRole {
    Planner,
    Reviewer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InvocationPaths {
    run_root: PathBuf,
    working_dir: PathBuf,
    config_home: PathBuf,
    output_last_message: PathBuf,
    output_schema: PathBuf,
}

impl InvocationPaths {
    fn validate(&self) -> Result<(), String> {
        let root = normalize_absolute(&self.run_root)?;
        for (name, path) in [
            ("run root", &root),
            ("working directory", &self.working_dir),
            ("configuration home", &self.config_home),
            ("output-last-message", &self.output_last_message),
            ("output-schema", &self.output_schema),
        ] {
            normalize_absolute(path)?;
            if path.to_str().is_none() {
                return Err(format!("{name} path is not representable as Unicode"));
            }
        }
        let canonical_root = root
            .canonicalize()
            .map_err(|e| format!("failed to canonicalize run root: {e}"))?;
        if !canonical_root.is_dir() {
            return Err("run root must be an existing directory".into());
        }
        for (name, path) in [
            ("working directory", &self.working_dir),
            ("configuration home", &self.config_home),
            ("output-schema", &self.output_schema),
        ] {
            let normalized = normalize_absolute(path)?;
            let canonical = normalized
                .canonicalize()
                .map_err(|e| format!("failed to canonicalize {name}: {e}"))?;
            if !canonical.starts_with(&canonical_root) || canonical == canonical_root {
                return Err(format!(
                    "{name} path must resolve inside the run-scoped root"
                ));
            }
        }
        if !self.working_dir.is_dir() || !self.config_home.is_dir() || !self.output_schema.is_file()
        {
            return Err("working/config directories and schema file must exist".into());
        }
        let schema: Value = serde_json::from_slice(
            &std::fs::read(&self.output_schema)
                .map_err(|e| format!("failed to read output schema: {e}"))?,
        )
        .map_err(|e| format!("output schema is not valid JSON: {e}"))?;
        validate_object_schema(&schema)?;
        let output_parent = self
            .output_last_message
            .parent()
            .ok_or_else(|| "output-last-message path has no parent".to_string())?
            .canonicalize()
            .map_err(|e| format!("failed to canonicalize output parent: {e}"))?;
        if !output_parent.starts_with(&canonical_root) || output_parent == canonical_root {
            return Err(
                "output-last-message parent must resolve inside the run-scoped root".into(),
            );
        }
        if std::fs::symlink_metadata(&self.output_last_message).is_ok() {
            if std::fs::symlink_metadata(&self.output_last_message)
                .map(|metadata| metadata.file_type().is_symlink())
                .unwrap_or(true)
            {
                return Err("output-last-message must not already be a symbolic link".into());
            }
            let output = self
                .output_last_message
                .canonicalize()
                .map_err(|e| format!("failed to canonicalize existing output: {e}"))?;
            if !output.starts_with(&canonical_root) || output == canonical_root {
                return Err(
                    "existing output-last-message resolves outside the run-scoped root".into(),
                );
            }
        }
        Ok(())
    }

    fn canonicalized(&self) -> Result<Self, String> {
        self.validate()?;
        let canonical_root = normalize_absolute(&self.run_root)?
            .canonicalize()
            .map_err(|e| format!("failed to canonicalize run root: {e}"))?;
        let canonical_inside = |name: &str, path: &Path| -> Result<PathBuf, String> {
            let resolved = normalize_absolute(path)?
                .canonicalize()
                .map_err(|e| format!("failed to canonicalize {name}: {e}"))?;
            if !resolved.starts_with(&canonical_root) || resolved == canonical_root {
                return Err(format!(
                    "{name} path must resolve inside the run-scoped root"
                ));
            }
            Ok(resolved)
        };
        let working_dir = canonical_inside("working directory", &self.working_dir)?;
        let config_home = canonical_inside("configuration home", &self.config_home)?;
        let output_schema = canonical_inside("output-schema", &self.output_schema)?;
        let output_parent = self
            .output_last_message
            .parent()
            .ok_or_else(|| "output-last-message path has no parent".to_string())?
            .canonicalize()
            .map_err(|e| format!("failed to canonicalize output parent: {e}"))?;
        if !output_parent.starts_with(&canonical_root) || output_parent == canonical_root {
            return Err(
                "output-last-message parent must resolve inside the run-scoped root".into(),
            );
        }
        let output_name = self
            .output_last_message
            .file_name()
            .ok_or_else(|| "output-last-message path has no file name".to_string())?;
        let output_last_message = output_parent.join(output_name);
        if std::fs::symlink_metadata(&output_last_message).is_ok() {
            let metadata = std::fs::symlink_metadata(&output_last_message)
                .map_err(|e| format!("failed to inspect existing output artifact: {e}"))?;
            if metadata.file_type().is_symlink() {
                return Err("output-last-message must not already be a symbolic link".into());
            }
            let output = output_last_message
                .canonicalize()
                .map_err(|e| format!("failed to canonicalize existing output: {e}"))?;
            if !output.starts_with(&canonical_root) || output == canonical_root {
                return Err(
                    "existing output-last-message resolves outside the run-scoped root".into(),
                );
            }
        }
        Ok(Self {
            run_root: canonical_root,
            working_dir,
            config_home,
            output_last_message,
            output_schema,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Invocation {
    role: InvocationRole,
    args: Vec<String>,
    stdin: Vec<u8>,
    scope: InvocationPaths,
    thread_id: Option<String>,
}

fn initial_invocation(
    role: InvocationRole,
    prompt: &str,
    paths: &InvocationPaths,
) -> Result<Invocation, String> {
    let paths = paths.canonicalized()?;
    if prompt.trim().is_empty() {
        return Err("prompt must not be empty".into());
    }
    Ok(Invocation {
        role,
        args: codex_exec_args(None, &paths),
        stdin: prompt.as_bytes().to_vec(),
        scope: paths,
        // A Reviewer always starts a new thread; a Planner's initial call does too.
        thread_id: None,
    })
}

fn resume_planner_invocation(
    prior: &Invocation,
    recorded_thread_id: &str,
    prompt: &str,
) -> Result<Invocation, String> {
    if prior.role != InvocationRole::Planner {
        return Err("Reviewer invocations must be fresh and cannot resume Planner state".into());
    }
    let paths = prior.scope.clone();
    let thread_id = validate_thread_id(recorded_thread_id)?;
    if prompt.trim().is_empty() {
        return Err("prompt must not be empty".into());
    }
    Ok(Invocation {
        role: InvocationRole::Planner,
        args: codex_exec_args(Some(&thread_id), &paths),
        stdin: prompt.as_bytes().to_vec(),
        scope: paths,
        thread_id: Some(thread_id),
    })
}

fn reviewer_invocation(
    planner: &Invocation,
    prompt: &str,
    proposed_paths: &InvocationPaths,
) -> Result<Invocation, String> {
    if planner.role != InvocationRole::Planner {
        return Err("Reviewer scope must be checked against a Planner invocation".into());
    }
    if prompt.trim().is_empty() {
        return Err("prompt must not be empty".into());
    }
    let planner_paths = &planner.scope;
    let reviewer_paths = proposed_paths.canonicalized()?;
    let overlaps = |left: &Path, right: &Path| left.starts_with(right) || right.starts_with(left);
    if overlaps(&planner_paths.run_root, &reviewer_paths.run_root) {
        return Err("Planner and Reviewer run roots must be disjoint".into());
    }
    if planner_paths.config_home == reviewer_paths.config_home {
        return Err("Planner and Reviewer configuration homes must be distinct".into());
    }
    Ok(Invocation {
        role: InvocationRole::Reviewer,
        args: codex_exec_args(None, &reviewer_paths),
        stdin: prompt.as_bytes().to_vec(),
        scope: reviewer_paths,
        thread_id: None,
    })
}

fn validate_object_schema(schema: &Value) -> Result<(), String> {
    let object = schema
        .as_object()
        .ok_or_else(|| "output schema must be a JSON object".to_string())?;
    let allowed = ["type", "required", "properties", "additionalProperties"];
    if object.len() != allowed.len() || object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("output schema has missing or unsupported top-level keywords".into());
    }
    if object.get("type").and_then(Value::as_str) != Some("object") {
        return Err("output schema type must be object".into());
    }
    if object.get("additionalProperties").and_then(Value::as_bool) != Some(false) {
        return Err("output schema must set additionalProperties to false".into());
    }
    let properties = object
        .get("properties")
        .and_then(Value::as_object)
        .filter(|properties| !properties.is_empty())
        .ok_or_else(|| "output schema properties must be a non-empty object".to_string())?;
    for (name, property) in properties {
        let property = property
            .as_object()
            .ok_or_else(|| format!("property {name} schema must be an object"))?;
        if property.len() != 1 || !property.contains_key("type") {
            return Err(format!(
                "property {name} schema supports only the type keyword"
            ));
        }
        let kind = property["type"]
            .as_str()
            .ok_or_else(|| format!("property {name} type must be a string"))?;
        if ![
            "string", "boolean", "integer", "number", "array", "object", "null",
        ]
        .contains(&kind)
        {
            return Err(format!("property {name} has unsupported type {kind}"));
        }
    }
    let required = object
        .get("required")
        .and_then(Value::as_array)
        .ok_or_else(|| "output schema required must be an array".to_string())?;
    let mut seen = std::collections::BTreeSet::new();
    for name in required {
        let name = name
            .as_str()
            .ok_or_else(|| "required entries must be strings".to_string())?;
        if !seen.insert(name) {
            return Err(format!("required contains duplicate property {name}"));
        }
        if !properties.contains_key(name) {
            return Err(format!("required property {name} is not declared"));
        }
    }
    Ok(())
}

fn validate_artifact_against_schema(schema: &Value, artifact: &Value) -> Result<(), String> {
    validate_object_schema(schema)?;
    let schema = schema.as_object().expect("validated schema object");
    let properties = schema["properties"]
        .as_object()
        .expect("validated properties");
    let required = schema["required"].as_array().expect("validated required");
    let artifact = artifact
        .as_object()
        .ok_or_else(|| "artifact must be a JSON object".to_string())?;
    for name in required.iter().filter_map(Value::as_str) {
        if !artifact.contains_key(name) {
            return Err(format!("artifact is missing required property {name}"));
        }
    }
    for (name, value) in artifact {
        let declaration = properties
            .get(name)
            .ok_or_else(|| format!("artifact contains undeclared property {name}"))?;
        let expected = declaration["type"]
            .as_str()
            .expect("validated property type");
        let matches = match expected {
            "string" => value.is_string(),
            "boolean" => value.is_boolean(),
            "integer" => value.as_i64().is_some(),
            "number" => value.as_f64().is_some_and(|number| number.is_finite()),
            "array" => value.is_array(),
            "object" => value.is_object(),
            "null" => value.is_null(),
            _ => unreachable!("schema types were validated"),
        };
        if !matches {
            return Err(format!(
                "artifact property {name} does not match {expected}"
            ));
        }
    }
    Ok(())
}

fn codex_exec_args(thread_id: Option<&str>, paths: &InvocationPaths) -> Vec<String> {
    let mut args = vec!["exec".to_string()];
    if let Some(thread_id) = thread_id {
        args.push("resume".into());
        args.push(thread_id.to_string());
    }
    args.extend([
        "--json".into(),
        "--output-last-message".into(),
        paths
            .output_last_message
            .to_str()
            .expect("validated output path must be Unicode")
            .to_string(),
        "--output-schema".into(),
        paths
            .output_schema
            .to_str()
            .expect("validated schema path must be Unicode")
            .to_string(),
        "-".into(),
    ]);
    args
}

fn parse_thread_started_jsonl(stdout: &[u8]) -> Result<String, String> {
    let text =
        std::str::from_utf8(stdout).map_err(|e| format!("JSONL stdout is not UTF-8: {e}"))?;
    if text.is_empty() || !text.ends_with('\n') {
        return Err(
            "JSONL stdout must end with a newline and contain a thread.started event".into(),
        );
    }

    let mut found: Option<String> = None;
    for (index, line) in text[..text.len() - 1].split('\n').enumerate() {
        if line.is_empty() {
            return Err(format!("JSONL record {} is empty", index + 1));
        }
        let event: Value = serde_json::from_str(line)
            .map_err(|e| format!("JSONL record {} is invalid JSON: {e}", index + 1))?;
        let object = event
            .as_object()
            .ok_or_else(|| format!("JSONL record {} must be an object", index + 1))?;
        let event_type = object
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("JSONL record {} has no string type", index + 1))?;
        match event_type {
            "thread.started" => {
                if found.is_some() {
                    return Err("JSONL contains duplicate thread.started events".into());
                }
                let id = object
                    .get("thread_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "thread.started has no string thread_id".to_string())?;
                found = Some(validate_thread_id(id)?);
            }
            _ if object.contains_key("thread_id") => {
                return Err(format!("unexpected thread_id in {event_type} event"));
            }
            _ => {}
        }
    }
    found.ok_or_else(|| "JSONL has no thread.started event".into())
}

fn validate_thread_id(value: &str) -> Result<String, String> {
    let valid = !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if !valid {
        return Err("thread_id has an invalid format".into());
    }
    Ok(value.to_string())
}

fn validate_thread_binding(expected: Option<&str>, observed: &str) -> Result<(), String> {
    let observed = validate_thread_id(observed)?;
    if let Some(expected) = expected {
        if expected != observed {
            return Err("observed thread_id does not match the requested resume thread".into());
        }
    }
    Ok(())
}

fn parse_final_reviewer_artifact(path: &Path) -> Result<CodexReviewVerdict, String> {
    let file = File::open(path)
        .map_err(|e| format!("failed to open dedicated output-last-message artifact: {e}"))?;
    let mut limited = file.take(MAX_ARTIFACT_BYTES + 1);
    let mut bytes = Vec::new();
    limited
        .read_to_end(&mut bytes)
        .map_err(|e| format!("failed to read dedicated output-last-message artifact: {e}"))?;
    if bytes.len() as u64 > MAX_ARTIFACT_BYTES {
        return Err("Reviewer artifact exceeds 1 MiB; result is non-approval".into());
    }
    serde_json::from_slice(&bytes)
        .map_err(|e| format!("Reviewer artifact failed CodexReviewVerdict validation: {e}"))
}

fn ensure_process_succeeded(
    result: &crate::orchestrator::types::ProcessResult,
) -> Result<(), String> {
    if result.timed_out || result.cancelled {
        return Err("process timed out or was cancelled; result is non-approval".into());
    }
    if result.stdout.is_truncated || result.stderr.is_truncated {
        return Err("process output was truncated; result is non-approval".into());
    }
    if result.exit_code != Some(0) {
        return Err("process exited unsuccessfully; result is non-approval".into());
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DispatchOutcome {
    DefinitelyNotDispatched,
    AmbiguousAfterDispatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ResumeRecovery {
    StartFreshWithCompleteArtifactBundle(Invocation),
    RecoveryRequiredNoResend,
}

fn resume_recovery(
    outcome: DispatchOutcome,
    complete_approved_bundle: &[u8],
    paths: &InvocationPaths,
) -> Result<ResumeRecovery, String> {
    match outcome {
        DispatchOutcome::DefinitelyNotDispatched => {
            if complete_approved_bundle.is_empty()
                || complete_approved_bundle.len() as u64 > MAX_ARTIFACT_BYTES
            {
                return Err("complete artifact bundle is empty or exceeds 1 MiB".into());
            }
            let bundle = std::str::from_utf8(complete_approved_bundle)
                .map_err(|e| format!("artifact bundle is not UTF-8: {e}"))?;
            let prompt = format!(
                "Resume planning in a fresh invocation using this complete approved artifact/finding bundle. Do not assume prior thread state.\n\n{bundle}"
            );
            Ok(ResumeRecovery::StartFreshWithCompleteArtifactBundle(
                initial_invocation(InvocationRole::Planner, &prompt, paths)?,
            ))
        }
        DispatchOutcome::AmbiguousAfterDispatch => Ok(ResumeRecovery::RecoveryRequiredNoResend),
    }
}

fn normalize_absolute(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("run-scoped paths must be absolute".into());
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                return Err("run-scoped paths must not contain parent traversal".into());
            }
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::types::{CapturedOutput, ProcessResult};
    use tempfile::tempdir;

    fn repo_test_temp_dir(prefix: &str) -> tempfile::TempDir {
        let repo_tmp = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-tmp");
        let _ = std::fs::create_dir_all(&repo_tmp);
        tempfile::Builder::new()
            .prefix(prefix)
            .tempdir_in(&repo_tmp)
            .expect("create repo test tempdir")
    }

    fn symlink_privilege_unavailable(error: &std::io::Error) -> bool {
        error.raw_os_error() == Some(1314)
    }

    fn paths(root: &Path) -> InvocationPaths {
        std::fs::create_dir_all(root.join("work")).unwrap();
        std::fs::create_dir_all(root.join("codex-home")).unwrap();
        std::fs::create_dir_all(root.join("artifacts")).unwrap();
        std::fs::create_dir_all(root.join("schemas")).unwrap();
        std::fs::write(
            root.join("schemas").join("review-verdict.json"),
            r#"{"type":"object","additionalProperties":false,"properties":{"verdict":{"type":"string"},"summary":{"type":"string"},"findings":{"type":"array"},"approval_binding":{"type":"object"}},"required":["verdict","summary","findings","approval_binding"]}"#,
        )
        .unwrap();
        InvocationPaths {
            run_root: root.to_path_buf(),
            working_dir: root.join("work"),
            config_home: root.join("codex-home"),
            output_last_message: root.join("artifacts").join("last-message.json"),
            output_schema: root.join("schemas").join("review-verdict.json"),
        }
    }

    #[cfg(windows)]
    async fn run_fixture_case(
        paths: &InvocationPaths,
        fixture_script: &Path,
        manifest_path: &Path,
        receipt: &Path,
        invocation_args: &[String],
        artifact_json: &str,
        synthetic_input: &str,
    ) -> ProcessResult {
        let _ = std::fs::remove_file(&paths.output_last_message);
        let _ = std::fs::remove_file(receipt);
        let manifest = serde_json::json!({
            "invocation_args": invocation_args,
            "working_dir": paths.working_dir,
            "config_home": paths.config_home,
            "expected_stdin": synthetic_input,
            "schema_path": paths.output_schema,
            "artifact_path": paths.output_last_message,
            "receipt_path": receipt,
            "artifact_json": artifact_json,
        });
        std::fs::write(manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let args = vec![
            "-NoProfile".to_string(),
            "-NonInteractive".to_string(),
            "-ExecutionPolicy".to_string(),
            "Bypass".to_string(),
            "-File".to_string(),
            fixture_script.to_str().unwrap().to_string(),
            manifest_path.to_str().unwrap().to_string(),
        ];
        ProcessRunner::new()
            .run_fixture_with_stdin(
                "powershell.exe",
                &args,
                Some(&paths.working_dir),
                Some(synthetic_input.as_bytes()),
                FIXTURE_TIMEOUT,
                None,
                &[
                    ("CODEX_HOME", paths.config_home.to_str().unwrap()),
                    ("TEMP", paths.run_root.to_str().unwrap()),
                    ("TMP", paths.run_root.to_str().unwrap()),
                ],
            )
            .await
            .unwrap()
    }

    #[cfg(windows)]
    fn create_test_junction(link: &Path, target: &Path) {
        use std::os::windows::ffi::OsStrExt;
        use std::ptr;

        #[link(name = "kernel32")]
        extern "system" {
            fn CreateFileW(
                name: *const u16,
                access: u32,
                share: u32,
                security: *const std::ffi::c_void,
                creation: u32,
                flags: u32,
                template: isize,
            ) -> isize;
            fn DeviceIoControl(
                handle: isize,
                code: u32,
                input: *const std::ffi::c_void,
                input_len: u32,
                output: *mut std::ffi::c_void,
                output_len: u32,
                returned: *mut u32,
                overlapped: *mut std::ffi::c_void,
            ) -> i32;
            fn CloseHandle(handle: isize) -> i32;
        }

        std::fs::create_dir(link).expect("create temporary junction directory");
        let link_w: Vec<u16> = link.as_os_str().encode_wide().chain([0]).collect();
        let handle = unsafe {
            CreateFileW(
                link_w.as_ptr(),
                0x4000_0000,
                0x0000_0007,
                ptr::null(),
                3,
                0x0020_0000 | 0x0200_0000,
                0,
            )
        };
        assert_ne!(handle, -1, "open temporary junction for reparse setup");
        let target_text = format!(
            "\\??\\{}",
            target
                .to_str()
                .expect("temporary target path must be Unicode")
        );
        let target_w: Vec<u16> = target_text.encode_utf16().collect();
        let name_bytes = (target_w.len() * 2) as u16;
        let data_length = 8u16 + name_bytes * 2 + 4;
        let mut buffer = Vec::<u8>::with_capacity(16 + data_length as usize);
        buffer.extend_from_slice(&0xA000_0003u32.to_le_bytes()); // IO_REPARSE_TAG_MOUNT_POINT
        buffer.extend_from_slice(&data_length.to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        buffer.extend_from_slice(&name_bytes.to_le_bytes());
        buffer.extend_from_slice(&(name_bytes + 2).to_le_bytes());
        buffer.extend_from_slice(&name_bytes.to_le_bytes());
        for unit in target_w
            .iter()
            .chain([0].iter())
            .chain(target_w.iter())
            .chain([0].iter())
        {
            buffer.extend_from_slice(&unit.to_le_bytes());
        }
        let mut returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                0x0009_00A4, // FSCTL_SET_REPARSE_POINT
                buffer.as_ptr().cast(),
                buffer.len() as u32,
                ptr::null_mut(),
                0,
                &mut returned,
                ptr::null_mut(),
            )
        };
        unsafe { CloseHandle(handle) };
        assert_ne!(ok, 0, "set temporary junction reparse point");
    }

    fn assert_tree_resolves_inside(root: &Path, path: &Path) {
        let canonical_root = root.canonicalize().unwrap();
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let entry_path = entry.path();
            let resolved = entry_path.canonicalize().unwrap();
            assert!(resolved.starts_with(&canonical_root));
            if entry.file_type().unwrap().is_dir() {
                assert_tree_resolves_inside(root, &entry_path);
            }
        }
    }

    fn process_result(stdout: &str) -> ProcessResult {
        ProcessResult {
            exit_code: Some(0),
            stdout: CapturedOutput {
                text: stdout.into(),
                is_truncated: false,
                total_bytes: stdout.len(),
            },
            stderr: CapturedOutput {
                text: String::new(),
                is_truncated: false,
                total_bytes: 0,
            },
            timed_out: false,
            cancelled: false,
            duration_ms: 1,
        }
    }

    const VALID_VERDICT: &str = r#"{"verdict":"APPROVE","summary":"synthetic fixture","findings":[],"approval_binding":{"target_type":"plan","root_plan_digest":null,"active_plan_digest":null,"candidate_plan_digest":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","allowlist_digest":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","policy_profile_digest":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"}}"#;

    #[test]
    fn builds_exact_initial_and_resume_argument_vectors_without_shell() {
        let dir = tempdir().unwrap();
        let planner_root = dir.path().join("planner-run");
        let invocation_paths = paths(&planner_root).canonicalized().unwrap();
        let initial = initial_invocation(
            InvocationRole::Planner,
            "synthetic prompt",
            &invocation_paths,
        )
        .unwrap();
        assert_eq!(initial.stdin, b"synthetic prompt");
        assert_eq!(initial.role, InvocationRole::Planner);
        assert_eq!(initial.scope, invocation_paths);
        assert_eq!(initial.thread_id, None);
        assert_eq!(initial.args[0], "exec");
        assert_eq!(initial.args[1], "--json");
        assert_eq!(initial.args[2], "--output-last-message");
        assert_eq!(
            initial.args[3],
            invocation_paths.output_last_message.to_str().unwrap()
        );
        assert_eq!(initial.args[4], "--output-schema");
        assert_eq!(
            initial.args[5],
            invocation_paths.output_schema.to_str().unwrap()
        );
        assert_eq!(initial.args[6], "-");
        assert!(initial
            .args
            .iter()
            .all(|arg| !arg.contains("&&") && !arg.contains("|")));

        let resumed = resume_planner_invocation(&initial, "thread-abc.123_X", "continue").unwrap();
        assert_eq!(&resumed.args[..3], ["exec", "resume", "thread-abc.123_X"]);
        assert_eq!(resumed.thread_id.as_deref(), Some("thread-abc.123_X"));
        assert_eq!(resumed.role, InvocationRole::Planner);
        assert_eq!(resumed.scope, initial.scope);
        assert!(validate_thread_binding(resumed.thread_id.as_deref(), "thread-abc.123_X").is_ok());
        assert!(validate_thread_binding(resumed.thread_id.as_deref(), "thread-other").is_err());
        let reviewer_root = dir.path().join("reviewer-run");
        let reviewer_paths = paths(&reviewer_root);
        let reviewer = reviewer_invocation(&initial, "review", &reviewer_paths).unwrap();
        assert_eq!(reviewer.thread_id, None);
        assert_ne!(reviewer.scope.config_home, resumed.scope.config_home);
        assert_ne!(reviewer.scope.working_dir, resumed.scope.working_dir);
        assert!(reviewer_invocation(&initial, "", &reviewer_paths).is_err());
        let nested_root = planner_root.join("nested-reviewer");
        let nested_paths = paths(&nested_root);
        assert!(reviewer_invocation(&initial, "review", &nested_paths).is_err());
        let mut shared_config = paths(&dir.path().join("another-reviewer"));
        shared_config.config_home = initial.scope.config_home.clone();
        assert!(reviewer_invocation(&initial, "review", &shared_config).is_err());
        assert!(resume_planner_invocation(&reviewer, "thread-x", "review").is_err());
    }

    #[test]
    fn validates_run_paths_and_rejects_escape_or_relative_paths() {
        let dir = tempdir().unwrap();
        let mut invalid = paths(dir.path());
        invalid.output_last_message = dir.path().join("..").join("outside.json");
        assert!(invalid.validate().is_err());
        invalid.output_last_message = PathBuf::from("relative.json");
        assert!(invalid.validate().is_err());

        let mut invalid = paths(dir.path());
        invalid.working_dir = PathBuf::from("relative-work");
        assert!(invalid.validate().is_err());

        let missing_schema = paths(dir.path());
        std::fs::remove_file(&missing_schema.output_schema).unwrap();
        assert!(missing_schema.validate().is_err());

        let invalid_schema = paths(dir.path());
        std::fs::write(&invalid_schema.output_schema, "not-json").unwrap();
        assert!(invalid_schema.validate().is_err());

        let non_object_schema = paths(dir.path());
        std::fs::write(&non_object_schema.output_schema, r#"{"type":"string"}"#).unwrap();
        assert!(non_object_schema.validate().is_err());
    }

    #[test]
    fn validates_only_the_declared_object_schema_subset_and_artifact_shape() {
        let schema: Value = serde_json::from_str(
            r#"{"type":"object","additionalProperties":false,"required":["s","b","i","n","a","o","z"],"properties":{"s":{"type":"string"},"b":{"type":"boolean"},"i":{"type":"integer"},"n":{"type":"number"},"a":{"type":"array"},"o":{"type":"object"},"z":{"type":"null"}}}"#,
        ).unwrap();
        validate_object_schema(&schema).unwrap();
        let valid: Value = serde_json::from_str(
            r#"{"s":"text","b":true,"i":1,"n":1.5,"a":[1],"o":{"nested":false},"z":null}"#,
        )
        .unwrap();
        validate_artifact_against_schema(&schema, &valid).unwrap();

        for integral in [
            "0",
            "1",
            "-2",
            "9223372036854775807",
            "-9223372036854775808",
        ] {
            let artifact: Value = serde_json::from_str(&format!(
                r#"{{"s":"text","b":true,"i":{integral},"n":1.5,"a":[],"o":{{}},"z":null}}"#
            ))
            .unwrap();
            validate_artifact_against_schema(&schema, &artifact)
                .unwrap_or_else(|error| panic!("rejected signed i64 token {integral}: {error}"));
        }
        for non_integer in [
            "1.0",
            "1e0",
            "10e-1",
            "1.00000000000000001",
            "1.2",
            "-0.5",
            "9223372036854775808",
            "-9223372036854775809",
        ] {
            let artifact: Value = serde_json::from_str(&format!(
                r#"{{"s":"text","b":true,"i":{non_integer},"n":1.5,"a":[],"o":{{}},"z":null}}"#
            ))
            .unwrap();
            assert!(
                validate_artifact_against_schema(&schema, &artifact).is_err(),
                "accepted non-i64 integer representation {non_integer}"
            );
        }
        let integral_number: Value =
            serde_json::from_str(r#"{"s":"text","b":true,"i":1,"n":1e0,"a":[],"o":{},"z":null}"#)
                .unwrap();
        validate_artifact_against_schema(&schema, &integral_number).unwrap();
        let fractional_number: Value =
            serde_json::from_str(r#"{"s":"text","b":true,"i":1,"n":1.5,"a":[],"o":{},"z":null}"#)
                .unwrap();
        validate_artifact_against_schema(&schema, &fractional_number).unwrap();

        for invalid_schema in [
            r#"{"type":"object","additionalProperties":false,"required":[],"properties":{"x":{"type":"string"}},"description":"unsupported"}"#,
            r#"{"type":"object","additionalProperties":false,"required":["x","x"],"properties":{"x":{"type":"string"}}}"#,
            r#"{"type":"object","additionalProperties":false,"required":["missing"],"properties":{"x":{"type":"string"}}}"#,
            r#"{"type":"object","additionalProperties":true,"required":[],"properties":{"x":{"type":"string"}}}"#,
            r#"{"type":"object","additionalProperties":false,"required":[],"properties":{}}"#,
            r#"{"type":"object","additionalProperties":false,"required":[],"properties":{"x":{"type":"string","description":"no"}}}"#,
            r#"{"type":"object","additionalProperties":false,"required":[],"properties":{"x":{"type":"unknown"}}}"#,
            r#"{"type":"object","additionalProperties":false,"required":"x","properties":{"x":{"type":"string"}}}"#,
        ] {
            let invalid: Value = serde_json::from_str(invalid_schema).unwrap();
            assert!(
                validate_object_schema(&invalid).is_err(),
                "accepted {invalid_schema}"
            );
        }

        for invalid_artifact in [
            r#"{"s":"text","b":true,"i":1,"n":1.5,"a":[],"o":{},"z":null,"extra":1}"#,
            r#"{"s":"text","b":true,"i":1,"n":1.5,"a":[],"o":{}}"#,
            r#"{"s":false,"b":true,"i":1,"n":1.5,"a":[],"o":{},"z":null}"#,
            r#"{"s":"text","b":true,"i":1.2,"n":1.5,"a":[],"o":{},"z":null}"#,
        ] {
            let artifact: Value = serde_json::from_str(invalid_artifact).unwrap();
            assert!(validate_artifact_against_schema(&schema, &artifact).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn canonicalizes_in_root_working_directory_aliases() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let canonical = paths(dir.path());
        let alias = dir.path().join("work-alias");
        symlink(dir.path().join("work"), &alias).unwrap();
        let mut aliased = canonical.clone();
        aliased.working_dir = alias;
        let regular_invocation =
            initial_invocation(InvocationRole::Planner, "x", &canonical).unwrap();
        let alias_invocation = initial_invocation(InvocationRole::Planner, "x", &aliased).unwrap();
        assert_eq!(
            alias_invocation.scope.working_dir,
            regular_invocation.scope.working_dir
        );
    }

    #[cfg(windows)]
    #[test]
    fn canonicalizes_in_root_junction_working_directory_aliases() {
        let dir = tempdir().unwrap();
        let canonical = paths(dir.path());
        let alias = dir.path().join("work-junction");
        create_test_junction(&alias, &dir.path().join("work"));
        let mut aliased = canonical.clone();
        aliased.working_dir = alias.clone();
        let regular_invocation =
            initial_invocation(InvocationRole::Planner, "x", &canonical).unwrap();
        let alias_invocation = initial_invocation(InvocationRole::Planner, "x", &aliased).unwrap();
        assert_eq!(
            alias_invocation.scope.working_dir,
            regular_invocation.scope.working_dir
        );
        std::fs::remove_dir(alias).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn rejects_non_unicode_windows_paths_instead_of_lossy_conversion() {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};

        let dir = tempdir().unwrap();
        let mut invalid = paths(dir.path());
        let mut wide: Vec<u16> = invalid.output_schema.as_os_str().encode_wide().collect();
        wide.push(0xD800); // Unpaired surrogate: valid OsString, not Unicode.
        invalid.output_schema = PathBuf::from(std::ffi::OsString::from_wide(&wide));
        assert!(invalid
            .validate()
            .unwrap_err()
            .contains("not representable as Unicode"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_unicode_paths_instead_of_lossy_conversion() {
        use std::os::unix::ffi::OsStringExt;

        let dir = tempdir().unwrap();
        let mut invalid = paths(dir.path());
        let mut bytes = invalid
            .output_schema
            .as_os_str()
            .as_encoded_bytes()
            .to_vec();
        bytes.push(0xFF);
        invalid.output_schema = PathBuf::from(std::ffi::OsString::from_vec(bytes));
        assert!(invalid
            .validate()
            .unwrap_err()
            .contains("not representable as Unicode"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape_from_run_scoped_root() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let mut scoped = paths(dir.path());
        let link = dir.path().join("escape");
        symlink(outside.path(), &link).unwrap();
        scoped.output_last_message = link.join("artifact.json");
        assert!(scoped.validate().is_err());
    }

    #[cfg(windows)]
    #[test]
    fn rejects_symbolic_link_escape_from_run_scoped_root() {
        use std::os::windows::ffi::OsStrExt;

        #[link(name = "kernel32")]
        extern "system" {
            fn CreateSymbolicLinkW(link: *const u16, target: *const u16, flags: u32) -> u8;
        }

        let dir = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let mut scoped = paths(dir.path());
        let link = dir.path().join("escape");
        let link_w: Vec<u16> = link.as_os_str().encode_wide().chain([0]).collect();
        let target_w: Vec<u16> = outside
            .path()
            .as_os_str()
            .encode_wide()
            .chain([0])
            .collect();
        let created = unsafe { CreateSymbolicLinkW(link_w.as_ptr(), target_w.as_ptr(), 0x1 | 0x2) };
        if created == 0 {
            let err = std::io::Error::last_os_error();
            if symlink_privilege_unavailable(&err) {
                eprintln!("[UNVERIFIED] symlink privilege unavailable (OS 1314)");
                return;
            }
            panic!("the isolated temp-path symlink fixture must be created: {err}");
        }
        scoped.output_last_message = link.join("artifact.json");
        assert!(scoped.validate().is_err());
    }

    #[cfg(windows)]
    #[test]
    fn rejects_junction_reparse_point_escape_from_run_scoped_root() {
        let dir = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let mut scoped = paths(dir.path());
        let junction = dir.path().join("outside-junction");
        create_test_junction(&junction, outside.path());
        assert!(std::fs::symlink_metadata(&junction)
            .unwrap()
            .file_type()
            .is_symlink());
        scoped.output_last_message = junction.join("artifact.json");
        assert!(scoped.canonicalized().is_err());
        std::fs::remove_dir(junction).unwrap();
    }

    #[test]
    fn parses_only_one_valid_thread_started_event_from_strict_jsonl() {
        let valid = b"{\"type\":\"item.started\"}\n{\"type\":\"thread.started\",\"thread_id\":\"thread-123\"}\n";
        assert_eq!(parse_thread_started_jsonl(valid).unwrap(), "thread-123");
        for invalid in [
            b"{\"type\":\"item.started\"}\n".as_slice(),
            b"{\"type\":\"thread.started\"}\n".as_slice(),
            b"{\"type\":\"thread.started\",\"thread_id\":\"one\"}\n{\"type\":\"thread.started\",\"thread_id\":\"two\"}\n".as_slice(),
            b"{\"type\":\"item.started\",\"thread_id\":\"unexpected\"}\n".as_slice(),
            b"not-json\n".as_slice(),
            b"{\"type\":\"thread.started\",\"thread_id\":\"thread-1\"}".as_slice(),
            b"\xff\n".as_slice(),
        ] {
            assert!(parse_thread_started_jsonl(invalid).is_err());
        }
    }

    #[test]
    fn reads_final_artifact_only_from_output_path_and_validates_typed_verdict() {
        let dir = tempdir().unwrap();
        let output = dir.path().join("last-message.json");
        std::fs::write(&output, VALID_VERDICT).unwrap();
        let verdict = parse_final_reviewer_artifact(&output).unwrap();
        assert_eq!(verdict.summary, "synthetic fixture");

        std::fs::write(&output, "{}\nprogress text").unwrap();
        assert!(parse_final_reviewer_artifact(&output).is_err());
        std::fs::write(&output, vec![b' '; MAX_ARTIFACT_BYTES as usize + 1]).unwrap();
        assert!(parse_final_reviewer_artifact(&output)
            .unwrap_err()
            .contains("exceeds 1 MiB"));
        assert!(parse_final_reviewer_artifact(&dir.path().join("missing.json")).is_err());
    }

    #[test]
    fn failures_and_ambiguous_resume_never_become_approval_or_automatic_resend() {
        let dir = tempdir().unwrap();
        let invocation_paths = paths(dir.path());
        let mut result = process_result("{}\n");
        assert!(ensure_process_succeeded(&result).is_ok());
        result.exit_code = Some(1);
        assert!(ensure_process_succeeded(&result).is_err());
        result.exit_code = Some(0);
        result.stdout.is_truncated = true;
        assert!(ensure_process_succeeded(&result).is_err());
        result.stdout.is_truncated = false;
        result.timed_out = true;
        assert!(ensure_process_succeeded(&result).is_err());

        let bundle = br#"{"approved_plan":"complete","findings":["all"]}"#;
        let recovered = resume_recovery(
            DispatchOutcome::DefinitelyNotDispatched,
            bundle,
            &invocation_paths,
        )
        .unwrap();
        let ResumeRecovery::StartFreshWithCompleteArtifactBundle(fresh) = recovered else {
            panic!("definitely-not-dispatched must use bounded artifact fallback");
        };
        assert_eq!(fresh.role, InvocationRole::Planner);
        assert_eq!(fresh.thread_id, None);
        assert_eq!(fresh.args[0], "exec");
        assert!(String::from_utf8(fresh.stdin)
            .unwrap()
            .ends_with(std::str::from_utf8(bundle).unwrap()));
        assert_eq!(
            resume_recovery(
                DispatchOutcome::AmbiguousAfterDispatch,
                bundle,
                &invocation_paths,
            )
            .unwrap(),
            ResumeRecovery::RecoveryRequiredNoResend
        );
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn local_child_fixture_uses_process_runner_and_leaves_only_temp_files() {
        let dir = repo_test_temp_dir("child-fixture");
        let paths = paths(dir.path());
        let fixture_script = dir.path().join("fixture-child.ps1");
        let receipt = dir.path().join("received-args.json");
        let synthetic_input = "synthetic local-only input";
        let invocation =
            initial_invocation(InvocationRole::Planner, synthetic_input, &paths).unwrap();
        let manifest_path = dir.path().join("fixture-manifest.json");
        let fixture = format!(
            r#"
$ErrorActionPreference = 'Stop'
trap {{ [Console]::Error.WriteLine(('FIXTURE_ERROR line={{0}} message={{1}}' -f $_.InvocationInfo.ScriptLineNumber, $_.Exception.Message)); exit 99 }}
function Test-OrdinalEqual([object]$Left, [object]$Right) {{
  if ($null -eq $Left -or $null -eq $Right) {{ return $null -eq $Left -and $null -eq $Right }}
  return [string]::Equals([string]$Left, [string]$Right, [StringComparison]::Ordinal)
}}
function Test-OrdinalContains([object[]]$Items, [string]$Needle) {{
  foreach ($item in $Items) {{ if (Test-OrdinalEqual $item $Needle) {{ return $true }} }}
  return $false
}}
function Get-ExactProperty([object]$Object, [string]$Name) {{
  foreach ($property in $Object.PSObject.Properties) {{ if (Test-OrdinalEqual $property.Name $Name) {{ return $property }} }}
  return $null
}}
function Test-JsonKind([object]$Value, [string]$Kind) {{
  if (Test-OrdinalEqual $Kind 'string') {{ return $Value -is [string] }}
  if (Test-OrdinalEqual $Kind 'boolean') {{ return $Value -is [bool] }}
  if (Test-OrdinalEqual $Kind 'array') {{ return $Value -is [array] }}
  if (Test-OrdinalEqual $Kind 'object') {{ return $Value -is [System.Management.Automation.PSCustomObject] }}
  if (Test-OrdinalEqual $Kind 'null') {{ return $null -eq $Value }}
  $numericTypes = @('System.Byte','System.SByte','System.Int16','System.UInt16','System.Int32','System.UInt32','System.Int64','System.UInt64','System.Decimal','System.Single','System.Double')
  if ($null -eq $Value -or -not (Test-OrdinalContains $numericTypes $Value.GetType().FullName)) {{ return $false }}
  $typeName = $Value.GetType().FullName
  if (Test-OrdinalEqual $Kind 'number') {{
    if (Test-OrdinalEqual $typeName 'System.Double') {{ return -not [double]::IsNaN([double]$Value) -and -not [double]::IsInfinity([double]$Value) }}
    if (Test-OrdinalEqual $typeName 'System.Single') {{ return -not [single]::IsNaN([single]$Value) -and -not [single]::IsInfinity([single]$Value) }}
    return $true
  }}
  if (-not (Test-OrdinalEqual $Kind 'integer')) {{ return $false }}
  $integerTypes = @('System.Byte','System.SByte','System.Int16','System.UInt16','System.Int32','System.UInt32','System.Int64')
  return Test-OrdinalContains $integerTypes $typeName
}}
$manifestPath = $args[0]
if ($args.Count -ne 1) {{ exit 2 }}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$expectedCwd = $manifest.working_dir
$expectedHome = $manifest.config_home
$expectedInput = $manifest.expected_stdin
$receipt = $manifest.receipt_path
$schemaPath = $manifest.schema_path
$artifactPath = $manifest.artifact_path
$argList = @($manifest.invocation_args)
if (-not [string]::Equals(([IO.Path]::GetFullPath((Get-Location).ProviderPath).TrimEnd('\')), ([IO.Path]::GetFullPath($expectedCwd).TrimEnd('\')), [StringComparison]::OrdinalIgnoreCase)) {{ [Console]::Error.WriteLine(('cwd mismatch actual={{0}} expected={{1}}' -f (Get-Location).ProviderPath, $expectedCwd)); exit 3 }}
if ([Console]::In.ReadToEnd() -ne $expectedInput) {{ exit 4 }}
if ($env:CODEX_HOME -ne $expectedHome) {{ exit 5 }}
$expectedArgs = @('exec', '--json', '--output-last-message', [string]$artifactPath, '--output-schema', [string]$schemaPath, '-')
if ($argList.Count -ne $expectedArgs.Count) {{ exit 6 }}
for ($index = 0; $index -lt $expectedArgs.Count; $index++) {{
  if (-not [string]::Equals([string]$argList[$index], [string]$expectedArgs[$index], [StringComparison]::Ordinal)) {{ exit 6 }}
}}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$expectedSchemaKeys = @('additionalProperties', 'properties', 'required', 'type')
$schemaProperties = @($schema.PSObject.Properties)
if ($schemaProperties.Count -ne $expectedSchemaKeys.Count) {{ exit 7 }}
foreach ($property in $schemaProperties) {{ if (-not (Test-OrdinalContains $expectedSchemaKeys $property.Name)) {{ exit 7 }} }}
$schemaType = Get-ExactProperty $schema 'type'
$schemaAdditional = Get-ExactProperty $schema 'additionalProperties'
$schemaDefinitions = Get-ExactProperty $schema 'properties'
$schemaRequired = Get-ExactProperty $schema 'required'
if ($null -eq $schemaType -or -not (Test-OrdinalEqual ([string]$schemaType.Value) 'object') -or $null -eq $schemaAdditional -or $schemaAdditional.Value -isnot [bool] -or $schemaAdditional.Value -ne $false) {{ exit 7 }}
if ($null -eq $schemaDefinitions -or $null -eq $schemaRequired -or $schemaRequired.Value -isnot [array]) {{ exit 8 }}
$propertyEntries = @($schemaDefinitions.Value.PSObject.Properties)
if ($propertyEntries.Count -eq 0) {{ exit 8 }}
$propertyNames = @($propertyEntries | ForEach-Object {{ $_.Name }})
$seenRequired = [System.Collections.Generic.List[string]]::new()
foreach ($requiredName in $schemaRequired.Value) {{
  if ($requiredName -isnot [string] -or (Test-OrdinalContains $seenRequired $requiredName) -or -not (Test-OrdinalContains $propertyNames $requiredName)) {{ exit 9 }}
  $seenRequired.Add($requiredName)
}}
$allowedTypes = @('string','boolean','integer','number','array','object','null')
foreach ($entry in $propertyEntries) {{
  $definitionProperties = @($entry.Value.PSObject.Properties)
  if ($definitionProperties.Count -ne 1 -or -not (Test-OrdinalEqual $definitionProperties[0].Name 'type') -or $definitionProperties[0].Value -isnot [string] -or -not (Test-OrdinalContains $allowedTypes $definitionProperties[0].Value)) {{ exit 11 }}
}}
$artifact = $manifest.artifact_json | ConvertFrom-Json
$artifactProperties = @($artifact.PSObject.Properties)
$artifactNames = @($artifactProperties | ForEach-Object {{ $_.Name }})
foreach ($name in $artifactNames) {{ if (-not (Test-OrdinalContains $propertyNames $name)) {{ exit 12 }} }}
foreach ($name in $schemaRequired.Value) {{ if (-not (Test-OrdinalContains $artifactNames $name)) {{ exit 13 }} }}
foreach ($artifactProperty in $artifactProperties) {{
  $definition = Get-ExactProperty $schemaDefinitions.Value $artifactProperty.Name
  $typeProperty = Get-ExactProperty $definition.Value 'type'
  if ($null -eq $typeProperty -or -not (Test-JsonKind $artifactProperty.Value ([string]$typeProperty.Value))) {{ exit 14 }}
}}
[System.IO.File]::WriteAllText($artifactPath, ($artifact | ConvertTo-Json -Depth 20 -Compress))
[System.IO.File]::WriteAllText($receipt, (ConvertTo-Json -InputObject $argList -Compress))
[Console]::Out.WriteLine('{{"type":"thread.started","thread_id":"thread-fixture-1"}}')
"#,
        );
        std::fs::write(&fixture_script, fixture).unwrap();
        let valid_schema = std::fs::read_to_string(&invocation.scope.output_schema).unwrap();
        let valid_artifact = r#"{"verdict":"APPROVE","summary":"synthetic fixture","findings":[],"approval_binding":{"target_type":"plan","root_plan_digest":null,"active_plan_digest":null,"candidate_plan_digest":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","allowlist_digest":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","policy_profile_digest":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"}}"#;
        let process = run_fixture_case(
            &invocation.scope,
            &fixture_script,
            &manifest_path,
            &receipt,
            &invocation.args,
            valid_artifact,
            synthetic_input,
        )
        .await;
        ensure_process_succeeded(&process).unwrap_or_else(|error| {
            panic!(
                "{error}; exit={:?}; stdout={}; stderr={}",
                process.exit_code, process.stdout.text, process.stderr.text
            )
        });
        assert_eq!(
            parse_thread_started_jsonl(process.stdout.text.as_bytes()).unwrap(),
            "thread-fixture-1"
        );
        assert_eq!(
            parse_final_reviewer_artifact(&invocation.scope.output_last_message)
                .unwrap()
                .summary,
            "synthetic fixture"
        );
        let receipt_bytes = std::fs::read(&receipt).unwrap();
        let receipt_text_storage = String::from_utf8_lossy(&receipt_bytes);
        let receipt_text = receipt_text_storage.trim_start_matches('\u{feff}').trim();
        let received_args: Vec<String> = serde_json::from_str(receipt_text).unwrap();
        assert_eq!(received_args, invocation.args);
        assert!(paths.output_schema.is_file());
        assert_tree_resolves_inside(dir.path(), dir.path());

        let case_mutations = [
            (
                "uppercase top-level schema key",
                valid_schema.replace("\"type\"", "\"TYPE\""),
                valid_artifact.to_string(),
            ),
            (
                "uppercase schema type label",
                valid_schema.replace("\"object\"", "\"OBJECT\""),
                valid_artifact.to_string(),
            ),
            (
                "case-changed schema property name",
                valid_schema.replacen("\"verdict\"", "\"Verdict\"", 1),
                valid_artifact.to_string(),
            ),
            (
                "case-changed required property name",
                valid_schema.replace("\"required\":[\"verdict\"", "\"required\":[\"VERDICT\""),
                valid_artifact.to_string(),
            ),
            (
                "uppercase property schema keyword",
                valid_schema.replace("\"type\":\"string\"", "\"Type\":\"string\""),
                valid_artifact.to_string(),
            ),
            (
                "uppercase property type label",
                valid_schema.replace("\"string\"", "\"STRING\""),
                valid_artifact.to_string(),
            ),
            (
                "case-changed artifact name",
                valid_schema.clone(),
                valid_artifact.replace("\"verdict\"", "\"Verdict\""),
            ),
        ];
        for (description, schema_text, artifact_text) in case_mutations {
            std::fs::write(&invocation.scope.output_schema, schema_text).unwrap();
            let child_args = codex_exec_args(None, &invocation.scope);
            let child_result = run_fixture_case(
                &invocation.scope,
                &fixture_script,
                &manifest_path,
                &receipt,
                &child_args,
                &artifact_text,
                synthetic_input,
            )
            .await;
            assert_ne!(
                child_result.exit_code,
                Some(0),
                "PowerShell child accepted {description}"
            );
            assert!(!invocation.scope.output_last_message.exists());
            assert!(!receipt.exists());
        }

        std::fs::write(&invocation.scope.output_schema, &valid_schema).unwrap();
        for (index, description) in ["wrong artifact argv path", "wrong schema argv path"]
            .into_iter()
            .enumerate()
        {
            let mut child_args = codex_exec_args(None, &invocation.scope);
            child_args[if index == 0 { 3 } else { 5 }] = dir
                .path()
                .join(format!("wrong-path-{index}.json"))
                .to_str()
                .unwrap()
                .to_string();
            let child_result = run_fixture_case(
                &invocation.scope,
                &fixture_script,
                &manifest_path,
                &receipt,
                &child_args,
                valid_artifact,
                synthetic_input,
            )
            .await;
            assert_ne!(
                child_result.exit_code,
                Some(0),
                "PowerShell child accepted {description}"
            );
            assert!(!invocation.scope.output_last_message.exists());
            assert!(!receipt.exists());
        }

        let numeric_schema = r#"{"type":"object","additionalProperties":false,"required":["i","n"],"properties":{"i":{"type":"integer"},"n":{"type":"number"}}}"#;
        let numeric_cases = [
            ("integer token", "1", "1.5", true),
            ("negative integer token", "-2", "1.5", true),
            ("i64 maximum", "9223372036854775807", "1.5", true),
            ("i64 minimum", "-9223372036854775808", "1.5", true),
            ("integral decimal", "1.0", "1.5", false),
            ("integral exponent", "1e0", "1.5", false),
            ("integral exponent decimal", "10e-1", "1.5", false),
            ("negative integral decimal", "-2.0", "1.5", false),
            (
                "precision-sensitive fraction",
                "1.00000000000000001",
                "1.5",
                false,
            ),
            ("fractional integer", "1.2", "1.5", false),
            ("negative fractional integer", "-0.5", "1.5", false),
            ("i64 overflow", "9223372036854775808", "1.5", false),
            ("i64 underflow", "-9223372036854775809", "1.5", false),
            ("integral number representation", "1", "1e0", true),
        ];
        std::fs::write(&invocation.scope.output_schema, numeric_schema).unwrap();
        for (description, integer_json, number_json, should_pass) in numeric_cases {
            let numeric_artifact = format!(r#"{{"i":{integer_json},"n":{number_json}}}"#);
            let child_args = codex_exec_args(None, &invocation.scope);
            let child_result = run_fixture_case(
                &invocation.scope,
                &fixture_script,
                &manifest_path,
                &receipt,
                &child_args,
                &numeric_artifact,
                synthetic_input,
            )
            .await;
            assert_eq!(
                child_result.exit_code == Some(0),
                should_pass,
                "unexpected PowerShell integer result for {description}: exit={:?}, stderr={}",
                child_result.exit_code,
                child_result.stderr.text
            );
            assert_eq!(invocation.scope.output_last_message.exists(), should_pass);
            assert_eq!(receipt.exists(), should_pass);
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_process_argument_encoder_round_trips_the_exact_generated_codex_vector() {
        let root = PathBuf::from(r"C:\temporary run root");
        let paths = InvocationPaths {
            run_root: root.clone(),
            working_dir: root.join("work space"),
            config_home: root.join("codex home"),
            output_last_message: root.join("artifacts").join("last message.json"),
            output_schema: root.join("schemas").join("review schema.json"),
        };
        let args = codex_exec_args(None, &paths);
        assert_eq!(
            crate::orchestrator::process_runner::round_trip_windows_args_for_test(&args),
            args
        );
    }
}
